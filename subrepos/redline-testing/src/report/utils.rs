use std::fs;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};
use sha2::{Digest, Sha256};

use super::types::{RawRecord, RenderedReport, Score, SvgArtifact};

pub(crate) fn verify_existing(
    input: &Path,
    raw_out: &Path,
    summary_out: &Path,
    ranked_out: &Path,
    ksloc_out: &Path,
    manifest_out: &Path,
    provenance_out: &Path,
    readme_out: &Path,
    rendered: &RenderedReport,
    svg_artifacts: &[SvgArtifact],
) -> Result<()> {
    verify_text(input, &rendered.raw)?;
    verify_text(raw_out, &rendered.raw)?;
    verify_text(summary_out, &rendered.summary)?;
    verify_text(ranked_out, &rendered.ranked)?;
    verify_text(ksloc_out, &rendered.ksloc)?;
    verify_text(manifest_out, &rendered.manifest)?;
    verify_text(provenance_out, &rendered.provenance)?;
    verify_text(readme_out, &rendered.readme)?;
    for artifact in svg_artifacts {
        verify_text(&artifact.path, &artifact.contents)?;
    }
    Ok(())
}

pub(crate) fn verify_text(path: &Path, expected: &str) -> Result<()> {
    let actual = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    if actual != expected {
        bail!("artifact drift detected: {}", path.display());
    }
    Ok(())
}

pub(crate) fn write_text(path: &Path, text: &str) -> Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    fs::write(path, text).with_context(|| format!("write {}", path.display()))
}

pub(crate) fn sha256_file(path: &Path) -> Result<String> {
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    Ok(format!("{:x}", Sha256::digest(&bytes)))
}

pub(crate) fn sha256_hex(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}

pub(crate) fn csv(value: &str) -> String {
    if value.contains([',', '"', '\n']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_owned()
    }
}

pub(crate) fn is_measured(record: &RawRecord) -> bool {
    record.status == "passed"
        && (record.repetition_index.is_some() || record.sample_role.starts_with("measured"))
}

pub(crate) fn median(values: impl Iterator<Item = u128>) -> u128 {
    let mut values = values.collect::<Vec<_>>();
    values.sort_unstable();
    values[values.len() / 2]
}

pub(crate) fn median_u64(values: impl Iterator<Item = u64>) -> Option<u64> {
    let mut values = values.collect::<Vec<_>>();
    if values.is_empty() {
        return None;
    }
    values.sort_unstable();
    Some(values[values.len() / 2])
}

pub(crate) fn improvement_pct(sqlite_median_ns: u128, redline_median_ns: u128) -> f64 {
    let effective_sqlite_ns = sqlite_median_ns.max(3_000_000);
    (effective_sqlite_ns as f64 - redline_median_ns as f64) / effective_sqlite_ns.max(1) as f64
        * 100.0
}

pub(crate) fn parse_score(path: &Path) -> Result<Score> {
    let text = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let value: serde_json::Value =
        serde_json::from_str(&text).with_context(|| format!("parse {}", path.display()))?;
    Ok(Score {
        score: value["score"].as_u64().unwrap_or(0),
        status: value["status"].as_str().unwrap_or("unknown").to_owned(),
    })
}

pub(crate) fn capture_version(path: &Path) -> Result<String> {
    let output = Command::new(path)
        .arg("--version")
        .output()
        .with_context(|| format!("run {} --version", path.display()))?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

pub(crate) fn canonical_display(path: &Path) -> String {
    fs::canonicalize(path)
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_else(|_| path.to_string_lossy().into_owned())
}

pub(crate) fn git_sha() -> String {
    Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .and_then(|output| output.status.success().then(|| output.stdout))
        .map(|stdout| String::from_utf8_lossy(&stdout).trim().to_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "<unknown>".to_owned())
}

pub(crate) fn git_dirty() -> bool {
    !Command::new("git")
        .args(["diff", "--quiet"])
        .status()
        .is_ok_and(|status| status.success())
}

pub(crate) fn normalized_command_line() -> Vec<String> {
    std::env::args().filter(|arg| arg != "--check").collect()
}
