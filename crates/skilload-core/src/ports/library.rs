use crate::domain::configuration::NativePath;
use crate::domain::library::{LibraryImportOperation, PortableLibraryDocument};
use crate::error::AppError;

pub trait LibraryTransferStore: Send + Sync {
    fn read_import(&self, input: &NativePath) -> Result<PortableLibraryDocument, AppError>;
    fn write_export(
        &self,
        output: &NativePath,
        document: &PortableLibraryDocument,
    ) -> Result<(), AppError>;
}

pub trait LibraryRepository: Send + Sync {
    fn export(&self) -> Result<PortableLibraryDocument, AppError>;
    fn import(
        &self,
        document: &PortableLibraryDocument,
        dry_run: bool,
    ) -> Result<LibraryImportOperation, AppError>;
}
