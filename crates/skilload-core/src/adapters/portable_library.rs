use crate::adapters::xdg::{SystemEnvironment, XdgRootResolver};
use crate::domain::configuration::NativePath;
use crate::domain::library::{
    MAX_PORTABLE_LIBRARY_DOCUMENT_BYTES, MAX_PORTABLE_LIBRARY_ENTRIES, PortableLibraryDocument,
};
use crate::error::AppError;
use crate::ports::configuration::{Environment, ResolvedRoots, StateRootResolver};
use crate::ports::library::LibraryTransferStore;
use rustix::fs::{AtFlags, FileType, RenameFlags, fstat, linkat, renameat_with, statat, unlinkat};
use std::collections::HashSet;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tempfile::{Builder, NamedTempFile};

const MAX_IMPORT_VALUES: u64 = 1_000_000;
const MAX_IMPORT_DEPTH: u64 = 8;
const MAX_IMPORT_STRING_BYTES: u64 = 1_048_576;
const MAX_IMPORT_NUMBER_BYTES: u64 = 128;
const INPUT_SCAN_BUFFER_BYTES: usize = 64 * 1024;

pub struct PortableLibraryTransferStore {
    environment: Arc<dyn Environment>,
    root_resolver: Arc<dyn StateRootResolver>,
    write_hooks: Arc<dyn TransferWriteHooks>,
}

impl PortableLibraryTransferStore {
    pub fn new() -> Self {
        Self {
            environment: Arc::new(SystemEnvironment),
            root_resolver: Arc::new(XdgRootResolver),
            write_hooks: Arc::new(NoopTransferWriteHooks),
        }
    }

    pub fn with_environment(
        environment: Arc<dyn Environment>,
        root_resolver: Arc<dyn StateRootResolver>,
    ) -> Self {
        Self {
            environment,
            root_resolver,
            write_hooks: Arc::new(NoopTransferWriteHooks),
        }
    }

    #[cfg(test)]
    fn with_write_hooks(
        environment: Arc<dyn Environment>,
        root_resolver: Arc<dyn StateRootResolver>,
        write_hooks: Arc<dyn TransferWriteHooks>,
    ) -> Self {
        Self {
            environment,
            root_resolver,
            write_hooks,
        }
    }

    fn resolve_roots(&self) -> Result<ResolvedRoots, AppError> {
        self.root_resolver.resolve(self.environment.as_ref())
    }

    fn read_input(&self, input: &NativePath) -> Result<Vec<u8>, AppError> {
        let file = open_regular_input(input, || {})?;
        JsonScanner::from_reader(file, input).scan()
    }

    fn write_document(
        &self,
        output: &NativePath,
        document: &PortableLibraryDocument,
    ) -> Result<(), AppError> {
        let roots = self.resolve_roots()?;
        let output_path = output.as_path();
        let parent = validated_output_parent(output_path, output)?;
        reject_protected_output(output_path, &parent.path, &roots, output)?;
        ensure_regular_output(output_path, output)?;

        let bytes = document.serialize_for_transfer()?;

        let mut staging = Builder::new()
            .prefix(".skilload-library-")
            .suffix(".tmp")
            .tempfile_in(&parent.path)
            .map_err(|error| export_io(output, "create export staging file", error))?;
        parent.revalidate(output)?;
        staging
            .as_file()
            .set_permissions(fs::Permissions::from_mode(0o600))
            .map_err(|error| export_io(output, "restrict export staging file", error))?;
        staging
            .write_all(&bytes)
            .map_err(|error| export_io(output, "write export staging file", error))?;
        staging
            .as_file()
            .sync_all()
            .map_err(|error| export_io(output, "sync export staging file", error))?;
        self.write_hooks.before_rename()?;

        let roots = self.root_resolver.revalidate(&roots)?;
        parent.revalidate(output)?;
        reject_protected_output(output_path, &parent.path, &roots, output)?;
        ensure_regular_output(output_path, output)?;
        parent.revalidate(output)?;
        let expected_output = observe_output_target(output_path, output)?;
        self.write_hooks
            .after_final_output_validation_before_publish(&parent.path)?;
        publish_staging(
            &mut staging,
            &parent,
            &roots,
            expected_output,
            output_path,
            output,
            PublishStagingHooks {
                after_identity_check: || {
                    self.write_hooks
                        .after_staging_identity_check_before_publish(&parent.path)
                },
                after_publication_link_before_rename: || {
                    self.write_hooks
                        .after_publication_link_before_rename(&parent.path)
                },
                after_publication_identity_check_before_exchange: || {
                    self.write_hooks
                        .after_publication_identity_check_before_exchange(&parent.path)
                },
                after_existing_output_exchange_before_cleanup: || {
                    self.write_hooks
                        .after_existing_output_exchange_before_cleanup(&parent.path)
                },
            },
        )?;
        self.write_hooks.after_rename_before_parent_sync()?;
        parent.revalidate(output)?;
        parent
            .directory
            .sync_all()
            .map_err(|error| export_io(output, "sync export parent directory", error))?;
        parent.revalidate(output)?;
        verify_staging_identity(
            &staging,
            &parent,
            output_path.file_name().ok_or_else(|| {
                AppError::validation("library_export_output_has_no_name", Some(output.clone()))
            })?,
            output,
        )?;
        Ok(())
    }
}

impl Default for PortableLibraryTransferStore {
    fn default() -> Self {
        Self::new()
    }
}

impl LibraryTransferStore for PortableLibraryTransferStore {
    fn read_import(&self, input: &NativePath) -> Result<PortableLibraryDocument, AppError> {
        let bytes = self.read_input(input)?;
        serde_json::from_slice(&bytes)
            .map_err(|_| AppError::validation("library_import_schema", Some(input.clone())))
    }

    fn write_export(
        &self,
        output: &NativePath,
        document: &PortableLibraryDocument,
    ) -> Result<(), AppError> {
        self.write_document(output, document)
    }
}

trait TransferWriteHooks: Send + Sync {
    fn before_rename(&self) -> Result<(), AppError> {
        Ok(())
    }

    fn after_final_output_validation_before_publish(
        &self,
        _output_parent: &Path,
    ) -> Result<(), AppError> {
        Ok(())
    }

    fn after_staging_identity_check_before_publish(
        &self,
        _output_parent: &Path,
    ) -> Result<(), AppError> {
        Ok(())
    }

    fn after_publication_link_before_rename(&self, _output_parent: &Path) -> Result<(), AppError> {
        Ok(())
    }

    fn after_publication_identity_check_before_exchange(
        &self,
        _output_parent: &Path,
    ) -> Result<(), AppError> {
        Ok(())
    }

    fn after_existing_output_exchange_before_cleanup(
        &self,
        _output_parent: &Path,
    ) -> Result<(), AppError> {
        Ok(())
    }

    fn after_rename_before_parent_sync(&self) -> Result<(), AppError> {
        Ok(())
    }
}

struct NoopTransferWriteHooks;

impl TransferWriteHooks for NoopTransferWriteHooks {}

fn open_regular_input(
    input: &NativePath,
    after_path_inspection: impl FnOnce(),
) -> Result<File, AppError> {
    let path = input.as_path();
    let before = fs::symlink_metadata(path).map_err(|_| {
        AppError::validation("library_import_input_unavailable", Some(input.clone()))
    })?;
    if before.file_type().is_symlink() || !before.file_type().is_file() {
        return Err(AppError::validation(
            "library_import_input_not_regular",
            Some(input.clone()),
        ));
    }
    after_path_inspection();
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .map_err(|_| {
            AppError::validation("library_import_input_not_regular", Some(input.clone()))
        })?;
    let descriptor = file.metadata().map_err(|_| {
        AppError::validation("library_import_input_not_regular", Some(input.clone()))
    })?;
    let after = fs::symlink_metadata(path).map_err(|_| {
        AppError::validation("library_import_input_identity_drift", Some(input.clone()))
    })?;
    if descriptor.file_type().is_symlink()
        || !descriptor.file_type().is_file()
        || after.file_type().is_symlink()
        || !after.file_type().is_file()
        || !same_identity(&before, &descriptor)
        || !same_identity(&descriptor, &after)
    {
        return Err(AppError::validation(
            "library_import_input_identity_drift",
            Some(input.clone()),
        ));
    }
    Ok(file)
}

struct ValidatedOutputParent {
    path: PathBuf,
    identity: (u64, u64),
    directory: File,
}

impl ValidatedOutputParent {
    fn revalidate(&self, output: &NativePath) -> Result<(), AppError> {
        let metadata = fs::symlink_metadata(&self.path).map_err(|_| {
            AppError::validation("library_export_parent_identity_drift", Some(output.clone()))
        })?;
        let descriptor_metadata = self.directory.metadata().map_err(|_| {
            AppError::validation("library_export_parent_identity_drift", Some(output.clone()))
        })?;
        if metadata.file_type().is_symlink()
            || !metadata.file_type().is_dir()
            || !descriptor_metadata.file_type().is_dir()
            || metadata_identity(&metadata) != self.identity
            || metadata_identity(&descriptor_metadata) != self.identity
        {
            return Err(AppError::validation(
                "library_export_parent_identity_drift",
                Some(output.clone()),
            ));
        }
        Ok(())
    }
}

fn validated_output_parent(
    path: &Path,
    output: &NativePath,
) -> Result<ValidatedOutputParent, AppError> {
    let absolute = absolute_path(path).map_err(|error| {
        AppError::validation(
            format!("library_export_output_path: {error}"),
            Some(output.clone()),
        )
    })?;
    let parent = absolute.parent().ok_or_else(|| {
        AppError::validation("library_export_output_has_no_parent", Some(output.clone()))
    })?;
    let metadata = fs::symlink_metadata(parent)
        .map_err(|_| AppError::validation("library_export_parent_missing", Some(output.clone())))?;
    if metadata.file_type().is_symlink() || !metadata.file_type().is_dir() {
        return Err(AppError::validation(
            "library_export_parent_not_real_directory",
            Some(output.clone()),
        ));
    }
    let path = fs::canonicalize(parent).map_err(|_| {
        AppError::validation("library_export_parent_unresolvable", Some(output.clone()))
    })?;
    let metadata = fs::symlink_metadata(&path).map_err(|_| {
        AppError::validation("library_export_parent_identity_drift", Some(output.clone()))
    })?;
    let mut options = OpenOptions::new();
    options
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW);
    let directory = options.open(&path).map_err(|_| {
        AppError::validation("library_export_parent_identity_drift", Some(output.clone()))
    })?;
    let descriptor_metadata = directory.metadata().map_err(|_| {
        AppError::validation("library_export_parent_identity_drift", Some(output.clone()))
    })?;
    if metadata.file_type().is_symlink()
        || !metadata.file_type().is_dir()
        || !descriptor_metadata.file_type().is_dir()
        || !same_identity(&metadata, &descriptor_metadata)
    {
        return Err(AppError::validation(
            "library_export_parent_identity_drift",
            Some(output.clone()),
        ));
    }
    Ok(ValidatedOutputParent {
        path,
        identity: metadata_identity(&metadata),
        directory,
    })
}

#[derive(Clone, Copy)]
enum OutputTargetIdentity {
    Existing((u64, u64)),
    Absent,
}

fn observe_output_target(
    output_path: &Path,
    output: &NativePath,
) -> Result<OutputTargetIdentity, AppError> {
    match fs::symlink_metadata(output_path) {
        Ok(metadata) if metadata.file_type().is_file() && !metadata.file_type().is_symlink() => {
            Ok(OutputTargetIdentity::Existing(metadata_identity(&metadata)))
        }
        Ok(_) => Err(AppError::validation(
            "library_export_output_not_regular",
            Some(output.clone()),
        )),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(OutputTargetIdentity::Absent),
        Err(_) => Err(AppError::validation(
            "library_export_output_unavailable",
            Some(output.clone()),
        )),
    }
}

struct OutputPublicationGuard<'parent> {
    parent: &'parent ValidatedOutputParent,
    name: std::ffi::OsString,
    identity: (u64, u64),
}

impl<'parent> OutputPublicationGuard<'parent> {
    fn capture(
        parent: &'parent ValidatedOutputParent,
        roots: &ResolvedRoots,
        expected_identity: (u64, u64),
        output_name: std::ffi::OsString,
        output: &NativePath,
    ) -> Result<Self, AppError> {
        let entry = statat(&parent.directory, &output_name, AtFlags::SYMLINK_NOFOLLOW).map_err(
            |error| {
                let error: io::Error = error.into();
                if error.kind() == io::ErrorKind::NotFound {
                    AppError::validation(
                        "library_export_publication_identity_drift",
                        Some(output.clone()),
                    )
                } else {
                    export_io(output, "inspect export output", error)
                }
            },
        )?;
        if FileType::from_raw_mode(entry.st_mode) != FileType::RegularFile
            || stat_identity(entry.st_dev, entry.st_ino) != Some(expected_identity)
        {
            return Err(AppError::validation(
                "library_export_publication_identity_drift",
                Some(output.clone()),
            ));
        }
        let guard = Self {
            parent,
            name: output_name,
            identity: expected_identity,
        };
        if !guard.matches(&guard.name) {
            return Err(AppError::validation(
                "library_export_publication_identity_drift",
                Some(output.clone()),
            ));
        }
        if protected_paths(roots, output)?.iter().any(|protected| {
            fs::symlink_metadata(protected)
                .ok()
                .is_some_and(|metadata| metadata_identity(&metadata) == guard.identity)
        }) {
            return Err(AppError::validation(
                "library_export_protected_target",
                Some(output.clone()),
            ));
        }
        Ok(guard)
    }

    fn matches(&self, name: &std::ffi::OsStr) -> bool {
        statat(&self.parent.directory, name, AtFlags::SYMLINK_NOFOLLOW)
            .ok()
            .is_some_and(|entry| {
                FileType::from_raw_mode(entry.st_mode) == FileType::RegularFile
                    && stat_identity(entry.st_dev, entry.st_ino) == Some(self.identity)
            })
    }
}

struct PublishStagingHooks<
    AfterIdentityCheck,
    AfterPublicationLink,
    AfterPublicationIdentity,
    AfterExistingOutputExchange,
> {
    after_identity_check: AfterIdentityCheck,
    after_publication_link_before_rename: AfterPublicationLink,
    after_publication_identity_check_before_exchange: AfterPublicationIdentity,
    after_existing_output_exchange_before_cleanup: AfterExistingOutputExchange,
}

fn publish_staging<
    AfterIdentityCheck,
    AfterPublicationLink,
    AfterPublicationIdentity,
    AfterExistingOutputExchange,
>(
    staging: &mut NamedTempFile,
    parent: &ValidatedOutputParent,
    roots: &ResolvedRoots,
    expected_output: OutputTargetIdentity,
    output_path: &Path,
    output: &NativePath,
    hooks: PublishStagingHooks<
        AfterIdentityCheck,
        AfterPublicationLink,
        AfterPublicationIdentity,
        AfterExistingOutputExchange,
    >,
) -> Result<(), AppError>
where
    AfterIdentityCheck: FnOnce() -> Result<(), AppError>,
    AfterPublicationLink: FnOnce() -> Result<(), AppError>,
    AfterPublicationIdentity: FnOnce() -> Result<(), AppError>,
    AfterExistingOutputExchange: FnOnce() -> Result<(), AppError>,
{
    let PublishStagingHooks {
        after_identity_check,
        after_publication_link_before_rename,
        after_publication_identity_check_before_exchange,
        after_existing_output_exchange_before_cleanup,
    } = hooks;
    let staging_name = staging
        .path()
        .file_name()
        .ok_or_else(|| {
            AppError::validation("library_export_staging_has_no_name", Some(output.clone()))
        })?
        .to_os_string();
    let output_name = output_path
        .file_name()
        .ok_or_else(|| {
            AppError::validation("library_export_output_has_no_name", Some(output.clone()))
        })?
        .to_os_string();
    if let Err(error) = verify_staging_identity(staging, parent, &staging_name, output) {
        cleanup_staging_if_owned(staging, parent, &staging_name);
        return Err(error);
    }
    if let Err(error) = after_identity_check() {
        cleanup_staging_if_owned(staging, parent, &staging_name);
        return Err(error);
    }
    if let Err(error) = parent.revalidate(output) {
        cleanup_staging_if_owned(staging, parent, &staging_name);
        return Err(error);
    }
    let output_guard = match expected_output {
        OutputTargetIdentity::Existing(expected_identity) => {
            match OutputPublicationGuard::capture(
                parent,
                roots,
                expected_identity,
                output_name.clone(),
                output,
            ) {
                Ok(guard) => Some(guard),
                Err(error) => {
                    cleanup_staging_if_owned(staging, parent, &staging_name);
                    return Err(error);
                }
            }
        }
        OutputTargetIdentity::Absent => None,
    };
    if let Err(error) = verify_staging_identity(staging, parent, &staging_name, output) {
        cleanup_staging_if_owned(staging, parent, &staging_name);
        return Err(error);
    }
    let publication_name = match link_staging_inode(staging, parent, &staging_name, output) {
        Ok(name) => name,
        Err(error) => {
            cleanup_staging_if_owned(staging, parent, &staging_name);
            return Err(error);
        }
    };
    if let Err(error) = after_publication_link_before_rename() {
        cleanup_staging_if_owned(staging, parent, &publication_name);
        cleanup_staging_if_owned(staging, parent, &staging_name);
        return Err(error);
    }
    if let Err(error) = parent.revalidate(output) {
        cleanup_staging_if_owned(staging, parent, &publication_name);
        cleanup_staging_if_owned(staging, parent, &staging_name);
        return Err(error);
    }
    if let Err(error) = verify_staging_identity(staging, parent, &publication_name, output) {
        cleanup_staging_if_owned(staging, parent, &publication_name);
        cleanup_staging_if_owned(staging, parent, &staging_name);
        return Err(error);
    }
    if let Some(output_guard) = output_guard {
        if !output_guard.matches(&output_name) {
            cleanup_staging_if_owned(staging, parent, &publication_name);
            cleanup_staging_if_owned(staging, parent, &staging_name);
            return Err(AppError::validation(
                "library_export_publication_identity_drift",
                Some(output.clone()),
            ));
        }
        if let Err(error) = after_publication_identity_check_before_exchange() {
            cleanup_staging_if_owned(staging, parent, &publication_name);
            cleanup_staging_if_owned(staging, parent, &staging_name);
            return Err(error);
        }
        if let Err(error) = renameat_with(
            &parent.directory,
            &publication_name,
            &parent.directory,
            &output_name,
            RenameFlags::EXCHANGE,
        ) {
            cleanup_staging_if_owned(staging, parent, &staging_name);
            cleanup_staging_if_owned(staging, parent, &publication_name);
            return Err(export_io(
                output,
                "exchange export publication with output guard",
                error.into(),
            ));
        }
        if verify_staging_identity(staging, parent, &output_name, output).is_err()
            || !output_guard.matches(&publication_name)
        {
            let restore = renameat_with(
                &parent.directory,
                &publication_name,
                &parent.directory,
                &output_name,
                RenameFlags::EXCHANGE,
            );
            cleanup_staging_if_owned(staging, parent, &publication_name);
            cleanup_staging_if_owned(staging, parent, &staging_name);
            return match restore {
                Ok(()) => Err(AppError::validation(
                    "library_export_publication_identity_drift",
                    Some(output.clone()),
                )),
                Err(error) => Err(export_io(
                    output,
                    "restore export target after publication identity drift",
                    error.into(),
                )),
            };
        }
        if let Err(error) = after_existing_output_exchange_before_cleanup() {
            cleanup_staging_if_owned(staging, parent, &staging_name);
            return Err(error);
        }
    } else {
        if let Err(error) = after_publication_identity_check_before_exchange() {
            cleanup_staging_if_owned(staging, parent, &publication_name);
            cleanup_staging_if_owned(staging, parent, &staging_name);
            return Err(error);
        }
        if let Err(error) = renameat_with(
            &parent.directory,
            &publication_name,
            &parent.directory,
            &output_name,
            RenameFlags::NOREPLACE,
        ) {
            cleanup_staging_if_owned(staging, parent, &publication_name);
            cleanup_staging_if_owned(staging, parent, &staging_name);
            return Err(export_io(
                output,
                "publish export to absent output",
                error.into(),
            ));
        }
        if let Err(error) = verify_staging_identity(staging, parent, &output_name, output) {
            cleanup_staging_if_owned(staging, parent, &staging_name);
            return Err(error);
        }
    }
    cleanup_staging_if_owned(staging, parent, &staging_name);
    Ok(())
}

fn link_staging_inode(
    staging: &mut NamedTempFile,
    parent: &ValidatedOutputParent,
    staging_name: &std::ffi::OsStr,
    output: &NativePath,
) -> Result<std::ffi::OsString, AppError> {
    let mut placeholder = Builder::new()
        .prefix(".skilload-publish-")
        .suffix(".tmp")
        .tempfile_in(&parent.path)
        .map_err(|error| export_io(output, "create export publication link", error))?;
    let publication_name = placeholder
        .path()
        .file_name()
        .ok_or_else(|| {
            AppError::validation(
                "library_export_publication_has_no_name",
                Some(output.clone()),
            )
        })?
        .to_os_string();
    let placeholder_identity = fstat(placeholder.as_file()).map_err(|_| {
        AppError::validation(
            "library_export_publication_identity_drift",
            Some(output.clone()),
        )
    })?;
    let placeholder_entry = statat(
        &parent.directory,
        &publication_name,
        AtFlags::SYMLINK_NOFOLLOW,
    )
    .map_err(|_| {
        AppError::validation(
            "library_export_publication_identity_drift",
            Some(output.clone()),
        )
    })?;
    if placeholder_identity.st_dev != placeholder_entry.st_dev
        || placeholder_identity.st_ino != placeholder_entry.st_ino
    {
        placeholder.disable_cleanup(true);
        return Err(AppError::validation(
            "library_export_publication_identity_drift",
            Some(output.clone()),
        ));
    }
    if let Err(error) = unlinkat(&parent.directory, &publication_name, AtFlags::empty()) {
        placeholder.disable_cleanup(true);
        return Err(export_io(
            output,
            "prepare export publication link",
            error.into(),
        ));
    }
    placeholder.disable_cleanup(true);
    verify_staging_identity(staging, parent, staging_name, output)?;
    linkat(
        &parent.directory,
        staging_name,
        &parent.directory,
        &publication_name,
        AtFlags::empty(),
    )
    .map_err(|error| export_io(output, "link held export staging file", error.into()))?;
    verify_staging_identity(staging, parent, &publication_name, output)?;
    Ok(publication_name)
}

fn verify_staging_identity(
    staging: &NamedTempFile,
    parent: &ValidatedOutputParent,
    staging_name: &std::ffi::OsStr,
    output: &NativePath,
) -> Result<(), AppError> {
    let held = fstat(staging.as_file()).map_err(|_| {
        AppError::validation(
            "library_export_staging_identity_drift",
            Some(output.clone()),
        )
    })?;
    let entry =
        statat(&parent.directory, staging_name, AtFlags::SYMLINK_NOFOLLOW).map_err(|_| {
            AppError::validation(
                "library_export_staging_identity_drift",
                Some(output.clone()),
            )
        })?;
    if held.st_dev != entry.st_dev || held.st_ino != entry.st_ino {
        return Err(AppError::validation(
            "library_export_staging_identity_drift",
            Some(output.clone()),
        ));
    }
    Ok(())
}

fn cleanup_staging_if_owned(
    staging: &mut NamedTempFile,
    parent: &ValidatedOutputParent,
    staging_name: &std::ffi::OsStr,
) {
    let owned = fstat(staging.as_file())
        .ok()
        .zip(statat(&parent.directory, staging_name, AtFlags::SYMLINK_NOFOLLOW).ok())
        .is_some_and(|(held, entry)| held.st_dev == entry.st_dev && held.st_ino == entry.st_ino);
    if owned {
        let _ = unlinkat(&parent.directory, staging_name, AtFlags::empty());
    }
    staging.disable_cleanup(true);
}

fn reject_protected_output(
    output_path: &Path,
    output_parent: &Path,
    roots: &ResolvedRoots,
    output: &NativePath,
) -> Result<(), AppError> {
    let output_absolute = absolute_path(output_path).map_err(|error| {
        AppError::validation(
            format!("library_export_output_path: {error}"),
            Some(output.clone()),
        )
    })?;
    let output_resolved = output_path
        .file_name()
        .map(|name| output_parent.join(name))
        .ok_or_else(|| {
            AppError::validation("library_export_output_has_no_name", Some(output.clone()))
        })?;
    let output_metadata = fs::symlink_metadata(output_path).ok();

    for protected in protected_paths(roots, output)? {
        let protected_absolute = absolute_path(&protected).map_err(|error| {
            AppError::invalid_state(
                "library_export",
                format!("cannot normalize protected target: {error}"),
                ["an absolute protected database path"],
            )
        })?;
        if output_absolute == protected_absolute {
            return Err(AppError::validation(
                "library_export_protected_target",
                Some(output.clone()),
            ));
        }
        if let Some(protected_resolved) = resolved_existing_path(&protected)
            && output_resolved == protected_resolved
        {
            return Err(AppError::validation(
                "library_export_protected_target",
                Some(output.clone()),
            ));
        }
        if let (Some(output_metadata), Ok(protected_metadata)) =
            (output_metadata.as_ref(), fs::symlink_metadata(&protected))
            && same_identity(output_metadata, &protected_metadata)
        {
            return Err(AppError::validation(
                "library_export_protected_target",
                Some(output.clone()),
            ));
        }
    }
    Ok(())
}

fn protected_paths(roots: &ResolvedRoots, output: &NativePath) -> Result<Vec<PathBuf>, AppError> {
    let database = roots.data.effective.join("skilload.db");
    let mut protected = vec![
        database.clone(),
        database.with_file_name("skilload.db-journal"),
        database.with_file_name("skilload.db-wal"),
        database.with_file_name("skilload.db-shm"),
        roots.state.effective.join("locks/database.lock"),
    ];
    // Migration backups are recovery assets, not ordinary export targets.
    // Preserve every published pair entry so aliases are rejected by the
    // same pathname and inode checks as the live generation. An absent
    // backups directory has no published recovery asset; every other
    // enumeration failure must reject the export rather than omit protection.
    let backup_directory = roots.data.effective.join("backups");
    let entries = match fs::read_dir(&backup_directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(protected),
        Err(_) => {
            return Err(AppError::validation(
                "library_export_protected_inventory_unavailable",
                Some(output.clone()),
            ));
        }
    };
    for entry in entries {
        let entry = entry.map_err(|_| {
            AppError::validation(
                "library_export_protected_inventory_unavailable",
                Some(output.clone()),
            )
        })?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if name.starts_with("skilload-db-v1-to-v2-")
            && (name.ends_with(".db") || name.ends_with(".manifest.json"))
        {
            protected.push(entry.path());
        }
    }
    Ok(protected)
}

fn resolved_existing_path(path: &Path) -> Option<PathBuf> {
    let parent = path.parent()?;
    let file_name = path.file_name()?;
    fs::canonicalize(parent)
        .ok()
        .map(|parent| parent.join(file_name))
}

fn ensure_regular_output(path: &Path, output: &NativePath) -> Result<(), AppError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_file() && !metadata.file_type().is_symlink() => {
            Ok(())
        }
        Ok(_) => Err(AppError::validation(
            "library_export_output_not_regular",
            Some(output.clone()),
        )),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(AppError::validation(
            "library_export_output_unavailable",
            Some(output.clone()),
        )),
    }
}

fn absolute_path(path: &Path) -> io::Result<PathBuf> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Ok(std::env::current_dir()?.join(path))
    }
}

fn stat_identity<T>(device: T, inode: u64) -> Option<(u64, u64)>
where
    T: TryInto<u64>,
{
    device.try_into().ok().map(|device| (device, inode))
}

fn metadata_identity(metadata: &fs::Metadata) -> (u64, u64) {
    (metadata.dev(), metadata.ino())
}

fn same_identity(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    metadata_identity(left) == metadata_identity(right)
}

fn export_io(output: &NativePath, action: &str, error: io::Error) -> AppError {
    AppError::validation(
        format!("library_export_io: {action}: {error}"),
        Some(output.clone()),
    )
}

struct JsonScanner<'input, R> {
    reader: io::BufReader<R>,
    buffered: Option<u8>,
    bytes: Vec<u8>,
    position: usize,
    values: u64,
    entries: u64,
    input: &'input NativePath,
}

impl<'input, R: Read> JsonScanner<'input, R> {
    fn from_reader(reader: R, input: &'input NativePath) -> Self {
        Self {
            reader: io::BufReader::with_capacity(INPUT_SCAN_BUFFER_BYTES, reader),
            buffered: None,
            bytes: Vec::with_capacity(INPUT_SCAN_BUFFER_BYTES),
            position: 0,
            values: 0,
            entries: 0,
            input,
        }
    }

    fn scan(mut self) -> Result<Vec<u8>, AppError> {
        self.skip_whitespace()?;
        if self.peek()? == Some(b'{') {
            self.parse_object(1, true)?;
        } else {
            self.parse_value(0)?;
        }
        self.skip_whitespace()?;
        if self.position != self.bytes.len() {
            return Err(self.malformed());
        }
        Ok(self.bytes)
    }

    fn parse_value(&mut self, parent_depth: u64) -> Result<(), AppError> {
        self.skip_whitespace()?;
        match self.peek()? {
            Some(b'{') => self.parse_object(parent_depth + 1, false),
            Some(b'[') => self.parse_array(parent_depth + 1),
            Some(b'"') => {
                self.count_value()?;
                self.parse_string().map(|_| ())
            }
            Some(b'-' | b'0'..=b'9') => {
                self.count_value()?;
                self.parse_number()
            }
            Some(b't') => {
                self.count_value()?;
                self.expect_literal(b"true")
            }
            Some(b'f') => {
                self.count_value()?;
                self.expect_literal(b"false")
            }
            Some(b'n') => {
                self.count_value()?;
                self.expect_literal(b"null")
            }
            _ => Err(self.malformed()),
        }
    }

    fn parse_object(&mut self, depth: u64, root: bool) -> Result<(), AppError> {
        self.ensure_depth(depth)?;
        self.count_value()?;
        self.expect_byte(b'{')?;
        self.skip_whitespace()?;
        let mut keys = HashSet::new();
        if self.consume_byte(b'}')? {
            return Ok(());
        }
        loop {
            self.skip_whitespace()?;
            if self.peek()? != Some(b'"') {
                return Err(self.malformed());
            }
            self.count_value()?;
            let key = self.parse_string()?;
            if !keys.insert(key.clone()) {
                return Err(AppError::validation(
                    "library_import_duplicate_object_key",
                    Some(self.input.clone()),
                ));
            }
            self.skip_whitespace()?;
            self.expect_byte(b':')?;
            if root && key == "entries" {
                self.skip_whitespace()?;
                if self.peek()? == Some(b'[') {
                    self.parse_entries_array(depth + 1)?;
                } else {
                    self.parse_value(depth)?;
                }
            } else {
                self.parse_value(depth)?;
            }
            self.skip_whitespace()?;
            if self.consume_byte(b'}')? {
                return Ok(());
            }
            self.expect_byte(b',')?;
        }
    }

    fn parse_array(&mut self, depth: u64) -> Result<(), AppError> {
        self.ensure_depth(depth)?;
        self.count_value()?;
        self.expect_byte(b'[')?;
        self.skip_whitespace()?;
        if self.consume_byte(b']')? {
            return Ok(());
        }
        loop {
            self.parse_value(depth)?;
            self.skip_whitespace()?;
            if self.consume_byte(b']')? {
                return Ok(());
            }
            self.expect_byte(b',')?;
        }
    }

    fn parse_entries_array(&mut self, depth: u64) -> Result<(), AppError> {
        self.ensure_depth(depth)?;
        self.count_value()?;
        self.expect_byte(b'[')?;
        self.skip_whitespace()?;
        if self.consume_byte(b']')? {
            return Ok(());
        }
        loop {
            self.skip_whitespace()?;
            if self.peek()? == Some(b'{') {
                self.entries += 1;
                if self.entries > MAX_PORTABLE_LIBRARY_ENTRIES {
                    return Err(self.limit(
                        "library_import_entries",
                        self.entries,
                        MAX_PORTABLE_LIBRARY_ENTRIES,
                    ));
                }
                self.parse_object(depth + 1, false)?;
            } else {
                self.parse_value(depth)?;
            }
            self.skip_whitespace()?;
            if self.consume_byte(b']')? {
                return Ok(());
            }
            self.expect_byte(b',')?;
        }
    }

    fn parse_string(&mut self) -> Result<String, AppError> {
        self.expect_byte(b'"')?;
        let mut value = String::new();
        loop {
            let Some(byte) = self.peek()? else {
                return Err(self.malformed());
            };
            match byte {
                b'"' => {
                    self.next_byte()?;
                    return Ok(value);
                }
                b'\\' => {
                    self.next_byte()?;
                    let Some(escaped) = self.next_byte()? else {
                        return Err(self.malformed());
                    };
                    match escaped {
                        b'"' => self.push_string_character(&mut value, '"')?,
                        b'\\' => self.push_string_character(&mut value, '\\')?,
                        b'/' => self.push_string_character(&mut value, '/')?,
                        b'b' => self.push_string_character(&mut value, '\u{0008}')?,
                        b'f' => self.push_string_character(&mut value, '\u{000C}')?,
                        b'n' => self.push_string_character(&mut value, '\n')?,
                        b'r' => self.push_string_character(&mut value, '\r')?,
                        b't' => self.push_string_character(&mut value, '\t')?,
                        b'u' => {
                            let first = self.parse_hex_escape()?;
                            let scalar = if (0xd800..=0xdbff).contains(&first) {
                                self.expect_byte(b'\\')?;
                                self.expect_byte(b'u')?;
                                let second = self.parse_hex_escape()?;
                                if !(0xdc00..=0xdfff).contains(&second) {
                                    return Err(self.malformed());
                                }
                                0x1_0000 + ((first - 0xd800) << 10) + (second - 0xdc00)
                            } else if (0xdc00..=0xdfff).contains(&first) {
                                return Err(self.malformed());
                            } else {
                                first
                            };
                            self.push_string_character(
                                &mut value,
                                char::from_u32(scalar).ok_or_else(|| self.malformed())?,
                            )?;
                        }
                        _ => return Err(self.malformed()),
                    }
                }
                0x00..=0x1f => return Err(self.malformed()),
                0x20..=0x7f => {
                    self.next_byte()?;
                    self.push_string_character(&mut value, byte as char)?;
                }
                _ => {
                    let width = utf8_width(byte).ok_or_else(|| self.malformed())?;
                    let mut encoded = [0_u8; 4];
                    for byte in encoded.iter_mut().take(width) {
                        let Some(next) = self.next_byte()? else {
                            return Err(self.malformed());
                        };
                        *byte = next;
                    }
                    let decoded =
                        std::str::from_utf8(&encoded[..width]).map_err(|_| self.malformed())?;
                    let character = decoded.chars().next().ok_or_else(|| self.malformed())?;
                    self.push_string_character(&mut value, character)?;
                }
            }
        }
    }

    fn parse_hex_escape(&mut self) -> Result<u32, AppError> {
        let mut value = 0_u32;
        for _ in 0..4 {
            let Some(byte) = self.next_byte()? else {
                return Err(self.malformed());
            };
            let digit = match byte {
                b'0'..=b'9' => (byte - b'0') as u32,
                b'a'..=b'f' => (byte - b'a' + 10) as u32,
                b'A'..=b'F' => (byte - b'A' + 10) as u32,
                _ => return Err(self.malformed()),
            };
            value = value * 16 + digit;
        }
        Ok(value)
    }

    fn push_string_character(&self, value: &mut String, character: char) -> Result<(), AppError> {
        let next = value.len() as u64 + character.len_utf8() as u64;
        if next > MAX_IMPORT_STRING_BYTES {
            return Err(self.limit("library_import_string_bytes", next, MAX_IMPORT_STRING_BYTES));
        }
        value.push(character);
        Ok(())
    }

    fn parse_number(&mut self) -> Result<(), AppError> {
        let start = self.position;
        if self.peek()? == Some(b'-') {
            self.advance_number(start)?;
        }
        match self.peek()? {
            Some(b'0') => self.advance_number(start)?,
            Some(b'1'..=b'9') => {
                self.advance_number(start)?;
                while matches!(self.peek()?, Some(b'0'..=b'9')) {
                    self.advance_number(start)?;
                }
            }
            _ => return Err(self.malformed()),
        }
        if self.peek()? == Some(b'.') {
            self.advance_number(start)?;
            if !matches!(self.peek()?, Some(b'0'..=b'9')) {
                return Err(self.malformed());
            }
            while matches!(self.peek()?, Some(b'0'..=b'9')) {
                self.advance_number(start)?;
            }
        }
        if matches!(self.peek()?, Some(b'e' | b'E')) {
            self.advance_number(start)?;
            if matches!(self.peek()?, Some(b'+' | b'-')) {
                self.advance_number(start)?;
            }
            if !matches!(self.peek()?, Some(b'0'..=b'9')) {
                return Err(self.malformed());
            }
            while matches!(self.peek()?, Some(b'0'..=b'9')) {
                self.advance_number(start)?;
            }
        }
        Ok(())
    }

    fn advance_number(&mut self, start: usize) -> Result<(), AppError> {
        if self.next_byte()?.is_none() {
            return Err(self.malformed());
        }
        let length = (self.position - start) as u64;
        if length > MAX_IMPORT_NUMBER_BYTES {
            return Err(self.limit(
                "library_import_number_bytes",
                length,
                MAX_IMPORT_NUMBER_BYTES,
            ));
        }
        Ok(())
    }

    fn expect_literal(&mut self, literal: &[u8]) -> Result<(), AppError> {
        for expected in literal {
            if self.next_byte()? != Some(*expected) {
                return Err(self.malformed());
            }
        }
        Ok(())
    }

    fn count_value(&mut self) -> Result<(), AppError> {
        self.values += 1;
        if self.values > MAX_IMPORT_VALUES {
            return Err(self.limit("library_import_values", self.values, MAX_IMPORT_VALUES));
        }
        Ok(())
    }

    fn ensure_depth(&self, depth: u64) -> Result<(), AppError> {
        if depth > MAX_IMPORT_DEPTH {
            Err(self.limit("library_import_depth", depth, MAX_IMPORT_DEPTH))
        } else {
            Ok(())
        }
    }

    fn limit(&self, kind: &str, measured: u64, allowed: u64) -> AppError {
        AppError::library_input_limit(kind, measured, allowed, self.input.clone())
    }

    fn malformed(&self) -> AppError {
        AppError::validation("library_import_json", Some(self.input.clone()))
    }

    fn skip_whitespace(&mut self) -> Result<(), AppError> {
        while matches!(self.peek()?, Some(b' ' | b'\n' | b'\r' | b'\t')) {
            self.next_byte()?;
        }
        Ok(())
    }

    fn expect_byte(&mut self, expected: u8) -> Result<(), AppError> {
        self.skip_whitespace()?;
        if self.consume_byte(expected)? {
            Ok(())
        } else {
            Err(self.malformed())
        }
    }

    fn consume_byte(&mut self, expected: u8) -> Result<bool, AppError> {
        if self.peek()? == Some(expected) {
            self.next_byte()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn next_byte(&mut self) -> Result<Option<u8>, AppError> {
        let byte = self.peek()?;
        if byte.is_some() {
            self.position += 1;
            self.buffered = None;
        }
        Ok(byte)
    }

    fn peek(&mut self) -> Result<Option<u8>, AppError> {
        if self.buffered.is_none() {
            let mut byte = [0_u8; 1];
            let read = self.reader.read(&mut byte).map_err(|error| {
                AppError::validation(
                    format!("library_import_read_failed: {error}"),
                    Some(self.input.clone()),
                )
            })?;
            if read == 0 {
                return Ok(None);
            }
            let measured = self.bytes.len() as u64 + 1;
            if measured > MAX_PORTABLE_LIBRARY_DOCUMENT_BYTES {
                return Err(self.limit(
                    "library_import_bytes",
                    measured,
                    MAX_PORTABLE_LIBRARY_DOCUMENT_BYTES,
                ));
            }
            self.bytes.push(byte[0]);
            self.buffered = Some(byte[0]);
        }
        Ok(self.buffered)
    }
}

#[cfg(test)]
impl<'input> JsonScanner<'input, io::Cursor<&'input [u8]>> {
    fn new(bytes: &'input [u8], input: &'input NativePath) -> Self {
        Self::from_reader(io::Cursor::new(bytes), input)
    }
}

fn utf8_width(first: u8) -> Option<usize> {
    match first {
        0xc2..=0xdf => Some(2),
        0xe0..=0xef => Some(3),
        0xf0..=0xf4 => Some(4),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::source::{RefKind, ResolvedSkill, SourceIdentity};
    use crate::ports::configuration::Environment;
    use std::collections::HashMap;
    use std::ffi::OsString;
    use std::os::unix::{ffi::OsStringExt, fs::symlink};
    use tempfile::tempdir;

    #[derive(Default)]
    struct TestEnvironment(HashMap<String, OsString>);

    impl TestEnvironment {
        fn with_roots(root: &Path) -> Self {
            let mut values = HashMap::new();
            values.insert("HOME".to_owned(), root.join("home").into_os_string());
            values.insert(
                "XDG_CONFIG_HOME".to_owned(),
                root.join("config").into_os_string(),
            );
            values.insert(
                "XDG_DATA_HOME".to_owned(),
                root.join("data").into_os_string(),
            );
            values.insert(
                "XDG_STATE_HOME".to_owned(),
                root.join("state").into_os_string(),
            );
            values.insert(
                "XDG_CACHE_HOME".to_owned(),
                root.join("cache").into_os_string(),
            );
            Self(values)
        }
    }

    impl Environment for TestEnvironment {
        fn var_os(&self, key: &str) -> Option<OsString> {
            self.0.get(key).cloned()
        }
    }

    fn document() -> PortableLibraryDocument {
        let source = SourceIdentity::new(
            "github:owner/repository#skills/review@refs/heads/main".to_owned(),
            "owner".to_owned(),
            "repository".to_owned(),
            "Repository".to_owned(),
            "skills/review".to_owned(),
            RefKind::Branch,
            "refs/heads/main".to_owned(),
        )
        .unwrap();
        PortableLibraryDocument {
            format_version: 1,
            entries: vec![crate::domain::library::PortableLibraryEntry {
                skill: ResolvedSkill::new(
                    source,
                    1,
                    "0123456789012345678901234567890123456789".to_owned(),
                    "sha256:0123456789012345678901234567890123456789012345678901234567890123"
                        .to_owned(),
                    "review".to_owned(),
                    "Description".to_owned(),
                    1,
                    1,
                )
                .unwrap(),
                alias: None,
                category: None,
                tags: vec!["Review".to_owned()],
                note: None,
            }],
        }
    }

    #[test]
    fn scanner_rejects_duplicate_keys_before_deserialization() {
        let temporary = tempdir().unwrap();
        let input = NativePath::new(temporary.path().join("input.json"));
        fs::write(
            input.as_path(),
            br#"{"format_version":1,"format_version":1,"entries":[]}"#,
        )
        .unwrap();
        let store = PortableLibraryTransferStore::new();
        let error = store.read_import(&input).unwrap_err();
        assert_eq!(error.code(), "validation_failed");
    }

    #[test]
    fn scanner_defers_non_array_entries_to_schema_validation() {
        let temporary = tempdir().unwrap();
        let input = NativePath::new(temporary.path().join("input.json"));
        fs::write(input.as_path(), br#"{"format_version":1,"entries":null}"#).unwrap();

        let error = PortableLibraryTransferStore::new()
            .read_import(&input)
            .unwrap_err();

        assert!(matches!(
            error,
            AppError::Validation { constraint, .. } if constraint == "library_import_schema"
        ));
    }

    #[test]
    fn scanner_rejects_fifo_without_waiting() {
        let temporary = tempdir().unwrap();
        let input = temporary.path().join("input.fifo");
        let status = std::process::Command::new("mkfifo")
            .arg(&input)
            .status()
            .unwrap();
        assert!(status.success());
        let store = PortableLibraryTransferStore::new();
        let error = store.read_import(&NativePath::new(input)).unwrap_err();
        assert_eq!(error.code(), "validation_failed");
    }

    #[test]
    fn input_gate_rejects_nonregular_paths_and_identity_swaps() {
        let temporary = tempdir().unwrap();
        let regular = temporary.path().join("regular.json");
        fs::write(&regular, b"{}").unwrap();
        let symlinked = temporary.path().join("input-symlink.json");
        symlink(&regular, &symlinked).unwrap();
        let directory = temporary.path().join("input-directory");
        fs::create_dir(&directory).unwrap();
        for path in [&symlinked, &directory, Path::new("/dev/null")] {
            let error =
                open_regular_input(&NativePath::new(path.to_path_buf()), || {}).unwrap_err();
            assert_eq!(error.code(), "validation_failed");
        }

        let input = temporary.path().join("identity.json");
        let replacement = temporary.path().join("replacement.json");
        fs::write(&input, b"{}").unwrap();
        fs::write(&replacement, b"[]").unwrap();
        let error = open_regular_input(&NativePath::new(input.clone()), || {
            fs::rename(&replacement, &input).unwrap();
        })
        .unwrap_err();
        assert_eq!(error.code(), "validation_failed");
    }

    #[test]
    fn output_refuses_all_active_database_generation_targets_before_staging() {
        let temporary = tempdir().unwrap();
        let data = temporary.path().join("data/skilload");
        let locks = temporary.path().join("state/skilload/locks");
        fs::create_dir_all(&data).unwrap();
        fs::create_dir_all(&locks).unwrap();
        let targets = [
            data.join("skilload.db"),
            data.join("skilload.db-journal"),
            data.join("skilload.db-wal"),
            data.join("skilload.db-shm"),
            locks.join("database.lock"),
        ];
        for target in &targets {
            fs::write(target, b"protected generation").unwrap();
        }
        let store = PortableLibraryTransferStore::with_environment(
            Arc::new(TestEnvironment::with_roots(temporary.path())),
            Arc::new(XdgRootResolver),
        );
        for target in targets {
            let output = NativePath::new(target.clone());
            let error = store.write_export(&output, &document()).unwrap_err();
            assert_eq!(error.code(), "validation_failed");
            assert_eq!(fs::read(target).unwrap(), b"protected generation");
        }
    }

    #[test]
    fn output_refuses_published_migration_backup_pair_before_staging() {
        let temporary = tempdir().unwrap();
        let backups = temporary.path().join("data/skilload/backups");
        fs::create_dir_all(&backups).unwrap();
        let backup = backups.join("skilload-db-v1-to-v2-1.db");
        let manifest = backups.join("skilload-db-v1-to-v2-1.manifest.json");
        fs::write(&backup, b"recovery database").unwrap();
        fs::write(&manifest, b"recovery manifest").unwrap();
        let alias = temporary.path().join("backup-alias.db");
        fs::hard_link(&backup, &alias).unwrap();

        let store = PortableLibraryTransferStore::with_environment(
            Arc::new(TestEnvironment::with_roots(temporary.path())),
            Arc::new(XdgRootResolver),
        );
        for target in [&backup, &manifest, &alias] {
            let before = fs::read(target).unwrap();
            let error = store
                .write_export(&NativePath::new(target.clone()), &document())
                .unwrap_err();
            assert_eq!(error.code(), "validation_failed");
            assert_eq!(fs::read(target).unwrap(), before);
        }
        assert!(fs::read_dir(&backups).unwrap().all(|entry| {
            !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".skilload-library-")
        }));
    }

    #[test]
    fn output_rejects_an_unreadable_migration_backup_inventory_before_staging() {
        let temporary = tempdir().unwrap();
        let backups = temporary.path().join("data/skilload/backups");
        let output_parent = temporary.path().join("output");
        let output = NativePath::new(output_parent.join("library.json"));
        fs::create_dir_all(&backups).unwrap();
        fs::create_dir(&output_parent).unwrap();
        let backup = backups.join("skilload-db-v1-to-v2-1.db");
        fs::write(&backup, b"recovery database").unwrap();
        let original_permissions = fs::metadata(&backups).unwrap().permissions();
        fs::set_permissions(&backups, fs::Permissions::from_mode(0o300)).unwrap();
        assert!(fs::read_dir(&backups).is_err());

        let store = PortableLibraryTransferStore::with_environment(
            Arc::new(TestEnvironment::with_roots(temporary.path())),
            Arc::new(XdgRootResolver),
        );
        let result = store.write_export(&output, &document());

        fs::set_permissions(&backups, original_permissions).unwrap();
        let error = result.unwrap_err();
        assert_eq!(error.code(), "validation_failed");
        assert_eq!(fs::read(&backup).unwrap(), b"recovery database");
        assert!(!output.as_path().exists());
        assert!(
            fs::read_dir(&output_parent).unwrap().next().is_none(),
            "the unavailable inventory must reject before staging"
        );
    }

    #[test]
    fn output_refuses_a_live_delete_mode_rollback_journal_before_staging() {
        let temporary = tempdir().unwrap();
        let data = temporary.path().join("data/skilload");
        fs::create_dir_all(&data).unwrap();
        let database = data.join("skilload.db");
        let connection = rusqlite::Connection::open(&database).unwrap();
        connection
            .execute_batch(
                "PRAGMA journal_mode = DELETE;
                 CREATE TABLE entries (id INTEGER PRIMARY KEY);
                 BEGIN IMMEDIATE;
                 INSERT INTO entries DEFAULT VALUES;",
            )
            .unwrap();
        let journal = data.join("skilload.db-journal");
        assert!(journal.is_file());

        let store = PortableLibraryTransferStore::with_environment(
            Arc::new(TestEnvironment::with_roots(temporary.path())),
            Arc::new(XdgRootResolver),
        );
        let error = store
            .write_export(&NativePath::new(journal.clone()), &document())
            .unwrap_err();
        assert_eq!(error.code(), "validation_failed");
        assert!(journal.is_file());
        assert!(fs::read_dir(&data).unwrap().all(|entry| {
            !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".skilload-library-")
        }));
        connection.execute_batch("ROLLBACK").unwrap();
    }

    fn scanner_error(bytes: &[u8]) -> AppError {
        let input = NativePath::new(PathBuf::from("/tmp/library-import.json"));
        JsonScanner::new(bytes, &input).scan().unwrap_err()
    }

    fn assert_limit(bytes: &[u8], kind: &str) {
        let error = scanner_error(bytes);
        assert_eq!(error.code(), "library_input_limit_exceeded");
        match error {
            AppError::LibraryInputLimit {
                limit_kind,
                allowed,
                ..
            } => {
                assert_eq!(limit_kind, kind);
                assert_eq!(
                    allowed,
                    match kind {
                        "library_import_entries" => MAX_PORTABLE_LIBRARY_ENTRIES,
                        "library_import_values" => MAX_IMPORT_VALUES,
                        "library_import_depth" => MAX_IMPORT_DEPTH,
                        "library_import_string_bytes" => MAX_IMPORT_STRING_BYTES,
                        "library_import_number_bytes" => MAX_IMPORT_NUMBER_BYTES,
                        _ => unreachable!(),
                    }
                );
            }
            error => panic!("expected {kind} limit, got {error:?}"),
        }
    }

    #[test]
    fn scanner_enforces_every_non_model_limit() {
        let string = format!(
            r#"{{"x":"{}"}}"#,
            "a".repeat(MAX_IMPORT_STRING_BYTES as usize + 1)
        );
        assert_limit(string.as_bytes(), "library_import_string_bytes");

        let number = format!(
            r#"{{"x":{}}}"#,
            "1".repeat(MAX_IMPORT_NUMBER_BYTES as usize + 1)
        );
        assert_limit(number.as_bytes(), "library_import_number_bytes");

        let mut depth = String::from(r#"{"x":"#);
        for _ in 0..8 {
            depth.push('[');
        }
        depth.push('0');
        for _ in 0..8 {
            depth.push(']');
        }
        depth.push('}');
        assert_limit(depth.as_bytes(), "library_import_depth");

        let entries = format!(
            r#"{{"format_version":1,"entries":[{}]}}"#,
            std::iter::repeat_n("{}", MAX_PORTABLE_LIBRARY_ENTRIES as usize + 1)
                .collect::<Vec<_>>()
                .join(",")
        );
        assert_limit(entries.as_bytes(), "library_import_entries");

        let values = format!(
            r#"{{"x":[{}]}}"#,
            std::iter::repeat_n("0", MAX_IMPORT_VALUES as usize)
                .collect::<Vec<_>>()
                .join(",")
        );
        assert_limit(values.as_bytes(), "library_import_values");
    }

    #[test]
    fn scanner_stops_at_the_first_number_byte_overage() {
        let input = NativePath::new(PathBuf::from("/tmp/library-import.json"));
        let number = format!(
            r#"{{"x":{}}}"#,
            "1".repeat(MAX_IMPORT_NUMBER_BYTES as usize + 1)
        );
        let prefix = r#"{"x":"#;
        let mut scanner = JsonScanner::new(number.as_bytes(), &input);
        for _ in 0..prefix.len() {
            assert!(scanner.next_byte().unwrap().is_some());
        }

        match scanner.parse_number().unwrap_err() {
            AppError::LibraryInputLimit {
                limit_kind,
                measured,
                allowed,
                ..
            } => {
                assert_eq!(limit_kind, "library_import_number_bytes");
                assert_eq!(measured, MAX_IMPORT_NUMBER_BYTES + 1);
                assert_eq!(allowed, MAX_IMPORT_NUMBER_BYTES);
            }
            error => panic!("expected number limit, got {error:?}"),
        }
        assert_eq!(
            scanner.position,
            prefix.len() + MAX_IMPORT_NUMBER_BYTES as usize + 1
        );
    }

    struct ChunkedReader {
        bytes: Vec<u8>,
        position: usize,
        chunk_size: usize,
    }

    impl std::io::Read for ChunkedReader {
        fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
            if buffer.is_empty() {
                return Ok(0);
            }
            let remaining = self.bytes.len().saturating_sub(self.position);
            if remaining == 0 {
                return Ok(0);
            }
            let read = remaining.min(buffer.len()).min(self.chunk_size);
            let end = self.position + read;
            buffer[..read].copy_from_slice(&self.bytes[self.position..end]);
            self.position = end;
            Ok(read)
        }
    }

    #[test]
    fn scanner_stops_reading_at_first_streamed_number_overage() {
        let input = NativePath::new(PathBuf::from("/tmp/library-import.json"));
        let prefix = format!(
            r#"{{"x":{}"#,
            "1".repeat(MAX_IMPORT_NUMBER_BYTES as usize + 1)
        );
        let mut bytes = prefix.into_bytes();
        bytes.extend(std::iter::repeat_n(b' ', INPUT_SCAN_BUFFER_BYTES * 2));
        let mut reader = ChunkedReader {
            bytes,
            position: 0,
            chunk_size: INPUT_SCAN_BUFFER_BYTES,
        };

        match JsonScanner::from_reader(&mut reader, &input)
            .scan()
            .unwrap_err()
        {
            AppError::LibraryInputLimit {
                limit_kind,
                measured,
                allowed,
                ..
            } => {
                assert_eq!(limit_kind, "library_import_number_bytes");
                assert_eq!(measured, MAX_IMPORT_NUMBER_BYTES + 1);
                assert_eq!(allowed, MAX_IMPORT_NUMBER_BYTES);
            }
            error => panic!("expected number limit, got {error:?}"),
        }
        assert_eq!(reader.position, INPUT_SCAN_BUFFER_BYTES);
        assert!(reader.position < reader.bytes.len());
    }

    #[test]
    fn scanner_accepts_each_exact_non_byte_ceiling() {
        let input = NativePath::new(PathBuf::from("/tmp/library-import.json"));
        let string = format!(
            r#"{{"x":"{}"}}"#,
            "a".repeat(MAX_IMPORT_STRING_BYTES as usize)
        );
        JsonScanner::new(string.as_bytes(), &input).scan().unwrap();

        let number = format!(
            r#"{{"x":{}}}"#,
            "1".repeat(MAX_IMPORT_NUMBER_BYTES as usize)
        );
        JsonScanner::new(number.as_bytes(), &input).scan().unwrap();

        let mut depth = String::from(r#"{"x":"#);
        for _ in 0..7 {
            depth.push('[');
        }
        depth.push('0');
        for _ in 0..7 {
            depth.push(']');
        }
        depth.push('}');
        JsonScanner::new(depth.as_bytes(), &input).scan().unwrap();

        let entries = format!(
            r#"{{"format_version":1,"entries":[{}]}}"#,
            std::iter::repeat_n("{}", MAX_PORTABLE_LIBRARY_ENTRIES as usize)
                .collect::<Vec<_>>()
                .join(",")
        );
        JsonScanner::new(entries.as_bytes(), &input).scan().unwrap();

        let values = format!(
            r#"{{"x":[{}]}}"#,
            std::iter::repeat_n("0", MAX_IMPORT_VALUES as usize - 3)
                .collect::<Vec<_>>()
                .join(",")
        );
        JsonScanner::new(values.as_bytes(), &input).scan().unwrap();
    }

    #[test]
    fn reader_reports_the_first_byte_overage_exactly() {
        let temporary = tempdir().unwrap();
        let input = NativePath::new(temporary.path().join("oversized.json"));
        let mut bytes = b"{}".to_vec();
        bytes.resize(MAX_PORTABLE_LIBRARY_DOCUMENT_BYTES as usize, b' ');
        fs::write(input.as_path(), bytes).unwrap();
        let store = PortableLibraryTransferStore::new();
        assert_eq!(
            store.read_input(&input).unwrap().len(),
            MAX_PORTABLE_LIBRARY_DOCUMENT_BYTES as usize
        );
        OpenOptions::new()
            .append(true)
            .open(input.as_path())
            .unwrap()
            .write_all(b" ")
            .unwrap();
        match store.read_input(&input).unwrap_err() {
            AppError::LibraryInputLimit {
                limit_kind,
                measured,
                allowed,
                ..
            } => {
                assert_eq!(limit_kind, "library_import_bytes");
                assert_eq!(measured, MAX_PORTABLE_LIBRARY_DOCUMENT_BYTES + 1);
                assert_eq!(allowed, MAX_PORTABLE_LIBRARY_DOCUMENT_BYTES);
            }
            error => panic!("expected input byte limit, got {error:?}"),
        }
    }

    struct PublicationRenameFailure {
        output: PathBuf,
    }

    impl TransferWriteHooks for PublicationRenameFailure {
        fn after_publication_link_before_rename(
            &self,
            _output_parent: &Path,
        ) -> Result<(), AppError> {
            if self.output.exists() {
                fs::remove_file(&self.output).unwrap();
            }
            fs::create_dir(&self.output).unwrap();
            fs::write(self.output.join("preserve"), b"external directory").unwrap();
            Ok(())
        }
    }

    struct AbsentOutputBeforePublication {
        output: PathBuf,
    }

    impl TransferWriteHooks for AbsentOutputBeforePublication {
        fn after_publication_link_before_rename(
            &self,
            _output_parent: &Path,
        ) -> Result<(), AppError> {
            assert!(matches!(
                fs::symlink_metadata(&self.output),
                Err(error) if error.kind() == io::ErrorKind::NotFound
            ));
            Ok(())
        }
    }

    #[test]
    fn export_keeps_an_absent_output_absent_until_no_clobber_publish() {
        let temporary = tempdir().unwrap();
        let output_parent = temporary.path().join("output");
        let output = NativePath::new(output_parent.join("library.json"));
        fs::create_dir(&output_parent).unwrap();
        let store = PortableLibraryTransferStore::with_write_hooks(
            Arc::new(TestEnvironment::with_roots(temporary.path())),
            Arc::new(XdgRootResolver),
            Arc::new(AbsentOutputBeforePublication {
                output: output.as_path().to_path_buf(),
            }),
        );

        store.write_export(&output, &document()).unwrap();

        assert!(fs::metadata(output.as_path()).unwrap().is_file());
    }

    struct PublicationLinkReplacement {
        replacement: PathBuf,
    }

    impl TransferWriteHooks for PublicationLinkReplacement {
        fn after_publication_link_before_rename(
            &self,
            output_parent: &Path,
        ) -> Result<(), AppError> {
            let publication = fs::read_dir(output_parent)
                .unwrap()
                .map(|entry| entry.unwrap().path())
                .find(|path| {
                    path.file_name().is_some_and(|name| {
                        name.to_string_lossy().starts_with(".skilload-publish-")
                    })
                })
                .unwrap();
            fs::remove_file(&publication).unwrap();
            fs::hard_link(&self.replacement, publication).unwrap();
            Ok(())
        }
    }

    #[test]
    fn export_rejects_a_replaced_publication_link_before_rename() {
        let temporary = tempdir().unwrap();
        let output_parent = temporary.path().join("output");
        let output = NativePath::new(output_parent.join("library.json"));
        let replacement = temporary.path().join("replacement.json");
        fs::create_dir(&output_parent).unwrap();
        fs::write(output.as_path(), b"old output").unwrap();
        fs::write(&replacement, b"replacement bytes").unwrap();
        let store = PortableLibraryTransferStore::with_write_hooks(
            Arc::new(TestEnvironment::with_roots(temporary.path())),
            Arc::new(XdgRootResolver),
            Arc::new(PublicationLinkReplacement {
                replacement: replacement.clone(),
            }),
        );

        let error = store.write_export(&output, &document()).unwrap_err();

        assert_eq!(error.code(), "validation_failed");
        assert_eq!(fs::read(output.as_path()).unwrap(), b"old output");
        assert_eq!(fs::read(&replacement).unwrap(), b"replacement bytes");
    }

    struct PublicationLinkReplacementAfterIdentityCheck {
        replacement: PathBuf,
    }

    impl TransferWriteHooks for PublicationLinkReplacementAfterIdentityCheck {
        fn after_publication_identity_check_before_exchange(
            &self,
            output_parent: &Path,
        ) -> Result<(), AppError> {
            let publication = fs::read_dir(output_parent)
                .unwrap()
                .map(|entry| entry.unwrap().path())
                .find(|path| {
                    path.file_name().is_some_and(|name| {
                        name.to_string_lossy().starts_with(".skilload-publish-")
                    })
                })
                .unwrap();
            fs::remove_file(&publication).unwrap();
            fs::hard_link(&self.replacement, publication).unwrap();
            Ok(())
        }
    }

    #[test]
    fn export_restores_the_old_output_when_publication_changes_after_final_check() {
        let temporary = tempdir().unwrap();
        let output_parent = temporary.path().join("output");
        let output = NativePath::new(output_parent.join("library.json"));
        let replacement = temporary.path().join("replacement.json");
        fs::create_dir(&output_parent).unwrap();
        fs::write(output.as_path(), b"old output").unwrap();
        fs::write(&replacement, b"replacement bytes").unwrap();
        let store = PortableLibraryTransferStore::with_write_hooks(
            Arc::new(TestEnvironment::with_roots(temporary.path())),
            Arc::new(XdgRootResolver),
            Arc::new(PublicationLinkReplacementAfterIdentityCheck {
                replacement: replacement.clone(),
            }),
        );

        let error = store.write_export(&output, &document()).unwrap_err();

        assert_eq!(error.code(), "validation_failed");
        assert_eq!(fs::read(output.as_path()).unwrap(), b"old output");
        assert_eq!(fs::read(&replacement).unwrap(), b"replacement bytes");
    }

    struct PublicationEntryReplacementAfterExchange {
        replacement: PathBuf,
    }

    impl TransferWriteHooks for PublicationEntryReplacementAfterExchange {
        fn after_existing_output_exchange_before_cleanup(
            &self,
            output_parent: &Path,
        ) -> Result<(), AppError> {
            let publication = fs::read_dir(output_parent)
                .unwrap()
                .map(|entry| entry.unwrap().path())
                .find(|path| {
                    path.file_name().is_some_and(|name| {
                        name.to_string_lossy().starts_with(".skilload-publish-")
                    })
                })
                .unwrap();
            fs::remove_file(&publication).unwrap();
            fs::hard_link(&self.replacement, publication).unwrap();
            Ok(())
        }
    }

    #[test]
    fn export_preserves_a_replaced_publication_entry_after_exchange() {
        let temporary = tempdir().unwrap();
        let output_parent = temporary.path().join("output");
        let output = NativePath::new(output_parent.join("library.json"));
        let replacement = temporary.path().join("replacement.json");
        fs::create_dir(&output_parent).unwrap();
        fs::write(output.as_path(), b"old output").unwrap();
        fs::write(&replacement, b"replacement bytes").unwrap();
        let expected = document().serialize_for_transfer().unwrap();
        let store = PortableLibraryTransferStore::with_write_hooks(
            Arc::new(TestEnvironment::with_roots(temporary.path())),
            Arc::new(XdgRootResolver),
            Arc::new(PublicationEntryReplacementAfterExchange {
                replacement: replacement.clone(),
            }),
        );

        store.write_export(&output, &document()).unwrap();

        assert_eq!(fs::read(output.as_path()).unwrap(), expected);
        let publication = fs::read_dir(&output_parent)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .find(|path| {
                path.file_name()
                    .is_some_and(|name| name.to_string_lossy().starts_with(".skilload-publish-"))
            })
            .unwrap();
        assert_eq!(fs::read(publication).unwrap(), b"replacement bytes");
    }

    #[test]
    fn export_removes_publication_link_when_rename_fails() {
        let temporary = tempdir().unwrap();
        let output_parent = temporary.path().join("output");
        let output = NativePath::new(output_parent.join("library.json"));
        fs::create_dir(&output_parent).unwrap();
        let store = PortableLibraryTransferStore::with_write_hooks(
            Arc::new(TestEnvironment::with_roots(temporary.path())),
            Arc::new(XdgRootResolver),
            Arc::new(PublicationRenameFailure {
                output: output.as_path().to_path_buf(),
            }),
        );

        let error = store.write_export(&output, &document()).unwrap_err();

        assert_eq!(error.code(), "validation_failed");
        assert!(output.as_path().is_dir());
        assert_eq!(
            fs::read(output.as_path().join("preserve")).unwrap(),
            b"external directory"
        );
        let staging_artifacts = fs::read_dir(&output_parent)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .filter(|name| {
                let name = name.to_string_lossy();
                name.starts_with(".skilload-library-") || name.starts_with(".skilload-publish-")
            })
            .collect::<Vec<_>>();
        assert!(staging_artifacts.is_empty(), "{staging_artifacts:?}");
    }

    struct AfterRenameFailure;

    impl TransferWriteHooks for AfterRenameFailure {
        fn after_rename_before_parent_sync(&self) -> Result<(), AppError> {
            Err(AppError::Internal {
                incident_id: "after-export-rename".to_owned(),
            })
        }
    }

    #[test]
    fn export_reports_post_rename_failure_without_claiming_old_output() {
        let temporary = tempdir().unwrap();
        let output_directory = temporary.path().join("output");
        fs::create_dir(&output_directory).unwrap();
        let output = NativePath::new(output_directory.join("library.json"));
        fs::write(output.as_path(), b"old output").unwrap();
        let store = PortableLibraryTransferStore::with_write_hooks(
            Arc::new(TestEnvironment::with_roots(temporary.path())),
            Arc::new(XdgRootResolver),
            Arc::new(AfterRenameFailure),
        );
        let error = store.write_export(&output, &document()).unwrap_err();
        assert_eq!(error.code(), "internal_invariant");
        assert_ne!(fs::read(output.as_path()).unwrap(), b"old output");
    }

    struct OutputReplacementAfterRename {
        output: PathBuf,
        displaced: PathBuf,
    }

    impl TransferWriteHooks for OutputReplacementAfterRename {
        fn after_rename_before_parent_sync(&self) -> Result<(), AppError> {
            fs::rename(&self.output, &self.displaced).unwrap();
            fs::write(&self.output, b"foreign output").unwrap();
            Ok(())
        }
    }

    #[test]
    fn export_rejects_an_output_replaced_before_final_parent_sync() {
        let temporary = tempdir().unwrap();
        let output_directory = temporary.path().join("output");
        fs::create_dir(&output_directory).unwrap();
        let output = NativePath::new(output_directory.join("library.json"));
        let store = PortableLibraryTransferStore::with_write_hooks(
            Arc::new(TestEnvironment::with_roots(temporary.path())),
            Arc::new(XdgRootResolver),
            Arc::new(OutputReplacementAfterRename {
                output: output.as_path().to_path_buf(),
                displaced: output_directory.join("displaced-library.json"),
            }),
        );

        let error = store.write_export(&output, &document()).unwrap_err();

        assert!(matches!(
            error,
            AppError::Validation { constraint, .. }
                if constraint == "library_export_staging_identity_drift"
        ));
        assert_eq!(fs::read(output.as_path()).unwrap(), b"foreign output");
    }
    struct ParentReplacementAfterValidation {
        output_parent: PathBuf,
        displaced_parent: PathBuf,
        protected_data: PathBuf,
    }

    impl TransferWriteHooks for ParentReplacementAfterValidation {
        fn after_final_output_validation_before_publish(
            &self,
            _output_parent: &Path,
        ) -> Result<(), AppError> {
            fs::rename(&self.output_parent, &self.displaced_parent).unwrap();
            symlink(&self.protected_data, &self.output_parent).unwrap();
            Ok(())
        }
    }

    #[test]
    fn export_does_not_publish_through_a_replaced_parent_directory() {
        let temporary = tempdir().unwrap();
        let protected_data = temporary.path().join("data/skilload");
        fs::create_dir_all(&protected_data).unwrap();
        let database = protected_data.join("skilload.db");
        fs::write(&database, b"protected database").unwrap();
        let output_parent = temporary.path().join("output");
        let displaced_parent = temporary.path().join("displaced-output");
        fs::create_dir(&output_parent).unwrap();
        let output = NativePath::new(output_parent.join("library.json"));
        let store = PortableLibraryTransferStore::with_write_hooks(
            Arc::new(TestEnvironment::with_roots(temporary.path())),
            Arc::new(XdgRootResolver),
            Arc::new(ParentReplacementAfterValidation {
                output_parent: output_parent.clone(),
                displaced_parent: displaced_parent.clone(),
                protected_data: protected_data.clone(),
            }),
        );

        let error = store.write_export(&output, &document()).unwrap_err();
        assert_eq!(error.code(), "validation_failed");
        assert_eq!(fs::read(database).unwrap(), b"protected database");
        assert!(!protected_data.join("library.json").exists());
        assert!(!displaced_parent.join("library.json").exists());
    }

    struct OutputReplacementAfterFinalValidation {
        output: PathBuf,
        displaced: PathBuf,
        replacement: PathBuf,
    }

    impl TransferWriteHooks for OutputReplacementAfterFinalValidation {
        fn after_final_output_validation_before_publish(
            &self,
            _output_parent: &Path,
        ) -> Result<(), AppError> {
            fs::rename(&self.output, &self.displaced).unwrap();
            fs::hard_link(&self.replacement, &self.output).unwrap();
            Ok(())
        }
    }

    #[test]
    fn export_does_not_replace_an_output_changed_after_final_validation() {
        let temporary = tempdir().unwrap();
        let output_parent = temporary.path().join("output");
        let output = NativePath::new(output_parent.join("library.json"));
        let displaced = output_parent.join("displaced-library.json");
        let replacement = temporary.path().join("replacement.json");
        fs::create_dir(&output_parent).unwrap();
        fs::write(output.as_path(), b"old output").unwrap();
        fs::write(&replacement, b"foreign output").unwrap();
        let store = PortableLibraryTransferStore::with_write_hooks(
            Arc::new(TestEnvironment::with_roots(temporary.path())),
            Arc::new(XdgRootResolver),
            Arc::new(OutputReplacementAfterFinalValidation {
                output: output.as_path().to_path_buf(),
                displaced,
                replacement: replacement.clone(),
            }),
        );

        let error = store.write_export(&output, &document()).unwrap_err();

        assert_eq!(error.code(), "validation_failed");
        assert_eq!(fs::read(output.as_path()).unwrap(), b"foreign output");
        assert_eq!(fs::read(&replacement).unwrap(), b"foreign output");
    }
    struct StagingReplacementAfterIdentityCheck {
        replacement: PathBuf,
    }

    impl TransferWriteHooks for StagingReplacementAfterIdentityCheck {
        fn after_staging_identity_check_before_publish(
            &self,
            output_parent: &Path,
        ) -> Result<(), AppError> {
            let staging = fs::read_dir(output_parent)
                .unwrap()
                .map(|entry| entry.unwrap().path())
                .find(|path| {
                    path.file_name().is_some_and(|name| {
                        name.to_string_lossy().starts_with(".skilload-library-")
                    })
                })
                .unwrap();
            fs::remove_file(&staging).unwrap();
            fs::hard_link(&self.replacement, &staging).unwrap();
            Ok(())
        }
    }

    #[test]
    fn export_reports_staging_replacement_after_identity_check() {
        let temporary = tempdir().unwrap();
        let output_parent = temporary.path().join("output");
        let output = NativePath::new(output_parent.join("library.json"));
        let replacement = temporary.path().join("replacement.json");
        fs::create_dir(&output_parent).unwrap();
        fs::write(output.as_path(), b"old output").unwrap();
        fs::write(&replacement, b"replacement bytes").unwrap();
        let store = PortableLibraryTransferStore::with_write_hooks(
            Arc::new(TestEnvironment::with_roots(temporary.path())),
            Arc::new(XdgRootResolver),
            Arc::new(StagingReplacementAfterIdentityCheck {
                replacement: replacement.clone(),
            }),
        );

        let error = store.write_export(&output, &document()).unwrap_err();
        assert_eq!(error.code(), "validation_failed");
        assert_eq!(fs::read(output.as_path()).unwrap(), b"old output");
        assert_eq!(fs::read(replacement).unwrap(), b"replacement bytes");
    }

    #[test]
    fn export_preserves_native_symlink_parent_dotdot_semantics() {
        let temporary = tempdir().unwrap();
        let physical_parent = temporary.path().join("physical");
        let physical_child = physical_parent.join("nested");
        let symlink_parent = temporary.path().join("symlink-parent");
        fs::create_dir_all(&physical_child).unwrap();
        symlink(&physical_child, &symlink_parent).unwrap();
        let output = NativePath::new(symlink_parent.join("..").join("library.json"));
        let store = PortableLibraryTransferStore::with_environment(
            Arc::new(TestEnvironment::with_roots(temporary.path())),
            Arc::new(XdgRootResolver),
        );

        store.write_export(&output, &document()).unwrap();

        assert!(physical_parent.join("library.json").is_file());
        assert!(!temporary.path().join("library.json").exists());
    }

    #[test]
    fn export_io_uses_a_typed_native_output_path() {
        let output = NativePath::new(PathBuf::from(OsString::from_vec(
            b"/tmp/library-output-\xff.json".to_vec(),
        )));

        let error = export_io(
            &output,
            "write export staging file",
            io::Error::other("fault"),
        );

        assert!(matches!(
            error,
            AppError::Validation {
                path: Some(path),
                ..
            } if path == output
        ));
    }
}
