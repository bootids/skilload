use crate::domain::configuration::ConfigDocument;
use crate::error::AppError;
use std::ffi::OsString;
use std::path::PathBuf;

pub trait Environment: Send + Sync {
    fn var_os(&self, key: &str) -> Option<OsString>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FilesystemIdentity {
    pub(crate) device: u64,
    pub(crate) inode: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RootAnchor {
    pub(crate) path: PathBuf,
    pub(crate) identity: FilesystemIdentity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RootBinding {
    pub effective: PathBuf,
    pub(crate) logical: PathBuf,
    pub(crate) existing_directories: Vec<RootAnchor>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedRoots {
    pub config: RootBinding,
    pub data: RootBinding,
    pub state: RootBinding,
    pub cache: RootBinding,
}

impl ResolvedRoots {
    pub(crate) fn same_paths(&self, other: &Self) -> bool {
        self.config.effective == other.config.effective
            && self.config.logical == other.config.logical
            && self.data.effective == other.data.effective
            && self.data.logical == other.data.logical
            && self.state.effective == other.state.effective
            && self.state.logical == other.state.logical
            && self.cache.effective == other.cache.effective
            && self.cache.logical == other.cache.logical
    }
}

pub trait StateRootResolver: Send + Sync {
    fn resolve(&self, environment: &dyn Environment) -> Result<ResolvedRoots, AppError>;
    fn revalidate(&self, roots: &ResolvedRoots) -> Result<ResolvedRoots, AppError>;
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
