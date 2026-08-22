use crate::domain::configuration::{
    ConfigDocument, ConfigEntries, ConfigEntry, ConfigKey, ConfigMutation, MutationOutcome,
};
use crate::error::AppError;
use crate::ports::configuration::{ConfigurationStore, StoreOutcome};
use crate::ports::doctor::DatabaseMaintenance;
use crate::ports::library::{LibraryRepository, LibraryTransferStore};
use std::ffi::OsString;
use std::sync::Arc;

pub struct Application {
    pub(super) configuration_store: Arc<dyn ConfigurationStore>,
    pub(super) library_repository: Arc<dyn LibraryRepository>,
    pub(super) library_transfer_store: Arc<dyn LibraryTransferStore>,
    pub(super) database_maintenance: Arc<dyn DatabaseMaintenance>,
}

impl Application {
    pub fn new(
        configuration_store: Arc<dyn ConfigurationStore>,
        library_repository: Arc<dyn LibraryRepository>,
        library_transfer_store: Arc<dyn LibraryTransferStore>,
        database_maintenance: Arc<dyn DatabaseMaintenance>,
    ) -> Self {
        Self {
            configuration_store,
            library_repository,
            library_transfer_store,
            database_maintenance,
        }
    }

    pub fn config_get(&self, key: ConfigKey) -> Result<ConfigEntry, AppError> {
        Ok(self.configuration_store.load()?.document.entry(key))
    }

    pub fn config_list(&self) -> Result<ConfigEntries, AppError> {
        Ok(self.configuration_store.load()?.document.entries())
    }

    pub fn config_set(
        &self,
        key: ConfigKey,
        raw_value: OsString,
    ) -> Result<ConfigMutation, AppError> {
        self.mutate(key, |document| document.set(key, raw_value.clone()))
    }

    pub fn config_unset(&self, key: ConfigKey) -> Result<ConfigMutation, AppError> {
        self.mutate(key, |document| {
            document.unset(key);
            Ok(())
        })
    }

    fn mutate(
        &self,
        key: ConfigKey,
        apply: impl Fn(&mut ConfigDocument) -> Result<(), AppError>,
    ) -> Result<ConfigMutation, AppError> {
        for _ in 0..3 {
            let loaded = self.configuration_store.load()?;
            let mut desired = loaded.document.clone();
            apply(&mut desired)?;
            if desired == loaded.document {
                return Ok(ConfigMutation {
                    outcome: MutationOutcome::Unchanged,
                    entry: loaded.document.entry(key),
                });
            }
            match self
                .configuration_store
                .replace(&loaded.baseline, &desired)?
            {
                StoreOutcome::Changed => {
                    return Ok(ConfigMutation {
                        outcome: MutationOutcome::Changed,
                        entry: desired.entry(key),
                    });
                }
                StoreOutcome::Stale => continue,
            }
        }
        Err(AppError::invalid_state(
            "configuration",
            "concurrent_modification",
            ["a stable configuration baseline"],
        ))
    }
}
