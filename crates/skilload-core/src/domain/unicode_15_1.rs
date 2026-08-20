use crate::error::AppError;
use unicode_normalization::UnicodeNormalization;

include!(concat!(env!("OUT_DIR"), "/unicode_15_1_generated.rs"));

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagValue {
    pub display: String,
    pub comparison_key: String,
}

pub fn normalize_tag(value: &str) -> Result<TagValue, AppError> {
    let display = value.trim_matches(is_white_space).nfc().collect::<String>();
    if display.is_empty() {
        return Err(AppError::validation("library_tag_empty", None));
    }
    if display.chars().any(is_forbidden_tag_character) {
        return Err(AppError::validation(
            "library_tag_forbidden_character",
            None,
        ));
    }
    if display.chars().count() > 64 {
        return Err(AppError::validation("library_tag_too_many_scalars", None));
    }
    if display.len() > 256 {
        return Err(AppError::validation(
            "library_tag_too_many_utf8_bytes",
            None,
        ));
    }

    let comparison_key = full_case_fold(&display).nfc().collect();
    Ok(TagValue {
        display,
        comparison_key,
    })
}

pub fn full_case_fold(value: &str) -> String {
    let mut folded = String::with_capacity(value.len());
    for character in value.chars() {
        let code_point = character as u32;
        match CASE_FOLD.binary_search_by_key(&code_point, |(source, _)| *source) {
            Ok(index) => {
                for mapped in CASE_FOLD[index].1 {
                    folded.push(char::from_u32(*mapped).expect("generated Unicode scalar"));
                }
            }
            Err(_) => folded.push(character),
        }
    }
    folded
}

pub fn is_white_space(character: char) -> bool {
    WHITE_SPACE.binary_search(&(character as u32)).is_ok()
}

fn is_forbidden_tag_character(character: char) -> bool {
    let value = character as u32;
    (value <= 0x1f)
        || value == 0x7f
        || (0x80..=0x9f).contains(&value)
        || matches!(
            value,
            0x2028 | 0x2029 | 0x061c | 0x200e | 0x200f | 0x202a..=0x202e | 0x2066..=0x2069
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_tags_with_pinned_unicode_data() {
        assert_eq!(UNICODE_VERSION, (15, 1, 0));
        let first = normalize_tag(" Review ").unwrap();
        let second = normalize_tag("review").unwrap();
        assert_eq!(first.display, "Review");
        assert_eq!(first.comparison_key, second.comparison_key);

        let composed = normalize_tag("café").unwrap();
        let decomposed = normalize_tag("cafe\u{301}").unwrap();
        assert_eq!(composed.comparison_key, decomposed.comparison_key);
        assert_ne!(
            normalize_tag("I").unwrap().comparison_key,
            normalize_tag("ı").unwrap().comparison_key
        );
    }

    #[test]
    fn rejects_forbidden_or_oversized_tags() {
        assert!(normalize_tag(" \u{202e} ").is_err());
        assert!(normalize_tag(&"a".repeat(65)).is_err());
        assert!(normalize_tag(&"a".repeat(257)).is_err());
    }
}
