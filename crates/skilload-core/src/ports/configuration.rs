use crate::domain::configuration::ConfigDocument;
use crate::error::AppError;
use std::ffi::OsString;
use std::path::PathBuf;

pub trait Environment: Send + Sync {
    fn var_os(&self, key: &str) -> Option<OsString>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RootBinding {
    pub effective: PathBuf,
    pub(crate) logical: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedRoots {
    pub config: RootBinding,
    pub data: RootBinding,
    pub state: RootBinding,
    pub cache: RootBinding,
}

pub trait StateRootResolver: Send + Sync {
    fn resolve(&self, environment: &dyn Environment) -> Result<ResolvedRoots, AppError>;
    fn revalidate(&self, roots: &ResolvedRoots) -> Result<(), AppError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ConfigSource {
    Absent,
    Present(Vec<u8>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigBaseline {
    pub(crate) roots: ResolvedRoots,
    pub(crate) source: ConfigSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedConfig {
    pub document: ConfigDocument,
    pub baseline: ConfigBaseline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoreOutcome {
    Changed,
    Stale,
}

pub trait ConfigurationStore: Send + Sync {
    fn load(&self) -> Result<LoadedConfig, AppError>;
    fn replace(
        &self,
        expected: &ConfigBaseline,
        desired: &ConfigDocument,
    ) -> Result<StoreOutcome, AppError>;
}
