use crate::adapters::configuration::{
    CreatedDirectory, LOCK_WAIT, acquire_restrictive_lock, ensure_restrictive_directory,
    environment_io, sync_created_directory_entries,
};
use crate::adapters::xdg::{SystemEnvironment, XdgRootResolver};
use crate::domain::configuration::NativePath;
use crate::domain::doctor::{
    DoctorAction, DoctorActionKind, DoctorData, DoctorFinding, DoctorOperation, DoctorOutcome,
    DoctorSeverity,
};
use crate::domain::library::{
    LIBRARY_FORMAT_VERSION, LibraryEntriesPage, LibraryEntry, LibraryImportOperation,
    LibraryImportOutcome, LibraryImportResult, LibraryMetadataChange, LibraryMetadataMutation,
    LibraryMetadataStoreResult, LibraryMutationOutcome, LibraryPage, LibrarySearchPage,
    LibrarySearchQuery, LibraryTrustState, PortableLibraryDocument, PortableLibraryEntry,
};
use crate::domain::source::{RefKind, ResolvedSkill, SourceIdentity, parse_decimal_u64};
use crate::domain::unicode_15_1::normalize_tag;
use crate::error::{AppError, Conflict};
use crate::ports::configuration::{Environment, ResolvedRoots, StateRootResolver};
use crate::ports::doctor::DatabaseMaintenance;
use crate::ports::library::LibraryRepository;
use rusqlite::{
    Connection, Error as SqlError, ErrorCode, OpenFlags, OptionalExtension, Transaction,
    backup::Backup, ffi, params,
};
use rustix::fs::{AtFlags, Dir, FileType, Mode, OFlags, fstat, linkat, openat, statat, unlinkat};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tempfile::{Builder, NamedTempFile};
use unicode_normalization::{UnicodeNormalization, is_nfc};

const SCHEMA_VERSION: u64 = 2;
const API_V1_UINT_MAX: i64 = 9_007_199_254_740_991;
const DATABASE_SIDECAR_SUFFIXES: [&str; 3] = ["-journal", "-wal", "-shm"];
const SQLITE_HEADER_MAGIC: &[u8; 16] = b"SQLite format 3\0";
const LIBRARY_FTS_CREATE_SQL: &str = "CREATE VIRTUAL TABLE library_fts USING fts5(\
canonical_source UNINDEXED, \
name, \
description, \
alias, \
tags_display, \
tags_comparison, \
category, \
note, \
repository, \
tokenize = 'unicode61 remove_diacritics 0')";
const FTS_ROW_COLUMNS: usize = 9;
const BACKUP_MANIFEST_FORMAT_VERSION: u64 = 1;
const MAX_BACKUP_MANIFEST_BYTES: usize = 4 * 1024;
const BASE_INTEGRITY_TABLES: [&str; 5] = [
    "sqlite_master",
    "schema_info",
    "state_revision",
    "library_entries",
    "library_tags",
];
const RECOVERABLE_EXPORT_INTEGRITY_TABLES: [&str; 2] = ["library_entries", "library_tags"];

const LIBRARY_FTS_SHADOW_TABLES: [&str; 5] = [
    "library_fts_data",
    "library_fts_idx",
    "library_fts_content",
    "library_fts_docsize",
    "library_fts_config",
];

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

    /// Hold the resolved `data/skilload` directory and prove it still names
    /// the root generation selected by `resolve_roots`.
    fn open_bound_data_directory(
        &self,
        roots: &ResolvedRoots,
    ) -> Result<ValidatedDataDirectory, AppError> {
        let roots = self.root_resolver.revalidate(roots)?;
        let directory = ValidatedDataDirectory::open(&roots.data.effective)?;
        self.root_resolver.revalidate(&roots)?;
        directory.revalidate()?;
        Ok(directory)
    }

    fn database_path(roots: &ResolvedRoots) -> PathBuf {
        roots.data.effective.join("skilload.db")
    }

    fn database_exists(
        directory: &ValidatedDataDirectory,
        database_name: &OsStr,
        path: &Path,
    ) -> Result<bool, AppError> {
        match statat(&directory.handle, database_name, AtFlags::SYMLINK_NOFOLLOW) {
            Ok(entry) if FileType::from_raw_mode(entry.st_mode) == FileType::RegularFile => {
                Ok(true)
            }
            Ok(_) => Err(AppError::invalid_state(
                "library_database",
                "database_path_is_not_a_regular_file",
                ["a regular data/skilload.db file"],
            )),
            Err(error) if error == rustix::io::Errno::NOENT => {
                Self::ensure_no_orphaned_database_sidecars(directory, database_name, path)?;
                Ok(false)
            }
            Err(error) => Err(environment_io(
                "XDG_DATA_HOME",
                path,
                "inspect skilload.db",
                io::Error::from(error),
            )),
        }
    }

    fn database_exists_with_details(
        &self,
        roots: &ResolvedRoots,
        path: &Path,
    ) -> Result<bool, AppError> {
        let roots = self.root_resolver.revalidate(roots)?;
        let Some(directory) = ValidatedDataDirectory::open_optional(&roots.data.effective)? else {
            self.root_resolver.revalidate(&roots)?;
            return Ok(false);
        };
        self.root_resolver.revalidate(&roots)?;
        directory.revalidate()?;
        self.hooks.before_existing_database_existence_probe(path)?;
        let database_name = path.file_name().ok_or_else(Self::database_identity_drift)?;
        let result = Self::database_exists(&directory, database_name, path).map_err(|error| {
            self.enrich_absent_database_corruption(error, &roots, path, &directory)
        });
        self.hooks.after_existing_database_existence_probe(path)?;
        directory.revalidate()?;
        self.root_resolver.revalidate(&roots)?;
        directory.revalidate()?;
        result
    }

    fn enrich_absent_database_corruption(
        &self,
        error: AppError,
        roots: &ResolvedRoots,
        path: &Path,
        directory: &ValidatedDataDirectory,
    ) -> AppError {
        let AppError::DatabaseCorrupt { database, .. } = error else {
            return error;
        };
        (|| {
            self.root_resolver.revalidate(roots)?;
            directory.revalidate()?;
            let database_name = path.file_name().ok_or_else(Self::database_identity_drift)?;
            match statat(&directory.handle, database_name, AtFlags::SYMLINK_NOFOLLOW) {
                Err(error) if error == rustix::io::Errno::NOENT => {}
                _ => return Err(Self::database_identity_drift()),
            }
            if !has_database_sidecar(directory, database_name) {
                return Err(Self::database_identity_drift());
            }
            let backups = Self::known_validated_backups(directory)?;
            directory.revalidate()?;
            self.root_resolver.revalidate(roots)?;
            directory.revalidate()?;
            Ok(AppError::DatabaseCorrupt {
                database,
                backups,
                recoverable_exports: Vec::new(),
            })
        })()
        .unwrap_or_else(|error| error)
    }

    fn ensure_no_orphaned_database_sidecars(
        directory: &ValidatedDataDirectory,
        database_name: &OsStr,
        path: &Path,
    ) -> Result<(), AppError> {
        for suffix in DATABASE_SIDECAR_SUFFIXES {
            let sidecar_name = database_sidecar_name(database_name, suffix);
            let sidecar = Self::database_sidecar_path(path, suffix)?;
            match statat(&directory.handle, &sidecar_name, AtFlags::SYMLINK_NOFOLLOW) {
                Ok(_) => {
                    return Err(AppError::database_corrupt(NativePath::new(
                        path.to_path_buf(),
                    )));
                }
                Err(error) if error == rustix::io::Errno::NOENT => {}
                Err(error) => {
                    return Err(environment_io(
                        "XDG_DATA_HOME",
                        &sidecar,
                        "inspect SQLite database sidecar",
                        io::Error::from(error),
                    ));
                }
            }
        }
        Ok(())
    }

    fn ensure_no_existing_database_sidecars(
        &self,
        directory: &ValidatedDataDirectory,
        database_name: &OsStr,
        path: &Path,
        identity: (u64, u64),
    ) -> Result<(), AppError> {
        for suffix in DATABASE_SIDECAR_SUFFIXES {
            let sidecar_name = database_sidecar_name(database_name, suffix);
            let sidecar = Self::database_sidecar_path(path, suffix)?;
            match statat(&directory.handle, &sidecar_name, AtFlags::SYMLINK_NOFOLLOW) {
                Ok(_) => {
                    return Err(self.database_corrupt_for_generation(directory, path, identity)?);
                }
                Err(error) if error == rustix::io::Errno::NOENT => {}
                Err(error) => {
                    return Err(environment_io(
                        "XDG_DATA_HOME",
                        &sidecar,
                        "inspect SQLite database sidecar",
                        io::Error::from(error),
                    ));
                }
            }
        }
        Ok(())
    }

    fn database_sidecar_path(path: &Path, suffix: &str) -> Result<PathBuf, AppError> {
        let database_name = path.file_name().ok_or_else(Self::database_identity_drift)?;
        Ok(path.with_file_name(database_sidecar_name(database_name, suffix)))
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
    fn revalidate_database_generation(
        directory: &ValidatedDataDirectory,
        path: &Path,
        identity: (u64, u64),
    ) -> Result<(), AppError> {
        let database_name = path.file_name().ok_or_else(Self::database_identity_drift)?;
        revalidate_database_entry(directory, database_name, identity)?;
        Self::revalidate_database_identity(path, identity)
    }

    /// Force SQLite to hold a read snapshot before a descriptor-bound path
    /// trusts that it has no journal/WAL/SHM companions. A companion that
    /// appears after this check cannot commit into the held snapshot.
    fn begin_existing_read_snapshot<'connection>(
        &self,
        connection: &'connection mut Connection,
        directory: &ValidatedDataDirectory,
        path: &Path,
        identity: (u64, u64),
    ) -> Result<Transaction<'connection>, AppError> {
        let database_name = path.file_name().ok_or_else(Self::database_identity_drift)?;
        let transaction = connection
            .transaction()
            .map_err(|error| database_error(path, error))?;
        transaction
            .query_row("PRAGMA schema_version", [], |row| row.get::<_, i64>(0))
            .map_err(|error| database_error(path, error))?;
        self.ensure_no_existing_database_sidecars(directory, database_name, path, identity)?;
        Self::revalidate_database_generation(directory, path, identity)?;
        Ok(transaction)
    }

    fn run_read_snapshot<T>(
        &self,
        connection: &mut Connection,
        directory: &ValidatedDataDirectory,
        path: &Path,
        identity: (u64, u64),
        operation: impl FnOnce(&Transaction<'_>) -> Result<T, AppError>,
    ) -> Result<T, AppError> {
        let transaction =
            self.begin_existing_read_snapshot(connection, directory, path, identity)?;
        let result = operation(&transaction);
        Self::revalidate_database_generation(directory, path, identity)?;
        let value = result?;
        transaction
            .commit()
            .map_err(|error| database_error(path, error))?;
        Self::revalidate_database_generation(directory, path, identity)?;
        Ok(value)
    }

    fn with_read_snapshot<T>(
        &self,
        directory: &ValidatedDataDirectory,
        path: &Path,
        operation: impl FnOnce(&Transaction<'_>) -> Result<T, AppError>,
    ) -> Result<T, AppError> {
        let (mut connection, identity) =
            self.open_existing_database(directory, path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        self.run_read_snapshot(&mut connection, directory, path, identity, operation)
            .map_err(|error| {
                self.enrich_database_corruption_for_generation(error, directory, path, identity)
            })
    }

    fn open_existing_database(
        &self,
        directory: &ValidatedDataDirectory,
        path: &Path,
        flags: OpenFlags,
    ) -> Result<(Connection, (u64, u64)), AppError> {
        let database_name = path.file_name().ok_or_else(Self::database_identity_drift)?;
        let (held_generation, identity) =
            self.pre_open_generation_gate(directory, database_name, path)?;
        self.hooks.before_existing_database_open(path)?;
        // A rollback journal can appear after the generation gate's first scan.
        // Reject it before SQLite attempts a read-only rollback.
        self.ensure_no_existing_database_sidecars(directory, database_name, path, identity)?;
        let connection = if flags.contains(OpenFlags::SQLITE_OPEN_READ_ONLY) {
            let held_path = PathBuf::from(format!("/dev/fd/{}", held_generation.as_raw_fd()));
            Connection::open_with_flags(&held_path, flags)
        } else {
            Connection::open_with_flags(path, flags | OpenFlags::SQLITE_OPEN_NOFOLLOW)
        }
        .map_err(|error| database_error(path, error))?;
        self.hooks.after_existing_database_open(path)?;
        // SQLite resolves `/dev/fd/*` to the temporary source name on Linux,
        // so `HAS_MOVED` would reject a descriptor whose planned path was restored.
        // The held descriptor and final identity revalidation establish the
        // read-only generation instead.
        if !flags.contains(OpenFlags::SQLITE_OPEN_READ_ONLY) {
            verify_sqlite_connection_identity(&connection)?;
            self.hooks
                .after_existing_database_connection_identity_check(path)?;
        }
        configure_connection(&connection, path)?;
        revalidate_database_entry(directory, database_name, identity)?;
        Self::revalidate_database_identity(path, identity)?;
        if !flags.contains(OpenFlags::SQLITE_OPEN_READ_ONLY) {
            verify_sqlite_connection_identity(&connection)?;
        }
        Ok((connection, identity))
    }

    fn read_existing(
        &self,
        directory: &ValidatedDataDirectory,
        path: &Path,
    ) -> Result<Vec<PortableLibraryEntry>, AppError> {
        self.with_read_snapshot(directory, path, |transaction| {
            validate_for_read(transaction, path)?;
            load_validated_entries(transaction, path)
        })
    }

    /// Read the independently portable Library projection. Recovery exports
    /// intentionally do not depend on `schema_info` or `state_revision`;
    /// their own tables, foreign keys, domain values, and integrity still
    /// have to be provably intact.
    fn read_exportable_entries(
        &self,
        directory: &ValidatedDataDirectory,
        path: &Path,
    ) -> Result<Vec<PortableLibraryEntry>, AppError> {
        self.with_read_snapshot(directory, path, |transaction| {
            load_recoverable_export_entries(transaction, path)
        })
    }

    fn read_exportable_entries_for_recovery(
        &self,
        directory: &ValidatedDataDirectory,
        path: &Path,
    ) -> Result<Vec<PortableLibraryEntry>, AppError> {
        let (mut connection, identity) =
            self.open_existing_database(directory, path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        self.run_read_snapshot(&mut connection, directory, path, identity, |transaction| {
            load_recoverable_export_entries(transaction, path)
        })
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
        let mut complete_entries = Vec::with_capacity(existing.len() + additions.len());
        complete_entries.extend_from_slice(existing);
        complete_entries.extend(additions.iter().cloned());
        PortableLibraryDocument {
            format_version: LIBRARY_FORMAT_VERSION,
            entries: complete_entries,
        }
        .into_transfer_size()?;

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

    fn with_existing_database<T>(
        &self,
        roots: &ResolvedRoots,
        operation: impl FnOnce(
            &Path,
            &ValidatedDataDirectory,
            &std::ffi::OsStr,
            &mut Connection,
            (u64, u64),
        ) -> Result<T, AppError>,
    ) -> Result<T, AppError> {
        let lock_path = roots.state.effective.join("locks/database.lock");
        let lock = acquire_restrictive_lock(roots, "database.lock", "database")?;
        let result = self.with_existing_database_locked(roots, operation);
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
    }

    fn with_existing_database_locked<T>(
        &self,
        roots: &ResolvedRoots,
        operation: impl FnOnce(
            &Path,
            &ValidatedDataDirectory,
            &std::ffi::OsStr,
            &mut Connection,
            (u64, u64),
        ) -> Result<T, AppError>,
    ) -> Result<T, AppError> {
        let roots = self.root_resolver.revalidate(roots)?;
        let database = Self::database_path(&roots);
        if !self.database_exists_with_details(&roots, &database)? {
            return Err(Self::database_identity_drift());
        }
        let data_directory = self.open_bound_data_directory(&roots)?;
        let database_name = database
            .file_name()
            .ok_or_else(Self::database_identity_drift)?
            .to_os_string();
        data_directory.revalidate()?;
        let (mut connection, identity) = self.open_existing_database(
            &data_directory,
            &database,
            OpenFlags::SQLITE_OPEN_READ_WRITE,
        )?;
        data_directory.revalidate()?;
        Self::revalidate_database_identity(&database, identity)?;
        operation(
            &database,
            &data_directory,
            database_name.as_os_str(),
            &mut connection,
            identity,
        )
        .map_err(|error| {
            self.enrich_database_corruption_for_generation(
                error,
                &data_directory,
                &database,
                identity,
            )
        })
    }

    fn import_existing(
        &self,
        roots: &ResolvedRoots,
        document: &PortableLibraryDocument,
    ) -> Result<LibraryImportOperation, AppError> {
        self.with_existing_database(
            roots,
            |database, data_directory, database_name, connection, identity| {
                self.import_existing_operation(
                    document,
                    database,
                    data_directory,
                    database_name,
                    connection,
                    identity,
                )
            },
        )
    }

    fn import_existing_with_lock(
        &self,
        roots: &ResolvedRoots,
        document: &PortableLibraryDocument,
    ) -> Result<LibraryImportOperation, AppError> {
        self.with_existing_database_locked(
            roots,
            |database, data_directory, database_name, connection, identity| {
                self.import_existing_operation(
                    document,
                    database,
                    data_directory,
                    database_name,
                    connection,
                    identity,
                )
            },
        )
    }

    fn import_existing_operation(
        &self,
        document: &PortableLibraryDocument,
        database: &Path,
        data_directory: &ValidatedDataDirectory,
        database_name: &std::ffi::OsStr,
        connection: &mut Connection,
        identity: (u64, u64),
    ) -> Result<LibraryImportOperation, AppError> {
        let transaction = connection
            .transaction()
            .map_err(|error| database_error(database, error))?;
        validate_database(&transaction, database)?;
        let existing = load_validated_entries(&transaction, database)?;
        let plan = Self::plan(document, &existing, false)?;
        if plan.additions.is_empty() {
            data_directory.revalidate()?;
            Self::revalidate_database_identity(database, identity)?;
            transaction
                .commit()
                .map_err(|error| database_error(database, error))?;
            Self::revalidate_database_identity(database, identity)?;
            return Ok(LibraryImportOperation {
                outcome: LibraryImportOutcome::Unchanged,
                data: plan.result,
            });
        }
        data_directory.revalidate()?;
        Self::revalidate_database_identity(database, identity)?;
        apply_additions(&transaction, &plan.additions, database)?;
        transaction
            .commit()
            .map_err(|error| database_error(database, error))?;
        self.hooks.after_commit_before_sync()?;
        sync_existing_database(database, data_directory, database_name, identity, || {
            self.hooks
                .after_existing_database_sync_before_parent_sync(database)
        })?;
        Ok(LibraryImportOperation {
            outcome: LibraryImportOutcome::Changed,
            data: plan.result,
        })
    }

    fn mutate_existing(
        &self,
        roots: &ResolvedRoots,
        mutation: &LibraryMetadataMutation,
    ) -> Result<LibraryMetadataStoreResult, AppError> {
        self.with_existing_database(
            roots,
            |database, data_directory, database_name, connection, identity| {
                let transaction = connection
                    .transaction()
                    .map_err(|error| database_error(database, error))?;
                validate_database(&transaction, database)?;
                let mut entries = load_validated_entries(&transaction, database)?;
                let target_index = entries
                    .iter()
                    .position(|entry| entry.skill.source.canonical == mutation.selector)
                    .ok_or_else(|| AppError::not_found("library", mutation.selector.clone()))?;
                let source = entries[target_index].skill.source.clone();

                if let LibraryMetadataChange::AliasSet(alias) = &mutation.change
                    && entries[target_index].alias.as_ref() != Some(alias)
                    && entries.iter().enumerate().any(|(index, entry)| {
                        index != target_index && entry.alias.as_ref() == Some(alias)
                    })
                {
                    return Err(AppError::conflict(vec![Conflict::internal_duplicate(
                        Some(alias.clone()),
                        source,
                    )]));
                }

                let outcome = entries[target_index].apply_metadata_change(&mutation.change)?;
                let changed_fields = if outcome == LibraryMutationOutcome::Changed {
                    vec![mutation.change.changed_field()]
                } else {
                    Vec::new()
                };
                if outcome == LibraryMutationOutcome::Unchanged {
                    let entry = entries[target_index].clone();
                    data_directory.revalidate()?;
                    Self::revalidate_database_identity(database, identity)?;
                    transaction
                        .commit()
                        .map_err(|error| database_error(database, error))?;
                    Self::revalidate_database_identity(database, identity)?;
                    return Ok(LibraryMetadataStoreResult {
                        outcome,
                        entry,
                        changed_fields,
                    });
                }

                let mut candidate = PortableLibraryDocument {
                    format_version: LIBRARY_FORMAT_VERSION,
                    entries,
                };
                candidate.validate_transfer_size()?;
                let entry = candidate
                    .entries
                    .iter()
                    .find(|entry| entry.skill.source == source)
                    .cloned()
                    .ok_or_else(|| AppError::Internal {
                        incident_id: "library_metadata_target_missing_after_validation".to_owned(),
                    })?;
                apply_metadata_change(&transaction, mutation, &source, &entry, database)?;
                advance_state_revision(&transaction, database)?;
                transaction
                    .commit()
                    .map_err(|error| database_error(database, error))?;
                self.hooks.after_commit_before_sync()?;
                sync_existing_database(database, data_directory, database_name, identity, || {
                    self.hooks
                        .after_existing_database_sync_before_parent_sync(database)
                })?;
                Ok(LibraryMetadataStoreResult {
                    outcome,
                    entry,
                    changed_fields,
                })
            },
        )
    }

    fn import_first(
        &self,
        roots: &ResolvedRoots,
        document: &PortableLibraryDocument,
        plan: ImportPlan,
    ) -> Result<LibraryImportOperation, AppError> {
        let lock_path = roots.state.effective.join("locks/database.lock");
        let mut created_directories = FirstImportDirectories::new();
        (|| {
            created_directories.record_created_directories(
                ensure_restrictive_directory(&roots.state.effective, "XDG_STATE_HOME")?,
                "XDG_STATE_HOME",
            );
            created_directories.record_created_directories(
                ensure_restrictive_directory(
                    &roots.state.effective.join("locks"),
                    "XDG_STATE_HOME",
                )?,
                "XDG_STATE_HOME",
            );
            self.hooks.before_first_lock()?;
            let lock = acquire_restrictive_lock(roots, "database.lock", "database")?;
            self.hooks.after_first_lock_acquired()?;

            let result = (|| {
                let roots = self.root_resolver.revalidate(roots)?;
                let database = Self::database_path(&roots);
                if self.database_exists_with_details(&roots, &database)? {
                    return self.import_existing_with_lock(&roots, document);
                }
                created_directories.record_created_directories(
                    ensure_restrictive_directory(&roots.data.effective, "XDG_DATA_HOME")?,
                    "XDG_DATA_HOME",
                );
                let roots = self.root_resolver.revalidate(&roots)?;
                let database = Self::database_path(&roots);
                if self.database_exists_with_details(&roots, &database)? {
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
                let mut connection = match staging.open_connection(
                    &staging_path,
                    || {
                        self.hooks
                            .after_first_staging_identity_check_before_open(&staging_path)
                    },
                    || {
                        self.hooks
                            .after_first_staging_identity_recheck_before_open(&staging_path)
                    },
                    || {
                        self.hooks
                            .after_first_staging_connection_open(&staging_path)
                    },
                ) {
                    Ok(connection) => connection,
                    Err(error) => return Err(error),
                };
                initialize_schema(&connection, &staging_path)?;
                let transaction = connection
                    .transaction()
                    .map_err(|error| database_error(&staging_path, error))?;
                apply_additions(&transaction, &plan.additions, &staging_path)?;
                self.hooks.before_commit(&staging_path)?;
                transaction
                    .commit()
                    .map_err(|error| database_error(&staging_path, error))?;
                self.hooks.after_commit_before_sync()?;
                drop(connection);
                staging.file.as_file().sync_all().map_err(|error| {
                    database_sync_error(&staging_path, "sync committed staging database", error)
                })?;
                self.hooks.before_publish()?;
                let roots = self.root_resolver.revalidate(&roots)?;
                let database = Self::database_path(&roots);
                data_directory.revalidate()?;
                if self.database_exists_with_details(&roots, &database)? {
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
                data_directory.revalidate()?;
                staging.verify_entry(&staging.name)?;
                let database_name = database
                    .file_name()
                    .ok_or_else(Self::database_identity_drift)?
                    .to_os_string();
                staging.link_to_absent_database(&database_name, &database)?;
                self.hooks
                    .after_first_publication_link_before_finalize(&database)?;
                data_directory.revalidate()?;
                staging.verify_entry(&database_name)?;
                self.hooks
                    .after_first_publication_identity_check_before_finalize(&database)?;
                data_directory.revalidate()?;
                staging.verify_entry(&database_name)?;
                staging.mark_published(&database_name, &database)?;
                staging.verify_entry(&database_name)?;
                data_directory.revalidate()?;
                data_directory.handle.sync_all().map_err(|error| {
                    database_sync_error(&database, "sync published database directory", error)
                })?;
                data_directory.revalidate()?;
                created_directories.sync_created_directories()?;
                self.hooks
                    .after_first_publish_sync_before_success(&database)?;
                data_directory.revalidate()?;
                staging.verify_entry(&database_name)?;
                Ok(LibraryImportOperation {
                    outcome: LibraryImportOutcome::Changed,
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

    fn pre_open_generation_gate(
        &self,
        directory: &ValidatedDataDirectory,
        database_name: &std::ffi::OsStr,
        path: &Path,
    ) -> Result<(File, (u64, u64)), AppError> {
        directory.revalidate()?;
        self.hooks.before_existing_database_generation_open(path)?;
        let mut file = File::from(
            openat(
                &directory.handle,
                database_name,
                OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK,
                Mode::empty(),
            )
            .map_err(|error| {
                environment_io(
                    "XDG_DATA_HOME",
                    path,
                    "open database for generation check",
                    io::Error::from(error),
                )
            })?,
        );
        let metadata = file.metadata().map_err(|error| {
            environment_io(
                "XDG_DATA_HOME",
                path,
                "inspect opened database for generation check",
                error,
            )
        })?;
        if !metadata.file_type().is_file() {
            return Err(Self::database_identity_drift());
        }
        let identity = metadata_identity(&metadata);
        revalidate_database_entry(directory, database_name, identity)?;
        let mut header = [0u8; 100];
        if file.read_exact(&mut header).is_err()
            || !header.starts_with(SQLITE_HEADER_MAGIC)
            || header[18] != 1
            || header[19] != 1
        {
            return Err(self.database_corrupt_for_generation(directory, path, identity)?);
        }
        self.ensure_no_existing_database_sidecars(directory, database_name, path, identity)?;
        Self::revalidate_database_generation(directory, path, identity)?;
        Ok((file, identity))
    }

    fn database_corrupt_for_generation(
        &self,
        directory: &ValidatedDataDirectory,
        path: &Path,
        identity: (u64, u64),
    ) -> Result<AppError, AppError> {
        self.hooks.before_database_corruption_details(path)?;
        Self::revalidate_database_generation(directory, path, identity)?;
        let backups = Self::known_validated_backups(directory)?;
        Self::revalidate_database_generation(directory, path, identity)?;
        Ok(AppError::DatabaseCorrupt {
            database: NativePath::new(path.to_path_buf()),
            backups,
            recoverable_exports: Vec::new(),
        })
    }

    fn recoverable_library_exports(
        &self,
        directory: &ValidatedDataDirectory,
        path: &Path,
        identity: (u64, u64),
    ) -> Result<Vec<String>, AppError> {
        Self::revalidate_database_generation(directory, path, identity)?;
        let readable = self
            .read_exportable_entries_for_recovery(directory, path)
            .is_ok();
        Self::revalidate_database_generation(directory, path, identity)?;
        Ok(readable
            .then(|| "library.export".to_owned())
            .into_iter()
            .collect())
    }

    fn enrich_database_corruption_for_generation(
        &self,
        error: AppError,
        directory: &ValidatedDataDirectory,
        path: &Path,
        identity: (u64, u64),
    ) -> AppError {
        let AppError::DatabaseCorrupt { database, .. } = error else {
            return error;
        };
        (|| {
            Self::revalidate_database_generation(directory, path, identity)?;
            let backups = Self::known_validated_backups(directory)?;
            let recoverable_exports =
                self.recoverable_library_exports(directory, path, identity)?;
            Self::revalidate_database_generation(directory, path, identity)?;
            Ok(AppError::DatabaseCorrupt {
                database,
                backups,
                recoverable_exports,
            })
        })()
        .unwrap_or_else(|error| error)
    }

    fn known_validated_backups(
        data_directory: &ValidatedDataDirectory,
    ) -> Result<Vec<NativePath>, AppError> {
        let Some(directory) =
            ValidatedDataDirectory::open_optional_child(data_directory, OsStr::new("backups"))?
        else {
            return Ok(Vec::new());
        };
        let mut stems = backup_manifest_stems(&directory)?;
        stems.sort();
        let mut validated = Vec::new();
        for stem in stems {
            if backup_pair_is_valid(&directory, &stem) {
                validated.push(NativePath::new(directory.path.join(format!("{stem}.db"))));
            }
        }
        directory.revalidate()?;
        data_directory.revalidate()?;
        Ok(validated)
    }

    fn list_page(
        &self,
        directory: &ValidatedDataDirectory,
        database: &Path,
        page: LibraryPage,
    ) -> Result<LibraryEntriesPage, AppError> {
        let (entries, total) = self.read_page(directory, database, page, ReadFilter::All)?;
        Ok(LibraryEntriesPage {
            entries,
            page,
            total,
        })
    }

    fn search_page(
        &self,
        directory: &ValidatedDataDirectory,
        database: &Path,
        query: &LibrarySearchQuery,
        page: LibraryPage,
    ) -> Result<LibrarySearchPage, AppError> {
        let expression = fts_match_expression(query);
        let (entries, total) =
            self.read_page(directory, database, page, ReadFilter::FtsMatch(&expression))?;
        Ok(LibrarySearchPage {
            original: query.original().to_owned(),
            entries,
            page,
            total,
        })
    }

    fn read_page(
        &self,
        directory: &ValidatedDataDirectory,
        database: &Path,
        page: LibraryPage,
        filter: ReadFilter<'_>,
    ) -> Result<(Vec<LibraryEntry>, u64), AppError> {
        self.with_read_snapshot(directory, database, |transaction| {
            let generation = validate_for_read(transaction, database)?;
            let total = match (&filter, generation) {
                (_, SchemaGeneration::Newer(found)) => {
                    return Err(schema_newer(found));
                }
                (ReadFilter::All, _) => count_entries(transaction, database)?,
                (ReadFilter::FtsMatch(_), SchemaGeneration::V1) => {
                    return Err(AppError::MigrationRequired {
                        domain: "library".to_owned(),
                        found_version: 1,
                        supported_version: SCHEMA_VERSION,
                    });
                }
                (ReadFilter::FtsMatch(expression), SchemaGeneration::V2) => {
                    validate_derived_database(transaction, database)
                        .map_err(map_derived_validation_error)?;
                    count_fts_matches(transaction, database, expression)?
                }
            };
            let entries = if page.offset() >= total {
                Vec::new()
            } else {
                query_page(transaction, database, &filter, &page)?
            };
            Ok((entries, total))
        })
    }

    fn get_entry(
        &self,
        directory: &ValidatedDataDirectory,
        database: &Path,
        selector: &str,
    ) -> Result<LibraryEntry, AppError> {
        self.with_read_snapshot(directory, database, |transaction| {
            let generation = validate_for_read(transaction, database)?;
            if let SchemaGeneration::Newer(found) = generation {
                return Err(schema_newer(found));
            }
            query_entry(transaction, database, selector)?
                .ok_or_else(|| AppError::not_found("library", selector.to_owned()))
        })
    }

    fn diagnosis_classification(
        &self,
        roots: &ResolvedRoots,
    ) -> Result<(Diagnosis, Vec<DoctorFinding>), AppError> {
        let roots = self.root_resolver.revalidate(roots)?;
        let database = Self::database_path(&roots);
        if !self.database_exists_with_details(&roots, &database)? {
            return Ok((Diagnosis::Absent, Vec::new()));
        }
        let directory = self.open_bound_data_directory(&roots)?;
        self.with_read_snapshot(&directory, &database, |transaction| {
            let generation = validate_for_read(transaction, &database)?;
            let (classification, finding) = match generation {
                SchemaGeneration::V1 => (
                    Diagnosis::RequiresMigration,
                    Some(DoctorFinding::database(
                        DoctorSeverity::Warning,
                        "library_database_migration_required",
                        "Library database schema 1 requires an explicit doctor --fix migration to schema 2.",
                        Some(NativePath::new(database.clone())),
                        true,
                        false,
                    )),
                ),
                SchemaGeneration::Newer(found) => (
                    Diagnosis::SchemaNewer,
                    Some(DoctorFinding::database(
                        DoctorSeverity::Error,
                        "library_schema_newer",
                        format!(
                            "Library database schema {found} is newer than the supported schema {SCHEMA_VERSION}."
                        ),
                        Some(NativePath::new(database.clone())),
                        false,
                        false,
                    )),
                ),
                SchemaGeneration::V2 => {
                    let derived_consistent =
                        self.derived_index_is_consistent(transaction, &database)?;
                    if derived_consistent {
                        (Diagnosis::Healthy, None)
                    } else {
                        (
                            Diagnosis::FtsInvalid,
                            Some(DoctorFinding::database(
                                DoctorSeverity::Warning,
                                "library_fts_invalid",
                                "Library full-text index is missing or inconsistent with base rows.",
                                Some(NativePath::new(database.clone())),
                                true,
                                false,
                            )),
                        )
                    }
                }
            };
            Ok(match finding {
                Some(finding) => (classification, vec![finding]),
                None => (classification, Vec::new()),
            })
        })
    }

    /// Compare derived index content against base rows on the live
    /// connection, then online-backup into memory and run the FTS5 special
    /// `integrity-check` there so the live filesystem stays untouched.
    fn derived_index_is_consistent(
        &self,
        connection: &Connection,
        database: &Path,
    ) -> Result<bool, AppError> {
        match validate_derived_database(connection, database) {
            Ok(()) => {}
            Err(AppError::DatabaseCorrupt { .. }) => return Ok(false),
            Err(error) => return Err(error),
        }
        let mut copy =
            Connection::open_in_memory().map_err(|error| database_error(database, error))?;
        let backup =
            Backup::new(connection, &mut copy).map_err(|error| database_error(database, error))?;
        backup
            .run_to_completion(512, Duration::ZERO, None)
            .map_err(|error| database_error(database, error))?;
        drop(backup);
        // A corruption/content failure in the scratch FTS check is derived
        // drift. Operational failures must escape so doctor never advertises
        // a repair without establishing an index inconsistency.
        match copy.execute(
            "INSERT INTO library_fts(library_fts) VALUES('integrity-check')",
            [],
        ) {
            Ok(_) => Ok(true),
            Err(error) => match database_error(database, error) {
                AppError::DatabaseCorrupt { .. } => Ok(false),
                error => Err(error),
            },
        }
    }

    fn migrate_v1(&self, roots: &ResolvedRoots) -> Result<Option<DoctorAction>, AppError> {
        let lock_path = roots.state.effective.join("locks/database.lock");
        let lock = acquire_restrictive_lock(roots, "database.lock", "database")?;
        let result = self.migrate_v1_locked(roots);
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
    }

    fn migrate_v1_locked(&self, roots: &ResolvedRoots) -> Result<Option<DoctorAction>, AppError> {
        let roots = self.root_resolver.revalidate(roots)?;
        let database = Self::database_path(&roots);
        if !self.database_exists_with_details(&roots, &database)? {
            return Err(Self::database_identity_drift());
        }
        let data_directory = self.open_bound_data_directory(&roots)?;
        let database_name = database
            .file_name()
            .ok_or_else(Self::database_identity_drift)?
            .to_os_string();
        data_directory.revalidate()?;
        let (mut connection, identity) = self.open_existing_database(
            &data_directory,
            &database,
            OpenFlags::SQLITE_OPEN_READ_WRITE,
        )?;
        let result: Result<Option<DoctorAction>, AppError> = (|| {
            data_directory.revalidate()?;
            Self::revalidate_database_identity(&database, identity)?;

            let state_revision_baseline = {
                let transaction = connection
                    .transaction()
                    .map_err(|error| database_error(&database, error))?;
                let generation = validate_base_for_generation(&transaction, &database)?;
                match generation {
                    SchemaGeneration::V2 => return Ok(None),
                    SchemaGeneration::V1 => {}
                    SchemaGeneration::Newer(found) => return Err(schema_newer(found)),
                }
                let revision = singleton_i64(
                    &transaction,
                    "SELECT revision FROM state_revision",
                    &database,
                )?;
                transaction
                    .commit()
                    .map_err(|error| database_error(&database, error))?;
                revision
            };
            Self::revalidate_database_identity(&database, identity)?;

            let entries = {
                let transaction = connection
                    .transaction()
                    .map_err(|error| database_error(&database, error))?;
                load_validated_entries(&transaction, &database)?
            };

            self.publish_validated_backup(
                &roots,
                &connection,
                &database,
                identity,
                state_revision_baseline,
            )?;

            let transaction = connection
                .transaction()
                .map_err(|error| database_error(&database, error))?;
            let generation = validate_base_for_generation(&transaction, &database)?;
            match generation {
                SchemaGeneration::V1 => {}
                SchemaGeneration::Newer(found) => return Err(schema_newer(found)),
                SchemaGeneration::V2 => {
                    return Err(AppError::invalid_state(
                        "library_database",
                        "migration_baseline_changed",
                        ["a validated schema 1 database after backup"],
                    ));
                }
            }
            let revision = singleton_i64(
                &transaction,
                "SELECT revision FROM state_revision",
                &database,
            )?;
            if revision != state_revision_baseline {
                return Err(AppError::invalid_state(
                    "library_database",
                    "migration_state_revision_changed",
                    ["the state revision recorded with the backup"],
                ));
            }
            transaction
                .execute_batch(LIBRARY_FTS_CREATE_SQL)
                .map_err(|error| database_error(&database, error))?;
            for entry in &entries {
                insert_fts_row(&transaction, entry, &database)?;
            }
            validate_derived_database(&transaction, &database)?;
            transaction
                .execute(
                    "INSERT INTO library_fts(library_fts) VALUES('integrity-check')",
                    [],
                )
                .map_err(|error| database_error(&database, error))?;
            let changed = transaction
                .execute("UPDATE schema_info SET version = 2 WHERE version = 1", [])
                .map_err(|error| database_error(&database, error))?;
            if changed != 1 {
                return Err(AppError::database_corrupt(NativePath::new(
                    database.to_path_buf(),
                )));
            }
            self.hooks.before_migration_commit(&database)?;
            transaction
                .commit()
                .map_err(|error| database_error(&database, error))?;
            self.hooks.after_migration_commit_before_sync(&database)?;
            sync_existing_database(
                &database,
                &data_directory,
                database_name.as_os_str(),
                identity,
                || {
                    self.hooks
                        .after_migration_database_sync_before_parent_sync(&database)
                },
            )?;
            Self::revalidate_database_identity(&database, identity)?;
            Ok(Some(DoctorAction {
                kind: DoctorActionKind::Migrate,
                target: NativePath::new(database.clone()),
                before: Some("schema_1".to_owned()),
                after: Some("schema_2".to_owned()),
            }))
        })();
        result.map_err(|error| {
            self.enrich_database_corruption_for_generation(
                error,
                &data_directory,
                &database,
                identity,
            )
        })
    }

    fn repair_fts(&self, roots: &ResolvedRoots) -> Result<Option<DoctorAction>, AppError> {
        let lock_path = roots.state.effective.join("locks/database.lock");
        let lock = acquire_restrictive_lock(roots, "database.lock", "database")?;
        let result = self.repair_fts_locked(roots);
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
    }

    fn repair_fts_locked(&self, roots: &ResolvedRoots) -> Result<Option<DoctorAction>, AppError> {
        let roots = self.root_resolver.revalidate(roots)?;
        let database = Self::database_path(&roots);
        if !self.database_exists_with_details(&roots, &database)? {
            return Err(Self::database_identity_drift());
        }
        let data_directory = self.open_bound_data_directory(&roots)?;
        let database_name = database
            .file_name()
            .ok_or_else(Self::database_identity_drift)?
            .to_os_string();
        data_directory.revalidate()?;
        let (mut connection, identity) = self.open_existing_database(
            &data_directory,
            &database,
            OpenFlags::SQLITE_OPEN_READ_WRITE,
        )?;
        let result: Result<Option<DoctorAction>, AppError> = (|| {
            data_directory.revalidate()?;
            Self::revalidate_database_identity(&database, identity)?;
            {
                let transaction = connection
                    .transaction()
                    .map_err(|error| database_error(&database, error))?;
                let generation = validate_base_for_fts_recovery(&transaction, &database)?;
                match generation {
                    SchemaGeneration::V2 => {}
                    SchemaGeneration::Newer(found) => return Err(schema_newer(found)),
                    SchemaGeneration::V1 => {
                        return Err(AppError::invalid_state(
                            "library_database",
                            "fts_repair_requires_schema_2",
                            ["a schema 2 database with intact base rows"],
                        ));
                    }
                }
                transaction
                    .commit()
                    .map_err(|error| database_error(&database, error))?;
            }
            // Re-diagnose under the durable lock: a concurrent `doctor --fix`
            // may have rebuilt the index after this process observed drift.
            // Rebuilding a healthy index would report a false `changed` result.
            if self.derived_index_is_consistent(&connection, &database)? {
                disable_writable_schema(&connection, &database)?;
                Self::revalidate_database_identity(&database, identity)?;
                return Ok(None);
            }
            if fts_schema_requires_detach(&connection) {
                let transaction = connection
                    .transaction()
                    .map_err(|error| database_error(&database, error))?;
                detach_damaged_fts_schema(&transaction, &database)?;
                transaction
                    .commit()
                    .map_err(|error| database_error(&database, error))?;
                reclaim_detached_fts_pages(&connection, &database)?;
                Self::revalidate_database_generation(&data_directory, &database, identity)?;
            }
            let transaction = connection
                .transaction()
                .map_err(|error| database_error(&database, error))?;
            rebuild_derived_index(&transaction, &database)?;
            validate_derived_database(&transaction, &database)?;
            transaction
                .execute(
                    "INSERT INTO library_fts(library_fts) VALUES('integrity-check')",
                    [],
                )
                .map_err(|error| database_error(&database, error))?;
            self.hooks.before_fts_rebuild_commit(&database)?;
            transaction
                .commit()
                .map_err(|error| database_error(&database, error))?;
            self.hooks.after_fts_rebuild_commit_before_sync(&database)?;
            sync_existing_database(
                &database,
                &data_directory,
                database_name.as_os_str(),
                identity,
                || Ok(()),
            )?;
            Self::revalidate_database_identity(&database, identity)?;
            Ok(Some(DoctorAction {
                kind: DoctorActionKind::Repair,
                target: NativePath::new(database.clone()),
                before: Some("fts_invalid".to_owned()),
                after: Some("fts_valid".to_owned()),
            }))
        })();
        result.map_err(|error| {
            self.enrich_database_corruption_for_generation(
                error,
                &data_directory,
                &database,
                identity,
            )
        })
    }

    /// Create, validate, and publish one standalone backup pair for the v1
    /// source connection. The live database is only migrated after the
    /// complete pair is durable in `data/backups/`.
    fn publish_validated_backup(
        &self,
        roots: &ResolvedRoots,
        source: &Connection,
        database: &Path,
        source_identity: (u64, u64),
        state_revision_baseline: i64,
    ) -> Result<(), AppError> {
        let backups_root = roots.data.effective.join("backups");
        let created_directories = ensure_restrictive_directory(&backups_root, "XDG_DATA_HOME")?;
        let backups_directory = ValidatedDataDirectory::open(&backups_root)?;
        backups_directory.revalidate()?;
        // On the first migration `data/backups` does not exist yet: syncing
        // the backups directory only persists its contents, so the new entry
        // in its parent data directory must be made crash-durable before any
        // live schema write can depend on the pair published here.
        sync_created_directory_entries(&created_directories, "XDG_DATA_HOME")?;

        let mut staging_db = Builder::new()
            .prefix(".skilload-backup-db-")
            .suffix(".tmp")
            .tempfile_in(&backups_root)
            .map_err(|error| {
                database_sync_error(database, "create staging backup database", error)
            })?;
        staging_db
            .as_file()
            .set_permissions(fs::Permissions::from_mode(0o600))
            .map_err(|error| {
                database_sync_error(database, "restrict staging backup database", error)
            })?;
        let staging_db_path = staging_db.path().to_path_buf();
        let staging_db_name = staging_db
            .path()
            .file_name()
            .ok_or_else(Self::database_identity_drift)?
            .to_os_string();

        self.hooks.before_backup_open(&staging_db_path)?;
        {
            let mut destination = Connection::open_with_flags(
                &staging_db_path,
                OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NOFOLLOW,
            )
            .map_err(|error| database_error(&staging_db_path, error))?;
            configure_connection(&destination, &staging_db_path)?;
            let backup = Backup::new(source, &mut destination)
                .map_err(|error| database_error(&staging_db_path, error))?;
            backup
                .run_to_completion(512, Duration::ZERO, None)
                .map_err(|error| database_error(&staging_db_path, error))?;
        }
        self.hooks.after_backup_copy(&staging_db_path)?;
        staging_db
            .as_file()
            .sync_all()
            .map_err(|error| database_sync_error(database, "sync staging backup", error))?;
        self.hooks.after_backup_sync(&staging_db_path)?;

        let database_bytes = staging_db
            .as_file()
            .metadata()
            .map_err(|error| database_sync_error(database, "inspect staging backup", error))?
            .len();
        let digest = sha256_of_file(staging_db.as_file())
            .map_err(|error| database_sync_error(database, "hash staging backup", error))?;
        self.hooks.after_backup_hash(&staging_db_path)?;

        let created_at_epoch_ns = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| {
                database_sync_error(
                    database,
                    "read backup clock",
                    io::Error::other(error.to_string()),
                )
            })?
            .as_nanos() as u64;
        let stem = format!("skilload-db-v1-to-v2-{created_at_epoch_ns}");
        let final_db_name = OsString::from(format!("{stem}.db"));
        let final_manifest_name = OsString::from(format!("{stem}.manifest.json"));

        {
            let mut verify_connection = Self::open_staging_backup(&staging_db_path)?;
            let transaction = verify_connection
                .transaction()
                .map_err(|error| database_error(&staging_db_path, error))?;
            let generation = read_schema_generation(&transaction, &staging_db_path)?;
            validate_base_database(&transaction, &staging_db_path)?;
            if generation != SchemaGeneration::V1 {
                return Err(AppError::invalid_state(
                    "library_database",
                    "backup_schema_unexpected",
                    ["a schema 1 standalone backup"],
                ));
            }
            let revision = singleton_i64(
                &transaction,
                "SELECT revision FROM state_revision",
                &staging_db_path,
            )?;
            if revision != state_revision_baseline {
                return Err(AppError::invalid_state(
                    "library_database",
                    "backup_state_revision_mismatch",
                    ["the live state revision at backup time"],
                ));
            }
        }
        self.hooks.after_backup_verify(&staging_db_path)?;

        let manifest = BackupManifestRecord {
            format_version: BACKUP_MANIFEST_FORMAT_VERSION,
            source_schema: 1,
            target_schema: SCHEMA_VERSION,
            created_at_epoch_ns,
            database_bytes,
            sha256: format!("sha256:{digest}"),
            source_device: source_identity.0,
            source_inode: source_identity.1,
            complete: true,
        };
        let manifest_bytes = serde_json::to_vec(&manifest).map_err(|error| {
            database_sync_error(database, "encode backup manifest", io::Error::other(error))
        })?;
        let mut staging_manifest = Builder::new()
            .prefix(".skilload-backup-manifest-")
            .suffix(".tmp")
            .tempfile_in(&backups_root)
            .map_err(|error| {
                database_sync_error(database, "create staging backup manifest", error)
            })?;
        staging_manifest
            .as_file()
            .set_permissions(fs::Permissions::from_mode(0o600))
            .map_err(|error| {
                database_sync_error(database, "restrict staging backup manifest", error)
            })?;
        staging_manifest
            .as_file()
            .write_all(&manifest_bytes)
            .map_err(|error| {
                database_sync_error(database, "write staging backup manifest", error)
            })?;
        staging_manifest.as_file().sync_all().map_err(|error| {
            database_sync_error(database, "sync staging backup manifest", error)
        })?;
        self.hooks.after_backup_manifest_sync(&staging_db_path)?;
        let staging_manifest_name = staging_manifest
            .path()
            .file_name()
            .ok_or_else(Self::database_identity_drift)?
            .to_os_string();

        backups_directory.revalidate()?;
        publish_no_clobber(
            &backups_directory,
            &staging_db_name,
            &final_db_name,
            database,
        )?;
        publish_no_clobber(
            &backups_directory,
            &staging_manifest_name,
            &final_manifest_name,
            database,
        )?;
        backups_directory
            .handle
            .sync_all()
            .map_err(|error| database_sync_error(database, "sync backups directory", error))?;
        backups_directory.revalidate()?;
        let held_db_identity = fstat(staging_db.as_file())
            .ok()
            .and_then(|stat| stat_identity(stat.st_dev, stat.st_ino));
        let held_manifest_identity = fstat(staging_manifest.as_file())
            .ok()
            .and_then(|stat| stat_identity(stat.st_dev, stat.st_ino));
        for (name, held) in [
            (&staging_db_name, held_db_identity),
            (&staging_manifest_name, held_manifest_identity),
        ] {
            let entry_identity = statat(&backups_directory.handle, name, AtFlags::SYMLINK_NOFOLLOW)
                .ok()
                .and_then(|entry| stat_identity(entry.st_dev, entry.st_ino));
            if entry_identity.is_some() && entry_identity == held {
                let _ = unlinkat(&backups_directory.handle, name, AtFlags::empty());
            }
        }
        staging_db.disable_cleanup(true);
        staging_manifest.disable_cleanup(true);
        verify_published_entry(&backups_directory, &final_db_name, staging_db.as_file())?;
        verify_published_entry(
            &backups_directory,
            &final_manifest_name,
            staging_manifest.as_file(),
        )?;
        backups_directory.handle.sync_all().map_err(|error| {
            database_sync_error(database, "sync published backup directory", error)
        })?;
        self.hooks
            .after_backup_publish(&backups_root.join(final_db_name))?;
        Ok(())
    }

    fn open_staging_backup(staging_path: &Path) -> Result<Connection, AppError> {
        let connection = Connection::open_with_flags(
            staging_path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NOFOLLOW,
        )
        .map_err(|error| database_error(staging_path, error))?;
        verify_sqlite_connection_identity(&connection)?;
        configure_connection(&connection, staging_path)?;
        Ok(connection)
    }
}

fn database_sidecar_name(database_name: &OsStr, suffix: &str) -> OsString {
    let mut sidecar_name = database_name.to_os_string();
    sidecar_name.push(suffix);
    sidecar_name
}

fn has_database_sidecar(directory: &ValidatedDataDirectory, database_name: &OsStr) -> bool {
    DATABASE_SIDECAR_SUFFIXES.into_iter().any(|suffix| {
        match statat(
            &directory.handle,
            database_sidecar_name(database_name, suffix),
            AtFlags::SYMLINK_NOFOLLOW,
        ) {
            Ok(_) => true,
            Err(error) => error != rustix::io::Errno::NOENT,
        }
    })
}

/// List candidate manifest stems through a held directory descriptor so a
/// replacement of the directory pathname cannot redirect later validation.
fn backup_manifest_stems(directory: &ValidatedDataDirectory) -> Result<Vec<String>, AppError> {
    let mut entries = Dir::read_from(&directory.handle).map_err(|error| {
        environment_io(
            "XDG_DATA_HOME",
            &directory.path,
            "enumerate database backup manifests",
            io::Error::from(error),
        )
    })?;
    let mut stems = Vec::new();
    while let Some(entry) = entries.read() {
        let entry = entry.map_err(|error| {
            environment_io(
                "XDG_DATA_HOME",
                &directory.path,
                "enumerate database backup manifests",
                io::Error::from(error),
            )
        })?;
        let Ok(name) = entry.file_name().to_str() else {
            continue;
        };
        if let Some(stem) = name.strip_suffix(".manifest.json") {
            stems.push(stem.to_owned());
        }
    }
    Ok(stems)
}

fn open_regular_file_at(
    directory: &ValidatedDataDirectory,
    name: &std::ffi::OsStr,
) -> Option<File> {
    let file = File::from(
        openat(
            &directory.handle,
            name,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK,
            Mode::empty(),
        )
        .ok()?,
    );
    if file.metadata().ok()?.file_type().is_file() {
        Some(file)
    } else {
        None
    }
}

fn directory_entry_matches_file(
    directory: &ValidatedDataDirectory,
    name: &std::ffi::OsStr,
    file: &File,
) -> bool {
    fstat(file)
        .ok()
        .zip(statat(&directory.handle, name, AtFlags::SYMLINK_NOFOLLOW).ok())
        .is_some_and(|(held, entry)| held.st_dev == entry.st_dev && held.st_ino == entry.st_ino)
}

/// Validate a held standalone v1 backup before it is shown in corruption
/// diagnostics. Its header excludes WAL-mode opens, and the SQLite checks
/// prove that the manifest's claimed source generation has intact base rows.
fn standalone_backup_is_valid(
    directory: &ValidatedDataDirectory,
    database_name: &OsStr,
    database: &mut File,
    path: &Path,
) -> bool {
    let mut header = [0u8; 100];
    if database.seek(SeekFrom::Start(0)).is_err()
        || database.read_exact(&mut header).is_err()
        || !header.starts_with(SQLITE_HEADER_MAGIC)
        || header[18] != 1
        || header[19] != 1
        || database.seek(SeekFrom::Start(0)).is_err()
    {
        return false;
    }
    let validation = (|| {
        let held_path = PathBuf::from(format!("/dev/fd/{}", database.as_raw_fd()));
        let Ok(mut connection) =
            Connection::open_with_flags(&held_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        else {
            return false;
        };
        if configure_connection(&connection, path).is_err() {
            return false;
        }
        let Ok(transaction) = connection.transaction() else {
            return false;
        };
        transaction
            .query_row("PRAGMA schema_version", [], |row| row.get::<_, i64>(0))
            .is_ok()
            && !has_database_sidecar(directory, database_name)
            && matches!(
                read_schema_generation(&transaction, path),
                Ok(SchemaGeneration::V1)
            )
            && validate_base_database(&transaction, path).is_ok()
            && !has_database_sidecar(directory, database_name)
            && transaction.commit().is_ok()
    })();
    database.seek(SeekFrom::Start(0)).is_ok() && validation
}

/// A backup pair counts as validated only when both files are opened through
/// the held directory with no-follow descriptors, remain linked under their
/// advertised names, have no SQLite sidecars, have a compatible manifest,
/// and contain the verified standalone v1 database it describes.
fn backup_pair_is_valid(directory: &ValidatedDataDirectory, stem: &str) -> bool {
    let database_name = OsString::from(format!("{stem}.db"));
    let manifest_name = OsString::from(format!("{stem}.manifest.json"));
    if has_database_sidecar(directory, &database_name) {
        return false;
    }
    let Some(mut manifest) = open_regular_file_at(directory, &manifest_name) else {
        return false;
    };
    if manifest.metadata().map_or(true, |metadata| {
        metadata.len() > MAX_BACKUP_MANIFEST_BYTES as u64
    }) {
        return false;
    }
    let mut record_bytes = Vec::with_capacity(MAX_BACKUP_MANIFEST_BYTES + 1);
    if Read::by_ref(&mut manifest)
        .take((MAX_BACKUP_MANIFEST_BYTES + 1) as u64)
        .read_to_end(&mut record_bytes)
        .is_err()
        || record_bytes.len() > MAX_BACKUP_MANIFEST_BYTES
    {
        return false;
    }
    let Ok(record) = serde_json::from_slice::<BackupManifestRecord>(&record_bytes) else {
        return false;
    };
    if !record.complete
        || record.format_version != BACKUP_MANIFEST_FORMAT_VERSION
        || record.source_schema != 1
        || record.target_schema != SCHEMA_VERSION
    {
        return false;
    }
    let Some(mut database) = open_regular_file_at(directory, &database_name) else {
        return false;
    };
    let Ok(metadata) = database.metadata() else {
        return false;
    };
    if record.database_bytes != metadata.len()
        || !standalone_backup_is_valid(
            directory,
            &database_name,
            &mut database,
            &directory.path.join(&database_name),
        )
    {
        return false;
    }
    let Ok(digest) = sha256_of_file(&database) else {
        return false;
    };
    format!("sha256:{digest}") == record.sha256
        && !has_database_sidecar(directory, &database_name)
        && directory_entry_matches_file(directory, &manifest_name, &manifest)
        && directory_entry_matches_file(directory, &database_name, &database)
}

impl Default for SqliteLibraryRepository {
    fn default() -> Self {
        Self::new()
    }
}

impl LibraryRepository for SqliteLibraryRepository {
    fn list(&self, page: &LibraryPage) -> Result<LibraryEntriesPage, AppError> {
        let roots = self.resolve_roots()?;
        let database = Self::database_path(&roots);
        if !self.database_exists_with_details(&roots, &database)? {
            return Ok(LibraryEntriesPage {
                entries: Vec::new(),
                page: *page,
                total: 0,
            });
        }
        let directory = self.open_bound_data_directory(&roots)?;
        self.list_page(&directory, &database, *page)
    }

    fn search(
        &self,
        query: &LibrarySearchQuery,
        page: &LibraryPage,
    ) -> Result<LibrarySearchPage, AppError> {
        let roots = self.resolve_roots()?;
        let database = Self::database_path(&roots);
        if !self.database_exists_with_details(&roots, &database)? {
            return Ok(LibrarySearchPage {
                original: query.original().to_owned(),
                entries: Vec::new(),
                page: *page,
                total: 0,
            });
        }
        let directory = self.open_bound_data_directory(&roots)?;
        self.search_page(&directory, &database, query, *page)
    }

    fn get(&self, selector: &str) -> Result<LibraryEntry, AppError> {
        let roots = self.resolve_roots()?;
        let database = Self::database_path(&roots);
        if !self.database_exists_with_details(&roots, &database)? {
            return Err(AppError::not_found("library", selector.to_owned()));
        }
        let directory = self.open_bound_data_directory(&roots)?;
        self.get_entry(&directory, &database, selector)
    }

    fn export(&self) -> Result<PortableLibraryDocument, AppError> {
        let roots = self.resolve_roots()?;
        let database = Self::database_path(&roots);
        if !self.database_exists_with_details(&roots, &database)? {
            return Ok(PortableLibraryDocument::empty());
        }
        let directory = self.open_bound_data_directory(&roots)?;
        let entries = self.read_exportable_entries(&directory, &database)?;
        let mut document = PortableLibraryDocument {
            format_version: LIBRARY_FORMAT_VERSION,
            entries,
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
            let existing = if self.database_exists_with_details(&roots, &database)? {
                let directory = self.open_bound_data_directory(&roots)?;
                self.read_existing(&directory, &database)?
            } else {
                Vec::new()
            };
            let plan = Self::plan(&document, &existing, true)?;
            return Ok(LibraryImportOperation {
                outcome: LibraryImportOutcome::Observed,
                data: plan.result,
            });
        }
        if self.database_exists_with_details(&roots, &database)? {
            self.import_existing(&roots, &document)
        } else if document.entries.is_empty() {
            Ok(LibraryImportOperation {
                outcome: LibraryImportOutcome::Unchanged,
                data: Self::plan(&document, &[], false)?.result,
            })
        } else {
            let plan = Self::plan(&document, &[], false)?;
            self.import_first(&roots, &document, plan)
        }
    }

    fn mutate_metadata(
        &self,
        mutation: &LibraryMetadataMutation,
    ) -> Result<LibraryMetadataStoreResult, AppError> {
        mutation.change.validate()?;
        let roots = self.resolve_roots()?;
        let database = Self::database_path(&roots);
        if !self.database_exists_with_details(&roots, &database)? {
            return Err(AppError::not_found("library", mutation.selector.clone()));
        }
        self.mutate_existing(&roots, mutation)
    }
}

enum Diagnosis {
    Absent,
    Healthy,
    RequiresMigration,
    SchemaNewer,
    FtsInvalid,
}

impl DatabaseMaintenance for SqliteLibraryRepository {
    fn inspect(&self) -> Result<DoctorData, AppError> {
        let roots = self.resolve_roots()?;
        let (classification, findings) = self.diagnosis_classification(&roots)?;
        let database_writable = matches!(classification, Diagnosis::Absent | Diagnosis::Healthy);
        Ok(DoctorData {
            fix_requested: false,
            findings,
            actions: Vec::new(),
            database_writable,
        })
    }
    fn fix(&self) -> Result<DoctorOperation, AppError> {
        let roots = self.resolve_roots()?;
        let (classification, mut findings) = self.diagnosis_classification(&roots)?;
        let action = match classification {
            Diagnosis::Absent | Diagnosis::Healthy | Diagnosis::SchemaNewer => None,
            Diagnosis::RequiresMigration => self.migrate_v1(&roots)?,
            Diagnosis::FtsInvalid => self.repair_fts(&roots)?,
        };
        match action {
            Some(action) => {
                for finding in &mut findings {
                    finding.fixed = true;
                }
                Ok(DoctorOperation {
                    outcome: DoctorOutcome::Changed,
                    data: DoctorData {
                        fix_requested: true,
                        findings,
                        actions: vec![action],
                        database_writable: true,
                    },
                })
            }
            None => {
                let (classification, findings) = if matches!(
                    classification,
                    Diagnosis::FtsInvalid | Diagnosis::RequiresMigration
                ) {
                    // A concurrent `doctor --fix` may have repaired the state
                    // between this diagnosis and acquiring the durable lock.
                    // Report the current state rather than the stale finding.
                    self.diagnosis_classification(&roots)?
                } else {
                    (classification, findings)
                };
                let database_writable =
                    matches!(classification, Diagnosis::Absent | Diagnosis::Healthy);
                Ok(DoctorOperation {
                    outcome: DoctorOutcome::Unchanged,
                    data: DoctorData {
                        fix_requested: true,
                        findings,
                        actions: Vec::new(),
                        database_writable,
                    },
                })
            }
        }
    }
}

/// Private versioned record describing one complete standalone migration
/// backup pair in `data/backups/`. Never part of API-v2 or portable export.
#[derive(Debug, Serialize, Deserialize)]
struct BackupManifestRecord {
    format_version: u64,
    source_schema: u64,
    target_schema: u64,
    created_at_epoch_ns: u64,
    database_bytes: u64,
    sha256: String,
    source_device: u64,
    source_inode: u64,
    complete: bool,
}

fn sha256_of_file(mut file: &File) -> io::Result<String> {
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 65_536];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    let digest = hasher.finalize();
    let mut hex = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(hex, "{byte:02x}");
    }
    Ok(hex)
}

/// Publish one staging entry under its final no-clobber name with `linkat`;
/// an existing foreign entry is preserved and reported.
fn publish_no_clobber(
    directory: &ValidatedDataDirectory,
    staging_name: &std::ffi::OsStr,
    final_name: &std::ffi::OsStr,
    database: &Path,
) -> Result<(), AppError> {
    linkat(
        &directory.handle,
        staging_name,
        &directory.handle,
        final_name,
        AtFlags::empty(),
    )
    .map_err(|error| {
        let error: io::Error = error.into();
        if error.kind() == io::ErrorKind::AlreadyExists {
            AppError::invalid_state(
                "library_database",
                "backup_name_collision",
                ["an unpublished backup name"],
            )
        } else {
            database_sync_error(database, "publish backup pair", error)
        }
    })
}

fn verify_published_entry(
    directory: &ValidatedDataDirectory,
    final_name: &std::ffi::OsStr,
    held: &File,
) -> Result<(), AppError> {
    let held_stat = fstat(held).map_err(|_| SqliteLibraryRepository::database_identity_drift())?;
    let entry = statat(&directory.handle, final_name, AtFlags::SYMLINK_NOFOLLOW)
        .map_err(|_| SqliteLibraryRepository::database_identity_drift())?;
    if stat_identity(entry.st_dev, entry.st_ino)
        != stat_identity(held_stat.st_dev, held_stat.st_ino)
    {
        return Err(SqliteLibraryRepository::database_identity_drift());
    }
    Ok(())
}

trait PersistenceHooks: Send + Sync {
    fn before_first_lock(&self) -> Result<(), AppError> {
        Ok(())
    }

    fn after_first_lock_acquired(&self) -> Result<(), AppError> {
        Ok(())
    }

    fn after_first_staging_identity_check_before_open(
        &self,
        _staging: &Path,
    ) -> Result<(), AppError> {
        Ok(())
    }

    fn after_first_staging_identity_recheck_before_open(
        &self,
        _staging: &Path,
    ) -> Result<(), AppError> {
        Ok(())
    }

    fn after_first_staging_connection_open(&self, _staging: &Path) -> Result<(), AppError> {
        Ok(())
    }

    fn before_commit(&self, _staging: &Path) -> Result<(), AppError> {
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
    fn after_first_publication_link_before_finalize(
        &self,
        _database: &Path,
    ) -> Result<(), AppError> {
        Ok(())
    }

    fn after_first_publication_identity_check_before_finalize(
        &self,
        _database: &Path,
    ) -> Result<(), AppError> {
        Ok(())
    }

    fn after_first_publish_sync_before_success(&self, _database: &Path) -> Result<(), AppError> {
        Ok(())
    }

    fn before_existing_database_existence_probe(&self, _database: &Path) -> Result<(), AppError> {
        Ok(())
    }

    fn after_existing_database_existence_probe(&self, _database: &Path) -> Result<(), AppError> {
        Ok(())
    }

    fn before_existing_database_generation_open(&self, _database: &Path) -> Result<(), AppError> {
        Ok(())
    }
    fn before_database_corruption_details(&self, _database: &Path) -> Result<(), AppError> {
        Ok(())
    }

    fn before_existing_database_open(&self, _database: &Path) -> Result<(), AppError> {
        Ok(())
    }

    fn after_existing_database_open(&self, _database: &Path) -> Result<(), AppError> {
        Ok(())
    }

    fn after_existing_database_connection_identity_check(
        &self,
        _database: &Path,
    ) -> Result<(), AppError> {
        Ok(())
    }

    fn after_existing_database_sync_before_parent_sync(
        &self,
        _database: &Path,
    ) -> Result<(), AppError> {
        Ok(())
    }

    fn before_backup_open(&self, _staging: &Path) -> Result<(), AppError> {
        Ok(())
    }

    fn after_backup_copy(&self, _staging: &Path) -> Result<(), AppError> {
        Ok(())
    }

    fn after_backup_sync(&self, _staging: &Path) -> Result<(), AppError> {
        Ok(())
    }

    fn after_backup_hash(&self, _staging: &Path) -> Result<(), AppError> {
        Ok(())
    }

    fn after_backup_verify(&self, _staging: &Path) -> Result<(), AppError> {
        Ok(())
    }

    fn after_backup_manifest_sync(&self, _staging: &Path) -> Result<(), AppError> {
        Ok(())
    }

    fn after_backup_publish(&self, _backup: &Path) -> Result<(), AppError> {
        Ok(())
    }

    fn before_migration_commit(&self, _database: &Path) -> Result<(), AppError> {
        Ok(())
    }

    fn after_migration_commit_before_sync(&self, _database: &Path) -> Result<(), AppError> {
        Ok(())
    }

    fn after_migration_database_sync_before_parent_sync(
        &self,
        _database: &Path,
    ) -> Result<(), AppError> {
        Ok(())
    }

    fn before_fts_rebuild_commit(&self, _database: &Path) -> Result<(), AppError> {
        Ok(())
    }

    fn after_fts_rebuild_commit_before_sync(&self, _database: &Path) -> Result<(), AppError> {
        Ok(())
    }
}

struct NoopPersistenceHooks;

impl PersistenceHooks for NoopPersistenceHooks {}

struct ImportPlan {
    additions: Vec<PortableLibraryEntry>,
    result: LibraryImportResult,
}

struct FirstImportCreatedDirectory {
    directory: CreatedDirectory,
    variable: &'static str,
}

struct FirstImportDirectories {
    created_directories: Vec<FirstImportCreatedDirectory>,
}

impl FirstImportDirectories {
    fn new() -> Self {
        Self {
            created_directories: Vec::new(),
        }
    }

    fn record_created_directories(
        &mut self,
        directories: Vec<CreatedDirectory>,
        variable: &'static str,
    ) {
        self.created_directories
            .extend(
                directories
                    .into_iter()
                    .map(|directory| FirstImportCreatedDirectory {
                        directory,
                        variable,
                    }),
            );
    }

    fn sync_created_directories(&self) -> Result<(), AppError> {
        for created in self.created_directories.iter().rev() {
            sync_created_directory_entries(
                std::slice::from_ref(&created.directory),
                created.variable,
            )?;
        }
        Ok(())
    }
}

struct ValidatedDataDirectory {
    path: PathBuf,
    identity: (u64, u64),
    handle: File,
}

impl ValidatedDataDirectory {
    fn open_optional(path: &Path) -> Result<Option<Self>, AppError> {
        match fs::symlink_metadata(path) {
            Ok(_) => Self::open(path).map(Some),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(environment_io(
                "XDG_DATA_HOME",
                path,
                "inspect data directory",
                error,
            )),
        }
    }

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
    fn open_optional_child(&self, name: &OsStr) -> Result<Option<Self>, AppError> {
        self.revalidate()?;
        let handle = match openat(
            &self.handle,
            name,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::NONBLOCK,
            Mode::empty(),
        ) {
            Ok(handle) => File::from(handle),
            Err(error) if error == rustix::io::Errno::NOENT => {
                self.revalidate()?;
                return Ok(None);
            }
            Err(_) => return Err(SqliteLibraryRepository::database_identity_drift()),
        };
        let metadata = handle
            .metadata()
            .map_err(|_| SqliteLibraryRepository::database_identity_drift())?;
        let entry = statat(&self.handle, name, AtFlags::SYMLINK_NOFOLLOW)
            .map_err(|_| SqliteLibraryRepository::database_identity_drift())?;
        let identity = metadata_identity(&metadata);
        if !metadata.file_type().is_dir()
            || stat_identity(entry.st_dev, entry.st_ino) != Some(identity)
        {
            return Err(SqliteLibraryRepository::database_identity_drift());
        }
        self.revalidate()?;
        Ok(Some(Self {
            path: self.path.join(Path::new(name)),
            identity,
            handle,
        }))
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
    publication_name: Option<OsString>,
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
            publication_name: None,
            published: false,
        })
    }

    fn open_connection(
        &self,
        path: &Path,
        after_identity_check_before_open: impl FnOnce() -> Result<(), AppError>,
        after_identity_recheck_before_open: impl FnOnce() -> Result<(), AppError>,
        after_connection_open: impl FnOnce() -> Result<(), AppError>,
    ) -> Result<Connection, AppError> {
        self.verify_entry(&self.name)?;
        after_identity_check_before_open()?;
        self.verify_entry(&self.name)?;
        after_identity_recheck_before_open()?;
        let connection = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NOFOLLOW,
        )
        .map_err(|error| database_error(path, error))?;
        after_connection_open()?;
        verify_sqlite_connection_identity(&connection)?;
        self.verify_entry(&self.name)?;
        configure_connection(&connection, path)?;
        Ok(connection)
    }

    fn mark_published(
        &mut self,
        database_name: &std::ffi::OsStr,
        database: &Path,
    ) -> Result<(), AppError> {
        if !self.entry_is_held(database_name) {
            return Err(SqliteLibraryRepository::database_identity_drift());
        }
        if self.entry_is_held(&self.name) {
            unlinkat(&self.directory.handle, &self.name, AtFlags::empty()).map_err(|error| {
                database_sync_error(database, "remove published staging link", error.into())
            })?;
        }
        self.file.disable_cleanup(true);
        self.published = true;
        Ok(())
    }

    fn link_to_absent_database(
        &mut self,
        database_name: &std::ffi::OsStr,
        database: &Path,
    ) -> Result<(), AppError> {
        self.verify_entry(&self.name)?;
        linkat(
            &self.directory.handle,
            &self.name,
            &self.directory.handle,
            database_name,
            AtFlags::empty(),
        )
        .map_err(|error| {
            let error: io::Error = error.into();
            if error.kind() == io::ErrorKind::AlreadyExists {
                SqliteLibraryRepository::database_identity_drift()
            } else {
                database_sync_error(database, "link committed staging database", error)
            }
        })?;
        self.publication_name = Some(database_name.to_os_string());
        self.verify_entry(database_name)
    }

    fn entry_is_held(&self, name: &std::ffi::OsStr) -> bool {
        fstat(self.file.as_file())
            .ok()
            .zip(statat(&self.directory.handle, name, AtFlags::SYMLINK_NOFOLLOW).ok())
            .is_some_and(|(held, entry)| held.st_dev == entry.st_dev && held.st_ino == entry.st_ino)
    }

    fn cleanup_entry_if_held(&self, name: &std::ffi::OsStr) {
        if self.entry_is_held(name) {
            let _ = unlinkat(&self.directory.handle, name, AtFlags::empty());
        }
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
        if let Some(publication_name) = &self.publication_name {
            self.cleanup_entry_if_held(publication_name);
        }
        self.cleanup_entry_if_held(&self.name);
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
            INSERT INTO schema_info (version) VALUES (2);
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
    connection
        .execute_batch(LIBRARY_FTS_CREATE_SQL)
        .map_err(|error| database_error(path, error))?;
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SchemaGeneration {
    V1,
    V2,
    Newer(u64),
}

fn schema_newer(found_version: u64) -> AppError {
    AppError::SchemaNewer {
        domain: "library".to_owned(),
        found_version,
        supported_version: SCHEMA_VERSION,
    }
}

fn read_schema_generation(
    connection: &Connection,
    path: &Path,
) -> Result<SchemaGeneration, AppError> {
    connection
        .execute_batch("PRAGMA foreign_keys = ON;")
        .map_err(|error| database_error(path, error))?;
    let raw_version = singleton_i64(connection, "SELECT version FROM schema_info", path)?;
    if !(1..=API_V1_UINT_MAX).contains(&raw_version) {
        return Err(AppError::database_corrupt(NativePath::new(
            path.to_path_buf(),
        )));
    }
    Ok(match raw_version {
        1 => SchemaGeneration::V1,
        2 => SchemaGeneration::V2,
        found => SchemaGeneration::Newer(found as u64),
    })
}

/// Base validation covers the v1 Library tables, foreign keys, and domain
/// rows. It applies identically to every generation so v1 databases stay
/// readable for `library list`, `library get`, and portable export. The
/// integrity pass checks the schema table and the four base tables
/// individually: FTS5 shadow-table damage must stay in the derived layer,
/// where it is a doctor-fixable finding instead of base corruption.
fn enable_writable_schema(connection: &Connection, path: &Path) -> Result<(), AppError> {
    // This is connection-local and permits inspection of intact base tables
    // when only the derived FTS schema text is malformed.
    connection
        .execute_batch("PRAGMA writable_schema = ON;")
        .map_err(|error| database_error(path, error))
}

fn disable_writable_schema(connection: &Connection, path: &Path) -> Result<(), AppError> {
    connection
        .execute_batch("PRAGMA writable_schema = OFF;")
        .map_err(|error| database_error(path, error))
}

fn validate_table_integrity(
    connection: &Connection,
    path: &Path,
    tables: &[&str],
) -> Result<(), AppError> {
    for table in tables {
        let integrity: Option<String> = connection
            .query_row(&format!("PRAGMA integrity_check('{table}')"), [], |row| {
                row.get(0)
            })
            .optional()
            .map_err(|error| database_error(path, error))?;
        if integrity.as_deref() != Some("ok") {
            return Err(AppError::database_corrupt(NativePath::new(
                path.to_path_buf(),
            )));
        }
    }
    Ok(())
}

fn validate_foreign_keys(connection: &Connection, path: &Path) -> Result<(), AppError> {
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

fn validate_base_database(connection: &Connection, path: &Path) -> Result<(), AppError> {
    let state_revision = singleton_i64(connection, "SELECT revision FROM state_revision", path)?;
    if state_revision < 0 {
        return Err(AppError::database_corrupt(NativePath::new(
            path.to_path_buf(),
        )));
    }
    validate_library_tags_schema(connection, path)?;
    validate_table_integrity(connection, path, &BASE_INTEGRITY_TABLES)?;
    validate_foreign_keys(connection, path)?;
    load_validated_entries(connection, path)?;
    Ok(())
}

/// Unknown newer generations must be classified before this binary assumes
/// any current base-table shape. Supported v1/v2 generations still require
/// complete base validation before they can be read or written.
fn validate_base_for_generation(
    connection: &Connection,
    path: &Path,
) -> Result<SchemaGeneration, AppError> {
    let generation = read_schema_generation(connection, path)?;
    if !matches!(generation, SchemaGeneration::Newer(_)) {
        validate_base_database(connection, path)?;
    }
    Ok(generation)
}

/// Validate the projection used by `library export` without relying on
/// non-portable operational metadata such as schema generation or revision.
fn load_recoverable_export_entries(
    connection: &Connection,
    path: &Path,
) -> Result<Vec<PortableLibraryEntry>, AppError> {
    enable_writable_schema(connection, path)?;
    validate_library_tags_schema(connection, path)?;
    validate_table_integrity(connection, path, &RECOVERABLE_EXPORT_INTEGRITY_TABLES)?;
    validate_foreign_keys(connection, path)?;
    let entries = load_validated_entries(connection, path)?;
    disable_writable_schema(connection, path)?;
    Ok(entries)
}

/// Derived validation proves the `library_fts` virtual table matches the
/// fixed creation statement and that its rows equal the deterministic
/// projection of the base rows. The FTS5 special `integrity-check` command
/// needs a writable connection, so ordinary reads stop at this content
/// comparison; doctor runs the special command on an in-memory copy.
fn validate_derived_database(connection: &Connection, path: &Path) -> Result<(), AppError> {
    let statement: Option<String> = connection
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'library_fts'",
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| database_error(path, error))?;
    if statement.as_deref().map(str::trim) != Some(LIBRARY_FTS_CREATE_SQL) {
        return Err(AppError::database_corrupt(NativePath::new(
            path.to_path_buf(),
        )));
    }
    let entries = load_validated_entries(connection, path)?;
    let mut rows = connection
        .prepare(
            "SELECT canonical_source, name, description, alias, tags_display, tags_comparison, category, note, repository
             FROM library_fts",
        )
        .map_err(|error| database_error(path, error))?;
    let projected = rows
        .query_map([], |row| {
            Ok([
                row.get::<_, String>(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
                row.get(7)?,
                row.get(8)?,
            ])
        })
        .map_err(|error| database_error(path, error))?;
    let mut by_source: HashMap<String, [String; FTS_ROW_COLUMNS]> = HashMap::new();
    for row in projected {
        let values = row.map_err(|error| database_error(path, error))?;
        if by_source.insert(values[0].clone(), values).is_some() {
            return Err(AppError::database_corrupt(NativePath::new(
                path.to_path_buf(),
            )));
        }
    }
    if by_source.len() != entries.len() {
        return Err(AppError::database_corrupt(NativePath::new(
            path.to_path_buf(),
        )));
    }
    for entry in &entries {
        let expected = fts_row_values(entry)
            .map_err(|_| AppError::database_corrupt(NativePath::new(path.to_path_buf())))?;
        match by_source.get(&entry.skill.source.canonical) {
            Some(actual) if actual == &expected => {}
            _ => {
                return Err(AppError::database_corrupt(NativePath::new(
                    path.to_path_buf(),
                )));
            }
        }
    }
    Ok(())
}

/// Read-path validation proves the complete base state, but tolerates a
/// malformed derived FTS schema so list/get can retain their base-only
/// contract and search can return the typed derived-index error.
fn validate_for_read(connection: &Connection, path: &Path) -> Result<SchemaGeneration, AppError> {
    enable_writable_schema(connection, path)?;
    let generation = validate_base_for_generation(connection, path)?;
    disable_writable_schema(connection, path)?;
    Ok(generation)
}

/// FTS repair keeps writable-schema tolerance enabled through its later
/// schema-row detach, which is the only path that may edit sqlite_master.
fn validate_base_for_fts_recovery(
    connection: &Connection,
    path: &Path,
) -> Result<SchemaGeneration, AppError> {
    enable_writable_schema(connection, path)?;
    validate_base_for_generation(connection, path)
}

fn fts_index_invalid() -> AppError {
    AppError::invalid_state(
        "library_database",
        "library_fts_invalid",
        ["a derived full-text index consistent with the base rows"],
    )
}

fn map_derived_validation_error(error: AppError) -> AppError {
    match error {
        AppError::DatabaseCorrupt { .. } => fts_index_invalid(),
        error => error,
    }
}

fn validate_fts_integrity(connection: &Connection, path: &Path) -> Result<(), AppError> {
    connection
        .execute(
            "INSERT INTO library_fts(library_fts) VALUES('integrity-check')",
            [],
        )
        .map(|_| ())
        .map_err(|error| map_derived_validation_error(database_error(path, error)))
}

fn fts_match_error(path: &Path, error: SqlError) -> AppError {
    match &error {
        SqlError::SqliteFailure(code, _)
            if matches!(
                code.code,
                ErrorCode::DatabaseCorrupt | ErrorCode::NotADatabase
            ) =>
        {
            fts_index_invalid()
        }
        _ => database_error(path, error),
    }
}

/// Write-path validation: base rows must be intact and the database must
/// already be schema v2 with a consistent derived index. A v1 database
/// reports `migration_required` and an unknown newer schema reports
/// `schema_newer`; neither is ever upgraded implicitly. Derived drift is a
/// typed, doctor-fixable `invalid_state`, never base corruption.
fn validate_database(connection: &Connection, path: &Path) -> Result<(), AppError> {
    enable_writable_schema(connection, path)?;
    let result = (|| {
        let generation = validate_base_for_generation(connection, path)?;
        match generation {
            SchemaGeneration::V1 => Err(AppError::MigrationRequired {
                domain: "library".to_owned(),
                found_version: 1,
                supported_version: SCHEMA_VERSION,
            }),
            SchemaGeneration::Newer(found) => Err(schema_newer(found)),
            SchemaGeneration::V2 => {
                validate_derived_database(connection, path)
                    .map_err(map_derived_validation_error)?;
                validate_fts_integrity(connection, path)
            }
        }
    })();
    if result.is_ok() {
        disable_writable_schema(connection, path)?;
    }
    result
}

fn singleton_i64(connection: &Connection, query: &str, path: &Path) -> Result<i64, AppError> {
    let mut statement = connection
        .prepare(query)
        .map_err(|error| database_error(path, error))?;
    let mut rows = statement
        .query([])
        .map_err(|error| database_error(path, error))?;
    let row = rows
        .next()
        .map_err(|error| database_error(path, error))?
        .ok_or_else(|| AppError::database_corrupt(NativePath::new(path.to_path_buf())))?;
    let value = row.get(0).map_err(|error| database_error(path, error))?;
    if rows
        .next()
        .map_err(|error| database_error(path, error))?
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
        .map_err(|error| database_error(path, error))?;
    let mut statement = connection
        .prepare("PRAGMA foreign_key_list(library_tags)")
        .map_err(|error| database_error(path, error))?;
    let relations = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(6)?,
            ))
        })
        .map_err(|error| database_error(path, error))?;
    let mut relation_count = 0;
    for relation in relations {
        let (table, from, to, on_delete) = relation.map_err(|error| database_error(path, error))?;
        if table != "library_entries"
            || from != "canonical_source"
            || to != "canonical_source"
            || on_delete != "CASCADE"
        {
            return Err(AppError::database_corrupt(NativePath::new(
                path.to_path_buf(),
            )));
        }
        relation_count += 1;
    }
    if relation_count != 1 {
        return Err(AppError::database_corrupt(NativePath::new(
            path.to_path_buf(),
        )));
    }
    Ok(())
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
        let mut portable = stored_to_entry(entry, path)?;
        portable.tags = load_tags(connection, path, &portable.skill.source.canonical)?;
        loaded.push(portable);
    }
    Ok(loaded)
}

fn stored_to_entry(entry: StoredEntry, path: &Path) -> Result<PortableLibraryEntry, AppError> {
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
    Ok(PortableLibraryEntry {
        skill,
        alias: entry.alias,
        category: entry.category,
        tags: Vec::new(),
        note: entry.note,
    })
}

fn load_validated_entries(
    connection: &Connection,
    path: &Path,
) -> Result<Vec<PortableLibraryEntry>, AppError> {
    PortableLibraryDocument {
        format_version: LIBRARY_FORMAT_VERSION,
        entries: load_entries(connection, path)?,
    }
    .validate()
    .map(|document| document.entries)
    .map_err(|_| AppError::database_corrupt(NativePath::new(path.to_path_buf())))
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
        let (comparison_key, display) = value.map_err(|error| database_error(path, error))?;
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

/// Preserve original free-text bytes in the derived index while adding the
/// NFC form that `LibrarySearchQuery` uses for literal query alternatives.
fn fts_free_text_projection(value: &str) -> String {
    if is_nfc(value) {
        return value.to_owned();
    }

    let mut projection = value.to_owned();
    projection.push('\n');
    projection.extend(value.nfc());
    projection
}

/// Deterministic full-text projection shared by import, metadata mutation,
/// migration, and doctor rebuild: one row per canonical source with tags
/// aggregated in comparison-key order using ASCII newline separators.
fn fts_row_values(entry: &PortableLibraryEntry) -> Result<[String; FTS_ROW_COLUMNS], AppError> {
    let mut tags = Vec::with_capacity(entry.tags.len());
    for tag in &entry.tags {
        let normalized = normalize_tag(tag)?;
        tags.push((normalized.comparison_key, normalized.display));
    }
    tags.sort_by(|left, right| left.0.cmp(&right.0));
    let tags_display = tags
        .iter()
        .map(|(_, display)| display.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let tags_comparison = tags
        .iter()
        .map(|(comparison_key, _)| comparison_key.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    Ok([
        entry.skill.source.canonical.clone(),
        fts_free_text_projection(&entry.skill.name),
        fts_free_text_projection(&entry.skill.description),
        entry
            .alias
            .as_deref()
            .map(fts_free_text_projection)
            .unwrap_or_default(),
        tags_display,
        tags_comparison,
        entry
            .category
            .as_deref()
            .map(fts_free_text_projection)
            .unwrap_or_default(),
        entry
            .note
            .as_deref()
            .map(fts_free_text_projection)
            .unwrap_or_default(),
        fts_free_text_projection(&entry.skill.source.repository_display),
    ])
}

fn insert_fts_row(
    connection: &Connection,
    entry: &PortableLibraryEntry,
    path: &Path,
) -> Result<(), AppError> {
    let values = fts_row_values(entry)?;
    connection
        .execute(
            "INSERT INTO library_fts (canonical_source, name, description, alias, tags_display, tags_comparison, category, note, repository)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                values[0], values[1], values[2], values[3], values[4], values[5], values[6],
                values[7], values[8]
            ],
        )
        .map_err(|error| database_error(path, error))?;
    Ok(())
}

fn replace_fts_row(
    connection: &Connection,
    entry: &PortableLibraryEntry,
    path: &Path,
) -> Result<(), AppError> {
    connection
        .execute(
            "DELETE FROM library_fts WHERE canonical_source = ?1",
            params![entry.skill.source.canonical],
        )
        .map_err(|error| database_error(path, error))?;
    insert_fts_row(connection, entry, path)
}

/// True when rebuilding must detach current FTS schema rows and compact the
/// database before recreating the derived index. This includes missing,
/// damaged, partial, and orphaned FTS schema so a previous interrupted
/// detach cannot leave unreachable pages behind.
fn fts_schema_requires_detach(connection: &Connection) -> bool {
    let statement: Option<String> = match connection
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'library_fts'",
            [],
            |row| row.get(0),
        )
        .optional()
    {
        Ok(statement) => statement,
        Err(_) => return true,
    };
    if statement.as_deref().map(str::trim) != Some(LIBRARY_FTS_CREATE_SQL) {
        return true;
    }
    for shadow in LIBRARY_FTS_SHADOW_TABLES {
        let present = matches!(
            connection.query_row(
                "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
                params![shadow],
                |row| row.get::<_, i64>(0),
            ),
            Ok(1)
        );
        if !present {
            return true;
        }
        let integrity =
            connection.query_row(&format!("PRAGMA integrity_check('{shadow}')"), [], |row| {
                row.get::<_, String>(0)
            });
        if integrity.as_deref() != Ok("ok") {
            return true;
        }
    }
    false
}

/// Physically damaged FTS5 shadow b-trees cannot be dropped or cleared
/// through SQL: any traversal fails with `SQLITE_CORRUPT`. Remove the six
/// schema rows directly instead — the `writable_schema` mechanism SQLite
/// documents for schema-level recovery — then commit and compact before
/// recreating the derived index. Base rows remain untouched.
fn detach_damaged_fts_schema(connection: &Connection, path: &Path) -> Result<(), AppError> {
    connection
        .execute_batch("PRAGMA writable_schema = ON;")
        .map_err(|error| database_error(path, error))?;
    let removal = connection.execute(
        "DELETE FROM sqlite_master WHERE name IN ('library_fts', 'library_fts_data', \
         'library_fts_idx', 'library_fts_content', 'library_fts_docsize', 'library_fts_config')",
        [],
    );
    let reset = connection.execute_batch("PRAGMA writable_schema = RESET;");
    removal.map_err(|error| database_error(path, error))?;
    reset.map_err(|error| database_error(path, error))?;
    let schema_version: i64 = connection
        .query_row("PRAGMA schema_version", [], |row| row.get(0))
        .map_err(|error| database_error(path, error))?;
    connection
        .pragma_update(None, "schema_version", schema_version + 1)
        .map_err(|error| database_error(path, error))?;
    Ok(())
}

fn reclaim_detached_fts_pages(connection: &Connection, path: &Path) -> Result<(), AppError> {
    connection
        .execute_batch("VACUUM;")
        .map_err(|error| database_error(path, error))
}

/// Drop and recreate a prepared derived index from verified base rows. Base
/// rows are never rewritten and `state_revision` never advances.
fn rebuild_derived_index(connection: &Connection, path: &Path) -> Result<(), AppError> {
    disable_writable_schema(connection, path)?;
    connection
        .execute("DROP TABLE IF EXISTS library_fts", [])
        .map_err(|error| database_error(path, error))?;
    connection
        .execute_batch(LIBRARY_FTS_CREATE_SQL)
        .map_err(|error| database_error(path, error))?;
    let entries = load_validated_entries(connection, path)?;
    for entry in &entries {
        insert_fts_row(connection, entry, path)?;
    }
    Ok(())
}
/// Encode a validated query as a fully quoted FTS5 expression: each term's
/// raw/folded alternatives form one parenthesised OR group and distinct
/// groups are joined with explicit `AND`. The user string is never mixed
/// into FTS grammar; every literal is a quoted FTS5 string.
fn fts_match_expression(query: &LibrarySearchQuery) -> String {
    let mut expression = String::new();
    for (index, term) in query.terms().iter().enumerate() {
        if index > 0 {
            expression.push_str(" AND ");
        }
        let alternatives = term.alternatives();
        if alternatives.len() == 1 {
            expression.push_str(&quote_fts_string(&alternatives[0]));
        } else {
            expression.push('(');
            for (alternative_index, alternative) in alternatives.iter().enumerate() {
                if alternative_index > 0 {
                    expression.push_str(" OR ");
                }
                expression.push_str(&quote_fts_string(alternative));
            }
            expression.push(')');
        }
    }
    expression
}

fn quote_fts_string(value: &str) -> String {
    let mut quoted = String::with_capacity(value.len() + 2);
    quoted.push('"');
    for character in value.chars() {
        if character == '"' {
            quoted.push_str("\"\"");
        } else {
            quoted.push(character);
        }
    }
    quoted.push('"');
    quoted
}

enum ReadFilter<'a> {
    All,
    FtsMatch(&'a str),
}

fn count_entries(connection: &Connection, path: &Path) -> Result<u64, AppError> {
    let count: i64 = connection
        .query_row("SELECT count(*) FROM library_entries", [], |row| row.get(0))
        .map_err(|error| database_error(path, error))?;
    u64::try_from(count)
        .map_err(|_| AppError::database_corrupt(NativePath::new(path.to_path_buf())))
}

fn count_fts_matches(
    connection: &Connection,
    path: &Path,
    expression: &str,
) -> Result<u64, AppError> {
    let count: i64 = connection
        .query_row(
            "SELECT count(*) FROM library_fts WHERE library_fts MATCH ?1",
            params![expression],
            |row| row.get(0),
        )
        .map_err(|error| fts_match_error(path, error))?;
    u64::try_from(count)
        .map_err(|_| AppError::database_corrupt(NativePath::new(path.to_path_buf())))
}

struct JoinedRow {
    stored: StoredEntry,
    tag_comparison_key: Option<String>,
    tag_display: Option<String>,
}

const JOINED_ENTRY_COLUMNS: &str = "e.canonical_source, e.owner, e.repository, e.repository_display, e.skill_path, e.ref_kind, e.ref_value,
       e.repository_id, e.commit_sha, e.integrity, e.name, e.description, e.entry_count, e.byte_count, e.alias, e.category, e.note,
       t.comparison_key, t.display";

fn joined_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<JoinedRow> {
    Ok(JoinedRow {
        stored: StoredEntry {
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
        },
        tag_comparison_key: row.get(17)?,
        tag_display: row.get(18)?,
    })
}

fn collect_joined_rows(
    rows: impl Iterator<Item = rusqlite::Result<JoinedRow>>,
    path: &Path,
    fts_corruption_is_derived: bool,
) -> Result<Vec<PortableLibraryEntry>, AppError> {
    let mut entries: Vec<PortableLibraryEntry> = Vec::new();
    for row in rows {
        let row = row.map_err(|error| {
            if fts_corruption_is_derived {
                fts_match_error(path, error)
            } else {
                database_error(path, error)
            }
        })?;
        let tag = match (row.tag_comparison_key, row.tag_display) {
            (Some(comparison_key), Some(display)) => {
                let normalized = normalize_tag(&display)
                    .map_err(|_| AppError::database_corrupt(NativePath::new(path.to_path_buf())))?;
                if normalized.display != display || normalized.comparison_key != comparison_key {
                    return Err(AppError::database_corrupt(NativePath::new(
                        path.to_path_buf(),
                    )));
                }
                Some(display)
            }
            (None, None) => None,
            _ => {
                return Err(AppError::database_corrupt(NativePath::new(
                    path.to_path_buf(),
                )));
            }
        };
        let canonical = row.stored.canonical.clone();
        if entries
            .last()
            .is_some_and(|last| last.skill.source.canonical == canonical)
        {
            if let Some(tag) = tag {
                entries.last_mut().expect("checked last").tags.push(tag);
            }
            continue;
        }
        let mut portable = stored_to_entry(row.stored, path)?;
        if let Some(tag) = tag {
            portable.tags.push(tag);
        }
        entries.push(portable);
    }
    Ok(entries)
}

/// Fetch one canonical page in source order with a single LEFT JOIN over
/// tags. Callers must already have checked `offset < total`, so the u64
/// offset converts losslessly into SQLite's signed range.
fn query_page(
    connection: &Connection,
    path: &Path,
    filter: &ReadFilter<'_>,
    page: &LibraryPage,
) -> Result<Vec<LibraryEntry>, AppError> {
    let offset = i64::try_from(page.offset()).map_err(|_| AppError::Internal {
        incident_id: "library_page_offset_beyond_total".to_owned(),
    })?;
    let sql = match filter {
        ReadFilter::All => format!(
            "WITH page_sources AS (
                SELECT canonical_source FROM library_entries ORDER BY canonical_source LIMIT ?1 OFFSET ?2
            )
            SELECT {JOINED_ENTRY_COLUMNS}
            FROM page_sources p
            JOIN library_entries e ON e.canonical_source = p.canonical_source
            LEFT JOIN library_tags t ON t.canonical_source = p.canonical_source
            ORDER BY e.canonical_source, t.comparison_key"
        ),
        ReadFilter::FtsMatch(_) => format!(
            "WITH matched AS (
                SELECT canonical_source FROM library_fts WHERE library_fts MATCH ?1
            ),
            page_sources AS (
                SELECT canonical_source FROM matched ORDER BY canonical_source LIMIT ?2 OFFSET ?3
            )
            SELECT {JOINED_ENTRY_COLUMNS}
            FROM page_sources p
            JOIN library_entries e ON e.canonical_source = p.canonical_source
            LEFT JOIN library_tags t ON t.canonical_source = p.canonical_source
            ORDER BY e.canonical_source, t.comparison_key"
        ),
    };
    let fts_corruption_is_derived = matches!(filter, ReadFilter::FtsMatch(_));
    let mut statement = connection.prepare(&sql).map_err(|error| {
        if fts_corruption_is_derived {
            fts_match_error(path, error)
        } else {
            database_error(path, error)
        }
    })?;
    let rows = match filter {
        ReadFilter::All => statement
            .query_map(params![i64::from(page.limit()), offset], joined_row)
            .map_err(|error| database_error(path, error))?,
        ReadFilter::FtsMatch(expression) => statement
            .query_map(
                params![expression, i64::from(page.limit()), offset],
                joined_row,
            )
            .map_err(|error| fts_match_error(path, error))?,
    };
    Ok(collect_joined_rows(rows, path, fts_corruption_is_derived)?
        .into_iter()
        .map(|entry| LibraryEntry::from_portable(entry, LibraryTrustState::Missing))
        .collect())
}

fn query_entry(
    connection: &Connection,
    path: &Path,
    selector: &str,
) -> Result<Option<LibraryEntry>, AppError> {
    let sql = format!(
        "SELECT {JOINED_ENTRY_COLUMNS}
         FROM library_entries e
         LEFT JOIN library_tags t ON t.canonical_source = e.canonical_source
         WHERE e.canonical_source = ?1
         ORDER BY t.comparison_key"
    );
    let mut statement = connection
        .prepare(&sql)
        .map_err(|error| database_error(path, error))?;
    let rows = statement
        .query_map(params![selector], joined_row)
        .map_err(|error| database_error(path, error))?;
    let entries = collect_joined_rows(rows, path, false)?;
    Ok(entries
        .into_iter()
        .next()
        .map(|entry| LibraryEntry::from_portable(entry, LibraryTrustState::Missing)))
}

fn apply_additions(
    connection: &Connection,
    additions: &[PortableLibraryEntry],
    path: &Path,
) -> Result<(), AppError> {
    advance_state_revision(connection, path)?;
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
        insert_fts_row(connection, entry, path)?;
    }
    Ok(())
}

fn apply_metadata_change(
    connection: &Connection,
    mutation: &LibraryMetadataMutation,
    source: &SourceIdentity,
    updated: &PortableLibraryEntry,
    path: &Path,
) -> Result<(), AppError> {
    let changed_rows = match &mutation.change {
        LibraryMetadataChange::AliasSet(value) => connection.execute(
            "UPDATE library_entries SET alias = ?1 WHERE canonical_source = ?2",
            params![value, source.canonical],
        ),
        LibraryMetadataChange::AliasClear => connection.execute(
            "UPDATE library_entries SET alias = NULL WHERE canonical_source = ?1",
            params![source.canonical],
        ),
        LibraryMetadataChange::CategorySet(value) => connection.execute(
            "UPDATE library_entries SET category = ?1 WHERE canonical_source = ?2",
            params![value, source.canonical],
        ),
        LibraryMetadataChange::CategoryClear => connection.execute(
            "UPDATE library_entries SET category = NULL WHERE canonical_source = ?1",
            params![source.canonical],
        ),
        LibraryMetadataChange::TagAdd(tag) => connection.execute(
            "INSERT INTO library_tags (canonical_source, comparison_key, display) VALUES (?1, ?2, ?3)",
            params![source.canonical, tag.comparison_key, tag.display],
        ),
        LibraryMetadataChange::TagRemove(tag) => connection.execute(
            "DELETE FROM library_tags WHERE canonical_source = ?1 AND comparison_key = ?2",
            params![source.canonical, tag.comparison_key],
        ),
        LibraryMetadataChange::NoteSet(value) => connection.execute(
            "UPDATE library_entries SET note = ?1 WHERE canonical_source = ?2",
            params![value, source.canonical],
        ),
        LibraryMetadataChange::NoteClear => connection.execute(
            "UPDATE library_entries SET note = NULL WHERE canonical_source = ?1",
            params![source.canonical],
        ),
    }
    .map_err(|error| database_error(path, error))?;
    if changed_rows != 1 {
        return Err(AppError::invalid_state(
            "library_database",
            "metadata_mutation_affected_unexpected_rows",
            ["exactly one durable Library metadata record"],
        ));
    }
    replace_fts_row(connection, updated, path)?;
    Ok(())
}

fn advance_state_revision(connection: &Connection, path: &Path) -> Result<(), AppError> {
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

fn configure_connection(connection: &Connection, path: &Path) -> Result<(), AppError> {
    connection
        .busy_timeout(LOCK_WAIT)
        .map_err(|error| database_error(path, error))
}

#[allow(unsafe_code)]
fn verify_sqlite_connection_identity(connection: &Connection) -> Result<(), AppError> {
    let mut moved: libc::c_int = 0;
    // SAFETY: `connection` owns a live SQLite handle; `main` is NUL-terminated,
    // and SQLite reads `moved` only for this synchronous file-control call.
    let status = unsafe {
        ffi::sqlite3_file_control(
            connection.handle(),
            c"main".as_ptr(),
            ffi::SQLITE_FCNTL_HAS_MOVED,
            (&mut moved as *mut libc::c_int).cast(),
        )
    };
    if status == ffi::SQLITE_OK && moved == 0 {
        Ok(())
    } else {
        Err(SqliteLibraryRepository::database_identity_drift())
    }
}

fn database_error(path: &Path, error: SqlError) -> AppError {
    match &error {
        SqlError::SqliteFailure(code, _)
            if matches!(
                code.code,
                ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked
            ) =>
        {
            AppError::Busy {
                lock_domain: "database".to_owned(),
                waited_ms: LOCK_WAIT.as_millis() as u64,
            }
        }
        SqlError::SqliteFailure(code, _)
            if matches!(
                code.code,
                ErrorCode::DatabaseCorrupt | ErrorCode::NotADatabase
            ) =>
        {
            AppError::database_corrupt(NativePath::new(path.to_path_buf()))
        }
        SqlError::SqliteFailure(_, Some(message))
            if message.contains("no such table")
                || message.contains("no such column")
                || message.contains("foreign key mismatch") =>
        {
            AppError::database_corrupt(NativePath::new(path.to_path_buf()))
        }
        SqlError::SqlInputError { msg, .. }
            if msg.contains("no such table")
                || msg.contains("no such column")
                || msg.contains("foreign key mismatch") =>
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
    AppError::invalid_state_at_path(
        "library_database",
        format!("{action}: {error}"),
        NativePath::new(path.to_path_buf()),
        ["a durable Library database generation"],
    )
}

fn sync_existing_database(
    database: &Path,
    directory: &ValidatedDataDirectory,
    database_name: &std::ffi::OsStr,
    identity: (u64, u64),
    after_database_sync: impl FnOnce() -> Result<(), AppError>,
) -> Result<(), AppError> {
    revalidate_database_entry(directory, database_name, identity)?;
    let file = File::from(
        openat(
            &directory.handle,
            database_name,
            OFlags::RDWR | OFlags::NOFOLLOW,
            Mode::empty(),
        )
        .map_err(|_| SqliteLibraryRepository::database_identity_drift())?,
    );
    let opened_identity = metadata_identity(
        &file
            .metadata()
            .map_err(|error| database_sync_error(database, "inspect committed database", error))?,
    );
    if opened_identity != identity {
        return Err(SqliteLibraryRepository::database_identity_drift());
    }
    revalidate_database_entry(directory, database_name, identity)?;
    file.sync_all()
        .map_err(|error| database_sync_error(database, "sync committed database", error))?;
    after_database_sync()?;
    revalidate_database_entry(directory, database_name, identity)?;
    directory
        .handle
        .sync_all()
        .map_err(|error| database_sync_error(database, "sync database directory", error))?;
    revalidate_database_entry(directory, database_name, identity)
}

fn revalidate_database_entry(
    directory: &ValidatedDataDirectory,
    database_name: &std::ffi::OsStr,
    identity: (u64, u64),
) -> Result<(), AppError> {
    directory.revalidate()?;
    let entry = statat(&directory.handle, database_name, AtFlags::SYMLINK_NOFOLLOW)
        .map_err(|_| SqliteLibraryRepository::database_identity_drift())?;
    if stat_identity(entry.st_dev, entry.st_ino) != Some(identity) {
        return Err(SqliteLibraryRepository::database_identity_drift());
    }
    Ok(())
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
    use crate::domain::library::LibraryChangedField;
    use crate::domain::source::RefKind;
    use crate::ports::configuration::Environment;
    use std::collections::HashMap;
    use std::ffi::OsString;
    use std::os::unix::{
        ffi::{OsStrExt, OsStringExt},
        fs::{FileTypeExt, symlink},
    };
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

    struct ReplaceDataRootAfterResolve {
        data_directory: PathBuf,
        displaced_directory: PathBuf,
        replaced: std::sync::Mutex<bool>,
    }

    impl StateRootResolver for ReplaceDataRootAfterResolve {
        fn resolve(&self, environment: &dyn Environment) -> Result<ResolvedRoots, AppError> {
            let roots = XdgRootResolver.resolve(environment)?;
            let mut replaced = self.replaced.lock().unwrap();
            if !*replaced {
                fs::rename(&self.data_directory, &self.displaced_directory).unwrap();
                fs::create_dir(&self.data_directory).unwrap();
                *replaced = true;
            }
            Ok(roots)
        }

        fn revalidate(&self, roots: &ResolvedRoots) -> Result<ResolvedRoots, AppError> {
            XdgRootResolver.revalidate(roots)
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

    fn metadata_mutation(selector: &str, change: LibraryMetadataChange) -> LibraryMetadataMutation {
        LibraryMetadataMutation {
            selector: selector.to_owned(),
            change,
        }
    }

    fn state_revision(database: &Path) -> i64 {
        Connection::open(database)
            .unwrap()
            .query_row("SELECT revision FROM state_revision", [], |row| row.get(0))
            .unwrap()
    }

    #[test]
    fn first_import_sync_attributes_state_directory_failure_to_state_root() {
        let temporary = tempdir().unwrap();
        let state_root = temporary.path().join("state");
        let state_locks = state_root.join("skilload/locks");
        let data_directory = temporary.path().join("data/skilload");
        fs::create_dir_all(&state_locks).unwrap();
        fs::create_dir_all(&data_directory).unwrap();

        let capture = |path: &Path| CreatedDirectory {
            path: path.to_path_buf(),
        };
        let created_directories = FirstImportDirectories {
            created_directories: vec![
                FirstImportCreatedDirectory {
                    directory: capture(&state_locks),
                    variable: "XDG_STATE_HOME",
                },
                FirstImportCreatedDirectory {
                    directory: capture(&data_directory),
                    variable: "XDG_DATA_HOME",
                },
            ],
        };
        fs::rename(&state_root, temporary.path().join("state-displaced")).unwrap();

        let error = created_directories.sync_created_directories().unwrap_err();

        assert!(matches!(
            error,
            AppError::InvalidEnvironment { variable, .. } if variable == "XDG_STATE_HOME"
        ));
    }

    #[test]
    fn dry_run_is_inert_and_first_import_round_trips() {
        let temporary = tempdir().unwrap();
        let environment = Arc::new(TestEnvironment::with_roots(temporary.path()));
        let repository =
            SqliteLibraryRepository::with_environment(environment, Arc::new(XdgRootResolver));
        let import = document(vec![entry("skills/review", Some("review"))]);
        let dry_run = repository.import(&import, true).unwrap();
        assert_eq!(dry_run.outcome, LibraryImportOutcome::Observed);
        assert_eq!(dry_run.data.added.len(), 1);
        assert!(!temporary.path().join("data/skilload").exists());
        assert!(!temporary.path().join("state/skilload").exists());

        let committed = repository.import(&import, false).unwrap();
        assert_eq!(committed.outcome, LibraryImportOutcome::Changed);
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
        assert_eq!(repeated.outcome, LibraryImportOutcome::Unchanged);
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
    fn complete_import_plan_rejects_more_entries_than_portable_transfer_allows() {
        let existing = (0..crate::domain::library::MAX_PORTABLE_LIBRARY_ENTRIES)
            .map(|index| entry(&format!("skills/{index}/review"), None))
            .collect::<Vec<_>>();

        let error = match SqliteLibraryRepository::plan(
            &document(vec![entry("skills/overflow", None)]),
            &existing,
            false,
        ) {
            Err(error) => error,
            Ok(_) => panic!("expected complete portable entry ceiling to reject import plan"),
        };

        assert!(matches!(
            error,
            AppError::Validation { constraint, .. }
                if constraint == "library_portable_document_entries"
        ));
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

    #[test]
    fn orphaned_database_sidecars_are_not_an_empty_library() {
        for suffix in DATABASE_SIDECAR_SUFFIXES {
            let temporary = tempdir().unwrap();
            let data = temporary.path().join("data/skilload");
            fs::create_dir_all(&data).unwrap();
            let database = data.join("skilload.db");
            let sidecar = data.join(format!("skilload.db{suffix}"));
            fs::write(&sidecar, b"orphaned SQLite sidecar").unwrap();
            let repository = SqliteLibraryRepository::with_environment(
                Arc::new(TestEnvironment::with_roots(temporary.path())),
                Arc::new(XdgRootResolver),
            );
            let import = document(vec![entry("skills/review", None)]);

            assert_eq!(repository.export().unwrap_err().code(), "database_corrupt");
            assert_eq!(
                repository.import(&import, true).unwrap_err().code(),
                "database_corrupt"
            );
            assert_eq!(
                repository.import(&import, false).unwrap_err().code(),
                "database_corrupt"
            );
            assert!(!database.exists());
            assert_eq!(fs::read(&sidecar).unwrap(), b"orphaned SQLite sidecar");
        }
    }

    struct BeforeCommitFailure;

    impl PersistenceHooks for BeforeCommitFailure {
        fn before_commit(&self, _staging: &Path) -> Result<(), AppError> {
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
    fn first_import_precommit_failure_retains_unproven_state_and_data_directories() {
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
        let data_directory = temporary.path().join("data/skilload");
        assert!(data_directory.is_dir());
        assert!(!data_directory.join("skilload.db").exists());
        let lock = temporary.path().join("state/skilload/locks/database.lock");
        assert!(lock.is_file());
        let lock_identity = metadata_identity(&fs::metadata(&lock).unwrap());

        let repository = SqliteLibraryRepository::with_environment(
            Arc::new(TestEnvironment::with_roots(temporary.path())),
            Arc::new(XdgRootResolver),
        );
        repository
            .import(&document(vec![entry("skills/review", None)]), false)
            .unwrap();
        assert_eq!(
            metadata_identity(&fs::metadata(lock).unwrap()),
            lock_identity
        );
    }

    struct SidecarBeforeCommitFailure;

    impl PersistenceHooks for SidecarBeforeCommitFailure {
        fn before_commit(&self, staging: &Path) -> Result<(), AppError> {
            fs::write(
                PathBuf::from(format!("{}-shm", staging.display())),
                b"staging sidecar",
            )
            .unwrap();
            Err(AppError::Internal {
                incident_id: "before-first-library-commit-with-sidecar".to_owned(),
            })
        }
    }

    #[test]
    fn first_import_precommit_failure_preserves_foreign_staging_sidecar() {
        let temporary = tempdir().unwrap();
        let repository = SqliteLibraryRepository::with_hooks(
            Arc::new(TestEnvironment::with_roots(temporary.path())),
            Arc::new(XdgRootResolver),
            Arc::new(SidecarBeforeCommitFailure),
        );

        let error = repository
            .import(&document(vec![entry("skills/review", None)]), false)
            .unwrap_err();

        assert_eq!(error.code(), "internal_invariant");
        let data_directory = temporary.path().join("data/skilload");
        let sidecars = fs::read_dir(&data_directory)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| {
                path.file_name()
                    .is_some_and(|name| name.to_string_lossy().ends_with("-shm"))
            })
            .collect::<Vec<_>>();
        assert_eq!(sidecars.len(), 1);
        assert_eq!(fs::read(&sidecars[0]).unwrap(), b"staging sidecar");
        assert!(
            temporary
                .path()
                .join("state/skilload/locks/database.lock")
                .is_file()
        );
    }

    #[test]
    fn first_import_staging_preserves_unproven_sqlite_sidecars() {
        let temporary = tempdir().unwrap();
        let data_directory_path = temporary.path().join("data/skilload");
        fs::create_dir_all(&data_directory_path).unwrap();
        let data_directory = ValidatedDataDirectory::open(&data_directory_path).unwrap();
        let staging_file = Builder::new()
            .prefix(".skilload-library-db-")
            .suffix(".tmp")
            .tempfile_in(&data_directory_path)
            .unwrap();
        let staging = FirstImportStaging::new(staging_file, &data_directory).unwrap();
        let mut sidecar_name = staging.name.clone();
        sidecar_name.push("-journal");
        let sidecar = data_directory_path.join(&sidecar_name);
        fs::write(&sidecar, b"SQLite journal").unwrap();

        drop(staging);
        assert!(sidecar.exists());
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
    fn tags_schema_without_entry_foreign_key_is_database_corrupt() {
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
            .execute_batch(
                "
                PRAGMA foreign_keys = OFF;
                DROP TABLE library_tags;
                CREATE TABLE library_tags (
                    canonical_source TEXT NOT NULL,
                    comparison_key TEXT NOT NULL,
                    display TEXT NOT NULL,
                    PRIMARY KEY (canonical_source, comparison_key)
                );
                ",
            )
            .unwrap();

        assert_eq!(repository.export().unwrap_err().code(), "database_corrupt");
    }

    #[test]
    fn foreign_key_parent_key_mismatch_is_database_corrupt() {
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
            .execute_batch(
                "
                PRAGMA foreign_keys = OFF;
                DROP TABLE library_entries;
                CREATE TABLE library_entries (canonical_source TEXT NOT NULL);
                ",
            )
            .unwrap();

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

        assert_eq!(repository.inspect().unwrap_err().code(), "database_corrupt");
        assert_eq!(repository.export().unwrap().entries.len(), 1);
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
    fn first_import_lock_failure_retains_created_state_directories() {
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
        assert!(temporary.path().join("state/skilload/locks").is_dir());
        assert!(!temporary.path().join("data/skilload").exists());
    }

    struct AfterFirstLockFailure;

    impl PersistenceHooks for AfterFirstLockFailure {
        fn after_first_lock_acquired(&self) -> Result<(), AppError> {
            Err(AppError::Internal {
                incident_id: "after-first-library-lock".to_owned(),
            })
        }
    }

    #[test]
    fn first_import_post_lock_failure_retains_the_durable_lock() {
        let temporary = tempdir().unwrap();
        let repository = SqliteLibraryRepository::with_hooks(
            Arc::new(TestEnvironment::with_roots(temporary.path())),
            Arc::new(XdgRootResolver),
            Arc::new(AfterFirstLockFailure),
        );

        let error = repository
            .import(&document(vec![entry("skills/review", None)]), false)
            .unwrap_err();

        assert_eq!(error.code(), "internal_invariant");
        assert!(!temporary.path().join("data/skilload").exists());
        assert!(
            temporary
                .path()
                .join("state/skilload/locks/database.lock")
                .is_file()
        );
    }

    struct WinnerPublishesBeforeFirstLock {
        environment: Arc<dyn Environment>,
    }

    impl PersistenceHooks for WinnerPublishesBeforeFirstLock {
        fn before_first_lock(&self) -> Result<(), AppError> {
            let winner = SqliteLibraryRepository::with_environment(
                self.environment.clone(),
                Arc::new(XdgRootResolver),
            );
            winner.import(&document(vec![entry("skills/winner", None)]), false)?;
            Ok(())
        }
    }

    #[test]
    fn first_import_replans_after_a_concurrent_winner_publishes() {
        let temporary = tempdir().unwrap();
        let environment: Arc<dyn Environment> =
            Arc::new(TestEnvironment::with_roots(temporary.path()));
        let repository = SqliteLibraryRepository::with_hooks(
            environment.clone(),
            Arc::new(XdgRootResolver),
            Arc::new(WinnerPublishesBeforeFirstLock { environment }),
        );

        let operation = repository
            .import(&document(vec![entry("skills/loser", None)]), false)
            .unwrap();

        assert_eq!(operation.outcome, LibraryImportOutcome::Changed);
        let sources = repository
            .export()
            .unwrap()
            .entries
            .into_iter()
            .map(|entry| entry.skill.source.canonical)
            .collect::<Vec<_>>();
        assert_eq!(
            sources,
            vec![
                "github:owner/repository#skills/loser@refs/heads/main",
                "github:owner/repository#skills/winner@refs/heads/main",
            ]
        );
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
        assert!(!database.exists());
        assert_eq!(fs::read(replacement).unwrap(), b"replacement database");
    }

    struct FirstImportPublishedDatabaseIsCommittedStaging;

    impl PersistenceHooks for FirstImportPublishedDatabaseIsCommittedStaging {
        fn after_first_publication_link_before_finalize(
            &self,
            database: &Path,
        ) -> Result<(), AppError> {
            let bytes = fs::read(database).unwrap();
            assert!(bytes.starts_with(b"SQLite format 3\0"));
            Err(AppError::Internal {
                incident_id: "inspect-first-published-database".to_owned(),
            })
        }
    }

    #[test]
    fn first_import_publishes_a_committed_database_without_an_empty_guard() {
        let temporary = tempdir().unwrap();
        let database = temporary.path().join("data/skilload/skilload.db");
        let repository = SqliteLibraryRepository::with_hooks(
            Arc::new(TestEnvironment::with_roots(temporary.path())),
            Arc::new(XdgRootResolver),
            Arc::new(FirstImportPublishedDatabaseIsCommittedStaging),
        );

        let error = repository
            .import(&document(vec![entry("skills/review", None)]), false)
            .unwrap_err();

        assert_eq!(error.code(), "internal_invariant");
        assert!(!database.exists());
    }

    struct FirstImportPublishedDatabaseReplacement {
        replacement: PathBuf,
    }

    impl PersistenceHooks for FirstImportPublishedDatabaseReplacement {
        fn after_first_publication_link_before_finalize(
            &self,
            database: &Path,
        ) -> Result<(), AppError> {
            fs::remove_file(database).unwrap();
            fs::hard_link(&self.replacement, database).unwrap();
            Ok(())
        }
    }

    #[test]
    fn first_import_preserves_a_replaced_live_database_before_finalize() {
        let temporary = tempdir().unwrap();
        let replacement = temporary.path().join("replacement.db");
        let database = temporary.path().join("data/skilload/skilload.db");
        fs::write(&replacement, b"foreign database").unwrap();
        let repository = SqliteLibraryRepository::with_hooks(
            Arc::new(TestEnvironment::with_roots(temporary.path())),
            Arc::new(XdgRootResolver),
            Arc::new(FirstImportPublishedDatabaseReplacement {
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
        assert_eq!(fs::read(&database).unwrap(), b"foreign database");
        assert_eq!(fs::read(replacement).unwrap(), b"foreign database");
    }

    struct FirstImportPublishedDatabaseReplacementAfterIdentityCheck {
        replacement: PathBuf,
    }

    impl PersistenceHooks for FirstImportPublishedDatabaseReplacementAfterIdentityCheck {
        fn after_first_publication_identity_check_before_finalize(
            &self,
            database: &Path,
        ) -> Result<(), AppError> {
            fs::remove_file(database).unwrap();
            fs::hard_link(&self.replacement, database).unwrap();
            Ok(())
        }
    }

    #[test]
    fn first_import_preserves_a_replaced_live_database_after_final_identity_check() {
        let temporary = tempdir().unwrap();
        let replacement = temporary.path().join("replacement.db");
        let database = temporary.path().join("data/skilload/skilload.db");
        fs::write(&replacement, b"foreign database").unwrap();
        let repository = SqliteLibraryRepository::with_hooks(
            Arc::new(TestEnvironment::with_roots(temporary.path())),
            Arc::new(XdgRootResolver),
            Arc::new(FirstImportPublishedDatabaseReplacementAfterIdentityCheck {
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
        assert_eq!(fs::read(&database).unwrap(), b"foreign database");
        assert_eq!(fs::read(replacement).unwrap(), b"foreign database");
    }

    struct PublishedDatabaseReplacementBeforeSuccess {
        displaced: PathBuf,
        replacement: PathBuf,
    }

    impl PersistenceHooks for PublishedDatabaseReplacementBeforeSuccess {
        fn after_first_publish_sync_before_success(&self, database: &Path) -> Result<(), AppError> {
            fs::rename(database, &self.displaced).unwrap();
            fs::hard_link(&self.replacement, database).unwrap();
            Ok(())
        }
    }

    #[test]
    fn first_import_rejects_a_database_replaced_after_final_sync() {
        let temporary = tempdir().unwrap();
        let replacement = temporary.path().join("replacement.db");
        let displaced = temporary.path().join("displaced.db");
        let database = temporary.path().join("data/skilload/skilload.db");
        fs::write(&replacement, b"foreign database").unwrap();
        let repository = SqliteLibraryRepository::with_hooks(
            Arc::new(TestEnvironment::with_roots(temporary.path())),
            Arc::new(XdgRootResolver),
            Arc::new(PublishedDatabaseReplacementBeforeSuccess {
                displaced: displaced.clone(),
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
        assert_eq!(fs::read(&database).unwrap(), b"foreign database");
        assert!(displaced.exists());
        assert_eq!(fs::read(replacement).unwrap(), b"foreign database");
    }
    struct FirstImportStagingSymlinkBeforeOpen {
        replacement: PathBuf,
    }

    impl PersistenceHooks for FirstImportStagingSymlinkBeforeOpen {
        fn after_first_staging_identity_check_before_open(
            &self,
            staging: &Path,
        ) -> Result<(), AppError> {
            fs::remove_file(staging).unwrap();
            symlink(&self.replacement, staging).unwrap();
            Ok(())
        }
    }

    #[test]
    fn first_import_does_not_follow_a_staging_replacement_before_open() {
        let temporary = tempdir().unwrap();
        let replacement = temporary.path().join("foreign.db");
        let replacement_bytes = b"foreign database".to_vec();
        fs::write(&replacement, &replacement_bytes).unwrap();
        let repository = SqliteLibraryRepository::with_hooks(
            Arc::new(TestEnvironment::with_roots(temporary.path())),
            Arc::new(XdgRootResolver),
            Arc::new(FirstImportStagingSymlinkBeforeOpen {
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
        assert_eq!(fs::read(&replacement).unwrap(), replacement_bytes);
        let data_directory = temporary.path().join("data/skilload");
        assert!(!data_directory.join("skilload.db").exists());
        let staging = fs::read_dir(&data_directory)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .find(|path| {
                path.file_name()
                    .is_some_and(|name| name.to_string_lossy().starts_with(".skilload-library-db-"))
            })
            .unwrap();
        assert!(
            fs::symlink_metadata(staging)
                .unwrap()
                .file_type()
                .is_symlink()
        );
    }

    struct FirstImportStagingAbaReplacement {
        displaced: PathBuf,
        replacement: PathBuf,
    }

    impl PersistenceHooks for FirstImportStagingAbaReplacement {
        fn after_first_staging_identity_recheck_before_open(
            &self,
            staging: &Path,
        ) -> Result<(), AppError> {
            fs::rename(staging, &self.displaced).unwrap();
            fs::rename(&self.replacement, staging).unwrap();
            Ok(())
        }

        fn after_first_staging_connection_open(&self, staging: &Path) -> Result<(), AppError> {
            fs::rename(staging, &self.replacement).unwrap();
            fs::rename(&self.displaced, staging).unwrap();
            Ok(())
        }
    }

    #[test]
    fn first_import_rejects_an_aba_staging_open_before_sql() {
        let temporary = tempdir().unwrap();
        let replacement = temporary.path().join("foreign.db");
        File::create(&replacement).unwrap();
        let repository = SqliteLibraryRepository::with_hooks(
            Arc::new(TestEnvironment::with_roots(temporary.path())),
            Arc::new(XdgRootResolver),
            Arc::new(FirstImportStagingAbaReplacement {
                displaced: temporary.path().join("displaced-staging.db"),
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
        assert_eq!(fs::metadata(replacement).unwrap().len(), 0);
        assert!(!temporary.path().join("data/skilload/skilload.db").exists());
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

    struct ExistingReadOnlyAbaReplacement {
        database: PathBuf,
        displaced: PathBuf,
        replacement: PathBuf,
        replacement_displaced: PathBuf,
    }

    impl PersistenceHooks for ExistingReadOnlyAbaReplacement {
        fn before_existing_database_open(&self, _database: &Path) -> Result<(), AppError> {
            fs::rename(&self.database, &self.displaced).unwrap();
            fs::rename(&self.replacement, &self.database).unwrap();
            Ok(())
        }

        fn after_existing_database_open(&self, _database: &Path) -> Result<(), AppError> {
            fs::rename(&self.database, &self.replacement_displaced).unwrap();
            fs::rename(&self.displaced, &self.database).unwrap();
            Ok(())
        }
    }

    #[test]
    fn export_uses_checked_generation_when_a_read_only_aba_is_restored() {
        let temporary = tempdir().unwrap();
        let environment = Arc::new(TestEnvironment::with_roots(temporary.path()));
        let initial = SqliteLibraryRepository::with_environment(
            environment.clone(),
            Arc::new(XdgRootResolver),
        );
        let original = document(vec![entry("skills/review", None)]);
        initial.import(&original, false).unwrap();
        let database = temporary.path().join("data/skilload/skilload.db");
        let replacement = temporary.path().join("replacement.db");
        fs::copy(&database, &replacement).unwrap();
        assert_eq!(
            Connection::open(&replacement)
                .unwrap()
                .execute(
                    "UPDATE library_entries SET note = ?1",
                    ["replacement generation"],
                )
                .unwrap(),
            1
        );
        let repository = SqliteLibraryRepository::with_hooks(
            environment,
            Arc::new(XdgRootResolver),
            Arc::new(ExistingReadOnlyAbaReplacement {
                database: database.clone(),
                displaced: temporary.path().join("displaced.db"),
                replacement: replacement.clone(),
                replacement_displaced: temporary.path().join("replacement-displaced.db"),
            }),
        );

        let exported = repository.export().unwrap();

        assert_eq!(exported.entries, original.validate().unwrap().entries);
        assert!(database.exists());
        assert!(!replacement.exists());
    }

    struct ExistingWritableAbaReplacement {
        database: PathBuf,
        displaced: PathBuf,
        replacement: PathBuf,
        replacement_displaced: PathBuf,
    }

    impl PersistenceHooks for ExistingWritableAbaReplacement {
        fn before_existing_database_open(&self, _database: &Path) -> Result<(), AppError> {
            fs::rename(&self.database, &self.displaced).unwrap();
            fs::rename(&self.replacement, &self.database).unwrap();
            Ok(())
        }

        fn after_existing_database_connection_identity_check(
            &self,
            _database: &Path,
        ) -> Result<(), AppError> {
            fs::rename(&self.database, &self.replacement_displaced).unwrap();
            fs::rename(&self.displaced, &self.database).unwrap();
            Ok(())
        }
    }

    #[test]
    fn writable_open_rejects_an_aba_generation_restored_after_initial_handle_check() {
        let temporary = tempdir().unwrap();
        let environment = Arc::new(TestEnvironment::with_roots(temporary.path()));
        let initial = SqliteLibraryRepository::with_environment(
            environment.clone(),
            Arc::new(XdgRootResolver),
        );
        initial
            .import(&document(vec![entry("skills/review", None)]), false)
            .unwrap();
        let initial_roots = initial.resolve_roots().unwrap();
        let database = SqliteLibraryRepository::database_path(&initial_roots);
        let replacement = temporary.path().join("replacement.db");
        fs::copy(&database, &replacement).unwrap();
        let replacement_displaced = temporary.path().join("replacement-displaced.db");
        let repository = SqliteLibraryRepository::with_hooks(
            environment,
            Arc::new(XdgRootResolver),
            Arc::new(ExistingWritableAbaReplacement {
                database: database.clone(),
                displaced: temporary.path().join("displaced.db"),
                replacement,
                replacement_displaced: replacement_displaced.clone(),
            }),
        );
        let roots = repository.resolve_roots().unwrap();
        let data_directory = repository.open_bound_data_directory(&roots).unwrap();

        let error = repository
            .open_existing_database(
                &data_directory,
                &database,
                OpenFlags::SQLITE_OPEN_READ_WRITE,
            )
            .unwrap_err();

        assert!(
            matches!(
                error,
                AppError::InvalidState { ref state, .. } if state == "database_identity_drift"
            ),
            "expected database_identity_drift, got {error:?}"
        );
        assert_ne!(
            metadata_identity(&fs::metadata(&database).unwrap()),
            metadata_identity(&fs::metadata(&replacement_displaced).unwrap()),
            "the restored path and the displaced connection target must be distinct generations"
        );
    }

    struct ExistingReadOnlyWalReplacement {
        database: PathBuf,
        displaced: PathBuf,
        replacement: PathBuf,
    }

    impl PersistenceHooks for ExistingReadOnlyWalReplacement {
        fn before_existing_database_open(&self, _database: &Path) -> Result<(), AppError> {
            fs::rename(&self.database, &self.displaced).unwrap();
            fs::rename(&self.replacement, &self.database).unwrap();
            Ok(())
        }
    }

    #[test]
    fn read_only_open_never_creates_sidecars_for_a_replaced_wal_generation() {
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
        let replacement = temporary.path().join("wal-replacement.db");
        fs::copy(&database, &replacement).unwrap();
        Connection::open(&replacement)
            .unwrap()
            .execute_batch("PRAGMA journal_mode = WAL;")
            .unwrap();
        let repository = SqliteLibraryRepository::with_hooks(
            environment,
            Arc::new(XdgRootResolver),
            Arc::new(ExistingReadOnlyWalReplacement {
                database: database.clone(),
                displaced: temporary.path().join("displaced.db"),
                replacement,
            }),
        );

        let error = repository.export().unwrap_err();

        assert!(matches!(
            error,
            AppError::InvalidState { state, .. } if state == "database_identity_drift"
        ));
        assert!(!database.with_file_name("skilload.db-shm").exists());
        assert!(!database.with_file_name("skilload.db-wal").exists());
    }

    struct ExistingDataDirectoryReplacement {
        data_directory: PathBuf,
        displaced_directory: PathBuf,
        replacement_directory: PathBuf,
    }

    impl PersistenceHooks for ExistingDataDirectoryReplacement {
        fn before_existing_database_generation_open(
            &self,
            _database: &Path,
        ) -> Result<(), AppError> {
            fs::rename(&self.data_directory, &self.displaced_directory).unwrap();
            fs::rename(&self.replacement_directory, &self.data_directory).unwrap();
            Ok(())
        }
    }

    #[test]
    fn read_only_open_rejects_a_replaced_data_directory() {
        let temporary = tempdir().unwrap();
        let environment = Arc::new(TestEnvironment::with_roots(temporary.path()));
        let initial = SqliteLibraryRepository::with_environment(
            environment.clone(),
            Arc::new(XdgRootResolver),
        );
        initial
            .import(&document(vec![entry("skills/review", None)]), false)
            .unwrap();
        let data_directory = temporary.path().join("data/skilload");
        let database = data_directory.join("skilload.db");
        let replacement_directory = temporary.path().join("replacement-data-directory");
        fs::create_dir(&replacement_directory).unwrap();
        let replacement = replacement_directory.join("skilload.db");
        fs::copy(&database, &replacement).unwrap();
        Connection::open(&replacement)
            .unwrap()
            .execute(
                "UPDATE library_entries SET note = ?1",
                ["replacement root generation"],
            )
            .unwrap();
        let repository = SqliteLibraryRepository::with_hooks(
            environment,
            Arc::new(XdgRootResolver),
            Arc::new(ExistingDataDirectoryReplacement {
                data_directory: data_directory.clone(),
                displaced_directory: temporary.path().join("displaced-data-directory"),
                replacement_directory,
            }),
        );

        let error = repository.export().unwrap_err();

        assert!(matches!(
            error,
            AppError::InvalidState { state, .. } if state == "database_identity_drift"
        ));
        assert_eq!(
            Connection::open(data_directory.join("skilload.db"))
                .unwrap()
                .query_row("SELECT note FROM library_entries", [], |row| row
                    .get::<_, String>(0),)
                .unwrap(),
            "replacement root generation"
        );
    }

    struct CorruptionDetailsDataDirectoryReplacement {
        data_directory: PathBuf,
        displaced_directory: PathBuf,
        replacement_directory: PathBuf,
    }

    impl PersistenceHooks for CorruptionDetailsDataDirectoryReplacement {
        fn before_database_corruption_details(&self, _database: &Path) -> Result<(), AppError> {
            fs::rename(&self.data_directory, &self.displaced_directory).unwrap();
            fs::rename(&self.replacement_directory, &self.data_directory).unwrap();
            Ok(())
        }
    }

    #[test]
    fn corruption_details_reject_a_replaced_data_directory() {
        let temporary = tempdir().unwrap();
        let environment = Arc::new(TestEnvironment::with_roots(temporary.path()));
        let initial = SqliteLibraryRepository::with_environment(
            environment.clone(),
            Arc::new(XdgRootResolver),
        );
        initial
            .import(&document(vec![entry("skills/review", None)]), false)
            .unwrap();
        let data_directory = temporary.path().join("data/skilload");
        let database = data_directory.join("skilload.db");
        let replacement_directory = temporary.path().join("replacement-data-directory");
        fs::create_dir(&replacement_directory).unwrap();
        fs::copy(&database, replacement_directory.join("skilload.db")).unwrap();
        fs::write(&database, b"not sqlite").unwrap();
        let repository = SqliteLibraryRepository::with_hooks(
            environment,
            Arc::new(XdgRootResolver),
            Arc::new(CorruptionDetailsDataDirectoryReplacement {
                data_directory,
                displaced_directory: temporary.path().join("displaced-data-directory"),
                replacement_directory,
            }),
        );

        let error = repository.inspect().unwrap_err();

        assert!(matches!(
            error,
            AppError::InvalidState { state, .. } if state == "database_identity_drift"
        ));
    }

    struct ExistingDatabaseFifoReplacement {
        database: PathBuf,
        displaced: PathBuf,
    }

    impl PersistenceHooks for ExistingDatabaseFifoReplacement {
        fn before_existing_database_generation_open(
            &self,
            _database: &Path,
        ) -> Result<(), AppError> {
            fs::rename(&self.database, &self.displaced).unwrap();
            let status = std::process::Command::new("mkfifo")
                .arg(&self.database)
                .status()
                .unwrap();
            assert!(status.success());
            Ok(())
        }
    }

    #[test]
    fn generation_gate_rejects_fifo_without_waiting() {
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
        let repository = SqliteLibraryRepository::with_hooks(
            environment,
            Arc::new(XdgRootResolver),
            Arc::new(ExistingDatabaseFifoReplacement {
                database: database.clone(),
                displaced: temporary.path().join("displaced.db"),
            }),
        );

        let error = repository.export().unwrap_err();

        assert!(matches!(
            error,
            AppError::InvalidState { state, .. } if state == "database_identity_drift"
        ));
        assert!(
            fs::symlink_metadata(database)
                .unwrap()
                .file_type()
                .is_fifo()
        );
    }

    struct ExistingDatabaseReplacementAfterSync {
        database: PathBuf,
        displaced: PathBuf,
    }

    impl PersistenceHooks for ExistingDatabaseReplacementAfterSync {
        fn after_existing_database_sync_before_parent_sync(
            &self,
            _database: &Path,
        ) -> Result<(), AppError> {
            fs::rename(&self.database, &self.displaced).unwrap();
            fs::write(&self.database, b"foreign database").unwrap();
            Ok(())
        }
    }

    #[test]
    fn existing_import_rejects_a_database_replaced_after_final_sync() {
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
        let repository = SqliteLibraryRepository::with_hooks(
            environment,
            Arc::new(XdgRootResolver),
            Arc::new(ExistingDatabaseReplacementAfterSync {
                database: database.clone(),
                displaced: temporary.path().join("displaced.db"),
            }),
        );

        let error = repository
            .import(&document(vec![entry("skills/new", None)]), false)
            .unwrap_err();

        assert!(matches!(
            error,
            AppError::InvalidState { state, .. } if state == "database_identity_drift"
        ));
        assert_eq!(fs::read(database).unwrap(), b"foreign database");
    }

    #[test]
    fn sqlite_contention_returns_a_bounded_busy_error() {
        let temporary = tempdir().unwrap();
        let repository = SqliteLibraryRepository::with_environment(
            Arc::new(TestEnvironment::with_roots(temporary.path())),
            Arc::new(XdgRootResolver),
        );
        repository
            .import(&document(vec![entry("skills/review", None)]), false)
            .unwrap();
        let database = temporary.path().join("data/skilload/skilload.db");
        let writer = Connection::open(&database).unwrap();
        writer.execute_batch("BEGIN EXCLUSIVE").unwrap();

        let started = std::time::Instant::now();
        let error = repository.export().unwrap_err();
        assert!(started.elapsed() >= LOCK_WAIT);
        assert!(
            matches!(
                &error,
                AppError::Busy {
                    lock_domain,
                    waited_ms,
                } if lock_domain == "database" && *waited_ms == LOCK_WAIT.as_millis() as u64
            ),
            "{error:?}"
        );
        writer.execute_batch("ROLLBACK").unwrap();
    }

    #[test]
    fn derived_validation_preserves_busy_errors() {
        let temporary = tempdir().unwrap();
        let repository = imported_repository(&temporary, vec![entry("skills/review", None)]);
        let database = temporary.path().join("data/skilload/skilload.db");
        let reader = Connection::open(&database).unwrap();
        reader.busy_timeout(Duration::ZERO).unwrap();
        let writer = Connection::open(&database).unwrap();
        writer.execute_batch("BEGIN EXCLUSIVE").unwrap();

        let error = repository
            .derived_index_is_consistent(&reader, &database)
            .unwrap_err();

        assert!(matches!(
            error,
            AppError::Busy {
                lock_domain,
                waited_ms,
            } if lock_domain == "database" && waited_ms == LOCK_WAIT.as_millis() as u64
        ));
        writer.execute_batch("ROLLBACK").unwrap();
    }

    #[test]
    fn database_sync_error_preserves_native_path_bytes() {
        let raw = b"/tmp/library-database-\xff.db";
        let path = PathBuf::from(OsString::from_vec(raw.to_vec()));
        let error = database_sync_error(&path, "sync database", io::Error::other("fault"));

        match error {
            AppError::InvalidState {
                path: Some(actual),
                expected,
                ..
            } => {
                assert_eq!(actual.as_path().as_os_str().as_bytes(), raw);
                assert_eq!(expected, ["a durable Library database generation"]);
            }
            error => panic!("expected typed database path, got {error:?}"),
        }
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

        assert_eq!(repository.inspect().unwrap_err().code(), "database_corrupt");
        assert_eq!(repository.export().unwrap().entries.len(), 1);
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

        assert_eq!(repository.inspect().unwrap_err().code(), "database_corrupt");
        assert_eq!(repository.export().unwrap().entries.len(), 1);
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

        assert_eq!(repository.inspect().unwrap_err().code(), "database_corrupt");
        assert_eq!(repository.export().unwrap().entries.len(), 1);
    }

    #[test]
    fn schema_version_zero_is_database_corrupt() {
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
            .execute_batch(
                "PRAGMA ignore_check_constraints = ON;
                 UPDATE schema_info SET version = 0;
                 PRAGMA ignore_check_constraints = OFF;",
            )
            .unwrap();
        let query = Query::new("review".to_owned()).unwrap();
        assert_eq!(repository.export().unwrap().entries.len(), 1);
        for error in [
            repository.list(&page(100, 0)).unwrap_err(),
            repository.search(&query, &page(100, 0)).unwrap_err(),
            repository
                .get("github:owner/repository#skills/review@refs/heads/main")
                .unwrap_err(),
            repository.inspect().unwrap_err(),
            repository.fix().unwrap_err(),
        ] {
            assert_eq!(error.code(), "database_corrupt", "{error:?}");
        }
    }

    #[test]
    fn corruption_details_name_library_export_when_only_revision_is_invalid() {
        let temporary = tempdir().unwrap();
        let entry = entry("skills/review", None);
        let repository = imported_repository(&temporary, vec![entry.clone()]);
        let database = temporary.path().join("data/skilload/skilload.db");
        Connection::open(&database)
            .unwrap()
            .execute_batch(
                "PRAGMA ignore_check_constraints = ON;
                 UPDATE state_revision SET revision = -1;
                 PRAGMA ignore_check_constraints = OFF;",
            )
            .unwrap();

        assert_eq!(repository.export().unwrap().entries, vec![entry]);
        let error = repository.inspect().unwrap_err();
        assert!(matches!(
            error,
            AppError::DatabaseCorrupt {
                recoverable_exports,
                ..
            } if recoverable_exports == ["library.export"]
        ));
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

    #[test]
    fn metadata_mutations_are_atomic_idempotent_and_exportable() {
        let temporary = tempdir().unwrap();
        let repository = SqliteLibraryRepository::with_environment(
            Arc::new(TestEnvironment::with_roots(temporary.path())),
            Arc::new(XdgRootResolver),
        );
        let import = document(vec![entry("skills/review", None)]);
        let source = import.entries[0].skill.source.canonical.clone();
        repository.import(&import, false).unwrap();
        let database = temporary.path().join("data/skilload/skilload.db");
        let initial_revision = state_revision(&database);

        let alias_set = repository
            .mutate_metadata(&metadata_mutation(
                &source,
                LibraryMetadataChange::alias_set("review-alias".to_owned()).unwrap(),
            ))
            .unwrap();
        assert_eq!(alias_set.outcome, LibraryMutationOutcome::Changed);
        assert_eq!(alias_set.changed_fields, vec![LibraryChangedField::Alias]);
        assert_eq!(alias_set.entry.alias.as_deref(), Some("review-alias"));

        let unchanged_bytes = fs::read(&database).unwrap();
        let unchanged_revision = state_revision(&database);
        let alias_repeat = repository
            .mutate_metadata(&metadata_mutation(
                &source,
                LibraryMetadataChange::alias_set("review-alias".to_owned()).unwrap(),
            ))
            .unwrap();
        assert_eq!(alias_repeat.outcome, LibraryMutationOutcome::Unchanged);
        assert!(alias_repeat.changed_fields.is_empty());
        assert_eq!(fs::read(&database).unwrap(), unchanged_bytes);
        assert_eq!(state_revision(&database), unchanged_revision);

        for change in [
            LibraryMetadataChange::AliasClear,
            LibraryMetadataChange::category_set("Code Review".to_owned()).unwrap(),
            LibraryMetadataChange::CategoryClear,
            LibraryMetadataChange::tag_add("Feature".to_owned()).unwrap(),
            LibraryMetadataChange::tag_remove(" feature ".to_owned()).unwrap(),
            LibraryMetadataChange::note_set("Local note".to_owned()).unwrap(),
            LibraryMetadataChange::NoteClear,
        ] {
            let result = repository
                .mutate_metadata(&metadata_mutation(&source, change))
                .unwrap();
            assert_eq!(result.outcome, LibraryMutationOutcome::Changed);
            assert_eq!(result.changed_fields.len(), 1);
        }

        assert_eq!(state_revision(&database), initial_revision + 8);
        let exported = repository.export().unwrap();
        assert_eq!(exported.entries[0].alias, None);
        assert_eq!(exported.entries[0].category, None);
        assert_eq!(exported.entries[0].tags, ["Review"]);
        assert_eq!(exported.entries[0].note, None);
    }

    #[test]
    fn metadata_alias_conflict_and_missing_target_do_not_mutate_state() {
        let temporary = tempdir().unwrap();
        let repository = SqliteLibraryRepository::with_environment(
            Arc::new(TestEnvironment::with_roots(temporary.path())),
            Arc::new(XdgRootResolver),
        );
        let absent = metadata_mutation(
            "github:owner/repository#skills/missing@refs/heads/main",
            LibraryMetadataChange::AliasClear,
        );
        let error = repository.mutate_metadata(&absent).unwrap_err();
        assert!(matches!(error, AppError::NotFound { .. }));
        assert!(!temporary.path().join("data").exists());
        assert!(!temporary.path().join("state").exists());

        let import = document(vec![entry("skills/one", None), entry("skills/two", None)]);
        let first = import.entries[0].skill.source.canonical.clone();
        let second = import.entries[1].skill.source.canonical.clone();
        repository.import(&import, false).unwrap();
        repository
            .mutate_metadata(&metadata_mutation(
                &first,
                LibraryMetadataChange::alias_set("shared".to_owned()).unwrap(),
            ))
            .unwrap();
        let database = temporary.path().join("data/skilload/skilload.db");
        let before = fs::read(&database).unwrap();
        let revision = state_revision(&database);
        let error = repository
            .mutate_metadata(&metadata_mutation(
                &second,
                LibraryMetadataChange::alias_set("shared".to_owned()).unwrap(),
            ))
            .unwrap_err();
        assert!(matches!(
            error,
            AppError::Conflict { conflicts }
                if conflicts[0].name.as_deref() == Some("shared")
                    && conflicts[0].source.as_ref().is_some_and(|source| source.canonical == second)
        ));
        assert_eq!(fs::read(&database).unwrap(), before);
        assert_eq!(state_revision(&database), revision);
    }

    #[test]
    fn sixty_fifth_metadata_tag_fails_without_a_write() {
        let temporary = tempdir().unwrap();
        let repository = SqliteLibraryRepository::with_environment(
            Arc::new(TestEnvironment::with_roots(temporary.path())),
            Arc::new(XdgRootResolver),
        );
        let import = document(vec![entry("skills/review", None)]);
        let source = import.entries[0].skill.source.canonical.clone();
        repository.import(&import, false).unwrap();
        for index in 0..63 {
            repository
                .mutate_metadata(&metadata_mutation(
                    &source,
                    LibraryMetadataChange::tag_add(format!("tag-{index}")).unwrap(),
                ))
                .unwrap();
        }
        let database = temporary.path().join("data/skilload/skilload.db");
        let before = fs::read(&database).unwrap();
        let revision = state_revision(&database);
        let error = repository
            .mutate_metadata(&metadata_mutation(
                &source,
                LibraryMetadataChange::tag_add("overflow".to_owned()).unwrap(),
            ))
            .unwrap_err();
        assert!(matches!(
            error,
            AppError::Validation { constraint, .. } if constraint == "library_tag_count"
        ));
        assert_eq!(fs::read(&database).unwrap(), before);
        assert_eq!(state_revision(&database), revision);
    }

    #[test]
    fn metadata_mutation_preserves_semantics_at_ten_thousand_entries() {
        let temporary = tempdir().unwrap();
        let repository = SqliteLibraryRepository::with_environment(
            Arc::new(TestEnvironment::with_roots(temporary.path())),
            Arc::new(XdgRootResolver),
        );
        let entries = (0..crate::domain::library::MAX_PORTABLE_LIBRARY_ENTRIES)
            .map(|index| entry(&format!("skills/{index}/review"), None))
            .collect::<Vec<_>>();
        let source = entries[9_999].skill.source.canonical.clone();
        repository.import(&document(entries), false).unwrap();

        let alias = repository
            .mutate_metadata(&metadata_mutation(
                &source,
                LibraryMetadataChange::alias_set("last-review".to_owned()).unwrap(),
            ))
            .unwrap();
        assert_eq!(alias.outcome, LibraryMutationOutcome::Changed);
        assert_eq!(alias.entry.alias.as_deref(), Some("last-review"));
        let equivalent_tag = repository
            .mutate_metadata(&metadata_mutation(
                &source,
                LibraryMetadataChange::tag_add("review".to_owned()).unwrap(),
            ))
            .unwrap();
        assert_eq!(equivalent_tag.outcome, LibraryMutationOutcome::Unchanged);
        assert!(equivalent_tag.changed_fields.is_empty());
        let exported = repository.export().unwrap();
        assert_eq!(exported.entries.len(), 10_000);
        assert_eq!(
            exported
                .entries
                .iter()
                .find(|entry| entry.skill.source.canonical == source)
                .unwrap()
                .alias
                .as_deref(),
            Some("last-review")
        );
    }

    #[test]
    fn metadata_mutation_uses_the_bounded_database_process_lock() {
        let temporary = tempdir().unwrap();
        let repository = SqliteLibraryRepository::with_environment(
            Arc::new(TestEnvironment::with_roots(temporary.path())),
            Arc::new(XdgRootResolver),
        );
        let import = document(vec![entry("skills/review", None)]);
        let source = import.entries[0].skill.source.canonical.clone();
        repository.import(&import, false).unwrap();
        let roots = repository.resolve_roots().unwrap();
        let lock = acquire_restrictive_lock(&roots, "database.lock", "database").unwrap();

        let started = std::time::Instant::now();
        let error = repository
            .mutate_metadata(&metadata_mutation(
                &source,
                LibraryMetadataChange::alias_set("blocked".to_owned()).unwrap(),
            ))
            .unwrap_err();
        assert!(started.elapsed() >= LOCK_WAIT);
        assert!(matches!(
            error,
            AppError::Busy {
                lock_domain,
                waited_ms,
            } if lock_domain == "database" && waited_ms == LOCK_WAIT.as_millis() as u64
        ));
        drop(lock);
    }

    #[test]
    fn metadata_mutation_rejects_a_portable_ceiling_overage_without_a_write() {
        const ENTRY_COUNT: usize = 5_000;

        let mut document = document(
            (0..ENTRY_COUNT)
                .map(|index| entry(&format!("skills/{index}/review"), None))
                .collect(),
        )
        .validate()
        .unwrap();
        for entry in &mut document.entries {
            entry.note = Some(String::new());
        }
        let base_size = document.serialize_for_transfer().unwrap().len() as u64;
        let mut one_character = document.clone();
        one_character.entries[0].note = Some("\u{10000}".to_owned());
        let encoded_character_bytes =
            one_character.serialize_for_transfer().unwrap().len() as u64 - base_size;
        assert!(encoded_character_bytes > 0);

        let mut characters_to_add = ((crate::domain::library::MAX_PORTABLE_LIBRARY_DOCUMENT_BYTES
            - base_size)
            / encoded_character_bytes) as usize;
        for entry in &mut document.entries {
            if characters_to_add == 0 {
                break;
            }
            let count = characters_to_add.min(4_096);
            entry.note = Some("\u{10000}".repeat(count));
            characters_to_add -= count;
        }
        assert_eq!(characters_to_add, 0);
        let accepted_bytes = document.serialize_for_transfer().unwrap().len() as u64;
        assert!(accepted_bytes <= crate::domain::library::MAX_PORTABLE_LIBRARY_DOCUMENT_BYTES);
        assert!(
            crate::domain::library::MAX_PORTABLE_LIBRARY_DOCUMENT_BYTES - accepted_bytes < 1_022,
            "the fixture must leave less room than a maximal category value adds"
        );

        let temporary = tempdir().unwrap();
        let repository = SqliteLibraryRepository::with_environment(
            Arc::new(TestEnvironment::with_roots(temporary.path())),
            Arc::new(XdgRootResolver),
        );
        let source = document.entries[0].skill.source.canonical.clone();
        repository.import(&document, false).unwrap();
        let database = temporary.path().join("data/skilload/skilload.db");
        let before = fs::read(&database).unwrap();
        let revision = state_revision(&database);

        let error = repository
            .mutate_metadata(&metadata_mutation(
                &source,
                LibraryMetadataChange::category_set("\u{10000}".repeat(256)).unwrap(),
            ))
            .unwrap_err();
        assert!(matches!(
            error,
            AppError::Validation { constraint, .. }
                if constraint == "library_portable_document_bytes"
        ));
        assert_eq!(fs::read(&database).unwrap(), before);
        assert_eq!(state_revision(&database), revision);
        assert!(repository.export().unwrap().entries[0].category.is_none());
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

    use crate::domain::library::{LibraryPage, LibrarySearchQuery as Query, LibraryTrustState};
    use sha2::{Digest, Sha256};

    fn imported_repository(
        temporary: &tempfile::TempDir,
        entries: Vec<PortableLibraryEntry>,
    ) -> SqliteLibraryRepository {
        let repository = SqliteLibraryRepository::with_environment(
            Arc::new(TestEnvironment::with_roots(temporary.path())),
            Arc::new(XdgRootResolver),
        );
        repository.import(&document(entries), false).unwrap();
        repository
    }

    fn searchable_entry(path: &str) -> PortableLibraryEntry {
        let mut portable = entry(path, Some("review-buddy"));
        portable.skill.description = "Deep code auditing helper".to_owned();
        portable.category = Some("quality".to_owned());
        portable.note = Some("use for code quality review".to_owned());
        portable.tags = vec!["Review".to_owned(), "Testing".to_owned()];
        portable
    }

    fn page(limit: u16, offset: u64) -> LibraryPage {
        LibraryPage::new(limit, offset).unwrap()
    }

    fn search(repository: &SqliteLibraryRepository, raw: &str) -> Vec<String> {
        let query = Query::new(raw.to_owned()).unwrap();
        let result = repository.search(&query, &page(100, 0)).unwrap();
        result
            .entries
            .iter()
            .map(|entry| entry.skill.source.canonical.clone())
            .collect()
    }

    #[test]
    fn indexed_reads_page_get_and_order_canonically() {
        let temporary = tempdir().unwrap();
        let repository = imported_repository(
            &temporary,
            vec![
                entry("skills/zebra", None),
                entry("skills/alpha", None),
                entry("skills/mid", None),
            ],
        );

        let default_page = repository.list(&page(100, 0)).unwrap();
        assert_eq!(default_page.total, 3);
        assert_eq!(
            default_page
                .entries
                .iter()
                .map(|entry| entry.skill.source.canonical.as_str())
                .collect::<Vec<_>>(),
            [
                "github:owner/repository#skills/alpha@refs/heads/main",
                "github:owner/repository#skills/mid@refs/heads/main",
                "github:owner/repository#skills/zebra@refs/heads/main",
            ]
        );
        assert_eq!(
            default_page.entries[0].trust_state,
            LibraryTrustState::Missing
        );
        assert_eq!(default_page.entries[0].tags, ["Review"]);

        let first = repository.list(&page(2, 0)).unwrap();
        let second = repository.list(&page(2, 1)).unwrap();
        let tail = repository.list(&page(2, 2)).unwrap();
        assert_eq!(first.entries.len(), 2);
        assert_eq!(second.entries.len(), 2);
        assert_eq!(tail.entries.len(), 1);
        assert_eq!(
            first.entries[1].skill.source.canonical,
            second.entries[0].skill.source.canonical
        );
        assert_eq!(
            second.entries[1].skill.source.canonical,
            tail.entries[0].skill.source.canonical
        );

        let at_total = repository.list(&page(100, 3)).unwrap();
        assert!(at_total.entries.is_empty());
        assert_eq!(at_total.total, 3);
        let beyond = repository.list(&page(1, u64::MAX)).unwrap();
        assert!(beyond.entries.is_empty());
        assert_eq!(beyond.total, 3);

        let exact = repository
            .get("github:owner/repository#skills/mid@refs/heads/main")
            .unwrap();
        assert_eq!(exact.skill.source.repository_display, "Repository");
        let missing = repository.get("github:owner/repository#skills/nope@refs/heads/main");
        assert!(matches!(missing, Err(AppError::NotFound { domain, .. }) if domain == "library"));
    }

    #[test]
    fn absent_reads_create_no_state_and_return_empty_views() {
        let temporary = tempdir().unwrap();
        let repository = SqliteLibraryRepository::with_environment(
            Arc::new(TestEnvironment::with_roots(temporary.path())),
            Arc::new(XdgRootResolver),
        );
        let listed = repository.list(&page(100, 0)).unwrap();
        assert_eq!((listed.total, listed.entries.len()), (0, 0));
        let query = Query::new("anything".to_owned()).unwrap();
        let searched = repository.search(&query, &page(100, 0)).unwrap();
        assert_eq!((searched.total, searched.entries.len()), (0, 0));
        assert_eq!(searched.original, "anything");
        assert!(matches!(
            repository.get("github:x/y#z@refs/heads/main"),
            Err(AppError::NotFound { .. })
        ));
        assert!(!temporary.path().join("data/skilload").exists());
        assert!(!temporary.path().join("state/skilload").exists());
    }

    #[test]
    fn absent_read_rejects_data_root_replaced_after_resolution() {
        let temporary = tempdir().unwrap();
        let data_directory = temporary.path().join("data/skilload");
        fs::create_dir_all(&data_directory).unwrap();
        let repository = SqliteLibraryRepository::with_environment(
            Arc::new(TestEnvironment::with_roots(temporary.path())),
            Arc::new(ReplaceDataRootAfterResolve {
                data_directory,
                displaced_directory: temporary.path().join("displaced-data-directory"),
                replaced: std::sync::Mutex::new(false),
            }),
        );

        assert!(matches!(
            repository.list(&page(100, 0)),
            Err(AppError::InvalidEnvironment { variable, .. }) if variable == "XDG_DATA_HOME"
        ));
    }

    #[test]
    fn search_matches_every_field_with_plain_term_semantics() {
        let temporary = tempdir().unwrap();
        let mut control = entry("skills/other", None);
        control.skill.description = "unrelated helper words".to_owned();
        control.tags.clear();
        let repository = imported_repository(
            &temporary,
            vec![searchable_entry("skills/review"), control.clone()],
        );
        let target = "github:owner/repository#skills/review@refs/heads/main".to_owned();
        let control_source = "github:owner/repository#skills/other@refs/heads/main".to_owned();

        for (term, expect_hit) in [
            ("review", true),       // name (and alias token)
            ("auditing", true),     // description
            ("review-buddy", true), // alias phrase
            ("quality", true),      // category and note token
            ("Testing", true),      // tag display spelling
            ("testing", true),      // tag folded spelling
            ("zebra", false),       // never indexed
        ] {
            let hits = search(&repository, term);
            if expect_hit {
                assert_eq!(hits, [target.as_str()], "term {term}");
            } else {
                assert!(hits.is_empty(), "unexpected hits for {term}");
            }
        }

        // note is "use for code quality review": both terms, not adjacent.
        assert_eq!(search(&repository, "code review"), [target.as_str()]);
        // description "Deep code auditing helper" + note "code quality review":
        // terms may hit different fields of the same entry.
        assert_eq!(search(&repository, "deep review"), [target.as_str()]);
        // repository display spelling is indexed for both entries.
        assert_eq!(
            search(&repository, "Repository"),
            [control_source.clone(), target.clone()]
        );
        // operators, filters, wildcards, and quotes stay plain literals.
        assert!(search(&repository, "OR NOT * name:review (x) -y NEAR").is_empty());
        assert!(search(&repository, "a\"b").is_empty());
        // AND across terms requires every term.
        assert!(search(&repository, "review zebra").is_empty());
        // case folding and full-fold alternatives.
        assert_eq!(search(&repository, "REVIEW"), [target.as_str()]);

        let umlaut_root = tempdir().unwrap();
        let mut umlaut = searchable_entry("skills/umlaut");
        umlaut.skill.description = "unrelated helper words".to_owned();
        umlaut.tags = vec![
            "Review".to_owned(),
            "Testing".to_owned(),
            "caf\u{e9}".to_owned(),
        ];
        umlaut.note = None;
        let repository = imported_repository(&umlaut_root, vec![umlaut, control]);
        let umlaut_target = "github:owner/repository#skills/umlaut@refs/heads/main".to_owned();
        assert_eq!(search(&repository, "cafe\u{301}"), [umlaut_target.as_str()]);
        assert_eq!(search(&repository, "CAF\u{c9}"), [umlaut_target.as_str()]);
        assert_eq!(
            search(&repository, "REVIEW TESTING"),
            [umlaut_target.as_str()]
        );
    }

    #[test]
    fn search_matches_nfc_forms_of_normalizable_free_text_fields() {
        let temporary = tempdir().unwrap();
        let decomposed = "cafe\u{301}";

        let mut description = entry("skills/nfc-description", None);
        description.skill.description = decomposed.to_owned();
        let mut alias = entry("skills/nfc-alias", None);
        alias.alias = Some(decomposed.to_owned());
        let mut category = entry("skills/nfc-category", None);
        category.category = Some(decomposed.to_owned());
        let mut note = entry("skills/nfc-note", None);
        note.note = Some(decomposed.to_owned());

        let entries = vec![description, alias, category, note];
        let mut expected = entries
            .iter()
            .map(|entry| entry.skill.source.canonical.clone())
            .collect::<Vec<_>>();
        expected.sort();
        let repository = imported_repository(&temporary, entries);

        for query in ["caf\u{e9}", decomposed] {
            assert_eq!(search(&repository, query), expected, "query {query:?}");
        }
    }

    #[test]
    fn same_name_sources_coexist_and_search_orders_by_source() {
        let temporary = tempdir().unwrap();
        let one = entry("skills/one/review", None);
        let two = entry("skills/two/review", None);
        assert_eq!(one.skill.name, two.skill.name);
        let repository = imported_repository(&temporary, vec![two, one]);

        let query = Query::new("review".to_owned()).unwrap();
        let result = repository.search(&query, &page(1, 0)).unwrap();
        assert_eq!(result.total, 2);
        assert_eq!(result.entries.len(), 1);
        assert_eq!(
            result.entries[0].skill.source.canonical,
            "github:owner/repository#skills/one/review@refs/heads/main"
        );
        let next = repository.search(&query, &page(1, 1)).unwrap();
        assert_eq!(
            next.entries[0].skill.source.canonical,
            "github:owner/repository#skills/two/review@refs/heads/main"
        );
    }

    fn initialize_v1_database(temporary: &tempfile::TempDir) -> PathBuf {
        let data = temporary.path().join("data/skilload");
        fs::create_dir_all(&data).unwrap();
        let database = data.join("skilload.db");
        let mut connection = Connection::open(&database).unwrap();
        initialize_schema(&connection, &database).unwrap();
        {
            let transaction = connection.transaction().unwrap();
            apply_additions(
                &transaction,
                &[searchable_entry("skills/review")],
                &database,
            )
            .unwrap();
            transaction.commit().unwrap();
        }
        connection
            .execute_batch(
                "DROP TABLE library_fts;
                 UPDATE schema_info SET version = 1;",
            )
            .unwrap();
        drop(connection);
        database
    }

    #[test]
    fn v1_reads_keep_working_while_search_and_writes_require_migration() {
        let temporary = tempdir().unwrap();
        initialize_v1_database(&temporary);
        let repository = SqliteLibraryRepository::with_environment(
            Arc::new(TestEnvironment::with_roots(temporary.path())),
            Arc::new(XdgRootResolver),
        );

        let listed = repository.list(&page(100, 0)).unwrap();
        assert_eq!(listed.total, 1);
        let got = repository
            .get("github:owner/repository#skills/review@refs/heads/main")
            .unwrap();
        assert_eq!(got.note.as_deref(), Some("use for code quality review"));
        assert_eq!(repository.export().unwrap().entries.len(), 1);

        let query = Query::new("review".to_owned()).unwrap();
        assert!(matches!(
            repository.search(&query, &page(100, 0)),
            Err(AppError::MigrationRequired {
                found_version: 1,
                supported_version: 2,
                ..
            })
        ));
        assert!(matches!(
            repository.import(&document(vec![entry("skills/new", None)]), false),
            Err(AppError::MigrationRequired { .. })
        ));
        assert!(matches!(
            repository.mutate_metadata(&metadata_mutation(
                "github:owner/repository#skills/review@refs/heads/main",
                LibraryMetadataChange::NoteSet("changed".to_owned()),
            )),
            Err(AppError::MigrationRequired { .. })
        ));
    }

    #[test]
    fn doctor_fix_migrates_v1_after_a_validated_backup() {
        let temporary = tempdir().unwrap();
        let database = initialize_v1_database(&temporary);
        let revision_before = state_revision(&database);
        let export_before = {
            let repository = SqliteLibraryRepository::with_environment(
                Arc::new(TestEnvironment::with_roots(temporary.path())),
                Arc::new(XdgRootResolver),
            );
            repository.export().unwrap()
        };
        let repository = SqliteLibraryRepository::with_environment(
            Arc::new(TestEnvironment::with_roots(temporary.path())),
            Arc::new(XdgRootResolver),
        );

        let diagnosis = repository.inspect().unwrap();
        assert!(!diagnosis.fix_requested);
        assert!(diagnosis.actions.is_empty());
        assert!(!diagnosis.database_writable);
        assert_eq!(diagnosis.findings.len(), 1);
        assert_eq!(
            diagnosis.findings[0].code,
            "library_database_migration_required"
        );
        assert!(diagnosis.findings[0].fixable_offline);
        assert!(!diagnosis.findings[0].fixed);

        let operation = repository.fix().unwrap();
        assert_eq!(operation.outcome.as_str(), "changed");
        assert_eq!(operation.data.actions.len(), 1);
        let action = &operation.data.actions[0];
        assert_eq!(action.kind.as_str(), "migrate");
        assert_eq!(action.before.as_deref(), Some("schema_1"));
        assert_eq!(action.after.as_deref(), Some("schema_2"));
        assert_eq!(
            action.target.as_path(),
            database.canonicalize().unwrap().as_path()
        );
        assert!(operation.data.findings[0].fixed);

        let backups_root = temporary.path().join("data/skilload/backups");
        let mut backup_dbs: Vec<_> = fs::read_dir(&backups_root)
            .unwrap()
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|extension| extension == "db"))
            .collect();
        backup_dbs.sort();
        assert_eq!(backup_dbs.len(), 1);
        let backup = &backup_dbs[0];
        let manifest = backup.with_extension("manifest.json");
        let manifest: serde_json::Value =
            serde_json::from_slice(&fs::read(&manifest).unwrap()).unwrap();
        assert_eq!(manifest["format_version"], 1);
        assert_eq!(manifest["source_schema"], 1);
        assert_eq!(manifest["target_schema"], 2);
        assert_eq!(manifest["complete"], true);
        assert_eq!(
            manifest["database_bytes"],
            fs::metadata(backup).unwrap().len()
        );
        let mut hasher = Sha256::new();
        hasher.update(fs::read(backup).unwrap());
        let digest = hasher.finalize();
        let mut hex = String::new();
        for byte in digest {
            use std::fmt::Write as _;
            let _ = write!(hex, "{byte:02x}");
        }
        assert_eq!(manifest["sha256"], format!("sha256:{hex}"));

        let schema: i64 = Connection::open(&database)
            .unwrap()
            .query_row("SELECT version FROM schema_info", [], |row| row.get(0))
            .unwrap();
        assert_eq!(schema, 2);
        assert_eq!(state_revision(&database), revision_before);
        assert_eq!(repository.export().unwrap().entries, export_before.entries);
        assert_eq!(search(&repository, "code review").len(), 1);

        let healthy = repository.inspect().unwrap();
        assert!(healthy.findings.is_empty());
        assert!(healthy.database_writable);
        let repeated = repository.fix().unwrap();
        assert_eq!(repeated.outcome.as_str(), "unchanged");
        let backup_count = fs::read_dir(&backups_root)
            .unwrap()
            .flatten()
            .filter(|entry| entry.path().extension().is_some_and(|e| e == "db"))
            .count();
        assert_eq!(backup_count, 1);
    }

    #[test]
    fn fts_drift_is_doctor_fixable_without_touching_base_rows() {
        let temporary = tempdir().unwrap();
        let database = temporary.path().join("data/skilload/skilload.db");
        let repository = imported_repository(
            &temporary,
            vec![
                searchable_entry("skills/review"),
                entry("skills/other", None),
            ],
        );
        let revision_before = state_revision(&database);
        let export_before = repository.export().unwrap();
        Connection::open(&database)
            .unwrap()
            .execute(
                "DELETE FROM library_fts WHERE canonical_source LIKE '%review%'",
                [],
            )
            .unwrap();

        let listed = repository.list(&page(100, 0)).unwrap();
        assert_eq!(listed.total, 2);
        assert!(
            repository
                .get("github:owner/repository#skills/review@refs/heads/main")
                .is_ok()
        );
        assert_eq!(repository.export().unwrap().entries.len(), 2);
        let query = Query::new("review".to_owned()).unwrap();
        assert!(matches!(
            repository.search(&query, &page(100, 0)),
            Err(AppError::InvalidState { state, .. }) if state == "library_fts_invalid"
        ));

        let diagnosis = repository.inspect().unwrap();
        assert_eq!(diagnosis.findings.len(), 1);
        assert_eq!(diagnosis.findings[0].code, "library_fts_invalid");
        assert!(diagnosis.findings[0].fixable_offline);
        assert!(!diagnosis.database_writable);

        let operation = repository.fix().unwrap();
        assert_eq!(operation.outcome.as_str(), "changed");
        assert_eq!(operation.data.actions.len(), 1);
        assert_eq!(operation.data.actions[0].kind.as_str(), "repair");
        assert_eq!(
            operation.data.actions[0].before.as_deref(),
            Some("fts_invalid")
        );
        assert_eq!(
            operation.data.actions[0].after.as_deref(),
            Some("fts_valid")
        );
        assert_eq!(state_revision(&database), revision_before);
        assert_eq!(repository.export().unwrap().entries, export_before.entries);
        assert_eq!(search(&repository, "code review").len(), 1);
        assert!(repository.inspect().unwrap().findings.is_empty());
        let repeated = repository.fix().unwrap();
        assert_eq!(repeated.outcome.as_str(), "unchanged");
    }

    #[test]
    fn mutations_reject_fts_special_integrity_drift_before_committing() {
        let temporary = tempdir().unwrap();
        let original = searchable_entry("skills/review");
        let source = original.skill.source.canonical.clone();
        let repository = imported_repository(&temporary, vec![original]);
        let database = temporary.path().join("data/skilload/skilload.db");
        Connection::open(&database)
            .unwrap()
            .execute("DELETE FROM library_fts_docsize", [])
            .unwrap();
        let before = fs::read(&database).unwrap();

        let import_error = repository
            .import(&document(vec![entry("skills/new", None)]), false)
            .unwrap_err();
        assert!(matches!(
            import_error,
            AppError::InvalidState { state, .. } if state == "library_fts_invalid"
        ));
        assert_eq!(fs::read(&database).unwrap(), before);

        let mutation_error = repository
            .mutate_metadata(&metadata_mutation(
                &source,
                LibraryMetadataChange::note_set("blocked".to_owned()).unwrap(),
            ))
            .unwrap_err();
        assert!(matches!(
            mutation_error,
            AppError::InvalidState { state, .. } if state == "library_fts_invalid"
        ));
        assert_eq!(fs::read(&database).unwrap(), before);

        let operation = repository.fix().unwrap();
        assert_eq!(operation.outcome.as_str(), "changed");
        assert!(repository.inspect().unwrap().findings.is_empty());
    }

    #[test]
    fn doctor_is_filesystem_inert_when_absent_or_healthy() {
        let temporary = tempdir().unwrap();
        let repository = SqliteLibraryRepository::with_environment(
            Arc::new(TestEnvironment::with_roots(temporary.path())),
            Arc::new(XdgRootResolver),
        );
        let absent = repository.inspect().unwrap();
        assert!(absent.findings.is_empty());
        assert!(absent.actions.is_empty());
        assert!(absent.database_writable);
        assert!(!temporary.path().join("data/skilload").exists());
        assert!(!temporary.path().join("state/skilload").exists());

        let repository = imported_repository(&temporary, vec![entry("skills/review", None)]);
        let database = temporary.path().join("data/skilload/skilload.db");
        let before = fs::metadata(&database).unwrap();
        let bytes_before = fs::read(&database).unwrap();
        let healthy = repository.inspect().unwrap();
        assert!(healthy.findings.is_empty());
        assert!(healthy.database_writable);
        let after = fs::metadata(&database).unwrap();
        assert_eq!(before.len(), after.len());
        assert_eq!(before.mtime(), after.mtime());
        assert_eq!(fs::read(&database).unwrap(), bytes_before);
        assert!(
            !temporary
                .path()
                .join("data/skilload/skilload.db-shm")
                .exists()
        );
        assert!(
            !temporary
                .path()
                .join("data/skilload/skilload.db-wal")
                .exists()
        );
    }

    #[test]
    fn wal_generations_are_rejected_before_sqlite_opens() {
        let temporary = tempdir().unwrap();
        let data = temporary.path().join("data/skilload");
        fs::create_dir_all(&data).unwrap();
        let database = data.join("skilload.db");
        {
            let connection = Connection::open(&database).unwrap();
            initialize_schema(&connection, &database).unwrap();
            connection
                .execute_batch("PRAGMA journal_mode = WAL;")
                .unwrap();
        }
        let bytes_before = fs::read(&database).unwrap();
        let repository = SqliteLibraryRepository::with_environment(
            Arc::new(TestEnvironment::with_roots(temporary.path())),
            Arc::new(XdgRootResolver),
        );
        for result in [
            repository.export().err(),
            repository.list(&page(100, 0)).err(),
            repository.inspect().err(),
        ] {
            let error = result.expect("wal generation must be rejected");
            assert_eq!(error.code(), "database_corrupt");
        }
        assert_eq!(fs::read(&database).unwrap(), bytes_before);
        assert!(!data.join("skilload.db-shm").exists());
        assert!(!data.join("skilload.db-wal").exists());

        fs::write(data.join("skilload.db-shm"), b"stray shm").unwrap();
        let query = Query::new("review".to_owned()).unwrap();
        assert_eq!(
            repository.search(&query, &page(100, 0)).unwrap_err().code(),
            "database_corrupt"
        );
    }

    #[test]
    fn rollback_journal_generation_is_rejected_before_sqlite_opens() {
        let temporary = tempdir().unwrap();
        let repository = imported_repository(&temporary, vec![entry("skills/review", None)]);
        let database = temporary.path().join("data/skilload/skilload.db");
        let journal = temporary.path().join("data/skilload/skilload.db-journal");
        let database_before = fs::read(&database).unwrap();
        let journal_before = b"unrecovered DELETE-mode rollback journal".to_vec();
        fs::write(&journal, &journal_before).unwrap();
        let query = Query::new("review".to_owned()).unwrap();

        for error in [
            repository.export().unwrap_err(),
            repository.list(&page(100, 0)).unwrap_err(),
            repository.search(&query, &page(100, 0)).unwrap_err(),
            repository
                .get("github:owner/repository#skills/review@refs/heads/main")
                .unwrap_err(),
            repository.inspect().unwrap_err(),
        ] {
            assert_eq!(error.code(), "database_corrupt", "{error:?}");
        }
        assert_eq!(fs::read(&database).unwrap(), database_before);
        assert_eq!(fs::read(&journal).unwrap(), journal_before);
        assert!(
            !temporary
                .path()
                .join("data/skilload/skilload.db-wal")
                .exists()
        );
        assert!(
            !temporary
                .path()
                .join("data/skilload/skilload.db-shm")
                .exists()
        );
    }

    struct SidecarAfterGenerationGate {
        journal: PathBuf,
    }

    impl PersistenceHooks for SidecarAfterGenerationGate {
        fn before_existing_database_open(&self, _database: &Path) -> Result<(), AppError> {
            fs::write(&self.journal, b"journal created after generation gate").unwrap();
            Ok(())
        }
    }

    #[test]
    fn read_snapshot_rejects_a_journal_created_after_generation_gate() {
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
        let journal = database.with_file_name("skilload.db-journal");
        let journal_bytes = b"journal created after generation gate".to_vec();
        let repository = SqliteLibraryRepository::with_hooks(
            environment,
            Arc::new(XdgRootResolver),
            Arc::new(SidecarAfterGenerationGate {
                journal: journal.clone(),
            }),
        );
        let query = Query::new("review".to_owned()).unwrap();

        for error in [
            repository.list(&page(100, 0)).unwrap_err(),
            repository.search(&query, &page(100, 0)).unwrap_err(),
            repository
                .get("github:owner/repository#skills/review@refs/heads/main")
                .unwrap_err(),
            repository.export().unwrap_err(),
            repository.inspect().unwrap_err(),
        ] {
            assert_eq!(error.code(), "database_corrupt", "{error:?}");
        }
        assert_eq!(fs::read(&journal).unwrap(), journal_bytes);
    }

    #[test]
    fn failed_read_revalidates_database_generation_before_returning_error() {
        let temporary = tempdir().unwrap();
        let repository = imported_repository(&temporary, vec![entry("skills/review", None)]);
        let roots = repository.resolve_roots().unwrap();
        let database = SqliteLibraryRepository::database_path(&roots);
        let directory = repository.open_bound_data_directory(&roots).unwrap();
        let (mut connection, identity) = repository
            .open_existing_database(&directory, &database, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .unwrap();
        let displaced = temporary.path().join("displaced-skilload.db");

        let error = repository
            .run_read_snapshot(
                &mut connection,
                &directory,
                &database,
                identity,
                |_transaction| {
                    fs::rename(&database, &displaced).unwrap();
                    fs::copy(&displaced, &database).unwrap();
                    Err::<(), _>(AppError::not_found("library", "missing".to_owned()))
                },
            )
            .unwrap_err();

        assert!(matches!(
            error,
            AppError::InvalidState { domain, state, .. }
                if domain == "library_database" && state == "database_identity_drift"
        ));
    }

    struct MigrationFailpoint(&'static str);

    impl PersistenceHooks for MigrationFailpoint {
        fn after_backup_copy(&self, _staging: &Path) -> Result<(), AppError> {
            if self.0 == "after_backup_copy" {
                return Err(AppError::Internal {
                    incident_id: "backup-copy-failure".to_owned(),
                });
            }
            Ok(())
        }

        fn before_migration_commit(&self, _database: &Path) -> Result<(), AppError> {
            if self.0 == "before_migration_commit" {
                return Err(AppError::Internal {
                    incident_id: "before-migration-commit".to_owned(),
                });
            }
            Ok(())
        }

        fn after_migration_commit_before_sync(&self, _database: &Path) -> Result<(), AppError> {
            if self.0 == "after_migration_commit_before_sync" {
                return Err(AppError::Internal {
                    incident_id: "after-migration-commit".to_owned(),
                });
            }
            Ok(())
        }

        fn after_fts_rebuild_commit_before_sync(&self, _database: &Path) -> Result<(), AppError> {
            if self.0 == "after_fts_rebuild_commit_before_sync" {
                return Err(AppError::Internal {
                    incident_id: "after-fts-rebuild-commit".to_owned(),
                });
            }
            Ok(())
        }
    }

    #[test]
    fn migration_failpoints_leave_a_coherent_state() {
        // Backup copy failure: live stays v1 and no complete pair is published.
        let temporary = tempdir().unwrap();
        let database = initialize_v1_database(&temporary);
        let revision_before = state_revision(&database);
        let repository = SqliteLibraryRepository::with_hooks(
            Arc::new(TestEnvironment::with_roots(temporary.path())),
            Arc::new(XdgRootResolver),
            Arc::new(MigrationFailpoint("after_backup_copy")),
        );
        assert_eq!(repository.fix().unwrap_err().code(), "internal_invariant");
        assert_eq!(read_schema_version(&database), 1);
        assert_eq!(state_revision(&database), revision_before);
        let backups_root = temporary.path().join("data/skilload/backups");
        let complete = fs::read_dir(&backups_root)
            .map(|entries| {
                entries
                    .flatten()
                    .filter(|entry| {
                        entry
                            .file_name()
                            .to_string_lossy()
                            .ends_with(".manifest.json")
                    })
                    .count()
            })
            .unwrap_or(0);
        assert_eq!(complete, 0);

        // Pre-commit migration failure: v1 plus one complete backup remain.
        let temporary = tempdir().unwrap();
        let database = initialize_v1_database(&temporary);
        let repository = SqliteLibraryRepository::with_hooks(
            Arc::new(TestEnvironment::with_roots(temporary.path())),
            Arc::new(XdgRootResolver),
            Arc::new(MigrationFailpoint("before_migration_commit")),
        );
        assert_eq!(repository.fix().unwrap_err().code(), "internal_invariant");
        assert_eq!(read_schema_version(&database), 1);
        assert_eq!(state_revision(&database), revision_before);
        assert!(
            SqliteLibraryRepository::with_environment(
                Arc::new(TestEnvironment::with_roots(temporary.path())),
                Arc::new(XdgRootResolver),
            )
            .inspect()
            .unwrap()
            .findings
            .iter()
            .any(|finding| finding.code == "library_database_migration_required")
        );
        let backups_root = temporary.path().join("data/skilload/backups");
        assert_eq!(
            fs::read_dir(&backups_root)
                .unwrap()
                .flatten()
                .filter(|entry| entry
                    .file_name()
                    .to_string_lossy()
                    .ends_with(".manifest.json"))
                .count(),
            1
        );

        // Post-commit failure: v2 is durable and the error does not claim v1.
        let temporary = tempdir().unwrap();
        let database = initialize_v1_database(&temporary);
        let repository = SqliteLibraryRepository::with_hooks(
            Arc::new(TestEnvironment::with_roots(temporary.path())),
            Arc::new(XdgRootResolver),
            Arc::new(MigrationFailpoint("after_migration_commit_before_sync")),
        );
        assert_eq!(repository.fix().unwrap_err().code(), "internal_invariant");
        assert_eq!(read_schema_version(&database), 2);
        let healthy = SqliteLibraryRepository::with_environment(
            Arc::new(TestEnvironment::with_roots(temporary.path())),
            Arc::new(XdgRootResolver),
        )
        .inspect()
        .unwrap();
        assert!(healthy.findings.is_empty());

        // FTS rebuild post-commit failure: base intact, index committed.
        let temporary = tempdir().unwrap();
        let database = temporary.path().join("data/skilload/skilload.db");
        imported_repository(&temporary, vec![searchable_entry("skills/review")]);
        let revision_before = state_revision(&database);
        Connection::open(&database)
            .unwrap()
            .execute("DELETE FROM library_fts", [])
            .unwrap();
        let repository = SqliteLibraryRepository::with_hooks(
            Arc::new(TestEnvironment::with_roots(temporary.path())),
            Arc::new(XdgRootResolver),
            Arc::new(MigrationFailpoint("after_fts_rebuild_commit_before_sync")),
        );
        assert_eq!(repository.fix().unwrap_err().code(), "internal_invariant");
        assert_eq!(state_revision(&database), revision_before);
        assert_eq!(read_schema_version(&database), 2);
        let healthy = SqliteLibraryRepository::with_environment(
            Arc::new(TestEnvironment::with_roots(temporary.path())),
            Arc::new(XdgRootResolver),
        )
        .inspect()
        .unwrap();
        assert!(healthy.findings.is_empty());
    }

    fn read_schema_version(database: &Path) -> i64 {
        Connection::open(database)
            .unwrap()
            .query_row("SELECT version FROM schema_info", [], |row| row.get(0))
            .unwrap()
    }

    #[test]
    fn reads_reject_live_rollback_journals_before_descriptor_opens() {
        let temporary = tempdir().unwrap();
        let repository = imported_repository(&temporary, vec![entry("skills/review", None)]);
        let database = temporary.path().join("data/skilload/skilload.db");

        let mut writer = Connection::open(&database).unwrap();
        let writer_transaction = writer.transaction().unwrap();
        apply_additions(
            &writer_transaction,
            &[entry("skills/uncommitted", None)],
            &database,
        )
        .unwrap();
        assert!(database.with_file_name("skilload.db-journal").exists());
        let query = Query::new("review".to_owned()).unwrap();
        for error in [
            repository.list(&page(100, 0)).unwrap_err(),
            repository.search(&query, &page(100, 0)).unwrap_err(),
        ] {
            assert_eq!(error.code(), "database_corrupt", "{error:?}");
        }
        writer_transaction.commit().unwrap();

        let listed = repository.list(&page(100, 0)).unwrap();
        assert_eq!(listed.total, 2);
    }

    #[test]
    fn corrupt_base_keeps_typed_details_with_known_backups() {
        let temporary = tempdir().unwrap();
        let database = initialize_v1_database(&temporary);
        let repository = SqliteLibraryRepository::with_environment(
            Arc::new(TestEnvironment::with_roots(temporary.path())),
            Arc::new(XdgRootResolver),
        );
        repository.fix().unwrap();

        let mut bytes = fs::read(&database).unwrap();
        bytes[100..132].fill(0xa5);
        fs::write(&database, &bytes).unwrap();

        let error = repository.inspect().unwrap_err();
        assert!(matches!(
            &error,
            AppError::DatabaseCorrupt { backups, .. } if backups.len() == 1
        ));
        let query = Query::new("review".to_owned()).unwrap();
        assert!(matches!(
            repository.search(&query, &page(100, 0)),
            Err(AppError::DatabaseCorrupt { .. })
        ));
        assert!(matches!(
            repository.fix(),
            Err(AppError::DatabaseCorrupt { .. })
        ));
    }

    #[test]
    fn backup_inventory_rejects_pairs_with_sqlite_sidecars() {
        for suffix in DATABASE_SIDECAR_SUFFIXES {
            let temporary = tempdir().unwrap();
            let database = initialize_v1_database(&temporary);
            let repository = SqliteLibraryRepository::with_environment(
                Arc::new(TestEnvironment::with_roots(temporary.path())),
                Arc::new(XdgRootResolver),
            );
            repository.fix().unwrap();
            let backups_root = temporary.path().join("data/skilload/backups");
            let backup = fs::read_dir(&backups_root)
                .unwrap()
                .flatten()
                .map(|entry| entry.path())
                .find(|path| path.extension().is_some_and(|extension| extension == "db"))
                .unwrap();
            let mut companion_name = backup.file_name().unwrap().to_os_string();
            companion_name.push(suffix);
            fs::write(backup.with_file_name(companion_name), b"backup sidecar").unwrap();
            let mut bytes = fs::read(&database).unwrap();
            bytes[100..132].fill(0xa5);
            fs::write(&database, &bytes).unwrap();

            assert!(matches!(
                repository.inspect().unwrap_err(),
                AppError::DatabaseCorrupt { backups, .. } if backups.is_empty()
            ));
        }
    }

    #[test]
    fn fts_shadow_corruption_stays_doctor_fixable() {
        let temporary = tempdir().unwrap();
        let database = temporary.path().join("data/skilload/skilload.db");
        imported_repository(
            &temporary,
            vec![
                searchable_entry("skills/review"),
                entry("skills/other", None),
            ],
        );
        // Damage only the FTS inverted-index shadow b-tree: flip cell bytes
        // at the tail of `library_fts_data`'s root page. Base rows, the FTS
        // content table, and the schema stay intact.
        let connection = Connection::open(&database).unwrap();
        let page_size: i64 = connection
            .query_row("PRAGMA page_size", [], |row| row.get(0))
            .unwrap();
        let data_page: i64 = connection
            .query_row(
                "SELECT rootpage FROM sqlite_master WHERE name = 'library_fts_data'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        drop(connection);
        let mut bytes = fs::read(&database).unwrap();
        let page_start = ((data_page - 1) * page_size) as usize;
        bytes[page_start + page_size as usize - 24..page_start + page_size as usize - 16]
            .fill(0xa5);
        fs::write(&database, &bytes).unwrap();
        // Premise: the general integrity check does report the damaged
        // shadow tree, so whole-database base validation would have
        // classified this as base corruption before the fix.
        let whole: String = Connection::open(&database)
            .unwrap()
            .query_row("PRAGMA integrity_check", [], |row| row.get(0))
            .unwrap();
        assert_ne!(
            whole, "ok",
            "fixture must damage a shadow tree the general check reports"
        );

        let repository = SqliteLibraryRepository::with_environment(
            Arc::new(TestEnvironment::with_roots(temporary.path())),
            Arc::new(XdgRootResolver),
        );
        let query = Query::new("code review".to_owned()).unwrap();
        let error = repository.search(&query, &page(100, 0)).unwrap_err();
        assert!(matches!(
            error,
            AppError::InvalidState { state, .. } if state == "library_fts_invalid"
        ));

        let diagnosis = repository.inspect().unwrap();
        assert_eq!(diagnosis.findings.len(), 1);
        assert_eq!(diagnosis.findings[0].code, "library_fts_invalid");
        assert!(diagnosis.findings[0].fixable_offline);
        assert!(!diagnosis.database_writable);

        let operation = repository.fix().unwrap();
        assert_eq!(operation.outcome.as_str(), "changed");
        assert_eq!(operation.data.actions.len(), 1);
        assert_eq!(operation.data.actions[0].kind.as_str(), "repair");
        assert_eq!(search(&repository, "code review").len(), 1);
        assert!(repository.inspect().unwrap().findings.is_empty());
        let integrity: String = Connection::open(&database)
            .unwrap()
            .query_row("PRAGMA integrity_check", [], |row| row.get(0))
            .unwrap();
        assert_eq!(integrity, "ok");
    }

    #[test]
    fn malformed_fts_schema_stays_derived_and_doctor_fixable() {
        let temporary = tempdir().unwrap();
        let database = temporary.path().join("data/skilload/skilload.db");
        let entry = searchable_entry("skills/review");
        let source = entry.skill.source.canonical.clone();
        let repository = imported_repository(&temporary, vec![entry.clone()]);
        let connection = Connection::open(&database).unwrap();
        connection
            .execute_batch("PRAGMA writable_schema = ON;")
            .unwrap();
        connection
            .execute(
                "UPDATE sqlite_master SET sql = ?1 WHERE type = 'table' AND name = 'library_fts'",
                ["CREATE VIRTUAL TABLE library_fts USING fts5("],
            )
            .unwrap();
        connection
            .execute_batch("PRAGMA writable_schema = OFF;")
            .unwrap();
        let schema_version: i64 = connection
            .query_row("PRAGMA schema_version", [], |row| row.get(0))
            .unwrap();
        connection
            .pragma_update(None, "schema_version", schema_version + 1)
            .unwrap();
        drop(connection);
        let before = fs::read(&database).unwrap();

        let listed = repository.list(&page(100, 0)).unwrap();
        assert_eq!(listed.total, 1);
        assert_eq!(listed.entries[0].skill.source.canonical, source);
        assert_eq!(
            repository.get(&source).unwrap().skill.source.canonical,
            source
        );
        assert_eq!(repository.export().unwrap().entries.len(), 1);
        let query = Query::new("code review".to_owned()).unwrap();
        assert!(matches!(
            repository.search(&query, &page(100, 0)),
            Err(AppError::InvalidState { state, .. }) if state == "library_fts_invalid"
        ));
        let diagnosis = repository.inspect().unwrap();
        assert_eq!(diagnosis.findings.len(), 1);
        assert_eq!(diagnosis.findings[0].code, "library_fts_invalid");
        assert_eq!(fs::read(&database).unwrap(), before);

        let operation = repository.fix().unwrap();
        assert_eq!(operation.outcome.as_str(), "changed");
        assert_eq!(search(&repository, "code review").len(), 1);
        assert!(repository.inspect().unwrap().findings.is_empty());
    }

    #[test]
    fn orphaned_fts_shadow_tables_are_doctor_fixable() {
        let temporary = tempdir().unwrap();
        let database = temporary.path().join("data/skilload/skilload.db");
        let repository = imported_repository(&temporary, vec![searchable_entry("skills/review")]);
        let connection = Connection::open(&database).unwrap();
        connection
            .execute_batch(
                "PRAGMA writable_schema = ON;
                 DELETE FROM sqlite_master WHERE name = 'library_fts';
                 PRAGMA writable_schema = OFF;",
            )
            .unwrap();
        assert_eq!(
            connection
                .query_row(
                    "SELECT count(*) FROM sqlite_master WHERE name = 'library_fts_data'",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap(),
            1,
            "fixture must retain orphaned FTS shadow tables"
        );
        let schema_version: i64 = connection
            .query_row("PRAGMA schema_version", [], |row| row.get(0))
            .unwrap();
        connection
            .pragma_update(None, "schema_version", schema_version + 1)
            .unwrap();
        drop(connection);

        let diagnosis = repository.inspect().unwrap();
        assert_eq!(diagnosis.findings[0].code, "library_fts_invalid");
        let operation = repository.fix().unwrap();
        assert_eq!(operation.outcome.as_str(), "changed");
        assert_eq!(operation.data.actions[0].kind.as_str(), "repair");
        assert_eq!(search(&repository, "review").len(), 1);
        assert!(repository.inspect().unwrap().findings.is_empty());
    }

    #[test]
    fn tampered_or_symlinked_backups_are_never_validated() {
        // Digest drift at equal length must be rejected.
        let temporary = tempdir().unwrap();
        let database = initialize_v1_database(&temporary);
        let repository = SqliteLibraryRepository::with_environment(
            Arc::new(TestEnvironment::with_roots(temporary.path())),
            Arc::new(XdgRootResolver),
        );
        repository.fix().unwrap();
        let backups_root = temporary.path().join("data/skilload/backups");
        let backup = fs::read_dir(&backups_root)
            .unwrap()
            .flatten()
            .find(|entry| entry.path().extension().is_some_and(|e| e == "db"))
            .unwrap()
            .path();
        let mut backup_bytes = fs::read(&backup).unwrap();
        let last = backup_bytes.len() - 1;
        backup_bytes[last] ^= 0xff;
        assert_eq!(
            backup_bytes.len(),
            fs::metadata(&backup).unwrap().len() as usize
        );
        fs::write(&backup, &backup_bytes).unwrap();
        let mut bytes = fs::read(&database).unwrap();
        bytes[100..132].fill(0xa5);
        fs::write(&database, &bytes).unwrap();
        assert!(matches!(
            repository.inspect().unwrap_err(),
            AppError::DatabaseCorrupt { backups, .. } if backups.is_empty()
        ));

        // A symlinked manifest must be rejected even when it parses and the
        // database side still matches its record.
        let temporary = tempdir().unwrap();
        let database = initialize_v1_database(&temporary);
        let repository = SqliteLibraryRepository::with_environment(
            Arc::new(TestEnvironment::with_roots(temporary.path())),
            Arc::new(XdgRootResolver),
        );
        repository.fix().unwrap();
        let backups_root = temporary.path().join("data/skilload/backups");
        let manifest = fs::read_dir(&backups_root)
            .unwrap()
            .flatten()
            .find(|entry| entry.path().extension().is_some_and(|e| e == "json"))
            .unwrap()
            .path();
        let target = backups_root.join("foreign-target.json");
        fs::rename(&manifest, &target).unwrap();
        symlink(&target, &manifest).unwrap();
        let mut bytes = fs::read(&database).unwrap();
        bytes[100..132].fill(0xa5);
        fs::write(&database, &bytes).unwrap();
        assert!(matches!(
            repository.inspect().unwrap_err(),
            AppError::DatabaseCorrupt { backups, .. } if backups.is_empty()
        ));
    }

    #[test]
    fn incompatible_or_nonstandalone_backups_are_never_advertised() {
        let temporary = tempdir().unwrap();
        let database = initialize_v1_database(&temporary);
        let v1_copy = temporary.path().join("source-v1.db");
        fs::copy(&database, &v1_copy).unwrap();
        let repository = SqliteLibraryRepository::with_environment(
            Arc::new(TestEnvironment::with_roots(temporary.path())),
            Arc::new(XdgRootResolver),
        );
        repository.fix().unwrap();
        let backups_root = temporary.path().join("data/skilload/backups");
        fs::remove_dir_all(&backups_root).unwrap();
        fs::create_dir(&backups_root).unwrap();

        let newer_stem = "newer-schema";
        seed_foreign_valid_pair(&backups_root, newer_stem, &v1_copy);
        let newer_manifest = backups_root.join(format!("{newer_stem}.manifest.json"));
        let mut newer_record: BackupManifestRecord =
            serde_json::from_slice(&fs::read(&newer_manifest).unwrap()).unwrap();
        newer_record.target_schema = SCHEMA_VERSION + 1;
        fs::write(&newer_manifest, serde_json::to_vec(&newer_record).unwrap()).unwrap();

        let foreign_stem = "foreign-sqlite";
        seed_foreign_valid_pair(&backups_root, foreign_stem, &v1_copy);
        let foreign_database = backups_root.join(format!("{foreign_stem}.db"));
        fs::remove_file(&foreign_database).unwrap();
        Connection::open(&foreign_database)
            .unwrap()
            .execute("CREATE TABLE foreign_data (value TEXT NOT NULL)", [])
            .unwrap();
        let foreign_manifest = backups_root.join(format!("{foreign_stem}.manifest.json"));
        let mut foreign_record: BackupManifestRecord =
            serde_json::from_slice(&fs::read(&foreign_manifest).unwrap()).unwrap();
        foreign_record.database_bytes = fs::metadata(&foreign_database).unwrap().len();
        foreign_record.sha256 = format!(
            "sha256:{}",
            sha256_of_file(&File::open(&foreign_database).unwrap()).unwrap()
        );
        fs::write(
            &foreign_manifest,
            serde_json::to_vec(&foreign_record).unwrap(),
        )
        .unwrap();

        let mut bytes = fs::read(&database).unwrap();
        bytes[100..132].fill(0xa5);
        fs::write(&database, &bytes).unwrap();
        assert!(matches!(
            repository.inspect().unwrap_err(),
            AppError::DatabaseCorrupt { backups, .. } if backups.is_empty()
        ));
    }

    #[test]
    fn oversized_backup_manifest_is_never_advertised() {
        let temporary = tempdir().unwrap();
        let database = initialize_v1_database(&temporary);
        let repository = SqliteLibraryRepository::with_environment(
            Arc::new(TestEnvironment::with_roots(temporary.path())),
            Arc::new(XdgRootResolver),
        );
        repository.fix().unwrap();
        let backups_root = temporary.path().join("data/skilload/backups");
        let manifest = fs::read_dir(&backups_root)
            .unwrap()
            .flatten()
            .find(|entry| {
                entry
                    .path()
                    .extension()
                    .is_some_and(|extension| extension == "json")
            })
            .unwrap()
            .path();
        fs::write(&manifest, vec![b' '; MAX_BACKUP_MANIFEST_BYTES + 1]).unwrap();

        let mut bytes = fs::read(&database).unwrap();
        bytes[100..132].fill(0xa5);
        fs::write(&database, &bytes).unwrap();
        assert!(matches!(
            repository.inspect().unwrap_err(),
            AppError::DatabaseCorrupt { backups, .. } if backups.is_empty()
        ));
    }

    fn seed_foreign_valid_pair(backups_root: &Path, stem: &str, source: &Path) {
        let database = backups_root.join(format!("{stem}.db"));
        fs::copy(source, &database).unwrap();
        let bytes = fs::read(&database).unwrap();
        let digest = Sha256::digest(&bytes);
        let mut hex = String::new();
        for byte in digest {
            use std::fmt::Write as _;
            let _ = write!(hex, "{byte:02x}");
        }
        let record = BackupManifestRecord {
            format_version: BACKUP_MANIFEST_FORMAT_VERSION,
            source_schema: 1,
            target_schema: SCHEMA_VERSION,
            created_at_epoch_ns: 9_999_999_999_999_999_999,
            database_bytes: bytes.len() as u64,
            sha256: format!("sha256:{hex}"),
            source_device: 0,
            source_inode: 0,
            complete: true,
        };
        fs::write(
            backups_root.join(format!("{stem}.manifest.json")),
            serde_json::to_vec(&record).unwrap(),
        )
        .unwrap();
    }

    #[test]
    fn migration_retains_all_validated_backup_pairs() {
        let temporary = tempdir().unwrap();
        let database = initialize_v1_database(&temporary);
        let backups_root = temporary.path().join("data/skilload/backups");
        fs::create_dir_all(&backups_root).unwrap();
        // Three fully valid pairs that sort before the new migration backup
        // prove retention never deletes a pair based only on its pathname.
        for index in 0..3 {
            seed_foreign_valid_pair(
                &backups_root,
                &format!("skilload-db-v1-to-v2-0000000000000000000{index}"),
                &database,
            );
        }
        let repository = SqliteLibraryRepository::with_environment(
            Arc::new(TestEnvironment::with_roots(temporary.path())),
            Arc::new(XdgRootResolver),
        );
        let operation = repository.fix().unwrap();
        assert_eq!(operation.outcome.as_str(), "changed");
        assert_eq!(read_schema_version(&database), 2);
        let pairs = fs::read_dir(&backups_root)
            .unwrap()
            .flatten()
            .filter(|entry| entry.path().extension().is_some_and(|e| e == "db"))
            .count();
        assert_eq!(
            pairs, 4,
            "migration retains every validated pair when deletion cannot be identity-bound"
        );
        assert!(repository.inspect().unwrap().findings.is_empty());
    }

    #[test]
    fn migration_rechecks_state_after_acquiring_lock() {
        let temporary = tempdir().unwrap();
        let database = initialize_v1_database(&temporary);
        let repository = SqliteLibraryRepository::with_environment(
            Arc::new(TestEnvironment::with_roots(temporary.path())),
            Arc::new(XdgRootResolver),
        );
        let roots = repository.resolve_roots().unwrap();
        let (stale_diagnosis, _) = repository.diagnosis_classification(&roots).unwrap();
        assert!(matches!(stale_diagnosis, Diagnosis::RequiresMigration));

        assert_eq!(
            repository
                .migrate_v1(&roots)
                .unwrap()
                .expect("first migration must change schema")
                .kind
                .as_str(),
            "migrate"
        );
        assert!(
            repository.migrate_v1(&roots).unwrap().is_none(),
            "a v1 diagnosis that waits behind a completed migration is unchanged"
        );
        let (current_diagnosis, findings) = repository.diagnosis_classification(&roots).unwrap();
        assert!(matches!(current_diagnosis, Diagnosis::Healthy));
        assert!(findings.is_empty());
        assert_eq!(repository.fix().unwrap().outcome.as_str(), "unchanged");
        assert_eq!(read_schema_version(&database), 2);
    }

    #[test]
    fn fts_repair_rechecks_drift_under_the_lock() {
        let temporary = tempdir().unwrap();
        let database = temporary.path().join("data/skilload/skilload.db");
        let repository = imported_repository(&temporary, vec![searchable_entry("skills/review")]);
        let bytes_before = fs::read(&database).unwrap();
        let roots = repository.resolve_roots().unwrap();
        let action = repository.repair_fts(&roots).unwrap();
        assert!(
            action.is_none(),
            "a healthy index must not be rebuilt under the lock"
        );
        assert_eq!(fs::read(&database).unwrap(), bytes_before);
        assert_eq!(search(&repository, "review").len(), 1);

        Connection::open(&database)
            .unwrap()
            .execute("DELETE FROM library_fts", [])
            .unwrap();
        let action = repository.repair_fts(&roots).unwrap();
        assert_eq!(
            action.expect("drift must still be repaired").kind.as_str(),
            "repair"
        );
        assert_eq!(search(&repository, "review").len(), 1);
    }

    struct ReplaceDatabaseOnOpen;

    impl PersistenceHooks for ReplaceDatabaseOnOpen {
        fn after_existing_database_open(&self, database: &Path) -> Result<(), AppError> {
            let replacement = database.with_extension("replacement");
            fs::copy(database, &replacement).unwrap();
            fs::rename(&replacement, database).unwrap();
            Ok(())
        }
    }

    #[test]
    fn doctor_never_reports_a_replaced_database() {
        let temporary = tempdir().unwrap();
        imported_repository(&temporary, vec![searchable_entry("skills/review")]);
        let repository = SqliteLibraryRepository::with_hooks(
            Arc::new(TestEnvironment::with_roots(temporary.path())),
            Arc::new(XdgRootResolver),
            Arc::new(ReplaceDatabaseOnOpen),
        );
        assert!(matches!(
            repository.inspect(),
            Err(AppError::InvalidState { state, .. }) if state == "database_identity_drift"
        ));
        assert!(matches!(
            repository.fix(),
            Err(AppError::InvalidState { state, .. }) if state == "database_identity_drift"
        ));
    }

    #[test]
    fn mutation_paths_list_known_backups_on_base_corruption() {
        let temporary = tempdir().unwrap();
        let database = initialize_v1_database(&temporary);
        let repository = SqliteLibraryRepository::with_environment(
            Arc::new(TestEnvironment::with_roots(temporary.path())),
            Arc::new(XdgRootResolver),
        );
        repository.fix().unwrap();
        let mut bytes = fs::read(&database).unwrap();
        bytes[100..132].fill(0xa5);
        fs::write(&database, &bytes).unwrap();
        for error in [
            repository
                .mutate_metadata(&metadata_mutation(
                    "github:owner/repository#skills/review@refs/heads/main",
                    LibraryMetadataChange::NoteSet("changed".to_owned()),
                ))
                .unwrap_err(),
            repository
                .import(&document(vec![entry("skills/new", None)]), false)
                .unwrap_err(),
        ] {
            assert!(
                matches!(&error, AppError::DatabaseCorrupt { backups, .. } if backups.len() == 1),
                "corruption refusals must list the validated backup"
            );
        }
    }
    #[test]
    fn newer_schema_is_diagnosed_but_never_written() {
        let temporary = tempdir().unwrap();
        let database = initialize_v1_database(&temporary);
        Connection::open(&database)
            .unwrap()
            .execute("UPDATE schema_info SET version = 9", [])
            .unwrap();
        let repository = SqliteLibraryRepository::with_environment(
            Arc::new(TestEnvironment::with_roots(temporary.path())),
            Arc::new(XdgRootResolver),
        );

        assert_eq!(repository.export().unwrap().entries.len(), 1);
        let query = Query::new("review".to_owned()).unwrap();
        assert!(matches!(
            repository.search(&query, &page(100, 0)),
            Err(AppError::SchemaNewer {
                found_version: 9,
                ..
            })
        ));
        assert!(matches!(
            repository.list(&page(100, 0)),
            Err(AppError::SchemaNewer { .. })
        ));
        let diagnosis = repository.inspect().unwrap();
        assert_eq!(diagnosis.findings[0].code, "library_schema_newer");
        assert!(!diagnosis.findings[0].fixable_offline);
        assert!(!diagnosis.database_writable);
        let operation = repository.fix().unwrap();
        assert_eq!(operation.outcome.as_str(), "unchanged");
        assert_eq!(read_schema_version(&database), 9);
    }
    #[test]
    fn newer_schema_precedes_current_base_validation() {
        let temporary = tempdir().unwrap();
        let database = initialize_v1_database(&temporary);
        Connection::open(&database)
            .unwrap()
            .execute_batch(
                "UPDATE schema_info SET version = 9;
                 ALTER TABLE library_entries RENAME TO library_entries_v9;",
            )
            .unwrap();
        let repository = SqliteLibraryRepository::with_environment(
            Arc::new(TestEnvironment::with_roots(temporary.path())),
            Arc::new(XdgRootResolver),
        );
        let source = "github:owner/repository#skills/review@refs/heads/main";
        let query = Query::new("review".to_owned()).unwrap();

        for error in [
            repository.list(&page(100, 0)).unwrap_err(),
            repository.search(&query, &page(100, 0)).unwrap_err(),
            repository.get(source).unwrap_err(),
            repository
                .mutate_metadata(&metadata_mutation(
                    source,
                    LibraryMetadataChange::note_set("blocked".to_owned()).unwrap(),
                ))
                .unwrap_err(),
            repository
                .import(&document(vec![entry("skills/new", None)]), false)
                .unwrap_err(),
        ] {
            assert!(matches!(
                error,
                AppError::SchemaNewer {
                    found_version: 9,
                    ..
                }
            ));
        }

        let diagnosis = repository.inspect().unwrap();
        assert_eq!(diagnosis.findings[0].code, "library_schema_newer");
        assert_eq!(repository.fix().unwrap().outcome.as_str(), "unchanged");
    }
    struct ExistingDatabaseAbsenceAba {
        data_directory: PathBuf,
        displaced_directory: PathBuf,
    }

    impl PersistenceHooks for ExistingDatabaseAbsenceAba {
        fn before_existing_database_existence_probe(
            &self,
            _database: &Path,
        ) -> Result<(), AppError> {
            fs::rename(&self.data_directory, &self.displaced_directory).unwrap();
            fs::create_dir(&self.data_directory).unwrap();
            Ok(())
        }

        fn after_existing_database_existence_probe(
            &self,
            _database: &Path,
        ) -> Result<(), AppError> {
            fs::remove_dir(&self.data_directory).unwrap();
            fs::rename(&self.displaced_directory, &self.data_directory).unwrap();
            Ok(())
        }
    }

    #[test]
    fn database_existence_probe_uses_the_held_data_directory_after_an_aba_swap() {
        let temporary = tempdir().unwrap();
        let environment = Arc::new(TestEnvironment::with_roots(temporary.path()));
        let initial = SqliteLibraryRepository::with_environment(
            environment.clone(),
            Arc::new(XdgRootResolver),
        );
        initial
            .import(&document(vec![entry("skills/review", None)]), false)
            .unwrap();
        let data_directory = temporary.path().join("data/skilload");
        let repository = SqliteLibraryRepository::with_hooks(
            environment,
            Arc::new(XdgRootResolver),
            Arc::new(ExistingDatabaseAbsenceAba {
                data_directory: data_directory.clone(),
                displaced_directory: temporary.path().join("displaced-data-directory"),
            }),
        );

        let result = repository.list(&page(100, 0)).unwrap();

        assert_eq!(result.entries.len(), 1);
        assert!(data_directory.join("skilload.db").is_file());
    }

    #[test]
    fn backup_manifest_enumeration_failure_is_propagated() {
        let temporary = tempdir().unwrap();
        let backup_directory = temporary.path().join("backups");
        let non_directory = temporary.path().join("not-a-directory");
        fs::create_dir(&backup_directory).unwrap();
        File::create(&non_directory).unwrap();
        let directory = ValidatedDataDirectory {
            path: backup_directory.clone(),
            identity: metadata_identity(&fs::metadata(&backup_directory).unwrap()),
            handle: File::open(&non_directory).unwrap(),
        };

        let error = backup_manifest_stems(&directory).unwrap_err();

        assert!(matches!(
            error,
            AppError::InvalidEnvironment { variable, .. } if variable == "XDG_DATA_HOME"
        ));
    }
}
