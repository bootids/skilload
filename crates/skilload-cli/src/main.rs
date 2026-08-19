#![forbid(unsafe_code)]

mod args;
mod human;
mod json;

use args::{Cli, Command, ConfigCommand, LibraryCommand};
use clap::{Parser, error::ErrorKind};
use skilload_core::adapters::configuration::FileConfigurationStore;
use skilload_core::adapters::portable_library::PortableLibraryTransferStore;
use skilload_core::adapters::sqlite_library::SqliteLibraryRepository;
use skilload_core::{
    AppError, Application, ConfigEntries, ConfigEntry, ConfigKey, LibraryExportRequest,
    LibraryImportRequest, LibraryImportResult, NativePath, PortableLibraryDocument,
};
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
    LibraryImport {
        outcome: &'static str,
        data: LibraryImportResult,
    },
    LibraryExport {
        output: NativePath,
        document: PortableLibraryDocument,
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
    let json_operation = args::json_configuration_operation(&arguments);
    let cli = match Cli::try_parse_from(&arguments) {
        Ok(cli) => cli,
        Err(error) => {
            if let Some(operation) = json_operation {
                return render_error(true, operation, &parser_usage_error());
            }
            return render_parse_error(error);
        }
    };
    let _ = cli.no_color;
    let Some(command) = cli.command else {
        if cli.json {
            eprintln!("error: --json requires an implemented command");
            return Err(2);
        }
        write_stdout(args::top_level_help().as_bytes()).map_err(report_stdout_error)?;
        return Ok(());
    };
    let application = Application::new(
        Arc::new(FileConfigurationStore::new()),
        Arc::new(SqliteLibraryRepository::new()),
        Arc::new(PortableLibraryTransferStore::new()),
    );
    let projection = dispatch(&application, command);
    match projection {
        Ok(projection) => render_success(cli.json, projection),
        Err((operation, error)) => render_error(cli.json, operation, &error),
    }
}

fn parser_usage_error() -> AppError {
    AppError::Usage {
        argument: None,
        value: None,
        path: None,
        expected: Vec::new(),
    }
}

fn render_parse_error(error: clap::Error) -> Result<(), u8> {
    let code = error.exit_code() as u8;
    if matches!(
        error.kind(),
        ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
    ) {
        let output = error.render().to_string();
        write_stdout(output.as_bytes()).map_err(report_stdout_error)?;
    } else {
        eprintln!("error [usage_error]: invalid command line; use --help for usage");
    }
    Err(code)
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
        Command::Library { command } => match command {
            LibraryCommand::Import { input, dry_run } => application
                .library_import(LibraryImportRequest {
                    input: NativePath::new(input),
                    dry_run,
                })
                .map(|operation| Projection::LibraryImport {
                    outcome: if operation.data.dry_run {
                        "observed"
                    } else {
                        operation.outcome.as_str()
                    },
                    data: operation.data,
                })
                .map_err(|error| ("library.import", error)),
            LibraryCommand::Export { output } => {
                let output = NativePath::new(output);
                application
                    .library_export(LibraryExportRequest {
                        output: output.clone(),
                    })
                    .map(|operation| Projection::LibraryExport {
                        output,
                        document: operation.document,
                    })
                    .map_err(|error| ("library.export", error))
            }
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
        Projection::LibraryImport { outcome, data } if json_output => {
            json::library_import("library.import", outcome, data)
                .map_err(|error| report_internal_serialization_error(error.to_string()))?
        }
        Projection::LibraryExport { document, .. } if json_output => {
            json::library_export("library.export", document)
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
        Projection::LibraryImport { outcome, data } => {
            return write_stdout(human::render_library_import(outcome, &data).as_bytes())
                .map_err(report_stdout_error);
        }
        Projection::LibraryExport { output, document } => {
            return write_stdout(human::render_library_export(&output, &document).as_bytes())
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
