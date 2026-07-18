use super::*;
use std::{
    collections::BTreeSet,
    ffi::CString,
    io::{Read, Write},
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::{
            ffi::OsStrExt,
            fs::{MetadataExt, OpenOptionsExt},
        },
    },
};

const SCHEMA: &str = "jain.release-flow/v1";
const MAX_INPUT_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Default)]
struct FlowArgs {
    manifest: Option<PathBuf>,
    evidence_root: Option<PathBuf>,
    cloud_spec: Option<PathBuf>,
    token_file: Option<PathBuf>,
    resume: Option<PathBuf>,
    record: Option<PathBuf>,
}

#[derive(Clone)]
struct EvidenceFile {
    value: JsonValue,
    digest: String,
    modified: SystemTime,
    refs: Vec<JsonValue>,
}

pub(super) fn command(args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    reject_live_push_environment()?;
    let args = parse_args(args)?;
    let manifest = args.manifest.ok_or("release-flow requires --manifest")?;
    let evidence_root = args
        .evidence_root
        .ok_or("release-flow requires --evidence-root")?;
    let evidence_root = physical_evidence_root(&evidence_root)?;
    let manifest_bytes = read_stable_regular_file(&manifest, MAX_INPUT_BYTES, false)?;
    let manifest_hash = sha256_bytes(&manifest_bytes);
    let data: toml::Value = std::str::from_utf8(&manifest_bytes)?.parse()?;
    validate_release_paths(&manifest, &evidence_root, &data)?;

    let (control_plane, mut control_failures) = control_plane_identity(&manifest, &data);
    let previous_receipt = if let Some(path) = args.resume.as_deref() {
        let (bytes, digest) = read_evidence_receipt(path, &evidence_root)?;
        let prior: JsonValue = serde_json::from_slice(&bytes)?;
        verify_resume_receipt(
            &prior,
            &manifest,
            &manifest_hash,
            &control_plane,
            &evidence_root,
        )?;
        json!({"path": path, "sha256": digest})
    } else {
        json!({"path": JsonValue::Null, "sha256": JsonValue::Null})
    };

    let mut gates = Vec::new();
    gates.push(candidate_gate(&data));
    gates.push(manifest_gate(&data, &manifest));
    gates.push(gate(
        "control_plane",
        if control_failures.is_empty() {
            "pass"
        } else {
            "blocked"
        },
        control_plane.clone(),
        Vec::new(),
        std::mem::take(&mut control_failures),
    ));

    let managed = managed_repositories(&data, &manifest);
    gates.push(repository_gate(&managed));
    gates.push(redline_gate(&data, &evidence_root));
    gates.push(forge_gate(&managed, args.token_file.as_deref()));
    gates.push(cloud_gate(args.cloud_spec.as_deref()));

    let overall_status = overall_status(&gates);
    let next_action = next_action(&gates, overall_status, args.record.is_some());
    let mut report = json!({
        "schema_version": SCHEMA,
        "release": RELEASE_VERSION,
        "mode": "read-only",
        "manifest": {"path": manifest, "sha256": manifest_hash},
        "control_plane": control_plane,
        "previous_receipt": previous_receipt,
        "gates": gates,
        "overall_status": overall_status,
        "next_action": next_action,
        "external_state_changed": false,
        "receipt_integrity_sha256": JsonValue::Null,
    });
    seal_report(&mut report)?;
    let mut bytes = serde_json::to_vec_pretty(&report)?;
    bytes.push(b'\n');
    if let Some(path) = args.record.as_deref() {
        record_receipt(path, &evidence_root, &bytes)?;
    }
    io::stdout().write_all(&bytes)?;
    Ok(())
}

fn reject_live_push_environment() -> Result<(), Box<dyn std::error::Error>> {
    match env::var_os("ATOMICSOUL_PUSH") {
        None => Ok(()),
        Some(value) if value == "0" => Ok(()),
        Some(_) => Err("release-flow refuses nonzero ATOMICSOUL_PUSH".into()),
    }
}

fn parse_args(args: Vec<String>) -> Result<FlowArgs, Box<dyn std::error::Error>> {
    fn assign(
        slot: &mut Option<PathBuf>,
        value: Option<String>,
        flag: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if slot.is_some() {
            return Err(format!("{flag} may be provided only once").into());
        }
        *slot = Some(PathBuf::from(
            value.ok_or_else(|| format!("{flag} needs a path"))?,
        ));
        Ok(())
    }

    let mut parsed = FlowArgs::default();
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--manifest" => assign(&mut parsed.manifest, iter.next(), "--manifest")?,
            "--evidence-root" => assign(&mut parsed.evidence_root, iter.next(), "--evidence-root")?,
            "--cloud-spec" => assign(&mut parsed.cloud_spec, iter.next(), "--cloud-spec")?,
            "--token-file" => assign(&mut parsed.token_file, iter.next(), "--token-file")?,
            "--resume" => assign(&mut parsed.resume, iter.next(), "--resume")?,
            "--record" => assign(&mut parsed.record, iter.next(), "--record")?,
            value => return Err(format!("unknown release-flow argument: {value}").into()),
        }
    }
    Ok(parsed)
}

fn validate_release_paths(
    manifest: &Path,
    evidence_root: &Path,
    data: &toml::Value,
) -> Result<(), Box<dyn std::error::Error>> {
    if !manifest.is_absolute() || fs::canonicalize(manifest)? != manifest {
        return Err("release-flow manifest must be an absolute physical path".into());
    }
    let authority = string(data, "manifest_authority").ok_or("manifest_authority is missing")?;
    if Path::new(&authority) != manifest {
        return Err("--manifest differs from manifest_authority".into());
    }
    let control = manifest
        .parent()
        .ok_or("manifest has no control-plane parent")?;
    let expected_evidence = control.join("docs/release-evidence").join(RELEASE_VERSION);
    if evidence_root != expected_evidence {
        return Err("--evidence-root differs from the authority release evidence root".into());
    }
    Ok(())
}

fn physical_evidence_root(path: &Path) -> Result<PathBuf, Box<dyn std::error::Error>> {
    if !path.is_absolute() {
        return Err("--evidence-root must be absolute".into());
    }
    let canonical = fs::canonicalize(path)?;
    let metadata = fs::symlink_metadata(path)?;
    if canonical != path || !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err("--evidence-root must be an existing physical directory".into());
    }
    Ok(canonical)
}

fn read_stable_regular_file(
    path: &Path,
    max_bytes: u64,
    private_mode: bool,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    if !path.is_absolute() || fs::canonicalize(path)? != path {
        return Err(format!("input is not an absolute physical path: {}", path.display()).into());
    }
    let mut file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)?;
    let before = file.metadata()?;
    let path_before = fs::symlink_metadata(path)?;
    if !before.is_file()
        || path_before.file_type().is_symlink()
        || before.nlink() != 1
        || before.len() > max_bytes
        || (before.dev(), before.ino()) != (path_before.dev(), path_before.ino())
        || (private_mode && before.mode() & 0o777 != 0o600)
    {
        return Err(format!("unsafe regular-file input: {}", path.display()).into());
    }
    let mut bytes = Vec::with_capacity(before.len() as usize);
    file.read_to_end(&mut bytes)?;
    let after = file.metadata()?;
    let path_after = fs::symlink_metadata(path)?;
    if (before.dev(), before.ino(), before.len()) != (after.dev(), after.ino(), after.len())
        || (after.dev(), after.ino()) != (path_after.dev(), path_after.ino())
        || bytes.len() as u64 != after.len()
    {
        return Err(format!("input changed while being read: {}", path.display()).into());
    }
    Ok(bytes)
}

fn sha256_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn gate(
    id: &str,
    status: &str,
    inputs: JsonValue,
    receipt_refs: Vec<JsonValue>,
    failures: Vec<String>,
) -> JsonValue {
    json!({
        "id": id,
        "status": status,
        "inputs": inputs,
        "receipt_refs": receipt_refs,
        "failures": failures,
    })
}

fn candidate_gate(data: &toml::Value) -> JsonValue {
    let inputs = json!({
        "release": string(data, "release_version"),
        "status": string(data, "status"),
        "formal_ga": data.get("formal_ga").and_then(toml::Value::as_bool),
        "sagemaker": string(data, "sagemaker"),
        "rollback_target": string(data, "rollback_target"),
    });
    let mut failures = Vec::new();
    if string(data, "release_version").as_deref() != Some(RELEASE_VERSION) {
        failures.push(format!("release_version must be {RELEASE_VERSION}"));
    }
    if string(data, "status").as_deref() != Some(RELEASE_STATUS) {
        failures.push(format!("status must be {RELEASE_STATUS}"));
    }
    if data.get("formal_ga").and_then(toml::Value::as_bool) != Some(false) {
        failures.push("formal_ga must remain false".to_owned());
    }
    if string(data, "sagemaker").as_deref() != Some("N/A") {
        failures.push("sagemaker must be N/A".to_owned());
    }
    if string(data, "rollback_target").as_deref() != Some(ROLLBACK_TARGET) {
        failures.push(format!("rollback_target must be {ROLLBACK_TARGET}"));
    }
    gate(
        "candidate_invariants",
        if failures.is_empty() {
            "pass"
        } else {
            "blocked"
        },
        inputs,
        Vec::new(),
        failures,
    )
}

fn manifest_gate(data: &toml::Value, manifest: &Path) -> JsonValue {
    let mut failures = Vec::new();
    if let Err(error) = validate_manifest_data(data, manifest, true) {
        failures.push(error.to_string());
    }
    if let Ok(rows) = manifest_repos(data) {
        for row in rows {
            let name = string(row, "name").unwrap_or_else(|| "<unnamed>".to_owned());
            if string(row, "identity_status").as_deref() != Some("bound") {
                failures.push(format!("{name}: release identity is not bound"));
            }
            if declared_release_tag(row).is_none() {
                failures.push(format!("{name}: immutable release tag is not bound"));
            }
        }
    }
    for (label, row) in [
        ("control_plane", data.get("control_plane")),
        (
            "external_dependencies.redline",
            data.get("external_dependencies")
                .and_then(|value| value.get("redline")),
        ),
    ] {
        match row {
            Some(row) if string(row, "identity_status").as_deref() == Some("bound") => {}
            Some(_) => failures.push(format!("{label}: release identity is not bound")),
            None => failures.push(format!("{label}: authority row is missing")),
        }
    }
    gate(
        "authority_manifest",
        if failures.is_empty() {
            "pass"
        } else {
            "blocked"
        },
        json!({"derived_bytes_checked": true}),
        Vec::new(),
        failures,
    )
}

fn control_plane_identity(manifest: &Path, data: &toml::Value) -> (JsonValue, Vec<String>) {
    let mut failures = Vec::new();
    let control = manifest.parent().unwrap_or_else(|| Path::new("/"));
    let split_root = string(data, "split_root")
        .map(PathBuf::from)
        .unwrap_or_default();
    if let Err(error) = validate_physical_git_checkout_beneath(control, &split_root) {
        failures.push(error.to_string());
    }
    let mut git_value = |args: &[&str]| match secure_git_output(Some(control), args) {
        Ok(value) => value,
        Err(error) => {
            failures.push(error.to_string());
            String::new()
        }
    };
    let head = git_value(&["rev-parse", "--verify", "HEAD^{commit}"]);
    let tree = git_value(&["rev-parse", "--verify", "HEAD^{tree}"]);
    let branch = git_value(&["branch", "--show-current"]);
    let output = secure_git_command(Some(control))
        .args([
            "status",
            "--porcelain=v2",
            "--branch",
            "--untracked-files=all",
        ])
        .output();
    let (status_sha256, clean) = match output {
        Ok(output) if output.status.success() => {
            let clean = output
                .stdout
                .split(|byte| *byte == b'\n')
                .filter(|line| !line.is_empty())
                .all(|line| line.starts_with(b"# "));
            (sha256_bytes(&output.stdout), clean)
        }
        Ok(_) => {
            failures.push("cannot read control-plane porcelain status".to_owned());
            (sha256_bytes(&[]), false)
        }
        Err(error) => {
            failures.push(format!("cannot run control-plane status: {error}"));
            (sha256_bytes(&[]), false)
        }
    };
    if branch != "main" {
        failures.push(format!("control-plane branch is {branch:?}, expected main"));
    }
    if !clean {
        failures.push("control-plane checkout is dirty".to_owned());
    }
    (
        json!({
            "head": head,
            "tree": tree,
            "branch": branch,
            "status_sha256": status_sha256,
            "clean": clean,
        }),
        failures,
    )
}

fn repository_gate(managed: &Result<Vec<ManagedRepo>, Box<dyn std::error::Error>>) -> JsonValue {
    let mut failures = Vec::new();
    let mut pass_count = 0usize;
    let count = match managed {
        Ok(repositories) => {
            for repo in repositories {
                let mut repo_failures = verify_local_repository(repo);
                if repo.tag.is_none() {
                    repo_failures.push("immutable tag is not bound".to_owned());
                }
                if repo_failures.is_empty() {
                    pass_count += 1;
                } else {
                    for failure in repo_failures {
                        failures.push(format!("{}: {failure}", repo.name));
                    }
                }
            }
            repositories.len()
        }
        Err(error) => {
            failures.push(error.to_string());
            0
        }
    };
    gate(
        "repository_state",
        if failures.is_empty() {
            "pass"
        } else {
            "blocked"
        },
        json!({"repository_count": count, "passing_repository_count": pass_count}),
        Vec::new(),
        failures,
    )
}

fn verify_local_repository(repo: &ManagedRepo) -> Vec<String> {
    let mut failures = Vec::new();
    let branch = secure_git_output(Some(&repo.path), &["branch", "--show-current"])
        .map_err(|error| failures.push(error.to_string()))
        .ok();
    if branch.as_deref() != Some(repo.branch.as_str()) {
        failures.push(format!("branch is {branch:?}, expected {}", repo.branch));
    }
    let head = secure_git_output(
        Some(&repo.path),
        &["rev-parse", "--verify", "HEAD^{commit}"],
    )
    .map_err(|error| failures.push(error.to_string()))
    .ok();
    match secure_git_output(
        Some(&repo.path),
        &["status", "--porcelain", "--untracked-files=all"],
    ) {
        Ok(status) if status.is_empty() => {}
        Ok(_) => failures.push("checkout is dirty".to_owned()),
        Err(error) => failures.push(error.to_string()),
    }
    let fetch = secure_git_output(Some(&repo.path), &["remote", "get-url", "--all", "origin"])
        .map_err(|error| failures.push(error.to_string()))
        .ok();
    let push = secure_git_output(
        Some(&repo.path),
        &["remote", "get-url", "--push", "--all", "origin"],
    )
    .map_err(|error| failures.push(error.to_string()))
    .ok();
    if fetch.as_deref() != Some(repo.remote.as_str())
        || push.as_deref() != Some(repo.remote.as_str())
    {
        failures.push("origin fetch and push endpoints differ from authority".to_owned());
    }
    let dot_git = repo.path.join(".git");
    match fs::symlink_metadata(&dot_git) {
        Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {}
        _ => failures.push("checkout does not own a physical .git directory".to_owned()),
    }
    for forbidden in [
        dot_git.join("commondir"),
        dot_git.join("worktrees"),
        dot_git.join("objects/info/alternates"),
    ] {
        if fs::symlink_metadata(&forbidden).is_ok() {
            failures.push(format!(
                "forbidden auxiliary Git metadata exists: {}",
                forbidden.display()
            ));
        }
    }
    if fs::canonicalize(&repo.path).ok().as_deref() != Some(repo.path.as_path()) {
        failures.push("checkout path is not canonical and physical".to_owned());
    }
    if let Some(tag) = repo.tag.as_deref() {
        let tag_ref = format!("refs/tags/{tag}^{{commit}}");
        match secure_git_output(Some(&repo.path), &["rev-parse", "--verify", &tag_ref]) {
            Ok(local) if head.as_deref() == Some(local.as_str()) => {}
            Ok(_) => failures.push(format!(
                "local immutable tag {tag} does not resolve to HEAD"
            )),
            Err(error) => failures.push(error.to_string()),
        }
    }
    failures
}

fn redline_gate(data: &toml::Value, evidence_root: &Path) -> JsonValue {
    let mut failures = external_dependency_failures(data);
    let split_root = string(data, "split_root")
        .map(PathBuf::from)
        .unwrap_or_default();
    let authoritative_lock = split_root.join("jain-redline/redline-split-ops/redline.lock.toml");
    let mirror_lock = split_root.join("jain-redline/redline.lock.toml");
    let mut lock_sha256 = JsonValue::Null;
    let mut lock_engine_commit = None;
    match (
        read_stable_regular_file(&authoritative_lock, MAX_INPUT_BYTES, false),
        read_stable_regular_file(&mirror_lock, MAX_INPUT_BYTES, false),
    ) {
        (Ok(authority), Ok(mirror)) if authority == mirror => {
            lock_sha256 = json!(sha256_bytes(&authority));
            let lock = std::str::from_utf8(&authority)
                .ok()
                .and_then(|text| text.parse::<toml::Value>().ok());
            lock_engine_commit = lock
                .as_ref()
                .and_then(|value| string(value, "engine_commit"));
            if lock
                .as_ref()
                .and_then(|value| value.get("proof"))
                .and_then(|proof| proof.get("cutover_eligible"))
                .and_then(toml::Value::as_bool)
                != Some(true)
            {
                failures.push("Redline proof lock is not cutover_eligible".to_owned());
            }
        }
        (Ok(_), Ok(_)) => failures.push("Redline lock mirror differs from authority".to_owned()),
        _ => failures.push("Redline authority and mirror locks are not physical files".to_owned()),
    }

    let mut family_receipts = Vec::new();
    let mut consumers: std::collections::BTreeMap<String, Vec<EvidenceFile>> =
        std::collections::BTreeMap::new();

    let mut files = match fs::read_dir(evidence_root) {
        Ok(entries) => entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension() == Some(OsStr::new("json")))
            .collect::<Vec<_>>(),
        Err(error) => {
            failures.push(format!("cannot list Redline evidence root: {error}"));
            Vec::new()
        }
    };
    files.sort();
    for path in files {
        let bytes = match read_stable_regular_file(&path, MAX_INPUT_BYTES, false) {
            Ok(bytes) => bytes,
            Err(_) => continue,
        };
        let value: JsonValue = match serde_json::from_slice(&bytes) {
            Ok(value) => value,
            Err(_) => continue,
        };
        let schema = value
            .get("schema_version")
            .and_then(JsonValue::as_str)
            .unwrap_or_default()
            .to_owned();
        if schema != "redline.family-ci/v1" && schema != "redline.consumer-evidence/v1" {
            continue;
        }
        let digest = sha256_bytes(&bytes);
        if let Err(error) = validate_digest_sidecar(&path, &digest) {
            failures.push(error.to_string());
            continue;
        }
        let modified = fs::metadata(&path)
            .and_then(|metadata| metadata.modified())
            .unwrap_or(UNIX_EPOCH);
        let mut evidence_refs = vec![json!({"path": path, "sha256": digest})];
        let sidecar = PathBuf::from(format!("{}.sha256", path.display()));
        if let Ok(sidecar_bytes) = read_stable_regular_file(&sidecar, 4096, false) {
            evidence_refs.push(json!({
                "path": sidecar,
                "sha256": sha256_bytes(&sidecar_bytes),
            }));
        }
        let evidence = EvidenceFile {
            value,
            digest,
            modified,
            refs: evidence_refs,
        };
        if schema == "redline.family-ci/v1" {
            family_receipts.push(evidence);
        } else if let Some(consumer) = evidence
            .value
            .get("consumer")
            .and_then(JsonValue::as_str)
            .map(str::to_owned)
        {
            consumers.entry(consumer).or_default().push(evidence);
        }
    }

    let mut selected = None;
    for family in family_receipts.iter().filter(|receipt| {
        receipt.value.get("status").and_then(JsonValue::as_str) == Some("pass")
            && redline_family_engine_commit(&receipt.value) == lock_engine_commit.as_deref()
    }) {
        let valid_consumer = |name: &str| {
            consumers.get(name).and_then(|rows| {
                rows.iter().find(|receipt| {
                    receipt.value.get("status").and_then(JsonValue::as_str) == Some("pass")
                        && receipt.value.get("family").and_then(JsonValue::as_str)
                            == Some("redline-split")
                        && receipt
                            .value
                            .get("family_ci_receipt_sha256")
                            .and_then(JsonValue::as_str)
                            == Some(family.digest.as_str())
                        && receipt
                            .value
                            .get("engine_commit")
                            .and_then(JsonValue::as_str)
                            == lock_engine_commit.as_deref()
                        && receipt.modified >= family.modified
                })
            })
        };
        if let (Some(jain), Some(jeryu)) =
            (valid_consumer("jain-split"), valid_consumer("jeryu-split"))
        {
            selected = Some((family.clone(), jain.clone(), jeryu.clone()));
            break;
        }
    }
    let (family_digest, consumer_count, mut refs) = match selected {
        Some((family, jain, jeryu)) => {
            let mut refs = family.refs;
            refs.extend(jain.refs);
            refs.extend(jeryu.refs);
            (Some(family.digest), 2usize, refs)
        }
        None => {
            failures.push(
                "no current Redline family-CI receipt has both fresh bound consumer proofs"
                    .to_owned(),
            );
            (None, 0usize, Vec::new())
        }
    };
    refs.sort_by(|left, right| left["path"].as_str().cmp(&right["path"].as_str()));
    gate(
        "redline_dependency",
        if failures.is_empty() {
            "pass"
        } else {
            "blocked"
        },
        json!({
            "family_ci_sha256": family_digest,
            "consumer_count": consumer_count,
            "lock_sha256": lock_sha256,
        }),
        refs,
        failures,
    )
}

fn redline_family_engine_commit(value: &JsonValue) -> Option<&str> {
    value
        .get("repositories")
        .and_then(JsonValue::as_array)
        .into_iter()
        .flatten()
        .find(|row| row.get("name").and_then(JsonValue::as_str) == Some("redline-core"))
        .and_then(|row| {
            row.get("commit")
                .or_else(|| row.get("head"))
                .and_then(JsonValue::as_str)
        })
}

fn validate_digest_sidecar(path: &Path, digest: &str) -> Result<(), Box<dyn std::error::Error>> {
    let sidecar = PathBuf::from(format!("{}.sha256", path.display()));
    let bytes = read_stable_regular_file(&sidecar, 4096, false)?;
    let basename = path
        .file_name()
        .and_then(OsStr::to_str)
        .ok_or("evidence basename is not UTF-8")?;
    let expected = format!("{digest}  {basename}\n");
    if bytes != expected.as_bytes() {
        return Err(format!("invalid evidence digest sidecar: {}", sidecar.display()).into());
    }
    Ok(())
}

fn forge_gate(
    managed: &Result<Vec<ManagedRepo>, Box<dyn std::error::Error>>,
    token_file: Option<&Path>,
) -> JsonValue {
    let Some(token_file) = token_file else {
        return gate(
            "forge_readback",
            "waiting_for_credential",
            json!({"credential_present": false}),
            Vec::new(),
            vec!["a readback-only forge credential is required".to_owned()],
        );
    };
    let client = match JeryuClient::from_token_file(token_file) {
        Ok(client) => client,
        Err(_) => {
            return gate(
                "forge_readback",
                "waiting_for_credential",
                json!({"credential_present": true, "credential_valid": false}),
                Vec::new(),
                vec!["the readback credential is unavailable or invalid".to_owned()],
            )
        }
    };
    let repositories = match managed {
        Ok(repositories) => repositories,
        Err(error) => {
            return gate(
                "forge_readback",
                "blocked",
                json!({"credential_present": true}),
                Vec::new(),
                vec![error.to_string()],
            )
        }
    };
    let mut failures = Vec::new();
    let mut human = Vec::new();
    let mut verified = 0usize;
    for repository in repositories {
        let result = (|| -> Result<bool, Box<dyn std::error::Error>> {
            let slug = forge_slug_from_remote(&repository.remote)?;
            let head = secure_git_output(
                Some(&repository.path),
                &["rev-parse", "--verify", "HEAD^{commit}"],
            )?;
            if secure_remote_ref(&repository.remote, "refs/heads/main", token_file)?.as_deref()
                != Some(head.as_str())
            {
                return Err("protected remote main differs from local HEAD".into());
            }
            if let Some(tag) = repository.tag.as_deref() {
                let reference = format!("refs/tags/{tag}");
                if secure_remote_ref(&repository.remote, &reference, token_file)?.as_deref()
                    != Some(head.as_str())
                {
                    return Err(format!("remote immutable tag {tag} differs from HEAD").into());
                }
            }
            let checks = client.execute(&JeryuRequest::checks(&slug, &head)?)?;
            validate_green_checks(&checks, &head, &repository.required_check)?;
            let statuses = client.execute(&JeryuRequest::commit_status_readback(&slug, &head)?)?;
            validate_green_status(&statuses, &head, &repository.required_check)?;
            let protection = client.execute(&JeryuRequest::protection(&slug, "main", None)?)?;
            validate_protection_policy(&protection, &slug, "main", &repository.required_check)?;
            let pulls = client.execute(&JeryuRequest::pr_list(&slug, "closed")?)?;
            let pull = pull_rows(&pulls).into_iter().find(|pull| {
                pull.get("head")
                    .and_then(|value| value.get("sha"))
                    .and_then(JsonValue::as_str)
                    == Some(head.as_str())
                    && pull
                        .get("base")
                        .and_then(|value| value.get("ref"))
                        .and_then(JsonValue::as_str)
                        == Some("main")
            });
            let Some(number) = pull.and_then(|pull| pull.get("number").and_then(JsonValue::as_u64))
            else {
                return Ok(false);
            };
            let approval = client.execute(&JeryuRequest::pr_readback(&slug, number)?)?;
            Ok(validate_approval_readback(&approval, &head).is_ok())
        })();
        match result {
            Ok(true) => verified += 1,
            Ok(false) => human.push(format!(
                "{}: exact-head approval is missing",
                repository.name
            )),
            Err(error) => failures.push(format!("{}: {error}", repository.name)),
        }
    }
    let (status, gate_failures) = if !failures.is_empty() {
        ("blocked", failures)
    } else if !human.is_empty() {
        ("waiting_for_human", human)
    } else {
        ("pass", Vec::new())
    };
    gate(
        "forge_readback",
        status,
        json!({
            "credential_present": true,
            "repository_count": repositories.len(),
            "verified_repository_count": verified,
        }),
        Vec::new(),
        gate_failures,
    )
}

fn secure_remote_ref(
    remote: &str,
    reference: &str,
    token_file: &Path,
) -> Result<Option<String>, Box<dyn std::error::Error>> {
    if !matches!(
        reference.strip_prefix("refs/"),
        Some(value) if value.starts_with("heads/") || value.starts_with("tags/")
    ) || reference.bytes().any(|byte| byte.is_ascii_control())
    {
        return Err("remote readback requires one exact heads or tags ref".into());
    }
    let output = secure_git_authenticated_output(
        None,
        token_file,
        &["ls-remote", "--refs", remote, reference],
    )?;
    if output.is_empty() {
        return Ok(None);
    }
    let mut lines = output.lines();
    let line = lines.next().ok_or("missing remote readback")?;
    if lines.next().is_some() {
        return Err("remote readback returned duplicate refs".into());
    }
    let mut fields = line.split('\t');
    let commit = fields.next().unwrap_or_default();
    let found = fields.next().unwrap_or_default();
    if fields.next().is_some()
        || found != reference
        || !is_full_sha(commit)
        || commit.chars().any(|ch| ch.is_ascii_uppercase())
    {
        return Err("remote readback was malformed".into());
    }
    Ok(Some(commit.to_owned()))
}

fn forge_slug_from_remote(remote: &str) -> Result<String, Box<dyn std::error::Error>> {
    let slug = remote
        .strip_prefix("http://127.0.0.1:8787/git/")
        .and_then(|value| value.strip_suffix(".git"))
        .ok_or("repository remote is not local Jeryu authority")?;
    validate_jeryu_repo_slug(slug)?;
    Ok(slug.to_owned())
}

fn validate_green_checks(
    response: &JsonValue,
    head: &str,
    required_check: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let rows = response
        .get("check_runs")
        .and_then(JsonValue::as_array)
        .ok_or("forge check readback has no check_runs")?;
    for name in ["jankurai/proof", required_check] {
        let count = rows
            .iter()
            .filter(|row| {
                row.get("name").and_then(JsonValue::as_str) == Some(name)
                    && row.get("head_sha").and_then(JsonValue::as_str) == Some(head)
                    && row.get("status").and_then(JsonValue::as_str) == Some("completed")
                    && row.get("conclusion").and_then(JsonValue::as_str) == Some("success")
            })
            .count();
        if count != 1 {
            return Err(format!("forge check {name} is not exactly green at HEAD").into());
        }
    }
    Ok(())
}

fn validate_green_status(
    response: &JsonValue,
    head: &str,
    required_check: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if response.get("sha").and_then(JsonValue::as_str) != Some(head) {
        return Err("forge commit-status readback names the wrong SHA".into());
    }
    let rows = response
        .get("statuses")
        .and_then(JsonValue::as_array)
        .ok_or("forge commit-status readback has no statuses")?;
    let count = rows
        .iter()
        .filter(|row| {
            row.get("context").and_then(JsonValue::as_str) == Some(required_check)
                && row.get("state").and_then(JsonValue::as_str) == Some("success")
        })
        .count();
    if count != 1 {
        return Err("required commit status is not exactly green at HEAD".into());
    }
    Ok(())
}

fn pull_rows(value: &JsonValue) -> Vec<&JsonValue> {
    value
        .as_array()
        .or_else(|| value.get("pulls").and_then(JsonValue::as_array))
        .map(|rows| rows.iter().collect())
        .unwrap_or_default()
}

fn cloud_gate(path: Option<&Path>) -> JsonValue {
    let Some(path) = path else {
        return gate(
            "cloud_release_spec",
            "pass",
            json!({"configured": false}),
            Vec::new(),
            Vec::new(),
        );
    };
    let mut failures = Vec::new();
    if path.starts_with("/tmp") {
        failures.push("cloud spec may not be read from /tmp".to_owned());
    }
    let bytes = match read_stable_regular_file(path, MAX_INPUT_BYTES, false) {
        Ok(bytes) => bytes,
        Err(error) => {
            failures.push(error.to_string());
            Vec::new()
        }
    };
    let spec: JsonValue = match serde_json::from_slice(&bytes) {
        Ok(spec) => spec,
        Err(error) => {
            failures.push(format!("cloud spec is invalid JSON: {error}"));
            JsonValue::Null
        }
    };
    if spec.get("schema_version").and_then(JsonValue::as_str) != Some("jain.cloud-release-spec/v1")
    {
        failures.push("cloud spec schema is not jain.cloud-release-spec/v1".to_owned());
    }
    if spec.get("release").and_then(JsonValue::as_str) != Some(RELEASE_VERSION)
        || spec.get("status").and_then(JsonValue::as_str) != Some(RELEASE_STATUS)
        || spec.get("formal_ga").and_then(JsonValue::as_bool) != Some(false)
        || spec.get("rollback_release").and_then(JsonValue::as_str) != Some(ROLLBACK_TARGET)
    {
        failures.push("cloud spec candidate identity is invalid".to_owned());
    }
    if !spec
        .get("release_identity_sha256")
        .and_then(JsonValue::as_str)
        .is_some_and(|value| is_full_hex(value, 64) && !value.chars().all(|ch| ch == '0'))
    {
        failures.push("cloud spec release identity is not a nonzero SHA-256".to_owned());
    }
    if spec.get("release_eligible").is_some() {
        failures.push("cloud spec contains a caller-supplied eligibility assertion".to_owned());
    }
    if spec
        .get("sources")
        .and_then(JsonValue::as_array)
        .map(Vec::len)
        != Some(44)
    {
        failures.push("cloud spec must bind exactly 44 source repositories".to_owned());
    }
    let expected_roles = BTreeSet::from([
        "guest-slot",
        "scq-gateway",
        "smartcluster-runtime",
        "standalone-appliance",
        "web-control",
        "worker-cpu",
        "worker-gpu",
    ]);
    let roles = spec
        .get("runtime_roles")
        .and_then(JsonValue::as_array)
        .map(|rows| {
            rows.iter()
                .filter_map(|row| row.get("role").and_then(JsonValue::as_str))
                .collect::<BTreeSet<_>>()
        })
        .unwrap_or_default();
    if roles != expected_roles {
        failures.push("cloud spec does not bind the exact seven runtime roles".to_owned());
    }
    gate(
        "cloud_release_spec",
        if failures.is_empty() {
            "pass"
        } else {
            "blocked"
        },
        json!({
            "configured": true,
            "spec_sha256": sha256_bytes(&bytes),
            "release_identity_sha256": spec.get("release_identity_sha256"),
        }),
        Vec::new(),
        failures,
    )
}

fn overall_status(gates: &[JsonValue]) -> &'static str {
    if gates.iter().any(|gate| gate["status"] == "blocked") {
        "blocked"
    } else if gates.iter().any(|gate| {
        gate["status"] == "waiting_for_credential" || gate["status"] == "waiting_for_human"
    }) {
        "waiting"
    } else {
        "ready"
    }
}

fn next_action(gates: &[JsonValue], overall: &str, recording: bool) -> JsonValue {
    if let Some(blocked) = gates.iter().find(|gate| gate["status"] == "blocked") {
        let role = match blocked["id"].as_str() {
            Some("candidate_invariants" | "authority_manifest") => "authority-manifest-owner",
            Some("control_plane") => "control-plane-owner",
            Some("repository_state") => "repository-owner",
            Some("redline_dependency") => "redline-proof-owner",
            Some("cloud_release_spec") => "cloud-release-owner",
            _ => "release-owner",
        };
        return json!({
            "kind": "repair_gate",
            "argv_without_secrets": [
                "splitctl", "release-flow", "--manifest", "<authority-manifest>",
                "--evidence-root", "<release-evidence-root>"
            ],
            "required_role": role,
        });
    }
    if gates
        .iter()
        .any(|gate| gate["status"] == "waiting_for_credential")
    {
        return json!({
            "kind": "provide_readback_credential",
            "argv_without_secrets": [
                "splitctl", "release-flow", "--manifest", "<authority-manifest>",
                "--evidence-root", "<release-evidence-root>",
                "--token-file", "<readback-token>"
            ],
            "required_role": "credential-custodian",
        });
    }
    if gates
        .iter()
        .any(|gate| gate["status"] == "waiting_for_human")
    {
        return json!({
            "kind": "obtain_exact_head_approval",
            "argv_without_secrets": [
                "splitctl", "jeryu-local", "pr-approve", "--repo", "<owner/repository>",
                "--number", "<pull-request>", "--expected-head", "<commit>",
                "--token-file", "<reviewer-token>", "--apply"
            ],
            "required_role": "independent-reviewer",
        });
    }
    if overall == "ready" && !recording {
        return json!({
            "kind": "record_release_status",
            "argv_without_secrets": [
                "splitctl", "release-flow", "--manifest", "<authority-manifest>",
                "--evidence-root", "<release-evidence-root>",
                "--record", "<new-immutable-receipt>"
            ],
            "required_role": "release-owner",
        });
    }
    json!({"kind": "none", "argv_without_secrets": [], "required_role": "none"})
}

fn seal_report(report: &mut JsonValue) -> Result<(), Box<dyn std::error::Error>> {
    report["receipt_integrity_sha256"] = JsonValue::Null;
    let digest = sha256_bytes(&serde_json::to_vec(report)?);
    report["receipt_integrity_sha256"] = json!(digest);
    Ok(())
}

fn verify_report_integrity(report: &JsonValue) -> Result<(), Box<dyn std::error::Error>> {
    let expected = report
        .get("receipt_integrity_sha256")
        .and_then(JsonValue::as_str)
        .filter(|value| is_full_hex(value, 64))
        .ok_or("resume receipt has no integrity SHA-256")?;
    let mut unsigned = report.clone();
    unsigned["receipt_integrity_sha256"] = JsonValue::Null;
    let actual = sha256_bytes(&serde_json::to_vec(&unsigned)?);
    if actual != expected {
        return Err("resume receipt integrity SHA-256 mismatch".into());
    }
    Ok(())
}

fn read_evidence_receipt(
    path: &Path,
    evidence_root: &Path,
) -> Result<(Vec<u8>, String), Box<dyn std::error::Error>> {
    if !path.starts_with(evidence_root) {
        return Err("receipt is outside the declared evidence root".into());
    }
    let bytes = read_stable_regular_file(path, MAX_INPUT_BYTES, true)?;
    let digest = sha256_bytes(&bytes);
    Ok((bytes, digest))
}

fn verify_resume_receipt(
    prior: &JsonValue,
    manifest: &Path,
    manifest_hash: &str,
    control_plane: &JsonValue,
    evidence_root: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    if prior.get("schema_version").and_then(JsonValue::as_str) != Some(SCHEMA) {
        return Err("resume receipt schema mismatch".into());
    }
    verify_report_integrity(prior)?;
    if prior
        .get("manifest")
        .and_then(|value| value.get("path"))
        .and_then(JsonValue::as_str)
        != manifest.to_str()
        || prior
            .get("manifest")
            .and_then(|value| value.get("sha256"))
            .and_then(JsonValue::as_str)
            != Some(manifest_hash)
    {
        return Err("resume receipt authority manifest identity drifted".into());
    }
    for key in ["head", "tree", "status_sha256"] {
        if prior.get("control_plane").and_then(|value| value.get(key)) != control_plane.get(key) {
            return Err(format!("resume receipt control-plane {key} drifted").into());
        }
    }
    for reference in prior
        .get("gates")
        .and_then(JsonValue::as_array)
        .into_iter()
        .flatten()
        .flat_map(|gate| {
            gate.get("receipt_refs")
                .and_then(JsonValue::as_array)
                .into_iter()
                .flatten()
        })
    {
        verify_receipt_reference(reference, evidence_root)?;
    }
    if let Some(previous) = prior.get("previous_receipt") {
        if previous.get("sha256").and_then(JsonValue::as_str).is_some() {
            verify_predecessor_chain(previous, evidence_root, 0)?;
        }
    }
    Ok(())
}

fn verify_predecessor_chain(
    reference: &JsonValue,
    evidence_root: &Path,
    depth: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    if depth >= 128 {
        return Err("release-flow predecessor chain is too deep".into());
    }
    let path = reference
        .get("path")
        .and_then(JsonValue::as_str)
        .map(PathBuf::from)
        .ok_or("predecessor reference has no path")?;
    let expected = reference
        .get("sha256")
        .and_then(JsonValue::as_str)
        .filter(|value| is_full_hex(value, 64))
        .ok_or("predecessor reference has no SHA-256")?;
    let (bytes, actual) = read_evidence_receipt(&path, evidence_root)?;
    if actual != expected {
        return Err("predecessor receipt SHA-256 mismatch".into());
    }
    let prior: JsonValue = serde_json::from_slice(&bytes)?;
    if prior.get("schema_version").and_then(JsonValue::as_str) != Some(SCHEMA) {
        return Err("predecessor receipt schema mismatch".into());
    }
    verify_report_integrity(&prior)?;
    for nested in prior
        .get("gates")
        .and_then(JsonValue::as_array)
        .into_iter()
        .flatten()
        .flat_map(|gate| {
            gate.get("receipt_refs")
                .and_then(JsonValue::as_array)
                .into_iter()
                .flatten()
        })
    {
        verify_receipt_reference(nested, evidence_root)?;
    }
    if let Some(previous) = prior.get("previous_receipt") {
        if previous.get("sha256").and_then(JsonValue::as_str).is_some() {
            verify_predecessor_chain(previous, evidence_root, depth + 1)?;
        }
    }
    Ok(())
}

fn verify_receipt_reference(
    reference: &JsonValue,
    evidence_root: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let path = reference
        .get("path")
        .and_then(JsonValue::as_str)
        .map(PathBuf::from)
        .ok_or("receipt reference has no path")?;
    let expected = reference
        .get("sha256")
        .and_then(JsonValue::as_str)
        .filter(|value| is_full_hex(value, 64))
        .ok_or("receipt reference has no SHA-256")?;
    if !path.starts_with(evidence_root) {
        return Err("receipt reference escapes the evidence root".into());
    }
    let bytes = read_stable_regular_file(&path, MAX_INPUT_BYTES, false)?;
    if sha256_bytes(&bytes) != expected {
        return Err(format!("receipt reference digest mismatch: {}", path.display()).into());
    }
    Ok(())
}

fn record_receipt(
    path: &Path,
    evidence_root: &Path,
    bytes: &[u8],
) -> Result<(), Box<dyn std::error::Error>> {
    if !path.is_absolute() || !path.starts_with(evidence_root) {
        return Err("--record must be an absolute path beneath the evidence root".into());
    }
    let parent = path.parent().ok_or("--record has no parent directory")?;
    if fs::canonicalize(parent)? != parent || !parent.starts_with(evidence_root) {
        return Err("--record parent is not a physical evidence directory".into());
    }
    let parent_metadata = fs::symlink_metadata(parent)?;
    if !parent_metadata.is_dir() || parent_metadata.file_type().is_symlink() {
        return Err("--record parent is not a physical directory".into());
    }
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Ok(_) => return Err("--record destination already exists".into()),
        Err(error) => return Err(error.into()),
    }
    let parent_c = CString::new(parent.as_os_str().as_bytes())?;
    let parent_fd = unsafe {
        libc::open(
            parent_c.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    if parent_fd < 0 {
        return Err(io::Error::last_os_error().into());
    }
    let parent_file = unsafe { fs::File::from_raw_fd(parent_fd) };
    let opened_parent = parent_file.metadata()?;
    if (opened_parent.dev(), opened_parent.ino()) != (parent_metadata.dev(), parent_metadata.ino())
    {
        return Err("--record parent identity changed before creation".into());
    }
    let filename = path
        .file_name()
        .filter(|name| !name.is_empty())
        .ok_or("--record has no filename")?;
    let filename_c = CString::new(filename.as_bytes())?;
    let receipt_fd = unsafe {
        libc::openat(
            parent_file.as_raw_fd(),
            filename_c.as_ptr(),
            libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            0o600,
        )
    };
    if receipt_fd < 0 {
        return Err(io::Error::last_os_error().into());
    }
    if unsafe { libc::fchmod(receipt_fd, 0o600) } != 0 {
        return Err(io::Error::last_os_error().into());
    }
    let mut receipt = unsafe { fs::File::from_raw_fd(receipt_fd) };
    let metadata = receipt.metadata()?;
    if !metadata.is_file() || metadata.nlink() != 1 || metadata.mode() & 0o777 != 0o600 {
        return Err("new release-flow receipt has unsafe metadata".into());
    }
    receipt.write_all(bytes)?;
    receipt.sync_all()?;
    let path_after = fs::symlink_metadata(path)?;
    let parent_after = fs::symlink_metadata(parent)?;
    if path_after.file_type().is_symlink()
        || (path_after.dev(), path_after.ino()) != (metadata.dev(), metadata.ino())
        || (parent_after.dev(), parent_after.ino()) != (opened_parent.dev(), opened_parent.ino())
    {
        return Err("release-flow receipt path changed during creation".into());
    }
    parent_file.sync_all()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(name: &str) -> Self {
            let path = env::temp_dir().join(format!(
                "jain-release-flow-{name}-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn receipt_integrity_detects_tampering() {
        let mut report = json!({
            "schema_version": SCHEMA,
            "receipt_integrity_sha256": JsonValue::Null,
            "overall_status": "blocked",
        });
        seal_report(&mut report).unwrap();
        verify_report_integrity(&report).unwrap();
        report["overall_status"] = json!("ready");
        assert!(verify_report_integrity(&report).is_err());
    }

    #[test]
    fn reducer_prioritizes_blockers_then_credentials_then_humans() {
        let pass = gate("pass", "pass", json!({}), Vec::new(), Vec::new());
        let blocked = gate(
            "authority",
            "blocked",
            json!({}),
            Vec::new(),
            vec!["blocked".to_owned()],
        );
        let credential = gate(
            "forge",
            "waiting_for_credential",
            json!({}),
            Vec::new(),
            Vec::new(),
        );
        let human = gate(
            "approval",
            "waiting_for_human",
            json!({}),
            Vec::new(),
            Vec::new(),
        );
        assert_eq!(overall_status(&[pass.clone(), blocked.clone()]), "blocked");
        assert_eq!(
            next_action(&[blocked, credential.clone()], "blocked", false)["kind"],
            "repair_gate"
        );
        assert_eq!(
            next_action(&[pass.clone(), credential], "waiting", false)["kind"],
            "provide_readback_credential"
        );
        assert_eq!(
            next_action(&[pass, human], "waiting", false)["kind"],
            "obtain_exact_head_approval"
        );
    }

    #[test]
    fn receipt_recording_is_exclusive_private_and_fsynced() {
        let root = TempDir::new("record");
        let receipt = root.0.join("receipt.json");
        record_receipt(&receipt, &root.0, b"{}\n").unwrap();
        let metadata = fs::metadata(&receipt).unwrap();
        assert_eq!(metadata.mode() & 0o777, 0o600);
        assert_eq!(metadata.nlink(), 1);
        assert!(record_receipt(&receipt, &root.0, b"changed\n").is_err());
        assert_eq!(fs::read(&receipt).unwrap(), b"{}\n");

        let outside = TempDir::new("outside");
        assert!(record_receipt(&outside.0.join("escape.json"), &root.0, b"{}\n").is_err());
        let link = root.0.join("link.json");
        std::os::unix::fs::symlink(&receipt, &link).unwrap();
        assert!(record_receipt(&link, &root.0, b"{}\n").is_err());

        let linked_parent = root.0.join("linked-parent");
        std::os::unix::fs::symlink(&root.0, &linked_parent).unwrap();
        assert!(
            record_receipt(&linked_parent.join("through-parent.json"), &root.0, b"{}\n").is_err()
        );
    }

    #[test]
    fn resume_binds_authority_control_and_all_evidence() {
        let root = TempDir::new("resume");
        let manifest = root.0.join("repos.manifest.toml");
        let manifest_hash = "a".repeat(64);
        let control = json!({
            "head": "b".repeat(40),
            "tree": "c".repeat(40),
            "status_sha256": "d".repeat(64),
        });
        let evidence = root.0.join("evidence.json");
        fs::write(&evidence, b"evidence\n").unwrap();
        let evidence_digest = sha256_bytes(b"evidence\n");

        let predecessor_path = root.0.join("predecessor.json");
        let mut predecessor = json!({
            "schema_version": SCHEMA,
            "manifest": {"path": manifest, "sha256": manifest_hash},
            "control_plane": control,
            "previous_receipt": {"path": JsonValue::Null, "sha256": JsonValue::Null},
            "gates": [],
            "receipt_integrity_sha256": JsonValue::Null,
        });
        seal_report(&mut predecessor).unwrap();
        let mut predecessor_bytes = serde_json::to_vec_pretty(&predecessor).unwrap();
        predecessor_bytes.push(b'\n');
        fs::write(&predecessor_path, &predecessor_bytes).unwrap();
        fs::set_permissions(&predecessor_path, fs::Permissions::from_mode(0o600)).unwrap();

        let mut prior = json!({
            "schema_version": SCHEMA,
            "manifest": {"path": manifest, "sha256": manifest_hash},
            "control_plane": control,
            "previous_receipt": {
                "path": predecessor_path,
                "sha256": sha256_bytes(&predecessor_bytes),
            },
            "gates": [{
                "receipt_refs": [{"path": evidence, "sha256": evidence_digest}],
            }],
            "receipt_integrity_sha256": JsonValue::Null,
        });
        seal_report(&mut prior).unwrap();
        verify_resume_receipt(&prior, &manifest, &manifest_hash, &control, &root.0).unwrap();

        fs::write(&evidence, b"tampered\n").unwrap();
        assert!(
            verify_resume_receipt(&prior, &manifest, &manifest_hash, &control, &root.0).is_err()
        );
        fs::write(&evidence, b"evidence\n").unwrap();

        let drifted_control = json!({
            "head": "e".repeat(40),
            "tree": "c".repeat(40),
            "status_sha256": "d".repeat(64),
        });
        assert!(verify_resume_receipt(
            &prior,
            &manifest,
            &manifest_hash,
            &drifted_control,
            &root.0
        )
        .is_err());
        assert!(
            verify_resume_receipt(&prior, &manifest, &"f".repeat(64), &control, &root.0).is_err()
        );

        fs::write(&predecessor_path, b"tampered\n").unwrap();
        assert!(
            verify_resume_receipt(&prior, &manifest, &manifest_hash, &control, &root.0).is_err()
        );
    }
}
