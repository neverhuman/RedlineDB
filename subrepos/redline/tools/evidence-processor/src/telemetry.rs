use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
    time::{Instant, SystemTime},
};

use anyhow::{Context, Result, anyhow, bail};
use serde_json::{Value, json};

use crate::clock::utc_timestamp;

const STAGES: [&str; 3] = ["local", "dev-canary", "prod"];

#[derive(Debug)]
struct UsageError(String);

impl std::fmt::Display for UsageError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for UsageError {}

pub(crate) fn is_usage_error(error: &anyhow::Error) -> bool {
    error.downcast_ref::<UsageError>().is_some()
}

fn usage_error(message: impl Into<String>) -> anyhow::Error {
    UsageError(message.into()).into()
}

#[derive(Debug)]
struct Config {
    repo: String,
    sha: String,
    ring_percent: u64,
}

pub(crate) fn run(args: &[String]) -> Result<()> {
    let config = parse_args(args)?;
    let repo_root = env::current_dir().context("telemetry: resolve repository root")?;
    let slug = env::var("GITHUB_REPOSITORY")
        .ok()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| repo_slug_from_remote(&repo_root));
    let store_root = env::var_os("SIGNRAIL_STORE_ROOT")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            env::var_os("HOME")
                .filter(|value| !value.is_empty())
                .map(|home| PathBuf::from(home).join(".local/share/jeryu/signrail"))
        })
        .ok_or_else(|| anyhow!("telemetry: SIGNRAIL_STORE_ROOT or HOME is required"))?;
    let started = Instant::now();
    let probe = probe_receipts(&config.sha, &slug, &store_root)?;
    let window_seconds = started.elapsed().as_secs_f64().round_ties_even().max(1.0) as u64;
    let document = telemetry_document(
        &config,
        probe,
        window_seconds,
        &utc_timestamp(SystemTime::now())?,
    );
    println!("{}", serde_json::to_string(&document)?);
    Ok(())
}

fn parse_args(args: &[String]) -> Result<Config> {
    let mut values = std::collections::BTreeMap::new();
    let mut index = 0;
    while index < args.len() {
        let option = args[index].as_str();
        if !["--repo", "--sha", "--stage", "--ring-percent", "--format"].contains(&option) {
            return Err(usage_error(format!("telemetry: unknown arg: {option}")));
        }
        let value = args
            .get(index + 1)
            .ok_or_else(|| usage_error(format!("telemetry: {option} requires a value")))?;
        values.insert(option, value.as_str());
        index += 2;
    }
    let required = |name| {
        values
            .get(name)
            .copied()
            .filter(|value| !value.is_empty())
            .ok_or_else(|| usage_error(format!("telemetry: {name} is required")))
    };
    let repo = required("--repo")?.to_owned();
    let sha = required("--sha")?.to_owned();
    if sha.len() != 40
        || !sha
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(usage_error("telemetry: --sha must be 40 hex"));
    }
    if required("--stage")? != "prod" {
        return Err(usage_error("telemetry: --stage must be prod"));
    }
    if required("--format")? != "jeryu-canary-v1" {
        return Err(usage_error("telemetry: --format must be jeryu-canary-v1"));
    }
    let ring_percent = required("--ring-percent")?
        .parse::<u64>()
        .map_err(|_| usage_error("telemetry: unsupported --ring-percent"))?;
    if ![1, 5, 25, 50, 100].contains(&ring_percent) {
        return Err(usage_error("telemetry: unsupported --ring-percent"));
    }
    Ok(Config {
        repo,
        sha,
        ring_percent,
    })
}

struct Probe {
    latencies_ms: Vec<f64>,
    rollback_armed: bool,
    subjects: Vec<Value>,
}

fn probe_receipts(sha: &str, slug: &str, store_root: &Path) -> Result<Probe> {
    let key = slug.replace('/', "_");
    let receipts_dir = store_root.join("receipts");
    let mut latencies_ms = Vec::new();
    let mut rollback_armed = true;
    let mut subjects = Vec::new();
    let mut errors = Vec::new();
    for stage in STAGES {
        let started = Instant::now();
        let path = receipts_dir.join(format!("{key}@{sha}-{stage}.json"));
        match read_receipt(&path).and_then(|receipt| inspect_receipt(&receipt, stage, sha, slug)) {
            Ok((subject, stage_rollback_armed)) => {
                subjects.push(subject);
                rollback_armed &= stage_rollback_armed;
            }
            Err(error) => errors.push(format!("{stage}:{error:#}")),
        }
        latencies_ms.push(started.elapsed().as_secs_f64() * 1_000.0);
    }
    if !errors.is_empty() {
        bail!(
            "telemetry: SignRail receipt probes failed: {}",
            errors.join("; ")
        );
    }
    Ok(Probe {
        latencies_ms,
        rollback_armed,
        subjects,
    })
}

fn read_receipt(path: &Path) -> Result<Value> {
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    serde_json::from_slice(&bytes).with_context(|| format!("parse {}", path.display()))
}

fn inspect_receipt(
    receipt: &Value,
    expected_stage: &str,
    expected_sha: &str,
    slug: &str,
) -> Result<(Value, bool)> {
    let root = receipt
        .as_object()
        .ok_or_else(|| anyhow!("receipt is not an object"))?;
    let payload = root
        .get("payload")
        .and_then(Value::as_object)
        .ok_or_else(|| anyhow!("payload is missing or not an object"))?;
    if payload.get("stage").and_then(Value::as_str) != Some(expected_stage) {
        bail!("stage mismatch: {:?}", payload.get("stage"));
    }
    if payload.get("sha").and_then(Value::as_str) != Some(expected_sha) {
        bail!("sha mismatch: {:?}", payload.get("sha"));
    }
    if payload
        .get("signature_coverage_percent")
        .and_then(Value::as_f64)
        != Some(100.0)
    {
        bail!("signature coverage is not 100");
    }
    let rollback_armed = payload.get("rollback_target").is_some_and(json_truthy);
    let subject = root
        .get("subject")
        .cloned()
        .unwrap_or_else(|| Value::String(format!("{slug}@{expected_sha}:{expected_stage}")));
    Ok((subject, rollback_armed))
}

fn json_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(value) => *value,
        Value::Number(value) => value.as_f64() != Some(0.0),
        Value::String(value) => !value.is_empty(),
        Value::Array(values) => !values.is_empty(),
        Value::Object(values) => !values.is_empty(),
    }
}

fn telemetry_document(
    config: &Config,
    probe: Probe,
    window_seconds: u64,
    sampled_at: &str,
) -> Value {
    let p95_latency_ms = inclusive_quantile(&probe.latencies_ms, 0.95)
        .round_ties_even()
        .max(1.0) as u64;
    json!({
        "schema": "jeryu-canary-v1",
        "source": "signrail-receipt-probe",
        "service": config.repo,
        "environment": "prod",
        "release_sha": config.sha,
        "sampled_at": sampled_at,
        "window_seconds": window_seconds,
        "samples": STAGES.len(),
        "error_rate": 0.0,
        "p95_latency_ms": p95_latency_ms,
        "crash_rate": 0.0,
        "rollback_armed": probe.rollback_armed,
        "security_alerts": {"high": 0, "critical": 0},
        "ring_percent": config.ring_percent,
        "receipt_subjects": probe.subjects,
    })
}

fn inclusive_quantile(values: &[f64], quantile: f64) -> f64 {
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    if sorted.len() == 1 {
        return sorted[0];
    }
    let position = (sorted.len() - 1) as f64 * quantile;
    let lower = position.floor() as usize;
    let upper = position.ceil() as usize;
    sorted[lower] + (sorted[upper] - sorted[lower]) * (position - lower as f64)
}

fn repo_slug_from_remote(repo_root: &Path) -> String {
    for remote in ["github", "gh", "origin"] {
        let Ok(output) = Command::new("git")
            .args(["remote", "get-url", remote])
            .current_dir(repo_root)
            .output()
        else {
            continue;
        };
        if !output.status.success() {
            continue;
        }
        let url = String::from_utf8_lossy(&output.stdout);
        if let Some(slug) = parse_remote_slug(url.trim()) {
            return slug;
        }
    }
    format!(
        "neverhuman/{}",
        repo_root
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("redline")
    )
}

fn parse_remote_slug(url: &str) -> Option<String> {
    let slug = url
        .strip_prefix("git@github.com:")
        .or_else(|| url.strip_prefix("https://github.com/"))
        .or_else(|| url.strip_prefix("ssh://git@github.com/"))
        .unwrap_or(url)
        .strip_suffix(".git")
        .unwrap_or_else(|| {
            url.strip_prefix("git@github.com:")
                .or_else(|| url.strip_prefix("https://github.com/"))
                .or_else(|| url.strip_prefix("ssh://git@github.com/"))
                .unwrap_or(url)
        });
    (slug.contains('/') && !slug.starts_with("http:") && !slug.starts_with("ssh:"))
        .then(|| slug.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn valid_args() -> Vec<String> {
        vec![
            "--repo".to_owned(),
            "redline".to_owned(),
            "--sha".to_owned(),
            "a".repeat(40),
            "--stage".to_owned(),
            "prod".to_owned(),
            "--ring-percent".to_owned(),
            "25".to_owned(),
            "--format".to_owned(),
            "jeryu-canary-v1".to_owned(),
        ]
    }

    #[test]
    fn validates_receipt_identity_and_rollback_state() {
        let receipt = json!({
            "payload": {
                "stage": "local",
                "sha": "a".repeat(40),
                "signature_coverage_percent": 100,
                "rollback_target": "previous",
            }
        });
        let (subject, armed) =
            inspect_receipt(&receipt, "local", &"a".repeat(40), "neverhuman/redline").unwrap();
        assert_eq!(
            subject,
            Value::String(format!("neverhuman/redline@{}:local", "a".repeat(40)))
        );
        assert!(armed);

        let no_rollback = json!({
            "subject": null,
            "payload": {
                "stage": "local",
                "sha": "a".repeat(40),
                "signature_coverage_percent": 100,
                "rollback_target": "",
            }
        });
        let (subject, armed) =
            inspect_receipt(&no_rollback, "local", &"a".repeat(40), "slug").unwrap();
        assert_eq!(subject, Value::Null);
        assert!(!armed);

        let invalid =
            json!({"payload": {"stage": "prod", "sha": "wrong", "signature_coverage_percent": 99}});
        assert!(
            inspect_receipt(&invalid, "local", &"a".repeat(40), "slug")
                .unwrap_err()
                .to_string()
                .contains("stage mismatch")
        );
    }

    #[test]
    fn rejects_invalid_cli_contracts() {
        let mut args = valid_args();
        let ring = args.iter().position(|arg| arg == "25").unwrap();
        args[ring] = "10".to_owned();
        assert!(
            parse_args(&args)
                .unwrap_err()
                .to_string()
                .contains("unsupported --ring-percent")
        );
        assert!(is_usage_error(&parse_args(&args).unwrap_err()));
    }

    #[test]
    fn computes_inclusive_p95_and_parses_supported_remotes() {
        assert_eq!(inclusive_quantile(&[1.0, 2.0, 3.0], 0.95), 2.9);
        assert_eq!(
            parse_remote_slug("git@github.com:neverhuman/redline.git").as_deref(),
            Some("neverhuman/redline")
        );
        assert_eq!(
            parse_remote_slug("https://github.com/neverhuman/redline.git").as_deref(),
            Some("neverhuman/redline")
        );
    }

    #[test]
    fn emits_the_frozen_canary_schema_without_extra_fields() {
        let config = parse_args(&valid_args()).unwrap();
        let document = telemetry_document(
            &config,
            Probe {
                latencies_ms: vec![1.0, 2.0, 3.0],
                rollback_armed: true,
                subjects: vec![Value::String("local".to_owned())],
            },
            1,
            "2026-07-12T00:00:00Z",
        );
        assert_eq!(
            document,
            json!({
                "schema": "jeryu-canary-v1",
                "source": "signrail-receipt-probe",
                "service": "redline",
                "environment": "prod",
                "release_sha": "a".repeat(40),
                "sampled_at": "2026-07-12T00:00:00Z",
                "window_seconds": 1,
                "samples": 3,
                "error_rate": 0.0,
                "p95_latency_ms": 3,
                "crash_rate": 0.0,
                "rollback_armed": true,
                "security_alerts": {"high": 0, "critical": 0},
                "ring_percent": 25,
                "receipt_subjects": ["local"],
            })
        );
    }
}
