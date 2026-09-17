mod artifact;
mod clock;
mod file_hash;
mod perf;
mod score_ratchet;
mod telemetry;
mod toolchain;
mod w2_manifest;

use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    fs::OpenOptions,
    io::Write,
    path::{Component, Path, PathBuf},
    process,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, anyhow, bail};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use crate::file_hash::sha256_file;

const EXPECTED_SCHEMA: &str = "redline-testing-official-evidence-v1";
const PROCESSED_SCHEMA: &str = "redline-testing-official-evidence-processed-v1";
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

fn normalize_hash_map(value: &Value) -> Result<BTreeMap<String, String>> {
    let mut hashes = BTreeMap::new();
    match value {
        Value::Object(entries) => {
            for (key, item) in entries {
                if item.is_object() {
                    let path = first_string(item, &[&["path"], &["file"], &["name"]]);
                    let hash = first_hash(item, &[&["sha256"], &["hash"], &["digest"], &["value"]]);
                    if let Some(hash) = hash {
                        insert_hash(
                            &mut hashes,
                            normalize_path(path.as_deref().unwrap_or(key)),
                            hash,
                        )?;
                        continue;
                    }
                }
                if let Some(hash) = normalize_hash(item) {
                    insert_hash(&mut hashes, normalize_path(key), hash)?;
                }
            }
        }
        Value::Array(entries) => {
            for item in entries {
                if let (Some(path), Some(hash)) = (
                    first_string(item, &[&["path"], &["file"], &["name"]]),
                    first_hash(item, &[&["sha256"], &["hash"], &["digest"], &["value"]]),
                ) {
                    insert_hash(&mut hashes, normalize_path(&path), hash)?;
                }
            }
        }
        _ => {}
    }
    Ok(hashes)
}

fn insert_hash(hashes: &mut BTreeMap<String, String>, path: String, hash: String) -> Result<()> {
    if hashes.insert(path.clone(), hash).is_some() {
        bail!("duplicate normalized output hash path: {path}");
    }
    Ok(())
}

fn lookup_hash(hashes: &BTreeMap<String, String>, candidates: &[String]) -> Option<String> {
    candidates
        .iter()
        .find_map(|candidate| hashes.get(&normalize_path(candidate)).cloned())
}

fn suite_map(suites: &Value) -> Result<BTreeMap<String, Value>> {
    let mut mapped = BTreeMap::new();
    match suites {
        Value::Object(entries) => {
            for (name, entry) in entries {
                if !entry.is_object() {
                    bail!("suite {name:?} is not an object");
                }
                let normalized = normalize_path(name);
                if mapped.insert(normalized.clone(), entry.clone()).is_some() {
                    bail!("duplicate normalized suite name: {normalized}");
                }
            }
        }
        Value::Array(entries) => {
            for entry in entries {
                if !entry.is_object() {
                    bail!("suite entry is not an object");
                }
                let name = first_string(entry, &[&["name"], &["suite"]])
                    .ok_or_else(|| anyhow!("suite entry is missing a name"))?;
                let normalized = normalize_path(&name);
                if mapped.insert(normalized.clone(), entry.clone()).is_some() {
                    bail!("duplicate normalized suite name: {normalized}");
                }
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
            return validate_evidence_path(value);
        }
    }
    bail!("suite entry missing path field: {}", keys.join(", "))
}

fn validate_evidence_path(value: &str) -> Result<String> {
    let normalized = normalize_path(value);
    let path = Path::new(&normalized);
    if normalized.is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        bail!("evidence path is not a relative descendant: {value:?}");
    }
    Ok(normalized)
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

fn expected_runner_sha(root: &Path, canonical_root: &Path) -> Result<String> {
    let provenance_path =
        confined_regular_path(root, canonical_root, "redline-testing-provenance.env")?;
    let provenance = fs::read_to_string(&provenance_path)
        .with_context(|| format!("read runner provenance {}", provenance_path.display()))?;
    let names = [
        "CI_REDLINE_TESTING_RELEASE_BINARY_SHA256",
        "CI_REDLINE_TESTING_BIN_SHA256",
        "CI_REDLINE_TESTING_EXPECTED_BINARY_SHA256",
    ];
    let mut hashes = BTreeMap::new();
    for (line_number, line) in provenance.lines().enumerate() {
        let Some((name, value)) = line.split_once('=') else {
            continue;
        };
        if names.contains(&name) {
            let hash = normalize_hash(&Value::String(value.to_owned())).ok_or_else(|| {
                anyhow!(
                    "runner provenance {}:{} has an invalid {name} digest",
                    provenance_path.display(),
                    line_number + 1
                )
            })?;
            if hashes.insert(name, hash).is_some() {
                bail!(
                    "runner provenance {} declares {name} more than once",
                    provenance_path.display()
                );
            }
        }
    }
    for name in names {
        if let Some(hash) = hashes.get(name) {
            return Ok(hash.clone());
        }
    }
    bail!("verified redline-testing runner SHA-256 is unavailable from provenance")
}

fn confined_regular_path(root: &Path, canonical_root: &Path, relative: &str) -> Result<PathBuf> {
    let relative = validate_evidence_path(relative)?;
    let path = root.join(&relative);
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            bail!("required evidence file is missing: {relative}")
        }
        Err(error) => {
            return Err(error).with_context(|| format!("inspect evidence file {relative}"));
        }
    };
    if metadata.file_type().is_symlink() {
        bail!("required evidence file must not be a symlink: {relative}");
    }
    if !metadata.is_file() {
        bail!("required evidence file is not a regular file: {relative}");
    }
    let canonical = fs::canonicalize(&path)
        .with_context(|| format!("resolve required evidence file {relative}"))?;
    if !canonical.starts_with(canonical_root) {
        bail!("required evidence file escapes the evidence root: {relative}");
    }
    Ok(path)
}

fn atomic_write_inside(root: &Path, relative: &str, bytes: &[u8]) -> Result<PathBuf> {
    let relative = validate_evidence_path(relative)?;
    let destination = root.join(&relative);
    match fs::symlink_metadata(&destination) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            bail!("processed evidence destination must not be a symlink: {relative}")
        }
        Ok(metadata) if !metadata.is_file() => {
            bail!("processed evidence destination is not a regular file: {relative}")
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(error).with_context(|| format!("inspect evidence output {relative}"));
        }
    }

    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock precedes Unix epoch")?
        .as_nanos();
    let temporary = root.join(format!(
        ".official-evidence.processed.tmp-{}-{unique}",
        process::id()
    ));
    let write_result = (|| -> Result<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .with_context(|| format!("create temporary evidence output {}", temporary.display()))?;
        file.write_all(bytes)
            .with_context(|| format!("write temporary evidence output {}", temporary.display()))?;
        file.sync_all()
            .with_context(|| format!("sync temporary evidence output {}", temporary.display()))?;
        fs::rename(&temporary, &destination).with_context(|| {
            format!(
                "replace processed evidence {} with {}",
                destination.display(),
                temporary.display()
            )
        })?;
        Ok(())
    })();
    if write_result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    write_result?;
    Ok(destination)
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
    let accounted = passed
        .checked_add(skipped)
        .ok_or_else(|| anyhow!("suite {name} passed + skipped overflows u64"))?;
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
            if total != 1127 || accounted != 1127 || skipped > max_skips {
                bail!(
                    "suite {name} expected 1127 with at most {max_skips} target-capability skips, got total={total} passed={passed} skipped={skipped}"
                );
            }
        }
        "rql_phase1" if total != 594 || accounted != 594 => bail!(
            "suite {name} expected 594 runnable cases, got total={total} passed={passed} skipped={skipped}"
        ),
        "rql_phase1" => {}
        _ if accounted != total => bail!(
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

fn process_official(root: PathBuf) -> Result<PathBuf> {
    let repo_root = env::current_dir().context("resolve repository root")?;
    let canonical_root = fs::canonicalize(&root)
        .with_context(|| format!("resolve evidence root {}", root.display()))?;
    let official_path = confined_regular_path(&root, &canonical_root, "official-evidence.json")?;
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

    let output_hashes = normalize_hash_map(&official["output_file_hashes"])?;
    if output_hashes.is_empty() {
        bail!("official evidence output_file_hashes is empty");
    }
    let expected_sha = expected_runner_sha(&root, &canonical_root)?;
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
        let file_path = confined_regular_path(&root, &canonical_root, relative)?;
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
    let processed_path = atomic_write_inside(&root, "official-evidence.processed.json", &bytes)?;
    Ok(processed_path)
}

fn dispatch(args: &[String]) -> Result<()> {
    match args.first().map(String::as_str) {
        None => {
            let path = process_official(PathBuf::from("target/redline-testing"))?;
            println!("redline-testing evidence processed: {}", path.display());
        }
        Some("process-official") => {
            if args.len() > 2 {
                bail!("process-official accepts at most one evidence root");
            }
            let root = args
                .get(1)
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("target/redline-testing"));
            let path = process_official(root)?;
            println!("redline-testing evidence processed: {}", path.display());
        }
        Some("artifact-metadata") => artifact::run(&args[1..])?,
        Some("jankurai-ratchet") => score_ratchet::run_jankurai(&args[1..])?,
        Some("perf-summary") => perf::run(&args[1..])?,
        Some("score-ratchet") => score_ratchet::run(&args[1..])?,
        Some("telemetry") => telemetry::run(&args[1..])?,
        Some("toolchain-check") => toolchain::run(&args[1..])?,
        Some("w2-manifest") => w2_manifest::run(&args[1..])?,
        Some(command) => bail!("unknown command {command:?}"),
    }
    Ok(())
}

fn main() {
    let args = env::args().skip(1).collect::<Vec<_>>();
    match dispatch(&args) {
        Ok(()) => {}
        Err(error) => {
            eprintln!("redline Rust control: {error:#}");
            process::exit(if telemetry::is_usage_error(&error) {
                2
            } else {
                1
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

    struct FixtureDir(PathBuf);

    impl Drop for FixtureDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn evidence_fixture(label: &str) -> (FixtureDir, Value, String) {
        let root = env::temp_dir().join(format!(
            "redline-evidence-{label}-{}-{}",
            process::id(),
            NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap();
        for path in ROOT_REQUIRED_PATHS {
            fs::write(root.join(path), format!("fixture:{path}\n")).unwrap();
        }
        let hashes = ROOT_REQUIRED_PATHS
            .iter()
            .map(|path| ((*path).to_owned(), sha256_file(&root.join(path)).unwrap()))
            .collect::<BTreeMap<_, _>>();
        let suite = |total: u64| {
            serde_json::json!({
                "total": total,
                "passed": total,
                "failed": 0,
                "skipped": 0,
                "raw_path": "all.jsonl",
                "summary_path": "summary.json",
                "ranked_path": "all-manifest.json",
                "manifest_path": "manifest.json",
                "provenance_path": "provenance.json",
            })
        };
        let runner_sha = "a".repeat(64);
        let official = serde_json::json!({
            "schema_version": EXPECTED_SCHEMA,
            "runner": {"binary_sha256": runner_sha},
            "target": {"name": "redlinedb"},
            "sqlite": {"version": "3.53.1"},
            "suites": {
                "sqlite_parity": suite(1127),
                "memory": suite(1127),
                "rql_phase1": suite(594),
                "beyond_sqlite": suite(1),
            },
            "status": "passed",
            "command_line": ["redline-testing", "run"],
            "generated_at_unix_ms": 1,
            "output_file_hashes": hashes,
        });
        fs::write(
            root.join("official-evidence.json"),
            serde_json::to_vec_pretty(&official).unwrap(),
        )
        .unwrap();
        fs::write(
            root.join("redline-testing-provenance.env"),
            format!("CI_REDLINE_TESTING_RELEASE_BINARY_SHA256={runner_sha}\n"),
        )
        .unwrap();
        (FixtureDir(root), official, runner_sha)
    }

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
        assert_eq!(normalize_hash_map(&object).unwrap()["all.jsonl"], digest);

        let digest = "c".repeat(64);
        let array = serde_json::json!([{"path": "summary.json", "sha256": digest}]);
        assert_eq!(normalize_hash_map(&array).unwrap()["summary.json"], digest);
    }

    #[test]
    fn rejects_ambiguous_normalized_keys() {
        let digest = "d".repeat(64);
        let hashes = serde_json::json!({
            "path-a": {"path": "./all.jsonl", "sha256": digest},
            "all.jsonl": digest,
        });
        assert!(
            normalize_hash_map(&hashes)
                .unwrap_err()
                .to_string()
                .contains("duplicate normalized output hash path")
        );

        let suites = serde_json::json!([
            {"name": "./sqlite_parity"},
            {"name": "sqlite_parity"},
        ]);
        assert!(
            suite_map(&suites)
                .unwrap_err()
                .to_string()
                .contains("duplicate normalized suite name")
        );
    }

    #[test]
    fn dispatch_rejects_unknown_commands() {
        let error = dispatch(&["not-a-command".to_owned()]).unwrap_err();
        assert!(error.to_string().contains("unknown command"));
    }

    #[test]
    fn processes_a_complete_bound_bundle() {
        let (fixture, _, _) = evidence_fixture("valid");
        let processed = process_official(fixture.0.clone()).unwrap();
        let document: Value = serde_json::from_slice(&fs::read(processed).unwrap()).unwrap();
        assert_eq!(document["status"], "passed");
        assert_eq!(document["suite_summaries"]["sqlite_parity"]["passed"], 1127);
        assert_eq!(
            document["runner_expected_binary_sha256"],
            document["runner_observed_binary_sha256"]
        );
    }

    #[test]
    fn rejects_hash_runner_and_count_mismatches() {
        let (hash_fixture, _, _) = evidence_fixture("hash-mismatch");
        fs::write(hash_fixture.0.join("summary.json"), "tampered\n").unwrap();
        assert!(
            process_official(hash_fixture.0.clone())
                .unwrap_err()
                .to_string()
                .contains("sha256 mismatch")
        );

        let (runner_fixture, _, _) = evidence_fixture("runner-mismatch");
        fs::write(
            runner_fixture.0.join("redline-testing-provenance.env"),
            format!(
                "CI_REDLINE_TESTING_RELEASE_BINARY_SHA256={}\n",
                "b".repeat(64)
            ),
        )
        .unwrap();
        assert!(
            process_official(runner_fixture.0.clone())
                .unwrap_err()
                .to_string()
                .contains("runner SHA-256 mismatch")
        );

        let (count_fixture, mut official, _) = evidence_fixture("count-mismatch");
        official["suites"]["memory"]["failed"] = Value::from(1);
        fs::write(
            count_fixture.0.join("official-evidence.json"),
            serde_json::to_vec_pretty(&official).unwrap(),
        )
        .unwrap();
        assert!(
            process_official(count_fixture.0.clone())
                .unwrap_err()
                .to_string()
                .contains("suite memory failed 1 test")
        );
    }

    #[test]
    fn rejects_escaping_paths_and_count_overflow() {
        for path in ["../outside", "/absolute", "nested/../outside", "./"] {
            assert!(validate_evidence_path(path).is_err(), "accepted {path:?}");
        }
        assert_eq!(
            validate_evidence_path("nested/evidence.json").unwrap(),
            "nested/evidence.json"
        );

        let entry = serde_json::json!({
            "total": u64::MAX,
            "passed": u64::MAX,
            "failed": 0,
            "skipped": 1,
            "raw_path": "all.jsonl",
            "summary_path": "summary.json",
            "ranked_path": "ranked.csv",
            "manifest_path": "manifest.json",
            "provenance_path": "provenance.json",
        });
        let error = validated_suite("beyond_sqlite", &entry, &mut BTreeSet::new()).unwrap_err();
        assert!(error.to_string().contains("overflows u64"));
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlinked_required_evidence() {
        use std::os::unix::fs::symlink;

        let (fixture, _, _) = evidence_fixture("symlink");
        let outside = fixture.0.with_extension("outside");
        fs::write(&outside, "fixture:all.jsonl\n").unwrap();
        fs::remove_file(fixture.0.join("all.jsonl")).unwrap();
        symlink(&outside, fixture.0.join("all.jsonl")).unwrap();
        let error = process_official(fixture.0.clone()).unwrap_err();
        assert!(error.to_string().contains("must not be a symlink"));
        fs::remove_file(outside).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn rejects_required_evidence_through_symlinked_parent() {
        use std::os::unix::fs::symlink;

        let (fixture, mut official, _) = evidence_fixture("parent-symlink");
        let outside = fixture.0.with_extension("outside-dir");
        fs::create_dir_all(&outside).unwrap();
        fs::write(outside.join("all.jsonl"), "outside\n").unwrap();
        symlink(&outside, fixture.0.join("nested")).unwrap();
        official["suites"]["beyond_sqlite"]["raw_path"] = Value::from("nested/all.jsonl");
        official["output_file_hashes"]["nested/all.jsonl"] =
            Value::from(sha256_file(&outside.join("all.jsonl")).unwrap());
        fs::write(
            fixture.0.join("official-evidence.json"),
            serde_json::to_vec_pretty(&official).unwrap(),
        )
        .unwrap();

        let error = process_official(fixture.0.clone()).unwrap_err();
        assert!(error.to_string().contains("escapes the evidence root"));
        fs::remove_dir_all(outside).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlinked_control_inputs_and_output() {
        use std::os::unix::fs::symlink;

        let (official_fixture, _, _) = evidence_fixture("official-symlink");
        let official_outside = official_fixture.0.with_extension("official-outside");
        fs::copy(
            official_fixture.0.join("official-evidence.json"),
            &official_outside,
        )
        .unwrap();
        fs::remove_file(official_fixture.0.join("official-evidence.json")).unwrap();
        symlink(
            &official_outside,
            official_fixture.0.join("official-evidence.json"),
        )
        .unwrap();
        assert!(
            process_official(official_fixture.0.clone())
                .unwrap_err()
                .to_string()
                .contains("must not be a symlink")
        );
        fs::remove_file(official_outside).unwrap();

        let (provenance_fixture, _, _) = evidence_fixture("provenance-symlink");
        let provenance_outside = provenance_fixture.0.with_extension("provenance-outside");
        fs::copy(
            provenance_fixture.0.join("redline-testing-provenance.env"),
            &provenance_outside,
        )
        .unwrap();
        fs::remove_file(provenance_fixture.0.join("redline-testing-provenance.env")).unwrap();
        symlink(
            &provenance_outside,
            provenance_fixture.0.join("redline-testing-provenance.env"),
        )
        .unwrap();
        assert!(
            process_official(provenance_fixture.0.clone())
                .unwrap_err()
                .to_string()
                .contains("must not be a symlink")
        );
        fs::remove_file(provenance_outside).unwrap();

        let (output_fixture, _, _) = evidence_fixture("output-symlink");
        let output_outside = output_fixture.0.with_extension("output-outside");
        fs::write(&output_outside, "do-not-overwrite\n").unwrap();
        symlink(
            &output_outside,
            output_fixture.0.join("official-evidence.processed.json"),
        )
        .unwrap();
        assert!(
            process_official(output_fixture.0.clone())
                .unwrap_err()
                .to_string()
                .contains("destination must not be a symlink")
        );
        assert_eq!(
            fs::read_to_string(&output_outside).unwrap(),
            "do-not-overwrite\n"
        );
        fs::remove_file(output_outside).unwrap();
    }
}
