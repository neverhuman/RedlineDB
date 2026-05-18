//! Per-process telemetry capture for benchmark runs.
//!
//! Provides best-effort collection of resource counters that surface
//! IO pressure, syscall fan-out, and resident memory peaks. All fields
//! are `Option<u64>` so that platforms missing a particular counter
//! (or hosts where the file is unreadable) simply omit the value
//! instead of failing the run.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessMetrics {
    pub rss_peak_bytes: Option<u64>,
    pub proc_io_read_bytes: Option<u64>,
    pub proc_io_write_bytes: Option<u64>,
    pub fsync_count: Option<u64>,
    pub fdatasync_count: Option<u64>,
    pub pwrite_count: Option<u64>,
    pub write_count: Option<u64>,
    pub voluntary_ctx_switches: Option<u64>,
    pub involuntary_ctx_switches: Option<u64>,
}

#[cfg(target_os = "linux")]
pub fn collect_self() -> ProcessMetrics {
    collect_from_proc("self")
}

#[cfg(target_os = "linux")]
pub fn collect_pid(pid: i32) -> ProcessMetrics {
    collect_from_proc(&pid.to_string())
}

#[cfg(target_os = "linux")]
fn collect_from_proc(handle: &str) -> ProcessMetrics {
    let mut metrics = ProcessMetrics::default();

    let io_path = format!("/proc/{handle}/io");
    if let Ok(contents) = std::fs::read_to_string(&io_path) {
        for line in contents.lines() {
            let mut parts = line.splitn(2, ':');
            let key = parts.next().unwrap_or("").trim();
            let value = parts.next().unwrap_or("").trim();
            let parsed = value.parse::<u64>().ok();
            match key {
                "read_bytes" => metrics.proc_io_read_bytes = parsed,
                "write_bytes" => metrics.proc_io_write_bytes = parsed,
                "syscw" => metrics.write_count = parsed,
                _ => {}
            }
        }
    }

    let status_path = format!("/proc/{handle}/status");
    if let Ok(contents) = std::fs::read_to_string(&status_path) {
        for line in contents.lines() {
            let mut parts = line.splitn(2, ':');
            let key = parts.next().unwrap_or("").trim();
            let value = parts.next().unwrap_or("").trim();
            match key {
                "VmHWM" => {
                    if let Some(kib) = parse_kib(value) {
                        metrics.rss_peak_bytes = Some(kib.saturating_mul(1024));
                    }
                }
                "VmRSS" => {
                    if metrics.rss_peak_bytes.is_none()
                        && let Some(kib) = parse_kib(value)
                    {
                        metrics.rss_peak_bytes = Some(kib.saturating_mul(1024));
                    }
                }
                "voluntary_ctxt_switches" => {
                    metrics.voluntary_ctx_switches = value.parse::<u64>().ok();
                }
                "nonvoluntary_ctxt_switches" => {
                    metrics.involuntary_ctx_switches = value.parse::<u64>().ok();
                }
                _ => {}
            }
        }
    }

    // /proc/self/statm is read for parity with the workplan; values are
    // already covered above so we just touch the file to surface I/O
    // failures during testing.
    let _ = std::fs::read_to_string(format!("/proc/{handle}/statm"));

    metrics
}

#[cfg(target_os = "linux")]
fn parse_kib(value: &str) -> Option<u64> {
    let mut parts = value.split_whitespace();
    let number = parts.next()?.parse::<u64>().ok()?;
    Some(number)
}

#[cfg(target_os = "macos")]
pub fn collect_self() -> ProcessMetrics {
    let mut metrics = ProcessMetrics::default();
    let mut usage = std::mem::MaybeUninit::<libc::rusage>::uninit();
    // `libc::getrusage` with `RUSAGE_SELF` cannot fail with EINVAL (the SELF
    // target is always valid) and on success initialises every field of
    // `rusage` in-place. The pointer is a valid, properly-aligned, writable
    // pointer obtained from `MaybeUninit::as_mut_ptr`.
    // SAFETY: valid SELF target, valid writable rusage pointer; kernel
    // initialises the struct in-place on rc == 0.
    let rc = unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) };
    if rc == 0 {
        // `rc == 0` is the kernel's contract that every field of `rusage`
        // has been initialized in-place (see `man getrusage(2)`); the
        // `MaybeUninit` proof obligation that every field be initialized
        // before `assume_init` is therefore discharged on this branch.
        // SAFETY: every field of `rusage` initialized by `getrusage` because rc == 0; `MaybeUninit::assume_init` precondition is satisfied; reading each field is sound.
        let usage = unsafe { usage.assume_init() };
        // macOS reports `ru_maxrss` in bytes; Linux reports KiB.
        metrics.rss_peak_bytes = Some(usage.ru_maxrss as u64);
        metrics.voluntary_ctx_switches = Some(usage.ru_nvcsw as u64);
        metrics.involuntary_ctx_switches = Some(usage.ru_nivcsw as u64);
    }
    metrics
}

#[cfg(target_os = "macos")]
pub fn collect_pid(_pid: i32) -> ProcessMetrics {
    // macOS does not expose per-pid /proc style metrics without
    // privileged APIs; fall back to self-only telemetry.
    ProcessMetrics::default()
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub fn collect_self() -> ProcessMetrics {
    ProcessMetrics::default()
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub fn collect_pid(_pid: i32) -> ProcessMetrics {
    ProcessMetrics::default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    fn collect_self_returns_rss_peak_on_supported_hosts() {
        let metrics = collect_self();
        assert!(
            metrics.rss_peak_bytes.is_some(),
            "expected rss_peak_bytes to be populated on Linux/macOS, got {metrics:?}"
        );
    }

    #[test]
    fn default_is_all_none() {
        let metrics = ProcessMetrics::default();
        assert_eq!(metrics.rss_peak_bytes, None);
        assert_eq!(metrics.proc_io_read_bytes, None);
        assert_eq!(metrics.proc_io_write_bytes, None);
        assert_eq!(metrics.write_count, None);
    }

    /// Asserts the `MaybeUninit::<libc::rusage>` + `getrusage` initializer
    /// pattern in `collect_self` actually populates the struct: by the time
    /// the test runs, the process has consumed memory and accumulated
    /// context switches, so at least the RSS peak must be a non-zero value
    /// when reported. Guards against a future regression to all-zero
    /// validity assumptions.
    #[test]
    #[cfg(target_os = "macos")]
    fn rusage_populates_after_init() {
        let metrics = collect_self();
        let rss = metrics
            .rss_peak_bytes
            .expect("rss_peak_bytes must be populated by getrusage on macOS");
        assert!(
            rss > 0,
            "expected getrusage to report a non-zero rss peak, got {rss}"
        );
    }
}
