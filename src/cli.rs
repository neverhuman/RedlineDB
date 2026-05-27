use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{Context, Result, bail};
use clap::{Args, Parser, Subcommand, ValueEnum};

use crate::beyond_sqlite;
use crate::evidence::{self, EvidenceConfig, OfficialEvidenceConfig, OfficialSuiteEvidence};
use crate::report::{self, JankuraiCompareOptions, ReportOptions, SentinelOptions};
use crate::sqlite_parity;

#[derive(Debug, Parser)]
#[command(name = "redline-testing")]
#[command(about = "Official RedlineDB conformance and benchmark runner")]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    command: CommandKind,
}

#[derive(Debug, Subcommand)]
enum CommandKind {
    Run(RunArgs),
    Report(ReportArgs),
    List(ListArgs),
    JankuraiCompare(JankuraiCompareArgs),
    Sentinel(SentinelArgs),
    Version,
}

#[derive(Debug, Args)]
struct RunArgs {
    #[arg(long, value_enum, default_value = "all")]
    suite: Suite,
    #[arg(long)]
    target_bin: PathBuf,
    #[arg(long, default_value = "auto")]
    sqlite_bin: String,
    #[arg(long, default_value = "auto")]
    workers: String,
    #[arg(long, default_value = "auto")]
    tmp_root: String,
    #[arg(long)]
    output: PathBuf,
    #[arg(long, default_value_t = 1)]
    repetitions: usize,
    #[arg(long, default_value_t = 0)]
    warmup: usize,
    #[arg(long, value_enum, default_value = "auto")]
    progress: ProgressMode,
    #[arg(long)]
    memory_samples: bool,
}

#[derive(Debug, Args)]
struct SelectArgs {
    #[arg(long)]
    priorities: Option<String>,
    #[arg(long)]
    profiles: Option<String>,
    #[arg(long)]
    include_quarantine: bool,
    #[arg(long)]
    case_list: Option<PathBuf>,
}

#[derive(Debug, Args)]
struct ReportArgs {
    #[arg(long, value_enum, default_value = "all")]
    suite: Suite,
    #[command(flatten)]
    select: SelectArgs,
    #[arg(long)]
    input: PathBuf,
    #[arg(long)]
    official_evidence: Option<PathBuf>,
    #[arg(long)]
    local_diagnostics: bool,
    #[arg(long)]
    out_dir: PathBuf,
    #[arg(long)]
    readme: PathBuf,
    #[arg(long)]
    plot: Option<PathBuf>,
    #[arg(long)]
    ksloc_plot: Option<PathBuf>,
    #[arg(long)]
    performance_histogram_plot: Option<PathBuf>,
    #[arg(long)]
    median_test_performance_plot: Option<PathBuf>,
    #[arg(long)]
    jankurai_score: Option<PathBuf>,
    #[arg(long)]
    jankurai_comparison: Option<PathBuf>,
    #[arg(long)]
    jankurai_comparison_plot: Option<PathBuf>,
    #[arg(long)]
    jankurai_score_plot: Option<PathBuf>,
    #[arg(long)]
    code_shape_plot: Option<PathBuf>,
    #[arg(long)]
    updated_date: String,
    #[arg(long)]
    expected_repetitions: Option<usize>,
    #[arg(long)]
    expected_warmup: Option<usize>,
    #[arg(long)]
    check: bool,
}

#[derive(Debug, Args)]
struct ListArgs {
    #[arg(long, value_enum, default_value = "all")]
    suite: Suite,
    #[command(flatten)]
    select: SelectArgs,
    #[arg(long, value_enum, default_value = "text")]
    format: ListFormat,
}

#[derive(Debug, Args)]
struct JankuraiCompareArgs {
    #[arg(long)]
    redlinedb_score: PathBuf,
    #[arg(long)]
    sqlite_score: PathBuf,
    #[arg(long)]
    sqlite_ref: String,
    #[arg(long)]
    updated_date: String,
    #[arg(long)]
    json: PathBuf,
    #[arg(long)]
    csv: PathBuf,
    #[arg(long)]
    check: bool,
}

#[derive(Debug, Args)]
struct SentinelArgs {
    #[arg(long)]
    input: PathBuf,
    #[arg(long = "ceiling-ns")]
    ceiling_ns: Vec<String>,
    #[arg(long)]
    enforce: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum Suite {
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
enum ProgressMode {
    Auto,
    Always,
    Never,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ListFormat {
    Text,
    Markdown,
    Json,
}

pub fn run(cli: Cli) -> Result<()> {
    match cli.command {
        CommandKind::Run(args) => run_suite(args),
        CommandKind::Report(args) => report(args),
        CommandKind::List(args) => list(args),
        CommandKind::JankuraiCompare(args) => jankurai_compare(args),
        CommandKind::Sentinel(args) => sentinel(args),
        CommandKind::Version => {
            println!("redline-testing {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
    }
}

fn run_suite(args: RunArgs) -> Result<()> {
    let workers = resolve_workers(&args.workers)?;
    validate_samples(args.repetitions, args.warmup)?;
    let tmp_root = resolve_tmp_root(&args.tmp_root)?;
    fs::create_dir_all(&tmp_root)
        .with_context(|| format!("create tmp root {}", tmp_root.display()))?;
    let sqlite_bin = resolve_sqlite_bin(&args.sqlite_bin);

    match args.suite {
        Suite::All => run_all_suites(&args, workers, tmp_root, sqlite_bin),
        Suite::SqliteParity | Suite::Memory | Suite::RqlPhase1 => {
            prepare_output(&args.output)?;
            let started = Instant::now();
            let summary = run_sqlite_like_suite(
                &args,
                args.suite,
                args.output.clone(),
                workers,
                tmp_root,
                sqlite_bin,
            )?;
            if progress_enabled(args.progress) {
                eprintln!(
                    "redline-testing {} total={} passed={} failed={} skipped={} elapsed_ns={}",
                    args.suite.as_str(),
                    summary.total,
                    summary.passed,
                    summary.failed,
                    summary.skipped,
                    started.elapsed().as_nanos()
                );
            }
            Ok(())
        }
        Suite::BeyondSqlite => {
            prepare_output(&args.output)?;
            let summary = run_beyond_sqlite_suite(&args, args.output.clone())?;
            if progress_enabled(args.progress) {
                eprintln!(
                    "redline-testing beyond_sqlite total={} passed={} failed={} skipped={}",
                    summary.total, summary.passed, summary.failed, summary.skipped
                );
            }
            Ok(())
        }
    }
}

fn run_all_suites(
    args: &RunArgs,
    workers: usize,
    tmp_root: PathBuf,
    sqlite_bin: PathBuf,
) -> Result<()> {
    let generated_at_unix_ms = evidence::now_unix_ms();
    let output_dir = args
        .output
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(output_dir).with_context(|| format!("create {}", output_dir.display()))?;
    let sqlite_output = output_dir.join("sqlite_parity.raw.jsonl");
    let memory_output = output_dir.join("memory.raw.jsonl");
    let rql_output = output_dir.join("rql_phase1.raw.jsonl");
    let beyond_output = output_dir.join("beyond_sqlite.raw.jsonl");

    prepare_output(&sqlite_output)?;
    let sqlite_summary = run_sqlite_like_suite(
        args,
        Suite::SqliteParity,
        sqlite_output.clone(),
        workers,
        tmp_root.clone(),
        sqlite_bin.clone(),
    )?;
    prepare_output(&memory_output)?;
    let memory_summary = run_sqlite_like_suite(
        args,
        Suite::Memory,
        memory_output.clone(),
        workers,
        tmp_root.clone(),
        sqlite_bin.clone(),
    )?;
    prepare_output(&rql_output)?;
    let rql_summary = run_sqlite_like_suite(
        args,
        Suite::RqlPhase1,
        rql_output.clone(),
        workers,
        tmp_root.clone(),
        sqlite_bin.clone(),
    )?;
    prepare_output(&beyond_output)?;
    let beyond_summary = run_beyond_sqlite_suite(args, beyond_output.clone())?;

    let mut combined = String::new();
    for path in [&sqlite_output, &memory_output, &rql_output, &beyond_output] {
        combined.push_str(
            &fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?,
        );
    }
    fs::write(&args.output, combined)
        .with_context(|| format!("write {}", args.output.display()))?;
    let all_manifest = write_all_manifest(
        output_dir,
        &args.output,
        [
            (
                "sqlite_parity",
                sqlite_output.as_path(),
                sqlite_summary.clone(),
            ),
            ("memory", memory_output.as_path(), memory_summary.clone()),
            ("rql_phase1", rql_output.as_path(), rql_summary.clone()),
            (
                "beyond_sqlite",
                beyond_output.as_path(),
                beyond_summary.clone(),
            ),
        ],
    )?;
    evidence::write_official_evidence(OfficialEvidenceConfig {
        output_dir: output_dir.to_path_buf(),
        all_output: args.output.clone(),
        all_manifest,
        target_bin: args.target_bin.clone(),
        sqlite_bin,
        tmp_root,
        workers: workers.to_string(),
        repetitions: args.repetitions,
        warmup: args.warmup,
        memory_samples: args.memory_samples,
        command_line: std::env::args().collect::<Vec<_>>(),
        generated_at_unix_ms,
        suites: vec![
            OfficialSuiteEvidence::new(
                "sqlite_parity",
                sqlite_output,
                evidence::suite_artifact_path(output_dir, "sqlite_parity", "summary.json"),
                evidence::suite_artifact_path(output_dir, "sqlite_parity", "ranked.csv"),
                evidence::suite_artifact_path(output_dir, "sqlite_parity", "manifest.json"),
                evidence::suite_artifact_path(output_dir, "sqlite_parity", "provenance.json"),
                &sqlite_summary,
            ),
            OfficialSuiteEvidence::new(
                "memory",
                memory_output,
                evidence::suite_artifact_path(output_dir, "memory", "summary.json"),
                evidence::suite_artifact_path(output_dir, "memory", "ranked.csv"),
                evidence::suite_artifact_path(output_dir, "memory", "manifest.json"),
                evidence::suite_artifact_path(output_dir, "memory", "provenance.json"),
                &memory_summary,
            ),
            OfficialSuiteEvidence::new(
                "rql_phase1",
                rql_output,
                evidence::suite_artifact_path(output_dir, "rql_phase1", "summary.json"),
                evidence::suite_artifact_path(output_dir, "rql_phase1", "ranked.csv"),
                evidence::suite_artifact_path(output_dir, "rql_phase1", "manifest.json"),
                evidence::suite_artifact_path(output_dir, "rql_phase1", "provenance.json"),
                &rql_summary,
            ),
            OfficialSuiteEvidence::new(
                "beyond_sqlite",
                beyond_output,
                evidence::suite_artifact_path(output_dir, "beyond_sqlite", "summary.json"),
                evidence::suite_artifact_path(output_dir, "beyond_sqlite", "ranked.csv"),
                evidence::suite_artifact_path(output_dir, "beyond_sqlite", "manifest.json"),
                evidence::suite_artifact_path(output_dir, "beyond_sqlite", "provenance.json"),
                &beyond_summary,
            ),
        ],
    })
}

fn run_sqlite_like_suite(
    args: &RunArgs,
    suite: Suite,
    output: PathBuf,
    workers: usize,
    tmp_root: PathBuf,
    sqlite_bin: PathBuf,
) -> Result<sqlite_parity::RunSummary> {
    let memory_samples = args.memory_samples || matches!(suite, Suite::Memory);
    let started_unix_ms = evidence::now_unix_ms();
    let summary = if matches!(suite, Suite::RqlPhase1) {
        sqlite_parity::run_rql_phase1(sqlite_parity::RqlPhase1RunConfig {
            reference_bin: sqlite_bin.clone(),
            target_bin: args.target_bin.clone(),
            output: output.clone(),
            tmp_root: tmp_root.clone(),
            workers,
            repetitions: args.repetitions,
            warmup: args.warmup,
            progress: progress_enabled(args.progress),
            memory_samples,
        })?
    } else {
        sqlite_parity::run(sqlite_parity::RunConfig {
            reference_bin: sqlite_bin.clone(),
            target_bin: args.target_bin.clone(),
            output: output.clone(),
            tmp_root: tmp_root.clone(),
            workers,
            repetitions: args.repetitions,
            warmup: args.warmup,
            progress: progress_enabled(args.progress),
            memory_samples,
        })?
    };
    evidence::write_sqlite_parity_evidence(EvidenceConfig {
        suite: suite.as_str().to_owned(),
        output,
        target_bin: args.target_bin.clone(),
        sqlite_bin,
        tmp_root,
        workers: workers.to_string(),
        repetitions: args.repetitions,
        warmup: args.warmup,
        memory_samples,
        command_line: std::env::args().collect::<Vec<_>>(),
        started_unix_ms,
        ended_unix_ms: evidence::now_unix_ms(),
        summary: summary.clone(),
    })?;
    Ok(summary)
}

fn run_beyond_sqlite_suite(args: &RunArgs, output: PathBuf) -> Result<sqlite_parity::RunSummary> {
    let started_unix_ms = evidence::now_unix_ms();
    beyond_sqlite::run(beyond_sqlite::RunConfig {
        target_bin: args.target_bin.clone(),
        output,
        command_line: std::env::args().collect::<Vec<_>>(),
        started_unix_ms,
        ended_unix_ms: evidence::now_unix_ms(),
    })
}

fn write_all_manifest<'a>(
    output_dir: &Path,
    output: &Path,
    summaries: impl IntoIterator<Item = (&'a str, &'a Path, sqlite_parity::RunSummary)>,
) -> Result<PathBuf> {
    let suites = summaries
        .into_iter()
        .map(|(suite, raw_path, summary)| {
            serde_json::json!({
                "suite": suite,
                "raw": raw_path.display().to_string(),
                "total": summary.total,
                "passed": summary.passed,
                "failed": summary.failed,
                "skipped": summary.skipped
            })
        })
        .collect::<Vec<_>>();
    let manifest = serde_json::json!({
        "schema_version": "redline-testing-all-manifest-v1",
        "suite": "all",
        "raw": output.display().to_string(),
        "suites": suites
    });
    let manifest_path = output_dir.join("all-manifest.json");
    fs::write(
        &manifest_path,
        format!("{}\n", serde_json::to_string_pretty(&manifest)?),
    )
    .with_context(|| format!("write {}", manifest_path.display()))?;
    Ok(manifest_path)
}

fn report(args: ReportArgs) -> Result<()> {
    report::generate(ReportOptions {
        suite: args.suite.as_str().to_owned(),
        input: args.input,
        official_evidence: args.official_evidence,
        local_diagnostics: args.local_diagnostics,
        out_dir: args.out_dir,
        readme: args.readme,
        plot: args.plot,
        ksloc_plot: args.ksloc_plot,
        performance_histogram_plot: args.performance_histogram_plot,
        median_test_performance_plot: args.median_test_performance_plot,
        jankurai_score: args.jankurai_score,
        jankurai_comparison: args.jankurai_comparison,
        jankurai_comparison_plot: args.jankurai_comparison_plot,
        jankurai_score_plot: args.jankurai_score_plot,
        code_shape_plot: args.code_shape_plot,
        updated_date: args.updated_date,
        expected_repetitions: args.expected_repetitions,
        expected_warmup: args.expected_warmup,
        check: args.check,
    })
}

fn list(args: ListArgs) -> Result<()> {
    if matches!(args.suite, Suite::BeyondSqlite) {
        return list_beyond_sqlite(args.format);
    }
    let selected = match args.suite {
        Suite::All | Suite::SqliteParity | Suite::Memory => sqlite_parity::all_cases()?,
        Suite::RqlPhase1 => sqlite_parity::rql_phase1_cases()?,
        Suite::BeyondSqlite => unreachable!("handled above"),
    };
    match args.format {
        ListFormat::Text => {
            for case in selected {
                println!(
                    "{} {} {} {} {}",
                    case.display_id(),
                    case.priority,
                    case.profile,
                    case.category,
                    case.name
                );
            }
        }
        ListFormat::Markdown => {
            println!("# SQLite Parity Test Index\n");
            println!("| ID | Priority | Profile | Category | Name | Case file |");
            println!("| --- | --- | --- | --- | --- | --- |");
            for case in selected {
                println!(
                    "| {} | {} | {} | {} | {} | `{}` |",
                    case.display_id(),
                    case.priority,
                    case.profile,
                    case.category,
                    case.name,
                    case.case_file_name()
                );
            }
        }
        ListFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&selected)?);
        }
    }
    Ok(())
}

fn list_beyond_sqlite(format: ListFormat) -> Result<()> {
    let features = beyond_sqlite::all_features()?;
    match format {
        ListFormat::Text => {
            for feature in features {
                println!(
                    "BEYOND-{:03} {} {} {}",
                    feature.rank,
                    feature.status_string(),
                    feature.proof_lane,
                    feature.title
                );
            }
        }
        ListFormat::Markdown => {
            println!("# Beyond-SQLite Feature Index\n");
            println!("| ID | Status | Owner | Proof lane | Title |");
            println!("| --- | --- | --- | --- | --- |");
            for feature in features {
                println!(
                    "| BEYOND-{:03} | {} | {} | {} | {} |",
                    feature.rank,
                    feature.status_string(),
                    feature.owner,
                    feature.proof_lane,
                    feature.title
                );
            }
        }
        ListFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&features)?);
        }
    }
    Ok(())
}

fn jankurai_compare(args: JankuraiCompareArgs) -> Result<()> {
    report::jankurai_compare(JankuraiCompareOptions {
        redlinedb_score: args.redlinedb_score,
        sqlite_score: args.sqlite_score,
        sqlite_ref: args.sqlite_ref,
        updated_date: args.updated_date,
        json: args.json,
        csv: args.csv,
        check: args.check,
    })
}

fn sentinel(args: SentinelArgs) -> Result<()> {
    report::sentinel(SentinelOptions {
        input: args.input,
        ceiling_ns: args.ceiling_ns,
        enforce: args.enforce,
    })
}

impl Suite {
    fn as_str(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::SqliteParity => "sqlite_parity",
            Self::Memory => "memory",
            Self::RqlPhase1 => "rql_phase1",
            Self::BeyondSqlite => "beyond_sqlite",
        }
    }
}

impl beyond_sqlite::Feature {
    fn status_string(&self) -> &'static str {
        match self.status {
            beyond_sqlite::FeatureStatus::ManifestBacklog => "manifest_backlog",
            beyond_sqlite::FeatureStatus::PassingReference => "passing_reference",
        }
    }
}

fn resolve_workers(value: &str) -> Result<usize> {
    if value == "auto" {
        return Ok(std::thread::available_parallelism()
            .map(usize::from)
            .unwrap_or(1)
            .max(1));
    }
    let workers = value
        .parse::<usize>()
        .map_err(|_| anyhow::anyhow!("--workers must be `auto` or a positive integer"))?;
    if workers == 0 {
        bail!("--workers must be positive");
    }
    Ok(workers)
}

fn validate_samples(repetitions: usize, warmup: usize) -> Result<()> {
    if repetitions == 0 {
        bail!("--repetitions must be positive");
    }
    if warmup > 1000 {
        bail!("--warmup is unreasonably large");
    }
    Ok(())
}

fn prepare_output(output: &Path) -> Result<()> {
    if let Some(parent) = output.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)
            .with_context(|| format!("create output parent {}", parent.display()))?;
    }
    fs::write(output, "").with_context(|| format!("truncate output {}", output.display()))
}

fn resolve_tmp_root(raw: &str) -> Result<PathBuf> {
    if raw != "auto" {
        return Ok(PathBuf::from(raw));
    }
    if let Some(path) = std::env::var_os("REDLINE_TESTING_TMPDIR")
        && !path.is_empty()
    {
        return Ok(PathBuf::from(path));
    }
    let shm = Path::new("/dev/shm/redline-testing");
    if is_writable_dir(shm) {
        return Ok(shm.to_path_buf());
    }
    Ok(std::env::temp_dir().join("redline-testing"))
}

fn resolve_sqlite_bin(raw: &str) -> PathBuf {
    if raw == "auto" {
        PathBuf::from("sqlite3")
    } else {
        PathBuf::from(raw)
    }
}

fn progress_enabled(mode: ProgressMode) -> bool {
    match mode {
        ProgressMode::Always => true,
        ProgressMode::Never => false,
        ProgressMode::Auto => std::io::IsTerminal::is_terminal(&std::io::stderr()),
    }
}

fn is_writable_dir(path: &Path) -> bool {
    if !path.is_dir() {
        return false;
    }
    let probe = path.join(format!(".redline-testing-{}", std::process::id()));
    match fs::File::create(&probe) {
        Ok(_) => {
            let _ = fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}
