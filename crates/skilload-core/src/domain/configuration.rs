use crate::error::AppError;
use serde::Deserialize;
use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};

pub const CONFIG_SCHEMA_VERSION: u16 = 1;
pub const DEFAULT_CACHE_LIMIT_BYTES: u64 = 536_870_912;
pub const MAX_CACHE_LIMIT_BYTES: u64 = i64::MAX as u64;
const MAX_API_V1_UINT: u64 = 9_007_199_254_740_991;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NativePath(PathBuf);

impl NativePath {
    pub fn new(path: PathBuf) -> Self {
        Self(path)
    }

    pub fn as_path(&self) -> &Path {
        &self.0
    }

    pub fn into_path_buf(self) -> PathBuf {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConfigKey {
    CacheLimitBytes,
    ClaudeExecutable,
    CodexExecutable,
}

impl ConfigKey {
    pub const fn all() -> [Self; 3] {
        [
            Self::CacheLimitBytes,
            Self::ClaudeExecutable,
            Self::CodexExecutable,
        ]
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CacheLimitBytes => "cache_limit_bytes",
            Self::ClaudeExecutable => "agents.claude.executable",
            Self::CodexExecutable => "agents.codex.executable",
        }
    }

    pub fn parse(value: &str) -> Result<Self, AppError> {
        match value {
            "cache_limit_bytes" => Ok(Self::CacheLimitBytes),
            "agents.claude.executable" => Ok(Self::ClaudeExecutable),
            "agents.codex.executable" => Ok(Self::CodexExecutable),
            _ => Err(AppError::usage(
                "key",
                value,
                Self::all().into_iter().map(Self::as_str),
            )),
        }
    }

    pub const fn default_command(self) -> Option<&'static str> {
        match self {
            Self::CacheLimitBytes => None,
            Self::ClaudeExecutable => Some("claude"),
            Self::CodexExecutable => Some("codex"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigValue {
    CacheLimitBytes(u64),
    Executable(NativePath),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigEntry {
    pub key: ConfigKey,
    pub configured: bool,
    pub value: Option<ConfigValue>,
    pub default_value: Option<u64>,
    pub default_command: Option<&'static str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigEntries {
    pub schema_version: u16,
    pub entries: [ConfigEntry; 3],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MutationOutcome {
    Changed,
    Unchanged,
}

impl MutationOutcome {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Changed => "changed",
            Self::Unchanged => "unchanged",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigMutation {
    pub outcome: MutationOutcome,
    pub entry: ConfigEntry,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConfigDocument {
    cache_limit_bytes: Option<u64>,
    claude_executable: Option<NativePath>,
    codex_executable: Option<NativePath>,
}

impl ConfigDocument {
    pub fn from_toml(input: &str) -> Result<Self, AppError> {
        let raw: RawConfig = toml::from_str(input).map_err(|_| {
            AppError::invalid_state(
                "configuration",
                "invalid_toml",
                ["version = 1 with only supported configuration keys"],
            )
        })?;

        let found_version = u64::try_from(raw.version).map_err(|_| {
            AppError::invalid_state("configuration", "invalid_version", ["version = 1"])
        })?;
        if found_version > MAX_API_V1_UINT {
            return Err(AppError::invalid_state(
                "configuration",
                "invalid_version",
                ["version = 1 with an API-v1 UInt schema version"],
            ));
        }

        match found_version.cmp(&u64::from(CONFIG_SCHEMA_VERSION)) {
            std::cmp::Ordering::Less => {
                return Err(AppError::MigrationRequired {
                    domain: "configuration".to_owned(),
                    found_version,
                    supported_version: u64::from(CONFIG_SCHEMA_VERSION),
                });
            }
            std::cmp::Ordering::Greater => {
                return Err(AppError::SchemaNewer {
                    domain: "configuration".to_owned(),
                    found_version,
                    supported_version: u64::from(CONFIG_SCHEMA_VERSION),
                });
            }
            std::cmp::Ordering::Equal => {}
        }

        let cache_limit_bytes = match raw.cache_limit_bytes {
            Some(value) => Some(validate_cache_limit_from_i64(value)?),
            None => None,
        };
        let agents = raw.agents.unwrap_or_default();
        let claude_executable = agents
            .claude
            .and_then(|agent| agent.executable)
            .map(validate_document_executable)
            .transpose()?;
        let codex_executable = agents
            .codex
            .and_then(|agent| agent.executable)
            .map(validate_document_executable)
            .transpose()?;

        Ok(Self {
            cache_limit_bytes,
            claude_executable,
            codex_executable,
        })
    }

    pub fn to_toml(&self) -> String {
        let mut output = String::from("version = 1\n");
        if let Some(value) = self.cache_limit_bytes {
            output.push_str(&format!("cache_limit_bytes = {value}\n"));
        }
        if self.claude_executable.is_some() || self.codex_executable.is_some() {
            output.push('\n');
        }
        if let Some(value) = &self.claude_executable {
            output.push_str("[agents.claude]\nexecutable = ");
            output.push_str(&toml_string(executable_string(value)));
            output.push('\n');
            if self.codex_executable.is_some() {
                output.push('\n');
            }
        }
        if let Some(value) = &self.codex_executable {
            output.push_str("[agents.codex]\nexecutable = ");
            output.push_str(&toml_string(executable_string(value)));
            output.push('\n');
        }
        output
    }

    pub fn entry(&self, key: ConfigKey) -> ConfigEntry {
        match key {
            ConfigKey::CacheLimitBytes => ConfigEntry {
                key,
                configured: self.cache_limit_bytes.is_some(),
                value: self.cache_limit_bytes.map(ConfigValue::CacheLimitBytes),
                default_value: Some(DEFAULT_CACHE_LIMIT_BYTES),
                default_command: None,
            },
            ConfigKey::ClaudeExecutable => executable_entry(key, self.claude_executable.clone()),
            ConfigKey::CodexExecutable => executable_entry(key, self.codex_executable.clone()),
        }
    }

    pub fn entries(&self) -> ConfigEntries {
        ConfigEntries {
            schema_version: CONFIG_SCHEMA_VERSION,
            entries: ConfigKey::all().map(|key| self.entry(key)),
        }
    }

    pub fn set(&mut self, key: ConfigKey, raw_value: OsString) -> Result<(), AppError> {
        match key {
            ConfigKey::CacheLimitBytes => {
                self.cache_limit_bytes = Some(validate_cache_limit_raw(raw_value)?);
            }
            ConfigKey::ClaudeExecutable => {
                self.claude_executable = Some(validate_executable_raw(raw_value)?);
            }
            ConfigKey::CodexExecutable => {
                self.codex_executable = Some(validate_executable_raw(raw_value)?);
            }
        }
        Ok(())
    }

    pub fn unset(&mut self, key: ConfigKey) {
        match key {
            ConfigKey::CacheLimitBytes => self.cache_limit_bytes = None,
            ConfigKey::ClaudeExecutable => self.claude_executable = None,
            ConfigKey::CodexExecutable => self.codex_executable = None,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConfig {
    version: i64,
    #[serde(default)]
    cache_limit_bytes: Option<i64>,
    #[serde(default)]
    agents: Option<RawAgents>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawAgents {
    #[serde(default)]
    claude: Option<RawAgent>,
    #[serde(default)]
    codex: Option<RawAgent>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawAgent {
    #[serde(default)]
    executable: Option<String>,
}

fn executable_entry(key: ConfigKey, value: Option<NativePath>) -> ConfigEntry {
    ConfigEntry {
        key,
        configured: value.is_some(),
        value: value.map(ConfigValue::Executable),
        default_value: None,
        default_command: key.default_command(),
    }
}

fn validate_cache_limit_raw(raw_value: OsString) -> Result<u64, AppError> {
    let value = raw_value
        .into_string()
        .map_err(|_| AppError::validation("cache_limit_bytes_must_be_ascii_decimal", None))?;
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(AppError::validation(
            "cache_limit_bytes_must_be_ascii_decimal",
            None,
        ));
    }
    let parsed = value.parse::<u64>().map_err(|_| {
        AppError::validation(
            "cache_limit_bytes_must_be_between_1_and_9223372036854775807",
            None,
        )
    })?;
    validate_cache_limit(parsed)
}

fn validate_cache_limit_from_i64(value: i64) -> Result<u64, AppError> {
    validate_cache_limit(u64::try_from(value).map_err(|_| {
        AppError::validation(
            "cache_limit_bytes_must_be_between_1_and_9223372036854775807",
            None,
        )
    })?)
}

fn validate_cache_limit(value: u64) -> Result<u64, AppError> {
    if value == 0 || value > MAX_CACHE_LIMIT_BYTES {
        return Err(AppError::validation(
            "cache_limit_bytes_must_be_between_1_and_9223372036854775807",
            None,
        ));
    }
    Ok(value)
}

fn validate_document_executable(value: String) -> Result<NativePath, AppError> {
    validate_executable_raw(OsString::from(value))
}

fn validate_executable_raw(raw_value: OsString) -> Result<NativePath, AppError> {
    let original = PathBuf::from(&raw_value);
    let value = raw_value.into_string().map_err(|_| {
        AppError::validation(
            "executable_path_must_be_valid_utf8",
            Some(NativePath::new(original.clone())),
        )
    })?;
    if value.is_empty() {
        return Err(AppError::validation(
            "executable_path_must_be_nonempty_absolute_path",
            Some(NativePath::new(original)),
        ));
    }
    if value.contains('\0') {
        return Err(AppError::validation(
            "executable_path_must_not_contain_nul",
            Some(NativePath::new(original)),
        ));
    }
    let path = Path::new(&value);
    if !path.is_absolute() {
        return Err(AppError::validation(
            "executable_path_must_be_nonempty_absolute_path",
            Some(NativePath::new(path.to_path_buf())),
        ));
    }
    if looks_like_command_line(&value) {
        return Err(AppError::validation(
            "executable_path_must_not_include_command_arguments",
            Some(NativePath::new(path.to_path_buf())),
        ));
    }
    Ok(NativePath::new(normalize_absolute(path)))
}

fn looks_like_command_line(value: &str) -> bool {
    value
        .split_ascii_whitespace()
        .skip(1)
        .any(|part| part.starts_with('-'))
}

pub fn normalize_absolute(path: &Path) -> PathBuf {
    debug_assert!(path.is_absolute());
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                if normalized != Path::new("/") {
                    normalized.pop();
                }
            }
            Component::Normal(part) => normalized.push(part),
        }
    }
    if normalized.as_os_str().is_empty() {
        PathBuf::from("/")
    } else {
        normalized
    }
}

fn executable_string(value: &NativePath) -> &str {
    value
        .as_path()
        .to_str()
        .expect("validated executable paths are valid UTF-8")
}

fn toml_string(value: &str) -> String {
    toml::Value::String(value.to_owned()).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::ffi::OsStringExt;

    #[test]
    fn cache_limits_accept_the_documented_boundaries() {
        assert_eq!(validate_cache_limit_raw("1".into()).unwrap(), 1);
        assert_eq!(
            validate_cache_limit_raw("9223372036854775807".into()).unwrap(),
            i64::MAX as u64
        );
        for value in ["0", "-1", "+1", " 1", "9223372036854775808"] {
            assert!(validate_cache_limit_raw(value.into()).is_err(), "{value}");
        }
    }

    #[test]
    fn scalar_and_schema_validation_remain_api_v1_representable() {
        assert!(matches!(
            validate_cache_limit_raw(OsString::from_vec(vec![0xff])),
            Err(AppError::Validation { path: None, .. })
        ));
        assert!(matches!(
            ConfigDocument::from_toml("version = 9007199254740993\n"),
            Err(AppError::InvalidState { state, .. }) if state == "invalid_version"
        ));
    }

    #[test]
    fn executable_values_are_utf8_absolute_paths_without_probing() {
        let path = validate_executable_raw("/opt/claude/../bin/claude".into()).unwrap();
        assert_eq!(path.as_path(), Path::new("/opt/bin/claude"));
        assert!(validate_executable_raw("relative/claude".into()).is_err());
        assert!(validate_executable_raw(OsString::from_vec(vec![b'/', 0xff])).is_err());
        assert!(validate_executable_raw("/opt/\0claude".into()).is_err());
        assert!(validate_executable_raw("/usr/bin/claude --version".into()).is_err());
    }

    #[test]
    fn strict_toml_rejects_unknown_fields_without_canonicalizing_them() {
        assert!(ConfigDocument::from_toml("version = 1\nunknown = true\n").is_err());
        assert!(ConfigDocument::from_toml("version = 1\nversion = 1\n").is_err());
        assert!(ConfigDocument::from_toml("version = 2\n").is_err());
    }

    #[test]
    fn strict_toml_rejects_wrong_types_and_invalid_configured_paths() {
        for input in [
            "version = 1\ncache_limit_bytes = \"1\"\n",
            "version = 1\n[agents.claude]\nexecutable = 1\n",
            "version = 1\n[agents.claude]\nexecutable = \"relative/claude\"\n",
            "version = 1\n[agents.claude]\nexecutable = \"/opt/\\u0000claude\"\n",
            "version = 1\ncache_limit_bytes = 0\n",
        ] {
            assert!(ConfigDocument::from_toml(input).is_err(), "{input}");
        }
        assert!(matches!(
            ConfigDocument::from_toml("version = 0\n"),
            Err(AppError::MigrationRequired { .. })
        ));
        assert!(matches!(
            ConfigDocument::from_toml("version = 2\n"),
            Err(AppError::SchemaNewer { .. })
        ));
    }

    #[test]
    fn canonical_document_has_stable_order() {
        let mut document = ConfigDocument::default();
        document
            .set(ConfigKey::CodexExecutable, "/opt/codex".into())
            .unwrap();
        document
            .set(ConfigKey::CacheLimitBytes, "1".into())
            .unwrap();
        document
            .set(ConfigKey::ClaudeExecutable, "/opt/claude".into())
            .unwrap();
        assert_eq!(
            document.to_toml(),
            "version = 1\ncache_limit_bytes = 1\n\n[agents.claude]\nexecutable = \"/opt/claude\"\n\n[agents.codex]\nexecutable = \"/opt/codex\"\n"
        );
    }
}
