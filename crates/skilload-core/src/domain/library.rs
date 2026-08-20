use crate::domain::configuration::NativePath;
use crate::domain::source::{ResolvedSkill, SourceIdentity};
use crate::domain::unicode_15_1::{TagValue, normalize_tag};
use crate::error::{AppError, Conflict};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    io::{self, Write},
};

pub const LIBRARY_FORMAT_VERSION: u64 = 1;

pub const MAX_PORTABLE_LIBRARY_DOCUMENT_BYTES: u64 = 67_108_864;
pub(crate) const MAX_PORTABLE_LIBRARY_ENTRIES: u64 = 10_000;

const MAX_LIBRARY_ALIAS_SCALARS: usize = 256;
const MAX_LIBRARY_ALIAS_BYTES: usize = 1_024;
const MAX_LIBRARY_CATEGORY_SCALARS: usize = 256;
const MAX_LIBRARY_CATEGORY_BYTES: usize = 1_024;
const MAX_LIBRARY_NOTE_SCALARS: usize = 4_096;
const MAX_LIBRARY_NOTE_BYTES: usize = 16_384;
const MAX_LIBRARY_TAGS: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PortableLibraryEntry {
    pub skill: ResolvedSkill,
    pub alias: Option<String>,
    pub category: Option<String>,
    pub tags: Vec<String>,
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PortableLibraryDocument {
    pub format_version: u64,
    pub entries: Vec<PortableLibraryEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryImportRequest {
    pub input: NativePath,
    pub dry_run: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryExportRequest {
    pub output: NativePath,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LibraryImportResult {
    pub format_version: u64,
    pub dry_run: bool,
    pub added: Vec<SourceIdentity>,
    pub updated: Vec<SourceIdentity>,
    pub kept: Vec<SourceIdentity>,
    pub conflicts: Vec<SourceIdentity>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LibraryImportOutcome {
    Observed,
    Changed,
    Unchanged,
}

impl LibraryImportOutcome {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Observed => "observed",
            Self::Changed => "changed",
            Self::Unchanged => "unchanged",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryImportOperation {
    pub outcome: LibraryImportOutcome,
    pub data: LibraryImportResult,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryExportOperation {
    pub document: PortableLibraryDocument,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LibraryMetadataChange {
    AliasSet(String),
    AliasClear,
    CategorySet(String),
    CategoryClear,
    TagAdd(TagValue),
    TagRemove(TagValue),
    NoteSet(String),
    NoteClear,
}

impl LibraryMetadataChange {
    pub fn alias_set(value: String) -> Result<Self, AppError> {
        validate_metadata_text(
            &value,
            MAX_LIBRARY_ALIAS_SCALARS,
            MAX_LIBRARY_ALIAS_BYTES,
            "library_alias",
        )?;
        Ok(Self::AliasSet(value))
    }

    pub fn category_set(value: String) -> Result<Self, AppError> {
        validate_metadata_text(
            &value,
            MAX_LIBRARY_CATEGORY_SCALARS,
            MAX_LIBRARY_CATEGORY_BYTES,
            "library_category",
        )?;
        Ok(Self::CategorySet(value))
    }

    pub fn tag_add(value: String) -> Result<Self, AppError> {
        Ok(Self::TagAdd(normalize_tag(&value)?))
    }

    pub fn tag_remove(value: String) -> Result<Self, AppError> {
        Ok(Self::TagRemove(normalize_tag(&value)?))
    }

    pub fn note_set(value: String) -> Result<Self, AppError> {
        validate_metadata_text(
            &value,
            MAX_LIBRARY_NOTE_SCALARS,
            MAX_LIBRARY_NOTE_BYTES,
            "library_note",
        )?;
        Ok(Self::NoteSet(value))
    }

    pub(crate) fn validate(&self) -> Result<(), AppError> {
        match self {
            Self::AliasSet(value) => validate_metadata_text(
                value,
                MAX_LIBRARY_ALIAS_SCALARS,
                MAX_LIBRARY_ALIAS_BYTES,
                "library_alias",
            ),
            Self::CategorySet(value) => validate_metadata_text(
                value,
                MAX_LIBRARY_CATEGORY_SCALARS,
                MAX_LIBRARY_CATEGORY_BYTES,
                "library_category",
            ),
            Self::TagAdd(value) | Self::TagRemove(value) => {
                let normalized = normalize_tag(&value.display)?;
                if normalized.display != value.display {
                    return Err(AppError::validation("library_tag_display", None));
                }
                if normalized.comparison_key != value.comparison_key {
                    return Err(AppError::validation("library_tag_comparison_key", None));
                }
                Ok(())
            }
            Self::NoteSet(value) => validate_metadata_text(
                value,
                MAX_LIBRARY_NOTE_SCALARS,
                MAX_LIBRARY_NOTE_BYTES,
                "library_note",
            ),
            Self::AliasClear | Self::CategoryClear | Self::NoteClear => Ok(()),
        }
    }

    pub const fn changed_field(&self) -> LibraryChangedField {
        match self {
            Self::AliasSet(_) | Self::AliasClear => LibraryChangedField::Alias,
            Self::CategorySet(_) | Self::CategoryClear => LibraryChangedField::Category,
            Self::TagAdd(_) | Self::TagRemove(_) => LibraryChangedField::Tags,
            Self::NoteSet(_) | Self::NoteClear => LibraryChangedField::Note,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LibraryChangedField {
    Alias,
    Category,
    Tags,
    Note,
}

impl LibraryChangedField {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Alias => "alias",
            Self::Category => "category",
            Self::Tags => "tags",
            Self::Note => "note",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LibraryMutationOutcome {
    Changed,
    Unchanged,
}

impl LibraryMutationOutcome {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Changed => "changed",
            Self::Unchanged => "unchanged",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LibraryTrustState {
    Missing,
    Revoked,
    Active,
}

impl LibraryTrustState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Missing => "missing",
            Self::Revoked => "revoked",
            Self::Active => "active",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LibraryEntry {
    pub skill: ResolvedSkill,
    pub alias: Option<String>,
    pub category: Option<String>,
    pub tags: Vec<String>,
    pub note: Option<String>,
    pub trust_state: LibraryTrustState,
}

impl LibraryEntry {
    pub fn from_portable(entry: PortableLibraryEntry, trust_state: LibraryTrustState) -> Self {
        Self {
            skill: entry.skill,
            alias: entry.alias,
            category: entry.category,
            tags: entry.tags,
            note: entry.note,
            trust_state,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryMetadataMutation {
    pub selector: String,
    pub change: LibraryMetadataChange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryMetadataStoreResult {
    pub outcome: LibraryMutationOutcome,
    pub entry: PortableLibraryEntry,
    pub changed_fields: Vec<LibraryChangedField>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryMutationOperation {
    pub outcome: LibraryMutationOutcome,
    pub source: SourceIdentity,
    pub entry: LibraryEntry,
    pub changed_fields: Vec<LibraryChangedField>,
}

impl PortableLibraryEntry {
    pub fn apply_metadata_change(
        &mut self,
        change: &LibraryMetadataChange,
    ) -> Result<LibraryMutationOutcome, AppError> {
        change.validate()?;
        match change {
            LibraryMetadataChange::AliasSet(value) => {
                if self.alias.as_ref() == Some(value) {
                    Ok(LibraryMutationOutcome::Unchanged)
                } else {
                    self.alias = Some(value.clone());
                    Ok(LibraryMutationOutcome::Changed)
                }
            }
            LibraryMetadataChange::AliasClear => {
                if self.alias.is_none() {
                    Ok(LibraryMutationOutcome::Unchanged)
                } else {
                    self.alias = None;
                    Ok(LibraryMutationOutcome::Changed)
                }
            }
            LibraryMetadataChange::CategorySet(value) => {
                if self.category.as_ref() == Some(value) {
                    Ok(LibraryMutationOutcome::Unchanged)
                } else {
                    self.category = Some(value.clone());
                    Ok(LibraryMutationOutcome::Changed)
                }
            }
            LibraryMetadataChange::CategoryClear => {
                if self.category.is_none() {
                    Ok(LibraryMutationOutcome::Unchanged)
                } else {
                    self.category = None;
                    Ok(LibraryMutationOutcome::Changed)
                }
            }
            LibraryMetadataChange::TagAdd(value) => {
                for tag in &self.tags {
                    if normalize_tag(tag)?.comparison_key == value.comparison_key {
                        return Ok(LibraryMutationOutcome::Unchanged);
                    }
                }
                if self.tags.len() >= MAX_LIBRARY_TAGS {
                    return Err(AppError::validation("library_tag_count", None));
                }
                self.tags.push(value.display.clone());
                Ok(LibraryMutationOutcome::Changed)
            }
            LibraryMetadataChange::TagRemove(value) => {
                for (index, tag) in self.tags.iter().enumerate() {
                    if normalize_tag(tag)?.comparison_key == value.comparison_key {
                        self.tags.remove(index);
                        return Ok(LibraryMutationOutcome::Changed);
                    }
                }
                Ok(LibraryMutationOutcome::Unchanged)
            }
            LibraryMetadataChange::NoteSet(value) => {
                if self.note.as_ref() == Some(value) {
                    Ok(LibraryMutationOutcome::Unchanged)
                } else {
                    self.note = Some(value.clone());
                    Ok(LibraryMutationOutcome::Changed)
                }
            }
            LibraryMetadataChange::NoteClear => {
                if self.note.is_none() {
                    Ok(LibraryMutationOutcome::Unchanged)
                } else {
                    self.note = None;
                    Ok(LibraryMutationOutcome::Changed)
                }
            }
        }
    }
}

impl PortableLibraryDocument {
    pub fn empty() -> Self {
        Self {
            format_version: LIBRARY_FORMAT_VERSION,
            entries: Vec::new(),
        }
    }

    pub fn validate(mut self) -> Result<Self, AppError> {
        if self.format_version != LIBRARY_FORMAT_VERSION {
            return Err(AppError::validation("library_format_version", None));
        }

        self.ensure_entry_count()?;

        let mut canonical_sources = HashSet::with_capacity(self.entries.len());
        let mut aliases = HashMap::new();
        for entry in &mut self.entries {
            validate_optional_text(
                entry.alias.as_deref(),
                MAX_LIBRARY_ALIAS_SCALARS,
                MAX_LIBRARY_ALIAS_BYTES,
                "library_alias",
            )?;
            validate_optional_text(
                entry.category.as_deref(),
                MAX_LIBRARY_CATEGORY_SCALARS,
                MAX_LIBRARY_CATEGORY_BYTES,
                "library_category",
            )?;
            validate_optional_text(
                entry.note.as_deref(),
                MAX_LIBRARY_NOTE_SCALARS,
                MAX_LIBRARY_NOTE_BYTES,
                "library_note",
            )?;

            let mut tag_keys = HashSet::with_capacity(entry.tags.len());
            let mut normalized_tags = Vec::with_capacity(entry.tags.len());
            for tag in &entry.tags {
                let tag = normalize_tag(tag)?;
                if tag_keys.insert(tag.comparison_key) {
                    normalized_tags.push(tag.display);
                }
            }
            if normalized_tags.len() > MAX_LIBRARY_TAGS {
                return Err(AppError::validation("library_tag_count", None));
            }
            entry.tags = normalized_tags;

            let source = entry.skill.source.clone();
            if !canonical_sources.insert(source.canonical.clone()) {
                return Err(AppError::conflict(vec![Conflict::internal_duplicate(
                    None, source,
                )]));
            }
            if let Some(alias) = &entry.alias
                && aliases.insert(alias.clone(), source.clone()).is_some()
            {
                return Err(AppError::conflict(vec![Conflict::internal_duplicate(
                    Some(alias.clone()),
                    source,
                )]));
            }
        }
        Ok(self)
    }

    pub fn sort_deterministically(&mut self) -> Result<(), AppError> {
        for entry in &mut self.entries {
            let mut tag_order = entry
                .tags
                .iter()
                .enumerate()
                .map(|(index, tag)| {
                    normalize_tag(tag).map(|normalized| (normalized.comparison_key, index))
                })
                .collect::<Result<Vec<_>, AppError>>()?;
            tag_order.sort_by(|(left, _), (right, _)| left.cmp(right));
            let mut tags = std::mem::take(&mut entry.tags);
            entry.tags = tag_order
                .into_iter()
                .map(|(_, index)| std::mem::take(&mut tags[index]))
                .collect();
        }
        self.entries.sort_by(|left, right| {
            left.skill
                .source
                .canonical
                .cmp(&right.skill.source.canonical)
        });
        Ok(())
    }

    fn ensure_entry_count(&self) -> Result<(), AppError> {
        if self.entries.len() > MAX_PORTABLE_LIBRARY_ENTRIES as usize {
            return Err(AppError::validation(
                "library_portable_document_entries",
                None,
            ));
        }
        Ok(())
    }

    pub fn ensure_transfer_size(&self) -> Result<(), AppError> {
        let mut document = self.clone();
        document.validate_transfer_size()
    }

    pub(crate) fn into_transfer_size(mut self) -> Result<(), AppError> {
        self.validate_transfer_size()
    }

    pub(crate) fn validate_transfer_size(&mut self) -> Result<(), AppError> {
        self.ensure_entry_count()?;
        self.sort_deterministically()?;
        self.encode_with_limit(MAX_PORTABLE_LIBRARY_DOCUMENT_BYTES, false)
            .map(|_| ())
    }

    pub fn serialize_for_transfer(&self) -> Result<Vec<u8>, AppError> {
        let mut document = self.clone().validate()?;
        document.sort_deterministically()?;
        document
            .encode_with_limit(MAX_PORTABLE_LIBRARY_DOCUMENT_BYTES, true)?
            .ok_or_else(|| AppError::Internal {
                incident_id: "library_transfer_encoder_not_capturing".to_owned(),
            })
    }

    fn encode_with_limit(&self, limit: u64, capture: bool) -> Result<Option<Vec<u8>>, AppError> {
        let mut writer = LimitedJsonWriter::new(limit, capture);
        if let Err(error) = serde_json::to_writer(&mut writer, self) {
            return Err(if writer.exceeded {
                AppError::validation("library_portable_document_bytes", None)
            } else {
                AppError::invalid_state(
                    "library_export",
                    format!("cannot serialize portable document: {error}"),
                    ["a serializable LibraryExportData document"],
                )
            });
        }
        Ok(writer.into_bytes())
    }
}

struct LimitedJsonWriter {
    bytes: Option<Vec<u8>>,
    limit: u64,
    written: u64,
    exceeded: bool,
}

impl LimitedJsonWriter {
    fn new(limit: u64, capture: bool) -> Self {
        Self {
            bytes: capture.then(Vec::new),
            limit,
            written: 0,
            exceeded: false,
        }
    }

    fn into_bytes(self) -> Option<Vec<u8>> {
        self.bytes
    }
}

impl Write for LimitedJsonWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let next = self.written.saturating_add(buffer.len() as u64);
        if next > self.limit {
            self.exceeded = true;
            return Err(io::Error::new(
                io::ErrorKind::WriteZero,
                "portable Library document exceeds its byte limit",
            ));
        }
        self.written = next;
        if let Some(bytes) = &mut self.bytes {
            bytes.extend_from_slice(buffer);
        }
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn validate_optional_text(
    value: Option<&str>,
    max_scalars: usize,
    max_bytes: usize,
    constraint: &str,
) -> Result<(), AppError> {
    let Some(value) = value else {
        return Ok(());
    };
    if value.chars().count() > max_scalars || value.len() > max_bytes {
        return Err(AppError::validation(constraint, None));
    }
    Ok(())
}

fn validate_metadata_text(
    value: &str,
    max_scalars: usize,
    max_bytes: usize,
    constraint: &str,
) -> Result<(), AppError> {
    validate_optional_text(Some(value), max_scalars, max_bytes, constraint)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::source::{RefKind, ResolvedSkill, SourceIdentity};

    fn source(path: &str) -> SourceIdentity {
        SourceIdentity::new(
            format!("github:owner/repository#{path}@refs/heads/main"),
            "owner".to_owned(),
            "repository".to_owned(),
            "Repository".to_owned(),
            path.to_owned(),
            RefKind::Branch,
            "refs/heads/main".to_owned(),
        )
        .unwrap()
    }

    fn entry(path: &str) -> PortableLibraryEntry {
        PortableLibraryEntry {
            skill: ResolvedSkill::new(
                source(path),
                42,
                "0123456789012345678901234567890123456789".to_owned(),
                "sha256:0123456789012345678901234567890123456789012345678901234567890123"
                    .to_owned(),
                "review".to_owned(),
                "Description".to_owned(),
                1,
                1,
            )
            .unwrap(),
            alias: None,
            category: None,
            tags: vec![" Review ".to_owned(), "review".to_owned()],
            note: None,
        }
    }

    #[test]
    fn validation_keeps_first_equivalent_tag_display() {
        let document = PortableLibraryDocument {
            format_version: 1,
            entries: vec![entry("skills/review")],
        }
        .validate()
        .unwrap();
        assert_eq!(document.entries[0].tags, ["Review"]);
    }

    #[test]
    fn sorting_invalid_tags_returns_validation_error_without_mutating_document() {
        let mut document = PortableLibraryDocument {
            format_version: 1,
            entries: vec![entry("skills/review")],
        };
        document.entries[0].tags = vec!["Review".to_owned(), "\u{202e}".to_owned()];
        let original = document.clone();

        assert!(matches!(
            document.sort_deterministically(),
            Err(AppError::Validation { constraint, .. })
                if constraint == "library_tag_forbidden_character"
        ));
        assert_eq!(document, original);
    }

    #[test]
    fn transfer_serialization_propagates_invalid_tag_validation() {
        let mut document = PortableLibraryDocument {
            format_version: 1,
            entries: vec![entry("skills/review")],
        };
        document.entries[0].tags = vec!["\u{202e}".to_owned()];

        assert!(matches!(
            document.serialize_for_transfer(),
            Err(AppError::Validation { constraint, .. })
                if constraint == "library_tag_forbidden_character"
        ));
    }

    #[test]
    fn transfer_serialization_rejects_documents_import_would_reject() {
        let invalid_version = PortableLibraryDocument {
            format_version: LIBRARY_FORMAT_VERSION + 1,
            entries: vec![entry("skills/review")],
        };
        assert!(matches!(
            invalid_version.serialize_for_transfer(),
            Err(AppError::Validation { constraint, .. }) if constraint == "library_format_version"
        ));

        let duplicate_sources = PortableLibraryDocument {
            format_version: LIBRARY_FORMAT_VERSION,
            entries: vec![entry("skills/review"), entry("skills/review")],
        };
        assert!(matches!(
            duplicate_sources.serialize_for_transfer(),
            Err(AppError::Conflict { .. })
        ));
    }

    #[test]
    fn validation_rejects_second_canonical_source() {
        let error = PortableLibraryDocument {
            format_version: 1,
            entries: vec![entry("skills/review"), entry("skills/review")],
        }
        .validate()
        .unwrap_err();
        assert_eq!(error.code(), "conflict");
    }

    #[test]
    fn validation_rejects_more_entries_than_portable_transfer_can_import() {
        let error = PortableLibraryDocument {
            format_version: 1,
            entries: (0..=MAX_PORTABLE_LIBRARY_ENTRIES)
                .map(|index| entry(&format!("skills/{index}/review")))
                .collect(),
        }
        .validate()
        .unwrap_err();

        assert!(matches!(
            error,
            AppError::Validation { constraint, .. }
                if constraint == "library_portable_document_entries"
        ));
    }
    #[test]
    fn transfer_encoding_rejects_a_document_over_its_byte_limit() {
        let document = PortableLibraryDocument {
            format_version: 1,
            entries: vec![entry("skills/review")],
        };

        assert!(matches!(
            document.encode_with_limit(1, false),
            Err(AppError::Validation { constraint, .. })
                if constraint == "library_portable_document_bytes"
        ));
    }

    #[test]
    fn transfer_encoding_rejects_valid_metadata_beyond_the_import_ceiling() {
        let note = "\u{10000}".repeat(4_096);
        let document = PortableLibraryDocument {
            format_version: 1,
            entries: (0..4_097)
                .map(|index| {
                    let mut entry = entry(&format!("skills/{index}/review"));
                    entry.note = Some(note.clone());
                    entry
                })
                .collect(),
        }
        .validate()
        .unwrap();

        assert!(matches!(
            document.ensure_transfer_size(),
            Err(AppError::Validation { constraint, .. })
                if constraint == "library_portable_document_bytes"
        ));
    }

    #[test]
    fn metadata_change_constructors_enforce_text_boundaries_and_keep_empty_values() {
        assert!(LibraryMetadataChange::alias_set("\u{10000}".repeat(256)).is_ok());
        assert!(matches!(
            LibraryMetadataChange::alias_set("a".repeat(257)),
            Err(AppError::Validation { constraint, .. }) if constraint == "library_alias"
        ));
        let text_1_025_bytes = format!("{}aaaaa", "\u{10000}".repeat(255));
        assert_eq!(text_1_025_bytes.len(), 1_025);
        assert!(matches!(
            LibraryMetadataChange::alias_set(text_1_025_bytes.clone()),
            Err(AppError::Validation { constraint, .. }) if constraint == "library_alias"
        ));
        assert!(LibraryMetadataChange::category_set("\u{10000}".repeat(256)).is_ok());
        assert!(matches!(
            LibraryMetadataChange::category_set("a".repeat(257)),
            Err(AppError::Validation { constraint, .. }) if constraint == "library_category"
        ));
        assert!(matches!(
            LibraryMetadataChange::category_set(text_1_025_bytes),
            Err(AppError::Validation { constraint, .. }) if constraint == "library_category"
        ));
        assert!(LibraryMetadataChange::note_set("\u{10000}".repeat(4_096)).is_ok());
        assert!(matches!(
            LibraryMetadataChange::note_set("a".repeat(4_097)),
            Err(AppError::Validation { constraint, .. }) if constraint == "library_note"
        ));
        let text_16_385_bytes = format!("{}aaaaa", "\u{10000}".repeat(4_095));
        assert_eq!(text_16_385_bytes.len(), 16_385);
        assert!(matches!(
            LibraryMetadataChange::note_set(text_16_385_bytes),
            Err(AppError::Validation { constraint, .. }) if constraint == "library_note"
        ));

        let mut entry = entry("skills/review");
        assert_eq!(
            entry
                .apply_metadata_change(&LibraryMetadataChange::alias_set(String::new()).unwrap())
                .unwrap(),
            LibraryMutationOutcome::Changed
        );
        assert_eq!(entry.alias.as_deref(), Some(""));
        assert_eq!(
            entry
                .apply_metadata_change(&LibraryMetadataChange::AliasClear)
                .unwrap(),
            LibraryMutationOutcome::Changed
        );
        assert_eq!(entry.alias, None);
    }

    #[test]
    fn metadata_tag_changes_use_equivalence_without_rewriting_the_first_display() {
        let mut entry = PortableLibraryDocument {
            format_version: LIBRARY_FORMAT_VERSION,
            entries: vec![entry("skills/review")],
        }
        .validate()
        .unwrap()
        .entries
        .pop()
        .unwrap();
        assert_eq!(entry.tags, ["Review"]);
        assert_eq!(
            entry
                .apply_metadata_change(
                    &LibraryMetadataChange::tag_add("review".to_owned()).unwrap()
                )
                .unwrap(),
            LibraryMutationOutcome::Unchanged
        );
        assert_eq!(entry.tags, ["Review"]);
        assert_eq!(
            entry
                .apply_metadata_change(
                    &LibraryMetadataChange::tag_remove(" REVIEW ".to_owned()).unwrap()
                )
                .unwrap(),
            LibraryMutationOutcome::Changed
        );
        assert!(entry.tags.is_empty());
    }

    #[test]
    fn metadata_changes_reject_directly_constructed_noncanonical_tag_values() {
        let mut entry = entry("skills/review");
        entry.tags.clear();
        let original = entry.clone();
        let change = LibraryMetadataChange::TagAdd(TagValue {
            display: " Review ".to_owned(),
            comparison_key: "review".to_owned(),
        });

        assert!(matches!(
            entry.apply_metadata_change(&change),
            Err(AppError::Validation { constraint, .. }) if constraint == "library_tag_display"
        ));
        assert_eq!(entry, original);
    }
}
