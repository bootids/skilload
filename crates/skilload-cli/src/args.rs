use clap::{CommandFactory, Parser, Subcommand};
use std::ffi::{OsStr, OsString};

#[derive(Debug, Parser)]
#[command(
    name = "skilload",
    version,
    about = "Manage local Agent Skills.",
    disable_help_subcommand = true,
    propagate_version = true
)]
pub struct Cli {
    #[arg(long, global = true, help = "Render a configuration result as JSON.")]
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

pub fn rejects_json_meta_invocation(arguments: &[OsString]) -> bool {
    let has_json = arguments
        .iter()
        .skip(1)
        .any(|argument| argument == OsStr::new("--json"));
    let has_meta = arguments.iter().skip(1).any(|argument| {
        matches!(
            argument.as_os_str(),
            value if value == OsStr::new("--help")
                || value == OsStr::new("-h")
                || value == OsStr::new("--version")
                || value == OsStr::new("-V")
        )
    });
    has_json && has_meta
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_real_configuration_leaves_are_registered() {
        let command = Cli::command();
        let config = command
            .get_subcommands()
            .find(|subcommand| subcommand.get_name() == "config")
            .unwrap();
        let names: Vec<_> = config
            .get_subcommands()
            .map(|subcommand| subcommand.get_name())
            .collect();
        assert_eq!(names, ["get", "set", "unset", "list"]);
        assert!(
            !config
                .get_subcommands()
                .any(|subcommand| subcommand.get_name() == "help")
        );
    }

    #[test]
    fn json_is_rejected_only_for_text_meta_invocations() {
        assert!(rejects_json_meta_invocation(&[
            "skilload".into(),
            "--json".into(),
            "--help".into()
        ]));
        assert!(!rejects_json_meta_invocation(&[
            "skilload".into(),
            "config".into(),
            "list".into(),
            "--json".into()
        ]));
    }
}
