pub mod certify;
mod chaos;
pub mod checksum;
pub mod config;
mod cross_engine;
mod engine;
pub mod failpoint_matrix;
mod gates;
mod metrics;
pub mod process_metrics;
mod recover;
pub mod report;
pub mod strace_capture;
pub mod workload;

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

pub use config::Cli;

pub fn run(cli: Cli) -> Result<()> {
    match cli.command {
        config::Command::Run(args) => {
            let record = workload::run_once(&args.into_run_spec()?)?;
            report::write_run_output(args.out.as_deref(), &record)?;
        }
        config::Command::Compare(args) => {
            let config = config::CompareConfig::load(&args.config)?;
            let records = run_compare(&config, args.seed)?;
            report::write_compare_output(args.out.as_deref(), args.report.as_deref(), &records)?;
        }
        config::Command::Certify(args) => {
            let config = config::CompareConfig::load(&args.config)?;
            let _ = certify::run(&config, &args)?;
        }
        config::Command::CrossEngine(args) => {
            let report = cross_engine::run_suite(&args)?;
            report::write_json(args.out.as_deref(), &report)?;
        }
        config::Command::Recover(args) => {
            let report = recover::run(&args)?;
            report::write_json(args.out.as_deref(), &report)?;
        }
        config::Command::RecoverMatrix(args) => {
            let report = recover::run_matrix(&args)?;
            report::write_json(args.out.as_deref(), &report)?;
        }
        config::Command::RecoverChild(args) => recover::run_child(&args)?,
        config::Command::FailpointMatrix(args) => {
            let report = failpoint_matrix::run(&args)?;
            failpoint_matrix::write_report(&args.out, &report)?;
            if !report.passed {
                bail!(
                    "failpoint-matrix gate failed: {} cases reported lost acked commits",
                    report.failed_cases
                );
            }
        }
        config::Command::FailpointChild(args) => failpoint_matrix::run_child(&args)?,
        config::Command::Gates(args) => {
            let records = report::read_jsonl(&args.input)?;
            let summary = gates::evaluate_records(&records);
            report::write_json(args.out.as_deref(), &summary)?;
        }
    }
    Ok(())
}

fn run_compare(config: &config::CompareConfig, seed: u64) -> Result<Vec<report::RunRecord>> {
    let mut records = Vec::new();
    fs::create_dir_all(&config.out_dir)?;
    for engine in &config.engines {
        for workload in &config.workloads {
            for durability in &config.durabilities {
                for &threads in &config.threads {
                    let spec = config.run_spec(engine, workload, durability, threads, seed)?;
                    let record = workload::run_once(&spec)?;
                    report::append_jsonl(&config.out_dir.join("compare.jsonl"), &record)?;
                    records.push(record);
                }
            }
        }
    }
    if records.is_empty() {
        bail!("compare config produced no benchmark runs");
    }
    Ok(records)
}

pub fn default_smoke_config_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("bench")
        .join("smoke.toml")
}

pub fn default_certification_config_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("bench")
        .join("certification.toml")
}

pub fn default_recovery_matrix_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("bench")
        .join("recovery-matrix.toml")
}

pub fn default_failpoint_matrix_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("bench")
        .join("failpoint-matrix.toml")
}

pub fn ensure_parent(path: Option<&Path>) -> Result<()> {
    if let Some(path) = path
        && let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)
            .with_context(|| format!("create parent directory for {}", path.display()))?;
    }
    Ok(())
}
