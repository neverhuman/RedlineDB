use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{Result, bail};

use super::case::Case;
use super::engine::{EngineOutput, EngineSpec, SkippedCase, default_tmp_root};
use super::normalize::normalize_output;
use super::report::{self, CompareRecord};

#[derive(Debug, Default)]
pub struct RunSummary {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub skipped: usize,
    pub elapsed: Duration,
    pub slowest: Vec<(String, u128)>,
}

pub fn run_cases(
    cases: &[Case],
    skipped: &[SkippedCase],
    engine: &EngineSpec,
    out: Option<&Path>,
    tmp_root: Option<PathBuf>,
) -> Result<RunSummary> {
    let tmp_root = match tmp_root {
        Some(tmp_root) => tmp_root,
        None => default_tmp_root(),
    };
    let started = Instant::now();
    let mut summary = RunSummary::default();
    for skipped_case in skipped {
        summary.total += 1;
        summary.skipped += 1;
        let artifact = report::write_skip_artifact(&skipped_case.case, &skipped_case.reason)?;
        report::append_jsonl(
            out,
            &report::skipped_case_record(
                &skipped_case.case,
                &engine.name,
                "skipped",
                Some(artifact),
                Some(skipped_case.reason.clone()),
            ),
        )?;
    }
    for case in cases {
        summary.total += 1;
        let output = engine.run_case(case, &tmp_root)?;
        let status = validate_case(case, &output);
        let artifact = if let Err(reason) = &status {
            Some(report::write_failure_artifact(
                case,
                &[&output],
                &reason.to_string(),
            )?)
        } else {
            None
        };
        report::append_jsonl(
            out,
            &report::case_record(
                case,
                &output,
                if status.is_ok() { "passed" } else { "failed" },
                artifact,
                status.as_ref().err().map(|reason| reason.to_string()),
            ),
        )?;
        summary
            .slowest
            .push((case.display_id(), output.elapsed.as_nanos()));
        if status.is_ok() {
            summary.passed += 1;
        } else {
            summary.failed += 1;
        }
    }
    summary.elapsed = started.elapsed();
    finish_summary(summary)
}

pub fn compare_cases(
    cases: &[Case],
    skipped: &[SkippedCase],
    reference: &EngineSpec,
    target: &EngineSpec,
    out: Option<&Path>,
    tmp_root: Option<PathBuf>,
) -> Result<RunSummary> {
    let tmp_root = tmp_root.unwrap_or_else(default_tmp_root);
    let started = Instant::now();
    let mut summary = RunSummary::default();
    for skipped_case in skipped {
        summary.total += 1;
        summary.skipped += 1;
        let artifact = report::write_skip_artifact(&skipped_case.case, &skipped_case.reason)?;
        report::append_jsonl(
            out,
            &report::skipped_compare_record(
                &skipped_case.case,
                &reference.name,
                &target.name,
                "skipped",
                Some(artifact),
                Some(skipped_case.reason.clone()),
            ),
        )?;
    }
    for case in cases {
        summary.total += 1;
        let reference_output = reference.run_case(case, &tmp_root)?;
        let target_output = target.run_case(case, &tmp_root)?;
        let status = validate_case(case, &reference_output)
            .and_then(|_| validate_case(case, &target_output))
            .and_then(|_| validate_compare(case, &reference_output, &target_output));
        let artifact = if let Err(reason) = &status {
            Some(report::write_failure_artifact(
                case,
                &[&reference_output, &target_output],
                &reason.to_string(),
            )?)
        } else {
            None
        };
        let ratio = latency_ratio(reference_output.elapsed, target_output.elapsed);
        report::append_jsonl(
            out,
            &CompareRecord {
                case_id: case.display_id(),
                name: case.name.clone(),
                priority: case.priority.to_string(),
                profile: case.profile.to_string(),
                category: case.category.clone(),
                reference_engine: reference_output.engine.clone(),
                target_engine: target_output.engine.clone(),
                status: if status.is_ok() {
                    "passed".to_owned()
                } else {
                    "failed".to_owned()
                },
                reference_exit_code: reference_output.status_code,
                target_exit_code: target_output.status_code,
                reference_elapsed_ns: reference_output.elapsed.as_nanos(),
                target_elapsed_ns: target_output.elapsed.as_nanos(),
                latency_ratio: ratio,
                artifact_dir: artifact,
                diagnostic: status.as_ref().err().map(|reason| reason.to_string()),
            },
        )?;
        summary
            .slowest
            .push((case.display_id(), target_output.elapsed.as_nanos()));
        if status.is_ok() {
            summary.passed += 1;
        } else {
            summary.failed += 1;
        }
    }
    summary.elapsed = started.elapsed();
    finish_summary(summary)
}

fn validate_case(case: &Case, output: &EngineOutput) -> Result<()> {
    if output.status_code != Some(case.expected_exit) {
        bail!(
            "exit mismatch: expected {}, got {:?}",
            case.expected_exit,
            output.status_code
        );
    }
    let stdout = normalize_output(&output.stdout);
    if let Some(expected_stdout) = &case.expected_stdout {
        let expected_stdout = normalize_output(expected_stdout);
        if stdout != expected_stdout {
            bail!("stdout mismatch: expected `{expected_stdout}`, got `{stdout}`");
        }
    }
    let stderr = normalize_output(&output.stderr);
    for expected in &case.expected_stdout_contains {
        if !stdout.contains(expected) {
            bail!("stdout missing required substring `{expected}`");
        }
    }
    for expected in &case.expected_stderr_contains {
        if !stderr.contains(expected) {
            bail!("stderr missing required substring `{expected}`");
        }
    }
    let combined = format!("{stdout}\n{stderr}");
    for expected in &case.expected_combined_contains {
        if !combined.contains(expected) {
            bail!("combined output missing required substring `{expected}`");
        }
    }
    Ok(())
}

fn validate_compare(case: &Case, reference: &EngineOutput, target: &EngineOutput) -> Result<()> {
    if reference.status_code != target.status_code {
        bail!(
            "exit mismatch: reference {:?}, target {:?}",
            reference.status_code,
            target.status_code
        );
    }
    if !case.compare_stdout {
        return Ok(());
    }
    let reference_stdout = normalize_output(&reference.stdout);
    let target_stdout = normalize_output(&target.stdout);
    if reference_stdout != target_stdout {
        bail!("stdout mismatch: reference `{reference_stdout}`, target `{target_stdout}`");
    }
    if case.expected_exit != 0 {
        return Ok(());
    }
    let reference_stderr = normalize_output(&reference.stderr);
    let target_stderr = normalize_output(&target.stderr);
    if reference_stderr != target_stderr {
        bail!("stderr mismatch: reference `{reference_stderr}`, target `{target_stderr}`");
    }
    Ok(())
}

fn latency_ratio(reference: Duration, target: Duration) -> f64 {
    let reference_ns = reference.as_nanos().max(1) as f64;
    target.as_nanos() as f64 / reference_ns
}

fn finish_summary(mut summary: RunSummary) -> Result<RunSummary> {
    summary.slowest.sort_by(|left, right| right.1.cmp(&left.1));
    summary.slowest.truncate(10);
    eprintln!(
        "sqlite_parity total={} passed={} failed={} skipped={} elapsed_ns={}",
        summary.total,
        summary.passed,
        summary.failed,
        summary.skipped,
        summary.elapsed.as_nanos()
    );
    eprintln!("sqlite_parity slowest={:?}", summary.slowest);
    if summary.failed > 0 {
        bail!(
            "sqlite parity failed {} of {} cases",
            summary.failed,
            summary.total
        );
    }
    Ok(summary)
}
