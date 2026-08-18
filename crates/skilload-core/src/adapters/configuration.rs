use crate::adapters::xdg::{SystemEnvironment, XdgRootResolver};
use crate::domain::configuration::{ConfigDocument, NativePath};
use crate::error::AppError;
use crate::ports::configuration::{
    ConfigBaseline, ConfigSource, ConfigurationStore, Environment, LoadedConfig, ResolvedRoots,
    StateRootResolver, StoreOutcome,
};
use std::fs::{self, DirBuilder, File, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use tempfile::Builder;

const LOCK_WAIT: Duration = Duration::from_secs(2);
const LOCK_RETRY: Duration = Duration::from_millis(25);

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
        Ok(self.resolve_roots()? == *expected)
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
        let state_root = &roots.state.effective;
        ensure_restrictive_directory(state_root, "XDG_STATE_HOME")?;
        let locks = state_root.join("locks");
        ensure_restrictive_directory(&locks, "XDG_STATE_HOME")?;
        let lock_path = locks.join("config.lock");
        if let Ok(metadata) = fs::symlink_metadata(&lock_path)
            && (metadata.file_type().is_symlink() || !metadata.file_type().is_file())
        {
            return Err(AppError::invalid_state(
                "configuration_lock",
                "lock_path_is_not_a_regular_file",
                ["a regular config.lock file"],
            ));
        }
        let file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .mode(0o600)
            .open(&lock_path)
            .map_err(|error| {
                environment_io("XDG_STATE_HOME", &lock_path, "open config lock", error)
            })?;
        file.set_permissions(fs::Permissions::from_mode(0o600))
            .map_err(|error| {
                environment_io("XDG_STATE_HOME", &lock_path, "restrict config lock", error)
            })?;

        let start = Instant::now();
        loop {
            match file.try_lock() {
                Ok(()) => return Ok(file),
                Err(std::fs::TryLockError::WouldBlock) => {
                    if start.elapsed() >= LOCK_WAIT {
                        return Err(AppError::Busy {
                            lock_domain: "configuration".to_owned(),
                            waited_ms: LOCK_WAIT.as_millis() as u64,
                        });
                    }
                    thread::sleep(LOCK_RETRY);
                }
                Err(std::fs::TryLockError::Error(error)) => {
                    return Err(environment_io(
                        "XDG_STATE_HOME",
                        &lock_path,
                        "lock config.lock",
                        error,
                    ));
                }
            }
        }
    }

    fn write_document(
        &self,
        roots: &ResolvedRoots,
        desired: &ConfigDocument,
    ) -> Result<(), AppError> {
        let config_root = &roots.config.effective;
        ensure_restrictive_directory(config_root, "XDG_CONFIG_HOME")?;
        self.root_resolver.revalidate(roots)?;
        let path = config_path(roots);
        ensure_regular_destination(&path)?;

        let mut staging = Builder::new()
            .prefix(".skilload-config-")
            .suffix(".tmp")
            .tempfile_in(config_root)
            .map_err(|error| {
                environment_io(
                    "XDG_CONFIG_HOME",
                    config_root,
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
        self.root_resolver.revalidate(roots)?;
        ensure_regular_destination(&path)?;
        staging.persist(&path).map_err(|error| {
            environment_io(
                "XDG_CONFIG_HOME",
                &path,
                "atomically replace config.toml",
                error.error,
            )
        })?;
        File::open(config_root)
            .and_then(|directory| directory.sync_all())
            .map_err(|error| {
                environment_io(
                    "XDG_CONFIG_HOME",
                    config_root,
                    "sync config directory",
                    error,
                )
            })?;
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
        self.root_resolver.revalidate(&expected.roots)?;
        let lock = self.lock(&expected.roots)?;
        let result = (|| {
            if !self.roots_still_match(&expected.roots)? {
                return Ok(StoreOutcome::Stale);
            }
            self.root_resolver.revalidate(&expected.roots)?;
            let current = self.load_from_roots(expected.roots.clone())?;
            if current.baseline.source != expected.source {
                return Ok(StoreOutcome::Stale);
            }
            self.write_document(&expected.roots, desired)?;
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

fn ensure_restrictive_directory(path: &Path, variable: &str) -> Result<(), AppError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || !metadata.file_type().is_dir() {
                return Err(AppError::invalid_environment(
                    variable,
                    Some(NativePath::new(path.to_path_buf())),
                    "expected a real directory",
                ));
            }
            Ok(())
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let mut builder = DirBuilder::new();
            builder.recursive(true).mode(0o700);
            builder.create(path).map_err(|error| {
                environment_io(variable, path, "create restrictive directory", error)
            })?;
            let metadata = fs::symlink_metadata(path).map_err(|error| {
                environment_io(variable, path, "inspect created directory", error)
            })?;
            if metadata.file_type().is_symlink() || !metadata.file_type().is_dir() {
                return Err(AppError::invalid_environment(
                    variable,
                    Some(NativePath::new(path.to_path_buf())),
                    "created path is not a real directory",
                ));
            }
            Ok(())
        }
        Err(error) => Err(environment_io(variable, path, "inspect directory", error)),
    }
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

fn environment_io(variable: &str, path: &Path, action: &str, error: io::Error) -> AppError {
    AppError::invalid_environment(
        variable,
        Some(NativePath::new(path.to_path_buf())),
        format!("cannot {action}: {error}"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
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

    #[test]
    fn absent_queries_and_unsets_create_no_roots() {
        let temporary = tempdir().unwrap();
        let application = Application::new(store(temporary.path()));
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
        let application = Application::new(store(temporary.path()));
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
    fn invalid_documents_and_final_symlinks_are_preserved() {
        let temporary = tempdir().unwrap();
        let config_root = temporary.path().join("config/skilload");
        fs::create_dir_all(&config_root).unwrap();
        let config = config_root.join("config.toml");
        fs::write(&config, "version = 1\nunknown = true\n").unwrap();
        let original = fs::read(&config).unwrap();
        let application = Application::new(store(temporary.path()));
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
        let application = Application::new(store);
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
