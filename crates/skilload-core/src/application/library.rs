use super::Application;
use crate::domain::library::{
    LibraryEntry, LibraryExportOperation, LibraryExportRequest, LibraryImportOperation,
    LibraryImportRequest, LibraryMetadataChange, LibraryMetadataMutation, LibraryMutationOperation,
    LibraryTrustState,
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

    pub fn library_alias_set(
        &self,
        selector: String,
        alias: String,
    ) -> Result<LibraryMutationOperation, AppError> {
        self.mutate_library_metadata(selector, LibraryMetadataChange::alias_set(alias)?)
    }

    pub fn library_alias_clear(
        &self,
        selector: String,
    ) -> Result<LibraryMutationOperation, AppError> {
        self.mutate_library_metadata(selector, LibraryMetadataChange::AliasClear)
    }

    pub fn library_category_set(
        &self,
        selector: String,
        category: String,
    ) -> Result<LibraryMutationOperation, AppError> {
        self.mutate_library_metadata(selector, LibraryMetadataChange::category_set(category)?)
    }

    pub fn library_category_clear(
        &self,
        selector: String,
    ) -> Result<LibraryMutationOperation, AppError> {
        self.mutate_library_metadata(selector, LibraryMetadataChange::CategoryClear)
    }

    pub fn library_tag_add(
        &self,
        selector: String,
        tag: String,
    ) -> Result<LibraryMutationOperation, AppError> {
        self.mutate_library_metadata(selector, LibraryMetadataChange::tag_add(tag)?)
    }

    pub fn library_tag_remove(
        &self,
        selector: String,
        tag: String,
    ) -> Result<LibraryMutationOperation, AppError> {
        self.mutate_library_metadata(selector, LibraryMetadataChange::tag_remove(tag)?)
    }

    pub fn library_note_set(
        &self,
        selector: String,
        note: String,
    ) -> Result<LibraryMutationOperation, AppError> {
        self.mutate_library_metadata(selector, LibraryMetadataChange::note_set(note)?)
    }

    pub fn library_note_clear(
        &self,
        selector: String,
    ) -> Result<LibraryMutationOperation, AppError> {
        self.mutate_library_metadata(selector, LibraryMetadataChange::NoteClear)
    }

    fn mutate_library_metadata(
        &self,
        selector: String,
        change: LibraryMetadataChange,
    ) -> Result<LibraryMutationOperation, AppError> {
        let result = self
            .library_repository
            .mutate_metadata(&LibraryMetadataMutation { selector, change })?;
        let source = result.entry.skill.source.clone();
        Ok(LibraryMutationOperation {
            outcome: result.outcome,
            source,
            entry: LibraryEntry::from_portable(result.entry, LibraryTrustState::Missing),
            changed_fields: result.changed_fields,
        })
    }
}
