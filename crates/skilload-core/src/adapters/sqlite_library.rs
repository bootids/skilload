use crate::adapters::configuration::{
    CreatedDirectory, acquire_restrictive_lock, acquire_restrictive_lock_with_identity,
    ensure_restrictive_directory, environment_io, sync_created_directory_entries,
};
use crate::adapters::xdg::{SystemEnvironment, XdgRootResolver};
use crate::domain::configuration::{MutationOutcome, NativePath};
use crate::domain::library::{
    LIBRARY_FORMAT_VERSION, LibraryImportOperation, LibraryImportResult, PortableLibraryDocument,
    PortableLibraryEntry,
};
use crate::domain::source::{RefKind, ResolvedSkill, SourceIdentity, parse_decimal_u64};
use crate::domain::unicode_15_1::normalize_tag;
use crate::error::{AppError, Conflict};
use crate::ports::configuration::{Environment, ResolvedRoots, StateRootResolver};
use crate::ports::library::LibraryRepository;
use rusqlite::{Connection, Error as SqlError, ErrorCode, OpenFlags, OptionalExtension, params};
use rustix::fs::{AtFlags, RenameFlags, fstat, renameat_with, statat, unlinkat};
use std::collections::HashMap;
use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::io;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tempfile::{Builder, NamedTempFile};

const SCHEMA_VERSION: u64 = 1;
const API_V1_UINT_MAX: i64 = 9_007_199_254_740_991;

pub struct SqliteLibraryRepository {
    environment: Arc<dyn Environment>,
    root_resolver: Arc<dyn StateRootResolver>,
    hooks: Arc<dyn PersistenceHooks>,
}

impl SqliteLibraryRepository {
    pub fn new() -> Self {
        Self {
            environment: Arc::new(SystemEnvironment),
            root_resolver: Arc::new(XdgRootResolver),
            hooks: Arc::new(NoopPersistenceHooks),
        }
    }

    pub fn with_environment(
        environment: Arc<dyn Environment>,
        root_resolver: Arc<dyn StateRootResolver>,
    ) -> Self {
        Self {
            environment,
            root_resolver,
            hooks: Arc::new(NoopPersistenceHooks),
        }
    }

    #[cfg(test)]
    fn with_hooks(
        environment: Arc<dyn Environment>,
        root_resolver: Arc<dyn StateRootResolver>,
        hooks: Arc<dyn PersistenceHooks>,
    ) -> Self {
        Self {
            environment,
            root_resolver,
            hooks,
        }
    }

    fn resolve_roots(&self) -> Result<ResolvedRoots, AppError> {
        self.root_resolver.resolve(self.environment.as_ref())
    }

    fn database_path(roots: &ResolvedRoots) -> PathBuf {
        roots.data.effective.join("skilload.db")
    }

    fn database_exists(path: &Path) -> Result<bool, AppError> {
        match fs::symlink_metadata(path) {
            Ok(metadata)
                if metadata.file_type().is_file() && !metadata.file_type().is_symlink() =>
            {
                Ok(true)
            }
            Ok(_) => Err(AppError::invalid_state(
                "library_database",
                "database_path_is_not_a_regular_file",
                ["a regular data/skilload.db file"],
            )),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(environment_io(
                "XDG_DATA_HOME",
                path,
                "inspect skilload.db",
                error,
            )),
        }
    }
    fn database_identity_drift() -> AppError {
        AppError::invalid_state(
            "library_database",
            "database_identity_drift",
            ["the planned regular database file"],
        )
    }

    fn existing_database_metadata(path: &Path) -> Result<fs::Metadata, AppError> {
        match fs::symlink_metadata(path) {
            Ok(metadata)
                if metadata.file_type().is_file() && !metadata.file_type().is_symlink() =>
            {
                Ok(metadata)
            }
            Ok(_) => Err(Self::database_identity_drift()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                Err(Self::database_identity_drift())
            }
            Err(error) => Err(environment_io(
                "XDG_DATA_HOME",
                path,
                "inspect planned skilload.db",
                error,
            )),
        }
    }

    fn revalidate_database_identity(path: &Path, expected: (u64, u64)) -> Result<(), AppError> {
        if metadata_identity(&Self::existing_database_metadata(path)?) == expected {
            Ok(())
        } else {
            Err(Self::database_identity_drift())
        }
    }

    fn open_existing_database(
        path: &Path,
        flags: OpenFlags,
    ) -> Result<(Connection, (u64, u64)), AppError> {
        let identity = metadata_identity(&Self::existing_database_metadata(path)?);
        let connection = Connection::open_with_flags(path, flags | OpenFlags::SQLITE_OPEN_NOFOLLOW)
            .map_err(|error| database_error(path, error))?;
        Self::revalidate_database_identity(path, identity)?;
        Ok((connection, identity))
    }

    fn read_existing(&self, path: &Path) -> Result<Vec<PortableLibraryEntry>, AppError> {
        let (mut connection, identity) =
            Self::open_existing_database(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        let transaction = connection
            .transaction()
            .map_err(|error| database_error(path, error))?;
        validate_database(&transaction, path)?;
        let entries = load_entries(&transaction, path)?;
        let validated = PortableLibraryDocument {
            format_version: LIBRARY_FORMAT_VERSION,
            entries: entries.clone(),
        }
        .validate()
        .map_err(|_| AppError::database_corrupt(NativePath::new(path.to_path_buf())))?;
        if validated.entries != entries {
            return Err(AppError::database_corrupt(NativePath::new(
                path.to_path_buf(),
            )));
        }
        Self::revalidate_database_identity(path, identity)?;
        transaction
            .commit()
            .map_err(|error| database_error(path, error))?;
        Self::revalidate_database_identity(path, identity)?;
        Ok(entries)
    }

    fn plan(
        document: &PortableLibraryDocument,
        existing: &[PortableLibraryEntry],
        dry_run: bool,
    ) -> Result<ImportPlan, AppError> {
        let mut by_canonical = HashMap::with_capacity(existing.len());
        let mut aliases = HashMap::new();
        for entry in existing {
            by_canonical.insert(entry.skill.source.canonical.as_str(), entry);
            if let Some(alias) = &entry.alias {
                aliases.insert(alias.as_str(), &entry.skill.source);
            }
        }

        let mut additions = Vec::new();
        let mut kept = Vec::new();
        for entry in &document.entries {
            if by_canonical.contains_key(entry.skill.source.canonical.as_str()) {
                kept.push(entry.skill.source.clone());
                continue;
            }
            if let Some(alias) = &entry.alias
                && aliases.contains_key(alias.as_str())
            {
                return Err(AppError::conflict(vec![Conflict::internal_duplicate(
                    Some(alias.clone()),
                    entry.skill.source.clone(),
                )]));
            }
            additions.push(entry.clone());
        }
        additions.sort_by(|left, right| {
            left.skill
                .source
                .canonical
                .cmp(&right.skill.source.canonical)
        });
        kept.sort_by(|left, right| left.canonical.cmp(&right.canonical));
        let added = additions
            .iter()
            .map(|entry| entry.skill.source.clone())
            .collect::<Vec<_>>();
        Ok(ImportPlan {
            additions,
            result: LibraryImportResult {
                format_version: LIBRARY_FORMAT_VERSION,
                dry_run,
                added,
                updated: Vec::new(),
                kept,
                conflicts: Vec::new(),
            },
        })
    }

    fn import_existing(
        &self,
        roots: &ResolvedRoots,
        document: &PortableLibraryDocument,
    ) -> Result<LibraryImportOperation, AppError> {
        let lock = acquire_restrictive_lock(roots, "database.lock", "database")?;
        let result = (|| {
            let roots = self.root_resolver.revalidate(roots)?;
            let database = Self::database_path(&roots);
            if !Self::database_exists(&database)? {
                return Err(Self::database_identity_drift());
            }
            let (mut connection, identity) =
                Self::open_existing_database(&database, OpenFlags::SQLITE_OPEN_READ_WRITE)?;
            self.hooks.after_existing_database_open(&database)?;
            Self::revalidate_database_identity(&database, identity)?;
            validate_database(&connection, &database)?;
            let existing = load_entries(&connection, &database)?;
            let plan = Self::plan(document, &existing, false)?;
            if plan.additions.is_empty() {
                Self::revalidate_database_identity(&database, identity)?;
                return Ok(LibraryImportOperation {
                    outcome: MutationOutcome::Unchanged,
                    data: plan.result,
                });
            }
            Self::revalidate_database_identity(&database, identity)?;
            let transaction = connection
                .transaction()
                .map_err(|error| database_error(&database, error))?;
            apply_additions(&transaction, &plan.additions, &database)?;
            transaction
                .commit()
                .map_err(|error| database_error(&database, error))?;
            self.hooks.after_commit_before_sync()?;
            sync_existing_database(&database, identity)?;
            File::open(&roots.data.effective)
                .and_then(|directory| directory.sync_all())
                .map_err(|error| {
                    database_sync_error(&database, "sync database directory", error)
                })?;
            Ok(LibraryImportOperation {
                outcome: MutationOutcome::Changed,
                data: plan.result,
            })
        })();
        let unlock = lock.unlock();
        if let Err(error) = unlock {
            return Err(environment_io(
                "XDG_STATE_HOME",
                &roots.state.effective.join("locks/database.lock"),
                "unlock database.lock",
                error,
            ));
        }
        result
    }

    fn import_first(
        &self,
        roots: &ResolvedRoots,
        plan: ImportPlan,
    ) -> Result<LibraryImportOperation, AppError> {
        let lock_path = roots.state.effective.join("locks/database.lock");
        let mut cleanup = FirstImportCleanup::new(lock_path.clone());
        (|| {
            cleanup
                .created_directories
                .extend(ensure_restrictive_directory(
                    &roots.state.effective,
                    "XDG_STATE_HOME",
                )?);
            cleanup
                .created_directories
                .extend(ensure_restrictive_directory(
                    &roots.state.effective.join("locks"),
                    "XDG_STATE_HOME",
                )?);
            self.hooks.before_first_lock()?;
            let (lock, created_lock) =
                acquire_restrictive_lock_with_identity(roots, "database.lock", "database")?;
            if let Some(identity) = created_lock {
                cleanup.created_lock = Some(CreatedLock {
                    identity,
                    handle: lock.try_clone().map_err(|error| {
                        environment_io(
                            "XDG_STATE_HOME",
                            &lock_path,
                            "retain created database.lock",
                            error,
                        )
                    })?,
                });
            }

            let result = (|| {
                let roots = self.root_resolver.revalidate(roots)?;
                let database = Self::database_path(&roots);
                if Self::database_exists(&database)? {
                    return Err(AppError::invalid_state(
                        "library_database",
                        "database_identity_drift",
                        ["an absent database before first import"],
                    ));
                }
                cleanup
                    .created_directories
                    .extend(ensure_restrictive_directory(
                        &roots.data.effective,
                        "XDG_DATA_HOME",
                    )?);
                let roots = self.root_resolver.revalidate(&roots)?;
                let database = Self::database_path(&roots);
                if Self::database_exists(&database)? {
                    return Err(AppError::invalid_state(
                        "library_database",
                        "database_identity_drift",
                        ["an absent database before first import"],
                    ));
                }
                let data_directory = ValidatedDataDirectory::open(&roots.data.effective)?;
                let staging_file = Builder::new()
                    .prefix(".skilload-library-db-")
                    .suffix(".tmp")
                    .tempfile_in(&data_directory.path)
                    .map_err(|error| {
                        database_sync_error(&database, "create staging database", error)
                    })?;
                let mut staging = FirstImportStaging::new(staging_file, &data_directory)?;
                staging
                    .file
                    .as_file()
                    .set_permissions(fs::Permissions::from_mode(0o600))
                    .map_err(|error| {
                        database_sync_error(&database, "restrict staging database", error)
                    })?;
                let staging_path = staging.file.path().to_path_buf();
                let mut connection = Connection::open(&staging_path)
                    .map_err(|error| database_error(&staging_path, error))?;
                initialize_schema(&connection, &staging_path)?;
                let transaction = connection
                    .transaction()
                    .map_err(|error| database_error(&staging_path, error))?;
                apply_additions(&transaction, &plan.additions, &staging_path)?;
                self.hooks.before_commit()?;
                transaction
                    .commit()
                    .map_err(|error| database_error(&staging_path, error))?;
                cleanup.committed = true;
                self.hooks.after_commit_before_sync()?;
                drop(connection);
                staging.file.as_file().sync_all().map_err(|error| {
                    database_sync_error(&staging_path, "sync committed staging database", error)
                })?;
                self.hooks.before_publish()?;
                let roots = self.root_resolver.revalidate(&roots)?;
                let database = Self::database_path(&roots);
                data_directory.revalidate()?;
                if Self::database_exists(&database)? {
                    return Err(AppError::invalid_state(
                        "library_database",
                        "database_identity_drift",
                        ["an absent database before first import publish"],
                    ));
                }
                self.hooks
                    .after_first_publish_destination_check(&database)?;
                data_directory.revalidate()?;
                staging.verify_entry(&staging.name)?;
                self.hooks
                    .after_first_staging_identity_check_before_publish(&database)?;
                let database_name = database
                    .file_name()
                    .ok_or_else(Self::database_identity_drift)?
                    .to_os_string();
                renameat_with(
                    &data_directory.handle,
                    &staging.name,
                    &data_directory.handle,
                    &database_name,
                    RenameFlags::NOREPLACE,
                )
                .map_err(|error| {
                    let error: io::Error = error.into();
                    if error.kind() == io::ErrorKind::AlreadyExists {
                        AppError::invalid_state(
                            "library_database",
                            "database_identity_drift",
                            ["an absent database before first import publish"],
                        )
                    } else {
                        database_sync_error(&database, "publish committed staging database", error)
                    }
                })?;
                if let Err(error) = staging.verify_entry(&database_name) {
                    staging.file.disable_cleanup(true);
                    return Err(error);
                }
                staging.mark_published();
                data_directory.revalidate()?;
                data_directory.handle.sync_all().map_err(|error| {
                    database_sync_error(&database, "sync published database directory", error)
                })?;
                data_directory.revalidate()?;
                sync_created_directory_entries(&cleanup.created_directories, "XDG_DATA_HOME")?;
                Ok(LibraryImportOperation {
                    outcome: MutationOutcome::Changed,
                    data: plan.result,
                })
            })();
            let unlock = lock.unlock();
            if let Err(error) = unlock {
                return Err(environment_io(
                    "XDG_STATE_HOME",
                    &lock_path,
                    "unlock database.lock",
                    error,
                ));
            }
            result
        })()
    }
}

impl Default for SqliteLibraryRepository {
    fn default() -> Self {
        Self::new()
    }
}

impl LibraryRepository for SqliteLibraryRepository {
    fn export(&self) -> Result<PortableLibraryDocument, AppError> {
        let roots = self.resolve_roots()?;
        let database = Self::database_path(&roots);
        if !Self::database_exists(&database)? {
            return Ok(PortableLibraryDocument::empty());
        }
        let mut document = PortableLibraryDocument {
            format_version: LIBRARY_FORMAT_VERSION,
            entries: self.read_existing(&database)?,
        };
        document.sort_deterministically()?;
        Ok(document)
    }

    fn import(
        &self,
        document: &PortableLibraryDocument,
        dry_run: bool,
    ) -> Result<LibraryImportOperation, AppError> {
        let document = document.clone().validate()?;
        let roots = self.resolve_roots()?;
        let database = Self::database_path(&roots);
        if dry_run {
            let existing = if Self::database_exists(&database)? {
                self.read_existing(&database)?
            } else {
                Vec::new()
            };
            let plan = Self::plan(&document, &existing, true)?;
            return Ok(LibraryImportOperation {
                outcome: MutationOutcome::Unchanged,
                data: plan.result,
            });
        }
        if Self::database_exists(&database)? {
            self.import_existing(&roots, &document)
        } else if document.entries.is_empty() {
            Ok(LibraryImportOperation {
                outcome: MutationOutcome::Unchanged,
                data: Self::plan(&document, &[], false)?.result,
            })
        } else {
            let plan = Self::plan(&document, &[], false)?;
            self.import_first(&roots, plan)
        }
    }
}

trait PersistenceHooks: Send + Sync {
    fn before_first_lock(&self) -> Result<(), AppError> {
        Ok(())
    }

    fn before_commit(&self) -> Result<(), AppError> {
        Ok(())
    }

    fn after_commit_before_sync(&self) -> Result<(), AppError> {
        Ok(())
    }

    fn before_publish(&self) -> Result<(), AppError> {
        Ok(())
    }

    fn after_first_publish_destination_check(&self, _database: &Path) -> Result<(), AppError> {
        Ok(())
    }

    fn after_first_staging_identity_check_before_publish(
        &self,
        _database: &Path,
    ) -> Result<(), AppError> {
        Ok(())
    }

    fn after_existing_database_open(&self, _database: &Path) -> Result<(), AppError> {
        Ok(())
    }
}

struct NoopPersistenceHooks;

impl PersistenceHooks for NoopPersistenceHooks {}

struct ImportPlan {
    additions: Vec<PortableLibraryEntry>,
    result: LibraryImportResult,
}

struct CreatedLock {
    identity: (u64, u64),
    handle: File,
}

struct FirstImportCleanup {
    lock_path: PathBuf,
    created_lock: Option<CreatedLock>,
    created_directories: Vec<CreatedDirectory>,
    committed: bool,
}

impl FirstImportCleanup {
    fn new(lock_path: PathBuf) -> Self {
        Self {
            lock_path,
            created_lock: None,
            created_directories: Vec::new(),
            committed: false,
        }
    }
}

impl Drop for FirstImportCleanup {
    fn drop(&mut self) {
        if !self.committed {
            cleanup_first_import(
                &self.lock_path,
                self.created_lock.as_ref(),
                &self.created_directories,
            );
        }
    }
}

struct ValidatedDataDirectory {
    path: PathBuf,
    identity: (u64, u64),
    handle: File,
}

impl ValidatedDataDirectory {
    fn open(path: &Path) -> Result<Self, AppError> {
        let metadata = fs::symlink_metadata(path).map_err(|error| {
            environment_io("XDG_DATA_HOME", path, "inspect data directory", error)
        })?;
        if metadata.file_type().is_symlink() || !metadata.file_type().is_dir() {
            return Err(SqliteLibraryRepository::database_identity_drift());
        }
        let mut options = OpenOptions::new();
        options
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW);
        let handle = options
            .open(path)
            .map_err(|error| environment_io("XDG_DATA_HOME", path, "open data directory", error))?;
        let handle_metadata = handle.metadata().map_err(|error| {
            environment_io(
                "XDG_DATA_HOME",
                path,
                "inspect opened data directory",
                error,
            )
        })?;
        if !handle_metadata.file_type().is_dir()
            || metadata_identity(&metadata) != metadata_identity(&handle_metadata)
        {
            return Err(SqliteLibraryRepository::database_identity_drift());
        }
        Ok(Self {
            path: path.to_path_buf(),
            identity: metadata_identity(&metadata),
            handle,
        })
    }

    fn revalidate(&self) -> Result<(), AppError> {
        let path_metadata = fs::symlink_metadata(&self.path)
            .map_err(|_| SqliteLibraryRepository::database_identity_drift())?;
        let handle_metadata = self
            .handle
            .metadata()
            .map_err(|_| SqliteLibraryRepository::database_identity_drift())?;
        if path_metadata.file_type().is_symlink()
            || !path_metadata.file_type().is_dir()
            || !handle_metadata.file_type().is_dir()
            || metadata_identity(&path_metadata) != self.identity
            || metadata_identity(&handle_metadata) != self.identity
        {
            return Err(SqliteLibraryRepository::database_identity_drift());
        }
        Ok(())
    }
}

struct FirstImportStaging<'directory> {
    file: NamedTempFile,
    directory: &'directory ValidatedDataDirectory,
    name: OsString,
    published: bool,
}

impl<'directory> FirstImportStaging<'directory> {
    fn new(
        file: NamedTempFile,
        directory: &'directory ValidatedDataDirectory,
    ) -> Result<Self, AppError> {
        let name = file
            .path()
            .file_name()
            .ok_or_else(SqliteLibraryRepository::database_identity_drift)?
            .to_os_string();
        Ok(Self {
            file,
            directory,
            name,
            published: false,
        })
    }

    fn mark_published(&mut self) {
        self.file.disable_cleanup(true);
        self.published = true;
    }

    fn verify_entry(&self, name: &std::ffi::OsStr) -> Result<(), AppError> {
        let held = fstat(self.file.as_file())
            .map_err(|_| SqliteLibraryRepository::database_identity_drift())?;
        let entry = statat(&self.directory.handle, name, AtFlags::SYMLINK_NOFOLLOW)
            .map_err(|_| SqliteLibraryRepository::database_identity_drift())?;
        if held.st_dev != entry.st_dev || held.st_ino != entry.st_ino {
            return Err(SqliteLibraryRepository::database_identity_drift());
        }
        Ok(())
    }
}

impl Drop for FirstImportStaging<'_> {
    fn drop(&mut self) {
        if self.published {
            return;
        }
        let owned = fstat(self.file.as_file())
            .ok()
            .zip(
                statat(
                    &self.directory.handle,
                    &self.name,
                    AtFlags::SYMLINK_NOFOLLOW,
                )
                .ok(),
            )
            .is_some_and(|(held, entry)| {
                held.st_dev == entry.st_dev && held.st_ino == entry.st_ino
            });
        if owned {
            let _ = unlinkat(&self.directory.handle, &self.name, AtFlags::empty());
        }
        self.file.disable_cleanup(true);
    }
}

fn initialize_schema(connection: &Connection, path: &Path) -> Result<(), AppError> {
    connection
        .execute_batch(
            "
            PRAGMA foreign_keys = ON;
            PRAGMA journal_mode = DELETE;
            CREATE TABLE schema_info (version INTEGER NOT NULL CHECK (version >= 1));
            INSERT INTO schema_info (version) VALUES (1);
            CREATE TABLE state_revision (revision INTEGER NOT NULL CHECK (revision >= 0));
            INSERT INTO state_revision (revision) VALUES (0);
            CREATE TABLE library_entries (
                canonical_source TEXT PRIMARY KEY NOT NULL,
                owner TEXT NOT NULL,
                repository TEXT NOT NULL,
                repository_display TEXT NOT NULL,
                skill_path TEXT NOT NULL,
                ref_kind TEXT NOT NULL,
                ref_value TEXT NOT NULL,
                repository_id TEXT NOT NULL,
                commit_sha TEXT NOT NULL,
                integrity TEXT NOT NULL,
                name TEXT NOT NULL,
                description TEXT NOT NULL,
                entry_count TEXT NOT NULL,
                byte_count TEXT NOT NULL,
                alias TEXT UNIQUE,
                category TEXT,
                note TEXT
            );
            CREATE TABLE library_tags (
                canonical_source TEXT NOT NULL,
                comparison_key TEXT NOT NULL,
                display TEXT NOT NULL,
                PRIMARY KEY (canonical_source, comparison_key),
                FOREIGN KEY (canonical_source) REFERENCES library_entries(canonical_source) ON DELETE CASCADE
            );
            ",
        )
        .map_err(|error| database_error(path, error))?;
    Ok(())
}

fn validate_database(connection: &Connection, path: &Path) -> Result<(), AppError> {
    connection
        .execute_batch("PRAGMA foreign_keys = ON;")
        .map_err(|error| database_error(path, error))?;
    let raw_version = singleton_i64(connection, "SELECT version FROM schema_info", path)?;
    if raw_version < 0 {
        return Err(AppError::database_corrupt(NativePath::new(
            path.to_path_buf(),
        )));
    }
    if raw_version > API_V1_UINT_MAX {
        return Err(AppError::database_corrupt(NativePath::new(
            path.to_path_buf(),
        )));
    }
    let version = raw_version as u64;
    if version > SCHEMA_VERSION {
        return Err(AppError::SchemaNewer {
            domain: "library".to_owned(),
            found_version: version,
            supported_version: SCHEMA_VERSION,
        });
    }
    if version < SCHEMA_VERSION {
        return Err(AppError::MigrationRequired {
            domain: "library".to_owned(),
            found_version: version,
            supported_version: SCHEMA_VERSION,
        });
    }
    let state_revision = singleton_i64(connection, "SELECT revision FROM state_revision", path)?;
    if state_revision < 0 {
        return Err(AppError::database_corrupt(NativePath::new(
            path.to_path_buf(),
        )));
    }
    validate_library_tags_schema(connection, path)?;
    let integrity: String = connection
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .map_err(|error| database_error(path, error))?;
    if integrity != "ok" {
        return Err(AppError::database_corrupt(NativePath::new(
            path.to_path_buf(),
        )));
    }
    let foreign_key_issue: Option<String> = connection
        .query_row("PRAGMA foreign_key_check", [], |row| row.get(0))
        .optional()
        .map_err(|error| database_error(path, error))?;
    if foreign_key_issue.is_some() {
        return Err(AppError::database_corrupt(NativePath::new(
            path.to_path_buf(),
        )));
    }
    Ok(())
}

fn singleton_i64(connection: &Connection, query: &str, path: &Path) -> Result<i64, AppError> {
    let mut statement = connection
        .prepare(query)
        .map_err(|_| AppError::database_corrupt(NativePath::new(path.to_path_buf())))?;
    let mut rows = statement
        .query([])
        .map_err(|_| AppError::database_corrupt(NativePath::new(path.to_path_buf())))?;
    let row = rows
        .next()
        .map_err(|_| AppError::database_corrupt(NativePath::new(path.to_path_buf())))?
        .ok_or_else(|| AppError::database_corrupt(NativePath::new(path.to_path_buf())))?;
    let value = row
        .get(0)
        .map_err(|_| AppError::database_corrupt(NativePath::new(path.to_path_buf())))?;
    if rows
        .next()
        .map_err(|_| AppError::database_corrupt(NativePath::new(path.to_path_buf())))?
        .is_some()
    {
        return Err(AppError::database_corrupt(NativePath::new(
            path.to_path_buf(),
        )));
    }
    Ok(value)
}

fn validate_library_tags_schema(connection: &Connection, path: &Path) -> Result<(), AppError> {
    connection
        .prepare("SELECT canonical_source, comparison_key, display FROM library_tags LIMIT 0")
        .map(|_| ())
        .map_err(|error| database_error(path, error))
}

fn load_entries(
    connection: &Connection,
    path: &Path,
) -> Result<Vec<PortableLibraryEntry>, AppError> {
    let mut statement = connection
        .prepare(
            "SELECT canonical_source, owner, repository, repository_display, skill_path, ref_kind, ref_value,
                    repository_id, commit_sha, integrity, name, description, entry_count, byte_count, alias, category, note
             FROM library_entries ORDER BY canonical_source",
        )
        .map_err(|error| database_error(path, error))?;
    let entries = statement
        .query_map([], |row| {
            Ok(StoredEntry {
                canonical: row.get(0)?,
                owner: row.get(1)?,
                repository: row.get(2)?,
                repository_display: row.get(3)?,
                skill_path: row.get(4)?,
                ref_kind: row.get(5)?,
                ref_value: row.get(6)?,
                repository_id: row.get(7)?,
                commit: row.get(8)?,
                integrity: row.get(9)?,
                name: row.get(10)?,
                description: row.get(11)?,
                entry_count: row.get(12)?,
                byte_count: row.get(13)?,
                alias: row.get(14)?,
                category: row.get(15)?,
                note: row.get(16)?,
            })
        })
        .map_err(|error| database_error(path, error))?;
    let mut loaded = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| database_error(path, error))?;
        let source = SourceIdentity::new(
            entry.canonical,
            entry.owner,
            entry.repository,
            entry.repository_display,
            entry.skill_path,
            parse_ref_kind(&entry.ref_kind, path)?,
            entry.ref_value,
        )
        .map_err(|_| AppError::database_corrupt(NativePath::new(path.to_path_buf())))?;
        let skill = ResolvedSkill::new(
            source.clone(),
            parse_decimal_u64(&entry.repository_id, "repository_id")
                .map_err(|_| AppError::database_corrupt(NativePath::new(path.to_path_buf())))?,
            entry.commit,
            entry.integrity,
            entry.name,
            entry.description,
            parse_decimal_u64(&entry.entry_count, "entry_count")
                .map_err(|_| AppError::database_corrupt(NativePath::new(path.to_path_buf())))?,
            parse_decimal_u64(&entry.byte_count, "byte_count")
                .map_err(|_| AppError::database_corrupt(NativePath::new(path.to_path_buf())))?,
        )
        .map_err(|_| AppError::database_corrupt(NativePath::new(path.to_path_buf())))?;
        let tags = load_tags(connection, path, &source.canonical)?;
        loaded.push(PortableLibraryEntry {
            skill,
            alias: entry.alias,
            category: entry.category,
            tags,
            note: entry.note,
        });
    }
    Ok(loaded)
}

fn load_tags(
    connection: &Connection,
    path: &Path,
    canonical: &str,
) -> Result<Vec<String>, AppError> {
    let mut statement = connection
        .prepare(
            "SELECT comparison_key, display FROM library_tags
             WHERE canonical_source = ?1 ORDER BY comparison_key",
        )
        .map_err(|error| database_error(path, error))?;
    let values = statement
        .query_map(params![canonical], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|error| database_error(path, error))?;
    let mut tags = Vec::new();
    for value in values {
        let (comparison_key, display) =
            value.map_err(|_| AppError::database_corrupt(NativePath::new(path.to_path_buf())))?;
        let normalized = normalize_tag(&display)
            .map_err(|_| AppError::database_corrupt(NativePath::new(path.to_path_buf())))?;
        if normalized.display != display || normalized.comparison_key != comparison_key {
            return Err(AppError::database_corrupt(NativePath::new(
                path.to_path_buf(),
            )));
        }
        tags.push(display);
    }
    Ok(tags)
}

fn apply_additions(
    connection: &Connection,
    additions: &[PortableLibraryEntry],
    path: &Path,
) -> Result<(), AppError> {
    let changed_revision = connection
        .execute(
            "UPDATE state_revision
             SET revision = revision + 1
             WHERE typeof(revision) = 'integer' AND revision >= 0 AND revision < ?1",
            params![i64::MAX],
        )
        .map_err(|error| database_error(path, error))?;
    if changed_revision != 1 {
        return Err(AppError::invalid_state(
            "library_database",
            "state_revision_not_incrementable",
            ["a nonnegative state revision below i64::MAX"],
        ));
    }
    for entry in additions {
        let source = &entry.skill.source;
        connection
            .execute(
                "INSERT INTO library_entries (
                    canonical_source, owner, repository, repository_display, skill_path, ref_kind, ref_value,
                    repository_id, commit_sha, integrity, name, description, entry_count, byte_count, alias, category, note
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
                params![
                    source.canonical,
                    source.owner,
                    source.repository,
                    source.repository_display,
                    source.path,
                    ref_kind_name(source.ref_kind),
                    source.ref_value,
                    entry.skill.repository_id.to_string(),
                    entry.skill.commit,
                    entry.skill.integrity,
                    entry.skill.name,
                    entry.skill.description,
                    entry.skill.entry_count.to_string(),
                    entry.skill.byte_count.to_string(),
                    entry.alias,
                    entry.category,
                    entry.note,
                ],
            )
            .map_err(|error| database_error(path, error))?;
        for tag in &entry.tags {
            let tag = normalize_tag(tag)?;
            connection
                .execute(
                    "INSERT INTO library_tags (canonical_source, comparison_key, display) VALUES (?1, ?2, ?3)",
                    params![source.canonical, tag.comparison_key, tag.display],
                )
                .map_err(|error| database_error(path, error))?;
        }
    }
    Ok(())
}

fn parse_ref_kind(value: &str, path: &Path) -> Result<RefKind, AppError> {
    match value {
        "branch" => Ok(RefKind::Branch),
        "tag" => Ok(RefKind::Tag),
        "commit" => Ok(RefKind::Commit),
        _ => Err(AppError::database_corrupt(NativePath::new(
            path.to_path_buf(),
        ))),
    }
}

fn ref_kind_name(value: RefKind) -> &'static str {
    match value {
        RefKind::Branch => "branch",
        RefKind::Tag => "tag",
        RefKind::Commit => "commit",
    }
}

fn database_error(path: &Path, error: SqlError) -> AppError {
    match &error {
        SqlError::SqliteFailure(code, _)
            if matches!(
                code.code,
                ErrorCode::DatabaseCorrupt | ErrorCode::NotADatabase
            ) =>
        {
            AppError::database_corrupt(NativePath::new(path.to_path_buf()))
        }
        SqlError::SqliteFailure(_, Some(message))
            if message.contains("no such table") || message.contains("no such column") =>
        {
            AppError::database_corrupt(NativePath::new(path.to_path_buf()))
        }
        SqlError::SqlInputError { msg, .. }
            if msg.contains("no such table") || msg.contains("no such column") =>
        {
            AppError::database_corrupt(NativePath::new(path.to_path_buf()))
        }
        SqlError::FromSqlConversionFailure(..)
        | SqlError::IntegralValueOutOfRange(..)
        | SqlError::InvalidColumnType(..)
        | SqlError::Utf8Error(..) => {
            AppError::database_corrupt(NativePath::new(path.to_path_buf()))
        }
        _ => AppError::invalid_state(
            "library_database",
            format!("sqlite_error: {error}"),
            ["a readable Library schema version 1 database"],
        ),
    }
}

fn database_sync_error(path: &Path, action: &str, error: io::Error) -> AppError {
    AppError::invalid_state(
        "library_database",
        format!("{action}: {error}"),
        [NativePath::new(path.to_path_buf())
            .as_path()
            .display()
            .to_string()],
    )
}

fn sync_existing_database(path: &Path, identity: (u64, u64)) -> Result<(), AppError> {
    SqliteLibraryRepository::revalidate_database_identity(path, identity)?;
    let mut options = OpenOptions::new();
    options
        .read(true)
        .write(true)
        .custom_flags(libc::O_NOFOLLOW);
    let file = options
        .open(path)
        .map_err(|_| SqliteLibraryRepository::database_identity_drift())?;
    let opened_identity = metadata_identity(
        &file
            .metadata()
            .map_err(|error| database_sync_error(path, "inspect committed database", error))?,
    );
    if opened_identity != identity {
        return Err(SqliteLibraryRepository::database_identity_drift());
    }
    SqliteLibraryRepository::revalidate_database_identity(path, identity)?;
    file.sync_all()
        .map_err(|error| database_sync_error(path, "sync committed database", error))
}

fn metadata_identity(metadata: &fs::Metadata) -> (u64, u64) {
    (metadata.dev(), metadata.ino())
}

fn cleanup_first_import(
    lock_path: &Path,
    created_lock: Option<&CreatedLock>,
    created_directories: &[CreatedDirectory],
) {
    if let Some(created_lock) = created_lock
        && current_entry_matches_created_identity(
            lock_path,
            created_lock.identity,
            &created_lock.handle,
        )
        .is_some_and(|metadata| {
            metadata.file_type().is_file() && !metadata.file_type().is_symlink()
        })
    {
        let _ = fs::remove_file(lock_path);
    }
    for directory in created_directories.iter().rev() {
        if current_entry_matches_created_identity(
            &directory.path,
            directory.identity,
            &directory.handle,
        )
        .is_some_and(|metadata| metadata.file_type().is_dir() && !metadata.file_type().is_symlink())
        {
            let _ = fs::remove_dir(&directory.path);
        }
    }
}

fn current_entry_matches_created_identity(
    path: &Path,
    identity: (u64, u64),
    handle: &File,
) -> Option<fs::Metadata> {
    let metadata = fs::symlink_metadata(path).ok()?;
    let handle_metadata = handle.metadata().ok()?;
    (metadata_identity(&metadata) == identity && metadata_identity(&handle_metadata) == identity)
        .then_some(metadata)
}

struct StoredEntry {
    canonical: String,
    owner: String,
    repository: String,
    repository_display: String,
    skill_path: String,
    ref_kind: String,
    ref_value: String,
    repository_id: String,
    commit: String,
    integrity: String,
    name: String,
    description: String,
    entry_count: String,
    byte_count: String,
    alias: Option<String>,
    category: Option<String>,
    note: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::source::RefKind;
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

    fn entry(path: &str, alias: Option<&str>) -> PortableLibraryEntry {
        let source = SourceIdentity::new(
            format!("github:owner/repository#{path}@refs/heads/main"),
            "owner".to_owned(),
            "repository".to_owned(),
            "Repository".to_owned(),
            path.to_owned(),
            RefKind::Branch,
            "refs/heads/main".to_owned(),
        )
        .unwrap();
        PortableLibraryEntry {
            skill: ResolvedSkill::new(
                source,
                42,
                "0123456789012345678901234567890123456789".to_owned(),
                "sha256:0123456789012345678901234567890123456789012345678901234567890123"
                    .to_owned(),
                path.rsplit('/').next().unwrap().to_owned(),
                "Description".to_owned(),
                1,
                10,
            )
            .unwrap(),
            alias: alias.map(ToOwned::to_owned),
            category: None,
            tags: vec!["Review".to_owned()],
            note: None,
        }
    }

    fn document(entries: Vec<PortableLibraryEntry>) -> PortableLibraryDocument {
        PortableLibraryDocument {
            format_version: 1,
            entries,
        }
    }

    #[test]
    fn dry_run_is_inert_and_first_import_round_trips() {
        let temporary = tempdir().unwrap();
        let environment = Arc::new(TestEnvironment::with_roots(temporary.path()));
        let repository =
            SqliteLibraryRepository::with_environment(environment, Arc::new(XdgRootResolver));
        let import = document(vec![entry("skills/review", Some("review"))]);
        let dry_run = repository.import(&import, true).unwrap();
        assert_eq!(dry_run.data.added.len(), 1);
        assert!(!temporary.path().join("data/skilload").exists());
        assert!(!temporary.path().join("state/skilload").exists());

        let committed = repository.import(&import, false).unwrap();
        assert_eq!(committed.outcome, MutationOutcome::Changed);
        let database = temporary.path().join("data/skilload/skilload.db");
        let connection = Connection::open(&database).unwrap();
        let mut options = connection.prepare("PRAGMA compile_options").unwrap();
        let options = options
            .query_map([], |row| row.get::<_, String>(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert!(options.iter().any(|option| option == "ENABLE_FTS5"));
        let exported = repository.export().unwrap();
        assert_eq!(exported.entries, import.clone().validate().unwrap().entries);

        let repeated = repository.import(&import, false).unwrap();
        assert_eq!(repeated.outcome, MutationOutcome::Unchanged);
    }

    #[test]
    fn alias_conflict_rolls_back_new_batch() {
        let temporary = tempdir().unwrap();
        let environment = Arc::new(TestEnvironment::with_roots(temporary.path()));
        let repository =
            SqliteLibraryRepository::with_environment(environment, Arc::new(XdgRootResolver));
        repository
            .import(&document(vec![entry("skills/one", Some("same"))]), false)
            .unwrap();
        let error = repository
            .import(
                &document(vec![
                    entry("skills/new", None),
                    entry("skills/two", Some("same")),
                ]),
                false,
            )
            .unwrap_err();
        assert_eq!(error.code(), "conflict");
        assert_eq!(repository.export().unwrap().entries.len(), 1);
    }

    #[test]
    fn first_import_conflict_precedes_any_state_creation() {
        let temporary = tempdir().unwrap();
        let repository = SqliteLibraryRepository::with_environment(
            Arc::new(TestEnvironment::with_roots(temporary.path())),
            Arc::new(XdgRootResolver),
        );
        let duplicate = entry("skills/review", None);
        let error = repository
            .import(&document(vec![duplicate.clone(), duplicate]), false)
            .unwrap_err();
        assert_eq!(error.code(), "conflict");
        assert!(!temporary.path().join("data/skilload").exists());
        assert!(!temporary.path().join("state/skilload").exists());
    }

    #[test]
    fn corrupt_database_is_not_replaced() {
        let temporary = tempdir().unwrap();
        let data = temporary.path().join("data/skilload");
        fs::create_dir_all(&data).unwrap();
        let database = data.join("skilload.db");
        fs::write(&database, b"not sqlite").unwrap();
        let environment = Arc::new(TestEnvironment::with_roots(temporary.path()));
        let repository =
            SqliteLibraryRepository::with_environment(environment, Arc::new(XdgRootResolver));
        let error = repository.export().unwrap_err();
        assert_eq!(error.code(), "database_corrupt");
        assert_eq!(fs::read(&database).unwrap(), b"not sqlite");
    }

    struct BeforeCommitFailure;

    impl PersistenceHooks for BeforeCommitFailure {
        fn before_commit(&self) -> Result<(), AppError> {
            Err(AppError::Internal {
                incident_id: "before-first-library-commit".to_owned(),
            })
        }
    }

    struct AfterCommitFailure;

    impl PersistenceHooks for AfterCommitFailure {
        fn after_commit_before_sync(&self) -> Result<(), AppError> {
            Err(AppError::Internal {
                incident_id: "after-first-library-commit".to_owned(),
            })
        }
    }

    #[test]
    fn first_import_precommit_failure_removes_only_created_state() {
        let temporary = tempdir().unwrap();
        let repository = SqliteLibraryRepository::with_hooks(
            Arc::new(TestEnvironment::with_roots(temporary.path())),
            Arc::new(XdgRootResolver),
            Arc::new(BeforeCommitFailure),
        );
        let error = repository
            .import(&document(vec![entry("skills/review", None)]), false)
            .unwrap_err();
        assert_eq!(error.code(), "internal_invariant");
        assert!(!temporary.path().join("data/skilload").exists());
        assert!(!temporary.path().join("state/skilload").exists());
    }

    #[test]
    fn first_import_postcommit_failure_is_not_reported_as_success() {
        let temporary = tempdir().unwrap();
        let repository = SqliteLibraryRepository::with_hooks(
            Arc::new(TestEnvironment::with_roots(temporary.path())),
            Arc::new(XdgRootResolver),
            Arc::new(AfterCommitFailure),
        );
        let error = repository
            .import(&document(vec![entry("skills/review", None)]), false)
            .unwrap_err();
        assert_eq!(error.code(), "internal_invariant");
        assert!(temporary.path().join("data/skilload").is_dir());
        assert!(temporary.path().join("state/skilload").is_dir());
    }
    #[test]
    fn malformed_sqlite_column_type_is_database_corrupt() {
        let temporary = tempdir().unwrap();
        let repository = SqliteLibraryRepository::with_environment(
            Arc::new(TestEnvironment::with_roots(temporary.path())),
            Arc::new(XdgRootResolver),
        );
        repository
            .import(&document(vec![entry("skills/review", None)]), false)
            .unwrap();
        let database = temporary.path().join("data/skilload/skilload.db");
        {
            let connection = Connection::open(&database).unwrap();
            connection
                .execute("UPDATE library_entries SET repository_id = x'00'", [])
                .unwrap();
        }
        assert_eq!(repository.export().unwrap_err().code(), "database_corrupt");
    }

    #[test]
    fn malformed_tag_comparison_key_is_database_corrupt() {
        let temporary = tempdir().unwrap();
        let repository = SqliteLibraryRepository::with_environment(
            Arc::new(TestEnvironment::with_roots(temporary.path())),
            Arc::new(XdgRootResolver),
        );
        repository
            .import(&document(vec![entry("skills/review", None)]), false)
            .unwrap();
        let database = temporary.path().join("data/skilload/skilload.db");
        Connection::open(&database)
            .unwrap()
            .execute(
                "UPDATE library_tags SET comparison_key = 'damaged-comparison-key'",
                [],
            )
            .unwrap();

        assert_eq!(repository.export().unwrap_err().code(), "database_corrupt");
    }

    #[test]
    fn empty_library_with_missing_tags_schema_is_database_corrupt() {
        let temporary = tempdir().unwrap();
        let data = temporary.path().join("data/skilload");
        let database = data.join("skilload.db");
        fs::create_dir_all(&data).unwrap();
        let connection = Connection::open(&database).unwrap();
        initialize_schema(&connection, &database).unwrap();
        connection.execute_batch("DROP TABLE library_tags").unwrap();
        drop(connection);
        let repository = SqliteLibraryRepository::with_environment(
            Arc::new(TestEnvironment::with_roots(temporary.path())),
            Arc::new(XdgRootResolver),
        );

        assert_eq!(repository.export().unwrap_err().code(), "database_corrupt");
    }

    #[test]
    fn multiple_state_revision_rows_are_database_corrupt() {
        let temporary = tempdir().unwrap();
        let repository = SqliteLibraryRepository::with_environment(
            Arc::new(TestEnvironment::with_roots(temporary.path())),
            Arc::new(XdgRootResolver),
        );
        repository
            .import(&document(vec![entry("skills/review", None)]), false)
            .unwrap();
        let database = temporary.path().join("data/skilload/skilload.db");
        Connection::open(&database)
            .unwrap()
            .execute("INSERT INTO state_revision (revision) VALUES (0)", [])
            .unwrap();

        assert_eq!(repository.export().unwrap_err().code(), "database_corrupt");
    }

    struct BeforeFirstLockFailure;

    impl PersistenceHooks for BeforeFirstLockFailure {
        fn before_first_lock(&self) -> Result<(), AppError> {
            Err(AppError::Internal {
                incident_id: "before-first-library-lock".to_owned(),
            })
        }
    }

    #[test]
    fn first_import_lock_failure_removes_created_state() {
        let temporary = tempdir().unwrap();
        let repository = SqliteLibraryRepository::with_hooks(
            Arc::new(TestEnvironment::with_roots(temporary.path())),
            Arc::new(XdgRootResolver),
            Arc::new(BeforeFirstLockFailure),
        );
        let error = repository
            .import(&document(vec![entry("skills/review", None)]), false)
            .unwrap_err();
        assert_eq!(error.code(), "internal_invariant");
        assert!(!temporary.path().join("data/skilload").exists());
        assert!(!temporary.path().join("state/skilload").exists());
    }

    struct PublishRace;

    impl PersistenceHooks for PublishRace {
        fn after_first_publish_destination_check(&self, database: &Path) -> Result<(), AppError> {
            fs::write(database, b"raced authoritative database").unwrap();
            Ok(())
        }
    }

    #[test]
    fn first_import_does_not_replace_a_database_created_during_publish() {
        let temporary = tempdir().unwrap();
        let repository = SqliteLibraryRepository::with_hooks(
            Arc::new(TestEnvironment::with_roots(temporary.path())),
            Arc::new(XdgRootResolver),
            Arc::new(PublishRace),
        );
        let error = repository
            .import(&document(vec![entry("skills/review", None)]), false)
            .unwrap_err();
        assert!(matches!(
            error,
            AppError::InvalidState { state, .. } if state == "database_identity_drift"
        ));
        assert_eq!(
            fs::read(temporary.path().join("data/skilload/skilload.db")).unwrap(),
            b"raced authoritative database"
        );
    }

    struct DataDirectoryReplacement {
        data_directory: PathBuf,
        displaced_directory: PathBuf,
    }

    impl PersistenceHooks for DataDirectoryReplacement {
        fn after_first_publish_destination_check(&self, _database: &Path) -> Result<(), AppError> {
            fs::rename(&self.data_directory, &self.displaced_directory).unwrap();
            fs::create_dir(&self.data_directory).unwrap();
            Ok(())
        }
    }

    #[test]
    fn first_import_rejects_a_replaced_data_directory_before_publish() {
        let temporary = tempdir().unwrap();
        let data_directory = temporary.path().join("data/skilload");
        let displaced_directory = temporary.path().join("displaced-data-directory");
        let repository = SqliteLibraryRepository::with_hooks(
            Arc::new(TestEnvironment::with_roots(temporary.path())),
            Arc::new(XdgRootResolver),
            Arc::new(DataDirectoryReplacement {
                data_directory: data_directory.clone(),
                displaced_directory: displaced_directory.clone(),
            }),
        );

        let error = repository
            .import(&document(vec![entry("skills/review", None)]), false)
            .unwrap_err();

        assert!(matches!(
            error,
            AppError::InvalidState { state, .. } if state == "database_identity_drift"
        ));
        assert!(!data_directory.join("skilload.db").exists());
        assert!(!displaced_directory.join("skilload.db").exists());
    }

    struct FirstImportStagingReplacement {
        replacement: PathBuf,
    }

    impl PersistenceHooks for FirstImportStagingReplacement {
        fn after_first_staging_identity_check_before_publish(
            &self,
            database: &Path,
        ) -> Result<(), AppError> {
            let staging = fs::read_dir(database.parent().unwrap())
                .unwrap()
                .map(|entry| entry.unwrap().path())
                .find(|path| {
                    path.file_name().is_some_and(|name| {
                        name.to_string_lossy().starts_with(".skilload-library-db-")
                    })
                })
                .unwrap();
            fs::remove_file(&staging).unwrap();
            fs::hard_link(&self.replacement, &staging).unwrap();
            Ok(())
        }
    }

    #[test]
    fn first_import_reports_staging_identity_drift_after_publish_race() {
        let temporary = tempdir().unwrap();
        let replacement = temporary.path().join("replacement.db");
        let database = temporary.path().join("data/skilload/skilload.db");
        fs::write(&replacement, b"replacement database").unwrap();
        let repository = SqliteLibraryRepository::with_hooks(
            Arc::new(TestEnvironment::with_roots(temporary.path())),
            Arc::new(XdgRootResolver),
            Arc::new(FirstImportStagingReplacement {
                replacement: replacement.clone(),
            }),
        );

        let error = repository
            .import(&document(vec![entry("skills/review", None)]), false)
            .unwrap_err();

        assert!(matches!(
            error,
            AppError::InvalidState { state, .. } if state == "database_identity_drift"
        ));
        assert_eq!(fs::read(database).unwrap(), b"replacement database");
        assert_eq!(fs::read(replacement).unwrap(), b"replacement database");
    }
    struct ExistingDatabaseReplacement {
        database: PathBuf,
        displaced: PathBuf,
        replacement: PathBuf,
    }

    impl PersistenceHooks for ExistingDatabaseReplacement {
        fn after_existing_database_open(&self, _database: &Path) -> Result<(), AppError> {
            fs::rename(&self.database, &self.displaced).unwrap();
            symlink(&self.replacement, &self.database).unwrap();
            Ok(())
        }
    }

    #[test]
    fn existing_import_rejects_a_database_replaced_after_open() {
        let temporary = tempdir().unwrap();
        let environment = Arc::new(TestEnvironment::with_roots(temporary.path()));
        let initial = SqliteLibraryRepository::with_environment(
            environment.clone(),
            Arc::new(XdgRootResolver),
        );
        initial
            .import(&document(vec![entry("skills/review", None)]), false)
            .unwrap();
        let database = temporary.path().join("data/skilload/skilload.db");
        let replacement = temporary.path().join("foreign.db");
        fs::write(&replacement, b"foreign database").unwrap();
        let repository = SqliteLibraryRepository::with_hooks(
            environment,
            Arc::new(XdgRootResolver),
            Arc::new(ExistingDatabaseReplacement {
                database: database.clone(),
                displaced: temporary.path().join("displaced.db"),
                replacement: replacement.clone(),
            }),
        );

        let error = repository
            .import(&document(vec![entry("skills/new", None)]), false)
            .unwrap_err();
        assert!(matches!(
            error,
            AppError::InvalidState { state, .. } if state == "database_identity_drift"
        ));
        assert_eq!(fs::read(replacement).unwrap(), b"foreign database");
    }

    #[test]
    fn missing_schema_column_is_database_corrupt() {
        let temporary = tempdir().unwrap();
        let repository = SqliteLibraryRepository::with_environment(
            Arc::new(TestEnvironment::with_roots(temporary.path())),
            Arc::new(XdgRootResolver),
        );
        repository
            .import(&document(vec![entry("skills/review", None)]), false)
            .unwrap();
        let database = temporary.path().join("data/skilload/skilload.db");
        Connection::open(&database)
            .unwrap()
            .execute_batch("ALTER TABLE library_entries RENAME COLUMN name TO renamed_name")
            .unwrap();

        let error = repository.export().unwrap_err();
        assert_eq!(error.code(), "database_corrupt", "{error:?}");
    }

    #[test]
    fn missing_schema_version_row_is_database_corrupt() {
        let temporary = tempdir().unwrap();
        let repository = SqliteLibraryRepository::with_environment(
            Arc::new(TestEnvironment::with_roots(temporary.path())),
            Arc::new(XdgRootResolver),
        );
        repository
            .import(&document(vec![entry("skills/review", None)]), false)
            .unwrap();
        let database = temporary.path().join("data/skilload/skilload.db");
        Connection::open(&database)
            .unwrap()
            .execute("DELETE FROM schema_info", [])
            .unwrap();

        assert_eq!(repository.export().unwrap_err().code(), "database_corrupt");
    }

    #[test]
    fn multiple_schema_version_rows_are_database_corrupt() {
        let temporary = tempdir().unwrap();
        let repository = SqliteLibraryRepository::with_environment(
            Arc::new(TestEnvironment::with_roots(temporary.path())),
            Arc::new(XdgRootResolver),
        );
        repository
            .import(&document(vec![entry("skills/review", None)]), false)
            .unwrap();
        let database = temporary.path().join("data/skilload/skilload.db");
        Connection::open(&database)
            .unwrap()
            .execute("INSERT INTO schema_info (version) VALUES (1)", [])
            .unwrap();

        assert_eq!(repository.export().unwrap_err().code(), "database_corrupt");
    }

    #[test]
    fn schema_version_above_api_uint_range_is_database_corrupt() {
        let temporary = tempdir().unwrap();
        let repository = SqliteLibraryRepository::with_environment(
            Arc::new(TestEnvironment::with_roots(temporary.path())),
            Arc::new(XdgRootResolver),
        );
        repository
            .import(&document(vec![entry("skills/review", None)]), false)
            .unwrap();
        let database = temporary.path().join("data/skilload/skilload.db");
        Connection::open(&database)
            .unwrap()
            .execute(
                "UPDATE schema_info SET version = ?1",
                params![API_V1_UINT_MAX + 1],
            )
            .unwrap();

        assert_eq!(repository.export().unwrap_err().code(), "database_corrupt");
    }

    #[test]
    fn nonincrementable_state_revision_rejects_import_without_mutation() {
        let temporary = tempdir().unwrap();
        let repository = SqliteLibraryRepository::with_environment(
            Arc::new(TestEnvironment::with_roots(temporary.path())),
            Arc::new(XdgRootResolver),
        );
        repository
            .import(&document(vec![entry("skills/review", None)]), false)
            .unwrap();
        let database = temporary.path().join("data/skilload/skilload.db");
        Connection::open(&database)
            .unwrap()
            .execute("UPDATE state_revision SET revision = ?1", params![i64::MAX])
            .unwrap();
        let before = fs::read(&database).unwrap();

        let error = repository
            .import(&document(vec![entry("skills/new", None)]), false)
            .unwrap_err();
        assert!(matches!(
            error,
            AppError::InvalidState { state, .. } if state == "state_revision_not_incrementable"
        ));
        assert_eq!(fs::read(database).unwrap(), before);
    }

    struct ReplaceCreatedLocksDirectory {
        locks: PathBuf,
    }

    impl PersistenceHooks for ReplaceCreatedLocksDirectory {
        fn before_first_lock(&self) -> Result<(), AppError> {
            fs::remove_dir(&self.locks).unwrap();
            fs::create_dir(&self.locks).unwrap();
            Err(AppError::Internal {
                incident_id: "replace-created-locks-directory".to_owned(),
            })
        }
    }

    #[test]
    fn first_import_cleanup_preserves_replaced_created_directory() {
        let temporary = tempdir().unwrap();
        let locks = temporary.path().join("state/skilload/locks");
        let repository = SqliteLibraryRepository::with_hooks(
            Arc::new(TestEnvironment::with_roots(temporary.path())),
            Arc::new(XdgRootResolver),
            Arc::new(ReplaceCreatedLocksDirectory {
                locks: locks.clone(),
            }),
        );

        let error = repository
            .import(&document(vec![entry("skills/review", None)]), false)
            .unwrap_err();
        assert_eq!(error.code(), "internal_invariant");
        assert!(locks.is_dir());
        assert!(temporary.path().join("state/skilload").is_dir());
    }
}
