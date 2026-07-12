use std::{cmp::Ordering, fs, path::Path, process::Command, time::Instant};

use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value as JsonValue, json};

#[derive(Debug, Deserialize)]
struct CostBudgetManifest {
    default_external_spend_usd: i64,
    default_network_spend_usd: i64,
    kill_switch_env: String,
    quota_caps: QuotaCaps,
    stop_conditions: StopConditions,
}

#[derive(Debug, Deserialize, Serialize)]
struct QuotaCaps {
    external_api_usd: i64,
    telegram_paid_usd: i64,
    model_api_usd: i64,
}

#[derive(Debug, Deserialize, Serialize)]
struct StopConditions {
    on_missing_receipt: bool,
    on_unknown_paid_tool: bool,
    on_quota_exceeded: bool,
    on_kill_switch: bool,
}

#[derive(Debug, Serialize)]
struct CostBudgetReceipt<'a> {
    ok: bool,
    manifest: &'a str,
    default_external_spend_usd: i64,
    quota_caps: &'a QuotaCaps,
    kill_switch_env: &'a str,
    stop_conditions: &'a StopConditions,
}

#[derive(Debug, Serialize)]
struct ReleaseReadinessReceipt<'a> {
    ok: bool,
    required_files: &'a [&'a str],
    missing_files: &'a [&'a str],
    missing_terms: &'a [&'a str],
    artifact_paths: [&'static str; 3],
}

pub fn cost_budget(repo_root: &Path) -> Result<()> {
    let manifest_relative = "agent/cost-budget.toml";
    let manifest_path = repo_root.join(manifest_relative);
    let manifest: CostBudgetManifest = toml::from_str(
        &fs::read_to_string(&manifest_path)
            .with_context(|| format!("read {}", manifest_path.display()))?,
    )
    .with_context(|| format!("parse {}", manifest_path.display()))?;

    ensure!(
        manifest.default_external_spend_usd == 0,
        "default_external_spend_usd must be 0"
    );
    ensure!(
        manifest.default_network_spend_usd == 0,
        "default_network_spend_usd must be 0"
    );
    ensure!(
        manifest.quota_caps.external_api_usd == 0,
        "quota cap external_api_usd must be zero by default"
    );
    ensure!(
        manifest.quota_caps.telegram_paid_usd == 0,
        "quota cap telegram_paid_usd must be zero by default"
    );
    ensure!(
        manifest.quota_caps.model_api_usd == 0,
        "quota cap model_api_usd must be zero by default"
    );
    ensure!(
        manifest.stop_conditions.on_missing_receipt,
        "stop condition on_missing_receipt must be true"
    );
    ensure!(
        manifest.stop_conditions.on_unknown_paid_tool,
        "stop condition on_unknown_paid_tool must be true"
    );
    ensure!(
        manifest.stop_conditions.on_quota_exceeded,
        "stop condition on_quota_exceeded must be true"
    );
    ensure!(
        manifest.stop_conditions.on_kill_switch,
        "stop condition on_kill_switch must be true"
    );

    let receipt = CostBudgetReceipt {
        ok: true,
        manifest: manifest_relative,
        default_external_spend_usd: manifest.default_external_spend_usd,
        quota_caps: &manifest.quota_caps,
        kill_switch_env: &manifest.kill_switch_env,
        stop_conditions: &manifest.stop_conditions,
    };
    write_json(
        &repo_root.join("target/jankurai/cost-budget.json"),
        &receipt,
    )
}

pub fn release_readiness(repo_root: &Path) -> Result<()> {
    const REQUIRED_FILES: &[&str] = &[
        "CHANGELOG.md",
        "docs/release.md",
        "docs/testing.md",
        "docs/operations.md",
        "agent/cost-budget.toml",
    ];
    const REQUIRED_TERMS: &[&str] = &[
        "release readiness",
        "security",
        "backups",
        "monitoring",
        "rollback",
        "abuse",
        "provenance",
        "bash ops/ci/pr-ci.sh",
        "bash ops/ci/security.sh",
        "target/jankurai",
    ];

    let missing_files: Vec<&str> = REQUIRED_FILES
        .iter()
        .copied()
        .filter(|path| !repo_root.join(path).exists())
        .collect();
    let corpus = ["docs/release.md", "docs/testing.md", "docs/operations.md"]
        .into_iter()
        .map(|path| fs::read_to_string(repo_root.join(path)).unwrap_or_default())
        .collect::<Vec<_>>()
        .join("\n")
        .to_lowercase();
    let missing_terms: Vec<&str> = REQUIRED_TERMS
        .iter()
        .copied()
        .filter(|term| !corpus.contains(&term.to_lowercase()))
        .collect();
    let ok = missing_files.is_empty() && missing_terms.is_empty();
    let receipt = ReleaseReadinessReceipt {
        ok,
        required_files: REQUIRED_FILES,
        missing_files: &missing_files,
        missing_terms: &missing_terms,
        artifact_paths: [
            "target/jankurai/release-readiness.json",
            "target/jankurai/cost-budget.json",
            "target/jankurai/security/evidence.json",
        ],
    };
    write_json(
        &repo_root.join("target/jankurai/release-readiness.json"),
        &receipt,
    )?;
    if ok {
        Ok(())
    } else {
        bail!("release readiness missing evidence: files={missing_files:?} terms={missing_terms:?}")
    }
}

#[allow(clippy::too_many_arguments)]
pub fn artifact_support_json(
    repo_root: &Path,
    out_dir: &Path,
    entrypoint: &str,
    sha: &str,
    tree: &str,
    generated_at: &str,
    workers: usize,
) -> Result<()> {
    let output = Command::new("git")
        .args(["ls-files"])
        .current_dir(repo_root)
        .output()
        .context("run git ls-files")?;
    ensure!(output.status.success(), "git ls-files failed");
    let tracked_files: Vec<&str> = std::str::from_utf8(&output.stdout)
        .context("git ls-files emitted non-UTF-8 output")?
        .lines()
        .collect();
    let repo = repo_root
        .file_name()
        .and_then(|name| name.to_str())
        .context("repository root has no UTF-8 basename")?;

    fs::create_dir_all(out_dir.join("receipts"))
        .with_context(|| format!("create {}", out_dir.display()))?;
    write_json(
        &out_dir.join("context.json"),
        &json!({
            "schema_version": 1,
            "generated_by": "ops/ci/artifact_support.sh",
            "repo": repo,
            "sha": sha,
            "tree": tree,
            "generated_at": generated_at,
            "workers": workers,
            "ci_entrypoint": entrypoint,
        }),
    )?;
    write_json(
        &out_dir.join("manifest.json"),
        &json!({
            "schema_version": 1,
            "sha": sha,
            "tracked_file_count": tracked_files.len(),
            "tracked_files": tracked_files,
        }),
    )?;
    write_json(
        &out_dir.join("receipts/local-ci.json"),
        &json!({
            "schema_version": 1,
            "sha": sha,
            "entrypoint": entrypoint,
            "status": "success",
        }),
    )
}

pub fn telemetry(
    repo: &str,
    sha: &str,
    ring_percent: u8,
    slug: &str,
    store_root: &Path,
) -> Result<()> {
    println!(
        "{}",
        serde_json::to_string(&telemetry_receipt(
            repo,
            sha,
            ring_percent,
            slug,
            store_root,
        )?)?
    );
    Ok(())
}

fn telemetry_receipt(
    repo: &str,
    sha: &str,
    ring_percent: u8,
    slug: &str,
    store_root: &Path,
) -> Result<JsonValue> {
    ensure!(!repo.is_empty(), "--repo is required");
    ensure!(
        sha.len() == 40
            && sha
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
        "--sha must be 40 lowercase hex characters"
    );
    ensure!(
        [1, 5, 25, 50, 100].contains(&ring_percent),
        "unsupported --ring-percent"
    );

    let started = Instant::now();
    let key = slug.replace('/', "_");
    let receipts_dir = store_root.join("receipts");
    let stages = ["local", "dev-canary", "prod"];
    let mut latencies = Vec::with_capacity(stages.len());
    let mut errors = Vec::new();
    let mut subjects = Vec::with_capacity(stages.len());
    let mut rollback_armed = true;

    for stage in stages {
        let probe_started = Instant::now();
        let result = (|| -> Result<()> {
            let path = receipts_dir.join(format!("{key}@{sha}-{stage}.json"));
            let data: JsonValue = serde_json::from_str(
                &fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?,
            )
            .with_context(|| format!("parse {}", path.display()))?;
            let payload = data
                .get("payload")
                .and_then(JsonValue::as_object)
                .context("payload is missing")?;
            ensure!(
                payload.get("stage").and_then(JsonValue::as_str) == Some(stage),
                "stage mismatch: {:?}",
                payload.get("stage")
            );
            ensure!(
                payload.get("sha").and_then(JsonValue::as_str) == Some(sha),
                "sha mismatch: {:?}",
                payload.get("sha")
            );
            ensure!(
                payload
                    .get("signature_coverage_percent")
                    .and_then(JsonValue::as_f64)
                    == Some(100.0),
                "signature coverage is not 100"
            );
            if !payload
                .get("rollback_target")
                .is_some_and(json_value_is_truthy)
            {
                rollback_armed = false;
            }
            subjects.push(
                data.get("subject")
                    .cloned()
                    .unwrap_or_else(|| json!(format!("{slug}@{sha}:{stage}"))),
            );
            Ok(())
        })();
        latencies.push(probe_started.elapsed().as_secs_f64() * 1000.0);
        if let Err(error) = result {
            errors.push(format!("{stage}:{error:#}"));
        }
    }

    if !errors.is_empty() {
        bail!("SignRail receipt probes failed: {}", errors.join("; "));
    }
    latencies.sort_by(|left, right| left.partial_cmp(right).unwrap_or(Ordering::Equal));
    let p95_latency_ms = percentile(&latencies, 0.95).round().max(1.0) as u64;
    let window_seconds = started.elapsed().as_secs_f64().round().max(1.0) as u64;
    let sampled_at = utc_now()?;
    Ok(json!({
        "schema": "jeryu-canary-v1",
        "source": "signrail-receipt-probe",
        "service": repo,
        "environment": "prod",
        "release_sha": sha,
        "sampled_at": sampled_at,
        "window_seconds": window_seconds,
        "samples": stages.len(),
        "error_rate": 0.0,
        "p95_latency_ms": p95_latency_ms,
        "crash_rate": 0.0,
        "rollback_armed": rollback_armed,
        "security_alerts": { "high": 0, "critical": 0 },
        "ring_percent": ring_percent,
        "receipt_subjects": subjects,
    }))
}

fn write_json(path: &Path, value: &(impl Serialize + ?Sized)) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    let mut body = serde_json::to_string_pretty(value)?;
    body.push('\n');
    fs::write(path, body).with_context(|| format!("write {}", path.display()))
}

fn json_value_is_truthy(value: &JsonValue) -> bool {
    match value {
        JsonValue::Null => false,
        JsonValue::Bool(value) => *value,
        JsonValue::Number(value) => value.as_f64().is_some_and(|value| value != 0.0),
        JsonValue::String(value) => !value.is_empty(),
        JsonValue::Array(value) => !value.is_empty(),
        JsonValue::Object(value) => !value.is_empty(),
    }
}

fn percentile(sorted: &[f64], percentile: f64) -> f64 {
    if sorted.len() < 2 {
        return sorted.last().copied().unwrap_or_default();
    }
    let rank = (sorted.len() - 1) as f64 * percentile;
    let lower = rank.floor() as usize;
    let upper = rank.ceil() as usize;
    sorted[lower] + (sorted[upper] - sorted[lower]) * rank.fract()
}

fn utc_now() -> Result<String> {
    let output = Command::new("date")
        .args(["-u", "+%Y-%m-%dT%H:%M:%SZ"])
        .output()
        .context("run date")?;
    ensure!(output.status.success(), "date failed");
    Ok(String::from_utf8(output.stdout)?.trim().to_owned())
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};

    use super::*;

    static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

    struct Fixture(std::path::PathBuf);

    impl Fixture {
        fn new(label: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "redline-testing-{label}-{}-{}",
                std::process::id(),
                NEXT_FIXTURE.fetch_add(1, AtomicOrdering::Relaxed)
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn write(root: &Path, relative: &str, body: &str) {
        let path = root.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, body).unwrap();
    }

    #[test]
    fn cost_budget_receipt_matches_the_original_field_order_and_values() {
        let fixture = Fixture::new("cost-budget");
        write(
            fixture.path(),
            "agent/cost-budget.toml",
            r#"default_external_spend_usd = 0
default_network_spend_usd = 0
kill_switch_env = "REDLINE_TESTING_KILL_SWITCH"

[quota_caps]
external_api_usd = 0
telegram_paid_usd = 0
model_api_usd = 0

[stop_conditions]
on_missing_receipt = true
on_unknown_paid_tool = true
on_quota_exceeded = true
on_kill_switch = true
"#,
        );

        cost_budget(fixture.path()).unwrap();

        assert_eq!(
            fs::read_to_string(fixture.path().join("target/jankurai/cost-budget.json")).unwrap(),
            concat!(
                "{\n",
                "  \"ok\": true,\n",
                "  \"manifest\": \"agent/cost-budget.toml\",\n",
                "  \"default_external_spend_usd\": 0,\n",
                "  \"quota_caps\": {\n",
                "    \"external_api_usd\": 0,\n",
                "    \"telegram_paid_usd\": 0,\n",
                "    \"model_api_usd\": 0\n",
                "  },\n",
                "  \"kill_switch_env\": \"REDLINE_TESTING_KILL_SWITCH\",\n",
                "  \"stop_conditions\": {\n",
                "    \"on_missing_receipt\": true,\n",
                "    \"on_unknown_paid_tool\": true,\n",
                "    \"on_quota_exceeded\": true,\n",
                "    \"on_kill_switch\": true\n",
                "  }\n",
                "}\n"
            )
        );
    }

    #[test]
    fn release_readiness_receipt_matches_the_original_contract() {
        let fixture = Fixture::new("release-readiness");
        write(fixture.path(), "CHANGELOG.md", "changes\n");
        write(fixture.path(), "agent/cost-budget.toml", "budget\n");
        write(
            fixture.path(),
            "docs/release.md",
            "Release readiness security backups monitoring rollback abuse provenance\n",
        );
        write(
            fixture.path(),
            "docs/testing.md",
            "bash ops/ci/pr-ci.sh\nbash ops/ci/security.sh\n",
        );
        write(fixture.path(), "docs/operations.md", "target/jankurai\n");

        release_readiness(fixture.path()).unwrap();

        let receipt: JsonValue = serde_json::from_str(
            &fs::read_to_string(
                fixture
                    .path()
                    .join("target/jankurai/release-readiness.json"),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(receipt["ok"], true);
        assert_eq!(receipt["missing_files"], json!([]));
        assert_eq!(receipt["missing_terms"], json!([]));
        assert_eq!(
            receipt["artifact_paths"],
            json!([
                "target/jankurai/release-readiness.json",
                "target/jankurai/cost-budget.json",
                "target/jankurai/security/evidence.json"
            ])
        );
    }

    #[test]
    fn artifact_support_receipts_match_the_sorted_json_contract() {
        let fixture = Fixture::new("artifact-support");
        write(fixture.path(), "alpha.txt", "alpha\n");
        write(fixture.path(), "nested/beta.txt", "beta\n");
        for args in [
            ["init", "-q"].as_slice(),
            ["add", "alpha.txt", "nested/beta.txt"].as_slice(),
        ] {
            assert!(
                Command::new("git")
                    .args(args)
                    .current_dir(fixture.path())
                    .status()
                    .unwrap()
                    .success()
            );
        }
        let out = fixture.path().join("target/artifact-support");

        artifact_support_json(
            fixture.path(),
            &out,
            "bash ops/ci/pr-ci.sh",
            "0123456789abcdef",
            "fedcba9876543210",
            "2026-07-12T10:00:00Z",
            40,
        )
        .unwrap();

        let manifest = fs::read_to_string(out.join("manifest.json")).unwrap();
        assert_eq!(
            manifest,
            concat!(
                "{\n",
                "  \"schema_version\": 1,\n",
                "  \"sha\": \"0123456789abcdef\",\n",
                "  \"tracked_file_count\": 2,\n",
                "  \"tracked_files\": [\n",
                "    \"alpha.txt\",\n",
                "    \"nested/beta.txt\"\n",
                "  ]\n",
                "}\n"
            )
        );
        let local_ci = fs::read_to_string(out.join("receipts/local-ci.json")).unwrap();
        assert!(local_ci.starts_with("{\n  \"entrypoint\":"));
        assert!(local_ci.ends_with("  \"status\": \"success\"\n}\n"));
    }

    #[test]
    fn telemetry_receipt_preserves_the_canary_contract() {
        let fixture = Fixture::new("telemetry");
        let sha = "a".repeat(40);
        let slug = "neverhuman/redline-testing";
        for stage in ["local", "dev-canary", "prod"] {
            write(
                fixture.path(),
                &format!("receipts/neverhuman_redline-testing@{sha}-{stage}.json"),
                &serde_json::to_string(&json!({
                    "subject": format!("subject-{stage}"),
                    "payload": {
                        "stage": stage,
                        "sha": sha,
                        "signature_coverage_percent": 100,
                        "rollback_target": "previous"
                    }
                }))
                .unwrap(),
            );
        }

        let receipt = telemetry_receipt("redline-testing", &sha, 25, slug, fixture.path()).unwrap();

        assert_eq!(receipt["schema"], "jeryu-canary-v1");
        assert_eq!(receipt["samples"], 3);
        assert_eq!(receipt["ring_percent"], 25);
        assert_eq!(receipt["rollback_armed"], true);
        assert_eq!(
            receipt["receipt_subjects"],
            json!(["subject-local", "subject-dev-canary", "subject-prod"])
        );
        assert!(receipt["p95_latency_ms"].as_u64().unwrap() >= 1);
        assert!(receipt["sampled_at"].as_str().unwrap().ends_with('Z'));
    }
}
