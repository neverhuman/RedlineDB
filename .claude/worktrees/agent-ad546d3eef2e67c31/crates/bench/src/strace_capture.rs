//! `strace -c` capture for benchmark child processes.
//!
//! On Linux hosts where the `strace` binary is on `PATH`, this module
//! attaches to the target pid and writes a syscall summary into the
//! requested output file. On any other platform, or when `strace`
//! is not installed, we return an explicit `reason` so the manifest
//! never silently drops telemetry.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StraceCapture {
    pub syscall_counts: Option<BTreeMap<String, u64>>,
    pub reason: Option<String>,
    pub output_path: Option<PathBuf>,
}

#[cfg(target_os = "linux")]
pub fn capture_or_skip(target_pid: i32, output_path: &Path) -> StraceCapture {
    use std::process::Command;

    let which = Command::new("which").arg("strace").output();
    let strace_available =
        matches!(which, Ok(out) if out.status.success() && !out.stdout.is_empty());
    if !strace_available {
        return StraceCapture {
            syscall_counts: None,
            reason: Some("strace not installed".to_owned()),
            output_path: None,
        };
    }

    if let Some(parent) = output_path.parent()
        && !parent.as_os_str().is_empty()
    {
        let _ = std::fs::create_dir_all(parent);
    }

    let attach = Command::new("strace")
        .arg("-c")
        .arg("-p")
        .arg(target_pid.to_string())
        .arg("-o")
        .arg(output_path)
        .output();

    match attach {
        Ok(output) if output.status.success() => {
            let raw = std::fs::read_to_string(output_path).unwrap_or_default();
            StraceCapture {
                syscall_counts: Some(parse_strace_summary(&raw)),
                reason: None,
                output_path: Some(output_path.to_path_buf()),
            }
        }
        Ok(output) => StraceCapture {
            syscall_counts: None,
            reason: Some(format!(
                "strace failed: status={} stderr={}",
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            )),
            output_path: None,
        },
        Err(err) => StraceCapture {
            syscall_counts: None,
            reason: Some(format!("strace launch error: {err}")),
            output_path: None,
        },
    }
}

#[cfg(not(target_os = "linux"))]
pub fn capture_or_skip(_target_pid: i32, _output_path: &Path) -> StraceCapture {
    StraceCapture {
        syscall_counts: None,
        reason: Some("linux required".to_owned()),
        output_path: None,
    }
}

/// Parse the table produced by `strace -c`. Lines look like:
///
/// ```text
/// % time     seconds  usecs/call     calls    errors syscall
/// ------ ----------- ----------- --------- --------- ----------------
///  39.00    0.001234           4       300        12 read
/// ```
///
/// We extract the `calls` column keyed by syscall name. The exact column
/// count varies between strace versions, so we treat the last token as
/// the syscall name and look back for the first numeric column that is
/// reasonable for `calls` (the second-to-last numeric column is normally
/// `errors`, so we prefer the third-to-last when there are at least
/// six tokens).
pub fn parse_strace_summary(raw: &str) -> BTreeMap<String, u64> {
    let mut out = BTreeMap::new();
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('%') || trimmed.starts_with('-') {
            continue;
        }
        let tokens: Vec<&str> = trimmed.split_whitespace().collect();
        if tokens.len() < 5 {
            continue;
        }
        let syscall = tokens[tokens.len() - 1];
        if syscall == "total" {
            continue;
        }
        // Walk backwards over the numeric columns to find the `calls` value.
        // Layouts seen in the wild:
        //   % time, seconds, usecs/call, calls, [errors,] syscall
        //   % time, seconds, usecs/call, calls, syscall (older)
        let calls = if tokens.len() >= 6 {
            tokens[tokens.len() - 3].parse::<u64>().ok()
        } else {
            tokens[tokens.len() - 2].parse::<u64>().ok()
        };
        if let Some(calls) = calls {
            out.insert(syscall.to_owned(), calls);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_linux_returns_explicit_reason() {
        // On Linux this exercises the "strace not installed" fast-path
        // when strace is absent, which still yields a reason. On macOS
        // we expect "linux required" verbatim.
        let cap = capture_or_skip(0, Path::new("/tmp/redlinedb-bench-strace-test"));
        if cfg!(target_os = "linux") {
            // Either strace is installed (and we may not have permission
            // to attach to pid 0) or we hit the fast-path. Either way we
            // expect *something* explicit.
            assert!(cap.reason.is_some() || cap.syscall_counts.is_some());
        } else {
            assert_eq!(cap.reason.as_deref(), Some("linux required"));
            assert!(cap.syscall_counts.is_none());
        }
    }

    #[test]
    fn parses_typical_strace_summary() {
        let raw = "% time     seconds  usecs/call     calls    errors syscall\n\
                   ------ ----------- ----------- --------- --------- ----------------\n\
                    39.00    0.001234           4       300        12 read\n\
                    11.00    0.000345           2       200         0 write\n\
                   ------ ----------- ----------- --------- --------- ----------------\n\
                   100.00    0.001579                  500        12 total\n";
        let parsed = parse_strace_summary(raw);
        assert_eq!(parsed.get("read"), Some(&300));
        assert_eq!(parsed.get("write"), Some(&200));
        assert!(!parsed.contains_key("total"));
    }
}
