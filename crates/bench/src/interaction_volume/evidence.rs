use std::path::Path;
use std::process::Command;

use super::*;
use crate::engine::{MAX_CONTAINER_WRITABLE_LAYER_BYTES, PostgresEndpoint, PostgresLiveIdentity};
use crate::report::collect_environment;

pub(super) const EXECUTION_EVIDENCE_SCHEMA: &str =
    "redline.interaction-volume-execution-evidence/v3";
const EVIDENCE_GENERATOR: &str = "ops/ci/interaction-volume-cert.sh";
const POSTGRES_DATA_DESTINATION: &str = "/var/lib/postgresql/data";
const RUN_ID_LABEL: &str = "redline.interaction-volume.run_id";
const POSTGRES_CONTAINER_ID_ENV: &str = "REDLINEDB_BENCH_POSTGRES_CONTAINER_ID";
const CI_TRIGGER_CONTRACT: &[u8] =
    include_bytes!("../../../../ops/ci/interaction-volume-trigger-contract.json");

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostgresExecutionEvidence {
    pub image_digest: String,
    pub digest_observation: String,
    pub digest_verified: bool,
    pub backend: String,
    pub isolation_verified: bool,
    pub dedicated_instance: bool,
    pub endpoint_scope: String,
    pub container_id: Option<String>,
    pub container_name: Option<String>,
    pub container_run_id: Option<String>,
    pub docker_daemon_id: Option<String>,
    pub endpoint_host: Option<String>,
    pub endpoint_port: Option<u16>,
    pub rootfs_read_only: bool,
    pub log_driver: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageContract {
    pub class: String,
    pub local_database_root: String,
    pub postgres_data_root: String,
    pub local_mount_identity: String,
    pub postgres_mount_identity: String,
    pub same_mount: bool,
    pub durable: bool,
    pub postgres_bind_mode: String,
    pub emergency_reserve_path: Option<String>,
    pub emergency_reserve_bytes: u64,
}

impl StorageContract {
    pub(super) fn comparison_eligible(&self, postgres: &PostgresExecutionEvidence) -> bool {
        self.class == "shared_host_durable_bind"
            && self.same_mount
            && self.durable
            && self.postgres_bind_mode == "rw"
            && self.local_mount_identity == self.postgres_mount_identity
            && self.emergency_reserve_bytes > 0
            && postgres.backend == "docker_bind"
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionEvidence {
    pub schema_version: String,
    pub generator: String,
    pub source_commit: String,
    pub source_dirty: bool,
    pub binary_sha256: String,
    pub postgres: PostgresExecutionEvidence,
    pub storage: StorageContract,
    pub ci_trigger: Option<CiTriggerEvidence>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CiTriggerEvidence {
    pub schema_version: String,
    pub pipeline_source: String,
    pub pipeline_id: String,
    pub job_id: String,
    pub project_id: String,
    pub runner_id: String,
    pub job_name: String,
    pub source_commit: String,
    pub project_path: String,
    pub server_url: String,
    pub project_url: String,
    pub pipeline_url: String,
    pub job_api_url: String,
    pub job_web_url: String,
    pub job_token_authenticated: bool,
    pub default_branch: String,
    pub commit_branch: String,
    pub commit_ref_protected: bool,
    pub scheduled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LivePostgresObservation {
    pub observed_at_unix_ms: u64,
    pub docker_daemon_id: String,
    pub container_id: String,
    pub container_name: String,
    pub container_run_id: String,
    pub container_image_id: String,
    pub container_image_reference: String,
    pub container_started_at: String,
    pub rootfs_read_only: bool,
    pub log_driver: String,
    pub container_size_rw_bytes: u64,
    pub data_mount_source: String,
    pub data_mount_destination: String,
    pub published_host_ip: String,
    pub published_host_port: u16,
    pub endpoint: PostgresEndpoint,
    pub server: PostgresLiveIdentity,
    pub local_mount_identity: String,
    pub postgres_mount_identity: String,
    pub emergency_reserve_allocated_bytes: u64,
}

#[derive(Debug, Clone)]
pub(super) struct ValidatedEvidence {
    pub document: ExecutionEvidence,
    pub sha256: String,
    pub source_observation_bound: bool,
    pub live_postgres: Option<LivePostgresObservation>,
    pub postgres_provenance_bound: bool,
    pub storage_comparison_eligible: bool,
    pub ci_trigger_bound: bool,
}

pub(super) fn load_execution_evidence(
    path: &Path,
    executable_sha256: &str,
    postgres_url: &str,
) -> Result<ValidatedEvidence> {
    let bytes =
        fs::read(path).with_context(|| format!("read execution evidence {}", path.display()))?;
    let document: ExecutionEvidence = serde_json::from_slice(&bytes)
        .with_context(|| format!("decode execution evidence {}", path.display()))?;
    if document.schema_version != EXECUTION_EVIDENCE_SCHEMA {
        bail!(
            "execution evidence schema must be {EXECUTION_EVIDENCE_SCHEMA}, got {}",
            document.schema_version
        );
    }
    if document.generator != EVIDENCE_GENERATOR {
        bail!(
            "execution evidence generator must be {EVIDENCE_GENERATOR}, got {}",
            document.generator
        );
    }
    if document.binary_sha256 != executable_sha256 {
        bail!(
            "execution evidence binary digest {} differs from running executable {}",
            document.binary_sha256,
            executable_sha256
        );
    }
    if document.postgres.image_digest != PINNED_POSTGRES_IMAGE_DIGEST {
        bail!(
            "execution evidence PostgreSQL digest differs from pinned {}",
            PINNED_POSTGRES_IMAGE_DIGEST
        );
    }
    validate_source_commit(&document.source_commit)?;
    let controlled_source = controlled_git_source();
    if let Some((commit, dirty)) = &controlled_source
        && (commit != &document.source_commit || dirty != &document.source_dirty)
    {
        bail!(
            "wrapper source evidence does not match the checked-out repository (commit {commit}, dirty {dirty})"
        );
    }

    let live_postgres = if document.postgres.backend == "docker_bind" {
        Some(observe_live_postgres(&document, postgres_url)?)
    } else {
        None
    };
    let postgres_provenance_bound = live_postgres.is_some();
    let storage_comparison_eligible =
        live_postgres.is_some() && document.storage.comparison_eligible(&document.postgres);
    let ci_trigger_bound =
        validate_ci_trigger(document.ci_trigger.as_ref(), &document.source_commit)?;
    Ok(ValidatedEvidence {
        sha256: format!("{:x}", Sha256::digest(&bytes)),
        document,
        source_observation_bound: controlled_source.is_some(),
        live_postgres,
        postgres_provenance_bound,
        storage_comparison_eligible,
        ci_trigger_bound,
    })
}

fn validate_ci_trigger(trigger: Option<&CiTriggerEvidence>, _source_commit: &str) -> Result<bool> {
    let Some(trigger) = trigger else {
        return Ok(false);
    };
    let contract: serde_json::Value = serde_json::from_slice(CI_TRIGGER_CONTRACT)
        .context("decode checked-in CI authority contract")?;
    let blocker = contract
        .pointer("/daily_blocker")
        .and_then(|value| value.as_str())
        .filter(|value| !value.is_empty())
        .context("checked-in CI authority contract lacks its blocker")?;
    let authority_unconfigured = contract.pointer("/schema_version").and_then(|v| v.as_str())
        == Some("redline.interaction-volume-trigger/v4")
        && contract
            .pointer("/authoritative_ci")
            .and_then(|v| v.as_str())
            == Some("host-native-jeryu-required")
        && contract.pointer("/daily_enabled").and_then(|v| v.as_bool()) == Some(false)
        && contract
            .pointer("/attestation_authority/kind")
            .and_then(|v| v.as_str())
            == Some("host_ci_jeryu_exact_head")
        && contract
            .pointer("/attestation_authority/canonical_https_origin")
            .is_some_and(serde_json::Value::is_null)
        && contract
            .pointer("/attestation_authority/ca_bundle")
            .is_some_and(serde_json::Value::is_null)
        && contract
            .pointer("/attestation_authority/ca_bundle_sha256")
            .is_some_and(serde_json::Value::is_null)
        && contract
            .pointer("/attestation_authority/signature_key_id")
            .is_some_and(serde_json::Value::is_null);
    if !authority_unconfigured {
        bail!(
            "checked-in CI authority contract is malformed or attempts an unauthenticated fallback"
        );
    }
    bail!(
        "daily certification is blocked before accepting trigger {}: {blocker}",
        trigger.schema_version
    )
}

pub(super) fn controlled_environment(evidence: &ValidatedEvidence) -> RunEnvironment {
    let mut environment = collect_environment();
    environment.git_sha = evidence
        .source_observation_bound
        .then(|| evidence.document.source_commit.clone());
    environment.git_dirty = evidence
        .source_observation_bound
        .then_some(evidence.document.source_dirty);
    // The generic environment accepts caller-provided image metadata. This certificate records
    // image identity only through the live Docker observation above.
    environment.image_digest = None;
    environment
}

fn observe_live_postgres(
    document: &ExecutionEvidence,
    postgres_url: &str,
) -> Result<LivePostgresObservation> {
    let expected = &document.postgres;
    let container_id = required_evidence(expected.container_id.as_deref(), "container id")?;
    validate_hex_id(container_id, "container id")?;
    if std::env::var(POSTGRES_CONTAINER_ID_ENV).as_deref() != Ok(container_id) {
        bail!("PostgreSQL storage-accounting container differs from execution evidence");
    }
    let container_name = required_evidence(expected.container_name.as_deref(), "container name")?;
    let container_run_id =
        required_evidence(expected.container_run_id.as_deref(), "container run id")?;
    validate_hex_id(container_run_id, "container run id")?;
    let expected_daemon =
        required_evidence(expected.docker_daemon_id.as_deref(), "Docker daemon id")?;
    let endpoint_host = required_evidence(expected.endpoint_host.as_deref(), "endpoint host")?;
    let endpoint_port = expected
        .endpoint_port
        .context("execution evidence lacks endpoint port")?;

    let daemon_id = docker_daemon_id()?;
    if daemon_id != expected_daemon {
        bail!(
            "live Docker daemon id {daemon_id} differs from execution evidence {expected_daemon}"
        );
    }
    let inspect = docker_json(&["inspect", "--size", container_id])?;
    let container = inspect
        .as_array()
        .and_then(|values| values.first())
        .context("docker inspect returned no container")?;
    let live_id = json_string(container, "/Id")?;
    if live_id != container_id {
        bail!("live container id {live_id} differs from evidence {container_id}");
    }
    let live_name = json_string(container, "/Name")?.trim_start_matches('/');
    if live_name != container_name {
        bail!("live container name {live_name} differs from evidence {container_name}");
    }
    if container
        .pointer("/State/Running")
        .and_then(|value| value.as_bool())
        != Some(true)
    {
        bail!("certification PostgreSQL container is not running");
    }
    if json_string(container, "/State/Health/Status")? != "healthy" {
        bail!("certification PostgreSQL container is not healthy");
    }
    let started_at = json_string(container, "/State/StartedAt")?.to_owned();
    let rootfs_read_only = container
        .pointer("/HostConfig/ReadonlyRootfs")
        .and_then(|value| value.as_bool())
        .context("Docker observation lacks read-only-rootfs state")?;
    let log_driver = json_string(container, "/HostConfig/LogConfig/Type")?.to_owned();
    let size_rw = container
        .pointer("/SizeRw")
        .and_then(|value| value.as_i64())
        .context("Docker size observation lacks writable-layer bytes")?;
    let container_size_rw_bytes =
        u64::try_from(size_rw).context("Docker writable-layer size was negative")?;
    if !expected.rootfs_read_only
        || expected.log_driver != "none"
        || !rootfs_read_only
        || log_driver != "none"
        || container_size_rw_bytes > MAX_CONTAINER_WRITABLE_LAYER_BYTES
    {
        bail!(
            "PostgreSQL container must have a read-only bounded rootfs layer and disabled logs \
             (evidence rootfs_read_only={}, log_driver={}; live rootfs_read_only={rootfs_read_only}, \
             log_driver={log_driver}, size_rw={container_size_rw_bytes}, \
             max_size_rw={MAX_CONTAINER_WRITABLE_LAYER_BYTES})",
            expected.rootfs_read_only,
            expected.log_driver
        );
    }
    let image_id = json_string(container, "/Image")?.to_owned();
    validate_hex_id(
        image_id.strip_prefix("sha256:").unwrap_or(&image_id),
        "image id",
    )?;
    let image_reference = json_string(container, "/Config/Image")?.to_owned();
    if !image_reference.ends_with(PINNED_POSTGRES_IMAGE_DIGEST) {
        bail!("live container image reference is not digest-pinned: {image_reference}");
    }
    let live_run_id = container
        .pointer(&format!("/Config/Labels/{RUN_ID_LABEL}"))
        .and_then(|value| value.as_str())
        .context("live container lacks certification run-id label")?;
    if live_run_id != container_run_id {
        bail!("live container run id differs from execution evidence");
    }
    validate_image_digest(&image_id)?;

    let mount = container
        .pointer("/Mounts")
        .and_then(|value| value.as_array())
        .and_then(|mounts| {
            mounts.iter().find(|mount| {
                mount
                    .pointer("/Destination")
                    .and_then(|value| value.as_str())
                    == Some(POSTGRES_DATA_DESTINATION)
            })
        })
        .context("live container lacks PostgreSQL data mount")?;
    if json_string(mount, "/Type")? != "bind"
        || mount.pointer("/RW").and_then(|value| value.as_bool()) != Some(true)
    {
        bail!("PostgreSQL data mount must be a read-write bind");
    }
    let mount_source = canonical_string(Path::new(json_string(mount, "/Source")?))?;
    let evidence_postgres_root = canonical_string(Path::new(&document.storage.postgres_data_root))?;
    if mount_source != evidence_postgres_root {
        bail!(
            "live PostgreSQL bind source {mount_source} differs from evidence {evidence_postgres_root}"
        );
    }

    let binding = container
        .pointer("/NetworkSettings/Ports/5432~1tcp")
        .and_then(|value| value.as_array())
        .and_then(|values| values.first())
        .context("live container lacks a published PostgreSQL port")?;
    let host_ip = json_string(binding, "/HostIp")?.to_owned();
    let host_port = json_string(binding, "/HostPort")?
        .parse::<u16>()
        .context("live container published an invalid PostgreSQL port")?;
    let endpoint = PostgresEngine::endpoint(postgres_url)?;
    if endpoint.host != endpoint_host
        || endpoint.port != endpoint_port
        || endpoint.port != host_port
    {
        bail!(
            "PostgreSQL DSN {}:{} is not the live container endpoint {endpoint_host}:{host_port} (evidence port {endpoint_port})",
            endpoint.host,
            endpoint.port
        );
    }
    validate_published_host(&host_ip, &endpoint.host)?;

    let local_root = canonical_string(Path::new(&document.storage.local_database_root))?;
    let local_mount_identity = mount_identity(Path::new(&local_root))?;
    let postgres_mount_identity = mount_identity(Path::new(&mount_source))?;
    if local_mount_identity != document.storage.local_mount_identity
        || postgres_mount_identity != document.storage.postgres_mount_identity
        || local_mount_identity != postgres_mount_identity
    {
        bail!("live storage mounts differ from the execution-evidence contract");
    }
    let reserve_path = document
        .storage
        .emergency_reserve_path
        .as_deref()
        .context("shared storage evidence lacks an emergency reserve")?;
    let reserve_path = canonical_string(Path::new(reserve_path))?;
    let reserve_mount = mount_identity(Path::new(&reserve_path))?;
    if reserve_mount != local_mount_identity {
        bail!("emergency reserve is not on the certification storage mount");
    }
    let reserve_allocated = allocated_bytes(Path::new(&reserve_path))?;
    if reserve_allocated < document.storage.emergency_reserve_bytes {
        bail!(
            "emergency reserve allocated {reserve_allocated} bytes, expected at least {}",
            document.storage.emergency_reserve_bytes
        );
    }

    let server = PostgresEngine::live_identity(postgres_url)?;
    if server.database != "redline_cert"
        || server.server_port != 5432
        || server.data_directory != POSTGRES_DATA_DESTINATION
    {
        bail!("live PostgreSQL identity is not the owned certification instance: {server:?}");
    }
    Ok(LivePostgresObservation {
        observed_at_unix_ms: unix_millis(),
        docker_daemon_id: daemon_id,
        container_id: live_id.to_owned(),
        container_name: live_name.to_owned(),
        container_run_id: live_run_id.to_owned(),
        container_image_id: image_id,
        container_image_reference: image_reference,
        container_started_at: started_at,
        rootfs_read_only,
        log_driver,
        container_size_rw_bytes,
        data_mount_source: mount_source,
        data_mount_destination: POSTGRES_DATA_DESTINATION.to_owned(),
        published_host_ip: host_ip,
        published_host_port: host_port,
        endpoint,
        server,
        local_mount_identity,
        postgres_mount_identity,
        emergency_reserve_allocated_bytes: reserve_allocated,
    })
}

fn validate_image_digest(image_id: &str) -> Result<()> {
    let inspect = docker_json(&["image", "inspect", image_id])?;
    let image = inspect
        .as_array()
        .and_then(|values| values.first())
        .context("docker image inspect returned no image")?;
    let matches = image
        .pointer("/RepoDigests")
        .and_then(|value| value.as_array())
        .is_some_and(|digests| {
            digests.iter().any(|digest| {
                digest
                    .as_str()
                    .is_some_and(|digest| digest.ends_with(PINNED_POSTGRES_IMAGE_DIGEST))
            })
        });
    if !matches {
        bail!("live container image id is not bound to the pinned PostgreSQL RepoDigest");
    }
    Ok(())
}

fn validate_published_host(host_ip: &str, endpoint_host: &str) -> Result<()> {
    if host_ip == "127.0.0.1" {
        let address = endpoint_host
            .parse::<std::net::IpAddr>()
            .context("loopback-published PostgreSQL endpoint must use a literal loopback IP")?;
        if !address.is_loopback() {
            bail!("loopback-published PostgreSQL endpoint is not loopback");
        }
        return Ok(());
    }
    if host_ip != "0.0.0.0" {
        bail!("unsupported Docker PostgreSQL published address {host_ip}");
    }
    let docker_host = std::env::var("DOCKER_HOST").context(
        "non-loopback Docker publication requires a controlled DOCKER_HOST service endpoint",
    )?;
    let daemon_host = docker_host
        .strip_prefix("tcp://")
        .and_then(|value| value.rsplit_once(':').map(|(host, _)| host))
        .context("DOCKER_HOST must be an explicit tcp://host:port endpoint")?;
    if daemon_host != endpoint_host {
        bail!(
            "PostgreSQL endpoint host {endpoint_host} differs from Docker daemon host {daemon_host}"
        );
    }
    Ok(())
}

fn docker_daemon_id() -> Result<String> {
    let output = command_text("docker", &["info", "--format", "{{json .ID}}"])?;
    let id: String = serde_json::from_str(&output).context("decode Docker daemon id")?;
    if id.trim().is_empty() {
        bail!("Docker daemon returned an empty id");
    }
    Ok(id)
}

fn docker_json(args: &[&str]) -> Result<serde_json::Value> {
    let output = command_text("docker", args)?;
    serde_json::from_str(&output).with_context(|| format!("decode docker {} JSON", args.join(" ")))
}

fn command_text(program: &str, args: &[&str]) -> Result<String> {
    let output = Command::new(program)
        .args(args)
        .output()
        .with_context(|| format!("execute controlled {program} observation"))?;
    if !output.status.success() {
        bail!(
            "controlled {} {} observation failed: {}",
            program,
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    String::from_utf8(output.stdout)
        .context("controlled observation returned non-UTF8 output")
        .map(|value| value.trim().to_owned())
}

fn mount_identity(path: &Path) -> Result<String> {
    let path = path
        .to_str()
        .context("storage path is not valid UTF-8 for controlled mount observation")?;
    let device = command_text("stat", &["-c", "%d", path])?;
    let filesystem = command_text("stat", &["-f", "-c", "%T", path])?;
    let mount = command_text(
        "findmnt",
        &["-T", path, "-n", "-o", "MAJ:MIN,FSTYPE,SOURCE,OPTIONS"],
    )?;
    Ok(format!("device={device};fstype={filesystem};mount={mount}"))
}

fn allocated_bytes(path: &Path) -> Result<u64> {
    let path = path
        .to_str()
        .context("reserve path is not valid UTF-8 for controlled observation")?;
    let blocks = command_text("stat", &["-c", "%b", path])?
        .parse::<u64>()
        .context("emergency reserve block count is invalid")?;
    blocks
        .checked_mul(512)
        .context("emergency reserve allocated-byte count overflowed")
}

fn json_string<'a>(value: &'a serde_json::Value, pointer: &str) -> Result<&'a str> {
    value
        .pointer(pointer)
        .and_then(|value| value.as_str())
        .with_context(|| format!("Docker observation lacks string {pointer}"))
}

fn canonical_string(path: &Path) -> Result<String> {
    fs::canonicalize(path)
        .with_context(|| format!("canonicalize live storage path {}", path.display()))?
        .into_os_string()
        .into_string()
        .map_err(|_| anyhow!("live storage path is not valid UTF-8"))
}

fn required_evidence<'a>(value: Option<&'a str>, field: &str) -> Result<&'a str> {
    value
        .filter(|value| !value.trim().is_empty())
        .with_context(|| format!("execution evidence lacks {field}"))
}

fn validate_hex_id(value: &str, field: &str) -> Result<()> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        bail!("{field} must be exactly 64 hexadecimal characters");
    }
    Ok(())
}

fn validate_source_commit(commit: &str) -> Result<()> {
    if commit.len() != 40 || !commit.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        bail!("execution evidence source commit must be a full 40-character hexadecimal SHA");
    }
    Ok(())
}

fn controlled_git_source() -> Option<(String, bool)> {
    let commit = git_output(&["rev-parse", "HEAD"])?;
    validate_source_commit(&commit).ok()?;
    let status = Command::new("git")
        .args(["status", "--porcelain", "--untracked-files=normal"])
        .output()
        .ok()?;
    if !status.status.success() {
        return None;
    }
    Some((commit, !status.stdout.is_empty()))
}

fn git_output(args: &[&str]) -> Option<String> {
    let output = Command::new("git").args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8(output.stdout).ok()?.trim().to_owned();
    (!value.is_empty()).then_some(value)
}

#[cfg(test)]
mod tests {
    use super::{CiTriggerEvidence, validate_ci_trigger};

    #[test]
    fn release_trigger_is_absent_or_fail_closed_when_malformed() {
        let source = "a".repeat(40);
        assert!(!validate_ci_trigger(None, &source).unwrap());
        let malformed = CiTriggerEvidence {
            schema_version: "redline.interaction-volume-ci-trigger/v2".to_owned(),
            pipeline_source: "schedule".to_owned(),
            pipeline_id: "0".to_owned(),
            job_id: "2".to_owned(),
            project_id: "3".to_owned(),
            runner_id: "4".to_owned(),
            job_name: "not-the-daily-job".to_owned(),
            source_commit: source.clone(),
            project_path: "jeryu/redline".to_owned(),
            server_url: "http://forge".to_owned(),
            project_url: "http://forge/jeryu/redline".to_owned(),
            pipeline_url: "http://forge/jeryu/redline/-/pipelines/0".to_owned(),
            job_api_url: "http://forge/api/v4/job".to_owned(),
            job_web_url: "http://forge/jeryu/redline/-/jobs/2".to_owned(),
            job_token_authenticated: true,
            default_branch: "main".to_owned(),
            commit_branch: "main".to_owned(),
            commit_ref_protected: true,
            scheduled: true,
        };
        let error = validate_ci_trigger(Some(&malformed), &source).unwrap_err();
        assert!(
            error.to_string().contains("daily certification is blocked"),
            "{error:#}"
        );

        let direct_fabrication = CiTriggerEvidence {
            schema_version: "redline.interaction-volume-ci-trigger/v2".to_owned(),
            pipeline_source: "schedule".to_owned(),
            pipeline_id: "1".to_owned(),
            job_id: "2".to_owned(),
            project_id: "3".to_owned(),
            runner_id: "4".to_owned(),
            job_name: "interaction-volume-daily".to_owned(),
            source_commit: source.clone(),
            project_path: "jeryu/redline-core".to_owned(),
            server_url: "http://forge".to_owned(),
            project_url: "http://forge/jeryu/redline-core".to_owned(),
            pipeline_url: "http://forge/jeryu/redline-core/-/pipelines/1".to_owned(),
            job_api_url: "http://forge/api/v4/job".to_owned(),
            job_web_url: "http://forge/jeryu/redline-core/-/jobs/2".to_owned(),
            job_token_authenticated: true,
            default_branch: "main".to_owned(),
            commit_branch: "main".to_owned(),
            commit_ref_protected: true,
            scheduled: true,
        };
        let error = validate_ci_trigger(Some(&direct_fabrication), &source).unwrap_err();
        assert!(
            error.to_string().contains("daily certification is blocked"),
            "{error:#}"
        );

        let mut fork = direct_fabrication.clone();
        fork.project_path = "customer/redline-core".to_owned();
        fork.project_url = "http://forge/customer/redline-core".to_owned();
        fork.pipeline_url = "http://forge/customer/redline-core/-/pipelines/1".to_owned();
        let error = validate_ci_trigger(Some(&fork), &source).unwrap_err();
        assert!(
            error.to_string().contains("daily certification is blocked"),
            "{error:#}"
        );
    }
}
