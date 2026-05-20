use std::path::PathBuf;

use anyhow::{Result, bail};
use clap::{Args, Parser, Subcommand, ValueEnum};

use super::catalog;
use super::engine::{EngineSpec, resolve_engine_bin};
use super::filter::Selection;
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
}

#[derive(Debug, Args)]
struct SelectArgs {
    #[arg(long)]
    priorities: Option<String>,
    #[arg(long)]
    profiles: Option<String>,
    #[arg(long)]
    include_quarantine: bool,
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
    runner::run_cases(&cases, &engine, args.out.as_deref(), args.tmp_dir)?;
    Ok(())
}

fn compare_selected(args: CompareArgs) -> Result<()> {
    validate_jobs(&args.jobs)?;
    let cases = selected_cases(&args.select)?;
    let reference = EngineSpec::new(args.reference_name, args.reference_bin);
    let target = EngineSpec::new(args.target_name, args.target_bin);
    runner::compare_cases(
        &cases,
        &reference,
        &target,
        args.out.as_deref(),
        args.tmp_dir,
    )?;
    Ok(())
}

fn selected_cases(args: &SelectArgs) -> Result<Vec<super::case::Case>> {
    let selection = args.selection()?;
    let cases = catalog::all_cases()?
        .into_iter()
        .filter(|case| selection.matches(case))
        .collect::<Vec<_>>();
    if cases.is_empty() {
        bail!("sqlite parity selection matched zero cases");
    }
    Ok(cases)
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
