use std::{path::PathBuf, process};

use anyhow::Result;
use clap::{Parser, Subcommand};
use redlinedb_bench::perf_evidence::{self, W2ManifestInput, capture_w2_runtime_metadata};

#[derive(Debug, Parser)]
#[command(about = "Generate performance statistics and evidence in Rust")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    SummarizeJsonl {
        input: PathBuf,
    },
    AssertDistinctBinaries {
        target: PathBuf,
        reference: PathBuf,
    },
    AppendW2Manifest {
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        profile: String,
        #[arg(long)]
        allocator: String,
        #[arg(long)]
        label: String,
        #[arg(long)]
        binary: PathBuf,
        #[arg(long)]
        suite: String,
        #[arg(long)]
        perf_jsonl: Option<String>,
        #[arg(long, allow_hyphen_values = true)]
        base_rustflags: String,
    },
}

fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Command::SummarizeJsonl { input } => {
            print!("{}", perf_evidence::summarize_jsonl_path(&input)?.render());
        }
        Command::AssertDistinctBinaries { target, reference } => {
            perf_evidence::assert_distinct_binaries(&target, &reference)?;
        }
        Command::AppendW2Manifest {
            output,
            profile,
            allocator,
            label,
            binary,
            suite,
            perf_jsonl,
            base_rustflags,
        } => {
            let (captured_at_utc, rustc_version, host) = capture_w2_runtime_metadata()?;
            perf_evidence::append_w2_manifest(&W2ManifestInput {
                output_path: output,
                captured_at_utc,
                profile,
                allocator,
                label,
                binary_path: binary,
                suite,
                perf_jsonl,
                rustc_version,
                base_rustflags,
                host,
            })?;
        }
    }
    Ok(())
}

fn main() {
    if let Err(error) = run(Cli::parse()) {
        eprintln!("perf evidence: {error:#}");
        process::exit(2);
    }
}
