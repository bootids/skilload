#![forbid(unsafe_code)]

pub mod adapters;
pub mod application;
pub mod domain;
pub mod error;
pub mod ports;

pub use application::Application;
pub use domain::configuration::{
    CONFIG_SCHEMA_VERSION, ConfigEntries, ConfigEntry, ConfigKey, ConfigMutation, ConfigValue,
    DEFAULT_CACHE_LIMIT_BYTES, MAX_CACHE_LIMIT_BYTES, MutationOutcome, NativePath,
};
pub use error::AppError;
