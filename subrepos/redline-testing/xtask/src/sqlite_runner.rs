//! Subprocess wrapper around `sqlite3` for both capture-during-generation
//! and validation-during-ship-gate.

use std::env;
use std::fs;
use std::io::Write as _;
use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{Context, Result, anyhow, bail};
use serde::{Deserialize, Serialize};

pub const PRELUDE: &str = ".mode list\n.headers off\n.separator |\n.nullvalue NULL\n";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Case {
    pub id: u64,
    pub folder: String,
    pub name: String,
    pub category: String,
    pub priority: String,
    pub profile: String,
    pub kind: String,
    pub description: String,
    pub status: String,
    pub db: String,
    pub args: Vec<String>,
    pub stdin: String,
    pub expected_exit: i32,
    pub compare_stdout: bool,
    pub expected_stdout: Option<String>,
    #[serde(default)]
    pub expected_stdout_contains: Vec<String>,
    #[serde(default)]
    pub expected_stderr_contains: Vec<String>,
    #[serde(default)]
    pub expected_combined_contains: Vec<String>,
    #[serde(default)]
    pub files: Vec<(String, String)>,
    pub script: Option<String>,
    pub notes: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_capabilities: Vec<String>,
}

pub struct RunOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

/// Run sqlite3 with the case's stdin and capture the result. Honors `args`,
/// `db`, and `files` (with `{{TMP}}` substitution).
pub fn execute(case: &Case, sqlite_bin: &str) -> Result<RunOutput> {
    let tmp = make_tmp(&case.folder)?;
    materialize_files(case, &tmp)?;
    let stdin = case.stdin.replace("{{TMP}}", &tmp.to_string_lossy());
    let mut command = if case.script.is_some() {
        Command::new("bash")
    } else {
        Command::new(sqlite_bin)
    };
    if let Some(script) = &case.script {
        let script_path = tmp.join("case.sh");
        fs::write(
            &script_path,
            script.replace("{{TMP}}", &tmp.to_string_lossy()),
        )
        .with_context(|| format!("write {}", script_path.display()))?;
        command
            .arg(&script_path)
            .env("SQLITE_BIN", sqlite_bin)
            .env("SQLITE_PARITY_TMP", &tmp);
    } else if case.args.is_empty() {
        let db = case.db.replace("{{TMP}}", &tmp.to_string_lossy());
        command.arg("-batch").arg("-bail").arg(db);
    } else {
        for arg in &case.args {
            command.arg(arg.replace("{{TMP}}", &tmp.to_string_lossy()));
        }
    }
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .with_context(|| format!("spawn {} for case {}", sqlite_bin, case.name))?;
    {
        let mut child_stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow!("no stdin for child"))?;
        if case.script.is_none() {
            child_stdin
                .write_all(stdin.as_bytes())
                .with_context(|| format!("write stdin for case {}", case.name))?;
        }
    }
    let out = child
        .wait_with_output()
        .with_context(|| format!("wait child for case {}", case.name))?;
    let _ = fs::remove_dir_all(&tmp);
    Ok(RunOutput {
        stdout: normalize_newlines(&String::from_utf8_lossy(&out.stdout)),
        stderr: normalize_newlines(&String::from_utf8_lossy(&out.stderr)),
        exit_code: out.status.code().unwrap_or(-1),
    })
}

/// Normalize CR/CRLF line endings to LF, matching the redline-testing runner's
/// `normalize_output` so the captured/validated expected_stdout is identical
/// regardless of authoring tool (Python text-mode capture vs raw Rust bytes).
fn normalize_newlines(s: &str) -> String {
    s.replace("\r\n", "\n").replace('\r', "\n")
}

/// Validate that a case's recorded expectations match what sqlite3 currently
/// produces. Returns Ok(()) if the case ships green, Err with a diagnostic
/// otherwise.
pub fn validate(case: &Case, sqlite_bin: &str) -> Result<()> {
    let out = execute(case, sqlite_bin)?;
    if out.exit_code != case.expected_exit {
        bail!(
            "exit mismatch expected={} got={} stderr={:?}",
            case.expected_exit,
            out.exit_code,
            out.stderr
        );
    }
    if case.compare_stdout {
        if let Some(expected) = case.expected_stdout.as_deref()
            && out.stdout != expected
        {
            bail!(
                "stdout mismatch\n-- expected --\n{:?}\n-- got --\n{:?}",
                expected,
                out.stdout
            );
        }
    }
    for needle in &case.expected_stdout_contains {
        if !out.stdout.contains(needle) {
            bail!(
                "expected_stdout_contains missing {:?}; got stdout={:?}",
                needle,
                out.stdout
            );
        }
    }
    for needle in &case.expected_stderr_contains {
        if !out.stderr.contains(needle) {
            bail!(
                "expected_stderr_contains missing {:?}; got stderr={:?}",
                needle,
                out.stderr
            );
        }
    }
    for needle in &case.expected_combined_contains {
        let combined = format!("{}{}", out.stdout, out.stderr);
        if !combined.contains(needle) {
            bail!(
                "expected_combined_contains missing {:?}; got combined={:?}",
                needle,
                combined
            );
        }
    }
    Ok(())
}

fn make_tmp(label: &str) -> Result<std::path::PathBuf> {
    let base = if Path::new("/dev/shm").is_dir() {
        Path::new("/dev/shm").to_path_buf()
    } else {
        env::temp_dir()
    };
    let sanitized: String = label
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .take(48)
        .collect();
    let pid = std::process::id();
    let nonce: u64 = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    let path = base.join(format!("xtask-{sanitized}-{pid}-{nonce}"));
    fs::create_dir_all(&path).with_context(|| format!("create tmp {}", path.display()))?;
    Ok(path)
}

fn materialize_files(case: &Case, tmp: &Path) -> Result<()> {
    for (name, content) in &case.files {
        let target = tmp.join(name);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
        }
        fs::write(&target, content.replace("{{TMP}}", &tmp.to_string_lossy()))
            .with_context(|| format!("write fixture {}", target.display()))?;
    }
    Ok(())
}
