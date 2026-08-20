use crate::domain::doctor::{DoctorData, DoctorOperation};
use crate::error::AppError;

/// Focused maintenance port for the current durable database. `inspect` is
/// read-only and filesystem-inert; `fix` is the only path that acquires the
/// durable mutation lock to migrate a supported older schema after a
/// validated standalone backup or to rebuild derived FTS state.
pub trait DatabaseMaintenance: Send + Sync {
    fn inspect(&self) -> Result<DoctorData, AppError>;
    fn fix(&self) -> Result<DoctorOperation, AppError>;
}
