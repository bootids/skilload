use skilload_core::{
    AppError, ConfigEntries, ConfigEntry, ConfigValue, LibraryImportResult, NativePath,
    PortableLibraryDocument,
};
use std::fmt::Write as _;
use std::os::unix::ffi::OsStrExt;

pub fn render_entry(operation: &str, outcome: &str, entry: &ConfigEntry) -> String {
    let mut output = format!("{operation}: {outcome}\n");
    append_entry(&mut output, entry);
    output
}

pub fn render_entries(entries: &ConfigEntries) -> String {
    let mut output = format!("schema_version: {}\n", entries.schema_version);
    for (index, entry) in entries.entries.iter().enumerate() {
        if index > 0 {
            output.push('\n');
        }
        append_entry(&mut output, entry);
    }
    output
}

pub fn render_library_import(outcome: &str, data: &LibraryImportResult) -> String {
    format!(
        "library.import: {outcome}\nformat_version: {}\ndry_run: {}\nadded: {}\nupdated: {}\nkept: {}\nconflicts: {}\n",
        data.format_version,
        data.dry_run,
        data.added.len(),
        data.updated.len(),
        data.kept.len(),
        data.conflicts.len(),
    )
}

pub fn render_library_export(output: &NativePath, document: &PortableLibraryDocument) -> String {
    format!(
        "library.export: observed\noutput: {}\nentries: {}\n",
        quote_path(output),
        document.entries.len(),
    )
}

pub fn render_error(error: &AppError) -> String {
    match error {
        AppError::Usage {
            argument,
            value,
            expected,
            ..
        } => format!(
            "error [{}]: invalid {} {}; expected {}\n",
            error.code(),
            quote_string(argument.as_deref().unwrap_or("argument")),
            quote_string(value.as_deref().unwrap_or("")),
            expected
                .iter()
                .map(|item| quote_string(item))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        AppError::Validation { constraint, path } => format!(
            "error [{}]: {}{}\n",
            error.code(),
            quote_string(constraint),
            path.as_ref()
                .map(|path| format!(" for {}", quote_path(path)))
                .unwrap_or_default()
        ),
        AppError::InputLimit {
            limit_kind,
            measured,
            allowed,
            path,
        } => format!(
            "error [{}]: limit {} measured {} exceeds allowed {} for {}\n",
            error.code(),
            quote_string(limit_kind),
            measured,
            allowed,
            quote_path(path)
        ),
        AppError::Conflict { conflicts } => format!(
            "error [{}]: Library import has {} conflict(s)\n",
            error.code(),
            conflicts.len()
        ),
        AppError::InvalidEnvironment {
            variable,
            path,
            reason,
        }
        | AppError::OverlappingStateRoots {
            variable,
            path,
            reason,
        } => format!(
            "error [{}]: {} {}{}\n",
            error.code(),
            quote_string(variable),
            quote_string(reason),
            path.as_ref()
                .map(|path| format!(" at {}", quote_path(path)))
                .unwrap_or_default()
        ),
        AppError::Busy {
            lock_domain,
            waited_ms,
        } => format!(
            "error [{}]: lock {} remained busy for {} ms\n",
            error.code(),
            quote_string(lock_domain),
            waited_ms
        ),
        AppError::SchemaNewer {
            domain,
            found_version,
            supported_version,
        }
        | AppError::MigrationRequired {
            domain,
            found_version,
            supported_version,
        } => format!(
            "error [{}]: {} schema {} is incompatible with supported schema {}\n",
            error.code(),
            quote_string(domain),
            found_version,
            supported_version
        ),
        AppError::DatabaseCorrupt { database, .. } => format!(
            "error [{}]: database {} requires database-corruption-v1 recovery\n",
            error.code(),
            quote_path(database)
        ),
        AppError::InvalidState {
            domain,
            state,
            expected,
        } => format!(
            "error [{}]: {} is {}; expected {}\n",
            error.code(),
            quote_string(domain),
            quote_string(state),
            expected
                .iter()
                .map(|item| quote_string(item))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        AppError::Internal { incident_id } => format!(
            "error [{}]: incident {}\n",
            error.code(),
            quote_string(incident_id)
        ),
    }
}

pub fn quote_string(value: &str) -> String {
    format!("\"{}\"", encode_utf8(value))
}

pub fn quote_path(path: &NativePath) -> String {
    format!("\"{}\"", display_path(path))
}

pub fn display_path(path: &NativePath) -> String {
    encode_native_bytes(path.as_path().as_os_str().as_bytes())
}

fn append_entry(output: &mut String, entry: &ConfigEntry) {
    let value = match &entry.value {
        Some(ConfigValue::CacheLimitBytes(value)) => quote_string(&value.to_string()),
        Some(ConfigValue::Executable(path)) => quote_path(path),
        None => "null".to_owned(),
    };
    let default_value = entry
        .default_value
        .map(|value| quote_string(&value.to_string()))
        .unwrap_or_else(|| "null".to_owned());
    let default_command = entry
        .default_command
        .map(quote_string)
        .unwrap_or_else(|| "null".to_owned());
    let _ = writeln!(output, "key: {}", quote_string(entry.key.as_str()));
    let _ = writeln!(output, "configured: {}", entry.configured);
    let _ = writeln!(output, "value: {value}");
    let _ = writeln!(output, "default_value: {default_value}");
    let _ = writeln!(output, "default_command: {default_command}");
}

fn encode_utf8(value: &str) -> String {
    let mut output = String::new();
    for character in value.chars() {
        push_character(&mut output, character);
    }
    output
}

fn encode_native_bytes(bytes: &[u8]) -> String {
    let mut output = String::new();
    let mut remaining = bytes;
    while !remaining.is_empty() {
        match std::str::from_utf8(remaining) {
            Ok(valid) => {
                output.push_str(&encode_utf8(valid));
                break;
            }
            Err(error) => {
                let valid_up_to = error.valid_up_to();
                if valid_up_to > 0 {
                    output.push_str(&encode_utf8(
                        std::str::from_utf8(&remaining[..valid_up_to])
                            .expect("reported valid prefix"),
                    ));
                }
                let invalid_length = error.error_len().unwrap_or(remaining.len() - valid_up_to);
                for byte in &remaining[valid_up_to..valid_up_to + invalid_length] {
                    let _ = write!(output, "\\x{byte:02X}");
                }
                remaining = &remaining[valid_up_to + invalid_length..];
            }
        }
    }
    output
}

fn push_character(output: &mut String, character: char) {
    match character {
        '"' => output.push_str("\\\""),
        '\\' => output.push_str("\\\\"),
        '\n' => output.push_str("\\n"),
        '\r' => output.push_str("\\r"),
        '\t' => output.push_str("\\t"),
        _ if requires_unicode_escape(character) => {
            let _ = write!(output, "\\u{{{:04X}}}", character as u32);
        }
        _ => output.push(character),
    }
}

fn requires_unicode_escape(character: char) -> bool {
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
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;
    use std::path::PathBuf;

    #[test]
    fn terminal_encoder_is_injective_for_controls_and_invalid_bytes() {
        assert_eq!(
            quote_string("\"\\\n\r\t\u{202e}"),
            "\"\\\"\\\\\\n\\r\\t\\u{202E}\""
        );
        let path = NativePath::new(PathBuf::from(OsString::from_vec(vec![b'/', 0xff, b'\n'])));
        assert_eq!(display_path(&path), "/\\xFF\\n");
    }

    #[test]
    fn terminal_encoder_escapes_every_control_family_used_by_the_contract() {
        let hostile = "\u{001b}[31m\u{0007}\u{007f}\u{0085}\u{2028}\u{2029}\u{061c}\u{200e}\u{200f}\u{202a}\u{202e}\u{2066}\u{2069}";
        let encoded = quote_string(hostile);
        for expected in [
            "\\u{001B}",
            "\\u{0007}",
            "\\u{007F}",
            "\\u{0085}",
            "\\u{2028}",
            "\\u{2029}",
            "\\u{061C}",
            "\\u{200E}",
            "\\u{200F}",
            "\\u{202A}",
            "\\u{202E}",
            "\\u{2066}",
            "\\u{2069}",
        ] {
            assert!(
                encoded.contains(expected),
                "missing {expected} in {encoded}"
            );
        }
        assert!(!encoded.contains('\u{001b}'));
        assert!(!encoded.contains('\u{0007}'));
    }
}
