use crate::error::AppError;
use serde::de::{Deserializer, Error as _};
use serde::{Deserialize, Serialize, Serializer};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RefIntent {
    Branch(String),
    Tag(String),
    Commit(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RefKind {
    Branch,
    Tag,
    Commit,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub struct SourceIdentity {
    pub canonical: String,
    pub owner: String,
    pub repository: String,
    pub repository_display: String,
    pub path: String,
    pub ref_kind: RefKind,
    pub ref_value: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSourceIdentity {
    canonical: String,
    owner: String,
    repository: String,
    repository_display: String,
    path: String,
    ref_kind: RefKind,
    ref_value: String,
}

impl<'de> Deserialize<'de> for SourceIdentity {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = RawSourceIdentity::deserialize(deserializer)?;
        Self::new(
            raw.canonical,
            raw.owner,
            raw.repository,
            raw.repository_display,
            raw.path,
            raw.ref_kind,
            raw.ref_value,
        )
        .map_err(D::Error::custom)
    }
}

impl SourceIdentity {
    pub fn new(
        canonical: String,
        owner: String,
        repository: String,
        repository_display: String,
        path: String,
        ref_kind: RefKind,
        ref_value: String,
    ) -> Result<Self, AppError> {
        validate_owner(&owner)?;
        validate_repository(&repository)?;
        if repository_display.is_empty() {
            return Err(AppError::validation(
                "source_repository_display_empty",
                None,
            ));
        }
        if !repository_display.eq_ignore_ascii_case(&repository) {
            return Err(AppError::validation(
                "source_repository_display_mismatch",
                None,
            ));
        }
        validate_path(&path)?;
        validate_ref(ref_kind, &ref_value)?;

        let expected = render_canonical(&owner, &repository, &path, ref_kind, &ref_value);
        if canonical != expected {
            return Err(AppError::validation("source_canonical_mismatch", None));
        }
        Ok(Self {
            canonical,
            owner,
            repository,
            repository_display,
            path,
            ref_kind,
            ref_value,
        })
    }

    pub fn ref_intent(&self) -> RefIntent {
        match self.ref_kind {
            RefKind::Branch => RefIntent::Branch(self.ref_value.clone()),
            RefKind::Tag => RefIntent::Tag(self.ref_value.clone()),
            RefKind::Commit => RefIntent::Commit(self.ref_value.clone()),
        }
    }

    pub fn render_canonical(&self) -> String {
        render_canonical(
            &self.owner,
            &self.repository,
            &self.path,
            self.ref_kind,
            &self.ref_value,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResolvedSkill {
    pub source: SourceIdentity,
    #[serde(serialize_with = "serialize_decimal_u64")]
    pub repository_id: u64,
    pub commit: String,
    pub integrity: String,
    pub name: String,
    pub description: String,
    #[serde(serialize_with = "serialize_decimal_u64")]
    pub entry_count: u64,
    #[serde(serialize_with = "serialize_decimal_u64")]
    pub byte_count: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawResolvedSkill {
    source: SourceIdentity,
    repository_id: String,
    commit: String,
    integrity: String,
    name: String,
    description: String,
    entry_count: String,
    byte_count: String,
}

impl<'de> Deserialize<'de> for ResolvedSkill {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = RawResolvedSkill::deserialize(deserializer)?;
        Self::new(
            raw.source,
            parse_decimal_u64(&raw.repository_id, "repository_id").map_err(D::Error::custom)?,
            raw.commit,
            raw.integrity,
            raw.name,
            raw.description,
            parse_decimal_u64(&raw.entry_count, "entry_count").map_err(D::Error::custom)?,
            parse_decimal_u64(&raw.byte_count, "byte_count").map_err(D::Error::custom)?,
        )
        .map_err(D::Error::custom)
    }
}

impl ResolvedSkill {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        source: SourceIdentity,
        repository_id: u64,
        commit: String,
        integrity: String,
        name: String,
        description: String,
        entry_count: u64,
        byte_count: u64,
    ) -> Result<Self, AppError> {
        if !is_lower_hex(&commit, 40) {
            return Err(AppError::validation("resolved_skill_commit", None));
        }
        if !integrity.starts_with("sha256:") || !is_lower_hex(&integrity[7..], 64) {
            return Err(AppError::validation("resolved_skill_integrity", None));
        }
        validate_skill_name(&name)?;
        validate_source_skill_name(&source, &name)?;
        if description.is_empty() || description.chars().count() > 1_024 {
            return Err(AppError::validation("resolved_skill_description", None));
        }
        Ok(Self {
            source,
            repository_id,
            commit,
            integrity,
            name,
            description,
            entry_count,
            byte_count,
        })
    }
}

pub fn parse_decimal_u64(value: &str, field: &str) -> Result<u64, AppError> {
    if value.is_empty()
        || (value.len() > 1 && value.starts_with('0'))
        || !value.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(AppError::validation(
            format!("resolved_skill_{field}"),
            None,
        ));
    }
    value
        .parse()
        .map_err(|_| AppError::validation(format!("resolved_skill_{field}"), None))
}

pub fn serialize_decimal_u64<S>(value: &u64, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&value.to_string())
}

fn validate_owner(value: &str) -> Result<(), AppError> {
    if value.is_empty()
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        return Err(AppError::validation("source_owner", None));
    }
    Ok(())
}

fn validate_repository(value: &str) -> Result<(), AppError> {
    if value.is_empty()
        || !value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'.' | b'_')
        })
    {
        return Err(AppError::validation("source_repository", None));
    }
    Ok(())
}

fn validate_path(path: &str) -> Result<(), AppError> {
    if path.is_empty() {
        return Ok(());
    }
    if path.starts_with('/')
        || path.ends_with('/')
        || path.contains('\\')
        || path.chars().any(char::is_control)
    {
        return Err(AppError::validation("source_path", None));
    }
    for segment in path.split('/') {
        if segment.is_empty()
            || matches!(segment, "." | "..")
            || segment.eq_ignore_ascii_case(".git")
        {
            return Err(AppError::validation("source_path", None));
        }
    }
    Ok(())
}

fn validate_ref(kind: RefKind, value: &str) -> Result<(), AppError> {
    let valid = match kind {
        RefKind::Branch => value
            .strip_prefix("refs/heads/")
            .is_some_and(valid_ref_suffix),
        RefKind::Tag => value
            .strip_prefix("refs/tags/")
            .is_some_and(valid_ref_suffix),
        RefKind::Commit => is_lower_hex(value, 40),
    };
    if valid {
        Ok(())
    } else {
        Err(AppError::validation("source_ref", None))
    }
}

fn valid_ref_suffix(value: &str) -> bool {
    !value.is_empty()
        && !value.starts_with('/')
        && !value.ends_with('/')
        && !value.ends_with('.')
        && !value.contains("..")
        && !value.contains("@{")
        && !value.chars().any(char::is_control)
        && !value
            .bytes()
            .any(|byte| matches!(byte, b' ' | b'~' | b'^' | b':' | b'?' | b'*' | b'[' | b'\\'))
        && value.split('/').all(|segment| {
            !segment.is_empty() && !segment.starts_with('.') && !segment.ends_with(".lock")
        })
}

fn validate_source_skill_name(source: &SourceIdentity, name: &str) -> Result<(), AppError> {
    let matches = if source.path.is_empty() {
        root_skill_name_matches(&source.repository_display, name)?
    } else {
        source.path.rsplit('/').next() == Some(name)
    };
    if matches {
        Ok(())
    } else {
        Err(AppError::validation("resolved_skill_name", None))
    }
}

fn root_skill_name_matches(display: &str, name: &str) -> Result<bool, AppError> {
    let mut output_length = 0;
    let mut pending_separator = false;
    let mut matches = true;

    for byte in display.bytes() {
        let byte = byte.to_ascii_lowercase();
        if matches!(byte, b'.' | b'_' | b'-') {
            pending_separator |= output_length > 0;
            continue;
        }
        if pending_separator {
            matches &= name.as_bytes().get(output_length) == Some(&b'-');
            output_length += 1;
            pending_separator = false;
        }
        matches &= name.as_bytes().get(output_length) == Some(&byte);
        output_length += 1;
    }

    if output_length == 0 || output_length > 64 {
        Err(AppError::validation("invalid_root_skill_name", None))
    } else {
        Ok(matches && output_length == name.len())
    }
}

fn validate_skill_name(name: &str) -> Result<(), AppError> {
    let valid = !name.is_empty()
        && name.len() <= 64
        && name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        && !name.starts_with('-')
        && !name.ends_with('-')
        && !name.contains("--");
    if valid {
        Ok(())
    } else {
        Err(AppError::validation("resolved_skill_name", None))
    }
}

fn is_lower_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

fn render_canonical(
    owner: &str,
    repository: &str,
    path: &str,
    ref_kind: RefKind,
    ref_value: &str,
) -> String {
    let encoded_path = encode_component(path, true);
    let encoded_ref = match ref_kind {
        RefKind::Commit => ref_value.to_owned(),
        RefKind::Branch | RefKind::Tag => encode_component(ref_value, true),
    };
    format!("github:{owner}/{repository}#{encoded_path}@{encoded_ref}")
}

fn encode_component(value: &str, preserve_slash: bool) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            encoded.push(byte as char);
        } else if preserve_slash && byte == b'/' {
            encoded.push('/');
        } else {
            use std::fmt::Write as _;
            let _ = write!(encoded, "%{byte:02X}");
        }
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source(path: &str, ref_kind: RefKind, ref_value: &str) -> SourceIdentity {
        source_with(
            "owner",
            "repository",
            "Repository",
            path,
            ref_kind,
            ref_value,
        )
        .unwrap()
    }

    fn source_with(
        owner: &str,
        repository: &str,
        repository_display: &str,
        path: &str,
        ref_kind: RefKind,
        ref_value: &str,
    ) -> Result<SourceIdentity, AppError> {
        SourceIdentity::new(
            render_canonical(owner, repository, path, ref_kind, ref_value),
            owner.to_owned(),
            repository.to_owned(),
            repository_display.to_owned(),
            path.to_owned(),
            ref_kind,
            ref_value.to_owned(),
        )
    }

    fn resolved_skill(source: SourceIdentity, name: &str) -> Result<ResolvedSkill, AppError> {
        ResolvedSkill::new(
            source,
            42,
            "0123456789012345678901234567890123456789".to_owned(),
            "sha256:0123456789012345678901234567890123456789012345678901234567890123".to_owned(),
            name.to_owned(),
            "A valid description".to_owned(),
            1,
            10,
        )
    }

    #[test]
    fn canonical_source_preserves_ref_namespace_and_encodes_delimiters() {
        let branch = source("skills/foo@bar", RefKind::Branch, "refs/heads/main");
        let tag = source("skills/foo@bar", RefKind::Tag, "refs/tags/main");
        assert_ne!(branch.canonical, tag.canonical);
        assert_eq!(
            branch.canonical,
            "github:owner/repository#skills/foo%40bar@refs/heads/main"
        );
        assert!(
            SourceIdentity::new(
                "github:owner/repository#skills/foo@refs/heads/main".to_owned(),
                "owner".to_owned(),
                "repository".to_owned(),
                "Repository".to_owned(),
                "skills/foo@bar".to_owned(),
                RefKind::Branch,
                "refs/heads/main".to_owned(),
            )
            .is_err()
        );
    }

    #[test]
    fn resolved_skill_requires_portable_evidence() {
        assert!(
            resolved_skill(
                source("skills/review", RefKind::Branch, "refs/heads/main"),
                "review",
            )
            .is_ok()
        );
    }

    #[test]
    fn portable_root_sources_accept_repository_punctuation_and_match_names() {
        for (repository, display) in [
            ("review_skill", "Review_Skill"),
            ("review.skill", "review.skill"),
        ] {
            let root = source_with(
                "owner",
                repository,
                display,
                "",
                RefKind::Branch,
                "refs/heads/main",
            )
            .unwrap();
            assert!(resolved_skill(root.clone(), "review-skill").is_ok());
            assert!(resolved_skill(root, "unrelated").is_err());
        }
        assert!(
            resolved_skill(
                source("skills/review", RefKind::Branch, "refs/heads/main"),
                "unrelated",
            )
            .is_err()
        );
        let invalid_root = source_with(
            "owner",
            "___",
            "___",
            "",
            RefKind::Branch,
            "refs/heads/main",
        )
        .unwrap();
        assert!(matches!(
            resolved_skill(invalid_root, "review"),
            Err(AppError::Validation { constraint, .. }) if constraint == "invalid_root_skill_name"
        ));
    }

    #[test]
    fn portable_source_rejects_mismatched_repository_display() {
        assert!(matches!(
            source_with(
                "owner",
                "repository",
                "unrelated",
                "",
                RefKind::Branch,
                "refs/heads/main",
            ),
            Err(AppError::Validation { constraint, .. })
                if constraint == "source_repository_display_mismatch"
        ));
    }

    #[test]
    fn portable_source_rejects_unsafe_paths_and_invalid_git_refs() {
        for path in ["skills/.git/review", "skills/\0review", "skills\\review"] {
            assert!(
                source_with(
                    "owner",
                    "repository",
                    "Repository",
                    path,
                    RefKind::Branch,
                    "refs/heads/main",
                )
                .is_err()
            );
        }
        for reference in [
            "refs/heads/a..b",
            "refs/heads/topic.lock",
            "refs/heads/topic.",
            "refs/heads/bad@{name",
            "refs/heads/bad\nname",
            "refs/heads/bad:name",
        ] {
            assert!(
                source_with(
                    "owner",
                    "repository",
                    "Repository",
                    "skills/review",
                    RefKind::Branch,
                    reference,
                )
                .is_err()
            );
        }
        assert!(
            source_with(
                "owner",
                "repository",
                "Repository",
                "skills/review",
                RefKind::Branch,
                "refs/heads/release/v1",
            )
            .is_ok()
        );
    }
}
