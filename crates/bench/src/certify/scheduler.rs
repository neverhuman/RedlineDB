use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};

use crate::config::{CertifyArgs, CompareConfig, RunSpec};
use crate::engine::engine_name;
use crate::report::RunRecord;

/// Lane BH P0 #1: cores reserved for the OS / parent harness when
/// the parallel certify scheduler decides how many children to
/// spawn concurrently. Empirically xbabe1's 128-core box loses too
/// much accuracy when the scheduler allocates the entire machine
/// to children, leaving no headroom for the parent's own bookkeeping
/// and OS jitter.
pub const RESERVED_CORES: usize = 4;
pub const MAX_PARALLEL_THREADS_ENV: &str = "REDLINEDB_BENCH_MAX_PARALLEL_THREADS";

/// Decide how many threads we may concurrently dispatch to children.
///
/// Returns at least 1 even on tiny boxes so the scheduler always
/// makes forward progress.
pub fn available_cores() -> usize {
    let detected = num_cpus::get().saturating_sub(RESERVED_CORES).max(1);
    match std::env::var(MAX_PARALLEL_THREADS_ENV)
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0)
    {
        Some(cap) => detected.min(cap).max(1),
        None => detected,
    }
}

/// Polling interval for the parallel scheduler's `try_wait` loop.
///
/// Small enough that finished children are reaped promptly; large
/// enough that the busy loop does not noticeably steal CPU from
/// running children.
const POLL_INTERVAL: Duration = Duration::from_millis(100);

/// A scheduled benchmark child the parallel scheduler will dispatch.
///
/// `is_warmup == true` means the resulting `RunRecord` is discarded
/// after the child exits; we still need to allocate a slot in the
/// scheduler so cache/disk priming actually happens.
#[derive(Debug, Clone)]
pub struct Job {
    pub spec: RunSpec,
    pub rep_idx: usize,
    pub is_warmup: bool,
}

/// Lane BH P0 #1: thin scheduler wrapper used by tests.
///
/// Takes a custom child-spawner closure so the unit test can
/// stand-in `sleep 1`-style fakes for the real bench child while
/// still exercising the bin-packing logic, polling, and reaping.
/// The scheduler returns `(reaped_count, max_in_flight_threads,
/// elapsed)` rather than `RunRecord`s — the test cares only about
/// throughput and parallelism, not record content.
pub fn dispatch_parallel_with_spawner<S>(
    jobs: Vec<Job>,
    available: usize,
    mut spawner: S,
) -> Result<SchedulerStats>
where
    S: FnMut(&Job) -> std::io::Result<Child>,
{
    let started = Instant::now();
    let mut queue: std::collections::VecDeque<(usize, Job)> =
        jobs.into_iter().enumerate().collect();
    let mut in_flight: Vec<(Child, usize)> = Vec::new();
    let mut reaped = 0_usize;
    let mut max_in_flight_threads = 0_usize;

    while !queue.is_empty() || !in_flight.is_empty() {
        loop {
            if queue.is_empty() {
                break;
            }
            let in_flight_threads: usize = in_flight.iter().map(|(_, t)| *t).sum();
            let free = available.saturating_sub(in_flight_threads);
            let next_threads = queue.front().map(|(_, job)| job.spec.threads).unwrap_or(0);
            let fits = next_threads <= free || (in_flight.is_empty() && next_threads > available);
            if !fits {
                break;
            }
            let (_, job) = queue.pop_front().expect("non-empty");
            let child = spawner(&job)?;
            in_flight.push((child, job.spec.threads));
        }

        let in_flight_threads: usize = in_flight.iter().map(|(_, t)| *t).sum();
        if in_flight_threads > max_in_flight_threads {
            max_in_flight_threads = in_flight_threads;
        }

        let mut idx = 0;
        let mut reaped_any = false;
        while idx < in_flight.len() {
            match in_flight[idx].0.try_wait()? {
                Some(_status) => {
                    let _ = in_flight.remove(idx);
                    reaped += 1;
                    reaped_any = true;
                }
                None => idx += 1,
            }
        }

        if !reaped_any && !in_flight.is_empty() {
            std::thread::sleep(POLL_INTERVAL);
        }
    }

    Ok(SchedulerStats {
        reaped,
        max_in_flight_threads,
        elapsed: started.elapsed(),
    })
}

/// Lightweight summary returned by [`dispatch_parallel_with_spawner`].
#[derive(Debug, Clone, Copy)]
pub struct SchedulerStats {
    /// Total number of children that reached `Ok(_)` exit status.
    pub reaped: usize,
    /// Peak sum of `threads` across all simultaneously in-flight
    /// children. Compared against `available` to confirm the
    /// scheduler is actually parallelizing.
    pub max_in_flight_threads: usize,
    /// Wall-clock time from the first dispatch to the last reap.
    pub elapsed: Duration,
}

/// Build the full set of jobs implied by `(config, args)`.
///
/// Warmup jobs come first per combo so dispatch sees them at the
/// front of the queue; the dispatcher otherwise treats them
/// identically to measured jobs. The `seed` value baked into each
/// spec is `args.seed + rep_idx` (with warmup repetitions counted
/// in the same wraparound) so reruns are deterministic.
pub fn build_job_queue(
    config: &CompareConfig,
    args: &CertifyArgs,
    warmup: usize,
    measured: usize,
) -> Result<Vec<Job>> {
    let mut jobs = Vec::new();
    for &engine in &config.engines {
        for &workload in &config.workloads {
            for &durability in &config.durabilities {
                for &threads in &config.threads {
                    for w in 0..warmup {
                        let seed = args.seed.wrapping_add(w as u64);
                        let spec =
                            config.run_spec(&engine, &workload, &durability, threads, seed)?;
                        jobs.push(Job {
                            spec,
                            rep_idx: w,
                            is_warmup: true,
                        });
                    }
                    for rep in 0..measured {
                        let seed = args.seed.wrapping_add((warmup + rep) as u64);
                        let spec =
                            config.run_spec(&engine, &workload, &durability, threads, seed)?;
                        jobs.push(Job {
                            spec,
                            rep_idx: rep,
                            is_warmup: false,
                        });
                    }
                }
            }
        }
    }
    Ok(jobs)
}

/// In-flight child slot tracked by the scheduler.
struct InFlight {
    child: Child,
    threads_used: usize,
    job: Job,
    out_path: PathBuf,
    run_dir: PathBuf,
    strace_path: Option<PathBuf>,
    wrap_with_strace: bool,
    /// Index in the original job queue, used to preserve a stable
    /// output order across reruns even when the scheduler
    /// completes children out of dispatch order.
    queue_index: usize,
}

/// The aggregated outcome of a finalized child plus warmup flag.
#[derive(Debug)]
pub struct ScheduledOutcome {
    pub record: RunRecord,
    pub strace_path: Option<PathBuf>,
    pub is_warmup: bool,
    pub queue_index: usize,
}

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
/// sorted by their original job position so the resulting JSONL /
/// CSV is byte-stable across reruns.
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
