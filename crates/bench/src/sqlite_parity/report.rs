use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Serialize;
use sha2::{Digest, Sha256};

use super::case::Case;
use super::engine::EngineOutput;

#[derive(Debug, Serialize)]
pub struct CaseRecord {
    pub case_id: String,
    pub name: String,
    pub priority: String,
    pub profile: String,
    pub category: String,
    pub engine: String,
    pub status: String,
    pub exit_code: Option<i32>,
    pub elapsed_ns: u128,
    pub stdout_sha256: String,
    pub stderr_sha256: String,
    pub artifact_dir: Option<PathBuf>,
    pub diagnostic: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CompareRecord {
    pub case_id: String,
    pub name: String,
    pub case_file: String,
    pub priority: String,
    pub profile: String,
    pub category: String,
    pub sample_index: usize,
    pub repetition_index: Option<usize>,
    pub sample_role: String,
    pub sqlite_version: Option<String>,
    pub reference_engine: String,
    pub target_engine: String,
    pub status: String,
    pub reference_exit_code: Option<i32>,
    pub target_exit_code: Option<i32>,
    pub reference_elapsed_ns: u128,
    pub target_elapsed_ns: u128,
    pub latency_ratio: f64,
    pub reference_stdout_sha256: String,
    pub reference_stderr_sha256: String,
    pub target_stdout_sha256: String,
    pub target_stderr_sha256: String,
    pub artifact_dir: Option<PathBuf>,
    pub diagnostic: Option<String>,
}

pub fn append_jsonl<T: Serialize>(out: Option<&Path>, value: &T) -> Result<()> {
    if let Some(out) = out {
        if let Some(parent) = out.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent)
                .with_context(|| format!("create parent for {}", out.display()))?;
        }
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(out)
            .with_context(|| format!("open jsonl report {}", out.display()))?;
        serde_json::to_writer(&mut file, value)?;
        file.write_all(b"\n")?;
    } else {
        println!("{}", serde_json::to_string(value)?);
    }
    Ok(())
}

pub fn case_record(
    case: &Case,
    output: &EngineOutput,
    status: impl Into<String>,
    artifact_dir: Option<PathBuf>,
    diagnostic: Option<String>,
) -> CaseRecord {
    CaseRecord {
        case_id: case.display_id(),
        name: case.name.clone(),
        priority: case.priority.to_string(),
        profile: case.profile.to_string(),
        category: case.category.clone(),
        engine: output.engine.clone(),
        status: status.into(),
        exit_code: output.status_code,
        elapsed_ns: output.elapsed.as_nanos(),
        stdout_sha256: sha256_hex(&output.stdout),
        stderr_sha256: sha256_hex(&output.stderr),
        artifact_dir,
        diagnostic,
    }
}

pub fn skipped_case_record(
    case: &Case,
    engine: impl Into<String>,
    status: impl Into<String>,
    artifact_dir: Option<PathBuf>,
    diagnostic: Option<String>,
) -> CaseRecord {
    CaseRecord {
        case_id: case.display_id(),
        name: case.name.clone(),
        priority: case.priority.to_string(),
        profile: case.profile.to_string(),
        category: case.category.clone(),
        engine: engine.into(),
        status: status.into(),
        exit_code: None,
        elapsed_ns: 0,
        stdout_sha256: String::new(),
        stderr_sha256: String::new(),
        artifact_dir,
        diagnostic,
    }
}

pub fn skipped_compare_record(
    case: &Case,
    reference_engine: impl Into<String>,
    target_engine: impl Into<String>,
    sqlite_version: Option<String>,
    status: impl Into<String>,
    artifact_dir: Option<PathBuf>,
    diagnostic: Option<String>,
) -> CompareRecord {
    CompareRecord {
        case_id: case.display_id(),
        name: case.name.clone(),
        case_file: case.case_file_name(),
        priority: case.priority.to_string(),
        profile: case.profile.to_string(),
        category: case.category.clone(),
        sample_index: 0,
        repetition_index: None,
        sample_role: "skipped".to_owned(),
        sqlite_version,
        reference_engine: reference_engine.into(),
        target_engine: target_engine.into(),
        status: status.into(),
        reference_exit_code: None,
        target_exit_code: None,
        reference_elapsed_ns: 0,
        target_elapsed_ns: 0,
        latency_ratio: 0.0,
        reference_stdout_sha256: String::new(),
        reference_stderr_sha256: String::new(),
        target_stdout_sha256: String::new(),
        target_stderr_sha256: String::new(),
        artifact_dir,
        diagnostic,
    }
}

pub fn compare_record(
    case: &Case,
    reference_output: &EngineOutput,
    target_output: &EngineOutput,
    sample_index: usize,
    repetition_index: Option<usize>,
    sample_role: impl Into<String>,
    sqlite_version: Option<String>,
    status: impl Into<String>,
    artifact_dir: Option<PathBuf>,
    diagnostic: Option<String>,
) -> CompareRecord {
    let reference_ns = reference_output.elapsed.as_nanos().max(1) as f64;
    let ratio = target_output.elapsed.as_nanos() as f64 / reference_ns;
    CompareRecord {
        case_id: case.display_id(),
        name: case.name.clone(),
        case_file: case.case_file_name(),
        priority: case.priority.to_string(),
        profile: case.profile.to_string(),
        category: case.category.clone(),
        sample_index,
        repetition_index,
        sample_role: sample_role.into(),
        sqlite_version,
        reference_engine: reference_output.engine.clone(),
        target_engine: target_output.engine.clone(),
        status: status.into(),
        reference_exit_code: reference_output.status_code,
        target_exit_code: target_output.status_code,
        reference_elapsed_ns: reference_output.elapsed.as_nanos(),
        target_elapsed_ns: target_output.elapsed.as_nanos(),
        latency_ratio: ratio,
        reference_stdout_sha256: sha256_hex(&reference_output.stdout),
        reference_stderr_sha256: sha256_hex(&reference_output.stderr),
        target_stdout_sha256: sha256_hex(&target_output.stdout),
        target_stderr_sha256: sha256_hex(&target_output.stderr),
        artifact_dir,
        diagnostic,
    }
}

pub fn write_failure_artifact(
    case: &Case,
    outputs: &[&EngineOutput],
    reason: &str,
) -> Result<PathBuf> {
    let root = Path::new("target")
        .join("sqlite-parity")
        .join("failures")
        .join(format!("{}_{}", case.display_id(), std::process::id()));
    fs::create_dir_all(&root).with_context(|| format!("create {}", root.display()))?;
    fs::write(root.join("input.sql"), &case.stdin)?;
    fs::write(root.join("reason.txt"), reason)?;
    for output in outputs {
        let prefix = output
            .engine
            .chars()
            .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
            .collect::<String>();
        fs::write(root.join(format!("{prefix}.stdout.txt")), &output.stdout)?;
        fs::write(root.join(format!("{prefix}.stderr.txt")), &output.stderr)?;
        fs::write(
            root.join(format!("{prefix}.exit.txt")),
            format!("{:?}\n", output.status_code),
        )?;
    }
    Ok(root)
}

pub fn write_skip_artifact(case: &Case, reason: &str) -> Result<PathBuf> {
    let root = Path::new("target")
        .join("sqlite-parity")
        .join("skips")
        .join(format!("{}_{}", case.display_id(), std::process::id()));
    fs::create_dir_all(&root).with_context(|| format!("create {}", root.display()))?;
    fs::write(root.join("input.sql"), &case.stdin)?;
    fs::write(root.join("reason.txt"), reason)?;
    Ok(root)
}

fn sha256_hex(value: &str) -> String {
    let digest = Sha256::digest(value.as_bytes());
    format!("{digest:x}")
}
