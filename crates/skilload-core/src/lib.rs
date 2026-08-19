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
pub use domain::library::{
    LIBRARY_FORMAT_VERSION, LibraryExportOperation, LibraryExportRequest, LibraryImportOperation,
    LibraryImportRequest, LibraryImportResult, PortableLibraryDocument, PortableLibraryEntry,
};
pub use domain::source::{RefIntent, RefKind, ResolvedSkill, SourceIdentity};
pub use error::{AppError, Conflict};
