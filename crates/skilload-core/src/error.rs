use crate::domain::configuration::NativePath;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum AppError {
    #[error("invalid argument")]
    Usage {
        argument: Option<String>,
        value: Option<String>,
        path: Option<NativePath>,
        expected: Vec<String>,
    },
    #[error("configuration validation failed: {constraint}")]
    Validation {
        constraint: String,
        path: Option<NativePath>,
    },
    #[error("invalid environment path for {variable}: {reason}")]
    InvalidEnvironment {
        variable: String,
        path: Option<NativePath>,
        reason: String,
    },
    #[error("state roots overlap: {reason}")]
    OverlappingStateRoots {
        variable: String,
        path: Option<NativePath>,
        reason: String,
    },
    #[error("configuration lock is busy")]
    Busy { lock_domain: String, waited_ms: u64 },
    #[error(
        "configuration schema {found_version} is newer than supported schema {supported_version}"
    )]
    SchemaNewer {
        domain: String,
        found_version: u64,
        supported_version: u64,
    },
    #[error(
        "configuration schema {found_version} requires migration to schema {supported_version}"
    )]
    MigrationRequired {
        domain: String,
        found_version: u64,
        supported_version: u64,
    },
    #[error("invalid {domain} state: {state}")]
    InvalidState {
        domain: String,
        state: String,
        expected: Vec<String>,
    },
    #[error("internal invariant failed: {incident_id}")]
    Internal { incident_id: String },
}

impl AppError {
    pub fn usage(
        argument: impl Into<String>,
        expected: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self::Usage {
            argument: Some(argument.into()),
            value: None,
            path: None,
            expected: expected.into_iter().map(Into::into).collect(),
        }
    }

    pub fn validation(constraint: impl Into<String>, path: Option<NativePath>) -> Self {
        Self::Validation {
            constraint: constraint.into(),
            path,
        }
    }

    pub fn invalid_environment(
        variable: impl Into<String>,
        path: Option<NativePath>,
        reason: impl Into<String>,
    ) -> Self {
        Self::InvalidEnvironment {
            variable: variable.into(),
            path,
            reason: reason.into(),
        }
    }

    pub fn invalid_state(
        domain: impl Into<String>,
        state: impl Into<String>,
        expected: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self::InvalidState {
            domain: domain.into(),
            state: state.into(),
            expected: expected.into_iter().map(Into::into).collect(),
        }
    }

    pub fn code(&self) -> &'static str {
        match self {
            Self::Usage { .. } => "usage_error",
            Self::Validation { .. } => "validation_failed",
            Self::InvalidEnvironment { .. } => "invalid_environment_path",
            Self::OverlappingStateRoots { .. } => "overlapping_state_roots",
            Self::Busy { .. } => "busy",
            Self::SchemaNewer { .. } => "schema_newer",
            Self::MigrationRequired { .. } => "migration_required",
            Self::InvalidState { .. } => "invalid_state",
            Self::Internal { .. } => "internal_invariant",
        }
    }

    pub fn exit_code(&self) -> u8 {
        match self {
            Self::Usage { .. } => 2,
            Self::Validation { .. }
            | Self::InvalidEnvironment { .. }
            | Self::OverlappingStateRoots { .. }
            | Self::InvalidState { .. } => 4,
            Self::Busy { .. } => 5,
            Self::SchemaNewer { .. } | Self::MigrationRequired { .. } | Self::Internal { .. } => 6,
        }
    }
}
