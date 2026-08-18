#![forbid(unsafe_code)]

mod args;
mod human;
mod json;

use args::{Cli, Command, ConfigCommand};
use clap::Parser;
use skilload_core::adapters::configuration::FileConfigurationStore;
use skilload_core::{AppError, Application, ConfigEntries, ConfigEntry, ConfigKey};
use std::env;
use std::ffi::OsString;
use std::io::{self, Write};
use std::process::ExitCode;
use std::sync::Arc;

enum Projection {
    Entry {
        operation: &'static str,
        outcome: &'static str,
        entry: ConfigEntry,
    },
    Entries {
        operation: &'static str,
        entries: ConfigEntries,
    },
}

fn main() -> ExitCode {
    match run(env::args_os().collect()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(code) => ExitCode::from(code),
    }
}

fn run(arguments: Vec<OsString>) -> Result<(), u8> {
    if args::rejects_json_meta_invocation(&arguments) {
        eprintln!("error: --json cannot be combined with --help or --version");
        return Err(2);
    }
    let cli = match Cli::try_parse_from(&arguments) {
        Ok(cli) => cli,
        Err(error) => {
            let code = error.exit_code() as u8;
            let _ = error.print();
            return Err(code);
        }
    };
    let _ = cli.no_color;
    let Some(command) = cli.command else {
        if cli.json {
            eprintln!("error: --json requires a configuration command");
            return Err(2);
        }
        write_stdout(args::top_level_help().as_bytes()).map_err(report_stdout_error)?;
        return Ok(());
    };
    let application = Application::new(Arc::new(FileConfigurationStore::new()));
    let projection = dispatch(&application, command);
    match projection {
        Ok(projection) => render_success(cli.json, projection),
        Err((operation, error)) => render_error(cli.json, operation, &error),
    }
}

fn dispatch(
    application: &Application,
    command: Command,
) -> Result<Projection, (&'static str, AppError)> {
    match command {
        Command::Config { command } => match command {
            ConfigCommand::Get { key } => {
                let key = ConfigKey::parse(&key).map_err(|error| ("config.get", error))?;
                application
                    .config_get(key)
                    .map(|entry| Projection::Entry {
                        operation: "config.get",
                        outcome: "observed",
                        entry,
                    })
                    .map_err(|error| ("config.get", error))
            }
            ConfigCommand::Set { key, value } => {
                let key = ConfigKey::parse(&key).map_err(|error| ("config.set", error))?;
                application
                    .config_set(key, value)
                    .map(|mutation| Projection::Entry {
                        operation: "config.set",
                        outcome: mutation.outcome.as_str(),
                        entry: mutation.entry,
                    })
                    .map_err(|error| ("config.set", error))
            }
            ConfigCommand::Unset { key } => {
                let key = ConfigKey::parse(&key).map_err(|error| ("config.unset", error))?;
                application
                    .config_unset(key)
                    .map(|mutation| Projection::Entry {
                        operation: "config.unset",
                        outcome: mutation.outcome.as_str(),
                        entry: mutation.entry,
                    })
                    .map_err(|error| ("config.unset", error))
            }
            ConfigCommand::List => application
                .config_list()
                .map(|entries| Projection::Entries {
                    operation: "config.list",
                    entries,
                })
                .map_err(|error| ("config.list", error)),
        },
    }
}

fn render_success(json_output: bool, projection: Projection) -> Result<(), u8> {
    let bytes = match projection {
        Projection::Entry {
            operation,
            outcome,
            entry,
        } if json_output => json::entry(operation, outcome, entry)
            .map_err(|error| report_internal_serialization_error(error.to_string()))?,
        Projection::Entries { operation, entries } if json_output => {
            json::entries(operation, entries)
                .map_err(|error| report_internal_serialization_error(error.to_string()))?
        }
        Projection::Entry {
            operation,
            outcome,
            entry,
        } => {
            return write_stdout(human::render_entry(operation, outcome, &entry).as_bytes())
                .map_err(report_stdout_error);
        }
        Projection::Entries { entries, .. } => {
            return write_stdout(human::render_entries(&entries).as_bytes())
                .map_err(report_stdout_error);
        }
    };
    let mut bytes = bytes;
    bytes.push(b'\n');
    write_stdout(&bytes).map_err(report_stdout_error)
}

fn render_error(json_output: bool, operation: &'static str, error: &AppError) -> Result<(), u8> {
    if json_output {
        let mut bytes = json::error(operation, error).map_err(|serialization| {
            report_internal_serialization_error(serialization.to_string())
        })?;
        bytes.push(b'\n');
        if let Err(write_error) = write_stdout(&bytes) {
            eprintln!("error [internal_invariant]: cannot write JSON output: {write_error}");
            return Err(6);
        }
    } else {
        eprint!("{}", human::render_error(error));
    }
    Err(error.exit_code())
}

fn write_stdout(bytes: &[u8]) -> io::Result<()> {
    let mut stdout = io::stdout().lock();
    stdout.write_all(bytes)?;
    stdout.flush()
}

fn report_stdout_error(error: io::Error) -> u8 {
    eprintln!("error [internal_invariant]: cannot write stdout: {error}");
    6
}

fn report_internal_serialization_error(message: String) -> u8 {
    eprintln!("error [internal_invariant]: cannot serialize JSON output: {message}");
    6
}
