use super::Application;
use crate::domain::library::{
    LibraryExportOperation, LibraryExportRequest, LibraryImportOperation, LibraryImportRequest,
};
use crate::error::AppError;

impl Application {
    pub fn library_import(
        &self,
        request: LibraryImportRequest,
    ) -> Result<LibraryImportOperation, AppError> {
        let document = self.library_transfer_store.read_import(&request.input)?;
        self.library_repository.import(&document, request.dry_run)
    }

    pub fn library_export(
        &self,
        request: LibraryExportRequest,
    ) -> Result<LibraryExportOperation, AppError> {
        let document = self.library_repository.export()?;
        self.library_transfer_store
            .write_export(&request.output, &document)?;
        Ok(LibraryExportOperation { document })
    }
}
