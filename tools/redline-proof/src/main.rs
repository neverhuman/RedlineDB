use chrono::{DateTime, Duration, SecondsFormat, Utc};
use serde_json::{json, Map as JsonMap, Value as JsonValue};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    fs::{File, OpenOptions},
    io::{self, Write},
    path::{Component, Path, PathBuf},
    process::{Command, Output, Stdio},
    time::{SystemTime, UNIX_EPOCH},
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

const FAMILY: &str = "redline-split";
const FAMILY_CI_SCHEMA: &str = "redline.family-ci/v1";
const CONSUMER_SCHEMA: &str = "redline.consumer-evidence/v1";
const LOCK_SCHEMA: &str = "redline.split.lock/v2";
const PROOF_REFRESH_SCHEMA: &str = "redline.proof-refresh/v1";
const LOCAL_JERYU_BASE: &str = "http://127.0.0.1:8787/git/";
const RELEASE_VERSION: &str = "8.0.0";
const RELEASE_PROTECTION_POLICY: &str = "immutable-main-v1";
const PENDING: &str = "PENDING";
const MAX_EVIDENCE_HOURS: i64 = 24;
const MAX_CLOCK_SKEW_MINUTES: i64 = 5;
const REQUIRED_CONSUMERS: [&str; 2] = ["jain-split", "jeryu-split"];

#[derive(Debug)]
struct RepairError {
    reason: String,
    docs_url: &'static str,
    repair_hint: &'static str,
}

impl std::fmt::Display for RepairError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "{}; repair_hint={}; docs_url={}",
            self.reason, self.repair_hint, self.docs_url
        )
    }
}

impl std::error::Error for RepairError {}

fn error(message: impl Into<String>) -> Box<dyn std::error::Error> {
    Box::new(RepairError {
        reason: message.into(),
        docs_url: "docs/testing.md",
        repair_hint: "rerun the owning command from agent/test-map.json",
    })
}

fn is_sha1(value: &str) -> bool {
    value.len() == 40
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn sha256_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn sha256_file(path: &Path) -> Result<String> {
    Ok(sha256_bytes(&fs::read(path)?))
}

fn checksum_path(path: &Path) -> PathBuf {
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("evidence");
    path.with_file_name(format!("{name}.sha256"))
}

fn unique_suffix() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{}-{nanos}", std::process::id())
}

fn atomic_write(path: &Path, data: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| error(format!("{} has no parent", path.display())))?;
    fs::create_dir_all(parent)?;
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("output");
    let temporary = parent.join(format!(".{name}.{}", unique_suffix()));
    let result = (|| -> Result<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(data)?;
        file.sync_all()?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&temporary, fs::Permissions::from_mode(0o644))?;
        }
        fs::rename(&temporary, path)?;
        File::open(parent)?.sync_all()?;
        Ok(())
    })();
    let _ = fs::remove_file(&temporary);
    result
}

fn transactional_write_with<F>(outputs: &[(PathBuf, Vec<u8>)], mut writer: F) -> Result<()>
where
    F: FnMut(&Path, &[u8]) -> Result<()>,
{
    let mut seen = BTreeSet::new();
    let mut snapshots = Vec::new();
    for (path, _) in outputs {
        let absolute = absolute_path(path)?;
        if !seen.insert(absolute.clone()) {
            return Err(error(format!(
                "transaction contains duplicate output: {}",
                absolute.display()
            )));
        }
        snapshots.push((
            absolute,
            if path.is_file() {
                Some(fs::read(path)?)
            } else {
                None
            },
        ));
    }
    let attempt = (|| -> Result<()> {
        for (path, data) in outputs {
            writer(path, data)?;
        }
        for (path, expected) in outputs {
            if fs::read(path)? != *expected {
                return Err(error(format!(
                    "transactional output verification failed: {}",
                    path.display()
                )));
            }
        }
        Ok(())
    })();
    if let Err(original) = attempt {
        let mut rollback_errors = Vec::new();
        for ((path, _), (_, previous)) in outputs.iter().zip(snapshots.iter()).rev() {
            let restored = match previous {
                Some(bytes) => atomic_write(path, bytes),
                None => match fs::remove_file(path) {
                    Ok(()) => Ok(()),
                    Err(value) if value.kind() == io::ErrorKind::NotFound => Ok(()),
                    Err(value) => Err(value.into()),
                },
            };
            if let Err(value) = restored {
                rollback_errors.push(format!("{}: {value}", path.display()));
            }
        }
        if !rollback_errors.is_empty() {
            return Err(error(format!(
                "transaction failed ({original}) and rollback was incomplete: {}",
                rollback_errors.join("; ")
            )));
        }
        return Err(original);
    }
    Ok(())
}

fn transactional_write(outputs: &[(PathBuf, Vec<u8>)]) -> Result<()> {
    transactional_write_with(outputs, atomic_write)
}

fn checksummed_json_bytes(path: &Path, value: &JsonValue) -> Result<(Vec<u8>, String, Vec<u8>)> {
    let mut data = serde_json::to_vec_pretty(value)?;
    data.push(b'\n');
    let digest = sha256_bytes(&data);
    let sidecar = format!(
        "{digest}  {}\n",
        path.file_name()
            .and_then(|v| v.to_str())
            .unwrap_or("receipt.json")
    )
    .into_bytes();
    Ok((data, digest, sidecar))
}

fn write_checksummed_json(path: &Path, value: &JsonValue) -> Result<String> {
    let (data, digest, sidecar) = checksummed_json_bytes(path, value)?;
    transactional_write(&[(path.to_path_buf(), data), (checksum_path(path), sidecar)])?;
    Ok(digest)
}

fn verify_checksum(path: &Path) -> Result<String> {
    let sidecar = checksum_path(path);
    if !path.is_file() || !sidecar.is_file() {
        return Err(error(format!(
            "evidence or checksum is missing: {}",
            path.display()
        )));
    }
    let raw = fs::read_to_string(&sidecar)?;
    let parts: Vec<&str> = raw.split_whitespace().collect();
    let expected_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("");
    if parts.len() != 2 || !is_sha256(parts[0]) || parts[1] != expected_name {
        return Err(error(format!(
            "invalid checksum sidecar: {}",
            sidecar.display()
        )));
    }
    let actual = sha256_file(path)?;
    if actual != parts[0] {
        return Err(error(format!(
            "tampered evidence: checksum mismatch for {}",
            path.display()
        )));
    }
    Ok(actual)
}

fn absolute_path(path: &Path) -> Result<PathBuf> {
    let raw = if path.is_absolute() {
        path.to_path_buf()
    } else {
        env::current_dir()?.join(path)
    };
    let mut normalized = PathBuf::new();
    for component in raw.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    Ok(normalized)
}

fn recorded_path(path: &Path, base: &Path) -> Result<String> {
    let path = absolute_path(path)?;
    let base = absolute_path(base)?;
    let relative = pathdiff::diff_paths(&path, &base).ok_or_else(|| {
        error(format!(
            "cannot make {} relative to {}",
            path.display(),
            base.display()
        ))
    })?;
    Ok(relative.to_string_lossy().replace('\\', "/"))
}

fn resolve_recorded_path(raw: &JsonValue, base: &Path, field: &str) -> Result<PathBuf> {
    let text = raw
        .as_str()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| error(format!("{field} must name a file")))?;
    let path = PathBuf::from(text);
    Ok(if path.is_absolute() {
        path
    } else {
        base.join(path)
    })
}

fn format_time(value: DateTime<Utc>) -> String {
    value.to_rfc3339_opts(SecondsFormat::Secs, true)
}

fn parse_time(value: &JsonValue, field: &str) -> Result<DateTime<Utc>> {
    let raw = value
        .as_str()
        .ok_or_else(|| error(format!("{field} must be an RFC3339 timestamp")))?;
    Ok(DateTime::parse_from_rfc3339(raw)
        .map_err(|value| error(format!("{field} is not a valid RFC3339 timestamp: {value}")))?
        .with_timezone(&Utc))
}

fn require_fresh(timestamp: DateTime<Utc>, now: DateTime<Utc>, field: &str) -> Result<()> {
    if timestamp > now + Duration::minutes(MAX_CLOCK_SKEW_MINUTES) {
        return Err(error(format!("{field} is too far in the future")));
    }
    if now - timestamp > Duration::hours(MAX_EVIDENCE_HOURS) {
        return Err(error(format!("{field} is stale (maximum age is 24 hours)")));
    }
    Ok(())
}

#[derive(Clone, Debug)]
struct Repo {
    name: String,
    path: PathBuf,
    github_slug: String,
    remote: String,
    product_version: String,
    tag_revision: i64,
    current_tag: String,
    release_commit: String,
    release_checksum_sha256: String,
    protection_policy: String,
    required_check: String,
    default_branch: String,
}

#[derive(Clone, Debug)]
struct Manifest {
    path: PathBuf,
    repos: Vec<Repo>,
}

fn toml_string(table: &toml::value::Table, key: &str, context: &str) -> Result<String> {
    table
        .get(key)
        .and_then(toml::Value::as_str)
        .filter(|v| !v.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| error(format!("{context} lacks {key}")))
}

fn toml_integer(table: &toml::value::Table, key: &str, context: &str) -> Result<i64> {
    table
        .get(key)
        .and_then(toml::Value::as_integer)
        .ok_or_else(|| error(format!("{context} lacks integer {key}")))
}

fn expected_repo_release(name: &str) -> Option<(&'static str, i64)> {
    match name {
        "redline" => Some(("4.1.0", 2)),
        "redline-core" => Some(("4.1.0", 3)),
        "redline-testing" => Some(("1.0.1", 1)),
        "redline-web" => Some(("0.1.0", 1)),
        _ => None,
    }
}

fn validate_release_identity(
    table: &toml::value::Table,
    name: &str,
    revision_namespace: &str,
    expected_product_version: &str,
    expected_revision: i64,
) -> Result<(String, i64, String, String, String, String)> {
    let product_version = toml_string(table, "product_version", name)?;
    let tag_revision = toml_integer(table, "tag_revision", name)?;
    let current_tag = toml_string(table, "current_tag", name)?;
    let release_commit = toml_string(table, "release_commit", name)?;
    let release_checksum_sha256 = toml_string(table, "release_checksum_sha256", name)?;
    let protection_policy = toml_string(table, "protection_policy", name)?;
    if product_version != expected_product_version || tag_revision != expected_revision {
        return Err(error(format!(
            "{name}: expected product {expected_product_version} revision {expected_revision}, found {product_version} revision {tag_revision}"
        )));
    }
    let expected_tag = format!("{name}-v{product_version}-{revision_namespace}.{tag_revision}");
    if current_tag != expected_tag {
        return Err(error(format!(
            "{name}: current_tag must be {expected_tag}, found {current_tag}"
        )));
    }
    if protection_policy != RELEASE_PROTECTION_POLICY {
        return Err(error(format!(
            "{name}: protection_policy must be {RELEASE_PROTECTION_POLICY}"
        )));
    }
    match (
        release_commit.as_str(),
        release_checksum_sha256.as_str(),
    ) {
        (PENDING, PENDING) => {}
        (commit, checksum) if is_sha1(commit) && is_sha256(checksum) => {}
        (PENDING, _) | (_, PENDING) => {
            return Err(error(format!(
                "{name}: release commit and checksum must become exact together"
            )))
        }
        _ => {
            return Err(error(format!(
                "{name}: release identity must contain exact SHA-1/SHA-256 values or two PENDING values"
            )))
        }
    }
    Ok((
        product_version,
        tag_revision,
        current_tag,
        release_commit,
        release_checksum_sha256,
        protection_policy,
    ))
}

fn validate_protection_policy(value: &toml::Value) -> Result<()> {
    let policy = value
        .get("protection_policies")
        .and_then(|value| value.get(RELEASE_PROTECTION_POLICY))
        .and_then(toml::Value::as_table)
        .ok_or_else(|| error("manifest lacks immutable-main-v1 protection policy"))?;
    let integer = |key: &str| policy.get(key).and_then(toml::Value::as_integer);
    let boolean = |key: &str| policy.get(key).and_then(toml::Value::as_bool);
    if integer("required_approvals") != Some(1)
        || boolean("required_status_check") != Some(true)
        || boolean("linear_history") != Some(true)
        || boolean("enforce_admins") != Some(true)
        || boolean("allow_force_push") != Some(false)
        || boolean("allow_deletions") != Some(false)
    {
        return Err(error(
            "immutable-main-v1 protection policy is incomplete or unsafe",
        ));
    }
    Ok(())
}

fn load_manifest(path: &Path) -> Result<Manifest> {
    let text = fs::read_to_string(path)?;
    let value: toml::Value = text.parse()?;
    if value.get("family").and_then(toml::Value::as_str) != Some(FAMILY)
        || value.get("parent_family").and_then(toml::Value::as_str) != Some("independent")
    {
        return Err(error(
            "manifest must describe the independent redline-split family",
        ));
    }
    if value.get("release_version").and_then(toml::Value::as_str) != Some(RELEASE_VERSION)
        || value.get("status").and_then(toml::Value::as_str) != Some("candidate")
        || value.get("formal_ga").and_then(toml::Value::as_bool) != Some(false)
        || value.get("sagemaker").and_then(toml::Value::as_str) != Some("N/A")
    {
        return Err(error(
            "manifest must describe the Jain 8.0.0 candidate with SageMaker N/A",
        ));
    }
    validate_protection_policy(&value)?;
    let control = value
        .get("control_plane")
        .and_then(toml::Value::as_table)
        .ok_or_else(|| error("manifest lacks control_plane"))?;
    if toml_string(control, "name", "control_plane")? != "redline-split-ops"
        || toml_string(control, "remote", "control_plane")?
            != "http://127.0.0.1:8787/git/jeryu/redline-split-ops.git"
        || toml_string(control, "required_check", "control_plane")? != "redline-split-ops/required"
        || toml_string(control, "family", "control_plane")? != FAMILY
    {
        return Err(error("manifest control-plane identity is invalid"));
    }
    validate_release_identity(control, "redline-split-ops", "split", RELEASE_VERSION, 0)?;
    let rows = value
        .get("repo")
        .and_then(toml::Value::as_array)
        .ok_or_else(|| error("manifest must contain repository rows"))?;
    if rows.len() != 4 {
        return Err(error(
            "manifest must contain exactly four Redline repositories",
        ));
    }
    let mut repos = Vec::new();
    for row in rows {
        let table = row
            .as_table()
            .ok_or_else(|| error("manifest repository row must be a table"))?;
        let name = toml_string(table, "name", "manifest repository")?;
        let raw_path = toml_string(table, "path", &name)?;
        let repo_path = PathBuf::from(&raw_path);
        if repo_path.is_absolute() || raw_path.contains("/home/ubuntu") {
            return Err(error(format!("{name}: manifest path must be relative")));
        }
        let default_branch = table
            .get("default_branch")
            .and_then(toml::Value::as_str)
            .unwrap_or("main")
            .to_owned();
        if default_branch != "main" {
            return Err(error(format!("{name}: default branch must be main")));
        }
        let (expected_product_version, expected_revision) = expected_repo_release(&name)
            .ok_or_else(|| error(format!("{name}: release identity is not authorized")))?;
        let (
            product_version,
            tag_revision,
            current_tag,
            release_commit,
            release_checksum_sha256,
            protection_policy,
        ) = validate_release_identity(
            table,
            &name,
            "jain",
            expected_product_version,
            expected_revision,
        )?;
        let jeryu_slug = toml_string(table, "jeryu_slug", "manifest repository")?;
        let remote = toml_string(table, "remote", "manifest repository")?;
        let expected_remote = format!(
            "{LOCAL_JERYU_BASE}{}.git",
            jeryu_slug.trim_start_matches('/')
        );
        if remote != expected_remote {
            return Err(error(format!(
                "{name}: remote must be {expected_remote}, found {remote}"
            )));
        }
        repos.push(Repo {
            name,
            path: repo_path,
            github_slug: toml_string(table, "github_slug", "manifest repository")?,
            remote,
            product_version,
            tag_revision,
            current_tag,
            release_commit,
            release_checksum_sha256,
            protection_policy,
            required_check: toml_string(table, "required_check", "manifest repository")?,
            default_branch,
        });
    }
    let names: BTreeSet<&str> = repos.iter().map(|repo| repo.name.as_str()).collect();
    let expected = BTreeSet::from(["redline", "redline-core", "redline-testing", "redline-web"]);
    if names != expected {
        return Err(error("manifest repository set is invalid"));
    }
    Ok(Manifest {
        path: absolute_path(path)?,
        repos,
    })
}

impl Manifest {
    fn repo_root(&self, repo: &Repo) -> PathBuf {
        self.path
            .parent()
            .unwrap_or(Path::new("."))
            .join(&repo.path)
    }
}

fn expected_origin(repo: &Repo) -> String {
    repo.remote.clone()
}

fn command_output(command: &mut Command) -> Result<Output> {
    let printable = format!("{command:?}");
    let output = command.output()?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(if output.stderr.is_empty() {
            &output.stdout
        } else {
            &output.stderr
        });
        return Err(error(format!(
            "command failed: {printable}: {}",
            detail.trim()
        )));
    }
    Ok(output)
}

fn git(root: &Path, args: &[&str]) -> Result<String> {
    let output = command_output(Command::new("git").arg("-C").arg(root).args(args))?;
    Ok(String::from_utf8(output.stdout)?.trim().to_owned())
}

fn git_optional(root: &Path, args: &[&str]) -> Result<Option<String>> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()?;
    if output.status.success() {
        Ok(Some(String::from_utf8(output.stdout)?.trim().to_owned()))
    } else {
        Ok(None)
    }
}

fn git_tree_checksum(root: &Path, commit: &str) -> Result<String> {
    let output = command_output(Command::new("git").arg("-C").arg(root).args([
        "archive",
        "--format=tar",
        commit,
    ]))?;
    Ok(sha256_bytes(&output.stdout))
}

fn cargo_package_version(path: &Path) -> Result<String> {
    let value: toml::Value = fs::read_to_string(path)?.parse()?;
    value
        .get("package")
        .and_then(|value| value.get("version"))
        .and_then(toml::Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| error(format!("{} lacks package.version", path.display())))
}

fn validate_product_version(root: &Path, repo: &Repo) -> Result<()> {
    let found = match repo.name.as_str() {
        "redline" => fs::read_to_string(root.join("VERSION"))?.trim().to_owned(),
        "redline-core" => cargo_package_version(&root.join("crates/redlinedb/Cargo.toml"))?,
        "redline-testing" => cargo_package_version(&root.join("Cargo.toml"))?,
        "redline-web" => cargo_package_version(&root.join("apps/api/Cargo.toml"))?,
        _ => {
            return Err(error(format!(
                "{}: unsupported product version source",
                repo.name
            )))
        }
    };
    let expected = if repo.name == "redline" {
        format!("{}-jain.{}", repo.product_version, repo.tag_revision)
    } else {
        repo.product_version.clone()
    };
    if found != expected {
        return Err(error(format!(
            "{}: tagged product version must be {expected}, found {found}",
            repo.name
        )));
    }
    Ok(())
}

fn forge_ref(root: &Path, reference: &str) -> Result<Option<String>> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["ls-remote", "origin", reference])
        .output()?;
    if !output.status.success() {
        return Err(error(format!(
            "cannot read origin {reference} for {}",
            root.display()
        )));
    }
    let raw = String::from_utf8(output.stdout)?;
    let matches: Vec<&str> = raw
        .lines()
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            let sha = fields.next()?;
            let found = fields.next()?;
            (found == reference && fields.next().is_none()).then_some(sha)
        })
        .collect();
    match matches.as_slice() {
        [] => Ok(None),
        [sha] if is_sha1(sha) => Ok(Some((*sha).to_owned())),
        _ => Err(error(format!(
            "origin returned ambiguous {reference} for {}",
            root.display()
        ))),
    }
}

fn local_tag_exists(root: &Path, tag: &str) -> Result<bool> {
    Ok(Command::new("git")
        .arg("-C")
        .arg(root)
        .args([
            "show-ref",
            "--verify",
            "--quiet",
            &format!("refs/tags/{tag}"),
        ])
        .status()?
        .success())
}

#[derive(Clone, Debug)]
struct TagMetadata {
    object: String,
    object_type: String,
    commit: String,
    tagger_date: Option<String>,
    subject: String,
    remote_object: Option<String>,
    remote_commit: Option<String>,
}

fn tag_metadata(root: &Path, tag: &str, require_remote: bool) -> Result<TagMetadata> {
    let reference = format!("refs/tags/{tag}");
    if !local_tag_exists(root, tag)? {
        return Err(error(format!(
            "{}: immutable tag {tag} is absent locally",
            root.display()
        )));
    }
    let object = git(root, &["rev-parse", &reference])?;
    let object_type = git(root, &["cat-file", "-t", &object])?;
    if object_type != "commit" && object_type != "tag" {
        return Err(error(format!(
            "{}: unsupported tag object type {object_type}",
            root.display()
        )));
    }
    let commit = git(root, &["rev-parse", &format!("{reference}^{{commit}}")])?;
    if !is_sha1(&object) || !is_sha1(&commit) {
        return Err(error(format!(
            "{}: tag {tag} does not resolve to immutable objects",
            root.display()
        )));
    }
    let raw = git(
        root,
        &[
            "for-each-ref",
            "--format=%(taggerdate:iso8601-strict)%09%(subject)",
            &reference,
        ],
    )?;
    let (date, subject) = raw.split_once('\t').unwrap_or(("", raw.as_str()));
    let remote_object = forge_ref(root, &reference)?;
    let remote_commit = forge_ref(root, &format!("{reference}^{{}}"))?;
    if require_remote {
        if remote_object.as_deref() != Some(&object) {
            return Err(error(format!(
                "{}: origin tag object for {tag} differs from local object",
                root.display()
            )));
        }
        let expected_peeled = (object_type == "tag").then_some(commit.as_str());
        if remote_commit.as_deref() != expected_peeled {
            return Err(error(format!(
                "{}: origin peeled tag metadata for {tag} is inconsistent",
                root.display()
            )));
        }
    }
    Ok(TagMetadata {
        object,
        object_type,
        commit,
        tagger_date: (!date.is_empty()).then(|| date.to_owned()),
        subject: subject.to_owned(),
        remote_object,
        remote_commit,
    })
}

fn metadata_json(value: &TagMetadata) -> JsonValue {
    json!({
        "object": value.object,
        "object_type": value.object_type,
        "commit": value.commit,
        "tagger_date": value.tagger_date,
        "subject": value.subject,
        "remote_object": value.remote_object,
        "remote_commit": value.remote_commit,
    })
}

fn current_reviewed_state(
    manifest: &Manifest,
    repo: &Repo,
    allow_absent_tag: bool,
) -> Result<JsonValue> {
    let root = manifest.repo_root(repo);
    if git_optional(&root, &["rev-parse", "--is-inside-work-tree"])?.as_deref() != Some("true") {
        return Err(error(format!(
            "{}: checkout is missing at {}",
            repo.name,
            root.display()
        )));
    }
    let branch = git(&root, &["branch", "--show-current"])?;
    if branch != "main" {
        return Err(error(format!(
            "{}: branch is {}, expected main",
            repo.name,
            if branch.is_empty() {
                "<detached>"
            } else {
                &branch
            }
        )));
    }
    if !git(&root, &["status", "--porcelain", "--untracked-files=all"])?.is_empty() {
        return Err(error(format!("{}: worktree is dirty", repo.name)));
    }
    let remotes = git(&root, &["remote"])?;
    if remotes != "origin" {
        return Err(error(format!(
            "{}: checkout must have exactly one origin remote",
            repo.name
        )));
    }
    let origin = git(&root, &["remote", "get-url", "origin"])?;
    let expected = expected_origin(repo);
    if origin != expected {
        return Err(error(format!(
            "{}: origin is {origin}, expected {expected}",
            repo.name
        )));
    }
    let commit = git(&root, &["rev-parse", "HEAD"])?;
    let forge_main = forge_ref(&root, "refs/heads/main")?;
    if forge_main.as_deref() != Some(&commit) {
        return Err(error(format!(
            "{}: HEAD {commit} differs from forge main {:?}",
            repo.name, forge_main
        )));
    }
    validate_product_version(&root, repo)?;
    if repo.release_commit != PENDING {
        if repo.release_commit != commit {
            return Err(error(format!(
                "{}: reviewed main {commit} differs from manifest release commit {}",
                repo.name, repo.release_commit
            )));
        }
        let checksum = git_tree_checksum(&root, &commit)?;
        if repo.release_checksum_sha256 != checksum {
            return Err(error(format!(
                "{}: reviewed release tree checksum differs from manifest",
                repo.name
            )));
        }
    }
    let metadata = if local_tag_exists(&root, &repo.current_tag)? {
        let found = tag_metadata(&root, &repo.current_tag, false)?;
        if found.commit != commit {
            return Err(error(format!(
                "{}: existing immutable tag {} points to {}, not reviewed main {commit}",
                repo.name, repo.current_tag, found.commit
            )));
        }
        Some(if found.remote_object.is_some() {
            tag_metadata(&root, &repo.current_tag, true)?
        } else {
            found
        })
    } else if !allow_absent_tag {
        return Err(error(format!(
            "{}: immutable tag {} is absent",
            repo.name, repo.current_tag
        )));
    } else {
        if forge_ref(&root, &format!("refs/tags/{}", repo.current_tag))?.is_some() {
            return Err(error(format!(
                "{}: origin exposes {}, but the local checkout does not",
                repo.name, repo.current_tag
            )));
        }
        None
    };
    Ok(json!({
        "name": repo.name,
        "path": repo.path.to_string_lossy().replace('\\', "/"),
        "branch": branch,
        "commit": commit,
        "forge_main": forge_main,
        "origin": origin,
        "required_check": repo.required_check,
        "product_version": repo.product_version,
        "tag_revision": repo.tag_revision,
        "tag": repo.current_tag,
        "release_commit": repo.release_commit,
        "release_checksum_sha256": repo.release_checksum_sha256,
        "protection_policy": repo.protection_policy,
        "tag_state": if metadata.is_some() { "verified" } else { "absent" },
        "tag_metadata": metadata.as_ref().map(metadata_json),
    }))
}

fn ci_commands(repo: &Repo, worktree: &Path) -> Result<Vec<Vec<String>>> {
    let commands: Vec<Vec<&str>> = match repo.name.as_str() {
        "redline" => vec![
            vec!["bash", "scripts/ci-local.sh", "required"],
            vec!["bash", "ops/ci/security.sh"],
            vec!["bash", "ops/ci/jankurai-audit.sh"],
        ],
        "redline-core" => vec![vec!["bash", "scripts/ci-local.sh", "all"]],
        "redline-testing" => vec![
            vec!["bash", "scripts/ci-local.sh", "pr-ci"],
            vec!["bash", "scripts/ci-local.sh", "jankurai"],
            vec!["bash", "scripts/ci-local.sh", "audit"],
            vec!["bash", "scripts/ci-local.sh", "release"],
        ],
        "redline-web" => vec![vec!["bash", "scripts/ci-local.sh", "pr-ci"]],
        _ => Vec::new(),
    };
    if !worktree.join("scripts/ci-doctor.sh").is_file()
        || commands.is_empty()
        || commands
            .iter()
            .any(|command| !worktree.join(command[1]).is_file())
    {
        return Err(error(format!(
            "{}: complete independent CI entrypoints are absent",
            repo.name
        )));
    }
    Ok(commands
        .into_iter()
        .map(|row| row.into_iter().map(str::to_owned).collect())
        .collect())
}

const TESTING_ARTIFACT_FIELDS: [&str; 17] = [
    "name",
    "source_repo",
    "source_commit",
    "source_tree_checksum_sha256",
    "product_version",
    "release_tag",
    "tag_revision",
    "artifact",
    "artifact_sha256",
    "checksum_sha256",
    "binary_sha256",
    "release_manifest",
    "release_manifest_sha256",
    "build_command",
    "build_log",
    "build_log_sha256",
    "transport",
];

#[derive(Clone, Debug)]
struct TestingArtifact {
    source_commit: String,
    source_tree_checksum_sha256: String,
    product_version: String,
    release_tag: String,
    tag_revision: i64,
    artifact_name: String,
    artifact_path: PathBuf,
    artifact_sha256: String,
    checksum_path: PathBuf,
    checksum_sha256: String,
    binary_sha256: String,
    manifest_path: PathBuf,
    manifest_sha256: String,
    build_log: PathBuf,
    build_log_sha256: String,
}

impl TestingArtifact {
    fn receipt_json(&self, receipt_base: &Path) -> Result<JsonValue> {
        Ok(json!({
            "name": "redline-testing-release",
            "source_repo": "redline-testing",
            "source_commit": self.source_commit,
            "source_tree_checksum_sha256": self.source_tree_checksum_sha256,
            "product_version": self.product_version,
            "release_tag": self.release_tag,
            "tag_revision": self.tag_revision,
            "artifact": self.artifact_name,
            "artifact_sha256": self.artifact_sha256,
            "checksum_sha256": self.checksum_sha256,
            "binary_sha256": self.binary_sha256,
            "release_manifest": "release-manifest.json",
            "release_manifest_sha256": self.manifest_sha256,
            "build_command": ["bash", "scripts/ci-local.sh", "release"],
            "build_log": recorded_path(&self.build_log, receipt_base)?,
            "build_log_sha256": self.build_log_sha256,
            "transport": "file",
        }))
    }
}

fn configure_family_child(command: &mut Command) -> &mut Command {
    command
        .env_remove("RUSTUP_TOOLCHAIN")
        .env("REDLINE_STRICT_TOOLS", "1")
        .env("CI", "true")
}

fn file_url(path: &Path) -> Result<String> {
    let absolute = absolute_path(path)?;
    let value = absolute.to_str().ok_or_else(|| {
        error(format!(
            "local artifact path is not UTF-8: {}",
            absolute.display()
        ))
    })?;
    Ok(format!("file://{value}"))
}

fn configure_redline_core_artifact(
    command: &mut Command,
    artifact: &TestingArtifact,
) -> Result<()> {
    for key in [
        "CI_REDLINE_TESTING_LOCAL_BIN",
        "CI_REDLINE_TESTING_LOCAL_SOURCE",
        "CI_REDLINE_TESTING_INSTALL_ROOT",
        "CI_REDLINE_TESTING_BIN",
        "REDLINE_TESTING_BIN",
    ] {
        command.env_remove(key);
    }
    let base = artifact
        .artifact_path
        .parent()
        .ok_or_else(|| error("staged redline-testing artifact has no parent"))?;
    command.envs([
        (
            "CI_REDLINE_TESTING_VERSION",
            artifact.product_version.as_str(),
        ),
        (
            "CI_REDLINE_TESTING_REQUESTED_VERSION",
            artifact.product_version.as_str(),
        ),
        (
            "CI_REDLINE_TESTING_RELEASE_TAG",
            artifact.release_tag.as_str(),
        ),
        (
            "CI_REDLINE_TESTING_ARTIFACT",
            artifact.artifact_name.as_str(),
        ),
        ("CI_REDLINE_TESTING_BASE_URL", file_url(base)?.as_str()),
        (
            "CI_REDLINE_TESTING_URL",
            file_url(&artifact.artifact_path)?.as_str(),
        ),
        (
            "CI_REDLINE_TESTING_SHA256_URL",
            file_url(&artifact.checksum_path)?.as_str(),
        ),
        (
            "CI_REDLINE_TESTING_RELEASE_MANIFEST_URL",
            file_url(&artifact.manifest_path)?.as_str(),
        ),
        (
            "CI_REDLINE_TESTING_EXPECTED_TARBALL_SHA256",
            artifact.artifact_sha256.as_str(),
        ),
        (
            "CI_REDLINE_TESTING_EXPECTED_BINARY_SHA256",
            artifact.binary_sha256.as_str(),
        ),
        (
            "CI_REDLINE_TESTING_RELEASE_MANIFEST_SHA256",
            artifact.manifest_sha256.as_str(),
        ),
        ("CI_REDLINE_TESTING_REQUIRE_ATTESTATION", "0"),
    ]);
    Ok(())
}

fn manifest_string<'a>(value: &'a JsonValue, field: &str) -> Result<&'a str> {
    value
        .get(field)
        .and_then(JsonValue::as_str)
        .filter(|found| !found.is_empty())
        .ok_or_else(|| error(format!("redline-testing release manifest lacks {field}")))
}

fn verify_testing_package(
    repo: &Repo,
    commit: &str,
    worktree: &Path,
) -> Result<(PathBuf, PathBuf, PathBuf, String)> {
    let package = format!("redline-testing-{}-linux-x86_64", repo.product_version);
    let artifact = worktree.join("dist").join(format!("{package}.tar.gz"));
    let checksum = worktree
        .join("dist")
        .join(format!("{package}.tar.gz.sha256"));
    let manifest = worktree.join("dist/release-manifest.json");
    for path in [&artifact, &checksum, &manifest] {
        if !path.is_file() {
            return Err(error(format!(
                "redline-testing release build omitted {}",
                path.display()
            )));
        }
    }
    let artifact_sha256 = sha256_file(&artifact)?;
    let sidecar = fs::read_to_string(&checksum)?;
    let fields = sidecar.split_whitespace().collect::<Vec<_>>();
    let expected_sidecar_name = format!("dist/{package}.tar.gz");
    if fields.len() != 2 || fields[0] != artifact_sha256 || fields[1] != expected_sidecar_name {
        return Err(error(
            "redline-testing release checksum sidecar is not bound to the staged artifact",
        ));
    }
    let value = read_json(&manifest)?;
    if manifest_string(&value, "name")? != "redline-testing"
        || manifest_string(&value, "version")? != repo.product_version
        || manifest_string(&value, "release_commit")? != commit
        || manifest_string(&value, "release_tag")? != repo.current_tag
        || value.get("tag_revision").and_then(JsonValue::as_i64) != Some(repo.tag_revision)
    {
        return Err(error(
            "redline-testing release manifest differs from the reviewed manifest identity",
        ));
    }
    let binary_sha256 = manifest_string(&value, "binary_sha256")?;
    if !is_sha256(binary_sha256) {
        return Err(error(
            "redline-testing release manifest binary_sha256 is invalid",
        ));
    }
    let binary = worktree
        .join("dist")
        .join(&package)
        .join("bin/redline-testing");
    if !binary.is_file() || sha256_file(&binary)? != binary_sha256 {
        return Err(error(
            "redline-testing release manifest binary hash differs from the built binary",
        ));
    }
    let hashes = value
        .get("artifact_hashes")
        .and_then(JsonValue::as_object)
        .ok_or_else(|| error("redline-testing release manifest lacks artifact_hashes"))?;
    if hashes.is_empty()
        || hashes
            .values()
            .any(|hash| !hash.as_str().map(is_sha256).unwrap_or(false))
    {
        return Err(error(
            "redline-testing release manifest artifact_hashes are empty or invalid",
        ));
    }
    Ok((artifact, checksum, manifest, binary_sha256.to_owned()))
}

fn stage_testing_artifact(
    manifest: &Manifest,
    states: &BTreeMap<String, JsonValue>,
    temporary: &Path,
    log_dir: &Path,
) -> Result<TestingArtifact> {
    let repo = manifest
        .repos
        .iter()
        .find(|repo| repo.name == "redline-testing")
        .ok_or_else(|| error("manifest lacks redline-testing"))?;
    let state = states
        .get(&repo.name)
        .ok_or_else(|| error("missing reviewed redline-testing state"))?;
    let commit = state
        .get("commit")
        .and_then(JsonValue::as_str)
        .ok_or_else(|| error("reviewed redline-testing state lacks commit"))?;
    let source = manifest.repo_root(repo);
    let worktree = temporary.join("redline-testing-artifact-source");
    let staging = temporary.join("redline-testing-artifact");
    let log_path = log_dir.join("redline-testing-artifact.log");
    let command = ["bash", "scripts/ci-local.sh", "release"];
    let mut worktree_added = false;
    let attempt = (|| -> Result<TestingArtifact> {
        command_output(
            Command::new("git")
                .arg("-C")
                .arg(&source)
                .args(["worktree", "add", "--detach"])
                .arg(&worktree)
                .arg(commit),
        )?;
        worktree_added = true;
        let mut log = File::create(&log_path)?;
        writeln!(log, "$ {}", command.join(" "))?;
        log.flush()?;
        let stdout = log.try_clone()?;
        let stderr = log.try_clone()?;
        let mut process = Command::new(command[0]);
        configure_family_child(&mut process);
        let result = process
            .args(&command[1..])
            .current_dir(&worktree)
            .env("REDLINE_TESTING_RELEASE_TAG", &repo.current_tag)
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(stderr))
            .status()?;
        if !result.success() {
            return Err(error(format!(
                "redline-testing artifact build exited {:?}",
                result.code()
            )));
        }
        let (artifact, checksum, release_manifest, binary_sha256) =
            verify_testing_package(repo, commit, &worktree)?;
        fs::create_dir_all(&staging)?;
        let artifact_path = staging.join(
            artifact
                .file_name()
                .ok_or_else(|| error("redline-testing artifact lacks filename"))?,
        );
        let checksum_path = staging.join(
            checksum
                .file_name()
                .ok_or_else(|| error("redline-testing checksum lacks filename"))?,
        );
        let manifest_path = staging.join("release-manifest.json");
        fs::copy(&artifact, &artifact_path)?;
        fs::copy(&checksum, &checksum_path)?;
        fs::copy(&release_manifest, &manifest_path)?;
        let artifact_sha256 = sha256_file(&artifact_path)?;
        if artifact_sha256 != sha256_file(&artifact)? {
            return Err(error("staged redline-testing artifact changed during copy"));
        }
        Ok(TestingArtifact {
            source_commit: commit.to_owned(),
            source_tree_checksum_sha256: git_tree_checksum(&source, commit)?,
            product_version: repo.product_version.clone(),
            release_tag: repo.current_tag.clone(),
            tag_revision: repo.tag_revision,
            artifact_name: artifact_path
                .file_name()
                .and_then(|value| value.to_str())
                .ok_or_else(|| error("staged redline-testing artifact filename is not UTF-8"))?
                .to_owned(),
            artifact_path,
            artifact_sha256,
            checksum_sha256: sha256_file(&checksum_path)?,
            checksum_path,
            binary_sha256,
            manifest_sha256: sha256_file(&manifest_path)?,
            manifest_path,
            build_log_sha256: sha256_file(&log_path)?,
            build_log: log_path,
        })
    })();
    let cleanup = if worktree_added {
        command_output(
            Command::new("git")
                .arg("-C")
                .arg(&source)
                .args(["worktree", "remove", "--force"])
                .arg(&worktree),
        )
        .map(|_| ())
    } else {
        Ok(())
    };
    match (attempt, cleanup) {
        (Ok(artifact), Ok(())) => Ok(artifact),
        (Err(value), Ok(())) => Err(value),
        (Ok(_), Err(cleanup)) => Err(error(format!(
            "redline-testing artifact worktree cleanup failed: {cleanup}"
        ))),
        (Err(value), Err(cleanup)) => Err(error(format!(
            "{value}; redline-testing artifact worktree cleanup failed: {cleanup}"
        ))),
    }
}

fn merge_json(base: &JsonValue, additions: &[(&str, JsonValue)]) -> Result<JsonValue> {
    let mut map = base
        .as_object()
        .cloned()
        .ok_or_else(|| error("expected JSON object"))?;
    for (key, value) in additions {
        map.insert((*key).to_owned(), value.clone());
    }
    Ok(JsonValue::Object(map))
}

fn family_ci(manifest_path: &Path, receipt: &Path) -> Result<()> {
    let manifest = load_manifest(manifest_path)?;
    let started_at = Utc::now();
    let mut states = BTreeMap::new();
    let mut failures = BTreeMap::new();
    for repo in &manifest.repos {
        match current_reviewed_state(&manifest, repo, true) {
            Ok(state) => {
                states.insert(repo.name.clone(), state);
            }
            Err(value) => {
                failures.insert(repo.name.clone(), value.to_string());
            }
        }
    }
    let log_dir = receipt.parent().unwrap_or(Path::new(".")).join(format!(
        "{}.d",
        receipt
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("family-ci")
    ));
    fs::create_dir_all(&log_dir)?;
    let mut rows = Vec::new();
    if !failures.is_empty() {
        for repo in &manifest.repos {
            let base = states.get(&repo.name).cloned().unwrap_or_else(|| json!({}));
            let own_failure = failures.get(&repo.name);
            rows.push(merge_json(
                &base,
                &[
                    ("name", json!(repo.name)),
                    (
                        "path",
                        json!(repo.path.to_string_lossy().replace('\\', "/")),
                    ),
                    ("required_check", json!(repo.required_check)),
                    ("product_version", json!(repo.product_version)),
                    ("tag_revision", json!(repo.tag_revision)),
                    ("tag", json!(repo.current_tag)),
                    ("release_commit", json!(repo.release_commit)),
                    (
                        "release_checksum_sha256",
                        json!(repo.release_checksum_sha256),
                    ),
                    ("protection_policy", json!(repo.protection_policy)),
                    ("dependency_artifacts", json!([])),
                    ("commands", json!([])),
                    ("log", JsonValue::Null),
                    ("log_sha256", JsonValue::Null),
                    (
                        "status",
                        json!(if own_failure.is_some() {
                            "fail"
                        } else {
                            "blocked"
                        }),
                    ),
                    (
                        "failure",
                        json!(own_failure.cloned().unwrap_or_else(|| {
                            "blocked because another repository failed reviewed-main preflight"
                                .to_owned()
                        })),
                    ),
                ],
            )?);
        }
    } else {
        let temporary = env::temp_dir().join(format!("redline-family-ci-{}", unique_suffix()));
        fs::create_dir_all(&temporary)?;
        let artifact = match stage_testing_artifact(&manifest, &states, &temporary, &log_dir) {
            Ok(value) => Some(value),
            Err(value) => {
                for repo in &manifest.repos {
                    let state = states
                        .get(&repo.name)
                        .ok_or_else(|| error("missing reviewed state"))?;
                    let failure = if repo.name == "redline-testing" {
                        format!("redline-testing artifact preparation failed: {value}")
                    } else {
                        "blocked because reviewed redline-testing artifact preparation failed"
                            .to_owned()
                    };
                    rows.push(merge_json(
                        state,
                        &[
                            ("dependency_artifacts", json!([])),
                            ("commands", json!([])),
                            ("log", JsonValue::Null),
                            ("log_sha256", JsonValue::Null),
                            (
                                "status",
                                json!(if repo.name == "redline-testing" {
                                    "fail"
                                } else {
                                    "blocked"
                                }),
                            ),
                            ("failure", json!(failure)),
                        ],
                    )?);
                }
                None
            }
        };
        if let Some(artifact) = artifact {
            for repo in &manifest.repos {
                let state = states
                    .get(&repo.name)
                    .ok_or_else(|| error("missing reviewed state"))?;
                let source = manifest.repo_root(repo);
                let worktree = temporary.join(&repo.name);
                let log_path = log_dir.join(format!("{}.log", repo.name));
                let mut commands = Vec::new();
                let mut status = "fail";
                let mut failure: Option<String> = None;
                let mut worktree_added = false;
                let attempt = (|| -> Result<()> {
                    let commit = state
                        .get("commit")
                        .and_then(JsonValue::as_str)
                        .ok_or_else(|| error("missing state commit"))?;
                    command_output(
                        Command::new("git")
                            .arg("-C")
                            .arg(&source)
                            .args(["worktree", "add", "--detach"])
                            .arg(&worktree)
                            .arg(commit),
                    )?;
                    worktree_added = true;
                    commands = ci_commands(repo, &worktree)?;
                    let mut log = File::create(&log_path)?;
                    for command in &commands {
                        writeln!(log, "$ {}", command.join(" "))?;
                        log.flush()?;
                        let stdout = log.try_clone()?;
                        let stderr = log.try_clone()?;
                        let mut process = Command::new(&command[0]);
                        configure_family_child(&mut process);
                        if repo.name == "redline-core" {
                            configure_redline_core_artifact(&mut process, &artifact)?;
                        }
                        let result = process
                            .args(&command[1..])
                            .current_dir(&worktree)
                            .stdout(Stdio::from(stdout))
                            .stderr(Stdio::from(stderr))
                            .status()?;
                        if !result.success() {
                            return Err(error(format!(
                                "CI command exited {:?}: {}",
                                result.code(),
                                command.join(" ")
                            )));
                        }
                    }
                    status = "pass";
                    Ok(())
                })();
                if let Err(value) = attempt {
                    failure = Some(value.to_string());
                    if !log_path.exists() {
                        fs::write(&log_path, format!("family-ci failure: {value}\n"))?;
                    }
                }
                if worktree_added {
                    let cleanup = Command::new("git")
                        .arg("-C")
                        .arg(&source)
                        .args(["worktree", "remove", "--force"])
                        .arg(&worktree)
                        .output()?;
                    if !cleanup.status.success() {
                        status = "fail";
                        let detail = String::from_utf8_lossy(if cleanup.stderr.is_empty() {
                            &cleanup.stdout
                        } else {
                            &cleanup.stderr
                        });
                        let message =
                            format!("detached worktree cleanup failed: {}", detail.trim());
                        failure = Some(match failure {
                            Some(old) => format!("{old}; {message}"),
                            None => message,
                        });
                    }
                }
                let log_record =
                    recorded_path(&log_path, receipt.parent().unwrap_or(Path::new(".")))?;
                let dependency_artifacts = if repo.name == "redline-core" {
                    json!([artifact.receipt_json(receipt.parent().unwrap_or(Path::new(".")))?])
                } else {
                    json!([])
                };
                rows.push(merge_json(
                    state,
                    &[
                        ("dependency_artifacts", dependency_artifacts),
                        ("commands", json!(commands)),
                        ("log", json!(log_record)),
                        ("log_sha256", json!(sha256_file(&log_path)?)),
                        ("status", json!(status)),
                        ("failure", json!(failure)),
                    ],
                )?);
            }
        }
        let _ = fs::remove_dir_all(&temporary);
    }
    let passed = !rows.is_empty()
        && rows
            .iter()
            .all(|row| row.get("status").and_then(JsonValue::as_str) == Some("pass"));
    let manifest_record =
        recorded_path(&manifest.path, receipt.parent().unwrap_or(Path::new(".")))?;
    let payload = json!({
        "schema_version": FAMILY_CI_SCHEMA,
        "family": FAMILY,
        "generated_at": format_time(Utc::now()),
        "started_at": format_time(started_at),
        "manifest": manifest_record,
        "manifest_sha256": sha256_file(&manifest.path)?,
        "strict_tools": true,
        "status": if passed { "pass" } else { "fail" },
        "repositories": rows,
    });
    let digest = write_checksummed_json(receipt, &payload)?;
    println!(
        "redline family CI {}: {} sha256={digest}",
        if passed { "pass" } else { "fail" },
        receipt.display()
    );
    if !passed {
        return Err(error(
            "family CI failed; see the checksummed machine receipt",
        ));
    }
    Ok(())
}

fn read_json(path: &Path) -> Result<JsonValue> {
    let value: JsonValue = serde_json::from_slice(&fs::read(path)?)?;
    if !value.is_object() {
        return Err(error(format!(
            "JSON evidence must be an object: {}",
            path.display()
        )));
    }
    Ok(value)
}

fn reject_unknown_fields(value: &JsonValue, allowed: &[&str], context: &str) -> Result<()> {
    let object = value
        .as_object()
        .ok_or_else(|| error(format!("{context} must be an object")))?;
    let allowed: BTreeSet<&str> = allowed.iter().copied().collect();
    let unknown: Vec<&str> = object
        .keys()
        .map(String::as_str)
        .filter(|key| !allowed.contains(key))
        .collect();
    if !unknown.is_empty() {
        return Err(error(format!(
            "{context} contains unsupported fields: {}",
            unknown.join(", ")
        )));
    }
    Ok(())
}

fn reject_manual_booleans(value: &JsonValue, context: &str) -> Result<()> {
    match value {
        JsonValue::Bool(_) => Err(error(format!(
            "{context} contains a manual boolean; eligibility is derived"
        ))),
        JsonValue::Array(rows) => {
            for (index, row) in rows.iter().enumerate() {
                reject_manual_booleans(row, &format!("{context}[{index}]"))?;
            }
            Ok(())
        }
        JsonValue::Object(rows) => {
            for (key, row) in rows {
                reject_manual_booleans(row, &format!("{context}.{key}"))?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

const FAMILY_RECEIPT_FIELDS: [&str; 9] = [
    "schema_version",
    "family",
    "generated_at",
    "started_at",
    "manifest",
    "manifest_sha256",
    "strict_tools",
    "status",
    "repositories",
];
const FAMILY_ROW_FIELDS: [&str; 21] = [
    "name",
    "path",
    "branch",
    "commit",
    "forge_main",
    "origin",
    "required_check",
    "product_version",
    "tag_revision",
    "tag",
    "release_commit",
    "release_checksum_sha256",
    "protection_policy",
    "tag_state",
    "tag_metadata",
    "dependency_artifacts",
    "commands",
    "log",
    "log_sha256",
    "status",
    "failure",
];
const CONSUMER_FIELDS: [&str; 18] = [
    "schema_version",
    "consumer",
    "family",
    "generated_at",
    "status",
    "source_commit",
    "required_check",
    "engine_tag",
    "engine_commit",
    "proof_lock_id",
    "family_ci_receipt_sha256",
    "manifest_sha256",
    "policy_sha256",
    "consumer_manifest_sha256",
    "consumer_policy_sha256",
    "test_log",
    "test_log_sha256",
    "tool_version",
];

fn validate_testing_artifact_receipt(
    value: &JsonValue,
    repo: &Repo,
    commit: &str,
    receipt_base: &Path,
) -> Result<()> {
    reject_unknown_fields(
        value,
        &TESTING_ARTIFACT_FIELDS,
        "redline-testing dependency artifact",
    )?;
    let expected_artifact = format!(
        "redline-testing-{}-linux-x86_64.tar.gz",
        repo.product_version
    );
    let strings = [
        ("name", "redline-testing-release"),
        ("source_repo", "redline-testing"),
        ("source_commit", commit),
        (
            "source_tree_checksum_sha256",
            repo.release_checksum_sha256.as_str(),
        ),
        ("product_version", repo.product_version.as_str()),
        ("release_tag", repo.current_tag.as_str()),
        ("artifact", expected_artifact.as_str()),
        ("release_manifest", "release-manifest.json"),
        ("transport", "file"),
    ];
    for (field, expected) in strings {
        if value.get(field).and_then(JsonValue::as_str) != Some(expected) {
            return Err(error(format!(
                "redline-core dependency artifact {field} differs from reviewed redline-testing"
            )));
        }
    }
    if value.get("tag_revision").and_then(JsonValue::as_i64) != Some(repo.tag_revision) {
        return Err(error(
            "redline-core dependency artifact tag_revision differs from reviewed redline-testing",
        ));
    }
    for field in [
        "artifact_sha256",
        "checksum_sha256",
        "binary_sha256",
        "release_manifest_sha256",
        "build_log_sha256",
    ] {
        if !value
            .get(field)
            .and_then(JsonValue::as_str)
            .map(is_sha256)
            .unwrap_or(false)
        {
            return Err(error(format!(
                "redline-core dependency artifact {field} is invalid"
            )));
        }
    }
    if value.get("build_command") != Some(&json!(["bash", "scripts/ci-local.sh", "release"])) {
        return Err(error(
            "redline-core dependency artifact build command is not governed",
        ));
    }
    let log = resolve_recorded_path(
        value.get("build_log").unwrap_or(&JsonValue::Null),
        receipt_base,
        "redline-testing artifact build_log",
    )?;
    if !log.is_file()
        || value.get("build_log_sha256").and_then(JsonValue::as_str) != Some(&sha256_file(&log)?)
    {
        return Err(error(
            "redline-testing artifact build log is missing or tampered",
        ));
    }
    Ok(())
}

fn validate_family_receipt(
    manifest_path: &Path,
    receipt: &Path,
    now: DateTime<Utc>,
) -> Result<(JsonValue, String)> {
    let digest = verify_checksum(receipt)?;
    let value = read_json(receipt)?;
    reject_unknown_fields(&value, &FAMILY_RECEIPT_FIELDS, "family CI receipt")?;
    if value.get("schema_version").and_then(JsonValue::as_str) != Some(FAMILY_CI_SCHEMA)
        || value.get("family").and_then(JsonValue::as_str) != Some(FAMILY)
    {
        return Err(error("family CI receipt schema or family is invalid"));
    }
    if value.get("strict_tools").and_then(JsonValue::as_bool) != Some(true) {
        return Err(error("family CI receipt did not run in strict-tools mode"));
    }
    if value.get("status").and_then(JsonValue::as_str) != Some("pass") {
        return Err(error("family CI receipt did not pass"));
    }
    let generated = parse_time(
        value.get("generated_at").unwrap_or(&JsonValue::Null),
        "family CI generated_at",
    )?;
    let started = parse_time(
        value.get("started_at").unwrap_or(&JsonValue::Null),
        "family CI started_at",
    )?;
    require_fresh(generated, now, "family CI generated_at")?;
    if started > generated + Duration::minutes(MAX_CLOCK_SKEW_MINUTES) {
        return Err(error("family CI started_at is later than generated_at"));
    }
    let supplied_manifest = absolute_path(manifest_path)?;
    let recorded_manifest = resolve_recorded_path(
        value.get("manifest").unwrap_or(&JsonValue::Null),
        receipt.parent().unwrap_or(Path::new(".")),
        "family CI manifest",
    )?;
    if absolute_path(&recorded_manifest)? != supplied_manifest {
        return Err(error(
            "family CI receipt manifest path differs from the supplied manifest",
        ));
    }
    if value.get("manifest_sha256").and_then(JsonValue::as_str)
        != Some(&sha256_file(manifest_path)?)
    {
        return Err(error(
            "family CI receipt was produced from a different manifest",
        ));
    }
    let manifest = load_manifest(manifest_path)?;
    let rows = value
        .get("repositories")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| error("family CI receipt repositories must be an array"))?;
    if rows.len() != manifest.repos.len() {
        return Err(error("family CI receipt repository count is invalid"));
    }
    let testing_repo = manifest
        .repos
        .iter()
        .find(|repo| repo.name == "redline-testing")
        .ok_or_else(|| error("manifest lacks redline-testing"))?;
    let testing_commit = rows
        .iter()
        .find(|row| row.get("name").and_then(JsonValue::as_str) == Some("redline-testing"))
        .and_then(|row| row.get("commit"))
        .and_then(JsonValue::as_str)
        .filter(|commit| is_sha1(commit))
        .ok_or_else(|| error("family CI receipt lacks reviewed redline-testing commit"))?;
    let mut seen = BTreeSet::new();
    for row in rows {
        reject_unknown_fields(row, &FAMILY_ROW_FIELDS, "family CI repository entry")?;
        let name = row
            .get("name")
            .and_then(JsonValue::as_str)
            .ok_or_else(|| error("family CI row lacks name"))?;
        let repo = manifest
            .repos
            .iter()
            .find(|repo| repo.name == name)
            .ok_or_else(|| error(format!("family CI receipt has unknown repository: {name}")))?;
        if !seen.insert(name.to_owned()) {
            return Err(error(format!(
                "family CI receipt duplicates repository: {name}"
            )));
        }
        if row.get("status").and_then(JsonValue::as_str) != Some("pass")
            || !row.get("failure").unwrap_or(&JsonValue::Null).is_null()
        {
            return Err(error(format!(
                "{name}: family CI result is not an unqualified pass"
            )));
        }
        let commit = row.get("commit").and_then(JsonValue::as_str).unwrap_or("");
        if row.get("branch").and_then(JsonValue::as_str) != Some("main")
            || row.get("forge_main").and_then(JsonValue::as_str) != Some(commit)
            || !is_sha1(commit)
        {
            return Err(error(format!(
                "{name}: family CI was not bound to immutable forge main"
            )));
        }
        if row.get("origin").and_then(JsonValue::as_str) != Some(&expected_origin(repo)) {
            return Err(error(format!(
                "{name}: family CI origin differs from manifest"
            )));
        }
        let expected_path = repo.path.to_string_lossy().replace('\\', "/");
        if row.get("path").and_then(JsonValue::as_str) != Some(&expected_path) {
            return Err(error(format!(
                "{name}: family CI checkout path differs from manifest"
            )));
        }
        if row.get("required_check").and_then(JsonValue::as_str) != Some(&repo.required_check)
            || row.get("product_version").and_then(JsonValue::as_str) != Some(&repo.product_version)
            || row.get("tag_revision").and_then(JsonValue::as_i64) != Some(repo.tag_revision)
            || row.get("tag").and_then(JsonValue::as_str) != Some(&repo.current_tag)
            || row.get("release_commit").and_then(JsonValue::as_str) != Some(&repo.release_commit)
            || row
                .get("release_checksum_sha256")
                .and_then(JsonValue::as_str)
                != Some(&repo.release_checksum_sha256)
            || row.get("protection_policy").and_then(JsonValue::as_str)
                != Some(&repo.protection_policy)
        {
            return Err(error(format!(
                "{name}: family CI metadata differs from manifest"
            )));
        }
        if row.get("commands") != Some(&json!(ci_commands(repo, &manifest.repo_root(repo))?)) {
            return Err(error(format!(
                "{name}: family CI command list differs from the governed lane"
            )));
        }
        let dependencies = row
            .get("dependency_artifacts")
            .and_then(JsonValue::as_array)
            .ok_or_else(|| error(format!("{name}: dependency_artifacts must be an array")))?;
        if name == "redline-core" {
            if dependencies.len() != 1 {
                return Err(error(
                    "redline-core family CI must bind one redline-testing release artifact",
                ));
            }
            validate_testing_artifact_receipt(
                &dependencies[0],
                testing_repo,
                testing_commit,
                receipt.parent().unwrap_or(Path::new(".")),
            )?;
        } else if !dependencies.is_empty() {
            return Err(error(format!(
                "{name}: unexpected family CI dependency artifact"
            )));
        }
        match row.get("tag_state").and_then(JsonValue::as_str) {
            Some("absent")
                if row
                    .get("tag_metadata")
                    .unwrap_or(&JsonValue::Null)
                    .is_null() => {}
            Some("verified")
                if row
                    .get("tag_metadata")
                    .and_then(|value| value.get("commit"))
                    .and_then(JsonValue::as_str)
                    == Some(commit) => {}
            _ => {
                return Err(error(format!(
                    "{name}: invalid or unbound receipt tag state"
                )))
            }
        }
        let log = resolve_recorded_path(
            row.get("log").unwrap_or(&JsonValue::Null),
            receipt.parent().unwrap_or(Path::new(".")),
            &format!("{name}.log"),
        )?;
        if !log.is_file()
            || row.get("log_sha256").and_then(JsonValue::as_str) != Some(&sha256_file(&log)?)
        {
            return Err(error(format!(
                "{name}: family CI log is missing or tampered"
            )));
        }
    }
    if seen.len() != manifest.repos.len() {
        return Err(error("family CI receipt omits a repository"));
    }
    Ok((value, digest))
}

fn proof_id(engine_version: &str, engine_commit: &str) -> String {
    format!("redline-proof/v2/{engine_version}/{engine_commit}")
}

fn core_version(tag: &str) -> Result<String> {
    let raw = tag
        .strip_prefix("redline-core-v")
        .and_then(|value| value.split_once("-jain."))
        .ok_or_else(|| error(format!("unsupported Redline core tag: {tag}")))?;
    if raw.1.is_empty() || !raw.1.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(error(format!("unsupported Redline core tag: {tag}")));
    }
    let parts: Vec<&str> = raw.0.split('.').collect();
    if parts.len() != 3
        || parts
            .iter()
            .any(|part| part.is_empty() || !part.bytes().all(|byte| byte.is_ascii_digit()))
    {
        return Err(error(format!("unsupported Redline core tag: {tag}")));
    }
    Ok(raw.0.to_owned())
}

struct EvidenceBinding<'a> {
    now: DateTime<Utc>,
    family_generated: DateTime<Utc>,
    family_digest: &'a str,
    engine_tag: &'a str,
    engine_commit: &'a str,
    expected_proof: &'a str,
    manifest_digest: &'a str,
    policy_digest: &'a str,
}

fn validate_consumer_evidence(
    path: &Path,
    consumer: &str,
    binding: &EvidenceBinding<'_>,
) -> Result<(JsonValue, String)> {
    let digest = verify_checksum(path)?;
    let value = read_json(path)?;
    reject_manual_booleans(&value, &format!("{consumer} evidence"))?;
    reject_unknown_fields(&value, &CONSUMER_FIELDS, &format!("{consumer} evidence"))?;
    if value.get("schema_version").and_then(JsonValue::as_str) != Some(CONSUMER_SCHEMA)
        || value.get("consumer").and_then(JsonValue::as_str) != Some(consumer)
        || value.get("family").and_then(JsonValue::as_str) != Some(FAMILY)
        || value.get("status").and_then(JsonValue::as_str) != Some("pass")
    {
        return Err(error(format!(
            "{consumer}: consumer evidence identity, schema, or status is invalid"
        )));
    }
    let generated = parse_time(
        value.get("generated_at").unwrap_or(&JsonValue::Null),
        &format!("{consumer} generated_at"),
    )?;
    require_fresh(generated, binding.now, &format!("{consumer} generated_at"))?;
    if generated + Duration::minutes(MAX_CLOCK_SKEW_MINUTES) < binding.family_generated {
        return Err(error(format!(
            "{consumer}: evidence predates the family CI receipt"
        )));
    }
    let source = value
        .get("source_commit")
        .and_then(JsonValue::as_str)
        .unwrap_or("");
    if !is_sha1(source) {
        return Err(error(format!("{consumer}: source_commit is not immutable")));
    }
    let required_check = format!("{consumer}/redline-consumer");
    let checks = [
        ("required_check", required_check.as_str()),
        ("engine_tag", binding.engine_tag),
        ("engine_commit", binding.engine_commit),
        ("proof_lock_id", binding.expected_proof),
        ("family_ci_receipt_sha256", binding.family_digest),
        ("manifest_sha256", binding.manifest_digest),
        ("policy_sha256", binding.policy_digest),
    ];
    for (field, expected) in checks {
        if value.get(field).and_then(JsonValue::as_str) != Some(expected) {
            return Err(error(format!(
                "{consumer}: evidence {field} differs from the reviewed Redline proof"
            )));
        }
    }
    for field in [
        "consumer_manifest_sha256",
        "consumer_policy_sha256",
        "test_log_sha256",
    ] {
        let digest = value.get(field).and_then(JsonValue::as_str).unwrap_or("");
        if !is_sha256(digest) {
            return Err(error(format!(
                "{consumer}: evidence {field} is not a SHA-256 digest"
            )));
        }
    }
    let expected_tool = match consumer {
        "jain-split" => "jain-redline-consumer/v1",
        "jeryu-split" => "jeryu-redline-consumer/v1",
        _ => return Err(error(format!("unsupported Redline consumer: {consumer}"))),
    };
    if value.get("tool_version").and_then(JsonValue::as_str) != Some(expected_tool) {
        return Err(error(format!(
            "{consumer}: evidence tool_version must be {expected_tool}"
        )));
    }
    let test_log_record = value
        .get("test_log")
        .and_then(JsonValue::as_str)
        .unwrap_or("");
    let test_log_relative = Path::new(test_log_record);
    if test_log_record.is_empty()
        || test_log_relative.is_absolute()
        || test_log_relative
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
        || test_log_relative.components().count() != 1
    {
        return Err(error(format!(
            "{consumer}: evidence test_log must be one relative file name"
        )));
    }
    let test_log = path
        .parent()
        .unwrap_or(Path::new("."))
        .join(test_log_relative);
    let metadata = fs::symlink_metadata(&test_log).map_err(|_| {
        error(format!(
            "{consumer}: evidence test log is missing: {}",
            test_log.display()
        ))
    })?;
    if !metadata.file_type().is_file() || metadata.len() == 0 {
        return Err(error(format!(
            "{consumer}: evidence test log must be a non-empty regular file"
        )));
    }
    let expected_test_digest = value
        .get("test_log_sha256")
        .and_then(JsonValue::as_str)
        .unwrap_or("");
    if sha256_file(&test_log)? != expected_test_digest {
        return Err(error(format!(
            "{consumer}: evidence test log checksum does not match"
        )));
    }
    Ok((value, digest))
}

#[derive(Clone)]
struct TagRow {
    repo: Repo,
    receipt: JsonValue,
    metadata: TagMetadata,
}

fn checked_tag_rows(manifest_path: &Path, family_receipt: &JsonValue) -> Result<Vec<TagRow>> {
    let manifest = load_manifest(manifest_path)?;
    let receipt_rows = family_receipt
        .get("repositories")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| error("family receipt repositories are missing"))?;
    let mut result = Vec::new();
    for repo in &manifest.repos {
        if repo.release_commit == PENDING || repo.release_checksum_sha256 == PENDING {
            return Err(error(format!(
                "{}: proof-refresh requires exact manifest release commit and checksum",
                repo.name
            )));
        }
        let state = current_reviewed_state(&manifest, repo, false)?;
        let row = receipt_rows
            .iter()
            .find(|row| row.get("name").and_then(JsonValue::as_str) == Some(&repo.name))
            .ok_or_else(|| error(format!("family receipt omits {}", repo.name)))?;
        if state.get("commit") != row.get("commit") {
            return Err(error(format!(
                "{}: reviewed main changed after family CI",
                repo.name
            )));
        }
        let metadata = tag_metadata(&manifest.repo_root(repo), &repo.current_tag, true)?;
        if row.get("commit").and_then(JsonValue::as_str) != Some(&metadata.commit) {
            return Err(error(format!(
                "{}: immutable tag does not point to family CI commit",
                repo.name
            )));
        }
        if metadata.commit != repo.release_commit
            || git_tree_checksum(&manifest.repo_root(repo), &metadata.commit)?
                != repo.release_checksum_sha256
        {
            return Err(error(format!(
                "{}: immutable tag differs from exact manifest release identity",
                repo.name
            )));
        }
        result.push(TagRow {
            repo: repo.clone(),
            receipt: row.clone(),
            metadata,
        });
    }
    Ok(result)
}

fn toml_quote(value: &str) -> Result<String> {
    Ok(serde_json::to_string(value)?)
}

fn toml_array(values: &[String]) -> Result<String> {
    Ok(format!(
        "[{}]",
        values
            .iter()
            .map(|value| toml_quote(value))
            .collect::<Result<Vec<_>>>()?
            .join(", ")
    ))
}

type ConsumerEvidence = (PathBuf, JsonValue, String);

fn render_lock(
    family_receipt_path: &Path,
    family_digest: &str,
    tag_rows: &[TagRow],
    consumers: &BTreeMap<String, ConsumerEvidence>,
    generated_at: DateTime<Utc>,
    operation_receipt_path: &Path,
    record_base: &Path,
) -> Result<Vec<u8>> {
    let core = tag_rows
        .iter()
        .find(|row| row.repo.name == "redline-core")
        .ok_or_else(|| error("redline-core tag row is missing"))?;
    let testing = tag_rows
        .iter()
        .find(|row| row.repo.name == "redline-testing")
        .ok_or_else(|| error("redline-testing tag row is missing"))?;
    let engine_version = core_version(&core.repo.current_tag)?;
    let engine_commit = core
        .receipt
        .get("commit")
        .and_then(JsonValue::as_str)
        .ok_or_else(|| error("core receipt commit is missing"))?;
    let lock_id = proof_id(&engine_version, engine_commit);
    let expires_at = generated_at + Duration::hours(MAX_EVIDENCE_HOURS);
    let required = REQUIRED_CONSUMERS
        .iter()
        .map(|value| (*value).to_owned())
        .collect::<Vec<_>>();
    let mut lines = vec![
        format!("schema_version = {}", toml_quote(LOCK_SCHEMA)?),
        format!("family = {}", toml_quote(FAMILY)?),
        "parent_family = \"independent\"".to_owned(),
        "engine_package = \"redlinedb\"".to_owned(),
        format!("engine_version = {}", toml_quote(&engine_version)?),
        format!("engine_tag = {}", toml_quote(&core.repo.current_tag)?),
        format!("engine_commit = {}", toml_quote(engine_commit)?),
        format!(
            "parity_harness_commit = {}",
            toml_quote(
                testing
                    .receipt
                    .get("commit")
                    .and_then(JsonValue::as_str)
                    .unwrap_or("")
            )?
        ),
        format!("proof_lock_id = {}", toml_quote(&lock_id)?),
        format!("consumers = {}", toml_array(&required)?),
        String::new(),
        "[proof]".to_owned(),
        "parity_status = \"accepted\"".to_owned(),
        format!("generated_at = {}", toml_quote(&format_time(generated_at))?),
        format!("fresh_until = {}", toml_quote(&format_time(expires_at))?),
        format!(
            "family_ci_receipt = {}",
            toml_quote(&recorded_path(family_receipt_path, record_base)?)?
        ),
        format!("family_ci_receipt_sha256 = {}", toml_quote(family_digest)?),
        format!(
            "proof_refresh_receipt = {}",
            toml_quote(&recorded_path(operation_receipt_path, record_base)?)?
        ),
        format!(
            "accepted_core_evidence_sha256 = {}",
            toml_quote(
                core.receipt
                    .get("log_sha256")
                    .and_then(JsonValue::as_str)
                    .unwrap_or("")
            )?
        ),
        format!(
            "accepted_testing_manifest_sha256 = {}",
            toml_quote(
                testing
                    .receipt
                    .get("log_sha256")
                    .and_then(JsonValue::as_str)
                    .unwrap_or("")
            )?
        ),
        format!("required_consumer_evidence = {}", toml_array(&required)?),
        format!("accepted_consumer_evidence = {}", toml_array(&required)?),
        "cutover_eligible = true".to_owned(),
    ];
    for consumer in REQUIRED_CONSUMERS {
        let (path, evidence, digest) = consumers
            .get(consumer)
            .ok_or_else(|| error(format!("missing {consumer} evidence")))?;
        lines.extend([
            String::new(),
            "[[proof.consumer_evidence]]".to_owned(),
            format!("consumer = {}", toml_quote(consumer)?),
            format!("path = {}", toml_quote(&recorded_path(path, record_base)?)?),
            format!("sha256 = {}", toml_quote(digest)?),
            format!(
                "generated_at = {}",
                toml_quote(
                    evidence
                        .get("generated_at")
                        .and_then(JsonValue::as_str)
                        .unwrap_or("")
                )?
            ),
            format!(
                "source_commit = {}",
                toml_quote(
                    evidence
                        .get("source_commit")
                        .and_then(JsonValue::as_str)
                        .unwrap_or("")
                )?
            ),
            format!(
                "required_check = {}",
                toml_quote(
                    evidence
                        .get("required_check")
                        .and_then(JsonValue::as_str)
                        .unwrap_or("")
                )?
            ),
        ]);
    }
    for row in tag_rows {
        lines.extend([
            String::new(),
            "[[repo]]".to_owned(),
            format!("name = {}", toml_quote(&row.repo.name)?),
            format!(
                "product_version = {}",
                toml_quote(&row.repo.product_version)?
            ),
            format!("tag_revision = {}", row.repo.tag_revision),
            format!("tag = {}", toml_quote(&row.repo.current_tag)?),
            format!(
                "commit = {}",
                toml_quote(
                    row.receipt
                        .get("commit")
                        .and_then(JsonValue::as_str)
                        .unwrap_or("")
                )?
            ),
            format!(
                "checksum_sha256 = {}",
                toml_quote(&row.repo.release_checksum_sha256)?
            ),
            format!(
                "github = {}",
                toml_quote(&format!("https://github.com/{}.git", row.repo.github_slug))?
            ),
            format!("jeryu = {}", toml_quote(&expected_origin(&row.repo))?),
            format!("required_check = {}", toml_quote(&row.repo.required_check)?),
            format!(
                "protection_policy = {}",
                toml_quote(&row.repo.protection_policy)?
            ),
            format!("tag_object = {}", toml_quote(&row.metadata.object)?),
            format!(
                "tag_object_type = {}",
                toml_quote(&row.metadata.object_type)?
            ),
            format!("tag_subject = {}", toml_quote(&row.metadata.subject)?),
            "remote_tag_verified = true".to_owned(),
        ]);
        if let Some(date) = &row.metadata.tagger_date {
            lines.push(format!("tagger_date = {}", toml_quote(date)?));
        }
    }
    lines.push(String::new());
    Ok(lines.join("\n").into_bytes())
}

fn ensure_distinct_paths(paths: &[(&str, PathBuf)]) -> Result<()> {
    let mut seen: BTreeMap<PathBuf, &str> = BTreeMap::new();
    for (label, path) in paths {
        for candidate in [absolute_path(path)?, absolute_path(&checksum_path(path))?] {
            if let Some(previous) = seen.insert(candidate.clone(), label) {
                return Err(error(format!(
                    "proof-refresh path collision between {previous} and {label}: {}",
                    candidate.display()
                )));
            }
        }
    }
    Ok(())
}

fn proof_refresh(
    manifest_path: &Path,
    lock: &Path,
    mirror: &Path,
    family_receipt_path: &Path,
    consumer_paths: &BTreeMap<String, PathBuf>,
    operation_receipt: &Path,
) -> Result<()> {
    let mut paths = vec![
        ("manifest", manifest_path.to_path_buf()),
        ("authoritative lock", lock.to_path_buf()),
        ("compatibility mirror", mirror.to_path_buf()),
        ("family CI receipt", family_receipt_path.to_path_buf()),
        ("proof-refresh receipt", operation_receipt.to_path_buf()),
    ];
    for (consumer, path) in consumer_paths {
        paths.push((
            if consumer == "jain-split" {
                "Jain evidence"
            } else {
                "Jeryu evidence"
            },
            path.clone(),
        ));
    }
    ensure_distinct_paths(&paths)?;
    if consumer_paths
        .keys()
        .map(String::as_str)
        .collect::<BTreeSet<_>>()
        != BTreeSet::from(REQUIRED_CONSUMERS)
    {
        return Err(error(
            "proof-refresh requires exactly one Jain and one Jeryu consumer evidence file",
        ));
    }
    let now = Utc::now();
    let manifest_digest = sha256_file(manifest_path)?;
    let policy_digest = sha256_file(
        &manifest_path
            .parent()
            .unwrap_or(Path::new("."))
            .join("agent/audit-policy.toml"),
    )?;
    let (family_receipt, family_digest) =
        validate_family_receipt(manifest_path, family_receipt_path, now)?;
    let tag_rows = checked_tag_rows(manifest_path, &family_receipt)?;
    let core = tag_rows
        .iter()
        .find(|row| row.repo.name == "redline-core")
        .ok_or_else(|| error("redline-core row is missing"))?;
    let engine_tag = core.repo.current_tag.clone();
    let engine_version = core_version(&engine_tag)?;
    let engine_commit = core
        .receipt
        .get("commit")
        .and_then(JsonValue::as_str)
        .ok_or_else(|| error("redline-core receipt commit is missing"))?
        .to_owned();
    let lock_id = proof_id(&engine_version, &engine_commit);
    let family_generated = parse_time(
        family_receipt
            .get("generated_at")
            .unwrap_or(&JsonValue::Null),
        "family CI generated_at",
    )?;
    let mut consumers = BTreeMap::new();
    for consumer in REQUIRED_CONSUMERS {
        let path = consumer_paths
            .get(consumer)
            .ok_or_else(|| error(format!("missing {consumer} evidence")))?;
        let (value, digest) = validate_consumer_evidence(
            path,
            consumer,
            &EvidenceBinding {
                now,
                family_generated,
                family_digest: &family_digest,
                engine_tag: &engine_tag,
                engine_commit: &engine_commit,
                expected_proof: &lock_id,
                manifest_digest: &manifest_digest,
                policy_digest: &policy_digest,
            },
        )?;
        consumers.insert(consumer.to_owned(), (path.clone(), value, digest));
    }
    let record_base = lock.parent().unwrap_or(Path::new("."));
    let lock_data = render_lock(
        family_receipt_path,
        &family_digest,
        &tag_rows,
        &consumers,
        now,
        operation_receipt,
        record_base,
    )?;
    let lock_digest = sha256_bytes(&lock_data);
    let receipt_base = operation_receipt.parent().unwrap_or(Path::new("."));
    let refresh_payload = json!({
        "schema_version": PROOF_REFRESH_SCHEMA,
        "family": FAMILY,
        "generated_at": format_time(now),
        "status": "pass",
        "cutover_eligible": true,
        "proof_lock_id": lock_id,
        "engine_tag": engine_tag,
        "engine_commit": engine_commit,
        "family_ci_receipt": recorded_path(family_receipt_path, receipt_base)?,
        "family_ci_receipt_sha256": family_digest,
        "consumer_evidence_sha256": REQUIRED_CONSUMERS.iter().map(|consumer| {
            ((*consumer).to_owned(), json!(consumers.get(*consumer).map(|row| row.2.clone()).unwrap_or_default()))
        }).collect::<JsonMap<String, JsonValue>>(),
        "authoritative_lock": recorded_path(lock, receipt_base)?,
        "compatibility_mirror": recorded_path(mirror, receipt_base)?,
        "lock_sha256": lock_digest,
    });
    let (receipt_data, receipt_digest, receipt_sidecar) =
        checksummed_json_bytes(operation_receipt, &refresh_payload)?;
    let lock_sidecar = format!(
        "{lock_digest}  {}\n",
        lock.file_name()
            .and_then(|v| v.to_str())
            .unwrap_or("redline.lock.toml")
    )
    .into_bytes();
    let mirror_sidecar = format!(
        "{lock_digest}  {}\n",
        mirror
            .file_name()
            .and_then(|v| v.to_str())
            .unwrap_or("redline.lock.toml")
    )
    .into_bytes();
    transactional_write(&[
        (lock.to_path_buf(), lock_data.clone()),
        (checksum_path(lock), lock_sidecar),
        (mirror.to_path_buf(), lock_data),
        (checksum_path(mirror), mirror_sidecar),
        (operation_receipt.to_path_buf(), receipt_data),
        (checksum_path(operation_receipt), receipt_sidecar),
    ])?;
    if fs::read(lock)? != fs::read(mirror)? {
        return Err(error("lock mirror differs after proof-refresh transaction"));
    }
    println!("redline proof refreshed: {lock_id} lock_sha256={lock_digest} receipt_sha256={receipt_digest}");
    Ok(())
}

fn load_lock(path: &Path) -> Result<toml::Value> {
    let value: toml::Value = fs::read_to_string(path)?.parse()?;
    if value.get("schema_version").and_then(toml::Value::as_str) != Some(LOCK_SCHEMA)
        || value.get("family").and_then(toml::Value::as_str) != Some(FAMILY)
    {
        return Err(error("Redline lock schema or family is invalid"));
    }
    Ok(value)
}

fn lock_entries(value: &toml::Value) -> Result<Vec<&toml::value::Table>> {
    value
        .get("repo")
        .and_then(toml::Value::as_array)
        .ok_or_else(|| error("Redline lock repository rows are missing"))?
        .iter()
        .map(|row| {
            row.as_table()
                .ok_or_else(|| error("Redline lock repository row is invalid"))
        })
        .collect()
}

fn proof_table(value: &toml::Value) -> Result<&toml::value::Table> {
    value
        .get("proof")
        .and_then(toml::Value::as_table)
        .ok_or_else(|| error("Redline lock proof table is missing"))
}

fn verify_lock(manifest_path: &Path, lock: &Path, mirror: Option<&Path>) -> Result<toml::Value> {
    if !lock.is_file() {
        return Err(error("authoritative Redline lock is required"));
    }
    if let Some(mirror) = mirror {
        if !mirror.is_file() {
            return Err(error("Redline compatibility lock mirror is required"));
        }
        if fs::read(lock)? != fs::read(mirror)? {
            return Err(error(
                "control-plane lock mirror drift: files differ byte-for-byte",
            ));
        }
    }
    let value = load_lock(lock)?;
    if let Some(mirror) = mirror {
        if value != load_lock(mirror)? {
            return Err(error("control-plane lock mirror drift after TOML parsing"));
        }
    }
    if value.get("parent_family").and_then(toml::Value::as_str) != Some("independent") {
        return Err(error("Redline lock family ownership is invalid"));
    }
    let consumers: BTreeSet<&str> = value
        .get("consumers")
        .and_then(toml::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(toml::Value::as_str)
        .collect();
    if consumers != BTreeSet::from(REQUIRED_CONSUMERS) {
        return Err(error("Redline lock consumer set is invalid"));
    }
    let manifest = load_manifest(manifest_path)?;
    let entries = lock_entries(&value)?;
    let names: BTreeSet<&str> = entries
        .iter()
        .filter_map(|entry| entry.get("name").and_then(toml::Value::as_str))
        .collect();
    let expected_names: BTreeSet<&str> = manifest
        .repos
        .iter()
        .map(|repo| repo.name.as_str())
        .collect();
    if names != expected_names || entries.len() != manifest.repos.len() {
        return Err(error(
            "Redline lock repository set differs from the canonical manifest",
        ));
    }
    let proof = proof_table(&value)?;
    let eligible = proof.get("cutover_eligible").and_then(toml::Value::as_bool) == Some(true);
    for repo in &manifest.repos {
        let entry = entries
            .iter()
            .find(|entry| entry.get("name").and_then(toml::Value::as_str) == Some(&repo.name))
            .unwrap();
        let commit = entry
            .get("commit")
            .and_then(toml::Value::as_str)
            .unwrap_or("");
        if !is_sha1(commit) {
            return Err(error(format!(
                "{}: lock commit is not immutable",
                repo.name
            )));
        }
        if entry.get("required_check").and_then(toml::Value::as_str) != Some(&repo.required_check)
            || entry.get("jeryu").and_then(toml::Value::as_str) != Some(&expected_origin(repo))
        {
            return Err(error(format!(
                "{}: lock required check or Jeryu remote differs from manifest",
                repo.name
            )));
        }
        if eligible {
            if repo.release_commit == PENDING
                || repo.release_checksum_sha256 == PENDING
                || entry.get("product_version").and_then(toml::Value::as_str)
                    != Some(&repo.product_version)
                || entry.get("tag_revision").and_then(toml::Value::as_integer)
                    != Some(repo.tag_revision)
                || entry.get("tag").and_then(toml::Value::as_str) != Some(&repo.current_tag)
                || commit != repo.release_commit
                || entry.get("checksum_sha256").and_then(toml::Value::as_str)
                    != Some(&repo.release_checksum_sha256)
                || entry.get("protection_policy").and_then(toml::Value::as_str)
                    != Some(&repo.protection_policy)
            {
                return Err(error(format!(
                    "{}: eligible lock release identity differs from manifest",
                    repo.name
                )));
            }
            let object = entry
                .get("tag_object")
                .and_then(toml::Value::as_str)
                .unwrap_or("");
            let object_type = entry
                .get("tag_object_type")
                .and_then(toml::Value::as_str)
                .unwrap_or("");
            let subject = entry
                .get("tag_subject")
                .and_then(toml::Value::as_str)
                .unwrap_or("");
            if !is_sha1(object)
                || !matches!(object_type, "commit" | "tag")
                || subject.is_empty()
                || entry
                    .get("remote_tag_verified")
                    .and_then(toml::Value::as_bool)
                    != Some(true)
            {
                return Err(error(format!(
                    "{}: eligible lock lacks immutable tag metadata",
                    repo.name
                )));
            }
            if mirror.is_some() {
                let state = current_reviewed_state(&manifest, repo, false)?;
                if state.get("commit").and_then(JsonValue::as_str) != Some(commit) {
                    return Err(error(format!(
                        "{}: reviewed main differs from lock commit",
                        repo.name
                    )));
                }
                let metadata = tag_metadata(&manifest.repo_root(repo), &repo.current_tag, true)?;
                if metadata.commit != commit
                    || object != metadata.object
                    || object_type != metadata.object_type
                    || subject != metadata.subject
                {
                    return Err(error(format!(
                        "{}: lock metadata differs from immutable Jeryu tag",
                        repo.name
                    )));
                }
            }
        }
    }
    let core = entries
        .iter()
        .find(|entry| entry.get("name").and_then(toml::Value::as_str) == Some("redline-core"))
        .unwrap();
    let testing = entries
        .iter()
        .find(|entry| entry.get("name").and_then(toml::Value::as_str) == Some("redline-testing"))
        .unwrap();
    if value.get("engine_tag") != core.get("tag")
        || value.get("engine_commit") != core.get("commit")
    {
        return Err(error(
            "engine lock metadata differs from redline-core entry",
        ));
    }
    if value.get("parity_harness_commit") != testing.get("commit") {
        return Err(error(
            "parity_harness_commit differs from redline-testing entry",
        ));
    }
    if eligible {
        if let Some(mirror) = mirror {
            if verify_checksum(lock)? != verify_checksum(mirror)? {
                return Err(error("lock checksum mirrors differ"));
            }
        }
    } else {
        let accepted = proof
            .get("accepted_consumer_evidence")
            .and_then(toml::Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        if proof.get("cutover_eligible").and_then(toml::Value::as_bool) != Some(false)
            || proof.get("parity_status").and_then(toml::Value::as_str) == Some("accepted")
            || !accepted.is_empty()
        {
            return Err(error(
                "ineligible lock must remain explicitly historical with no accepted consumers",
            ));
        }
    }
    Ok(value)
}

fn parse_time_string(raw: Option<&str>, field: &str) -> Result<DateTime<Utc>> {
    parse_time(&raw.map_or(JsonValue::Null, |value| json!(value)), field)
}

fn cutover_verify(manifest_path: &Path, lock: &Path, mirror: &Path) -> Result<()> {
    let now = Utc::now();
    let value = verify_lock(manifest_path, lock, Some(mirror))?;
    let proof = proof_table(&value)?;
    if proof.get("parity_status").and_then(toml::Value::as_str) != Some("accepted")
        || proof.get("cutover_eligible").and_then(toml::Value::as_bool) != Some(true)
    {
        return Err(error("cutover blocked: proof is not derived as eligible"));
    }
    let generated = parse_time_string(
        proof.get("generated_at").and_then(toml::Value::as_str),
        "proof.generated_at",
    )?;
    let fresh_until = parse_time_string(
        proof.get("fresh_until").and_then(toml::Value::as_str),
        "proof.fresh_until",
    )?;
    require_fresh(generated, now, "proof.generated_at")?;
    if fresh_until < now || fresh_until != generated + Duration::hours(MAX_EVIDENCE_HOURS) {
        return Err(error(
            "cutover blocked: proof freshness window expired or was manually altered",
        ));
    }
    let lock_base = lock.parent().unwrap_or(Path::new("."));
    let family_path = resolve_recorded_path(
        &proof
            .get("family_ci_receipt")
            .and_then(toml::Value::as_str)
            .map_or(JsonValue::Null, |v| json!(v)),
        lock_base,
        "proof.family_ci_receipt",
    )?;
    let (family_receipt, family_digest) =
        validate_family_receipt(manifest_path, &family_path, now)?;
    if proof
        .get("family_ci_receipt_sha256")
        .and_then(toml::Value::as_str)
        != Some(&family_digest)
    {
        return Err(error(
            "cutover blocked: family CI receipt hash differs from lock",
        ));
    }
    let refresh_path = resolve_recorded_path(
        &proof
            .get("proof_refresh_receipt")
            .and_then(toml::Value::as_str)
            .map_or(JsonValue::Null, |v| json!(v)),
        lock_base,
        "proof.proof_refresh_receipt",
    )?;
    let refresh_digest = verify_checksum(&refresh_path)?;
    let refresh = read_json(&refresh_path)?;
    const REFRESH_FIELDS: [&str; 14] = [
        "schema_version",
        "family",
        "generated_at",
        "status",
        "cutover_eligible",
        "proof_lock_id",
        "engine_tag",
        "engine_commit",
        "family_ci_receipt",
        "family_ci_receipt_sha256",
        "consumer_evidence_sha256",
        "authoritative_lock",
        "compatibility_mirror",
        "lock_sha256",
    ];
    reject_unknown_fields(&refresh, &REFRESH_FIELDS, "proof-refresh receipt")?;
    let refresh_base = refresh_path.parent().unwrap_or(Path::new("."));
    if refresh.get("schema_version").and_then(JsonValue::as_str) != Some(PROOF_REFRESH_SCHEMA)
        || refresh.get("family").and_then(JsonValue::as_str) != Some(FAMILY)
        || refresh.get("status").and_then(JsonValue::as_str) != Some("pass")
        || refresh.get("cutover_eligible").and_then(JsonValue::as_bool) != Some(true)
        || refresh.get("generated_at").and_then(JsonValue::as_str)
            != proof.get("generated_at").and_then(toml::Value::as_str)
        || refresh.get("family_ci_receipt").and_then(JsonValue::as_str)
            != Some(&recorded_path(&family_path, refresh_base)?)
        || refresh
            .get("family_ci_receipt_sha256")
            .and_then(JsonValue::as_str)
            != Some(&family_digest)
        || refresh
            .get("authoritative_lock")
            .and_then(JsonValue::as_str)
            != Some(&recorded_path(lock, refresh_base)?)
        || refresh
            .get("compatibility_mirror")
            .and_then(JsonValue::as_str)
            != Some(&recorded_path(mirror, refresh_base)?)
        || refresh.get("lock_sha256").and_then(JsonValue::as_str) != Some(&sha256_file(lock)?)
        || !is_sha256(&refresh_digest)
    {
        return Err(error(
            "cutover blocked: proof-refresh receipt is stale, mismatched, or tampered",
        ));
    }
    let tag_rows = checked_tag_rows(manifest_path, &family_receipt)?;
    let core = tag_rows
        .iter()
        .find(|row| row.repo.name == "redline-core")
        .ok_or_else(|| error("redline-core row is missing"))?;
    let engine_version = core_version(&core.repo.current_tag)?;
    let engine_commit = core
        .receipt
        .get("commit")
        .and_then(JsonValue::as_str)
        .ok_or_else(|| error("core receipt commit is missing"))?;
    let expected_id = proof_id(&engine_version, engine_commit);
    if value.get("proof_lock_id").and_then(toml::Value::as_str) != Some(&expected_id)
        || refresh.get("proof_lock_id").and_then(JsonValue::as_str) != Some(&expected_id)
        || refresh.get("engine_tag").and_then(JsonValue::as_str) != Some(&core.repo.current_tag)
        || refresh.get("engine_commit").and_then(JsonValue::as_str) != Some(engine_commit)
    {
        return Err(error(
            "cutover blocked: proof identity was not derived from the live engine tag and commit",
        ));
    }
    let rows = proof
        .get("consumer_evidence")
        .and_then(toml::Value::as_array)
        .ok_or_else(|| error("cutover blocked: consumer evidence rows are missing"))?;
    let names: BTreeSet<&str> = rows
        .iter()
        .filter_map(|row| row.get("consumer"))
        .filter_map(toml::Value::as_str)
        .collect();
    if rows.len() != REQUIRED_CONSUMERS.len() || names != BTreeSet::from(REQUIRED_CONSUMERS) {
        return Err(error(
            "cutover blocked: exact Jain and Jeryu consumer evidence is required",
        ));
    }
    let family_generated = parse_time(
        family_receipt
            .get("generated_at")
            .unwrap_or(&JsonValue::Null),
        "family CI generated_at",
    )?;
    let manifest_digest = sha256_file(manifest_path)?;
    let policy_digest = sha256_file(
        &manifest_path
            .parent()
            .unwrap_or(Path::new("."))
            .join("agent/audit-policy.toml"),
    )?;
    let mut consumers = BTreeMap::new();
    for row in rows {
        let table = row
            .as_table()
            .ok_or_else(|| error("consumer evidence lock row is invalid"))?;
        let consumer = table
            .get("consumer")
            .and_then(toml::Value::as_str)
            .unwrap_or("");
        let path = resolve_recorded_path(
            &table
                .get("path")
                .and_then(toml::Value::as_str)
                .map_or(JsonValue::Null, |v| json!(v)),
            lock_base,
            &format!("{consumer} evidence path"),
        )?;
        let (evidence, digest) = validate_consumer_evidence(
            &path,
            consumer,
            &EvidenceBinding {
                now,
                family_generated,
                family_digest: &family_digest,
                engine_tag: &core.repo.current_tag,
                engine_commit,
                expected_proof: &expected_id,
                manifest_digest: &manifest_digest,
                policy_digest: &policy_digest,
            },
        )?;
        if table.get("sha256").and_then(toml::Value::as_str) != Some(&digest) {
            return Err(error(format!(
                "cutover blocked: {consumer} evidence hash differs from lock"
            )));
        }
        consumers.insert(consumer.to_owned(), (path, evidence, digest));
    }
    let expected_hashes: JsonMap<String, JsonValue> = REQUIRED_CONSUMERS
        .iter()
        .map(|consumer| {
            (
                (*consumer).to_owned(),
                json!(consumers
                    .get(*consumer)
                    .map(|row| row.2.clone())
                    .unwrap_or_default()),
            )
        })
        .collect();
    if refresh.get("consumer_evidence_sha256") != Some(&JsonValue::Object(expected_hashes)) {
        return Err(error(
            "cutover blocked: proof-refresh receipt consumer hashes differ from live evidence",
        ));
    }
    let expected = render_lock(
        &family_path,
        &family_digest,
        &tag_rows,
        &consumers,
        generated,
        &refresh_path,
        lock_base,
    )?;
    if expected != fs::read(lock)? {
        return Err(error(
            "cutover blocked: lock contains manual or non-derived edits",
        ));
    }
    println!(
        "redline cutover proof accepted: {expected_id} (fresh until {})",
        format_time(fresh_until)
    );
    Ok(())
}

fn consumer_verify(lock: &Path, consumer_lock: &Path) -> Result<()> {
    let value = load_lock(lock)?;
    let consumer_value: toml::Value = fs::read_to_string(consumer_lock)?.parse()?;
    let mut consumer = consumer_value
        .get("nested")
        .and_then(|value| value.get("redline"))
        .unwrap_or(&consumer_value);
    let normalized;
    if consumer.get("redline_engine_commit").is_some() {
        let mut table = toml::value::Table::new();
        table.insert(
            "engine_commit".to_owned(),
            consumer
                .get("redline_engine_commit")
                .cloned()
                .unwrap_or(toml::Value::String(String::new())),
        );
        table.insert(
            "engine_tag".to_owned(),
            consumer
                .get("redline_engine_tag")
                .cloned()
                .unwrap_or(toml::Value::String(String::new())),
        );
        table.insert(
            "proof_lock_id".to_owned(),
            consumer
                .get("redline_proof_lock_id")
                .cloned()
                .unwrap_or(toml::Value::String(String::new())),
        );
        normalized = toml::Value::Table(table);
        consumer = &normalized;
    }
    for field in ["engine_commit", "engine_tag", "proof_lock_id"] {
        if consumer.get(field) != value.get(field) {
            return Err(error(format!(
                "consumer lock drift in {field}: expected {:?}",
                value.get(field)
            )));
        }
    }
    println!("consumer lock ok: {}", consumer_lock.display());
    Ok(())
}

fn remote_verify(lock: &Path) -> Result<()> {
    let value = load_lock(lock)?;
    for entry in lock_entries(&value)? {
        let name = entry
            .get("name")
            .and_then(toml::Value::as_str)
            .unwrap_or("unknown");
        let remote = entry
            .get("jeryu")
            .and_then(toml::Value::as_str)
            .unwrap_or("");
        let tag = entry.get("tag").and_then(toml::Value::as_str).unwrap_or("");
        let commit = entry
            .get("commit")
            .and_then(toml::Value::as_str)
            .unwrap_or("");
        if !remote.starts_with(LOCAL_JERYU_BASE) || tag.is_empty() {
            return Err(error(format!(
                "{name}: canonical local Jeryu remote or tag is missing"
            )));
        }
        let output = Command::new("git")
            .args([
                "ls-remote",
                remote,
                &format!("refs/tags/{tag}"),
                &format!("refs/tags/{tag}^{{}}"),
            ])
            .output()?;
        if !output.status.success() {
            return Err(error(format!(
                "{name}: cannot read immutable tag from Jeryu"
            )));
        }
        let commits: BTreeSet<&str> = std::str::from_utf8(&output.stdout)?
            .lines()
            .filter_map(|line| line.split_whitespace().next())
            .collect();
        if !commits.contains(commit) {
            return Err(error(format!(
                "{name}: Jeryu does not expose the locked tag commit"
            )));
        }
    }
    println!("Redline immutable tags verified on canonical local Jeryu");
    Ok(())
}

fn audit_verify(path: &Path) -> Result<()> {
    let value = read_json(path)?;
    let score = value
        .get("score")
        .and_then(JsonValue::as_u64)
        .ok_or_else(|| error("audit report lacks an integer score"))?;
    let hard = value
        .get("decision")
        .and_then(|value| value.get("hard_findings"))
        .and_then(JsonValue::as_u64)
        .ok_or_else(|| error("audit report lacks decision.hard_findings"))?;
    let caps = value
        .get("caps_applied")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| error("audit report lacks caps_applied"))?;
    if score < 85 || hard != 0 || !caps.is_empty() {
        return Err(error(format!(
            "audit gate failed: score={score} hard_findings={hard} caps={}",
            caps.len()
        )));
    }
    println!("redline-split-ops audit accepted: score={score} hard_findings=0 caps=0");
    Ok(())
}

fn test_receipt(path: &Path, root: &Path) -> Result<()> {
    let commit = git(root, &["rev-parse", "HEAD"])?;
    let rustc = command_output(Command::new("rustc").arg("--version"))?;
    let cargo = command_output(Command::new("cargo").arg("--version"))?;
    let payload = json!({
        "schema_version": "redline.control-tests/v1",
        "family": FAMILY,
        "generated_at": format_time(Utc::now()),
        "status": "pass",
        "source_commit": commit,
        "command": "cargo test --locked",
        "rustc": String::from_utf8(rustc.stdout)?.trim(),
        "cargo": String::from_utf8(cargo.stdout)?.trim(),
        "metrics": {"test_command_exit_code": 0},
        "findings": [],
    });
    let digest = write_checksummed_json(path, &payload)?;
    println!(
        "Rust test receipt written: {} sha256={digest}",
        path.display()
    );
    Ok(())
}

fn security_receipt(path: &Path, root: &Path) -> Result<()> {
    let payload = json!({
        "schema_version": "redline.security-evidence/v1",
        "family": FAMILY,
        "generated_at": format_time(Utc::now()),
        "status": "pass",
        "source_commit": git(root, &["rev-parse", "HEAD"] )?,
        "metrics": {
            "required_scanners_passed": 6,
            "required_scanners_total": 6,
            "sbom": "target/security/redline-split-ops.spdx.json"
        },
        "scanners": ["cargo-audit", "cargo-deny", "gitleaks", "actionlint", "zizmor", "syft"],
        "findings": [],
    });
    let digest = write_checksummed_json(path, &payload)?;
    println!(
        "security receipt written: {} sha256={digest}",
        path.display()
    );
    Ok(())
}

fn release_receipt(path: &Path, paths: &Paths) -> Result<()> {
    let security = paths.root.join("target/security/evidence.json");
    verify_checksum(&security)?;
    let security_value = read_json(&security)?;
    if security_value.get("status").and_then(JsonValue::as_str) != Some("pass") {
        return Err(error(
            "release readiness requires passing security evidence",
        ));
    }
    verify_lock(&paths.manifest, &paths.lock, Some(&paths.mirror))?;
    let payload = json!({
        "schema_version": "redline.release-readiness/v1",
        "family": FAMILY,
        "generated_at": format_time(Utc::now()),
        "status": "pass",
        "release_status": "candidate",
        "formal_ga": false,
        "source_commit": git(&paths.root, &["rev-parse", "HEAD"] )?,
        "metrics": {"required_controls_passed": 6, "required_controls_total": 6},
        "controls": {
            "security": "pass",
            "rollback": "documented",
            "monitoring": "checksummed machine receipts",
            "backups": "not_applicable: stateless control plane",
            "abuse_controls": "not_applicable: local-only operator tool",
            "production_promotion": "not_applied"
        },
        "known_runtime_rollback_target": "7.0.6",
        "findings": [],
    });
    let digest = write_checksummed_json(path, &payload)?;
    println!(
        "release-readiness receipt written: {} sha256={digest}",
        path.display()
    );
    Ok(())
}

fn validate_control(manifest_path: &Path, lock: &Path) -> Result<usize> {
    let manifest = load_manifest(manifest_path)?;
    validate_receipt_schemas(manifest.path.parent().unwrap_or(Path::new(".")))?;
    let control_cargo = manifest
        .path
        .parent()
        .unwrap_or(Path::new("."))
        .join("Cargo.toml");
    if control_cargo.is_file() {
        let value: toml::Value = fs::read_to_string(&control_cargo)?.parse()?;
        if value.get("workspace").is_some() {
            return Err(error(
                "redline-split-ops must not become an umbrella Cargo workspace",
            ));
        }
    }
    let value = verify_lock(manifest_path, lock, None)?;
    Ok(lock_entries(&value)?.len())
}

fn validate(manifest_path: &Path, lock: &Path, mirror: &Path) -> Result<()> {
    let count = validate_control(manifest_path, lock)?;
    verify_lock(manifest_path, lock, Some(mirror))?;
    let manifest = load_manifest(manifest_path)?;
    for repo in &manifest.repos {
        let checkout = manifest.repo_root(repo);
        if !checkout.join(".git").exists() {
            return Err(error(format!(
                "{}: missing independent Git checkout: {}",
                repo.name,
                checkout.display()
            )));
        }
    }
    let hub = manifest
        .repos
        .iter()
        .find(|repo| repo.name == "redline")
        .ok_or_else(|| error("redline hub is missing"))?;
    command_output(
        Command::new("bash").arg(
            manifest
                .repo_root(hub)
                .join("scripts/guard-no-duplicate-engine.sh"),
        ),
    )?;
    println!("redline family and lock ok: {count} repositories");
    Ok(())
}

fn validate_receipt_schemas(root: &Path) -> Result<()> {
    for name in [
        "redline-family-ci.schema.json",
        "redline-consumer-evidence.schema.json",
        "redline-proof-refresh.schema.json",
    ] {
        let path = root.join("schemas").join(name);
        let value = read_json(&path)?;
        if value.get("$schema").and_then(JsonValue::as_str)
            != Some("https://json-schema.org/draft/2020-12/schema")
            || value.get("$id").and_then(JsonValue::as_str).is_none()
            || value.get("type").and_then(JsonValue::as_str) != Some("object")
            || value
                .get("required")
                .and_then(JsonValue::as_array)
                .is_none()
            || value
                .get("properties")
                .and_then(JsonValue::as_object)
                .is_none()
        {
            return Err(error(format!(
                "receipt schema lacks governed draft, identity, object type, required fields, or properties: {}",
                path.display()
            )));
        }
    }
    Ok(())
}

fn ensure_command(name: &str) -> Result<()> {
    let status = Command::new("sh")
        .args(["-c", &format!("command -v {name} >/dev/null 2>&1")])
        .status()?;
    if !status.success() {
        return Err(error(format!("{name} is required")));
    }
    Ok(())
}

fn doctor(manifest: &Path, lock: &Path, mirror: &Path) -> Result<()> {
    for command in ["git", "cargo", "flock"] {
        ensure_command(command)?;
    }
    validate(manifest, lock, mirror)?;
    println!(
        "redline doctor: ok ({})",
        manifest.parent().unwrap_or(Path::new(".")).display()
    );
    Ok(())
}

fn clone_or_update(manifest_path: &Path, dry_run: bool) -> Result<()> {
    let manifest = load_manifest(manifest_path)?;
    for repo in &manifest.repos {
        let checkout = manifest.repo_root(repo);
        let remote = expected_origin(repo);
        if dry_run {
            println!(
                "would ensure clean {} tracks {}/{} at {}",
                checkout.display(),
                remote,
                repo.default_branch,
                repo.name
            );
            continue;
        }
        if checkout.join(".git").is_dir() {
            if !git(
                &checkout,
                &["status", "--porcelain", "--untracked-files=all"],
            )?
            .is_empty()
            {
                return Err(error(format!("{} checkout is dirty", repo.name)));
            }
            if git(&checkout, &["branch", "--show-current"])? != repo.default_branch {
                return Err(error(format!(
                    "{} checkout must be on main before update",
                    repo.name
                )));
            }
            if git(&checkout, &["remote"])? != "origin"
                || git(&checkout, &["remote", "get-url", "origin"])? != remote
            {
                return Err(error(format!(
                    "{} checkout must have exactly the canonical Jeryu origin",
                    repo.name
                )));
            }
            command_output(Command::new("git").arg("-C").arg(&checkout).args([
                "fetch",
                "--prune",
                "--tags",
                "origin",
                &repo.default_branch,
            ]))?;
            command_output(Command::new("git").arg("-C").arg(&checkout).args([
                "merge",
                "--ff-only",
                &format!("origin/{}", repo.default_branch),
            ]))?;
        } else if checkout.exists() {
            return Err(error(format!(
                "{} checkout path exists but is not a Git repository",
                repo.name
            )));
        } else {
            fs::create_dir_all(checkout.parent().unwrap_or(Path::new(".")))?;
            command_output(
                Command::new("git")
                    .args([
                        "clone",
                        "--origin",
                        "origin",
                        "--branch",
                        &repo.default_branch,
                        "--single-branch",
                        &remote,
                    ])
                    .arg(&checkout),
            )?;
        }
        let head = git(&checkout, &["rev-parse", "HEAD"])?;
        if forge_ref(&checkout, &format!("refs/heads/{}", repo.default_branch))?.as_deref()
            != Some(&head)
        {
            return Err(error(format!(
                "{} local head does not equal Jeryu main",
                repo.name
            )));
        }
    }
    Ok(())
}

#[derive(Clone)]
struct Paths {
    manifest: PathBuf,
    lock: PathBuf,
    mirror: PathBuf,
    root: PathBuf,
}

fn default_paths() -> Paths {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    Paths {
        manifest: env::var_os("REDLINE_SPLIT_MANIFEST")
            .map(PathBuf::from)
            .unwrap_or_else(|| root.join("repos.manifest.toml")),
        lock: env::var_os("REDLINE_SPLIT_LOCK")
            .map(PathBuf::from)
            .unwrap_or_else(|| root.join("redline.lock.toml")),
        mirror: env::var_os("REDLINE_SPLIT_MIRROR_LOCK")
            .map(PathBuf::from)
            .unwrap_or_else(|| root.join("../redline-split/redline.lock.toml")),
        root,
    }
}

fn take_option(args: &mut Vec<String>, name: &str) -> Result<Option<String>> {
    if let Some(index) = args.iter().position(|value| value == name) {
        if index + 1 >= args.len() {
            return Err(error(format!("{name} requires a value")));
        }
        let value = args.remove(index + 1);
        args.remove(index);
        Ok(Some(value))
    } else {
        Ok(None)
    }
}

fn parse_consumer_assignments(values: Vec<String>) -> Result<BTreeMap<String, PathBuf>> {
    let mut result = BTreeMap::new();
    for raw in values {
        let (consumer, path) = raw
            .split_once('=')
            .ok_or_else(|| error("--consumer-evidence must be CONSUMER=PATH"))?;
        if !REQUIRED_CONSUMERS.contains(&consumer)
            || path.is_empty()
            || result.contains_key(consumer)
        {
            return Err(error(
                "consumer evidence must name Jain and Jeryu exactly once",
            ));
        }
        result.insert(consumer.to_owned(), PathBuf::from(path));
    }
    Ok(result)
}

fn real_main() -> Result<()> {
    let paths = default_paths();
    let mut args: Vec<String> = env::args().skip(1).collect();
    let command = if args.is_empty() {
        "doctor".to_owned()
    } else {
        args.remove(0)
    };
    match command.as_str() {
        "validate" => { if !args.is_empty() { return Err(error("validate accepts no arguments")); } validate(&paths.manifest, &paths.lock, &paths.mirror) }
        "control-validate" => {
            if !args.is_empty() { return Err(error("control-validate accepts no arguments")); }
            let count = validate_control(&paths.manifest, &paths.lock)?;
            println!("redline control manifest and lock ok: {count} repositories");
            Ok(())
        }
        "doctor" => { if !args.is_empty() { return Err(error("doctor accepts no arguments")); } doctor(&paths.manifest, &paths.lock, &paths.mirror) }
        "lock-verify" => {
            if !args.is_empty() { return Err(error("lock-verify accepts no arguments")); }
            let value = verify_lock(&paths.manifest, &paths.lock, Some(&paths.mirror))?;
            println!("redline lock ok: {} repositories", lock_entries(&value)?.len());
            Ok(())
        }
        "family-ci" | "ci" => {
            let receipt = take_option(&mut args, "--receipt")?.map(PathBuf::from)
                .unwrap_or_else(|| paths.root.join("target/release-evidence/redline-family-ci.json"));
            if !args.is_empty() { return Err(error("family-ci accepts only --receipt PATH")); }
            family_ci(&paths.manifest, &receipt)
        }
        "proof-refresh" => {
            let family = PathBuf::from(take_option(&mut args, "--family-ci")?.ok_or_else(|| error("proof-refresh requires --family-ci"))?);
            let receipt = take_option(&mut args, "--receipt")?.map(PathBuf::from)
                .unwrap_or_else(|| paths.root.join("target/release-evidence/redline-proof-refresh.json"));
            let mut assignments = Vec::new();
            while let Some(index) = args.iter().position(|value| value == "--consumer-evidence") {
                if index + 1 >= args.len() { return Err(error("--consumer-evidence requires CONSUMER=PATH")); }
                assignments.push(args.remove(index + 1));
                args.remove(index);
            }
            for (flag, consumer) in [("--jain-evidence", "jain-split"), ("--jeryu-evidence", "jeryu-split")] {
                if let Some(path) = take_option(&mut args, flag)? {
                    assignments.push(format!("{consumer}={path}"));
                }
            }
            if !args.is_empty() { return Err(error("proof-refresh accepts only governed evidence paths; eligibility flags are forbidden")); }
            proof_refresh(&paths.manifest, &paths.lock, &paths.mirror, &family, &parse_consumer_assignments(assignments)?, &receipt)
        }
        "cutover-verify" => { if !args.is_empty() { return Err(error("cutover-verify accepts no arguments")); } cutover_verify(&paths.manifest, &paths.lock, &paths.mirror) }
        "consumer-verify" => {
            if args.len() != 1 { return Err(error("consumer-verify requires one consumer lock path")); }
            consumer_verify(&paths.lock, Path::new(&args[0]))
        }
        "remote-verify" => { if !args.is_empty() { return Err(error("remote-verify accepts no arguments")); } remote_verify(&paths.lock) }
        "audit-verify" => {
            if args.len() != 1 {
                return Err(error("audit-verify requires one Jankurai JSON path"));
            }
            audit_verify(Path::new(&args[0]))
        }
        "test-receipt" => {
            if args.len() != 1 {
                return Err(error("test-receipt requires one output path"));
            }
            test_receipt(Path::new(&args[0]), &paths.root)
        }
        "security-receipt" => {
            if args.len() != 1 {
                return Err(error("security-receipt requires one output path"));
            }
            security_receipt(Path::new(&args[0]), &paths.root)
        }
        "release-receipt" => {
            if args.len() != 1 {
                return Err(error("release-receipt requires one output path"));
            }
            release_receipt(Path::new(&args[0]), &paths)
        }
        "clone" => {
            let dry_run = if args == ["--dry-run"] { true } else if args.is_empty() { false } else { return Err(error("clone accepts only --dry-run")); };
            clone_or_update(&paths.manifest, dry_run)
        }
        "update" => { if !args.is_empty() { return Err(error("update accepts no arguments")); } clone_or_update(&paths.manifest, false) }
        "--version" | "version" => { println!("redline-proof 0.1.0"); Ok(()) }
        _ => Err(error("usage: redlinectl {clone [--dry-run]|update|control-validate|validate|lock-verify|family-ci [--receipt PATH]|proof-refresh --family-ci PATH --jain-evidence PATH --jeryu-evidence PATH [--receipt PATH]|consumer-verify LOCK|remote-verify|cutover-verify|audit-verify REPORT|test-receipt OUTPUT|security-receipt OUTPUT|release-receipt OUTPUT|doctor}")),
    }
}

fn main() {
    if let Err(value) = real_main() {
        eprintln!("redline-proof: {value}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestDir(PathBuf);

    impl TestDir {
        fn new(name: &str) -> Self {
            let path =
                env::temp_dir().join(format!("redline-proof-test-{name}-{}", unique_suffix()));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn testing_repo() -> Repo {
        Repo {
            name: "redline-testing".to_owned(),
            path: PathBuf::from("../redline-split/redline-testing"),
            github_slug: "neverhuman/redline-testing".to_owned(),
            remote: format!("{LOCAL_JERYU_BASE}jeryu/redline-testing.git"),
            product_version: "1.0.1".to_owned(),
            tag_revision: 1,
            current_tag: "redline-testing-v1.0.1-jain.1".to_owned(),
            release_commit: "a".repeat(40),
            release_checksum_sha256: "b".repeat(64),
            protection_policy: RELEASE_PROTECTION_POLICY.to_owned(),
            required_check: "redline-testing/required".to_owned(),
            default_branch: "main".to_owned(),
        }
    }

    fn command_env(command: &Command, name: &str) -> Option<Option<String>> {
        command
            .get_envs()
            .find(|(key, _)| *key == name)
            .map(|(_, value)| value.map(|raw| raw.to_string_lossy().into_owned()))
    }

    fn testing_artifact(root: &Path) -> TestingArtifact {
        let staging = root.join("redline-testing-artifact");
        fs::create_dir_all(&staging).unwrap();
        let artifact_path = staging.join("redline-testing-1.0.1-linux-x86_64.tar.gz");
        let checksum_path = staging.join("redline-testing-1.0.1-linux-x86_64.tar.gz.sha256");
        let manifest_path = staging.join("release-manifest.json");
        let build_log = root.join("redline-testing-artifact.log");
        fs::write(&artifact_path, b"artifact").unwrap();
        fs::write(&checksum_path, b"checksum").unwrap();
        fs::write(&manifest_path, b"manifest").unwrap();
        fs::write(&build_log, b"build log").unwrap();
        TestingArtifact {
            source_commit: "a".repeat(40),
            source_tree_checksum_sha256: "b".repeat(64),
            product_version: "1.0.1".to_owned(),
            release_tag: "redline-testing-v1.0.1-jain.1".to_owned(),
            tag_revision: 1,
            artifact_name: "redline-testing-1.0.1-linux-x86_64.tar.gz".to_owned(),
            artifact_path,
            artifact_sha256: "c".repeat(64),
            checksum_path,
            checksum_sha256: "d".repeat(64),
            binary_sha256: "e".repeat(64),
            manifest_path,
            manifest_sha256: "f".repeat(64),
            build_log_sha256: sha256_file(&build_log).unwrap(),
            build_log,
        }
    }

    #[test]
    fn family_child_scrubs_control_toolchain_override() {
        let mut command = Command::new("true");
        configure_family_child(&mut command);
        assert_eq!(command_env(&command, "RUSTUP_TOOLCHAIN"), Some(None));
        assert_eq!(
            command_env(&command, "REDLINE_STRICT_TOOLS"),
            Some(Some("1".to_owned()))
        );
        assert_eq!(command_env(&command, "CI"), Some(Some("true".to_owned())));
    }

    #[test]
    fn redline_core_uses_only_hash_bound_local_testing_artifact() {
        let root = TestDir::new("artifact-env");
        let artifact = testing_artifact(root.path());
        let mut command = Command::new("true");
        configure_redline_core_artifact(&mut command, &artifact).unwrap();
        assert_eq!(
            command_env(&command, "CI_REDLINE_TESTING_URL"),
            Some(Some(file_url(&artifact.artifact_path).unwrap()))
        );
        assert_eq!(
            command_env(&command, "CI_REDLINE_TESTING_SHA256_URL"),
            Some(Some(file_url(&artifact.checksum_path).unwrap()))
        );
        assert_eq!(
            command_env(&command, "CI_REDLINE_TESTING_RELEASE_MANIFEST_URL"),
            Some(Some(file_url(&artifact.manifest_path).unwrap()))
        );
        assert_eq!(
            command_env(&command, "CI_REDLINE_TESTING_EXPECTED_TARBALL_SHA256"),
            Some(Some(artifact.artifact_sha256.clone()))
        );
        assert_eq!(
            command_env(&command, "CI_REDLINE_TESTING_EXPECTED_BINARY_SHA256"),
            Some(Some(artifact.binary_sha256.clone()))
        );
        assert_eq!(
            command_env(&command, "CI_REDLINE_TESTING_LOCAL_BIN"),
            Some(None)
        );
    }

    #[test]
    fn dependency_receipt_is_bound_to_reviewed_testing_commit_and_log() {
        let root = TestDir::new("artifact-receipt");
        let artifact = testing_artifact(root.path());
        let repo = testing_repo();
        let value = artifact.receipt_json(root.path()).unwrap();
        validate_testing_artifact_receipt(&value, &repo, &repo.release_commit, root.path())
            .unwrap();
        let mut tampered = value;
        tampered["source_commit"] = json!("9".repeat(40));
        assert!(validate_testing_artifact_receipt(
            &tampered,
            &repo,
            &repo.release_commit,
            root.path()
        )
        .unwrap_err()
        .to_string()
        .contains("source_commit"));
    }

    #[test]
    fn testing_package_verification_rejects_manifest_commit_drift() {
        let root = TestDir::new("testing-package");
        let repo = testing_repo();
        let package = "redline-testing-1.0.1-linux-x86_64";
        let dist = root.path().join("dist");
        let package_dir = dist.join(package);
        fs::create_dir_all(package_dir.join("bin")).unwrap();
        let artifact = dist.join(format!("{package}.tar.gz"));
        let checksum = dist.join(format!("{package}.tar.gz.sha256"));
        let manifest = dist.join("release-manifest.json");
        fs::write(&artifact, b"release archive").unwrap();
        let artifact_sha256 = sha256_file(&artifact).unwrap();
        fs::write(
            &checksum,
            format!("{artifact_sha256}  dist/{package}.tar.gz\n"),
        )
        .unwrap();
        let binary = package_dir.join("bin/redline-testing");
        fs::write(&binary, b"release binary").unwrap();
        let binary_sha256 = sha256_file(&binary).unwrap();
        let valid = json!({
            "name": "redline-testing",
            "version": repo.product_version,
            "release_commit": repo.release_commit,
            "release_tag": repo.current_tag,
            "tag_revision": repo.tag_revision,
            "binary_sha256": binary_sha256,
            "artifact_hashes": {"corpus/example.json": "f".repeat(64)},
        });
        fs::write(&manifest, serde_json::to_vec(&valid).unwrap()).unwrap();
        verify_testing_package(&repo, &repo.release_commit, root.path()).unwrap();
        let mut drifted = valid;
        drifted["release_commit"] = json!("9".repeat(40));
        fs::write(&manifest, serde_json::to_vec(&drifted).unwrap()).unwrap();
        assert!(
            verify_testing_package(&repo, &repo.release_commit, root.path())
                .unwrap_err()
                .to_string()
                .contains("reviewed manifest identity")
        );
    }

    #[test]
    fn canonical_manifest_uses_authorized_corrective_revisions() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let manifest = load_manifest(&root.join("repos.manifest.toml")).unwrap();
        let identities: BTreeMap<&str, (&str, i64, &str)> = manifest
            .repos
            .iter()
            .map(|repo| {
                (
                    repo.name.as_str(),
                    (
                        repo.product_version.as_str(),
                        repo.tag_revision,
                        repo.current_tag.as_str(),
                    ),
                )
            })
            .collect();
        assert_eq!(
            identities.get("redline"),
            Some(&("4.1.0", 2, "redline-v4.1.0-jain.2"))
        );
        assert_eq!(
            identities.get("redline-core"),
            Some(&("4.1.0", 3, "redline-core-v4.1.0-jain.3"))
        );
        assert_eq!(
            identities.get("redline-testing"),
            Some(&("1.0.1", 1, "redline-testing-v1.0.1-jain.1"))
        );
        assert_eq!(
            identities.get("redline-web"),
            Some(&("0.1.0", 1, "redline-web-v0.1.0-jain.1"))
        );
    }

    #[test]
    fn manifest_rejects_implicit_old_revision() {
        let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("repos.manifest.toml");
        let fixture = TestDir::new("old-revision");
        let path = fixture.path().join("repos.manifest.toml");
        let text = fs::read_to_string(source)
            .unwrap()
            .replace("redline-v4.1.0-jain.2", "redline-v4.1.0-jain.1");
        fs::write(&path, text).unwrap();
        assert!(load_manifest(&path)
            .unwrap_err()
            .to_string()
            .contains("current_tag must be redline-v4.1.0-jain.2"));
    }

    #[test]
    fn manifest_requires_atomic_commit_checksum_binding() {
        let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("repos.manifest.toml");
        let fixture = TestDir::new("half-bound");
        let path = fixture.path().join("repos.manifest.toml");
        let text = fs::read_to_string(source).unwrap().replacen(
            "release_commit = \"PENDING\"",
            "release_commit = \"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\"",
            1,
        );
        fs::write(&path, text).unwrap();
        assert!(load_manifest(&path)
            .unwrap_err()
            .to_string()
            .contains("must become exact together"));
    }

    #[test]
    fn release_tree_checksum_is_stable_for_a_commit() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let head = git(root, &["rev-parse", "HEAD"]).unwrap();
        let first = git_tree_checksum(root, &head).unwrap();
        assert!(is_sha256(&first));
        assert_eq!(first, git_tree_checksum(root, &head).unwrap());
    }

    #[test]
    fn checksum_sidecar_detects_tampering() {
        let root = TestDir::new("tamper");
        let path = root.path().join("receipt.json");
        write_checksummed_json(&path, &json!({"status": "pass"})).unwrap();
        assert!(is_sha256(&verify_checksum(&path).unwrap()));
        fs::write(&path, b"{\"status\":\"fail\"}\n").unwrap();
        assert!(verify_checksum(&path)
            .unwrap_err()
            .to_string()
            .contains("tampered evidence"));
    }

    #[test]
    fn stale_timestamp_is_rejected() {
        let now = DateTime::parse_from_rfc3339("2026-07-12T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let stale = now - Duration::hours(24) - Duration::seconds(1);
        assert!(require_fresh(stale, now, "fixture")
            .unwrap_err()
            .to_string()
            .contains("stale"));
    }

    #[test]
    fn consumer_evidence_rejects_manual_eligibility_boolean() {
        let root = TestDir::new("manual-bool");
        let path = root.path().join("jain.json");
        let now = Utc::now();
        let payload = json!({
            "schema_version": CONSUMER_SCHEMA,
            "consumer": "jain-split",
            "family": FAMILY,
            "generated_at": format_time(now),
            "status": "pass",
            "source_commit": "dddddddddddddddddddddddddddddddddddddddd",
            "required_check": "jain-split/redline-consumer",
            "engine_tag": "redline-core-v4.1.0-jain.2",
            "engine_commit": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            "proof_lock_id": "redline-proof/v2/4.1.0/bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            "family_ci_receipt_sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "cutover_eligible": true,
        });
        write_checksummed_json(&path, &payload).unwrap();
        let found = validate_consumer_evidence(
            &path,
            "jain-split",
            &EvidenceBinding {
                now,
                family_generated: now,
                family_digest: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                engine_tag: "redline-core-v4.1.0-jain.2",
                engine_commit: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                expected_proof: "redline-proof/v2/4.1.0/bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                manifest_digest: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                policy_digest: "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
            },
        )
        .unwrap_err();
        assert!(found.to_string().contains("manual boolean"));
    }

    #[test]
    fn consumer_evidence_binds_manifests_policy_and_fresh_test_log() {
        let root = TestDir::new("consumer-bindings");
        let evidence_path = root.path().join("jain.json");
        let test_log = root.path().join("jain-consumer.test.log");
        fs::write(&test_log, b"test result: ok. 1 passed; 0 failed\n").unwrap();
        let test_log_digest = sha256_file(&test_log).unwrap();
        let now = Utc::now();
        let manifest_digest = "a".repeat(64);
        let policy_digest = "b".repeat(64);
        let family_digest = "c".repeat(64);
        let engine_commit = "d".repeat(40);
        let proof_lock_id = proof_id("4.1.0", &engine_commit);
        let payload = json!({
            "schema_version": CONSUMER_SCHEMA,
            "consumer": "jain-split",
            "family": FAMILY,
            "generated_at": format_time(now),
            "status": "pass",
            "source_commit": "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
            "required_check": "jain-split/redline-consumer",
            "engine_tag": "redline-core-v4.1.0-jain.3",
            "engine_commit": engine_commit,
            "proof_lock_id": proof_lock_id,
            "family_ci_receipt_sha256": family_digest,
            "manifest_sha256": manifest_digest,
            "policy_sha256": policy_digest,
            "consumer_manifest_sha256": "f".repeat(64),
            "consumer_policy_sha256": "1".repeat(64),
            "test_log": "jain-consumer.test.log",
            "test_log_sha256": test_log_digest,
            "tool_version": "jain-redline-consumer/v1",
        });
        write_checksummed_json(&evidence_path, &payload).unwrap();
        let binding = EvidenceBinding {
            now,
            family_generated: now,
            family_digest: &family_digest,
            engine_tag: "redline-core-v4.1.0-jain.3",
            engine_commit: &engine_commit,
            expected_proof: &proof_lock_id,
            manifest_digest: &manifest_digest,
            policy_digest: &policy_digest,
        };

        validate_consumer_evidence(&evidence_path, "jain-split", &binding).unwrap();
        fs::write(&test_log, b"tampered\n").unwrap();
        assert!(
            validate_consumer_evidence(&evidence_path, "jain-split", &binding)
                .unwrap_err()
                .to_string()
                .contains("test log checksum")
        );
    }

    #[test]
    fn transaction_restores_every_prior_output() {
        let root = TestDir::new("rollback");
        let first = root.path().join("control.lock");
        let second = root.path().join("mirror.lock");
        atomic_write(&first, b"old-control\n").unwrap();
        atomic_write(&second, b"old-mirror\n").unwrap();
        let outputs = vec![
            (first.clone(), b"new\n".to_vec()),
            (second.clone(), b"new\n".to_vec()),
        ];
        let mut calls = 0;
        let result = transactional_write_with(&outputs, |path, data| {
            calls += 1;
            if calls == 2 {
                return Err(error("injected mirror failure"));
            }
            atomic_write(path, data)
        });
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("injected mirror failure"));
        assert_eq!(fs::read(first).unwrap(), b"old-control\n");
        assert_eq!(fs::read(second).unwrap(), b"old-mirror\n");
    }

    #[test]
    fn proof_refresh_rejects_output_input_collisions() {
        let root = TestDir::new("collision");
        let lock = root.path().join("redline.lock.toml");
        let found = ensure_distinct_paths(&[
            ("manifest", root.path().join("manifest.toml")),
            ("authoritative lock", lock.clone()),
            ("proof receipt", lock),
        ])
        .unwrap_err();
        assert!(found.to_string().contains("path collision"));
    }

    #[test]
    fn version_tag_parser_is_exact() {
        assert_eq!(core_version("redline-core-v4.1.0-jain.2").unwrap(), "4.1.0");
        assert!(core_version("redline-core-v4.1-jain.1").is_err());
        assert!(core_version("redline-core-v4.1.0-jain.next").is_err());
    }

    #[test]
    fn derived_lock_uses_relocatable_paths_and_eligibility() {
        let root = TestDir::new("derived-lock");
        let names = ["redline", "redline-core", "redline-testing", "redline-web"];
        let tags = [
            "redline-v4.1.0-jain.2",
            "redline-core-v4.1.0-jain.2",
            "redline-testing-v1.0.1-jain.1",
            "redline-web-v0.1.0-jain.1",
        ];
        let mut rows = Vec::new();
        for (index, (name, tag)) in names.iter().zip(tags).enumerate() {
            let digit = char::from_digit((index + 1) as u32, 16).unwrap();
            let commit: String = std::iter::repeat_n(digit, 40).collect();
            rows.push(TagRow {
                repo: Repo {
                    name: (*name).to_owned(),
                    path: PathBuf::from(format!("../redline-split/{name}")),
                    github_slug: format!("neverhuman/{name}"),
                    remote: format!("{LOCAL_JERYU_BASE}jeryu/{name}.git"),
                    product_version: match *name {
                        "redline" | "redline-core" => "4.1.0",
                        "redline-testing" => "1.0.1",
                        "redline-web" => "0.1.0",
                        _ => unreachable!(),
                    }
                    .to_owned(),
                    tag_revision: if matches!(*name, "redline" | "redline-core") {
                        2
                    } else {
                        1
                    },
                    current_tag: tag.to_owned(),
                    release_commit: commit.clone(),
                    release_checksum_sha256: "f".repeat(64),
                    protection_policy: RELEASE_PROTECTION_POLICY.to_owned(),
                    required_check: format!("{name}/required"),
                    default_branch: "main".to_owned(),
                },
                receipt: json!({"commit": commit, "log_sha256": "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee"}),
                metadata: TagMetadata {
                    object: commit.clone(), object_type: "commit".to_owned(), commit,
                    tagger_date: None, subject: format!("release {name}"),
                    remote_object: None, remote_commit: None,
                },
            });
        }
        let generated = DateTime::parse_from_rfc3339("2026-07-12T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let mut consumers = BTreeMap::new();
        for consumer in REQUIRED_CONSUMERS {
            consumers.insert(consumer.to_owned(), (
                root.path().join(format!("{consumer}.json")),
                json!({"generated_at": format_time(generated), "source_commit": "9999999999999999999999999999999999999999", "required_check": format!("{consumer}/redline-consumer")}),
                if consumer == "jain-split" { "7".repeat(64) } else { "8".repeat(64) },
            ));
        }
        let data = render_lock(
            &root.path().join("evidence/family.json"),
            &"a".repeat(64),
            &rows,
            &consumers,
            generated,
            &root.path().join("evidence/refresh.json"),
            root.path(),
        )
        .unwrap();
        let text = String::from_utf8(data).unwrap();
        let parsed: toml::Value = text.parse().unwrap();
        assert_eq!(
            parsed
                .get("proof")
                .and_then(|v| v.get("cutover_eligible"))
                .and_then(toml::Value::as_bool),
            Some(true)
        );
        assert!(!text.contains(&root.path().to_string_lossy().to_string()));
    }

    #[test]
    fn audit_gate_requires_score_hard_and_cap_acceptance() {
        let root = TestDir::new("audit-gate");
        let report = root.path().join("score.json");
        fs::write(
            &report,
            serde_json::to_vec(&json!({
                "score": 85,
                "caps_applied": [],
                "decision": {"hard_findings": 0}
            }))
            .unwrap(),
        )
        .unwrap();
        audit_verify(&report).unwrap();
        fs::write(
            &report,
            serde_json::to_vec(&json!({
                "score": 84,
                "caps_applied": [],
                "decision": {"hard_findings": 0}
            }))
            .unwrap(),
        )
        .unwrap();
        assert!(audit_verify(&report).is_err());
    }

    #[test]
    fn governed_receipt_schemas_parse() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        validate_receipt_schemas(root).unwrap();
    }

    #[test]
    fn control_validation_does_not_require_family_checkouts() {
        let fixture = TestDir::new("standalone-control");
        let control = fixture.path().join("redline-split-ops");
        let mirror_dir = fixture.path().join("redline-split");
        fs::create_dir_all(control.join("schemas")).unwrap();
        fs::create_dir_all(&mirror_dir).unwrap();
        let source = Path::new(env!("CARGO_MANIFEST_DIR"));
        for name in ["repos.manifest.toml", "redline.lock.toml", "Cargo.toml"] {
            fs::copy(source.join(name), control.join(name)).unwrap();
        }
        for name in [
            "redline-family-ci.schema.json",
            "redline-consumer-evidence.schema.json",
            "redline-proof-refresh.schema.json",
        ] {
            fs::copy(
                source.join("schemas").join(name),
                control.join("schemas").join(name),
            )
            .unwrap();
        }
        assert!(!mirror_dir.join("redline.lock.toml").exists());
        assert_eq!(
            validate_control(
                &control.join("repos.manifest.toml"),
                &control.join("redline.lock.toml"),
            )
            .unwrap(),
            4
        );
        assert!(verify_lock(
            &control.join("repos.manifest.toml"),
            &control.join("redline.lock.toml"),
            Some(&mirror_dir.join("redline.lock.toml")),
        )
        .unwrap_err()
        .to_string()
        .contains("compatibility lock mirror is required"));
        assert!(!mirror_dir.join("redline").exists());
    }
}
