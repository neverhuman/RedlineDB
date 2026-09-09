//! Performance JSONL statistics and W2 manifest generation.

use std::{
    collections::BTreeSet,
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context, Result, anyhow, bail};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

#[derive(Clone, Debug, PartialEq)]
pub struct JsonlSummary {
    pub cases: usize,
    pub samples: usize,
    pub median: Option<f64>,
    pub p90: Option<f64>,
    pub faster_samples: usize,
}

impl JsonlSummary {
    pub fn render(&self) -> String {
        let mut output = format!(
            "  cases measured: {}\n  samples:        {}\n",
            self.cases, self.samples
        );
        if let Some(median) = self.median {
            output.push_str(&format!("  ratio median:   {median:.3}\n"));
            if let Some(p90) = self.p90 {
                output.push_str(&format!("  ratio p90:      {p90:.3}\n"));
            }
            output.push_str(&format!(
                "  cases faster than sqlite: {}/{}\n",
                self.faster_samples, self.samples
            ));
        }
        output
    }
}

pub fn summarize_jsonl_path(path: &Path) -> Result<JsonlSummary> {
    let file = File::open(path).with_context(|| format!("open JSONL input {}", path.display()))?;
    summarize_jsonl(BufReader::new(file))
}

pub fn summarize_jsonl(reader: impl BufRead) -> Result<JsonlSummary> {
    let mut case_ids = BTreeSet::new();
    let mut ratios = Vec::new();

    for (index, line) in reader.lines().enumerate() {
        let line = line.with_context(|| format!("read JSONL line {}", index + 1))?;
        let Ok(row) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        let Some(row) = row.as_object() else {
            continue;
        };
        let measured = row.get("status").and_then(Value::as_str) == Some("passed")
            && row
                .get("sample_role")
                .and_then(Value::as_str)
                .is_some_and(|role| role.starts_with("measured"));
        if !measured {
            continue;
        }
        let Some(ratio) = row
            .get("latency_ratio")
            .and_then(Value::as_f64)
            .filter(|ratio| *ratio > 0.0)
        else {
            continue;
        };
        let case_id = row
            .get("case_id")
            .ok_or_else(|| anyhow!("measured JSONL row {} is missing case_id", index + 1))?;
        case_ids.insert(serde_json::to_string(case_id)?);
        ratios.push(ratio);
    }

    ratios.sort_by(f64::total_cmp);
    Ok(JsonlSummary {
        cases: case_ids.len(),
        samples: ratios.len(),
        median: median(&ratios),
        p90: (ratios.len() >= 10).then(|| exclusive_decile_p90(&ratios)),
        faster_samples: ratios.iter().filter(|ratio| **ratio < 1.0).count(),
    })
}

fn median(sorted: &[f64]) -> Option<f64> {
    match sorted.len() {
        0 => None,
        count if count % 2 == 1 => Some(sorted[count / 2]),
        count => Some((sorted[count / 2 - 1] + sorted[count / 2]) / 2.0),
    }
}

/// Match `statistics.quantiles(values, n=10)[-1]` from the retired reference.
fn exclusive_decile_p90(sorted: &[f64]) -> f64 {
    const QUANTILES: usize = 10;
    const INDEX: usize = 9;

    let sample_boundaries = sorted.len() + 1;
    let scaled = INDEX * sample_boundaries;
    let boundary = (scaled / QUANTILES).clamp(1, sorted.len() - 1);
    let remainder = scaled - boundary * QUANTILES;
    (sorted[boundary - 1] * (QUANTILES - remainder) as f64 + sorted[boundary] * remainder as f64)
        / QUANTILES as f64
}

pub fn assert_distinct_binaries(target: &Path, reference: &Path) -> Result<()> {
    if sha256_file(target)? == sha256_file(reference)? {
        bail!("target binary sha256 equals sqlite3 reference — refusing");
    }
    Ok(())
}

#[derive(Clone, Debug)]
pub struct W2ManifestInput {
    pub output_path: PathBuf,
    pub captured_at_utc: String,
    pub profile: String,
    pub allocator: String,
    pub label: String,
    pub binary_path: PathBuf,
    pub suite: String,
    pub perf_jsonl: Option<String>,
    pub rustc_version: String,
    pub base_rustflags: String,
    pub host: HostMetadata,
}

#[derive(Clone, Debug, Serialize)]
pub struct HostMetadata {
    pub node: String,
    pub machine: String,
    pub system: String,
    pub release: String,
}

#[derive(Serialize)]
struct W2Manifest<'a> {
    schema_version: &'static str,
    captured_at_utc: &'a str,
    profile: &'a str,
    allocator: &'a str,
    label: &'a str,
    binary: BinaryMetadata<'a>,
    perf: PerfMetadata<'a>,
    build: BuildMetadata<'a>,
    host: &'a HostMetadata,
}

#[derive(Serialize)]
struct BinaryMetadata<'a> {
    path: &'a str,
    sha256: String,
    size_bytes: u64,
}

#[derive(Serialize)]
struct PerfMetadata<'a> {
    suite: &'a str,
    jsonl: Option<&'a str>,
    pgo_training_corpus: &'static str,
}

#[derive(Serialize)]
struct BuildMetadata<'a> {
    rustc: &'a str,
    base_rustflags: &'a str,
}

pub fn append_w2_manifest(input: &W2ManifestInput) -> Result<()> {
    let line = w2_manifest_line(input)?;
    if let Some(parent) = input.output_path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)
            .with_context(|| format!("create manifest directory {}", parent.display()))?;
    }
    let mut output = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&input.output_path)
        .with_context(|| format!("open W2 manifest {}", input.output_path.display()))?;
    output
        .write_all(line.as_bytes())
        .with_context(|| format!("append W2 manifest {}", input.output_path.display()))?;
    output.write_all(b"\n")?;
    Ok(())
}

pub fn w2_manifest_line(input: &W2ManifestInput) -> Result<String> {
    for (name, value) in [
        ("captured_at_utc", input.captured_at_utc.as_str()),
        ("profile", input.profile.as_str()),
        ("allocator", input.allocator.as_str()),
        ("label", input.label.as_str()),
        ("suite", input.suite.as_str()),
        ("rustc_version", input.rustc_version.as_str()),
    ] {
        if value.trim().is_empty() {
            bail!("W2 manifest field {name} must not be empty");
        }
    }
    let binary_path = input
        .binary_path
        .to_str()
        .ok_or_else(|| anyhow!("binary path must be valid UTF-8"))?;
    let size_bytes = input
        .binary_path
        .metadata()
        .with_context(|| format!("stat W2 binary {}", input.binary_path.display()))?
        .len();
    let manifest = W2Manifest {
        schema_version: "w2-matrix/1",
        captured_at_utc: &input.captured_at_utc,
        profile: &input.profile,
        allocator: &input.allocator,
        label: &input.label,
        binary: BinaryMetadata {
            path: binary_path,
            sha256: sha256_file(&input.binary_path)?,
            size_bytes,
        },
        perf: PerfMetadata {
            suite: &input.suite,
            jsonl: input.perf_jsonl.as_deref(),
            pgo_training_corpus: "full",
        },
        build: BuildMetadata {
            rustc: &input.rustc_version,
            base_rustflags: &input.base_rustflags,
        },
        host: &input.host,
    };
    serde_json::to_string(&manifest).context("serialize W2 manifest entry")
}

pub fn capture_w2_runtime_metadata() -> Result<(String, String, HostMetadata)> {
    Ok((
        command_stdout("date", &["-u", "+%Y-%m-%dT%H:%M:%SZ"])?,
        command_stdout("rustc", &["--version"])?,
        HostMetadata {
            node: command_stdout("hostname", &[])?,
            machine: command_stdout("uname", &["-m"])?,
            system: command_stdout("uname", &["-s"])?,
            release: command_stdout("uname", &["-r"])?,
        },
    ))
}

fn command_stdout(program: &str, arguments: &[&str]) -> Result<String> {
    let output = Command::new(program)
        .args(arguments)
        .output()
        .with_context(|| format!("run {program}"))?;
    if !output.status.success() {
        bail!(
            "{program} exited with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    let value = String::from_utf8(output.stdout)
        .with_context(|| format!("{program} emitted non-UTF-8 output"))?;
    let value = value.trim();
    if value.is_empty() {
        bail!("{program} emitted empty output");
    }
    Ok(value.to_owned())
}

fn sha256_file(path: &Path) -> Result<String> {
    let mut file = File::open(path).with_context(|| format!("open {}", path.display()))?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .with_context(|| format!("read {}", path.display()))?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    #[test]
    fn summary_matches_frozen_exclusive_decile_golden() {
        let input = include_str!("../tests/fixtures/perf-evidence/measured.jsonl");
        let summary = summarize_jsonl(Cursor::new(input)).unwrap();
        assert_eq!(
            summary.render(),
            concat!(
                "  cases measured: 10\n",
                "  samples:        10\n",
                "  ratio median:   5.500\n",
                "  ratio p90:      9.900\n",
                "  cases faster than sqlite: 0/10\n"
            )
        );
    }

    #[test]
    fn measured_row_without_case_id_is_rejected() {
        let input = r#"{"status":"passed","sample_role":"measured","latency_ratio":1.0}"#;
        let error = summarize_jsonl(Cursor::new(input)).unwrap_err();
        assert!(error.to_string().contains("missing case_id"));
    }

    #[test]
    fn w2_manifest_matches_golden_shape_and_hash() {
        let directory = tempfile::tempdir().unwrap();
        let binary = directory.path().join("redlinedb");
        fs::write(&binary, []).unwrap();
        let input = W2ManifestInput {
            output_path: directory.path().join("manifest.jsonl"),
            captured_at_utc: "2026-07-12T12:34:56Z".to_owned(),
            profile: "release".to_owned(),
            allocator: "mimalloc".to_owned(),
            label: "w2-release-mimalloc-fixture".to_owned(),
            binary_path: binary.clone(),
            suite: "full".to_owned(),
            perf_jsonl: Some("target/perf/fixture.jsonl".to_owned()),
            rustc_version: "rustc 1.95.0 (fixture)".to_owned(),
            base_rustflags: "-Ctarget-cpu=x86-64-v3".to_owned(),
            host: HostMetadata {
                node: "fixture-node".to_owned(),
                machine: "x86_64".to_owned(),
                system: "Linux".to_owned(),
                release: "fixture-kernel".to_owned(),
            },
        };
        let expected = format!(
            concat!(
                "{{\"schema_version\":\"w2-matrix/1\",",
                "\"captured_at_utc\":\"2026-07-12T12:34:56Z\",",
                "\"profile\":\"release\",\"allocator\":\"mimalloc\",",
                "\"label\":\"w2-release-mimalloc-fixture\",",
                "\"binary\":{{\"path\":\"{}\",",
                "\"sha256\":\"e3b0c44298fc1c149afbf4c8996fb924",
                "27ae41e4649b934ca495991b7852b855\",\"size_bytes\":0}},",
                "\"perf\":{{\"suite\":\"full\",",
                "\"jsonl\":\"target/perf/fixture.jsonl\",",
                "\"pgo_training_corpus\":\"full\"}},",
                "\"build\":{{\"rustc\":\"rustc 1.95.0 (fixture)\",",
                "\"base_rustflags\":\"-Ctarget-cpu=x86-64-v3\"}},",
                "\"host\":{{\"node\":\"fixture-node\",\"machine\":\"x86_64\",",
                "\"system\":\"Linux\",\"release\":\"fixture-kernel\"}}}}"
            ),
            binary.display()
        );
        assert_eq!(w2_manifest_line(&input).unwrap(), expected);
        append_w2_manifest(&input).unwrap();
        assert_eq!(
            fs::read_to_string(&input.output_path).unwrap(),
            format!("{expected}\n")
        );
    }

    #[test]
    fn missing_manifest_binary_is_rejected() {
        let input = W2ManifestInput {
            output_path: PathBuf::from("unused"),
            captured_at_utc: "2026-07-12T12:34:56Z".to_owned(),
            profile: "release".to_owned(),
            allocator: "mimalloc".to_owned(),
            label: "fixture".to_owned(),
            binary_path: PathBuf::from("definitely-missing-redlinedb-binary"),
            suite: "none".to_owned(),
            perf_jsonl: None,
            rustc_version: "rustc fixture".to_owned(),
            base_rustflags: String::new(),
            host: HostMetadata {
                node: "node".to_owned(),
                machine: "machine".to_owned(),
                system: "system".to_owned(),
                release: "release".to_owned(),
            },
        };
        assert!(
            w2_manifest_line(&input)
                .unwrap_err()
                .to_string()
                .contains("stat W2 binary")
        );
    }

    #[test]
    fn identical_perf_binaries_are_rejected() {
        let directory = tempfile::tempdir().unwrap();
        let binary = directory.path().join("redlinedb");
        fs::write(&binary, b"same artifact").unwrap();
        let error = assert_distinct_binaries(&binary, &binary).unwrap_err();
        assert!(error.to_string().contains("equals sqlite3 reference"));
    }
}
