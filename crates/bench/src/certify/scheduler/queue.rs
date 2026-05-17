use std::collections::VecDeque;
use std::process::Child;
use std::time::Instant;

use anyhow::Result;

use crate::config::{CertifyArgs, CompareConfig};

use super::types::{Job, MAX_PARALLEL_THREADS_ENV, POLL_INTERVAL, RESERVED_CORES, SchedulerStats};

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
    let mut queue: VecDeque<(usize, Job)> = jobs.into_iter().enumerate().collect();
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
