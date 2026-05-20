use clap::Parser;
use redlinedb_bench::sqlite_parity::cli::{Cli, run};

fn main() -> anyhow::Result<()> {
    run(Cli::parse())
}
