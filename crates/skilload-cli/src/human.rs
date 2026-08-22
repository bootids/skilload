use skilload_core::{
    AppError, ConfigEntries, ConfigEntry, ConfigValue, DoctorOperation, LibraryEntriesPage,
    LibraryEntry, LibraryImportResult, LibraryMutationOperation, LibrarySearchPage, NativePath,
    PortableLibraryDocument, RefKind, SourceIdentity,
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
    let mut output = format!(
        "library.import: {outcome}\nformat_version: {}\ndry_run: {}\n",
        data.format_version, data.dry_run,
    );
    append_sources(&mut output, "added", &data.added);
    append_sources(&mut output, "updated", &data.updated);
    append_sources(&mut output, "kept", &data.kept);
    append_sources(&mut output, "conflicts", &data.conflicts);
    output
}

fn append_sources(output: &mut String, label: &str, sources: &[SourceIdentity]) {
    let _ = writeln!(output, "{label}: {}", sources.len());
    for source in sources {
        let _ = writeln!(output, "  - {}", quote_string(&source.canonical));
    }
}

pub fn render_library_export(output: &NativePath, document: &PortableLibraryDocument) -> String {
    format!(
        "library.export: observed\noutput: {}\nentries: {}\n",
        quote_path(output),
        document.entries.len(),
    )
}

pub fn render_library_mutation(
    operation: &str,
    outcome: &str,
    mutation: &LibraryMutationOperation,
) -> String {
    let changed_fields = if mutation.changed_fields.is_empty() {
        "none".to_owned()
    } else {
        mutation
            .changed_fields
            .iter()
            .map(|field| quote_string(field.as_str()))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let alias = mutation
        .entry
        .alias
        .as_deref()
        .map(quote_string)
        .unwrap_or_else(|| "null".to_owned());
    let category = mutation
        .entry
        .category
        .as_deref()
        .map(quote_string)
        .unwrap_or_else(|| "null".to_owned());
    let note = mutation
        .entry
        .note
        .as_deref()
        .map(quote_string)
        .unwrap_or_else(|| "null".to_owned());
    let mut output = format!(
        "{operation}: {outcome}\nsource: {}\nchanged_fields: {changed_fields}\ntrust_state: {}\nalias: {alias}\ncategory: {category}\ntags: {}\nnote: {note}\n",
        quote_string(&mutation.source.canonical),
        mutation.entry.trust_state.as_str(),
        mutation.entry.tags.len(),
    );
    for tag in &mutation.entry.tags {
        let _ = writeln!(output, "  - {}", quote_string(tag));
    }
    output
}

pub fn render_library_entries(data: &LibraryEntriesPage) -> String {
    let mut output = format!(
        "library.list: observed\ntotal: {}\noffset: {}\nlimit: {}\nreturned: {}\n",
        data.total,
        data.page.offset(),
        data.page.limit(),
        data.entries.len(),
    );
    append_library_entries(&mut output, &data.entries);
    output
}

pub fn render_library_search(data: &LibrarySearchPage) -> String {
    let mut output = format!(
        "library.search: observed\nquery: {}\ntotal: {}\noffset: {}\nlimit: {}\nreturned: {}\n",
        quote_string(&data.original),
        data.total,
        data.page.offset(),
        data.page.limit(),
        data.entries.len(),
    );
    append_library_entries(&mut output, &data.entries);
    output
}

pub fn render_library_get(entry: &LibraryEntry) -> String {
    let mut output = String::from("library.get: observed\n");
    append_library_entry(&mut output, entry);
    output
}

pub fn render_doctor(operation: &DoctorOperation) -> String {
    let mut output = format!(
        "doctor: {}\nfix_requested: {}\ndatabase_writable: {}\n",
        operation.outcome.as_str(),
        operation.data.fix_requested,
        operation.data.database_writable,
    );
    let _ = writeln!(output, "findings: {}", operation.data.findings.len());
    for finding in &operation.data.findings {
        let target = finding
            .target
            .as_ref()
            .map(quote_path)
            .unwrap_or_else(|| "null".to_owned());
        let _ = writeln!(
            output,
            "  - severity: {}\n    code: {}\n    message: {}\n    target: {target}\n    fixable_offline: {}\n    fixed: {}",
            finding.severity.as_str(),
            quote_string(&finding.code),
            quote_string(&finding.message),
            finding.fixable_offline,
            finding.fixed,
        );
    }
    let _ = writeln!(output, "actions: {}", operation.data.actions.len());
    for action in &operation.data.actions {
        let before = action
            .before
            .as_deref()
            .map(quote_string)
            .unwrap_or_else(|| "null".to_owned());
        let after = action
            .after
            .as_deref()
            .map(quote_string)
            .unwrap_or_else(|| "null".to_owned());
        let _ = writeln!(
            output,
            "  - kind: {}\n    target: {}\n    before: {before}\n    after: {after}",
            action.kind.as_str(),
            quote_path(&action.target),
        );
    }
    output
}

fn append_library_entries(output: &mut String, entries: &[LibraryEntry]) {
    for entry in entries {
        output.push_str("  - ");
        append_library_entry(output, entry);
    }
}

fn append_library_entry(output: &mut String, entry: &LibraryEntry) {
    let source = &entry.skill.source;
    let ref_kind = match source.ref_kind {
        RefKind::Branch => "branch",
        RefKind::Tag => "tag",
        RefKind::Commit => "commit",
    };
    let alias = entry
        .alias
        .as_deref()
        .map(quote_string)
        .unwrap_or_else(|| "null".to_owned());
    let category = entry
        .category
        .as_deref()
        .map(quote_string)
        .unwrap_or_else(|| "null".to_owned());
    let note = entry
        .note
        .as_deref()
        .map(quote_string)
        .unwrap_or_else(|| "null".to_owned());
    let _ = writeln!(
        output,
        "source: {}\nsource_owner: {}\nsource_repository: {}\nsource_repository_display: {}\nsource_path: {}\nsource_ref_kind: {}\nsource_ref_value: {}\nrepository_id: {}\ncommit: {}\nintegrity: {}\nname: {}\ndescription: {}\nentry_count: {}\nbyte_count: {}\nalias: {alias}\ncategory: {category}\ntags: {}\nnote: {note}\ntrust_state: {}",
        quote_string(&source.canonical),
        quote_string(&source.owner),
        quote_string(&source.repository),
        quote_string(&source.repository_display),
        quote_string(&source.path),
        quote_string(ref_kind),
        quote_string(&source.ref_value),
        entry.skill.repository_id,
        quote_string(&entry.skill.commit),
        quote_string(&entry.skill.integrity),
        quote_string(&entry.skill.name),
        quote_string(&entry.skill.description),
        entry.skill.entry_count,
        entry.skill.byte_count,
        entry.tags.len(),
        entry.trust_state.as_str(),
    );
    for tag in &entry.tags {
        let _ = writeln!(output, "  tag: {}", quote_string(tag));
    }
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
        AppError::LibraryInputLimit {
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
        AppError::Conflict { conflicts } => {
            let mut output = format!(
                "error [{}]: Requested change has {} conflict(s)\n",
                error.code(),
                conflicts.len()
            );
            for conflict in conflicts {
                let name = conflict
                    .name
                    .as_deref()
                    .map(quote_string)
                    .unwrap_or_else(|| "null".to_owned());
                let source = conflict
                    .source
                    .as_ref()
                    .map(|source| quote_string(&source.canonical))
                    .unwrap_or_else(|| "null".to_owned());
                let _ = writeln!(
                    output,
                    "  - kind: {}; name: {name}; source: {source}",
                    quote_string(&conflict.kind)
                );
            }
            output
        }
        AppError::NotFound { domain, selector } => format!(
            "error [{}]: {} target {} was not found\n",
            error.code(),
            quote_string(domain),
            quote_string(selector)
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
        AppError::DatabaseCorrupt {
            database,
            backups,
            recoverable_exports,
        } => {
            let mut output = format!(
                "error [{}]: database {} requires database-corruption-v1 recovery\n",
                error.code(),
                quote_path(database)
            );
            for backup in backups {
                let _ = writeln!(output, "  backup: {}", quote_path(backup));
            }
            for export in recoverable_exports {
                let _ = writeln!(output, "  recoverable_export: {}", quote_string(export));
            }
            output
        }
        AppError::InvalidState {
            domain,
            state,
            path,
            expected,
        } => format!(
            "error [{}]: {} is {}; expected {}{}\n",
            error.code(),
            quote_string(domain),
            quote_string(state),
            expected
                .iter()
                .map(|item| quote_string(item))
                .collect::<Vec<_>>()
                .join(", "),
            path.as_ref()
                .map(|path| format!(" at {}", quote_path(path)))
                .unwrap_or_default()
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
    fn library_read_renderers_project_complete_terminal_safe_entries() {
        let source = SourceIdentity::new(
            "github:owner/repository#skills/review@refs/heads/main".to_owned(),
            "owner".to_owned(),
            "repository".to_owned(),
            "Repository".to_owned(),
            "skills/review".to_owned(),
            RefKind::Branch,
            "refs/heads/main".to_owned(),
        )
        .unwrap();
        let entry = LibraryEntry {
            skill: skilload_core::ResolvedSkill::new(
                source,
                42,
                "0123456789012345678901234567890123456789".to_owned(),
                "sha256:0123456789012345678901234567890123456789012345678901234567890123"
                    .to_owned(),
                "review".to_owned(),
                "Description\nwith control".to_owned(),
                3,
                30,
            )
            .unwrap(),
            alias: Some("alias".to_owned()),
            category: Some("category".to_owned()),
            tags: vec!["Review".to_owned()],
            note: Some("note".to_owned()),
            trust_state: skilload_core::LibraryTrustState::Missing,
        };
        let page = skilload_core::LibraryPage::new(100, 0).unwrap();
        let list = LibraryEntriesPage {
            entries: vec![entry.clone()],
            page,
            total: 1,
        };
        let search = LibrarySearchPage {
            original: "review".to_owned(),
            entries: vec![entry.clone()],
            page,
            total: 1,
        };
        let expected = [
            "source: \"github:owner/repository#skills/review@refs/heads/main\"",
            "source_owner: \"owner\"",
            "source_repository: \"repository\"",
            "source_repository_display: \"Repository\"",
            "source_path: \"skills/review\"",
            "source_ref_kind: \"branch\"",
            "source_ref_value: \"refs/heads/main\"",
            "repository_id: 42",
            "commit: \"0123456789012345678901234567890123456789\"",
            "integrity: \"sha256:0123456789012345678901234567890123456789012345678901234567890123\"",
            "name: \"review\"",
            "description: \"Description\\nwith control\"",
            "entry_count: 3",
            "byte_count: 30",
            "alias: \"alias\"",
            "category: \"category\"",
            "tags: 1",
            "  tag: \"Review\"",
            "note: \"note\"",
            "trust_state: missing",
        ];

        for rendered in [
            render_library_entries(&list),
            render_library_search(&search),
            render_library_get(&entry),
        ] {
            for field in expected {
                assert!(rendered.contains(field), "missing {field} in {rendered}");
            }
        }
    }

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
    #[test]
    fn library_import_renderer_lists_quoted_planned_sources() {
        let source = SourceIdentity::new(
            "github:owner/repository#skills/review@refs/heads/main".to_owned(),
            "owner".to_owned(),
            "repository".to_owned(),
            "Repository".to_owned(),
            "skills/review".to_owned(),
            skilload_core::RefKind::Branch,
            "refs/heads/main".to_owned(),
        )
        .unwrap();
        let rendered = render_library_import(
            "observed",
            &LibraryImportResult {
                format_version: 1,
                dry_run: true,
                added: vec![source.clone()],
                updated: Vec::new(),
                kept: vec![source],
                conflicts: Vec::new(),
            },
        );

        assert!(
            rendered.contains(
                "added: 1\n  - \"github:owner/repository#skills/review@refs/heads/main\"\n"
            )
        );
        assert!(
            rendered.contains(
                "kept: 1\n  - \"github:owner/repository#skills/review@refs/heads/main\"\n"
            )
        );
    }

    #[test]
    fn library_import_conflicts_include_quoted_actionable_details() {
        let source = SourceIdentity::new(
            "github:owner/repository#skills/review@refs/heads/main".to_owned(),
            "owner".to_owned(),
            "repository".to_owned(),
            "Repository".to_owned(),
            "skills/review".to_owned(),
            skilload_core::RefKind::Branch,
            "refs/heads/main".to_owned(),
        )
        .unwrap();
        let rendered = render_error(&AppError::conflict(vec![
            skilload_core::Conflict::internal_duplicate(Some("alias\n".to_owned()), source.clone()),
            skilload_core::Conflict::internal_duplicate(None, source),
        ]));

        assert!(rendered.contains("error [conflict]: Requested change has 2 conflict(s)\n"));

        assert!(rendered.contains(
            "kind: \"internal_duplicate\"; name: \"alias\\n\"; source: \"github:owner/repository#skills/review@refs/heads/main\""
        ));
        assert!(rendered.contains(
            "kind: \"internal_duplicate\"; name: null; source: \"github:owner/repository#skills/review@refs/heads/main\""
        ));
    }
    #[test]
    fn database_corruption_renderer_lists_terminal_safe_recovery_assets() {
        let database = NativePath::new(PathBuf::from("/tmp/live.db"));
        let backup = NativePath::new(PathBuf::from(OsString::from_vec(vec![
            b'/', b't', b'm', b'p', b'/', 0xff,
        ])));
        let rendered = render_error(&AppError::DatabaseCorrupt {
            database,
            backups: vec![backup],
            recoverable_exports: vec!["library.export\n".to_owned()],
        });

        assert!(rendered.contains("backup: \"/tmp/\\xFF\""));
        assert!(rendered.contains("recoverable_export: \"library.export\\n\""));
    }
}
