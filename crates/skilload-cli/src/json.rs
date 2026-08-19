use crate::human::display_path;
use serde::Serialize;
use skilload_core::{
    AppError, ConfigEntries, ConfigEntry, ConfigValue, LibraryImportResult, NativePath,
    PortableLibraryDocument, SourceIdentity,
};
use std::os::unix::ffi::OsStrExt;

const API_VERSION: u8 = 2;

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
    Limit(LimitDetails),
    Conflict(ConflictDetails),
    Environment(EnvironmentDetails),
    Busy(BusyDetails),
    Schema(SchemaDetails),
    DatabaseCorrupt(DatabaseCorruptDetails),
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
struct LimitDetails {
    limit_kind: String,
    measured: String,
    allowed: String,
    source: Option<()>,
    source_path: Option<()>,
    path: Option<PathValue>,
}

#[derive(Serialize)]
struct ConflictDetails {
    conflicts: Vec<ConflictProjection>,
}

#[derive(Serialize)]
struct ConflictProjection {
    kind: String,
    name: Option<String>,
    agent: Option<()>,
    path: Option<PathValue>,
    source: Option<SourceIdentity>,
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
struct DatabaseCorruptDetails {
    database: PathValue,
    backups: Vec<PathValue>,
    recoverable_exports: Vec<String>,
    recovery_procedure: &'static str,
}

#[derive(Serialize)]
struct InvalidStateDetails {
    domain: String,
    state: String,
    path: Option<PathValue>,
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
        api_version: API_VERSION,
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
        api_version: API_VERSION,
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

pub fn library_import(
    operation: &'static str,
    outcome: &'static str,
    data: LibraryImportResult,
) -> serde_json::Result<Vec<u8>> {
    serde_json::to_vec(&SuccessEnvelope {
        api_version: API_VERSION,
        operation,
        ok: true,
        result: SuccessResult { outcome, data },
    })
}

pub fn library_export(
    operation: &'static str,
    document: PortableLibraryDocument,
) -> serde_json::Result<Vec<u8>> {
    serde_json::to_vec(&SuccessEnvelope {
        api_version: API_VERSION,
        operation,
        ok: true,
        result: SuccessResult {
            outcome: "observed",
            data: document,
        },
    })
}

pub fn error(operation: &'static str, error: &AppError) -> serde_json::Result<Vec<u8>> {
    serde_json::to_vec(&ErrorEnvelope {
        api_version: API_VERSION,
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
        AppError::LibraryInputLimit {
            limit_kind,
            measured,
            allowed,
            path,
        } => ErrorDetails::Limit(LimitDetails {
            limit_kind: limit_kind.clone(),
            measured: measured.to_string(),
            allowed: allowed.to_string(),
            source: None,
            source_path: None,
            path: Some(path_value(path)),
        }),
        AppError::Conflict { conflicts } => ErrorDetails::Conflict(ConflictDetails {
            conflicts: conflicts
                .iter()
                .map(|conflict| ConflictProjection {
                    kind: conflict.kind.clone(),
                    name: conflict.name.clone(),
                    agent: None,
                    path: None,
                    source: conflict.source.clone(),
                })
                .collect(),
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
            path,
            expected,
        } => ErrorDetails::InvalidState(InvalidStateDetails {
            domain: domain.clone(),
            state: state.clone(),
            path: path.as_ref().map(path_value),
            expected: expected.clone(),
        }),
        AppError::DatabaseCorrupt {
            database,
            backups,
            recoverable_exports,
        } => ErrorDetails::DatabaseCorrupt(DatabaseCorruptDetails {
            database: path_value(database),
            backups: backups.iter().map(path_value).collect(),
            recoverable_exports: recoverable_exports.clone(),
            recovery_procedure: "database-corruption-v1",
        }),
        AppError::Internal { incident_id } => ErrorDetails::Internal(InternalDetails {
            incident_id: incident_id.clone(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use skilload_core::{ConfigKey, DEFAULT_CACHE_LIMIT_BYTES, NativePath};
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;
    use std::path::PathBuf;

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
        assert_eq!(value["api_version"], 2);
        assert_eq!(value["operation"], "config.get");
        assert_eq!(
            value["result"]["data"]["entry"]["default_value"],
            "536870912"
        );
        assert!(value["result"]["data"]["entry"]["value"].is_null());
    }

    #[test]
    fn api_v2_error_paths_preserve_native_bytes() {
        let raw = b"/tmp/library-output-\xff.json";
        let error = AppError::validation(
            "library_export_io",
            Some(NativePath::new(PathBuf::from(OsString::from_vec(
                raw.to_vec(),
            )))),
        );
        let value: serde_json::Value =
            serde_json::from_slice(&super::error("library.export", &error).unwrap()).unwrap();

        assert_eq!(value["api_version"], 2);
        assert_eq!(value["error"]["code"], "validation_failed");
        assert_eq!(
            value["error"]["details"]["path"]["bytes_base64"],
            STANDARD.encode(raw)
        );
        assert!(value["error"]["details"].get("expected").is_none());
    }

    #[test]
    fn api_v2_invalid_state_paths_preserve_native_bytes() {
        let raw = b"/tmp/library-database-\xff.db";
        let error = AppError::invalid_state_at_path(
            "library_database",
            "sync_failed",
            NativePath::new(PathBuf::from(OsString::from_vec(raw.to_vec()))),
            ["a synced database"],
        );
        let value: serde_json::Value =
            serde_json::from_slice(&super::error("library.import", &error).unwrap()).unwrap();

        assert_eq!(value["error"]["code"], "invalid_state");
        assert_eq!(
            value["error"]["details"]["path"]["bytes_base64"],
            STANDARD.encode(raw)
        );
        assert_eq!(
            value["error"]["details"]["expected"][0],
            "a synced database"
        );
    }

    #[test]
    fn api_v2_library_limit_uses_its_dedicated_code() {
        let error = AppError::library_input_limit(
            "library_import_number_bytes",
            129,
            128,
            NativePath::new(PathBuf::from("/tmp/library-import.json")),
        );
        let value: serde_json::Value =
            serde_json::from_slice(&super::error("library.import", &error).unwrap()).unwrap();

        assert_eq!(value["api_version"], 2);
        assert_eq!(value["error"]["code"], "library_input_limit_exceeded");
        assert_eq!(
            value["error"]["details"]["limit_kind"],
            "library_import_number_bytes"
        );
        assert_eq!(value["error"]["details"]["measured"], "129");
        assert_eq!(value["error"]["details"]["allowed"], "128");
        assert_eq!(
            value["error"]["details"]["path"]["bytes_base64"],
            STANDARD.encode(b"/tmp/library-import.json")
        );
    }
}
