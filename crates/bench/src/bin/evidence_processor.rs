use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    path::{Path, PathBuf},
    process,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, anyhow, bail};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

const EXPECTED_SCHEMA: &str = "redline-testing-official-evidence-v1";
const PROCESSED_SCHEMA: &str = "redline-testing-official-evidence-processed-v1";
const EXPECTED_SQLITE_CASES: u64 = 2_445;
const EXPECTED_RQL_PHASE1_CASES: u64 = 1_385;
const REQUIRED_TOP_LEVEL_FIELDS: &[&str] = &[
    "schema_version",
    "runner",
    "target",
    "sqlite",
    "suites",
    "status",
    "command_line",
    "generated_at_unix_ms",
    "output_file_hashes",
];
const REQUIRED_SUITE_NAMES: &[&str] = &["sqlite_parity", "memory", "rql_phase1", "beyond_sqlite"];
const ROOT_REQUIRED_PATHS: &[&str] = &[
    "all.jsonl",
    "all-manifest.json",
    "summary.json",
    "manifest.json",
    "provenance.json",
    "memory-summary.json",
    "memory-manifest.json",
    "memory-provenance.json",
    "beyond-sqlite-summary.json",
    "beyond-sqlite-manifest.json",
    "beyond-sqlite-provenance.json",
];

fn normalize_path(value: &str) -> String {
    let normalized = value.trim().replace('\\', "/");
    normalized
        .strip_prefix("./")
        .unwrap_or(&normalized)
        .to_owned()
}

fn normalize_hash(value: &Value) -> Option<String> {
    let mut candidate = value.as_str()?.trim().to_ascii_lowercase();
    if let Some(stripped) = candidate.strip_prefix("sha256:") {
        candidate = stripped.to_owned();
    }
    (candidate.len() == 64 && candidate.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .then_some(candidate)
}

fn dig<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    path.iter()
        .try_fold(value, |current, segment| current.as_object()?.get(*segment))
}

fn first_string(value: &Value, paths: &[&[&str]]) -> Option<String> {
    paths.iter().find_map(|path| {
        let candidate = dig(value, path)?.as_str()?.trim();
        (!candidate.is_empty()).then(|| candidate.to_owned())
    })
}

fn first_hash(value: &Value, paths: &[&[&str]]) -> Option<String> {
    paths
        .iter()
        .find_map(|path| dig(value, path).and_then(normalize_hash))
}

fn normalize_hash_map(value: &Value) -> BTreeMap<String, String> {
    let mut hashes = BTreeMap::new();
    match value {
        Value::Object(entries) => {
            for (key, item) in entries {
                if item.is_object() {
                    let path = first_string(item, &[&["path"], &["file"], &["name"]]);
                    let hash = first_hash(item, &[&["sha256"], &["hash"], &["digest"], &["value"]]);
                    if let Some(hash) = hash {
                        hashes.insert(normalize_path(path.as_deref().unwrap_or(key)), hash);
                        continue;
                    }
                }
                if let Some(hash) = normalize_hash(item) {
                    hashes.insert(normalize_path(key), hash);
                }
            }
        }
        Value::Array(entries) => {
            for item in entries {
                if let (Some(path), Some(hash)) = (
                    first_string(item, &[&["path"], &["file"], &["name"]]),
                    first_hash(item, &[&["sha256"], &["hash"], &["digest"], &["value"]]),
                ) {
                    hashes.insert(normalize_path(&path), hash);
                }
            }
        }
        _ => {}
    }
    hashes
}

fn lookup_hash(hashes: &BTreeMap<String, String>, candidates: &[String]) -> Option<String> {
    candidates
        .iter()
        .find_map(|candidate| hashes.get(&normalize_path(candidate)).cloned())
}

fn sha256_file(path: &Path) -> Result<String> {
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

fn suite_map(suites: &Value) -> Result<BTreeMap<String, Value>> {
    let mut mapped = BTreeMap::new();
    match suites {
        Value::Object(entries) => {
            for (name, entry) in entries {
                if !entry.is_object() {
                    bail!("suite {name:?} is not an object");
                }
                mapped.insert(normalize_path(name), entry.clone());
            }
        }
        Value::Array(entries) => {
            for entry in entries {
                if !entry.is_object() {
                    bail!("suite entry is not an object");
                }
                let name = first_string(entry, &[&["name"], &["suite"]])
                    .ok_or_else(|| anyhow!("suite entry is missing a name"))?;
                mapped.insert(normalize_path(&name), entry.clone());
            }
        }
        _ => bail!("suites must be an object or array"),
    }
    Ok(mapped)
}

fn suite_int(entry: &Value, keys: &[&str]) -> Result<u64> {
    let object = entry
        .as_object()
        .ok_or_else(|| anyhow!("suite entry is not an object"))?;
    for key in keys {
        match object.get(*key) {
            Some(Value::Number(value)) => {
                if let Some(value) = value.as_u64() {
                    return Ok(value);
                }
            }
            Some(Value::String(value)) if value.bytes().all(|byte| byte.is_ascii_digit()) => {
                return value
                    .parse()
                    .with_context(|| format!("parse suite integer field {key}"));
            }
            _ => {}
        }
    }
    bail!("suite entry missing integer field: {}", keys.join(", "))
}

fn suite_path(entry: &Value, keys: &[&str]) -> Result<String> {
    let object = entry
        .as_object()
        .ok_or_else(|| anyhow!("suite entry is not an object"))?;
    for key in keys {
        if let Some(value) = object
            .get(*key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return Ok(normalize_path(value));
        }
    }
    bail!("suite entry missing path field: {}", keys.join(", "))
}

fn runner_sha(evidence: &Value) -> Result<String> {
    let runner = evidence
        .get("runner")
        .filter(|value| value.is_object())
        .ok_or_else(|| anyhow!("runner field is missing or not an object"))?;
    first_hash(
        runner,
        &[
            &["binary_sha256"],
            &["sha256"],
            &["release_binary_sha256"],
            &["release_artifact", "binary_sha256"],
            &["release_artifact", "bin_sha256"],
            &["binary", "sha256"],
        ],
    )
    .ok_or_else(|| anyhow!("runner object does not expose a binary SHA-256"))
}

fn expected_runner_sha() -> Result<String> {
    for name in [
        "CI_REDLINE_TESTING_RELEASE_BINARY_SHA256",
        "CI_REDLINE_TESTING_BIN_SHA256",
        "CI_REDLINE_TESTING_EXPECTED_BINARY_SHA256",
    ] {
        if let Ok(value) = env::var(name) {
            if !value.is_empty() {
                return normalize_hash(&Value::String(value)).ok_or_else(|| {
                    anyhow!("verified redline-testing runner SHA-256 is not a valid digest")
                });
            }
        }
    }
    bail!("verified redline-testing runner SHA-256 is unavailable from provenance")
}

fn hash_candidates(repo_root: &Path, root: &Path, relative: &str) -> Vec<String> {
    let file_path = root.join(relative);
    let absolute = if file_path.is_absolute() {
        file_path.clone()
    } else {
        repo_root.join(&file_path)
    };
    vec![
        relative.to_owned(),
        file_path.to_string_lossy().into_owned(),
        absolute.to_string_lossy().into_owned(),
        Path::new(relative)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
    ]
}

fn validated_suite(
    name: &str,
    entry: &Value,
    required_paths: &mut BTreeSet<String>,
) -> Result<Map<String, Value>> {
    let total = suite_int(entry, &["total"])?;
    let passed = suite_int(entry, &["passed"])?;
    let failed = suite_int(entry, &["failed"])?;
    let skipped = suite_int(entry, &["skipped"])?;
    let raw_path = suite_path(entry, &["raw_path", "raw"])?;
    let summary_path = suite_path(entry, &["summary_path", "summary"])?;
    let ranked_path = suite_path(entry, &["ranked_path", "ranked"])?;
    let manifest_path = suite_path(entry, &["manifest_path", "manifest"])?;
    let provenance_path = suite_path(entry, &["provenance_path", "provenance"])?;

    required_paths.extend([
        raw_path.clone(),
        summary_path.clone(),
        ranked_path.clone(),
        manifest_path.clone(),
        provenance_path.clone(),
    ]);

    if failed != 0 {
        bail!("suite {name} failed {failed} test(s)");
    }
    match name {
        "sqlite_parity" | "memory" => {
            let max_skips = 4;
            if total != EXPECTED_SQLITE_CASES
                || passed + skipped != EXPECTED_SQLITE_CASES
                || skipped > max_skips
            {
                bail!(
                    "suite {name} expected {EXPECTED_SQLITE_CASES} with at most {max_skips} target-capability skips, got total={total} passed={passed} skipped={skipped}"
                );
            }
        }
        "rql_phase1" => {
            if total != EXPECTED_RQL_PHASE1_CASES || passed + skipped != EXPECTED_RQL_PHASE1_CASES {
                bail!(
                    "suite {name} expected {EXPECTED_RQL_PHASE1_CASES} runnable cases, got total={total} passed={passed} skipped={skipped}"
                );
            }
        }
        _ if passed + skipped != total => bail!(
            "suite {name} has inconsistent totals: passed={passed} skipped={skipped} total={total}"
        ),
        _ => {}
    }

    let mut result = Map::new();
    for (key, value) in [
        ("total", Value::from(total)),
        ("passed", Value::from(passed)),
        ("failed", Value::from(failed)),
        ("skipped", Value::from(skipped)),
        ("raw_path", Value::String(raw_path)),
        ("summary_path", Value::String(summary_path)),
        ("ranked_path", Value::String(ranked_path)),
        ("manifest_path", Value::String(manifest_path)),
        ("provenance_path", Value::String(provenance_path)),
    ] {
        result.insert(key.to_owned(), value);
    }
    Ok(result)
}

fn add_suite_hashes(
    repo_root: &Path,
    root: &Path,
    output_hashes: &BTreeMap<String, String>,
    suite: &mut Map<String, Value>,
) -> Result<()> {
    for (path_key, hash_key) in [
        ("raw_path", "raw_sha256"),
        ("summary_path", "summary_sha256"),
        ("ranked_path", "ranked_sha256"),
        ("manifest_path", "manifest_sha256"),
        ("provenance_path", "provenance_sha256"),
    ] {
        let path = suite
            .get(path_key)
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("processed suite missing {path_key}"))?;
        let candidates = hash_candidates(repo_root, root, path);
        let hash = lookup_hash(output_hashes, &candidates).ok_or_else(|| {
            anyhow!(
                "official evidence does not declare a hash for any of: {}",
                candidates.join(", ")
            )
        })?;
        suite.insert(hash_key.to_owned(), Value::String(hash));
    }
    Ok(())
}

fn run(root: PathBuf) -> Result<PathBuf> {
    let repo_root = env::current_dir().context("resolve repository root")?;
    let official_path = root.join("official-evidence.json");
    let processed_path = root.join("official-evidence.processed.json");
    let official_bytes = fs::read(&official_path)
        .with_context(|| format!("read official evidence {}", official_path.display()))?;
    let official: Value = serde_json::from_slice(&official_bytes)
        .with_context(|| format!("parse official evidence {}", official_path.display()))?;
    let object = official
        .as_object()
        .ok_or_else(|| anyhow!("official evidence is not an object"))?;

    let missing = REQUIRED_TOP_LEVEL_FIELDS
        .iter()
        .filter(|field| !object.contains_key(**field))
        .copied()
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        bail!(
            "official evidence missing top-level field(s): {}",
            missing.join(", ")
        );
    }
    if official.get("schema_version").and_then(Value::as_str) != Some(EXPECTED_SCHEMA) {
        bail!(
            "official evidence schema_version {:?} != {EXPECTED_SCHEMA:?}",
            official.get("schema_version")
        );
    }
    let status = official
        .get("status")
        .and_then(Value::as_str)
        .map(|value| value.trim().to_ascii_lowercase())
        .ok_or_else(|| anyhow!("status is missing or not a string"))?;
    if !["passed", "pass", "success", "succeeded", "ok"].contains(&status.as_str()) {
        bail!("official evidence status is not successful: {status:?}");
    }

    let output_hashes = normalize_hash_map(&official["output_file_hashes"]);
    if output_hashes.is_empty() {
        bail!("official evidence output_file_hashes is empty");
    }
    let expected_sha = expected_runner_sha()?;
    let observed_sha = runner_sha(&official)?;
    if observed_sha != expected_sha {
        bail!("runner SHA-256 mismatch: expected {expected_sha}, got {observed_sha}");
    }

    let suites = suite_map(&official["suites"])?;
    let missing_suites = REQUIRED_SUITE_NAMES
        .iter()
        .filter(|name| !suites.contains_key(**name))
        .copied()
        .collect::<Vec<_>>();
    if !missing_suites.is_empty() {
        bail!(
            "official evidence missing suite(s): {}",
            missing_suites.join(", ")
        );
    }

    let mut required_paths = ROOT_REQUIRED_PATHS
        .iter()
        .map(|path| (*path).to_owned())
        .collect::<BTreeSet<_>>();
    let mut validated = BTreeMap::new();
    for name in REQUIRED_SUITE_NAMES {
        validated.insert(
            (*name).to_owned(),
            validated_suite(name, &suites[*name], &mut required_paths)?,
        );
    }

    for relative in &required_paths {
        let file_path = root.join(relative);
        if !file_path.is_file() {
            bail!("required evidence file is missing: {relative}");
        }
        let actual = sha256_file(&file_path)?;
        let candidates = hash_candidates(&repo_root, &root, relative);
        let expected = lookup_hash(&output_hashes, &candidates)
            .ok_or_else(|| anyhow!("official evidence does not declare a hash for {relative}"))?;
        if actual != expected {
            bail!("sha256 mismatch for {relative}: expected {expected}, got {actual}");
        }
    }
    for suite in validated.values_mut() {
        add_suite_hashes(&repo_root, &root, &output_hashes, suite)?;
    }

    let validated_ms = u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .context("system clock precedes Unix epoch")?
            .as_millis(),
    )
    .context("validated timestamp does not fit in u64")?;
    let mut processed = Map::new();
    processed.insert(
        "schema_version".to_owned(),
        Value::String(PROCESSED_SCHEMA.to_owned()),
    );
    processed.insert(
        "source_path".to_owned(),
        Value::String(official_path.to_string_lossy().into_owned()),
    );
    processed.insert(
        "source_sha256".to_owned(),
        Value::String(format!("{:x}", Sha256::digest(&official_bytes))),
    );
    processed.insert("validated_at_unix_ms".to_owned(), Value::from(validated_ms));
    for key in ["generated_at_unix_ms", "command_line", "target", "sqlite"] {
        processed.insert(key.to_owned(), official[key].clone());
    }
    processed.insert(
        "runner_expected_binary_sha256".to_owned(),
        Value::String(expected_sha),
    );
    processed.insert(
        "runner_observed_binary_sha256".to_owned(),
        Value::String(observed_sha),
    );
    processed.insert("status".to_owned(), Value::String("passed".to_owned()));
    processed.insert(
        "suite_summaries".to_owned(),
        Value::Object(
            validated
                .into_iter()
                .map(|(name, suite)| (name, Value::Object(suite)))
                .collect(),
        ),
    );
    processed.insert(
        "output_file_hashes".to_owned(),
        serde_json::to_value(output_hashes).context("serialize output hash map")?,
    );
    processed.insert("official_evidence".to_owned(), official);

    let mut bytes = serde_json::to_vec_pretty(&Value::Object(processed))?;
    bytes.push(b'\n');
    fs::write(&processed_path, bytes)
        .with_context(|| format!("write {}", processed_path.display()))?;
    Ok(processed_path)
}

fn main() {
    let root = env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/redline-testing"));
    match run(root) {
        Ok(path) => println!("redline-testing evidence processed: {}", path.display()),
        Err(error) => {
            eprintln!("redline-testing evidence processor: {error:#}");
            process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_normalization_accepts_supported_forms() {
        let digest = "a".repeat(64);
        assert_eq!(
            normalize_hash(&Value::String(format!("sha256:{digest}"))),
            Some(digest)
        );
        assert_eq!(normalize_hash(&Value::String("xyz".to_owned())), None);
    }

    #[test]
    fn hash_maps_accept_object_and_array_shapes() {
        let digest = "b".repeat(64);
        let object = serde_json::json!({"./all.jsonl": format!("sha256:{digest}")});
        assert_eq!(normalize_hash_map(&object)["all.jsonl"], digest);

        let digest = "c".repeat(64);
        let array = serde_json::json!([{"path": "summary.json", "sha256": digest}]);
        assert_eq!(normalize_hash_map(&array)["summary.json"], digest);
    }

    #[test]
    fn accepts_the_pinned_v101_suite_sizes() {
        let suite = |total, passed, skipped| {
            serde_json::json!({
                "total": total,
                "passed": passed,
                "failed": 0,
                "skipped": skipped,
                "raw_path": "raw.jsonl",
                "summary_path": "summary.json",
                "ranked_path": "ranked.csv",
                "manifest_path": "manifest.json",
                "provenance_path": "provenance.json"
            })
        };
        let mut paths = BTreeSet::new();
        assert!(validated_suite("sqlite_parity", &suite(2_445, 2_441, 4), &mut paths).is_ok());
        assert!(validated_suite("memory", &suite(2_445, 2_441, 4), &mut paths).is_ok());
        assert!(validated_suite("rql_phase1", &suite(1_385, 1_129, 256), &mut paths).is_ok());
        assert!(validated_suite("sqlite_parity", &suite(1_127, 1_123, 4), &mut paths).is_err());
    }
}
