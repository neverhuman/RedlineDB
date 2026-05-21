use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{Result, bail};

use super::case::Case;
use super::engine::{EngineOutput, EngineSpec, SkippedCase, default_tmp_root};
use super::normalize::normalize_output;
use super::report;

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
            let artifact = report::write_failure_artifact(case, &[&output], &reason.to_string())?;
            eprintln!(
                "sqlite_parity failure case={} reason={} artifact={}",
                case.display_id(),
                reason,
                artifact.display()
            );
            Some(artifact)
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
    warmup: usize,
    repetitions: usize,
    sqlite_version: Option<String>,
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
                sqlite_version.clone(),
                "skipped",
                Some(artifact),
                Some(skipped_case.reason.clone()),
            ),
        )?;
    }
    let total_samples = warmup.saturating_add(repetitions);
    for case in cases {
        summary.total += 1;
        let mut case_failed = false;
        for sample_index in 0..total_samples {
            let measured_index = sample_index.checked_sub(warmup);
            let sample_role = if let Some(index) = measured_index {
                format!("measured:{}", index.saturating_add(1))
            } else {
                "warmup".to_owned()
            };
            let reference_output = reference.run_case(case, &tmp_root)?;
            let target_output = target.run_case(case, &tmp_root)?;
            let status = validate_compare(case, &reference_output, &target_output);
            let artifact = if let Err(reason) = &status {
                let artifact = report::write_failure_artifact(
                    case,
                    &[&reference_output, &target_output],
                    &reason.to_string(),
                )?;
                eprintln!(
                    "sqlite_parity failure case={} reason={} artifact={}",
                    case.display_id(),
                    reason,
                    artifact.display()
                );
                Some(artifact)
            } else {
                None
            };
            report::append_jsonl(
                out,
                &report::compare_record(
                    case,
                    &reference_output,
                    &target_output,
                    sample_index,
                    measured_index.map(|index| index.saturating_add(1)),
                    sample_role,
                    sqlite_version.clone(),
                    if status.is_ok() { "passed" } else { "failed" },
                    artifact,
                    status.as_ref().err().map(|reason| reason.to_string()),
                ),
            )?;
            if measured_index.is_some() {
                summary
                    .slowest
                    .push((case.display_id(), target_output.elapsed.as_nanos()));
            }
            if status.is_err() {
                case_failed = true;
            }
        }
        if case_failed {
            summary.failed += 1;
        } else {
            summary.passed += 1;
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
    let reference_stdout = normalize_compare_output(case, reference, &reference.stdout);
    let target_stdout = normalize_compare_output(case, target, &target.stdout);
    if reference_stdout != target_stdout {
        bail!("stdout mismatch: reference `{reference_stdout}`, target `{target_stdout}`");
    }
    if reference.status_code != Some(0) || case.status == "catalog_only" {
        return Ok(());
    }
    let reference_stderr = normalize_compare_output(case, reference, &reference.stderr);
    let target_stderr = normalize_compare_output(case, target, &target.stderr);
    if reference_stderr != target_stderr {
        bail!("stderr mismatch: reference `{reference_stderr}`, target `{target_stderr}`");
    }
    Ok(())
}

fn normalize_compare_output(case: &Case, output: &EngineOutput, value: &str) -> String {
    let normalized = normalize_output(value);
    let marker = format!(
        "/{}-{}-{}",
        case.display_id(),
        sanitize_engine_name(&output.engine),
        std::process::id()
    );
    normalized.replace(&marker, "/{{CASE_TMP}}")
}

fn sanitize_engine_name(value: &str) -> String {
    value
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect()
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
