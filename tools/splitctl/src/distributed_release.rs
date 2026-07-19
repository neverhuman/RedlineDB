use serde_json::{json, Map, Value as JsonValue};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    io::{self, Write},
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
};

const RELEASE: &str = "9.0.0-distributed.1";
const ROLLBACK: &str = "7.0.6";
const MAX_AUTHORITY_BYTES: u64 = 16 * 1024 * 1024;

pub(crate) fn validate_command(args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    let mut release_spec = None;
    let mut soak_status = None;
    let mut qualification = None;
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--release-spec" => {
                release_spec = Some(PathBuf::from(
                    iter.next().ok_or("--release-spec needs a path")?,
                ))
            }
            "--soak-status" => {
                soak_status = Some(PathBuf::from(
                    iter.next().ok_or("--soak-status needs a path")?,
                ))
            }
            "--qualification" => {
                qualification = Some(PathBuf::from(
                    iter.next().ok_or("--qualification needs a path")?,
                ))
            }
            value => return Err(format!("unknown distributed-validate argument: {value}").into()),
        }
    }
    let release_spec = release_spec.ok_or("--release-spec is required")?;
    let soak_status = soak_status.ok_or("--soak-status is required")?;
    let qualification = qualification.ok_or("--qualification is required")?;
    let (release_bytes, release_value) = read_authority_json(&release_spec, "release spec")?;
    let (soak_bytes, soak_value) = read_authority_json(&soak_status, "soak status")?;
    let (qualification_bytes, qualification_value) =
        read_authority_json(&qualification, "qualification")?;

    validate_release_spec(&release_value)?;
    validate_soak_status(&soak_value)?;
    validate_qualification(&qualification_value)?;
    validate_cross_bindings(
        &release_value,
        &soak_value,
        &qualification_value,
        &soak_bytes,
        &qualification_bytes,
    )?;
    println!(
        "distributed authority valid: release={} spec_sha256={} soak_sha256={} qualification_sha256={}",
        RELEASE,
        sha256_bytes(&release_bytes),
        sha256_bytes(&soak_bytes),
        sha256_bytes(&qualification_bytes)
    );
    Ok(())
}

pub(crate) fn dag_command(args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    let mut manifest = None;
    let mut release = None;
    let mut output = None;
    let mut apply = false;
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--manifest" => {
                manifest = Some(PathBuf::from(iter.next().ok_or("--manifest needs a path")?))
            }
            "--release" => release = Some(iter.next().ok_or("--release needs a value")?),
            "--json" => output = Some(PathBuf::from(iter.next().ok_or("--json needs a path")?)),
            "--apply" => apply = true,
            value => return Err(format!("unknown release-dag argument: {value}").into()),
        }
    }
    let manifest = manifest.ok_or("--manifest is required")?;
    if release.as_deref() != Some(RELEASE) {
        return Err(format!("--release must be {RELEASE}").into());
    }
    let bytes = read_regular_bytes(&manifest, "release DAG manifest")?;
    let data: toml::Value = std::str::from_utf8(&bytes)?.parse()?;
    let report = build_dag(&data, sha256_bytes(&bytes))?;
    emit_json(report, output.as_deref(), apply)
}

pub(crate) fn evidence_index_command(args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    let mut root = None;
    let mut historical_roots = Vec::new();
    let mut conditional_historical_roots = Vec::new();
    let mut absent_historical_roots = Vec::new();
    let mut release = None;
    let mut output = None;
    let mut apply = false;
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--root" => root = Some(PathBuf::from(iter.next().ok_or("--root needs a path")?)),
            "--historical-root" => historical_roots.push(PathBuf::from(
                iter.next().ok_or("--historical-root needs a path")?,
            )),
            "--historical-root-if-present" => conditional_historical_roots.push(PathBuf::from(
                iter.next()
                    .ok_or("--historical-root-if-present needs a path")?,
            )),
            "--absent-historical-root" => absent_historical_roots.push(PathBuf::from(
                iter.next().ok_or("--absent-historical-root needs a path")?,
            )),
            "--release" => release = Some(iter.next().ok_or("--release needs a value")?),
            "--json" => output = Some(PathBuf::from(iter.next().ok_or("--json needs a path")?)),
            "--apply" => apply = true,
            value => {
                return Err(format!("unknown distributed-evidence-index argument: {value}").into())
            }
        }
    }
    let root = root.ok_or("--root is required")?;
    if release.as_deref() != Some(RELEASE) {
        return Err(format!("--release must be {RELEASE}").into());
    }
    if historical_roots.is_empty()
        && conditional_historical_roots.is_empty()
        && absent_historical_roots.is_empty()
    {
        return Err("at least one historical evidence boundary is required".into());
    }
    for conditional in conditional_historical_roots {
        match fs::symlink_metadata(&conditional) {
            Ok(_) => historical_roots.push(conditional),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                absent_historical_roots.push(conditional)
            }
            Err(error) => return Err(error.into()),
        }
    }
    let output_canonical = output.as_deref().map(resolve_output_path).transpose()?;
    let report = build_evidence_index(
        &root,
        &historical_roots,
        &absent_historical_roots,
        output_canonical.as_deref(),
    )?;
    emit_json(report, output_canonical.as_deref(), apply)
}

fn read_authority_json(
    path: &Path,
    label: &str,
) -> Result<(Vec<u8>, JsonValue), Box<dyn std::error::Error>> {
    let bytes = read_regular_bytes(path, label)?;
    let value = serde_json::from_slice(&bytes)?;
    Ok((bytes, value))
}

fn read_regular_bytes(path: &Path, label: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.file_type().is_file()
        || metadata.file_type().is_symlink()
        || metadata.nlink() != 1
        || metadata.len() == 0
        || metadata.len() > MAX_AUTHORITY_BYTES
    {
        return Err(format!("{label} is not a bounded independent regular file").into());
    }
    Ok(fs::read(path)?)
}

fn validate_release_spec(value: &JsonValue) -> Result<(), Box<dyn std::error::Error>> {
    reject_historical_identity(value)?;
    let object = exact_object(
        value,
        &[
            "activation_eligible",
            "artifact_set_sha256",
            "artifacts",
            "formal_ga",
            "hosts",
            "public_routed",
            "receipts",
            "redline",
            "release",
            "release_id",
            "rollback",
            "rollback_target",
            "routes",
            "schema_version",
            "signatures",
            "source_freeze_sha256",
            "source_matrix",
            "status",
        ],
        "distributed release spec",
    )?;
    expect_string(object, "schema_version", "jain.distributed-release/v1")?;
    expect_string(object, "release", RELEASE)?;
    expect_string(
        object,
        "release_id",
        "9.0.0-distributed.1/release-authority",
    )?;
    expect_string(object, "status", "candidate")?;
    expect_bool(object, "formal_ga", false)?;
    expect_bool(object, "public_routed", false)?;
    expect_bool(object, "activation_eligible", false)?;
    expect_string(object, "rollback_target", ROLLBACK)?;
    require_sha256(object, "source_freeze_sha256")?;
    require_sha256(object, "artifact_set_sha256")?;

    let source_matrix = require_array(object, "source_matrix")?;
    if source_matrix.is_empty() {
        return Err("source_matrix must not be empty".into());
    }
    let mut repositories = BTreeSet::new();
    for (index, row) in source_matrix.iter().enumerate() {
        let row = exact_object(
            row,
            &["checksum_sha256", "commit", "repository", "tag", "tree"],
            &format!("source_matrix[{index}]"),
        )?;
        let repository = require_string(row, "repository")?;
        if !valid_repository_name(repository) || !repositories.insert(repository.to_owned()) {
            return Err(
                format!("source_matrix repository is invalid or duplicate: {repository}").into(),
            );
        }
        let tag = require_string(row, "tag")?;
        if !tag.contains("-v9.0.0-distributed.1-") {
            return Err(format!("source_matrix tag is not distributed.1: {tag}").into());
        }
        require_hex(row, "commit", 40)?;
        require_hex(row, "tree", 40)?;
        require_sha256(row, "checksum_sha256")?;
    }
    if require_string(object, "source_freeze_sha256")? != source_matrix_digest(source_matrix)? {
        return Err("source_freeze_sha256 does not bind the canonical source matrix".into());
    }

    let artifacts = require_array(object, "artifacts")?;
    let mut names = BTreeSet::new();
    let mut kinds = BTreeSet::new();
    for (index, artifact) in artifacts.iter().enumerate() {
        let artifact = exact_object(
            artifact,
            &[
                "kind",
                "name",
                "readback_sha256",
                "sha256",
                "signature_receipt_sha256",
            ],
            &format!("artifacts[{index}]"),
        )?;
        let name = require_string(artifact, "name")?;
        if name.is_empty() || !names.insert(name.to_owned()) {
            return Err(format!("artifact name is empty or duplicate: {name}").into());
        }
        let kind = require_string(artifact, "kind")?;
        if !matches!(
            kind,
            "caddy"
                | "hub"
                | "node"
                | "worker"
                | "pack"
                | "compose"
                | "installer"
                | "appliance_bundle"
        ) {
            return Err(format!("unsupported distributed artifact kind: {kind}").into());
        }
        kinds.insert(kind.to_owned());
        let digest = require_sha256(artifact, "sha256")?;
        let readback = require_sha256(artifact, "readback_sha256")?;
        if digest != readback {
            return Err(format!("artifact {name} was not bound to exact readback bytes").into());
        }
        require_sha256(artifact, "signature_receipt_sha256")?;
    }
    if require_string(object, "artifact_set_sha256")? != artifact_set_digest(artifacts)? {
        return Err("artifact_set_sha256 does not bind the canonical artifact inventory".into());
    }
    for required in ["caddy", "hub", "node", "worker", "appliance_bundle"] {
        if !kinds.contains(required) {
            return Err(
                format!("required distributed artifact kind is missing: {required}").into(),
            );
        }
    }

    validate_hosts(require_array(object, "hosts")?)?;
    validate_routes(object.get("routes").ok_or("routes is required")?)?;
    let receipts = exact_object(
        object.get("receipts").ok_or("receipts is required")?,
        &[
            "accelerated_qualification_sha256",
            "caddy_unchanged_sha256",
            "gpu_sha256",
            "jeryu_parity_sha256",
            "migration_sha256",
            "redline_jain_consumer_sha256",
            "redline_jeryu_consumer_sha256",
            "redline_lock_sha256",
            "rollback_sha256",
            "soak_status_sha256",
            "storage_sha256",
        ],
        "receipts",
    )?;
    for key in receipts.keys() {
        require_sha256(receipts, key)?;
    }
    validate_redline(
        object.get("redline").ok_or("redline is required")?,
        receipts,
    )?;
    validate_rollback(
        object.get("rollback").ok_or("rollback is required")?,
        receipts,
    )?;
    validate_signatures(require_array(object, "signatures")?, 2, true)?;
    Ok(())
}

fn validate_hosts(hosts: &[JsonValue]) -> Result<(), Box<dyn std::error::Error>> {
    if hosts.len() != 3 {
        return Err("distributed release must bind exactly three hosts".into());
    }
    let expected = BTreeMap::from([
        (
            "AtomicSoul",
            ("edge-storage", 2_199_023_255_552_u64, vec![18080_u64]),
        ),
        (
            "xbabe2",
            ("follower-compute", 805_306_368_000_u64, vec![7445_u64]),
        ),
        (
            "xbabe3",
            (
                "primary-compute",
                85_899_345_920_u64,
                vec![7443_u64, 7444_u64, 7445_u64],
            ),
        ),
    ]);
    let mut seen = BTreeSet::new();
    let mut epochs = BTreeSet::new();
    for (index, host) in hosts.iter().enumerate() {
        let host = exact_object(
            host,
            &[
                "epoch",
                "external_caddy_unchanged",
                "host_id",
                "image_set_sha256",
                "ports",
                "role",
                "storage_quota_bytes",
            ],
            &format!("hosts[{index}]"),
        )?;
        let id = require_string(host, "host_id")?;
        if !seen.insert(id.to_owned()) {
            return Err(format!("duplicate host identity: {id}").into());
        }
        let (role, quota, ports) = expected
            .get(id)
            .ok_or_else(|| format!("unexpected distributed host: {id}"))?;
        expect_string(host, "role", role)?;
        expect_u64(host, "storage_quota_bytes", *quota)?;
        let epoch = require_u64(host, "epoch")?;
        if epoch == 0 {
            return Err(format!("host {id} has a zero controller epoch").into());
        }
        epochs.insert(epoch);
        require_sha256(host, "image_set_sha256")?;
        expect_bool(host, "external_caddy_unchanged", true)?;
        let actual_ports = require_array(host, "ports")?
            .iter()
            .map(|value| value.as_u64().ok_or("host port is not an integer"))
            .collect::<Result<Vec<_>, _>>()?;
        if &actual_ports != ports {
            return Err(format!("host {id} ports differ from the unrouted topology").into());
        }
    }
    if epochs.len() != 1 {
        return Err("distributed hosts do not bind one active epoch".into());
    }
    Ok(())
}

fn validate_routes(value: &JsonValue) -> Result<(), Box<dyn std::error::Error>> {
    let routes = exact_object(
        value,
        &[
            "canary_listen",
            "dns_unchanged",
            "external_caddy_etag_after",
            "external_caddy_etag_before",
            "external_caddy_route_sha256_after",
            "external_caddy_route_sha256_before",
            "public_admission_unchanged",
            "standalone_listen",
        ],
        "routes",
    )?;
    expect_string(routes, "canary_listen", "127.0.0.1:18080")?;
    expect_string(routes, "standalone_listen", "127.0.0.1:8080")?;
    expect_bool(routes, "dns_unchanged", true)?;
    expect_bool(routes, "public_admission_unchanged", true)?;
    let etag_before = require_string(routes, "external_caddy_etag_before")?;
    let etag_after = require_string(routes, "external_caddy_etag_after")?;
    if etag_before.is_empty() || etag_before != etag_after {
        return Err("external Caddy ETag changed during staging".into());
    }
    let route_before = require_sha256(routes, "external_caddy_route_sha256_before")?;
    let route_after = require_sha256(routes, "external_caddy_route_sha256_after")?;
    if route_before != route_after {
        return Err("external Caddy route array changed during staging".into());
    }
    Ok(())
}

fn validate_redline(
    value: &JsonValue,
    receipts: &Map<String, JsonValue>,
) -> Result<(), Box<dyn std::error::Error>> {
    let redline = exact_object(
        value,
        &[
            "cutover_eligible",
            "engine_commit",
            "engine_tag",
            "family_ci_sha256",
            "jain_consumer_sha256",
            "jeryu_consumer_sha256",
            "lock_sha256",
        ],
        "redline",
    )?;
    expect_bool(redline, "cutover_eligible", true)?;
    let tag = require_string(redline, "engine_tag")?;
    let Some(revision) = tag.strip_prefix("redline-core-v4.1.0-jain.") else {
        return Err("Redline engine tag is not a Jain 4.1.0 immutable tag".into());
    };
    if revision
        .parse::<u64>()
        .ok()
        .filter(|value| *value > 0)
        .is_none()
    {
        return Err("Redline engine tag revision is invalid".into());
    }
    require_hex(redline, "engine_commit", 40)?;
    require_sha256(redline, "family_ci_sha256")?;
    for (redline_key, receipt_key) in [
        ("lock_sha256", "redline_lock_sha256"),
        ("jain_consumer_sha256", "redline_jain_consumer_sha256"),
        ("jeryu_consumer_sha256", "redline_jeryu_consumer_sha256"),
    ] {
        if require_sha256(redline, redline_key)? != require_sha256(receipts, receipt_key)? {
            return Err(format!("Redline {redline_key} differs from release receipts").into());
        }
    }
    Ok(())
}

fn validate_rollback(
    value: &JsonValue,
    receipts: &Map<String, JsonValue>,
) -> Result<(), Box<dyn std::error::Error>> {
    let rollback = exact_object(
        value,
        &[
            "artifact_set_sha256",
            "receipt_sha256",
            "source_freeze_sha256",
            "target_release",
        ],
        "rollback",
    )?;
    expect_string(rollback, "target_release", ROLLBACK)?;
    require_sha256(rollback, "source_freeze_sha256")?;
    require_sha256(rollback, "artifact_set_sha256")?;
    if require_sha256(rollback, "receipt_sha256")? != require_sha256(receipts, "rollback_sha256")? {
        return Err("rollback identity and release receipt differ".into());
    }
    Ok(())
}

fn validate_soak_status(value: &JsonValue) -> Result<(), Box<dyn std::error::Error>> {
    reject_historical_identity(value)?;
    let object = exact_object(
        value,
        &[
            "activation_eligible",
            "duration_seconds",
            "formal_ga",
            "public_routed",
            "reason",
            "receipt_id",
            "release",
            "replacement_receipt_sha256",
            "schema_version",
            "status",
        ],
        "soak status",
    )?;
    expect_string(object, "schema_version", "jain.soak-status/v1")?;
    expect_string(object, "release", RELEASE)?;
    expect_string(object, "receipt_id", "9.0.0-distributed.1/soak-status")?;
    expect_string(object, "status", "not_run")?;
    expect_u64(object, "duration_seconds", 0)?;
    expect_string(object, "reason", "unrouted_preproduction")?;
    require_sha256(object, "replacement_receipt_sha256")?;
    expect_bool(object, "formal_ga", false)?;
    expect_bool(object, "public_routed", false)?;
    expect_bool(object, "activation_eligible", false)?;
    Ok(())
}

fn validate_qualification(value: &JsonValue) -> Result<(), Box<dyn std::error::Error>> {
    reject_historical_identity(value)?;
    let object = exact_object(
        value,
        &[
            "artifact_set_sha256",
            "invalidating_findings",
            "matrix",
            "parity_mismatches",
            "random_seed_sha256",
            "receipt_id",
            "release",
            "schema_version",
            "severity_1_2_defects",
            "signatures",
            "soak_duration_seconds",
            "soak_status",
            "source_freeze_sha256",
            "status",
        ],
        "accelerated qualification",
    )?;
    expect_string(
        object,
        "schema_version",
        "jain.accelerated-qualification/v1",
    )?;
    expect_string(object, "release", RELEASE)?;
    expect_string(
        object,
        "receipt_id",
        "9.0.0-distributed.1/accelerated-qualification",
    )?;
    expect_string(object, "status", "pass")?;
    expect_string(object, "soak_status", "not_run")?;
    expect_u64(object, "soak_duration_seconds", 0)?;
    expect_u64(object, "severity_1_2_defects", 0)?;
    expect_u64(object, "parity_mismatches", 0)?;
    require_sha256(object, "source_freeze_sha256")?;
    require_sha256(object, "artifact_set_sha256")?;
    let expected_seed = qualification_seed(require_string(object, "source_freeze_sha256")?);
    if require_string(object, "random_seed_sha256")? != expected_seed {
        return Err("qualification random seed is not derived from source freeze".into());
    }
    if !require_array(object, "invalidating_findings")?.is_empty() {
        return Err("qualification contains invalidating findings".into());
    }
    let matrix = exact_object(
        object.get("matrix").ok_or("matrix is required")?,
        &[
            "gpu",
            "guest_path",
            "ha",
            "host_resources",
            "jeryu",
            "migration_rollback",
            "offline_appliance",
            "security",
            "storage",
            "ui",
            "workload_coverage",
        ],
        "qualification matrix",
    )?;
    for key in matrix.keys() {
        require_sha256(matrix, key)?;
    }
    validate_signatures(require_array(object, "signatures")?, 1, false)?;
    Ok(())
}

fn validate_cross_bindings(
    release: &JsonValue,
    soak: &JsonValue,
    qualification: &JsonValue,
    soak_bytes: &[u8],
    qualification_bytes: &[u8],
) -> Result<(), Box<dyn std::error::Error>> {
    let release = release.as_object().ok_or("release spec is not an object")?;
    let soak = soak.as_object().ok_or("soak status is not an object")?;
    let qualification = qualification
        .as_object()
        .ok_or("qualification is not an object")?;
    for key in ["source_freeze_sha256", "artifact_set_sha256"] {
        if require_string(release, key)? != require_string(qualification, key)? {
            return Err(format!("release and qualification {key} differ").into());
        }
    }
    let receipts = release
        .get("receipts")
        .and_then(JsonValue::as_object)
        .ok_or("release receipts are missing")?;
    let qualification_sha = sha256_bytes(qualification_bytes);
    let soak_sha = sha256_bytes(soak_bytes);
    if require_string(receipts, "accelerated_qualification_sha256")? != qualification_sha {
        return Err("release does not bind exact accelerated qualification bytes".into());
    }
    if require_string(receipts, "soak_status_sha256")? != soak_sha {
        return Err("release does not bind exact soak status bytes".into());
    }
    if require_string(soak, "replacement_receipt_sha256")? != qualification_sha {
        return Err("no-soak status does not bind the accelerated qualification receipt".into());
    }
    Ok(())
}

fn validate_signatures(
    signatures: &[JsonValue],
    minimum: usize,
    exact: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if signatures.len() < minimum || (exact && signatures.len() != minimum) {
        return Err(format!("expected {} distinct signatures", minimum).into());
    }
    let mut owners = BTreeSet::new();
    let mut keys = BTreeSet::new();
    for (index, signature) in signatures.iter().enumerate() {
        let signature = exact_object(
            signature,
            &["key_fingerprint_sha256", "owner_id", "signature_sha256"],
            &format!("signatures[{index}]"),
        )?;
        let owner = require_string(signature, "owner_id")?;
        let key = require_sha256(signature, "key_fingerprint_sha256")?;
        require_sha256(signature, "signature_sha256")?;
        if owner.is_empty() || !owners.insert(owner.to_owned()) || !keys.insert(key.to_owned()) {
            return Err("signature owners and keys must be distinct".into());
        }
    }
    Ok(())
}

fn build_dag(
    data: &toml::Value,
    manifest_sha256: String,
) -> Result<JsonValue, Box<dyn std::error::Error>> {
    let mut nodes: BTreeMap<String, (i64, BTreeSet<String>)> = BTreeMap::new();
    for key in ["repo", "infrastructure_repo"] {
        for row in data
            .get(key)
            .and_then(toml::Value::as_array)
            .into_iter()
            .flatten()
        {
            let table = row.as_table().ok_or("release DAG row is not a table")?;
            let name = table
                .get("name")
                .and_then(toml::Value::as_str)
                .ok_or("release DAG row has no name")?;
            if !valid_repository_name(name) || nodes.contains_key(name) {
                return Err(
                    format!("release DAG repository is invalid or duplicate: {name}").into(),
                );
            }
            let wave = table
                .get("rollout_wave")
                .and_then(toml::Value::as_integer)
                .ok_or_else(|| format!("release DAG repository has no rollout_wave: {name}"))?;
            let dependencies = toml_string_set(table.get("cross_repo_deps"), name)?;
            nodes.insert(name.to_owned(), (wave, dependencies));
        }
    }
    if nodes.is_empty() {
        return Err("release DAG has no repositories".into());
    }
    for row in data
        .get("infrastructure_repo")
        .and_then(toml::Value::as_array)
        .into_iter()
        .flatten()
    {
        let table = row.as_table().ok_or("infrastructure row is not a table")?;
        let source = table
            .get("name")
            .and_then(toml::Value::as_str)
            .ok_or("infrastructure row has no name")?;
        for consumer in toml_string_set(table.get("dependency_edges"), source)? {
            nodes
                .get_mut(&consumer)
                .ok_or_else(|| {
                    format!("release DAG dependency edge names unknown consumer: {consumer}")
                })?
                .1
                .insert(source.to_owned());
        }
    }
    for (name, (_, dependencies)) in &nodes {
        for dependency in dependencies {
            if dependency == name || !nodes.contains_key(dependency) {
                return Err(format!(
                    "release DAG dependency is self-referential or unknown: {name}->{dependency}"
                )
                .into());
            }
        }
    }

    let mut remaining = nodes
        .iter()
        .map(|(name, (_, dependencies))| (name.clone(), dependencies.clone()))
        .collect::<BTreeMap<_, _>>();
    let mut ordered = Vec::new();
    while !remaining.is_empty() {
        let ready = remaining
            .iter()
            .filter(|(_, dependencies)| dependencies.is_empty())
            .map(|(name, _)| name.clone())
            .min_by_key(|name| (nodes[name].0, name.clone()))
            .ok_or("release DAG contains a dependency cycle")?;
        remaining.remove(&ready);
        for dependencies in remaining.values_mut() {
            dependencies.remove(&ready);
        }
        ordered.push(ready);
    }
    let rows = ordered
        .iter()
        .enumerate()
        .map(|(position, name)| {
            let (wave, dependencies) = &nodes[name];
            json!({
                "position": position,
                "repository": name,
                "declared_wave": wave,
                "depends_on": dependencies.iter().collect::<Vec<_>>(),
            })
        })
        .collect::<Vec<_>>();
    Ok(json!({
        "schema_version": "jain.release-dag/v1",
        "release": RELEASE,
        "manifest_sha256": manifest_sha256,
        "repository_count": rows.len(),
        "repositories": rows,
        "status": "pass",
    }))
}

fn toml_string_set(
    value: Option<&toml::Value>,
    owner: &str,
) -> Result<BTreeSet<String>, Box<dyn std::error::Error>> {
    let mut values = BTreeSet::new();
    for value in value.and_then(toml::Value::as_array).into_iter().flatten() {
        let value = value
            .as_str()
            .ok_or_else(|| format!("{owner} dependency is not a string"))?;
        if !values.insert(value.to_owned()) {
            return Err(format!("{owner} has a duplicate dependency: {value}").into());
        }
    }
    Ok(values)
}

#[derive(Clone)]
struct IndexedFile {
    relative: String,
    size: u64,
    sha256: String,
    device: u64,
    inode: u64,
    receipt_ids: BTreeSet<String>,
}

fn build_evidence_index(
    root: &Path,
    historical_roots: &[PathBuf],
    absent_historical_roots: &[PathBuf],
    output: Option<&Path>,
) -> Result<JsonValue, Box<dyn std::error::Error>> {
    let root = canonical_directory(root, "distributed evidence root")?;
    if output.is_some_and(|output| output.starts_with(&root)) {
        return Err("evidence index output must be outside the indexed root".into());
    }
    let mut present_historical = Vec::new();
    let mut historical_files = Vec::new();
    for historical in historical_roots {
        let historical = canonical_directory(historical, "historical evidence root")?;
        if historical.starts_with(&root)
            || root.starts_with(&historical)
            || present_historical.iter().any(|prior: &PathBuf| {
                historical.starts_with(prior) || prior.starts_with(&historical)
            })
        {
            return Err("distributed and historical evidence roots overlap or repeat".into());
        }
        historical_files.extend(index_tree(&historical, None)?);
        present_historical.push(historical);
    }
    let mut asserted_absent = Vec::new();
    for absent in absent_historical_roots {
        match fs::symlink_metadata(absent) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                asserted_absent.push(absolute_lexical(absent)?);
            }
            Err(error) => return Err(error.into()),
            Ok(_) => {
                return Err(format!(
                    "historical evidence root asserted absent but exists: {}",
                    absent.display()
                )
                .into())
            }
        }
    }
    let files = index_tree(&root, None)?;
    if files.is_empty() {
        return Err("distributed evidence root is empty".into());
    }
    let historical_hashes = historical_files
        .iter()
        .map(|file| file.sha256.as_str())
        .collect::<BTreeSet<_>>();
    let historical_ids = historical_files
        .iter()
        .flat_map(|file| file.receipt_ids.iter())
        .collect::<BTreeSet<_>>();
    let historical_inodes = historical_files
        .iter()
        .map(|file| (file.device, file.inode))
        .collect::<BTreeSet<_>>();
    let mut current_hashes = BTreeSet::new();
    let mut current_inodes = BTreeSet::new();
    let mut current_receipt_ids = BTreeSet::new();
    for file in &files {
        if !current_hashes.insert(file.sha256.as_str()) {
            return Err(format!(
                "distributed evidence contains a duplicate file hash: {}",
                file.relative
            )
            .into());
        }
        if !current_inodes.insert((file.device, file.inode)) {
            return Err(format!(
                "distributed evidence contains an aliased file: {}",
                file.relative
            )
            .into());
        }
        if historical_hashes.contains(file.sha256.as_str()) {
            return Err(format!(
                "distributed evidence reuses historical file hash: {}",
                file.relative
            )
            .into());
        }
        if historical_inodes.contains(&(file.device, file.inode)) {
            return Err(format!(
                "distributed evidence aliases a historical file: {}",
                file.relative
            )
            .into());
        }
        for id in &file.receipt_ids {
            if !current_receipt_ids.insert(id) {
                return Err(format!(
                    "distributed evidence contains a duplicate receipt identifier: {id}"
                )
                .into());
            }
            if historical_ids.contains(id) {
                return Err(format!(
                    "distributed evidence reuses historical receipt identifier: {id}"
                )
                .into());
            }
        }
    }
    let receipt_ids = files
        .iter()
        .flat_map(|file| file.receipt_ids.iter().cloned())
        .collect::<BTreeSet<_>>();
    let rows = files
        .iter()
        .map(|file| {
            json!({
                "path": file.relative,
                "size": file.size,
                "sha256": file.sha256,
                "receipt_ids": file.receipt_ids.iter().collect::<Vec<_>>(),
            })
        })
        .collect::<Vec<_>>();
    Ok(json!({
        "schema_version": "jain.distributed-evidence-index/v1",
        "release": RELEASE,
        "root": root,
        "historical_roots": present_historical,
        "absent_historical_roots": asserted_absent,
        "file_count": rows.len(),
        "files": rows,
        "receipt_identifiers": receipt_ids,
        "status": "pass",
    }))
}

fn canonical_directory(path: &Path, label: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.file_type().is_dir() || metadata.file_type().is_symlink() {
        return Err(format!("{label} is not a physical directory").into());
    }
    Ok(fs::canonicalize(path)?)
}

fn index_tree(
    root: &Path,
    excluded_output: Option<&Path>,
) -> Result<Vec<IndexedFile>, Box<dyn std::error::Error>> {
    fn visit(
        root: &Path,
        directory: &Path,
        excluded_output: Option<&Path>,
        files: &mut Vec<IndexedFile>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut entries = fs::read_dir(directory)?.collect::<Result<Vec<_>, _>>()?;
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let path = entry.path();
            if excluded_output.is_some_and(|output| output == path) {
                continue;
            }
            let metadata = fs::symlink_metadata(&path)?;
            if metadata.file_type().is_symlink() {
                return Err(format!("evidence tree contains a symlink: {}", path.display()).into());
            }
            if metadata.file_type().is_dir() {
                visit(root, &path, excluded_output, files)?;
                continue;
            }
            if !metadata.file_type().is_file()
                || metadata.nlink() != 1
                || metadata.len() > MAX_AUTHORITY_BYTES
            {
                return Err(format!(
                    "evidence is not an independent bounded regular file: {}",
                    path.display()
                )
                .into());
            }
            let bytes = fs::read(&path)?;
            let mut receipt_ids = BTreeSet::new();
            if path.extension().and_then(|extension| extension.to_str()) == Some("json") {
                let value: JsonValue = serde_json::from_slice(&bytes)?;
                collect_receipt_ids(&value, &mut receipt_ids)?;
            }
            files.push(IndexedFile {
                relative: path
                    .strip_prefix(root)?
                    .to_str()
                    .ok_or("evidence path is not UTF-8")?
                    .to_owned(),
                size: metadata.len(),
                sha256: sha256_bytes(&bytes),
                device: metadata.dev(),
                inode: metadata.ino(),
                receipt_ids,
            });
            if files.len() > 100_000 {
                return Err("evidence tree exceeds 100000 files".into());
            }
        }
        Ok(())
    }
    let mut files = Vec::new();
    visit(root, root, excluded_output, &mut files)?;
    Ok(files)
}

fn collect_receipt_ids(
    value: &JsonValue,
    receipt_ids: &mut BTreeSet<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    match value {
        JsonValue::Object(object) => {
            for (key, value) in object {
                if key == "receipt_id" {
                    let id = value.as_str().ok_or("receipt_id is not a string")?;
                    if id.is_empty() || !receipt_ids.insert(id.to_owned()) {
                        return Err(
                            format!("receipt_id is empty or duplicate in one file: {id}").into(),
                        );
                    }
                }
                collect_receipt_ids(value, receipt_ids)?;
            }
        }
        JsonValue::Array(values) => {
            for value in values {
                collect_receipt_ids(value, receipt_ids)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn emit_json(
    value: JsonValue,
    output: Option<&Path>,
    apply: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut bytes = serde_json::to_vec_pretty(&value)?;
    bytes.push(b'\n');
    if let Some(output) = output {
        if !apply {
            return Err("writing JSON output requires --apply".into());
        }
        let parent = output.parent().ok_or("JSON output has no parent")?;
        let parent = fs::canonicalize(parent)?;
        if output.exists() || output.symlink_metadata().is_ok() {
            return Err(format!("refusing to replace JSON output: {}", output.display()).into());
        }
        let name = output.file_name().ok_or("JSON output has no file name")?;
        let exact = parent.join(name);
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o644)
            .open(&exact)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        println!("wrote {}", exact.display());
    } else {
        io::stdout().write_all(&bytes)?;
    }
    Ok(())
}

fn resolve_output_path(path: &Path) -> Result<PathBuf, Box<dyn std::error::Error>> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Ok(env::current_dir()?.join(path))
    }
}

fn absolute_lexical(path: &Path) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        env::current_dir()?.join(path)
    };
    if path
        .components()
        .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Err(format!("path contains parent traversal: {}", path.display()).into());
    }
    Ok(path)
}

fn exact_object<'a>(
    value: &'a JsonValue,
    expected: &[&str],
    label: &str,
) -> Result<&'a Map<String, JsonValue>, Box<dyn std::error::Error>> {
    let object = value
        .as_object()
        .ok_or_else(|| format!("{label} is not an object"))?;
    let actual = object.keys().map(String::as_str).collect::<BTreeSet<_>>();
    let expected = expected.iter().copied().collect::<BTreeSet<_>>();
    if actual != expected {
        return Err(format!(
            "{label} fields are not closed: actual={actual:?} expected={expected:?}"
        )
        .into());
    }
    Ok(object)
}

fn require_string<'a>(
    object: &'a Map<String, JsonValue>,
    key: &str,
) -> Result<&'a str, Box<dyn std::error::Error>> {
    object
        .get(key)
        .and_then(JsonValue::as_str)
        .ok_or_else(|| format!("{key} is not a string").into())
}

fn expect_string(
    object: &Map<String, JsonValue>,
    key: &str,
    expected: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let actual = require_string(object, key)?;
    if actual != expected {
        return Err(format!("{key} must be {expected}, got {actual}").into());
    }
    Ok(())
}

fn expect_bool(
    object: &Map<String, JsonValue>,
    key: &str,
    expected: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let actual = object
        .get(key)
        .and_then(JsonValue::as_bool)
        .ok_or_else(|| format!("{key} is not a boolean"))?;
    if actual != expected {
        return Err(format!("{key} must be {expected}").into());
    }
    Ok(())
}

fn require_u64(
    object: &Map<String, JsonValue>,
    key: &str,
) -> Result<u64, Box<dyn std::error::Error>> {
    object
        .get(key)
        .and_then(JsonValue::as_u64)
        .ok_or_else(|| format!("{key} is not a non-negative integer").into())
}

fn expect_u64(
    object: &Map<String, JsonValue>,
    key: &str,
    expected: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    let actual = require_u64(object, key)?;
    if actual != expected {
        return Err(format!("{key} must be {expected}, got {actual}").into());
    }
    Ok(())
}

fn require_array<'a>(
    object: &'a Map<String, JsonValue>,
    key: &str,
) -> Result<&'a [JsonValue], Box<dyn std::error::Error>> {
    object
        .get(key)
        .and_then(JsonValue::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| format!("{key} is not an array").into())
}

fn require_hex<'a>(
    object: &'a Map<String, JsonValue>,
    key: &str,
    length: usize,
) -> Result<&'a str, Box<dyn std::error::Error>> {
    let value = require_string(object, key)?;
    if value.len() != length
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        || value.bytes().all(|byte| byte == b'0')
    {
        return Err(format!("{key} is not a non-zero lowercase {length}-hex value").into());
    }
    Ok(value)
}

fn require_sha256<'a>(
    object: &'a Map<String, JsonValue>,
    key: &str,
) -> Result<&'a str, Box<dyn std::error::Error>> {
    require_hex(object, key, 64)
}

fn reject_historical_identity(value: &JsonValue) -> Result<(), Box<dyn std::error::Error>> {
    match value {
        JsonValue::String(value)
            if value.contains("9.0.0-alpha.6") || value.contains("9.0.0-appliance.1") =>
        {
            Err("distributed authority reuses a historical release identity".into())
        }
        JsonValue::Array(values) => {
            for value in values {
                reject_historical_identity(value)?;
            }
            Ok(())
        }
        JsonValue::Object(object) => {
            for value in object.values() {
                reject_historical_identity(value)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn valid_repository_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_')
        })
}

fn source_matrix_digest(rows: &[JsonValue]) -> Result<String, Box<dyn std::error::Error>> {
    let mut lines = Vec::new();
    for row in rows {
        let row = row
            .as_object()
            .ok_or("source matrix row is not an object")?;
        lines.push(format!(
            "{}\0{}\0{}\0{}\0{}\n",
            require_string(row, "repository")?,
            require_string(row, "tag")?,
            require_string(row, "commit")?,
            require_string(row, "tree")?,
            require_string(row, "checksum_sha256")?,
        ));
    }
    lines.sort();
    Ok(sha256_bytes(lines.concat().as_bytes()))
}

fn artifact_set_digest(rows: &[JsonValue]) -> Result<String, Box<dyn std::error::Error>> {
    let mut lines = Vec::new();
    for row in rows {
        let row = row.as_object().ok_or("artifact row is not an object")?;
        lines.push(format!(
            "{}\0{}\0{}\0{}\0{}\n",
            require_string(row, "kind")?,
            require_string(row, "name")?,
            require_string(row, "sha256")?,
            require_string(row, "readback_sha256")?,
            require_string(row, "signature_receipt_sha256")?,
        ));
    }
    lines.sort();
    Ok(sha256_bytes(lines.concat().as_bytes()))
}

fn qualification_seed(source_freeze_sha256: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"jain.accelerated-qualification/v1\0");
    hasher.update(source_freeze_sha256.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn sha256_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        os::unix::fs::symlink,
        sync::atomic::{AtomicU64, Ordering},
    };

    static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

    struct TestDir(PathBuf);

    impl TestDir {
        fn new(label: &str) -> Self {
            let sequence = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
            let path = env::temp_dir().join(format!(
                "splitctl-distributed-{label}-{}-{sequence}",
                std::process::id()
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn digest(character: char) -> String {
        std::iter::repeat_n(character, 64).collect()
    }

    fn valid_documents() -> (JsonValue, JsonValue, JsonValue) {
        let source_matrix = vec![json!({
            "repository": "jain-fabric",
            "tag": "jain-fabric-v9.0.0-distributed.1-split.1",
            "commit": "1".repeat(40), "tree": "2".repeat(40),
            "checksum_sha256": digest('3')
        })];
        let artifacts = [
            ("caddy", "jain-caddy", '4'),
            ("hub", "jainhub", '5'),
            ("node", "jainnode", '6'),
            ("worker", "jain-platform", '7'),
            ("appliance_bundle", "jain-appliance.tar.zst", '8'),
        ]
        .into_iter()
        .map(|(kind, name, character)| {
            json!({
                "kind": kind, "name": name,
                "sha256": digest(character), "readback_sha256": digest(character),
                "signature_receipt_sha256": digest('9')
            })
        })
        .collect::<Vec<_>>();
        let source_freeze = source_matrix_digest(&source_matrix).unwrap();
        let artifact_set = artifact_set_digest(&artifacts).unwrap();
        let qualification = json!({
            "schema_version": "jain.accelerated-qualification/v1",
            "release": RELEASE,
            "receipt_id": "9.0.0-distributed.1/accelerated-qualification",
            "source_freeze_sha256": source_freeze,
            "artifact_set_sha256": artifact_set,
            "random_seed_sha256": qualification_seed(&source_freeze),
            "status": "pass",
            "soak_status": "not_run",
            "soak_duration_seconds": 0,
            "severity_1_2_defects": 0,
            "parity_mismatches": 0,
            "invalidating_findings": [],
            "matrix": {
                "offline_appliance": digest('1'), "guest_path": digest('2'),
                "workload_coverage": digest('3'), "jeryu": digest('4'),
                "gpu": digest('5'), "storage": digest('6'), "ha": digest('7'),
                "security": digest('8'), "migration_rollback": digest('9'),
                "ui": digest('c'), "host_resources": digest('d')
            },
            "signatures": [{
                "owner_id": "qualifier-1",
                "key_fingerprint_sha256": digest('e'),
                "signature_sha256": digest('f')
            }]
        });
        let qualification_bytes = serde_json::to_vec_pretty(&qualification).unwrap();
        let qualification_sha = sha256_bytes(&qualification_bytes);
        let soak = json!({
            "schema_version": "jain.soak-status/v1",
            "release": RELEASE,
            "receipt_id": "9.0.0-distributed.1/soak-status",
            "status": "not_run",
            "duration_seconds": 0,
            "reason": "unrouted_preproduction",
            "replacement_receipt_sha256": qualification_sha,
            "formal_ga": false,
            "public_routed": false,
            "activation_eligible": false
        });
        let soak_bytes = serde_json::to_vec_pretty(&soak).unwrap();
        let hosts = [
            (
                "AtomicSoul",
                "edge-storage",
                2_199_023_255_552_u64,
                vec![18080_u64],
            ),
            (
                "xbabe2",
                "follower-compute",
                805_306_368_000_u64,
                vec![7445_u64],
            ),
            (
                "xbabe3",
                "primary-compute",
                85_899_345_920_u64,
                vec![7443_u64, 7444_u64, 7445_u64],
            ),
        ]
        .into_iter()
        .map(|(host_id, role, quota, ports)| {
            json!({
                "host_id": host_id, "role": role, "storage_quota_bytes": quota,
                "epoch": 1, "image_set_sha256": digest('a'), "ports": ports,
                "external_caddy_unchanged": true
            })
        })
        .collect::<Vec<_>>();
        let release = json!({
            "schema_version": "jain.distributed-release/v1",
            "release": RELEASE,
            "release_id": "9.0.0-distributed.1/release-authority",
            "status": "candidate",
            "formal_ga": false,
            "public_routed": false,
            "activation_eligible": false,
            "rollback_target": ROLLBACK,
            "source_freeze_sha256": source_freeze,
            "artifact_set_sha256": artifact_set,
            "source_matrix": source_matrix,
            "artifacts": artifacts,
            "hosts": hosts,
            "routes": {
                "canary_listen": "127.0.0.1:18080",
                "standalone_listen": "127.0.0.1:8080",
                "dns_unchanged": true,
                "public_admission_unchanged": true,
                "external_caddy_etag_before": "fixture-etag",
                "external_caddy_etag_after": "fixture-etag",
                "external_caddy_route_sha256_before": digest('c'),
                "external_caddy_route_sha256_after": digest('c')
            },
            "redline": {
                "engine_tag": "redline-core-v4.1.0-jain.4",
                "engine_commit": "4".repeat(40),
                "family_ci_sha256": digest('f'),
                "lock_sha256": digest('1'),
                "jain_consumer_sha256": digest('2'),
                "jeryu_consumer_sha256": digest('3'),
                "cutover_eligible": true
            },
            "rollback": {
                "target_release": ROLLBACK,
                "source_freeze_sha256": digest('d'),
                "artifact_set_sha256": digest('e'),
                "receipt_sha256": digest('5')
            },
            "receipts": {
                "accelerated_qualification_sha256": qualification_sha,
                "soak_status_sha256": sha256_bytes(&soak_bytes),
                "redline_lock_sha256": digest('1'),
                "redline_jain_consumer_sha256": digest('2'),
                "redline_jeryu_consumer_sha256": digest('3'),
                "migration_sha256": digest('4'), "rollback_sha256": digest('5'),
                "gpu_sha256": digest('6'), "jeryu_parity_sha256": digest('7'),
                "storage_sha256": digest('8'), "caddy_unchanged_sha256": digest('9')
            },
            "signatures": [
                {"owner_id":"owner-1","key_fingerprint_sha256":digest('1'),"signature_sha256":digest('2')},
                {"owner_id":"owner-2","key_fingerprint_sha256":digest('3'),"signature_sha256":digest('4')}
            ]
        });
        (release, soak, qualification)
    }

    #[test]
    fn distributed_documents_are_closed_and_cross_bound() {
        let (release, soak, qualification) = valid_documents();
        let soak_bytes = serde_json::to_vec_pretty(&soak).unwrap();
        let qualification_bytes = serde_json::to_vec_pretty(&qualification).unwrap();
        validate_release_spec(&release).unwrap();
        validate_soak_status(&soak).unwrap();
        validate_qualification(&qualification).unwrap();
        validate_cross_bindings(
            &release,
            &soak,
            &qualification,
            &soak_bytes,
            &qualification_bytes,
        )
        .unwrap();

        for (name, mutation) in [
            ("ga", ("formal_ga", json!(true))),
            ("route", ("public_routed", json!(true))),
            ("activation", ("activation_eligible", json!(true))),
        ] {
            let mut hostile = release.clone();
            hostile[mutation.0] = mutation.1;
            assert!(validate_release_spec(&hostile).is_err(), "{name}");
        }
        let mut stale = release.clone();
        stale["source_matrix"][0]["tag"] = json!("jain-fabric-v9.0.0-appliance.1-split.1");
        assert!(validate_release_spec(&stale).is_err());
        let mut unbound_source = release.clone();
        unbound_source["source_matrix"][0]["checksum_sha256"] = json!(digest('4'));
        assert!(validate_release_spec(&unbound_source).is_err());
        let mut changed_route = release.clone();
        changed_route["routes"]["external_caddy_etag_after"] = json!("changed");
        assert!(validate_release_spec(&changed_route).is_err());
        let mut split_epoch = release.clone();
        split_epoch["hosts"][0]["epoch"] = json!(2);
        assert!(validate_release_spec(&split_epoch).is_err());
        let mut mismatched_redline = release.clone();
        mismatched_redline["redline"]["lock_sha256"] = json!(digest('a'));
        assert!(validate_release_spec(&mismatched_redline).is_err());
        let mut duplicate_owner = release.clone();
        duplicate_owner["signatures"][1]["owner_id"] = json!("owner-1");
        assert!(validate_release_spec(&duplicate_owner).is_err());
        let mut extra = qualification.clone();
        extra["soak_passed"] = json!(true);
        assert!(validate_qualification(&extra).is_err());
        let mut wrong_seed = qualification.clone();
        wrong_seed["random_seed_sha256"] = json!(digest('9'));
        assert!(validate_qualification(&wrong_seed).is_err());
    }

    #[test]
    fn distributed_validate_command_reads_exact_independent_files() {
        let temp = TestDir::new("validate-command");
        let (release, soak, qualification) = valid_documents();
        let release_path = temp.0.join("release.json");
        let soak_path = temp.0.join("soak.json");
        let qualification_path = temp.0.join("qualification.json");
        fs::write(&release_path, serde_json::to_vec_pretty(&release).unwrap()).unwrap();
        fs::write(&soak_path, serde_json::to_vec_pretty(&soak).unwrap()).unwrap();
        fs::write(
            &qualification_path,
            serde_json::to_vec_pretty(&qualification).unwrap(),
        )
        .unwrap();
        validate_command(vec![
            "--release-spec".to_owned(),
            release_path.display().to_string(),
            "--soak-status".to_owned(),
            soak_path.display().to_string(),
            "--qualification".to_owned(),
            qualification_path.display().to_string(),
        ])
        .unwrap();

        let linked = temp.0.join("linked-soak.json");
        symlink(&soak_path, &linked).unwrap();
        assert!(validate_command(vec![
            "--release-spec".to_owned(),
            release_path.display().to_string(),
            "--soak-status".to_owned(),
            linked.display().to_string(),
            "--qualification".to_owned(),
            qualification_path.display().to_string(),
        ])
        .is_err());
    }

    #[test]
    fn release_dag_is_deterministic_and_rejects_cycles() {
        let manifest: toml::Value = r#"
            [[repo]]
            name = "leaf"
            rollout_wave = 3
            cross_repo_deps = ["middle"]
            [[repo]]
            name = "root"
            rollout_wave = 1
            cross_repo_deps = []
            [[repo]]
            name = "middle"
            rollout_wave = 2
            cross_repo_deps = ["root"]
            [[infrastructure_repo]]
            name = "fabric"
            rollout_wave = 0
            cross_repo_deps = []
            dependency_edges = ["leaf"]
        "#
        .parse()
        .unwrap();
        let report = build_dag(&manifest, digest('a')).unwrap();
        let names = report["repositories"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| row["repository"].as_str().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(names, ["fabric", "root", "middle", "leaf"]);
        let cyclic: toml::Value = r#"
            [[repo]]
            name="a"
            rollout_wave=1
            cross_repo_deps=["b"]
            [[repo]]
            name="b"
            rollout_wave=1
            cross_repo_deps=["a"]
        "#
        .parse()
        .unwrap();
        assert!(build_dag(&cyclic, digest('b')).is_err());
    }

    #[test]
    fn evidence_index_rejects_hash_receipt_symlink_and_absence_reuse() {
        let temp = TestDir::new("evidence");
        let current = temp.0.join("current");
        let historical = temp.0.join("historical");
        let absent = temp.0.join("absent");
        fs::create_dir(&current).unwrap();
        fs::create_dir(&historical).unwrap();
        fs::write(
            current.join("current.json"),
            br#"{"receipt_id":"new/receipt"}"#,
        )
        .unwrap();
        fs::write(
            historical.join("old.json"),
            br#"{"receipt_id":"old/receipt"}"#,
        )
        .unwrap();
        build_evidence_index(
            &current,
            std::slice::from_ref(&historical),
            std::slice::from_ref(&absent),
            None,
        )
        .unwrap();

        fs::write(
            historical.join("same.bin"),
            br#"{"receipt_id":"new/receipt"}"#,
        )
        .unwrap();
        assert!(build_evidence_index(
            &current,
            std::slice::from_ref(&historical),
            std::slice::from_ref(&absent),
            None
        )
        .is_err());
        fs::remove_file(historical.join("same.bin")).unwrap();
        fs::write(
            historical.join("same.bin"),
            fs::read(current.join("current.json")).unwrap(),
        )
        .unwrap();
        assert!(build_evidence_index(
            &current,
            std::slice::from_ref(&historical),
            std::slice::from_ref(&absent),
            None
        )
        .is_err());
        fs::remove_file(historical.join("same.bin")).unwrap();
        symlink(historical.join("old.json"), current.join("link")).unwrap();
        assert!(build_evidence_index(
            &current,
            std::slice::from_ref(&historical),
            std::slice::from_ref(&absent),
            None
        )
        .is_err());
        fs::remove_file(current.join("link")).unwrap();
        fs::create_dir(&absent).unwrap();
        assert!(build_evidence_index(
            &current,
            std::slice::from_ref(&historical),
            std::slice::from_ref(&absent),
            None
        )
        .is_err());
    }

    #[test]
    fn distributed_json_schemas_are_closed_and_parseable() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("schemas");
        for (name, schema_version) in [
            (
                "distributed-release.v1.schema.json",
                "jain.distributed-release/v1",
            ),
            ("soak-status.v1.schema.json", "jain.soak-status/v1"),
            (
                "accelerated-qualification.v1.schema.json",
                "jain.accelerated-qualification/v1",
            ),
        ] {
            let value: JsonValue =
                serde_json::from_slice(&fs::read(root.join(name)).unwrap()).unwrap();
            assert_eq!(value["type"], "object");
            assert_eq!(value["additionalProperties"], false);
            assert_eq!(
                value["properties"]["schema_version"]["const"],
                schema_version
            );
            assert_eq!(value["properties"]["release"]["const"], RELEASE);
        }
    }
}
