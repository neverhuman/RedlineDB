use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use clap::{Args, Parser, Subcommand, ValueEnum};

use super::catalog;
use super::engine::{EngineSpec, partition_cases, resolve_engine_bin};
use super::filter::Selection;
use super::report_gen;
use super::runner;

#[derive(Debug, Parser)]
#[command(name = "sqlite_parity")]
pub struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    List(ListArgs),
    Run(RunArgs),
    Compare(CompareArgs),
    Report(ReportArgs),
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

impl SelectArgs {
    fn selection(&self) -> Result<Selection> {
        Selection::from_cli(
            self.priorities.as_deref(),
            self.profiles.as_deref(),
            self.include_quarantine,
        )
    }
}

#[derive(Debug, Args)]
struct ListArgs {
    #[command(flatten)]
    select: SelectArgs,
    #[arg(long, value_enum, default_value = "text")]
    format: ListFormat,
}

#[derive(Debug, Args)]
struct RunArgs {
    #[command(flatten)]
    select: SelectArgs,
    #[arg(long)]
    sqlite_bin: Option<PathBuf>,
    #[arg(long)]
    target_bin: Option<PathBuf>,
    #[arg(long)]
    engine_name: String,
    #[arg(long)]
    out: Option<PathBuf>,
    #[arg(long)]
    tmp_dir: Option<PathBuf>,
    #[arg(long, default_value = "auto")]
    jobs: String,
}

#[derive(Debug, Args)]
struct CompareArgs {
    #[command(flatten)]
    select: SelectArgs,
    #[arg(long, default_value = "sqlite3")]
    reference_name: String,
    #[arg(long)]
    reference_bin: PathBuf,
    #[arg(long, default_value = "redlinedb")]
    target_name: String,
    #[arg(long)]
    target_bin: PathBuf,
    #[arg(long)]
    out: Option<PathBuf>,
    #[arg(long)]
    tmp_dir: Option<PathBuf>,
    #[arg(long, default_value = "auto")]
    jobs: String,
    #[arg(long, default_value_t = 1)]
    repetitions: usize,
    #[arg(long, default_value_t = 0)]
    warmup: usize,
}

#[derive(Debug, Args)]
struct ReportArgs {
    #[arg(long)]
    input: PathBuf,
    #[arg(long)]
    case_list: PathBuf,
    #[arg(long)]
    out_dir: PathBuf,
    #[arg(long)]
    readme: PathBuf,
    #[arg(long)]
    plot: PathBuf,
    #[arg(long)]
    updated_date: String,
    #[arg(long)]
    check: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ListFormat {
    Text,
    Markdown,
    Json,
}

pub fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Command::List(args) => list(args),
        Command::Run(args) => run_selected(args),
        Command::Compare(args) => compare_selected(args),
        Command::Report(args) => report(args),
    }
}

fn list(args: ListArgs) -> Result<()> {
    let cases = selected_cases(&args.select)?;
    match args.format {
        ListFormat::Text => {
            for case in cases {
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
            println!("Canonical generated registry for `redlinedb-bench --bin sqlite_parity`.\n");
            println!("| ID | Priority | Profile | Category | Name | Case file |");
            println!("| --- | --- | --- | --- | --- | --- |");
            for case in cases {
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
            println!("{}", serde_json::to_string_pretty(&cases)?);
        }
    }
    Ok(())
}

fn run_selected(args: RunArgs) -> Result<()> {
    validate_jobs(&args.jobs)?;
    let cases = selected_cases(&args.select)?;
    let bin = resolve_engine_bin(&args.engine_name, args.sqlite_bin, args.target_bin)?;
    let engine = EngineSpec::new(args.engine_name, bin);
    let capabilities = engine.sqlite_shell_capabilities()?;
    let partition = partition_cases(cases, capabilities.as_ref());
    runner::run_cases(
        &partition.runnable,
        &partition.skipped,
        &engine,
        args.out.as_deref(),
        args.tmp_dir,
    )?;
    Ok(())
}

fn compare_selected(args: CompareArgs) -> Result<()> {
    validate_jobs(&args.jobs)?;
    validate_samples(args.repetitions, args.warmup)?;
    let cases = selected_cases(&args.select)?;
    let reference = EngineSpec::new(args.reference_name, args.reference_bin);
    let target = EngineSpec::new(args.target_name, args.target_bin);
    let capabilities = reference.sqlite_shell_capabilities()?;
    let sqlite_version = capabilities
        .as_ref()
        .map(|capabilities| capabilities.version.clone());
    let partition = partition_cases(cases, capabilities.as_ref());
    runner::compare_cases(
        &partition.runnable,
        &partition.skipped,
        &reference,
        &target,
        args.out.as_deref(),
        args.tmp_dir,
        args.warmup,
        args.repetitions,
        sqlite_version,
    )?;
    Ok(())
}

fn report(args: ReportArgs) -> Result<()> {
    report_gen::generate(report_gen::ReportOptions {
        input: args.input,
        case_list: args.case_list,
        out_dir: args.out_dir,
        readme: args.readme,
        plot: args.plot,
        updated_date: args.updated_date,
        check: args.check,
        command: std::env::args().collect(),
    })
}

fn selected_cases(args: &SelectArgs) -> Result<Vec<super::case::Case>> {
    let selection = args.selection()?;
    let case_list = args.case_list.as_deref().map(parse_case_list).transpose()?;
    let all_cases = catalog::all_cases()?;
    if let Some(ids) = &case_list {
        validate_known_case_ids(ids, &all_cases)?;
    }
    let cases = all_cases
        .into_iter()
        .filter(|case| selection.matches(case))
        .filter(|case| {
            case_list
                .as_ref()
                .is_none_or(|ids| ids.contains(&case.display_id()))
        })
        .collect::<Vec<_>>();
    if cases.is_empty() {
        bail!("sqlite parity selection matched zero cases");
    }
    Ok(cases)
}

pub(crate) fn parse_case_list(path: &Path) -> Result<BTreeSet<String>> {
    let text = fs::read_to_string(path)
        .with_context(|| format!("read sqlite parity case list {}", path.display()))?;
    parse_case_list_text(&text)
}

pub(crate) fn parse_case_list_text(text: &str) -> Result<BTreeSet<String>> {
    let mut ids = BTreeSet::new();
    for (index, line) in text.lines().enumerate() {
        let id = line
            .split_once('#')
            .map_or(line, |(prefix, _)| prefix)
            .trim();
        if id.is_empty() {
            continue;
        }
        if id.len() != 5 || !id.bytes().all(|byte| byte.is_ascii_digit()) {
            bail!(
                "invalid sqlite parity case id `{id}` at {}",
                index.saturating_add(1)
            );
        }
        if !ids.insert(id.to_owned()) {
            bail!(
                "duplicate sqlite parity case id `{id}` at {}",
                index.saturating_add(1)
            );
        }
    }
    if ids.is_empty() {
        bail!("sqlite parity case list is empty");
    }
    Ok(ids)
}

pub(crate) fn validate_known_case_ids(
    ids: &BTreeSet<String>,
    cases: &[super::case::Case],
) -> Result<()> {
    let known = cases
        .iter()
        .map(super::case::Case::display_id)
        .collect::<BTreeSet<_>>();
    let unknown = ids
        .iter()
        .filter(|id| !known.contains(*id))
        .cloned()
        .collect::<Vec<_>>();
    if !unknown.is_empty() {
        bail!("unknown sqlite parity case ids: {}", unknown.join(", "));
    }
    Ok(())
}

fn validate_jobs(value: &str) -> Result<()> {
    if value == "auto" {
        return Ok(());
    }
    let jobs = value
        .parse::<usize>()
        .map_err(|_| anyhow::anyhow!("--jobs must be `auto` or a positive integer"))?;
    if jobs == 0 {
        bail!("--jobs must be positive");
    }
    Ok(())
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

#[cfg(test)]
mod tests {
    use std::io::Write;

    use tempfile::NamedTempFile;

    use super::*;

    #[test]
    fn parses_case_list_with_blank_lines_and_comments() {
        let ids = parse_case_list_text(
            r#"
                # approved cases
                00004

                00005 # inline note
                00009
            "#,
        )
        .expect("parse case list");

        assert_eq!(
            ids.into_iter().collect::<Vec<_>>(),
            vec!["00004", "00005", "00009"]
        );
    }

    #[test]
    fn rejects_malformed_case_list_ids() {
        let err = parse_case_list_text("4\n").expect_err("reject non-5-digit id");
        assert!(err.to_string().contains("invalid sqlite parity case id"));
    }

    #[test]
    fn rejects_duplicate_case_list_ids() {
        let err = parse_case_list_text("00004\n00004\n").expect_err("reject duplicate id");
        assert!(err.to_string().contains("duplicate sqlite parity case id"));
    }

    #[test]
    fn unknown_case_list_ids_are_hard_errors() {
        let mut file = NamedTempFile::new().expect("temp case list");
        writeln!(file, "00004").expect("write known id");
        writeln!(file, "99999").expect("write unknown id");

        let args = SelectArgs {
            priorities: None,
            profiles: None,
            include_quarantine: false,
            case_list: Some(file.path().to_path_buf()),
        };

        let err = selected_cases(&args).expect_err("unknown id must fail");
        assert!(err.to_string().contains("unknown sqlite parity case ids"));
    }

    #[test]
    fn case_list_zero_match_keeps_hard_error() {
        let mut file = NamedTempFile::new().expect("temp case list");
        writeln!(file, "00004").expect("write known id");

        let args = SelectArgs {
            priorities: Some("P4".to_owned()),
            profiles: None,
            include_quarantine: true,
            case_list: Some(file.path().to_path_buf()),
        };

        let err = selected_cases(&args).expect_err("zero-match selection must fail");
        assert!(err.to_string().contains("matched zero cases"));
    }
}
