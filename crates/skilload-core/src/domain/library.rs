use crate::domain::configuration::{MutationOutcome, NativePath};
use crate::domain::source::{ResolvedSkill, SourceIdentity};
use crate::domain::unicode_15_1::normalize_tag;
use crate::error::{AppError, Conflict};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

pub const LIBRARY_FORMAT_VERSION: u64 = 1;

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryImportOperation {
    pub outcome: MutationOutcome,
    pub data: LibraryImportResult,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryExportOperation {
    pub document: PortableLibraryDocument,
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

        let mut canonical_sources = HashSet::with_capacity(self.entries.len());
        let mut aliases = HashMap::new();
        for entry in &mut self.entries {
            validate_optional_text(entry.alias.as_deref(), 256, 1_024, "library_alias")?;
            validate_optional_text(entry.category.as_deref(), 256, 1_024, "library_category")?;
            validate_optional_text(entry.note.as_deref(), 4_096, 16_384, "library_note")?;

            let mut tag_keys = HashSet::with_capacity(entry.tags.len());
            let mut normalized_tags = Vec::with_capacity(entry.tags.len());
            for tag in &entry.tags {
                let tag = normalize_tag(tag)?;
                if tag_keys.insert(tag.comparison_key) {
                    normalized_tags.push(tag.display);
                }
            }
            if normalized_tags.len() > 64 {
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
            entry.tags.sort_by(|left, right| {
                let left = normalize_tag(left).expect("stored tag was validated");
                let right = normalize_tag(right).expect("stored tag was validated");
                left.comparison_key.cmp(&right.comparison_key)
            });
        }
        self.entries.sort_by(|left, right| {
            left.skill
                .source
                .canonical
                .cmp(&right.skill.source.canonical)
        });
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
    fn validation_rejects_second_canonical_source() {
        let error = PortableLibraryDocument {
            format_version: 1,
            entries: vec![entry("skills/review"), entry("skills/review")],
        }
        .validate()
        .unwrap_err();
        assert_eq!(error.code(), "conflict");
    }
}
