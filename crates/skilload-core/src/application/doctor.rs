use super::Application;
use crate::domain::doctor::{DoctorOperation, DoctorOutcome};
use crate::error::AppError;

impl Application {
    pub fn doctor(&self, fix: bool) -> Result<DoctorOperation, AppError> {
        if fix {
            self.database_maintenance.fix()
        } else {
            Ok(DoctorOperation {
                outcome: DoctorOutcome::Observed,
                data: self.database_maintenance.inspect()?,
            })
        }
    }
}
