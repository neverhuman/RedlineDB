mod beyond_sqlite;
mod cli;
mod evidence;
mod exceptions;
mod report;
mod sqlite_parity;

use std::process::ExitCode;

use clap::Parser;

fn main() -> ExitCode {
    match cli::run(cli::Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            // Emit the typed, agent-friendly repair receipt instead of an
            // opaque one-line error, so the next rerun is local.
            eprintln!("{}", exceptions::render(&error));
            ExitCode::FAILURE
        }
    }
}
