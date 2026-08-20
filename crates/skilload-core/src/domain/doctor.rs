use crate::domain::configuration::NativePath;
use crate::domain::source::SourceIdentity;

/// Severity of one doctor finding. `as_str` values are the API-v2
/// `DoctorFinding.severity` enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DoctorSeverity {
    Error,
    Warning,
    Info,
}

impl DoctorSeverity {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warning => "warning",
            Self::Info => "info",
        }
    }
}

/// Kind of one committed doctor action. This slice only ever emits the
/// stable `migrate` and `repair` kinds from the API-v2 `Action.kind` enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DoctorActionKind {
    Migrate,
    Repair,
}

impl DoctorActionKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Migrate => "migrate",
            Self::Repair => "repair",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoctorFinding {
    pub severity: DoctorSeverity,
    pub code: String,
    pub message: String,
    pub source: Option<SourceIdentity>,
    pub target: Option<NativePath>,
    /// Whether `doctor --fix` can resolve this finding offline in the
    /// current binary.
    pub fixable_offline: bool,
    /// Whether this specific invocation already repaired the finding.
    pub fixed: bool,
}

impl DoctorFinding {
    pub fn database(
        severity: DoctorSeverity,
        code: impl Into<String>,
        message: impl Into<String>,
        target: Option<NativePath>,
        fixable_offline: bool,
        fixed: bool,
    ) -> Self {
        Self {
            severity,
            code: code.into(),
            message: message.into(),
            source: None,
            target,
            fixable_offline,
            fixed,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoctorAction {
    pub kind: DoctorActionKind,
    /// Live durable database this action targeted.
    pub target: NativePath,
    /// Stable non-path state label before the action, such as `schema_1`
    /// or `fts_invalid`.
    pub before: Option<String>,
    /// Stable non-path state label after the action, such as `schema_2`
    /// or `fts_valid`.
    pub after: Option<String>,
}

/// Presentation-neutral doctor result. `database_writable` states whether
/// the current binary permits durable database mutations against the
/// observed state; it is not an operating-system permission probe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoctorData {
    pub fix_requested: bool,
    pub findings: Vec<DoctorFinding>,
    pub actions: Vec<DoctorAction>,
    pub database_writable: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DoctorOutcome {
    Observed,
    Changed,
    Unchanged,
}

impl DoctorOutcome {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Observed => "observed",
            Self::Changed => "changed",
            Self::Unchanged => "unchanged",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoctorOperation {
    pub outcome: DoctorOutcome,
    pub data: DoctorData,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doctor_enum_strings_match_the_api_v2_catalog() {
        assert_eq!(DoctorSeverity::Error.as_str(), "error");
        assert_eq!(DoctorSeverity::Warning.as_str(), "warning");
        assert_eq!(DoctorSeverity::Info.as_str(), "info");
        assert_eq!(DoctorActionKind::Migrate.as_str(), "migrate");
        assert_eq!(DoctorActionKind::Repair.as_str(), "repair");
        assert_eq!(DoctorOutcome::Observed.as_str(), "observed");
        assert_eq!(DoctorOutcome::Changed.as_str(), "changed");
        assert_eq!(DoctorOutcome::Unchanged.as_str(), "unchanged");
    }
}
