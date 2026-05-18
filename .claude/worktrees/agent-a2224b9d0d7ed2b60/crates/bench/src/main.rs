use clap::Parser;

fn main() -> anyhow::Result<()> {
    redlinedb_bench::run(redlinedb_bench::Cli::parse())
}
