use clap::{CommandFactory, Parser, Subcommand};
use std::ffi::{OsStr, OsString};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(
    name = "skilload",
    version,
    about = "Manage local Agent Skills.",
    disable_help_subcommand = true,
    propagate_version = true
)]
pub struct Cli {
    #[arg(long, global = true, help = "Render a command result as JSON.")]
    pub json: bool,
    #[arg(long = "no-color", global = true, help = "Disable terminal color.")]
    pub no_color: bool,
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
    Library {
        #[command(subcommand)]
        command: LibraryCommand,
    },
}

#[derive(Debug, Subcommand)]
pub enum ConfigCommand {
    Get {
        key: String,
    },
    Set {
        key: String,
        #[arg(allow_hyphen_values = true)]
        value: OsString,
    },
    Unset {
        key: String,
    },
    List,
}

#[derive(Debug, Subcommand)]
pub enum LibraryCommand {
    Import {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        dry_run: bool,
    },
    Export {
        #[arg(long)]
        output: PathBuf,
    },
    Alias {
        #[command(subcommand)]
        command: LibraryAliasCommand,
    },
    Category {
        #[command(subcommand)]
        command: LibraryCategoryCommand,
    },
    Tag {
        #[command(subcommand)]
        command: LibraryTagCommand,
    },
    Note {
        #[command(subcommand)]
        command: LibraryNoteCommand,
    },
}

#[derive(Debug, Subcommand)]
pub enum LibraryAliasCommand {
    Set {
        source: String,
        #[arg(allow_hyphen_values = true)]
        alias: String,
    },
    Clear {
        source: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum LibraryCategoryCommand {
    Set {
        source: String,
        #[arg(allow_hyphen_values = true)]
        category: String,
    },
    Clear {
        source: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum LibraryTagCommand {
    Add {
        source: String,
        #[arg(allow_hyphen_values = true)]
        tag: String,
    },
    Remove {
        source: String,
        #[arg(allow_hyphen_values = true)]
        tag: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum LibraryNoteCommand {
    Set {
        source: String,
        #[arg(allow_hyphen_values = true)]
        note: String,
    },
    Clear {
        source: String,
    },
}

pub fn top_level_help() -> String {
    let mut command = Cli::command();
    let mut output = Vec::new();
    command
        .write_long_help(&mut output)
        .expect("writing help to memory succeeds");
    let mut help = String::from_utf8(output).expect("clap help is valid UTF-8");
    if !help.ends_with('\n') {
        help.push('\n');
    }
    help
}

pub fn json_operation(arguments: &[OsString]) -> Option<&'static str> {
    if !json_requested(arguments) {
        return None;
    }
    let mut positionals = arguments.iter().skip(1).filter(|argument| {
        !matches!(
            argument.as_os_str(),
            value if value == OsStr::new("--json") || value == OsStr::new("--no-color")
        ) && !is_option_like(argument.as_os_str())
    });
    match positionals.next()?.as_os_str() {
        value if value == OsStr::new("config") => match positionals.next()?.as_os_str() {
            value if value == OsStr::new("get") => Some("config.get"),
            value if value == OsStr::new("set") => Some("config.set"),
            value if value == OsStr::new("unset") => Some("config.unset"),
            value if value == OsStr::new("list") => Some("config.list"),
            _ => None,
        },
        value if value == OsStr::new("library") => match positionals.next()?.as_os_str() {
            value if value == OsStr::new("import") => Some("library.import"),
            value if value == OsStr::new("export") => Some("library.export"),
            value if value == OsStr::new("alias") => match positionals.next()?.as_os_str() {
                value if value == OsStr::new("set") => Some("library.alias.set"),
                value if value == OsStr::new("clear") => Some("library.alias.clear"),
                _ => None,
            },
            value if value == OsStr::new("category") => match positionals.next()?.as_os_str() {
                value if value == OsStr::new("set") => Some("library.category.set"),
                value if value == OsStr::new("clear") => Some("library.category.clear"),
                _ => None,
            },
            value if value == OsStr::new("tag") => match positionals.next()?.as_os_str() {
                value if value == OsStr::new("add") => Some("library.tag.add"),
                value if value == OsStr::new("remove") => Some("library.tag.remove"),
                _ => None,
            },
            value if value == OsStr::new("note") => match positionals.next()?.as_os_str() {
                value if value == OsStr::new("set") => Some("library.note.set"),
                value if value == OsStr::new("clear") => Some("library.note.clear"),
                _ => None,
            },
            _ => None,
        },
        _ => None,
    }
}

fn is_option_like(argument: &OsStr) -> bool {
    argument
        .to_str()
        .is_some_and(|value| value.starts_with('-') && value != "-")
}

pub fn json_requested(arguments: &[OsString]) -> bool {
    arguments
        .iter()
        .skip(1)
        .any(|argument| argument == OsStr::new("--json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_real_configuration_and_library_leaves_are_registered() {
        let command = Cli::command();
        let top_level: Vec<_> = command
            .get_subcommands()
            .map(|subcommand| subcommand.get_name())
            .collect();
        assert_eq!(top_level, ["config", "library"]);

        let config = command
            .get_subcommands()
            .find(|subcommand| subcommand.get_name() == "config")
            .unwrap();
        let config_names: Vec<_> = config
            .get_subcommands()
            .map(|subcommand| subcommand.get_name())
            .collect();
        assert_eq!(config_names, ["get", "set", "unset", "list"]);

        let library = command
            .get_subcommands()
            .find(|subcommand| subcommand.get_name() == "library")
            .unwrap();
        let library_names: Vec<_> = library
            .get_subcommands()
            .map(|subcommand| subcommand.get_name())
            .collect();
        assert_eq!(
            library_names,
            ["import", "export", "alias", "category", "tag", "note"]
        );
        for (group, leaves) in [
            ("alias", &["set", "clear"][..]),
            ("category", &["set", "clear"][..]),
            ("tag", &["add", "remove"][..]),
            ("note", &["set", "clear"][..]),
        ] {
            let command = library
                .get_subcommands()
                .find(|command| command.get_name() == group)
                .unwrap();
            let names: Vec<_> = command
                .get_subcommands()
                .map(|command| command.get_name())
                .collect();
            assert_eq!(names, leaves);
        }
        assert!(
            !library
                .get_subcommands()
                .any(|subcommand| subcommand.get_name() == "help")
        );
    }

    #[test]
    fn json_parser_failures_preserve_identifiable_implemented_operations() {
        assert_eq!(
            json_operation(&[
                "skilload".into(),
                "--json".into(),
                "config".into(),
                "set".into(),
                "cache_limit_bytes".into(),
            ]),
            Some("config.set")
        );
        assert_eq!(
            json_operation(&[
                "skilload".into(),
                "config".into(),
                "--no-color".into(),
                "list".into(),
                "--json".into(),
            ]),
            Some("config.list")
        );
        assert_eq!(
            json_operation(&[
                "skilload".into(),
                "--json".into(),
                "--bogus".into(),
                "config".into(),
                "list".into(),
            ]),
            Some("config.list")
        );
        assert_eq!(
            json_operation(&[
                "skilload".into(),
                "--json".into(),
                "config".into(),
                "--bogus".into(),
                "list".into(),
            ]),
            Some("config.list")
        );
        assert_eq!(
            json_operation(&[
                "skilload".into(),
                "--json".into(),
                "config".into(),
                "unknown".into(),
            ]),
            None
        );
        assert_eq!(
            json_operation(&[
                "skilload".into(),
                "--json".into(),
                "library".into(),
                "import".into(),
            ]),
            Some("library.import")
        );
        assert_eq!(
            json_operation(&[
                "skilload".into(),
                "library".into(),
                "export".into(),
                "--json".into(),
                "--output".into(),
            ]),
            Some("library.export")
        );
        assert_eq!(
            json_operation(&[
                "skilload".into(),
                "--json".into(),
                "library".into(),
                "tag".into(),
                "remove".into(),
            ]),
            Some("library.tag.remove")
        );
        assert_eq!(
            json_operation(&[
                "skilload".into(),
                "--json".into(),
                "library".into(),
                "note".into(),
                "unknown".into(),
            ]),
            None
        );
    }
}
