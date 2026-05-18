use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};

use crate::engine::engine_name;
use crate::report::RunRecord;

use super::types::{InFlight, Job, POLL_INTERVAL, ScheduledOutcome};

/// Lane BH P0 #1: bin-packing parallel scheduler.
///
/// Reserves [`RESERVED_CORES`] for the OS/parent harness, then
/// greedily fits jobs onto the remaining cores by their `threads`
/// count. Behavior:
///   - 128-thread runs occupy the box alone.
///   - Mixed sizes pack greedily: a 64-thread job runs alongside
///     8× 8-thread jobs at 124 threads in flight (≤ 124 available).
///   - Jobs that require more than the total available core budget
///     still run, alone, with no stalling — the scheduler relaxes
///     its bin-pack guard for the head-of-queue when that job
///     individually exceeds `available`.
///
/// Output preserves dispatch order via `queue_index`: outcomes are
/// sorted by their original job position so the resulting JSONL / CSV
/// is byte-stable across reruns.
pub fn dispatch_parallel(
    jobs: Vec<Job>,
    raw_dir: &Path,
    with_strace: bool,
    available: usize,
) -> Result<Vec<ScheduledOutcome>> {
    let mut queue: std::collections::VecDeque<(usize, Job)> =
        jobs.into_iter().enumerate().collect();
    let mut in_flight: Vec<InFlight> = Vec::new();
    let mut outcomes: Vec<ScheduledOutcome> = Vec::new();

    while !queue.is_empty() || !in_flight.is_empty() {
        loop {
            if queue.is_empty() {
                break;
            }
            let in_flight_threads: usize = in_flight.iter().map(|s| s.threads_used).sum();
            let free = available.saturating_sub(in_flight_threads);
            let next_threads = queue.front().map(|(_, job)| job.spec.threads).unwrap_or(0);
            let fits = next_threads <= free || (in_flight.is_empty() && next_threads > available);
            if !fits {
                break;
            }
            let (queue_index, job) = queue.pop_front().expect("non-empty");
            let slot = spawn_child(&job, raw_dir, with_strace, queue_index)?;
            in_flight.push(slot);
        }

        let mut idx = 0;
        let mut reaped_any = false;
        while idx < in_flight.len() {
            let try_status = in_flight[idx]
                .child
                .try_wait()
                .with_context(|| "wal certify scheduler poll".to_owned())?;
            match try_status {
                Some(_status) => {
                    let slot = in_flight.remove(idx);
                    let outcome = finalize_child(slot)?;
                    outcomes.push(outcome);
                    reaped_any = true;
                }
                None => idx += 1,
            }
        }

        if !reaped_any && !in_flight.is_empty() {
            std::thread::sleep(POLL_INTERVAL);
        }
    }

    outcomes.sort_by_key(|o| o.queue_index);
    Ok(outcomes)
}

/// Spawn a single child for `job` and return the in-flight slot.
fn spawn_child(
    job: &Job,
    raw_dir: &Path,
    with_strace: bool,
    queue_index: usize,
) -> Result<InFlight> {
    let exe = std::env::current_exe().context("resolve bench executable")?;
    let warmup_tag = if job.is_warmup { "w" } else { "r" };
    let run_dir = raw_dir.join(format!(
        "{:?}-{}-{}-t{}-{}{}",
        job.spec.engine,
        job.spec.workload.as_str(),
        job.spec.durability.as_str(),
        job.spec.threads,
        warmup_tag,
        job.rep_idx
    ));
    fs::create_dir_all(&run_dir)?;
    let out_path = run_dir.join("record.json");

    let strace_path = run_dir.join("strace.txt");
    let wrap_with_strace = with_strace && cfg!(target_os = "linux") && which_strace();

    let mut command = if wrap_with_strace {
        let mut c = Command::new("strace");
        c.arg("-c").arg("-o").arg(&strace_path).arg("--").arg(&exe);
        c
    } else {
        Command::new(&exe)
    };
    let stdout_path = run_dir.join("stdout.log");
    let stderr_path = run_dir.join("stderr.log");
    let stdout_file = fs::File::create(&stdout_path)
        .with_context(|| format!("create child stdout log {}", stdout_path.display()))?;
    let stderr_file = fs::File::create(&stderr_path)
        .with_context(|| format!("create child stderr log {}", stderr_path.display()))?;
    let child = command
        .arg("run")
        .arg("--engine")
        .arg(engine_name(job.spec.engine))
        .arg("--workload")
        .arg(job.spec.workload.as_str())
        .arg("--durability")
        .arg(job.spec.durability.as_str())
        .arg("--threads")
        .arg(job.spec.threads.to_string())
        .arg("--rows")
        .arg(job.spec.rows.to_string())
        .arg("--seconds")
        .arg(job.spec.duration.as_secs().to_string())
        .arg("--cache-mib")
        .arg((job.spec.cache_bytes / (1024 * 1024)).max(1).to_string())
        .arg("--seed")
        .arg(job.spec.seed.to_string())
        .arg("--out")
        .arg(&out_path)
        .stdout(Stdio::from(stdout_file))
        .stderr(Stdio::from(stderr_file))
        .spawn()
        .with_context(|| format!("spawn certification child for {:?}", job.spec.engine))?;

    Ok(InFlight {
        child,
        threads_used: job.spec.threads,
        job: job.clone(),
        out_path,
        run_dir,
        strace_path: Some(strace_path),
        wrap_with_strace,
        queue_index,
    })
}

/// Wait for an exited slot's child and parse the resulting record.
fn finalize_child(mut slot: InFlight) -> Result<ScheduledOutcome> {
    let status = slot
        .child
        .wait()
        .with_context(|| format!("await child for {:?}", slot.job.spec.engine))?;
    if !status.success() {
        bail!(
            "certification child failed for {:?}: {} (run dir: {})",
            slot.job.spec.engine,
            status,
            slot.run_dir.display()
        );
    }
    let raw = fs::read_to_string(&slot.out_path)
        .with_context(|| format!("read run record {}", slot.out_path.display()))?;
    let record: RunRecord = serde_json::from_str(&raw)?;
    let strace_path = match (slot.wrap_with_strace, slot.strace_path.take()) {
        (true, Some(p)) if p.exists() => Some(p),
        _ => None,
    };
    Ok(ScheduledOutcome {
        record,
        strace_path,
        is_warmup: slot.job.is_warmup,
        queue_index: slot.queue_index,
    })
}

#[cfg(target_os = "linux")]
fn which_strace() -> bool {
    Command::new("which")
        .arg("strace")
        .output()
        .map(|out| out.status.success() && !out.stdout.is_empty())
        .unwrap_or(false)
}

#[cfg(not(target_os = "linux"))]
fn which_strace() -> bool {
    false
}
