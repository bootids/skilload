use crate::human::display_path;
use serde::Serialize;
use skilload_core::{AppError, ConfigEntries, ConfigEntry, ConfigValue, NativePath};
use std::os::unix::ffi::OsStrExt;

#[derive(Serialize)]
struct SuccessEnvelope<T: Serialize> {
    api_version: u8,
    operation: &'static str,
    ok: bool,
    result: SuccessResult<T>,
}

#[derive(Serialize)]
struct SuccessResult<T: Serialize> {
    outcome: &'static str,
    data: T,
}

#[derive(Serialize)]
struct ErrorEnvelope {
    api_version: u8,
    operation: &'static str,
    ok: bool,
    error: ErrorBody,
}

#[derive(Serialize)]
struct ErrorBody {
    code: &'static str,
    message: String,
    details: ErrorDetails,
}

#[derive(Serialize)]
#[serde(untagged)]
enum ErrorDetails {
    Usage(UsageDetails),
    Validation(ValidationDetails),
    Environment(EnvironmentDetails),
    Busy(BusyDetails),
    Schema(SchemaDetails),
    InvalidState(InvalidStateDetails),
    Internal(InternalDetails),
}

#[derive(Serialize)]
struct UsageDetails {
    argument: Option<String>,
    value: Option<String>,
    path: Option<PathValue>,
    expected: Vec<String>,
}

#[derive(Serialize)]
struct ValidationDetails {
    constraint: String,
    source: Option<()>,
    source_path: Option<()>,
    path: Option<PathValue>,
}

#[derive(Serialize)]
struct EnvironmentDetails {
    variable: String,
    path: Option<PathValue>,
    reason: String,
}

#[derive(Serialize)]
struct BusyDetails {
    lock_domain: String,
    waited_ms: u64,
}

#[derive(Serialize)]
struct SchemaDetails {
    domain: String,
    found_version: u64,
    supported_version: u64,
}

#[derive(Serialize)]
struct InvalidStateDetails {
    domain: String,
    state: String,
    expected: Vec<String>,
}

#[derive(Serialize)]
struct InternalDetails {
    incident_id: String,
}

#[derive(Serialize)]
pub struct PathValue {
    display: String,
    bytes_base64: String,
}

#[derive(Serialize)]
struct ConfigEntryData {
    schema_version: u16,
    entry: ConfigEntryProjection,
}

#[derive(Serialize)]
struct ConfigEntriesData {
    schema_version: u16,
    entries: Vec<ConfigEntryProjection>,
}

#[derive(Serialize)]
struct ConfigEntryProjection {
    key: &'static str,
    configured: bool,
    value: Option<ConfigValueProjection>,
    default_value: Option<String>,
    default_command: Option<&'static str>,
}

#[derive(Serialize)]
#[serde(untagged)]
enum ConfigValueProjection {
    Decimal(String),
    Path(PathValue),
}

pub fn entry(
    operation: &'static str,
    outcome: &'static str,
    entry: ConfigEntry,
) -> serde_json::Result<Vec<u8>> {
    serde_json::to_vec(&SuccessEnvelope {
        api_version: 1,
        operation,
        ok: true,
        result: SuccessResult {
            outcome,
            data: ConfigEntryData {
                schema_version: 1,
                entry: entry.into(),
            },
        },
    })
}

pub fn entries(operation: &'static str, entries: ConfigEntries) -> serde_json::Result<Vec<u8>> {
    serde_json::to_vec(&SuccessEnvelope {
        api_version: 1,
        operation,
        ok: true,
        result: SuccessResult {
            outcome: "observed",
            data: ConfigEntriesData {
                schema_version: entries.schema_version,
                entries: entries.entries.into_iter().map(Into::into).collect(),
            },
        },
    })
}

pub fn error(operation: &'static str, error: &AppError) -> serde_json::Result<Vec<u8>> {
    serde_json::to_vec(&ErrorEnvelope {
        api_version: 1,
        operation,
        ok: false,
        error: ErrorBody {
            code: error.code(),
            message: error.to_string(),
            details: error_details(error),
        },
    })
}

impl From<ConfigEntry> for ConfigEntryProjection {
    fn from(entry: ConfigEntry) -> Self {
        let value = entry.value.map(|value| match value {
            ConfigValue::CacheLimitBytes(value) => {
                ConfigValueProjection::Decimal(value.to_string())
            }
            ConfigValue::Executable(path) => ConfigValueProjection::Path(path_value(&path)),
        });
        Self {
            key: entry.key.as_str(),
            configured: entry.configured,
            value,
            default_value: entry.default_value.map(|value| value.to_string()),
            default_command: entry.default_command,
        }
    }
}

pub fn path_value(path: &NativePath) -> PathValue {
    use base64::{Engine as _, engine::general_purpose::STANDARD};

    let bytes = path.as_path().as_os_str().as_bytes();
    PathValue {
        display: display_path(path),
        bytes_base64: STANDARD.encode(bytes),
    }
}

fn error_details(error: &AppError) -> ErrorDetails {
    match error {
        AppError::Usage {
            argument,
            value,
            path,
            expected,
        } => ErrorDetails::Usage(UsageDetails {
            argument: argument.clone(),
            value: value.clone(),
            path: path.as_ref().map(path_value),
            expected: expected.clone(),
        }),
        AppError::Validation { constraint, path } => ErrorDetails::Validation(ValidationDetails {
            constraint: constraint.clone(),
            source: None,
            source_path: None,
            path: path.as_ref().map(path_value),
        }),
        AppError::InvalidEnvironment {
            variable,
            path,
            reason,
        }
        | AppError::OverlappingStateRoots {
            variable,
            path,
            reason,
        } => ErrorDetails::Environment(EnvironmentDetails {
            variable: variable.clone(),
            path: path.as_ref().map(path_value),
            reason: reason.clone(),
        }),
        AppError::Busy {
            lock_domain,
            waited_ms,
        } => ErrorDetails::Busy(BusyDetails {
            lock_domain: lock_domain.clone(),
            waited_ms: *waited_ms,
        }),
        AppError::SchemaNewer {
            domain,
            found_version,
            supported_version,
        }
        | AppError::MigrationRequired {
            domain,
            found_version,
            supported_version,
        } => ErrorDetails::Schema(SchemaDetails {
            domain: domain.clone(),
            found_version: *found_version,
            supported_version: *supported_version,
        }),
        AppError::InvalidState {
            domain,
            state,
            expected,
        } => ErrorDetails::InvalidState(InvalidStateDetails {
            domain: domain.clone(),
            state: state.clone(),
            expected: expected.clone(),
        }),
        AppError::Internal { incident_id } => ErrorDetails::Internal(InternalDetails {
            incident_id: incident_id.clone(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use skilload_core::{ConfigKey, DEFAULT_CACHE_LIMIT_BYTES};

    #[test]
    fn default_cache_get_matches_the_api_shape() {
        let config_entry = ConfigEntry {
            key: ConfigKey::CacheLimitBytes,
            configured: false,
            value: None,
            default_value: Some(DEFAULT_CACHE_LIMIT_BYTES),
            default_command: None,
        };
        let value: serde_json::Value =
            serde_json::from_slice(&entry("config.get", "observed", config_entry).unwrap())
                .unwrap();
        assert_eq!(value["api_version"], 1);
        assert_eq!(value["operation"], "config.get");
        assert_eq!(
            value["result"]["data"]["entry"]["default_value"],
            "536870912"
        );
        assert!(value["result"]["data"]["entry"]["value"].is_null());
    }
}
