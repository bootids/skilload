use crate::adapters::xdg::{SystemEnvironment, XdgRootResolver};
use crate::domain::configuration::{ConfigDocument, NativePath};
use crate::error::AppError;
use crate::ports::configuration::{
    ConfigBaseline, ConfigSource, ConfigurationStore, Environment, LoadedConfig, ResolvedRoots,
    StateRootResolver, StoreOutcome,
};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use tempfile::Builder;

pub(crate) const LOCK_WAIT: Duration = Duration::from_secs(2);
pub(crate) const LOCK_RETRY: Duration = Duration::from_millis(25);

pub struct FileConfigurationStore {
    environment: Arc<dyn Environment>,
    root_resolver: Arc<dyn StateRootResolver>,
    write_hooks: Arc<dyn WriteHooks>,
}

impl FileConfigurationStore {
    pub fn new() -> Self {
        Self {
            environment: Arc::new(SystemEnvironment),
            root_resolver: Arc::new(XdgRootResolver),
            write_hooks: Arc::new(NoopWriteHooks),
        }
    }

    pub fn with_environment(
        environment: Arc<dyn Environment>,
        root_resolver: Arc<dyn StateRootResolver>,
    ) -> Self {
        Self {
            environment,
            root_resolver,
            write_hooks: Arc::new(NoopWriteHooks),
        }
    }

    #[cfg(test)]
    fn with_write_hooks(
        environment: Arc<dyn Environment>,
        root_resolver: Arc<dyn StateRootResolver>,
        write_hooks: Arc<dyn WriteHooks>,
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

    fn roots_still_match(&self, expected: &ResolvedRoots) -> Result<bool, AppError> {
        Ok(self.resolve_roots()?.same_paths(expected))
    }

    fn load_from_roots(&self, roots: ResolvedRoots) -> Result<LoadedConfig, AppError> {
        let config_path = config_path(&roots);
        match fs::symlink_metadata(&config_path) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
                    return Err(AppError::invalid_state(
                        "configuration",
                        "config_path_is_not_a_regular_file",
                        ["a regular config.toml file"],
                    ));
                }
                let bytes = fs::read(&config_path).map_err(|error| {
                    environment_io("XDG_CONFIG_HOME", &config_path, "read config.toml", error)
                })?;
                let text = String::from_utf8(bytes.clone()).map_err(|_| {
                    AppError::invalid_state(
                        "configuration",
                        "config_file_is_not_valid_utf8",
                        ["a valid UTF-8 TOML configuration document"],
                    )
                })?;
                let document = ConfigDocument::from_toml(&text)?;
                Ok(LoadedConfig {
                    document,
                    baseline: ConfigBaseline {
                        roots,
                        source: ConfigSource::Present(bytes),
                    },
                })
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(LoadedConfig {
                document: ConfigDocument::default(),
                baseline: ConfigBaseline {
                    roots,
                    source: ConfigSource::Absent,
                },
            }),
            Err(error) => Err(environment_io(
                "XDG_CONFIG_HOME",
                &config_path,
                "inspect config.toml",
                error,
            )),
        }
    }

    fn lock(&self, roots: &ResolvedRoots) -> Result<File, AppError> {
        acquire_restrictive_lock(roots, "config.lock", "configuration")
    }

    fn write_document(
        &self,
        roots: &ResolvedRoots,
        desired: &ConfigDocument,
    ) -> Result<(), AppError> {
        let created_directories =
            ensure_restrictive_directory(&roots.config.effective, "XDG_CONFIG_HOME")?;
        let roots = self.root_resolver.revalidate(roots)?;
        let config_root = roots.config.effective.clone();
        let path = config_path(&roots);
        ensure_regular_destination(&path)?;

        let mut staging = Builder::new()
            .prefix(".skilload-config-")
            .suffix(".tmp")
            .tempfile_in(&config_root)
            .map_err(|error| {
                environment_io(
                    "XDG_CONFIG_HOME",
                    &config_root,
                    "create config staging file",
                    error,
                )
            })?;
        staging
            .as_file()
            .set_permissions(fs::Permissions::from_mode(0o600))
            .map_err(|error| {
                environment_io(
                    "XDG_CONFIG_HOME",
                    &path,
                    "restrict config staging file",
                    error,
                )
            })?;
        staging
            .write_all(desired.to_toml().as_bytes())
            .map_err(|error| {
                environment_io("XDG_CONFIG_HOME", &path, "write config staging file", error)
            })?;
        staging.as_file().sync_all().map_err(|error| {
            environment_io("XDG_CONFIG_HOME", &path, "sync config staging file", error)
        })?;
        self.write_hooks.before_rename()?;
        let roots = self.root_resolver.revalidate(&roots)?;
        let path = config_path(&roots);
        ensure_regular_destination(&path)?;
        staging.persist(&path).map_err(|error| {
            environment_io(
                "XDG_CONFIG_HOME",
                &path,
                "atomically replace config.toml",
                error.error,
            )
        })?;
        File::open(&roots.config.effective)
            .and_then(|directory| directory.sync_all())
            .map_err(|error| {
                environment_io(
                    "XDG_CONFIG_HOME",
                    &roots.config.effective,
                    "sync config directory",
                    error,
                )
            })?;
        sync_created_directory_entries(&created_directories, "XDG_CONFIG_HOME")?;
        self.write_hooks.after_rename_and_sync()?;
        Ok(())
    }
}

impl Default for FileConfigurationStore {
    fn default() -> Self {
        Self::new()
    }
}

impl ConfigurationStore for FileConfigurationStore {
    fn load(&self) -> Result<LoadedConfig, AppError> {
        self.load_from_roots(self.resolve_roots()?)
    }

    fn replace(
        &self,
        expected: &ConfigBaseline,
        desired: &ConfigDocument,
    ) -> Result<StoreOutcome, AppError> {
        if !self.roots_still_match(&expected.roots)? {
            return Ok(StoreOutcome::Stale);
        }
        let roots = self.root_resolver.revalidate(&expected.roots)?;
        let lock = self.lock(&roots)?;
        let result = (|| {
            if !self.roots_still_match(&expected.roots)? {
                return Ok(StoreOutcome::Stale);
            }
            let roots = self.root_resolver.revalidate(&roots)?;
            let current = self.load_from_roots(roots.clone())?;
            if current.baseline.source != expected.source {
                return Ok(StoreOutcome::Stale);
            }
            self.write_document(&roots, desired)?;
            Ok(StoreOutcome::Changed)
        })();
        let unlock_result = lock.unlock();
        if let Err(error) = unlock_result {
            return Err(environment_io(
                "XDG_STATE_HOME",
                &expected.roots.state.effective.join("locks/config.lock"),
                "unlock config.lock",
                error,
            ));
        }
        result
    }
}

trait WriteHooks: Send + Sync {
    fn before_rename(&self) -> Result<(), AppError> {
        Ok(())
    }

    fn after_rename_and_sync(&self) -> Result<(), AppError> {
        Ok(())
    }
}

struct NoopWriteHooks;

impl WriteHooks for NoopWriteHooks {}

fn config_path(roots: &ResolvedRoots) -> PathBuf {
    roots.config.effective.join("config.toml")
}

pub(crate) fn acquire_restrictive_lock(
    roots: &ResolvedRoots,
    lock_name: &str,
    lock_domain: &str,
) -> Result<File, AppError> {
    acquire_restrictive_lock_with_identity(roots, lock_name, lock_domain).map(|(file, _)| file)
}

pub(crate) fn acquire_restrictive_lock_with_identity(
    roots: &ResolvedRoots,
    lock_name: &str,
    lock_domain: &str,
) -> Result<(File, Option<(u64, u64)>), AppError> {
    acquire_restrictive_lock_with_identity_after_path_inspection(
        roots,
        lock_name,
        lock_domain,
        |_| {},
    )
}

fn acquire_restrictive_lock_with_identity_after_path_inspection(
    roots: &ResolvedRoots,
    lock_name: &str,
    lock_domain: &str,
    mut after_path_inspection: impl FnMut(&Path),
) -> Result<(File, Option<(u64, u64)>), AppError> {
    let state_root = &roots.state.effective;
    ensure_restrictive_directory(state_root, "XDG_STATE_HOME")?;
    let locks = state_root.join("locks");
    ensure_restrictive_directory(&locks, "XDG_STATE_HOME")?;
    let lock_path = locks.join(lock_name);
    let (file, created_lock, expected_identity) = match fs::symlink_metadata(&lock_path) {
        Ok(metadata) => {
            ensure_regular_lock_file(&lock_path, &metadata, lock_domain)?;
            let identity = metadata_identity(&metadata);
            after_path_inspection(&lock_path);
            (open_restrictive_lock(&lock_path)?, None, identity)
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            match create_restrictive_lock(&lock_path) {
                Ok(file) => {
                    let metadata = file.metadata().map_err(|error| {
                        environment_io(
                            "XDG_STATE_HOME",
                            &lock_path,
                            "inspect created durable lock",
                            error,
                        )
                    })?;
                    let identity = metadata_identity(&metadata);
                    (file, Some(identity), identity)
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                    let metadata = fs::symlink_metadata(&lock_path).map_err(|error| {
                        environment_io(
                            "XDG_STATE_HOME",
                            &lock_path,
                            "inspect raced durable lock",
                            error,
                        )
                    })?;
                    ensure_regular_lock_file(&lock_path, &metadata, lock_domain)?;
                    let identity = metadata_identity(&metadata);
                    after_path_inspection(&lock_path);
                    (open_restrictive_lock(&lock_path)?, None, identity)
                }
                Err(error) => {
                    return Err(environment_io(
                        "XDG_STATE_HOME",
                        &lock_path,
                        "open durable lock",
                        error,
                    ));
                }
            }
        }
        Err(error) => {
            return Err(environment_io(
                "XDG_STATE_HOME",
                &lock_path,
                "inspect durable lock",
                error,
            ));
        }
    };

    let lock_result: Result<(), AppError> = (|| {
        ensure_opened_lock_identity(&lock_path, expected_identity, &file, lock_domain)?;
        file.set_permissions(fs::Permissions::from_mode(0o600))
            .map_err(|error| {
                environment_io("XDG_STATE_HOME", &lock_path, "restrict durable lock", error)
            })?;

        let start = Instant::now();
        loop {
            match file.try_lock() {
                Ok(()) => {
                    ensure_opened_lock_identity(&lock_path, expected_identity, &file, lock_domain)?;
                    return Ok(());
                }
                Err(std::fs::TryLockError::WouldBlock) => {
                    if start.elapsed() >= LOCK_WAIT {
                        return Err(AppError::Busy {
                            lock_domain: lock_domain.to_owned(),
                            waited_ms: LOCK_WAIT.as_millis() as u64,
                        });
                    }
                    thread::sleep(LOCK_RETRY);
                }
                Err(std::fs::TryLockError::Error(error)) => {
                    return Err(environment_io(
                        "XDG_STATE_HOME",
                        &lock_path,
                        "lock durable lock",
                        error,
                    ));
                }
            }
        }
    })();
    if let Err(error) = lock_result {
        if !matches!(&error, AppError::Busy { .. })
            && let Some(identity) = created_lock
        {
            remove_created_lock(&lock_path, identity, &file);
        }
        drop(file);
        return Err(error);
    }

    Ok((file, created_lock))
}

fn open_restrictive_lock(path: &Path) -> Result<File, AppError> {
    let mut options = OpenOptions::new();
    options
        .read(true)
        .write(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW);
    options
        .open(path)
        .map_err(|error| environment_io("XDG_STATE_HOME", path, "open durable lock", error))
}

fn create_restrictive_lock(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options
        .create_new(true)
        .read(true)
        .write(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW);
    options.open(path)
}

fn ensure_regular_lock_file(
    _path: &Path,
    metadata: &fs::Metadata,
    lock_domain: &str,
) -> Result<(), AppError> {
    if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
        Err(AppError::invalid_state(
            lock_domain,
            "lock_path_is_not_a_regular_file",
            ["a regular durable lock file"],
        ))
    } else {
        Ok(())
    }
}

fn ensure_opened_lock_identity(
    path: &Path,
    expected_identity: (u64, u64),
    file: &File,
    lock_domain: &str,
) -> Result<(), AppError> {
    let path_metadata = fs::symlink_metadata(path)
        .map_err(|error| environment_io("XDG_STATE_HOME", path, "inspect durable lock", error))?;
    ensure_regular_lock_file(path, &path_metadata, lock_domain)?;
    let descriptor_metadata = file.metadata().map_err(|error| {
        environment_io("XDG_STATE_HOME", path, "inspect opened durable lock", error)
    })?;
    if metadata_identity(&path_metadata) != expected_identity
        || metadata_identity(&descriptor_metadata) != expected_identity
        || metadata_identity(&path_metadata) != metadata_identity(&descriptor_metadata)
    {
        return Err(AppError::invalid_state(
            lock_domain,
            "lock_path_identity_drift",
            ["the inspected durable lock file"],
        ));
    }
    Ok(())
}

pub(crate) fn remove_created_lock(path: &Path, created_identity: (u64, u64), handle: &File) {
    if fs::symlink_metadata(path)
        .ok()
        .zip(handle.metadata().ok())
        .is_some_and(|(metadata, handle_metadata)| {
            metadata.file_type().is_file()
                && !metadata.file_type().is_symlink()
                && metadata_identity(&metadata) == created_identity
                && metadata_identity(&handle_metadata) == created_identity
        })
    {
        let _ = fs::remove_file(path);
    }
}

fn metadata_identity(metadata: &fs::Metadata) -> (u64, u64) {
    (metadata.dev(), metadata.ino())
}

#[derive(Debug)]
pub(crate) struct CreatedDirectory {
    pub(crate) path: PathBuf,
    pub(crate) identity: (u64, u64),
    pub(crate) handle: File,
}

pub(crate) fn ensure_restrictive_directory(
    path: &Path,
    variable: &str,
) -> Result<Vec<CreatedDirectory>, AppError> {
    ensure_restrictive_directory_after_creation(path, variable, |_| Ok(()))
}

fn ensure_restrictive_directory_after_creation(
    path: &Path,
    variable: &str,
    mut after_creation: impl FnMut(&CreatedDirectory) -> Result<(), AppError>,
) -> Result<Vec<CreatedDirectory>, AppError> {
    let mut missing_directories = Vec::new();
    let mut ancestor = path;
    loop {
        match fs::symlink_metadata(ancestor) {
            Ok(metadata) => {
                ensure_real_directory(ancestor, &metadata, variable)?;
                break;
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                missing_directories.push(ancestor.to_path_buf());
                ancestor = ancestor.parent().ok_or_else(|| {
                    AppError::invalid_environment(
                        variable,
                        Some(NativePath::new(path.to_path_buf())),
                        "directory has no existing ancestor",
                    )
                })?;
            }
            Err(error) => {
                return Err(environment_io(
                    variable,
                    ancestor,
                    "inspect directory",
                    error,
                ));
            }
        }
    }

    let mut created_directories = Vec::new();
    let result = (|| {
        for directory in missing_directories.into_iter().rev() {
            if let Some(created_directory) = create_restrictive_directory(&directory, variable)? {
                created_directories.push(created_directory);
                after_creation(
                    created_directories
                        .last()
                        .expect("created directory was just appended"),
                )?;
            }
        }

        let metadata = fs::symlink_metadata(path)
            .map_err(|error| environment_io(variable, path, "inspect directory", error))?;
        ensure_real_directory(path, &metadata, variable)?;
        restrict_directory_permissions(path, variable)?;
        Ok(())
    })();
    if let Err(error) = result {
        cleanup_created_directories(&created_directories);
        return Err(error);
    }
    Ok(created_directories)
}

fn create_restrictive_directory(
    directory: &Path,
    variable: &str,
) -> Result<Option<CreatedDirectory>, AppError> {
    let created = match fs::create_dir(directory) {
        Ok(()) => true,
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => false,
        Err(error) => {
            return Err(environment_io(
                variable,
                directory,
                "create restrictive directory",
                error,
            ));
        }
    };
    let action = if created {
        "inspect created directory"
    } else {
        "inspect raced directory"
    };
    let metadata = fs::symlink_metadata(directory)
        .map_err(|error| environment_io(variable, directory, action, error))?;
    ensure_real_directory(directory, &metadata, variable)?;
    if !created {
        restrict_directory_permissions(directory, variable)?;
        return Ok(None);
    }

    let mut options = OpenOptions::new();
    options
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW);
    let handle = options
        .open(directory)
        .map_err(|error| environment_io(variable, directory, "open created directory", error))?;
    let handle_metadata = handle.metadata().map_err(|error| {
        environment_io(
            variable,
            directory,
            "inspect opened created directory",
            error,
        )
    })?;
    if metadata_identity(&metadata) != metadata_identity(&handle_metadata) {
        return Err(AppError::invalid_environment(
            variable,
            Some(NativePath::new(directory.to_path_buf())),
            "created directory identity drift",
        ));
    }
    handle
        .set_permissions(fs::Permissions::from_mode(0o700))
        .map_err(|error| {
            environment_io(
                variable,
                directory,
                "restrict created directory permissions",
                error,
            )
        })?;
    Ok(Some(CreatedDirectory {
        path: directory.to_path_buf(),
        identity: metadata_identity(&metadata),
        handle,
    }))
}

fn cleanup_created_directories(created_directories: &[CreatedDirectory]) {
    for directory in created_directories.iter().rev() {
        if fs::symlink_metadata(&directory.path)
            .ok()
            .zip(directory.handle.metadata().ok())
            .is_some_and(|(metadata, handle_metadata)| {
                metadata.file_type().is_dir()
                    && !metadata.file_type().is_symlink()
                    && metadata_identity(&metadata) == directory.identity
                    && metadata_identity(&handle_metadata) == directory.identity
            })
        {
            let _ = fs::remove_dir(&directory.path);
        }
    }
}

fn ensure_real_directory(
    path: &Path,
    metadata: &fs::Metadata,
    variable: &str,
) -> Result<(), AppError> {
    if metadata.file_type().is_symlink() || !metadata.file_type().is_dir() {
        return Err(AppError::invalid_environment(
            variable,
            Some(NativePath::new(path.to_path_buf())),
            "expected a real directory",
        ));
    }
    Ok(())
}

fn restrict_directory_permissions(path: &Path, variable: &str) -> Result<(), AppError> {
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
        .map_err(|error| environment_io(variable, path, "restrict directory permissions", error))
}

pub(crate) fn sync_created_directory_entries(
    created_directories: &[CreatedDirectory],
    variable: &str,
) -> Result<(), AppError> {
    for directory in created_directories.iter().rev() {
        let parent = directory.path.parent().ok_or_else(|| {
            AppError::invalid_environment(
                variable,
                Some(NativePath::new(directory.path.clone())),
                "new directory has no parent",
            )
        })?;
        File::open(parent)
            .and_then(|parent| parent.sync_all())
            .map_err(|error| {
                environment_io(
                    variable,
                    parent,
                    "sync newly created directory entry",
                    error,
                )
            })?;
    }
    Ok(())
}

fn ensure_regular_destination(path: &Path) -> Result<(), AppError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_file() && !metadata.file_type().is_symlink() => {
            Ok(())
        }
        Ok(_) => Err(AppError::invalid_state(
            "configuration",
            "config_path_is_not_a_regular_file",
            ["a regular config.toml file"],
        )),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(environment_io(
            "XDG_CONFIG_HOME",
            path,
            "inspect config.toml destination",
            error,
        )),
    }
}

pub(crate) fn environment_io(
    variable: &str,
    path: &Path,
    action: &str,
    error: io::Error,
) -> AppError {
    AppError::invalid_environment(
        variable,
        Some(NativePath::new(path.to_path_buf())),
        format!("cannot {action}: {error}"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::portable_library::PortableLibraryTransferStore;
    use crate::adapters::sqlite_library::SqliteLibraryRepository;
    use crate::application::Application;
    use crate::ports::configuration::Environment;
    use std::collections::BTreeMap;
    use std::ffi::OsString;
    use std::os::unix::fs::{MetadataExt, symlink};
    use std::sync::atomic::{AtomicBool, Ordering};
    use tempfile::tempdir;

    #[derive(Default)]
    struct TestEnvironment(BTreeMap<String, OsString>);

    impl TestEnvironment {
        fn isolated(root: &Path) -> Self {
            let mut values = BTreeMap::new();
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

    fn store(root: &Path) -> Arc<FileConfigurationStore> {
        Arc::new(FileConfigurationStore::with_environment(
            Arc::new(TestEnvironment::isolated(root)),
            Arc::new(XdgRootResolver),
        ))
    }

    fn application(store: Arc<FileConfigurationStore>) -> Application {
        Application::new(
            store,
            Arc::new(SqliteLibraryRepository::new()),
            Arc::new(PortableLibraryTransferStore::new()),
        )
    }

    #[test]
    fn absent_queries_and_unsets_create_no_roots() {
        let temporary = tempdir().unwrap();
        let application = application(store(temporary.path()));
        let entries = application.config_list().unwrap();
        assert_eq!(entries.entries.len(), 3);
        let mutation = application
            .config_unset(crate::ConfigKey::CodexExecutable)
            .unwrap();
        assert_eq!(mutation.outcome, crate::MutationOutcome::Unchanged);
        assert!(!temporary.path().join("config").exists());
        assert!(!temporary.path().join("state").exists());
    }

    #[test]
    fn mutation_is_atomic_and_idempotent_with_restrictive_modes() {
        let temporary = tempdir().unwrap();
        let application = application(store(temporary.path()));
        let first = application
            .config_set(crate::ConfigKey::CacheLimitBytes, "1073741824".into())
            .unwrap();
        assert_eq!(first.outcome, crate::MutationOutcome::Changed);
        let config = temporary.path().join("config/skilload/config.toml");
        let before = fs::read(&config).unwrap();
        let inode = fs::metadata(&config).unwrap().ino();
        let modified = fs::metadata(&config).unwrap().mtime();
        let repeat = application
            .config_set(crate::ConfigKey::CacheLimitBytes, "1073741824".into())
            .unwrap();
        assert_eq!(repeat.outcome, crate::MutationOutcome::Unchanged);
        assert_eq!(before, fs::read(&config).unwrap());
        assert_eq!(inode, fs::metadata(&config).unwrap().ino());
        assert_eq!(modified, fs::metadata(&config).unwrap().mtime());
        assert_eq!(fs::metadata(&config).unwrap().mode() & 0o777, 0o600);
        assert_eq!(
            fs::metadata(config.parent().unwrap()).unwrap().mode() & 0o777,
            0o700
        );
        assert_eq!(
            fs::metadata(temporary.path().join("state/skilload"))
                .unwrap()
                .mode()
                & 0o777,
            0o700
        );
    }

    #[test]
    fn restrictive_directories_restore_owner_search_permission() {
        let temporary = tempdir().unwrap();
        let directory = temporary.path().join("restrictive");
        fs::create_dir(&directory).unwrap();
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o600)).unwrap();

        ensure_restrictive_directory(&directory, "XDG_STATE_HOME").unwrap();

        assert_eq!(fs::metadata(&directory).unwrap().mode() & 0o777, 0o700);
    }

    #[test]
    fn raced_directory_entries_restore_owner_search_permission() {
        let temporary = tempdir().unwrap();
        let directory = temporary.path().join("raced");
        fs::create_dir(&directory).unwrap();
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o600)).unwrap();

        assert!(
            create_restrictive_directory(&directory, "XDG_STATE_HOME")
                .unwrap()
                .is_none()
        );

        assert_eq!(fs::metadata(&directory).unwrap().mode() & 0o777, 0o700);
    }

    #[test]
    fn restrictive_directory_rolls_back_partial_created_prefix() {
        let temporary = tempdir().unwrap();
        let directory = temporary.path().join("first/second");

        let error =
            ensure_restrictive_directory_after_creation(&directory, "XDG_STATE_HOME", |created| {
                if created.path.ends_with("second") {
                    Err(AppError::Internal {
                        incident_id: "after-second-directory-create".to_owned(),
                    })
                } else {
                    Ok(())
                }
            })
            .unwrap_err();

        assert_eq!(error.code(), "internal_invariant");
        assert!(!temporary.path().join("first").exists());
    }

    #[test]
    fn lock_rejects_replacement_after_path_inspection() {
        let temporary = tempdir().unwrap();
        let environment = TestEnvironment::isolated(temporary.path());
        let roots = XdgRootResolver.resolve(&environment).unwrap();
        let locks = roots.state.effective.join("locks");
        let lock = locks.join("database.lock");
        let displaced = temporary.path().join("displaced.lock");
        let replacement = temporary.path().join("replacement.lock");
        fs::create_dir_all(&locks).unwrap();
        fs::write(&lock, b"inspected lock").unwrap();
        fs::write(&replacement, b"replacement lock").unwrap();

        let error = acquire_restrictive_lock_with_identity_after_path_inspection(
            &roots,
            "database.lock",
            "database",
            |path| {
                fs::rename(path, &displaced).unwrap();
                fs::rename(&replacement, path).unwrap();
            },
        )
        .unwrap_err();

        assert!(matches!(
            error,
            AppError::InvalidState { state, .. } if state == "lock_path_identity_drift"
        ));
        assert_eq!(fs::read(lock).unwrap(), b"replacement lock");
    }

    #[test]
    fn invalid_documents_and_final_symlinks_are_preserved() {
        let temporary = tempdir().unwrap();
        let config_root = temporary.path().join("config/skilload");
        fs::create_dir_all(&config_root).unwrap();
        let config = config_root.join("config.toml");
        fs::write(&config, "version = 1\nunknown = true\n").unwrap();
        let original = fs::read(&config).unwrap();
        let application = application(store(temporary.path()));
        assert!(
            application
                .config_set(crate::ConfigKey::CacheLimitBytes, "1".into())
                .is_err()
        );
        assert_eq!(original, fs::read(&config).unwrap());

        fs::remove_file(&config).unwrap();
        let target = temporary.path().join("target.toml");
        fs::write(&target, "version = 1\n").unwrap();
        symlink(&target, &config).unwrap();
        assert!(application.config_list().is_err());
        assert_eq!(fs::read(&target).unwrap(), b"version = 1\n");
    }

    struct FailingHooks {
        before: AtomicBool,
        after: AtomicBool,
    }

    impl WriteHooks for FailingHooks {
        fn before_rename(&self) -> Result<(), AppError> {
            if self.before.load(Ordering::SeqCst) {
                return Err(AppError::Internal {
                    incident_id: "before-rename".to_owned(),
                });
            }
            Ok(())
        }

        fn after_rename_and_sync(&self) -> Result<(), AppError> {
            if self.after.load(Ordering::SeqCst) {
                return Err(AppError::Internal {
                    incident_id: "after-rename-sync".to_owned(),
                });
            }
            Ok(())
        }
    }

    struct RecreatedConfigRootHooks {
        config_root: PathBuf,
        retired_root: PathBuf,
    }

    impl WriteHooks for RecreatedConfigRootHooks {
        fn before_rename(&self) -> Result<(), AppError> {
            fs::rename(&self.config_root, &self.retired_root).map_err(|error| {
                environment_io(
                    "XDG_CONFIG_HOME",
                    &self.config_root,
                    "replace configuration root in test",
                    error,
                )
            })?;
            fs::create_dir(&self.config_root).map_err(|error| {
                environment_io(
                    "XDG_CONFIG_HOME",
                    &self.config_root,
                    "recreate configuration root in test",
                    error,
                )
            })
        }
    }

    #[test]
    fn writes_reject_recreated_configuration_root_after_initial_binding() {
        let temporary = tempdir().unwrap();
        let config_root = temporary.path().join("config/skilload");
        let retired_root = temporary.path().join("retired-config-root");
        let store = Arc::new(FileConfigurationStore::with_write_hooks(
            Arc::new(TestEnvironment::isolated(temporary.path())),
            Arc::new(XdgRootResolver),
            Arc::new(RecreatedConfigRootHooks {
                config_root: config_root.clone(),
                retired_root: retired_root.clone(),
            }),
        ));
        let application = application(store);

        assert!(
            application
                .config_set(crate::ConfigKey::CacheLimitBytes, "1".into())
                .is_err()
        );
        assert!(!config_root.join("config.toml").exists());
        assert!(!retired_root.join("config.toml").exists());
    }

    #[test]
    fn failpoints_preserve_old_bytes_before_rename_and_sync_before_after_failure() {
        let temporary = tempdir().unwrap();
        let hooks = Arc::new(FailingHooks {
            before: AtomicBool::new(false),
            after: AtomicBool::new(false),
        });
        let store = Arc::new(FileConfigurationStore::with_write_hooks(
            Arc::new(TestEnvironment::isolated(temporary.path())),
            Arc::new(XdgRootResolver),
            hooks.clone(),
        ));
        let application = application(store);
        application
            .config_set(crate::ConfigKey::CacheLimitBytes, "1".into())
            .unwrap();
        let config = temporary.path().join("config/skilload/config.toml");
        let old = fs::read(&config).unwrap();
        hooks.before.store(true, Ordering::SeqCst);
        assert!(
            application
                .config_set(crate::ConfigKey::ClaudeExecutable, "/opt/claude".into())
                .is_err()
        );
        assert_eq!(old, fs::read(&config).unwrap());
        hooks.before.store(false, Ordering::SeqCst);
        hooks.after.store(true, Ordering::SeqCst);
        assert!(
            application
                .config_set(crate::ConfigKey::ClaudeExecutable, "/opt/claude".into())
                .is_err()
        );
        let new = fs::read_to_string(&config).unwrap();
        assert!(new.contains("/opt/claude"));
        assert!(
            !temporary
                .path()
                .join("config/skilload/.skilload-config-leftover.tmp")
                .exists()
        );
    }
}
