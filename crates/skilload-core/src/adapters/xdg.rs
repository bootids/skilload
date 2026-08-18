use crate::domain::configuration::{NativePath, normalize_absolute};
use crate::error::AppError;
use crate::ports::configuration::{Environment, ResolvedRoots, RootBinding, StateRootResolver};
use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::{Component, Path, PathBuf};

pub struct SystemEnvironment;

impl Environment for SystemEnvironment {
    fn var_os(&self, key: &str) -> Option<OsString> {
        env::var_os(key)
    }
}

#[derive(Debug, Default)]
pub struct XdgRootResolver;

impl StateRootResolver for XdgRootResolver {
    fn resolve(&self, environment: &dyn Environment) -> Result<ResolvedRoots, AppError> {
        let config = resolve_root(environment, "XDG_CONFIG_HOME", ".config")?;
        let data = resolve_root(environment, "XDG_DATA_HOME", ".local/share")?;
        let state = resolve_root(environment, "XDG_STATE_HOME", ".local/state")?;
        let cache = resolve_root(environment, "XDG_CACHE_HOME", ".cache")?;
        let roots = ResolvedRoots {
            config,
            data,
            state,
            cache,
        };
        ensure_disjoint(&roots)?;
        Ok(roots)
    }

    fn revalidate(&self, roots: &ResolvedRoots) -> Result<(), AppError> {
        for (variable, binding) in root_bindings(roots) {
            let effective = resolve_existing_prefix(&binding.logical, variable)?;
            if effective != binding.effective {
                return Err(AppError::invalid_environment(
                    variable,
                    Some(NativePath::new(binding.logical.clone())),
                    "the resolved root identity changed",
                ));
            }
        }
        ensure_disjoint(roots)
    }
}

fn resolve_root(
    environment: &dyn Environment,
    variable: &'static str,
    home_suffix: &'static str,
) -> Result<RootBinding, AppError> {
    let base = match environment
        .var_os(variable)
        .and_then(|value| valid_absolute(value).map(|path| normalize_absolute(&path)))
    {
        Some(base) => base,
        None => fallback_home(environment, variable, home_suffix)?,
    };
    let logical = normalize_absolute(&base.join("skilload"));
    let effective = resolve_existing_prefix(&logical, variable)?;
    Ok(RootBinding { logical, effective })
}

fn valid_absolute(value: OsString) -> Option<PathBuf> {
    if value.is_empty() {
        return None;
    }
    let path = PathBuf::from(value);
    path.is_absolute().then_some(path)
}

fn fallback_home(
    environment: &dyn Environment,
    variable: &'static str,
    home_suffix: &'static str,
) -> Result<PathBuf, AppError> {
    let Some(home) = environment.var_os("HOME") else {
        return Err(AppError::invalid_environment(
            "HOME",
            None,
            format!("a nonempty absolute HOME is required because {variable} is absent"),
        ));
    };
    let home = PathBuf::from(home);
    if home.as_os_str().is_empty() || !home.is_absolute() {
        return Err(AppError::invalid_environment(
            "HOME",
            Some(NativePath::new(home)),
            format!("a nonempty absolute HOME is required because {variable} is absent"),
        ));
    }
    Ok(normalize_absolute(&home.join(home_suffix)))
}

fn resolve_existing_prefix(path: &Path, variable: &str) -> Result<PathBuf, AppError> {
    let components: Vec<_> = path.components().collect();
    let mut current = PathBuf::new();
    for (index, component) in components.iter().enumerate() {
        match component {
            Component::Prefix(prefix) => current.push(prefix.as_os_str()),
            Component::RootDir => current.push(component.as_os_str()),
            Component::CurDir | Component::ParentDir => {
                return Err(AppError::invalid_environment(
                    variable,
                    Some(NativePath::new(path.to_path_buf())),
                    "root normalization left a relative component",
                ));
            }
            Component::Normal(part) => {
                let candidate = current.join(part);
                match fs::symlink_metadata(&candidate) {
                    Ok(_) => {
                        current = fs::canonicalize(&candidate).map_err(|error| {
                            AppError::invalid_environment(
                                variable,
                                Some(NativePath::new(candidate.clone())),
                                format!("cannot resolve an existing root prefix: {error}"),
                            )
                        })?;
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                        current.push(part);
                        for remaining in components.iter().skip(index + 1) {
                            if let Component::Normal(next) = remaining {
                                current.push(next);
                            }
                        }
                        return Ok(current);
                    }
                    Err(error) => {
                        return Err(AppError::invalid_environment(
                            variable,
                            Some(NativePath::new(candidate)),
                            format!("cannot inspect an existing root prefix: {error}"),
                        ));
                    }
                }
            }
        }
    }
    Ok(current)
}

fn root_bindings(roots: &ResolvedRoots) -> [(&'static str, &RootBinding); 4] {
    [
        ("XDG_CONFIG_HOME", &roots.config),
        ("XDG_DATA_HOME", &roots.data),
        ("XDG_STATE_HOME", &roots.state),
        ("XDG_CACHE_HOME", &roots.cache),
    ]
}

fn ensure_disjoint(roots: &ResolvedRoots) -> Result<(), AppError> {
    let bindings = root_bindings(roots);
    for (index, (left_name, left)) in bindings.iter().enumerate() {
        for (right_name, right) in bindings.iter().skip(index + 1) {
            if left.effective.starts_with(&right.effective)
                || right.effective.starts_with(&left.effective)
            {
                return Err(AppError::OverlappingStateRoots {
                    variable: (*left_name).to_owned(),
                    path: Some(NativePath::new(left.effective.clone())),
                    reason: format!(
                        "{} and {} resolve to equal or nested application roots",
                        left_name, right_name
                    ),
                });
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::sync::Arc;
    use tempfile::tempdir;

    #[derive(Default)]
    struct TestEnvironment(BTreeMap<String, OsString>);

    impl TestEnvironment {
        fn with(mut self, key: &str, value: impl Into<OsString>) -> Self {
            self.0.insert(key.to_owned(), value.into());
            self
        }
    }

    impl Environment for TestEnvironment {
        fn var_os(&self, key: &str) -> Option<OsString> {
            self.0.get(key).cloned()
        }
    }

    #[test]
    fn relative_xdg_values_fall_back_without_using_the_current_directory() {
        let temporary = tempdir().unwrap();
        let home = temporary.path().join("home");
        let environment = TestEnvironment::default()
            .with("HOME", home.clone())
            .with("XDG_CONFIG_HOME", ".config-from-cwd")
            .with("XDG_DATA_HOME", ".data-from-cwd")
            .with("XDG_STATE_HOME", ".state-from-cwd")
            .with("XDG_CACHE_HOME", ".cache-from-cwd");
        let roots = XdgRootResolver.resolve(&environment).unwrap();
        let canonical_temporary = fs::canonicalize(temporary.path()).unwrap();
        assert_eq!(
            roots.config.effective,
            canonical_temporary.join("home/.config/skilload")
        );
        assert_eq!(
            roots.data.effective,
            canonical_temporary.join("home/.local/share/skilload")
        );
        assert!(!temporary.path().join(".config-from-cwd").exists());
    }

    #[test]
    fn equal_and_symlink_aliased_roots_are_rejected_before_state_access() {
        let temporary = tempdir().unwrap();
        let shared = temporary.path().join("shared");
        fs::create_dir_all(&shared).unwrap();
        let alias = temporary.path().join("alias");
        std::os::unix::fs::symlink(&shared, &alias).unwrap();
        let environment = TestEnvironment::default()
            .with("XDG_CONFIG_HOME", shared)
            .with("XDG_DATA_HOME", alias)
            .with("XDG_STATE_HOME", temporary.path().join("state"))
            .with("XDG_CACHE_HOME", temporary.path().join("cache"));
        assert!(matches!(
            XdgRootResolver.resolve(&environment),
            Err(AppError::OverlappingStateRoots { .. })
        ));
    }

    #[test]
    fn invalid_home_is_rejected_when_a_fallback_is_needed() {
        let environment = TestEnvironment::default().with("HOME", "relative-home");
        assert!(matches!(
            XdgRootResolver.resolve(&environment),
            Err(AppError::InvalidEnvironment { variable, .. }) if variable == "HOME"
        ));
    }

    #[test]
    fn empty_xdg_values_use_the_documented_home_fallbacks() {
        let temporary = tempdir().unwrap();
        let home = temporary.path().join("home");
        fs::create_dir_all(&home).unwrap();
        let environment = TestEnvironment::default()
            .with("HOME", home)
            .with("XDG_CONFIG_HOME", "")
            .with("XDG_DATA_HOME", "")
            .with("XDG_STATE_HOME", "")
            .with("XDG_CACHE_HOME", "");
        let roots = XdgRootResolver.resolve(&environment).unwrap();
        let canonical_home = fs::canonicalize(temporary.path()).unwrap().join("home");
        assert_eq!(
            roots.config.effective,
            canonical_home.join(".config/skilload")
        );
        assert_eq!(
            roots.data.effective,
            canonical_home.join(".local/share/skilload")
        );
        assert_eq!(
            roots.state.effective,
            canonical_home.join(".local/state/skilload")
        );
        assert_eq!(
            roots.cache.effective,
            canonical_home.join(".cache/skilload")
        );
    }

    #[test]
    fn every_root_pair_rejects_equal_or_nested_effective_roots() {
        let temporary = tempdir().unwrap();
        let names = [
            "XDG_CONFIG_HOME",
            "XDG_DATA_HOME",
            "XDG_STATE_HOME",
            "XDG_CACHE_HOME",
        ];
        for left in 0..names.len() {
            for right in left + 1..names.len() {
                let mut equal =
                    TestEnvironment::default().with("HOME", temporary.path().join("home"));
                for (index, name) in names.iter().enumerate() {
                    equal.0.insert(
                        (*name).to_owned(),
                        temporary
                            .path()
                            .join(format!("root-{index}"))
                            .into_os_string(),
                    );
                }
                let shared = temporary.path().join(format!("root-{left}"));
                equal
                    .0
                    .insert(names[right].to_owned(), shared.clone().into_os_string());
                assert!(matches!(
                    XdgRootResolver.resolve(&equal),
                    Err(AppError::OverlappingStateRoots { .. })
                ));

                let mut nested = equal;
                nested.0.insert(
                    names[right].to_owned(),
                    shared.join("skilload/nested").into_os_string(),
                );
                assert!(matches!(
                    XdgRootResolver.resolve(&nested),
                    Err(AppError::OverlappingStateRoots { .. })
                ));
            }
        }
    }

    #[test]
    fn revalidation_detects_a_root_symlink_swap() {
        let temporary = tempdir().unwrap();
        let one = temporary.path().join("one");
        let two = temporary.path().join("two");
        fs::create_dir_all(&one).unwrap();
        fs::create_dir_all(&two).unwrap();
        let config = temporary.path().join("config");
        std::os::unix::fs::symlink(&one, &config).unwrap();
        let environment = Arc::new(
            TestEnvironment::default()
                .with("XDG_CONFIG_HOME", config.clone())
                .with("XDG_DATA_HOME", temporary.path().join("data"))
                .with("XDG_STATE_HOME", temporary.path().join("state"))
                .with("XDG_CACHE_HOME", temporary.path().join("cache")),
        );
        let resolver = XdgRootResolver;
        let roots = resolver.resolve(environment.as_ref()).unwrap();
        fs::remove_file(&config).unwrap();
        std::os::unix::fs::symlink(&two, &config).unwrap();
        assert!(resolver.revalidate(&roots).is_err());
    }
}
