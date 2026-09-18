use std::path::{Path, PathBuf};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
    mpsc,
};
use std::time::{Duration, Instant};

use anyhow::{Result, bail};

use super::case::Case;
use super::engine::{EngineOutput, EngineSpec, SkippedCase};
use super::normalize::normalize_output;
use super::report;
use super::text::sanitize_identifier;

#[derive(Debug, Clone, Default)]
pub struct RunSummary {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub skipped: usize,
    pub elapsed: Duration,
    pub slowest: Vec<(String, u128)>,
}

pub fn compare_cases(
    cases: &[Case],
    skipped: &[SkippedCase],
    reference: &EngineSpec,
    target: &EngineSpec,
    out: &Path,
    tmp_root: impl AsRef<Path>,
    workers: usize,
    warmup: usize,
    repetitions: usize,
    sqlite_version: Option<String>,
    progress: bool,
    memory_samples: bool,
) -> Result<RunSummary> {
    let tmp_root = tmp_root.as_ref();
    let started = Instant::now();
    let mut summary = RunSummary::default();
    for skipped_case in skipped {
        summary.total += 1;
        summary.skipped += 1;
        let artifact = report::write_skip_artifact(&skipped_case.case, &skipped_case.reason)?;
        report::append_jsonl(
            Some(out),
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
    let case_runs = run_case_set(
        cases,
        reference,
        target,
        tmp_root.to_path_buf(),
        workers,
        total_samples,
        warmup,
        sqlite_version,
        progress,
        memory_samples,
    )?;
    for case_run in case_runs {
        summary.total += 1;
        for record in &case_run.records {
            report::append_jsonl(Some(out), record)?;
        }
        summary.slowest.extend(case_run.slowest);
        if case_run.failed {
            summary.failed += 1;
        } else {
            summary.passed += 1;
        }
    }
    summary.elapsed = started.elapsed();
    finish_summary(summary, progress)
}

struct CaseRun {
    records: Vec<report::CompareRecord>,
    failed: bool,
    slowest: Vec<(String, u128)>,
}

fn run_case_set(
    cases: &[Case],
    reference: &EngineSpec,
    target: &EngineSpec,
    tmp_root: PathBuf,
    workers: usize,
    total_samples: usize,
    warmup: usize,
    sqlite_version: Option<String>,
    progress: bool,
    memory_samples: bool,
) -> Result<Vec<CaseRun>> {
    if workers <= 1 || cases.len() <= 1 {
        return cases
            .iter()
            .map(|case| {
                run_one_case(
                    case,
                    reference,
                    target,
                    &tmp_root,
                    total_samples,
                    warmup,
                    sqlite_version.clone(),
                    progress,
                    memory_samples,
                )
            })
            .collect();
    }

    let workers = workers.min(cases.len());
    let cases = Arc::new(cases.to_vec());
    let next = Arc::new(AtomicUsize::new(0));
    let first_error = Arc::new(Mutex::new(None::<String>));
    let (tx, rx) = mpsc::channel::<(usize, Result<CaseRun>)>();
    let mut handles = Vec::with_capacity(workers);
    for _ in 0..workers {
        let cases = Arc::clone(&cases);
        let next = Arc::clone(&next);
        let first_error = Arc::clone(&first_error);
        let tx = tx.clone();
        let reference = reference.clone();
        let target = target.clone();
        let tmp_root = tmp_root.clone();
        let sqlite_version = sqlite_version.clone();
        handles.push(std::thread::spawn(move || {
            loop {
                if first_error.lock().is_ok_and(|guard| guard.is_some()) {
                    break;
                }
                let index = next.fetch_add(1, Ordering::SeqCst);
                let Some(case) = cases.get(index) else {
                    break;
                };
                let result = run_one_case(
                    case,
                    &reference,
                    &target,
                    &tmp_root,
                    total_samples,
                    warmup,
                    sqlite_version.clone(),
                    progress,
                    memory_samples,
                );
                if let Err(err) = &result
                    && let Ok(mut guard) = first_error.lock()
                {
                    *guard = Some(err.to_string());
                }
                if tx.send((index, result)).is_err() {
                    break;
                }
            }
        }));
    }
    drop(tx);

    let mut ordered = (0..cases.len()).map(|_| None).collect::<Vec<_>>();
    for (index, result) in rx {
        ordered[index] = Some(result);
    }
    for handle in handles {
        handle
            .join()
            .map_err(|_| anyhow::anyhow!("sqlite parity worker thread panicked"))?;
    }
    ordered
        .into_iter()
        .enumerate()
        .map(|(index, result)| {
            result.unwrap_or_else(|| {
                Err(anyhow::anyhow!("sqlite parity worker skipped case {index}"))
            })
        })
        .collect()
}

fn run_one_case(
    case: &Case,
    reference: &EngineSpec,
    target: &EngineSpec,
    tmp_root: &Path,
    total_samples: usize,
    warmup: usize,
    sqlite_version: Option<String>,
    progress: bool,
    memory_samples: bool,
) -> Result<CaseRun> {
    if progress {
        eprintln!("sqlite_parity case={} status=running", case.display_id());
    }
    let mut failed = false;
    let mut records = Vec::with_capacity(total_samples);
    let mut slowest = Vec::new();
    for sample_index in 0..total_samples {
        let measured_index = sample_index.checked_sub(warmup);
        let sample_role = if let Some(index) = measured_index {
            format!("measured:{}", index.saturating_add(1))
        } else {
            "warmup".to_owned()
        };
        let reference_output = reference.run_case(case, tmp_root, memory_samples)?;
        let target_output = target.run_case(case, tmp_root, memory_samples)?;
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
        records.push(report::compare_record(
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
        ));
        if measured_index.is_some() {
            slowest.push((case.display_id(), target_output.elapsed.as_nanos()));
        }
        if status.is_err() {
            failed = true;
        }
    }
    if progress {
        let status = if failed { "failed" } else { "passed" };
        eprintln!("sqlite_parity case={} status={status}", case.display_id());
    }
    Ok(CaseRun {
        records,
        failed,
        slowest,
    })
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
    let mut normalized = normalize_output(value);
    if case.id == 208 {
        normalized = normalized
            .lines()
            .filter(|line| !line.starts_with("trace.xRandomness("))
            .collect::<Vec<_>>()
            .join("\n");
    }
    let marker = format!(
        "/{}-{}-{}",
        case.display_id(),
        sanitize_identifier(&output.engine),
        std::process::id()
    );
    normalized.replace(&marker, "/{{CASE_TMP}}")
}

pub fn validate_compare_engines(reference: &EngineSpec, target: &EngineSpec) -> Result<()> {
    let reference_identity = reference.binary_identity()?;
    let target_identity = target.binary_identity()?;
    if reference_identity.executable_path == target_identity.executable_path
        || reference_identity.executable_sha256 == target_identity.executable_sha256
    {
        bail!(
            "sqlite parity compare requires distinct reference and target binaries: reference={} target={}",
            reference_identity.executable_path,
            target_identity.executable_path
        );
    }
    if target.name.eq_ignore_ascii_case("redlinedb")
        && !target_identity
            .version
            .to_ascii_lowercase()
            .contains("redlinedb")
    {
        bail!(
            "sqlite parity target `{}` must identify as RedlineDB via --version, got `{}` from {}",
            target.name,
            target_identity.version,
            target_identity.executable_path
        );
    }
    Ok(())
}

fn finish_summary(mut summary: RunSummary, progress: bool) -> Result<RunSummary> {
    summary.slowest.sort_by(|left, right| right.1.cmp(&left.1));
    summary.slowest.truncate(10);
    if progress {
        eprintln!(
            "sqlite_parity total={} passed={} failed={} skipped={} elapsed_ns={}",
            summary.total,
            summary.passed,
            summary.failed,
            summary.skipped,
            summary.elapsed.as_nanos()
        );
        eprintln!("sqlite_parity slowest={:?}", summary.slowest);
    }
    if summary.failed > 0 {
        bail!(
            "sqlite parity failed {} of {} cases",
            summary.failed,
            summary.total
        );
    }
    Ok(summary)
}
