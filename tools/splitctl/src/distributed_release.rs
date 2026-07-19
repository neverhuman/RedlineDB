use serde_json::{json, Map, Value as JsonValue};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    env,
    ffi::{CString, OsStr, OsString},
    fs::{self, File},
    io::{self, Read, Write},
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::{ffi::OsStrExt, fs::MetadataExt},
    },
    path::{Component, Path, PathBuf},
};

const RELEASE: &str = "9.0.0-distributed.1";
const ROLLBACK: &str = "7.0.6";
const MAX_AUTHORITY_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FileIdentity {
    device: u64,
    inode: u64,
    mode: u32,
    links: u64,
    length: u64,
    uid: u32,
    gid: u32,
    modified_seconds: i64,
    modified_nanoseconds: i64,
    changed_seconds: i64,
    changed_nanoseconds: i64,
}

impl FileIdentity {
    fn from_metadata(metadata: &fs::Metadata) -> Self {
        Self {
            device: metadata.dev(),
            inode: metadata.ino(),
            mode: metadata.mode(),
            links: metadata.nlink(),
            length: metadata.len(),
            uid: metadata.uid(),
            gid: metadata.gid(),
            modified_seconds: metadata.mtime(),
            modified_nanoseconds: metadata.mtime_nsec(),
            changed_seconds: metadata.ctime(),
            changed_nanoseconds: metadata.ctime_nsec(),
        }
    }
}

struct OpenedPhysical {
    file: File,
    absolute: PathBuf,
    identity: FileIdentity,
    component_ids: Vec<(u64, u64)>,
}

struct PreparedOutput {
    parent: OpenedPhysical,
    name: OsString,
    absolute: PathBuf,
}

#[derive(Clone)]
struct ProofDocuments {
    dag_bytes: Vec<u8>,
    dag: JsonValue,
    routes_before_bytes: Vec<u8>,
    routes_after_bytes: Vec<u8>,
    caddy_proof_bytes: Vec<u8>,
    caddy_proof: JsonValue,
    family_ci_bytes: Vec<u8>,
    family_ci: JsonValue,
    redline_lock_bytes: Vec<u8>,
    redline_lock_mirror_bytes: Vec<u8>,
    jain_consumer_bytes: Vec<u8>,
    jain_consumer: JsonValue,
    jeryu_consumer_bytes: Vec<u8>,
    jeryu_consumer: JsonValue,
    rollback_bytes: Vec<u8>,
    rollback: JsonValue,
    signature_receipts: Vec<(Vec<u8>, JsonValue)>,
}

pub(crate) fn validate_command(args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    let mut release_spec = None;
    let mut soak_status = None;
    let mut qualification = None;
    let mut release_dag = None;
    let mut caddy_routes_before = None;
    let mut caddy_routes_after = None;
    let mut caddy_proof = None;
    let mut redline_family_ci = None;
    let mut redline_lock = None;
    let mut redline_lock_mirror = None;
    let mut redline_jain_consumer = None;
    let mut redline_jeryu_consumer = None;
    let mut rollback_proof = None;
    let mut signature_receipts = Vec::new();
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
            "--release-dag" => {
                release_dag = Some(PathBuf::from(
                    iter.next().ok_or("--release-dag needs a path")?,
                ))
            }
            "--caddy-routes-before" => {
                caddy_routes_before = Some(PathBuf::from(
                    iter.next().ok_or("--caddy-routes-before needs a path")?,
                ))
            }
            "--caddy-routes-after" => {
                caddy_routes_after = Some(PathBuf::from(
                    iter.next().ok_or("--caddy-routes-after needs a path")?,
                ))
            }
            "--caddy-proof" => {
                caddy_proof = Some(PathBuf::from(
                    iter.next().ok_or("--caddy-proof needs a path")?,
                ))
            }
            "--redline-family-ci" => {
                redline_family_ci = Some(PathBuf::from(
                    iter.next().ok_or("--redline-family-ci needs a path")?,
                ))
            }
            "--redline-lock" => {
                redline_lock = Some(PathBuf::from(
                    iter.next().ok_or("--redline-lock needs a path")?,
                ))
            }
            "--redline-lock-mirror" => {
                redline_lock_mirror = Some(PathBuf::from(
                    iter.next().ok_or("--redline-lock-mirror needs a path")?,
                ))
            }
            "--redline-jain-consumer" => {
                redline_jain_consumer = Some(PathBuf::from(
                    iter.next().ok_or("--redline-jain-consumer needs a path")?,
                ))
            }
            "--redline-jeryu-consumer" => {
                redline_jeryu_consumer = Some(PathBuf::from(
                    iter.next().ok_or("--redline-jeryu-consumer needs a path")?,
                ))
            }
            "--rollback-proof" => {
                rollback_proof = Some(PathBuf::from(
                    iter.next().ok_or("--rollback-proof needs a path")?,
                ))
            }
            "--signature-receipt" => signature_receipts.push(PathBuf::from(
                iter.next().ok_or("--signature-receipt needs a path")?,
            )),
            value => return Err(format!("unknown distributed-validate argument: {value}").into()),
        }
    }
    let release_spec = release_spec.ok_or("--release-spec is required")?;
    let soak_status = soak_status.ok_or("--soak-status is required")?;
    let qualification = qualification.ok_or("--qualification is required")?;
    let release_dag = release_dag.ok_or("--release-dag is required")?;
    let caddy_routes_before = caddy_routes_before.ok_or("--caddy-routes-before is required")?;
    let caddy_routes_after = caddy_routes_after.ok_or("--caddy-routes-after is required")?;
    let caddy_proof = caddy_proof.ok_or("--caddy-proof is required")?;
    let redline_family_ci = redline_family_ci.ok_or("--redline-family-ci is required")?;
    let redline_lock = redline_lock.ok_or("--redline-lock is required")?;
    let redline_lock_mirror = redline_lock_mirror.ok_or("--redline-lock-mirror is required")?;
    let redline_jain_consumer =
        redline_jain_consumer.ok_or("--redline-jain-consumer is required")?;
    let redline_jeryu_consumer =
        redline_jeryu_consumer.ok_or("--redline-jeryu-consumer is required")?;
    let rollback_proof = rollback_proof.ok_or("--rollback-proof is required")?;
    if signature_receipts.len() != 2 {
        return Err("exactly two --signature-receipt paths are required".into());
    }
    let (release_bytes, release_value) = read_authority_json(&release_spec, "release spec")?;
    let (soak_bytes, soak_value) = read_authority_json(&soak_status, "soak status")?;
    let (qualification_bytes, qualification_value) =
        read_authority_json(&qualification, "qualification")?;
    let (dag_bytes, dag_value) = read_authority_json(&release_dag, "release DAG")?;
    let routes_before_bytes = read_regular_bytes(&caddy_routes_before, "Caddy routes before")?;
    let routes_after_bytes = read_regular_bytes(&caddy_routes_after, "Caddy routes after")?;
    let (caddy_proof_bytes, caddy_proof_value) =
        read_authority_json(&caddy_proof, "Caddy unchanged proof")?;
    let (family_ci_bytes, family_ci_value) =
        read_authority_json(&redline_family_ci, "Redline family CI")?;
    let redline_lock_bytes = read_regular_bytes(&redline_lock, "Redline authoritative lock")?;
    let redline_lock_mirror_bytes =
        read_regular_bytes(&redline_lock_mirror, "Redline compatibility lock")?;
    let (jain_consumer_bytes, jain_consumer_value) =
        read_authority_json(&redline_jain_consumer, "Redline Jain consumer evidence")?;
    let (jeryu_consumer_bytes, jeryu_consumer_value) =
        read_authority_json(&redline_jeryu_consumer, "Redline Jeryu consumer evidence")?;
    let (rollback_bytes, rollback_value) = read_authority_json(&rollback_proof, "rollback proof")?;
    let mut signature_documents = Vec::new();
    for path in &signature_receipts {
        signature_documents.push(read_authority_json(path, "owner signature receipt")?);
    }

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
    let proofs = ProofDocuments {
        dag_bytes,
        dag: dag_value,
        routes_before_bytes,
        routes_after_bytes,
        caddy_proof_bytes,
        caddy_proof: caddy_proof_value,
        family_ci_bytes,
        family_ci: family_ci_value,
        redline_lock_bytes,
        redline_lock_mirror_bytes,
        jain_consumer_bytes,
        jain_consumer: jain_consumer_value,
        jeryu_consumer_bytes,
        jeryu_consumer: jeryu_consumer_value,
        rollback_bytes,
        rollback: rollback_value,
        signature_receipts: signature_documents,
    };
    validate_proof_bindings(&release_value, &proofs)?;
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
    let mut locked_cargo_graph = None;
    let mut release = None;
    let mut output = None;
    let mut apply = false;
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--manifest" => {
                manifest = Some(PathBuf::from(iter.next().ok_or("--manifest needs a path")?))
            }
            "--locked-cargo-graph" => {
                locked_cargo_graph = Some(PathBuf::from(
                    iter.next().ok_or("--locked-cargo-graph needs a path")?,
                ))
            }
            "--release" => release = Some(iter.next().ok_or("--release needs a value")?),
            "--json" => output = Some(PathBuf::from(iter.next().ok_or("--json needs a path")?)),
            "--apply" => apply = true,
            value => return Err(format!("unknown release-dag argument: {value}").into()),
        }
    }
    let manifest = manifest.ok_or("--manifest is required")?;
    let locked_cargo_graph = locked_cargo_graph.ok_or("--locked-cargo-graph is required")?;
    if release.as_deref() != Some(RELEASE) {
        return Err(format!("--release must be {RELEASE}").into());
    }
    let bytes = read_regular_bytes(&manifest, "release DAG manifest")?;
    let data: toml::Value = std::str::from_utf8(&bytes)?.parse()?;
    let (locked_graph_bytes, locked_graph) =
        read_authority_json(&locked_cargo_graph, "locked Cargo graph")?;
    let prepared_output = output.as_deref().map(prepare_output).transpose()?;
    let report = build_dag(
        &data,
        sha256_bytes(&bytes),
        &locked_graph,
        sha256_bytes(&locked_graph_bytes),
    )?;
    emit_json(report, prepared_output.as_ref(), apply)
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
    let prepared_output = output.as_deref().map(prepare_output).transpose()?;
    let report = build_evidence_index(
        &root,
        &historical_roots,
        &absent_historical_roots,
        prepared_output.as_ref(),
    )?;
    emit_json(report, prepared_output.as_ref(), apply)
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
    read_regular_bytes_with_hook(path, label, || {})
}

fn read_regular_bytes_with_hook<F>(
    path: &Path,
    label: &str,
    hook: F,
) -> Result<Vec<u8>, Box<dyn std::error::Error>>
where
    F: FnOnce(),
{
    let opened = open_physical(path, libc::O_RDONLY, label)?;
    let metadata = opened.file.metadata()?;
    if !metadata.file_type().is_file()
        || opened.identity.links != 1
        || opened.identity.length == 0
        || opened.identity.length > MAX_AUTHORITY_BYTES
    {
        return Err(format!("{label} is not a bounded independent regular file").into());
    }
    hook();
    let mut bytes = Vec::with_capacity(opened.identity.length as usize);
    (&opened.file)
        .take(MAX_AUTHORITY_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 != opened.identity.length
        || FileIdentity::from_metadata(&opened.file.metadata()?) != opened.identity
    {
        return Err(format!("{label} changed while reading").into());
    }
    ensure_path_identity(&opened, libc::O_RDONLY, label)?;
    Ok(bytes)
}

fn validate_release_spec(value: &JsonValue) -> Result<(), Box<dyn std::error::Error>> {
    reject_historical_identity(value)?;
    let object = exact_object(
        value,
        &[
            "activation_eligible",
            "artifact_set_sha256",
            "artifacts",
            "canonical_payload_sha256",
            "formal_ga",
            "hosts",
            "public_routed",
            "receipts",
            "redline",
            "release",
            "release_dag_sha256",
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
    require_sha256(object, "release_dag_sha256")?;
    require_sha256(object, "canonical_payload_sha256")?;

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
        if !valid_release_tag(repository, tag) {
            return Err(
                format!("source_matrix tag does not bind repository {repository}: {tag}").into(),
            );
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
    for required in [
        "caddy",
        "hub",
        "node",
        "worker",
        "pack",
        "compose",
        "installer",
        "appliance_bundle",
    ] {
        if !kinds.contains(required) {
            return Err(
                format!("required distributed artifact kind is missing: {required}").into(),
            );
        }
    }

    validate_hosts(
        require_array(object, "hosts")?,
        &deployed_image_set_digest(artifacts)?,
    )?;
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
    validate_release_signatures(require_array(object, "signatures")?)?;
    let expected_payload = canonical_release_payload_digest(value)?;
    if require_string(object, "canonical_payload_sha256")? != expected_payload {
        return Err("canonical_payload_sha256 does not bind the unsigned release payload".into());
    }
    for signature in require_array(object, "signatures")? {
        let signature = signature
            .as_object()
            .ok_or("release signature row is not an object")?;
        if require_sha256(signature, "payload_sha256")? != expected_payload {
            return Err("release signature row signs the wrong canonical payload".into());
        }
    }
    Ok(())
}

fn validate_hosts(
    hosts: &[JsonValue],
    expected_image_set_sha256: &str,
) -> Result<(), Box<dyn std::error::Error>> {
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
        if require_sha256(host, "image_set_sha256")? != expected_image_set_sha256 {
            return Err(format!(
                "host {id} image_set_sha256 does not bind canonical registry readback images"
            )
            .into());
        }
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

fn validate_proof_bindings(
    release: &JsonValue,
    proofs: &ProofDocuments,
) -> Result<(), Box<dyn std::error::Error>> {
    validate_release_dag_binding(release, &proofs.dag_bytes, &proofs.dag)?;
    validate_caddy_proof(release, proofs)?;
    validate_redline_proofs(release, proofs)?;
    validate_rollback_proof(release, &proofs.rollback_bytes, &proofs.rollback)?;
    validate_owner_signature_receipts(release, &proofs.signature_receipts)?;
    Ok(())
}

fn validate_release_dag_binding(
    release: &JsonValue,
    dag_bytes: &[u8],
    dag: &JsonValue,
) -> Result<(), Box<dyn std::error::Error>> {
    let release = release.as_object().ok_or("release spec is not an object")?;
    let dag = exact_object(
        dag,
        &[
            "locked_cargo_graph_sha256",
            "manifest_sha256",
            "release",
            "repositories",
            "repository_count",
            "rollout_wave_order",
            "schema_version",
            "status",
        ],
        "release DAG",
    )?;
    expect_string(dag, "schema_version", "jain.release-dag/v1")?;
    expect_string(dag, "release", RELEASE)?;
    expect_string(dag, "status", "pass")?;
    require_sha256(dag, "manifest_sha256")?;
    require_sha256(dag, "locked_cargo_graph_sha256")?;
    if require_sha256(release, "release_dag_sha256")? != sha256_bytes(dag_bytes) {
        return Err("release_dag_sha256 does not bind exact DAG bytes".into());
    }
    let waves = require_array(dag, "rollout_wave_order")?;
    let wave_values = waves
        .iter()
        .map(JsonValue::as_i64)
        .collect::<Option<Vec<_>>>()
        .ok_or("release DAG rollout wave is not an integer")?;
    if wave_values.is_empty() || wave_values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err("release DAG rollout_wave_order is invalid".into());
    }
    let rows = require_array(dag, "repositories")?;
    if require_u64(dag, "repository_count")? != rows.len() as u64 || rows.is_empty() {
        return Err("release DAG repository_count is invalid".into());
    }
    let mut dag_repositories = BTreeSet::new();
    let mut dependencies_by_repository = BTreeMap::new();
    let mut dag_sources = BTreeMap::new();
    for (position, row) in rows.iter().enumerate() {
        let row = exact_object(
            row,
            &[
                "cargo_lock_sha256",
                "checksum_sha256",
                "commit",
                "declared_wave",
                "depends_on",
                "position",
                "repository",
                "tag",
                "tree",
            ],
            &format!("release DAG repositories[{position}]"),
        )?;
        expect_u64(row, "position", position as u64)?;
        require_u64(row, "declared_wave")?;
        let repository = require_string(row, "repository")?;
        if !valid_repository_name(repository) || !dag_repositories.insert(repository.to_owned()) {
            return Err(
                format!("release DAG repository is invalid or duplicate: {repository}").into(),
            );
        }
        match row.get("cargo_lock_sha256") {
            Some(JsonValue::Null) => {}
            Some(JsonValue::String(value)) if valid_nonzero_hex(value, 64) => {}
            _ => return Err(format!("release DAG Cargo lock is invalid: {repository}").into()),
        }
        let tag = require_string(row, "tag")?;
        if !valid_release_tag(repository, tag) {
            return Err(format!("release DAG tag does not bind {repository}").into());
        }
        let commit = require_hex(row, "commit", 40)?;
        let tree = require_hex(row, "tree", 40)?;
        let checksum = require_sha256(row, "checksum_sha256")?;
        dag_sources.insert(
            repository.to_owned(),
            (
                tag.to_owned(),
                commit.to_owned(),
                tree.to_owned(),
                checksum.to_owned(),
            ),
        );
        let dependencies = require_array(row, "depends_on")?
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .filter(|value| valid_repository_name(value))
                    .map(str::to_owned)
                    .ok_or("release DAG dependency is invalid")
            })
            .collect::<Result<BTreeSet<_>, _>>()?;
        if dependencies.len() != require_array(row, "depends_on")?.len() {
            return Err(format!("release DAG has duplicate dependencies: {repository}").into());
        }
        dependencies_by_repository.insert(repository.to_owned(), dependencies);
    }
    for (repository, dependencies) in &dependencies_by_repository {
        if dependencies
            .iter()
            .any(|dependency| dependency == repository || !dag_repositories.contains(dependency))
        {
            return Err(
                format!("release DAG has an unknown or self dependency: {repository}").into(),
            );
        }
    }
    let source_rows = require_array(release, "source_matrix")?;
    let source_repositories = source_rows
        .iter()
        .map(|row| {
            row.as_object()
                .and_then(|row| row.get("repository"))
                .and_then(JsonValue::as_str)
                .map(str::to_owned)
                .ok_or("source matrix repository is invalid")
        })
        .collect::<Result<BTreeSet<_>, _>>()?;
    if source_repositories != dag_repositories {
        return Err("source_matrix is not the complete DAG repository inventory".into());
    }
    let release_sources = source_rows
        .iter()
        .map(|row| {
            let row = row
                .as_object()
                .ok_or("source matrix row is not an object")?;
            Ok((
                require_string(row, "repository")?.to_owned(),
                (
                    require_string(row, "tag")?.to_owned(),
                    require_string(row, "commit")?.to_owned(),
                    require_string(row, "tree")?.to_owned(),
                    require_string(row, "checksum_sha256")?.to_owned(),
                ),
            ))
        })
        .collect::<Result<BTreeMap<_, _>, Box<dyn std::error::Error>>>()?;
    if release_sources != dag_sources {
        return Err("source_matrix identities differ from authority-derived DAG rows".into());
    }
    Ok(())
}

fn validate_caddy_proof(
    release: &JsonValue,
    proofs: &ProofDocuments,
) -> Result<(), Box<dyn std::error::Error>> {
    let release = release.as_object().ok_or("release spec is not an object")?;
    let routes = release
        .get("routes")
        .and_then(JsonValue::as_object)
        .ok_or("release routes are missing")?;
    let receipts = release
        .get("receipts")
        .and_then(JsonValue::as_object)
        .ok_or("release receipts are missing")?;
    let proof = exact_object(
        &proofs.caddy_proof,
        &[
            "dns_unchanged",
            "etag_after",
            "etag_before",
            "public_admission_unchanged",
            "receipt_id",
            "release",
            "route_array_sha256_after",
            "route_array_sha256_before",
            "schema_version",
            "status",
        ],
        "Caddy unchanged proof",
    )?;
    expect_string(proof, "schema_version", "jain.caddy-unchanged/v1")?;
    expect_string(proof, "release", RELEASE)?;
    expect_string(proof, "receipt_id", "9.0.0-distributed.1/caddy-unchanged")?;
    expect_string(proof, "status", "pass")?;
    expect_bool(proof, "dns_unchanged", true)?;
    expect_bool(proof, "public_admission_unchanged", true)?;
    let before_sha = sha256_bytes(&proofs.routes_before_bytes);
    let after_sha = sha256_bytes(&proofs.routes_after_bytes);
    if proofs.routes_before_bytes != proofs.routes_after_bytes
        || require_sha256(proof, "route_array_sha256_before")? != before_sha
        || require_sha256(proof, "route_array_sha256_after")? != after_sha
        || require_sha256(routes, "external_caddy_route_sha256_before")? != before_sha
        || require_sha256(routes, "external_caddy_route_sha256_after")? != after_sha
    {
        return Err("Caddy proof does not bind identical exact route-array bytes".into());
    }
    for (route_key, proof_key) in [
        ("external_caddy_etag_before", "etag_before"),
        ("external_caddy_etag_after", "etag_after"),
    ] {
        if require_string(routes, route_key)? != require_string(proof, proof_key)? {
            return Err("Caddy proof ETag differs from release routes".into());
        }
    }
    if require_sha256(receipts, "caddy_unchanged_sha256")?
        != sha256_bytes(&proofs.caddy_proof_bytes)
    {
        return Err("release does not bind exact Caddy proof bytes".into());
    }
    Ok(())
}

fn validate_redline_proofs(
    release: &JsonValue,
    proofs: &ProofDocuments,
) -> Result<(), Box<dyn std::error::Error>> {
    let release = release.as_object().ok_or("release spec is not an object")?;
    let redline = release
        .get("redline")
        .and_then(JsonValue::as_object)
        .ok_or("release Redline identity is missing")?;
    let receipts = release
        .get("receipts")
        .and_then(JsonValue::as_object)
        .ok_or("release receipts are missing")?;
    let engine_tag = require_string(redline, "engine_tag")?;
    let engine_commit = require_string(redline, "engine_commit")?;
    let proof_lock_id = format!("redline-proof/v2/4.1.0/{engine_commit}");
    let family_sha = sha256_bytes(&proofs.family_ci_bytes);
    let lock_sha = sha256_bytes(&proofs.redline_lock_bytes);
    let jain_sha = sha256_bytes(&proofs.jain_consumer_bytes);
    let jeryu_sha = sha256_bytes(&proofs.jeryu_consumer_bytes);
    if proofs.redline_lock_bytes != proofs.redline_lock_mirror_bytes {
        return Err("Redline authoritative and compatibility locks differ byte-for-byte".into());
    }
    for (object, key, expected) in [
        (redline, "family_ci_sha256", family_sha.as_str()),
        (redline, "lock_sha256", lock_sha.as_str()),
        (redline, "jain_consumer_sha256", jain_sha.as_str()),
        (redline, "jeryu_consumer_sha256", jeryu_sha.as_str()),
        (receipts, "redline_lock_sha256", lock_sha.as_str()),
        (receipts, "redline_jain_consumer_sha256", jain_sha.as_str()),
        (
            receipts,
            "redline_jeryu_consumer_sha256",
            jeryu_sha.as_str(),
        ),
    ] {
        if require_sha256(object, key)? != expected {
            return Err(format!("Redline exact proof bytes do not bind {key}").into());
        }
    }

    let family = proofs
        .family_ci
        .as_object()
        .ok_or("Redline family CI is not an object")?;
    expect_string(family, "schema_version", "redline.family-ci/v1")?;
    expect_string(family, "family", "redline-split")?;
    expect_string(family, "status", "pass")?;
    let repositories = require_array(family, "repositories")?;
    if repositories
        .iter()
        .any(|row| row.get("status").and_then(JsonValue::as_str) != Some("pass"))
    {
        return Err("Redline family CI contains a non-passing repository".into());
    }
    let family_repositories = repositories
        .iter()
        .map(|row| {
            row.get("name")
                .and_then(JsonValue::as_str)
                .ok_or("Redline family CI repository has no name")
        })
        .collect::<Result<BTreeSet<_>, _>>()?;
    if family_repositories
        != BTreeSet::from(["redline", "redline-core", "redline-testing", "redline-web"])
        || repositories.len() != 4
    {
        return Err("Redline family CI repository inventory is not exact".into());
    }
    let core = repositories
        .iter()
        .find(|row| row.get("name").and_then(JsonValue::as_str) == Some("redline-core"))
        .and_then(JsonValue::as_object)
        .ok_or("Redline family CI has no redline-core row")?;
    if require_string(core, "tag")? != engine_tag
        || require_string(core, "release_commit")? != engine_commit
    {
        return Err("Redline family CI engine identity differs from release".into());
    }

    validate_redline_consumer(
        &proofs.jain_consumer,
        "jain-split",
        engine_tag,
        engine_commit,
        &proof_lock_id,
        &family_sha,
    )?;
    validate_redline_consumer(
        &proofs.jeryu_consumer,
        "jeryu-split",
        engine_tag,
        engine_commit,
        &proof_lock_id,
        &family_sha,
    )?;

    let lock: toml::Value = std::str::from_utf8(&proofs.redline_lock_bytes)?.parse()?;
    let lock = lock.as_table().ok_or("Redline lock is not a table")?;
    for (key, expected) in [
        ("schema_version", "redline.split.lock/v2"),
        ("family", "redline-split"),
        ("engine_tag", engine_tag),
        ("engine_commit", engine_commit),
        ("proof_lock_id", proof_lock_id.as_str()),
    ] {
        if lock.get(key).and_then(toml::Value::as_str) != Some(expected) {
            return Err(format!("Redline lock {key} differs from release proof identity").into());
        }
    }
    let consumers = lock
        .get("consumers")
        .and_then(toml::Value::as_array)
        .ok_or("Redline lock consumers are missing")?
        .iter()
        .map(|value| {
            value
                .as_str()
                .ok_or("Redline lock consumer is not a string")
        })
        .collect::<Result<BTreeSet<_>, _>>()?;
    if consumers != BTreeSet::from(["jain-split", "jeryu-split"]) {
        return Err("Redline lock consumer set is not exact".into());
    }
    let proof = lock
        .get("proof")
        .and_then(toml::Value::as_table)
        .ok_or("Redline lock proof table is missing")?;
    if proof.get("cutover_eligible").and_then(toml::Value::as_bool) != Some(true)
        || proof
            .get("family_ci_receipt_sha256")
            .and_then(toml::Value::as_str)
            != Some(family_sha.as_str())
    {
        return Err("Redline lock is not cutover-eligible for exact family CI bytes".into());
    }
    for key in ["required_consumer_evidence", "accepted_consumer_evidence"] {
        let values = proof
            .get(key)
            .and_then(toml::Value::as_array)
            .ok_or_else(|| format!("Redline lock {key} is missing"))?
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .ok_or("Redline proof consumer is not a string")
            })
            .collect::<Result<BTreeSet<_>, _>>()?;
        if values != BTreeSet::from(["jain-split", "jeryu-split"]) {
            return Err(format!("Redline lock {key} is not exact").into());
        }
    }
    let evidence = proof
        .get("consumer_evidence")
        .and_then(toml::Value::as_array)
        .ok_or("Redline lock consumer evidence rows are missing")?;
    let mut evidence_digests = BTreeMap::new();
    for row in evidence {
        let row = row
            .as_table()
            .ok_or("Redline consumer evidence row is not a table")?;
        let consumer = row
            .get("consumer")
            .and_then(toml::Value::as_str)
            .ok_or("Redline lock consumer evidence has no consumer")?;
        let digest = row
            .get("sha256")
            .and_then(toml::Value::as_str)
            .ok_or("Redline lock consumer evidence has no digest")?;
        if evidence_digests.insert(consumer, digest).is_some() {
            return Err("Redline lock repeats consumer evidence".into());
        }
    }
    if evidence_digests
        != BTreeMap::from([
            ("jain-split", jain_sha.as_str()),
            ("jeryu-split", jeryu_sha.as_str()),
        ])
    {
        return Err("Redline lock does not bind both exact consumer evidence files".into());
    }
    Ok(())
}

fn validate_redline_consumer(
    value: &JsonValue,
    expected_consumer: &str,
    engine_tag: &str,
    engine_commit: &str,
    proof_lock_id: &str,
    family_sha256: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let object = exact_object(
        value,
        &[
            "consumer",
            "consumer_manifest_sha256",
            "consumer_policy_sha256",
            "engine_commit",
            "engine_tag",
            "family",
            "family_ci_receipt_sha256",
            "generated_at",
            "manifest_sha256",
            "policy_sha256",
            "proof_lock_id",
            "required_check",
            "schema_version",
            "source_commit",
            "status",
            "test_log",
            "test_log_sha256",
            "tool_version",
        ],
        "Redline consumer evidence",
    )?;
    expect_string(object, "schema_version", "redline.consumer-evidence/v1")?;
    expect_string(object, "consumer", expected_consumer)?;
    expect_string(object, "family", "redline-split")?;
    expect_string(object, "status", "pass")?;
    expect_string(object, "engine_tag", engine_tag)?;
    expect_string(object, "engine_commit", engine_commit)?;
    expect_string(object, "proof_lock_id", proof_lock_id)?;
    if require_sha256(object, "family_ci_receipt_sha256")? != family_sha256 {
        return Err("Redline consumer does not bind exact family CI bytes".into());
    }
    require_hex(object, "source_commit", 40)?;
    for key in [
        "manifest_sha256",
        "policy_sha256",
        "consumer_manifest_sha256",
        "consumer_policy_sha256",
        "test_log_sha256",
    ] {
        require_sha256(object, key)?;
    }
    let required_check = format!("{expected_consumer}/redline-consumer");
    expect_string(object, "required_check", &required_check)?;
    if require_string(object, "generated_at")?.is_empty()
        || require_string(object, "test_log")?.is_empty()
        || require_string(object, "tool_version")?.is_empty()
    {
        return Err("Redline consumer evidence has an empty identity field".into());
    }
    Ok(())
}

fn validate_rollback_proof(
    release: &JsonValue,
    proof_bytes: &[u8],
    proof: &JsonValue,
) -> Result<(), Box<dyn std::error::Error>> {
    let release = release.as_object().ok_or("release spec is not an object")?;
    let rollback = release
        .get("rollback")
        .and_then(JsonValue::as_object)
        .ok_or("release rollback identity is missing")?;
    let receipts = release
        .get("receipts")
        .and_then(JsonValue::as_object)
        .ok_or("release receipts are missing")?;
    let proof = exact_object(
        proof,
        &[
            "artifact_set_sha256",
            "receipt_id",
            "release",
            "roll_forward_artifact_set_sha256",
            "rollback_verified",
            "schema_version",
            "source_freeze_sha256",
            "status",
            "target_release",
        ],
        "rollback proof",
    )?;
    expect_string(proof, "schema_version", "jain.rollback-proof/v1")?;
    expect_string(proof, "release", RELEASE)?;
    expect_string(proof, "receipt_id", "9.0.0-distributed.1/rollback-proof")?;
    expect_string(proof, "status", "pass")?;
    expect_string(proof, "target_release", ROLLBACK)?;
    expect_bool(proof, "rollback_verified", true)?;
    for key in ["source_freeze_sha256", "artifact_set_sha256"] {
        if require_sha256(proof, key)? != require_sha256(rollback, key)? {
            return Err(format!("rollback proof does not bind release {key}").into());
        }
    }
    if require_sha256(proof, "roll_forward_artifact_set_sha256")?
        != require_sha256(release, "artifact_set_sha256")?
    {
        return Err(
            "rollback proof does not bind identical distributed roll-forward artifacts".into(),
        );
    }
    let digest = sha256_bytes(proof_bytes);
    if require_sha256(rollback, "receipt_sha256")? != digest
        || require_sha256(receipts, "rollback_sha256")? != digest
    {
        return Err("release does not bind exact rollback proof bytes".into());
    }
    Ok(())
}

fn validate_owner_signature_receipts(
    release: &JsonValue,
    receipts: &[(Vec<u8>, JsonValue)],
) -> Result<(), Box<dyn std::error::Error>> {
    if receipts.len() != 2 {
        return Err("exactly two owner signature receipts are required".into());
    }
    let release = release.as_object().ok_or("release spec is not an object")?;
    let payload = require_sha256(release, "canonical_payload_sha256")?;
    let signatures = require_array(release, "signatures")?;
    let mut by_receipt = BTreeMap::new();
    for row in signatures {
        let row = row
            .as_object()
            .ok_or("release signature row is not an object")?;
        by_receipt.insert(require_string(row, "receipt_id")?, row);
    }
    for (bytes, value) in receipts {
        let receipt = exact_object(
            value,
            &[
                "key_fingerprint_sha256",
                "owner_id",
                "payload_sha256",
                "receipt_id",
                "release",
                "schema_version",
                "signature_sha256",
                "status",
                "verifier",
            ],
            "owner signature receipt",
        )?;
        expect_string(receipt, "schema_version", "jain.owner-signature/v1")?;
        expect_string(receipt, "release", RELEASE)?;
        expect_string(receipt, "status", "verified")?;
        expect_string(receipt, "verifier", "jain-owner-signature-verify/v1")?;
        if require_sha256(receipt, "payload_sha256")? != payload {
            return Err("owner signature receipt signs the wrong canonical payload".into());
        }
        let receipt_id = require_string(receipt, "receipt_id")?;
        let row = by_receipt
            .remove(receipt_id)
            .ok_or("owner signature receipt is not declared by release")?;
        for key in [
            "owner_id",
            "key_fingerprint_sha256",
            "payload_sha256",
            "signature_sha256",
        ] {
            if require_string(receipt, key)? != require_string(row, key)? {
                return Err(format!("owner signature receipt differs at {key}").into());
            }
        }
        if require_sha256(row, "receipt_sha256")? != sha256_bytes(bytes) {
            return Err("release does not bind exact owner signature receipt bytes".into());
        }
    }
    if !by_receipt.is_empty() {
        return Err("release has an owner signature without an exact receipt file".into());
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

fn validate_release_signatures(signatures: &[JsonValue]) -> Result<(), Box<dyn std::error::Error>> {
    if signatures.len() != 2 {
        return Err("expected exactly two distinct owner signature receipts".into());
    }
    let mut owners = BTreeSet::new();
    let mut keys = BTreeSet::new();
    let mut receipt_ids = BTreeSet::new();
    for (index, signature) in signatures.iter().enumerate() {
        let signature = exact_object(
            signature,
            &[
                "key_fingerprint_sha256",
                "owner_id",
                "payload_sha256",
                "receipt_id",
                "receipt_sha256",
                "signature_sha256",
            ],
            &format!("signatures[{index}]"),
        )?;
        let owner = require_string(signature, "owner_id")?;
        let key = require_sha256(signature, "key_fingerprint_sha256")?;
        let receipt_id = require_string(signature, "receipt_id")?;
        if receipt_id != format!("{RELEASE}/owner-signature/{owner}") {
            return Err("owner signature receipt_id does not bind its owner".into());
        }
        require_sha256(signature, "payload_sha256")?;
        require_sha256(signature, "receipt_sha256")?;
        require_sha256(signature, "signature_sha256")?;
        if owner.is_empty()
            || receipt_id.is_empty()
            || !owners.insert(owner.to_owned())
            || !keys.insert(key.to_owned())
            || !receipt_ids.insert(receipt_id.to_owned())
        {
            return Err("signature owners, keys, and receipt IDs must be distinct".into());
        }
    }
    Ok(())
}

struct DagNode {
    wave: i64,
    dependencies: BTreeSet<String>,
    has_cargo_members: bool,
    tag: String,
    commit: String,
    tree: String,
    checksum_sha256: String,
}

fn build_dag(
    data: &toml::Value,
    manifest_sha256: String,
    locked_graph: &JsonValue,
    locked_cargo_graph_sha256: String,
) -> Result<JsonValue, Box<dyn std::error::Error>> {
    let manifest_root = data
        .as_table()
        .ok_or("release DAG manifest is not a table")?;
    let rollout_wave_order = manifest_root
        .get("rollout_wave_order")
        .and_then(toml::Value::as_array)
        .ok_or("release DAG manifest has no rollout_wave_order")?
        .iter()
        .map(|value| value.as_integer().ok_or("rollout wave is not an integer"))
        .collect::<Result<Vec<_>, _>>()?;
    if rollout_wave_order.is_empty()
        || rollout_wave_order.iter().any(|wave| *wave < 0)
        || rollout_wave_order.windows(2).any(|pair| pair[0] >= pair[1])
    {
        return Err("rollout_wave_order must be non-empty, unique, and ascending".into());
    }
    let wave_positions = rollout_wave_order
        .iter()
        .enumerate()
        .map(|(position, wave)| (*wave, position))
        .collect::<BTreeMap<_, _>>();

    let locked_graph = exact_object(
        locked_graph,
        &["edges", "release", "repositories", "schema_version"],
        "locked Cargo graph",
    )?;
    expect_string(locked_graph, "schema_version", "jain.locked-cargo-graph/v1")?;
    expect_string(locked_graph, "release", RELEASE)?;
    let mut locked_repositories = BTreeMap::new();
    for (index, row) in require_array(locked_graph, "repositories")?
        .iter()
        .enumerate()
    {
        let row = exact_object(
            row,
            &["cargo_lock_sha256", "repository"],
            &format!("locked Cargo graph repositories[{index}]"),
        )?;
        let repository = require_string(row, "repository")?;
        if !valid_repository_name(repository) || locked_repositories.contains_key(repository) {
            return Err(
                format!("locked Cargo repository is invalid or duplicate: {repository}").into(),
            );
        }
        let lock = match row.get("cargo_lock_sha256") {
            Some(JsonValue::Null) => None,
            Some(JsonValue::String(value)) => {
                if !valid_nonzero_hex(value, 64) {
                    return Err(format!("locked Cargo digest is invalid: {repository}").into());
                }
                Some(value.to_owned())
            }
            _ => return Err(format!("locked Cargo digest has invalid type: {repository}").into()),
        };
        locked_repositories.insert(repository.to_owned(), lock);
    }
    let mut locked_edges = BTreeSet::new();
    for (index, row) in require_array(locked_graph, "edges")?.iter().enumerate() {
        let row = exact_object(
            row,
            &["consumer", "dependency"],
            &format!("locked Cargo graph edges[{index}]"),
        )?;
        let consumer = require_string(row, "consumer")?;
        let dependency = require_string(row, "dependency")?;
        if !valid_repository_name(consumer)
            || !valid_repository_name(dependency)
            || !locked_edges.insert((consumer.to_owned(), dependency.to_owned()))
        {
            return Err(format!(
                "locked Cargo edge is invalid or duplicate: {consumer}->{dependency}"
            )
            .into());
        }
    }

    let mut nodes: BTreeMap<String, DagNode> = BTreeMap::new();
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
            if !wave_positions.contains_key(&wave) {
                return Err(format!(
                    "repository {name} uses wave {wave} outside rollout_wave_order"
                )
                .into());
            }
            let dependencies = toml_string_set(table.get("cross_repo_deps"), name)?;
            let has_cargo_members = table
                .get("cargo_members")
                .and_then(toml::Value::as_array)
                .is_some_and(|members| !members.is_empty());
            let tag = table
                .get("current_tag")
                .and_then(toml::Value::as_str)
                .ok_or_else(|| format!("release DAG repository has no current_tag: {name}"))?;
            if !valid_release_tag(name, tag) {
                return Err(format!("release DAG repository has an invalid tag: {name}").into());
            }
            let commit = manifest_hex(table, "release_commit", 40, name)?;
            let tree = manifest_hex(table, "release_tree", 40, name)?;
            let checksum_sha256 = manifest_hex(table, "release_checksum_sha256", 64, name)?;
            nodes.insert(
                name.to_owned(),
                DagNode {
                    wave,
                    dependencies,
                    has_cargo_members,
                    tag: tag.to_owned(),
                    commit,
                    tree,
                    checksum_sha256,
                },
            );
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
                .dependencies
                .insert(source.to_owned());
        }
    }
    if locked_repositories.keys().collect::<BTreeSet<_>>() != nodes.keys().collect::<BTreeSet<_>>()
    {
        return Err("locked Cargo repository inventory differs from authority manifest".into());
    }
    for (name, node) in &nodes {
        if node.has_cargo_members && locked_repositories[name].is_none() {
            return Err(format!("Rust repository has no locked Cargo digest: {name}").into());
        }
    }
    let manifest_edges = nodes
        .iter()
        .flat_map(|(consumer, node)| {
            node.dependencies
                .iter()
                .map(move |dependency| (consumer.clone(), dependency.clone()))
        })
        .collect::<BTreeSet<_>>();
    if locked_edges != manifest_edges {
        return Err("locked Cargo edges disagree with authority dependency edges".into());
    }
    for (name, node) in &nodes {
        for dependency in &node.dependencies {
            if dependency == name || !nodes.contains_key(dependency) {
                return Err(format!(
                    "release DAG dependency is self-referential or unknown: {name}->{dependency}"
                )
                .into());
            }
            let dependency_wave = nodes[dependency].wave;
            if wave_positions[&dependency_wave] > wave_positions[&node.wave] {
                return Err(format!(
                    "authority wave orders dependency after consumer: {name}->{dependency}"
                )
                .into());
            }
        }
    }

    let mut remaining = nodes
        .iter()
        .map(|(name, node)| (name.clone(), node.dependencies.clone()))
        .collect::<BTreeMap<_, _>>();
    let mut ordered = Vec::new();
    while !remaining.is_empty() {
        let ready = remaining
            .iter()
            .filter(|(_, dependencies)| dependencies.is_empty())
            .map(|(name, _)| name.clone())
            .min_by_key(|name| (wave_positions[&nodes[name].wave], name.clone()))
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
            let node = &nodes[name];
            json!({
                "position": position,
                "repository": name,
                "declared_wave": node.wave,
                "depends_on": node.dependencies.iter().collect::<Vec<_>>(),
                "cargo_lock_sha256": locked_repositories[name],
                "tag": node.tag.as_str(),
                "commit": node.commit.as_str(),
                "tree": node.tree.as_str(),
                "checksum_sha256": node.checksum_sha256.as_str(),
            })
        })
        .collect::<Vec<_>>();
    Ok(json!({
        "schema_version": "jain.release-dag/v1",
        "release": RELEASE,
        "manifest_sha256": manifest_sha256,
        "locked_cargo_graph_sha256": locked_cargo_graph_sha256,
        "rollout_wave_order": rollout_wave_order,
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

fn manifest_hex(
    table: &toml::map::Map<String, toml::Value>,
    key: &str,
    length: usize,
    repository: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let value = table
        .get(key)
        .and_then(toml::Value::as_str)
        .ok_or_else(|| format!("release DAG repository has no {key}: {repository}"))?;
    if !valid_nonzero_hex(value, length) {
        return Err(format!("release DAG repository has invalid {key}: {repository}").into());
    }
    Ok(value.to_owned())
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
    output: Option<&PreparedOutput>,
) -> Result<JsonValue, Box<dyn std::error::Error>> {
    let root = open_physical_directory(root, "distributed evidence root")?;
    let output_parent_identity =
        output.map(|output| (output.parent.identity.device, output.parent.identity.inode));
    if output.is_some_and(|output| output.absolute.starts_with(&root.absolute))
        || output_parent_identity == Some((root.identity.device, root.identity.inode))
    {
        return Err("evidence index output must be outside the indexed root".into());
    }
    let mut present_historical = Vec::new();
    let mut present_historical_ids = BTreeSet::new();
    let mut historical_files = Vec::new();
    for historical in historical_roots {
        let historical = open_physical_directory(historical, "historical evidence root")?;
        let identity = (historical.identity.device, historical.identity.inode);
        if identity == (root.identity.device, root.identity.inode)
            || !present_historical_ids.insert(identity)
            || historical.absolute.starts_with(&root.absolute)
            || root.absolute.starts_with(&historical.absolute)
            || present_historical.iter().any(|prior: &PathBuf| {
                historical.absolute.starts_with(prior) || prior.starts_with(&historical.absolute)
            })
        {
            return Err("distributed and historical evidence roots overlap or repeat".into());
        }
        if output.is_some_and(|output| output.absolute.starts_with(&historical.absolute)) {
            return Err("evidence index output must be outside historical evidence".into());
        }
        historical_files.extend(index_tree(&historical, output_parent_identity)?);
        present_historical.push(historical.absolute);
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
    let files = index_tree(&root, output_parent_identity)?;
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
        "root": root.absolute,
        "historical_roots": present_historical,
        "absent_historical_roots": asserted_absent,
        "file_count": rows.len(),
        "files": rows,
        "receipt_identifiers": receipt_ids,
        "status": "pass",
    }))
}

fn index_tree(
    root: &OpenedPhysical,
    forbidden_directory_identity: Option<(u64, u64)>,
) -> Result<Vec<IndexedFile>, Box<dyn std::error::Error>> {
    fn visit(
        directory: &File,
        relative_directory: &Path,
        forbidden_directory_identity: Option<(u64, u64)>,
        files: &mut Vec<IndexedFile>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let directory_metadata = directory.metadata()?;
        if forbidden_directory_identity
            == Some((directory_metadata.dev(), directory_metadata.ino()))
        {
            return Err("evidence index output parent aliases an evidence directory".into());
        }
        // `/proc/self/fd` is used only to enumerate names. Every child is opened and
        // validated relative to the retained directory descriptor below.
        let descriptor_path = PathBuf::from(format!("/proc/self/fd/{}", directory.as_raw_fd()));
        let mut entries = fs::read_dir(descriptor_path)?.collect::<Result<Vec<_>, _>>()?;
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let name = entry.file_name();
            if name.as_bytes().is_empty()
                || name.as_bytes().contains(&b'/')
                || name.as_bytes().contains(&0)
            {
                return Err("evidence tree contains an unsafe path component".into());
            }
            let relative = relative_directory.join(&name);
            let mut child = open_at_file(
                directory.as_raw_fd(),
                &name,
                libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC,
                0,
                "evidence entry",
            )?;
            let metadata = child.metadata()?;
            let identity = FileIdentity::from_metadata(&metadata);
            if metadata.file_type().is_dir() {
                visit(&child, &relative, forbidden_directory_identity, files)?;
                let reopened = open_at_file(
                    directory.as_raw_fd(),
                    &name,
                    libc::O_RDONLY
                        | libc::O_DIRECTORY
                        | libc::O_NOFOLLOW
                        | libc::O_NONBLOCK
                        | libc::O_CLOEXEC,
                    0,
                    "evidence directory",
                )?;
                if FileIdentity::from_metadata(&reopened.metadata()?) != identity {
                    return Err(format!(
                        "evidence directory changed while indexing: {}",
                        relative.display()
                    )
                    .into());
                }
                continue;
            }
            if !metadata.file_type().is_file()
                || identity.links != 1
                || identity.length > MAX_AUTHORITY_BYTES
            {
                return Err(format!(
                    "evidence is not an independent bounded regular file: {}",
                    relative.display()
                )
                .into());
            }
            let mut bytes = Vec::with_capacity(identity.length as usize);
            Read::by_ref(&mut child)
                .take(MAX_AUTHORITY_BYTES + 1)
                .read_to_end(&mut bytes)?;
            if bytes.len() as u64 != identity.length
                || FileIdentity::from_metadata(&child.metadata()?) != identity
            {
                return Err(format!(
                    "evidence file changed while indexing: {}",
                    relative.display()
                )
                .into());
            }
            let reopened = open_at_file(
                directory.as_raw_fd(),
                &name,
                libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC,
                0,
                "evidence file",
            )?;
            if FileIdentity::from_metadata(&reopened.metadata()?) != identity {
                return Err(format!(
                    "evidence file path changed while indexing: {}",
                    relative.display()
                )
                .into());
            }
            let mut receipt_ids = BTreeSet::new();
            if relative
                .extension()
                .and_then(|extension| extension.to_str())
                == Some("json")
            {
                let value: JsonValue = serde_json::from_slice(&bytes)?;
                collect_receipt_ids(&value, &mut receipt_ids)?;
            }
            files.push(IndexedFile {
                relative: relative
                    .to_str()
                    .ok_or("evidence path is not UTF-8")?
                    .to_owned(),
                size: identity.length,
                sha256: sha256_bytes(&bytes),
                device: identity.device,
                inode: identity.inode,
                receipt_ids,
            });
            if files.len() > 100_000 {
                return Err("evidence tree exceeds 100000 files".into());
            }
        }
        Ok(())
    }
    let mut files = Vec::new();
    visit(
        &root.file,
        Path::new(""),
        forbidden_directory_identity,
        &mut files,
    )?;
    ensure_path_identity(root, libc::O_RDONLY | libc::O_DIRECTORY, "evidence root")?;
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
    output: Option<&PreparedOutput>,
    apply: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut bytes = serde_json::to_vec_pretty(&value)?;
    bytes.push(b'\n');
    if let Some(output) = output {
        if !apply {
            return Err("writing JSON output requires --apply".into());
        }
        ensure_path_identity(
            &output.parent,
            libc::O_RDONLY | libc::O_DIRECTORY,
            "JSON output parent",
        )?;
        let mut file = open_at_file(
            output.parent.file.as_raw_fd(),
            &output.name,
            libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            0o644,
            "JSON output",
        )?;
        let metadata = file.metadata()?;
        if !metadata.file_type().is_file() || metadata.nlink() != 1 {
            return Err("JSON output is not a new independent regular file".into());
        }
        file.write_all(&bytes)?;
        file.sync_all()?;
        output.parent.file.sync_all()?;
        println!("wrote {}", output.absolute.display());
    } else {
        io::stdout().write_all(&bytes)?;
    }
    Ok(())
}

fn prepare_output(path: &Path) -> Result<PreparedOutput, Box<dyn std::error::Error>> {
    let absolute = normalized_absolute(path)?;
    let name = absolute
        .file_name()
        .ok_or("JSON output has no file name")?
        .to_owned();
    if name.as_bytes().is_empty() || name.as_bytes().contains(&0) {
        return Err("JSON output has an unsafe file name".into());
    }
    let parent_path = absolute.parent().ok_or("JSON output has no parent")?;
    let parent = open_physical_directory(parent_path, "JSON output parent")?;
    match fs::symlink_metadata(&absolute) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
        Ok(_) => {
            return Err(format!("refusing to replace JSON output: {}", absolute.display()).into())
        }
    }
    Ok(PreparedOutput {
        parent,
        name,
        absolute,
    })
}

fn absolute_lexical(path: &Path) -> Result<PathBuf, Box<dyn std::error::Error>> {
    normalized_absolute(path)
}

fn normalized_absolute(path: &Path) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        env::current_dir()?.join(path)
    };
    let mut normalized = PathBuf::from("/");
    for component in absolute.components() {
        match component {
            Component::RootDir | Component::CurDir => {}
            Component::Normal(value) => normalized.push(value),
            Component::ParentDir => {
                return Err(format!("path contains parent traversal: {}", path.display()).into())
            }
            Component::Prefix(_) => return Err("unsupported path prefix".into()),
        }
    }
    Ok(normalized)
}

fn open_physical_directory(
    path: &Path,
    label: &str,
) -> Result<OpenedPhysical, Box<dyn std::error::Error>> {
    let opened = open_physical(path, libc::O_RDONLY | libc::O_DIRECTORY, label)?;
    if !opened.file.metadata()?.file_type().is_dir() {
        return Err(format!("{label} is not a physical directory").into());
    }
    Ok(opened)
}

fn open_physical(
    path: &Path,
    final_flags: i32,
    label: &str,
) -> Result<OpenedPhysical, Box<dyn std::error::Error>> {
    let absolute = normalized_absolute(path)?;
    let components = absolute
        .components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value.to_owned()),
            _ => None,
        })
        .collect::<Vec<_>>();
    let mut file = open_at_file(
        libc::AT_FDCWD,
        OsStr::new("/"),
        libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        0,
        label,
    )?;
    let mut component_ids = Vec::new();
    let root_metadata = file.metadata()?;
    component_ids.push((root_metadata.dev(), root_metadata.ino()));
    for (index, component) in components.iter().enumerate() {
        let is_final = index + 1 == components.len();
        let flags = if is_final {
            final_flags | libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC
        } else {
            libc::O_RDONLY
                | libc::O_DIRECTORY
                | libc::O_NOFOLLOW
                | libc::O_NONBLOCK
                | libc::O_CLOEXEC
        };
        let next = open_at_file(file.as_raw_fd(), component, flags, 0, label)?;
        let metadata = next.metadata()?;
        if !is_final && !metadata.file_type().is_dir() {
            return Err(format!("{label} has a non-directory parent").into());
        }
        component_ids.push((metadata.dev(), metadata.ino()));
        file = next;
    }
    let identity = FileIdentity::from_metadata(&file.metadata()?);
    Ok(OpenedPhysical {
        file,
        absolute,
        identity,
        component_ids,
    })
}

fn ensure_path_identity(
    opened: &OpenedPhysical,
    final_flags: i32,
    label: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let reopened = open_physical(&opened.absolute, final_flags, label)?;
    if reopened.identity != opened.identity || reopened.component_ids != opened.component_ids {
        return Err(format!("{label} path changed while in use").into());
    }
    Ok(())
}

fn open_at_file(
    directory: i32,
    name: &OsStr,
    flags: i32,
    mode: libc::mode_t,
    label: &str,
) -> Result<File, Box<dyn std::error::Error>> {
    let name = CString::new(name.as_bytes()).map_err(|_| format!("{label} contains a NUL byte"))?;
    // SAFETY: the name is NUL-terminated, directory is AT_FDCWD or a live
    // descriptor, and a successful descriptor is immediately owned by File.
    let descriptor = unsafe { libc::openat(directory, name.as_ptr(), flags, mode) };
    if descriptor < 0 {
        return Err(format!(
            "cannot securely open {label}: {}",
            io::Error::last_os_error()
        )
        .into());
    }
    // SAFETY: openat returned a new owned descriptor.
    Ok(unsafe { File::from_raw_fd(descriptor) })
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
    if !valid_nonzero_hex(value, length) {
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

fn valid_nonzero_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        && !value.bytes().all(|byte| byte == b'0')
}

fn valid_release_tag(repository: &str, tag: &str) -> bool {
    let prefix = format!("{repository}-v{RELEASE}-split.");
    tag.strip_prefix(&prefix)
        .and_then(|revision| revision.parse::<u64>().ok())
        .is_some_and(|revision| revision > 0)
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

fn deployed_image_set_digest(rows: &[JsonValue]) -> Result<String, Box<dyn std::error::Error>> {
    let mut lines = Vec::new();
    for row in rows {
        let row = row.as_object().ok_or("artifact row is not an object")?;
        let kind = require_string(row, "kind")?;
        if matches!(kind, "caddy" | "hub" | "node" | "worker") {
            lines.push(format!(
                "{}\0{}\0{}\n",
                kind,
                require_string(row, "name")?,
                require_string(row, "readback_sha256")?,
            ));
        }
    }
    lines.sort();
    if lines.len() != 4 {
        return Err("canonical deployed image inventory must contain exactly four images".into());
    }
    Ok(sha256_bytes(lines.concat().as_bytes()))
}

fn canonical_release_payload_digest(
    release: &JsonValue,
) -> Result<String, Box<dyn std::error::Error>> {
    let mut payload = release.clone();
    let object = payload
        .as_object_mut()
        .ok_or("release spec is not an object")?;
    object.remove("canonical_payload_sha256");
    object.remove("signatures");
    let mut bytes = b"jain.distributed-release/v1\0".to_vec();
    bytes.extend(serde_json::to_vec(&payload)?);
    Ok(sha256_bytes(&bytes))
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

    struct TestDocuments {
        release: JsonValue,
        soak: JsonValue,
        qualification: JsonValue,
        proofs: ProofDocuments,
    }

    fn consumer_evidence(
        consumer: &str,
        engine_tag: &str,
        engine_commit: &str,
        family_sha256: &str,
    ) -> JsonValue {
        let (required_check, tool_version) = match consumer {
            "jain-split" => ("jain-split/redline-consumer", "jain-redline-consumer/v1"),
            "jeryu-split" => ("jeryu-split/redline-consumer", "jeryu-redline-consumer/v1"),
            _ => unreachable!(),
        };
        json!({
            "schema_version": "redline.consumer-evidence/v1",
            "consumer": consumer,
            "family": "redline-split",
            "generated_at": "2026-07-19T00:00:00Z",
            "status": "pass",
            "source_commit": "5".repeat(40),
            "required_check": required_check,
            "engine_tag": engine_tag,
            "engine_commit": engine_commit,
            "proof_lock_id": format!("redline-proof/v2/4.1.0/{engine_commit}"),
            "family_ci_receipt_sha256": family_sha256,
            "manifest_sha256": digest('1'),
            "policy_sha256": digest('2'),
            "consumer_manifest_sha256": digest('3'),
            "consumer_policy_sha256": digest('4'),
            "test_log": format!("{consumer}.log"),
            "test_log_sha256": digest('5'),
            "tool_version": tool_version
        })
    }

    fn valid_bundle() -> TestDocuments {
        let (mut release, mut soak, mut qualification) = valid_documents();
        let (artifact_set, image_set) = {
            let artifacts = release["artifacts"].as_array_mut().unwrap();
            for (kind, name, character) in [
                ("pack", "jain-cpu-demo-pack", 'a'),
                ("compose", "compose.yaml", 'b'),
                ("installer", "jain-appliance", 'c'),
            ] {
                artifacts.push(json!({
                    "kind": kind,
                    "name": name,
                    "sha256": digest(character),
                    "readback_sha256": digest(character),
                    "signature_receipt_sha256": digest('d')
                }));
            }
            (
                artifact_set_digest(artifacts).unwrap(),
                deployed_image_set_digest(artifacts).unwrap(),
            )
        };
        release["artifact_set_sha256"] = json!(artifact_set);
        qualification["artifact_set_sha256"] = json!(artifact_set);
        for host in release["hosts"].as_array_mut().unwrap() {
            host["image_set_sha256"] = json!(image_set);
        }

        let dag = json!({
            "schema_version": "jain.release-dag/v1",
            "release": RELEASE,
            "manifest_sha256": digest('a'),
            "locked_cargo_graph_sha256": digest('b'),
            "rollout_wave_order": [0],
            "repository_count": 1,
            "repositories": [{
                "position": 0,
                "repository": "jain-fabric",
                "declared_wave": 0,
                "depends_on": [],
                "cargo_lock_sha256": digest('c'),
                "tag": "jain-fabric-v9.0.0-distributed.1-split.1",
                "commit": "1".repeat(40),
                "tree": "2".repeat(40),
                "checksum_sha256": digest('3')
            }],
            "status": "pass"
        });
        let dag_bytes = serde_json::to_vec_pretty(&dag).unwrap();
        release["release_dag_sha256"] = json!(sha256_bytes(&dag_bytes));

        let routes_before_bytes = br#"[{"handle":"legacy-public"}]
"#
        .to_vec();
        let routes_after_bytes = routes_before_bytes.clone();
        let route_sha = sha256_bytes(&routes_before_bytes);
        release["routes"]["external_caddy_route_sha256_before"] = json!(route_sha);
        release["routes"]["external_caddy_route_sha256_after"] = json!(route_sha);
        let caddy_proof = json!({
            "schema_version": "jain.caddy-unchanged/v1",
            "release": RELEASE,
            "receipt_id": "9.0.0-distributed.1/caddy-unchanged",
            "status": "pass",
            "etag_before": "fixture-etag",
            "etag_after": "fixture-etag",
            "route_array_sha256_before": route_sha,
            "route_array_sha256_after": route_sha,
            "dns_unchanged": true,
            "public_admission_unchanged": true
        });
        let caddy_proof_bytes = serde_json::to_vec_pretty(&caddy_proof).unwrap();
        release["receipts"]["caddy_unchanged_sha256"] = json!(sha256_bytes(&caddy_proof_bytes));

        let engine_tag = "redline-core-v4.1.0-jain.4";
        let engine_commit = "4".repeat(40);
        let family_ci = json!({
            "schema_version": "redline.family-ci/v1",
            "family": "redline-split",
            "status": "pass",
            "repositories": [
                {"name": "redline", "status": "pass"},
                {
                    "name": "redline-core",
                    "tag": engine_tag,
                    "release_commit": engine_commit,
                    "status": "pass"
                },
                {"name": "redline-testing", "status": "pass"},
                {"name": "redline-web", "status": "pass"}
            ]
        });
        let family_ci_bytes = serde_json::to_vec_pretty(&family_ci).unwrap();
        let family_sha = sha256_bytes(&family_ci_bytes);
        let jain_consumer =
            consumer_evidence("jain-split", engine_tag, &engine_commit, &family_sha);
        let jeryu_consumer =
            consumer_evidence("jeryu-split", engine_tag, &engine_commit, &family_sha);
        let jain_consumer_bytes = serde_json::to_vec_pretty(&jain_consumer).unwrap();
        let jeryu_consumer_bytes = serde_json::to_vec_pretty(&jeryu_consumer).unwrap();
        let jain_sha = sha256_bytes(&jain_consumer_bytes);
        let jeryu_sha = sha256_bytes(&jeryu_consumer_bytes);
        let redline_lock_bytes = format!(
            "schema_version = \"redline.split.lock/v2\"\n\
             family = \"redline-split\"\n\
             engine_tag = \"{engine_tag}\"\n\
             engine_commit = \"{engine_commit}\"\n\
             proof_lock_id = \"redline-proof/v2/4.1.0/{engine_commit}\"\n\
             consumers = [\"jain-split\", \"jeryu-split\"]\n\
             \n\
             [proof]\n\
             family_ci_receipt_sha256 = \"{family_sha}\"\n\
             required_consumer_evidence = [\"jain-split\", \"jeryu-split\"]\n\
             accepted_consumer_evidence = [\"jain-split\", \"jeryu-split\"]\n\
             cutover_eligible = true\n\
             \n\
             [[proof.consumer_evidence]]\n\
             consumer = \"jain-split\"\n\
             sha256 = \"{jain_sha}\"\n\
             \n\
             [[proof.consumer_evidence]]\n\
             consumer = \"jeryu-split\"\n\
             sha256 = \"{jeryu_sha}\"\n"
        )
        .into_bytes();
        let redline_lock_mirror_bytes = redline_lock_bytes.clone();
        let lock_sha = sha256_bytes(&redline_lock_bytes);
        release["redline"] = json!({
            "engine_tag": engine_tag,
            "engine_commit": engine_commit,
            "family_ci_sha256": family_sha,
            "lock_sha256": lock_sha,
            "jain_consumer_sha256": jain_sha,
            "jeryu_consumer_sha256": jeryu_sha,
            "cutover_eligible": true
        });
        release["receipts"]["redline_lock_sha256"] = json!(lock_sha);
        release["receipts"]["redline_jain_consumer_sha256"] = json!(jain_sha);
        release["receipts"]["redline_jeryu_consumer_sha256"] = json!(jeryu_sha);

        let rollback = json!({
            "schema_version": "jain.rollback-proof/v1",
            "release": RELEASE,
            "receipt_id": "9.0.0-distributed.1/rollback-proof",
            "status": "pass",
            "target_release": ROLLBACK,
            "source_freeze_sha256": digest('d'),
            "artifact_set_sha256": digest('e'),
            "roll_forward_artifact_set_sha256": artifact_set,
            "rollback_verified": true
        });
        let rollback_bytes = serde_json::to_vec_pretty(&rollback).unwrap();
        let rollback_sha = sha256_bytes(&rollback_bytes);
        release["rollback"]["receipt_sha256"] = json!(rollback_sha);
        release["receipts"]["rollback_sha256"] = json!(rollback_sha);

        let qualification_bytes = serde_json::to_vec_pretty(&qualification).unwrap();
        let qualification_sha = sha256_bytes(&qualification_bytes);
        soak["replacement_receipt_sha256"] = json!(qualification_sha);
        let soak_bytes = serde_json::to_vec_pretty(&soak).unwrap();
        release["receipts"]["accelerated_qualification_sha256"] = json!(qualification_sha);
        release["receipts"]["soak_status_sha256"] = json!(sha256_bytes(&soak_bytes));

        release["canonical_payload_sha256"] = json!(digest('a'));
        release["signatures"] = json!([]);
        let payload = canonical_release_payload_digest(&release).unwrap();
        release["canonical_payload_sha256"] = json!(payload);
        let mut signature_receipts = Vec::new();
        let mut signatures = Vec::new();
        for (owner, key_character, signature_character) in
            [("owner-1", '1', '2'), ("owner-2", '3', '4')]
        {
            let receipt_id = format!("{RELEASE}/owner-signature/{owner}");
            let receipt = json!({
                "schema_version": "jain.owner-signature/v1",
                "release": RELEASE,
                "receipt_id": receipt_id,
                "status": "verified",
                "owner_id": owner,
                "key_fingerprint_sha256": digest(key_character),
                "payload_sha256": payload,
                "signature_sha256": digest(signature_character)
                ,"verifier": "jain-owner-signature-verify/v1"
            });
            let bytes = serde_json::to_vec_pretty(&receipt).unwrap();
            signatures.push(json!({
                "receipt_id": receipt_id,
                "owner_id": owner,
                "key_fingerprint_sha256": digest(key_character),
                "payload_sha256": payload,
                "signature_sha256": digest(signature_character),
                "receipt_sha256": sha256_bytes(&bytes)
            }));
            signature_receipts.push((bytes, receipt));
        }
        release["signatures"] = JsonValue::Array(signatures);

        TestDocuments {
            release,
            soak,
            qualification,
            proofs: ProofDocuments {
                dag_bytes,
                dag,
                routes_before_bytes,
                routes_after_bytes,
                caddy_proof_bytes,
                caddy_proof,
                family_ci_bytes,
                family_ci,
                redline_lock_bytes,
                redline_lock_mirror_bytes,
                jain_consumer_bytes,
                jain_consumer,
                jeryu_consumer_bytes,
                jeryu_consumer,
                rollback_bytes,
                rollback,
                signature_receipts,
            },
        }
    }

    #[test]
    fn distributed_documents_are_closed_and_cross_bound() {
        let documents = valid_bundle();
        let release = documents.release;
        let soak = documents.soak;
        let qualification = documents.qualification;
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
        validate_proof_bindings(&release, &documents.proofs).unwrap();

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
        let mut wrong_repository_tag = release.clone();
        wrong_repository_tag["source_matrix"][0]["tag"] =
            json!("jain-shard-v9.0.0-distributed.1-split.1");
        assert!(validate_release_spec(&wrong_repository_tag).is_err());
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
        let mut wrong_host_images = release.clone();
        wrong_host_images["hosts"][1]["image_set_sha256"] = json!(digest('f'));
        assert!(validate_release_spec(&wrong_host_images).is_err());
        let mut missing_compose = release.clone();
        missing_compose["artifacts"]
            .as_array_mut()
            .unwrap()
            .retain(|artifact| artifact["kind"] != "compose");
        assert!(validate_release_spec(&missing_compose).is_err());
        let mut extra = qualification.clone();
        extra["soak_passed"] = json!(true);
        assert!(validate_qualification(&extra).is_err());
        let mut wrong_seed = qualification.clone();
        wrong_seed["random_seed_sha256"] = json!(digest('9'));
        assert!(validate_qualification(&wrong_seed).is_err());

        let mut wrong_signature = documents.proofs.clone();
        wrong_signature.signature_receipts[0].1["payload_sha256"] = json!(digest('f'));
        assert!(validate_proof_bindings(&release, &wrong_signature).is_err());
        let mut unrelated_rollback = documents.proofs.clone();
        unrelated_rollback.rollback["source_freeze_sha256"] = json!(digest('f'));
        unrelated_rollback.rollback_bytes =
            serde_json::to_vec_pretty(&unrelated_rollback.rollback).unwrap();
        assert!(validate_proof_bindings(&release, &unrelated_rollback).is_err());
        let mut changed_caddy = documents.proofs.clone();
        changed_caddy.routes_after_bytes.push(b' ');
        assert!(validate_proof_bindings(&release, &changed_caddy).is_err());
        let mut wrong_redline = documents.proofs.clone();
        wrong_redline.jain_consumer["engine_commit"] = json!("6".repeat(40));
        wrong_redline.jain_consumer_bytes =
            serde_json::to_vec_pretty(&wrong_redline.jain_consumer).unwrap();
        assert!(validate_proof_bindings(&release, &wrong_redline).is_err());
        let mut wrong_dag_identity = documents.proofs.dag.clone();
        wrong_dag_identity["repositories"][0]["commit"] = json!("6".repeat(40));
        let wrong_dag_bytes = serde_json::to_vec_pretty(&wrong_dag_identity).unwrap();
        let mut release_for_wrong_dag = release.clone();
        release_for_wrong_dag["release_dag_sha256"] = json!(sha256_bytes(&wrong_dag_bytes));
        assert!(validate_release_dag_binding(
            &release_for_wrong_dag,
            &wrong_dag_bytes,
            &wrong_dag_identity
        )
        .is_err());
    }

    #[test]
    fn distributed_validate_command_reads_exact_independent_files() {
        let temp = TestDir::new("validate-command");
        let documents = valid_bundle();
        let release_path = temp.0.join("release.json");
        let soak_path = temp.0.join("soak.json");
        let qualification_path = temp.0.join("qualification.json");
        let dag_path = temp.0.join("dag.json");
        let caddy_before_path = temp.0.join("caddy-before.json");
        let caddy_after_path = temp.0.join("caddy-after.json");
        let caddy_proof_path = temp.0.join("caddy-proof.json");
        let family_path = temp.0.join("redline-family.json");
        let lock_path = temp.0.join("redline.lock.toml");
        let mirror_path = temp.0.join("redline-mirror.lock.toml");
        let jain_consumer_path = temp.0.join("redline-jain.json");
        let jeryu_consumer_path = temp.0.join("redline-jeryu.json");
        let rollback_path = temp.0.join("rollback.json");
        let signature_one_path = temp.0.join("signature-one.json");
        let signature_two_path = temp.0.join("signature-two.json");
        fs::write(
            &release_path,
            serde_json::to_vec_pretty(&documents.release).unwrap(),
        )
        .unwrap();
        fs::write(
            &soak_path,
            serde_json::to_vec_pretty(&documents.soak).unwrap(),
        )
        .unwrap();
        fs::write(
            &qualification_path,
            serde_json::to_vec_pretty(&documents.qualification).unwrap(),
        )
        .unwrap();
        for (path, bytes) in [
            (&dag_path, documents.proofs.dag_bytes.as_slice()),
            (
                &caddy_before_path,
                documents.proofs.routes_before_bytes.as_slice(),
            ),
            (
                &caddy_after_path,
                documents.proofs.routes_after_bytes.as_slice(),
            ),
            (
                &caddy_proof_path,
                documents.proofs.caddy_proof_bytes.as_slice(),
            ),
            (&family_path, documents.proofs.family_ci_bytes.as_slice()),
            (&lock_path, documents.proofs.redline_lock_bytes.as_slice()),
            (
                &mirror_path,
                documents.proofs.redline_lock_mirror_bytes.as_slice(),
            ),
            (
                &jain_consumer_path,
                documents.proofs.jain_consumer_bytes.as_slice(),
            ),
            (
                &jeryu_consumer_path,
                documents.proofs.jeryu_consumer_bytes.as_slice(),
            ),
            (&rollback_path, documents.proofs.rollback_bytes.as_slice()),
            (
                &signature_one_path,
                documents.proofs.signature_receipts[0].0.as_slice(),
            ),
            (
                &signature_two_path,
                documents.proofs.signature_receipts[1].0.as_slice(),
            ),
        ] {
            fs::write(path, bytes).unwrap();
        }
        let arguments = vec![
            "--release-spec".to_owned(),
            release_path.display().to_string(),
            "--soak-status".to_owned(),
            soak_path.display().to_string(),
            "--qualification".to_owned(),
            qualification_path.display().to_string(),
            "--release-dag".to_owned(),
            dag_path.display().to_string(),
            "--caddy-routes-before".to_owned(),
            caddy_before_path.display().to_string(),
            "--caddy-routes-after".to_owned(),
            caddy_after_path.display().to_string(),
            "--caddy-proof".to_owned(),
            caddy_proof_path.display().to_string(),
            "--redline-family-ci".to_owned(),
            family_path.display().to_string(),
            "--redline-lock".to_owned(),
            lock_path.display().to_string(),
            "--redline-lock-mirror".to_owned(),
            mirror_path.display().to_string(),
            "--redline-jain-consumer".to_owned(),
            jain_consumer_path.display().to_string(),
            "--redline-jeryu-consumer".to_owned(),
            jeryu_consumer_path.display().to_string(),
            "--rollback-proof".to_owned(),
            rollback_path.display().to_string(),
            "--signature-receipt".to_owned(),
            signature_one_path.display().to_string(),
            "--signature-receipt".to_owned(),
            signature_two_path.display().to_string(),
        ];
        validate_command(arguments.clone()).unwrap();

        let linked = temp.0.join("linked-soak.json");
        symlink(&soak_path, &linked).unwrap();
        let mut linked_arguments = arguments;
        let soak_index = linked_arguments
            .iter()
            .position(|argument| argument == "--soak-status")
            .unwrap()
            + 1;
        linked_arguments[soak_index] = linked.display().to_string();
        assert!(validate_command(linked_arguments).is_err());
    }

    #[test]
    fn release_dag_is_lock_derived_deterministic_and_rejects_disagreement() {
        let manifest: toml::Value = r#"
            rollout_wave_order = [0, 1, 2, 3]
            [[repo]]
            name = "leaf"
            rollout_wave = 3
            cross_repo_deps = ["middle"]
            cargo_members = ["."]
            current_tag = "leaf-v9.0.0-distributed.1-split.1"
            release_commit = "1111111111111111111111111111111111111111"
            release_tree = "2222222222222222222222222222222222222222"
            release_checksum_sha256 = "3333333333333333333333333333333333333333333333333333333333333333"
            [[repo]]
            name = "root"
            rollout_wave = 1
            cross_repo_deps = []
            cargo_members = ["."]
            current_tag = "root-v9.0.0-distributed.1-split.1"
            release_commit = "1111111111111111111111111111111111111111"
            release_tree = "2222222222222222222222222222222222222222"
            release_checksum_sha256 = "3333333333333333333333333333333333333333333333333333333333333333"
            [[repo]]
            name = "middle"
            rollout_wave = 2
            cross_repo_deps = ["root"]
            cargo_members = ["."]
            current_tag = "middle-v9.0.0-distributed.1-split.1"
            release_commit = "1111111111111111111111111111111111111111"
            release_tree = "2222222222222222222222222222222222222222"
            release_checksum_sha256 = "3333333333333333333333333333333333333333333333333333333333333333"
            [[infrastructure_repo]]
            name = "fabric"
            rollout_wave = 0
            cross_repo_deps = []
            dependency_edges = ["leaf"]
            cargo_members = ["."]
            current_tag = "fabric-v9.0.0-distributed.1-split.1"
            release_commit = "1111111111111111111111111111111111111111"
            release_tree = "2222222222222222222222222222222222222222"
            release_checksum_sha256 = "3333333333333333333333333333333333333333333333333333333333333333"
        "#
        .parse()
        .unwrap();
        let locked_repositories = ["fabric", "root", "middle", "leaf"]
            .into_iter()
            .map(|repository| {
                json!({
                    "repository": repository,
                    "cargo_lock_sha256": digest('a')
                })
            })
            .collect::<Vec<_>>();
        let locked_graph = json!({
            "schema_version": "jain.locked-cargo-graph/v1",
            "release": RELEASE,
            "repositories": locked_repositories,
            "edges": [
                {"consumer": "middle", "dependency": "root"},
                {"consumer": "leaf", "dependency": "middle"},
                {"consumer": "leaf", "dependency": "fabric"}
            ]
        });
        let report = build_dag(&manifest, digest('a'), &locked_graph, digest('b')).unwrap();
        let names = report["repositories"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| row["repository"].as_str().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(names, ["fabric", "root", "middle", "leaf"]);
        let repeated = build_dag(&manifest, digest('a'), &locked_graph, digest('b')).unwrap();
        assert_eq!(
            serde_json::to_vec(&report).unwrap(),
            serde_json::to_vec(&repeated).unwrap()
        );
        let mut omitted_edge = locked_graph.clone();
        omitted_edge["edges"].as_array_mut().unwrap().pop();
        assert!(build_dag(&manifest, digest('a'), &omitted_edge, digest('b')).is_err());
        let wrong_wave: toml::Value = r#"
            rollout_wave_order = [0, 1]
            [[repo]]
            name = "root"
            rollout_wave = 2
            cross_repo_deps = []
            cargo_members = ["."]
            current_tag = "root-v9.0.0-distributed.1-split.1"
            release_commit = "1111111111111111111111111111111111111111"
            release_tree = "2222222222222222222222222222222222222222"
            release_checksum_sha256 = "3333333333333333333333333333333333333333333333333333333333333333"
        "#
        .parse()
        .unwrap();
        let wrong_wave_graph = json!({
            "schema_version": "jain.locked-cargo-graph/v1",
            "release": RELEASE,
            "repositories": [{"repository":"root","cargo_lock_sha256":digest('a')}],
            "edges": []
        });
        assert!(build_dag(&wrong_wave, digest('a'), &wrong_wave_graph, digest('b')).is_err());
        let cyclic: toml::Value = r#"
            rollout_wave_order = [1]
            [[repo]]
            name="a"
            rollout_wave=1
            cross_repo_deps=["b"]
            cargo_members=["."]
            current_tag="a-v9.0.0-distributed.1-split.1"
            release_commit="1111111111111111111111111111111111111111"
            release_tree="2222222222222222222222222222222222222222"
            release_checksum_sha256="3333333333333333333333333333333333333333333333333333333333333333"
            [[repo]]
            name="b"
            rollout_wave=1
            cross_repo_deps=["a"]
            cargo_members=["."]
            current_tag="b-v9.0.0-distributed.1-split.1"
            release_commit="1111111111111111111111111111111111111111"
            release_tree="2222222222222222222222222222222222222222"
            release_checksum_sha256="3333333333333333333333333333333333333333333333333333333333333333"
        "#
        .parse()
        .unwrap();
        let cyclic_graph = json!({
            "schema_version": "jain.locked-cargo-graph/v1",
            "release": RELEASE,
            "repositories": [
                {"repository":"a","cargo_lock_sha256":digest('a')},
                {"repository":"b","cargo_lock_sha256":digest('b')}
            ],
            "edges": [
                {"consumer":"a","dependency":"b"},
                {"consumer":"b","dependency":"a"}
            ]
        });
        assert!(build_dag(&cyclic, digest('b'), &cyclic_graph, digest('c')).is_err());
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
    fn authority_reads_and_evidence_outputs_reject_path_replacement_and_aliases() {
        let temp = TestDir::new("descriptor-custody");
        let authority = temp.0.join("authority.json");
        let moved = temp.0.join("authority-opened.json");
        fs::write(&authority, br#"{"status":"original"}"#).unwrap();
        let result = read_regular_bytes_with_hook(&authority, "test authority", || {
            fs::rename(&authority, &moved).unwrap();
            fs::write(&authority, br#"{"status":"replacement"}"#).unwrap();
        });
        assert!(result.is_err());

        let current = temp.0.join("current");
        let historical = temp.0.join("historical");
        let outside = temp.0.join("outside");
        fs::create_dir(&current).unwrap();
        fs::create_dir(&historical).unwrap();
        fs::create_dir(&outside).unwrap();
        fs::write(
            current.join("current.json"),
            br#"{"receipt_id":"new/descriptor"}"#,
        )
        .unwrap();
        fs::write(
            historical.join("old.json"),
            br#"{"receipt_id":"old/descriptor"}"#,
        )
        .unwrap();
        let alias = temp.0.join("current-alias");
        symlink(&current, &alias).unwrap();
        let output = alias.join("index.json");
        assert!(evidence_index_command(vec![
            "--release".to_owned(),
            RELEASE.to_owned(),
            "--root".to_owned(),
            current.display().to_string(),
            "--historical-root".to_owned(),
            historical.display().to_string(),
            "--json".to_owned(),
            output.display().to_string(),
            "--apply".to_owned(),
        ])
        .is_err());
        assert!(!current.join("index.json").exists());

        let safe_output = outside.join("index.json");
        evidence_index_command(vec![
            "--release".to_owned(),
            RELEASE.to_owned(),
            "--root".to_owned(),
            current.display().to_string(),
            "--historical-root".to_owned(),
            historical.display().to_string(),
            "--json".to_owned(),
            safe_output.display().to_string(),
            "--apply".to_owned(),
        ])
        .unwrap();
        assert!(safe_output.is_file());
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
            ("caddy-unchanged.v1.schema.json", "jain.caddy-unchanged/v1"),
            ("rollback-proof.v1.schema.json", "jain.rollback-proof/v1"),
            ("owner-signature.v1.schema.json", "jain.owner-signature/v1"),
            (
                "locked-cargo-graph.v1.schema.json",
                "jain.locked-cargo-graph/v1",
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
