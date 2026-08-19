use assert_cmd::Command;
use serde_json::Value;
use std::ffi::OsString;
use std::fs;
use std::os::unix::ffi::OsStringExt;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::process::{Command as ProcessCommand, Output};
use std::sync::Arc;
use std::thread;
use tempfile::tempdir;

fn command(root: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_skilload"));
    command
        .env_clear()
        .env("HOME", root.join("home"))
        .env("XDG_CONFIG_HOME", root.join("config"))
        .env("XDG_DATA_HOME", root.join("data"))
        .env("XDG_STATE_HOME", root.join("state"))
        .env("XDG_CACHE_HOME", root.join("cache"));
    command
}

fn execute(root: &Path, arguments: &[&str]) -> Output {
    let mut command = command(root);
    command.args(arguments).output().unwrap()
}

fn execute_with_restrictive_umask(root: &Path, arguments: &[&str]) -> Output {
    let mut command = ProcessCommand::new("/bin/sh");
    command
        .arg("-c")
        .arg("umask 0177; exec \"$@\"")
        .arg("skilload")
        .arg(env!("CARGO_BIN_EXE_skilload"))
        .args(arguments)
        .env_clear()
        .env("HOME", root.join("home"))
        .env("XDG_CONFIG_HOME", root.join("config"))
        .env("XDG_DATA_HOME", root.join("data"))
        .env("XDG_STATE_HOME", root.join("state"))
        .env("XDG_CACHE_HOME", root.join("cache"));
    command.output().unwrap()
}

fn json(output: &Output) -> Value {
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn config_file(root: &Path) -> std::path::PathBuf {
    root.join("config/skilload/config.toml")
}

fn portable_document(path: &str, alias: Option<&str>) -> String {
    let alias = alias
        .map(|value| format!("\"{value}\""))
        .unwrap_or_else(|| "null".to_owned());
    format!(
        r#"{{
  "format_version": 1,
  "entries": [
    {{
      "skill": {{
        "source": {{
          "canonical": "github:owner/repository#{path}@refs/heads/main",
          "owner": "owner",
          "repository": "repository",
          "repository_display": "Repository",
          "path": "{path}",
          "ref_kind": "branch",
          "ref_value": "refs/heads/main"
        }},
        "repository_id": "42",
        "commit": "0123456789012345678901234567890123456789",
        "integrity": "sha256:0123456789012345678901234567890123456789012345678901234567890123",
        "name": "review",
        "description": "Portable Library entry",
        "entry_count": "1",
        "byte_count": "10"
      }},
      "alias": {alias},
      "category": null,
      "tags": [" Review ", "review"],
      "note": null
    }}
  ]
}}"#
    )
}

#[test]
fn library_import_export_is_portable_atomic_and_inert_when_dry_run() {
    let first = tempdir().unwrap();
    fs::create_dir(first.path().join("home")).unwrap();
    let input = first.path().join("portable-library.json");
    fs::write(&input, portable_document("skills/review", Some("review"))).unwrap();
    let input = input.to_str().unwrap();

    let dry_run = json(&execute(
        first.path(),
        &["library", "import", "--input", input, "--dry-run", "--json"],
    ));
    assert_eq!(dry_run["operation"], "library.import");
    assert_eq!(dry_run["result"]["outcome"], "observed");
    assert_eq!(dry_run["result"]["data"]["dry_run"], true);
    assert_eq!(
        dry_run["result"]["data"]["added"].as_array().unwrap().len(),
        1
    );
    for root in ["config", "data", "state", "cache"] {
        assert!(
            !first.path().join(root).exists(),
            "{root} must remain absent"
        );
    }

    let committed = json(&execute(
        first.path(),
        &["library", "import", "--input", input, "--json"],
    ));
    assert_eq!(committed["result"]["outcome"], "changed");
    assert_eq!(committed["result"]["data"]["dry_run"], false);
    let database = first.path().join("data/skilload/skilload.db");
    let database_inode = fs::metadata(&database).unwrap().ino();
    assert!(
        first
            .path()
            .join("state/skilload/locks/database.lock")
            .is_file()
    );
    assert!(!first.path().join("config").exists());
    assert!(!first.path().join("cache").exists());

    let exported_path = first.path().join("round-trip.json");
    let exported = json(&execute(
        first.path(),
        &[
            "library",
            "export",
            "--output",
            exported_path.to_str().unwrap(),
            "--json",
        ],
    ));
    assert_eq!(exported["operation"], "library.export");
    assert_eq!(exported["result"]["outcome"], "observed");
    assert_eq!(
        exported["result"]["data"]["entries"][0]["tags"],
        serde_json::json!(["Review"])
    );
    let exported_document: Value =
        serde_json::from_slice(&fs::read(&exported_path).unwrap()).unwrap();
    assert_eq!(exported_document, exported["result"]["data"]);

    let repeated = json(&execute(
        first.path(),
        &["library", "import", "--input", input, "--json"],
    ));
    assert_eq!(repeated["result"]["outcome"], "unchanged");
    assert_eq!(database_inode, fs::metadata(&database).unwrap().ino());

    let second = tempdir().unwrap();
    fs::create_dir(second.path().join("home")).unwrap();
    let imported_again = json(&execute(
        second.path(),
        &[
            "library",
            "import",
            "--input",
            exported_path.to_str().unwrap(),
            "--json",
        ],
    ));
    assert_eq!(imported_again["result"]["outcome"], "changed");
    let second_export = second.path().join("second-round-trip.json");
    let _ = json(&execute(
        second.path(),
        &[
            "library",
            "export",
            "--output",
            second_export.to_str().unwrap(),
            "--json",
        ],
    ));
    let second_document: Value = serde_json::from_slice(&fs::read(second_export).unwrap()).unwrap();
    assert_eq!(exported_document, second_document);
}

#[test]
fn library_import_errors_are_structured_and_leave_existing_data_unchanged() {
    let temporary = tempdir().unwrap();
    fs::create_dir(temporary.path().join("home")).unwrap();
    let valid = temporary.path().join("valid.json");
    fs::write(&valid, portable_document("skills/review", Some("review"))).unwrap();
    assert!(
        execute(
            temporary.path(),
            &[
                "library",
                "import",
                "--input",
                valid.to_str().unwrap(),
                "--json"
            ],
        )
        .status
        .success()
    );
    let database = temporary.path().join("data/skilload/skilload.db");
    let before = fs::read(&database).unwrap();

    let duplicate = temporary.path().join("duplicate.json");
    let mut duplicate_document: Value =
        serde_json::from_str(&portable_document("skills/duplicate", None)).unwrap();
    let duplicate_entry = duplicate_document["entries"][0].clone();
    duplicate_document["entries"]
        .as_array_mut()
        .unwrap()
        .push(duplicate_entry);
    fs::write(&duplicate, serde_json::to_vec(&duplicate_document).unwrap()).unwrap();
    let output = execute(
        temporary.path(),
        &[
            "--json",
            "library",
            "import",
            "--input",
            duplicate.to_str().unwrap(),
        ],
    );
    assert!(!output.status.success());
    let error: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(error["operation"], "library.import");
    assert_eq!(error["error"]["code"], "conflict");
    assert!(error["error"]["details"]["conflicts"][0]["name"].is_null());
    assert_eq!(fs::read(&database).unwrap(), before);

    let duplicate_key = temporary.path().join("duplicate-key.json");
    fs::write(
        &duplicate_key,
        r#"{"format_version":1,"format_version":1,"entries":[]}"#,
    )
    .unwrap();
    let output = execute(
        temporary.path(),
        &[
            "--json",
            "library",
            "import",
            "--input",
            duplicate_key.to_str().unwrap(),
        ],
    );
    assert!(!output.status.success());
    let error: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(error["error"]["code"], "validation_failed");
    assert_eq!(fs::read(&database).unwrap(), before);

    let protected_output = execute(
        temporary.path(),
        &[
            "--json",
            "library",
            "export",
            "--output",
            database.to_str().unwrap(),
        ],
    );
    assert!(!protected_output.status.success());
    assert_eq!(fs::read(&database).unwrap(), before);
}

#[test]
fn help_and_absent_queries_are_offline_and_filesystem_inert() {
    let temporary = tempdir().unwrap();
    fs::create_dir(temporary.path().join("home")).unwrap();

    let help = execute(temporary.path(), &[]);
    assert!(help.status.success());
    assert!(String::from_utf8_lossy(&help.stdout).contains("Usage: skilload"));
    assert!(help.stderr.is_empty());

    let explicit_help = execute(temporary.path(), &["--help"]);
    assert!(explicit_help.status.success());
    assert!(String::from_utf8_lossy(&explicit_help.stdout).contains("Usage: skilload"));
    assert!(explicit_help.stderr.is_empty());

    let version = execute(temporary.path(), &["--version"]);
    assert!(version.status.success());
    assert_eq!(version.stdout, b"skilload 0.0.1\n");
    assert!(version.stderr.is_empty());

    let list = json(&execute(temporary.path(), &["config", "list", "--json"]));
    let entries = list["result"]["data"]["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 3);
    assert_eq!(entries[0]["key"], "cache_limit_bytes");
    assert_eq!(entries[1]["key"], "agents.claude.executable");
    assert_eq!(entries[2]["key"], "agents.codex.executable");
    assert_eq!(entries[0]["default_value"], "536870912");
    for root in ["config", "data", "state", "cache"] {
        assert!(
            !temporary.path().join(root).exists(),
            "{root} must remain absent"
        );
    }
}

#[test]
fn configuration_round_trips_in_json_and_human_modes_without_other_roots() {
    let temporary = tempdir().unwrap();
    fs::create_dir(temporary.path().join("home")).unwrap();

    let set_cache = json(&execute(
        temporary.path(),
        &["config", "set", "cache_limit_bytes", "1073741824", "--json"],
    ));
    assert_eq!(set_cache["operation"], "config.set");
    assert_eq!(set_cache["result"]["outcome"], "changed");
    assert_eq!(set_cache["result"]["data"]["entry"]["value"], "1073741824");

    let set_agent = json(&execute(
        temporary.path(),
        &[
            "--json",
            "config",
            "set",
            "agents.claude.executable",
            "/opt/claude/../bin/claude",
        ],
    ));
    assert_eq!(set_agent["result"]["outcome"], "changed");
    assert_eq!(
        set_agent["result"]["data"]["entry"]["value"]["display"],
        "/opt/bin/claude"
    );
    assert_eq!(
        set_agent["result"]["data"]["entry"]["value"]["bytes_base64"],
        "L29wdC9iaW4vY2xhdWRl"
    );

    let get_agent = json(&execute(
        temporary.path(),
        &["config", "get", "agents.claude.executable", "--json"],
    ));
    assert_eq!(get_agent["result"]["outcome"], "observed");
    assert_eq!(get_agent["result"]["data"]["entry"]["configured"], true);

    let human = execute(temporary.path(), &["config", "list", "--no-color"]);
    assert!(human.status.success());
    assert!(String::from_utf8_lossy(&human.stdout).contains("key: \"agents.claude.executable\""));

    assert_eq!(
        fs::read_to_string(config_file(temporary.path())).unwrap(),
        "version = 1\ncache_limit_bytes = 1073741824\n\n[agents.claude]\nexecutable = \"/opt/bin/claude\"\n"
    );
    assert!(
        temporary
            .path()
            .join("state/skilload/locks/config.lock")
            .is_file()
    );
    assert!(!temporary.path().join("data").exists());
    assert!(!temporary.path().join("cache").exists());
}

#[test]
fn first_mutation_creates_nested_xdg_roots_under_restrictive_umask() {
    let temporary = tempdir().unwrap();
    let output = execute_with_restrictive_umask(
        temporary.path(),
        &["config", "set", "cache_limit_bytes", "1", "--json"],
    );

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read_to_string(config_file(temporary.path())).unwrap(),
        "version = 1\ncache_limit_bytes = 1\n"
    );
    for directory in [
        "config",
        "config/skilload",
        "state",
        "state/skilload",
        "state/skilload/locks",
    ] {
        assert_eq!(
            fs::metadata(temporary.path().join(directory))
                .unwrap()
                .mode()
                & 0o777,
            0o700,
            "{directory}"
        );
    }
}

#[test]
fn repeated_mutations_preserve_file_identity_and_final_unset_keeps_schema_document() {
    let temporary = tempdir().unwrap();
    fs::create_dir(temporary.path().join("home")).unwrap();
    json(&execute(
        temporary.path(),
        &["config", "set", "cache_limit_bytes", "1", "--json"],
    ));
    let config = config_file(temporary.path());
    let metadata = fs::metadata(&config).unwrap();
    let bytes = fs::read(&config).unwrap();
    let repeated = json(&execute(
        temporary.path(),
        &["config", "set", "cache_limit_bytes", "1", "--json"],
    ));
    assert_eq!(repeated["result"]["outcome"], "unchanged");
    let after = fs::metadata(&config).unwrap();
    assert_eq!(fs::read(&config).unwrap(), bytes);
    assert_eq!(after.ino(), metadata.ino());
    assert_eq!(after.mtime(), metadata.mtime());
    assert_eq!(after.mtime_nsec(), metadata.mtime_nsec());

    let unset = json(&execute(
        temporary.path(),
        &["config", "unset", "cache_limit_bytes", "--json"],
    ));
    assert_eq!(unset["result"]["outcome"], "changed");
    assert_eq!(fs::read_to_string(&config).unwrap(), "version = 1\n");
    let again = json(&execute(
        temporary.path(),
        &["config", "unset", "cache_limit_bytes", "--json"],
    ));
    assert_eq!(again["result"]["outcome"], "unchanged");
}

#[test]
fn invalid_input_schema_and_unknown_commands_never_rewrite_or_create_state() {
    let temporary = tempdir().unwrap();
    fs::create_dir(temporary.path().join("home")).unwrap();
    for value in ["0", "-1", "9223372036854775808"] {
        let output = execute(
            temporary.path(),
            &["config", "set", "cache_limit_bytes", value, "--json"],
        );
        assert_eq!(output.status.code(), Some(4));
        let document: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(document["error"]["code"], "validation_failed");
        assert!(!config_file(temporary.path()).exists());
    }

    for command in [
        "add",
        "rm",
        "use",
        "init",
        "claude",
        "codex",
        "tui",
        "web",
        "collection",
    ] {
        let output = execute(temporary.path(), &[command]);
        assert_eq!(output.status.code(), Some(2), "{command}");
    }
    assert!(!temporary.path().join("config").exists());
    assert!(!temporary.path().join("state").exists());

    let config_root = temporary.path().join("config/skilload");
    fs::create_dir_all(&config_root).unwrap();
    let config = config_file(temporary.path());
    fs::write(&config, "version = 1\nunknown = true\n").unwrap();
    let original = fs::read(&config).unwrap();
    let output = execute(
        temporary.path(),
        &["config", "set", "cache_limit_bytes", "1", "--json"],
    );
    assert_eq!(output.status.code(), Some(4));
    assert_eq!(fs::read(&config).unwrap(), original);
}

#[test]
fn out_of_range_schema_versions_keep_json_details_within_api_v1() {
    let temporary = tempdir().unwrap();
    fs::create_dir(temporary.path().join("home")).unwrap();
    let config_root = temporary.path().join("config/skilload");
    fs::create_dir_all(&config_root).unwrap();
    let config = config_file(temporary.path());
    let original = b"version = 9007199254740993\n";
    fs::write(&config, original).unwrap();

    let output = execute(temporary.path(), &["config", "list", "--json"]);

    assert_eq!(output.status.code(), Some(4));
    let document: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(document["error"]["code"], "invalid_state");
    assert_eq!(document["error"]["details"]["domain"], "configuration");
    assert_eq!(document["error"]["details"]["state"], "invalid_version");
    assert!(document["error"]["details"].get("found_version").is_none());
    assert_eq!(fs::read(&config).unwrap(), original);
}

#[test]
fn json_meta_and_invalid_native_path_errors_are_safe() {
    let temporary = tempdir().unwrap();
    fs::create_dir(temporary.path().join("home")).unwrap();
    for meta_arguments in [["--json", "--help"], ["--json", "-hV"], ["--json", "-Vh"]] {
        let meta = execute(temporary.path(), &meta_arguments);
        assert_eq!(meta.status.code(), Some(2));
        assert!(meta.stdout.is_empty());
        assert!(String::from_utf8_lossy(&meta.stderr).contains("--json cannot be combined"));
    }

    let no_operation = execute(temporary.path(), &["--json"]);
    assert_eq!(no_operation.status.code(), Some(2));
    assert!(no_operation.stdout.is_empty());
    assert!(
        String::from_utf8_lossy(&no_operation.stderr).contains("requires an implemented command")
    );

    let mut path_command = command(temporary.path());
    path_command
        .arg("config")
        .arg("set")
        .arg("agents.codex.executable")
        .arg(OsString::from_vec(vec![b'/', 0xff]))
        .arg("--json");
    let output = path_command.output().unwrap();
    assert_eq!(output.status.code(), Some(4));
    let document: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(document["error"]["code"], "validation_failed");
    assert_eq!(document["error"]["details"]["path"]["bytes_base64"], "L/8=");
    assert!(!config_file(temporary.path()).exists());
    let mut numeric_command = command(temporary.path());
    numeric_command
        .arg("config")
        .arg("set")
        .arg("cache_limit_bytes")
        .arg(OsString::from_vec(vec![0xff]))
        .arg("--json");
    let numeric_output = numeric_command.output().unwrap();
    assert_eq!(numeric_output.status.code(), Some(4));
    let numeric_document: Value = serde_json::from_slice(&numeric_output.stdout).unwrap();
    assert_eq!(numeric_document["error"]["code"], "validation_failed");
    assert!(numeric_document["error"]["details"]["path"].is_null());
}

#[test]
fn unknown_configuration_keys_redact_credential_shaped_values() {
    let temporary = tempdir().unwrap();
    fs::create_dir(temporary.path().join("home")).unwrap();
    let credential = "ghp_0123456789abcdefghijklmnopqrstuvwxyz";

    let json_output = execute(temporary.path(), &["--json", "config", "get", credential]);
    assert_eq!(json_output.status.code(), Some(2));
    assert!(json_output.stderr.is_empty());
    assert!(!String::from_utf8_lossy(&json_output.stdout).contains(credential));
    let document: Value = serde_json::from_slice(&json_output.stdout).unwrap();
    assert_eq!(document["operation"], "config.get");
    assert_eq!(document["error"]["code"], "usage_error");
    assert_eq!(document["error"]["details"]["argument"], "key");
    assert!(document["error"]["details"]["value"].is_null());
    assert_eq!(
        document["error"]["details"]["expected"],
        serde_json::json!([
            "cache_limit_bytes",
            "agents.claude.executable",
            "agents.codex.executable"
        ])
    );

    let human_output = execute(temporary.path(), &["config", "get", credential]);
    assert_eq!(human_output.status.code(), Some(2));
    assert!(human_output.stdout.is_empty());
    assert!(!String::from_utf8_lossy(&human_output.stderr).contains(credential));
    assert!(!temporary.path().join("config").exists());
    assert!(!temporary.path().join("state").exists());
}

#[test]
fn parser_failures_are_terminal_safe_and_preserve_json_configuration_operations() {
    let temporary = tempdir().unwrap();
    let hostile = execute(temporary.path(), &["\u{001b}]0;owned\u{0007}\nunknown"]);
    assert_eq!(hostile.status.code(), Some(2));
    assert!(hostile.stdout.is_empty());
    assert_eq!(
        hostile.stderr,
        b"error [usage_error]: invalid command line; use --help for usage\n"
    );

    let malformed_json = execute(
        temporary.path(),
        &["--json", "config", "set", "cache_limit_bytes"],
    );
    assert_eq!(malformed_json.status.code(), Some(2));
    assert!(malformed_json.stderr.is_empty());
    let document: Value = serde_json::from_slice(&malformed_json.stdout).unwrap();
    assert_eq!(document["api_version"], 1);
    assert_eq!(document["operation"], "config.set");
    assert_eq!(document["ok"], false);
    assert_eq!(document["error"]["code"], "usage_error");
    assert!(document["error"]["details"]["argument"].is_null());
    assert!(document["error"]["details"]["value"].is_null());
    assert_eq!(
        document["error"]["details"]["expected"],
        serde_json::json!([])
    );
    for arguments in [
        ["--json", "--bogus", "config", "list"],
        ["--json", "config", "--bogus", "list"],
    ] {
        let malformed_json = execute(temporary.path(), &arguments);
        assert_eq!(malformed_json.status.code(), Some(2));
        assert!(malformed_json.stderr.is_empty());
        let document: Value = serde_json::from_slice(&malformed_json.stdout).unwrap();
        assert_eq!(document["operation"], "config.list");
        assert_eq!(document["ok"], false);
        assert_eq!(document["error"]["code"], "usage_error");
    }
    assert!(!temporary.path().join("config").exists());
    assert!(!temporary.path().join("state").exists());
}
#[test]
fn concurrent_distinct_setters_merge_their_configuration_changes() {
    let temporary = tempdir().unwrap();
    fs::create_dir(temporary.path().join("home")).unwrap();
    let root = Arc::new(temporary.path().to_path_buf());
    let cache_root = root.clone();
    let cache = thread::spawn(move || {
        execute(
            cache_root.as_path(),
            &["config", "set", "cache_limit_bytes", "1", "--json"],
        )
    });
    let agent_root = root.clone();
    let agent = thread::spawn(move || {
        execute(
            agent_root.as_path(),
            &[
                "config",
                "set",
                "agents.codex.executable",
                "/opt/codex",
                "--json",
            ],
        )
    });
    assert!(cache.join().unwrap().status.success());
    assert!(agent.join().unwrap().status.success());
    let content = fs::read_to_string(config_file(root.as_path())).unwrap();
    assert!(content.contains("cache_limit_bytes = 1"));
    assert!(content.contains("[agents.codex]"));
}
