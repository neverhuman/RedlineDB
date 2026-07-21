use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(name = "redline-testing")]
#[command(about = "Official RedlineDB conformance and benchmark runner")]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub(crate) command: CommandKind,
}

#[derive(Debug, Subcommand)]
pub(crate) enum CommandKind {
    Run(RunArgs),
    DockerParity(Box<DockerParityArgs>),
    MajorGate(MajorGateArgs),
    Report(ReportArgs),
    List(ListArgs),
    JankuraiCompare(JankuraiCompareArgs),
    Sentinel(SentinelArgs),
    Version,
}

#[derive(Debug, Args)]
pub(crate) struct RunArgs {
    /// Versioned compatibility contract. When present, `--suite` is ignored
    /// and the selected contract is the sole execution authority.
    #[arg(long)]
    pub(crate) contract: Option<String>,
    /// Deterministic selector: `all`, `id:<id>[,<id>...]`,
    /// `category:<name>`, or `priority:<P0..P4>`.
    #[arg(long, default_value = "all")]
    pub(crate) cases: String,
    #[arg(long, value_enum, default_value = "diagnostic")]
    pub(crate) mode: RunMode,
    #[arg(long, value_enum, default_value = "all")]
    pub(crate) suite: Suite,
    #[arg(long)]
    pub(crate) target_bin: PathBuf,
    /// Argument passed to the target before the harness-owned shell arguments.
    /// Repeat the option to preserve argument ordering.
    #[arg(long = "target-arg", allow_hyphen_values = true)]
    pub(crate) target_args: Vec<String>,
    #[arg(long, default_value = "auto")]
    pub(crate) sqlite_bin: String,
    #[arg(long, default_value = "auto")]
    pub(crate) workers: String,
    #[arg(long, default_value = "auto")]
    pub(crate) tmp_root: String,
    #[arg(long)]
    pub(crate) output: PathBuf,
    #[arg(long, default_value_t = 1)]
    pub(crate) repetitions: usize,
    #[arg(long, default_value_t = 0)]
    pub(crate) warmup: usize,
    #[arg(long, value_enum, default_value = "auto")]
    pub(crate) progress: ProgressMode,
    #[arg(long)]
    pub(crate) memory_samples: bool,
}

#[derive(Debug, Args)]
pub(crate) struct DockerParityArgs {
    #[arg(long, value_enum, default_value = "release")]
    pub(crate) mode: RunMode,
    #[arg(long)]
    pub(crate) target_bin: PathBuf,
    #[arg(long = "target-arg", allow_hyphen_values = true)]
    pub(crate) target_args: Vec<String>,
    #[arg(long, default_value = "/opt/redline/bin/sqlite3")]
    pub(crate) sqlite_bin: PathBuf,
    #[arg(long)]
    pub(crate) evidence_dir: PathBuf,
    #[arg(
        long,
        default_value = "/opt/redline/share/custody/docker-custody-receipt.json"
    )]
    pub(crate) custody_receipt: PathBuf,
    #[arg(long, default_value = "all")]
    pub(crate) sqlite_cases: String,
    #[arg(long, default_value = "all")]
    pub(crate) postgres_oracle_cases: String,
    #[arg(long, default_value = "all")]
    pub(crate) postgres_target_cases: String,
    #[arg(long, default_value = "auto")]
    pub(crate) workers: String,
}

#[derive(Debug, Args)]
pub(crate) struct MajorGateArgs {
    #[arg(long)]
    pub(crate) baseline: String,
    #[arg(long)]
    pub(crate) candidate: String,
}

#[derive(Debug, Args)]
pub(crate) struct SelectArgs {
    #[arg(long)]
    pub(crate) priorities: Option<String>,
    #[arg(long)]
    pub(crate) profiles: Option<String>,
    #[arg(long)]
    pub(crate) include_quarantine: bool,
    #[arg(long)]
    pub(crate) case_list: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub(crate) struct ReportArgs {
    #[arg(long, value_enum, default_value = "all")]
    pub(crate) suite: Suite,
    #[command(flatten)]
    pub(crate) select: SelectArgs,
    #[arg(long)]
    pub(crate) input: PathBuf,
    #[arg(long)]
    pub(crate) official_evidence: Option<PathBuf>,
    #[arg(long)]
    pub(crate) local_diagnostics: bool,
    #[arg(long)]
    pub(crate) out_dir: PathBuf,
    #[arg(long)]
    pub(crate) readme: PathBuf,
    #[arg(long)]
    pub(crate) plot: Option<PathBuf>,
    #[arg(long)]
    pub(crate) ksloc_plot: Option<PathBuf>,
    #[arg(long)]
    pub(crate) performance_histogram_plot: Option<PathBuf>,
    #[arg(long)]
    pub(crate) median_test_performance_plot: Option<PathBuf>,
    #[arg(long)]
    pub(crate) jankurai_score: Option<PathBuf>,
    #[arg(long)]
    pub(crate) jankurai_comparison: Option<PathBuf>,
    #[arg(long)]
    pub(crate) jankurai_comparison_plot: Option<PathBuf>,
    #[arg(long)]
    pub(crate) jankurai_score_plot: Option<PathBuf>,
    #[arg(long)]
    pub(crate) code_shape_plot: Option<PathBuf>,
    #[arg(long)]
    pub(crate) updated_date: String,
    #[arg(long)]
    pub(crate) expected_repetitions: Option<usize>,
    #[arg(long)]
    pub(crate) expected_warmup: Option<usize>,
    #[arg(long)]
    pub(crate) check: bool,
}

#[derive(Debug, Args)]
pub(crate) struct ListArgs {
    #[arg(long, value_enum, default_value = "all")]
    pub(crate) suite: Suite,
    #[command(flatten)]
    pub(crate) select: SelectArgs,
    #[arg(long, value_enum, default_value = "text")]
    pub(crate) format: ListFormat,
}

#[derive(Debug, Args)]
pub(crate) struct JankuraiCompareArgs {
    #[arg(long)]
    pub(crate) redlinedb_score: PathBuf,
    #[arg(long)]
    pub(crate) sqlite_score: PathBuf,
    #[arg(long)]
    pub(crate) sqlite_ref: String,
    #[arg(long)]
    pub(crate) updated_date: String,
    #[arg(long)]
    pub(crate) json: PathBuf,
    #[arg(long)]
    pub(crate) csv: PathBuf,
    #[arg(long)]
    pub(crate) check: bool,
}

#[derive(Debug, Args)]
pub(crate) struct SentinelArgs {
    #[arg(long)]
    pub(crate) input: PathBuf,
    #[arg(long = "ceiling-ns")]
    pub(crate) ceiling_ns: Vec<String>,
    #[arg(long)]
    pub(crate) enforce: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub(crate) enum Suite {
    All,
    #[value(name = "sqlite_parity", alias = "sqlite-parity")]
    SqliteParity,
    #[value(name = "memory")]
    Memory,
    #[value(name = "rql_phase1", alias = "rql-phase1")]
    RqlPhase1,
    #[value(name = "beyond_sqlite", alias = "beyond-sqlite")]
    BeyondSqlite,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub(crate) enum ProgressMode {
    Auto,
    Always,
    Never,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum RunMode {
    Diagnostic,
    Release,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub(crate) enum ListFormat {
    Text,
    Markdown,
    Json,
}

impl Suite {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::SqliteParity => "sqlite_parity",
            Self::Memory => "memory",
            Self::RqlPhase1 => "rql_phase1",
            Self::BeyondSqlite => "beyond_sqlite",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeated_target_arguments_preserve_order_and_hyphen_values() {
        let cli = Cli::try_parse_from([
            "redline-testing",
            "docker-parity",
            "--target-bin",
            "/opt/redline/bin/redlinedb",
            "--evidence-dir",
            "/evidence",
            "--target-arg",
            "--compatibility",
            "--target-arg",
            "postgres",
        ])
        .unwrap();
        let CommandKind::DockerParity(args) = cli.command else {
            panic!("expected docker-parity command")
        };
        assert_eq!(args.target_args, ["--compatibility", "postgres"]);
    }
}
