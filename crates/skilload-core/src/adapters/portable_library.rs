use crate::adapters::xdg::{SystemEnvironment, XdgRootResolver};
use crate::domain::configuration::{NativePath, normalize_absolute};
use crate::domain::library::PortableLibraryDocument;
use crate::error::AppError;
use crate::ports::configuration::{Environment, ResolvedRoots, StateRootResolver};
use crate::ports::library::LibraryTransferStore;
use rustix::fs::renameat;
use std::collections::HashSet;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tempfile::{Builder, NamedTempFile};

const MAX_IMPORT_BYTES: u64 = 67_108_864;
const MAX_IMPORT_ENTRIES: u64 = 10_000;
const MAX_IMPORT_VALUES: u64 = 1_000_000;
const MAX_IMPORT_DEPTH: u64 = 8;
const MAX_IMPORT_STRING_BYTES: u64 = 1_048_576;
const MAX_IMPORT_NUMBER_BYTES: u64 = 128;

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
        let mut file = open_regular_input(input, || {})?;
        let mut bytes = Vec::with_capacity(64 * 1024);
        let mut buffer = [0_u8; 64 * 1024];
        loop {
            let remaining = (MAX_IMPORT_BYTES + 1).saturating_sub(bytes.len() as u64);
            let chunk = remaining.min(buffer.len() as u64) as usize;
            let read = file.read(&mut buffer[..chunk]).map_err(|error| {
                AppError::validation(
                    format!("library_import_read_failed: {error}"),
                    Some(input.clone()),
                )
            })?;
            if read == 0 {
                break;
            }
            let next = bytes.len() as u64 + read as u64;
            if next > MAX_IMPORT_BYTES {
                return Err(AppError::input_limit(
                    "library_import_bytes",
                    next,
                    MAX_IMPORT_BYTES,
                    input.clone(),
                ));
            }
            bytes.extend_from_slice(&buffer[..read]);
        }
        Ok(bytes)
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

        let mut document = document.clone();
        document.sort_deterministically()?;
        let bytes = serde_json::to_vec(&document).map_err(|error| {
            AppError::invalid_state(
                "library_export",
                format!("cannot serialize portable document: {error}"),
                ["a serializable LibraryExportData document"],
            )
        })?;

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
        self.write_hooks
            .after_final_output_validation_before_publish(&parent.path)?;
        publish_staging(&mut staging, &parent, output_path, output)?;
        self.write_hooks.after_rename_before_parent_sync()?;
        parent.revalidate(output)?;
        parent
            .directory
            .sync_all()
            .map_err(|error| export_io(output, "sync export parent directory", error))?;
        parent.revalidate(output)?;
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
        JsonScanner::new(&bytes, input).scan()?;
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
        if metadata.file_type().is_symlink()
            || !metadata.file_type().is_dir()
            || metadata_identity(&metadata) != self.identity
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

fn publish_staging(
    staging: &mut NamedTempFile,
    parent: &ValidatedOutputParent,
    output_path: &Path,
    output: &NativePath,
) -> Result<(), AppError> {
    let staging_name = staging.path().file_name().ok_or_else(|| {
        AppError::validation("library_export_staging_has_no_name", Some(output.clone()))
    })?;
    let output_name = output_path.file_name().ok_or_else(|| {
        AppError::validation("library_export_output_has_no_name", Some(output.clone()))
    })?;
    renameat(
        &parent.directory,
        staging_name,
        &parent.directory,
        output_name,
    )
    .map_err(|error| export_io(output, "atomically replace export output", error.into()))?;
    staging.disable_cleanup(true);
    Ok(())
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

    for protected in protected_paths(roots) {
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

fn protected_paths(roots: &ResolvedRoots) -> [PathBuf; 4] {
    let database = roots.data.effective.join("skilload.db");
    [
        database.clone(),
        database.with_file_name("skilload.db-wal"),
        database.with_file_name("skilload.db-shm"),
        roots.state.effective.join("locks/database.lock"),
    ]
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
        Ok(normalize_absolute(path))
    } else {
        Ok(normalize_absolute(&std::env::current_dir()?.join(path)))
    }
}

fn metadata_identity(metadata: &fs::Metadata) -> (u64, u64) {
    (metadata.dev(), metadata.ino())
}

fn same_identity(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    metadata_identity(left) == metadata_identity(right)
}

fn export_io(output: &NativePath, action: &str, error: io::Error) -> AppError {
    AppError::invalid_state(
        "library_export",
        format!("{action}: {error}"),
        [output.as_path().display().to_string()],
    )
}

struct JsonScanner<'a> {
    bytes: &'a [u8],
    position: usize,
    values: u64,
    entries: u64,
    input: &'a NativePath,
}

impl<'a> JsonScanner<'a> {
    fn new(bytes: &'a [u8], input: &'a NativePath) -> Self {
        Self {
            bytes,
            position: 0,
            values: 0,
            entries: 0,
            input,
        }
    }

    fn scan(mut self) -> Result<(), AppError> {
        self.skip_whitespace();
        if self.peek() == Some(b'{') {
            self.parse_object(1, true)?;
        } else {
            self.parse_value(0)?;
        }
        self.skip_whitespace();
        if self.position != self.bytes.len() {
            return Err(self.malformed());
        }
        Ok(())
    }

    fn parse_value(&mut self, parent_depth: u64) -> Result<(), AppError> {
        self.skip_whitespace();
        match self.peek() {
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
        self.skip_whitespace();
        let mut keys = HashSet::new();
        if self.consume_byte(b'}') {
            return Ok(());
        }
        loop {
            self.skip_whitespace();
            if self.peek() != Some(b'"') {
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
            self.skip_whitespace();
            self.expect_byte(b':')?;
            if root && key == "entries" {
                self.parse_entries_array(depth + 1)?;
            } else {
                self.parse_value(depth)?;
            }
            self.skip_whitespace();
            if self.consume_byte(b'}') {
                return Ok(());
            }
            self.expect_byte(b',')?;
        }
    }

    fn parse_array(&mut self, depth: u64) -> Result<(), AppError> {
        self.ensure_depth(depth)?;
        self.count_value()?;
        self.expect_byte(b'[')?;
        self.skip_whitespace();
        if self.consume_byte(b']') {
            return Ok(());
        }
        loop {
            self.parse_value(depth)?;
            self.skip_whitespace();
            if self.consume_byte(b']') {
                return Ok(());
            }
            self.expect_byte(b',')?;
        }
    }

    fn parse_entries_array(&mut self, depth: u64) -> Result<(), AppError> {
        self.ensure_depth(depth)?;
        self.count_value()?;
        self.expect_byte(b'[')?;
        self.skip_whitespace();
        if self.consume_byte(b']') {
            return Ok(());
        }
        loop {
            self.skip_whitespace();
            if self.peek() == Some(b'{') {
                self.entries += 1;
                if self.entries > MAX_IMPORT_ENTRIES {
                    return Err(self.limit(
                        "library_import_entries",
                        self.entries,
                        MAX_IMPORT_ENTRIES,
                    ));
                }
                self.parse_object(depth + 1, false)?;
            } else {
                self.parse_value(depth)?;
            }
            self.skip_whitespace();
            if self.consume_byte(b']') {
                return Ok(());
            }
            self.expect_byte(b',')?;
        }
    }

    fn parse_string(&mut self) -> Result<String, AppError> {
        self.expect_byte(b'"')?;
        let mut value = String::new();
        loop {
            let Some(byte) = self.peek() else {
                return Err(self.malformed());
            };
            match byte {
                b'"' => {
                    self.position += 1;
                    return Ok(value);
                }
                b'\\' => {
                    self.position += 1;
                    let escaped = self.next_byte().ok_or_else(|| self.malformed())?;
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
                    self.position += 1;
                    self.push_string_character(&mut value, byte as char)?;
                }
                _ => {
                    let width = utf8_width(byte).ok_or_else(|| self.malformed())?;
                    if self.position + width > self.bytes.len() {
                        return Err(self.malformed());
                    }
                    let decoded =
                        std::str::from_utf8(&self.bytes[self.position..self.position + width])
                            .map_err(|_| self.malformed())?;
                    let character = decoded.chars().next().ok_or_else(|| self.malformed())?;
                    self.position += width;
                    self.push_string_character(&mut value, character)?;
                }
            }
        }
    }

    fn parse_hex_escape(&mut self) -> Result<u32, AppError> {
        let mut value = 0_u32;
        for _ in 0..4 {
            let byte = self.next_byte().ok_or_else(|| self.malformed())?;
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
        self.consume_byte(b'-');
        match self.next_byte() {
            Some(b'0') => {}
            Some(b'1'..=b'9') => {
                while matches!(self.peek(), Some(b'0'..=b'9')) {
                    self.position += 1;
                }
            }
            _ => return Err(self.malformed()),
        }
        if self.consume_byte(b'.') {
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return Err(self.malformed());
            }
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.position += 1;
            }
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            self.position += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.position += 1;
            }
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return Err(self.malformed());
            }
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.position += 1;
            }
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
        if self.bytes.get(self.position..self.position + literal.len()) == Some(literal) {
            self.position += literal.len();
            Ok(())
        } else {
            Err(self.malformed())
        }
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
        AppError::input_limit(kind, measured, allowed, self.input.clone())
    }

    fn malformed(&self) -> AppError {
        AppError::validation("library_import_json", Some(self.input.clone()))
    }

    fn skip_whitespace(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\n' | b'\r' | b'\t')) {
            self.position += 1;
        }
    }

    fn expect_byte(&mut self, expected: u8) -> Result<(), AppError> {
        self.skip_whitespace();
        if self.consume_byte(expected) {
            Ok(())
        } else {
            Err(self.malformed())
        }
    }

    fn consume_byte(&mut self, expected: u8) -> bool {
        if self.peek() == Some(expected) {
            self.position += 1;
            true
        } else {
            false
        }
    }

    fn next_byte(&mut self) -> Option<u8> {
        let value = self.peek()?;
        self.position += 1;
        Some(value)
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.position).copied()
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
    use std::os::unix::fs::symlink;
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

    fn scanner_error(bytes: &[u8]) -> AppError {
        let input = NativePath::new(PathBuf::from("/tmp/library-import.json"));
        JsonScanner::new(bytes, &input).scan().unwrap_err()
    }

    fn assert_limit(bytes: &[u8], kind: &str) {
        match scanner_error(bytes) {
            AppError::InputLimit {
                limit_kind,
                allowed,
                ..
            } => {
                assert_eq!(limit_kind, kind);
                assert_eq!(
                    allowed,
                    match kind {
                        "library_import_entries" => MAX_IMPORT_ENTRIES,
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
            std::iter::repeat_n("{}", MAX_IMPORT_ENTRIES as usize + 1)
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
            std::iter::repeat_n("{}", MAX_IMPORT_ENTRIES as usize)
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
        fs::write(input.as_path(), vec![b' '; MAX_IMPORT_BYTES as usize]).unwrap();
        let store = PortableLibraryTransferStore::new();
        assert_eq!(
            store.read_input(&input).unwrap().len(),
            MAX_IMPORT_BYTES as usize
        );
        OpenOptions::new()
            .append(true)
            .open(input.as_path())
            .unwrap()
            .write_all(b" ")
            .unwrap();
        match store.read_input(&input).unwrap_err() {
            AppError::InputLimit {
                limit_kind,
                measured,
                allowed,
                ..
            } => {
                assert_eq!(limit_kind, "library_import_bytes");
                assert_eq!(measured, MAX_IMPORT_BYTES + 1);
                assert_eq!(allowed, MAX_IMPORT_BYTES);
            }
            error => panic!("expected input byte limit, got {error:?}"),
        }
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
        assert!(displaced_parent.join("library.json").is_file());
    }
}
