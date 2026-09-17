use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    process::Command,
    time::SystemTime,
};

use anyhow::{Context, Result, anyhow, bail};
use serde_json::{Value, json};

use crate::{clock::utc_timestamp, file_hash::sha256_file};

#[derive(Debug)]
struct Config {
    manifest: PathBuf,
    profile: String,
    allocator: String,
    label: String,
    binary: PathBuf,
    suite: String,
    perf_jsonl: Option<String>,
    base_rustflags: String,
}

pub(crate) fn run(args: &[String]) -> Result<()> {
    let config = parse_args(args)?;
    let metadata = fs::metadata(&config.binary)
        .with_context(|| format!("w2-manifest: inspect {}", config.binary.display()))?;
    let entry = entry(
        &config,
        &sha256_file(&config.binary)?,
        metadata.len(),
        &command_output("rustc", &["--version"])?,
        &Host {
            node: command_output("hostname", &[])?,
            machine: command_output("uname", &["-m"])?,
            system: command_output("uname", &["-s"])?,
            release: command_output("uname", &["-r"])?,
        },
        &utc_timestamp(SystemTime::now())?,
    );
    let parent = config.manifest.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)
        .with_context(|| format!("w2-manifest: create {}", parent.display()))?;
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&config.manifest)
        .with_context(|| format!("w2-manifest: open {}", config.manifest.display()))?;
    serde_json::to_writer(&mut file, &entry).context("w2-manifest: serialize entry")?;
    file.write_all(b"\n")
        .with_context(|| format!("w2-manifest: append {}", config.manifest.display()))?;
    Ok(())
}

fn parse_args(args: &[String]) -> Result<Config> {
    let mut options = BTreeMap::new();
    let mut index = 0;
    while index < args.len() {
        let option = args[index].as_str();
        if ![
            "--manifest",
            "--profile",
            "--allocator",
            "--label",
            "--binary",
            "--suite",
            "--perf-jsonl",
            "--base-rustflags",
        ]
        .contains(&option)
        {
            bail!("w2-manifest: unknown option {option:?}");
        }
        let value = args
            .get(index + 1)
            .ok_or_else(|| anyhow!("w2-manifest: {option} requires a value"))?;
        options.insert(option, value.as_str());
        index += 2;
    }
    let required = |name| {
        options
            .get(name)
            .copied()
            .filter(|value| !value.is_empty())
            .ok_or_else(|| anyhow!("w2-manifest: {name} is required"))
    };
    Ok(Config {
        manifest: PathBuf::from(required("--manifest")?),
        profile: required("--profile")?.to_owned(),
        allocator: required("--allocator")?.to_owned(),
        label: required("--label")?.to_owned(),
        binary: PathBuf::from(required("--binary")?),
        suite: required("--suite")?.to_owned(),
        perf_jsonl: options
            .get("--perf-jsonl")
            .copied()
            .filter(|value| !value.is_empty())
            .map(str::to_owned),
        base_rustflags: options
            .get("--base-rustflags")
            .copied()
            .unwrap_or_default()
            .to_owned(),
    })
}

struct Host {
    node: String,
    machine: String,
    system: String,
    release: String,
}

fn entry(
    config: &Config,
    binary_sha: &str,
    binary_size: u64,
    rustc: &str,
    host: &Host,
    captured_at: &str,
) -> Value {
    json!({
        "schema_version": "w2-matrix/1",
        "captured_at_utc": captured_at,
        "profile": config.profile,
        "allocator": config.allocator,
        "label": config.label,
        "binary": {
            "path": config.binary,
            "sha256": binary_sha,
            "size_bytes": binary_size,
        },
        "perf": {
            "suite": config.suite,
            "jsonl": config.perf_jsonl,
            "pgo_training_corpus": "full",
        },
        "build": {
            "rustc": rustc,
            "base_rustflags": config.base_rustflags,
        },
        "host": {
            "node": host.node,
            "machine": host.machine,
            "system": host.system,
            "release": host.release,
        },
    })
}

fn command_output(program: &str, args: &[&str]) -> Result<String> {
    let output = Command::new(program)
        .args(args)
        .output()
        .with_context(|| format!("w2-manifest: execute {program}"))?;
    if !output.status.success() {
        bail!(
            "w2-manifest: {program} failed with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    String::from_utf8(output.stdout)
        .with_context(|| format!("w2-manifest: {program} output is not UTF-8"))
        .map(|value| value.trim().to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> Config {
        Config {
            manifest: PathBuf::from("manifest.jsonl"),
            profile: "release".to_owned(),
            allocator: "mimalloc".to_owned(),
            label: "candidate".to_owned(),
            binary: PathBuf::from("target/release/redlinedb"),
            suite: "full".to_owned(),
            perf_jsonl: None,
            base_rustflags: "-C target-cpu=x86-64-v3".to_owned(),
        }
    }

    #[test]
    fn builds_typed_manifest_entry() {
        let entry = entry(
            &config(),
            &"a".repeat(64),
            42,
            "rustc 1.95.0",
            &Host {
                node: "builder".to_owned(),
                machine: "x86_64".to_owned(),
                system: "Linux".to_owned(),
                release: "6.8".to_owned(),
            },
            "2026-07-12T00:00:00Z",
        );
        assert_eq!(entry["binary"]["size_bytes"], 42);
        assert_eq!(entry["perf"]["jsonl"], Value::Null);
        assert_eq!(entry["perf"]["pgo_training_corpus"], "full");
        assert_eq!(entry["host"]["system"], "Linux");
    }

    #[test]
    fn rejects_missing_required_arguments() {
        let error = parse_args(&[]).unwrap_err();
        assert!(error.to_string().contains("--manifest is required"));
    }
}
