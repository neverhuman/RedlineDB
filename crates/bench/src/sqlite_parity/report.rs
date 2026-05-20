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
}

#[derive(Debug, Serialize)]
pub struct CompareRecord {
    pub case_id: String,
    pub name: String,
    pub priority: String,
    pub profile: String,
    pub category: String,
    pub reference_engine: String,
    pub target_engine: String,
    pub status: String,
    pub reference_exit_code: Option<i32>,
    pub target_exit_code: Option<i32>,
    pub reference_elapsed_ns: u128,
    pub target_elapsed_ns: u128,
    pub latency_ratio: f64,
    pub artifact_dir: Option<PathBuf>,
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

fn sha256_hex(value: &str) -> String {
    let digest = Sha256::digest(value.as_bytes());
    format!("{digest:x}")
}
