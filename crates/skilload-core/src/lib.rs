#![deny(unsafe_code)]

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
pub use domain::doctor::{
    DoctorAction, DoctorActionKind, DoctorData, DoctorFinding, DoctorOperation, DoctorOutcome,
    DoctorSeverity,
};
pub use domain::library::{
    LIBRARY_FORMAT_VERSION, LIBRARY_PAGE_DEFAULT_LIMIT, LibraryChangedField, LibraryEntriesPage,
    LibraryEntry, LibraryExportOperation, LibraryExportRequest, LibraryImportOperation,
    LibraryImportOutcome, LibraryImportRequest, LibraryImportResult, LibraryMetadataChange,
    LibraryMetadataMutation, LibraryMetadataStoreResult, LibraryMutationOperation,
    LibraryMutationOutcome, LibraryPage, LibrarySearchPage, LibrarySearchQuery, LibrarySearchTerm,
    LibraryTrustState, MAX_LIBRARY_PAGE_LIMIT, PortableLibraryDocument, PortableLibraryEntry,
};
pub use domain::source::{RefIntent, RefKind, ResolvedSkill, SourceIdentity};
pub use error::{AppError, Conflict};
