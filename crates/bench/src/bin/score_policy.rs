use std::{path::PathBuf, process};

use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
use redlinedb_bench::score_policy;

#[derive(Debug, Parser)]
#[command(about = "Validate Jankurai score policy without ad-hoc JSON parsing")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Compare {
        before: PathBuf,
        after: PathBuf,
        operation: Operation,
    },
    AuditAcceptance {
        report: PathBuf,
    },
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum Operation {
    Commit,
    Push,
}

impl Operation {
    fn as_str(self) -> &'static str {
        match self {
            Self::Commit => "commit",
            Self::Push => "push",
        }
    }
}

fn run(cli: Cli) -> Result<i32> {
    match cli.command {
        Command::Compare {
            before,
            after,
            operation,
        } => {
            let regressions = score_policy::compare_files(&before, &after)?;
            if regressions.is_empty() {
                println!("Score ratchet passed.");
                Ok(0)
            } else {
                eprint!(
                    "{}",
                    score_policy::rejection_message(operation.as_str(), &regressions)
                );
                Ok(1)
            }
        }
        Command::AuditAcceptance { report } => {
            Ok(i32::from(!score_policy::audit_file_is_acceptable(&report)?))
        }
    }
}

fn main() {
    match run(Cli::parse()) {
        Ok(code) => process::exit(code),
        Err(error) => {
            eprintln!("score policy: {error:#}");
            process::exit(2);
        }
    }
}
