// Repository-local release and Jeryu control-plane CLI.
mod jeryu_client;
mod release_candidate;

use jeryu_client::{write_token_for_askpass, HostCiPublication, JeryuClient, JeryuRequest};
use serde::de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor};
use serde_json::{json, Map, Value as JsonValue};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    env,
    ffi::OsStr,
    fs,
    io::{self, Read, Write},
    os::{
        fd::AsRawFd,
        unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
    },
    path::{Component, Path, PathBuf},
    process::{Command, Stdio},
    time::{SystemTime, UNIX_EPOCH},
};

const RELEASE_VERSION: &str = "8.0.1";
const RELEASE_STATUS: &str = "candidate";
const ROLLBACK_TARGET: &str = "7.0.6";
const LOCAL_JERYU_ORIGIN: &str = "http://127.0.0.1:8787";
const APPLIANCE_DEPLOY_REMOTE: &str = "http://127.0.0.1:8787/git/veox/jain-deploy.git";
const APPLIANCE_VERIFIER_NAME: &str = "jain-deploy-local-appliance-runner/v1";
const APPLIANCE_VERIFIER_SHA256: &str =
    "6f218a58092cab5d0d4e457b2f35163191bb490cbd4d3d322147eb2fdd8711f7";
const ROOT_UID: u32 = 0;
const ROOT_GID: u32 = 0;
const FAMILY_REMOTE_PREFIX: &str = "http://127.0.0.1:8787/git/veox/";
const INFRA_REMOTE_PREFIX: &str = "http://127.0.0.1:8787/git/veox/";
const NESTED_REDLINE_REMOTE_PREFIX: &str = "http://127.0.0.1:8787/git/jeryu/";
const LEGACY_FAMILY_PIN_PREFIX: &str = "http://127.0.0.1:8787/git/jeryu/";
const LEGACY_INFRA_PIN_PREFIX: &str = "http://127.0.0.1:8787/git/jain-split/";
const JERYU_ASKPASS_MODE: &str = "JAIN_SPLITCTL_JERYU_ASKPASS";
const JERYU_ASKPASS_TOKEN_FILE: &str = "JAIN_SPLITCTL_JERYU_TOKEN_FILE";
const JERYU_GIT_USERNAME: &str = "x-access-token";
// The authenticated family closure currently contains 516 locks / 4,812,548
// bytes (487 locks from protected jain-model-zoo; largest lock 113,831 bytes).
// Keep finite headroom without allowing count to multiply parser memory use.
const MAX_CARGO_LOCKS: usize = 1024;
const MAX_CARGO_LOCK_BYTES: u64 = 1024 * 1024;
const MAX_CARGO_LOCK_BYTES_TOTAL: u64 = 64 * 1024 * 1024;

/// Resolve the control-plane checkout at runtime so release binaries do not
/// embed the physical path of the checkout that compiled them. Commands are
/// normally run from that checkout; installed binaries can also discover it
/// from their `target/*` location before falling back to the working tree.
fn control_plane_root() -> PathBuf {
    fn containing_root(start: &Path) -> Option<PathBuf> {
        start.ancestors().find_map(|candidate| {
            (candidate.join("Cargo.toml").is_file()
                && candidate.join("repos.manifest.toml").is_file())
            .then(|| candidate.to_path_buf())
        })
    }

    if let Some(explicit) = env::var_os("JAIN_SPLIT_OPS_ROOT") {
        let explicit = PathBuf::from(explicit);
        if explicit.is_absolute() {
            if let Some(root) = containing_root(&explicit) {
                return root;
            }
        }
        panic!("JAIN_SPLIT_OPS_ROOT must name a jain-split-ops checkout");
    }
    if let Ok(current) = env::current_dir() {
        if let Some(root) = containing_root(&current) {
            return root;
        }
    }
    if let Ok(executable) = env::current_exe() {
        if let Some(root) = containing_root(&executable) {
            return root;
        }
    }
    panic!("splitctl must run from, or beneath, a jain-split-ops checkout");
}

#[derive(Debug, Clone)]
struct Repo {
    name: String,
    path: PathBuf,
    profile: String,
    authored: bool,
    cargo_members: Vec<String>,
    copy_paths: Vec<String>,
    source_paths: Vec<String>,
}

#[derive(Debug, Clone)]
struct ManagedRepo {
    name: String,
    path: PathBuf,
    remote: String,
    required_check: String,
    branch: String,
    tag: Option<String>,
    kind: String,
    family: String,
    family_registered: bool,
    inventory_status: String,
    runtime_authority: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NestedEngineTopology {
    dependency_name: String,
    family: String,
    split_root: PathBuf,
    manifest_path: PathBuf,
    container_path: PathBuf,
    control_plane_path: PathBuf,
    engine_repository: String,
    engine_remote: String,
    engine_required_check: String,
    pending_repositories: Vec<PendingNestedRepository>,
    bound_identity: Option<BoundEngineIdentity>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PendingNestedRepository {
    name: String,
    remote: String,
    required_check: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BoundEngineIdentity {
    tag: String,
    product_version: String,
    tag_revision: i64,
    release_commit: String,
    release_tree: String,
    release_checksum_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ReleaseFeatureMatrix {
    package: String,
    feature_sets: Vec<Vec<String>>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if env::var_os(JERYU_ASKPASS_MODE).is_some() {
        return jeryu_git_askpass(env::args_os().skip(1).collect());
    }
    let mut args = env::args().skip(1);
    match args.next().as_deref() {
        Some("refresh-ci-contract") => {
            let mut selected = Vec::new();
            let mut authored_only = false;
            while let Some(arg) = args.next() {
                match arg.as_str() {
                    "--repo" => selected.push(args.next().ok_or("--repo needs a name")?),
                    "--authored" => authored_only = true,
                    value => return Err(format!("unknown argument: {value}").into()),
                }
            }
            refresh(&selected, authored_only)?;
        }
        Some("materialize") => {
            let mut selected = Vec::new();
            while let Some(arg) = args.next() {
                match arg.as_str() {
                    "--repo" => selected.push(args.next().ok_or("--repo needs a name")?),
                    value => return Err(format!("unknown argument: {value}").into()),
                }
            }
            refresh(&selected, false)?;
            println!("Rust split materialization contract refreshed");
        }
        Some("refresh-bare-mirrors") => refresh_bare_mirrors(args.collect())?,
        Some("host-ci-snapshot-request") => host_ci_snapshot_request_command(args.collect())?,
        Some("cargo-cache-stage") => cargo_cache_stage_command(args.collect())?,
        Some("validate-local-jeryu") => {
            let mut manifest = None;
            let mut skip_remotes = false;
            let mut sealed_outer_projection = false;
            while let Some(arg) = args.next() {
                match arg.as_str() {
                    "--manifest" => manifest = Some(PathBuf::from(args.next().ok_or("--manifest needs a path")?)),
                    "--skip-remotes" => skip_remotes = true,
                    "--sealed-outer-projection" => sealed_outer_projection = true,
                    value => return Err(format!("unknown argument: {value}").into()),
                }
            }
            validate_local_jeryu(manifest, skip_remotes, sealed_outer_projection)?;
        }
        Some("preflight") => preflight(args.collect())?,
        Some("source-coverage") => {
            let mut manifest = PathBuf::from("repos.manifest.toml");
            let mut json_output = false;
            while let Some(arg) = args.next() {
                match arg.as_str() {
                    "--manifest" => manifest = PathBuf::from(args.next().ok_or("--manifest needs a path")?),
                    "--json" => json_output = true,
                    value => return Err(format!("unknown argument: {value}").into()),
                }
            }
            source_coverage(&manifest, json_output)?;
        }
        Some("seal-source-inventory") => seal_source_inventory_command(args.collect())?,
        Some("python-boundary") => python_boundary()?,
        Some("jeryu-doctor") => {
            let mut manifest = None;
            let mut skip_remotes = false;
            let mut fix_remotes = false;
            let mut register_family = false;
            let mut install_hooks = false;
            while let Some(arg) = args.next() {
                match arg.as_str() {
                    "--manifest" => manifest = Some(PathBuf::from(args.next().ok_or("--manifest needs a path")?)),
                    "--skip-remotes" => skip_remotes = true,
                    "--fix-remotes" => fix_remotes = true,
                    "--register-family" => register_family = true,
                    "--install-hooks" => install_hooks = true,
                    value => return Err(format!("unknown argument: {value}").into()),
                }
            }
            if register_family {
                return Err("family registration is unsupported until it is migrated to the typed Rust Jeryu transport".into());
            }
            if fix_remotes {
                fix_local_remotes(manifest.clone())?;
            }
            if install_hooks {
                install_worktree_ban_hooks(manifest.clone())?;
            }
            validate_local_jeryu(manifest, skip_remotes, false)?;
        }
        Some("jeryu-local") => jeryu_local(args.collect())?,
        Some("jeryu-publish-host-ci") => {
            if let Err(error) = jeryu_publish_host_ci(args.collect()) {
                eprintln!("splitctl: {}", error.message);
                std::process::exit(if error.publication_started { 42 } else { 41 });
            }
        }
        Some("manifest") => manifest_command(args.collect())?,
        Some("managed-repos") => managed_repos_command(args.collect())?,
        Some("host-ci-authority") => host_ci_authority_command(args.collect())?,
        Some("release-cargo-commands") => release_cargo_commands_command(args.collect())?,
        Some("sync-derived-manifests") => sync_derived_manifests_command(args.collect())?,
        Some("jankurai-evidence") => jankurai_evidence_command(args.collect())?,
        Some("validate-manifest") => validate_manifest_command(args.collect())?,
        Some("validate-family") => preflight(args.collect())?,
        Some("validate-family-lock") => validate_family_lock(args.collect())?,
        Some("regenerate-lock") => regenerate_lock(args.collect())?,
        Some("release-preflight") => release_preflight(args.collect())?,
        Some("release-snapshot") => release_snapshot(args.collect())?,
        Some("release-candidate") => release_candidate::command(args.collect())?,
        Some("release-status") => release_status(args.collect())?,
        Some("validate-appliance-promotion") => validate_appliance_promotion(args.collect())?,
        Some("bootstrap-main") => bootstrap_main_command(args.collect())?,
        Some("immutable-tag") => immutable_tag_command(args.collect())?,
        Some("verify-worktrees") => verify_worktrees_command(args.collect())?,
        Some("reconcile") => reconcile(args.collect())?,
        Some("bump-version") => bump_version(args.collect())?,
        Some("--version") | Some("version") => println!("splitctl 0.1.0"),
        _ => return Err("usage: splitctl refresh-ci-contract [--repo NAME]... | materialize [--repo NAME]... | host-ci-snapshot-request --source PATH --destination PATH --expected-uid UID --expected-gid GID --max-bytes BYTES | cargo-cache-stage --lock PATH [--lock PATH]... --source PATH --destination PATH --receipt PATH --expected-source-uid UID --expected-source-gid GID | manifest [--manifest PATH] [--json] | managed-repos [--manifest PATH] --json | host-ci-authority [--manifest PATH] --repo NAME | release-cargo-commands [--manifest PATH] --repo NAME | sync-derived-manifests [--manifest PATH] [--target NAME]... [--receipt PATH] [--apply] | jankurai-evidence --repository NAME --commit SHA --worktree PATH --report-root PATH --report PATH --auditor PATH --attempt-id ID --lane-conclusion success|failure [--lane-failure-reason REASON] --clean-tracked-tree-start BOOL --receipt PATH | validate-manifest [--manifest PATH] [--check-paths] [--check-derived] | validate-local-jeryu [--manifest PATH] [--skip-remotes] [--sealed-outer-projection] | validate-family [--manifest PATH] [--json PATH] | validate-family-lock [--manifest PATH] [--lock PATH] | regenerate-lock [--manifest PATH] [--output PATH] --apply | release-preflight [--manifest PATH] [--json PATH] | release-snapshot [--manifest PATH] [--json PATH] | release-candidate [--manifest PATH] [--repo NAME]... [--journal PATH --token-file PATH --apply] [--receipt PATH] | release-status [--manifest PATH] [--appliance-canary-aggregate PATH --appliance-canary-verifier-receipt PATH --token-file PATH] [--json PATH] | validate-appliance-promotion --aggregate PATH --verifier-receipt PATH --token-file PATH [--json PATH] | bootstrap-main --repo PATH --remote URL --reviewed-commit SHA [--receipt PATH] [--apply] | immutable-tag --repo PATH --remote URL --tag TAG --commit SHA --token-file PATH [--receipt PATH] [--apply] | verify-worktrees [--manifest PATH] [--receipt PATH] | preflight [--manifest PATH] [--json PATH] | source-coverage [--manifest PATH] [--json] | seal-source-inventory [--manifest PATH] --source-root PATH [--apply] | python-boundary | jeryu-doctor [--manifest PATH] | reconcile [--manifest PATH] [--base-ref REF] [--apply] [--json PATH] | bump-version [--manifest PATH] --from VERSION --new VERSION --rewrite-split-tags".into()),
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LockedRegistryPackage {
    name: String,
    version: String,
    checksum: String,
}

#[derive(Debug)]
struct LockedCargoInputs {
    registry_packages: Vec<LockedRegistryPackage>,
    governed_git_repositories: Vec<String>,
}

#[derive(Debug)]
struct BoundedCargoLock {
    contents: String,
    digest: String,
    bytes: u64,
}

fn cargo_cache_stage_command(args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    let mut locks = Vec::new();
    let mut source = None;
    let mut destination = None;
    let mut receipt = None;
    let mut expected_source_uid = None;
    let mut expected_source_gid = None;
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--lock" => locks.push(PathBuf::from(iter.next().ok_or("--lock needs a path")?)),
            "--source" => source = Some(PathBuf::from(iter.next().ok_or("--source needs a path")?)),
            "--destination" => {
                destination = Some(PathBuf::from(
                    iter.next().ok_or("--destination needs a path")?,
                ))
            }
            "--receipt" => {
                receipt = Some(PathBuf::from(iter.next().ok_or("--receipt needs a path")?))
            }
            "--expected-source-uid" => {
                expected_source_uid = Some(
                    iter.next()
                        .ok_or("--expected-source-uid needs a value")?
                        .parse::<u32>()?,
                )
            }
            "--expected-source-gid" => {
                expected_source_gid = Some(
                    iter.next()
                        .ok_or("--expected-source-gid needs a value")?
                        .parse::<u32>()?,
                )
            }
            value => return Err(format!("unknown cargo-cache-stage argument: {value}").into()),
        }
    }
    stage_locked_cargo_caches(
        &locks,
        &source.ok_or("--source is required")?,
        &destination.ok_or("--destination is required")?,
        &receipt.ok_or("--receipt is required")?,
        expected_source_uid.ok_or("--expected-source-uid is required")?,
        expected_source_gid.ok_or("--expected-source-gid is required")?,
    )
}

fn same_file_metadata(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    left.file_type().is_file()
        && right.file_type().is_file()
        && left.dev() == right.dev()
        && left.ino() == right.ino()
        && left.mode() == right.mode()
        && left.nlink() == right.nlink()
        && left.uid() == right.uid()
        && left.gid() == right.gid()
        && left.len() == right.len()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
        && left.ctime() == right.ctime()
        && left.ctime_nsec() == right.ctime_nsec()
}

fn read_bounded_cargo_lock(
    lock_path: &Path,
    max_bytes: u64,
) -> Result<BoundedCargoLock, Box<dyn std::error::Error>> {
    const O_NONBLOCK: i32 = 0o4000;
    const O_NOFOLLOW: i32 = 0o400000;
    let mut input = fs::OpenOptions::new()
        .read(true)
        .custom_flags(O_NONBLOCK | O_NOFOLLOW)
        .open(lock_path)?;
    let before = input.metadata()?;
    if !before.file_type().is_file() || before.nlink() != 1 || before.len() > max_bytes {
        return Err(format!(
            "unsafe Cargo lock inode or per-file limit ({max_bytes} bytes) exceeded: {}",
            lock_path.display()
        )
        .into());
    }
    let mut bytes = Vec::with_capacity(before.len() as usize);
    (&mut input).take(max_bytes + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > max_bytes {
        return Err(format!(
            "Cargo lock exceeded its {max_bytes}-byte per-file limit while reading: {}",
            lock_path.display()
        )
        .into());
    }
    let after = input.metadata()?;
    let path_after = fs::symlink_metadata(lock_path)?;
    if bytes.len() as u64 != before.len()
        || !same_file_metadata(&before, &after)
        || !same_file_metadata(&before, &path_after)
    {
        return Err(format!("Cargo lock changed while reading: {}", lock_path.display()).into());
    }
    let digest = format!("{:x}", Sha256::digest(&bytes));
    let contents = String::from_utf8(bytes)
        .map_err(|_| format!("Cargo lock is not UTF-8: {}", lock_path.display()))?;
    Ok(BoundedCargoLock {
        contents,
        digest,
        bytes: before.len(),
    })
}

fn read_bounded_cargo_locks_with_limits(
    lock_paths: &[PathBuf],
    max_lock_bytes: u64,
    max_total_bytes: u64,
) -> Result<Vec<BoundedCargoLock>, Box<dyn std::error::Error>> {
    let mut total = 0_u64;
    let mut locks = Vec::with_capacity(lock_paths.len());
    for lock_path in lock_paths {
        let lock = read_bounded_cargo_lock(lock_path, max_lock_bytes)?;
        total = total
            .checked_add(lock.bytes)
            .ok_or("Cargo lock aggregate byte count overflowed")?;
        if total > max_total_bytes {
            return Err(
                format!("Cargo locks exceed the {max_total_bytes}-byte aggregate limit").into(),
            );
        }
        locks.push(lock);
    }
    Ok(locks)
}

fn read_bounded_cargo_locks(
    lock_paths: &[PathBuf],
) -> Result<Vec<BoundedCargoLock>, Box<dyn std::error::Error>> {
    read_bounded_cargo_locks_with_limits(
        lock_paths,
        MAX_CARGO_LOCK_BYTES,
        MAX_CARGO_LOCK_BYTES_TOTAL,
    )
}

fn locked_cargo_inputs_from_contents(
    contents: &str,
) -> Result<LockedCargoInputs, Box<dyn std::error::Error>> {
    let lock: toml::Value = contents.parse()?;
    let root = lock.as_table().ok_or("Cargo lock root is not a table")?;
    if root
        .keys()
        .any(|key| !matches!(key.as_str(), "version" | "package"))
    {
        return Err("Cargo lock contains an unknown top-level field".into());
    }
    let version = root
        .get("version")
        .and_then(toml::Value::as_integer)
        .ok_or("Cargo lock has no integer version")?;
    if !matches!(version, 3 | 4) {
        return Err(format!("unsupported Cargo lock version: {version}").into());
    }
    let packages = match root.get("package") {
        Some(value) => value
            .as_array()
            .ok_or("Cargo lock package field is not an array")?,
        None => {
            return Ok(LockedCargoInputs {
                registry_packages: Vec::new(),
                governed_git_repositories: Vec::new(),
            });
        }
    };
    let mut locked = Vec::new();
    let mut governed_git_repositories = Vec::new();
    for package in packages {
        let Some(table) = package.as_table() else {
            return Err("Cargo lock package is not a table".into());
        };
        let source = match table.get("source") {
            None => continue,
            Some(value) => value
                .as_str()
                .ok_or("Cargo lock package source is not a string")?,
        };
        if let Some(repository) = governed_locked_git_repository(source) {
            if table.contains_key("checksum") {
                return Err(format!(
                    "governed locked Git package unexpectedly has a checksum: {source}"
                )
                .into());
            }
            governed_git_repositories.push(repository.to_owned());
            continue;
        }
        if source != "registry+https://github.com/rust-lang/crates.io-index" {
            return Err(format!("unsupported locked registry source: {source}").into());
        }
        let name = table
            .get("name")
            .and_then(toml::Value::as_str)
            .ok_or("registry package has no name")?;
        let version = table
            .get("version")
            .and_then(toml::Value::as_str)
            .ok_or("registry package has no version")?;
        let checksum = table
            .get("checksum")
            .and_then(toml::Value::as_str)
            .ok_or("registry package has no checksum")?;
        if !valid_cargo_cache_component(name)
            || !valid_cargo_cache_component(version)
            || !is_full_hex(checksum, 64)
        {
            return Err(format!("unsafe locked registry package: {name} {version}").into());
        }
        locked.push(LockedRegistryPackage {
            name: name.to_owned(),
            version: version.to_owned(),
            checksum: checksum.to_owned(),
        });
    }
    locked.sort_by(|left, right| {
        (&left.name, &left.version, &left.checksum).cmp(&(
            &right.name,
            &right.version,
            &right.checksum,
        ))
    });
    locked.dedup();
    for pair in locked.windows(2) {
        if pair[0].name == pair[1].name && pair[0].version == pair[1].version {
            return Err(format!(
                "conflicting locked checksums for {} {}",
                pair[0].name, pair[0].version
            )
            .into());
        }
    }
    governed_git_repositories.sort();
    governed_git_repositories.dedup();
    Ok(LockedCargoInputs {
        registry_packages: locked,
        governed_git_repositories,
    })
}

#[cfg(test)]
fn locked_cargo_inputs(lock_path: &Path) -> Result<LockedCargoInputs, Box<dyn std::error::Error>> {
    let lock = read_bounded_cargo_lock(lock_path, MAX_CARGO_LOCK_BYTES)?;
    locked_cargo_inputs_from_contents(&lock.contents)
}

fn governed_locked_git_repository(source: &str) -> Option<&str> {
    let source = source.strip_prefix("git+http://127.0.0.1:8787/git/")?;
    let (identity, commit) = source.rsplit_once('#')?;
    if identity.contains('#') || !is_full_hex(commit, 40) {
        return None;
    }
    let (repo_path, tag) = identity.split_once("?tag=")?;
    if repo_path.contains('?') || !valid_cargo_cache_component(tag) {
        return None;
    }
    let mut components = repo_path.split('/');
    let owner = components.next().unwrap_or_default();
    let repo = components
        .next()
        .and_then(|value| value.strip_suffix(".git"))
        .unwrap_or_default();
    (components.next().is_none()
        && matches!(owner, "veox" | "jeryu" | "jain-split" | "redline")
        && valid_cargo_cache_component(repo)
        && !repo.starts_with('.')
        && !repo.ends_with('.')
        && tag.starts_with(&format!("{repo}-v")))
    .then_some(repo)
}

fn valid_cargo_cache_component(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'+'))
}

fn sparse_index_relative_path(name: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let name = name.to_ascii_lowercase();
    if !valid_cargo_cache_component(&name) {
        return Err(format!("unsafe crates.io package name: {name}").into());
    }
    let relative = match name.len() {
        1 => PathBuf::from("1").join(&name),
        2 => PathBuf::from("2").join(&name),
        3 => PathBuf::from("3").join(&name[0..1]).join(&name),
        _ => PathBuf::from(&name[0..2]).join(&name[2..4]).join(&name),
    };
    Ok(relative)
}

fn sha256_regular_file(path: &Path, kind: &str) -> Result<String, Box<dyn std::error::Error>> {
    let metadata = physical_regular_file(path, kind)?;
    if metadata.nlink() != 1 || metadata.len() > 512 * 1024 * 1024 {
        return Err(format!("unsafe {kind} inode: {}", path.display()).into());
    }
    let mut input = fs::File::open(path)?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = input.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn governed_cache_directory(
    path: &Path,
    kind: &str,
    expected_uid: u32,
    expected_gid: u32,
) -> Result<fs::Metadata, Box<dyn std::error::Error>> {
    let metadata = physical_directory(path, kind)?;
    if metadata.uid() != expected_uid
        || metadata.gid() != expected_gid
        || metadata.mode() & 0o022 != 0
    {
        return Err(format!("unsafe {kind} ownership or mode: {}", path.display()).into());
    }
    Ok(metadata)
}

fn governed_cache_file_digest(
    path: &Path,
    kind: &str,
    expected_uid: u32,
    expected_gid: u32,
    max_bytes: u64,
) -> Result<(u64, String), Box<dyn std::error::Error>> {
    const O_NONBLOCK: i32 = 0o4000;
    const O_NOFOLLOW: i32 = 0o400000;
    let mut input = fs::OpenOptions::new()
        .read(true)
        .custom_flags(O_NONBLOCK | O_NOFOLLOW)
        .open(path)?;
    let metadata = input.metadata()?;
    if !metadata.file_type().is_file()
        || metadata.nlink() != 1
        || metadata.uid() != expected_uid
        || metadata.gid() != expected_gid
        || metadata.mode() & 0o022 != 0
        || metadata.len() > max_bytes
    {
        return Err(format!("unsafe {kind} inode: {}", path.display()).into());
    }
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    let mut total = 0_u64;
    loop {
        let count = input.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        total += count as u64;
        if total > max_bytes {
            return Err(format!("{kind} exceeded its byte limit: {}", path.display()).into());
        }
        digest.update(&buffer[..count]);
    }
    if total != metadata.len() {
        return Err(format!("{kind} size changed while reading: {}", path.display()).into());
    }
    Ok((total, format!("{:x}", digest.finalize())))
}

fn direct_physical_directories(
    root: &Path,
    kind: &str,
    expected_uid: u32,
    expected_gid: u32,
) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    governed_cache_directory(root, kind, expected_uid, expected_gid)?;
    let mut directories = Vec::new();
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_str().ok_or("cache directory name is not UTF-8")?;
        if !valid_cargo_cache_component(name) {
            return Err(format!("unsafe cache directory name: {name}").into());
        }
        let path = entry.path();
        governed_cache_directory(&path, kind, expected_uid, expected_gid)?;
        directories.push(path);
    }
    directories.sort();
    Ok(directories)
}

fn unique_existing_file(
    candidates: impl IntoIterator<Item = PathBuf>,
    kind: &str,
    expected_uid: u32,
    expected_gid: u32,
    max_bytes: u64,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let mut matches = Vec::new();
    for candidate in candidates {
        match fs::symlink_metadata(&candidate) {
            Ok(_) => {
                governed_cache_file_digest(
                    &candidate,
                    kind,
                    expected_uid,
                    expected_gid,
                    max_bytes,
                )?;
                matches.push(candidate);
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    if matches.len() != 1 {
        return Err(format!(
            "{kind} must have exactly one source, found {}",
            matches.len()
        )
        .into());
    }
    Ok(matches.remove(0))
}

fn copy_verified_file(
    source: &Path,
    destination: &Path,
    kind: &str,
    expected_uid: u32,
    expected_gid: u32,
    max_bytes: u64,
) -> Result<(u64, String), Box<dyn std::error::Error>> {
    let (source_size, source_digest) =
        governed_cache_file_digest(source, kind, expected_uid, expected_gid, max_bytes)?;
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
        fs::set_permissions(parent, fs::Permissions::from_mode(0o700))?;
    }
    let mut input = fs::OpenOptions::new()
        .read(true)
        .custom_flags(0o4000 | 0o400000)
        .open(source)?;
    let mut output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(destination)?;
    let copied = io::copy(&mut input, &mut output)?;
    output.sync_all()?;
    if copied != source_size {
        return Err(format!("short copy for {kind}: {}", source.display()).into());
    }
    let (_, source_after) =
        governed_cache_file_digest(source, kind, expected_uid, expected_gid, max_bytes)?;
    let destination_metadata = output.metadata()?;
    if !destination_metadata.file_type().is_file()
        || destination_metadata.nlink() != 1
        || destination_metadata.len() != source_size
        || source_after != source_digest
        || sha256_regular_file(destination, kind)? != source_digest
    {
        return Err(format!("{kind} changed during copy: {}", source.display()).into());
    }
    Ok((source_size, source_digest))
}

fn trusted_registry_directory(
    root: &Path,
    path: &Path,
    expected_uid: u32,
    expected_gid: u32,
    kind: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let relative = path
        .strip_prefix(root)
        .map_err(|_| format!("{kind} escaped the registry root"))?;
    if !relative
        .components()
        .all(|component| matches!(component, Component::Normal(_)))
        && !relative.as_os_str().is_empty()
    {
        return Err(format!("unsafe {kind} path: {}", path.display()).into());
    }
    let mut current = root.to_path_buf();
    for component in std::iter::once(None).chain(relative.components().map(Some)) {
        if let Some(Component::Normal(name)) = component {
            current.push(name);
        }
        let metadata = physical_directory(&current, kind)?;
        if metadata.uid() != expected_uid
            || metadata.gid() != expected_gid
            || metadata.mode() & 0o7777 != 0o555
        {
            return Err(format!("unsafe {kind} identity or mode: {}", current.display()).into());
        }
    }
    Ok(())
}

fn trusted_registry_file(
    root: &Path,
    path: &Path,
    expected_uid: u32,
    expected_gid: u32,
    kind: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("{kind} has no parent"))?;
    trusted_registry_directory(root, parent, expected_uid, expected_gid, kind)?;
    let metadata = physical_regular_file(path, kind)?;
    if metadata.uid() != expected_uid
        || metadata.gid() != expected_gid
        || metadata.mode() & 0o7777 != 0o444
        || metadata.nlink() != 1
    {
        return Err(format!("unsafe {kind} identity or mode: {}", path.display()).into());
    }
    Ok(())
}

struct CargoCacheStageGuard {
    path: Option<PathBuf>,
}

impl CargoCacheStageGuard {
    fn create(parent: &Path, destination_name: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        for attempt in 0..16_u8 {
            let path = parent.join(format!(
                ".{destination_name}.stage-{}-{nonce}-{attempt}",
                std::process::id()
            ));
            match fs::create_dir(&path) {
                Ok(()) => {
                    fs::set_permissions(&path, fs::Permissions::from_mode(0o700))?;
                    return Ok(Self { path: Some(path) });
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error.into()),
            }
        }
        Err("cannot create unique Cargo cache staging directory".into())
    }

    fn path(&self) -> &Path {
        self.path.as_deref().expect("staging path is present")
    }

    fn commit(mut self, destination: &Path) -> Result<(), Box<dyn std::error::Error>> {
        let path = self.path.take().expect("staging path is present");
        if let Err(error) = fs::rename(&path, destination) {
            self.path = Some(path);
            return Err(error.into());
        }
        Ok(())
    }
}

impl Drop for CargoCacheStageGuard {
    fn drop(&mut self) {
        if let Some(path) = self.path.take() {
            let _ = fs::remove_dir_all(path);
        }
    }
}

#[cfg(test)]
fn stage_locked_cargo_cache(
    lock_path: &Path,
    source: &Path,
    destination: &Path,
    receipt: &Path,
    expected_source_uid: u32,
    expected_source_gid: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    stage_locked_cargo_caches(
        &[lock_path.to_path_buf()],
        source,
        destination,
        receipt,
        expected_source_uid,
        expected_source_gid,
    )
}

fn stage_locked_cargo_caches(
    lock_paths: &[PathBuf],
    source: &Path,
    destination: &Path,
    receipt: &Path,
    expected_source_uid: u32,
    expected_source_gid: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    const MAX_CONFIG_BYTES: u64 = 1024 * 1024;
    const MAX_ARCHIVE_BYTES: u64 = 512 * 1024 * 1024;
    const MAX_INDEX_RECORD_BYTES: u64 = 16 * 1024 * 1024;
    if lock_paths.is_empty() {
        return Err("at least one --lock is required".into());
    }
    if lock_paths.len() > MAX_CARGO_LOCKS {
        return Err(format!("at most {MAX_CARGO_LOCKS} Cargo locks may be staged").into());
    }
    let mut unique_locks = std::collections::BTreeSet::new();
    for lock_path in lock_paths {
        if !lock_path.is_absolute()
            || !lock_path
                .components()
                .all(|component| matches!(component, Component::RootDir | Component::Normal(_)))
        {
            return Err(
                format!("Cargo lock path is not normalized: {}", lock_path.display()).into(),
            );
        }
        if !unique_locks.insert(lock_path.clone()) {
            return Err(format!("duplicate Cargo lock path: {}", lock_path.display()).into());
        }
    }
    if !source.is_absolute() || !destination.is_absolute() || destination.exists() {
        return Err("Cargo cache destination must be a new absolute path".into());
    }
    trusted_registry_directory(
        source,
        source,
        expected_source_uid,
        expected_source_gid,
        "root Cargo registry cache",
    )?;
    let receipt_relative = receipt
        .strip_prefix(destination)
        .map_err(|_| "Cargo cache receipt must be beneath the destination")?;
    if !receipt.is_absolute()
        || receipt_relative.as_os_str().is_empty()
        || !receipt_relative
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
    {
        return Err("Cargo cache receipt must be beneath the destination".into());
    }
    let destination_parent = destination
        .parent()
        .ok_or("Cargo cache destination has no parent")?;
    physical_directory(destination_parent, "Cargo cache destination parent")?;
    let destination_name = destination
        .file_name()
        .and_then(OsStr::to_str)
        .filter(|name| valid_cargo_cache_component(name))
        .ok_or("Cargo cache destination name is unsafe")?;
    let mut packages = Vec::new();
    let mut governed_git_repositories = Vec::new();
    let mut lock_digests = Vec::with_capacity(lock_paths.len());
    let bounded_locks = read_bounded_cargo_locks(lock_paths)?;
    for lock in &bounded_locks {
        let inputs = locked_cargo_inputs_from_contents(&lock.contents)?;
        packages.extend(inputs.registry_packages);
        governed_git_repositories.extend(inputs.governed_git_repositories);
        lock_digests.push(lock.digest.clone());
    }
    packages.sort_by(|left, right| {
        (&left.name, &left.version, &left.checksum).cmp(&(
            &right.name,
            &right.version,
            &right.checksum,
        ))
    });
    packages.dedup();
    governed_git_repositories.sort();
    governed_git_repositories.dedup();
    for pair in packages.windows(2) {
        if pair[0].name == pair[1].name && pair[0].version == pair[1].version {
            return Err(format!(
                "conflicting locked checksums for {} {} across Cargo locks",
                pair[0].name, pair[0].version
            )
            .into());
        }
    }
    lock_digests.sort();
    let archive_parent = source.join("cache");
    let index_parent = source.join("index");
    trusted_registry_directory(
        source,
        &archive_parent,
        expected_source_uid,
        expected_source_gid,
        "archive cache root",
    )?;
    trusted_registry_directory(
        source,
        &index_parent,
        expected_source_uid,
        expected_source_gid,
        "sparse index cache root",
    )?;
    let archive_roots = direct_physical_directories(
        &archive_parent,
        "archive cache",
        expected_source_uid,
        expected_source_gid,
    )?;
    let index_roots = direct_physical_directories(
        &index_parent,
        "sparse index cache",
        expected_source_uid,
        expected_source_gid,
    )?;
    for root in archive_roots.iter().chain(index_roots.iter()) {
        trusted_registry_directory(
            source,
            root,
            expected_source_uid,
            expected_source_gid,
            "registry cache directory",
        )?;
    }
    let sparse_roots = index_roots
        .iter()
        .filter(|root| root.join(".cache").is_dir())
        .cloned()
        .collect::<Vec<_>>();
    if sparse_roots.len() != 1 {
        return Err(format!(
            "expected one sparse index cache, found {}",
            sparse_roots.len()
        )
        .into());
    }
    let sparse_root = &sparse_roots[0];
    let sparse_name = sparse_root
        .file_name()
        .and_then(OsStr::to_str)
        .ok_or("sparse index cache name is unavailable")?;
    let config_source = sparse_root.join("config.json");
    trusted_registry_file(
        source,
        &config_source,
        expected_source_uid,
        expected_source_gid,
        "sparse index config",
    )?;
    let staging = CargoCacheStageGuard::create(destination_parent, destination_name)?;
    let staging_root = staging.path();
    copy_verified_file(
        &config_source,
        &staging_root
            .join("index")
            .join(sparse_name)
            .join("config.json"),
        "sparse index config",
        expected_source_uid,
        expected_source_gid,
        MAX_CONFIG_BYTES,
    )?;

    let mut staged = Vec::new();
    let mut staged_index_records = std::collections::BTreeSet::new();
    for package in &packages {
        let archive_name = format!("{}-{}.crate", package.name, package.version);
        let archive_source = unique_existing_file(
            archive_roots.iter().map(|root| root.join(&archive_name)),
            "locked crate archive",
            expected_source_uid,
            expected_source_gid,
            MAX_ARCHIVE_BYTES,
        )?;
        trusted_registry_file(
            source,
            &archive_source,
            expected_source_uid,
            expected_source_gid,
            "locked crate archive",
        )?;
        let (_, actual_checksum) = governed_cache_file_digest(
            &archive_source,
            "locked crate archive",
            expected_source_uid,
            expected_source_gid,
            MAX_ARCHIVE_BYTES,
        )?;
        if actual_checksum != package.checksum {
            return Err(format!(
                "locked crate checksum mismatch for {} {}",
                package.name, package.version
            )
            .into());
        }
        let archive_root_name = archive_source
            .parent()
            .and_then(Path::file_name)
            .and_then(OsStr::to_str)
            .ok_or("archive cache name is unavailable")?;
        let archive_destination = staging_root
            .join("cache")
            .join(archive_root_name)
            .join(&archive_name);
        copy_verified_file(
            &archive_source,
            &archive_destination,
            "locked crate archive",
            expected_source_uid,
            expected_source_gid,
            MAX_ARCHIVE_BYTES,
        )?;
        if sha256_regular_file(&archive_destination, "staged crate archive")? != package.checksum {
            return Err(format!("staged crate checksum changed for {archive_name}").into());
        }

        let index_relative = sparse_index_relative_path(&package.name)?;
        if staged_index_records.insert(index_relative.clone()) {
            let index_source = unique_existing_file(
                std::iter::once(sparse_root.join(".cache").join(&index_relative)),
                "sparse index record",
                expected_source_uid,
                expected_source_gid,
                MAX_INDEX_RECORD_BYTES,
            )
            .map_err(|error| {
                format!(
                    "sparse index record unavailable for {} {}: {error}",
                    package.name, package.version
                )
            })?;
            trusted_registry_file(
                source,
                &index_source,
                expected_source_uid,
                expected_source_gid,
                "sparse index record",
            )?;
            copy_verified_file(
                &index_source,
                &staging_root
                    .join("index")
                    .join(sparse_name)
                    .join(".cache")
                    .join(&index_relative),
                "sparse index record",
                expected_source_uid,
                expected_source_gid,
                MAX_INDEX_RECORD_BYTES,
            )?;
        }
        staged.push(json!({
            "name": package.name,
            "version": package.version,
            "checksum": package.checksum,
        }));
    }
    let mut final_lock_digests = read_bounded_cargo_locks(lock_paths)?
        .into_iter()
        .map(|lock| lock.digest)
        .collect::<Vec<_>>();
    final_lock_digests.sort();
    if final_lock_digests != lock_digests {
        return Err("a Cargo lock changed while staging the registry cache".into());
    }
    let receipt_value = json!({
        "schema_version": "jain.locked-cargo-cache/v2",
        "lock_count": lock_digests.len(),
        "lock_sha256s": lock_digests,
        "package_count": staged.len(),
        "packages": staged,
        "governed_git_repositories": governed_git_repositories,
    });
    let receipt_bytes = serde_json::to_vec_pretty(&receipt_value)?;
    let staged_receipt = staging_root.join(receipt_relative);
    write_atomic_bytes(&staged_receipt, &receipt_bytes)?;
    fs::set_permissions(&staged_receipt, fs::Permissions::from_mode(0o600))?;
    staging.commit(destination)?;
    println!("staged {} checksum-verified crates", packages.len());
    Ok(())
}

fn host_ci_snapshot_request_command(args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    let mut source = None;
    let mut destination = None;
    let mut expected_uid = None;
    let mut expected_gid = None;
    let mut max_bytes = None;
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--source" => source = Some(PathBuf::from(iter.next().ok_or("--source needs a path")?)),
            "--destination" => {
                destination = Some(PathBuf::from(
                    iter.next().ok_or("--destination needs a path")?,
                ))
            }
            "--expected-uid" => {
                expected_uid = Some(
                    iter.next()
                        .ok_or("--expected-uid needs a value")?
                        .parse::<u32>()?,
                )
            }
            "--expected-gid" => {
                expected_gid = Some(
                    iter.next()
                        .ok_or("--expected-gid needs a value")?
                        .parse::<u32>()?,
                )
            }
            "--max-bytes" => {
                max_bytes = Some(
                    iter.next()
                        .ok_or("--max-bytes needs a value")?
                        .parse::<u64>()?,
                )
            }
            value => {
                return Err(format!("unknown host-ci-snapshot-request argument: {value}").into())
            }
        }
    }
    let source = source.ok_or("--source is required")?;
    let destination = destination.ok_or("--destination is required")?;
    let max_bytes = max_bytes.ok_or("--max-bytes is required")?;
    if max_bytes == 0 || max_bytes > 1_048_576 {
        return Err("--max-bytes must be between 1 and 1048576".into());
    }
    snapshot_host_ci_request(
        &source,
        &destination,
        expected_uid.ok_or("--expected-uid is required")?,
        expected_gid.ok_or("--expected-gid is required")?,
        max_bytes,
    )
}

fn snapshot_host_ci_request(
    source: &Path,
    destination: &Path,
    expected_uid: u32,
    expected_gid: u32,
    max_bytes: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    // Linux O_NONBLOCK prevents a path-swap to a FIFO from hanging the root
    // broker; O_NOFOLLOW binds the read to a non-symlink inode.
    const O_NONBLOCK: i32 = 0o4000;
    const O_NOFOLLOW: i32 = 0o400000;
    let input = fs::OpenOptions::new()
        .read(true)
        .custom_flags(O_NONBLOCK | O_NOFOLLOW)
        .open(source)?;
    let metadata = input.metadata()?;
    if !metadata.file_type().is_file()
        || metadata.nlink() != 1
        || metadata.uid() != expected_uid
        || metadata.gid() != expected_gid
        || metadata.mode() & 0o7777 != 0o600
        || metadata.len() > max_bytes
    {
        return Err("unsafe host-CI request inode".into());
    }
    let mut bytes = Vec::with_capacity((max_bytes + 1) as usize);
    input.take(max_bytes + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > max_bytes {
        return Err("host-CI request exceeds byte limit".into());
    }
    let mut output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(destination)?;
    output.write_all(&bytes)?;
    output.sync_all()?;
    let output_metadata = output.metadata()?;
    if !output_metadata.file_type().is_file()
        || output_metadata.nlink() != 1
        || output_metadata.mode() & 0o7777 != 0o600
    {
        drop(output);
        let _ = fs::remove_file(destination);
        return Err("unsafe host-CI request snapshot".into());
    }
    Ok(())
}

fn manifest_command(args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    let mut path = PathBuf::from("repos.manifest.toml");
    let mut check_paths = false;
    let mut json_output = false;
    let mut check_derived = false;
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--manifest" => path = PathBuf::from(iter.next().ok_or("--manifest needs a path")?),
            "--check-paths" => check_paths = true,
            "--json" => json_output = true,
            "--check-derived" => check_derived = true,
            value if !value.starts_with('-') => path = PathBuf::from(value),
            value => return Err(format!("unknown manifest argument: {value}").into()),
        }
    }
    let data: toml::Value = fs::read_to_string(&path)?.parse()?;
    if check_derived {
        validate_manifest_data(&data, &path, check_paths)?;
        let expected = manifest_sha256(&path)?;
        for (target, derived) in derived_manifest_targets(&data, &path)? {
            validate_derived_manifest(&derived, &expected, &data, &path, &target)?;
        }
    }
    let repos = family_repos(&data)?;
    if repos.is_empty() {
        return Err("manifest has no repos".into());
    }
    let mut rows = Vec::new();
    for repo in &repos {
        let name = string(repo, "name").ok_or("repo missing name")?;
        let repo_path = PathBuf::from(string(repo, "path").ok_or("repo missing path")?);
        for field in [
            "github_slug",
            "jeryu_slug",
            "profile",
            "default_branch",
            "required_check",
        ] {
            if string(repo, field).is_none() {
                return Err(format!("{name} missing {field}").into());
            }
        }
        if string(repo, "default_branch").as_deref() != Some("main") {
            return Err(format!("{name} default_branch must be main").into());
        }
        if repo.get("has_jeryu_std").and_then(toml::Value::as_bool) != Some(true) {
            return Err(format!("{name} must set has_jeryu_std=true").into());
        }
        if check_paths && repo_is_onboarded(repo) {
            for required in ["AGENTS.md", "agent/owner-map.json", "agent/test-map.json"] {
                if !repo_path.join(required).exists() {
                    return Err(format!("{name} missing {required}").into());
                }
            }
        }
        rows.push((
            name,
            repo_path,
            string(repo, "github_slug").unwrap(),
            string(repo, "jeryu_slug").unwrap(),
            string(repo, "required_check").unwrap(),
        ));
    }
    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "family": data.get("family"),
                "repo": repos,
                "control_plane": data.get("control_plane"),
                "infrastructure_repo": data.get("infrastructure_repo"),
                "release_version": data.get("release_version"),
                "release_status": data.get("status"),
                "formal_ga": data.get("formal_ga"),
                "rollback_target": data.get("rollback_target"),
                "fleet_jobs": data.get("fleet_jobs"),
                "ci_jobs": data.get("ci_jobs"),
                "family_repo_count": repos.len(),
                "canonical_manifest_sha256": manifest_sha256(&path)?,
                "manifest_authority": "jain-split-ops/repos.manifest.toml",
                "infrastructure_repo_count": data
                    .get("infrastructure_repo")
                    .and_then(toml::Value::as_array)
                    .map_or(0, Vec::len),
            }))?
        );
    } else {
        for (name, path, github, jeryu, required) in rows {
            println!("{name}|{}|{github}|{jeryu}|{required}", path.display());
        }
    }
    Ok(())
}

fn managed_repos_command(args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    let root = control_plane_root();
    let mut manifest = root.join("repos.manifest.toml");
    let mut json_output = false;
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--manifest" => manifest = PathBuf::from(iter.next().ok_or("--manifest needs a path")?),
            "--json" => json_output = true,
            value => return Err(format!("unknown managed-repos argument: {value}").into()),
        }
    }
    if !json_output {
        return Err("managed-repos requires --json".into());
    }
    let data: toml::Value = fs::read_to_string(&manifest)?.parse()?;
    let repos = managed_repositories(&data, &manifest)?;
    let mut family_counts = std::collections::BTreeMap::new();
    for repo in &repos {
        *family_counts.entry(repo.family.clone()).or_insert(0usize) += 1;
    }
    let nested_family_gates = registered_nested_families(&data)
        .into_iter()
        .map(|(key, family)| {
            json!({
                "registration": key,
                "family": string(family, "family"),
                "release_identity": string(family, "release_identity"),
                "symlink_policy": string(family, "symlink_policy"),
                "redline_source_policy": string(family, "redline_source_policy"),
                "retirement_pending": registered_nested_projection(family)
                    .filter_map(|repo| {
                        (string(repo, "runtime_authority").as_deref()
                            == Some("retirement-pending"))
                            .then(|| string(repo, "name"))
                            .flatten()
                    })
                    .collect::<Vec<_>>(),
            })
        })
        .collect::<Vec<_>>();
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "schema_version": "jain.managed-repositories/v1",
            "manifest": manifest,
            "repositories": repos.iter().map(managed_repo_json).collect::<Vec<_>>(),
            "repository_count": repos.len(),
            "active_repository_count": repos.iter()
                .filter(|repo| repo.inventory_status == "active")
                .count(),
            "family_counts": family_counts,
            "nested_family_gates": nested_family_gates,
        }))?
    );
    Ok(())
}

fn managed_repo_json(repo: &ManagedRepo) -> JsonValue {
    json!({
        "name": repo.name,
        "path": repo.path,
        "remote": repo.remote,
        "required_check": repo.required_check,
        "branch": repo.branch,
        "tag": repo.tag,
        "kind": repo.kind,
        "family": repo.family,
        "family_registered": repo.family_registered,
        "inventory_status": repo.inventory_status,
        "runtime_authority": repo.runtime_authority,
    })
}

fn host_ci_authority_command(args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    let mut manifest = None;
    let mut repo_name = None;
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--manifest" => {
                manifest = Some(PathBuf::from(iter.next().ok_or("--manifest needs a path")?))
            }
            "--repo" => repo_name = Some(iter.next().ok_or("--repo needs a name")?),
            value => return Err(format!("unknown host-ci-authority argument: {value}").into()),
        }
    }
    let repo_name = repo_name.ok_or("host-ci-authority requires --repo")?;
    let manifest = manifest.unwrap_or_else(|| control_plane_root().join("repos.manifest.toml"));
    let data: toml::Value = fs::read_to_string(&manifest)?.parse()?;
    println!(
        "{}",
        serde_json::to_string_pretty(&host_ci_authority(&data, &repo_name)?)?
    );
    Ok(())
}

fn host_ci_authority(data: &toml::Value, repo_name: &str) -> Result<JsonValue, String> {
    let mut matches = Vec::new();
    let mut add = |raw: &toml::Value,
                   name_key: &str,
                   check_key: &str,
                   remote_key: &str,
                   owner_key: &str,
                   expected_owner: Option<&str>|
     -> Result<(), String> {
        let Some(name) = string(raw, name_key) else {
            return Ok(());
        };
        if name != repo_name {
            return Ok(());
        }
        let owner = string(raw, owner_key)
            .or_else(|| expected_owner.map(str::to_owned))
            .unwrap_or_else(|| "jeryu".to_owned());
        if expected_owner.is_some_and(|expected| owner != expected) {
            return Err(format!(
                "host CI authority for {name} has non-canonical forge owner: {owner}"
            ));
        }
        let slug = format!("{owner}/{name}");
        validate_jeryu_repo_slug(&slug).map_err(|error| error.to_string())?;
        if string(raw, "jeryu_slug").is_some_and(|declared| declared != slug) {
            return Err(format!(
                "host CI authority for {name} has non-canonical Jeryu slug"
            ));
        }
        let required_check = string(raw, check_key)
            .ok_or_else(|| format!("host CI authority for {name} is missing {check_key}"))?;
        if required_check != format!("{name}/required") {
            return Err(format!(
                "host CI authority for {name} must require exact {name}/required"
            ));
        }
        let remote = string(raw, remote_key)
            .or_else(|| {
                string(raw, "jeryu_slug")
                    .map(|declared| format!("{LOCAL_JERYU_ORIGIN}/git/{declared}.git"))
            })
            .ok_or_else(|| format!("host CI authority for {name} is missing {remote_key}"))?;
        let expected_remote = format!("{LOCAL_JERYU_ORIGIN}/git/{owner}/{name}.git");
        if remote != expected_remote {
            return Err(format!(
                "host CI authority for {name} has non-canonical remote: {remote}"
            ));
        }
        let release_cuda_compute_capability_required =
            release_cuda_compute_capability_required(raw)?;
        matches.push((
            owner,
            required_check,
            remote,
            release_cuda_compute_capability_required,
        ));
        Ok(())
    };

    if let Some(control) = data.get("control_plane") {
        add(
            control,
            "name",
            "required_check",
            "remote",
            "forge_owner",
            Some("veox"),
        )?;
    }
    for key in ["repo", "infrastructure_repo"] {
        for raw in data
            .get(key)
            .and_then(toml::Value::as_array)
            .into_iter()
            .flatten()
        {
            add(
                raw,
                "name",
                "required_check",
                "remote",
                "forge_owner",
                Some("veox"),
            )?;
        }
    }
    if let Some(external) = data
        .get("external_dependencies")
        .and_then(toml::Value::as_table)
    {
        for raw in external.values() {
            add(raw, "repository", "required_check", "remote", "owner", None)?;
        }
    }
    if let Some(nested) = data.get("nested_families").and_then(toml::Value::as_table) {
        for raw in nested.values() {
            add(
                raw,
                "control_plane_name",
                "control_plane_required_check",
                "control_plane_remote",
                "forge_owner",
                None,
            )?;
            for pending in raw
                .get("pending_repository")
                .and_then(toml::Value::as_array)
                .into_iter()
                .flatten()
            {
                add(
                    pending,
                    "name",
                    "required_check",
                    "remote",
                    "forge_owner",
                    None,
                )?;
            }
            for repository in raw
                .get("repository")
                .and_then(toml::Value::as_array)
                .into_iter()
                .flatten()
            {
                add(
                    repository,
                    "name",
                    "required_check",
                    "remote",
                    "forge_owner",
                    string(raw, "forge_owner").as_deref(),
                )?;
            }
        }
    }

    if matches.len() != 1 {
        return Err(format!(
            "host CI repository authority for {repo_name} is absent or ambiguous"
        ));
    }
    let (forge_owner, required_check, remote, release_cuda_compute_capability_required) =
        matches.pop().unwrap();
    Ok(json!({
        "schema_version": "jain.host-ci-repository-authority/v1",
        "repository": repo_name,
        "forge_owner": forge_owner,
        "required_check": required_check,
        "remote": remote,
        "release_cuda_compute_capability_required": release_cuda_compute_capability_required,
    }))
}

fn release_cargo_commands_command(args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    let mut manifest = None;
    let mut repo_name = None;
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--manifest" => {
                manifest = Some(PathBuf::from(iter.next().ok_or("--manifest needs a path")?))
            }
            "--repo" => repo_name = Some(iter.next().ok_or("--repo needs a name")?),
            value => return Err(format!("unknown release-cargo-commands argument: {value}").into()),
        }
    }
    let repo_name = repo_name.ok_or("release-cargo-commands requires --repo")?;
    let manifest = manifest.unwrap_or_else(|| control_plane_root().join("repos.manifest.toml"));
    let data: toml::Value = fs::read_to_string(&manifest)?.parse()?;
    validate_manifest_data(&data, &manifest, false)?;
    let raw = release_repo_entry(&data, &repo_name)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&release_cargo_policy(&repo_name, raw)?)?
    );
    Ok(())
}

fn release_repo_entry<'a>(
    data: &'a toml::Value,
    repo_name: &str,
) -> Result<&'a toml::Value, String> {
    for key in ["repo", "infrastructure_repo"] {
        if let Some(raw) = data
            .get(key)
            .and_then(toml::Value::as_array)
            .into_iter()
            .flatten()
            .find(|raw| string(raw, "name").as_deref() == Some(repo_name))
        {
            return Ok(raw);
        }
    }
    if let Some(control) = data.get("control_plane") {
        if string(control, "name").as_deref() == Some(repo_name) {
            return Ok(control);
        }
    }
    if let Some(external) = data
        .get("external_dependencies")
        .and_then(toml::Value::as_table)
        .and_then(|dependencies| {
            dependencies
                .values()
                .find(|raw| string(raw, "repository").as_deref() == Some(repo_name))
        })
    {
        return Ok(external);
    }
    if let Some(nested) = data.get("nested_families").and_then(toml::Value::as_table) {
        for raw in nested.values() {
            if string(raw, "control_plane_name").as_deref() == Some(repo_name) {
                return Ok(raw);
            }
            if let Some(pending) = raw
                .get("pending_repository")
                .and_then(toml::Value::as_array)
                .into_iter()
                .flatten()
                .find(|pending| string(pending, "name").as_deref() == Some(repo_name))
            {
                return Ok(pending);
            }
            if let Some(repository) = raw
                .get("repository")
                .and_then(toml::Value::as_array)
                .into_iter()
                .flatten()
                .find(|repository| string(repository, "name").as_deref() == Some(repo_name))
            {
                return Ok(repository);
            }
        }
    }
    Err(format!(
        "repository {repo_name} is not declared by the canonical manifest"
    ))
}

fn release_cargo_policy(repo_name: &str, raw: &toml::Value) -> Result<JsonValue, String> {
    let release_cuda_compute_capability_required = release_cuda_compute_capability_required(raw)?;
    let Some(matrix) = release_feature_matrix(raw)? else {
        return Ok(json!({
            "schema_version": "jain.split.release-cargo-commands/v1",
            "repo": repo_name,
            "mode": "all-features",
            "release_cuda_compute_capability_required": release_cuda_compute_capability_required,
            "commands": [
                release_cargo_command("build-all-features", "build", None),
                release_cargo_command("test-all-features", "test", None),
            ],
        }));
    };

    let commands = matrix
        .feature_sets
        .iter()
        .enumerate()
        .flat_map(|(index, features)| {
            let qualified = features
                .iter()
                .map(|feature| format!("{}/{feature}", matrix.package))
                .collect::<Vec<_>>()
                .join(",");
            [
                release_cargo_command(
                    &format!("build-feature-set-{}", index + 1),
                    "build",
                    Some(&qualified),
                ),
                release_cargo_command(
                    &format!("test-feature-set-{}", index + 1),
                    "test",
                    Some(&qualified),
                ),
            ]
        })
        .collect::<Vec<_>>();
    Ok(json!({
        "schema_version": "jain.split.release-cargo-commands/v1",
        "repo": repo_name,
        "mode": "feature-matrix",
        "release_cuda_compute_capability_required": release_cuda_compute_capability_required,
        "release_package": matrix.package,
        "release_feature_sets": matrix.feature_sets,
        "commands": commands,
    }))
}

fn release_cuda_compute_capability_required(raw: &toml::Value) -> Result<bool, String> {
    match raw.get("release_cuda_compute_capability_required") {
        None => Ok(false),
        Some(value) => value
            .as_bool()
            .ok_or_else(|| "release_cuda_compute_capability_required must be a boolean".to_owned()),
    }
}

fn release_cargo_command(label: &str, subcommand: &str, features: Option<&str>) -> JsonValue {
    let mut args = vec![subcommand.to_owned(), "--locked".to_owned()];
    if subcommand == "build" {
        args.push("--release".to_owned());
    }
    if let Some(features) = features {
        args.extend([
            "--workspace".to_owned(),
            "--no-default-features".to_owned(),
            "--features".to_owned(),
            features.to_owned(),
        ]);
    } else {
        args.push("--all-features".to_owned());
    }
    args.push("--all-targets".to_owned());
    json!({"label": label, "program": "cargo", "args": args})
}

fn release_feature_matrix(raw: &toml::Value) -> Result<Option<ReleaseFeatureMatrix>, String> {
    let package_value = raw.get("release_package");
    let sets_value = raw.get("release_feature_sets");
    if package_value.is_none() && sets_value.is_none() {
        return Ok(None);
    }
    let package = package_value
        .and_then(toml::Value::as_str)
        .ok_or("release_package must be a string when release_feature_sets is declared")?;
    if !valid_cargo_token(package) {
        return Err(format!(
            "release_package contains an unsafe Cargo token: {package}"
        ));
    }
    let raw_sets = sets_value
        .and_then(toml::Value::as_array)
        .ok_or("release_feature_sets must be an array when release_package is declared")?;
    if raw_sets.len() < 2 {
        return Err("release_feature_sets must declare at least two legal feature sets".to_owned());
    }

    let mut feature_sets = Vec::with_capacity(raw_sets.len());
    let mut normalized_sets = Vec::with_capacity(raw_sets.len());
    let mut seen_sets = std::collections::BTreeSet::new();
    for (set_index, raw_set) in raw_sets.iter().enumerate() {
        let raw_features = raw_set
            .as_array()
            .ok_or_else(|| format!("release_feature_sets[{set_index}] must be an array"))?;
        if raw_features.is_empty() {
            return Err(format!(
                "release_feature_sets[{set_index}] must not be empty"
            ));
        }
        let mut features = Vec::with_capacity(raw_features.len());
        let mut normalized = std::collections::BTreeSet::new();
        for raw_feature in raw_features {
            let feature = raw_feature.as_str().ok_or_else(|| {
                format!("release_feature_sets[{set_index}] must contain only strings")
            })?;
            if !valid_cargo_token(feature) {
                return Err(format!(
                    "release_feature_sets[{set_index}] contains an unsafe Cargo token: {feature}"
                ));
            }
            if !normalized.insert(feature.to_owned()) {
                return Err(format!(
                    "release_feature_sets[{set_index}] contains duplicate feature {feature}"
                ));
            }
            features.push(feature.to_owned());
        }
        let normalized_key = normalized.iter().cloned().collect::<Vec<_>>();
        if !seen_sets.insert(normalized_key) {
            return Err("release_feature_sets contains duplicate legal sets".to_owned());
        }
        feature_sets.push(features);
        normalized_sets.push(normalized);
    }
    for (left_index, left) in normalized_sets.iter().enumerate() {
        for (right_index, right) in normalized_sets.iter().enumerate() {
            if left_index != right_index && left.is_subset(right) {
                return Err(format!(
                    "release_feature_sets[{left_index}] is not maximal; it is a subset of release_feature_sets[{right_index}]"
                ));
            }
        }
    }
    Ok(Some(ReleaseFeatureMatrix {
        package: package.to_owned(),
        feature_sets,
    }))
}

fn valid_cargo_token(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_'))
}

fn declared_release_tag(value: &toml::Value) -> Option<String> {
    string(value, "immutable_tag").or_else(|| string(value, "current_tag"))
}

fn inventory_status(value: &toml::Value) -> String {
    string(value, "inventory_status").unwrap_or_else(|| "active".to_owned())
}

fn runtime_authority(value: &toml::Value, fallback: &str) -> String {
    string(value, "runtime_authority").unwrap_or_else(|| fallback.to_owned())
}

fn registered_nested_families(data: &toml::Value) -> Vec<(&str, &toml::Value)> {
    data.get("nested_families")
        .and_then(toml::Value::as_table)
        .into_iter()
        .flat_map(|families| families.iter())
        .filter(|(_, family)| family.get("engine_repository").is_none())
        .map(|(name, family)| (name.as_str(), family))
        .collect()
}

fn registered_nested_projection(family: &toml::Value) -> impl Iterator<Item = &toml::Value> {
    family
        .get("repository")
        .and_then(toml::Value::as_array)
        .into_iter()
        .flatten()
}

fn validate_registered_nested_family_declaration(
    key: &str,
    family: &toml::Value,
    split_root: &Path,
) -> Result<(), String> {
    let qualified = format!("nested_families.{key}");
    if family.get("required").and_then(toml::Value::as_bool) != Some(true) {
        return Err(format!("{qualified}.required must be true"));
    }
    let identity = string(family, "family")
        .filter(|value| valid_cargo_token(value))
        .ok_or_else(|| format!("{qualified}.family must be one unaliased family token"))?;
    if string(family, "release_identity").as_deref() != Some(identity.as_str()) {
        return Err(format!(
            "{qualified}.release_identity must equal its independent family identity {identity}"
        ));
    }
    let lineage = string(family, "release_lineage")
        .ok_or_else(|| format!("{qualified}.release_lineage is required"))?;
    if !lineage
        .strip_prefix('v')
        .is_some_and(|version| !version.is_empty() && version.chars().all(|ch| ch.is_ascii_digit()))
    {
        return Err(format!(
            "{qualified}.release_lineage must be v followed by digits"
        ));
    }
    let owner = string(family, "forge_owner")
        .filter(|value| valid_cargo_token(value))
        .ok_or_else(|| format!("{qualified}.forge_owner is required"))?;
    if string(family, "inventory_mode").as_deref() != Some("recursive-authority") {
        return Err(format!(
            "{qualified}.inventory_mode must be recursive-authority"
        ));
    }
    let redline_source_policy = string(family, "redline_source_policy")
        .ok_or_else(|| format!("{qualified}.redline_source_policy is required"))?;
    if !matches!(
        redline_source_policy.as_str(),
        "enforced" | "retirement-pending"
    ) {
        return Err(format!(
            "{qualified}.redline_source_policy must be enforced or retirement-pending"
        ));
    }
    let symlink_policy = string(family, "symlink_policy")
        .ok_or_else(|| format!("{qualified}.symlink_policy is required"))?;
    if !matches!(symlink_policy.as_str(), "enforced" | "retirement-pending") {
        return Err(format!(
            "{qualified}.symlink_policy must be enforced or retirement-pending"
        ));
    }
    let container = exact_absolute_path(
        &string(family, "container_path")
            .ok_or_else(|| format!("{qualified}.container_path is required"))?,
        &format!("{qualified}.container_path"),
    )?;
    if container == split_root || container.parent() != Some(split_root) {
        return Err(format!(
            "{qualified}.container_path must be a direct child of split_root"
        ));
    }
    let control = exact_absolute_path(
        &string(family, "control_plane")
            .ok_or_else(|| format!("{qualified}.control_plane is required"))?,
        &format!("{qualified}.control_plane"),
    )?;
    let control_name = string(family, "control_plane_name")
        .filter(|value| valid_cargo_token(value))
        .ok_or_else(|| format!("{qualified}.control_plane_name is required"))?;
    if control != container.join(&control_name) {
        return Err(format!(
            "{qualified}.control_plane must be {}/{}",
            container.display(),
            control_name
        ));
    }
    let manifest = exact_absolute_path(
        &string(family, "manifest_path")
            .ok_or_else(|| format!("{qualified}.manifest_path is required"))?,
        &format!("{qualified}.manifest_path"),
    )?;
    if manifest != control.join("repos.manifest.toml") {
        return Err(format!(
            "{qualified}.manifest_path must be {}/repos.manifest.toml",
            control.display()
        ));
    }
    let control_remote = format!("{LOCAL_JERYU_ORIGIN}/git/{owner}/{control_name}.git");
    if string(family, "control_plane_remote").as_deref() != Some(control_remote.as_str()) {
        return Err(format!(
            "{qualified}.control_plane_remote must be {control_remote}"
        ));
    }
    if string(family, "control_plane_required_check").as_deref()
        != Some(format!("{control_name}/required").as_str())
    {
        return Err(format!(
            "{qualified}.control_plane_required_check must be {control_name}/required"
        ));
    }
    let control_tag = string(family, "control_plane_current_tag")
        .ok_or_else(|| format!("{qualified}.control_plane_current_tag is required"))?;
    if !control_tag.starts_with(&format!("{control_name}-{lineage}."))
        || !control_tag.contains("-split.")
    {
        return Err(format!(
            "{qualified}.control_plane_current_tag must preserve {lineage} split lineage"
        ));
    }
    if string(family, "control_plane_inventory_status").as_deref() != Some("active") {
        return Err(format!(
            "{qualified}.control_plane_inventory_status must be active"
        ));
    }
    if string(family, "control_plane_runtime_authority").as_deref() != Some("control-plane") {
        return Err(format!(
            "{qualified}.control_plane_runtime_authority must be control-plane"
        ));
    }
    let canonical_redline = split_root.join("jain-redline");
    if string(family, "redline_authority").as_deref()
        != Some(canonical_redline.to_string_lossy().as_ref())
    {
        return Err(format!(
            "{qualified}.redline_authority must be {}",
            canonical_redline.display()
        ));
    }

    let mut names = std::collections::BTreeSet::new();
    let mut paths = std::collections::BTreeSet::new();
    let mut remotes = std::collections::BTreeSet::new();
    let mut retirement_pending = Vec::new();
    for repository in registered_nested_projection(family) {
        let name = string(repository, "name")
            .filter(|value| valid_cargo_token(value))
            .ok_or_else(|| format!("{qualified}.repository name is required"))?;
        if !names.insert(name.clone()) {
            return Err(format!("{qualified} has duplicate repository name {name}"));
        }
        if name == control_name {
            return Err(format!(
                "{qualified} must declare its control plane separately from repository projections"
            ));
        }
        let path = exact_absolute_path(
            &string(repository, "path")
                .ok_or_else(|| format!("{qualified}.repository[{name}].path is required"))?,
            &format!("{qualified}.repository[{name}].path"),
        )?;
        if path != container.join(&name) {
            return Err(format!(
                "{qualified}.repository[{name}].path must be {}/{}",
                container.display(),
                name
            ));
        }
        if !paths.insert(path) {
            return Err(format!("{qualified} has a duplicate repository path"));
        }
        let slug = format!("{owner}/{name}");
        if string(repository, "jeryu_slug").as_deref() != Some(slug.as_str()) {
            return Err(format!(
                "{qualified}.repository[{name}].jeryu_slug must be {slug}"
            ));
        }
        let remote = format!("{LOCAL_JERYU_ORIGIN}/git/{slug}.git");
        if string(repository, "remote").as_deref() != Some(remote.as_str()) {
            return Err(format!(
                "{qualified}.repository[{name}].remote must be {remote}"
            ));
        }
        if !remotes.insert(remote) {
            return Err(format!("{qualified} has a duplicate repository remote"));
        }
        if string(repository, "required_check").as_deref()
            != Some(format!("{name}/required").as_str())
        {
            return Err(format!(
                "{qualified}.repository[{name}].required_check must be {name}/required"
            ));
        }
        if string(repository, "default_branch").as_deref() != Some("main") {
            return Err(format!(
                "{qualified}.repository[{name}].default_branch must be main"
            ));
        }
        let tag = string(repository, "current_tag")
            .ok_or_else(|| format!("{qualified}.repository[{name}].current_tag is required"))?;
        if !tag.starts_with(&format!("{name}-{lineage}.")) || !tag.contains("-split.") {
            return Err(format!(
                "{qualified}.repository[{name}].current_tag must preserve {lineage} split lineage"
            ));
        }
        if string(repository, "inventory_status").as_deref() != Some("active") {
            return Err(format!(
                "{qualified}.repository[{name}].inventory_status must be active while projected"
            ));
        }
        let runtime = string(repository, "runtime_authority").ok_or_else(|| {
            format!("{qualified}.repository[{name}].runtime_authority is required")
        })?;
        if !matches!(
            runtime.as_str(),
            "library" | "shadow-only" | "retirement-pending"
        ) {
            return Err(format!(
                "{qualified}.repository[{name}].runtime_authority is invalid"
            ));
        }
        if runtime == "retirement-pending" {
            retirement_pending.push(name);
        }
    }
    if names.is_empty() {
        return Err(format!("{qualified} must project at least one repository"));
    }
    match symlink_policy.as_str() {
        "enforced" if !retirement_pending.is_empty() => {
            return Err(format!(
                "{qualified}.symlink_policy cannot be enforced while a repository is retirement-pending"
            ));
        }
        "retirement-pending" if retirement_pending.is_empty() => {
            return Err(format!(
                "{qualified}.symlink_policy must be enforced when no repository is retirement-pending"
            ));
        }
        _ => {}
    }

    if identity == "jeryu-split" {
        let expected_container = split_root.join("jeryu-split");
        if container != expected_container {
            return Err(format!(
                "{qualified}.container_path must be {}",
                expected_container.display()
            ));
        }
        if owner != "jeryu" || lineage != "v5" || control_name != "jeryu-release-ops" {
            return Err(format!(
                "{qualified} must preserve the jeryu owner, v5 lineage, and jeryu-release-ops control plane"
            ));
        }
        if symlink_policy == "retirement-pending" && retirement_pending.as_slice() != ["jeryu-web"]
        {
            return Err(format!(
                "{qualified} retirement-pending gate must be owned only by jeryu-web"
            ));
        }
    }
    Ok(())
}

fn compare_registered_nested_family_child(
    key: &str,
    outer: &toml::Value,
    child: &toml::Value,
) -> Result<(), String> {
    let qualified = format!("nested_families.{key}");
    let family = string(outer, "family").unwrap_or_default();
    for (field, child_field) in [
        ("family", "repo_family"),
        ("release_identity", "release_identity"),
        ("release_lineage", "release_lineage"),
    ] {
        if string(outer, field) != string(child, child_field) {
            return Err(format!(
                "{qualified}.{field} differs from child {child_field}"
            ));
        }
    }
    let child_root = string(child, "split_root").unwrap_or_default();
    if string(outer, "container_path").as_deref() != Some(child_root.as_str()) {
        return Err(format!(
            "{qualified}.container_path differs from child split_root"
        ));
    }
    if string(child, "manifest_authority") != string(outer, "manifest_path") {
        return Err(format!(
            "{qualified}.manifest_path differs from child manifest_authority"
        ));
    }
    let child_control = child
        .get("control_plane")
        .ok_or_else(|| format!("{qualified} child is missing control_plane"))?;
    for (outer_field, child_field) in [
        ("control_plane_name", "name"),
        ("control_plane", "path"),
        ("control_plane_remote", "remote"),
        ("control_plane_required_check", "required_check"),
        ("control_plane_current_tag", "current_tag"),
        ("control_plane_inventory_status", "inventory_status"),
        ("control_plane_runtime_authority", "runtime_authority"),
    ] {
        if string(outer, outer_field) != string(child_control, child_field) {
            return Err(format!(
                "{qualified}.{outer_field} differs from child control_plane.{child_field}"
            ));
        }
    }
    let child_redline = child
        .get("nested_families")
        .and_then(|nested| nested.get("redline"))
        .ok_or_else(|| format!("{qualified} child is missing canonical Redline authority"))?;
    if string(child_redline, "container_path") != string(outer, "redline_authority")
        || string(child_redline, "source_authority").as_deref() != Some("jain-redline")
    {
        return Err(format!(
            "{qualified} child must resolve Redline from the canonical Jain authority"
        ));
    }

    let projected = registered_nested_projection(outer)
        .map(|repo| (string(repo, "name").unwrap_or_default(), repo))
        .collect::<std::collections::BTreeMap<_, _>>();
    let child_repos = child
        .get("repo")
        .and_then(toml::Value::as_array)
        .into_iter()
        .flatten()
        .map(|repo| (string(repo, "name").unwrap_or_default(), repo))
        .collect::<std::collections::BTreeMap<_, _>>();
    if projected.keys().collect::<Vec<_>>() != child_repos.keys().collect::<Vec<_>>() {
        return Err(format!(
            "{qualified}.repository names differ from child {family} authority"
        ));
    }
    for (name, projection) in projected {
        let child_repo = child_repos[&name];
        for field in [
            "path",
            "jeryu_slug",
            "remote",
            "required_check",
            "default_branch",
            "current_tag",
            "inventory_status",
            "runtime_authority",
        ] {
            if projection.get(field) != child_repo.get(field) {
                return Err(format!(
                    "{qualified}.repository[{name}].{field} differs from child authority"
                ));
            }
        }
    }
    Ok(())
}

fn first_symlink(root: &Path) -> Result<Option<PathBuf>, String> {
    for entry in fs::read_dir(root)
        .map_err(|error| format!("cannot scan {} for symlinks: {error}", root.display()))?
    {
        let entry = entry.map_err(|error| {
            format!(
                "cannot inspect directory entry beneath {}: {error}",
                root.display()
            )
        })?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)
            .map_err(|error| format!("cannot inspect {}: {error}", path.display()))?;
        if metadata.file_type().is_symlink() {
            return Ok(Some(path));
        }
        if metadata.is_dir() {
            if let Some(found) = first_symlink(&path)? {
                return Ok(Some(found));
            }
        }
    }
    Ok(None)
}

fn validate_registered_nested_family_local(
    key: &str,
    family: &toml::Value,
    split_root: &Path,
    check_child_authority: bool,
) -> Result<(), String> {
    validate_registered_nested_family_declaration(key, family, split_root)?;
    let qualified = format!("nested_families.{key}");
    let container = PathBuf::from(string(family, "container_path").unwrap());
    let control = PathBuf::from(string(family, "control_plane").unwrap());
    let manifest = PathBuf::from(string(family, "manifest_path").unwrap());
    let container_metadata = physical_directory(&container, "registered family container")?;
    let control_metadata = physical_directory(&control, "registered family control plane")?;
    if check_child_authority {
        physical_regular_file(&manifest, "registered family manifest")?;
    }
    let manifest_parent = physical_directory(
        manifest
            .parent()
            .ok_or_else(|| format!("{qualified}.manifest_path has no parent"))?,
        "registered family manifest parent",
    )?;
    if physical_identity(&manifest_parent) != physical_identity(&control_metadata) {
        return Err(format!(
            "{qualified}.manifest_path is not physically inside the declared control plane"
        ));
    }
    physical_directory(
        &control.join(".git"),
        "registered family control-plane Git directory",
    )?;
    for old in [
        Path::new("/home/ubuntu/jeryu-split"),
        Path::new("/home/ubuntu/jain-split/jeryu"),
    ] {
        if old.exists() {
            return Err(format!(
                "forbidden legacy Jeryu source root exists: {}",
                old.display()
            ));
        }
    }
    let duplicate_redline_exists = container.join("jeryu-redline").exists();
    match (
        string(family, "redline_source_policy").as_deref(),
        duplicate_redline_exists,
    ) {
        (Some("enforced"), true) => {
            return Err(format!(
                "duplicate Jeryu Redline source container exists beneath {}",
                container.display()
            ));
        }
        (Some("retirement-pending"), false) => {
            return Err(format!(
                "{qualified}.redline_source_policy must be enforced after duplicate Redline retirement"
            ));
        }
        _ => {}
    }
    let container_identity = physical_identity(&container_metadata);
    let mut physical_repositories = std::collections::BTreeMap::from([(
        physical_identity(&control_metadata),
        string(family, "control_plane_name").unwrap_or_else(|| "control-plane".to_owned()),
    )]);
    for repository in registered_nested_projection(family) {
        let name = string(repository, "name").unwrap();
        let path = PathBuf::from(string(repository, "path").unwrap());
        let metadata = physical_directory(&path, "registered nested repository")?;
        let parent = physical_directory(
            path.parent()
                .ok_or_else(|| format!("{name}: repository path has no parent"))?,
            "registered nested repository parent",
        )?;
        if physical_identity(&parent) != container_identity {
            return Err(format!(
                "{name}: repository is not a direct physical child of {}",
                container.display()
            ));
        }
        physical_directory(
            &path.join(".git"),
            "registered nested repository Git directory",
        )?;
        if let Some(existing) =
            physical_repositories.insert(physical_identity(&metadata), name.clone())
        {
            return Err(format!(
                "registered family repositories {existing} and {name} share one physical identity"
            ));
        }
    }
    if string(family, "symlink_policy").as_deref() == Some("enforced") {
        if let Some(path) = first_symlink(&container)? {
            return Err(format!(
                "{qualified} contains a forbidden symlink: {}",
                path.display()
            ));
        }
    }
    if check_child_authority {
        let child: toml::Value = fs::read_to_string(&manifest)
            .map_err(|error| format!("cannot read {}: {error}", manifest.display()))?
            .parse()
            .map_err(|error| format!("cannot parse {}: {error}", manifest.display()))?;
        compare_registered_nested_family_child(key, family, &child)?;
    }
    Ok(())
}

fn managed_repositories(
    data: &toml::Value,
    _manifest: &Path,
) -> Result<Vec<ManagedRepo>, Box<dyn std::error::Error>> {
    let mut managed = Vec::new();
    let family = string(data, "repo_family").unwrap_or_else(|| "jain-split".to_owned());
    for raw in manifest_repos(data)? {
        let repo = repo_from(raw)?;
        let infrastructure =
            raw.get("kind").and_then(toml::Value::as_str) == Some("required-infrastructure");
        managed.push(ManagedRepo {
            name: repo.name,
            path: repo.path,
            remote: declared_remote(raw).ok_or("managed repository is missing its remote")?,
            required_check: string(raw, "required_check")
                .ok_or("managed repository is missing its required check")?,
            branch: string(raw, "default_branch").unwrap_or_else(|| "main".to_owned()),
            tag: declared_release_tag(raw),
            kind: if infrastructure {
                "required-infrastructure".to_owned()
            } else {
                "family".to_owned()
            },
            family: family.clone(),
            family_registered: if infrastructure {
                raw.get("family_registered")
                    .and_then(toml::Value::as_bool)
                    .unwrap_or(false)
            } else {
                true
            },
            inventory_status: inventory_status(raw),
            runtime_authority: runtime_authority(
                raw,
                if infrastructure {
                    "infrastructure"
                } else {
                    "product"
                },
            ),
        });
    }

    let control = data
        .get("control_plane")
        .ok_or("manifest is missing its control_plane")?;
    let control_name = string(control, "name").ok_or("control plane is missing its name")?;
    managed.push(ManagedRepo {
        name: control_name.clone(),
        path: PathBuf::from(string(control, "path").ok_or("control plane is missing its path")?),
        remote: declared_remote(control).ok_or("control plane is missing its remote")?,
        required_check: string(control, "required_check")
            .ok_or("control plane is missing its required check")?,
        branch: string(control, "branch").unwrap_or_else(|| "main".to_owned()),
        tag: declared_release_tag(control),
        kind: "control-plane".to_owned(),
        family: family.clone(),
        family_registered: true,
        inventory_status: inventory_status(control),
        runtime_authority: runtime_authority(control, "control-plane"),
    });

    if data.get("nested_families").is_some() {
        let topology = nested_engine_topology(data)?;
        validate_nested_engine_topology_paths(&topology)?;
        let nested_path = &topology.manifest_path;
        let nested: toml::Value = fs::read_to_string(nested_path)?.parse()?;
        validate_child_family_authority(&topology, &nested)?;
        let nested_paths = validated_nested_repository_paths(&topology, &nested)?;
        let nested_family = string(&nested, "family").ok_or("nested manifest is missing family")?;
        for raw in nested
            .get("repo")
            .and_then(toml::Value::as_array)
            .into_iter()
            .flatten()
        {
            let name = string(raw, "name").ok_or("nested repository is missing its name")?;
            let path = nested_paths
                .get(&name)
                .cloned()
                .ok_or_else(|| format!("nested repository {name} has no validated path"))?;
            managed.push(ManagedRepo {
                name,
                path,
                remote: declared_remote(raw).ok_or("nested repository is missing its remote")?,
                required_check: string(raw, "required_check")
                    .ok_or("nested repository is missing its required check")?,
                branch: string(raw, "default_branch").unwrap_or_else(|| "main".to_owned()),
                tag: declared_release_tag(raw),
                kind: "nested-family".to_owned(),
                family: nested_family.clone(),
                family_registered: true,
                inventory_status: inventory_status(raw),
                runtime_authority: runtime_authority(raw, "nested-library"),
            });
        }
        for pending in &topology.pending_repositories {
            let path = nested_paths.get(&pending.name).cloned().ok_or_else(|| {
                format!(
                    "pending nested repository {} has no validated path",
                    pending.name
                )
            })?;
            managed.push(ManagedRepo {
                name: pending.name.clone(),
                path,
                remote: pending.remote.clone(),
                required_check: pending.required_check.clone(),
                branch: "main".to_owned(),
                tag: None,
                kind: "nested-family".to_owned(),
                family: nested_family.clone(),
                family_registered: true,
                inventory_status: "active".to_owned(),
                runtime_authority: "pending-nested-library".to_owned(),
            });
        }
        let nested_control = nested
            .get("control_plane")
            .ok_or("nested manifest is missing its control_plane")?;
        let nested_control_name =
            string(nested_control, "name").ok_or("nested control plane is missing its name")?;
        managed.push(ManagedRepo {
            name: nested_control_name,
            path: topology.control_plane_path,
            remote: declared_remote(nested_control)
                .ok_or("nested control plane is missing its remote")?,
            required_check: string(nested_control, "required_check")
                .ok_or("nested control plane is missing its required check")?,
            branch: string(nested_control, "branch").unwrap_or_else(|| "main".to_owned()),
            tag: declared_release_tag(nested_control),
            kind: "nested-control-plane".to_owned(),
            family: nested_family,
            family_registered: true,
            inventory_status: inventory_status(nested_control),
            runtime_authority: runtime_authority(nested_control, "control-plane"),
        });
    }

    let split_root = exact_absolute_path(
        &string(data, "split_root").ok_or("manifest is missing split_root")?,
        "split_root",
    )?;
    for (key, registration) in registered_nested_families(data) {
        validate_registered_nested_family_declaration(key, registration, &split_root)?;
        let nested_family = string(registration, "family")
            .ok_or_else(|| format!("nested family {key} is missing family"))?;
        for raw in registered_nested_projection(registration) {
            let name = string(raw, "name")
                .ok_or_else(|| format!("nested family {key} repository is missing name"))?;
            managed.push(ManagedRepo {
                name,
                path: PathBuf::from(
                    string(raw, "path").ok_or("nested repository is missing its path")?,
                ),
                remote: declared_remote(raw).ok_or("nested repository is missing its remote")?,
                required_check: string(raw, "required_check")
                    .ok_or("nested repository is missing its required check")?,
                branch: string(raw, "default_branch").unwrap_or_else(|| "main".to_owned()),
                tag: declared_release_tag(raw),
                kind: "nested-family".to_owned(),
                family: nested_family.clone(),
                family_registered: true,
                inventory_status: inventory_status(raw),
                runtime_authority: runtime_authority(raw, "nested-library"),
            });
        }
        managed.push(ManagedRepo {
            name: string(registration, "control_plane_name")
                .ok_or("nested registration is missing its control-plane name")?,
            path: PathBuf::from(
                string(registration, "control_plane")
                    .ok_or("nested registration is missing its control-plane path")?,
            ),
            remote: string(registration, "control_plane_remote")
                .ok_or("nested registration is missing its control-plane remote")?,
            required_check: string(registration, "control_plane_required_check")
                .ok_or("nested registration is missing its control-plane required check")?,
            branch: "main".to_owned(),
            tag: string(registration, "control_plane_current_tag"),
            kind: "nested-control-plane".to_owned(),
            family: nested_family,
            family_registered: true,
            inventory_status: string(registration, "control_plane_inventory_status")
                .unwrap_or_else(|| "active".to_owned()),
            runtime_authority: string(registration, "control_plane_runtime_authority")
                .unwrap_or_else(|| "control-plane".to_owned()),
        });
    }

    let mut names = std::collections::BTreeSet::new();
    let mut paths = std::collections::BTreeSet::new();
    let mut remotes = std::collections::BTreeSet::new();
    for repo in &managed {
        if !names.insert(repo.name.clone()) {
            return Err(format!("duplicate managed repository name: {}", repo.name).into());
        }
        if !paths.insert(repo.path.clone()) {
            return Err(
                format!("duplicate managed repository path: {}", repo.path.display()).into(),
            );
        }
        if !remotes.insert(repo.remote.clone()) {
            return Err(format!("duplicate managed repository remote: {}", repo.remote).into());
        }
    }
    Ok(managed)
}

fn optional_typed_string(
    value: &toml::Value,
    key: &str,
    qualified_key: &str,
) -> Result<Option<String>, String> {
    match value.get(key) {
        None => Ok(None),
        Some(raw) => raw
            .as_str()
            .map(|text| Some(text.to_owned()))
            .ok_or_else(|| format!("{qualified_key} must be a string when present")),
    }
}

fn optional_typed_integer(
    value: &toml::Value,
    key: &str,
    qualified_key: &str,
) -> Result<Option<i64>, String> {
    match value.get(key) {
        None => Ok(None),
        Some(raw) => raw
            .as_integer()
            .map(Some)
            .ok_or_else(|| format!("{qualified_key} must be an integer when present")),
    }
}

fn exact_absolute_path(raw: &str, qualified_key: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(raw);
    if !path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                std::path::Component::CurDir | std::path::Component::ParentDir
            )
        })
        || path.components().collect::<PathBuf>().as_os_str() != OsStr::new(raw)
    {
        return Err(format!(
            "{qualified_key} must use exact normalized absolute spelling"
        ));
    }
    Ok(path)
}

fn valid_bound_engine_tag(
    repository: &str,
    tag: &str,
    product_version: &str,
    tag_revision: i64,
) -> bool {
    if tag_revision < 0
        || product_version.split('.').count() < 3
        || product_version
            .split('.')
            .any(|part| part.is_empty() || !part.bytes().all(|byte| byte.is_ascii_digit()))
    {
        return false;
    }
    let prefix = format!("{repository}-v{product_version}-");
    let Some(suffix) = tag.strip_prefix(&prefix) else {
        return false;
    };
    let Some((series, revision)) = suffix.rsplit_once('.') else {
        return false;
    };
    !series.is_empty()
        && series
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        && revision == tag_revision.to_string()
}

fn validate_managed_release_identity(
    value: &toml::Value,
    qualified_key: &str,
    repository: &str,
    tag_key: &str,
) -> Result<(), String> {
    let status = optional_typed_string(
        value,
        "identity_status",
        &format!("{qualified_key}.identity_status"),
    )?;
    let tag = optional_typed_string(value, tag_key, &format!("{qualified_key}.{tag_key}"))?;
    let product_version = optional_typed_string(
        value,
        "product_version",
        &format!("{qualified_key}.product_version"),
    )?;
    let tag_revision = optional_typed_integer(
        value,
        "tag_revision",
        &format!("{qualified_key}.tag_revision"),
    )?;
    let release_commit = optional_typed_string(
        value,
        "release_commit",
        &format!("{qualified_key}.release_commit"),
    )?;
    let release_tree = optional_typed_string(
        value,
        "release_tree",
        &format!("{qualified_key}.release_tree"),
    )?;
    let release_checksum = optional_typed_string(
        value,
        "release_checksum_sha256",
        &format!("{qualified_key}.release_checksum_sha256"),
    )?;
    match status.as_deref() {
        Some("pending") => {
            if tag.is_some()
                || product_version.is_some()
                || tag_revision.is_some()
                || release_commit.is_some()
                || release_tree.is_some()
                || release_checksum.is_some()
            {
                return Err(format!(
                    "pending {qualified_key} identity must omit every bound-only identity field"
                ));
            }
        }
        None | Some("bound") => {
            let tag = tag.ok_or_else(|| format!("bound {qualified_key} must declare {tag_key}"))?;
            let product_version = product_version
                .ok_or_else(|| format!("bound {qualified_key} must declare product_version"))?;
            if product_version != RELEASE_VERSION {
                return Err(format!(
                    "bound {qualified_key}.product_version must be {RELEASE_VERSION}"
                ));
            }
            let tag_revision = tag_revision
                .filter(|revision| *revision >= 0)
                .ok_or_else(|| format!("bound {qualified_key}.tag_revision must be nonnegative"))?;
            let expected_tag = format!("{repository}-v{RELEASE_VERSION}-split.{tag_revision}");
            if tag != expected_tag {
                return Err(format!(
                    "bound {qualified_key}.{tag_key} must be {expected_tag}"
                ));
            }
            for (field, candidate, length) in [
                ("release_commit", release_commit, 40),
                ("release_tree", release_tree, 40),
                ("release_checksum_sha256", release_checksum, 64),
            ] {
                if !candidate
                    .as_deref()
                    .is_some_and(|candidate| is_full_hex(candidate, length))
                {
                    return Err(format!(
                        "bound {qualified_key}.{field} must be {length} lowercase hex"
                    ));
                }
            }
        }
        Some(other) => {
            return Err(format!(
                "{qualified_key}.identity_status must be pending or bound, found {other}"
            ))
        }
    }
    Ok(())
}

fn derived_manifest_identity_status(
    data: &toml::Value,
    target: &str,
) -> Result<Option<String>, String> {
    match data
        .get("derived_manifests")
        .and_then(|value| value.get(target))
    {
        Some(value) => optional_typed_string(
            value,
            "identity_status",
            &format!("derived_manifests.{target}.identity_status"),
        ),
        None => Ok(None),
    }
}

fn derived_manifest_is_pending(data: &toml::Value, target: &str) -> Result<bool, String> {
    match derived_manifest_identity_status(data, target)?.as_deref() {
        Some("pending") => Ok(true),
        None | Some("bound") => Ok(false),
        Some(other) => Err(format!(
            "derived_manifests.{target}.identity_status must be pending or bound, found {other}"
        )),
    }
}

fn nested_engine_topology(data: &toml::Value) -> Result<NestedEngineTopology, String> {
    let split_root_raw = string(data, "split_root")
        .ok_or_else(|| "split_root is required for nested-family topology".to_owned())?;
    let split_root = exact_absolute_path(&split_root_raw, "split_root")?;
    let nested_families = data
        .get("nested_families")
        .and_then(toml::Value::as_table)
        .ok_or_else(|| "manifest must declare nested_families".to_owned())?;
    let mut engine_families = nested_families
        .iter()
        .filter(|(_, nested)| nested.get("engine_repository").is_some());
    let (dependency_name, nested) = engine_families
        .next()
        .ok_or_else(|| "manifest must declare one required nested engine family".to_owned())?;
    if engine_families.next().is_some() {
        return Err("manifest must declare exactly one required nested engine family".to_owned());
    }
    let nested_key = format!("nested_families.{dependency_name}");
    if nested.get("required").and_then(toml::Value::as_bool) != Some(true) {
        return Err(format!("{nested_key}.required must be true"));
    }
    let family = string(nested, "family")
        .filter(|family| valid_cargo_token(family))
        .ok_or_else(|| format!("{nested_key}.family must be one unaliased family token"))?;
    let authority_mode = optional_typed_string(
        nested,
        "authority_mode",
        &format!("{nested_key}.authority_mode"),
    )?;
    if authority_mode
        .as_deref()
        .is_some_and(|mode| mode != "child")
    {
        return Err(format!(
            "{nested_key}.authority_mode must be child when present"
        ));
    }
    let manifest_path = exact_absolute_path(
        &string(nested, "manifest_path")
            .ok_or_else(|| format!("{nested_key}.manifest_path is required"))?,
        &format!("{nested_key}.manifest_path"),
    )?;
    let container_path = exact_absolute_path(
        &string(nested, "container_path")
            .ok_or_else(|| format!("{nested_key}.container_path is required"))?,
        &format!("{nested_key}.container_path"),
    )?;
    let control_plane_path = exact_absolute_path(
        &string(nested, "control_plane")
            .ok_or_else(|| format!("{nested_key}.control_plane is required"))?,
        &format!("{nested_key}.control_plane"),
    )?;
    for (key, path) in [
        ("manifest_path", &manifest_path),
        ("container_path", &container_path),
        ("control_plane", &control_plane_path),
    ] {
        if path == &split_root || !path.starts_with(&split_root) {
            return Err(format!("{nested_key}.{key} must be beneath split_root"));
        }
    }
    let engine_repository = string(nested, "engine_repository")
        .filter(|repository| valid_cargo_token(repository))
        .ok_or_else(|| {
            format!("{nested_key}.engine_repository must be one unaliased path component")
        })?;
    let engine_remote = string(nested, "engine_remote")
        .filter(|remote| {
            remote.starts_with(NESTED_REDLINE_REMOTE_PREFIX) && remote.ends_with(".git")
        })
        .ok_or_else(|| format!("{nested_key}.engine_remote must be a local Jeryu Git remote"))?;
    let expected_remote = format!("{NESTED_REDLINE_REMOTE_PREFIX}{engine_repository}.git");
    if engine_remote != expected_remote {
        return Err(format!(
            "{nested_key}.engine_remote must be {expected_remote}"
        ));
    }
    let external = data
        .get("external_dependencies")
        .and_then(|value| value.get(dependency_name))
        .ok_or_else(|| format!("manifest must declare external_dependencies.{dependency_name}"))?;
    let external_key = format!("external_dependencies.{dependency_name}");
    if string(external, "repository").as_deref() != Some(engine_repository.as_str()) {
        return Err(format!(
            "{external_key}.repository must match {nested_key}.engine_repository"
        ));
    }
    if string(external, "remote").as_deref() != Some(engine_remote.as_str()) {
        return Err(format!(
            "{external_key}.remote must match {nested_key}.engine_remote"
        ));
    }
    let engine_required_check = optional_typed_string(
        external,
        "required_check",
        &format!("{external_key}.required_check"),
    )?
    .unwrap_or_else(|| format!("{engine_repository}/required"));
    if engine_required_check != format!("{engine_repository}/required") {
        return Err(format!(
            "{external_key}.required_check must be {engine_repository}/required when present"
        ));
    }
    let external_status = optional_typed_string(
        external,
        "identity_status",
        &format!("{external_key}.identity_status"),
    )?;
    let engine_status = optional_typed_string(
        nested,
        "engine_identity_status",
        &format!("{nested_key}.engine_identity_status"),
    )?;
    let external_tag = optional_typed_string(
        external,
        "immutable_tag",
        &format!("{external_key}.immutable_tag"),
    )?;
    let engine_tag =
        optional_typed_string(nested, "engine_tag", &format!("{nested_key}.engine_tag"))?;
    let product_version = optional_typed_string(
        external,
        "product_version",
        &format!("{external_key}.product_version"),
    )?;
    let tag_revision = optional_typed_integer(
        external,
        "tag_revision",
        &format!("{external_key}.tag_revision"),
    )?;
    let release_commit = optional_typed_string(
        external,
        "release_commit",
        &format!("{external_key}.release_commit"),
    )?;
    let release_tree = optional_typed_string(
        external,
        "release_tree",
        &format!("{external_key}.release_tree"),
    )?;
    let release_checksum_sha256 = optional_typed_string(
        external,
        "release_checksum_sha256",
        &format!("{external_key}.release_checksum_sha256"),
    )?;
    let engine_release_tree = optional_typed_string(
        nested,
        "engine_release_tree",
        &format!("{nested_key}.engine_release_tree"),
    )?;
    let mut bound_identity = None;
    match (external_status.as_deref(), engine_status.as_deref()) {
        (Some("pending"), Some("pending")) => {
            if external_tag.is_some()
                || engine_tag.is_some()
                || product_version.is_some()
                || tag_revision.is_some()
                || release_commit.is_some()
                || release_tree.is_some()
                || release_checksum_sha256.is_some()
                || [
                    "engine_product_version",
                    "engine_tag_revision",
                    "engine_release_commit",
                    "engine_release_tree",
                    "engine_release_checksum_sha256",
                ]
                .iter()
                .any(|key| nested.get(*key).is_some())
            {
                return Err(format!(
                    "pending {dependency_name} identity must omit every bound-only identity field"
                ));
            }
        }
        (None, None) | (Some("bound"), Some("bound")) => {
            let tag = external_tag.ok_or_else(|| {
                format!("bound {external_key} must declare immutable_tag")
            })?;
            if engine_tag.as_deref() != Some(tag.as_str()) {
                return Err(format!(
                    "{nested_key}.engine_tag must match {external_key}.immutable_tag"
                ));
            }
            let product_version = product_version.ok_or_else(|| {
                format!("bound {external_key} must declare product_version")
            })?;
            let tag_revision = tag_revision
                .ok_or_else(|| format!("bound {external_key} must declare tag_revision"))?;
            let release_commit = release_commit
                .filter(|value| is_full_hex(value, 40))
                .ok_or_else(|| {
                    format!("bound {external_key}.release_commit must be 40 lowercase hex")
                })?;
            let release_tree = release_tree
                .filter(|value| is_full_hex(value, 40))
                .ok_or_else(|| {
                    format!("bound {external_key}.release_tree must be 40 lowercase hex")
                })?;
            if engine_release_tree.as_deref() != Some(release_tree.as_str()) {
                return Err(format!(
                    "{nested_key}.engine_release_tree must match {external_key}.release_tree"
                ));
            }
            let release_checksum_sha256 = release_checksum_sha256
                .filter(|value| is_full_hex(value, 64))
                .ok_or_else(|| {
                    format!(
                        "bound {external_key}.release_checksum_sha256 must be 64 lowercase hex"
                    )
                })?;
            if !valid_bound_engine_tag(
                &engine_repository,
                &tag,
                &product_version,
                tag_revision,
            ) {
                return Err(format!(
                    "{external_key}.immutable_tag must match repository, product_version, and tag_revision"
                ));
            }
            bound_identity = Some(BoundEngineIdentity {
                tag,
                product_version,
                tag_revision,
                release_commit,
                release_tree,
                release_checksum_sha256,
            });
        }
        _ => {
            return Err(format!(
                "{dependency_name} identity statuses must be paired pending, paired bound, or both absent for bound"
            ))
        }
    }
    let mut pending_repositories = Vec::new();
    let mut pending_names = std::collections::BTreeSet::new();
    for (index, raw) in nested
        .get("pending_repository")
        .and_then(toml::Value::as_array)
        .into_iter()
        .flatten()
        .enumerate()
    {
        let row_key = format!("{nested_key}.pending_repository[{index}]");
        let name = optional_typed_string(raw, "name", &format!("{row_key}.name"))?
            .filter(|name| valid_cargo_token(name))
            .ok_or_else(|| format!("{row_key}.name must be one unaliased repository token"))?;
        if !pending_names.insert(name.clone()) {
            return Err(format!("duplicate pending nested repository: {name}"));
        }
        if name == engine_repository {
            return Err(format!(
                "{row_key}.name duplicates the separately governed engine repository"
            ));
        }
        if optional_typed_string(
            raw,
            "identity_status",
            &format!("{row_key}.identity_status"),
        )?
        .as_deref()
            != Some("pending")
        {
            return Err(format!("{row_key}.identity_status must be pending"));
        }
        if [
            "path",
            "immutable_tag",
            "current_tag",
            "product_version",
            "tag_revision",
            "release_commit",
            "release_tree",
            "release_checksum_sha256",
        ]
        .iter()
        .any(|field| raw.get(*field).is_some())
        {
            return Err(format!(
                "{row_key} must omit path and every bound-only identity field"
            ));
        }
        let remote = optional_typed_string(raw, "remote", &format!("{row_key}.remote"))?
            .ok_or_else(|| format!("{row_key}.remote is required"))?;
        let expected_remote = format!("{NESTED_REDLINE_REMOTE_PREFIX}{name}.git");
        if remote != expected_remote {
            return Err(format!("{row_key}.remote must be {expected_remote}"));
        }
        let required_check =
            optional_typed_string(raw, "required_check", &format!("{row_key}.required_check"))?
                .ok_or_else(|| format!("{row_key}.required_check is required"))?;
        if required_check != format!("{name}/required") {
            return Err(format!("{row_key}.required_check must be {name}/required"));
        }
        release_feature_matrix(raw).map_err(|error| format!("{row_key}: {error}"))?;
        pending_repositories.push(PendingNestedRepository {
            name,
            remote,
            required_check,
        });
    }
    Ok(NestedEngineTopology {
        dependency_name: dependency_name.to_owned(),
        family,
        split_root,
        manifest_path,
        container_path,
        control_plane_path,
        engine_repository,
        engine_remote,
        engine_required_check,
        pending_repositories,
        bound_identity,
    })
}

fn physical_metadata(path: &Path, kind: &str) -> Result<fs::Metadata, String> {
    if !path.is_absolute() {
        return Err(format!(
            "{kind} must be an absolute physical path: {}",
            path.display()
        ));
    }
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component.as_os_str());
        let metadata = fs::symlink_metadata(&current)
            .map_err(|error| format!("{kind} is missing at {}: {error}", current.display()))?;
        if metadata.file_type().is_symlink() {
            return Err(format!(
                "{kind} contains a symlink component: {}",
                current.display()
            ));
        }
    }
    fs::metadata(path).map_err(|error| format!("cannot inspect {kind} {}: {error}", path.display()))
}

fn physical_directory(path: &Path, kind: &str) -> Result<fs::Metadata, String> {
    let metadata = physical_metadata(path, kind)?;
    if !metadata.is_dir() {
        return Err(format!("{kind} is not a directory: {}", path.display()));
    }
    Ok(metadata)
}

fn physical_regular_file(path: &Path, kind: &str) -> Result<fs::Metadata, String> {
    let metadata = physical_metadata(path, kind)?;
    if !metadata.is_file() {
        return Err(format!("{kind} is not a regular file: {}", path.display()));
    }
    Ok(metadata)
}

fn physical_identity(metadata: &fs::Metadata) -> (u64, u64) {
    (metadata.dev(), metadata.ino())
}

fn exact_relative_path(from: &Path, to: &Path) -> Result<String, String> {
    if !from.is_absolute() || !to.is_absolute() {
        return Err("relative path endpoints must be absolute".to_owned());
    }
    let from_components = from.components().collect::<Vec<_>>();
    let to_components = to.components().collect::<Vec<_>>();
    let common = from_components
        .iter()
        .zip(&to_components)
        .take_while(|(left, right)| left == right)
        .count();
    if common == 0 {
        return Err("relative path endpoints do not share a filesystem root".to_owned());
    }
    let mut relative = PathBuf::new();
    for _ in common..from_components.len() {
        relative.push("..");
    }
    for component in &to_components[common..] {
        match component {
            std::path::Component::Normal(value) => relative.push(value),
            _ => return Err("relative path target is not normalized".to_owned()),
        }
    }
    if relative.as_os_str().is_empty() {
        Ok(".".to_owned())
    } else {
        Ok(relative.display().to_string())
    }
}

fn validate_nested_engine_topology_paths(topology: &NestedEngineTopology) -> Result<(), String> {
    physical_directory(&topology.split_root, "split root")?;
    let control = physical_directory(&topology.control_plane_path, "nested control plane")?;
    physical_directory(&topology.container_path, "nested family container")?;
    physical_regular_file(&topology.manifest_path, "nested family manifest")?;
    let manifest_parent = topology
        .manifest_path
        .parent()
        .ok_or_else(|| "nested family manifest has no parent directory".to_owned())?;
    let manifest_parent = physical_directory(manifest_parent, "nested family manifest parent")?;
    if physical_identity(&manifest_parent) != physical_identity(&control) {
        return Err(format!(
            "nested family manifest parent {} is not the declared physical control plane {}",
            topology
                .manifest_path
                .parent()
                .unwrap_or(Path::new("."))
                .display(),
            topology.control_plane_path.display()
        ));
    }
    Ok(())
}

fn validated_nested_repository_paths(
    topology: &NestedEngineTopology,
    nested: &toml::Value,
) -> Result<std::collections::BTreeMap<String, PathBuf>, String> {
    validate_nested_engine_topology_paths(topology)?;
    let nested_dir = topology
        .manifest_path
        .parent()
        .ok_or_else(|| "nested family manifest has no parent".to_owned())?;
    let control = nested
        .get("control_plane")
        .ok_or_else(|| "nested manifest is missing its control_plane".to_owned())?;
    let control_raw = string(control, "path")
        .ok_or_else(|| "nested control plane is missing its path".to_owned())?;
    let expected_control = exact_relative_path(nested_dir, &topology.control_plane_path)?;
    if control_raw != expected_control {
        return Err(format!(
            "nested control-plane path must be exactly {expected_control}"
        ));
    }
    let nested_control =
        physical_directory(&nested_dir.join(&control_raw), "nested control plane")?;
    let declared_control = physical_directory(
        &topology.control_plane_path,
        "declared nested control plane",
    )?;
    if physical_identity(&nested_control) != physical_identity(&declared_control) {
        return Err(format!(
            "nested control plane path {control_raw} is not the declared physical path {}",
            topology.control_plane_path.display()
        ));
    }
    physical_directory(
        &topology.control_plane_path.join(".git"),
        "nested control-plane Git directory",
    )?;

    let container = physical_directory(&topology.container_path, "nested family container")?;
    let container_identity = physical_identity(&container);
    if let Some(container_raw) = string(nested, "container") {
        let expected = exact_relative_path(nested_dir, &topology.container_path)?;
        if container_raw != expected {
            return Err(format!(
                "nested family container path must be exactly {expected}"
            ));
        }
        let nested_container =
            physical_directory(&nested_dir.join(&container_raw), "nested family container")?;
        if physical_identity(&nested_container) != container_identity {
            return Err(format!(
                "nested container path {container_raw} is not the declared physical path {}",
                topology.container_path.display()
            ));
        }
    }
    if let Some(authority) = string(nested, "manifest_authority") {
        let absolute_authority = topology.manifest_path.display().to_string();
        let relative_authority = exact_relative_path(nested_dir, &topology.manifest_path)?;
        if authority != absolute_authority && authority != relative_authority {
            return Err(format!(
                "nested manifest_authority must be exactly {absolute_authority} or {relative_authority}"
            ));
        }
        let declared_authority = if Path::new(&authority).is_absolute() {
            PathBuf::from(&authority)
        } else {
            nested_dir.join(&authority)
        };
        let declared_metadata =
            physical_regular_file(&declared_authority, "nested manifest authority")?;
        let topology_metadata = physical_regular_file(
            &topology.manifest_path,
            "declared nested manifest authority",
        )?;
        if physical_identity(&declared_metadata) != physical_identity(&topology_metadata) {
            return Err(format!(
                "nested manifest_authority is not the declared physical manifest {}",
                topology.manifest_path.display()
            ));
        }
    }

    let mut identities = std::collections::BTreeMap::new();
    let control_parent = topology
        .control_plane_path
        .parent()
        .ok_or_else(|| "nested control plane has no parent".to_owned())?;
    let control_parent = physical_directory(control_parent, "nested control-plane parent")?;
    if physical_identity(&control_parent) == container_identity {
        identities.insert(
            physical_identity(&declared_control),
            string(control, "name").unwrap_or_else(|| "nested-control-plane".to_owned()),
        );
    }
    let mut paths = std::collections::BTreeMap::new();
    for raw in nested
        .get("repo")
        .and_then(toml::Value::as_array)
        .into_iter()
        .flatten()
    {
        let name = string(raw, "name")
            .ok_or_else(|| "nested repository is missing its name".to_owned())?;
        if !valid_cargo_token(&name) {
            return Err(format!(
                "nested repository name must be one unaliased path component: {name}"
            ));
        }
        let raw_path = string(raw, "path")
            .ok_or_else(|| format!("nested repository {name} is missing its path"))?;
        let expected = exact_relative_path(nested_dir, &topology.container_path.join(&name))?;
        if raw_path != expected {
            return Err(format!(
                "{name}: nested repository path must be exactly {expected}"
            ));
        }
        let resolved = nested_dir.join(&raw_path);
        let metadata = physical_directory(&resolved, "nested family repository")?;
        let parent = resolved
            .parent()
            .ok_or_else(|| format!("{name}: nested repository has no parent"))?;
        let parent = physical_directory(parent, "nested family repository parent")?;
        if physical_identity(&parent) != container_identity {
            return Err(format!(
                "{name}: path {} is not a direct physical child of {}",
                resolved.display(),
                topology.container_path.display()
            ));
        }
        let identity = physical_identity(&metadata);
        if let Some(existing) = identities.insert(identity, name.clone()) {
            return Err(format!(
                "nested family repositories {existing} and {name} share one physical path"
            ));
        }
        physical_directory(
            &resolved.join(".git"),
            "nested family repository Git directory",
        )?;
        let canonical = fs::canonicalize(&resolved).map_err(|error| {
            format!(
                "cannot canonicalize {name} at {}: {error}",
                resolved.display()
            )
        })?;
        let canonical_metadata =
            physical_directory(&canonical, "canonical nested family repository")?;
        if physical_identity(&canonical_metadata) != identity {
            return Err(format!(
                "{name}: repository changed while validating {}",
                resolved.display()
            ));
        }
        if paths.insert(name.clone(), canonical).is_some() {
            return Err(format!("duplicate nested repository name: {name}"));
        }
    }

    for pending in &topology.pending_repositories {
        let name = &pending.name;
        let resolved = topology.container_path.join(name);
        let metadata = physical_directory(&resolved, "pending nested family repository")?;
        let parent = resolved
            .parent()
            .ok_or_else(|| format!("{name}: pending nested repository has no parent"))?;
        let parent = physical_directory(parent, "pending nested family repository parent")?;
        if physical_identity(&parent) != container_identity {
            return Err(format!(
                "{name}: pending path {} is not a direct physical child of {}",
                resolved.display(),
                topology.container_path.display()
            ));
        }
        let identity = physical_identity(&metadata);
        if let Some(existing) = identities.insert(identity, name.clone()) {
            return Err(format!(
                "nested family repositories {existing} and {name} share one physical path"
            ));
        }
        physical_directory(
            &resolved.join(".git"),
            "pending nested family repository Git directory",
        )?;
        let canonical = fs::canonicalize(&resolved).map_err(|error| {
            format!(
                "cannot canonicalize pending {name} at {}: {error}",
                resolved.display()
            )
        })?;
        let canonical_metadata =
            physical_directory(&canonical, "canonical pending nested family repository")?;
        if physical_identity(&canonical_metadata) != identity {
            return Err(format!(
                "{name}: pending repository changed while validating {}",
                resolved.display()
            ));
        }
        if paths.insert(name.clone(), canonical).is_some() {
            return Err(format!("duplicate nested repository name: {name}"));
        }
    }

    Ok(paths)
}

fn declared_nested_lock_path(
    topology: &NestedEngineTopology,
    nested: &toml::Value,
) -> Result<PathBuf, String> {
    let raw = string(nested, "lock")
        .ok_or_else(|| "nested family manifest must declare lock".to_owned())?;
    let relative = Path::new(&raw);
    if raw.is_empty()
        || relative.is_absolute()
        || relative
            .components()
            .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        return Err(format!(
            "nested family lock must be an unaliased relative path: {raw}"
        ));
    }
    Ok(topology.control_plane_path.join(relative))
}

fn validate_child_family_authority(
    topology: &NestedEngineTopology,
    nested: &toml::Value,
) -> Result<(), String> {
    let child_family = string(nested, "family")
        .filter(|family| valid_cargo_token(family))
        .ok_or_else(|| "nested family manifest must declare one family token".to_owned())?;
    if child_family != topology.family {
        return Err(format!(
            "nested family {child_family} must match parent declaration {}",
            topology.family
        ));
    }
    Ok(())
}

fn validate_child_engine_authority(
    topology: &NestedEngineTopology,
    nested: &toml::Value,
) -> Result<(), String> {
    let rows = nested
        .get("repo")
        .and_then(toml::Value::as_array)
        .ok_or_else(|| "nested family manifest must declare repo rows".to_owned())?;
    let matching = rows
        .iter()
        .filter(|row| string(row, "name").as_deref() == Some(topology.engine_repository.as_str()))
        .collect::<Vec<_>>();
    if matching.len() != 1 {
        return Err(format!(
            "nested family must declare exactly one engine repository row named {}",
            topology.engine_repository
        ));
    }
    let engine = matching[0];
    let row_key = format!("repo[{}]", topology.engine_repository);
    if string(engine, "role").as_deref() != Some("canonical-engine") {
        return Err(format!("{row_key}.role must be canonical-engine"));
    }
    if declared_remote(engine).as_deref() != Some(topology.engine_remote.as_str()) {
        return Err(format!(
            "{row_key}.remote must match external dependency remote {}",
            topology.engine_remote
        ));
    }
    let Some(bound) = &topology.bound_identity else {
        return Ok(());
    };
    let child_tag =
        optional_typed_string(engine, "immutable_tag", &format!("{row_key}.immutable_tag"))?
            .or(optional_typed_string(
                engine,
                "current_tag",
                &format!("{row_key}.current_tag"),
            )?)
            .ok_or_else(|| format!("{row_key} must declare immutable_tag or current_tag"))?;
    let child_version = optional_typed_string(
        engine,
        "product_version",
        &format!("{row_key}.product_version"),
    )?
    .ok_or_else(|| format!("{row_key}.product_version is required"))?;
    let child_revision =
        optional_typed_integer(engine, "tag_revision", &format!("{row_key}.tag_revision"))?
            .ok_or_else(|| format!("{row_key}.tag_revision is required"))?;
    let child_commit = optional_typed_string(
        engine,
        "release_commit",
        &format!("{row_key}.release_commit"),
    )?
    .ok_or_else(|| format!("{row_key}.release_commit is required"))?;
    let child_tree =
        optional_typed_string(engine, "release_tree", &format!("{row_key}.release_tree"))?
            .ok_or_else(|| format!("{row_key}.release_tree is required"))?;
    let child_checksum = optional_typed_string(
        engine,
        "release_checksum_sha256",
        &format!("{row_key}.release_checksum_sha256"),
    )?
    .ok_or_else(|| format!("{row_key}.release_checksum_sha256 is required"))?;
    if child_tag != bound.tag
        || child_version != bound.product_version
        || child_revision != bound.tag_revision
        || child_commit != bound.release_commit
        || child_tree != bound.release_tree
        || child_checksum != bound.release_checksum_sha256
    {
        return Err(format!(
            "{row_key} identity must exactly match the bound parent external dependency"
        ));
    }
    Ok(())
}

fn validate_nested_family_local(
    data: &toml::Value,
    skip_remotes: bool,
    errors: &mut Vec<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let topology = match nested_engine_topology(data) {
        Ok(topology) => topology,
        Err(error) => {
            errors.push(error);
            return Ok(());
        }
    };
    if let Err(error) = validate_nested_engine_topology_paths(&topology) {
        errors.push(error);
        return Ok(());
    }
    let nested: toml::Value = fs::read_to_string(&topology.manifest_path)?.parse()?;
    if let Err(error) = validate_child_family_authority(&topology, &nested) {
        errors.push(error);
        return Ok(());
    }
    let nested_paths = match validated_nested_repository_paths(&topology, &nested) {
        Ok(paths) => paths,
        Err(error) => {
            errors.push(error);
            return Ok(());
        }
    };
    if let Err(error) = validate_child_engine_authority(&topology, &nested) {
        errors.push(error);
        return Ok(());
    }
    let control = nested
        .get("control_plane")
        .ok_or("nested manifest is missing its control_plane")?;
    if !skip_remotes {
        let expected = declared_remote(control)
            .ok_or("nested control plane is missing its declared remote")?;
        let remotes = git_remotes(&topology.control_plane_path)?;
        if remotes.len() != 1 || remotes.get("origin") != Some(&vec![expected.clone()]) {
            let control_name =
                string(control, "name").unwrap_or_else(|| "nested-control-plane".to_owned());
            errors.push(format!(
                "{control_name}: nested remotes must contain exactly origin -> {expected}"
            ));
        }
    }

    let mut declared = std::collections::BTreeMap::new();
    let control_parent = topology
        .control_plane_path
        .parent()
        .ok_or("nested control plane has no parent")?;
    let control_parent = physical_directory(control_parent, "nested control-plane parent")?;
    let container = physical_directory(&topology.container_path, "nested family container")?;
    if physical_identity(&control_parent) == physical_identity(&container) {
        let declared_control = physical_directory(
            &topology.control_plane_path,
            "declared nested control plane",
        )?;
        declared.insert(
            physical_identity(&declared_control),
            string(control, "name").unwrap_or_else(|| "nested-control-plane".to_owned()),
        );
    }
    for raw in nested
        .get("repo")
        .and_then(toml::Value::as_array)
        .into_iter()
        .flatten()
    {
        let mut repo = repo_from(raw)?;
        repo.path = nested_paths
            .get(&repo.name)
            .cloned()
            .ok_or_else(|| format!("{} has no validated nested path", repo.name))?;
        let repo_metadata = physical_directory(&repo.path, "nested family repository")?;
        let identity = physical_identity(&repo_metadata);
        if let Some(existing) = declared.insert(identity, repo.name.clone()) {
            errors.push(format!(
                "nested family repositories {existing} and {} share one physical path",
                repo.name
            ));
        }
        if !skip_remotes {
            let expected = declared_remote(raw)
                .ok_or_else(|| format!("{} is missing a declared remote", repo.name))?;
            let remotes = git_remotes(&repo.path)?;
            if remotes.len() != 1 || remotes.get("origin") != Some(&vec![expected.clone()]) {
                errors.push(format!(
                    "{}: nested remotes must contain exactly origin -> {expected}",
                    repo.name
                ));
            }
        }
        check_cargo_sources(&repo, errors)?;
    }
    for pending in &topology.pending_repositories {
        let path = nested_paths
            .get(&pending.name)
            .cloned()
            .ok_or_else(|| format!("{} has no validated pending nested path", pending.name))?;
        let metadata = physical_directory(&path, "pending nested family repository")?;
        if let Some(existing) = declared.insert(physical_identity(&metadata), pending.name.clone())
        {
            errors.push(format!(
                "nested family repositories {existing} and {} share one physical path",
                pending.name
            ));
        }
        if !skip_remotes {
            let remotes = git_remotes(&path)?;
            if remotes.len() != 1 || remotes.get("origin") != Some(&vec![pending.remote.clone()]) {
                errors.push(format!(
                    "{}: pending nested remotes must contain exactly origin -> {}",
                    pending.name, pending.remote
                ));
            }
        }
        check_cargo_sources(
            &Repo {
                name: pending.name.clone(),
                path,
                profile: String::new(),
                authored: false,
                cargo_members: Vec::new(),
                copy_paths: Vec::new(),
                source_paths: Vec::new(),
            },
            errors,
        )?;
    }

    for entry in fs::read_dir(&topology.container_path)? {
        let path = entry?.path();
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                errors.push(format!(
                    "nested family container contains a symlink component: {}",
                    path.display()
                ));
                continue;
            }
            Ok(metadata) if metadata.is_dir() => metadata,
            _ => continue,
        };
        let dot_git = path.join(".git");
        if !dot_git.exists() {
            continue;
        }
        if let Err(error) = physical_directory(&dot_git, "nested child Git directory") {
            errors.push(error);
            continue;
        }
        if !declared.contains_key(&physical_identity(&metadata)) {
            errors.push(format!(
                "undeclared nested-family Git root: {}",
                path.display()
            ));
        }
    }
    Ok(())
}

fn sync_derived_manifests_command(args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    let root = control_plane_root();
    let mut manifest = root.join("repos.manifest.toml");
    let mut receipt = None;
    let mut apply = false;
    let mut selected_targets = BTreeSet::new();
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--manifest" => manifest = PathBuf::from(iter.next().ok_or("--manifest needs a path")?),
            "--receipt" => {
                receipt = Some(PathBuf::from(iter.next().ok_or("--receipt needs a path")?))
            }
            "--target" => {
                let target = iter.next().ok_or("--target needs a name")?;
                if !selected_targets.insert(target.clone()) {
                    return Err(format!("duplicate derived manifest target: {target}").into());
                }
            }
            "--apply" => apply = true,
            value => return Err(format!("unknown sync-derived-manifests argument: {value}").into()),
        }
    }
    let receipt = match receipt {
        Some(path) => path,
        None => release_evidence_path("derived-manifest-sync.json"),
    };
    let mut report = receipt_header(
        "jain.derived-manifest-sync/v1",
        "sync-derived-manifests",
        apply,
    );
    report["manifest"] = json!(manifest);
    let result = (|| {
        let data: toml::Value = fs::read_to_string(&manifest)?.parse()?;
        validate_manifest_data(&data, &manifest, false)?;
        let canonical_hash = manifest_sha256(&manifest)?;
        let mut rows = Vec::new();
        let mut targets = derived_manifest_targets(&data, &manifest)?;
        if !selected_targets.is_empty() {
            let known = targets
                .iter()
                .map(|(target, _)| target.clone())
                .collect::<BTreeSet<_>>();
            if !selected_targets.is_subset(&known) {
                return Err(format!(
                    "unknown derived manifest targets: {:?}",
                    selected_targets.difference(&known).collect::<Vec<_>>()
                )
                .into());
            }
            targets.retain(|(target, _)| selected_targets.contains(target));
        }
        if apply {
            for (target, _) in &targets {
                if derived_manifest_is_pending(&data, target)? {
                    return Err(
                        "cannot apply derived manifests while a consumer identity is pending"
                            .into(),
                    );
                }
            }
        }
        for (target, path) in targets {
            let pending = derived_manifest_is_pending(&data, &target)?;
            let rendered = render_derived_manifest(&data, &manifest, &target, &canonical_hash)?;
            let expected_sha256 = sha256_bytes(rendered.as_bytes());
            let current = fs::read(&path).ok();
            let current_sha256 = current.as_deref().map(sha256_bytes);
            let changed = current.as_deref() != Some(rendered.as_bytes());
            if apply && changed && !pending {
                write_atomic_bytes(&path, rendered.as_bytes())?;
            }
            rows.push(json!({
                "target": target,
                "path": path,
                "identity_status": if pending {"pending"} else {"bound"},
                "changed": changed,
                "current_sha256": current_sha256,
                "expected_sha256": expected_sha256,
                "action": if pending {"pending"} else if apply && changed {"updated"} else if changed {"would-update"} else {"verified"},
            }));
        }
        report["canonical_manifest_sha256"] = json!(canonical_hash);
        report["derived_manifests"] = json!(rows);
        Ok(())
    })();
    finish_receipted_operation(&receipt, &mut report, result)
}

fn derived_manifest_targets(
    data: &toml::Value,
    manifest: &Path,
) -> Result<Vec<(String, PathBuf)>, Box<dyn std::error::Error>> {
    let split_root = PathBuf::from(string(data, "split_root").ok_or("split_root is required")?);
    let defaults = [
        ("portal", split_root.join("jain/repos.manifest.toml")),
        ("deploy", split_root.join("jain-deploy/repos.manifest.toml")),
    ];
    defaults
        .into_iter()
        .map(|(target, default)| {
            let declared = data
                .get("derived_manifests")
                .and_then(|value| value.get(target))
                .and_then(|value| string(value, "path"))
                .map(PathBuf::from);
            let path = match declared {
                Some(path) if path.is_relative() => {
                    manifest.parent().unwrap_or(Path::new(".")).join(path)
                }
                Some(path) => path,
                None => default,
            };
            Ok((target.to_owned(), path))
        })
        .collect()
}

fn render_derived_manifest(
    canonical: &toml::Value,
    manifest: &Path,
    target: &str,
    canonical_hash: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let mut derived = canonical.clone();
    if let Some(subset) = canonical
        .get("derived_manifests")
        .and_then(|value| value.get(target))
        .and_then(|value| value.get("repos"))
        .and_then(toml::Value::as_array)
    {
        let selected = subset
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .map(str::to_owned)
                    .ok_or("derived manifest repo subsets must contain only strings")
            })
            .collect::<Result<std::collections::BTreeSet<_>, _>>()?;
        let known = family_repos(canonical)?
            .iter()
            .filter_map(|repo| string(repo, "name"))
            .collect::<std::collections::BTreeSet<_>>();
        if !selected.is_subset(&known) {
            return Err(format!(
                "derived manifest {target} declares unknown repositories: {:?}",
                selected.difference(&known).collect::<Vec<_>>()
            )
            .into());
        }
        let table = derived
            .as_table_mut()
            .ok_or("canonical manifest is not a table")?;
        let repos = table
            .get_mut("repo")
            .and_then(toml::Value::as_array_mut)
            .ok_or("canonical manifest has no repo array")?;
        repos.retain(|repo| string(repo, "name").is_some_and(|name| selected.contains(&name)));
        table.insert(
            "required_repos".to_owned(),
            toml::Value::Array(selected.iter().cloned().map(toml::Value::String).collect()),
        );
    }
    let authority = string(canonical, "manifest_authority")
        .map(PathBuf::from)
        .unwrap_or_else(|| fs::canonicalize(manifest).unwrap_or_else(|_| manifest.to_path_buf()));
    let table = derived
        .as_table_mut()
        .ok_or("canonical manifest is not a table")?;
    table.insert(
        "canonical_manifest_sha256".to_owned(),
        toml::Value::String(canonical_hash.to_owned()),
    );
    table.insert(
        "manifest_authority".to_owned(),
        toml::Value::String(authority.display().to_string()),
    );
    table.insert(
        "derived_manifest_target".to_owned(),
        toml::Value::String(target.to_owned()),
    );
    let body = toml::to_string_pretty(&derived)?;
    Ok(format!(
        "# GENERATED DERIVED MANIFEST: authority is {}.\n# Do not edit; regenerate with splitctl sync-derived-manifests.\n{body}",
        authority.display()
    ))
}

fn sha256_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn is_full_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn numeric_toml(value: Option<&toml::Value>, field: &str) -> Result<f64, String> {
    value
        .and_then(|value| {
            value
                .as_float()
                .or_else(|| value.as_integer().map(|number| number as f64))
        })
        .ok_or_else(|| format!("governed audit policy is missing numeric {field}"))
}

fn jankurai_evidence_command(args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    let mut repository = None;
    let mut commit = None;
    let mut worktree = None;
    let mut report_root = None;
    let mut report = None;
    let mut auditor = None;
    let mut attempt_id = None;
    let mut lane_conclusion = None;
    let mut lane_failure_reason = None;
    let mut clean_tracked_tree_start = None;
    let mut receipt = None;
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--repository" => repository = Some(iter.next().ok_or("--repository needs a name")?),
            "--commit" => commit = Some(iter.next().ok_or("--commit needs a SHA")?),
            "--worktree" => {
                worktree = Some(PathBuf::from(iter.next().ok_or("--worktree needs a path")?))
            }
            "--report-root" => {
                report_root = Some(PathBuf::from(
                    iter.next().ok_or("--report-root needs a path")?,
                ))
            }
            "--report" => report = Some(PathBuf::from(iter.next().ok_or("--report needs a path")?)),
            "--auditor" => {
                auditor = Some(PathBuf::from(iter.next().ok_or("--auditor needs a path")?))
            }
            "--attempt-id" => attempt_id = Some(iter.next().ok_or("--attempt-id needs a value")?),
            "--lane-conclusion" => {
                lane_conclusion = Some(iter.next().ok_or("--lane-conclusion needs a value")?)
            }
            "--lane-failure-reason" => {
                lane_failure_reason =
                    Some(iter.next().ok_or("--lane-failure-reason needs a value")?)
            }
            "--clean-tracked-tree-start" => {
                clean_tracked_tree_start = Some(
                    match iter
                        .next()
                        .ok_or("--clean-tracked-tree-start needs true or false")?
                        .as_str()
                    {
                        "true" => true,
                        "false" => false,
                        _ => return Err("--clean-tracked-tree-start needs true or false".into()),
                    },
                )
            }
            "--receipt" => {
                receipt = Some(PathBuf::from(iter.next().ok_or("--receipt needs a path")?))
            }
            value => return Err(format!("unknown jankurai-evidence argument: {value}").into()),
        }
    }

    let repository = repository.ok_or("jankurai-evidence requires --repository")?;
    if repository.is_empty()
        || !repository
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        return Err("--repository must be a lowercase repository name".into());
    }
    let commit = commit.ok_or("jankurai-evidence requires --commit")?;
    if !is_full_hex(&commit, 40) {
        return Err("--commit must be a full lowercase 40-character Git SHA".into());
    }
    let worktree = worktree
        .ok_or("jankurai-evidence requires --worktree")?
        .canonicalize()?;
    if worktree.file_name().and_then(|name| name.to_str()) != Some(repository.as_str()) {
        return Err("exact-SHA worktree basename must match --repository".into());
    }
    let report_root = report_root
        .ok_or("jankurai-evidence requires --report-root")?
        .canonicalize()?;
    let report = report
        .ok_or("jankurai-evidence requires --report")?
        .canonicalize()?;
    if report.parent() != Some(report_root.as_path())
        || report.file_name().and_then(|name| name.to_str()) != Some("report.json")
    {
        return Err("Jankurai report must be report.json directly beneath --report-root".into());
    }
    let report_metadata = fs::symlink_metadata(&report)?;
    if !report_metadata.file_type().is_file()
        || std::os::unix::fs::MetadataExt::nlink(&report_metadata) != 1
    {
        return Err("Jankurai report must be a regular single-link file".into());
    }
    let receipt = receipt.ok_or("jankurai-evidence requires --receipt")?;
    if receipt.file_name().and_then(|name| name.to_str()) != Some("receipt.json")
        || receipt
            .parent()
            .ok_or("Jankurai receipt has no parent")?
            .canonicalize()?
            != report_root
    {
        return Err("Jankurai receipt must be receipt.json beneath --report-root".into());
    }
    if receipt.exists() || receipt.is_symlink() {
        return Err("Jankurai receipt destination must not already exist".into());
    }

    let auditor = auditor
        .ok_or("jankurai-evidence requires --auditor")?
        .canonicalize()?;
    let auditor_metadata = fs::symlink_metadata(&auditor)?;
    if !auditor_metadata.file_type().is_file()
        || std::os::unix::fs::MetadataExt::nlink(&auditor_metadata) != 1
    {
        return Err("Jankurai auditor must be a regular single-link file".into());
    }
    let auditor_bytes = fs::read(&auditor)?;
    let auditor_identity = (
        physical_identity(&auditor_metadata),
        auditor_metadata.len(),
        auditor_metadata.mode(),
        auditor_metadata.nlink(),
    );
    let mut busy_retries = 0_u8;
    let auditor_output = loop {
        match Command::new(&auditor).arg("--version").output() {
            Ok(output) => break output,
            Err(error) if error.raw_os_error() == Some(26) => {
                if busy_retries == 3 {
                    return Err(error.into());
                }
                busy_retries += 1;
                let current = fs::symlink_metadata(&auditor)?;
                if (
                    physical_identity(&current),
                    current.len(),
                    current.mode(),
                    current.nlink(),
                ) != auditor_identity
                    || fs::read(&auditor)? != auditor_bytes
                {
                    return Err("Jankurai auditor changed while execution was busy".into());
                }
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            Err(error) => return Err(error.into()),
        }
    };
    let auditor_metadata_after = fs::symlink_metadata(&auditor)?;
    if (
        physical_identity(&auditor_metadata_after),
        auditor_metadata_after.len(),
        auditor_metadata_after.mode(),
        auditor_metadata_after.nlink(),
    ) != auditor_identity
        || fs::read(&auditor)? != auditor_bytes
    {
        return Err("Jankurai auditor changed during version verification".into());
    }
    if !auditor_output.status.success() {
        return Err("Jankurai auditor did not report its version".into());
    }
    let auditor_version = String::from_utf8(auditor_output.stdout)?;
    let auditor_version = auditor_version.trim();
    let auditor_release = auditor_version
        .split_whitespace()
        .last()
        .filter(|value| !value.is_empty())
        .ok_or("Jankurai auditor version is malformed")?;

    let attempt_id = attempt_id.ok_or("jankurai-evidence requires --attempt-id")?;
    if attempt_id.is_empty()
        || !attempt_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err("--attempt-id must be a portable non-empty identifier".into());
    }
    let lane_conclusion = lane_conclusion.ok_or("jankurai-evidence requires --lane-conclusion")?;
    if !matches!(lane_conclusion.as_str(), "success" | "failure") {
        return Err("--lane-conclusion must be success or failure".into());
    }
    if lane_conclusion == "failure" && lane_failure_reason.as_deref().is_none_or(str::is_empty) {
        return Err("failed evidence requires --lane-failure-reason".into());
    }
    let clean_tracked_tree_start =
        clean_tracked_tree_start.ok_or("jankurai-evidence requires --clean-tracked-tree-start")?;
    let checkout_commit = resolve_commit(&worktree, "HEAD")?;
    if checkout_commit != commit {
        return Err(format!(
            "Jankurai worktree HEAD {checkout_commit} does not match requested commit {commit}"
        )
        .into());
    }

    let report_bytes = fs::read(&report)?;
    let score_report: JsonValue = serde_json::from_slice(&report_bytes)?;
    let report_repository = score_report
        .get("repo")
        .and_then(JsonValue::as_str)
        .ok_or("score report is missing repo identity")?;
    if report_repository != "." {
        return Err("score report must identify the exact worktree as repo=.".into());
    }
    let report_head = score_report
        .get("git")
        .and_then(|git| git.get("head"))
        .and_then(JsonValue::as_str)
        .ok_or("score report is missing git.head")?;
    if !(7..=40).contains(&report_head.len())
        || !report_head
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(
            "score report git.head must be a lowercase 7- to 40-character Git identity".into(),
        );
    }
    let report_commit = resolve_commit(&worktree, report_head)?;
    if report_commit != commit {
        return Err(format!(
            "score report git.head resolves to {report_commit}, expected {commit}"
        )
        .into());
    }
    let report_run_id = score_report
        .get("run_id")
        .and_then(JsonValue::as_str)
        .filter(|value| !value.is_empty())
        .ok_or("score report is missing run_id")?;
    let report_auditor_version = score_report
        .get("auditor_version")
        .and_then(JsonValue::as_str)
        .ok_or("score report is missing auditor_version")?;
    let input_fingerprint = score_report
        .get("input_fingerprint")
        .and_then(JsonValue::as_str)
        .ok_or("score report is missing input_fingerprint")?;
    let policy_fingerprint = score_report
        .get("policy_fingerprint")
        .and_then(JsonValue::as_str)
        .ok_or("score report is missing policy_fingerprint")?;
    if !is_sha256_fingerprint(input_fingerprint) || !is_sha256_fingerprint(policy_fingerprint) {
        return Err("score report fingerprints must be non-zero sha256 identities".into());
    }
    let score = score_report
        .get("score")
        .and_then(JsonValue::as_f64)
        .ok_or("score report is missing numeric score")?;
    let hard_value = score_report
        .get("decision")
        .and_then(|value| value.get("hard_findings"))
        .or_else(|| score_report.get("hard_findings"));
    let hard_findings = match hard_value {
        Some(JsonValue::Array(values)) => values.len() as u64,
        Some(value) => value
            .as_u64()
            .ok_or("hard_findings must be an array or integer")?,
        None => return Err("score report is missing hard_findings".into()),
    };
    let caps_value = score_report
        .get("caps_applied")
        .ok_or("score report is missing caps_applied")?;
    let caps_applied = match caps_value {
        JsonValue::Array(values) => values.len() as u64,
        value => value
            .as_u64()
            .ok_or("caps_applied must be an array or integer")?,
    };
    let decision = score_report
        .get("decision")
        .ok_or("score report is missing decision")?;
    let decision_passed = decision.get("passed").and_then(JsonValue::as_bool) == Some(true);
    let reported_minimum_score = decision
        .get("minimum_score")
        .and_then(JsonValue::as_f64)
        .ok_or("score report decision is missing minimum_score")?;
    let ratchet = decision
        .get("ratchet")
        .ok_or("score report decision is missing ratchet")?;
    let reported_ratchet_passed = ratchet.get("passed").and_then(JsonValue::as_bool) == Some(true);
    let reported_baseline_score = ratchet
        .get("baseline_score")
        .and_then(JsonValue::as_f64)
        .ok_or("score report ratchet is missing baseline_score")?;
    let reported_allowed_drop = ratchet
        .get("allowed_drop")
        .and_then(JsonValue::as_f64)
        .ok_or("score report ratchet is missing allowed_drop")?;
    let conformance_decision = score_report
        .get("conformance_decision")
        .and_then(JsonValue::as_str)
        .ok_or("score report is missing conformance_decision")?;
    let conformance_blockers = score_report
        .get("conformance_blockers")
        .and_then(JsonValue::as_array)
        .ok_or("score report is missing conformance_blockers")?;
    let report_dirty = score_report
        .get("dirty_worktree")
        .and_then(JsonValue::as_bool)
        .ok_or("score report is missing dirty_worktree")?;
    let report_git_dirty = score_report
        .get("git")
        .and_then(|git| git.get("dirty_worktree"))
        .and_then(JsonValue::as_bool)
        .ok_or("score report is missing git.dirty_worktree")?;
    let report_policy = score_report
        .get("policy")
        .ok_or("score report is missing policy identity")?;
    let report_policy_path = report_policy
        .get("path")
        .and_then(JsonValue::as_str)
        .ok_or("score report policy is missing path")?;
    let report_policy_auditor = report_policy
        .get("auditor_version")
        .and_then(JsonValue::as_str)
        .ok_or("score report policy is missing auditor_version")?;
    let report_policy_minimum = report_policy
        .get("minimum_score")
        .and_then(JsonValue::as_f64)
        .ok_or("score report policy is missing minimum_score")?;

    let policy_path = worktree.join(report_policy_path);
    let governed_policy_path = worktree.join("agent/audit-policy.toml");
    let governed_policy_metadata = fs::symlink_metadata(&governed_policy_path)?;
    if policy_path.canonicalize()? != governed_policy_path
        || governed_policy_path.canonicalize()? != governed_policy_path
        || !governed_policy_metadata.file_type().is_file()
        || std::os::unix::fs::MetadataExt::nlink(&governed_policy_metadata) != 1
    {
        return Err("score report policy path is not the governed repository policy".into());
    }
    let policy_bytes = fs::read(&governed_policy_path)?;
    let policy_data: toml::Value = String::from_utf8(policy_bytes.clone())?.parse()?;
    let computed_policy_fingerprint = format!("sha256:{}", sha256_bytes(&policy_bytes));
    let required_tool = string(&policy_data, "required_tool")
        .ok_or("governed audit policy is missing required_tool")?;
    let required_tool_version = string(&policy_data, "required_tool_version")
        .ok_or("governed audit policy is missing required_tool_version")?;
    let minimum_score = numeric_toml(policy_data.get("minimum_score"), "minimum_score")?;
    let allowed_drop = policy_data
        .get("allowed_score_drop")
        .map(|value| numeric_toml(Some(value), "allowed_score_drop"))
        .transpose()?
        .unwrap_or(0.0);
    if allowed_drop < 0.0 {
        return Err("governed allowed_score_drop cannot be negative".into());
    }

    let baseline_path = worktree.join("agent/jankurai-baseline.json");
    let (baseline_bytes, baseline_score, baseline_auditor) =
        match fs::symlink_metadata(&baseline_path) {
            Ok(metadata) => {
                if baseline_path.canonicalize()? != baseline_path
                    || !metadata.file_type().is_file()
                    || std::os::unix::fs::MetadataExt::nlink(&metadata) != 1
                {
                    return Err(
                        "governed Jankurai baseline must be a regular single-link file".into(),
                    );
                }
                let bytes = fs::read(&baseline_path)?;
                let baseline: JsonValue = serde_json::from_slice(&bytes)?;
                let score = baseline
                    .get("score")
                    .and_then(JsonValue::as_f64)
                    .ok_or("governed Jankurai baseline is missing numeric score")?;
                let auditor = baseline
                    .get("auditor")
                    .and_then(JsonValue::as_str)
                    .ok_or("governed Jankurai baseline is missing auditor")?
                    .to_owned();
                (Some(bytes), Some(score), Some(auditor))
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => (None, None, None),
            Err(error) => return Err(error.into()),
        };
    let baseline_configured = baseline_bytes.is_some();
    let governed_ratchet_passed =
        baseline_score.is_none_or(|baseline_score| score >= baseline_score - allowed_drop);
    let clean_tracked_tree_finish = git_tracked_tree_clean(&worktree)?;

    let mut evidence = receipt_header(
        "jain.jankurai-exact-sha-evidence/v1",
        "jankurai-evidence",
        false,
    );
    evidence["mode"] = json!("evidence");
    evidence["attempt_id"] = json!(attempt_id);
    evidence["run_id"] = json!(report_run_id);
    evidence["lane"] = json!({
        "conclusion": lane_conclusion,
        "failure_reason": lane_failure_reason,
    });
    evidence["repository"] = json!(repository);
    evidence["commit"] = json!(commit);
    evidence["worktree"] = json!(worktree);
    evidence["report"] = json!(report);
    evidence["report_sha256"] = json!(sha256_bytes(&report_bytes));
    evidence["policy"] = json!({
        "path": governed_policy_path,
        "sha256": sha256_bytes(&policy_bytes),
        "minimum_score": minimum_score,
        "allowed_score_drop": allowed_drop,
    });
    evidence["baseline"] = json!({
        "configured": baseline_configured,
        "mode": if baseline_configured { "governed-baseline" } else { "policy-floor-only" },
        "path": baseline_path,
        "sha256": baseline_bytes.as_deref().map(sha256_bytes),
        "score": baseline_score,
        "auditor": baseline_auditor.as_deref(),
    });
    evidence["report_identity"] = json!({
        "repo": report_repository,
        "git_head": report_head,
        "commit": report_commit,
        "input_fingerprint": input_fingerprint,
        "policy_fingerprint": policy_fingerprint,
        "computed_policy_fingerprint": computed_policy_fingerprint,
        "required_tool": required_tool,
        "required_tool_version": required_tool_version,
        "auditor_version": report_auditor_version,
        "policy_path": report_policy_path,
        "policy_auditor_version": report_policy_auditor,
        "dirty_worktree": report_dirty,
        "git_dirty_worktree": report_git_dirty,
        "decision_passed": decision_passed,
        "reported_minimum_score": reported_minimum_score,
        "reported_ratchet_passed": reported_ratchet_passed,
        "reported_baseline_score": reported_baseline_score,
        "reported_allowed_drop": reported_allowed_drop,
        "conformance_decision": conformance_decision,
        "conformance_blockers": conformance_blockers,
    });
    evidence["score"] = json!(score);
    evidence["hard_findings"] = json!(hard_findings);
    evidence["caps_applied"] = json!(caps_applied);
    evidence["ratchet_passed"] = json!(governed_ratchet_passed);
    evidence["clean_tracked_tree_at_start"] = json!(clean_tracked_tree_start);
    evidence["clean_tracked_tree_at_finish"] = json!(clean_tracked_tree_finish);
    evidence["auditor"] = json!({
        "path": auditor,
        "version": auditor_version,
        "sha256": sha256_bytes(&auditor_bytes),
    });

    let mut authority_failures = Vec::new();
    let mut gate_failures = Vec::new();
    if !clean_tracked_tree_start {
        authority_failures.push("exact-SHA worktree had tracked changes before audit".to_owned());
    }
    if !clean_tracked_tree_finish {
        authority_failures.push("exact-SHA worktree had tracked changes after audit".to_owned());
    }
    if report_dirty || report_git_dirty {
        authority_failures.push("Jankurai audited a dirty tracked worktree".to_owned());
    }
    if report_auditor_version != auditor_release || report_policy_auditor != auditor_release {
        authority_failures.push(format!(
            "auditor version mismatch: executable={auditor_release} report={report_auditor_version} policy={report_policy_auditor}"
        ));
    }
    if policy_fingerprint != computed_policy_fingerprint {
        authority_failures
            .push("Jankurai policy fingerprint does not match agent/audit-policy.toml".to_owned());
    }
    if required_tool != "jankurai" || required_tool_version != auditor_release {
        authority_failures.push(format!(
            "governed policy tool mismatch: required={required_tool}@{required_tool_version} executable={auditor_release}"
        ));
    }
    if reported_minimum_score != minimum_score || report_policy_minimum != minimum_score {
        authority_failures.push(format!(
            "reported score floor differs from governed policy: decision={reported_minimum_score} report_policy={report_policy_minimum} governed={minimum_score}"
        ));
    }
    if !decision_passed {
        gate_failures.push("Jankurai decision.passed is not true".to_owned());
    }
    if score < minimum_score {
        gate_failures.push(format!(
            "score {score} is below governed floor {minimum_score}"
        ));
    }
    if baseline_configured && !governed_ratchet_passed {
        gate_failures.push(format!(
            "Jankurai ratchet failed: score={score} baseline={} allowed_drop={allowed_drop}",
            baseline_score.expect("configured baseline has a score")
        ));
    }
    if reported_allowed_drop != allowed_drop {
        authority_failures.push(format!(
            "reported allowed drop {reported_allowed_drop} differs from governed {allowed_drop}"
        ));
    }
    if conformance_decision != "pass" || !conformance_blockers.is_empty() {
        gate_failures.push("Jankurai conformance did not pass without blockers".to_owned());
    }
    if hard_findings != 0 {
        gate_failures.push(format!("hard findings present: {hard_findings}"));
    }
    if caps_applied != 0 {
        gate_failures.push(format!("caps applied: {caps_applied}"));
    }
    if lane_conclusion == "failure" {
        gate_failures.push(format!(
            "authoritative lane failed: {}",
            lane_failure_reason
                .as_deref()
                .unwrap_or("unspecified failure")
        ));
    }
    let failures = authority_failures
        .iter()
        .chain(&gate_failures)
        .cloned()
        .collect::<Vec<_>>();
    evidence["authority_failures"] = json!(authority_failures);
    evidence["gate_failures"] = json!(gate_failures);
    evidence["failures"] = json!(failures);
    let result = if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("; ").into())
    };
    finish_receipted_operation(&receipt, &mut evidence, result)
}

fn is_sha256_fingerprint(value: &str) -> bool {
    value
        .strip_prefix("sha256:")
        .is_some_and(|digest| is_full_hex(digest, 64) && !digest.bytes().all(|byte| byte == b'0'))
}

fn git_tracked_tree_clean(repo: &Path) -> Result<bool, Box<dyn std::error::Error>> {
    for args in [
        ["diff", "--quiet", "HEAD", "--"].as_slice(),
        ["diff", "--cached", "--quiet"].as_slice(),
    ] {
        let status = Command::new("git")
            .args([
                "-c",
                "core.fsmonitor=false",
                "-c",
                "core.hooksPath=/dev/null",
                "-c",
                "diff.external=",
                "-C",
            ])
            .arg(repo)
            .args(args)
            .status()?;
        if !status.success() {
            return Ok(false);
        }
    }
    Ok(true)
}

fn write_atomic_bytes(path: &Path, bytes: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    let staging_path = path.with_extension(format!("tmp-{}", std::process::id()));
    fs::write(&staging_path, bytes)?;
    fs::rename(&staging_path, path)?;
    Ok(())
}

fn validate_manifest_command(args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    let mut path = PathBuf::from("repos.manifest.toml");
    let mut check_paths = false;
    let mut check_derived = false;
    let mut check_nested_authorities = false;
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--manifest" => path = PathBuf::from(iter.next().ok_or("--manifest needs a path")?),
            "--check-paths" => check_paths = true,
            "--check-derived" => check_derived = true,
            "--check-nested-authorities" => check_nested_authorities = true,
            value => return Err(format!("unknown validate-manifest argument: {value}").into()),
        }
    }
    let data: toml::Value = fs::read_to_string(&path)?.parse()?;
    validate_manifest_data(&data, &path, check_paths)?;
    if check_nested_authorities {
        let split_root = exact_absolute_path(
            &string(&data, "split_root").ok_or("manifest is missing split_root")?,
            "split_root",
        )?;
        for (key, registration) in registered_nested_families(&data) {
            validate_registered_nested_family_local(key, registration, &split_root, true)?;
        }
    }
    if check_derived {
        let expected = manifest_sha256(&path)?;
        for (target, derived) in derived_manifest_targets(&data, &path)? {
            validate_derived_manifest(&derived, &expected, &data, &path, &target)?;
        }
    }
    println!(
        "manifest valid: {} family repositories, {} infrastructure repositories, sha256 {}",
        family_repos(&data)?.len(),
        data.get("infrastructure_repo")
            .and_then(toml::Value::as_array)
            .map_or(0, Vec::len),
        manifest_sha256(&path)?
    );
    Ok(())
}

fn validate_manifest_data(
    data: &toml::Value,
    manifest: &Path,
    check_paths: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut errors = Vec::new();
    if string(data, "schema_version").as_deref() != Some("1") {
        errors.push("schema_version must be \"1\"".to_owned());
    }
    if string(data, "release_version").as_deref() != Some(RELEASE_VERSION) {
        errors.push(format!("release_version must be {RELEASE_VERSION}"));
    }
    if string(data, "status").as_deref() != Some(RELEASE_STATUS) {
        errors.push(format!("status must be {RELEASE_STATUS}"));
    }
    if data.get("formal_ga").and_then(toml::Value::as_bool) != Some(false) {
        errors.push("formal_ga must be false".to_owned());
    }
    if string(data, "sagemaker").as_deref() != Some("N/A") {
        errors.push("sagemaker must be N/A".to_owned());
    }
    if string(data, "rollback_target").as_deref() != Some(ROLLBACK_TARGET) {
        errors.push(format!("rollback_target must be {ROLLBACK_TARGET}"));
    }
    if data.get("workers").is_some() {
        errors.push(
            "workers is retired; fleet_jobs and ci_jobs are the sole job authorities".to_owned(),
        );
    }
    for key in ["fleet_jobs", "ci_jobs"] {
        if !data
            .get(key)
            .and_then(toml::Value::as_integer)
            .is_some_and(|jobs| (1..=64).contains(&jobs))
        {
            errors.push(format!("{key} must be an integer from 1 through 64"));
        }
    }
    if let Err(error) = validate_source_inventory_declaration(data, manifest, check_paths) {
        errors.push(error);
    }
    if string(data, "repo_family").as_deref() != Some("jain-split") {
        errors.push("repo_family must be jain-split".to_owned());
    }
    let split_root = string(data, "split_root").map(PathBuf::from);
    if let Some(split_root) = &split_root {
        let expected_authority = split_root.join("jain-split-ops/repos.manifest.toml");
        if string(data, "manifest_authority").as_deref()
            != Some(expected_authority.to_string_lossy().as_ref())
        {
            errors.push(format!(
                "manifest_authority must be {}",
                expected_authority.display()
            ));
        }
    } else {
        errors.push("split_root is required".to_owned());
    }
    if let Some(split_root) = &split_root {
        for target in ["portal", "deploy"] {
            if let Some(declaration) = data
                .get("derived_manifests")
                .and_then(|value| value.get(target))
            {
                let key = format!("derived_manifests.{target}.path");
                match string(declaration, "path")
                    .ok_or_else(|| format!("{key} is required"))
                    .and_then(|raw| exact_absolute_path(&raw, &key))
                {
                    Ok(path) if path.starts_with(split_root) && path != *split_root => {}
                    Ok(_) => errors.push(format!("{key} must be beneath split_root")),
                    Err(error) => errors.push(error),
                }
                if let Err(error) = derived_manifest_is_pending(data, target) {
                    errors.push(error);
                }
            }
        }
    }
    let repos = family_repos(data)?;
    let mut names = std::collections::BTreeSet::new();
    let mut paths = std::collections::BTreeSet::new();
    for raw in &repos {
        let name = string(raw, "name").unwrap_or_else(|| "<missing-name>".to_owned());
        if !names.insert(name.clone()) {
            errors.push(format!("duplicate family repository: {name}"));
        }
        let path = match string(raw, "path") {
            Some(path) => PathBuf::from(path),
            None => {
                errors.push(format!("{name}: path is required"));
                continue;
            }
        };
        if !paths.insert(path.clone()) {
            errors.push(format!("duplicate repository path: {}", path.display()));
        }
        if split_root
            .as_ref()
            .is_some_and(|root| root.join(&name) != path)
        {
            errors.push(format!(
                "{name}: path must be {}",
                split_root.as_ref().unwrap().join(&name).display()
            ));
        }
        for field in [
            "github_slug",
            "jeryu_slug",
            "profile",
            "role",
            "default_branch",
            "required_check",
        ] {
            if string(raw, field).is_none() {
                errors.push(format!("{name}: missing {field}"));
            }
        }
        if string(raw, "default_branch").as_deref() != Some("main") {
            errors.push(format!("{name}: default_branch must be main"));
        }
        if string(raw, "required_check").as_deref() != Some(format!("{name}/required").as_str()) {
            errors.push(format!("{name}: required_check must be {name}/required"));
        }
        if let Err(error) =
            validate_managed_release_identity(raw, &format!("repo[{name}]"), &name, "current_tag")
        {
            errors.push(error);
        }
        if raw.get("has_jeryu_std").and_then(toml::Value::as_bool) != Some(true) {
            errors.push(format!("{name}: has_jeryu_std must be true"));
        }
        if let Err(error) = release_feature_matrix(raw) {
            errors.push(format!("{name}: {error}"));
        }
        let expected_remote = format!("{FAMILY_REMOTE_PREFIX}{name}.git");
        if declared_remote(raw).as_deref() != Some(expected_remote.as_str()) {
            errors.push(format!("{name}: remote must be {expected_remote}"));
        }
        if check_paths && repo_is_onboarded(raw) {
            for required in ["AGENTS.md", "agent/owner-map.json", "agent/test-map.json"] {
                if !path.join(required).is_file() {
                    errors.push(format!("{name}: missing {required}"));
                }
            }
        }
    }
    let required = strings(data, "required_repos")
        .into_iter()
        .collect::<std::collections::BTreeSet<_>>();
    if required != names {
        errors.push(format!(
            "required_repos differs from family repo names (required {}, repos {})",
            required.len(),
            names.len()
        ));
    }
    if repos.len() != 26 {
        errors.push(format!(
            "expected 26 family repositories, found {}",
            repos.len()
        ));
    }
    let infrastructure = data
        .get("infrastructure_repo")
        .and_then(toml::Value::as_array)
        .ok_or("manifest must declare infrastructure_repo")?;
    let smartcluster = infrastructure
        .iter()
        .find(|raw| string(raw, "name").as_deref() == Some("jain-smartcluster"));
    if infrastructure.len() != 1 || smartcluster.is_none() {
        errors.push(
            "manifest must declare exactly one infrastructure repo: jain-smartcluster".to_owned(),
        );
    } else if let Some(raw) = smartcluster {
        for (key, expected) in [
            ("kind", "required-infrastructure"),
            ("forge_owner", "veox"),
            ("forge_slug", "veox/jain-smartcluster"),
            ("required_check", "jain-smartcluster/required"),
            ("default_branch", "main"),
        ] {
            if string(raw, key).as_deref() != Some(expected) {
                errors.push(format!("jain-smartcluster: {key} must be {expected}"));
            }
        }
        let expected_infra_remote = format!("{INFRA_REMOTE_PREFIX}jain-smartcluster.git");
        if declared_remote(raw).as_deref() != Some(expected_infra_remote.as_str()) {
            errors.push("jain-smartcluster: remote must use the veox namespace".to_owned());
        }
        if raw.get("family_registered").and_then(toml::Value::as_bool) != Some(true) {
            errors.push("jain-smartcluster: family_registered must be true".to_owned());
        }
        if let Err(error) = validate_managed_release_identity(
            raw,
            "infrastructure_repo[jain-smartcluster]",
            "jain-smartcluster",
            "immutable_tag",
        ) {
            errors.push(error);
        }
    }
    let control = data
        .get("control_plane")
        .ok_or("manifest must declare control_plane")?;
    if string(control, "name").as_deref() != Some("jain-split-ops") {
        errors.push("control_plane.name must be jain-split-ops".to_owned());
    }
    if string(control, "required_check").as_deref() != Some("jain-split-ops/required") {
        errors.push("control_plane.required_check must be jain-split-ops/required".to_owned());
    }
    if string(control, "forge_owner").as_deref() != Some("veox") {
        errors.push("control_plane.forge_owner must be veox".to_owned());
    }
    if let Err(error) =
        validate_managed_release_identity(control, "control_plane", "jain-split-ops", "current_tag")
    {
        errors.push(error);
    }
    if let Some(root) = split_root.as_deref() {
        for (key, registration) in registered_nested_families(data) {
            if let Err(error) =
                validate_registered_nested_family_declaration(key, registration, root)
            {
                errors.push(error);
            } else if check_paths {
                if let Err(error) =
                    validate_registered_nested_family_local(key, registration, root, false)
                {
                    errors.push(error);
                }
            }
        }
    }
    if check_paths {
        if let Err(error) = validate_nested_family_local(data, true, &mut errors) {
            errors.push(format!("nested family path validation failed: {error}"));
        }
    } else if let Err(error) = nested_engine_topology(data) {
        errors.push(error);
    }
    if !errors.is_empty() {
        return Err(format!(
            "manifest validation failed ({}):\n{}",
            manifest.display(),
            errors.join("\n")
        )
        .into());
    }
    Ok(())
}

fn manifest_sha256(path: &Path) -> Result<String, Box<dyn std::error::Error>> {
    let bytes = fs::read(path)?;
    let digest = Sha256::digest(bytes);
    Ok(format!("{digest:x}"))
}

fn validate_derived_manifest(
    path: &Path,
    expected: &str,
    canonical: &toml::Value,
    canonical_path: &Path,
    target: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if derived_manifest_is_pending(canonical, target)? {
        return Ok(());
    }
    if !path.is_file() {
        return Err(format!("required derived manifest is missing: {}", path.display()).into());
    }
    let actual_text = fs::read_to_string(path)?;
    let data: toml::Value = actual_text.parse()?;
    let actual = string(&data, "canonical_manifest_sha256")
        .ok_or_else(|| format!("{} missing canonical_manifest_sha256", path.display()))?;
    if actual != expected {
        return Err(format!(
            "{} is out of date: canonical manifest sha256 is {}, expected {}",
            path.display(),
            actual,
            expected
        )
        .into());
    }
    let rendered = render_derived_manifest(canonical, canonical_path, target, expected)?;
    if actual_text != rendered {
        return Err(format!(
            "{} differs from the canonical generated {} manifest",
            path.display(),
            target
        )
        .into());
    }
    Ok(())
}

fn validate_family_lock(args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    let root = control_plane_root();
    let mut manifest = root.join("repos.manifest.toml");
    let mut lock = root.parent().unwrap_or(&root).join("jain/family.lock");
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--manifest" => manifest = PathBuf::from(iter.next().ok_or("--manifest needs a path")?),
            "--lock" => lock = PathBuf::from(iter.next().ok_or("--lock needs a path")?),
            value => return Err(format!("unknown validate-family-lock argument: {value}").into()),
        }
    }
    let data: toml::Value = fs::read_to_string(&manifest)?.parse()?;
    validate_manifest_data(&data, &manifest, false)?;
    let lock_data: toml::Value = fs::read_to_string(&lock)?.parse()?;
    let expected_hash = manifest_sha256(&manifest)?;
    let mut errors = Vec::new();
    if string(&lock_data, "source_manifest_sha256").as_deref() != Some(expected_hash.as_str()) {
        errors.push("source_manifest_sha256 does not match the canonical manifest".to_owned());
    }
    if string(&lock_data, "release").as_deref()
        != Some(format!("{RELEASE_VERSION}-split.0").as_str())
    {
        errors.push(format!("lock release is not {RELEASE_VERSION}-split.0"));
    }
    let family = family_repos(&data)?;
    let lock_repos = lock_data
        .get("repo")
        .and_then(toml::Value::as_array)
        .cloned()
        .unwrap_or_default();
    if lock_repos.len() != family.len() {
        errors.push(format!(
            "lock has {} family entries, expected {}",
            lock_repos.len(),
            family.len()
        ));
    }
    for raw in family.iter().copied().chain(
        data.get("infrastructure_repo")
            .and_then(toml::Value::as_array)
            .into_iter()
            .flatten(),
    ) {
        let name = string(raw, "name").unwrap_or_default();
        let tag = declared_release_tag(raw);
        let found = lock_repos
            .iter()
            .chain(
                lock_data
                    .get("infrastructure_repo")
                    .and_then(toml::Value::as_array)
                    .into_iter()
                    .flatten(),
            )
            .find(|entry| {
                string(entry, "repo")
                    .or_else(|| string(entry, "name"))
                    .as_deref()
                    == Some(name.as_str())
            });
        match found {
            Some(entry) => {
                if string(entry, "tag") != tag {
                    errors.push(format!("{name}: lock tag does not match the manifest"));
                }
                let commit = string(entry, "commit").unwrap_or_default();
                if commit.len() != 40 || !commit.chars().all(|ch| ch.is_ascii_hexdigit()) {
                    errors.push(format!(
                        "{name}: lock commit is not an immutable 40-character SHA"
                    ));
                }
            }
            None => errors.push(format!("{name}: missing from family lock")),
        }
    }
    if !errors.is_empty() {
        return Err(format!("family lock validation failed:\n{}", errors.join("\n")).into());
    }
    println!("family lock valid: {}", lock.display());
    Ok(())
}

fn regenerate_lock(args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    let root = control_plane_root();
    let mut manifest = root.join("repos.manifest.toml");
    let mut output = root.parent().unwrap_or(&root).join("jain/family.lock");
    let mut apply = false;
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--manifest" => manifest = PathBuf::from(iter.next().ok_or("--manifest needs a path")?),
            "--output" => output = PathBuf::from(iter.next().ok_or("--output needs a path")?),
            "--apply" => apply = true,
            value => return Err(format!("unknown regenerate-lock argument: {value}").into()),
        }
    }
    if !apply {
        return Err("regenerate-lock is a write; pass --apply explicitly".into());
    }
    let data: toml::Value = fs::read_to_string(&manifest)?.parse()?;
    validate_manifest_data(&data, &manifest, false)?;
    let manifest_hash = manifest_sha256(&manifest)?;
    let release = string(&data, "release_version").ok_or("manifest missing release_version")?;
    let mut text = format!(
        "schema_version = \"1.0.0\"\nfamily = \"jain-split\"\nrelease = \"{release}-split.0\"\ngenerator_version = \"splitctl 0.1.0\"\nsource = \"{}\"\nsource_manifest_sha256 = \"{manifest_hash}\"\nfamily_repo_count = {}\ninfrastructure_repo_count = {}\ndependency_resolution = \"immutable-git-tag\"\n\n",
        manifest.display(),
        family_repos(&data)?.len(),
        data.get("infrastructure_repo")
            .and_then(toml::Value::as_array)
            .map_or(0, Vec::len)
    );
    for raw in manifest_repos(&data)? {
        let repo = repo_from(raw)?;
        let tag = declared_release_tag(raw)
            .ok_or_else(|| format!("{} missing immutable tag", repo.name))?;
        let tag_ref = format!("refs/tags/{tag}^{{}}");
        let commit = git_query(&repo.path, &["rev-parse", &tag_ref])
            .ok_or_else(|| format!("{} is missing immutable tag {tag}", repo.name))?;
        let table =
            if raw.get("kind").and_then(toml::Value::as_str) == Some("required-infrastructure") {
                "infrastructure_repo"
            } else {
                "repo"
            };
        text.push_str(&format!(
            "[[{table}]]\nrepo = \"{}\"\ntag = \"{tag}\"\ncommit = \"{commit}\"\njeryu = \"{}\"\nrequired_check = \"{}\"\n\n",
            repo.name,
            declared_remote(raw).ok_or_else(|| format!("{} missing remote", repo.name))?,
            string(raw, "required_check").unwrap_or_else(|| format!("{}/required", repo.name))
        ));
    }
    if data.get("nested_families").is_some() {
        text.push_str(&render_nested_lock_section(&nested_engine_topology(
            &data,
        )?)?);
    }
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&output, text)?;
    println!("regenerated family lock: {}", output.display());
    Ok(())
}

fn render_nested_lock_section(topology: &NestedEngineTopology) -> Result<String, String> {
    let identity = topology.bound_identity.as_ref().ok_or_else(|| {
        format!(
            "cannot render pending nested dependency {} into a release lock",
            topology.dependency_name
        )
    })?;
    Ok(format!(
        "[nested.{}]\nfamily = \"{}\"\nproduct_version = \"{}\"\ntag_revision = {}\nremote = \"{}\"\ntag = \"{}\"\ncommit = \"{}\"\ntree = \"{}\"\nchecksum_sha256 = \"{}\"\nrequired_check = \"{}\"\n\n",
        topology.dependency_name,
        topology.family,
        identity.product_version,
        identity.tag_revision,
        topology.engine_remote,
        identity.tag,
        identity.release_commit,
        identity.release_tree,
        identity.release_checksum_sha256,
        topology.engine_required_check,
    ))
}

fn release_preflight(args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    preflight(args)
}

fn release_snapshot(args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    let root = control_plane_root();
    let mut manifest = root.join("repos.manifest.toml");
    let mut output = None;
    let mut apply = false;
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--manifest" => manifest = PathBuf::from(iter.next().ok_or("--manifest needs a path")?),
            "--json" => output = Some(PathBuf::from(iter.next().ok_or("--json needs a path")?)),
            "--apply" => apply = true,
            value => return Err(format!("unknown release-snapshot argument: {value}").into()),
        }
    }
    let data: toml::Value = fs::read_to_string(&manifest)?.parse()?;
    validate_manifest_data(&data, &manifest, false)?;
    let rows = snapshot_rows(&data)?;
    let status = if rows.iter().all(|row| row["status"] == "pass") {
        "pass"
    } else {
        "fail"
    };
    let report = json!({
        "schema_version": "jain.release.snapshot/v1",
        "release": RELEASE_VERSION,
        "release_status": RELEASE_STATUS,
        "formal_ga": false,
        "sagemaker": "N/A",
        "rollback_target": ROLLBACK_TARGET,
        "manifest": manifest,
        "manifest_sha256": manifest_sha256(&manifest)?,
        "repositories": rows,
        "status": status,
    });
    if let Some(path) = output {
        if !apply {
            return Err("release-snapshot writes a receipt; pass --apply explicitly".into());
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, serde_json::to_vec_pretty(&report)?)?;
        println!("wrote {}", path.display());
    } else {
        println!("{}", serde_json::to_string_pretty(&report)?);
    }
    if status == "pass" {
        Ok(())
    } else {
        Err("release snapshot is not releasable".into())
    }
}

const MAX_APPLIANCE_AGGREGATE_BYTES: u64 = 1024 * 1024;

#[derive(Debug)]
struct QualifiedApplianceCanary {
    matrix: JsonValue,
    aggregate_sha256: String,
    verifier_receipt_sha256: String,
}

struct PhysicalJsonEvidence {
    value: JsonValue,
    sha256: String,
}

#[derive(Clone, Copy)]
struct StrictJsonSeed;

struct StrictJsonVisitor;

impl<'de> DeserializeSeed<'de> for StrictJsonSeed {
    type Value = JsonValue;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_any(StrictJsonVisitor)
    }
}

impl<'de> Visitor<'de> for StrictJsonVisitor {
    type Value = JsonValue;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("an unambiguous JSON value")
    }

    fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E> {
        Ok(JsonValue::Bool(value))
    }

    fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E> {
        Ok(JsonValue::Number(value.into()))
    }

    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E> {
        Ok(JsonValue::Number(value.into()))
    }

    fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        serde_json::Number::from_f64(value)
            .map(JsonValue::Number)
            .ok_or_else(|| E::custom("non-finite JSON number"))
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E> {
        Ok(JsonValue::String(value.to_owned()))
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E> {
        Ok(JsonValue::String(value))
    }

    fn visit_none<E>(self) -> Result<Self::Value, E> {
        Ok(JsonValue::Null)
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(JsonValue::Null)
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut values = Vec::new();
        while let Some(value) = sequence.next_element_seed(StrictJsonSeed)? {
            values.push(value);
        }
        Ok(JsonValue::Array(values))
    }

    fn visit_map<A>(self, mut entries: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut object = Map::new();
        while let Some(key) = entries.next_key::<String>()? {
            if object.contains_key(&key) {
                return Err(de::Error::custom(format!("duplicate JSON field: {key}")));
            }
            object.insert(key, entries.next_value_seed(StrictJsonSeed)?);
        }
        Ok(JsonValue::Object(object))
    }
}

fn exact_json_object<'a>(
    value: &'a JsonValue,
    keys: &[&str],
) -> Option<&'a Map<String, JsonValue>> {
    value.as_object().filter(|object| {
        object.len() == keys.len() && keys.iter().all(|key| object.contains_key(*key))
    })
}

fn valid_nonzero_lower_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        && value.bytes().any(|byte| byte != b'0')
}

fn valid_sha256(value: &str) -> bool {
    valid_nonzero_lower_hex(value, 64)
}

fn valid_digest(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(valid_sha256)
}

fn valid_https_url(value: &str) -> bool {
    let Some(rest) = value.strip_prefix("https://") else {
        return false;
    };
    if rest.is_empty()
        || value.chars().any(char::is_whitespace)
        || rest.contains('\\')
        || rest.contains(['?', '#'])
    {
        return false;
    }
    let authority_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let authority = &rest[..authority_end];
    if authority.is_empty()
        || !authority.is_ascii()
        || authority.contains(['@', '%'])
        || authority.starts_with('[')
    {
        return false;
    }
    let (host, port) = match authority.rsplit_once(':') {
        Some((host, port)) if !host.contains(':') => (host, Some(port)),
        Some(_) => return false,
        None => (authority, None),
    };
    if port.is_some_and(|port| {
        port.is_empty()
            || !port.bytes().all(|byte| byte.is_ascii_digit())
            || port
                .parse::<u16>()
                .ok()
                .filter(|value| *value > 0)
                .is_none()
    }) {
        return false;
    }
    let host = host.to_ascii_lowercase();
    host.contains('.')
        && host.parse::<std::net::IpAddr>().is_err()
        && host != "localhost"
        && !host.ends_with(".localhost")
        && !host.ends_with(".localdomain")
        && host.split('.').all(|label| {
            !label.is_empty()
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        })
}

fn valid_deploy_release_tag(value: &str) -> bool {
    value
        .strip_prefix(&format!("jain-deploy-v{RELEASE_VERSION}-split."))
        .is_some_and(|suffix| {
            !suffix.is_empty()
                && suffix.bytes().all(|byte| byte.is_ascii_digit())
                && suffix
                    .parse::<u64>()
                    .is_ok_and(|number| number.to_string() == suffix)
        })
}

fn valid_oci_repository(value: &str) -> bool {
    if value.is_empty() || value.len() > 255 || !value.is_ascii() {
        return false;
    }
    let mut components = value.split('/');
    let registry = components.next().unwrap_or_default();
    let paths = components.collect::<Vec<_>>();
    let registry = registry.to_ascii_lowercase();
    let registry_is_canonical = registry.len() <= 253
        && registry == value.split('/').next().unwrap_or_default()
        && registry.contains('.')
        && registry.parse::<std::net::IpAddr>().is_err()
        && registry != "localhost"
        && !registry.ends_with(".localhost")
        && !registry.ends_with(".localdomain")
        && registry.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        });
    registry_is_canonical
        && !paths.is_empty()
        && paths
            .iter()
            .all(|component| valid_oci_path_component(component))
}

fn valid_oci_path_component(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.is_empty() || bytes.len() > 128 {
        return false;
    }
    let alphanumeric = |byte: u8| byte.is_ascii_lowercase() || byte.is_ascii_digit();
    if !alphanumeric(bytes[0]) {
        return false;
    }
    let mut index = 0;
    while index < bytes.len() && alphanumeric(bytes[index]) {
        index += 1;
    }
    while index < bytes.len() {
        match bytes[index] {
            b'.' => index += 1,
            b'_' => {
                index += 1;
                if index < bytes.len() && bytes[index] == b'_' {
                    index += 1;
                }
            }
            b'-' => {
                while index < bytes.len() && bytes[index] == b'-' {
                    index += 1;
                }
            }
            _ => return false,
        }
        let start = index;
        while index < bytes.len() && alphanumeric(bytes[index]) {
            index += 1;
        }
        if index == start {
            return false;
        }
    }
    true
}

fn valid_repo_digest(value: &str, expected_digest: &str) -> bool {
    let mut parts = value.split('@');
    let repository = parts.next().unwrap_or_default();
    let digest = parts.next().unwrap_or_default();
    parts.next().is_none()
        && valid_oci_repository(repository)
        && digest == expected_digest
        && valid_digest(digest)
}

fn appliance_artifact_set_sha256(artifacts: &JsonValue) -> Option<String> {
    let artifacts = exact_json_object(
        artifacts,
        &[
            "installer",
            "manager",
            "cli",
            "compose",
            "compose_gpu",
            "provenance",
            "browser_suite",
            "training_data",
            "scoring_data",
        ],
    )?;
    let rows = [
        ("browser_suite", artifacts["browser_suite"].as_str()?),
        ("cli", artifacts["cli"].as_str()?),
        ("compose", artifacts["compose"].as_str()?),
        ("compose_gpu", artifacts["compose_gpu"].as_str()?),
        ("installer", artifacts["installer"].as_str()?),
        ("manager", artifacts["manager"].as_str()?),
        ("provenance", artifacts["provenance"].as_str()?),
        ("scoring_data", artifacts["scoring_data"].as_str()?),
        ("training_data", artifacts["training_data"].as_str()?),
    ]
    .into_iter()
    .map(|(name, digest)| format!("{name}\t{digest}\n"))
    .collect::<String>();
    Some(sha256_bytes(rows.as_bytes()))
}

fn valid_utc_second_timestamp(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != 20
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || bytes[10] != b'T'
        || bytes[13] != b':'
        || bytes[16] != b':'
        || bytes[19] != b'Z'
        || bytes
            .iter()
            .enumerate()
            .any(|(index, byte)| ![4, 7, 10, 13, 16, 19].contains(&index) && !byte.is_ascii_digit())
    {
        return false;
    }
    let number = |start: usize, end: usize| {
        value[start..end]
            .parse::<u32>()
            .expect("timestamp digits were validated")
    };
    let year = number(0, 4);
    let month = number(5, 7);
    let day = number(8, 10);
    let hour = number(11, 13);
    let minute = number(14, 16);
    let second = number(17, 19);
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return false,
    };
    year > 0 && day > 0 && day <= days && hour < 24 && minute < 60 && second < 60
}

fn secret_like_json(value: &JsonValue) -> bool {
    match value {
        JsonValue::Object(object) => object.iter().any(|(key, value)| {
            let key = key.to_ascii_lowercase();
            let forbidden_key = matches!(
                key.as_str(),
                "password" | "passwd" | "token" | "secret" | "private_key" | "credential"
            ) || key
                .split(|character: char| !character.is_ascii_alphanumeric())
                .any(|part| {
                    matches!(
                        part,
                        "password" | "passwd" | "token" | "secret" | "credential"
                    )
                });
            forbidden_key || secret_like_json(value)
        }),
        JsonValue::Array(values) => values.iter().any(secret_like_json),
        JsonValue::String(value) => {
            let value = value.to_ascii_lowercase();
            let bearer = ["bearer ", "bearer\t", "bearer\n", "bearer\r"]
                .iter()
                .any(|marker| value.contains(marker));
            let query_secret = [
                "?token=",
                "&token=",
                "?password=",
                "&password=",
                "?secret=",
                "&secret=",
                "?api_key=",
                "&api_key=",
                "?api-key=",
                "&api-key=",
                "?apikey=",
                "&apikey=",
            ]
            .iter()
            .any(|marker| value.contains(marker));
            bearer
                || query_secret
                || (value.contains("-----begin ") && value.contains("private key-----"))
        }
        _ => false,
    }
}

fn read_physical_json_evidence(
    path: &Path,
    label: &str,
    expected_uid: u32,
    expected_gid: u32,
) -> Result<PhysicalJsonEvidence, Box<dyn std::error::Error>> {
    let mut file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK)
        .open(path)
        .map_err(|error| format!("cannot open physical {label}: {error}"))?;
    let before = file.metadata()?;
    if !before.file_type().is_file()
        || before.nlink() != 1
        || before.len() == 0
        || before.len() > MAX_APPLIANCE_AGGREGATE_BYTES
        || before.mode() & 0o222 != 0
        || before.uid() != expected_uid
        || before.gid() != expected_gid
    {
        return Err(format!(
            "{label} must be a nonempty, non-writable, single-link regular file owned by {expected_uid}:{expected_gid} and no larger than 1 MiB"
        )
        .into());
    }
    let mut bytes = Vec::with_capacity(before.len() as usize);
    (&mut file)
        .take(MAX_APPLIANCE_AGGREGATE_BYTES + 1)
        .read_to_end(&mut bytes)?;
    let after = file.metadata()?;
    let path_after = fs::symlink_metadata(path)?;
    if bytes.len() as u64 != before.len()
        || !same_file_metadata(&before, &after)
        || !same_file_metadata(&before, &path_after)
    {
        return Err(format!("{label} changed while it was read").into());
    }
    let mut deserializer = serde_json::Deserializer::from_slice(&bytes);
    let raw = StrictJsonSeed
        .deserialize(&mut deserializer)
        .map_err(|error| format!("{label} is not unambiguous JSON: {error}"))?;
    deserializer
        .end()
        .map_err(|error| format!("{label} has trailing JSON data: {error}"))?;
    if secret_like_json(&raw) {
        return Err(format!("{label} contains secret-like evidence").into());
    }
    Ok(PhysicalJsonEvidence {
        value: raw,
        sha256: sha256_bytes(&bytes),
    })
}

fn appliance_verifier_seal(receipt: &Map<String, JsonValue>) -> Option<String> {
    let verifier = exact_json_object(&receipt["verifier"], &["name", "sha256"])?;
    let rows = [
        ("verifier_name", verifier["name"].as_str()?),
        ("verifier_sha256", verifier["sha256"].as_str()?),
        ("aggregate_sha256", receipt["aggregate_sha256"].as_str()?),
        ("release_tag", receipt["release_tag"].as_str()?),
        ("source_commit", receipt["source_commit"].as_str()?),
        ("release_job_id", receipt["release_job_id"].as_str()?),
        (
            "attestation_sha256",
            receipt["attestation_sha256"].as_str()?,
        ),
        ("signature_sha256", receipt["signature_sha256"].as_str()?),
        ("public_key_sha256", receipt["public_key_sha256"].as_str()?),
        ("verified_at", receipt["verified_at"].as_str()?),
    ]
    .into_iter()
    .map(|(name, value)| format!("{name}\t{value}\n"))
    .collect::<String>();
    Some(sha256_bytes(rows.as_bytes()))
}

fn validate_appliance_verifier_receipt(
    receipt: &JsonValue,
    aggregate: &QualifiedApplianceCanary,
) -> Result<(), Box<dyn std::error::Error>> {
    let receipt = exact_json_object(
        receipt,
        &[
            "schema_version",
            "verifier",
            "aggregate_sha256",
            "release_tag",
            "source_commit",
            "release_job_id",
            "attestation_sha256",
            "signature_sha256",
            "public_key_sha256",
            "verified_at",
            "seal_sha256",
        ],
    )
    .ok_or("appliance verifier receipt violates its closed shape")?;
    if receipt["schema_version"].as_str() != Some("jain.local-appliance-canary-verifier/v1") {
        return Err("unsupported appliance verifier receipt schema".into());
    }
    let verifier = exact_json_object(&receipt["verifier"], &["name", "sha256"])
        .ok_or("appliance verifier identity violates its closed shape")?;
    if verifier["name"].as_str() != Some(APPLIANCE_VERIFIER_NAME)
        || verifier["sha256"].as_str() != Some(APPLIANCE_VERIFIER_SHA256)
    {
        return Err("appliance verifier identity is not the reviewed release verifier".into());
    }
    let matrix = &aggregate.matrix;
    if receipt["aggregate_sha256"].as_str() != Some(aggregate.aggregate_sha256.as_str())
        || receipt["release_tag"] != matrix["release_tag"]
        || receipt["source_commit"] != matrix["source_commit"]
        || receipt["release_job_id"] != matrix["release_job"]["id"]
        || receipt["attestation_sha256"] != matrix["release_job"]["attestation_sha256"]
        || receipt["public_key_sha256"] != matrix["public_key_sha256"]
        || !receipt["signature_sha256"]
            .as_str()
            .is_some_and(valid_sha256)
        || !receipt["verified_at"]
            .as_str()
            .is_some_and(valid_utc_second_timestamp)
        || !receipt["seal_sha256"].as_str().is_some_and(valid_sha256)
        || receipt["seal_sha256"].as_str() != appliance_verifier_seal(receipt).as_deref()
    {
        return Err(
            "appliance verifier receipt does not seal the qualified release identities".into(),
        );
    }
    Ok(())
}

fn read_qualified_appliance_canary_with_authority(
    aggregate_path: &Path,
    verifier_receipt_path: &Path,
    expected_uid: u32,
    expected_gid: u32,
    expected_tag_commit: Option<&str>,
) -> Result<QualifiedApplianceCanary, Box<dyn std::error::Error>> {
    let aggregate = read_physical_json_evidence(
        aggregate_path,
        "appliance aggregate",
        expected_uid,
        expected_gid,
    )?;
    validate_qualified_appliance_canary(&aggregate.value)?;
    let mut qualification = QualifiedApplianceCanary {
        matrix: aggregate.value,
        aggregate_sha256: aggregate.sha256,
        verifier_receipt_sha256: String::new(),
    };
    if expected_tag_commit
        .is_some_and(|expected| qualification.matrix["source_commit"].as_str() != Some(expected))
    {
        return Err(
            "appliance immutable Deploy tag does not resolve to the claimed source commit".into(),
        );
    }
    let verifier = read_physical_json_evidence(
        verifier_receipt_path,
        "appliance verifier receipt",
        expected_uid,
        expected_gid,
    )?;
    validate_appliance_verifier_receipt(&verifier.value, &qualification)?;
    qualification.verifier_receipt_sha256 = verifier.sha256;
    Ok(qualification)
}

fn read_qualified_appliance_canary(
    aggregate_path: &Path,
    verifier_receipt_path: &Path,
    token_file: &Path,
) -> Result<QualifiedApplianceCanary, Box<dyn std::error::Error>> {
    let qualification = read_qualified_appliance_canary_with_authority(
        aggregate_path,
        verifier_receipt_path,
        ROOT_UID,
        ROOT_GID,
        None,
    )?;
    let tag = qualification.matrix["release_tag"]
        .as_str()
        .ok_or("appliance aggregate release tag is not a string")?;
    let source_commit = qualification.matrix["source_commit"]
        .as_str()
        .ok_or("appliance aggregate source commit is not a string")?;
    let tag_ref = format!("refs/tags/{tag}");
    let forge_commit = secure_ls_remote_at(APPLIANCE_DEPLOY_REMOTE, &tag_ref, token_file)?
        .ok_or("appliance immutable Deploy tag is absent from the governed forge")?;
    if forge_commit != source_commit {
        return Err(
            "appliance immutable Deploy tag does not resolve to the claimed source commit".into(),
        );
    }
    Ok(qualification)
}

fn validate_qualified_appliance_canary(
    matrix: &JsonValue,
) -> Result<(), Box<dyn std::error::Error>> {
    let matrix = exact_json_object(
        matrix,
        &[
            "schema_version",
            "qualification",
            "fixture",
            "release",
            "release_tag",
            "source_commit",
            "status",
            "formal_ga",
            "rollback_release",
            "release_job",
            "manifest_sha256",
            "public_key_sha256",
            "artifact_identities",
            "artifact_set_sha256",
            "oci",
            "lanes",
            "created_at",
        ],
    )
    .ok_or("appliance aggregate violates its closed top-level shape")?;
    if matrix["schema_version"].as_str() != Some("jain.local-appliance-canary-matrix/v1") {
        return Err("unsupported appliance aggregate schema".into());
    }
    if matrix["qualification"].as_bool() != Some(true) || matrix["fixture"].as_bool() != Some(false)
    {
        return Err("appliance promotion requires a non-fixture qualified aggregate".into());
    }
    let release = matrix["release"]
        .as_str()
        .ok_or("appliance aggregate release is not a string")?;
    if release != RELEASE_VERSION {
        return Err(format!(
            "appliance aggregate release is {}, expected {RELEASE_VERSION}",
            release
        )
        .into());
    }
    let release_tag = matrix["release_tag"].as_str().unwrap_or_default();
    let source_commit = matrix["source_commit"].as_str().unwrap_or_default();
    if !valid_deploy_release_tag(release_tag) || !valid_nonzero_lower_hex(source_commit, 40) {
        return Err("appliance aggregate has an invalid release tag or source commit".into());
    }
    if matrix["status"].as_str() != Some(RELEASE_STATUS)
        || matrix["formal_ga"].as_bool() != Some(false)
        || matrix["rollback_release"].as_str() != Some(ROLLBACK_TARGET)
    {
        return Err("appliance aggregate violates candidate/GA/rollback policy".into());
    }
    let release_job = exact_json_object(
        &matrix["release_job"],
        &[
            "id",
            "attestation_url",
            "attestation_sha256",
            "signature_url",
            "verified",
        ],
    )
    .ok_or("appliance aggregate release job violates its closed shape")?;
    if release_job["verified"].as_bool() != Some(true)
        || !release_job["id"].as_str().is_some_and(valid_sha256)
        || !release_job["attestation_sha256"]
            .as_str()
            .is_some_and(valid_sha256)
        || !release_job["attestation_url"]
            .as_str()
            .is_some_and(valid_https_url)
        || !release_job["signature_url"]
            .as_str()
            .is_some_and(valid_https_url)
    {
        return Err(
            "appliance aggregate lacks a verified signed HTTPS release-job identity".into(),
        );
    }
    if !matrix["manifest_sha256"].as_str().is_some_and(valid_sha256)
        || !matrix["public_key_sha256"]
            .as_str()
            .is_some_and(valid_sha256)
        || !matrix["artifact_set_sha256"]
            .as_str()
            .is_some_and(valid_sha256)
    {
        return Err(
            "appliance aggregate has an invalid manifest, key, or artifact-set digest".into(),
        );
    }
    let artifacts = &matrix["artifact_identities"];
    let artifact_names = [
        "installer",
        "manager",
        "cli",
        "compose",
        "compose_gpu",
        "provenance",
        "browser_suite",
        "training_data",
        "scoring_data",
    ];
    let artifact_object = exact_json_object(artifacts, &artifact_names)
        .ok_or("appliance aggregate artifacts violate their closed shape")?;
    if artifact_names
        .iter()
        .any(|name| !artifact_object[*name].as_str().is_some_and(valid_sha256))
    {
        return Err("appliance aggregate has an invalid artifact identity".into());
    }
    if matrix["artifact_set_sha256"].as_str() != appliance_artifact_set_sha256(artifacts).as_deref()
    {
        return Err("appliance aggregate artifact-set digest does not bind its identities".into());
    }
    let oci = exact_json_object(
        &matrix["oci"],
        &[
            "index",
            "index_digest",
            "platform",
            "platform_digest",
            "runtime_image_id",
            "runtime_repo_digest",
        ],
    )
    .ok_or("appliance aggregate OCI identity violates its closed shape")?;
    let index_digest = oci["index_digest"].as_str().unwrap_or_default();
    if oci["platform"].as_str() != Some("linux/amd64")
        || !valid_digest(index_digest)
        || !oci["platform_digest"].as_str().is_some_and(valid_digest)
        || !oci["runtime_image_id"].as_str().is_some_and(valid_digest)
        || !oci["index"]
            .as_str()
            .is_some_and(|value| valid_repo_digest(value, index_digest))
        || !oci["runtime_repo_digest"]
            .as_str()
            .is_some_and(|value| valid_repo_digest(value, index_digest))
    {
        return Err("appliance aggregate has a mismatched or invalid OCI identity".into());
    }
    let lanes = exact_json_object(&matrix["lanes"], &["cpu", "gpu"])
        .ok_or("appliance aggregate lanes violate their closed shape")?;
    let cpu = exact_json_object(&lanes["cpu"], &["receipt_sha256", "qualification"])
        .ok_or("appliance CPU lane violates its closed shape")?;
    let gpu = exact_json_object(&lanes["gpu"], &["receipt_sha256", "qualification"])
        .ok_or("appliance GPU lane violates its closed shape")?;
    let cpu_receipt = cpu["receipt_sha256"].as_str().unwrap_or_default();
    let gpu_receipt = gpu["receipt_sha256"].as_str().unwrap_or_default();
    if cpu["qualification"].as_bool() != Some(true)
        || gpu["qualification"].as_bool() != Some(true)
        || !valid_sha256(cpu_receipt)
        || !valid_sha256(gpu_receipt)
        || cpu_receipt == gpu_receipt
    {
        return Err("appliance aggregate requires distinct qualified CPU and GPU receipts".into());
    }
    if !matrix["created_at"]
        .as_str()
        .is_some_and(valid_utc_second_timestamp)
    {
        return Err("appliance aggregate created_at is not a valid UTC second timestamp".into());
    }
    Ok(())
}

fn appliance_canary_summary(qualification: &QualifiedApplianceCanary) -> JsonValue {
    let matrix = &qualification.matrix;
    json!({
        "status": "pass",
        "qualification": true,
        "fixture": false,
        "aggregate_sha256": qualification.aggregate_sha256,
        "verifier_receipt_sha256": qualification.verifier_receipt_sha256,
        "release": matrix["release"],
        "release_tag": matrix["release_tag"],
        "source_commit": matrix["source_commit"],
        "release_job_id": matrix["release_job"]["id"],
        "manifest_sha256": matrix["manifest_sha256"],
        "public_key_sha256": matrix["public_key_sha256"],
        "artifact_set_sha256": matrix["artifact_set_sha256"],
        "oci_index_digest": matrix["oci"]["index_digest"],
        "oci_platform_digest": matrix["oci"]["platform_digest"],
        "cpu_receipt_sha256": matrix["lanes"]["cpu"]["receipt_sha256"],
        "gpu_receipt_sha256": matrix["lanes"]["gpu"]["receipt_sha256"],
        "created_at": matrix["created_at"],
    })
}

fn write_or_print_json_report(
    output: Option<&Path>,
    report: &JsonValue,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(path) = output {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, serde_json::to_vec_pretty(report)?)?;
        println!("wrote {}", path.display());
    } else {
        println!("{}", serde_json::to_string_pretty(report)?);
    }
    Ok(())
}

fn appliance_promotion_report(qualification: &QualifiedApplianceCanary) -> JsonValue {
    json!({
        "schema_version": "jain.appliance-promotion-validation/v1",
        "release": RELEASE_VERSION,
        "status": RELEASE_STATUS,
        "formal_ga": false,
        "rollback_target": ROLLBACK_TARGET,
        "appliance_canary": appliance_canary_summary(qualification),
        "promotion_evidence_ready": true,
        "production_promotion_authorized": false,
        "reason": "qualified appliance evidence is valid; this validation does not authorize publication, promotion, routing, or activation",
    })
}

fn release_status(args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    let root = control_plane_root();
    let mut manifest = root.join("repos.manifest.toml");
    let mut output = None;
    let mut appliance_canary_aggregate = None;
    let mut appliance_canary_verifier_receipt = None;
    let mut token_file = None;
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--manifest" => manifest = PathBuf::from(iter.next().ok_or("--manifest needs a path")?),
            "--json" => output = Some(PathBuf::from(iter.next().ok_or("--json needs a path")?)),
            "--appliance-canary-aggregate" => {
                appliance_canary_aggregate = Some(PathBuf::from(
                    iter.next()
                        .ok_or("--appliance-canary-aggregate needs a path")?,
                ))
            }
            "--appliance-canary-verifier-receipt" => {
                appliance_canary_verifier_receipt = Some(PathBuf::from(
                    iter.next()
                        .ok_or("--appliance-canary-verifier-receipt needs a path")?,
                ))
            }
            "--token-file" => {
                token_file = Some(PathBuf::from(
                    iter.next().ok_or("--token-file needs a path")?,
                ))
            }
            value => return Err(format!("unknown release-status argument: {value}").into()),
        }
    }
    let data: toml::Value = fs::read_to_string(&manifest)?.parse()?;
    validate_manifest_data(&data, &manifest, false)?;
    let qualification = match (
        appliance_canary_aggregate.as_deref(),
        appliance_canary_verifier_receipt.as_deref(),
        token_file.as_deref(),
    ) {
        (None, None, None) => Ok(None),
        (Some(aggregate), Some(verifier), Some(token_file)) => {
            read_qualified_appliance_canary(aggregate, verifier, token_file).map(Some)
        }
        _ => Err(
            "release status requires the aggregate, verifier receipt, and token file together"
                .into(),
        ),
    };
    let (appliance_canary, blocked_reason) = match qualification {
        Ok(Some(qualification)) => (appliance_canary_summary(&qualification), None),
        Ok(None) => (
            json!({
                "status": "blocked",
                "qualification": false,
                "fixture": null,
                "aggregate_sha256": null,
                "verifier_receipt_sha256": null,
                "reason": "a qualified non-fixture CPU+GPU appliance aggregate and root-sealed verifier receipt are required",
            }),
            Some("a qualified non-fixture CPU+GPU appliance aggregate and root-sealed verifier receipt are required".to_owned()),
        ),
        Err(error) => {
            let reason = format!("appliance aggregate rejected: {error}");
            (
                json!({
                    "status": "blocked",
                    "qualification": false,
                    "fixture": null,
                    "aggregate_sha256": null,
                    "verifier_receipt_sha256": null,
                    "reason": reason,
                }),
                Some(reason),
            )
        }
    };
    let promotion_evidence_ready = blocked_reason.is_none();
    let report = json!({
        "schema_version": "jain.release.status/v1",
        "release": RELEASE_VERSION,
        "status": RELEASE_STATUS,
        "formal_ga": false,
        "sagemaker": "N/A",
        "rollback_target": ROLLBACK_TARGET,
        "manifest_sha256": manifest_sha256(&manifest)?,
        "family_repo_count": family_repos(&data)?.len(),
        "infrastructure_repo_count": data.get("infrastructure_repo").and_then(toml::Value::as_array).map_or(0, Vec::len),
        "appliance_canary": appliance_canary,
        "promotion_evidence_ready": promotion_evidence_ready,
        "production_promotion_authorized": false,
        "reason": if promotion_evidence_ready {
            "appliance qualification evidence is ready; status remains candidate and production activation still requires explicit owner authorization and all remaining release gates"
        } else {
            "promotion is blocked until a genuine qualified CPU+GPU appliance aggregate and root-sealed verifier receipt are supplied"
        },
    });
    write_or_print_json_report(output.as_deref(), &report)?;
    if let Some(reason) = blocked_reason {
        return Err(format!("release status blocked: {reason}").into());
    }
    Ok(())
}

fn validate_appliance_promotion(args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    let mut aggregate = None;
    let mut verifier_receipt = None;
    let mut token_file = None;
    let mut output = None;
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--aggregate" => {
                aggregate = Some(PathBuf::from(
                    iter.next().ok_or("--aggregate needs a path")?,
                ))
            }
            "--verifier-receipt" => {
                verifier_receipt = Some(PathBuf::from(
                    iter.next().ok_or("--verifier-receipt needs a path")?,
                ))
            }
            "--token-file" => {
                token_file = Some(PathBuf::from(
                    iter.next().ok_or("--token-file needs a path")?,
                ))
            }
            "--json" => output = Some(PathBuf::from(iter.next().ok_or("--json needs a path")?)),
            value => {
                return Err(
                    format!("unknown validate-appliance-promotion argument: {value}").into(),
                )
            }
        }
    }
    let qualification = read_qualified_appliance_canary(
        aggregate
            .as_deref()
            .ok_or("--aggregate is required for appliance promotion validation")?,
        verifier_receipt
            .as_deref()
            .ok_or("--verifier-receipt is required for appliance promotion validation")?,
        token_file
            .as_deref()
            .ok_or("--token-file is required for appliance promotion validation")?,
    )?;
    let report = appliance_promotion_report(&qualification);
    write_or_print_json_report(output.as_deref(), &report)
}

fn snapshot_rows(data: &toml::Value) -> Result<Vec<JsonValue>, Box<dyn std::error::Error>> {
    let mut rows = Vec::new();
    for raw in manifest_repos(data)? {
        let repo = repo_from(raw)?;
        let mut failures = Vec::new();
        if !repo.path.join(".git").exists() {
            failures.push("missing git checkout".to_owned());
        }
        let branch = git_query(&repo.path, &["branch", "--show-current"]);
        if branch.as_deref() != Some("main") {
            failures.push(format!("branch is {:?}, expected main", branch));
        }
        if git_query(
            &repo.path,
            &["status", "--porcelain", "--untracked-files=all"],
        )
        .is_some_and(|value| !value.is_empty())
        {
            failures.push("worktree is dirty".to_owned());
        }
        let origin = git_query(&repo.path, &["remote", "get-url", "origin"]);
        let expected = declared_remote(raw).unwrap_or_default();
        if origin.as_deref() != Some(expected.as_str()) {
            failures.push("origin does not match manifest".to_owned());
        }
        let tag = declared_release_tag(raw).unwrap_or_default();
        let commit = git_query(&repo.path, &["rev-parse", "HEAD"]);
        let tag_ref = format!("refs/tags/{tag}^{{}}");
        let tag_commit = git_query(&repo.path, &["rev-parse", &tag_ref]);
        if commit.is_none() || tag_commit != commit {
            failures.push(format!(
                "immutable tag {tag} is absent or does not point to HEAD"
            ));
        }
        rows.push(json!({"name": repo.name, "kind": raw.get("kind").and_then(toml::Value::as_str).unwrap_or("family"), "path": repo.path, "branch": branch, "commit": commit, "tag": tag, "tag_commit": tag_commit, "remote": origin, "status": if failures.is_empty() {"pass"} else {"fail"}, "failures": failures}));
    }
    Ok(rows)
}

fn bootstrap_main_command(args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    let mut repo = None;
    let mut remote = None;
    let mut reviewed_commit = None;
    let mut receipt = None;
    let mut apply = false;
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--repo" => repo = Some(PathBuf::from(iter.next().ok_or("--repo needs a path")?)),
            "--remote" => remote = Some(iter.next().ok_or("--remote needs a URL")?),
            "--reviewed-commit" => {
                reviewed_commit = Some(iter.next().ok_or("--reviewed-commit needs a SHA")?)
            }
            "--receipt" => {
                receipt = Some(PathBuf::from(iter.next().ok_or("--receipt needs a path")?))
            }
            "--apply" => apply = true,
            value => return Err(format!("unknown bootstrap-main argument: {value}").into()),
        }
    }
    let repo = repo.ok_or("bootstrap-main requires --repo")?;
    let remote = remote.ok_or("bootstrap-main requires --remote")?;
    let reviewed_commit = reviewed_commit.ok_or("bootstrap-main requires --reviewed-commit")?;
    let receipt = match receipt {
        Some(path) => path,
        None => {
            let name = repo
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("repository");
            release_evidence_path(&format!("bootstrap-main-{}.json", receipt_component(name)))
        }
    };
    let mut report = receipt_header("jain.bootstrap-main/v1", "bootstrap-main", apply);
    report["repository"] = json!(repo);
    report["remote"] = json!(remote);
    report["reviewed_commit_input"] = json!(reviewed_commit);
    let result = bootstrap_main(&repo, &remote, &reviewed_commit, apply, &mut report);
    finish_receipted_operation(&receipt, &mut report, result)
}

fn bootstrap_main(
    repo: &Path,
    remote: &str,
    reviewed_commit: &str,
    apply: bool,
    report: &mut JsonValue,
) -> Result<(), Box<dyn std::error::Error>> {
    let reviewed = resolve_commit(repo, reviewed_commit)?;
    report["reviewed_commit"] = json!(reviewed);
    let before = ls_remote_ref(repo, remote, "refs/heads/main")?;
    report["before"] = json!({"remote_main": before});
    match before {
        Some(existing) if existing == reviewed => {
            report["action"] = json!("verified-existing");
            report["after"] = json!({"remote_main": existing});
            Ok(())
        }
        Some(existing) => {
            report["action"] = json!("refused-existing-history");
            report["after"] = json!({"remote_main": existing});
            Err(format!(
                "refusing to replace existing remote main {existing} with reviewed commit {reviewed}"
            )
            .into())
        }
        None if !apply => {
            report["action"] = json!("would-create");
            report["after"] = json!({"remote_main": JsonValue::Null});
            Ok(())
        }
        None => {
            create_remote_main_cas(repo, remote, &reviewed, report)?;
            let after = ls_remote_ref(repo, remote, "refs/heads/main")?;
            report["action"] = json!("created");
            report["after"] = json!({"remote_main": after});
            if after.as_deref() == Some(reviewed.as_str()) {
                Ok(())
            } else {
                Err("remote main did not resolve to the reviewed commit after bootstrap".into())
            }
        }
    }
}

fn create_remote_main_cas(
    repo: &Path,
    remote: &str,
    reviewed: &str,
    report: &mut JsonValue,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(bare) = local_jeryu_bare_repo(remote)? {
        let output = Command::new("git")
            .args([
                "--git-dir",
                bare.to_str().ok_or("non-UTF8 Jeryu repository path")?,
                "update-ref",
                "refs/heads/main",
                reviewed,
                "0000000000000000000000000000000000000000",
            ])
            .output()?;
        if !output.status.success() {
            return Err(format!(
                "local Jeryu compare-and-swap bootstrap failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            )
            .into());
        }
        report["transport"] = json!("local-jeryu-server-update-ref-cas");
        return Ok(());
    }
    let refspec = format!("{reviewed}:refs/heads/main");
    run_git_strict(
        repo,
        &[
            "push",
            "--porcelain",
            "--force-with-lease=refs/heads/main:",
            remote,
            &refspec,
        ],
    )?;
    report["transport"] = json!("git-push-absent-lease");
    Ok(())
}

fn local_jeryu_bare_repo(remote: &str) -> Result<Option<PathBuf>, Box<dyn std::error::Error>> {
    let Some(slug) = remote
        .strip_prefix(&format!("{LOCAL_JERYU_ORIGIN}/git/"))
        .and_then(|value| value.strip_suffix(".git"))
    else {
        return Ok(None);
    };
    validate_jeryu_repo_slug(slug)?;
    let root = env::var_os("JERYU_GIT_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/home/ubuntu/.local/share/jeryu/git"));
    let path = root.join(format!("{slug}.git"));
    if !path.is_dir() {
        return Err(format!(
            "declared local Jeryu bare repository is missing: {}",
            path.display()
        )
        .into());
    }
    Ok(Some(path))
}

fn immutable_tag_command(args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    reject_legacy_jeryu_environment()?;
    let mut repo = None;
    let mut remote = None;
    let mut tag = None;
    let mut commit = None;
    let mut token_file = None;
    let mut receipt = None;
    let mut apply = false;
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--repo" => repo = Some(PathBuf::from(iter.next().ok_or("--repo needs a path")?)),
            "--remote" => remote = Some(iter.next().ok_or("--remote needs a URL")?),
            "--tag" => tag = Some(iter.next().ok_or("--tag needs a name")?),
            "--commit" => commit = Some(iter.next().ok_or("--commit needs a SHA")?),
            "--token-file" => {
                token_file = Some(PathBuf::from(
                    iter.next().ok_or("--token-file needs a path")?,
                ))
            }
            "--receipt" => {
                receipt = Some(PathBuf::from(iter.next().ok_or("--receipt needs a path")?))
            }
            "--apply" => apply = true,
            value => return Err(format!("unknown immutable-tag argument: {value}").into()),
        }
    }
    let control_root = control_plane_root();
    let split_root = control_root.parent().ok_or("splitctl root has no parent")?;
    let repo = validate_physical_git_checkout_beneath(
        &repo.ok_or("immutable-tag requires --repo")?,
        split_root,
    )?;
    let remote = remote.ok_or("immutable-tag requires --remote")?;
    let repo_slug = fixed_jeryu_git_slug(&remote)?;
    let tag = tag.ok_or("immutable-tag requires --tag")?;
    let commit = commit.ok_or("immutable-tag requires --commit")?;
    let token_file = token_file.ok_or("immutable-tag requires --token-file")?;
    let identity = authenticated_repository_identity(&repo_slug, &token_file)?;
    let receipt = match receipt {
        Some(path) => path,
        None => release_evidence_path(&format!("immutable-tag-{}.json", receipt_component(&tag))),
    };
    let mut report = receipt_header("jain.immutable-tag/v1", "immutable-tag", apply);
    report["repository"] = json!(repo);
    report["remote"] = json!(remote);
    report["api_identity"] = identity;
    report["token_metadata_validated"] = json!(true);
    report["tag"] = json!(tag);
    report["commit_input"] = json!(commit);
    let result = create_or_verify_immutable_tag(
        &repo,
        &remote,
        &tag,
        &commit,
        &token_file,
        apply,
        &mut report,
    );
    finish_receipted_operation(&receipt, &mut report, result)
}

fn create_or_verify_immutable_tag(
    repo: &Path,
    remote: &str,
    tag: &str,
    commit: &str,
    token_file: &Path,
    apply: bool,
    report: &mut JsonValue,
) -> Result<(), Box<dyn std::error::Error>> {
    if !is_full_sha(commit) || commit.chars().any(|ch| ch.is_ascii_uppercase()) {
        return Err("immutable-tag --commit must be a lowercase full 40-character SHA".into());
    }
    if secure_git_output(Some(repo), &["remote"])? != "origin"
        || secure_git_output(Some(repo), &["remote", "get-url", "origin"])? != remote
    {
        return Err("immutable-tag requires the sole exact canonical origin".into());
    }
    let tag_ref = format!("refs/tags/{tag}");
    secure_git_output(Some(repo), &["check-ref-format", &tag_ref])?;
    let reviewed = secure_git_output(
        Some(repo),
        &["rev-parse", "--verify", &format!("{commit}^{{commit}}")],
    )?;
    if !is_full_sha(&reviewed) || reviewed.chars().any(|ch| ch.is_ascii_uppercase()) {
        return Err("immutable tag commit did not resolve to a lowercase full SHA".into());
    }
    report["commit"] = json!(reviewed);
    let remote_main = secure_ls_remote_at(remote, "refs/heads/main", token_file)?;
    report["remote_main"] = json!(remote_main);
    if remote_main.as_deref() != Some(reviewed.as_str()) {
        report["action"] = json!("refused-non-main-tag");
        return Err(format!(
            "refusing to tag {reviewed}: remote main resolves to {}",
            remote_main.as_deref().unwrap_or("<absent>")
        )
        .into());
    }
    let local_before = secure_local_ref_commit(repo, &tag_ref)?;
    let remote_before = secure_ls_remote_at(remote, &tag_ref, token_file)?;
    report["before"] = json!({"local": local_before, "remote": remote_before});
    if let Some(existing) = local_before
        .as_ref()
        .filter(|existing| *existing != &reviewed)
    {
        report["action"] = json!("refused-local-tag-move");
        return Err(
            format!("refusing to move local tag {tag} from {existing} to {reviewed}").into(),
        );
    }
    if let Some(existing) = remote_before
        .as_ref()
        .filter(|existing| *existing != &reviewed)
    {
        report["action"] = json!("refused-remote-tag-move");
        return Err(
            format!("refusing to move remote tag {tag} from {existing} to {reviewed}").into(),
        );
    }
    if !apply {
        report["action"] = json!(match (local_before.is_some(), remote_before.is_some()) {
            (true, true) => "verified-existing",
            (false, false) => "would-create-local-and-remote",
            (false, true) => "would-create-local",
            (true, false) => "would-create-remote",
        });
        report["after"] = json!({"local": local_before, "remote": remote_before});
        return Ok(());
    }
    if remote_before.is_none() {
        let lease = format!("--force-with-lease={tag_ref}:");
        let refspec = format!("{reviewed}:{tag_ref}");
        secure_materialization_git_status(
            repo,
            remote,
            token_file,
            &["push", "--porcelain", &lease, remote, &refspec],
        )?;
    }
    let remote_after_push = secure_ls_remote_at(remote, &tag_ref, token_file)?;
    if remote_after_push.as_deref() != Some(reviewed.as_str()) {
        return Err("immutable remote tag compare-and-swap did not read back exactly".into());
    }
    if local_before.is_none()
        && !secure_git_status(
            Some(repo),
            &[
                "update-ref",
                &tag_ref,
                &reviewed,
                "0000000000000000000000000000000000000000",
            ],
        )?
    {
        return Err("immutable local tag compare-and-swap failed".into());
    }
    let local_after = secure_local_ref_commit(repo, &tag_ref)?;
    let remote_after = secure_ls_remote_at(remote, &tag_ref, token_file)?;
    let remote_main_after = secure_ls_remote_at(remote, "refs/heads/main", token_file)?;
    report["after"] = json!({
        "local": local_after,
        "remote": remote_after,
        "remote_main": remote_main_after,
    });
    if local_after.as_deref() != Some(reviewed.as_str())
        || remote_after.as_deref() != Some(reviewed.as_str())
    {
        return Err("immutable tag verification did not resolve to the reviewed commit".into());
    }
    if remote_main_after.as_deref() != Some(reviewed.as_str()) {
        report["action"] = json!("tag-retained-main-advanced");
        report["requires_next_unused_tag"] = json!(true);
        return Err(
            "remote main advanced during immutable tag CAS; retain this tag and repeat with the next unused suffix"
                .into(),
        );
    }
    report["action"] = json!(if local_before.is_some() && remote_before.is_some() {
        "verified-existing"
    } else {
        "created-and-verified"
    });
    Ok(())
}

fn verify_worktrees_command(args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    let root = control_plane_root();
    let mut manifest = root.join("repos.manifest.toml");
    let mut receipt = None;
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--manifest" => manifest = PathBuf::from(iter.next().ok_or("--manifest needs a path")?),
            "--receipt" => {
                receipt = Some(PathBuf::from(iter.next().ok_or("--receipt needs a path")?))
            }
            value => return Err(format!("unknown verify-worktrees argument: {value}").into()),
        }
    }
    let receipt = match receipt {
        Some(path) => path,
        None => release_evidence_path("release-worktree-verification.json"),
    };
    let mut report = receipt_header(
        "jain.release-worktree-verification/v1",
        "verify-worktrees",
        false,
    );
    report["manifest"] = json!(manifest);
    let result = (|| {
        let data: toml::Value = fs::read_to_string(&manifest)?.parse()?;
        let repositories = managed_repositories(&data, &manifest)?;
        let rows = repositories
            .iter()
            .map(verify_managed_worktree)
            .collect::<Vec<_>>();
        let status = if rows.iter().all(|row| row["status"] == "pass") {
            "pass"
        } else {
            "fail"
        };
        report["repository_count"] = json!(rows.len());
        report["repositories"] = json!(rows);
        if status == "pass" {
            Ok(())
        } else {
            Err("managed release worktree verification failed".into())
        }
    })();
    finish_receipted_operation(&receipt, &mut report, result)
}

#[cfg(test)]
#[derive(Debug, PartialEq, Eq)]
struct PrimaryRegistration {
    path: PathBuf,
    head: String,
    branch: String,
}

#[cfg(test)]
fn parse_single_primary_registration(
    porcelain: &[u8],
) -> Result<PrimaryRegistration, Box<dyn std::error::Error>> {
    if porcelain.is_empty() || !porcelain.ends_with(b"\0\0") {
        return Err("registration porcelain is empty or not NUL terminated".into());
    }
    let mut records = Vec::new();
    let mut fields = Vec::new();
    for field in porcelain[..porcelain.len() - 1].split(|byte| *byte == 0) {
        if field.is_empty() {
            if fields.is_empty() {
                return Err("registration porcelain contains an empty record".into());
            }
            records.push(std::mem::take(&mut fields));
        } else {
            fields.push(std::str::from_utf8(field)?.to_owned());
        }
    }
    if !fields.is_empty() || records.len() != 1 {
        return Err("registration porcelain must describe exactly one primary checkout".into());
    }
    let mut path = None;
    let mut head = None;
    let mut branch = None;
    for (index, field) in records.pop().unwrap().into_iter().enumerate() {
        let (name, value) = field
            .split_once(' ')
            .ok_or("registration porcelain field is malformed")?;
        if value.is_empty() {
            return Err("registration porcelain field has an empty value".into());
        }
        match name {
            "worktree" if index == 0 && path.is_none() => path = Some(PathBuf::from(value)),
            "HEAD" if head.is_none() && is_full_sha(value) => head = Some(value.to_owned()),
            "branch" if branch.is_none() && value.starts_with("refs/heads/") => {
                branch = Some(value.to_owned())
            }
            _ => {
                return Err("registration porcelain contains duplicate or unsupported state".into())
            }
        }
    }
    let path = path.ok_or("registration porcelain has no primary checkout path")?;
    if !path.is_absolute() {
        return Err("registered primary checkout path is not absolute".into());
    }
    Ok(PrimaryRegistration {
        path,
        head: head.ok_or("registration porcelain has no exact HEAD")?,
        branch: branch.ok_or("registration porcelain has no named branch")?,
    })
}

fn verify_managed_worktree(repo: &ManagedRepo) -> JsonValue {
    let mut failures = Vec::new();
    let branch = strict_git_output(&repo.path, &["branch", "--show-current"])
        .map_err(|error| failures.push(error.to_string()))
        .ok();
    if branch.as_deref() != Some(repo.branch.as_str()) {
        failures.push(format!("branch is {:?}, expected {}", branch, repo.branch));
    }
    let head = strict_git_output(&repo.path, &["rev-parse", "--verify", "HEAD^{commit}"])
        .map_err(|error| failures.push(error.to_string()))
        .ok();
    let dirty = strict_git_output(
        &repo.path,
        &["status", "--porcelain", "--untracked-files=all"],
    )
    .map(|value| !value.is_empty())
    .map_err(|error| failures.push(error.to_string()))
    .unwrap_or(true);
    if dirty {
        failures.push("worktree is dirty".to_owned());
    }

    let remotes = strict_git_output(&repo.path, &["remote"])
        .map(|value| value.lines().map(str::to_owned).collect::<Vec<_>>())
        .map_err(|error| failures.push(error.to_string()))
        .unwrap_or_default();
    if remotes != ["origin"] {
        failures.push(format!(
            "remotes are {:?}, expected exactly origin",
            remotes
        ));
    }
    let fetch_endpoints = strict_git_output(&repo.path, &["remote", "get-url", "--all", "origin"])
        .map(|value| value.lines().map(str::to_owned).collect::<Vec<_>>())
        .map_err(|error| failures.push(error.to_string()))
        .unwrap_or_default();
    let push_endpoints = strict_git_output(
        &repo.path,
        &["remote", "get-url", "--push", "--all", "origin"],
    )
    .map(|value| value.lines().map(str::to_owned).collect::<Vec<_>>())
    .map_err(|error| failures.push(error.to_string()))
    .unwrap_or_default();
    if fetch_endpoints != [repo.remote.clone()] || push_endpoints != [repo.remote.clone()] {
        failures.push(format!(
            "origin fetch/push endpoints must both be exactly {}",
            repo.remote
        ));
    }

    let tracking_main = local_ref_commit(&repo.path, "refs/remotes/origin/main")
        .map_err(|error| failures.push(error.to_string()))
        .ok()
        .flatten();
    let remote_main = ls_remote_ref(&repo.path, &repo.remote, "refs/heads/main")
        .map_err(|error| failures.push(error.to_string()))
        .ok()
        .flatten();
    if head.is_none() || tracking_main != head || remote_main != head {
        failures.push("HEAD, origin/main, and remote main are not equal".to_owned());
    }

    let dot_git = repo.path.join(".git");
    match fs::symlink_metadata(&dot_git) {
        Ok(metadata) if metadata.file_type().is_dir() && !metadata.file_type().is_symlink() => {}
        Ok(_) => {
            failures.push("primary checkout does not own a physical .git directory".to_owned())
        }
        Err(error) => failures.push(format!("cannot inspect primary .git directory: {error}")),
    }
    for forbidden in [
        dot_git.join("worktrees"),
        dot_git.join("commondir"),
        dot_git.join("objects/info/alternates"),
    ] {
        if fs::symlink_metadata(&forbidden).is_ok() {
            failures.push(format!(
                "forbidden auxiliary or shared Git metadata exists: {}",
                forbidden.display()
            ));
        }
    }
    let canonical_repo = fs::canonicalize(&repo.path)
        .map_err(|error| failures.push(format!("cannot canonicalize checkout: {error}")))
        .ok();
    if canonical_repo.as_deref() != Some(repo.path.as_path()) {
        failures.push("checkout path has a symlink, alias, or non-canonical component".to_owned());
    }
    let git_dir = strict_git_output(
        &repo.path,
        &["rev-parse", "--path-format=absolute", "--absolute-git-dir"],
    )
    .map_err(|error| failures.push(error.to_string()))
    .ok();
    let common_dir = strict_git_output(
        &repo.path,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
    )
    .map_err(|error| failures.push(error.to_string()))
    .ok();
    let expected_git_dir = dot_git.to_string_lossy();
    if git_dir.as_deref() != Some(expected_git_dir.as_ref())
        || common_dir.as_deref() != Some(expected_git_dir.as_ref())
    {
        failures.push(
            "checkout Git directory is auxiliary, shared, alternate, or outside the primary"
                .to_owned(),
        );
    }

    let (local_tag, remote_tag) = if let Some(tag) = &repo.tag {
        let tag_ref = format!("refs/tags/{tag}");
        let local = local_ref_commit(&repo.path, &tag_ref)
            .map_err(|error| failures.push(error.to_string()))
            .ok()
            .flatten();
        let remote = ls_remote_ref(&repo.path, &repo.remote, &tag_ref)
            .map_err(|error| failures.push(error.to_string()))
            .ok()
            .flatten();
        if head.is_none() || local != head || remote != head {
            failures.push(format!(
                "local and remote immutable tag {tag} must both resolve to HEAD"
            ));
        }
        (local, remote)
    } else {
        (None, None)
    };
    json!({
        "name": repo.name,
        "path": repo.path,
        "kind": repo.kind,
        "family": repo.family,
        "family_registered": repo.family_registered,
        "required_check": repo.required_check,
        "expected_branch": repo.branch,
        "branch": branch,
        "head": head,
        "expected_remote": repo.remote,
        "origin_fetch_endpoints": fetch_endpoints,
        "origin_push_endpoints": push_endpoints,
        "origin_main": tracking_main,
        "remote_main": remote_main,
        "tag": repo.tag,
        "local_tag": local_tag,
        "remote_tag": remote_tag,
        "dirty": dirty,
        "git_dir": git_dir,
        "git_common_dir": common_dir,
        "status": if failures.is_empty() {"pass"} else {"fail"},
        "failures": failures,
    })
}

fn receipt_header(schema: &str, operation: &str, apply: bool) -> JsonValue {
    json!({
        "schema_version": schema,
        "operation": operation,
        "mode": if apply {"apply"} else {"dry-run"},
        "timestamp_unix": SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_secs())
            .unwrap_or(0),
        "status": "pending",
    })
}

fn release_evidence_path(filename: &str) -> PathBuf {
    control_plane_root()
        .join("docs/release-evidence")
        .join(RELEASE_VERSION)
        .join(filename)
}

fn receipt_component(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '-'
            }
        })
        .collect()
}

fn finish_receipted_operation(
    receipt: &Path,
    report: &mut JsonValue,
    result: Result<(), Box<dyn std::error::Error>>,
) -> Result<(), Box<dyn std::error::Error>> {
    match &result {
        Ok(()) => report["status"] = json!("pass"),
        Err(error) => {
            report["status"] = json!("fail");
            report["error"] = json!(error.to_string());
        }
    }
    write_json_receipt(receipt, report)?;
    println!("wrote {}", receipt.display());
    result
}

fn finish_optional_evidence(
    evidence_out: Option<&Path>,
    report: &mut JsonValue,
    result: Result<(), Box<dyn std::error::Error>>,
) -> Result<(), Box<dyn std::error::Error>> {
    match &result {
        Ok(()) => report["status"] = json!("pass"),
        Err(error) => {
            report["status"] = json!("fail");
            report["error"] = json!(error.to_string());
        }
    }
    if let Some(path) = evidence_out {
        write_json_receipt(path, report)?;
        println!("wrote {}", path.display());
    } else {
        println!("{}", serde_json::to_string_pretty(report)?);
    }
    result
}

fn write_json_receipt(path: &Path, report: &JsonValue) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    let staging_path = path.with_extension(format!("tmp-{}", std::process::id()));
    let mut bytes = serde_json::to_vec_pretty(report)?;
    bytes.push(b'\n');
    fs::write(&staging_path, bytes)?;
    fs::rename(&staging_path, path)?;
    Ok(())
}

fn resolve_commit(repo: &Path, value: &str) -> Result<String, Box<dyn std::error::Error>> {
    strict_git_output(
        repo,
        &["rev-parse", "--verify", &format!("{value}^{{commit}}")],
    )
}

fn local_ref_commit(
    repo: &Path,
    reference: &str,
) -> Result<Option<String>, Box<dyn std::error::Error>> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["rev-parse", "--verify", &format!("{reference}^{{commit}}")])
        .output()?;
    if output.status.success() {
        Ok(Some(String::from_utf8(output.stdout)?.trim().to_owned()))
    } else {
        Ok(None)
    }
}

fn ls_remote_ref(
    repo: &Path,
    remote: &str,
    reference: &str,
) -> Result<Option<String>, Box<dyn std::error::Error>> {
    let peeled = format!("{reference}^{{}}");
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["ls-remote", remote, reference, &peeled])
        .output()?;
    if !output.status.success() {
        return Err(format!(
            "git ls-remote failed in {}: {}",
            repo.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        )
        .into());
    }
    let text = String::from_utf8(output.stdout)?;
    let mut direct = None;
    let mut dereferenced = None;
    for line in text.lines() {
        let mut fields = line.split_whitespace();
        let sha = fields.next().unwrap_or_default();
        let name = fields.next().unwrap_or_default();
        if name == peeled {
            dereferenced = Some(sha.to_owned());
        } else if name == reference {
            direct = Some(sha.to_owned());
        }
    }
    Ok(dereferenced.or(direct))
}

fn strict_git_output(repo: &Path, args: &[&str]) -> Result<String, Box<dyn std::error::Error>> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()?;
    if !output.status.success() {
        return Err(format!(
            "git {} failed in {}: {}",
            args.join(" "),
            repo.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        )
        .into());
    }
    Ok(String::from_utf8(output.stdout)?.trim().to_owned())
}

fn run_git_strict(repo: &Path, args: &[&str]) -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()?;
    if !output.status.success() {
        return Err(format!(
            "git {} failed in {}: {}",
            args.join(" "),
            repo.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        )
        .into());
    }
    Ok(())
}

fn fix_local_remotes(manifest: Option<PathBuf>) -> Result<(), Box<dyn std::error::Error>> {
    let root = control_plane_root();
    let path = manifest.unwrap_or_else(|| root.join("repos.manifest.toml"));
    let data: toml::Value = fs::read_to_string(&path)?.parse()?;
    for repo in managed_repositories(&data, &path)? {
        if repo.path.join(".git").exists() {
            canonicalize_remote(&repo.path, &repo.remote)?;
        }
    }
    Ok(())
}

fn install_worktree_ban_hooks(manifest: Option<PathBuf>) -> Result<(), Box<dyn std::error::Error>> {
    let root = control_plane_root();
    let hooks_dir = root.join("hooks");
    if !hooks_dir.join("pre-push").is_file() {
        return Err(format!(
            "shared hooks directory is missing pre-push: {}",
            hooks_dir.display()
        )
        .into());
    }
    let path = manifest.unwrap_or_else(|| root.join("repos.manifest.toml"));
    let data: toml::Value = fs::read_to_string(&path)?.parse()?;
    let hooks_path = hooks_dir.display().to_string();
    for repo in managed_repositories(&data, &path)? {
        if repo.path.join(".git").exists() {
            run_git_at(&repo.path, &["config", "core.hooksPath", &hooks_path])?;
        }
    }
    Ok(())
}

fn canonicalize_remote(path: &Path, expected: &str) -> Result<(), Box<dyn std::error::Error>> {
    let remotes = git_query(path, &["remote"]).unwrap_or_default();
    for remote in remotes.lines().filter(|name| *name != "origin") {
        run_git_at(path, &["remote", "remove", remote])?;
    }
    let origin = git_query(path, &["remote", "get-url", "origin"]);
    if origin.is_none() {
        run_git_at(path, &["remote", "add", "origin", expected])?;
    } else if origin.as_deref() != Some(expected) {
        run_git_at(path, &["remote", "set-url", "origin", expected])?;
    }
    Ok(())
}

fn run_git_at(root: &Path, args: &[&str]) -> Result<(), Box<dyn std::error::Error>> {
    let status = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("git command failed in {}", root.display()).into())
    }
}

fn jeryu_local(args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    reject_legacy_jeryu_environment()?;
    let command = args
        .first()
        .ok_or("jeryu-local needs a subcommand")?
        .as_str();
    if matches!(
        command,
        "pr-ready"
            | "pr-close"
            | "pr-approve"
            | "pr-merge"
            | "protection-apply"
            | "protection-readback"
    ) {
        return jeryu_lifecycle(args);
    }
    if command == "branch-push" {
        return jeryu_branch_push(args);
    }
    if command == "main-fetch" {
        return jeryu_main_fetch(args);
    }
    if command == "git-materialize" {
        return jeryu_git_materialize(args);
    }
    if command == "ref-readback" {
        return jeryu_ref_readback(args);
    }
    let json_output = args.iter().any(|arg| arg == "--json");
    let apply = args.iter().any(|arg| arg == "--apply");
    let value = |flag: &str| -> Result<String, Box<dyn std::error::Error>> {
        args.iter()
            .position(|arg| arg == flag)
            .and_then(|index| args.get(index + 1))
            .cloned()
            .ok_or_else(|| format!("{flag} needs a value").into())
    };
    let request = match command {
        "repo-list" => JeryuRequest::repo_list()?,
        "pr-list" => {
            let repo = value("--repo")?;
            JeryuRequest::pr_list(
                &repo,
                &value("--state").unwrap_or_else(|_| "open".to_owned()),
            )?
        }
        "pr-open" => {
            let repo = value("--repo")?;
            JeryuRequest::pr_open(
                &repo,
                &value("--title")?,
                &value("--head")?,
                &value("--expected-head")?,
                &value("--base").unwrap_or_else(|_| "main".to_owned()),
                &value("--body").unwrap_or_default(),
                args.iter().any(|arg| arg == "--draft"),
                &value("--actor").unwrap_or_else(|_| "codex".to_owned()),
            )?
        }
        "checks" => {
            let repo = value("--repo")?;
            JeryuRequest::checks(&repo, &value("--sha")?)?
        }
        _ => return Err(format!("unsupported jeryu-local command: {command}").into()),
    };
    if command == "pr-open" && !apply {
        let mut report = receipt_header("jain.jeryu-pr-open/v1", "jeryu-local pr-open", false);
        report["action"] = json!("would-apply");
        report["request"] = jeryu_request_json(&request);
        let evidence_out = value("--evidence-out").ok().map(PathBuf::from);
        return finish_optional_evidence(evidence_out.as_deref(), &mut report, Ok(()));
    }
    let token_file = PathBuf::from(
        value("--token-file")
            .map_err(|_| "Jeryu API operations require an explicit --token-file path")?,
    );
    let client = JeryuClient::from_token_file(&token_file)?;
    let response = client.execute(&request)?;
    if command == "pr-open" {
        let number = response
            .get("number")
            .and_then(JsonValue::as_u64)
            .ok_or("Jeryu PR-open response has no PR number")?;
        let readback = client.execute(&JeryuRequest::pr_details(&value("--repo")?, number)?)?;
        validate_pr_open_readback(
            &readback,
            number,
            &value("--head")?,
            &value("--expected-head")?,
            &value("--base").unwrap_or_else(|_| "main".to_owned()),
        )?;
        let mut report = receipt_header("jain.jeryu-pr-open/v1", "jeryu-local pr-open", true);
        report["action"] = json!("opened-and-verified");
        report["request"] = jeryu_request_json(&request);
        report["response"] = response;
        report["readback"] = readback;
        report["external_state_changed"] = json!(true);
        let evidence_out = value("--evidence-out").ok().map(PathBuf::from);
        return finish_optional_evidence(evidence_out.as_deref(), &mut report, Ok(()));
    }
    if json_output {
        println!("{}", serde_json::to_string_pretty(&response)?);
    } else {
        println!("{}", compact_json_output(&response));
    }
    Ok(())
}

fn validate_pr_open_readback(
    response: &JsonValue,
    number: u64,
    head: &str,
    expected_head: &str,
    base: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let valid = response.get("number").and_then(JsonValue::as_u64) == Some(number)
        && response.get("state").and_then(JsonValue::as_str) == Some("open")
        && response
            .get("head")
            .and_then(|value| value.get("ref"))
            .and_then(JsonValue::as_str)
            == Some(head)
        && response
            .get("head")
            .and_then(|value| value.get("sha"))
            .and_then(JsonValue::as_str)
            == Some(expected_head)
        && response
            .get("base")
            .and_then(|value| value.get("ref"))
            .and_then(JsonValue::as_str)
            == Some(base);
    if valid {
        Ok(())
    } else {
        Err("Jeryu PR readback does not exactly match the requested head, base, and state".into())
    }
}

fn reject_legacy_jeryu_environment() -> Result<(), Box<dyn std::error::Error>> {
    for (name, _) in env::vars_os() {
        if forbidden_jeryu_environment_name(&name.to_string_lossy()) {
            return Err(format!(
                "{} is forbidden for local Jeryu operations; use the fixed loopback origin and explicit token-file transport",
                name.to_string_lossy()
            )
            .into());
        }
    }
    Ok(())
}

fn forbidden_jeryu_environment_name(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    matches!(
        upper.as_str(),
        "JERYU_BASE"
            | "JERYU_MERGE_TOKEN"
            | "JERYU_MERGE_TOKEN_FILE"
            | "HTTP_PROXY"
            | "HTTPS_PROXY"
            | "ALL_PROXY"
            | "NO_PROXY"
            | "SSH_ASKPASS"
            | "SSH_ASKPASS_REQUIRE"
            | "GIT_CONFIG"
            | "GIT_CONFIG_COUNT"
            | "GIT_DIR"
            | "GIT_WORK_TREE"
            | "GIT_COMMON_DIR"
            | "GIT_OBJECT_DIRECTORY"
            | "GIT_ALTERNATE_OBJECT_DIRECTORIES"
            | "GIT_ASKPASS"
            | "GIT_PROXY_COMMAND"
            | "GIT_SSH"
            | "GIT_SSH_COMMAND"
            | "GIT_EXEC_PATH"
            | "GIT_TEMPLATE_DIR"
    ) || upper.starts_with("GIT_CONFIG_KEY_")
        || upper.starts_with("GIT_CONFIG_VALUE_")
}

fn jeryu_git_askpass(args: Vec<std::ffi::OsString>) -> Result<(), Box<dyn std::error::Error>> {
    let askpass = env::var_os("GIT_ASKPASS").ok_or("controlled Jeryu askpass has no executable")?;
    let askpass_path = Path::new(&askpass);
    let askpass_text = askpass_path
        .to_str()
        .ok_or("controlled Jeryu askpass path is not UTF-8")?;
    let descriptor = askpass_text
        .strip_prefix("/proc/self/fd/")
        .filter(|value| !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit()))
        .ok_or("controlled Jeryu askpass is not a pinned descriptor")?;
    let descriptor = descriptor.parse::<i32>()?;
    let askpass_metadata = fs::metadata(askpass_path)?;
    let running_metadata = fs::metadata("/proc/self/exe")?;
    if env::var(JERYU_ASKPASS_MODE).as_deref() != Ok("v1")
        || descriptor < 3
        || askpass_metadata.dev() != running_metadata.dev()
        || askpass_metadata.ino() != running_metadata.ino()
        || env::var_os("GIT_TERMINAL_PROMPT").as_deref() != Some(OsStr::new("0"))
        || args.len() != 1
    {
        return Err("invalid controlled Jeryu askpass invocation".into());
    }
    let prompt = args[0]
        .to_str()
        .ok_or("Jeryu askpass prompt is not UTF-8")?;
    let token_file = env::var_os(JERYU_ASKPASS_TOKEN_FILE);
    write_jeryu_askpass_response(prompt, token_file.as_deref(), &mut io::stdout().lock())
}

fn write_jeryu_askpass_response(
    prompt: &str,
    token_file: Option<&OsStr>,
    output: &mut impl Write,
) -> Result<(), Box<dyn std::error::Error>> {
    match prompt {
        "Username for 'http://127.0.0.1:8787': " => {
            writeln!(output, "{JERYU_GIT_USERNAME}")?;
            output.flush()?;
            Ok(())
        }
        "Password for 'http://x-access-token@127.0.0.1:8787': " => {
            let token_file = token_file.ok_or("controlled Jeryu askpass has no token file")?;
            write_token_for_askpass(Path::new(token_file), unsafe { libc::geteuid() }, output)?;
            Ok(())
        }
        _ => Err("Jeryu askpass rejected an unexpected prompt".into()),
    }
}

fn fixed_jeryu_git_remote(repo: &str) -> Result<String, Box<dyn std::error::Error>> {
    validate_jeryu_repo_slug(repo)?;
    Ok(format!("{LOCAL_JERYU_ORIGIN}/git/{repo}.git"))
}

fn fixed_jeryu_git_slug(remote: &str) -> Result<String, Box<dyn std::error::Error>> {
    let slug = remote
        .strip_prefix(&format!("{LOCAL_JERYU_ORIGIN}/git/"))
        .and_then(|value| value.strip_suffix(".git"))
        .ok_or("remote is not a fixed local Jeryu HTTP repository")?;
    validate_jeryu_repo_slug(slug)?;
    if fixed_jeryu_git_remote(slug)? != remote {
        return Err("remote is not the canonical fixed local Jeryu URL".into());
    }
    Ok(slug.to_owned())
}

fn validate_materialization_remote(
    repo: &str,
    remote: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if remote == fixed_jeryu_git_remote(repo)? {
        return Ok(());
    }
    #[cfg(debug_assertions)]
    {
        let path = Path::new(remote);
        if path.is_absolute()
            && fs::canonicalize(path).is_ok_and(|resolved| resolved == path)
            && fs::symlink_metadata(path).is_ok_and(|metadata| metadata.file_type().is_dir())
        {
            return Ok(());
        }
    }
    Err("Git materialization remote is not the fixed local Jeryu repository".into())
}

fn validate_heads_ref(reference: &str) -> Result<(), Box<dyn std::error::Error>> {
    let branch = reference
        .strip_prefix("refs/heads/")
        .ok_or("Git materialization requires an exact heads ref")?;
    validate_release_branch(branch)
}

fn parse_advertised_heads(
    output: &str,
    expected_head: &str,
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    if output.len() > 1024 * 1024 {
        return Err("Git materialization ref advertisement is oversized".into());
    }
    let mut matches = Vec::new();
    for line in output.lines() {
        let mut fields = line.split('\t');
        let sha = fields.next().unwrap_or_default();
        let reference = fields.next().unwrap_or_default();
        if fields.next().is_some()
            || !is_full_sha(sha)
            || sha.chars().any(|ch| ch.is_ascii_uppercase())
        {
            return Err("Git materialization ref advertisement is malformed".into());
        }
        validate_heads_ref(reference)?;
        if sha == expected_head {
            matches.push(reference.to_owned());
        }
    }
    matches.sort_unstable();
    matches.dedup();
    if matches.is_empty() {
        return Err("requested head is not an advertised product ref".into());
    }
    Ok(matches)
}

fn validate_ancestor_tag_ref(
    repo: &str,
    reference: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let repo_name = repo
        .split_once('/')
        .map(|(_, name)| name)
        .ok_or("ancestor-tag repository has no owner")?;
    let tag = reference
        .strip_prefix("refs/tags/")
        .ok_or("declared ancestor object must resolve through an exact tag ref")?;
    let release = tag
        .strip_prefix(&format!("{repo_name}-v"))
        .ok_or("declared ancestor tag does not belong to the repository")?;
    let Some((version, split)) = release.rsplit_once("-split.") else {
        return Err("declared ancestor tag is not an immutable split release".into());
    };
    if version.is_empty()
        || split.is_empty()
        || !split.bytes().all(|byte| byte.is_ascii_digit())
        || !version
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-'))
    {
        return Err("declared ancestor tag has an invalid immutable release identity".into());
    }
    secure_git_output(None, &["check-ref-format", reference])?;
    Ok(())
}

fn resolve_unique_advertised_ancestor_tag(
    repo: &str,
    output: &str,
    expected_object: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    if output.len() > 1024 * 1024 {
        return Err("ancestor-tag advertisement is oversized".into());
    }
    if !is_full_sha(expected_object) || expected_object.chars().any(|ch| ch.is_ascii_uppercase()) {
        return Err("declared ancestor tag object must be a lowercase full SHA".into());
    }
    let mut matches = Vec::new();
    let mut count = 0usize;
    for line in output.lines() {
        count = count
            .checked_add(1)
            .ok_or("ancestor-tag advertisement count overflow")?;
        if count > 4096 {
            return Err("ancestor-tag advertisement has too many refs".into());
        }
        let mut fields = line.split('\t');
        let object = fields.next().unwrap_or_default();
        let reference = fields.next().unwrap_or_default();
        if fields.next().is_some()
            || !is_full_sha(object)
            || object.chars().any(|ch| ch.is_ascii_uppercase())
        {
            return Err("ancestor-tag advertisement is malformed".into());
        }
        validate_ancestor_tag_ref(repo, reference)?;
        if object == expected_object {
            matches.push(reference.to_owned());
        }
    }
    matches.sort_unstable();
    matches.dedup();
    match matches.as_slice() {
        [reference] => Ok(reference.clone()),
        [] => Err("declared ancestor object is not an advertised immutable tag".into()),
        _ => Err("declared ancestor object resolves through multiple tag refs".into()),
    }
}

fn secure_ls_remote_at(
    remote: &str,
    reference: &str,
    token_file: &Path,
) -> Result<Option<String>, Box<dyn std::error::Error>> {
    if reference.starts_with("refs/heads/") {
        validate_heads_ref(reference)?;
    } else if reference.starts_with("refs/tags/") {
        secure_git_output(None, &["check-ref-format", reference])?;
    } else {
        return Err("remote readback requires an exact heads or tags ref".into());
    }
    let output = secure_materialization_git_output(
        remote,
        token_file,
        &["ls-remote", "--refs", remote, reference],
    )?;
    if output.is_empty() {
        return Ok(None);
    }
    let mut lines = output.lines();
    let line = lines.next().ok_or("missing ls-remote result")?;
    if lines.next().is_some() {
        return Err("fixed-origin readback returned duplicate refs".into());
    }
    let mut fields = line.split('\t');
    let sha = fields.next().unwrap_or_default();
    let found_ref = fields.next().unwrap_or_default();
    if fields.next().is_some()
        || found_ref != reference
        || !is_full_sha(sha)
        || sha.chars().any(|ch| ch.is_ascii_uppercase())
    {
        return Err("fixed-origin readback was malformed".into());
    }
    Ok(Some(sha.to_owned()))
}

fn secure_materialization_git_output(
    _remote: &str,
    token_file: &Path,
    args: &[&str],
) -> Result<String, Box<dyn std::error::Error>> {
    let mut command = secure_git_authenticated_command(None, token_file)?;
    #[cfg(debug_assertions)]
    if Path::new(_remote).is_absolute() {
        command.command.args(["-c", "protocol.file.allow=always"]);
    }
    let output = command.command.args(args).output()?;
    if !output.status.success() {
        return Err(format!("authenticated Git materialization {} failed", args[0]).into());
    }
    let stdout = std::str::from_utf8(&output.stdout)
        .map_err(|_| "authenticated Git materialization output was not UTF-8")?;
    Ok(stdout.trim().to_owned())
}

fn secure_materialization_git_status(
    repo: &Path,
    _remote: &str,
    token_file: &Path,
    args: &[&str],
) -> Result<(), Box<dyn std::error::Error>> {
    let mut command = secure_git_authenticated_command(Some(repo), token_file)?;
    #[cfg(debug_assertions)]
    if Path::new(_remote).is_absolute() {
        command.command.args(["-c", "protocol.file.allow=always"]);
    }
    let output = command.command.args(args).output()?;
    if !output.status.success() {
        return Err(format!("authenticated Git materialization {} failed", args[0]).into());
    }
    Ok(())
}

fn validate_materialization_object_tree(
    repo: &Path,
    revision: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let output = secure_git_command(Some(repo))
        .args([
            "ls-tree",
            "-r",
            "--full-tree",
            "--format=%(objectmode)",
            revision,
        ])
        .output()?;
    if !output.status.success() {
        return Err("Git materialization object-tree preflight failed".into());
    }
    let modes = std::str::from_utf8(&output.stdout)
        .map_err(|_| "Git materialization object-tree preflight was not UTF-8")?;
    for mode in modes.lines() {
        match mode {
            "100644" | "100755" | "160000" => {}
            "120000" => {
                return Err("Git materialization object tree contains a prohibited symlink".into())
            }
            _ => {
                return Err(format!(
                    "Git materialization object tree contains unsupported mode {mode:?}"
                )
                .into())
            }
        }
    }
    Ok(())
}

fn declared_standard_version_tag(
    repo: &str,
    destination: &Path,
) -> Result<Option<String>, Box<dyn std::error::Error>> {
    let repo_name = repo
        .split_once('/')
        .map(|(_, name)| name)
        .ok_or("declared release-tag repository has no owner")?;
    let path = destination.join("agent/standard-version.toml");
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    if !metadata.file_type().is_file()
        || metadata.file_type().is_symlink()
        || metadata.len() > 16 * 1024
        || metadata.nlink() != 1
    {
        return Err("declared release-tag metadata is not a bounded regular file".into());
    }
    let data: toml::Value = fs::read_to_string(&path)?.parse()?;
    if data.get("workspace").and_then(toml::Value::as_str) != Some(repo_name) {
        return Err("declared release-tag workspace differs from the repository".into());
    }
    let Some(tag) = data.get("version").and_then(toml::Value::as_str) else {
        return Err("declared release-tag metadata has no version".into());
    };
    let prefix = format!("{repo_name}-v");
    let Some(release) = tag.strip_prefix(&prefix) else {
        // Developer-only repositories may use a package version rather than an
        // immutable family tag. They receive no retained tag authority.
        return Ok(None);
    };
    let Some((version, suffix)) = release.rsplit_once("-split.") else {
        return Err("declared release tag has no immutable split suffix".into());
    };
    if version.is_empty()
        || suffix.is_empty()
        || !suffix.bytes().all(|byte| byte.is_ascii_digit())
        || !version
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-'))
        || !valid_cargo_cache_component(tag)
    {
        return Err("declared release tag is not a governed immutable tag".into());
    }
    let tag_ref = format!("refs/tags/{tag}");
    secure_git_output(None, &["check-ref-format", &tag_ref])?;
    Ok(Some(tag.to_owned()))
}

struct MaterializationDirectory {
    path: PathBuf,
    device: u64,
    inode: u64,
    keep: bool,
}

impl MaterializationDirectory {
    fn create(path: PathBuf) -> Result<Self, Box<dyn std::error::Error>> {
        if !path.is_absolute() || path.file_name().is_none() {
            return Err("Git materialization destination must be an absolute child path".into());
        }
        let parent = path
            .parent()
            .ok_or("Git materialization destination has no parent")?;
        if !fs::canonicalize(parent).is_ok_and(|resolved| resolved == parent)
            || fs::symlink_metadata(&path).is_ok()
        {
            return Err(
                "Git materialization destination must be new under a canonical parent".into(),
            );
        }
        fs::create_dir(&path)?;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700))?;
        let metadata = fs::symlink_metadata(&path)?;
        if !metadata.file_type().is_dir() || metadata.file_type().is_symlink() {
            return Err("Git materialization destination is not a physical directory".into());
        }
        Ok(Self {
            path,
            device: metadata.dev(),
            inode: metadata.ino(),
            keep: false,
        })
    }

    fn commit(mut self) {
        self.keep = true;
    }
}

impl Drop for MaterializationDirectory {
    fn drop(&mut self) {
        if self.keep {
            return;
        }
        let safe_to_remove = fs::symlink_metadata(&self.path).is_ok_and(|metadata| {
            metadata.file_type().is_dir()
                && !metadata.file_type().is_symlink()
                && metadata.dev() == self.device
                && metadata.ino() == self.inode
        });
        if safe_to_remove {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

fn jeryu_git_materialize(args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    let mut repo = None;
    let mut remote = None;
    let mut reference = None;
    let mut expected_head = None;
    let mut resolve_ref_head = false;
    let mut destination = None;
    let mut token_file = None;
    let mut retain_origin = false;
    let mut retain_declared_release_tag = false;
    let mut retain_ancestor_tag_object = None;
    let mut git_lfs_path = None;
    let mut git_lfs_sha256 = None;
    let mut iter = args.into_iter().skip(1);
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--repo" => repo = Some(iter.next().ok_or("--repo needs owner/name")?),
            "--remote" => remote = Some(iter.next().ok_or("--remote needs a value")?),
            "--ref" => reference = Some(iter.next().ok_or("--ref needs a value")?),
            "--expected-head" => {
                expected_head = Some(iter.next().ok_or("--expected-head needs a SHA")?)
            }
            "--resolve-ref-head" => resolve_ref_head = true,
            "--destination" => {
                destination = Some(PathBuf::from(
                    iter.next().ok_or("--destination needs a path")?,
                ))
            }
            "--token-file" => {
                token_file = Some(PathBuf::from(
                    iter.next().ok_or("--token-file needs a path")?,
                ))
            }
            "--git-lfs-path" => {
                git_lfs_path = Some(PathBuf::from(
                    iter.next().ok_or("--git-lfs-path needs a path")?,
                ))
            }
            "--git-lfs-sha256" => {
                git_lfs_sha256 = Some(iter.next().ok_or("--git-lfs-sha256 needs a digest")?)
            }
            "--retain-origin" => retain_origin = true,
            "--retain-declared-release-tag" => retain_declared_release_tag = true,
            "--retain-ancestor-tag-object" => {
                if retain_ancestor_tag_object.is_some() {
                    return Err("git-materialize accepts one ancestor tag object".into());
                }
                retain_ancestor_tag_object = Some(
                    iter.next()
                        .ok_or("--retain-ancestor-tag-object needs a SHA")?,
                );
            }
            value => return Err(format!("unknown git-materialize argument: {value}").into()),
        }
    }
    let repo = repo.ok_or("git-materialize requires --repo")?;
    let remote = remote.ok_or("git-materialize requires --remote")?;
    let destination = destination.ok_or("git-materialize requires --destination")?;
    let token_file = token_file.ok_or("git-materialize requires --token-file")?;
    validate_jeryu_repo_slug(&repo)?;
    validate_materialization_remote(&repo, &remote)?;
    drop(JeryuClient::from_token_file(&token_file)?);
    if resolve_ref_head && expected_head.is_some() {
        return Err("git-materialize accepts only one head authority mode".into());
    }
    if resolve_ref_head && reference.is_none() {
        return Err("--resolve-ref-head requires an exact --ref".into());
    }
    let expected_head = if resolve_ref_head {
        let exact_ref = reference.as_deref().expect("validated exact ref");
        validate_heads_ref(exact_ref)?;
        secure_ls_remote_at(&remote, exact_ref, &token_file)?
            .ok_or("authenticated Git materialization ref is absent")?
    } else {
        expected_head.ok_or("git-materialize requires --expected-head or --resolve-ref-head")?
    };
    if !is_full_sha(&expected_head) || expected_head.chars().any(|ch| ch.is_ascii_uppercase()) {
        return Err("--expected-head must be a lowercase full 40-character commit SHA".into());
    }
    let git_lfs = match (git_lfs_path, git_lfs_sha256) {
        (None, None) => None,
        (Some(path), Some(digest)) if repo == "veox/jain-starforge" => {
            Some(validate_pinned_git_lfs(&path, &digest)?)
        }
        (Some(_), Some(_)) => {
            return Err("git-lfs materialization is restricted to veox/jain-starforge".into())
        }
        _ => return Err("git-lfs path and SHA-256 must be supplied together".into()),
    };
    let reference = if let Some(reference) = reference {
        validate_heads_ref(&reference)?;
        reference
    } else {
        let advertised = secure_materialization_git_output(
            &remote,
            &token_file,
            &["ls-remote", "--refs", &remote, "refs/heads/*"],
        )?;
        parse_advertised_heads(&advertised, &expected_head)?
            .into_iter()
            .next()
            .ok_or("requested head is not an advertised product ref")?
    };
    if secure_ls_remote_at(&remote, &reference, &token_file)?.as_deref()
        != Some(expected_head.as_str())
    {
        return Err("Git materialization ref does not equal the expected head".into());
    }

    let destination_text = destination
        .to_str()
        .ok_or("Git materialization destination is not UTF-8")?;
    let created = MaterializationDirectory::create(destination.clone())?;
    if !secure_git_status(None, &["init", "--quiet", destination_text])? {
        return Err("Git materialization init failed".into());
    }
    secure_materialization_git_status(
        &destination,
        &remote,
        &token_file,
        &["fetch", "--quiet", "--no-tags", &remote, &reference],
    )?;
    if secure_git_output(
        Some(&destination),
        &["rev-parse", "--verify", "FETCH_HEAD^{commit}"],
    )? != expected_head
    {
        return Err("Git materialization fetched a different commit".into());
    }
    validate_materialization_object_tree(&destination, "FETCH_HEAD")?;
    if !secure_git_status(
        Some(&destination),
        &["checkout", "--quiet", "--detach", "FETCH_HEAD"],
    )? {
        return Err("Git materialization checkout failed".into());
    }
    if secure_git_output(
        Some(&destination),
        &["rev-parse", "--verify", "HEAD^{commit}"],
    )? != expected_head
    {
        return Err("Git materialization fetched a different commit".into());
    }
    let mut release_tag_ref = String::new();
    let mut release_tag_commit = String::new();
    if retain_declared_release_tag {
        if let Some(tag) = declared_standard_version_tag(&repo, &destination)? {
            let tag_ref = format!("refs/tags/{tag}");
            if let Some(advertised_tag) = secure_ls_remote_at(&remote, &tag_ref, &token_file)? {
                let tag_refspec = format!("{tag_ref}:{tag_ref}");
                secure_materialization_git_status(
                    &destination,
                    &remote,
                    &token_file,
                    &["fetch", "--quiet", "--no-tags", &remote, &tag_refspec],
                )?;
                let local_tag =
                    secure_git_output(Some(&destination), &["rev-parse", "--verify", &tag_ref])?;
                let local_commit = secure_git_output(
                    Some(&destination),
                    &["rev-parse", "--verify", &format!("{tag_ref}^{{commit}}")],
                )?;
                if local_tag != advertised_tag || local_commit != advertised_tag {
                    return Err(
                        "declared release tag is not an exact lightweight commit tag".into(),
                    );
                }
                validate_materialization_object_tree(&destination, &tag_ref)?;
                if !secure_git_status(
                    Some(&destination),
                    &["merge-base", "--is-ancestor", &local_commit, &expected_head],
                )? {
                    return Err("declared release tag is not an ancestor of product head".into());
                }
                if secure_ls_remote_at(&remote, &tag_ref, &token_file)?.as_deref()
                    != Some(advertised_tag.as_str())
                {
                    return Err("declared release tag moved during fetch".into());
                }
                release_tag_ref = tag_ref;
                release_tag_commit = local_commit;
            }
        }
    }
    let mut ancestor_tag_ref = String::new();
    let mut ancestor_tag_object = String::new();
    let mut ancestor_tag_commit = String::new();
    if let Some(requested_object) = retain_ancestor_tag_object.as_deref() {
        if !is_full_sha(requested_object)
            || requested_object.chars().any(|ch| ch.is_ascii_uppercase())
        {
            return Err("--retain-ancestor-tag-object must be a lowercase full SHA".into());
        }
        let advertised = secure_materialization_git_output(
            &remote,
            &token_file,
            &["ls-remote", "--refs", &remote, "refs/tags/*"],
        )?;
        let tag_ref = resolve_unique_advertised_ancestor_tag(&repo, &advertised, requested_object)?;
        let tag_refspec = format!("{tag_ref}:{tag_ref}");
        secure_materialization_git_status(
            &destination,
            &remote,
            &token_file,
            &["fetch", "--quiet", "--no-tags", &remote, &tag_refspec],
        )?;
        let local_object =
            secure_git_output(Some(&destination), &["rev-parse", "--verify", &tag_ref])?;
        if local_object != requested_object
            || secure_git_output(Some(&destination), &["cat-file", "-t", requested_object])?
                != "tag"
        {
            return Err("declared ancestor tag object changed during materialization".into());
        }
        let local_commit = secure_git_output(
            Some(&destination),
            &["rev-parse", "--verify", &format!("{tag_ref}^{{commit}}")],
        )?;
        if !secure_git_status(
            Some(&destination),
            &["merge-base", "--is-ancestor", &local_commit, &expected_head],
        )? {
            return Err(
                "declared ancestor tag does not peel to an ancestor of product head".into(),
            );
        }
        validate_materialization_object_tree(&destination, &local_commit)?;
        if secure_ls_remote_at(&remote, &tag_ref, &token_file)?.as_deref() != Some(requested_object)
        {
            return Err("declared ancestor tag moved during fetch".into());
        }
        ancestor_tag_ref = tag_ref;
        ancestor_tag_object = local_object;
        ancestor_tag_commit = local_commit;
    }
    let retained_tags = secure_git_output(
        Some(&destination),
        &["for-each-ref", "--format=%(refname)", "refs/tags"],
    )?;
    let mut expected_tags = [release_tag_ref.as_str(), ancestor_tag_ref.as_str()]
        .into_iter()
        .filter(|reference| !reference.is_empty())
        .collect::<Vec<_>>();
    expected_tags.sort_unstable();
    expected_tags.dedup();
    if retained_tags.lines().collect::<Vec<_>>() != expected_tags {
        return Err("Git materialization retained unexpected tag refs".into());
    }
    if let Some(git_lfs) = git_lfs.as_deref() {
        if !secure_git_status(Some(&destination), &["remote", "add", "origin", &remote])? {
            return Err("Git materialization could not configure its exact LFS origin".into());
        }
        hydrate_authenticated_lfs(&destination, &remote, &expected_head, &token_file, git_lfs)?;
    }
    if secure_ls_remote_at(&remote, &reference, &token_file)?.as_deref()
        != Some(expected_head.as_str())
    {
        return Err("Git materialization ref moved during fetch".into());
    }
    let materialized_status = if let Some(git_lfs) = git_lfs.as_deref() {
        secure_lfs_git_output(
            &destination,
            git_lfs,
            &["status", "--porcelain=v1", "--untracked-files=all"],
        )?
    } else {
        secure_git_output(
            Some(&destination),
            &["status", "--porcelain=v1", "--untracked-files=all"],
        )?
    };
    if !materialized_status.is_empty() {
        return Err("Git materialization checkout is not clean standalone authority".into());
    }
    if retain_origin {
        if (git_lfs.is_none()
            && !secure_git_status(Some(&destination), &["remote", "add", "origin", &remote])?)
            || secure_git_output(Some(&destination), &["remote", "get-url", "origin"])? != remote
        {
            return Err("Git materialization could not retain the reviewed origin".into());
        }
    } else {
        if git_lfs.is_some()
            && !secure_git_status(Some(&destination), &["remote", "remove", "origin"])?
        {
            return Err("Git materialization could not remove its LFS origin".into());
        }
        if !secure_git_output(Some(&destination), &["remote"])?.is_empty() {
            return Err("Git materialization retained an unexpected remote".into());
        }
    }
    created.commit();
    println!(
        "{}",
        serde_json::to_string(&json!({
            "schema_version": "jain.jeryu-git-materialization/v1",
            "repository": repo,
            "remote": remote,
            "reference": reference,
            "commit": expected_head,
            "destination": destination,
            "origin_retained": retain_origin,
            "lfs_hydrated": git_lfs.is_some(),
            "release_tag_ref": release_tag_ref,
            "release_tag_commit": release_tag_commit,
            "ancestor_tag_ref": ancestor_tag_ref,
            "ancestor_tag_object": ancestor_tag_object,
            "ancestor_tag_commit": ancestor_tag_commit,
            "status": "pass"
        }))?
    );
    Ok(())
}

fn jeryu_ref_readback(args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    let mut repo = None;
    let mut remote = None;
    let mut reference = None;
    let mut expected_head = None;
    let mut token_file = None;
    let mut iter = args.into_iter().skip(1);
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--repo" => repo = Some(iter.next().ok_or("--repo needs owner/name")?),
            "--remote" => remote = Some(iter.next().ok_or("--remote needs a value")?),
            "--ref" => reference = Some(iter.next().ok_or("--ref needs a value")?),
            "--expected-head" => {
                expected_head = Some(iter.next().ok_or("--expected-head needs a SHA")?)
            }
            "--token-file" => {
                token_file = Some(PathBuf::from(
                    iter.next().ok_or("--token-file needs a path")?,
                ))
            }
            value => return Err(format!("unknown ref-readback argument: {value}").into()),
        }
    }
    let repo = repo.ok_or("ref-readback requires --repo")?;
    let remote = remote.ok_or("ref-readback requires --remote")?;
    let expected_head = expected_head.ok_or("ref-readback requires --expected-head")?;
    let token_file = token_file.ok_or("ref-readback requires --token-file")?;
    validate_jeryu_repo_slug(&repo)?;
    validate_materialization_remote(&repo, &remote)?;
    drop(JeryuClient::from_token_file(&token_file)?);
    if !is_full_sha(&expected_head) || expected_head.chars().any(|ch| ch.is_ascii_uppercase()) {
        return Err("--expected-head must be a lowercase full 40-character commit SHA".into());
    }

    let advertised_refs = if let Some(reference) = reference {
        if reference.starts_with("refs/heads/") {
            validate_heads_ref(&reference)?;
        } else if reference.starts_with("refs/tags/") {
            secure_git_output(None, &["check-ref-format", &reference])?;
        } else {
            return Err("remote readback requires an exact heads or tags ref".into());
        }
        if secure_ls_remote_at(&remote, &reference, &token_file)?.as_deref()
            != Some(expected_head.as_str())
        {
            return Err("advertised ref does not equal the expected head".into());
        }
        vec![reference]
    } else {
        let advertised = secure_materialization_git_output(
            &remote,
            &token_file,
            &["ls-remote", "--refs", &remote, "refs/heads/*"],
        )?;
        parse_advertised_heads(&advertised, &expected_head)?
    };
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "schema_version": "jain.jeryu-ref-readback/v1",
            "status": "pass",
            "repository": repo,
            "remote": remote,
            "expected_head": expected_head,
            "advertised_refs": advertised_refs,
        }))?
    );
    Ok(())
}

fn secure_git_command(repo: Option<&Path>) -> Command {
    let mut command = Command::new("/usr/bin/git");
    command
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("LC_ALL", "C")
        .env("HOME", "/nonexistent")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_ATTR_NOSYSTEM", "1")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_ASKPASS", "/bin/false")
        .env("SSH_ASKPASS", "/bin/false")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_PROTOCOL_FROM_USER", "0")
        .args([
            "-c",
            "core.hooksPath=/dev/null",
            "-c",
            "core.fsmonitor=false",
            "-c",
            "core.attributesFile=/dev/null",
            "-c",
            "core.excludesFile=/dev/null",
            "-c",
            "diff.external=",
            "-c",
            "filter.lfs.process=",
            "-c",
            "filter.lfs.clean=",
            "-c",
            "filter.lfs.smudge=",
            "-c",
            "credential.helper=",
            "-c",
            "core.askPass=/bin/false",
            "-c",
            "http.proxy=",
            "-c",
            "https.proxy=",
            "-c",
            "protocol.allow=never",
            "-c",
            "protocol.http.allow=always",
        ]);
    if let Some(repo) = repo {
        command.arg("-C").arg(repo);
    }
    command
}

struct AuthenticatedGitCommand {
    command: Command,
    _askpass_executable: fs::File,
}

fn configure_jeryu_askpass(
    command: &mut Command,
    token_file: &Path,
) -> Result<fs::File, Box<dyn std::error::Error>> {
    if !token_file.is_absolute() {
        return Err("Jeryu token path must be absolute".into());
    }
    let executable = fs::File::open("/proc/self/exe")?;
    let metadata = executable.metadata()?;
    if !metadata.file_type().is_file()
        || metadata.uid() != unsafe { libc::geteuid() }
        || metadata.mode() & 0o111 == 0
    {
        return Err("splitctl executable is not a trusted current-owner regular file".into());
    }
    let descriptor = executable.as_raw_fd();
    // SAFETY: `descriptor` is owned by `executable`; clearing only CLOEXEC deliberately
    // pins these exact running bytes across the child chain until it invokes askpass.
    let descriptor_flags = unsafe { libc::fcntl(descriptor, libc::F_GETFD) };
    if descriptor_flags < 0
        || unsafe {
            libc::fcntl(
                descriptor,
                libc::F_SETFD,
                descriptor_flags & !libc::FD_CLOEXEC,
            )
        } < 0
    {
        return Err(format!(
            "cannot pin splitctl askpass descriptor: {}",
            io::Error::last_os_error()
        )
        .into());
    }
    command
        .env("GIT_ASKPASS", format!("/proc/self/fd/{descriptor}"))
        .env(JERYU_ASKPASS_MODE, "v1")
        .env(JERYU_ASKPASS_TOKEN_FILE, token_file);
    Ok(executable)
}

fn secure_git_authenticated_command(
    repo: Option<&Path>,
    token_file: &Path,
) -> Result<AuthenticatedGitCommand, Box<dyn std::error::Error>> {
    let mut command = secure_git_command(repo);
    let executable = configure_jeryu_askpass(&mut command, token_file)?;
    command.args([
        "-c",
        "credential.username=x-access-token",
        "-c",
        "credential.useHttpPath=false",
        "-c",
        "http.followRedirects=false",
        "-c",
        "http.maxRequests=1",
        "-c",
        "http.lowSpeedLimit=1",
        "-c",
        "http.lowSpeedTime=15",
    ]);
    Ok(AuthenticatedGitCommand {
        command,
        _askpass_executable: executable,
    })
}

fn validate_pinned_git_lfs(
    path: &Path,
    expected_sha256: &str,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    if !path.is_absolute() || !is_full_hex(expected_sha256, 64) {
        return Err("git-lfs authority requires an absolute path and SHA-256".into());
    }
    let canonical = fs::canonicalize(path)?;
    let metadata = fs::symlink_metadata(path)?;
    if canonical != Path::new("/usr/bin/git-lfs")
        || canonical != path
        || !metadata.file_type().is_file()
        || metadata.file_type().is_symlink()
        || (metadata.uid() != 0 && metadata.uid() != unsafe { libc::geteuid() })
        || metadata.mode() & 0o111 == 0
        || metadata.mode() & 0o022 != 0
        || metadata.nlink() != 1
        || sha256_regular_file(path, "git-lfs executable")? != expected_sha256
    {
        return Err("git-lfs executable digest or physical metadata mismatch".into());
    }
    let output = Command::new(&canonical)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("LC_ALL", "C")
        .env("HOME", "/nonexistent")
        .arg("version")
        .output()?;
    if !output.status.success()
        || std::str::from_utf8(&output.stdout)?.trim()
            != "git-lfs/3.4.1 (GitHub; linux amd64; go 1.22.2)"
    {
        return Err("git-lfs executable version mismatch".into());
    }
    Ok(canonical)
}

fn hydrate_authenticated_lfs(
    repo: &Path,
    remote: &str,
    expected_head: &str,
    token_file: &Path,
    git_lfs: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    if !secure_git_output(
        Some(repo),
        &["ls-tree", "--name-only", expected_head, ".lfsconfig"],
    )?
    .is_empty()
    {
        return Err("tracked .lfsconfig is forbidden in authenticated materialization".into());
    }
    let mut fetch = Command::new(git_lfs);
    let local_remote = cfg!(debug_assertions) && Path::new(remote).is_absolute();
    fetch
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("LC_ALL", "C")
        .env("HOME", "/nonexistent")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_ATTR_NOSYSTEM", "1")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_CONFIG_COUNT", if local_remote { "4" } else { "5" })
        .env("GIT_CONFIG_KEY_0", "credential.username")
        .env("GIT_CONFIG_VALUE_0", "x-access-token")
        .env("GIT_CONFIG_KEY_1", "credential.useHttpPath")
        .env("GIT_CONFIG_VALUE_1", "false")
        .env("GIT_CONFIG_KEY_2", "http.followRedirects")
        .env("GIT_CONFIG_VALUE_2", "false")
        .env("GIT_CONFIG_KEY_3", "http.maxRequests")
        .env("GIT_CONFIG_VALUE_3", "1")
        .current_dir(repo);
    if !local_remote {
        fetch
            .env("GIT_CONFIG_KEY_4", format!("lfs.{remote}/info/lfs.access"))
            .env("GIT_CONFIG_VALUE_4", "basic");
    }
    let _askpass = configure_jeryu_askpass(&mut fetch, token_file)?;
    let output = fetch.args(["fetch", "origin", expected_head]).output()?;
    if !output.status.success() {
        return Err("authenticated git-lfs fetch failed".into());
    }

    for args in [["checkout"].as_slice(), ["fsck"].as_slice()] {
        let filter_process = format!("{} filter-process", git_lfs.display());
        let filter_clean = format!("{} clean -- %f", git_lfs.display());
        let filter_smudge = format!("{} smudge -- %f", git_lfs.display());
        let output = Command::new(git_lfs)
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("LC_ALL", "C")
            .env("HOME", "/nonexistent")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_ATTR_NOSYSTEM", "1")
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GIT_CONFIG_COUNT", "4")
            .env("GIT_CONFIG_KEY_0", "filter.lfs.process")
            .env("GIT_CONFIG_VALUE_0", filter_process)
            .env("GIT_CONFIG_KEY_1", "filter.lfs.clean")
            .env("GIT_CONFIG_VALUE_1", filter_clean)
            .env("GIT_CONFIG_KEY_2", "filter.lfs.smudge")
            .env("GIT_CONFIG_VALUE_2", filter_smudge)
            .env("GIT_CONFIG_KEY_3", "filter.lfs.required")
            .env("GIT_CONFIG_VALUE_3", "true")
            .current_dir(repo)
            .args(args)
            .output()?;
        if !output.status.success() {
            return Err(format!("offline git-lfs {} failed", args[0]).into());
        }
    }
    Ok(())
}

fn secure_git_output(
    repo: Option<&Path>,
    args: &[&str],
) -> Result<String, Box<dyn std::error::Error>> {
    let output = secure_git_command(repo).args(args).output()?;
    if !output.status.success() {
        return Err(format!(
            "credentialless git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        )
        .into());
    }
    let stdout = std::str::from_utf8(&output.stdout)
        .map_err(|_| "credentialless git output was not UTF-8")?;
    Ok(stdout.trim().to_owned())
}

fn secure_lfs_git_output(
    repo: &Path,
    git_lfs: &Path,
    args: &[&str],
) -> Result<String, Box<dyn std::error::Error>> {
    let process = format!("filter.lfs.process={} filter-process", git_lfs.display());
    let clean = format!("filter.lfs.clean={} clean -- %f", git_lfs.display());
    let smudge = format!("filter.lfs.smudge={} smudge -- %f", git_lfs.display());
    let output = secure_git_command(Some(repo))
        .args([
            "-c",
            &process,
            "-c",
            &clean,
            "-c",
            &smudge,
            "-c",
            "filter.lfs.required=true",
        ])
        .args(args)
        .output()?;
    if !output.status.success() {
        return Err(format!(
            "credentialless pinned-LFS git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        )
        .into());
    }
    Ok(std::str::from_utf8(&output.stdout)?.trim().to_owned())
}

fn secure_git_authenticated_output(
    repo: Option<&Path>,
    token_file: &Path,
    args: &[&str],
) -> Result<String, Box<dyn std::error::Error>> {
    let mut command = secure_git_authenticated_command(repo, token_file)?;
    let output = command.command.args(args).output()?;
    if !output.status.success() {
        return Err(format!("authenticated fixed-origin Git {} failed", args[0]).into());
    }
    let stdout = std::str::from_utf8(&output.stdout)
        .map_err(|_| "authenticated fixed-origin Git output was not UTF-8")?;
    Ok(stdout.trim().to_owned())
}

fn secure_git_authenticated_status(
    repo: Option<&Path>,
    token_file: &Path,
    args: &[&str],
) -> Result<(), Box<dyn std::error::Error>> {
    let mut command = secure_git_authenticated_command(repo, token_file)?;
    let output = command.command.args(args).output()?;
    if !output.status.success() {
        return Err(format!("authenticated fixed-origin Git {} failed", args[0]).into());
    }
    Ok(())
}

fn secure_git_status(
    repo: Option<&Path>,
    args: &[&str],
) -> Result<bool, Box<dyn std::error::Error>> {
    Ok(secure_git_command(repo).args(args).status()?.success())
}

fn secure_local_ref_commit(
    repo: &Path,
    reference: &str,
) -> Result<Option<String>, Box<dyn std::error::Error>> {
    let output = secure_git_command(Some(repo))
        .args(["rev-parse", "--verify", &format!("{reference}^{{commit}}")])
        .output()?;
    if !output.status.success() {
        return Ok(None);
    }
    let value = std::str::from_utf8(&output.stdout)?.trim().to_owned();
    if !is_full_sha(&value) || value.chars().any(|ch| ch.is_ascii_uppercase()) {
        return Err("local ref did not resolve to a lowercase full SHA".into());
    }
    Ok(Some(value))
}

fn secure_ls_remote(
    repo: &str,
    reference: &str,
    token_file: &Path,
) -> Result<Option<String>, Box<dyn std::error::Error>> {
    if !reference.starts_with("refs/heads/")
        || reference.bytes().any(|byte| byte.is_ascii_control())
    {
        return Err("remote readback requires an exact heads ref".into());
    }
    let remote = fixed_jeryu_git_remote(repo)?;
    let output = secure_git_authenticated_output(
        None,
        token_file,
        &["ls-remote", "--refs", &remote, reference],
    )?;
    if output.is_empty() {
        return Ok(None);
    }
    let mut lines = output.lines();
    let line = lines.next().ok_or("missing ls-remote result")?;
    if lines.next().is_some() {
        return Err("fixed-origin readback returned duplicate refs".into());
    }
    let mut fields = line.split('\t');
    let sha = fields.next().unwrap_or_default();
    let found_ref = fields.next().unwrap_or_default();
    if fields.next().is_some()
        || found_ref != reference
        || !is_full_sha(sha)
        || sha.chars().any(|ch| ch.is_ascii_uppercase())
    {
        return Err("fixed-origin readback was malformed".into());
    }
    Ok(Some(sha.to_owned()))
}

fn is_full_sha(value: &str) -> bool {
    value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn validate_release_branch(value: &str) -> Result<(), Box<dyn std::error::Error>> {
    if value.is_empty()
        || value.starts_with('-')
        || value.ends_with(['.', '/'])
        || value.contains("..")
        || value.contains("@{")
        || value.contains("//")
        || value.contains('\\')
        || value.split('/').any(|part| {
            part.is_empty()
                || matches!(part, "." | "..")
                || part.ends_with(".lock")
                || part.bytes().any(|byte| {
                    byte.is_ascii_control()
                        || byte.is_ascii_whitespace()
                        || matches!(byte, b'~' | b'^' | b':' | b'?' | b'*' | b'[')
                })
        })
    {
        return Err("--branch must be a safe full branch name".into());
    }
    Ok(())
}

fn validate_physical_git_checkout_beneath(
    path: &Path,
    split_root: &Path,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    if !path.is_absolute() {
        return Err("--repo-path must be absolute".into());
    }
    let canonical = fs::canonicalize(path)?;
    if canonical != path {
        return Err("--repo-path must already be canonical".into());
    }
    if !canonical.starts_with(split_root) {
        return Err("--repo-path must remain beneath the split root".into());
    }
    let dot_git = canonical.join(".git");
    let metadata = fs::symlink_metadata(&dot_git)?;
    if !metadata.file_type().is_dir() || metadata.file_type().is_symlink() {
        return Err("checkout must own a physical .git directory".into());
    }
    for forbidden in [
        dot_git.join("commondir"),
        dot_git.join("worktrees"),
        dot_git.join("objects/info/alternates"),
    ] {
        if fs::symlink_metadata(&forbidden).is_ok() {
            return Err(format!(
                "checkout contains forbidden Git metadata: {}",
                forbidden.display()
            )
            .into());
        }
    }
    reject_local_git_injection(&canonical)?;
    let git_dir = secure_git_output(
        Some(&canonical),
        &["rev-parse", "--path-format=absolute", "--absolute-git-dir"],
    )?;
    let common_dir = secure_git_output(
        Some(&canonical),
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
    )?;
    let expected = dot_git.to_string_lossy();
    if git_dir != expected || common_dir != expected {
        return Err("checkout does not own an independent Git directory".into());
    }
    Ok(canonical)
}

fn reject_local_git_injection(repo: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let path = repo.join(".git/config");
    let metadata = fs::symlink_metadata(&path)?;
    if !metadata.file_type().is_file()
        || metadata.file_type().is_symlink()
        || metadata.nlink() != 1
        || metadata.len() > 1024 * 1024
    {
        return Err("local Git configuration is not a bounded independent regular file".into());
    }
    let text = std::str::from_utf8(&fs::read(&path)?)
        .map_err(|_| "local Git configuration is not UTF-8")?
        .to_owned();
    let mut section = None;
    let mut section_header = String::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with(['#', ';']) {
            continue;
        }
        if line.ends_with('\\') {
            return Err("local Git configuration continuations are forbidden".into());
        }
        if line.starts_with('[') {
            if !line.ends_with(']') {
                return Err("local Git configuration section is malformed".into());
            }
            let header = line[1..line.len() - 1].trim().to_ascii_lowercase();
            let name = header.split_ascii_whitespace().next().unwrap_or_default();
            if !matches!(name, "core" | "remote" | "branch" | "user" | "lfs") {
                return Err(format!(
                    "local Git configuration section is forbidden for branch publication: {name}"
                )
                .into());
            }
            section = Some(name.to_owned());
            section_header = header;
            continue;
        }
        let current = section
            .as_deref()
            .ok_or("local Git configuration key is outside a section")?;
        let (key, value) = line
            .split_once('=')
            .ok_or("local Git configuration key is malformed")?;
        let key = key.trim().to_ascii_lowercase();
        let value = value.trim();
        let allowed = match current {
            "core" => match key.as_str() {
                "repositoryformatversion" => value == "0",
                "filemode" | "logallrefupdates" => matches!(value, "true" | "false"),
                "bare" => value == "false",
                _ => false,
            },
            "remote" => match key.as_str() {
                "url" => {
                    value.starts_with("http://127.0.0.1:8787/git/")
                        && value.ends_with(".git")
                        && !value
                            .bytes()
                            .any(|byte| byte.is_ascii_control() || byte.is_ascii_whitespace())
                }
                "fetch" => {
                    value.starts_with("+refs/heads/")
                        && value.contains(":refs/remotes/")
                        && !value.bytes().any(|byte| byte.is_ascii_control())
                }
                _ => false,
            },
            "branch" => match key.as_str() {
                "remote" => value == "origin",
                "merge" => {
                    value.starts_with("refs/heads/")
                        && !value.bytes().any(|byte| byte.is_ascii_control())
                }
                _ => false,
            },
            "user" => {
                matches!(key.as_str(), "name" | "email")
                    && !value.bytes().any(|byte| byte.is_ascii_control())
            }
            "lfs" => {
                let repo_name = repo.file_name().and_then(OsStr::to_str).unwrap_or_default();
                if repo_name != "jain-starforge" {
                    false
                } else if section_header == "lfs" {
                    key == "repositoryformatversion" && value == "0"
                } else {
                    let expected_veox =
                        format!("lfs \"http://127.0.0.1:8787/git/veox/{repo_name}.git/info/lfs\"");
                    let expected_jeryu =
                        format!("lfs \"http://127.0.0.1:8787/git/jeryu/{repo_name}.git/info/lfs\"");
                    (section_header == expected_veox || section_header == expected_jeryu)
                        && key == "access"
                        && value == "basic"
                }
            }
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "local Git configuration key is forbidden for branch publication: {current}.{key}"
            )
            .into());
        }
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct MainFetchSnapshot {
    head: String,
    tree: String,
    branch: String,
    status: String,
    refs: BTreeMap<String, String>,
    remotes: String,
    origin_url: String,
    worktrees: String,
    config_sha256: String,
    index_sha256: String,
    fetch_head_sha256: Option<String>,
}

impl MainFetchSnapshot {
    fn json(&self) -> JsonValue {
        json!({
            "head": self.head,
            "tree": self.tree,
            "branch": self.branch,
            "status": self.status,
            "refs": self.refs,
            "remotes": self.remotes,
            "origin_url": self.origin_url,
            "worktrees": self.worktrees,
            "config_sha256": self.config_sha256,
            "index_sha256": self.index_sha256,
            "fetch_head_sha256": self.fetch_head_sha256,
        })
    }
}

fn optional_file_sha256(path: &Path) -> Result<Option<String>, Box<dyn std::error::Error>> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(sha256_bytes(&bytes))),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

fn exact_ref_snapshot(repo: &Path) -> Result<BTreeMap<String, String>, Box<dyn std::error::Error>> {
    let output = secure_git_output(
        Some(repo),
        &[
            "for-each-ref",
            "--format=%(refname)%09%(objectname)%09symref=%(symref)",
        ],
    )?;
    let mut refs = BTreeMap::new();
    for line in output.lines() {
        let mut fields = line.splitn(3, '\t');
        let reference = fields.next().ok_or("local ref snapshot is malformed")?;
        let object = fields.next().ok_or("local ref snapshot is malformed")?;
        let symbolic_target = fields
            .next()
            .and_then(|field| field.strip_prefix("symref="))
            .ok_or("local ref snapshot is malformed")?;
        if reference.is_empty()
            || !is_full_sha(object)
            || object.chars().any(|ch| ch.is_ascii_uppercase())
        {
            return Err("local ref snapshot contains an invalid ref or object".into());
        }
        let identity = if symbolic_target.is_empty() {
            object.to_owned()
        } else {
            secure_git_output(None, &["check-ref-format", symbolic_target])?;
            format!("symref:{symbolic_target}")
        };
        if refs.insert(reference.to_owned(), identity).is_some() {
            return Err("local ref snapshot contains a duplicate ref".into());
        }
    }
    Ok(refs)
}

fn main_fetch_snapshot(repo: &Path) -> Result<MainFetchSnapshot, Box<dyn std::error::Error>> {
    let dot_git = repo.join(".git");
    Ok(MainFetchSnapshot {
        head: secure_git_output(Some(repo), &["rev-parse", "--verify", "HEAD^{commit}"])?,
        tree: secure_git_output(Some(repo), &["rev-parse", "--verify", "HEAD^{tree}"])?,
        branch: secure_git_output(Some(repo), &["branch", "--show-current"])?,
        status: secure_git_output(
            Some(repo),
            &["status", "--porcelain=v1", "--untracked-files=all"],
        )?,
        refs: exact_ref_snapshot(repo)?,
        remotes: secure_git_output(Some(repo), &["remote"])?,
        origin_url: secure_git_output(Some(repo), &["remote", "get-url", "origin"])?,
        worktrees: secure_git_output(Some(repo), &["worktree", "list", "--porcelain"])?,
        config_sha256: sha256_bytes(&fs::read(dot_git.join("config"))?),
        index_sha256: sha256_bytes(&fs::read(dot_git.join("index"))?),
        fetch_head_sha256: optional_file_sha256(&dot_git.join("FETCH_HEAD"))?,
    })
}

fn validate_main_fetch_side_effects(
    before: &MainFetchSnapshot,
    after: &MainFetchSnapshot,
    expected_head: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut before_without_main = before.clone();
    let mut after_without_main = after.clone();
    before_without_main.refs.remove("refs/remotes/origin/main");
    after_without_main.refs.remove("refs/remotes/origin/main");
    if before_without_main != after_without_main {
        return Err(
            "main fetch changed checkout, configuration, worktree, tag, or non-main ref state"
                .into(),
        );
    }
    if after
        .refs
        .get("refs/remotes/origin/main")
        .map(String::as_str)
        != Some(expected_head)
    {
        return Err(
            "main fetch did not update origin/main to the authenticated expected head".into(),
        );
    }
    Ok(())
}

fn validate_repo_list_identity(
    response: &JsonValue,
    repo: &str,
) -> Result<JsonValue, Box<dyn std::error::Error>> {
    let (owner, name) = repo
        .split_once('/')
        .ok_or("repository identity needs owner/name")?;
    let rows = response
        .get("repositories")
        .and_then(JsonValue::as_array)
        .ok_or("Jeryu repository readback has no repositories array")?;
    let matches = rows
        .iter()
        .filter(|row| {
            row.pointer("/id/host").and_then(JsonValue::as_str) == Some("jeryu")
                && row.pointer("/id/owner").and_then(JsonValue::as_str) == Some(owner)
                && row.pointer("/id/name").and_then(JsonValue::as_str) == Some(name)
        })
        .collect::<Vec<_>>();
    if matches.len() != 1 {
        return Err("Jeryu repository identity readback is missing or ambiguous".into());
    }
    let row = matches[0];
    let expected_clone = format!("/git/{repo}.git");
    if row.get("default_branch").and_then(JsonValue::as_str) != Some("main")
        || row.get("clone_http_url").and_then(JsonValue::as_str) != Some(expected_clone.as_str())
    {
        return Err(
            "Jeryu repository identity readback has the wrong main branch or clone path".into(),
        );
    }
    Ok(json!({
        "host": "jeryu",
        "owner": owner,
        "name": name,
        "default_branch": "main",
        "clone_http_url": expected_clone,
    }))
}

fn authenticated_repository_identity(
    repo: &str,
    token_file: &Path,
) -> Result<JsonValue, Box<dyn std::error::Error>> {
    let client = JeryuClient::from_token_file(token_file)?;
    let repository_readback = client.execute(&JeryuRequest::repo_list()?)?;
    validate_repo_list_identity(&repository_readback, repo)
}

fn fetch_authenticated_main(
    repo: &Path,
    remote: &str,
    token_file: &Path,
    expected_head: Option<&str>,
    apply: bool,
    report: &mut JsonValue,
) -> Result<(), Box<dyn std::error::Error>> {
    let remote_head = secure_ls_remote_at(remote, "refs/heads/main", token_file)?
        .ok_or("authenticated remote main is absent")?;
    if let Some(expected) = expected_head {
        if !is_full_sha(expected)
            || expected.chars().any(|ch| ch.is_ascii_uppercase())
            || expected != remote_head
        {
            return Err("authenticated remote main differs from --expected-head".into());
        }
    }
    let before = main_fetch_snapshot(repo)?;
    report["before"] = before.json();
    report["remote_main"] = json!(remote_head);
    if !before.status.is_empty() {
        return Err("main fetch requires a clean canonical checkout".into());
    }
    if before.remotes != "origin" || before.origin_url != remote {
        return Err(
            "main fetch requires the sole exact canonical origin without rewriting it".into(),
        );
    }
    if !apply {
        report["action"] = json!(if before
            .refs
            .get("refs/remotes/origin/main")
            .map(String::as_str)
            == Some(remote_head.as_str())
        {
            "verified-existing"
        } else {
            "would-fetch-main"
        });
        report["after"] = before.json();
        return Ok(());
    }
    let expected_head = expected_head.ok_or("main-fetch --apply requires --expected-head")?;
    let refspec = "refs/heads/main:refs/remotes/origin/main";
    secure_materialization_git_status(
        repo,
        remote,
        token_file,
        &[
            "fetch",
            "--quiet",
            "--no-tags",
            "--no-prune",
            "--no-recurse-submodules",
            "--no-write-fetch-head",
            "--no-auto-maintenance",
            remote,
            refspec,
        ],
    )?;
    let after = main_fetch_snapshot(repo)?;
    report["after"] = after.json();
    report["external_state_changed"] = json!(before.refs != after.refs);
    validate_main_fetch_side_effects(&before, &after, expected_head)?;
    report["action"] = json!(if before.refs == after.refs {
        "verified-existing"
    } else {
        "fetched-and-verified"
    });
    Ok(())
}

fn jeryu_main_fetch(args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    let control_root = control_plane_root();
    let split_root = control_root.parent().ok_or("splitctl root has no parent")?;
    let mut repo = None;
    let mut repo_path = None;
    let mut expected_head = None;
    let mut token_file = None;
    let mut evidence_out = None;
    let mut apply = false;
    let mut iter = args.into_iter().skip(1);
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--repo" => repo = Some(iter.next().ok_or("--repo needs owner/name")?),
            "--repo-path" => {
                repo_path = Some(PathBuf::from(
                    iter.next().ok_or("--repo-path needs a path")?,
                ))
            }
            "--expected-head" => {
                expected_head = Some(iter.next().ok_or("--expected-head needs a SHA")?)
            }
            "--token-file" => {
                token_file = Some(PathBuf::from(
                    iter.next().ok_or("--token-file needs a path")?,
                ))
            }
            "--evidence-out" => {
                evidence_out = Some(PathBuf::from(
                    iter.next().ok_or("--evidence-out needs a path")?,
                ))
            }
            "--apply" => apply = true,
            value => return Err(format!("unknown main-fetch argument: {value}").into()),
        }
    }
    let repo = repo.ok_or("main-fetch requires --repo")?;
    validate_jeryu_repo_slug(&repo)?;
    let path = validate_physical_git_checkout_beneath(
        &repo_path.ok_or("main-fetch requires --repo-path")?,
        split_root,
    )?;
    let token_file = token_file.ok_or("main-fetch requires --token-file")?;
    if apply && expected_head.is_none() {
        return Err("main-fetch --apply requires --expected-head".into());
    }
    let remote = fixed_jeryu_git_remote(&repo)?;
    let identity = authenticated_repository_identity(&repo, &token_file)?;

    let mut report = receipt_header("jain.jeryu-main-fetch/v1", "jeryu-local main-fetch", apply);
    report["repository"] = json!(repo);
    report["repository_path"] = json!(path);
    report["remote"] = json!(remote);
    report["api_identity"] = identity;
    report["token_metadata_validated"] = json!(true);
    let result = fetch_authenticated_main(
        &path,
        &remote,
        &token_file,
        expected_head.as_deref(),
        apply,
        &mut report,
    );
    finish_optional_evidence(evidence_out.as_deref(), &mut report, result)
}

fn jeryu_branch_push(args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    let control_root = control_plane_root();
    let split_root = control_root
        .parent()
        .ok_or("splitctl root has no parent")?
        .to_path_buf();
    jeryu_branch_push_beneath(args, &split_root)
}

fn jeryu_branch_push_beneath(
    args: Vec<String>,
    split_root: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut repo = None;
    let mut repo_path = None;
    let mut branch = None;
    let mut expected_head = None;
    let mut token_file = None;
    let mut evidence_out = None;
    let mut apply = false;
    let mut iter = args.into_iter().skip(1);
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--repo" => repo = Some(iter.next().ok_or("--repo needs owner/name")?),
            "--repo-path" => {
                repo_path = Some(PathBuf::from(
                    iter.next().ok_or("--repo-path needs a path")?,
                ))
            }
            "--branch" => branch = Some(iter.next().ok_or("--branch needs a value")?),
            "--expected-head" => {
                expected_head = Some(iter.next().ok_or("--expected-head needs a SHA")?)
            }
            "--token-file" => {
                token_file = Some(PathBuf::from(
                    iter.next().ok_or("--token-file needs a path")?,
                ))
            }
            "--evidence-out" => {
                evidence_out = Some(PathBuf::from(
                    iter.next().ok_or("--evidence-out needs a path")?,
                ))
            }
            "--apply" => apply = true,
            value => return Err(format!("unknown branch-push argument: {value}").into()),
        }
    }
    let repo = repo.ok_or("branch-push requires --repo")?;
    let path = validate_physical_git_checkout_beneath(
        &repo_path.ok_or("branch-push requires --repo-path")?,
        split_root,
    )?;
    let branch = branch.ok_or("branch-push requires --branch")?;
    let expected_head = expected_head.ok_or("branch-push requires --expected-head")?;
    validate_jeryu_repo_slug(&repo)?;
    validate_release_branch(&branch)?;
    if !is_full_sha(&expected_head) || expected_head.chars().any(|ch| ch.is_ascii_uppercase()) {
        return Err("--expected-head must be a lowercase full 40-character commit SHA".into());
    }
    if secure_git_output(Some(&path), &["rev-parse", "--verify", "HEAD^{commit}"])? != expected_head
        || secure_git_output(Some(&path), &["branch", "--show-current"])? != branch
        || !secure_git_output(
            Some(&path),
            &["status", "--porcelain=v1", "--untracked-files=all"],
        )?
        .is_empty()
    {
        return Err("branch publication requires the clean named branch at the exact head".into());
    }
    let token_file = if apply {
        Some(token_file.ok_or("branch-push --apply requires --token-file")?)
    } else {
        None
    };

    let remote = fixed_jeryu_git_remote(&repo)?;
    let reference = format!("refs/heads/{branch}");
    let mut report = receipt_header(
        "jain.jeryu-branch-publication/v1",
        "jeryu-local branch-push",
        apply,
    );
    report["repository"] = json!(repo);
    report["repository_path"] = json!(path);
    report["branch"] = json!(branch);
    report["expected_head"] = json!(expected_head);
    report["remote"] = json!(remote);
    report["external_state_changed"] = json!(false);
    report["token_metadata_validated"] = json!(false);
    if let Some(token_file) = token_file.as_deref() {
        report["api_identity"] = authenticated_repository_identity(&repo, token_file)?;
        report["token_metadata_validated"] = json!(true);
    }
    let result = (|| {
        if !apply {
            report["action"] = json!("would-push-and-read-back");
            return Ok(());
        }
        let token_file = token_file
            .as_deref()
            .ok_or("branch-push --apply requires --token-file")?;
        let before = secure_ls_remote(&repo, &reference, token_file)?;
        report["before"] = json!(before);
        if let Some(before) = before.as_deref() {
            if before != expected_head
                && (!secure_git_status(
                    Some(&path),
                    &["cat-file", "-e", &format!("{before}^{{commit}}")],
                )? || !secure_git_status(
                    Some(&path),
                    &["merge-base", "--is-ancestor", before, &expected_head],
                )?)
            {
                return Err("branch publication is not a non-force fast-forward".into());
            }
        }
        if before.as_deref() != Some(expected_head.as_str()) {
            let refspec = format!("{expected_head}:{reference}");
            secure_git_authenticated_status(
                Some(&path),
                token_file,
                &["push", "--porcelain", "--no-verify", &remote, &refspec],
            )?;
            report["push_result"] = json!("authenticated-non-force-push-completed");
            report["external_state_changed"] = json!(true);
        }
        let after = secure_ls_remote(&repo, &reference, token_file)?;
        report["after"] = json!(after);
        if after.as_deref() != Some(expected_head.as_str()) {
            return Err("branch publication readback does not equal the exact head".into());
        }
        report["action"] = json!(if before.as_deref() == Some(expected_head.as_str()) {
            "verified-existing"
        } else {
            "pushed-and-verified"
        });
        Ok(())
    })();
    finish_optional_evidence(evidence_out.as_deref(), &mut report, result)
}

fn compact_json_output(value: &JsonValue) -> String {
    if value.is_null() {
        String::new()
    } else {
        value.to_string()
    }
}

struct HostCiPublishCommandError {
    publication_started: bool,
    message: String,
}

impl HostCiPublishCommandError {
    fn before(message: impl Into<String>) -> Self {
        Self {
            publication_started: false,
            message: message.into(),
        }
    }
}

fn jeryu_publish_host_ci(args: Vec<String>) -> Result<(), HostCiPublishCommandError> {
    reject_legacy_jeryu_environment()
        .map_err(|error| HostCiPublishCommandError::before(error.to_string()))?;
    let mut token_file = None;
    let mut repo = None;
    let mut head_sha = None;
    let mut required_check = None;
    let mut conclusion = None;
    let mut proof_summary = None;
    let mut proof_receipt_sha256 = None;
    let mut proof_attempt_id = None;
    let mut status_description = None;
    let mut apply = false;
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        let value = |iter: &mut std::vec::IntoIter<String>| {
            iter.next()
                .ok_or_else(|| HostCiPublishCommandError::before(format!("{arg} needs a value")))
        };
        match arg.as_str() {
            "--token-file" => token_file = Some(PathBuf::from(value(&mut iter)?)),
            "--repo" => repo = Some(value(&mut iter)?),
            "--head-sha" => head_sha = Some(value(&mut iter)?),
            "--required-check" => required_check = Some(value(&mut iter)?),
            "--conclusion" => conclusion = Some(value(&mut iter)?),
            "--proof-summary" => proof_summary = Some(value(&mut iter)?),
            "--proof-receipt-sha256" => proof_receipt_sha256 = Some(value(&mut iter)?),
            "--proof-attempt-id" => proof_attempt_id = Some(value(&mut iter)?),
            "--status-description" => status_description = Some(value(&mut iter)?),
            "--apply" => apply = true,
            value => {
                return Err(HostCiPublishCommandError::before(format!(
                    "unknown jeryu-publish-host-ci argument: {value}"
                )))
            }
        }
    }
    let required = |value: Option<String>, name: &str| {
        value.ok_or_else(|| HostCiPublishCommandError::before(format!("{name} is required")))
    };
    let publication = HostCiPublication {
        repo: required(repo, "--repo")?,
        head_sha: required(head_sha, "--head-sha")?,
        required_check: required(required_check, "--required-check")?,
        conclusion: required(conclusion, "--conclusion")?,
        proof_summary: required(proof_summary, "--proof-summary")?,
        proof_receipt_sha256: required(proof_receipt_sha256, "--proof-receipt-sha256")?,
        proof_attempt_id: required(proof_attempt_id, "--proof-attempt-id")?,
        status_description: required(status_description, "--status-description")?,
    };
    publication
        .validate()
        .map_err(|error| HostCiPublishCommandError::before(error.to_string()))?;
    if !apply {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "action": "would-publish",
                "repository": publication.repo,
                "head_sha": publication.head_sha,
                "required_check": publication.required_check,
                "conclusion": publication.conclusion,
            }))
            .map_err(|error| HostCiPublishCommandError::before(error.to_string()))?
        );
        return Ok(());
    }
    let token_file =
        token_file.ok_or_else(|| HostCiPublishCommandError::before("--token-file is required"))?;
    let client = JeryuClient::from_token_file(&token_file)
        .map_err(|error| HostCiPublishCommandError::before(error.to_string()))?;
    client
        .publish_host_ci(&publication)
        .map_err(|error| HostCiPublishCommandError {
            publication_started: error.publication_started(),
            message: error.to_string(),
        })
}

fn jeryu_lifecycle(args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    let command = args
        .first()
        .ok_or("jeryu lifecycle command is missing")?
        .clone();
    let mut repo = None;
    let mut number = None;
    let mut expected_head = None;
    let mut body = None;
    let mut branch = "main".to_owned();
    let mut required_check = None;
    let mut evidence_out = None;
    let mut token_file = None;
    let mut apply = false;
    let mut iter = args.into_iter().skip(1);
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--repo" => repo = Some(iter.next().ok_or("--repo needs owner/name")?),
            "--number" => number = Some(iter.next().ok_or("--number needs a value")?),
            "--expected-head" => {
                expected_head = Some(iter.next().ok_or("--expected-head needs a SHA")?)
            }
            "--body" => body = Some(iter.next().ok_or("--body needs a value")?),
            "--branch" => branch = iter.next().ok_or("--branch needs a value")?,
            "--required-check" => {
                required_check = Some(iter.next().ok_or("--required-check needs a value")?)
            }
            "--evidence-out" => {
                evidence_out = Some(PathBuf::from(
                    iter.next().ok_or("--evidence-out needs a path")?,
                ))
            }
            "--token-file" => {
                token_file = Some(PathBuf::from(
                    iter.next().ok_or("--token-file needs a path")?,
                ))
            }
            "--apply" => apply = true,
            value => return Err(format!("unknown {command} argument: {value}").into()),
        }
    }
    let repo = repo.ok_or("jeryu lifecycle command requires --repo")?;
    validate_jeryu_repo_slug(&repo)?;
    if branch != "main" {
        return Err("release branch protection may only target main".into());
    }
    let request = if command == "pr-approve" {
        plan_jeryu_approval_request(
            &repo,
            number.as_deref(),
            expected_head.as_deref(),
            body.as_deref(),
        )?
    } else {
        plan_jeryu_lifecycle_request(
            &command,
            &repo,
            number.as_deref(),
            expected_head.as_deref(),
            &branch,
            required_check.as_deref(),
        )?
    };
    let read_only = command == "protection-readback";
    if read_only && apply {
        return Err("protection-readback is read-only and does not accept --apply".into());
    }
    let mut report = receipt_header(
        "jain.jeryu-lifecycle/v1",
        &format!("jeryu-local {command}"),
        apply,
    );
    if read_only {
        report["mode"] = json!("read-only");
    }
    report["repository"] = json!(repo);
    report["request"] = jeryu_request_json(&request);
    let result = (|| {
        if !apply && !read_only {
            report["action"] = json!("would-apply");
            return Ok(());
        }
        let token_file = token_file
            .as_deref()
            .ok_or("Jeryu API operations require an explicit --token-file path")?;
        let client = JeryuClient::from_token_file(token_file)?;
        if command == "pr-merge" {
            let number = number
                .as_deref()
                .ok_or("missing PR number")?
                .parse::<u64>()?;
            let premerge = client.execute(&JeryuRequest::pr_details(&repo, number)?)?;
            validate_pr_open_readback(
                &premerge,
                number,
                premerge
                    .get("head")
                    .and_then(|value| value.get("ref"))
                    .and_then(JsonValue::as_str)
                    .ok_or("pre-merge PR readback has no head branch")?,
                expected_head.as_deref().ok_or("missing expected head")?,
                "main",
            )?;
            report["premerge_readback"] = premerge;
        }
        let response = client.execute(&request)?;
        report["response"] = response.clone();
        match command.as_str() {
            "pr-ready" if response.get("draft").and_then(JsonValue::as_bool) != Some(false) => {
                return Err("Jeryu PR ready readback still reports draft=true".into())
            }
            "pr-close" if response.get("state").and_then(JsonValue::as_str) != Some("closed") => {
                return Err("Jeryu PR close readback does not report state=closed".into())
            }
            "pr-approve" => {
                let number = number
                    .as_deref()
                    .ok_or("missing PR number")?
                    .parse::<u64>()?;
                let readback = JeryuRequest::pr_readback(&repo, number)?;
                let approval = client.execute(&readback)?;
                validate_approval_readback(
                    &approval,
                    expected_head.as_deref().ok_or("missing expected head")?,
                )?;
                report["readback"] = approval;
            }
            "pr-merge" => {
                let expected_head = expected_head.as_deref().ok_or("missing expected head")?;
                if response.get("merged").and_then(JsonValue::as_bool) != Some(true)
                    || response.get("sha").and_then(JsonValue::as_str) != Some(expected_head)
                {
                    return Err(
                        "Jeryu PR merge response does not prove the expected merged commit".into(),
                    );
                }
                let main = secure_ls_remote(&repo, "refs/heads/main", token_file)?;
                if main.as_deref() != Some(expected_head) {
                    return Err(
                        "protected main does not resolve to the expected merged commit".into(),
                    );
                }
                report["main_readback"] = json!(main);
            }
            "protection-apply" => {
                let readback = JeryuRequest::protection(&repo, &branch, None)?;
                let policy = client.execute(&readback)?;
                report["readback"] = policy.clone();
                validate_protection_policy(
                    &policy,
                    &repo,
                    &branch,
                    required_check.as_deref().ok_or("missing required check")?,
                )?;
            }
            "protection-readback" => validate_protection_policy(
                &response,
                &repo,
                &branch,
                required_check.as_deref().ok_or("missing required check")?,
            )?,
            _ => {}
        }
        report["action"] = json!(if read_only {
            "verified"
        } else {
            "applied-and-verified"
        });
        Ok(())
    })();
    finish_optional_evidence(evidence_out.as_deref(), &mut report, result)
}

fn plan_jeryu_approval_request(
    repo: &str,
    number: Option<&str>,
    expected_head: Option<&str>,
    body: Option<&str>,
) -> Result<JeryuRequest, Box<dyn std::error::Error>> {
    let number = number.ok_or("PR approval requires --number")?;
    let number = number
        .parse::<u64>()
        .map_err(|_| "--number must be a positive integer")?;
    if number == 0 {
        return Err("--number must be a positive integer".into());
    }
    let expected_head = expected_head.ok_or("PR approval requires --expected-head")?;
    if expected_head.len() != 40 || !expected_head.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return Err("--expected-head must be a full 40-character commit SHA".into());
    }
    JeryuRequest::pr_approval(repo, number, expected_head, body).map_err(Into::into)
}

fn validate_approval_readback(
    response: &JsonValue,
    expected_head: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let summary = response
        .get("summary")
        .ok_or("Jeryu approval readback is missing its PR summary")?;
    let head_matches = summary.get("head_sha").and_then(JsonValue::as_str) == Some(expected_head);
    let approvals = summary
        .get("review")
        .and_then(|review| review.get("approvals"))
        .and_then(JsonValue::as_u64)
        .unwrap_or(0);
    if head_matches && approvals >= 1 {
        Ok(())
    } else {
        Err("Jeryu approval readback does not prove approval of the expected head".into())
    }
}

fn plan_jeryu_lifecycle_request(
    command: &str,
    repo: &str,
    number: Option<&str>,
    expected_head: Option<&str>,
    branch: &str,
    required_check: Option<&str>,
) -> Result<JeryuRequest, Box<dyn std::error::Error>> {
    match command {
        "pr-ready" | "pr-close" | "pr-merge" => {
            let number = number.ok_or("PR lifecycle command requires --number")?;
            let number = number
                .parse::<u64>()
                .map_err(|_| "--number must be a positive integer")?;
            match command {
                "pr-ready" => JeryuRequest::pr_update(repo, number, json!({"draft": false})),
                "pr-close" => JeryuRequest::pr_update(repo, number, json!({"state": "closed"})),
                "pr-merge" => JeryuRequest::pr_merge(
                    repo,
                    number,
                    expected_head.ok_or("PR merge requires --expected-head")?,
                ),
                _ => unreachable!(),
            }
            .map_err(Into::into)
        }
        "protection-apply" | "protection-readback" => {
            let required_check =
                required_check.ok_or("protection command requires --required-check")?;
            if required_check.trim().is_empty() {
                return Err("--required-check must not be empty".into());
            }
            JeryuRequest::protection(
                repo,
                branch,
                if command == "protection-apply" {
                    Some(immutable_main_policy(required_check))
                } else {
                    None
                },
            )
            .map_err(Into::into)
        }
        value => Err(format!("unsupported Jeryu lifecycle command: {value}").into()),
    }
}

fn immutable_main_policy(required_check: &str) -> JsonValue {
    json!({
        "required_status_checks": [required_check],
        "required_approving_review_count": 1,
        "required_linear_history": true,
        "enforce_admins": true,
        "allow_force_pushes": false,
        "allow_deletions": false,
        "require_signed_commits": false,
        "require_jankurai_proof": false,
    })
}

fn validate_jeryu_repo_slug(repo: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut components = repo.split('/');
    let owner = components.next().unwrap_or_default();
    let name = components.next().unwrap_or_default();
    let safe_component = |value: &str| {
        !value.is_empty()
            && value != "."
            && value != ".."
            && !value.starts_with('.')
            && !value.ends_with('.')
            && value
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
    };
    if components.next().is_some() || !safe_component(owner) || !safe_component(name) {
        return Err("--repo must be a safe owner/name slug".into());
    }
    Ok(())
}

fn jeryu_request_json(request: &JeryuRequest) -> JsonValue {
    json!({
        "method": request.method(),
        "path": request.path(),
        "body": request.body().and_then(|body| serde_json::from_str::<JsonValue>(body).ok()),
    })
}

fn validate_protection_policy(
    policy: &JsonValue,
    repo: &str,
    branch: &str,
    required_check: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let object = policy
        .as_object()
        .ok_or("branch protection readback is not an object")?;
    let expected_keys = [
        "allow_deletions",
        "allow_force_pushes",
        "enforce_admins",
        "required_jankurai_proof",
        "required_linear_history",
        "required_pull_request_reviews",
        "required_signatures",
        "required_status_checks",
        "updated_at",
        "url",
    ];
    let exact_keys = |value: &JsonValue, keys: &[&str]| {
        value.as_object().is_some_and(|object| {
            object.len() == keys.len() && keys.iter().all(|key| object.contains_key(*key))
        })
    };
    let enabled = |name: &str, expected: bool| {
        policy.get(name).is_some_and(|value| {
            exact_keys(value, &["enabled"])
                && value.get("enabled").and_then(JsonValue::as_bool) == Some(expected)
        })
    };
    let checks = policy.get("required_status_checks");
    let reviews = policy.get("required_pull_request_reviews");
    let expected_url = format!("/repos/{repo}/branches/{branch}/protection");
    let url_valid = policy.get("url").and_then(JsonValue::as_str) == Some(expected_url.as_str());
    let updated_at_valid = policy
        .get("updated_at")
        .and_then(JsonValue::as_str)
        .is_some_and(|value| !value.is_empty() && value.len() <= 128);
    let valid = object.len() == expected_keys.len()
        && expected_keys.iter().all(|key| object.contains_key(*key))
        && checks.is_some_and(|value| {
            exact_keys(value, &["contexts", "strict"])
                && value.get("strict").and_then(JsonValue::as_bool) == Some(true)
                && value
                    .get("contexts")
                    .and_then(JsonValue::as_array)
                    .is_some_and(|contexts| {
                        contexts.len() == 1 && contexts[0].as_str() == Some(required_check)
                    })
        })
        && reviews.is_some_and(|value| {
            exact_keys(value, &["required_approving_review_count"])
                && value
                    .get("required_approving_review_count")
                    .and_then(JsonValue::as_u64)
                    == Some(1)
        })
        && enabled("required_linear_history", true)
        && enabled("enforce_admins", true)
        && enabled("allow_force_pushes", false)
        && enabled("allow_deletions", false)
        && enabled("required_signatures", false)
        && enabled("required_jankurai_proof", false)
        && url_valid
        && updated_at_valid;
    if valid {
        return Ok(());
    }
    Err("branch protection readback is not the exact immutable-main policy".into())
}

fn reconcile(args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    let root = control_plane_root();
    let mut manifest = root.join("repos.manifest.toml");
    let mut base_ref = None;
    let mut apply = false;
    let mut output = root.join("target/reconcile-report.json");
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--manifest" => manifest = PathBuf::from(iter.next().ok_or("--manifest needs a path")?),
            "--base-ref" => base_ref = Some(iter.next().ok_or("--base-ref needs a ref")?),
            "--apply" => apply = true,
            "--json" => output = PathBuf::from(iter.next().ok_or("--json needs a path")?),
            value => return Err(format!("unknown reconcile argument: {value}").into()),
        }
    }
    let data: toml::Value = fs::read_to_string(&manifest)?.parse()?;
    let source_root =
        PathBuf::from(string(&data, "source_root").ok_or("manifest missing source_root")?);
    let base_ref = base_ref
        .or_else(|| string(&data, "source_sha"))
        .ok_or("manifest missing source_sha")?;
    let diff = git_output(&source_root, &["diff", "--name-only", &base_ref, "HEAD"])?;
    let status = git_output(&source_root, &["status", "--porcelain"])?;
    let mut paths = std::collections::BTreeSet::new();
    paths.extend(
        diff.lines()
            .filter(|line| !line.is_empty())
            .map(ToOwned::to_owned),
    );
    for line in status.lines().filter(|line| line.len() > 3) {
        paths.insert(line[3..].trim().to_owned());
    }
    let repos = manifest_repos(&data)?;
    let mut items = Vec::new();
    for rel in paths {
        let owners: Vec<&toml::Value> = repos
            .iter()
            .copied()
            .filter(|repo| {
                strings(repo, "source_paths")
                    .iter()
                    .any(|pattern| path_matches(&rel, pattern))
            })
            .collect();
        if owners.is_empty() {
            items.push(json!({"path": rel, "disposition": "unmapped"}));
            continue;
        }
        if owners.len() > 1 {
            items.push(json!({"path": rel, "disposition": "ambiguous"}));
            continue;
        }
        let repo = owners[0];
        let name = string(repo, "name").ok_or("repo missing name")?;
        let destination =
            PathBuf::from(string(repo, "path").ok_or("repo missing path")?).join(&rel);
        let source = source_root.join(&rel);
        let disposition = if apply {
            copy_or_remove(&source, &destination)?;
            if source.exists() {
                "copied"
            } else {
                "deleted"
            }
        } else {
            "planned"
        };
        items.push(json!({"path": rel, "repo": name, "disposition": disposition}));
    }
    let report = json!({
        "schema_version": "jain.split.reconcile/v1",
        "source_root": source_root,
        "base_ref": base_ref,
        "apply": apply,
        "items": items,
    });
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&output, serde_json::to_vec_pretty(&report)?)?;
    println!("wrote {}", output.display());
    if report["items"].as_array().is_some_and(|items| {
        items
            .iter()
            .any(|item| item["disposition"] == "unmapped" || item["disposition"] == "ambiguous")
    }) {
        Err("reconcile contains unmapped or ambiguous paths".into())
    } else {
        Ok(())
    }
}

fn git_output(root: &Path, args: &[&str]) -> Result<String, Box<dyn std::error::Error>> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()?;
    if !output.status.success() {
        return Err(format!("git command failed in {}", root.display()).into());
    }
    Ok(String::from_utf8(output.stdout)?)
}

fn copy_or_remove(source: &Path, destination: &Path) -> io::Result<()> {
    if !source.exists() {
        if destination.is_dir() {
            fs::remove_dir_all(destination)?;
        } else if destination.exists() {
            fs::remove_file(destination)?;
        }
        return Ok(());
    }
    if source.is_dir() {
        if destination.exists() {
            fs::remove_dir_all(destination)?;
        }
        copy_dir(source, destination)
    } else {
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(source, destination).map(|_| ())
    }
}

fn copy_dir(source: &Path, destination: &Path) -> io::Result<()> {
    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let target = destination.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

fn bump_version(args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    let root = control_plane_root();
    let mut manifest = root.join("repos.manifest.toml");
    let mut from_version = None;
    let mut new = None;
    let mut update_lock = false;
    let mut rewrite_tags = false;
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--manifest" => manifest = PathBuf::from(iter.next().ok_or("--manifest needs a path")?),
            "--from" => from_version = Some(iter.next().ok_or("--from needs a version")?),
            "--new" => new = Some(iter.next().ok_or("--new needs a version")?),
            "--update-lock-shas" => update_lock = true,
            "--rewrite-split-tags" => rewrite_tags = true,
            value => return Err(format!("unknown bump-version argument: {value}").into()),
        }
    }
    let from_version = from_version.unwrap_or_else(|| "7.0.1".to_owned());
    if update_lock {
        return update_lock_shas(&manifest);
    }
    let new = new.ok_or("--new is required unless --update-lock-shas is used")?;
    if !rewrite_tags {
        return Err(
            "refusing to rewrite split tag pins by default; pass --rewrite-split-tags".into(),
        );
    }
    let data: toml::Value = fs::read_to_string(&manifest)?.parse()?;
    let from_tag = format!("v{from_version}-split.");
    let new_tag = format!("v{new}-split.0");
    replace_file(&manifest, |text| text.replace(&from_tag, &new_tag))?;
    for repo in manifest_repos(&data)? {
        let name = string(repo, "name").ok_or("repo missing name")?;
        let path = PathBuf::from(string(repo, "path").ok_or("repo missing path")?);
        let version_file = path.join("VERSION");
        if version_file.exists() {
            fs::write(version_file, format!("{name}-{new_tag}\n"))?;
        }
        rewrite_cargo_tree(&path, &from_version, &new)?;
        let changelog = path.join("CHANGELOG.md");
        if changelog.exists() {
            let text = fs::read_to_string(&changelog)?;
            if !text.contains(&new_tag) {
                fs::write(
                    changelog,
                    format!("## {name}-{new_tag}\n\n- Split-family release pin refresh.\n\n{text}"),
                )?;
            }
        }
    }
    for path in [
        root.join("../jain/repos.manifest.toml"),
        root.join("../jain-deploy/repos.manifest.toml"),
    ] {
        if path.exists() {
            replace_file(&path, |text| text.replace(&from_tag, &new_tag))?;
        }
    }
    Ok(())
}

fn replace_file(path: &Path, update: impl FnOnce(&str) -> String) -> io::Result<()> {
    let original = fs::read_to_string(path)?;
    let changed = update(&original);
    if changed != original {
        fs::write(path, changed)?;
        println!("changed: {}", path.display());
    }
    Ok(())
}

fn rewrite_cargo_tree(root: &Path, from_version: &str, new: &str) -> io::Result<()> {
    if !root.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(root)? {
        let path = entry?.path();
        if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| matches!(name, "target" | ".git" | ".stage" | "vendor"))
        {
            continue;
        }
        if path.is_dir() {
            rewrite_cargo_tree(&path, from_version, new)?;
        } else if path.file_name().and_then(|name| name.to_str()) == Some("Cargo.toml") {
            replace_file(&path, |text| {
                text.replace(
                    &format!("version = \"{from_version}\""),
                    &format!("version = \"{new}\""),
                )
                .replace(
                    &format!("v{from_version}-split."),
                    &format!("v{new}-split."),
                )
            })?;
        }
    }
    Ok(())
}

fn update_lock_shas(manifest: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let data: toml::Value = fs::read_to_string(manifest)?.parse()?;
    let mut updates = Vec::new();
    for repo in manifest_repos(&data)? {
        let name = string(repo, "name").ok_or("repo missing name")?;
        let path = PathBuf::from(string(repo, "path").ok_or("repo missing path")?);
        if path.join(".git").exists() {
            updates.push((
                name,
                git_output(&path, &["rev-parse", "HEAD"])?.trim().to_owned(),
            ));
        }
    }
    for lock in [
        PathBuf::from("../jain/family.lock"),
        PathBuf::from("../jain-deploy/jain-split.lock.toml"),
    ] {
        if !lock.exists() {
            continue;
        }
        replace_file(&lock, |text| {
            let mut output = text.to_owned();
            for (name, sha) in &updates {
                let marker = format!("repo = \"{name}\"");
                if let Some(start) = output.find(&marker) {
                    if let Some(commit) = output[start..].find("commit = \"") {
                        let begin = start + commit + 10;
                        if let Some(end) = output[begin..].find('"') {
                            output.replace_range(begin..begin + end, sha);
                        }
                    }
                }
            }
            output
        })?;
    }
    Ok(())
}

fn source_inventory_path(
    data: &toml::Value,
    manifest: &Path,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let relative = string(data, "source_inventory").ok_or("manifest missing source_inventory")?;
    if relative != "authority/source-paths.txt" {
        return Err("source_inventory must be authority/source-paths.txt".into());
    }
    let absolute_manifest = if manifest.is_absolute() {
        manifest.to_path_buf()
    } else {
        env::current_dir()?.join(manifest)
    };
    Ok(absolute_manifest
        .parent()
        .ok_or("manifest has no parent directory")?
        .join(relative))
}

fn declared_source_inventory_identity(
    data: &toml::Value,
) -> Result<(usize, String), Box<dyn std::error::Error>> {
    let count = data
        .get("source_inventory_count")
        .and_then(toml::Value::as_integer)
        .ok_or("source_inventory_count must be an integer")?;
    let count = usize::try_from(count)
        .ok()
        .filter(|count| *count > 0)
        .ok_or("source_inventory_count must be positive")?;
    let sha256 = string(data, "source_inventory_sha256")
        .ok_or("manifest missing source_inventory_sha256")?;
    if !is_full_hex(&sha256, 64) {
        return Err("source_inventory_sha256 must be 64 lowercase hex characters".into());
    }
    Ok((count, sha256))
}

fn validate_source_inventory_declaration(
    data: &toml::Value,
    manifest: &Path,
    check_file: bool,
) -> Result<(), String> {
    let source_sha = string(data, "source_sha").ok_or("source_sha is required")?;
    if !is_full_hex(&source_sha, 40) {
        return Err("source_sha must be 40 lowercase hex characters".to_owned());
    }
    source_inventory_path(data, manifest).map_err(|error| error.to_string())?;
    declared_source_inventory_identity(data).map_err(|error| error.to_string())?;
    if check_file {
        read_source_inventory(data, manifest).map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn parse_source_inventory(
    bytes: &[u8],
    source_sha: &str,
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    if bytes.is_empty() || !bytes.ends_with(b"\n") {
        return Err("source inventory must be non-empty and end with one newline".into());
    }
    let text = std::str::from_utf8(bytes)?;
    let expected_header = format!(
        "# Generated by: splitctl seal-source-inventory\n\
# DO NOT EDIT BY HAND\n\
# Source: immutable Git tree {source_sha}\n\
# Regenerate: cargo run --locked --quiet -- seal-source-inventory --manifest repos.manifest.toml --source-root SOURCE_ROOT --apply\n"
    );
    let paths = text
        .strip_prefix(&expected_header)
        .ok_or("source inventory is missing its exact generated provenance header")?;
    let mut files = Vec::new();
    let mut previous: Option<&str> = None;
    for path in paths.strip_suffix('\n').unwrap_or(paths).split('\n') {
        let normalized = Path::new(path);
        if path.is_empty()
            || path.contains('\r')
            || path.contains('\\')
            || normalized.is_absolute()
            || normalized
                .components()
                .any(|component| !matches!(component, std::path::Component::Normal(_)))
            || normalized.components().collect::<PathBuf>().as_os_str() != OsStr::new(path)
        {
            return Err(format!("source inventory path is not normalized: {path:?}").into());
        }
        if previous.is_some_and(|prior| prior >= path) {
            return Err(format!(
                "source inventory paths must be strictly sorted and unique: {path}"
            )
            .into());
        }
        previous = Some(path);
        files.push(path.to_owned());
    }
    Ok(files)
}

fn read_source_inventory(
    data: &toml::Value,
    manifest: &Path,
) -> Result<(PathBuf, Vec<String>, String), Box<dyn std::error::Error>> {
    let path = source_inventory_path(data, manifest)?;
    let metadata = physical_regular_file(&path, "source inventory")?;
    if metadata.nlink() != 1 {
        return Err("source inventory must have exactly one filesystem link".into());
    }
    let bytes = fs::read(&path)?;
    let source_sha = string(data, "source_sha").ok_or("manifest missing source_sha")?;
    let files = parse_source_inventory(&bytes, &source_sha)?;
    let actual_sha256 = sha256_bytes(&bytes);
    let (expected_count, expected_sha256) = declared_source_inventory_identity(data)?;
    if files.len() != expected_count || actual_sha256 != expected_sha256 {
        return Err(format!(
            "source inventory identity mismatch: expected count {expected_count} sha256 {expected_sha256}, got count {} sha256 {actual_sha256}",
            files.len()
        )
        .into());
    }
    Ok((path, files, actual_sha256))
}

fn render_source_inventory(
    files: &[String],
    source_sha: &str,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut normalized = files.to_vec();
    normalized.sort();
    normalized.dedup();
    let mut bytes = format!(
        "# Generated by: splitctl seal-source-inventory\n\
# DO NOT EDIT BY HAND\n\
# Source: immutable Git tree {source_sha}\n\
# Regenerate: cargo run --locked --quiet -- seal-source-inventory --manifest repos.manifest.toml --source-root SOURCE_ROOT --apply\n"
    )
    .into_bytes();
    bytes.extend_from_slice(normalized.join("\n").as_bytes());
    bytes.push(b'\n');
    parse_source_inventory(&bytes, source_sha)?;
    Ok(bytes)
}

fn seal_source_inventory_command(args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    let root = control_plane_root();
    let mut manifest = root.join("repos.manifest.toml");
    let mut source_root = None;
    let mut apply = false;
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--manifest" => manifest = PathBuf::from(iter.next().ok_or("--manifest needs a path")?),
            "--source-root" => {
                source_root = Some(PathBuf::from(
                    iter.next().ok_or("--source-root needs a path")?,
                ))
            }
            "--apply" => apply = true,
            value => return Err(format!("unknown seal-source-inventory argument: {value}").into()),
        }
    }
    let source_root = source_root.ok_or("--source-root is required")?;
    physical_directory(&source_root, "source inventory Git input")?;
    let data: toml::Value = fs::read_to_string(&manifest)?.parse()?;
    validate_source_inventory_declaration(&data, &manifest, false)?;
    let source_sha = string(&data, "source_sha").ok_or("manifest missing source_sha")?;
    let files = git_files(&source_root, &source_sha)?;
    let bytes = render_source_inventory(&files, &source_sha)?;
    let actual_count = files.len();
    let actual_sha256 = sha256_bytes(&bytes);
    let (expected_count, expected_sha256) = declared_source_inventory_identity(&data)?;
    if actual_count != expected_count || actual_sha256 != expected_sha256 {
        return Err(format!(
            "immutable source tree does not match the declared inventory identity: actual count {actual_count}, actual sha256 {actual_sha256}"
        )
        .into());
    }
    let output = source_inventory_path(&data, &manifest)?;
    let action = if fs::read(&output).ok().as_deref() == Some(bytes.as_slice()) {
        "verified"
    } else if apply {
        if output.exists() {
            physical_regular_file(&output, "existing source inventory")?;
        } else {
            let parent = output
                .parent()
                .ok_or("source inventory output has no parent")?;
            if parent.exists() {
                physical_directory(parent, "source inventory output directory")?;
            } else {
                physical_directory(
                    parent
                        .parent()
                        .ok_or("source inventory output has no control-plane parent")?,
                    "source inventory control plane",
                )?;
                fs::create_dir(parent)?;
                physical_directory(parent, "source inventory output directory")?;
            }
        }
        write_atomic_bytes(&output, &bytes)?;
        "written"
    } else {
        "would-write"
    };
    if apply {
        read_source_inventory(&data, &manifest)?;
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "schema_version": "jain.split.source-inventory/v1",
            "source_sha": source_sha,
            "source_root": source_root,
            "output": output,
            "count": actual_count,
            "sha256": actual_sha256,
            "apply": apply,
            "action": action,
        }))?
    );
    Ok(())
}

fn source_coverage(manifest: &Path, json_output: bool) -> Result<(), Box<dyn std::error::Error>> {
    let data: toml::Value = fs::read_to_string(manifest)?.parse()?;
    let source_root =
        PathBuf::from(string(&data, "source_root").ok_or("manifest missing source_root")?);
    let source_sha = string(&data, "source_sha").ok_or("manifest missing source_sha")?;
    let (source_inventory, files, source_inventory_sha256) =
        read_source_inventory(&data, manifest)?;
    let retired = strings(&data, "retired_paths");
    let shared = strings(&data, "shared_source_paths");
    let repos = manifest_repos(&data)?;
    let mut missing = Vec::new();
    let mut duplicates = Map::new();
    let mut owned = 0usize;
    let mut retired_count = 0usize;
    let mut shared_count = 0usize;
    for path in &files {
        let mut owners = Vec::new();
        for repo in repos.iter().copied() {
            let name = string(repo, "name").unwrap_or_else(|| "<unknown>".to_owned());
            for pattern in strings(repo, "source_paths") {
                if path_matches(path, &pattern) {
                    owners.push(name.clone());
                    break;
                }
            }
        }
        let is_retired = retired.iter().any(|pattern| path_matches(path, pattern));
        let is_shared = shared.iter().any(|pattern| path_matches(path, pattern));
        let classes =
            usize::from(!owners.is_empty()) + usize::from(is_retired) + usize::from(is_shared);
        if classes == 0 {
            missing.push(path.clone());
        } else if owners.len() > 1 || classes > 1 {
            let mut labels = owners;
            if is_retired {
                labels.push("retired".to_owned());
            }
            if is_shared {
                labels.push("shared".to_owned());
            }
            duplicates.insert(path.clone(), json!(labels));
        } else if !owners.is_empty() {
            owned += 1;
        } else if is_retired {
            retired_count += 1;
        } else {
            shared_count += 1;
        }
    }
    let report = json!({
        "schema_version": "jain.split.source-coverage/v1",
        "source_root": source_root,
        "source_sha": source_sha,
        "source_inventory": source_inventory,
        "source_inventory_sha256": source_inventory_sha256,
        "tracked_files": files.len(),
        "owned_count": owned,
        "retired_count": retired_count,
        "shared_count": shared_count,
        "missing_count": missing.len(),
        "duplicate_count": duplicates.len(),
        "missing": missing,
        "duplicates": duplicates,
        "status": if missing.is_empty() && duplicates.is_empty() { "pass" } else { "fail" },
    });
    if json_output {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else if report["status"] == "pass" {
        println!(
            "source coverage pass: {} tracked files; {} owned, {} retired, {} shared",
            files.len(),
            owned,
            retired_count,
            shared_count
        );
    } else {
        eprintln!(
            "source coverage failed: {} missing, {} duplicate assignments",
            missing.len(),
            duplicates.len()
        );
    }
    if report["status"] == "pass" {
        Ok(())
    } else {
        Err("source coverage failed".into())
    }
}

fn python_boundary_exception(relative: &str) -> Option<&'static str> {
    match relative {
        "jain-router/tools/refit_export_c08.py" => Some("frozen-offline-parity-oracle"),
        "jain-smartcluster/jope/ten_guest_activation_receipt.py" => {
            Some("temporary-protected-rust-port-cycle")
        }
        _ => None,
    }
}

fn python_scan_excluded_name(name: &str) -> bool {
    matches!(
        name,
        ".bundles" | ".git" | "target" | ".stage" | ".venv" | "vendor" | "node_modules"
    )
}

fn python_boundary() -> Result<(), Box<dyn std::error::Error>> {
    let root = control_plane_root()
        .parent()
        .ok_or("split root unavailable")?
        .to_path_buf();
    let mut files = Vec::new();
    collect_python(&root, &mut files)?;
    let mut unexpected = Vec::new();
    let mut declared = Vec::new();
    for path in files {
        let rel = path
            .strip_prefix(&root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        let allowed = rel.starts_with("jain-model-zoo/ops/parity/")
            || rel.starts_with("redline-split/")
            || rel == "redline-split-ops/scripts/redline_proof.py"
            || rel == "redline-split-ops/tests/test_redline_proof.py"
            || rel.contains("/parity/")
            || rel.contains("/oracle/")
            || rel.starts_with("jain-deploy/ops/ci/testdata/")
            || rel.starts_with("jain-python/python/ai-service/examples/")
            || rel.starts_with("jain-python/python/ai-service/src/")
            || rel.starts_with("jain-python/python/ai-service/tests/")
            || python_boundary_exception(&rel).is_some();
        if allowed {
            declared.push(rel);
        } else {
            unexpected.push(rel);
        }
    }
    if !unexpected.is_empty() {
        return Err(format!(
            "unexpected Python outside declared parity/customer boundary:\n{}",
            unexpected.join("\n")
        )
        .into());
    }
    println!(
        "python boundary ok: {} declared files (customer SDK, parity, and nested Redline proof only)",
        declared.len()
    );
    Ok(())
}

fn preflight(args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    let root = control_plane_root();
    let mut manifest = root.join("repos.manifest.toml");
    let mut output = root.join("target/preflight-report.json");
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--manifest" => manifest = PathBuf::from(iter.next().ok_or("--manifest needs a path")?),
            "--json" => output = PathBuf::from(iter.next().ok_or("--json needs a path")?),
            value => return Err(format!("unknown preflight argument: {value}").into()),
        }
    }

    let data: toml::Value = fs::read_to_string(&manifest)?.parse()?;
    let split_root =
        PathBuf::from(string(&data, "split_root").ok_or("manifest missing split_root")?);
    let repos = manifest_repos(&data)?;
    let required: std::collections::BTreeSet<_> =
        strings(&data, "required_repos").into_iter().collect();
    let excluded = data
        .get("excluded_path")
        .and_then(toml::Value::as_array)
        .map(|items| items.iter().collect::<Vec<_>>())
        .unwrap_or_default();
    let excluded_names = excluded
        .iter()
        .filter_map(|item| string(item, "name"))
        .collect::<std::collections::BTreeSet<_>>();
    let mut names = std::collections::BTreeSet::new();
    let mut paths = std::collections::BTreeSet::new();
    let mut rows = Vec::new();
    for raw in &repos {
        let repo = repo_from(raw)?;
        let mut failures = Vec::new();
        let path_key = repo.path.display().to_string();
        if !names.insert(repo.name.clone()) {
            failures.push("duplicate repository name".to_owned());
        }
        if !paths.insert(path_key.clone()) {
            failures.push("duplicate repository path".to_owned());
        }
        let expected_path = split_root.join(&repo.name);
        if repo.path != expected_path {
            failures.push(format!(
                "repository path must be {}, found {}",
                expected_path.display(),
                repo.path.display()
            ));
        }

        if raw.get("kind").and_then(toml::Value::as_str) == Some("required-infrastructure") {
            if string(raw, "forge_owner").as_deref() != Some("veox") {
                failures.push("infrastructure forge_owner must be veox".to_owned());
            }
            if string(raw, "forge_slug").as_deref() != Some("veox/jain-smartcluster") {
                failures.push("infrastructure forge_slug does not match the manifest".to_owned());
            }
            if raw.get("family_registered").and_then(toml::Value::as_bool) != Some(true) {
                failures.push("required infrastructure must be family registered".to_owned());
            }
            if raw.get("required").and_then(toml::Value::as_bool) != Some(true) {
                failures.push("infrastructure dependency must be required".to_owned());
            }
            if strings(raw, "dependency_edges").is_empty() {
                failures.push("infrastructure dependency_edges must not be empty".to_owned());
            }
        }

        let checkout = repo.path.join(".git").exists();
        if !checkout {
            failures.push("missing git checkout".to_owned());
        }
        let branch = git_query(&repo.path, &["branch", "--show-current"]);
        let commit = git_query(&repo.path, &["rev-parse", "HEAD"]);
        let dirty_output = git_query_with_status(
            &repo.path,
            &["status", "--porcelain", "--untracked-files=all"],
        );
        let dirty = dirty_output.as_ref().is_some_and(|value| !value.is_empty());
        let untracked = dirty_output
            .as_ref()
            .is_some_and(|value| value.lines().any(|line| line.starts_with("?? ")));
        if branch.as_deref() != Some("main") {
            failures.push(format!("branch is {:?}, expected main", branch));
        }
        if dirty {
            failures.push("worktree is dirty".to_owned());
        }
        let expected_remote = declared_remote(raw).unwrap_or_default();
        let origin = git_query(&repo.path, &["remote", "get-url", "origin"]);
        let remotes = git_query(&repo.path, &["remote"])
            .unwrap_or_default()
            .lines()
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>();
        if origin.as_deref() != Some(expected_remote.as_str()) || remotes != ["origin"] {
            failures
                .push("remote policy does not resolve to exactly local-Jeryu origin".to_owned());
        }

        let tag = declared_release_tag(raw).unwrap_or_default();
        let tag_arg = format!("refs/tags/{tag}^{{}}");
        let tag_commit = git_query(&repo.path, &["rev-parse", tag_arg.as_str()]);
        let tag_ok = commit.is_some() && tag_commit == commit;
        if !tag_ok {
            failures.push(format!(
                "immutable tag {tag:?} is absent or does not point at HEAD"
            ));
        }

        let generated = [
            "AGENTS.md",
            "agent/owner-map.json",
            "agent/test-map.json",
            "agent/generated-zones.toml",
            "agent/proof-lanes.toml",
        ];
        let missing_generated = generated
            .iter()
            .filter(|relative| !repo.path.join(relative).is_file())
            .map(|relative| (*relative).to_owned())
            .collect::<Vec<_>>();
        if !missing_generated.is_empty() {
            failures.push(format!(
                "missing generated-zone inputs: {}",
                missing_generated.join(", ")
            ));
        }

        let dependency_resolution = if repo.path.join("Cargo.toml").is_file() {
            let cargo = fs::read_to_string(repo.path.join("Cargo.toml")).unwrap_or_default();
            let lock = fs::read_to_string(repo.path.join("Cargo.lock")).unwrap_or_default();
            let mut modes = Vec::new();
            if cargo.contains("path =") {
                modes.push("path");
            }
            if cargo.contains("git =") || lock.contains("source = \"git+") {
                modes.push("git");
            }
            if lock.contains("source = \"registry+") {
                modes.push("registry");
            }
            modes
        } else {
            Vec::new()
        };
        rows.push(json!({
            "name": repo.name,
            "path": repo.path,
            "kind": raw.get("kind").and_then(toml::Value::as_str).unwrap_or("family"),
            "branch": branch,
            "commit": commit,
            "tag": tag,
            "tag_commit": tag_commit,
            "remote": origin,
            "dirty": dirty,
            "untracked": untracked,
            "generated": missing_generated.is_empty(),
            "dependency_resolution": dependency_resolution,
            "status": if failures.is_empty() { "pass" } else { "fail" },
            "failures": failures,
        }));
    }

    let mut manifest_failures = Vec::new();
    if string(&data, "release_version").as_deref() != Some(RELEASE_VERSION) {
        manifest_failures.push(format!("release_version must be {RELEASE_VERSION}"));
    }
    let family_names = family_repos(&data)?
        .iter()
        .filter_map(|raw| string(raw, "name"))
        .collect::<std::collections::BTreeSet<_>>();
    if family_names != required {
        manifest_failures.push(format!(
            "required_repos differs from family [[repo]] names (required {}, repos {})",
            required.len(),
            family_names.len()
        ));
    }
    let control = data.get("control_plane");
    let control_name = control.and_then(|value| string(value, "name"));
    if control_name.as_deref() != Some("jain-split-ops") {
        manifest_failures.push("control_plane.name must be jain-split-ops".to_owned());
    }
    for expected in [
        "jain-tools",
        "jain-feature-expansion",
        "jain-web-brand-favicon",
    ] {
        if !excluded_names.contains(expected) {
            manifest_failures.push(format!("missing excluded-path declaration: {expected}"));
        }
    }
    let mut extras = Vec::new();
    if let Ok(entries) = fs::read_dir(&split_root) {
        let known = managed_repositories(&data, &manifest)?
            .into_iter()
            .map(|repo| repo.name)
            .chain(excluded_names.iter().cloned())
            .collect::<std::collections::BTreeSet<_>>();
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir()
                && path.join(".git").exists()
                && path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| !known.contains(name))
            {
                extras.push(path.file_name().unwrap().to_string_lossy().to_string());
            }
        }
    }
    if !extras.is_empty() {
        manifest_failures.push(format!(
            "untracked git repositories in split root: {}",
            extras.join(", ")
        ));
    }

    let managed_worktrees = managed_repositories(&data, &manifest)?
        .iter()
        .map(verify_managed_worktree)
        .collect::<Vec<_>>();
    let status = if manifest_failures.is_empty()
        && rows.iter().all(|row| row["status"] == "pass")
        && managed_worktrees.iter().all(|row| row["status"] == "pass")
    {
        "pass"
    } else {
        "fail"
    };
    let external_dependency_failures = external_dependency_failures(&data);
    let status = if status == "pass" && external_dependency_failures.is_empty() {
        "pass"
    } else {
        "fail"
    };
    let family_failures = rows
        .iter()
        .filter(|row| row["kind"] != "required-infrastructure" && row["status"] == "fail")
        .flat_map(|row| {
            row["failures"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|failure| failure.as_str())
                .map(|failure| {
                    format!(
                        "{}: {failure}",
                        row["name"].as_str().unwrap_or("<unknown-repository>")
                    )
                })
        })
        .collect::<Vec<_>>();
    let infrastructure_failures = rows
        .iter()
        .filter(|row| row["kind"] == "required-infrastructure" && row["status"] == "fail")
        .flat_map(|row| {
            row["failures"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|failure| failure.as_str())
                .map(|failure| {
                    format!(
                        "{}: {failure}",
                        row["name"].as_str().unwrap_or("<unknown-repository>")
                    )
                })
        })
        .collect::<Vec<_>>();
    let excluded_paths = excluded
        .iter()
        .map(|item| {
            json!({
                "name": string(item, "name"),
                "path": string(item, "path"),
                "owner": string(item, "owner"),
                "reason": string(item, "reason"),
            })
        })
        .collect::<Vec<_>>();
    let report = json!({
        "schema_version": "jain-split-ops.preflight/v1",
        "manifest": manifest,
        "control_plane": control,
        "family_repo_count": family_repos(&data)?.len(),
        "infrastructure_repo_count": data
            .get("infrastructure_repo")
            .and_then(toml::Value::as_array)
            .map_or(0, Vec::len),
        "repo_count": rows.len(),
        "repositories": rows,
        "managed_release_worktrees": managed_worktrees,
        "managed_family_failures": family_failures,
        "infrastructure_failures": infrastructure_failures,
        "external_dependency_failures": external_dependency_failures,
        "excluded_paths": excluded_paths,
        "unmanaged_extras": extras,
        "extras": extras.clone(),
        "manifest_failures": manifest_failures,
        "status": status,
    });
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&output, serde_json::to_vec_pretty(&report)?)?;
    println!(
        "preflight {}: {} repositories; report {}",
        status,
        repos.len(),
        output.display()
    );
    if status == "pass" {
        Ok(())
    } else {
        Err("split preflight failed".into())
    }
}

fn external_dependency_failures(data: &toml::Value) -> Vec<String> {
    let mut failures = Vec::new();
    let topology = match nested_engine_topology(data) {
        Ok(topology) => topology,
        Err(error) => return vec![error],
    };
    let Some(bound) = &topology.bound_identity else {
        return vec![format!(
            "external dependency {} identity is pending",
            topology.dependency_name
        )];
    };
    let tag = &bound.tag;
    let remote = &topology.engine_remote;
    let label = &topology.engine_repository;
    if let Err(error) = validate_nested_engine_topology_paths(&topology) {
        return vec![error];
    }
    let core_path = topology.container_path.join(&topology.engine_repository);
    if !core_path.join(".git").exists() {
        failures.push(format!(
            "{label} checkout missing at {}",
            core_path.display()
        ));
        return failures;
    }
    if git_query(&core_path, &["remote", "get-url", "origin"]).as_deref() != Some(remote.as_str()) {
        failures.push(format!("{label} origin must be {remote}"));
    }
    let tag_ref = format!("refs/tags/{tag}^{{}}");
    let local_commit = git_query(&core_path, &["rev-parse", &tag_ref]);
    if local_commit.as_deref() != Some(bound.release_commit.as_str()) {
        failures.push(format!(
            "{label} immutable tag {tag} must resolve locally to {}",
            bound.release_commit
        ));
    }
    let remote_tag = git_query(&core_path, &["ls-remote", "origin", &tag_ref]);
    let remote_commit = remote_tag
        .as_deref()
        .and_then(|line| line.split_whitespace().next());
    if remote_commit != Some(bound.release_commit.as_str()) {
        failures.push(format!(
            "{label} immutable tag {tag} must resolve on Jeryu to {}",
            bound.release_commit
        ));
    }
    let tag_tree_ref = format!("refs/tags/{tag}^{{tree}}");
    let local_tree = git_query(&core_path, &["rev-parse", &tag_tree_ref]);
    if local_tree.as_deref() != Some(bound.release_tree.as_str()) {
        failures.push(format!(
            "{label} immutable tag {tag} tree must resolve locally to {}",
            bound.release_tree
        ));
    }
    match git_archive_sha256(&core_path, &bound.release_commit) {
        Some(actual) if actual == bound.release_checksum_sha256 => {}
        Some(actual) => failures.push(format!(
            "{label} archive checksum {actual} differs from bound {}",
            bound.release_checksum_sha256
        )),
        None => failures.push(format!(
            "{label} archive checksum could not be computed for {}",
            bound.release_commit
        )),
    }
    let nested = match fs::read_to_string(&topology.manifest_path)
        .ok()
        .and_then(|text| text.parse::<toml::Value>().ok())
    {
        Some(nested) => nested,
        None => {
            failures.push(format!(
                "unable to parse nested {} manifest {}",
                topology.family,
                topology.manifest_path.display()
            ));
            return failures;
        }
    };
    let lock_path = match declared_nested_lock_path(&topology, &nested) {
        Ok(path) => path,
        Err(error) => {
            failures.push(error);
            return failures;
        }
    };
    if physical_regular_file(&lock_path, "nested family lock").is_ok() {
        if let Ok(lock) = fs::read_to_string(&lock_path).and_then(|text| {
            text.parse::<toml::Value>()
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
        }) {
            if lock
                .get("proof")
                .and_then(|proof| proof.get("cutover_eligible"))
                .and_then(toml::Value::as_bool)
                != Some(true)
            {
                failures.push(format!(
                    "{} proof lock is not cutover_eligible",
                    topology.family
                ));
            }
            if let (Some(expected), Some(actual)) = (string(&lock, "engine_commit"), local_commit) {
                if expected != actual {
                    failures.push(format!(
                        "{label} tag commit {actual} differs from lock {expected}"
                    ));
                }
            }
        } else {
            failures.push(format!(
                "unable to parse {} lock {}",
                topology.family,
                lock_path.display()
            ));
        }
    } else {
        failures.push(format!(
            "{} family lock is missing or non-physical: {}",
            topology.family,
            lock_path.display()
        ));
    }
    failures
}

fn git_query(root: &Path, args: &[&str]) -> Option<String> {
    git_query_with_status(root, args).filter(|value| !value.is_empty())
}

fn git_archive_sha256(root: &Path, commit: &str) -> Option<String> {
    let output = Command::new("git")
        .args(["-C"])
        .arg(root)
        .args(["archive", "--format=tar", commit])
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| sha256_bytes(&output.stdout))
}

fn git_query_with_status(root: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn collect_python(root: &Path, out: &mut Vec<PathBuf>) -> io::Result<()> {
    if !root.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(root)? {
        let path = entry?.path();
        if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(python_scan_excluded_name)
        {
            continue;
        }
        if path.is_dir() {
            collect_python(&path, out)?;
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("py") {
            out.push(path);
        }
    }
    Ok(())
}

fn git_files(root: &Path, sha: &str) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    if !is_full_hex(sha, 40) {
        return Err("source commit must be 40 lowercase hex characters".into());
    }
    physical_directory(root, "source inventory Git input")?;
    let commit_ref = format!("{sha}^{{commit}}");
    let resolved = git_query(root, &["rev-parse", "--verify", &commit_ref])
        .ok_or_else(|| format!("source commit is unavailable: {sha}"))?;
    if resolved != sha {
        return Err(format!("source commit resolved to {resolved}, expected {sha}").into());
    }
    let output = Command::new("git")
        .args(["-c", "core.hooksPath=/dev/null", "-c", "diff.external="])
        .arg("-C")
        .arg(root)
        .args(["ls-tree", "-r", "-z", "--name-only", sha])
        .output()?;
    if !output.status.success() {
        return Err(format!("git ls-tree failed for {sha}").into());
    }
    if !output.stdout.ends_with(&[0]) {
        return Err("git ls-tree returned a malformed non-NUL-terminated inventory".into());
    }
    let mut files = output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
        .map(|path| String::from_utf8(path.to_vec()))
        .collect::<Result<Vec<_>, _>>()?;
    files.sort();
    let before = files.len();
    files.dedup();
    if files.len() != before {
        return Err("git tree contains duplicate source paths".into());
    }
    render_source_inventory(&files, sha)?;
    Ok(files)
}

fn path_matches(path: &str, pattern: &str) -> bool {
    if let Some(base) = pattern.strip_suffix("/**") {
        return path == base.trim_end_matches('/')
            || path.starts_with(&format!("{}/", base.trim_end_matches('/')));
    }
    fn walk(path: &[u8], pattern: &[u8]) -> bool {
        match pattern.first() {
            None => path.is_empty(),
            Some(b'*') => {
                walk(path, &pattern[1..]) || (!path.is_empty() && walk(&path[1..], pattern))
            }
            Some(byte) => path.first() == Some(byte) && walk(&path[1..], &pattern[1..]),
        }
    }
    walk(path.as_bytes(), pattern.as_bytes())
}

fn validate_local_jeryu(
    manifest: Option<PathBuf>,
    skip_remotes: bool,
    sealed_outer_projection: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if sealed_outer_projection && env::var("JAIN_HOST_CI_NETWORK_ISOLATED").as_deref() != Ok("1") {
        return Err(
            "--sealed-outer-projection is reserved for root-sealed isolated host CI".into(),
        );
    }
    let root = control_plane_root();
    let manifest_path = manifest.unwrap_or_else(|| root.join("repos.manifest.toml"));
    let data: toml::Value = fs::read_to_string(&manifest_path)?.parse()?;
    let repos = manifest_repos(&data)?;
    if repos.is_empty() {
        return Err("manifest has no repositories".into());
    }
    let mut errors = Vec::new();
    for raw in &repos {
        if !repo_is_onboarded(raw) {
            continue;
        }
        let repo = repo_from(raw)?;
        if !repo.path.join(".git").exists() && !skip_remotes {
            errors.push(format!(
                "{}: missing git checkout at {}",
                repo.name,
                repo.path.display()
            ));
            continue;
        }
        if !skip_remotes {
            let expected = declared_remote(raw)
                .ok_or_else(|| format!("{} is missing a declared remote", repo.name))?;
            let remotes = git_remotes(&repo.path)?;
            if remotes.len() != 1 || remotes.get("origin") != Some(&vec![expected.clone()]) {
                errors.push(format!(
                    "{}: remotes must contain exactly origin -> {}",
                    repo.name, expected
                ));
            }
            for (remote, urls) in &remotes {
                for url in urls {
                    if url != &expected {
                        errors.push(format!(
                            "{}: remote {} points outside local Jeryu: {}",
                            repo.name, remote, url
                        ));
                    }
                }
            }
        }
        check_cargo_sources(&repo, &mut errors)?;
    }
    validate_nested_family_local(&data, skip_remotes, &mut errors)?;
    let split_root = exact_absolute_path(
        &string(&data, "split_root").ok_or("manifest is missing split_root")?,
        "split_root",
    )?;
    for (key, registration) in registered_nested_families(&data) {
        let result = if sealed_outer_projection {
            validate_registered_nested_family_declaration(key, registration, &split_root)
        } else {
            validate_registered_nested_family_local(key, registration, &split_root, true)
        };
        if let Err(error) = result {
            errors.push(error);
        }
    }
    if !skip_remotes {
        let split_root = repos
            .first()
            .and_then(|raw| string(raw, "path"))
            .map(PathBuf::from)
            .and_then(|p| p.parent().map(Path::to_path_buf))
            .unwrap_or_else(|| root.parent().unwrap_or(&root).to_path_buf());
        for entry in fs::read_dir(&split_root)? {
            let path = entry?.path();
            if !path.is_dir() || !path.join(".git").exists() {
                continue;
            }
            let remotes = git_remotes(&path)?;
            if remotes.keys().any(|name| name != "origin") {
                errors.push(format!("{}: expected only origin remote", path.display()));
            }
            let expected = managed_repositories(&data, &manifest_path)?
                .into_iter()
                .find(|repo| repo.path == path)
                .map(|repo| repo.remote);
            let excluded = data
                .get("excluded_path")
                .and_then(toml::Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|raw| string(raw, "path"))
                .map(PathBuf::from)
                .any(|excluded_path| excluded_path == path);
            if expected.is_none() && excluded {
                continue;
            }
            if expected.is_none() {
                errors.push(format!("{}: unmanaged git checkout", path.display()));
                continue;
            }
            for (remote, urls) in remotes {
                for url in urls {
                    if expected.as_deref() != Some(url.as_str()) {
                        errors.push(format!(
                            "{}: remote {} does not match the manifest: {}",
                            path.display(),
                            remote,
                            url
                        ));
                    }
                }
            }
        }
    }
    if errors.is_empty() {
        println!(
            "local Jeryu policy ok: {} repos",
            managed_repositories(&data, &manifest_path)?.len()
        );
        Ok(())
    } else {
        Err(errors.join("\n").into())
    }
}

fn git_remotes(
    root: &Path,
) -> Result<std::collections::BTreeMap<String, Vec<String>>, Box<dyn std::error::Error>> {
    let output = Command::new("git")
        .args(["-C", root.to_str().ok_or("non-UTF8 repo path")?, "remote"])
        .output()?;
    if !output.status.success() {
        return Err(format!("cannot read remotes for {}", root.display()).into());
    }
    let names = String::from_utf8(output.stdout)?;
    let mut remotes = std::collections::BTreeMap::new();
    for name in names.lines().filter(|name| !name.is_empty()) {
        let output = Command::new("git")
            .args([
                "-C",
                root.to_str().ok_or("non-UTF8 repo path")?,
                "remote",
                "get-url",
                "--all",
                name,
            ])
            .output()?;
        if !output.status.success() {
            return Err(format!("cannot read remote {name} for {}", root.display()).into());
        }
        remotes.insert(
            name.to_owned(),
            String::from_utf8(output.stdout)?
                .lines()
                .map(str::to_owned)
                .collect(),
        );
    }
    Ok(remotes)
}

fn check_cargo_sources(
    repo: &Repo,
    errors: &mut Vec<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut files = Vec::new();
    collect_named_files(&repo.path, &mut files, &["Cargo.toml", "Cargo.lock"])?;
    for path in files {
        let text = fs::read_to_string(&path).unwrap_or_default();
        for (line_no, line) in text.lines().enumerate() {
            let internal_source = line.contains("git =")
                || line.starts_with("source = \"git+")
                || line.starts_with("[patch.");
            if internal_source
                && [
                    "github.com/neverhuman",
                    "git@github.com:neverhuman",
                    "jain-portal-preview",
                    "bare-mirrors",
                ]
                .iter()
                .any(|marker| line.contains(marker))
            {
                errors.push(format!(
                    "{}:{}:{}: Cargo source contains forbidden remote",
                    repo.name,
                    path.strip_prefix(&repo.path).unwrap_or(&path).display(),
                    line_no + 1
                ));
            }
            if !line.contains("git") {
                continue;
            }
            if line.contains(FAMILY_REMOTE_PREFIX)
                || line.contains(INFRA_REMOTE_PREFIX)
                || line.contains(LEGACY_FAMILY_PIN_PREFIX)
                || line.contains(LEGACY_INFRA_PIN_PREFIX)
            {
                continue;
            }
            if line.contains("git =") || line.starts_with("source = \"git+") {
                errors.push(format!(
                    "{}:{}:{}: internal git source is not local Jeryu",
                    repo.name,
                    path.strip_prefix(&repo.path).unwrap_or(&path).display(),
                    line_no + 1
                ));
            }
        }
    }
    Ok(())
}

fn collect_named_files(root: &Path, out: &mut Vec<PathBuf>, names: &[&str]) -> io::Result<()> {
    if !root.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(root)? {
        let path = entry?.path();
        if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| {
                matches!(
                    name,
                    ".git" | "target" | ".venv" | "node_modules" | ".stage" | "vendor"
                )
            })
        {
            continue;
        }
        if path.is_dir() {
            collect_named_files(&path, out, names)?;
        } else if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| names.contains(&name))
        {
            out.push(path);
        }
    }
    Ok(())
}

fn refresh(selected: &[String], authored_only: bool) -> Result<(), Box<dyn std::error::Error>> {
    let root = control_plane_root();
    let manifest_path = env::var_os("JAIN_SPLIT_MANIFEST")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("repos.manifest.toml"));
    let data: toml::Value = fs::read_to_string(&manifest_path)?.parse()?;
    let repos = manifest_repos(&data)?;
    let known: Vec<String> = repos.iter().filter_map(|r| string(r, "name")).collect();
    for name in selected {
        if !known.iter().any(|known_name| known_name == name) {
            return Err(format!("unknown repo: {name}").into());
        }
    }
    for raw in repos {
        let repo = repo_from(raw)?;
        if authored_only && !repo.authored {
            continue;
        }
        if selected.is_empty() || selected.iter().any(|name| name == &repo.name) {
            refresh_repo(&repo)?;
            println!("refreshed CI contract: {}", repo.name);
        }
    }
    Ok(())
}

fn refresh_bare_mirrors(args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    let root = control_plane_root();
    let mut manifest = root.join("repos.manifest.toml");
    let mut selected = Vec::new();
    let mut receipt = None;
    let mut apply = false;
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--manifest" => manifest = PathBuf::from(iter.next().ok_or("--manifest needs a path")?),
            "--repo" => selected.push(iter.next().ok_or("--repo needs a name")?),
            "--receipt" => {
                receipt = Some(PathBuf::from(iter.next().ok_or("--receipt needs a path")?))
            }
            "--apply" => apply = true,
            value => return Err(format!("unknown argument: {value}").into()),
        }
    }
    let receipt = receipt.unwrap_or_else(|| release_evidence_path("bare-mirror-refresh.json"));
    let mut report = receipt_header("jain.bare-mirror-refresh/v1", "refresh-bare-mirrors", apply);
    report["manifest"] = json!(manifest.display().to_string());
    report["selected_repositories"] = json!(selected);
    let data: toml::Value = fs::read_to_string(&manifest)?.parse()?;
    let split_root =
        PathBuf::from(string(&data, "split_root").ok_or("manifest missing split_root")?);
    let mirror_root = split_root.join("target/bare-mirrors");
    let repos = manifest_repos(&data)?;
    let known: Vec<String> = repos.iter().filter_map(|r| string(r, "name")).collect();
    for name in &selected {
        if !known.iter().any(|known_name| known_name == name) {
            return Err(format!("unknown repo: {name}").into());
        }
    }
    let mut items = Vec::new();
    let result = (|| {
        if apply {
            fs::create_dir_all(&mirror_root)?;
        }
        for raw in repos {
            let repo = repo_from(raw)?;
            if !selected.is_empty() && !selected.iter().any(|name| name == &repo.name) {
                continue;
            }
            if !repo.path.join(".git").exists() {
                return Err(format!(
                    "{}: source checkout missing at {}",
                    repo.name,
                    repo.path.display()
                )
                .into());
            }
            let mirror = mirror_root.join(format!("{}.git", repo.name));
            let existed = mirror.exists();
            let source_head = strict_git_output(&repo.path, &["rev-parse", "HEAD"])?;
            let source_refs = strict_git_output(
                &repo.path,
                &[
                    "for-each-ref",
                    "--sort=refname",
                    "--format=%(objectname) %(refname)",
                    "refs/heads",
                    "refs/tags",
                ],
            )?;
            if apply {
                if existed {
                    run_git(&[
                        "--git-dir",
                        mirror.to_str().ok_or("non-UTF8 mirror path")?,
                        "fetch",
                        "--quiet",
                        "--force",
                        "--prune",
                        "--prune-tags",
                        repo.path.to_str().ok_or("non-UTF8 repo path")?,
                        "+refs/heads/*:refs/heads/*",
                        "+refs/tags/*:refs/tags/*",
                    ])?;
                } else {
                    run_git(&[
                        "clone",
                        "--mirror",
                        "--quiet",
                        repo.path.to_str().ok_or("non-UTF8 repo path")?,
                        mirror.to_str().ok_or("non-UTF8 mirror path")?,
                    ])?;
                }
                let mirror_refs = strict_git_output(
                    &mirror,
                    &[
                        "for-each-ref",
                        "--sort=refname",
                        "--format=%(objectname) %(refname)",
                        "refs/heads",
                        "refs/tags",
                    ],
                )?;
                if mirror_refs != source_refs {
                    return Err(
                        format!("{}: refreshed mirror refs differ from source", repo.name).into(),
                    );
                }
            }
            items.push(json!({
                "name": repo.name,
                "source": repo.path,
                "source_head": source_head,
                "mirror": mirror,
                "mirror_existed": existed,
                "action": if apply {
                    if existed {"refreshed"} else {"created"}
                } else if existed {
                    "would-refresh"
                } else {
                    "would-create"
                },
                "refs_verified": apply,
            }));
        }
        Ok(())
    })();
    report["repositories"] = json!(items);
    finish_receipted_operation(&receipt, &mut report, result)
}

fn run_git(args: &[&str]) -> Result<(), Box<dyn std::error::Error>> {
    let status = Command::new("git").args(args).status()?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("git command failed with status {status}").into())
    }
}

fn repo_from(value: &toml::Value) -> Result<Repo, Box<dyn std::error::Error>> {
    Ok(Repo {
        name: string(value, "name").ok_or("repo missing name")?,
        path: PathBuf::from(string(value, "path").ok_or("repo missing path")?),
        profile: string(value, "profile").unwrap_or_default(),
        authored: value
            .get("authored")
            .and_then(toml::Value::as_bool)
            .unwrap_or(false),
        cargo_members: strings(value, "cargo_members"),
        copy_paths: strings(value, "copy_paths"),
        source_paths: strings(value, "source_paths"),
    })
}

fn family_repos(data: &toml::Value) -> Result<Vec<&toml::Value>, Box<dyn std::error::Error>> {
    let repos = data
        .get("repo")
        .and_then(toml::Value::as_array)
        .ok_or("manifest has no [[repo]] entries")?;
    if repos.is_empty() {
        return Err("manifest has no family repositories".into());
    }
    Ok(repos.iter().collect())
}

fn manifest_repos(data: &toml::Value) -> Result<Vec<&toml::Value>, Box<dyn std::error::Error>> {
    let mut repos = family_repos(data)?;
    if let Some(infrastructure) = data
        .get("infrastructure_repo")
        .and_then(toml::Value::as_array)
    {
        repos.extend(infrastructure.iter());
    }
    Ok(repos)
}

fn declared_remote(value: &toml::Value) -> Option<String> {
    string(value, "remote").or_else(|| {
        string(value, "jeryu_slug").map(|slug| format!("{LOCAL_JERYU_ORIGIN}/git/{slug}.git"))
    })
}

fn string(value: &toml::Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(toml::Value::as_str)
        .map(ToOwned::to_owned)
}

fn repo_is_onboarded(value: &toml::Value) -> bool {
    value
        .get("onboarded")
        .and_then(toml::Value::as_bool)
        .unwrap_or(true)
}

fn strings(value: &toml::Value, key: &str) -> Vec<String> {
    value
        .get(key)
        .and_then(toml::Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(toml::Value::as_str)
                .map(ToOwned::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

fn refresh_repo(repo: &Repo) -> Result<(), Box<dyn std::error::Error>> {
    if !repo.path.is_dir() {
        return Err(format!(
            "{} checkout is missing at {}",
            repo.name,
            repo.path.display()
        )
        .into());
    }
    write(
        &repo.path.join("agent/owner-map.json"),
        &render_owner_map(repo),
    )?;
    write(
        &repo.path.join("agent/test-map.json"),
        &render_test_map(repo),
    )?;
    write(
        &repo.path.join("agent/coverage-sources.toml"),
        &render_coverage_sources(repo),
    )?;
    write(
        &repo.path.join("agent/security-policy.toml"),
        &render_security_policy(repo),
    )?;
    write(&repo.path.join(".gitleaks.toml"), &render_gitleaks(repo))?;
    if !repo.cargo_members.is_empty() || repo.path.join("Cargo.toml").exists() {
        write(&repo.path.join("deny.toml"), &render_deny())?;
    }
    write(&repo.path.join("scripts/ci-local.sh"), render_ci_local())?;
    Ok(())
}

fn write(path: &Path, body: &str) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, body)
}

fn route_variants(pattern: &str) -> Vec<String> {
    if let Some(base) = pattern.strip_suffix("/**") {
        let base = base.trim_end_matches('/');
        return vec![format!("{base}/"), format!("{base}/**")];
    }
    if let Some(base) = pattern.strip_suffix('/') {
        return vec![format!("{base}/"), format!("{base}/**")];
    }
    vec![pattern.to_owned()]
}

fn add_route(map: &mut Map<String, JsonValue>, pattern: &str, value: JsonValue) {
    for variant in route_variants(pattern) {
        map.insert(variant, value.clone());
    }
}

fn standard_routes(repo: &Repo) -> Vec<String> {
    let mut routes: Vec<String> = vec![
        ".cargo/",
        ".ci-status/",
        ".config/",
        ".dockerignore",
        ".gitattributes",
        ".github/",
        ".gitignore",
        "AGENTS.md",
        "CHANGELOG.md",
        "Justfile",
        "LICENSE",
        "PRODUCT_VERSION",
        "README.md",
        "SPLIT.md",
        "THIRD_PARTY_NOTICES.md",
        "VERSION",
        "agent/",
        ".autonomy/",
        ".jeryu/",
        ".gitleaks.toml",
        "db/",
        "deny.toml",
        "docs/",
        "ops/",
        "ops/AGENTS.md",
        "ops/ci/",
        "ops/dev/",
        "ops/git-hooks/",
        "ops/split/",
        "rust-toolchain.toml",
        "scripts/",
        "schemas/",
        "tests/",
        "tools/",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    if repo.profile == "public-portal" {
        routes.extend(["family.lock", "repos.manifest.toml"].map(str::to_owned));
    }
    if !repo.cargo_members.is_empty() || repo.path.join("Cargo.toml").exists() {
        routes.extend(["Cargo.toml", "Cargo.lock"].map(str::to_owned));
    }
    if repo.name == "jain-web" {
        routes.extend(
            ["apps/web/", "contracts/", "package.json", "pnpm-lock.yaml"].map(str::to_owned),
        );
    }
    if repo.name == "jain-python" {
        routes.extend(["python/ai-service/", "contracts/"].map(str::to_owned));
    }
    if repo.name == "jain-deploy" {
        routes.extend(["deployment/", "jain-split.lock.toml", ".stage/"].map(str::to_owned));
    }
    if repo.name == "jain-starforge" {
        routes.extend(["artifacts/foundation/", "artifacts/starforge/"].map(str::to_owned));
    }
    routes.extend(repo.copy_paths.iter().map(|path| {
        if repo.path.join(path).is_dir() {
            format!("{path}/")
        } else {
            path.clone()
        }
    }));
    routes.extend(repo.source_paths.iter().cloned());
    routes.extend(repo.cargo_members.iter().map(|path| format!("{path}/")));
    routes
}

fn render_owner_map(repo: &Repo) -> String {
    let mut owners = Map::new();
    for route in standard_routes(repo) {
        add_route(&mut owners, &route, json!("split"));
    }
    for (route, owner) in [
        (".cargo/", "workspace"),
        (".config/", "workspace"),
        (".dockerignore", "workspace"),
        (".gitattributes", "workspace"),
        (".gitleaks.toml", "ops"),
        (".github/", "ops"),
        (".gitignore", "workspace"),
        (".jankurai/", "audit"),
        ("AGENTS.md", "split"),
        ("CHANGELOG.md", "release"),
        ("Cargo.lock", "workspace"),
        ("Cargo.toml", "workspace"),
        ("Justfile", "workspace"),
        ("LICENSE", "workspace"),
        ("README.md", "docs"),
        ("SPLIT.md", "split"),
        ("VERSION", "release"),
        ("agent/", "agent"),
        ("deny.toml", "workspace"),
        ("docs/", "docs"),
        ("family.lock", "split"),
        ("ops/", "ops"),
        ("repos.manifest.toml", "split"),
        ("rust-toolchain.toml", "workspace"),
        ("scripts/", "ops"),
        ("tests/", "tests"),
    ] {
        add_route(&mut owners, route, json!(owner));
    }
    for path in &repo.copy_paths {
        let route = if repo.path.join(path).is_dir() {
            format!("{path}/")
        } else {
            path.clone()
        };
        add_route(&mut owners, &route, json!(repo.name));
    }
    for path in &repo.source_paths {
        add_route(&mut owners, path, json!(repo.name));
    }
    for path in &repo.cargo_members {
        add_route(&mut owners, &format!("{path}/"), json!(repo.name));
    }
    let body = json!({"workspace": repo.name, "owners": owners});
    serde_json::to_string_pretty(&body)
        .map(|s| format!("{s}\n"))
        .unwrap()
}

fn test_route(command: &str, purpose: &str, lane: &str) -> JsonValue {
    json!({"command": command, "lane": lane, "purpose": purpose})
}

fn render_test_map(repo: &Repo) -> String {
    let mut tests = Map::new();
    for route in standard_routes(repo) {
        add_route(
            &mut tests,
            &route,
            test_route(
                "just check",
                "verify workspace metadata remains parseable",
                "check",
            ),
        );
    }
    for (route, value) in [
        (
            ".cargo/",
            test_route(
                "just check",
                "verify portable cargo configuration remains parseable",
                "check",
            ),
        ),
        (
            ".ci-status/",
            test_route(
                "just required",
                "verify generated fleet evidence is reproducible by the required lane",
                "required",
            ),
        ),
        (
            ".gitleaks.toml",
            test_route(
                "just security",
                "verify the committed secret-scan policy remains narrow and parseable",
                "security",
            ),
        ),
        (
            ".github/",
            test_route(
                "just check && just tool-adoption",
                "verify workflows remain pinned and adoption evidence is wired",
                "ci",
            ),
        ),
        (
            ".jankurai/",
            test_route(
                "just score",
                "verify committed audit evidence matches the pinned Jankurai lane",
                "score",
            ),
        ),
        (
            "db/",
            test_route(
                "just check && just score",
                "verify database or no-database boundary evidence remains routed",
                "db",
            ),
        ),
        (
            "AGENTS.md",
            test_route(
                "just score",
                "verify split metadata and agent maps",
                "score",
            ),
        ),
        (
            "Cargo.lock",
            test_route(
                "just required",
                "verify the split lockfile resolves",
                "required",
            ),
        ),
        (
            "Cargo.toml",
            test_route(
                "just required",
                "verify workspace manifest shape and proof lane",
                "required",
            ),
        ),
        (
            "Justfile",
            test_route(
                "just fast && just check && just required",
                "verify local command wrappers remain canonical",
                "ci",
            ),
        ),
        (
            "README.md",
            test_route(
                "just check",
                "verify root navigation remains current",
                "check",
            ),
        ),
        (
            "PRODUCT_VERSION",
            test_route(
                "just score",
                "verify documentation version metadata remains routed",
                "score",
            ),
        ),
        (
            "SPLIT.md",
            test_route(
                "just score",
                "verify split provenance and inherited source cap policy",
                "score",
            ),
        ),
        (
            "THIRD_PARTY_NOTICES.md",
            test_route(
                "just security",
                "verify third-party attribution and license notices remain routed",
                "security",
            ),
        ),
        (
            "schemas/",
            test_route(
                "just score",
                "verify committed evidence schemas remain aligned with generated reports",
                "score",
            ),
        ),
        (
            "ops/",
            test_route(
                "just check && just tool-adoption",
                "verify local CI wrappers are reproducible and audit-visible",
                "ci",
            ),
        ),
    ] {
        add_route(&mut tests, route, value);
    }
    let source_command = if !repo.cargo_members.is_empty()
        || repo.name == "jain-web"
        || repo.name == "jain-python"
    {
        "just required"
    } else {
        "just check"
    };
    for path in &repo.source_paths {
        add_route(
            &mut tests,
            path,
            test_route(
                source_command,
                &format!(
                    "verify {} owned source remains build-addressable",
                    repo.name
                ),
                "required",
            ),
        );
    }
    for path in &repo.copy_paths {
        let route = if repo.path.join(path).is_dir() {
            format!("{path}/")
        } else {
            path.clone()
        };
        add_route(
            &mut tests,
            &route,
            test_route(
                "just required",
                &format!(
                    "verify copied {} surface remains build-addressable",
                    repo.name
                ),
                "required",
            ),
        );
    }
    for path in &repo.cargo_members {
        add_route(
            &mut tests,
            &format!("{path}/"),
            test_route(
                "just required",
                &format!(
                    "verify {} Cargo member remains build-addressable",
                    repo.name
                ),
                "required",
            ),
        );
    }
    let body = json!({"workspace": repo.name, "tests": tests});
    serde_json::to_string_pretty(&body)
        .map(|s| format!("{s}\n"))
        .unwrap()
}

fn render_coverage_sources(repo: &Repo) -> String {
    let mut out = String::from("version = 1\n\n");
    if !repo.cargo_members.is_empty() || repo.path.join("Cargo.toml").exists() {
        out.push_str("[[source]]\nid = \"rust-lcov\"\nkind = \"line_coverage\"\nformat = \"lcov\"\nmode = \"advisory\"\nowner = \"tools\"\nlane = \"coverage-audit\"\nartifacts = [\"target/llvm-cov/lcov.info\", \"target/jankurai/coverage/rust-lcov.info\"]\napplies_to = [\"crates/**/*.rs\"]\nrules = [\"HLT-008-FALSE-GREEN-RISK\"]\n\n");
    }
    out.push_str("[[source]]\nid = \"security-evidence\"\nkind = \"supply_chain\"\nformat = \"generic-json-summary\"\nmode = \"auto\"\nowner = \"ops\"\nlane = \"security\"\nartifacts = [\"target/jankurai/security/evidence.json\", \"target/security/evidence.json\"]\napplies_to = [\".github/**\", \"Cargo.toml\", \"Cargo.lock\", \"package.json\", \"apps/web/package.json\"]\nrules = [\"HLT-016-SUPPLY-CHAIN-DRIFT\"]\n");
    out
}

fn render_security_policy(repo: &Repo) -> String {
    let npm = if repo.path.join("apps/web/package.json").exists() {
        ", \"npm\""
    } else {
        ""
    };
    format!("schema_version = \"1.0.0\"\nworkspace = \"{}\"\n\nenabled_tools = [\"gitleaks\", \"cargo-audit\"{}, \"zizmor\", \"syft\", \"cargo-deny\", \"grype\", \"trivy\"]\nrequired_tools = [\"gitleaks\"]\nadvisory_tools = [\"cargo-audit\", \"zizmor\", \"syft\", \"cargo-deny\", \"grype\", \"trivy\"]\n\n[severity_thresholds]\nfail_lane_on = \"high\"\n", repo.name, npm)
}

fn render_gitleaks(repo: &Repo) -> String {
    let extra = match repo.name.as_str() {
        "jain-web" => "  '''^apps/web/node_modules/''',\n  '''^apps/web/dist/''',\n  '''^apps/web/playwright-report/''',\n  '''^apps/web/test-results/''',\n",
        "jain-llm" => "  '''^crates/jain-llm/tests/contracts\\.rs$''',\n",
        _ => "",
    };
    format!("# Generated Jain split secret-scan policy.\n\n[extend]\nuseDefault = true\n\n[allowlist]\ndescription = \"generated build outputs and explicitly non-secret fixture paths\"\npaths = [\n  '''^target/''',\n  '''^\\.jankurai/''',\n  '''^(vendor|vendor-crates)/.*/\\.cargo-checksum\\.json$''',\n{}]\n", extra)
}

fn render_deny() -> String {
    let names = [
        "jain-domain",
        "jain-math",
        "jain-catboost",
        "jain-xgboost",
        "jain-lightgbm",
        "jain-jable",
        "jain-battle-gpu",
        "jain-starforge",
        "jain-core",
        "jain-report",
        "jain-tui",
        "jain-cli",
        "jain-web",
        "jain-llm",
        "jain-research",
        "jain-deploy",
    ];
    let sources = names
        .iter()
        .map(|name| format!("  \"http://127.0.0.1:8787/git/jeryu/{name}.git\",\n"))
        .collect::<String>();
    format!("[advisories]\nignore = []\nunmaintained = \"workspace\"\n\n[licenses]\nallow = [\"Apache-2.0\", \"BSD-2-Clause\", \"BSD-3-Clause\", \"CC0-1.0\", \"CDLA-Permissive-2.0\", \"ISC\", \"MIT\", \"Unicode-3.0\", \"Zlib\"]\n\n[bans]\nmultiple-versions = \"warn\"\nwildcards = \"warn\"\n\n[sources]\nunknown-registry = \"deny\"\nunknown-git = \"deny\"\nallow-git = [\n{}]\n", sources)
}

fn render_ci_local() -> &'static str {
    "#!/usr/bin/env bash\nset -euo pipefail\n\nlane=\"${1:-required}\"\ncase \"$lane\" in\n  required) bash ops/ci/required.sh ;;\n  fast) bash ops/ci/fast.sh ;;\n  check) bash ops/ci/check.sh ;;\n  score) bash ops/ci/score.sh ;;\n  security) bash ops/ci/security.sh ;;\n  security-network) bash ops/ci/security-network.sh ;;\n  tool-adoption) bash ops/ci/tool-adoption.sh ;;\n  contract-drift) bash ops/ci/contract-drift.sh ;;\n  artifact-support) bash ops/ci/artifact_support.sh ;;\n  e2e) bash ops/ci/e2e.sh ;;\n  *) printf 'unknown lane: %s\\n' \"$lane\" >&2; exit 2 ;;\nesac\n"
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{symlink, PermissionsExt};
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

    struct TestDir(PathBuf);

    fn test_temp_parent(cargo_target: Option<PathBuf>, control_root: &Path) -> PathBuf {
        cargo_target
            .filter(|path| path.is_absolute())
            .unwrap_or_else(|| control_root.join("target"))
            .join("test-tmp")
    }

    impl TestDir {
        fn new(label: &str) -> Self {
            let parent = test_temp_parent(
                env::var_os("CARGO_TARGET_DIR").map(PathBuf::from),
                &control_plane_root(),
            );
            fs::create_dir_all(&parent).unwrap();
            let path = parent.join(format!(
                "jain-split-ops-{label}-{}-{}",
                std::process::id(),
                NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }

        fn new_private_temp(label: &str) -> Self {
            let path = env::temp_dir().join(format!(
                "jain-split-ops-{label}-{}-{}",
                std::process::id(),
                NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
            Self(path)
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            fn make_removable(path: &Path) {
                let Ok(metadata) = fs::symlink_metadata(path) else {
                    return;
                };
                if !metadata.file_type().is_dir() {
                    return;
                }
                let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o700));
                if let Ok(entries) = fs::read_dir(path) {
                    for entry in entries.flatten() {
                        make_removable(&entry.path());
                    }
                }
            }
            make_removable(&self.0);
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn qualified_appliance_matrix() -> JsonValue {
        let sha = |character: char| character.to_string().repeat(64);
        let index_digest = format!("sha256:{}", sha('a'));
        let mut matrix = json!({
            "schema_version": "jain.local-appliance-canary-matrix/v1",
            "qualification": true,
            "fixture": false,
            "release": RELEASE_VERSION,
            "release_tag": "jain-deploy-v8.0.1-split.5",
            "source_commit": "be6f00f5d0f501c3acad32e66dbfdb674a2fd532",
            "status": "candidate",
            "formal_ga": false,
            "rollback_release": "7.0.6",
            "release_job": {
                "id": sha('2'),
                "attestation_url": "https://release.jain.local/jobs/job-1.json",
                "attestation_sha256": sha('3'),
                "signature_url": "https://release.jain.local/jobs/job-1.json.sig",
                "verified": true
            },
            "manifest_sha256": sha('4'),
            "public_key_sha256": sha('5'),
            "artifact_identities": {
                "installer": sha('6'),
                "manager": sha('7'),
                "cli": sha('8'),
                "compose": sha('9'),
                "compose_gpu": sha('a'),
                "provenance": sha('b'),
                "browser_suite": sha('c'),
                "training_data": sha('d'),
                "scoring_data": sha('e')
            },
            "artifact_set_sha256": sha('f'),
            "oci": {
                "index": format!("registry.jain.local/appliance@{index_digest}"),
                "index_digest": index_digest,
                "platform": "linux/amd64",
                "platform_digest": format!("sha256:{}", sha('b')),
                "runtime_image_id": format!("sha256:{}", sha('c')),
                "runtime_repo_digest": format!("registry.jain.local/appliance@sha256:{}", sha('a'))
            },
            "lanes": {
                "cpu": {"receipt_sha256": sha('d'), "qualification": true},
                "gpu": {"receipt_sha256": sha('e'), "qualification": true}
            },
            "created_at": "2026-07-21T23:00:00Z"
        });
        matrix["artifact_set_sha256"] =
            json!(appliance_artifact_set_sha256(&matrix["artifact_identities"]).unwrap());
        matrix
    }

    fn write_immutable_json(root: &Path, name: &str, value: &JsonValue) -> PathBuf {
        let path = root.join(name);
        fs::write(&path, serde_json::to_vec_pretty(value).unwrap()).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o444)).unwrap();
        path
    }

    fn qualified_appliance_verifier(matrix: &JsonValue, aggregate_sha256: &str) -> JsonValue {
        let mut receipt = json!({
            "schema_version": "jain.local-appliance-canary-verifier/v1",
            "verifier": {
                "name": APPLIANCE_VERIFIER_NAME,
                "sha256": APPLIANCE_VERIFIER_SHA256
            },
            "aggregate_sha256": aggregate_sha256,
            "release_tag": matrix["release_tag"],
            "source_commit": matrix["source_commit"],
            "release_job_id": matrix["release_job"]["id"],
            "attestation_sha256": matrix["release_job"]["attestation_sha256"],
            "signature_sha256": "a".repeat(64),
            "public_key_sha256": matrix["public_key_sha256"],
            "verified_at": "2026-07-21T23:01:00Z",
            "seal_sha256": "1".repeat(64)
        });
        receipt["seal_sha256"] =
            json!(appliance_verifier_seal(receipt.as_object().unwrap()).unwrap());
        receipt
    }

    fn write_qualified_appliance_evidence(
        root: &Path,
        name: &str,
        matrix: &JsonValue,
    ) -> (PathBuf, PathBuf, u32, u32) {
        let aggregate = write_immutable_json(root, &format!("{name}-aggregate.json"), matrix);
        let aggregate_sha256 = sha256_bytes(&fs::read(&aggregate).unwrap());
        let verifier = write_immutable_json(
            root,
            &format!("{name}-verifier.json"),
            &qualified_appliance_verifier(matrix, &aggregate_sha256),
        );
        let metadata = fs::metadata(&aggregate).unwrap();
        (aggregate, verifier, metadata.uid(), metadata.gid())
    }

    #[test]
    fn appliance_promotion_accepts_only_real_same_release_cpu_gpu_aggregate() {
        let temp = TestDir::new("appliance-promotion-valid");
        let matrix = qualified_appliance_matrix();
        let (aggregate, verifier, uid, gid) =
            write_qualified_appliance_evidence(temp.path(), "valid", &matrix);
        let qualification = read_qualified_appliance_canary_with_authority(
            &aggregate,
            &verifier,
            uid,
            gid,
            matrix["source_commit"].as_str(),
        )
        .unwrap();
        assert_eq!(qualification.matrix["release"], RELEASE_VERSION);
        assert_eq!(qualification.matrix["lanes"]["cpu"]["qualification"], true);
        assert_eq!(qualification.matrix["lanes"]["gpu"]["qualification"], true);
        assert!(valid_sha256(&qualification.aggregate_sha256));
        assert!(valid_sha256(&qualification.verifier_receipt_sha256));
        assert_eq!(appliance_canary_summary(&qualification)["status"], "pass");
    }

    #[test]
    fn appliance_promotion_rejects_fixture_false_green_mismatch_and_secrets() {
        let temp = TestDir::new("appliance-promotion-hostile");
        let mut cases = Vec::new();

        let mut value = qualified_appliance_matrix();
        value["qualification"] = json!(false);
        cases.push(("false-qualification", value));

        let mut value = qualified_appliance_matrix();
        value["fixture"] = json!(true);
        value["qualification"] = json!(false);
        value["release_job"]["verified"] = json!(false);
        value["lanes"]["cpu"]["qualification"] = json!(false);
        value["lanes"]["gpu"]["qualification"] = json!(false);
        cases.push(("fixture", value));

        let mut value = qualified_appliance_matrix();
        value["release"] = json!("8.0.2");
        cases.push(("wrong-release", value));

        let mut value = qualified_appliance_matrix();
        value["release_tag"] = json!("jain-deploy-v8.0.1-candidate.forged");
        cases.push(("non-immutable-tag", value));

        let mut value = qualified_appliance_matrix();
        value["lanes"]["gpu"]["qualification"] = json!(false);
        cases.push(("gpu-fallback", value));

        let mut value = qualified_appliance_matrix();
        value["lanes"]["gpu"]["receipt_sha256"] = value["lanes"]["cpu"]["receipt_sha256"].clone();
        cases.push(("duplicate-lane", value));

        let mut value = qualified_appliance_matrix();
        value["oci"]["index"] = json!(format!(
            "registry.jain.local/appliance@sha256:{}",
            "f".repeat(64)
        ));
        cases.push(("index-mismatch", value));

        let mut value = qualified_appliance_matrix();
        value["oci"]["runtime_repo_digest"] = json!(format!(
            "registry.jain.local/appliance@sha256:{}",
            "f".repeat(64)
        ));
        cases.push(("runtime-mismatch", value));

        let index_digest = qualified_appliance_matrix()["oci"]["index_digest"]
            .as_str()
            .unwrap()
            .to_owned();
        for (name, repository) in [
            (
                "oci-multiple-separators",
                format!("identity@registry.jain.local/appliance@{index_digest}"),
            ),
            (
                "oci-url-form",
                format!("https://registry.jain.local/appliance@{index_digest}"),
            ),
            (
                "oci-encoded-form",
                format!("registry.jain.local/team%2fappliance@{index_digest}"),
            ),
            (
                "oci-tag-form",
                format!("registry.jain.local/appliance:latest@{index_digest}"),
            ),
            (
                "oci-noncanonical-case",
                format!("Registry.Jain.Local/appliance@{index_digest}"),
            ),
        ] {
            let mut value = qualified_appliance_matrix();
            value["oci"]["index"] = json!(repository);
            cases.push((name, value));
        }

        let mut value = qualified_appliance_matrix();
        value["manifest_sha256"] = json!("0".repeat(64));
        cases.push(("zero-digest", value));

        let mut value = qualified_appliance_matrix();
        value["artifact_set_sha256"] = json!("f".repeat(64));
        cases.push(("artifact-set-mismatch", value));

        let mut value = qualified_appliance_matrix();
        value["release_job"]["verified"] = json!(false);
        cases.push(("unverified-job", value));

        let mut value = qualified_appliance_matrix();
        value["release_job"]["attestation_url"] = json!("http://127.0.0.1/job.json");
        cases.push(("fixture-url", value));

        let mut value = qualified_appliance_matrix();
        value["release_job"]["attestation_url"] =
            json!("https://release.jain.local/job.json?token=exposed");
        cases.push(("secret-value", value));

        let mut value = qualified_appliance_matrix();
        value["release_job"]["attestation_url"] =
            json!("https://release.jain.local/job.json#token=exposed");
        cases.push(("secret-fragment", value));

        let mut value = qualified_appliance_matrix();
        value["release_job"]["attestation_url"] =
            json!("https://release.jain.local/job.json?download=1");
        cases.push(("query-evidence", value));

        let mut value = qualified_appliance_matrix();
        value["release_job"]["attestation_url"] =
            json!("https://operator:credential@release.jain.local/job.json");
        cases.push(("credential-userinfo", value));

        let mut value = qualified_appliance_matrix();
        value["release_job"]["attestation_url"] = json!("https://127.0.0.1/job.json");
        cases.push(("loopback-authority", value));

        let mut value = qualified_appliance_matrix();
        value["release_job"]["attestation_url"] = json!("https:///job.json");
        cases.push(("missing-authority", value));

        let mut value = qualified_appliance_matrix();
        value["created_at"] = json!("2026-02-30T23:00:00Z");
        cases.push(("invalid-time", value));

        let mut value = qualified_appliance_matrix();
        value
            .as_object_mut()
            .unwrap()
            .insert("unknown".to_owned(), json!(true));
        cases.push(("unknown-field", value));

        let mut value = qualified_appliance_matrix();
        value["release_job"]
            .as_object_mut()
            .unwrap()
            .insert("unknown".to_owned(), json!(true));
        cases.push(("unknown-nested-field", value));

        let mut value = qualified_appliance_matrix();
        value.as_object_mut().unwrap().remove("artifact_set_sha256");
        cases.push(("missing-field", value));

        for (name, value) in cases {
            let (aggregate, verifier, uid, gid) =
                write_qualified_appliance_evidence(temp.path(), name, &value);
            assert!(
                read_qualified_appliance_canary_with_authority(
                    &aggregate,
                    &verifier,
                    uid,
                    gid,
                    value["source_commit"].as_str(),
                )
                .is_err(),
                "accepted hostile aggregate {name}"
            );
        }
    }

    #[test]
    fn oci_repository_digest_grammar_is_closed_and_canonical() {
        let digest = format!("sha256:{}", "a".repeat(64));
        for repository in [
            "registry.jain.local/team/appliance.v1",
            "registry.jain.local/team/appliance_v1",
            "registry.jain.local/team/appliance__v1",
            "registry.jain.local/team/appliance---v1",
        ] {
            assert!(valid_repo_digest(
                &format!("{repository}@{digest}"),
                &digest,
            ));
        }
        for repository in [
            "registry.jain.local",
            "registry.jain.local/",
            "registry.jain.local//appliance",
            "registry.jain.local/../appliance",
            "registry.jain.local/team\\appliance",
            "registry.jain.local/team%2fappliance",
            "registry.jain.local/team/appliance?tag=latest",
            "registry.jain.local/team/appliance#fragment",
            "registry.jain.local/team/appliance:latest",
            "registry.jain.local/team/a..b",
            "registry.jain.local/team/a.-b",
            "registry.jain.local/team/a_.b",
            "registry.jain.local/team/a___b",
            "Registry.Jain.Local/team/appliance",
            "127.0.0.1/team/appliance",
            "localhost/team/appliance",
        ] {
            assert!(
                !valid_repo_digest(&format!("{repository}@{digest}"), &digest),
                "accepted noncanonical OCI repository {repository}"
            );
        }
        assert!(!valid_repo_digest(
            &format!("identity@registry.jain.local/appliance@{digest}"),
            &digest,
        ));
        assert!(!valid_repo_digest(
            &format!("{}.jain.local/appliance@{digest}", "a".repeat(64)),
            &digest,
        ));
        assert!(!valid_repo_digest(
            &format!("registry.jain.local/{}@{digest}", "a".repeat(129)),
            &digest,
        ));
    }

    #[test]
    fn appliance_promotion_rejects_ambiguous_or_unsafe_files() {
        let temp = TestDir::new("appliance-promotion-files");
        let value = qualified_appliance_matrix();
        let (valid_aggregate, verifier, uid, gid) =
            write_qualified_appliance_evidence(temp.path(), "authority", &value);

        let writable = temp.path().join("writable.json");
        fs::write(&writable, serde_json::to_vec(&value).unwrap()).unwrap();
        assert!(read_qualified_appliance_canary_with_authority(
            &writable,
            &verifier,
            uid,
            gid,
            value["source_commit"].as_str(),
        )
        .is_err());

        let original = write_immutable_json(temp.path(), "linked.json", &value);
        let linked = temp.path().join("linked-copy.json");
        fs::hard_link(&original, &linked).unwrap();
        assert!(read_qualified_appliance_canary_with_authority(
            &original,
            &verifier,
            uid,
            gid,
            value["source_commit"].as_str(),
        )
        .is_err());
        assert!(read_qualified_appliance_canary_with_authority(
            &linked,
            &verifier,
            uid,
            gid,
            value["source_commit"].as_str(),
        )
        .is_err());

        let symlink_path = temp.path().join("symlink.json");
        symlink(&original, &symlink_path).unwrap();
        assert!(read_qualified_appliance_canary_with_authority(
            &symlink_path,
            &verifier,
            uid,
            gid,
            value["source_commit"].as_str(),
        )
        .is_err());

        let duplicate = temp.path().join("duplicate.json");
        let bytes = serde_json::to_string(&value).unwrap().replacen(
            "\"qualification\":true",
            "\"qualification\":true,\"qualification\":true",
            1,
        );
        fs::write(&duplicate, bytes).unwrap();
        fs::set_permissions(&duplicate, fs::Permissions::from_mode(0o444)).unwrap();
        assert!(read_qualified_appliance_canary_with_authority(
            &duplicate,
            &verifier,
            uid,
            gid,
            value["source_commit"].as_str(),
        )
        .is_err());
        assert!(read_qualified_appliance_canary_with_authority(
            &valid_aggregate,
            &verifier,
            uid.wrapping_add(1),
            gid,
            value["source_commit"].as_str(),
        )
        .is_err());
    }

    #[test]
    fn appliance_promotion_rejects_unsealed_or_mismatched_verifier_authority() {
        let temp = TestDir::new("appliance-promotion-verifier");
        let matrix = qualified_appliance_matrix();
        let (aggregate, verifier, uid, gid) =
            write_qualified_appliance_evidence(temp.path(), "valid", &matrix);

        assert!(read_qualified_appliance_canary_with_authority(
            &aggregate,
            &verifier,
            uid,
            gid,
            Some("1a6f00f5d0f501c3acad32e66dbfdb674a2fd532"),
        )
        .is_err());

        let aggregate_sha256 = sha256_bytes(&fs::read(&aggregate).unwrap());
        let mut cases = Vec::new();

        let mut receipt = qualified_appliance_verifier(&matrix, &aggregate_sha256);
        receipt["verifier"]["sha256"] = json!("b".repeat(64));
        receipt["seal_sha256"] =
            json!(appliance_verifier_seal(receipt.as_object().unwrap()).unwrap());
        cases.push(("wrong-verifier", receipt));

        let mut receipt = qualified_appliance_verifier(&matrix, &aggregate_sha256);
        receipt["aggregate_sha256"] = json!("b".repeat(64));
        receipt["seal_sha256"] =
            json!(appliance_verifier_seal(receipt.as_object().unwrap()).unwrap());
        cases.push(("wrong-aggregate", receipt));

        let mut receipt = qualified_appliance_verifier(&matrix, &aggregate_sha256);
        receipt["attestation_sha256"] = json!("b".repeat(64));
        receipt["seal_sha256"] =
            json!(appliance_verifier_seal(receipt.as_object().unwrap()).unwrap());
        cases.push(("wrong-attestation", receipt));

        let mut receipt = qualified_appliance_verifier(&matrix, &aggregate_sha256);
        receipt["signature_sha256"] = json!("0".repeat(64));
        receipt["seal_sha256"] =
            json!(appliance_verifier_seal(receipt.as_object().unwrap()).unwrap());
        cases.push(("zero-signature", receipt));

        let mut receipt = qualified_appliance_verifier(&matrix, &aggregate_sha256);
        receipt["seal_sha256"] = json!("b".repeat(64));
        cases.push(("forged-seal", receipt));

        for (name, receipt) in cases {
            let receipt =
                write_immutable_json(temp.path(), &format!("{name}-verifier.json"), &receipt);
            assert!(read_qualified_appliance_canary_with_authority(
                &aggregate,
                &receipt,
                uid,
                gid,
                matrix["source_commit"].as_str(),
            )
            .is_err());
        }

        fs::set_permissions(&verifier, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(read_qualified_appliance_canary_with_authority(
            &aggregate,
            &verifier,
            uid,
            gid,
            matrix["source_commit"].as_str(),
        )
        .is_err());
    }

    #[test]
    fn release_status_writes_blocked_evidence_without_aggregate() {
        let temp = TestDir::new("release-status-appliance-blocked");
        let output = temp.path().join("release-status.json");
        let error = release_status(vec![
            "--manifest".to_owned(),
            control_plane_root()
                .join("repos.manifest.toml")
                .display()
                .to_string(),
            "--json".to_owned(),
            output.display().to_string(),
        ])
        .unwrap_err();
        assert!(error.to_string().contains("release status blocked"));
        let report: JsonValue = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
        assert_eq!(report["appliance_canary"]["status"], "blocked");
        assert_eq!(report["promotion_evidence_ready"], false);
        assert_eq!(report["production_promotion_authorized"], false);
        assert_eq!(report["formal_ga"], false);
    }

    #[test]
    fn qualified_summary_never_authorizes_activation() {
        let temp = TestDir::new("release-status-appliance-qualified");
        let matrix = qualified_appliance_matrix();
        let (aggregate, verifier, uid, gid) =
            write_qualified_appliance_evidence(temp.path(), "summary", &matrix);
        let qualification = read_qualified_appliance_canary_with_authority(
            &aggregate,
            &verifier,
            uid,
            gid,
            matrix["source_commit"].as_str(),
        )
        .unwrap();
        let summary = appliance_canary_summary(&qualification);
        assert_eq!(summary["status"], "pass");
        assert_eq!(summary["qualification"], true);
        assert!(valid_sha256(
            summary["verifier_receipt_sha256"].as_str().unwrap()
        ));
        let report = appliance_promotion_report(&qualification);
        assert_eq!(report["promotion_evidence_ready"], true);
        assert_eq!(report["production_promotion_authorized"], false);
        assert_eq!(report["formal_ga"], false);
    }

    struct CargoCacheFixture {
        lock: PathBuf,
        source: PathBuf,
        destination: PathBuf,
        receipt: PathBuf,
        archive: PathBuf,
        index_record: PathBuf,
        source_uid: u32,
        source_gid: u32,
    }

    fn cargo_cache_fixture(root: &Path, archive_bytes: &[u8]) -> CargoCacheFixture {
        let source = root.join("registry-source");
        let archive_root = source.join("cache/index.crates.io-1949cf8c6b5b557f");
        let sparse_root = source.join("index/index.crates.io-6f17d22bba15001f");
        let archive = archive_root.join("demo-1.2.3.crate");
        let index_record = sparse_root.join(".cache/de/mo/demo");
        fs::create_dir_all(&archive_root).unwrap();
        fs::create_dir_all(index_record.parent().unwrap()).unwrap();
        fs::write(&archive, archive_bytes).unwrap();
        fs::write(
            sparse_root.join("config.json"),
            br#"{"dl":"https://static.crates.io/crates","api":"https://crates.io"}"#,
        )
        .unwrap();
        fs::write(&index_record, b"fixture sparse index record\n").unwrap();
        for path in [&archive, &sparse_root.join("config.json"), &index_record] {
            fs::set_permissions(path, fs::Permissions::from_mode(0o444)).unwrap();
        }
        for path in [
            source.join("cache"),
            archive_root.clone(),
            source.join("index"),
            sparse_root.clone(),
            sparse_root.join(".cache"),
            sparse_root.join(".cache/de"),
            sparse_root.join(".cache/de/mo"),
            source.clone(),
        ] {
            fs::set_permissions(path, fs::Permissions::from_mode(0o555)).unwrap();
        }
        let lock = root.join("Cargo.lock");
        fs::write(
            &lock,
            format!(
                "version = 4\n\n[[package]]\nname = \"demo\"\nversion = \"1.2.3\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\nchecksum = \"{}\"\n",
                sha256_bytes(archive_bytes)
            ),
        )
        .unwrap();
        let destination = root.join("staged-registry");
        let receipt = destination.join("stage-receipt.json");
        let source_metadata = fs::metadata(&source).unwrap();
        CargoCacheFixture {
            lock,
            source,
            destination,
            receipt,
            archive,
            index_record,
            source_uid: source_metadata.uid(),
            source_gid: source_metadata.gid(),
        }
    }

    fn zero_package_locks(root: &Path, count: usize) -> Vec<PathBuf> {
        let lock_root = root.join("closure-locks");
        fs::create_dir(&lock_root).unwrap();
        (0..count)
            .map(|index| {
                let lock = lock_root.join(format!("Cargo-{index:04}.lock"));
                fs::write(&lock, "version = 4\n").unwrap();
                lock
            })
            .collect()
    }

    #[test]
    fn locked_cargo_cache_accepts_authenticated_closure_scale_and_finite_maximum() {
        for count in [516, MAX_CARGO_LOCKS] {
            let temp = TestDir::new(&format!("cargo-cache-stage-lock-count-{count}"));
            let fixture = cargo_cache_fixture(temp.path(), b"unused archive");
            let locks = zero_package_locks(temp.path(), count);
            stage_locked_cargo_caches(
                &locks,
                &fixture.source,
                &fixture.destination,
                &fixture.receipt,
                fixture.source_uid,
                fixture.source_gid,
            )
            .unwrap();
            let receipt: JsonValue =
                serde_json::from_slice(&fs::read(&fixture.receipt).unwrap()).unwrap();
            assert_eq!(receipt["lock_count"], count);
            assert_eq!(receipt["package_count"], 0);
        }
    }

    #[test]
    fn locked_cargo_cache_rejects_count_above_finite_maximum_before_staging() {
        let temp = TestDir::new("cargo-cache-stage-lock-count-overflow");
        let fixture = cargo_cache_fixture(temp.path(), b"unused archive");
        let locks = vec![fixture.lock.clone(); MAX_CARGO_LOCKS + 1];
        let error = stage_locked_cargo_caches(
            &locks,
            &fixture.source,
            &fixture.destination,
            &fixture.receipt,
            fixture.source_uid,
            fixture.source_gid,
        )
        .unwrap_err();
        assert!(error
            .to_string()
            .contains(&format!("at most {MAX_CARGO_LOCKS} Cargo locks")));
        assert!(!fixture.destination.exists());
    }

    #[test]
    fn locked_cargo_cache_rejects_per_file_and_aggregate_byte_overflow() {
        let single = TestDir::new("cargo-cache-stage-oversized-lock");
        let fixture = cargo_cache_fixture(single.path(), b"unused archive");
        fs::write(&fixture.lock, vec![b'x'; MAX_CARGO_LOCK_BYTES as usize + 1]).unwrap();
        let error = stage_locked_cargo_cache(
            &fixture.lock,
            &fixture.source,
            &fixture.destination,
            &fixture.receipt,
            fixture.source_uid,
            fixture.source_gid,
        )
        .unwrap_err();
        assert!(error.to_string().contains("per-file limit"));
        assert!(!fixture.destination.exists());

        let aggregate = TestDir::new("cargo-cache-stage-aggregate-overflow");
        let first = aggregate.path().join("first.lock");
        let second = aggregate.path().join("second.lock");
        fs::write(&first, "version = 4\n").unwrap();
        fs::write(&second, "version = 4\n").unwrap();
        let paths = vec![first, second];
        let total = paths
            .iter()
            .map(|path| fs::metadata(path).unwrap().len())
            .sum::<u64>();
        let error = read_bounded_cargo_locks_with_limits(&paths, MAX_CARGO_LOCK_BYTES, total - 1)
            .unwrap_err();
        assert!(error.to_string().contains("aggregate limit"));
        assert!(!aggregate.path().join("staged-registry").exists());
    }

    #[test]
    fn locked_cargo_cache_stages_only_checksum_verified_inputs() {
        let temp = TestDir::new("cargo-cache-stage-success");
        let fixture = cargo_cache_fixture(temp.path(), b"deterministic crate archive");
        stage_locked_cargo_cache(
            &fixture.lock,
            &fixture.source,
            &fixture.destination,
            &fixture.receipt,
            fixture.source_uid,
            fixture.source_gid,
        )
        .unwrap();

        assert_eq!(
            fs::read(
                fixture
                    .destination
                    .join("cache/index.crates.io-1949cf8c6b5b557f/demo-1.2.3.crate")
            )
            .unwrap(),
            b"deterministic crate archive"
        );
        assert_eq!(
            fs::read(
                fixture
                    .destination
                    .join("index/index.crates.io-6f17d22bba15001f/.cache/de/mo/demo")
            )
            .unwrap(),
            b"fixture sparse index record\n"
        );
        let receipt: JsonValue =
            serde_json::from_slice(&fs::read(&fixture.receipt).unwrap()).unwrap();
        assert_eq!(receipt["schema_version"], "jain.locked-cargo-cache/v2");
        assert_eq!(receipt["lock_count"], 1);
        assert_eq!(receipt["package_count"], 1);
        assert_eq!(receipt["governed_git_repositories"], json!([]));
        assert_eq!(
            receipt["lock_sha256s"],
            json!([sha256_regular_file(&fixture.lock, "test Cargo lock").unwrap()])
        );
        assert!(!fs::read_dir(temp.path())
            .unwrap()
            .flatten()
            .any(|entry| entry
                .file_name()
                .to_string_lossy()
                .starts_with(".staged-registry.stage-")));
    }

    #[test]
    fn locked_cargo_cache_accepts_only_immutable_governed_git_sources() {
        let accepted = [
            "git+http://127.0.0.1:8787/git/veox/jain-math.git?tag=jain-math-v8.0.1-split.1#da87246d8339fab457b576bf0c08c3d394b9c92b",
            "git+http://127.0.0.1:8787/git/jeryu/redline-core.git?tag=redline-core-v4.1.0-jain.4#3567bdced0ca1fe3671c9ebda876c914e2fc2c9e",
            "git+http://127.0.0.1:8787/git/jain-split/jain-core.git?tag=jain-core-v8.0.1-split.1#aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "git+http://127.0.0.1:8787/git/redline/redline-testing.git?tag=redline-testing-v4.1.0-jain.1#bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        ];
        let expected_repositories = ["jain-math", "redline-core", "jain-core", "redline-testing"];
        for (source, expected_repository) in accepted.into_iter().zip(expected_repositories) {
            assert!(
                governed_locked_git_repository(source).is_some(),
                "rejected {source}"
            );
            assert_eq!(
                governed_locked_git_repository(source),
                Some(expected_repository)
            );
        }

        let rejected = [
            "git+https://github.com/neverhuman/jain-math.git?tag=jain-math-v8.0.1-split.1#da87246d8339fab457b576bf0c08c3d394b9c92b",
            "git+http://127.0.0.1:8787/git/unknown/jain-math.git?tag=jain-math-v8.0.1-split.1#da87246d8339fab457b576bf0c08c3d394b9c92b",
            "git+http://127.0.0.1:8787/git/veox/jain-math.git?branch=main#da87246d8339fab457b576bf0c08c3d394b9c92b",
            "git+http://127.0.0.1:8787/git/veox/jain-math.git?tag=jain-math-v8.0.1-split.1",
            "git+http://127.0.0.1:8787/git/veox/jain-math.git?tag=wrong-v8.0.1-split.1#da87246d8339fab457b576bf0c08c3d394b9c92b",
            "git+http://127.0.0.1:8787/git/veox/jain-math.git?tag=jain-math-v8.0.1-split.1#DA87246D8339FAB457B576BF0C08C3D394B9C92B",
            "git+http://127.0.0.1:8787/git/veox/../jain-math.git?tag=jain-math-v8.0.1-split.1#da87246d8339fab457b576bf0c08c3d394b9c92b",
        ];
        for source in rejected {
            assert!(
                governed_locked_git_repository(source).is_none(),
                "accepted {source}"
            );
            assert_eq!(governed_locked_git_repository(source), None);
        }
    }

    #[test]
    fn locked_cargo_cache_skips_validated_git_packages_but_binds_the_lock() {
        let temp = TestDir::new("cargo-cache-stage-governed-git");
        let fixture = cargo_cache_fixture(temp.path(), b"deterministic crate archive");
        let registry_lock = fs::read_to_string(&fixture.lock).unwrap();
        fs::write(
            &fixture.lock,
            format!(
                "{registry_lock}\n[[package]]\nname = \"feat-math\"\nversion = \"8.0.1\"\nsource = \"git+http://127.0.0.1:8787/git/jeryu/jain-math.git?tag=jain-math-v8.0.1-split.1#da87246d8339fab457b576bf0c08c3d394b9c92b\"\n\n[[package]]\nname = \"feat-math-alias\"\nversion = \"8.0.1\"\nsource = \"git+http://127.0.0.1:8787/git/veox/jain-math.git?tag=jain-math-v8.0.1-split.1#da87246d8339fab457b576bf0c08c3d394b9c92b\"\n"
            ),
        )
        .unwrap();

        stage_locked_cargo_cache(
            &fixture.lock,
            &fixture.source,
            &fixture.destination,
            &fixture.receipt,
            fixture.source_uid,
            fixture.source_gid,
        )
        .unwrap();

        let receipt: JsonValue =
            serde_json::from_slice(&fs::read(&fixture.receipt).unwrap()).unwrap();
        assert_eq!(receipt["package_count"], 1);
        assert_eq!(receipt["packages"][0]["name"], "demo");
        assert_eq!(receipt["governed_git_repositories"], json!(["jain-math"]));
        assert_eq!(
            receipt["lock_sha256s"],
            json!([sha256_regular_file(&fixture.lock, "test Cargo lock").unwrap()])
        );
    }

    #[test]
    fn locked_cargo_cache_accepts_digest_bound_zero_package_v3_v4_locks() {
        for version in [3, 4] {
            let temp = TestDir::new(&format!("cargo-cache-stage-empty-v{version}"));
            let fixture = cargo_cache_fixture(temp.path(), b"unused archive");
            fs::write(&fixture.lock, format!("version = {version}\n")).unwrap();

            stage_locked_cargo_cache(
                &fixture.lock,
                &fixture.source,
                &fixture.destination,
                &fixture.receipt,
                fixture.source_uid,
                fixture.source_gid,
            )
            .unwrap();

            let receipt: JsonValue =
                serde_json::from_slice(&fs::read(&fixture.receipt).unwrap()).unwrap();
            assert_eq!(receipt["lock_count"], 1);
            assert_eq!(receipt["package_count"], 0);
            assert_eq!(receipt["packages"], json!([]));
            assert_eq!(
                receipt["lock_sha256s"],
                json!([sha256_regular_file(&fixture.lock, "empty Cargo lock").unwrap()])
            );
        }
    }

    #[test]
    fn zero_package_cargo_lock_rejects_malformed_unknown_and_source_bearing_forms() {
        let temp = TestDir::new("cargo-cache-stage-empty-rejections");
        for (name, contents, message) in [
            ("missing-version", "", "no integer version"),
            ("string-version", "version = \"4\"\n", "no integer version"),
            (
                "unknown-version",
                "version = 5\n",
                "unsupported Cargo lock version",
            ),
            (
                "unknown-field",
                "version = 4\nmetadata = {}\n",
                "unknown top-level field",
            ),
            (
                "source-bearing",
                "version = 4\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\n",
                "unknown top-level field",
            ),
            (
                "package-not-array",
                "version = 4\npackage = {}\n",
                "not an array",
            ),
            (
                "malformed-package-source",
                "version = 4\n\n[[package]]\nname = \"local\"\nversion = \"1.0.0\"\nsource = 42\n",
                "source is not a string",
            ),
        ] {
            let path = temp.path().join(format!("{name}.lock"));
            fs::write(&path, contents).unwrap();
            let error = locked_cargo_inputs(&path).unwrap_err();
            assert!(error.to_string().contains(message), "{name}: {error}");
        }
    }

    #[test]
    fn nested_redline_web_zero_registry_lock_is_digest_bound() {
        let temp = TestDir::new("cargo-cache-stage-redline-web-empty");
        let fixture = cargo_cache_fixture(temp.path(), b"registry archive");
        let nested = temp.path().join("jain-redline/redline-web/Cargo.lock");
        fs::create_dir_all(nested.parent().unwrap()).unwrap();
        fs::write(
            &nested,
            "version = 4\n\n[[package]]\nname = \"redline-web-release-control\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();

        stage_locked_cargo_caches(
            std::slice::from_ref(&nested),
            &fixture.source,
            &fixture.destination,
            &fixture.receipt,
            fixture.source_uid,
            fixture.source_gid,
        )
        .unwrap();
        let receipt: JsonValue =
            serde_json::from_slice(&fs::read(&fixture.receipt).unwrap()).unwrap();
        assert_eq!(receipt["lock_count"], 1);
        assert_eq!(receipt["package_count"], 0);
        assert!(receipt["lock_sha256s"]
            .as_array()
            .unwrap()
            .contains(&json!(sha256_regular_file(
                &nested,
                "nested Redline Web lock"
            )
            .unwrap())));
    }

    #[test]
    fn locked_cargo_cache_rejects_checksummed_governed_git_packages() {
        let temp = TestDir::new("cargo-cache-stage-checksummed-git");
        let fixture = cargo_cache_fixture(temp.path(), b"deterministic crate archive");
        fs::write(
            &fixture.lock,
            "version = 4\n\n[[package]]\nname = \"feat-math\"\nversion = \"8.0.1\"\nsource = \"git+http://127.0.0.1:8787/git/jeryu/jain-math.git?tag=jain-math-v8.0.1-split.1#da87246d8339fab457b576bf0c08c3d394b9c92b\"\nchecksum = \"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\"\n",
        )
        .unwrap();

        let error = stage_locked_cargo_cache(
            &fixture.lock,
            &fixture.source,
            &fixture.destination,
            &fixture.receipt,
            fixture.source_uid,
            fixture.source_gid,
        )
        .unwrap_err();
        assert!(error
            .to_string()
            .contains("governed locked Git package unexpectedly has a checksum"));
        assert!(!fixture.destination.exists());
    }

    #[test]
    fn locked_cargo_cache_stages_shared_index_once_for_multiple_versions() {
        let temp = TestDir::new("cargo-cache-stage-multiple-versions");
        let fixture = cargo_cache_fixture(temp.path(), b"first archive version");
        let archive_parent = fixture.archive.parent().unwrap();
        let second_archive = archive_parent.join("demo-2.0.0.crate");
        let second_bytes = b"second archive version";
        fs::set_permissions(archive_parent, fs::Permissions::from_mode(0o755)).unwrap();
        fs::write(&second_archive, second_bytes).unwrap();
        fs::set_permissions(&second_archive, fs::Permissions::from_mode(0o444)).unwrap();
        fs::set_permissions(archive_parent, fs::Permissions::from_mode(0o555)).unwrap();
        fs::write(
            &fixture.lock,
            format!(
                "version = 4\n\n[[package]]\nname = \"demo\"\nversion = \"1.2.3\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\nchecksum = \"{}\"\n\n[[package]]\nname = \"demo\"\nversion = \"2.0.0\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\nchecksum = \"{}\"\n",
                sha256_regular_file(&fixture.archive, "first test archive").unwrap(),
                sha256_bytes(second_bytes),
            ),
        )
        .unwrap();

        stage_locked_cargo_cache(
            &fixture.lock,
            &fixture.source,
            &fixture.destination,
            &fixture.receipt,
            fixture.source_uid,
            fixture.source_gid,
        )
        .unwrap();

        let cache_root = fixture
            .destination
            .join("cache/index.crates.io-1949cf8c6b5b557f");
        assert_eq!(
            fs::read(cache_root.join("demo-1.2.3.crate")).unwrap(),
            b"first archive version"
        );
        assert_eq!(
            fs::read(cache_root.join("demo-2.0.0.crate")).unwrap(),
            second_bytes
        );
        assert_eq!(
            fs::read(
                fixture
                    .destination
                    .join("index/index.crates.io-6f17d22bba15001f/.cache/de/mo/demo")
            )
            .unwrap(),
            b"fixture sparse index record\n"
        );
        let receipt: JsonValue =
            serde_json::from_slice(&fs::read(&fixture.receipt).unwrap()).unwrap();
        assert_eq!(receipt["package_count"], 2);
        assert_eq!(receipt["packages"][0]["version"], "1.2.3");
        assert_eq!(receipt["packages"][1]["version"], "2.0.0");
    }

    #[test]
    fn locked_cargo_cache_unions_multiple_locks_deterministically() {
        let temp = TestDir::new("cargo-cache-stage-multiple-locks");
        let fixture = cargo_cache_fixture(temp.path(), b"first archive");
        let archive_parent = fixture.archive.parent().unwrap();
        let second_archive = archive_parent.join("other-4.5.6.crate");
        let second_bytes = b"second archive";
        let second_index = fixture
            .index_record
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("ot/he/other");
        fs::set_permissions(archive_parent, fs::Permissions::from_mode(0o755)).unwrap();
        fs::write(&second_archive, second_bytes).unwrap();
        fs::set_permissions(&second_archive, fs::Permissions::from_mode(0o444)).unwrap();
        fs::set_permissions(archive_parent, fs::Permissions::from_mode(0o555)).unwrap();
        let sparse_cache_root = second_index
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .parent()
            .unwrap();
        fs::set_permissions(sparse_cache_root, fs::Permissions::from_mode(0o755)).unwrap();
        fs::create_dir_all(second_index.parent().unwrap()).unwrap();
        fs::write(&second_index, b"second sparse index record\n").unwrap();
        fs::set_permissions(&second_index, fs::Permissions::from_mode(0o444)).unwrap();
        for directory in [
            second_index.parent().unwrap(),
            second_index.parent().unwrap().parent().unwrap(),
        ] {
            fs::set_permissions(directory, fs::Permissions::from_mode(0o555)).unwrap();
        }
        fs::set_permissions(sparse_cache_root, fs::Permissions::from_mode(0o555)).unwrap();
        let second_lock = temp.path().join("fuzz-Cargo.lock");
        fs::write(
            &second_lock,
            format!(
                "version = 4\n\n[[package]]\nname = \"other\"\nversion = \"4.5.6\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\nchecksum = \"{}\"\n",
                sha256_bytes(second_bytes)
            ),
        )
        .unwrap();

        stage_locked_cargo_caches(
            &[second_lock.clone(), fixture.lock.clone()],
            &fixture.source,
            &fixture.destination,
            &fixture.receipt,
            fixture.source_uid,
            fixture.source_gid,
        )
        .unwrap();

        let receipt: JsonValue =
            serde_json::from_slice(&fs::read(&fixture.receipt).unwrap()).unwrap();
        let mut expected_digests = vec![
            sha256_regular_file(&fixture.lock, "first test Cargo lock").unwrap(),
            sha256_regular_file(&second_lock, "second test Cargo lock").unwrap(),
        ];
        expected_digests.sort();
        assert_eq!(receipt["lock_count"], 2);
        assert_eq!(receipt["lock_sha256s"], json!(expected_digests));
        assert_eq!(receipt["package_count"], 2);
        assert_eq!(receipt["packages"][0]["name"], "demo");
        assert_eq!(receipt["packages"][1]["name"], "other");
        assert_eq!(
            fs::read(
                fixture
                    .destination
                    .join("cache/index.crates.io-1949cf8c6b5b557f/other-4.5.6.crate")
            )
            .unwrap(),
            second_bytes
        );
    }

    #[test]
    fn locked_cargo_cache_rejects_duplicate_lock_paths_before_staging() {
        let temp = TestDir::new("cargo-cache-stage-duplicate-lock");
        let fixture = cargo_cache_fixture(temp.path(), b"expected archive");
        let error = stage_locked_cargo_caches(
            &[fixture.lock.clone(), fixture.lock.clone()],
            &fixture.source,
            &fixture.destination,
            &fixture.receipt,
            fixture.source_uid,
            fixture.source_gid,
        )
        .unwrap_err();
        assert!(error.to_string().contains("duplicate Cargo lock path"));
        assert!(!fixture.destination.exists());
    }

    #[test]
    fn locked_cargo_cache_rejects_hardlinked_lock_paths() {
        let temp = TestDir::new("cargo-cache-stage-nonphysical-lock");
        let fixture = cargo_cache_fixture(temp.path(), b"expected archive");
        let hardlink_lock = temp.path().join("hardlink-Cargo.lock");
        fs::hard_link(&fixture.lock, &hardlink_lock).unwrap();
        let error = stage_locked_cargo_caches(
            std::slice::from_ref(&fixture.lock),
            &fixture.source,
            &fixture.destination,
            &fixture.receipt,
            fixture.source_uid,
            fixture.source_gid,
        )
        .unwrap_err();
        assert!(error.to_string().contains("unsafe Cargo lock inode"));
        assert!(!fixture.destination.exists());

        fs::remove_file(hardlink_lock).unwrap();
        let alias = fixture.lock.parent().unwrap().join("child/../Cargo.lock");
        let error = stage_locked_cargo_caches(
            &[alias],
            &fixture.source,
            &fixture.destination,
            &fixture.receipt,
            fixture.source_uid,
            fixture.source_gid,
        )
        .unwrap_err();
        assert!(error.to_string().contains("not normalized"));
        assert!(!fixture.destination.exists());
    }

    #[test]
    fn locked_cargo_cache_rejects_cross_lock_checksum_conflicts() {
        let temp = TestDir::new("cargo-cache-stage-cross-lock-conflict");
        let fixture = cargo_cache_fixture(temp.path(), b"expected archive");
        let conflicting_lock = temp.path().join("conflicting-Cargo.lock");
        fs::write(
            &conflicting_lock,
            "version = 4\n\n[[package]]\nname = \"demo\"\nversion = \"1.2.3\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\nchecksum = \"ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff\"\n",
        )
        .unwrap();
        let error = stage_locked_cargo_caches(
            &[fixture.lock.clone(), conflicting_lock],
            &fixture.source,
            &fixture.destination,
            &fixture.receipt,
            fixture.source_uid,
            fixture.source_gid,
        )
        .unwrap_err();
        assert!(error
            .to_string()
            .contains("conflicting locked checksums for demo 1.2.3 across Cargo locks"));
        assert!(!fixture.destination.exists());
    }

    #[test]
    fn locked_cargo_cache_rejects_tampered_archive() {
        let temp = TestDir::new("cargo-cache-stage-tamper");
        let fixture = cargo_cache_fixture(temp.path(), b"expected archive");
        fs::set_permissions(&fixture.archive, fs::Permissions::from_mode(0o644)).unwrap();
        fs::write(&fixture.archive, b"tampered archive").unwrap();
        fs::set_permissions(&fixture.archive, fs::Permissions::from_mode(0o444)).unwrap();
        let error = stage_locked_cargo_cache(
            &fixture.lock,
            &fixture.source,
            &fixture.destination,
            &fixture.receipt,
            fixture.source_uid,
            fixture.source_gid,
        )
        .unwrap_err();
        assert!(error.to_string().contains("checksum mismatch"));
        assert!(!fixture.destination.exists());
    }

    #[test]
    fn locked_cargo_cache_rejects_nonregular_archive() {
        let temp = TestDir::new("cargo-cache-stage-nonregular-archive");
        let fixture = cargo_cache_fixture(temp.path(), b"expected archive");
        let archive_parent = fixture.archive.parent().unwrap();
        fs::set_permissions(archive_parent, fs::Permissions::from_mode(0o755)).unwrap();
        fs::remove_file(&fixture.archive).unwrap();
        fs::create_dir(&fixture.archive).unwrap();
        fs::set_permissions(archive_parent, fs::Permissions::from_mode(0o555)).unwrap();
        let error = stage_locked_cargo_cache(
            &fixture.lock,
            &fixture.source,
            &fixture.destination,
            &fixture.receipt,
            fixture.source_uid,
            fixture.source_gid,
        )
        .unwrap_err();
        let message = error.to_string().to_ascii_lowercase();
        assert!(message.contains("regular file") || message.contains("inode"));
        assert!(!fixture.destination.exists());
    }

    #[test]
    fn locked_cargo_cache_rejects_missing_sparse_index_record() {
        let temp = TestDir::new("cargo-cache-stage-index");
        let fixture = cargo_cache_fixture(temp.path(), b"expected archive");
        let index_parent = fixture.index_record.parent().unwrap();
        fs::set_permissions(index_parent, fs::Permissions::from_mode(0o755)).unwrap();
        fs::remove_file(&fixture.index_record).unwrap();
        fs::set_permissions(index_parent, fs::Permissions::from_mode(0o555)).unwrap();
        let error = stage_locked_cargo_cache(
            &fixture.lock,
            &fixture.source,
            &fixture.destination,
            &fixture.receipt,
            fixture.source_uid,
            fixture.source_gid,
        )
        .unwrap_err();
        assert!(error
            .to_string()
            .contains("sparse index record must have exactly one source"));
        assert!(!fixture.destination.exists());
    }

    #[test]
    fn locked_cargo_cache_rejects_hardlinked_archive() {
        let temp = TestDir::new("cargo-cache-stage-hardlink");
        let fixture = cargo_cache_fixture(temp.path(), b"expected archive");
        let cache_parent = fixture.source.join("cache");
        let second_root = cache_parent.join("duplicate-cache");
        fs::set_permissions(&cache_parent, fs::Permissions::from_mode(0o755)).unwrap();
        fs::create_dir(&second_root).unwrap();
        fs::hard_link(&fixture.archive, second_root.join("demo-1.2.3.crate")).unwrap();
        fs::set_permissions(&second_root, fs::Permissions::from_mode(0o555)).unwrap();
        fs::set_permissions(&cache_parent, fs::Permissions::from_mode(0o555)).unwrap();

        let error = stage_locked_cargo_cache(
            &fixture.lock,
            &fixture.source,
            &fixture.destination,
            &fixture.receipt,
            fixture.source_uid,
            fixture.source_gid,
        )
        .unwrap_err();
        assert!(error
            .to_string()
            .contains("unsafe locked crate archive inode"));
        assert!(!fixture.destination.exists());
    }

    #[test]
    fn locked_cargo_cache_rejects_duplicate_archive_roots() {
        let temp = TestDir::new("cargo-cache-stage-duplicate");
        let fixture = cargo_cache_fixture(temp.path(), b"expected archive");
        let cache_parent = fixture.source.join("cache");
        let second_root = cache_parent.join("duplicate-cache");
        let duplicate = second_root.join("demo-1.2.3.crate");
        fs::set_permissions(&cache_parent, fs::Permissions::from_mode(0o755)).unwrap();
        fs::create_dir(&second_root).unwrap();
        fs::copy(&fixture.archive, &duplicate).unwrap();
        fs::set_permissions(&duplicate, fs::Permissions::from_mode(0o444)).unwrap();
        fs::set_permissions(&second_root, fs::Permissions::from_mode(0o555)).unwrap();
        fs::set_permissions(&cache_parent, fs::Permissions::from_mode(0o555)).unwrap();

        let error = stage_locked_cargo_cache(
            &fixture.lock,
            &fixture.source,
            &fixture.destination,
            &fixture.receipt,
            fixture.source_uid,
            fixture.source_gid,
        )
        .unwrap_err();
        assert!(error
            .to_string()
            .contains("locked crate archive must have exactly one source, found 2"));
        assert!(!fixture.destination.exists());
    }

    #[test]
    fn locked_cargo_cache_rejects_existing_destination() {
        let temp = TestDir::new("cargo-cache-stage-destination");
        let fixture = cargo_cache_fixture(temp.path(), b"expected archive");
        fs::create_dir(&fixture.destination).unwrap();
        let sentinel = fixture.destination.join("sentinel");
        fs::write(&sentinel, b"preserve").unwrap();

        let error = stage_locked_cargo_cache(
            &fixture.lock,
            &fixture.source,
            &fixture.destination,
            &fixture.receipt,
            fixture.source_uid,
            fixture.source_gid,
        )
        .unwrap_err();
        assert!(error.to_string().contains("new absolute path"));
        assert_eq!(fs::read(&sentinel).unwrap(), b"preserve");
    }

    #[test]
    fn locked_cargo_cache_rejects_receipt_path_escape() {
        let temp = TestDir::new("cargo-cache-stage-receipt-escape");
        let fixture = cargo_cache_fixture(temp.path(), b"expected archive");
        let escaped_receipt = fixture.destination.join("../escaped-receipt.json");
        let error = stage_locked_cargo_cache(
            &fixture.lock,
            &fixture.source,
            &fixture.destination,
            &escaped_receipt,
            fixture.source_uid,
            fixture.source_gid,
        )
        .unwrap_err();
        assert!(error
            .to_string()
            .contains("receipt must be beneath the destination"));
    }

    #[test]
    fn python_boundary_exceptions_are_exact_and_classified() {
        assert_eq!(
            python_boundary_exception("jain-router/tools/refit_export_c08.py"),
            Some("frozen-offline-parity-oracle")
        );
        assert_eq!(
            python_boundary_exception("jain-smartcluster/jope/ten_guest_activation_receipt.py"),
            Some("temporary-protected-rust-port-cycle")
        );
        for near_miss in [
            "jain-router/tools/refit_export_c08.py.bak",
            "jain-router/tools/another_export.py",
            "jain-smartcluster/jope/ten_guest_activation_receipt.py.bak",
            "jain-smartcluster/jope/another.py",
        ] {
            assert_eq!(python_boundary_exception(near_miss), None, "{near_miss}");
        }
    }

    #[test]
    fn python_boundary_excludes_preservation_and_generated_trees() {
        for excluded in [
            ".bundles",
            ".git",
            "target",
            ".stage",
            ".venv",
            "vendor",
            "node_modules",
        ] {
            assert!(python_scan_excluded_name(excluded), "{excluded}");
        }
        for source_name in ["bundles", ".bundle", "src", "python"] {
            assert!(!python_scan_excluded_name(source_name), "{source_name}");
        }
    }

    #[test]
    fn test_scratch_prefers_absolute_cargo_target_over_control_authority() {
        let control_root = Path::new("/opt/jain-ci/authority/control-plane");
        assert_eq!(
            test_temp_parent(
                Some(PathBuf::from("/bounded/bootstrap/cargo-target")),
                control_root,
            ),
            PathBuf::from("/bounded/bootstrap/cargo-target/test-tmp")
        );
        assert_eq!(
            test_temp_parent(Some(PathBuf::from("relative-target")), control_root),
            control_root.join("target/test-tmp")
        );
        assert_eq!(
            test_temp_parent(None, control_root),
            control_root.join("target/test-tmp")
        );
    }

    fn command(mut command: Command) {
        let output = command.output().unwrap();
        assert!(
            output.status.success(),
            "command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn init_source(root: &Path) -> (PathBuf, String) {
        let repo = root.join("source");
        let mut init = Command::new("git");
        init.args(["init", "-b", "main"]).arg(&repo);
        command(init);
        run_git_strict(&repo, &["config", "user.name", "Release Test"]).unwrap();
        run_git_strict(&repo, &["config", "user.email", "release@example.invalid"]).unwrap();
        fs::write(repo.join("payload.txt"), "reviewed\n").unwrap();
        run_git_strict(&repo, &["add", "payload.txt"]).unwrap();
        run_git_strict(&repo, &["commit", "-m", "reviewed onboarding"]).unwrap();
        let sha = resolve_commit(&repo, "HEAD").unwrap();
        (repo, sha)
    }

    fn standalone_physical_clone(root: &Path, name: &str, destination: &Path) {
        let sources = root.join("target/fixture-sources");
        fs::create_dir_all(&sources).unwrap();
        let source = sources.join(name);
        let mut init = Command::new("git");
        init.args(["init", "-b", "main"]).arg(&source);
        command(init);
        run_git_strict(&source, &["config", "user.name", "Redline Fixture"]).unwrap();
        run_git_strict(
            &source,
            &["config", "user.email", "redline-fixture@example.invalid"],
        )
        .unwrap();
        fs::write(source.join("payload.txt"), format!("{name}\n")).unwrap();
        run_git_strict(&source, &["add", "payload.txt"]).unwrap();
        run_git_strict(&source, &["commit", "-m", "fixture source"]).unwrap();
        fs::create_dir_all(destination.parent().unwrap()).unwrap();
        let mut clone = Command::new("git");
        clone
            .args(["clone", "--quiet", "--no-local"])
            .arg(&source)
            .arg(destination);
        command(clone);
        run_git_strict(destination, &["remote", "remove", "origin"]).unwrap();
    }

    fn synthetic_nested_engine_topology(
        root: &Path,
        release: &str,
    ) -> (toml::Value, NestedEngineTopology) {
        let child = release == "8.0.1";
        let container = if child {
            root.join("jain-redline")
        } else {
            root.join("redline-split")
        };
        let control = if child {
            container.join("redline-split-ops")
        } else {
            root.join("redline-split-ops")
        };
        standalone_physical_clone(root, "control-source", &control);
        for name in ["redline-core", "redline-web"] {
            standalone_physical_clone(root, &format!("{name}-source"), &container.join(name));
        }
        let core_path = if child {
            "../redline-core"
        } else {
            "../redline-split/redline-core"
        };
        let web_path = if child {
            "../redline-web"
        } else {
            "../redline-split/redline-web"
        };
        let nested_container = if child { ".." } else { "../redline-split" };
        let engine_tag = if child {
            "redline-core-v4.1.0-jain.4"
        } else {
            "redline-core-v4.1.0-jain.1"
        };
        let engine_commit =
            git_query(&container.join("redline-core"), &["rev-parse", "HEAD"]).unwrap();
        let engine_tree = git_query(
            &container.join("redline-core"),
            &["rev-parse", "HEAD^{tree}"],
        )
        .unwrap();
        let engine_checksum =
            git_archive_sha256(&container.join("redline-core"), &engine_commit).unwrap();
        fs::write(
            control.join("repos.manifest.toml"),
            format!(
                r#"family = "redline-split"
container = "{nested_container}"
lock = "redline.lock.toml"
[control_plane]
name = "redline-split-ops"
path = "."
remote = "http://127.0.0.1:8787/git/jeryu/redline-split-ops.git"
required_check = "redline-split-ops/required"
[[repo]]
name = "redline-core"
path = "{core_path}"
remote = "http://127.0.0.1:8787/git/jeryu/redline-core.git"
role = "canonical-engine"
product_version = "4.1.0"
tag_revision = {}
current_tag = "{engine_tag}"
release_commit = "{engine_commit}"
release_tree = "{engine_tree}"
release_checksum_sha256 = "{engine_checksum}"
required_check = "redline-core/required"
[[repo]]
name = "redline-web"
path = "{web_path}"
remote = "http://127.0.0.1:8787/git/jeryu/redline-web.git"
required_check = "redline-web/required"
"#,
                if child { 4 } else { 1 },
            ),
        )
        .unwrap();
        fs::write(
            control.join("redline.lock.toml"),
            "[proof]\ncutover_eligible = true\n",
        )
        .unwrap();
        let authority = if child {
            "authority_mode = \"child\"\n"
        } else {
            ""
        };
        let parent: toml::Value = format!(
            r#"release_version = "{release}"
split_root = "{}"
[external_dependencies.redline]
repository = "redline-core"
remote = "http://127.0.0.1:8787/git/jeryu/redline-core.git"
immutable_tag = "{engine_tag}"
product_version = "4.1.0"
tag_revision = {}
release_commit = "{engine_commit}"
release_tree = "{engine_tree}"
release_checksum_sha256 = "{engine_checksum}"
[nested_families.redline]
family = "redline-split"
{authority}manifest_path = "{}"
container_path = "{}"
control_plane = "{}"
required = true
engine_repository = "redline-core"
engine_remote = "http://127.0.0.1:8787/git/jeryu/redline-core.git"
engine_tag = "{engine_tag}"
engine_release_tree = "{engine_tree}"
"#,
            root.display(),
            if child { 4 } else { 1 },
            control.join("repos.manifest.toml").display(),
            container.display(),
            control.display(),
        )
        .parse()
        .unwrap();
        let topology = nested_engine_topology(&parent).unwrap();
        (parent, topology)
    }

    fn init_bare(root: &Path) -> PathBuf {
        let remote = root.join("remote.git");
        let mut init = Command::new("git");
        init.args(["init", "--bare"]).arg(&remote);
        command(init);
        remote
    }

    fn commit_next(repo: &Path) -> String {
        fs::write(repo.join("payload.txt"), "different history\n").unwrap();
        run_git_strict(repo, &["add", "payload.txt"]).unwrap();
        run_git_strict(repo, &["commit", "-m", "different"]).unwrap();
        resolve_commit(repo, "HEAD").unwrap()
    }

    fn read_json(path: &Path) -> JsonValue {
        serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
    }

    fn immutable_main_readback(required_check: &str) -> JsonValue {
        json!({
            "url": "/repos/jeryu/example/branches/main/protection",
            "required_status_checks": {
                "strict": true,
                "contexts": [required_check],
            },
            "required_pull_request_reviews": {
                "required_approving_review_count": 1,
            },
            "enforce_admins": {"enabled": true},
            "required_linear_history": {"enabled": true},
            "allow_force_pushes": {"enabled": false},
            "allow_deletions": {"enabled": false},
            "required_signatures": {"enabled": false},
            "required_jankurai_proof": {"enabled": false},
            "updated_at": "2026-07-16T18:00:00Z",
        })
    }

    struct JankuraiFixture {
        _root: TestDir,
        repo: PathBuf,
        commit: String,
        report_root: PathBuf,
        report: PathBuf,
        receipt: PathBuf,
        policy: PathBuf,
        baseline: PathBuf,
        auditor: PathBuf,
    }

    impl JankuraiFixture {
        fn new(label: &str) -> Self {
            Self::with_baseline(label, true)
        }

        fn without_baseline(label: &str) -> Self {
            Self::with_baseline(label, false)
        }

        fn with_baseline(label: &str, include_baseline: bool) -> Self {
            let root = TestDir::new(label);
            let repo = root.path().join("source");
            command({
                let mut command = Command::new("git");
                command.args(["init", "-b", "main"]).arg(&repo);
                command
            });
            run_git_strict(&repo, &["config", "user.name", "Release Test"]).unwrap();
            run_git_strict(&repo, &["config", "user.email", "release@example.invalid"]).unwrap();
            let policy = repo.join("agent/audit-policy.toml");
            let baseline = repo.join("agent/jankurai-baseline.json");
            fs::create_dir_all(policy.parent().unwrap()).unwrap();
            fs::write(repo.join("payload.txt"), "reviewed\n").unwrap();
            fs::write(
                &policy,
                "minimum_score = 85\nallowed_score_drop = 0\nrequired_tool = \"jankurai\"\nrequired_tool_version = \"1.6.11\"\n",
            )
            .unwrap();
            if include_baseline {
                fs::write(
                    &baseline,
                    serde_json::to_vec(&json!({
                        "schema": "jain.split.jankurai-baseline/v1",
                        "score": 90,
                        "caps": [],
                        "hard_findings": 0,
                        "auditor": "jankurai 1.6.11",
                    }))
                    .unwrap(),
                )
                .unwrap();
            }
            run_git_strict(&repo, &["add", "."]).unwrap();
            run_git_strict(&repo, &["commit", "-m", "reviewed audit fixture"]).unwrap();
            let commit = resolve_commit(&repo, "HEAD").unwrap();

            let report_root = root.path().join("proof");
            fs::create_dir(&report_root).unwrap();
            let report = report_root.join("report.json");
            let receipt = report_root.join("receipt.json");
            let auditor = root.path().join("jankurai");
            let mut auditor_file = fs::File::create(&auditor).unwrap();
            auditor_file
                .write_all(b"#!/bin/sh\necho 'jankurai 1.6.11'\n")
                .unwrap();
            auditor_file.sync_all().unwrap();
            drop(auditor_file);
            let mut permissions = fs::metadata(&auditor).unwrap().permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(&auditor, permissions).unwrap();

            Self {
                _root: root,
                repo,
                commit,
                report_root,
                report,
                receipt,
                policy,
                baseline,
                auditor,
            }
        }

        fn valid_report(&self) -> JsonValue {
            json!({
                "score": 92,
                "repo": ".",
                "auditor_version": "1.6.11",
                "input_fingerprint": format!("sha256:{}", "1".repeat(64)),
                "policy_fingerprint": format!(
                    "sha256:{}",
                    sha256_bytes(&fs::read(&self.policy).unwrap())
                ),
                "dirty_worktree": false,
                "git": {
                    "head": &self.commit,
                    "dirty_worktree": false,
                },
                "decision": {
                    "passed": true,
                    "minimum_score": 85,
                    "hard_findings": [],
                    "ratchet": {
                        "passed": true,
                        "baseline_score": 90,
                        "allowed_drop": 0,
                    },
                },
                "caps_applied": [],
                "conformance_decision": "pass",
                "conformance_blockers": [],
                "run_id": "auditor-run-1",
                "policy": {
                    "path": "./agent/audit-policy.toml",
                    "minimum_score": 85,
                    "auditor_version": "1.6.11",
                },
            })
        }

        fn args(&self, clean_start: bool) -> Vec<String> {
            vec![
                "--repository".to_owned(),
                "source".to_owned(),
                "--commit".to_owned(),
                self.commit.clone(),
                "--worktree".to_owned(),
                self.repo.display().to_string(),
                "--report-root".to_owned(),
                self.report_root.display().to_string(),
                "--report".to_owned(),
                self.report.display().to_string(),
                "--auditor".to_owned(),
                self.auditor.display().to_string(),
                "--attempt-id".to_owned(),
                "attempt-1".to_owned(),
                "--lane-conclusion".to_owned(),
                "success".to_owned(),
                "--clean-tracked-tree-start".to_owned(),
                clean_start.to_string(),
                "--receipt".to_owned(),
                self.receipt.display().to_string(),
            ]
        }

        fn validate(
            &self,
            report: &JsonValue,
            clean_start: bool,
        ) -> Result<(), Box<dyn std::error::Error>> {
            if self.receipt.exists() {
                fs::remove_file(&self.receipt).unwrap();
            }
            fs::write(&self.report, serde_json::to_vec(report).unwrap()).unwrap();
            jankurai_evidence_command(self.args(clean_start))
        }
    }

    fn rejected_report(label: &str, expected: &str, mutate: impl FnOnce(&mut JsonValue)) {
        let fixture = JankuraiFixture::new(label);
        let mut report = fixture.valid_report();
        mutate(&mut report);
        let error = fixture.validate(&report, true).unwrap_err().to_string();
        assert!(
            error.contains(expected),
            "{label}: expected {expected:?} in {error:?}"
        );
    }

    fn gate_failed_report(label: &str, expected: &str, mutate: impl FnOnce(&mut JsonValue)) {
        let fixture = JankuraiFixture::new(label);
        let mut report = fixture.valid_report();
        mutate(&mut report);
        let error = fixture.validate(&report, true).unwrap_err().to_string();
        assert!(
            error.contains(expected),
            "{label}: expected {expected:?} in {error:?}"
        );
        let evidence = read_json(&fixture.receipt);
        assert_eq!(evidence["status"], "fail");
        assert_eq!(evidence["authority_failures"], json!([]));
        assert!(
            evidence["gate_failures"]
                .as_array()
                .unwrap()
                .iter()
                .any(|failure| failure.as_str().unwrap().contains(expected)),
            "{label}: missing classified gate failure in {evidence}"
        );
    }

    #[test]
    fn jankurai_evidence_binds_valid_exact_sha_policy_and_auditor() {
        let fixture = JankuraiFixture::new("jankurai-valid");
        let mut report = fixture.valid_report();
        report["git"]["head"] = json!(&fixture.commit[..7]);
        fixture.validate(&report, true).unwrap();
        let evidence = read_json(&fixture.receipt);
        assert_eq!(
            evidence["schema_version"],
            "jain.jankurai-exact-sha-evidence/v1"
        );
        assert_eq!(evidence["status"], "pass");
        assert_eq!(evidence["repository"], "source");
        assert_eq!(evidence["commit"], fixture.commit);
        assert_eq!(evidence["report_identity"]["commit"], fixture.commit);
        assert_eq!(
            evidence["report_identity"]["git_head"],
            &fixture.commit[..7]
        );
        assert_eq!(evidence["run_id"], "auditor-run-1");
        assert_eq!(evidence["attempt_id"], "attempt-1");
        assert_eq!(evidence["score"], 92.0);
        assert_eq!(evidence["hard_findings"], 0);
        assert_eq!(evidence["caps_applied"], 0);
        assert_eq!(evidence["ratchet_passed"], true);
        assert_eq!(evidence["baseline"]["score"], 90.0);
        assert_eq!(evidence["baseline"]["mode"], "governed-baseline");
        assert_eq!(evidence["clean_tracked_tree_at_start"], true);
        assert_eq!(evidence["clean_tracked_tree_at_finish"], true);
        assert!(is_full_hex(
            evidence["auditor"]["sha256"].as_str().unwrap(),
            64
        ));
        assert!(is_full_hex(evidence["report_sha256"].as_str().unwrap(), 64));
        assert!(is_full_hex(
            evidence["policy"]["sha256"].as_str().unwrap(),
            64
        ));
        assert!(is_full_hex(
            evidence["baseline"]["sha256"].as_str().unwrap(),
            64
        ));
        assert_eq!(
            evidence["baseline"]["sha256"],
            sha256_bytes(&fs::read(&fixture.baseline).unwrap())
        );
    }

    #[test]
    fn jankurai_evidence_supports_governed_floor_without_a_baseline() {
        let fixture = JankuraiFixture::without_baseline("jankurai-floor-only");
        let mut report = fixture.valid_report();
        report["decision"]["ratchet"]["passed"] = json!(false);
        report["decision"]["ratchet"]["baseline_score"] = json!(92);
        fixture.validate(&report, true).unwrap();
        let evidence = read_json(&fixture.receipt);
        assert_eq!(evidence["status"], "pass");
        assert_eq!(evidence["baseline"]["configured"], false);
        assert_eq!(evidence["baseline"]["mode"], "policy-floor-only");
        assert!(evidence["baseline"]["sha256"].is_null());
        assert!(evidence["baseline"]["score"].is_null());
        assert!(evidence["baseline"]["auditor"].is_null());
        assert_eq!(evidence["ratchet_passed"], true);
        assert_eq!(
            evidence["report_identity"]["reported_ratchet_passed"],
            false
        );
        assert_eq!(evidence["authority_failures"], json!([]));
        assert_eq!(evidence["gate_failures"], json!([]));
    }

    #[test]
    fn jankurai_evidence_rejects_identity_policy_and_score_failures() {
        rejected_report("jankurai-repository", "repo=.", |report| {
            report["repo"] = json!("different");
        });
        rejected_report("jankurai-short-sha", "7- to 40-character", |report| {
            report["git"]["head"] = json!("111111");
        });
        rejected_report("jankurai-uppercase-sha", "7- to 40-character", |report| {
            report["git"]["head"] = json!("ABCDEF1");
        });
        rejected_report("jankurai-policy", "policy fingerprint", |report| {
            report["policy_fingerprint"] = json!(format!("sha256:{}", "2".repeat(64)));
        });
        rejected_report(
            "jankurai-policy-path",
            "governed repository policy",
            |report| {
                report["policy"]["path"] = json!("./agent/jankurai-baseline.json");
            },
        );
        rejected_report("jankurai-floor-identity", "score floor differs", |report| {
            report["decision"]["minimum_score"] = json!(84);
        });
        gate_failed_report("jankurai-floor", "below governed floor", |report| {
            report["score"] = json!(84);
        });
        gate_failed_report("jankurai-ratchet", "ratchet failed", |report| {
            report["score"] = json!(89);
            report["decision"]["ratchet"]["passed"] = json!(false);
        });
        gate_failed_report("jankurai-conformance", "conformance", |report| {
            report["conformance_decision"] = json!("fail");
            report["conformance_blockers"] = json!(["blocked"]);
        });
        gate_failed_report("jankurai-hard", "hard findings present", |report| {
            report["decision"]["hard_findings"] = json!([{"rule": "hard"}]);
        });
        gate_failed_report("jankurai-cap", "caps applied", |report| {
            report["caps_applied"] = json!(["cap"]);
        });
        rejected_report("jankurai-auditor", "auditor version mismatch", |report| {
            report["auditor_version"] = json!("1.6.10");
        });
        rejected_report(
            "jankurai-report-dirty",
            "dirty tracked worktree",
            |report| {
                report["dirty_worktree"] = json!(true);
            },
        );
    }

    #[test]
    fn jankurai_evidence_preserves_a_historical_baseline_auditor() {
        let mut fixture = JankuraiFixture::new("jankurai-baseline-auditor");
        let mut baseline = read_json(&fixture.baseline);
        baseline["auditor"] = json!("jankurai 1.6.10");
        fs::write(&fixture.baseline, serde_json::to_vec(&baseline).unwrap()).unwrap();
        run_git_strict(&fixture.repo, &["add", "agent/jankurai-baseline.json"]).unwrap();
        run_git_strict(
            &fixture.repo,
            &["commit", "-m", "historical baseline auditor"],
        )
        .unwrap();
        fixture.commit = resolve_commit(&fixture.repo, "HEAD").unwrap();
        fixture.validate(&fixture.valid_report(), true).unwrap();
        let evidence = read_json(&fixture.receipt);
        assert_eq!(evidence["status"], "pass");
        assert_eq!(evidence["baseline"]["auditor"], "jankurai 1.6.10");
        assert_eq!(evidence["auditor"]["version"], "jankurai 1.6.11");
    }

    #[test]
    fn jankurai_evidence_rejects_dirty_start_finish_and_linked_report() {
        let fixture = JankuraiFixture::new("jankurai-dirty-start");
        let error = fixture
            .validate(&fixture.valid_report(), false)
            .unwrap_err()
            .to_string();
        assert!(error.contains("before audit"));

        let fixture = JankuraiFixture::new("jankurai-dirty-finish");
        fs::write(fixture.repo.join("payload.txt"), "dirty\n").unwrap();
        let error = fixture
            .validate(&fixture.valid_report(), true)
            .unwrap_err()
            .to_string();
        assert!(error.contains("after audit"));

        let fixture = JankuraiFixture::new("jankurai-hardlink-report");
        fs::write(
            &fixture.report,
            serde_json::to_vec(&fixture.valid_report()).unwrap(),
        )
        .unwrap();
        fs::hard_link(
            &fixture.report,
            fixture.report_root.join("report-copy.json"),
        )
        .unwrap();
        let error = jankurai_evidence_command(fixture.args(true))
            .unwrap_err()
            .to_string();
        assert!(error.contains("single-link"));

        let fixture = JankuraiFixture::new("jankurai-nonregular-policy");
        let report = fixture.valid_report();
        fs::remove_file(&fixture.policy).unwrap();
        fs::create_dir(&fixture.policy).unwrap();
        let error = fixture.validate(&report, true).unwrap_err().to_string();
        assert!(error.contains("governed repository policy"));

        let fixture = JankuraiFixture::new("jankurai-nonregular-baseline");
        let report = fixture.valid_report();
        fs::remove_file(&fixture.baseline).unwrap();
        fs::create_dir(&fixture.baseline).unwrap();
        let error = fixture.validate(&report, true).unwrap_err().to_string();
        assert!(error.contains("baseline must be a regular single-link"));
    }

    #[test]
    fn canonical_release_feature_matrices_derive_exact_cargo_commands() {
        let manifest: toml::Value = fs::read_to_string(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("repos.manifest.toml"),
        )
        .unwrap()
        .parse()
        .unwrap();

        let battle = release_cargo_policy(
            "jain-battle-gpu",
            release_repo_entry(&manifest, "jain-battle-gpu").unwrap(),
        )
        .unwrap();
        assert_eq!(battle["mode"], "feature-matrix");
        assert_eq!(battle["release_cuda_compute_capability_required"], false);
        assert_eq!(
            battle["commands"][0]["args"],
            json!([
                "build",
                "--locked",
                "--release",
                "--workspace",
                "--no-default-features",
                "--features",
                "battle-gpu/gpu,battle-gpu/gpu-dynamic-loading",
                "--all-targets",
            ])
        );
        assert_eq!(
            battle["commands"][1]["args"],
            json!([
                "test",
                "--locked",
                "--workspace",
                "--no-default-features",
                "--features",
                "battle-gpu/gpu,battle-gpu/gpu-dynamic-loading",
                "--all-targets",
            ])
        );
        assert_eq!(
            battle["commands"][2]["args"],
            json!([
                "build",
                "--locked",
                "--release",
                "--workspace",
                "--no-default-features",
                "--features",
                "battle-gpu/gpu-dynamic-linking",
                "--all-targets",
            ])
        );
        assert_eq!(
            battle["commands"][3]["args"],
            json!([
                "test",
                "--locked",
                "--workspace",
                "--no-default-features",
                "--features",
                "battle-gpu/gpu-dynamic-linking",
                "--all-targets",
            ])
        );

        let core = release_cargo_policy(
            "jain-core",
            release_repo_entry(&manifest, "jain-core").unwrap(),
        )
        .unwrap();
        assert_eq!(core["mode"], "feature-matrix");
        assert_eq!(core["release_cuda_compute_capability_required"], true);
        assert_eq!(
            core["commands"][0]["args"],
            json!([
                "build",
                "--locked",
                "--release",
                "--workspace",
                "--no-default-features",
                "--features",
                "feat-core/ci-smoke,feat-core/catboost,feat-core/xgboost,feat-core/lightgbm,feat-core/jable,feat-core/jable_required_smoke,feat-core/starforge-cpu,feat-core/hyperion-cpu,feat-core/jope,feat-core/invention-gpu",
                "--all-targets",
            ])
        );
        assert_eq!(
            core["commands"][1]["args"],
            json!([
                "test",
                "--locked",
                "--workspace",
                "--no-default-features",
                "--features",
                "feat-core/ci-smoke,feat-core/catboost,feat-core/xgboost,feat-core/lightgbm,feat-core/jable,feat-core/jable_required_smoke,feat-core/starforge-cpu,feat-core/hyperion-cpu,feat-core/jope,feat-core/invention-gpu",
                "--all-targets",
            ])
        );
        assert_eq!(
            core["commands"][2]["args"],
            json!([
                "build",
                "--locked",
                "--release",
                "--workspace",
                "--no-default-features",
                "--features",
                "feat-core/ci-smoke,feat-core/catboost,feat-core/xgboost,feat-core/lightgbm,feat-core/jable,feat-core/jable_required_smoke,feat-core/starforge-cpu,feat-core/starforge-cuda,feat-core/hyperion-cpu,feat-core/hyperion-cuda,feat-core/jope",
                "--all-targets",
            ])
        );
        assert_eq!(
            core["commands"][3]["args"],
            json!([
                "test",
                "--locked",
                "--workspace",
                "--no-default-features",
                "--features",
                "feat-core/ci-smoke,feat-core/catboost,feat-core/xgboost,feat-core/lightgbm,feat-core/jable,feat-core/jable_required_smoke,feat-core/starforge-cpu,feat-core/starforge-cuda,feat-core/hyperion-cpu,feat-core/hyperion-cuda,feat-core/jope",
                "--all-targets",
            ])
        );

        let central = release_cargo_policy(
            "redline-central",
            release_repo_entry(&manifest, "redline-central").unwrap(),
        )
        .unwrap();
        assert_eq!(central["mode"], "feature-matrix");
        assert_eq!(central["commands"].as_array().unwrap().len(), 6);
        for (index, feature) in ["backend-redline", "oracle-sqlite", "oracle-postgres"]
            .iter()
            .enumerate()
        {
            assert_eq!(
                central["commands"][index * 2]["args"][6],
                format!("db-shim/{feature}")
            );
            assert_eq!(
                central["commands"][index * 2 + 1]["args"][5],
                format!("db-shim/{feature}")
            );
        }

        let nested_control = release_cargo_policy(
            "redline-split-ops",
            release_repo_entry(&manifest, "redline-split-ops").unwrap(),
        )
        .unwrap();
        assert_eq!(nested_control["mode"], "all-features");

        let external = release_cargo_policy(
            "redline-core",
            release_repo_entry(&manifest, "redline-core").unwrap(),
        )
        .unwrap();
        assert_eq!(external["mode"], "all-features");

        let generic: toml::Value = "name = \"example\"".parse().unwrap();
        let generic = release_cargo_policy("example", &generic).unwrap();
        assert_eq!(generic["mode"], "all-features");
        assert_eq!(generic["release_cuda_compute_capability_required"], false);
        assert_eq!(
            generic["commands"][0]["args"],
            json!([
                "build",
                "--locked",
                "--release",
                "--all-features",
                "--all-targets"
            ])
        );
        assert_eq!(
            generic["commands"][1]["args"],
            json!(["test", "--locked", "--all-features", "--all-targets"])
        );
    }

    #[test]
    fn host_ci_authority_covers_root_nested_external_and_infrastructure_rows() {
        let manifest: toml::Value = fs::read_to_string(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("repos.manifest.toml"),
        )
        .unwrap()
        .parse()
        .unwrap();

        for (repo, owner) in [
            ("jain-split-ops", "veox"),
            ("jain-report", "veox"),
            ("jain-smartcluster", "veox"),
            ("redline-core", "jeryu"),
            ("redline-split-ops", "jeryu"),
            ("redline-central", "jeryu"),
            ("jeryu", "jeryu"),
            ("jeryu-release-ops", "jeryu"),
            ("jeryu-web", "jeryu"),
        ] {
            let authority = host_ci_authority(&manifest, repo).unwrap();
            assert_eq!(authority["repository"], repo);
            assert_eq!(authority["forge_owner"], owner);
            assert_eq!(authority["required_check"], format!("{repo}/required"));
            assert_eq!(
                authority["remote"],
                format!("http://127.0.0.1:8787/git/{owner}/{repo}.git")
            );
            assert_eq!(authority["release_cuda_compute_capability_required"], false);
        }

        for repo in ["jain-starforge", "jain-core", "jain-cli", "jain-web"] {
            let authority = host_ci_authority(&manifest, repo).unwrap();
            assert_eq!(authority["release_cuda_compute_capability_required"], true);
            let release =
                release_cargo_policy(repo, release_repo_entry(&manifest, repo).unwrap()).unwrap();
            assert_eq!(release["release_cuda_compute_capability_required"], true);
        }
    }

    #[test]
    fn host_ci_authority_rejects_missing_duplicate_and_noncanonical_rows() {
        let manifest: toml::Value = r#"
[control_plane]
name = "jain-split-ops"
forge_owner = "jeryu"
remote = "http://127.0.0.1:8787/git/jeryu/jain-split-ops.git"
required_check = "jain-split-ops/required"
[nested_families.redline]
forge_owner = "jeryu"
control_plane_name = "redline-split-ops"
control_plane_remote = "http://127.0.0.1:8787/git/jeryu/redline-split-ops.git"
control_plane_required_check = "redline-split-ops/required"
[[nested_families.redline.pending_repository]]
name = "redline-central"
forge_owner = "jeryu"
remote = "http://127.0.0.1:8787/git/jeryu/redline-central.git"
required_check = "redline-central/required"
"#
        .parse()
        .unwrap();
        assert!(host_ci_authority(&manifest, "absent").is_err());

        let mut duplicate = manifest.clone();
        let pending = duplicate["nested_families"]["redline"]["pending_repository"]
            .as_array_mut()
            .unwrap();
        pending.push(pending[0].clone());
        assert!(host_ci_authority(&duplicate, "redline-central")
            .unwrap_err()
            .contains("absent or ambiguous"));

        let mut wrong_remote = manifest.clone();
        wrong_remote["nested_families"]["redline"]["control_plane_remote"] =
            toml::Value::String("http://127.0.0.1:8787/git/jeryu/other.git".to_owned());
        assert!(host_ci_authority(&wrong_remote, "redline-split-ops")
            .unwrap_err()
            .contains("non-canonical remote"));

        let mut wrong_check = manifest;
        wrong_check["nested_families"]["redline"]["control_plane_required_check"] =
            toml::Value::String("redline-split-ops/wrong".to_owned());
        assert!(host_ci_authority(&wrong_check, "redline-split-ops")
            .unwrap_err()
            .contains("must require exact"));

        let stale_product: toml::Value = r#"
[[repo]]
name = "jain-report"
forge_owner = "jeryu"
jeryu_slug = "jeryu/jain-report"
required_check = "jain-report/required"
"#
        .parse()
        .unwrap();
        assert!(host_ci_authority(&stale_product, "jain-report")
            .unwrap_err()
            .contains("non-canonical forge owner"));

        let bad_cuda_policy: toml::Value = r#"
[[repo]]
name = "jain-report"
forge_owner = "veox"
jeryu_slug = "veox/jain-report"
required_check = "jain-report/required"
release_cuda_compute_capability_required = "yes"
"#
        .parse()
        .unwrap();
        assert!(host_ci_authority(&bad_cuda_policy, "jain-report")
            .unwrap_err()
            .contains("must be a boolean"));
    }

    #[test]
    fn registered_nested_family_declaration_rejects_aliases_and_duplicate_identity() {
        let manifest: toml::Value = fs::read_to_string(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("repos.manifest.toml"),
        )
        .unwrap()
        .parse()
        .unwrap();
        let split_root = Path::new("/home/ubuntu/jain-split");
        let canonical = &manifest["nested_families"]["jeryu"];
        validate_registered_nested_family_declaration("jeryu", canonical, split_root).unwrap();

        let mut old_root = canonical.clone();
        old_root["container_path"] = toml::Value::String("/home/ubuntu/jeryu-split".to_owned());
        assert!(
            validate_registered_nested_family_declaration("jeryu", &old_root, split_root)
                .unwrap_err()
                .contains("direct child of split_root")
        );

        let mut alternate_redline = canonical.clone();
        alternate_redline["redline_authority"] =
            toml::Value::String("/home/ubuntu/jain-split/jeryu-split/jeryu-redline".to_owned());
        assert!(validate_registered_nested_family_declaration(
            "jeryu",
            &alternate_redline,
            split_root
        )
        .unwrap_err()
        .contains("redline_authority must be /home/ubuntu/jain-split/jain-redline"));

        let mut owner_alias = canonical.clone();
        owner_alias["forge_owner"] = toml::Value::String("veox".to_owned());
        assert!(
            validate_registered_nested_family_declaration("jeryu", &owner_alias, split_root)
                .unwrap_err()
                .contains("control_plane_remote must be http://127.0.0.1:8787/git/veox/")
        );

        let mut duplicate = canonical.clone();
        let repositories = duplicate["repository"].as_array_mut().unwrap();
        repositories.push(repositories[0].clone());
        assert!(
            validate_registered_nested_family_declaration("jeryu", &duplicate, split_root)
                .unwrap_err()
                .contains("duplicate repository name jeryu")
        );

        let mut premature_symlink_enforcement = canonical.clone();
        premature_symlink_enforcement["symlink_policy"] =
            toml::Value::String("enforced".to_owned());
        assert!(validate_registered_nested_family_declaration(
            "jeryu",
            &premature_symlink_enforcement,
            split_root
        )
        .unwrap_err()
        .contains("cannot be enforced while a repository is retirement-pending"));
    }

    #[test]
    fn registered_nested_family_projection_must_match_child_authority() {
        let manifest: toml::Value = fs::read_to_string(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("repos.manifest.toml"),
        )
        .unwrap()
        .parse()
        .unwrap();
        let outer = &manifest["nested_families"]["jeryu"];
        let mut child: toml::Value = r#"
repo_family = "jeryu-split"
release_identity = "jeryu-split"
release_lineage = "v5"
split_root = "/home/ubuntu/jain-split/jeryu-split"
manifest_authority = "/home/ubuntu/jain-split/jeryu-split/jeryu-release-ops/repos.manifest.toml"
[control_plane]
name = "jeryu-release-ops"
path = "/home/ubuntu/jain-split/jeryu-split/jeryu-release-ops"
remote = "http://127.0.0.1:8787/git/jeryu/jeryu-release-ops.git"
required_check = "jeryu-release-ops/required"
current_tag = "jeryu-release-ops-v5.0.0-split.0"
inventory_status = "active"
runtime_authority = "control-plane"
[nested_families.redline]
source_authority = "jain-redline"
container_path = "/home/ubuntu/jain-split/jain-redline"
"#
        .parse()
        .unwrap();
        child
            .as_table_mut()
            .unwrap()
            .insert("repo".to_owned(), outer.get("repository").unwrap().clone());
        compare_registered_nested_family_child("jeryu", outer, &child).unwrap();

        let mut drift = child;
        drift["repo"][0]["runtime_authority"] = toml::Value::String("shadow-only".to_owned());
        assert!(
            compare_registered_nested_family_child("jeryu", outer, &drift)
                .unwrap_err()
                .contains("repository[jeryu].runtime_authority differs")
        );
    }

    #[test]
    fn release_feature_matrix_rejects_partial_unsafe_and_non_maximal_policy() {
        let partial: toml::Value = r#"
release_feature_sets = [["gpu"], ["gpu-dynamic-linking"]]
"#
        .parse()
        .unwrap();
        assert!(release_feature_matrix(&partial)
            .unwrap_err()
            .contains("release_package must be a string"));

        let unsafe_feature: toml::Value = r#"
release_package = "battle-gpu"
release_feature_sets = [["gpu"], ["gpu,dynamic-linking"]]
"#
        .parse()
        .unwrap();
        assert!(release_feature_matrix(&unsafe_feature)
            .unwrap_err()
            .contains("unsafe Cargo token"));

        let non_maximal: toml::Value = r#"
release_package = "battle-gpu"
release_feature_sets = [["gpu"], ["gpu", "gpu-dynamic-loading"]]
"#
        .parse()
        .unwrap();
        assert!(release_feature_matrix(&non_maximal)
            .unwrap_err()
            .contains("is not maximal"));
    }

    #[test]
    fn bootstrap_main_is_dry_run_cas_idempotent_and_refuses_history() {
        let root = TestDir::new("bootstrap-main");
        let (repo, reviewed) = init_source(root.path());
        let remote = init_bare(root.path());
        let receipt = root.path().join("bootstrap.json");
        let base_args = || {
            vec![
                "--repo".to_owned(),
                repo.display().to_string(),
                "--remote".to_owned(),
                remote.display().to_string(),
                "--reviewed-commit".to_owned(),
                reviewed.clone(),
                "--receipt".to_owned(),
                receipt.display().to_string(),
            ]
        };

        bootstrap_main_command(base_args()).unwrap();
        assert_eq!(
            ls_remote_ref(&repo, remote.to_str().unwrap(), "refs/heads/main").unwrap(),
            None
        );
        assert_eq!(read_json(&receipt)["action"], "would-create");

        let mut apply = base_args();
        apply.push("--apply".to_owned());
        bootstrap_main_command(apply.clone()).unwrap();
        assert_eq!(
            ls_remote_ref(&repo, remote.to_str().unwrap(), "refs/heads/main").unwrap(),
            Some(reviewed.clone())
        );
        bootstrap_main_command(apply).unwrap();
        assert_eq!(read_json(&receipt)["action"], "verified-existing");

        let different = commit_next(&repo);
        let mut refuse = base_args();
        let index = refuse
            .iter()
            .position(|value| value == "--reviewed-commit")
            .unwrap();
        refuse[index + 1] = different;
        refuse.push("--apply".to_owned());
        assert!(bootstrap_main_command(refuse).is_err());
        assert_eq!(read_json(&receipt)["status"], "fail");
        assert_eq!(
            ls_remote_ref(&repo, remote.to_str().unwrap(), "refs/heads/main").unwrap(),
            Some(reviewed)
        );
    }

    #[test]
    fn immutable_tag_is_dry_run_exact_idempotent_and_never_moves() {
        let root = TestDir::new("immutable-tag");
        let (repo, reviewed) = init_source(root.path());
        let remote = init_bare(root.path());
        run_git_strict(
            &repo,
            &["remote", "add", "origin", remote.to_str().unwrap()],
        )
        .unwrap();
        let main_refspec = format!("{reviewed}:refs/heads/main");
        run_git_strict(&repo, &["push", remote.to_str().unwrap(), &main_refspec]).unwrap();
        let token_root = TestDir::new_private_temp("immutable-tag-token");
        let token_file = token_root.path().join("token");
        fs::write(&token_file, b"fixture-token-0123456789\n").unwrap();
        fs::set_permissions(&token_file, fs::Permissions::from_mode(0o600)).unwrap();
        let mut report = receipt_header("test", "immutable-tag", false);
        for invalid in ["HEAD", &reviewed[..12]] {
            assert!(create_or_verify_immutable_tag(
                &repo,
                remote.to_str().unwrap(),
                "example-v8.0.0-split.0",
                invalid,
                &token_file,
                true,
                &mut report,
            )
            .unwrap_err()
            .to_string()
            .contains("lowercase full 40-character SHA"));
        }
        assert_eq!(
            secure_ls_remote_at(
                remote.to_str().unwrap(),
                "refs/tags/example-v8.0.0-split.0",
                &token_file,
            )
            .unwrap(),
            None
        );
        create_or_verify_immutable_tag(
            &repo,
            remote.to_str().unwrap(),
            "example-v8.0.0-split.0",
            &reviewed,
            &token_file,
            false,
            &mut report,
        )
        .unwrap();
        assert_eq!(
            secure_local_ref_commit(&repo, "refs/tags/example-v8.0.0-split.0").unwrap(),
            None
        );
        for _ in 0..2 {
            create_or_verify_immutable_tag(
                &repo,
                remote.to_str().unwrap(),
                "example-v8.0.0-split.0",
                &reviewed,
                &token_file,
                true,
                &mut report,
            )
            .unwrap();
        }
        assert_eq!(report["action"], "verified-existing");

        let different = commit_next(&repo);
        assert!(create_or_verify_immutable_tag(
            &repo,
            remote.to_str().unwrap(),
            "example-v8.0.0-split.0",
            &different,
            &token_file,
            true,
            &mut report,
        )
        .is_err());
        assert_eq!(
            secure_local_ref_commit(&repo, "refs/tags/example-v8.0.0-split.0").unwrap(),
            Some(reviewed.clone())
        );
        assert_eq!(
            ls_remote_ref(
                &repo,
                remote.to_str().unwrap(),
                "refs/tags/example-v8.0.0-split.0"
            )
            .unwrap(),
            Some(reviewed)
        );
    }

    #[test]
    fn immutable_tag_retains_prior_suffix_if_main_advances_during_cas() {
        let root = TestDir::new("immutable-tag-main-race");
        let (repo, reviewed) = init_source(root.path());
        let remote = init_bare(root.path());
        run_git_strict(
            &repo,
            &["remote", "add", "origin", remote.to_str().unwrap()],
        )
        .unwrap();
        let main_refspec = format!("{reviewed}:refs/heads/main");
        run_git_strict(&repo, &["push", remote.to_str().unwrap(), &main_refspec]).unwrap();
        let successor = commit_next(&repo);
        let successor_refspec = format!("{successor}:refs/heads/race-successor");
        run_git_strict(
            &repo,
            &["push", remote.to_str().unwrap(), &successor_refspec],
        )
        .unwrap();

        let hook = remote.join("hooks/pre-receive");
        fs::write(
            &hook,
            format!(
                "#!/bin/sh\nunset GIT_QUARANTINE_PATH\n/usr/bin/git update-ref refs/heads/main {successor} {reviewed}\n"
            ),
        )
        .unwrap();
        fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).unwrap();

        let token_root = TestDir::new_private_temp("immutable-tag-race-token");
        let token_file = token_root.path().join("token");
        fs::write(&token_file, b"fixture-token-0123456789\n").unwrap();
        fs::set_permissions(&token_file, fs::Permissions::from_mode(0o600)).unwrap();
        let mut report = receipt_header("test", "immutable-tag", true);
        let error = create_or_verify_immutable_tag(
            &repo,
            remote.to_str().unwrap(),
            "example-v8.0.0-split.0",
            &reviewed,
            &token_file,
            true,
            &mut report,
        )
        .unwrap_err();
        assert!(
            error.to_string().contains("next unused suffix"),
            "unexpected error: {error}"
        );
        assert_eq!(report["action"], "tag-retained-main-advanced");
        assert_eq!(report["requires_next_unused_tag"], true);
        assert_eq!(
            secure_ls_remote_at(
                remote.to_str().unwrap(),
                "refs/tags/example-v8.0.0-split.0",
                &token_file,
            )
            .unwrap(),
            Some(reviewed)
        );
        assert_eq!(
            secure_ls_remote_at(remote.to_str().unwrap(), "refs/heads/main", &token_file).unwrap(),
            Some(successor)
        );
    }

    #[test]
    fn worktree_verification_checks_clean_origin_main_and_remote_tag() {
        let root = TestDir::new("worktree-verification");
        let (repo, reviewed) = init_source(root.path());
        let remote = init_bare(root.path());
        run_git_strict(
            &repo,
            &["remote", "add", "origin", remote.to_str().unwrap()],
        )
        .unwrap();
        run_git_strict(&repo, &["push", "-u", "origin", "main"]).unwrap();
        let mut tag_report = receipt_header("test", "tag", true);
        let token_root = TestDir::new_private_temp("worktree-tag-token");
        let token_file = token_root.path().join("token");
        fs::write(&token_file, b"fixture-token-0123456789\n").unwrap();
        fs::set_permissions(&token_file, fs::Permissions::from_mode(0o600)).unwrap();
        create_or_verify_immutable_tag(
            &repo,
            remote.to_str().unwrap(),
            "example-v8.0.0-split.0",
            &reviewed,
            &token_file,
            true,
            &mut tag_report,
        )
        .unwrap();
        let managed = ManagedRepo {
            name: "example".to_owned(),
            path: repo.clone(),
            remote: remote.display().to_string(),
            required_check: "example/required".to_owned(),
            branch: "main".to_owned(),
            tag: Some("example-v8.0.0-split.0".to_owned()),
            kind: "family".to_owned(),
            family: "jain-split".to_owned(),
            family_registered: true,
            inventory_status: "active".to_owned(),
            runtime_authority: "product".to_owned(),
        };
        assert_eq!(verify_managed_worktree(&managed)["status"], "pass");

        fs::write(repo.join("untracked.txt"), "dirty\n").unwrap();
        assert_eq!(verify_managed_worktree(&managed)["status"], "fail");
        fs::remove_file(repo.join("untracked.txt")).unwrap();
        run_git_strict(
            &repo,
            &["remote", "add", "unmanaged", remote.to_str().unwrap()],
        )
        .unwrap();
        assert_eq!(verify_managed_worktree(&managed)["status"], "fail");
        run_git_strict(&repo, &["remote", "remove", "unmanaged"]).unwrap();

        commit_next(&repo);
        assert_eq!(verify_managed_worktree(&managed)["status"], "fail");
    }

    #[test]
    fn worktree_verification_fails_when_auxiliary_registration_metadata_exists() {
        let root = TestDir::new("worktree-linked-ban");
        let (repo, _reviewed) = init_source(root.path());
        let remote = init_bare(root.path());
        run_git_strict(
            &repo,
            &["remote", "add", "origin", remote.to_str().unwrap()],
        )
        .unwrap();
        run_git_strict(&repo, &["push", "-u", "origin", "main"]).unwrap();
        let managed = ManagedRepo {
            name: "example".to_owned(),
            path: repo.clone(),
            remote: remote.display().to_string(),
            required_check: "example/required".to_owned(),
            branch: "main".to_owned(),
            tag: None,
            kind: "family".to_owned(),
            family: "jain-split".to_owned(),
            family_registered: true,
            inventory_status: "active".to_owned(),
            runtime_authority: "product".to_owned(),
        };
        assert_eq!(verify_managed_worktree(&managed)["status"], "pass");

        fs::create_dir(repo.join(".git/worktrees")).unwrap();
        let report = verify_managed_worktree(&managed);
        assert_eq!(report["status"], "fail");
        assert!(report["failures"]
            .as_array()
            .unwrap()
            .iter()
            .any(|failure| failure
                .as_str()
                .unwrap()
                .contains("forbidden auxiliary or shared Git metadata")));

        fs::remove_dir(repo.join(".git/worktrees")).unwrap();
        assert_eq!(verify_managed_worktree(&managed)["status"], "pass");
    }

    #[test]
    fn synthetic_registration_porcelain_requires_one_exact_primary() {
        let sha = "a".repeat(40);
        let valid = format!(
            "worktree /home/ubuntu/jain-split/example\0HEAD {sha}\0branch refs/heads/main\0\0"
        );
        let parsed = parse_single_primary_registration(valid.as_bytes()).unwrap();
        assert_eq!(parsed.path, Path::new("/home/ubuntu/jain-split/example"));
        assert_eq!(parsed.head, sha);
        assert_eq!(parsed.branch, "refs/heads/main");

        for invalid in [
            Vec::new(),
            b"worktree /one\0HEAD a\0branch refs/heads/main\0\0".to_vec(),
            format!("worktree /one\0HEAD {sha}\0detached\0\0").into_bytes(),
            format!(
                "worktree /one\0HEAD {sha}\0branch refs/heads/main\0\0worktree /two\0HEAD {sha}\0branch refs/heads/main\0\0"
            )
            .into_bytes(),
            format!(
                "worktree /one\0worktree /two\0HEAD {sha}\0branch refs/heads/main\0\0"
            )
            .into_bytes(),
        ] {
            assert!(parse_single_primary_registration(&invalid).is_err());
        }
    }

    #[test]
    fn jeryu_lifecycle_plans_exact_requests_and_policy() {
        let ready = plan_jeryu_lifecycle_request(
            "pr-ready",
            "jeryu/example",
            Some("7"),
            None,
            "main",
            None,
        )
        .unwrap();
        assert_eq!(ready.method(), "PATCH");
        assert_eq!(ready.path(), "/repos/jeryu/example/pulls/7");
        assert_eq!(
            serde_json::from_str::<JsonValue>(ready.body().unwrap()).unwrap(),
            json!({"draft": false})
        );
        let close = plan_jeryu_lifecycle_request(
            "pr-close",
            "jeryu/example",
            Some("7"),
            None,
            "main",
            None,
        )
        .unwrap();
        assert_eq!(
            serde_json::from_str::<JsonValue>(close.body().unwrap()).unwrap(),
            json!({"state": "closed"})
        );
        let expected_head = "a".repeat(40);
        let merge = plan_jeryu_lifecycle_request(
            "pr-merge",
            "jeryu/example",
            Some("7"),
            Some(&expected_head),
            "main",
            None,
        )
        .unwrap();
        assert_eq!(merge.method(), "PUT");
        assert_eq!(merge.path(), "/repos/jeryu/example/pulls/7/merge");
        assert_eq!(
            serde_json::from_str::<JsonValue>(merge.body().unwrap()).unwrap(),
            json!({"sha": expected_head, "merge_method": "merge"})
        );
        let protection = plan_jeryu_lifecycle_request(
            "protection-apply",
            "jeryu/example",
            None,
            None,
            "main",
            Some("example/required"),
        )
        .unwrap();
        let policy: JsonValue = serde_json::from_str(protection.body().unwrap()).unwrap();
        assert_eq!(policy, immutable_main_policy("example/required"));
        validate_protection_policy(
            &immutable_main_readback("example/required"),
            "jeryu/example",
            "main",
            "example/required",
        )
        .unwrap();
        assert!(validate_protection_policy(
            &json!({}),
            "jeryu/example",
            "main",
            "example/required"
        )
        .is_err());
        assert!(plan_jeryu_lifecycle_request(
            "pr-merge",
            "jeryu/example",
            Some("7"),
            None,
            "main",
            None,
        )
        .is_err());
    }

    #[test]
    fn jeryu_pr_open_readback_is_exact() {
        let sha = "a".repeat(40);
        let readback = json!({
            "number": 9,
            "state": "open",
            "head": {"ref": "codex/release", "sha": sha},
            "base": {"ref": "main"},
        });
        validate_pr_open_readback(&readback, 9, "codex/release", &sha, "main").unwrap();
        let mut wrong = readback.clone();
        wrong["head"]["sha"] = json!("b".repeat(40));
        assert!(validate_pr_open_readback(&wrong, 9, "codex/release", &sha, "main").is_err());
    }

    #[test]
    fn hostile_git_askpass_proxy_and_config_environment_names_are_rejected() {
        for name in [
            "GIT_CONFIG_COUNT",
            "GIT_ASKPASS",
            "SSH_ASKPASS",
            "SSH_ASKPASS_REQUIRE",
            "HTTP_PROXY",
            "https_proxy",
            "ALL_PROXY",
            "NO_PROXY",
            "JERYU_BASE",
            "JERYU_MERGE_TOKEN_FILE",
        ] {
            assert!(forbidden_jeryu_environment_name(name), "accepted {name}");
        }
        assert!(!forbidden_jeryu_environment_name("PATH"));
        assert!(!forbidden_jeryu_environment_name("LC_ALL"));
    }

    #[test]
    fn controlled_jeryu_askpass_is_prompt_exact_and_token_file_bound() {
        let root = TestDir::new_private_temp("jeryu-askpass");
        let token_file = root.path().join("token");
        fs::write(&token_file, b"fixture-token-0123456789").unwrap();
        fs::set_permissions(&token_file, fs::Permissions::from_mode(0o600)).unwrap();

        let mut username = Vec::new();
        write_jeryu_askpass_response(
            "Username for 'http://127.0.0.1:8787': ",
            None,
            &mut username,
        )
        .unwrap();
        assert_eq!(username, b"x-access-token\n");

        let mut password = Vec::new();
        write_jeryu_askpass_response(
            "Password for 'http://x-access-token@127.0.0.1:8787': ",
            Some(token_file.as_os_str()),
            &mut password,
        )
        .unwrap();
        assert_eq!(password, b"fixture-token-0123456789\n");
        assert!(write_jeryu_askpass_response(
            "Password for 'http://attacker.invalid': ",
            Some(token_file.as_os_str()),
            &mut Vec::new(),
        )
        .is_err());

        let command = secure_git_authenticated_command(None, &token_file).unwrap();
        let secret = OsStr::new("fixture-token-0123456789");
        assert!(command.command.get_args().all(|arg| arg != secret));
        assert!(command
            .command
            .get_envs()
            .all(|(name, value)| name != secret && value != Some(secret)));
        assert_eq!(
            command
                .command
                .get_envs()
                .find(|(name, _)| *name == OsStr::new(JERYU_ASKPASS_TOKEN_FILE))
                .and_then(|(_, value)| value),
            Some(token_file.as_os_str())
        );
        let askpass = command
            .command
            .get_envs()
            .find(|(name, _)| *name == OsStr::new("GIT_ASKPASS"))
            .and_then(|(_, value)| value)
            .unwrap();
        let askpass_metadata = fs::metadata(Path::new(askpass)).unwrap();
        let running_metadata = fs::metadata("/proc/self/exe").unwrap();
        assert_eq!(askpass_metadata.dev(), running_metadata.dev());
        assert_eq!(askpass_metadata.ino(), running_metadata.ino());
        let descriptor_flags =
            unsafe { libc::fcntl(command._askpass_executable.as_raw_fd(), libc::F_GETFD) };
        assert!(descriptor_flags >= 0);
        assert_eq!(descriptor_flags & libc::FD_CLOEXEC, 0);
    }

    #[test]
    fn advertised_head_selection_is_strict_and_deterministic() {
        let head = "a".repeat(40);
        let output = format!(
            "{head}\trefs/heads/zeta\n{}\trefs/heads/other\n{head}\trefs/heads/alpha\n",
            "b".repeat(40)
        );
        assert_eq!(
            parse_advertised_heads(&output, &head).unwrap(),
            vec!["refs/heads/alpha", "refs/heads/zeta"]
        );
        assert!(parse_advertised_heads("malformed", &head).is_err());
        assert!(parse_advertised_heads(&format!("{head}\trefs/tags/not-a-head\n"), &head).is_err());
        assert!(
            parse_advertised_heads(&format!("{}\trefs/heads/other\n", "b".repeat(40)), &head)
                .is_err()
        );
    }

    #[test]
    fn advertised_ancestor_tag_selection_is_unique_bounded_and_repository_scoped() {
        let object = "a".repeat(40);
        let valid = format!("{object}\trefs/tags/example-v7.0.1-split.5\n");
        assert_eq!(
            resolve_unique_advertised_ancestor_tag("jeryu/example", &valid, &object).unwrap(),
            "refs/tags/example-v7.0.1-split.5"
        );
        assert!(resolve_unique_advertised_ancestor_tag(
            "jeryu/example",
            &format!(
                "{object}\trefs/tags/example-v7.0.1-split.5\n{object}\trefs/tags/example-v7.0.1-split.6\n"
            ),
            &object,
        )
        .unwrap_err()
        .to_string()
        .contains("multiple"));
        assert!(resolve_unique_advertised_ancestor_tag(
            "jeryu/example",
            &format!("{object}\trefs/tags/other-v7.0.1-split.5\n"),
            &object,
        )
        .is_err());
        assert!(
            resolve_unique_advertised_ancestor_tag("jeryu/example", "malformed", &object,).is_err()
        );
        assert!(resolve_unique_advertised_ancestor_tag(
            "jeryu/example",
            &format!("{}\trefs/tags/example-v7.0.1-split.5\n", "b".repeat(40)),
            &object,
        )
        .unwrap_err()
        .to_string()
        .contains("not an advertised"));
        let overbound = (0..4097)
            .map(|index| {
                format!(
                    "{}\trefs/tags/example-v7.0.1-split.{index}\n",
                    "b".repeat(40)
                )
            })
            .collect::<String>();
        assert!(
            resolve_unique_advertised_ancestor_tag("jeryu/example", &overbound, &object,)
                .unwrap_err()
                .to_string()
                .contains("too many")
        );
    }

    #[test]
    fn authenticated_ref_readback_requires_exact_advertised_authority() {
        let root = TestDir::new("ref-readback");
        let (source, head) = init_source(root.path());
        let remote = init_bare(root.path());
        run_git_strict(
            &source,
            &[
                "push",
                remote.to_str().unwrap(),
                &format!("{head}:refs/heads/main"),
            ],
        )
        .unwrap();
        let token_root = TestDir::new_private_temp("ref-readback-token");
        let token_file = token_root.path().join("token");
        fs::write(&token_file, b"fixture-token-0123456789\n").unwrap();
        fs::set_permissions(&token_file, fs::Permissions::from_mode(0o600)).unwrap();
        let args = |expected: &str, token: &Path, reference: Option<&str>| {
            let mut args = vec![
                "ref-readback".to_owned(),
                "--repo".to_owned(),
                "jeryu/example".to_owned(),
                "--remote".to_owned(),
                remote.display().to_string(),
                "--expected-head".to_owned(),
                expected.to_owned(),
                "--token-file".to_owned(),
                token.display().to_string(),
            ];
            if let Some(reference) = reference {
                args.extend(["--ref".to_owned(), reference.to_owned()]);
            }
            args
        };

        jeryu_ref_readback(args(&head, &token_file, None)).unwrap();
        jeryu_ref_readback(args(&head, &token_file, Some("refs/heads/main"))).unwrap();
        assert!(jeryu_ref_readback(args(&"b".repeat(40), &token_file, None)).is_err());
        assert!(jeryu_ref_readback(args(
            &head,
            &root.path().join("missing-token"),
            Some("refs/heads/main")
        ))
        .is_err());
        assert!(jeryu_ref_readback(args(&head, &token_file, Some("refs/tags/main"))).is_err());
    }

    #[test]
    fn git_materialization_is_exact_standalone_and_credential_gated() {
        let root = TestDir::new("git-materialization");
        let (source, head) = init_source(root.path());
        let remote = init_bare(root.path());
        let refspec = format!("{head}:refs/heads/main");
        run_git_strict(&source, &["push", remote.to_str().unwrap(), &refspec]).unwrap();
        fs::write(source.join("feature-only"), b"ambient feature bytes\n").unwrap();
        run_git_strict(&source, &["add", "feature-only"]).unwrap();
        run_git_strict(&source, &["commit", "-m", "ambient feature head"]).unwrap();
        let ambient_head = resolve_commit(&source, "HEAD").unwrap();
        assert_ne!(ambient_head, head);
        fs::write(source.join("dirty-only"), b"ambient dirty bytes\n").unwrap();
        let token_root = TestDir::new_private_temp("git-materialization-token");
        let token_file = token_root.path().join("token");
        fs::write(&token_file, b"fixture-token-0123456789\n").unwrap();
        fs::set_permissions(&token_file, fs::Permissions::from_mode(0o600)).unwrap();
        let destination = root.path().join("materialized");
        let args = |token: &Path, destination: &Path| {
            vec![
                "git-materialize".to_owned(),
                "--repo".to_owned(),
                "jeryu/example".to_owned(),
                "--remote".to_owned(),
                remote.display().to_string(),
                "--ref".to_owned(),
                "refs/heads/main".to_owned(),
                "--expected-head".to_owned(),
                head.clone(),
                "--destination".to_owned(),
                destination.display().to_string(),
                "--token-file".to_owned(),
                token.display().to_string(),
            ]
        };
        let rejected = root.path().join("rejected");
        assert!(
            jeryu_git_materialize(args(&root.path().join("missing-token"), &rejected)).is_err()
        );
        assert!(!rejected.exists());

        jeryu_git_materialize(args(&token_file, &destination)).unwrap();
        assert_eq!(resolve_commit(&destination, "HEAD").unwrap(), head);
        assert!(strict_git_output(&destination, &["remote"])
            .unwrap()
            .trim()
            .is_empty());
        assert!(
            strict_git_output(&destination, &["status", "--porcelain=v1"])
                .unwrap()
                .trim()
                .is_empty()
        );
        let second = root.path().join("second");
        fs::create_dir(&second).unwrap();
        assert!(jeryu_git_materialize(args(&token_file, &second)).is_err());

        let resolved = root.path().join("resolved-main");
        jeryu_git_materialize(vec![
            "git-materialize".to_owned(),
            "--repo".to_owned(),
            "jeryu/example".to_owned(),
            "--remote".to_owned(),
            remote.display().to_string(),
            "--ref".to_owned(),
            "refs/heads/main".to_owned(),
            "--resolve-ref-head".to_owned(),
            "--destination".to_owned(),
            resolved.display().to_string(),
            "--token-file".to_owned(),
            token_file.display().to_string(),
        ])
        .unwrap();
        assert_eq!(resolve_commit(&resolved, "HEAD").unwrap(), head);
        assert!(strict_git_output(&resolved, &["remote"])
            .unwrap()
            .trim()
            .is_empty());

        let ambiguous = root.path().join("ambiguous-head-authority");
        let mut ambiguous_args = args(&token_file, &ambiguous);
        ambiguous_args.push("--resolve-ref-head".to_owned());
        assert!(jeryu_git_materialize(ambiguous_args).is_err());
        assert!(!ambiguous.exists());

        run_git_strict(
            &source,
            &[
                "push",
                remote.to_str().unwrap(),
                &format!("{ambient_head}:refs/heads/main"),
            ],
        )
        .unwrap();
        assert!(jeryu_ref_readback(vec![
            "ref-readback".to_owned(),
            "--repo".to_owned(),
            "jeryu/example".to_owned(),
            "--remote".to_owned(),
            remote.display().to_string(),
            "--ref".to_owned(),
            "refs/heads/main".to_owned(),
            "--expected-head".to_owned(),
            head,
            "--token-file".to_owned(),
            token_file.display().to_string(),
        ])
        .is_err());
    }

    #[test]
    fn git_materialization_retains_only_the_authenticated_declared_ancestor_tag() {
        let root = TestDir::new("git-materialization-release-tag");
        let (source, _) = init_source(root.path());
        fs::create_dir_all(source.join("agent")).unwrap();
        let tag = "example-v8.0.1-split.1";
        fs::write(
            source.join("agent/standard-version.toml"),
            format!(
                "schema_version = \"1.0.0\"\nworkspace = \"example\"\nversion = \"{tag}\"\nrelease_authority = \"jain-deploy\"\n"
            ),
        )
        .unwrap();
        run_git_strict(&source, &["add", "agent/standard-version.toml"]).unwrap();
        run_git_strict(&source, &["commit", "-m", "declare release baseline"]).unwrap();
        let baseline = resolve_commit(&source, "HEAD").unwrap();
        run_git_strict(
            &source,
            &["update-ref", &format!("refs/tags/{tag}"), &baseline],
        )
        .unwrap();
        fs::write(source.join("successor"), b"reviewed successor\n").unwrap();
        run_git_strict(&source, &["add", "successor"]).unwrap();
        run_git_strict(&source, &["commit", "-m", "reviewed successor"]).unwrap();
        let head = resolve_commit(&source, "HEAD").unwrap();
        let remote = init_bare(root.path());
        run_git_strict(
            &source,
            &[
                "push",
                remote.to_str().unwrap(),
                &format!("{head}:refs/heads/main"),
                &format!("refs/tags/{tag}:refs/tags/{tag}"),
            ],
        )
        .unwrap();
        let token_root = TestDir::new_private_temp("git-materialization-release-tag-token");
        let token_file = token_root.path().join("token");
        fs::write(&token_file, b"fixture-token-0123456789\n").unwrap();
        fs::set_permissions(&token_file, fs::Permissions::from_mode(0o600)).unwrap();
        let args = |destination: &Path| {
            vec![
                "git-materialize".to_owned(),
                "--repo".to_owned(),
                "jeryu/example".to_owned(),
                "--remote".to_owned(),
                remote.display().to_string(),
                "--ref".to_owned(),
                "refs/heads/main".to_owned(),
                "--expected-head".to_owned(),
                head.clone(),
                "--destination".to_owned(),
                destination.display().to_string(),
                "--token-file".to_owned(),
                token_file.display().to_string(),
                "--retain-declared-release-tag".to_owned(),
            ]
        };

        let retained = root.path().join("retained");
        jeryu_git_materialize(args(&retained)).unwrap();
        assert_eq!(
            resolve_commit(&retained, &format!("refs/tags/{tag}")).unwrap(),
            baseline
        );
        assert_eq!(
            strict_git_output(
                &retained,
                &["for-each-ref", "--format=%(refname)", "refs/tags"]
            )
            .unwrap(),
            format!("refs/tags/{tag}")
        );

        run_git_strict(&source, &["update-ref", &format!("refs/tags/{tag}"), &head]).unwrap();
        run_git_strict(
            &source,
            &[
                "push",
                "--force",
                remote.to_str().unwrap(),
                &format!("refs/tags/{tag}:refs/tags/{tag}"),
            ],
        )
        .unwrap();
        assert!(jeryu_ref_readback(vec![
            "ref-readback".to_owned(),
            "--repo".to_owned(),
            "jeryu/example".to_owned(),
            "--remote".to_owned(),
            remote.display().to_string(),
            "--ref".to_owned(),
            format!("refs/tags/{tag}"),
            "--expected-head".to_owned(),
            baseline.clone(),
            "--token-file".to_owned(),
            token_file.display().to_string(),
        ])
        .is_err());

        run_git_strict(
            &source,
            &[
                "push",
                remote.to_str().unwrap(),
                &format!(":refs/tags/{tag}"),
            ],
        )
        .unwrap();
        let absent = root.path().join("absent");
        jeryu_git_materialize(args(&absent)).unwrap();
        assert!(strict_git_output(
            &absent,
            &["for-each-ref", "--format=%(refname)", "refs/tags"]
        )
        .unwrap()
        .is_empty());

        let tree = strict_git_output(&source, &["rev-parse", "HEAD^{tree}"]).unwrap();
        let unrelated = strict_git_output(
            &source,
            &["commit-tree", &tree, "-m", "unrelated tag target"],
        )
        .unwrap();
        run_git_strict(
            &source,
            &["update-ref", &format!("refs/tags/{tag}"), &unrelated],
        )
        .unwrap();
        run_git_strict(
            &source,
            &[
                "push",
                remote.to_str().unwrap(),
                &format!("refs/tags/{tag}:refs/tags/{tag}"),
            ],
        )
        .unwrap();
        let rejected = root.path().join("non-ancestor");
        let error = jeryu_git_materialize(args(&rejected)).unwrap_err();
        assert!(error.to_string().contains("not an ancestor"));
        assert!(!rejected.exists());
    }

    #[test]
    fn git_materialization_retains_only_the_requested_annotated_ancestor_object() {
        let root = TestDir::new("git-materialization-ancestor-object");
        let (source, baseline) = init_source(root.path());
        let tag = "example-v7.0.1-split.5";
        run_git_strict(
            &source,
            &["tag", "-a", tag, "-m", "contract baseline", &baseline],
        )
        .unwrap();
        let tag_object = strict_git_output(&source, &["rev-parse", tag]).unwrap();
        assert_ne!(tag_object, baseline);
        fs::write(source.join("successor"), b"protected successor\n").unwrap();
        run_git_strict(&source, &["add", "successor"]).unwrap();
        run_git_strict(&source, &["commit", "-m", "protected successor"]).unwrap();
        let head = resolve_commit(&source, "HEAD").unwrap();
        let remote = init_bare(root.path());
        run_git_strict(
            &source,
            &[
                "push",
                remote.to_str().unwrap(),
                &format!("{head}:refs/heads/main"),
                &format!("refs/tags/{tag}:refs/tags/{tag}"),
            ],
        )
        .unwrap();
        let token_root = TestDir::new_private_temp("git-materialization-ancestor-token");
        let token_file = token_root.path().join("token");
        fs::write(&token_file, b"fixture-token-0123456789\n").unwrap();
        fs::set_permissions(&token_file, fs::Permissions::from_mode(0o600)).unwrap();
        let args = |destination: &Path, object: &str| {
            vec![
                "git-materialize".to_owned(),
                "--repo".to_owned(),
                "jeryu/example".to_owned(),
                "--remote".to_owned(),
                remote.display().to_string(),
                "--ref".to_owned(),
                "refs/heads/main".to_owned(),
                "--expected-head".to_owned(),
                head.clone(),
                "--destination".to_owned(),
                destination.display().to_string(),
                "--token-file".to_owned(),
                token_file.display().to_string(),
                "--retain-ancestor-tag-object".to_owned(),
                object.to_owned(),
            ]
        };

        let retained = root.path().join("retained");
        jeryu_git_materialize(args(&retained, &tag_object)).unwrap();
        assert_eq!(
            strict_git_output(&retained, &["rev-parse", tag]).unwrap(),
            tag_object
        );
        assert_eq!(resolve_commit(&retained, tag).unwrap(), baseline);
        assert_eq!(
            strict_git_output(
                &retained,
                &["for-each-ref", "--format=%(refname)", "refs/tags"]
            )
            .unwrap(),
            format!("refs/tags/{tag}")
        );
        assert!(strict_git_output(&retained, &["remote"])
            .unwrap()
            .is_empty());

        let missing = root.path().join("missing");
        assert!(jeryu_git_materialize(args(&missing, &"f".repeat(40))).is_err());
        assert!(!missing.exists());
        let malformed = root.path().join("malformed");
        assert!(jeryu_git_materialize(args(&malformed, "short")).is_err());
        assert!(!malformed.exists());

        let lightweight = "example-v7.0.1-split.6";
        run_git_strict(&source, &["tag", lightweight, &baseline]).unwrap();
        run_git_strict(
            &source,
            &[
                "push",
                remote.to_str().unwrap(),
                &format!("refs/tags/{lightweight}:refs/tags/{lightweight}"),
            ],
        )
        .unwrap();
        let lightweight_rejected = root.path().join("lightweight");
        assert!(jeryu_git_materialize(args(&lightweight_rejected, &baseline)).is_err());
        assert!(!lightweight_rejected.exists());

        let tree = strict_git_output(&source, &["rev-parse", "HEAD^{tree}"]).unwrap();
        let unrelated =
            strict_git_output(&source, &["commit-tree", &tree, "-m", "unrelated"]).unwrap();
        let unrelated_tag = "example-v7.0.1-split.7";
        run_git_strict(
            &source,
            &[
                "tag",
                "-a",
                unrelated_tag,
                "-m",
                "unrelated contract source",
                &unrelated,
            ],
        )
        .unwrap();
        let unrelated_object = strict_git_output(&source, &["rev-parse", unrelated_tag]).unwrap();
        run_git_strict(
            &source,
            &[
                "push",
                remote.to_str().unwrap(),
                &format!("refs/tags/{unrelated_tag}:refs/tags/{unrelated_tag}"),
            ],
        )
        .unwrap();
        let non_ancestor = root.path().join("non-ancestor-object");
        let error = jeryu_git_materialize(args(&non_ancestor, &unrelated_object)).unwrap_err();
        assert!(error.to_string().contains("ancestor"));
        assert!(!non_ancestor.exists());
    }

    #[test]
    fn git_materialization_rejects_symlink_mode_before_checkout() {
        let root = TestDir::new("git-materialization-symlink-mode");
        let source = root.path().join("source");
        let mut init = Command::new("git");
        init.args(["init", "-b", "main"]).arg(&source);
        command(init);
        run_git_strict(&source, &["config", "user.name", "Release Test"]).unwrap();
        run_git_strict(
            &source,
            &["config", "user.email", "release@example.invalid"],
        )
        .unwrap();
        fs::write(
            source.join("blob-source"),
            "never materialize this target\n",
        )
        .unwrap();
        let blob = strict_git_output(&source, &["hash-object", "-w", "blob-source"]).unwrap();
        let cache_info = format!("120000,{blob},prohibited-link");
        run_git_strict(
            &source,
            &["update-index", "--add", "--cacheinfo", &cache_info],
        )
        .unwrap();
        let tree = strict_git_output(&source, &["write-tree"]).unwrap();
        let head = strict_git_output(
            &source,
            &["commit-tree", &tree, "-m", "prohibited symlink tree"],
        )
        .unwrap();
        run_git_strict(&source, &["update-ref", "refs/heads/main", &head]).unwrap();
        assert!(!source.join("prohibited-link").exists());

        let remote = init_bare(root.path());
        run_git_strict(
            &source,
            &[
                "push",
                remote.to_str().unwrap(),
                &format!("{head}:refs/heads/main"),
            ],
        )
        .unwrap();
        let token_root = TestDir::new_private_temp("git-materialization-symlink-token");
        let token_file = token_root.path().join("token");
        fs::write(&token_file, b"fixture-token-0123456789\n").unwrap();
        fs::set_permissions(&token_file, fs::Permissions::from_mode(0o600)).unwrap();
        let destination = root.path().join("rejected");
        let error = jeryu_git_materialize(vec![
            "git-materialize".to_owned(),
            "--repo".to_owned(),
            "jeryu/example".to_owned(),
            "--remote".to_owned(),
            remote.display().to_string(),
            "--ref".to_owned(),
            "refs/heads/main".to_owned(),
            "--expected-head".to_owned(),
            head,
            "--destination".to_owned(),
            destination.display().to_string(),
            "--token-file".to_owned(),
            token_file.display().to_string(),
        ])
        .unwrap_err();
        assert!(error.to_string().contains("prohibited symlink"));
        assert!(!destination.exists());
        assert!(!source.join("prohibited-link").exists());
    }

    #[test]
    fn starforge_materialization_hydrates_real_lfs_pointer_from_pinned_binary() {
        let git_lfs = PathBuf::from("/usr/bin/git-lfs");
        assert!(git_lfs.is_file(), "pinned /usr/bin/git-lfs is required");
        let root = TestDir::new("git-materialization-lfs");
        let source = root.path().join("jain-starforge");
        let mut init = Command::new("git");
        init.args(["init", "-b", "main"]).arg(&source);
        command(init);
        run_git_strict(&source, &["config", "user.name", "LFS Fixture"]).unwrap();
        run_git_strict(&source, &["config", "user.email", "lfs@example.invalid"]).unwrap();
        let process = format!("{} filter-process", git_lfs.display());
        let clean = format!("{} clean -- %f", git_lfs.display());
        let smudge = format!("{} smudge -- %f", git_lfs.display());
        run_git_strict(&source, &["config", "filter.lfs.process", &process]).unwrap();
        run_git_strict(&source, &["config", "filter.lfs.clean", &clean]).unwrap();
        run_git_strict(&source, &["config", "filter.lfs.smudge", &smudge]).unwrap();
        run_git_strict(&source, &["config", "filter.lfs.required", "true"]).unwrap();
        fs::write(
            source.join(".gitattributes"),
            "*.bin filter=lfs diff=lfs merge=lfs -text\n",
        )
        .unwrap();
        let payload = b"real offline LFS payload\n";
        fs::write(source.join("artifact.bin"), payload).unwrap();
        run_git_strict(&source, &["add", ".gitattributes", "artifact.bin"]).unwrap();
        run_git_strict(&source, &["commit", "-m", "reviewed LFS object"]).unwrap();
        let head = resolve_commit(&source, "HEAD").unwrap();
        let pointer = strict_git_output(&source, &["show", "HEAD:artifact.bin"]).unwrap();
        assert!(pointer.starts_with("version https://git-lfs.github.com/spec/v1\n"));

        let remote = init_bare(root.path());
        run_git_strict(
            &source,
            &[
                "-c",
                "core.hooksPath=/dev/null",
                "push",
                remote.to_str().unwrap(),
                &format!("{head}:refs/heads/main"),
            ],
        )
        .unwrap();
        let mut lfs_push = Command::new(&git_lfs);
        lfs_push.current_dir(&source).args([
            "push",
            &format!("file://{}", remote.display()),
            "--all",
        ]);
        command(lfs_push);

        let token_root = TestDir::new_private_temp("git-materialization-lfs-token");
        let token_file = token_root.path().join("token");
        fs::write(&token_file, b"fixture-token-0123456789\n").unwrap();
        fs::set_permissions(&token_file, fs::Permissions::from_mode(0o600)).unwrap();
        let destination = root.path().join("hydrated");
        jeryu_git_materialize(vec![
            "git-materialize".to_owned(),
            "--repo".to_owned(),
            "veox/jain-starforge".to_owned(),
            "--remote".to_owned(),
            remote.display().to_string(),
            "--ref".to_owned(),
            "refs/heads/main".to_owned(),
            "--expected-head".to_owned(),
            head,
            "--destination".to_owned(),
            destination.display().to_string(),
            "--token-file".to_owned(),
            token_file.display().to_string(),
            "--git-lfs-path".to_owned(),
            git_lfs.display().to_string(),
            "--git-lfs-sha256".to_owned(),
            sha256_regular_file(&git_lfs, "test git-lfs").unwrap(),
        ])
        .unwrap();
        assert_eq!(fs::read(destination.join("artifact.bin")).unwrap(), payload);
        assert!(strict_git_output(&destination, &["remote"])
            .unwrap()
            .is_empty());

        fs::write(
            source.join(".lfsconfig"),
            "[lfs \"customtransfer.hostile\"]\npath = /tmp/execute-me\n",
        )
        .unwrap();
        run_git_strict(&source, &["add", ".lfsconfig"]).unwrap();
        run_git_strict(&source, &["commit", "-m", "hostile LFS config"]).unwrap();
        let hostile_head = resolve_commit(&source, "HEAD").unwrap();
        run_git_strict(
            &source,
            &[
                "-c",
                "core.hooksPath=/dev/null",
                "push",
                remote.to_str().unwrap(),
                &format!("{hostile_head}:refs/heads/hostile"),
            ],
        )
        .unwrap();
        let hostile_destination = root.path().join("hostile");
        let error = jeryu_git_materialize(vec![
            "git-materialize".to_owned(),
            "--repo".to_owned(),
            "veox/jain-starforge".to_owned(),
            "--remote".to_owned(),
            remote.display().to_string(),
            "--ref".to_owned(),
            "refs/heads/hostile".to_owned(),
            "--expected-head".to_owned(),
            hostile_head,
            "--destination".to_owned(),
            hostile_destination.display().to_string(),
            "--token-file".to_owned(),
            token_file.display().to_string(),
            "--git-lfs-path".to_owned(),
            git_lfs.display().to_string(),
            "--git-lfs-sha256".to_owned(),
            sha256_regular_file(&git_lfs, "test git-lfs").unwrap(),
        ])
        .unwrap_err();
        assert!(error
            .to_string()
            .contains("tracked .lfsconfig is forbidden"));
        assert!(!hostile_destination.exists());
    }

    #[test]
    fn branch_push_apply_requires_an_explicit_token_before_remote_access() {
        let root = TestDir::new("branch-push-token-required");
        let (repo, head) = init_source(root.path());
        let error = jeryu_branch_push_beneath(
            vec![
                "branch-push".to_owned(),
                "--repo".to_owned(),
                "jeryu/example".to_owned(),
                "--repo-path".to_owned(),
                repo.display().to_string(),
                "--branch".to_owned(),
                "main".to_owned(),
                "--expected-head".to_owned(),
                head,
                "--apply".to_owned(),
            ],
            root.path(),
        )
        .unwrap_err();
        assert!(error.to_string().contains("requires --token-file"));
    }

    #[test]
    fn repository_api_identity_is_exact_and_unambiguous() {
        let row = json!({
            "id": {"host": "jeryu", "owner": "veox", "name": "example"},
            "default_branch": "main",
            "clone_http_url": "/git/veox/example.git",
        });
        let response = json!({"repositories": [row.clone()]});
        assert_eq!(
            validate_repo_list_identity(&response, "veox/example").unwrap(),
            json!({
                "host": "jeryu",
                "owner": "veox",
                "name": "example",
                "default_branch": "main",
                "clone_http_url": "/git/veox/example.git",
            })
        );
        assert!(validate_repo_list_identity(&response, "veox/other").is_err());
        assert!(validate_repo_list_identity(
            &json!({"repositories": [row.clone(), row]}),
            "veox/example"
        )
        .is_err());
        assert!(validate_repo_list_identity(
            &json!({"repositories": [{
                "id": {"host": "jeryu", "owner": "veox", "name": "example"},
                "default_branch": "trunk",
                "clone_http_url": "/git/veox/example.git",
            }]}),
            "veox/example"
        )
        .is_err());
    }

    #[test]
    fn authenticated_main_fetch_changes_only_origin_main() {
        let root = TestDir::new("main-fetch");
        let (repo, initial) = init_source(root.path());
        let remote = init_bare(root.path());
        run_git_strict(
            &repo,
            &["remote", "add", "origin", remote.to_str().unwrap()],
        )
        .unwrap();
        let initial_refspec = format!("{initial}:refs/heads/main");
        run_git_strict(&repo, &["push", remote.to_str().unwrap(), &initial_refspec]).unwrap();
        run_git_strict(
            &repo,
            &[
                "fetch",
                "--no-tags",
                remote.to_str().unwrap(),
                "refs/heads/main:refs/remotes/origin/main",
            ],
        )
        .unwrap();
        run_git_strict(
            &repo,
            &[
                "symbolic-ref",
                "refs/remotes/origin/HEAD",
                "refs/remotes/origin/main",
            ],
        )
        .unwrap();

        let remote_head = commit_next(&repo);
        let remote_refspec = format!("{remote_head}:refs/heads/main");
        run_git_strict(&repo, &["push", remote.to_str().unwrap(), &remote_refspec]).unwrap();
        run_git_strict(
            &repo,
            &["update-ref", "refs/tags/remote-only", &remote_head],
        )
        .unwrap();
        run_git_strict(
            &repo,
            &[
                "push",
                remote.to_str().unwrap(),
                "refs/tags/remote-only:refs/tags/remote-only",
            ],
        )
        .unwrap();
        run_git_strict(&repo, &["update-ref", "-d", "refs/tags/remote-only"]).unwrap();

        let token_root = TestDir::new_private_temp("main-fetch-token");
        let token_file = token_root.path().join("token");
        fs::write(&token_file, b"fixture-token-0123456789\n").unwrap();
        fs::set_permissions(&token_file, fs::Permissions::from_mode(0o600)).unwrap();

        let before = main_fetch_snapshot(&repo).unwrap();
        assert_eq!(
            before
                .refs
                .get("refs/remotes/origin/HEAD")
                .map(String::as_str),
            Some("symref:refs/remotes/origin/main")
        );
        let mut report = receipt_header("test", "main-fetch", false);
        fetch_authenticated_main(
            &repo,
            remote.to_str().unwrap(),
            &token_file,
            Some(&remote_head),
            false,
            &mut report,
        )
        .unwrap();
        assert_eq!(report["action"], "would-fetch-main");
        assert_eq!(main_fetch_snapshot(&repo).unwrap(), before);

        fetch_authenticated_main(
            &repo,
            remote.to_str().unwrap(),
            &token_file,
            Some(&remote_head),
            true,
            &mut report,
        )
        .unwrap();
        let after = main_fetch_snapshot(&repo).unwrap();
        assert_eq!(
            after.refs.get("refs/remotes/origin/main"),
            Some(&remote_head)
        );
        assert_eq!(after.head, before.head);
        assert_eq!(after.tree, before.tree);
        assert_eq!(
            after.refs.get("refs/remotes/origin/HEAD"),
            before.refs.get("refs/remotes/origin/HEAD")
        );
        assert!(!after.refs.contains_key("refs/tags/remote-only"));
        assert_eq!(report["action"], "fetched-and-verified");

        let mut changed_target = after.clone();
        changed_target.refs.insert(
            "refs/remotes/origin/HEAD".to_owned(),
            "symref:refs/remotes/origin/other".to_owned(),
        );
        assert!(validate_main_fetch_side_effects(&after, &changed_target, &remote_head).is_err());
    }

    #[test]
    fn branch_push_dry_run_is_local_read_only_and_rejects_config_injection() {
        let root = TestDir::new("branch-push-dry-run");
        let (repo, head) = init_source(root.path());
        let before = strict_git_output(&repo, &["status", "--porcelain=v1"]).unwrap();
        jeryu_branch_push_beneath(
            vec![
                "branch-push".to_owned(),
                "--repo".to_owned(),
                "jeryu/example".to_owned(),
                "--repo-path".to_owned(),
                repo.display().to_string(),
                "--branch".to_owned(),
                "main".to_owned(),
                "--expected-head".to_owned(),
                head.clone(),
            ],
            root.path(),
        )
        .unwrap();
        assert_eq!(
            strict_git_output(&repo, &["status", "--porcelain=v1"]).unwrap(),
            before
        );
        assert_eq!(resolve_commit(&repo, "HEAD").unwrap(), head);

        let marker = root.path().join("fsmonitor-invoked");
        let monitor = root.path().join("hostile-fsmonitor.sh");
        fs::write(
            &monitor,
            format!("#!/bin/sh\nprintf invoked >'{}'\n", marker.display()),
        )
        .unwrap();
        fs::set_permissions(&monitor, fs::Permissions::from_mode(0o755)).unwrap();
        run_git_strict(
            &repo,
            &["config", "core.fsmonitor", monitor.to_str().unwrap()],
        )
        .unwrap();
        assert!(jeryu_branch_push_beneath(
            vec![
                "branch-push".to_owned(),
                "--repo".to_owned(),
                "jeryu/example".to_owned(),
                "--repo-path".to_owned(),
                repo.display().to_string(),
                "--branch".to_owned(),
                "main".to_owned(),
                "--expected-head".to_owned(),
                head.clone(),
            ],
            root.path()
        )
        .is_err());
        assert!(
            !marker.exists(),
            "dry-run branch publication executed hostile core.fsmonitor"
        );
        run_git_strict(&repo, &["config", "--unset", "core.fsmonitor"]).unwrap();

        run_git_strict(
            &repo,
            &[
                "config",
                "url.http://attacker.invalid/.insteadOf",
                LOCAL_JERYU_ORIGIN,
            ],
        )
        .unwrap();
        assert!(reject_local_git_injection(&repo).is_err());
    }

    #[test]
    fn starforge_branch_publication_accepts_only_bounded_lfs_access_metadata() {
        let root = TestDir::new("starforge-lfs-config");
        let repo = root.path().join("jain-starforge");
        fs::create_dir(&repo).unwrap();
        run_git_strict(&repo, &["init", "--quiet"]).unwrap();
        run_git_strict(
            &repo,
            &[
                "config",
                "lfs.http://127.0.0.1:8787/git/veox/jain-starforge.git/info/lfs.access",
                "basic",
            ],
        )
        .unwrap();
        run_git_strict(&repo, &["config", "lfs.repositoryformatversion", "0"]).unwrap();
        reject_local_git_injection(&repo).unwrap();

        for (key, value) in [
            ("lfs.customtransfer.hostile.path", "/tmp/execute-me"),
            ("lfs.fetchinclude", "../../secret"),
            ("filter.lfs.process", "/tmp/execute-me"),
        ] {
            run_git_strict(&repo, &["config", key, value]).unwrap();
            assert!(reject_local_git_injection(&repo).is_err(), "accepted {key}");
            run_git_strict(&repo, &["config", "--unset-all", key]).unwrap();
        }
        run_git_strict(
            &repo,
            &[
                "config",
                "lfs.http://attacker.invalid/jain-starforge.git/info/lfs.access",
                "basic",
            ],
        )
        .unwrap();
        assert!(reject_local_git_injection(&repo).is_err());
    }

    #[test]
    fn jeryu_approval_binds_and_reads_back_the_exact_head() {
        let expected_head = "a".repeat(40);
        let request = plan_jeryu_approval_request(
            "jeryu/example",
            Some("7"),
            Some(&expected_head),
            Some("Reviewed release-critical change."),
        )
        .unwrap();
        assert_eq!(request.method(), "POST");
        assert_eq!(
            request.path(),
            "/api/v1/repos/jeryu%2Fexample/pulls/7/reviews"
        );
        let body: JsonValue = serde_json::from_str(request.body().unwrap()).unwrap();
        assert_eq!(body["verdict"], "approve");
        assert_eq!(body["expected_head_sha"], expected_head);
        assert_eq!(body["thread_comments"], json!([]));

        let readback = json!({
            "summary": {
                "head_sha": expected_head,
                "review": {"approvals": 1}
            }
        });
        validate_approval_readback(&readback, &expected_head).unwrap();
        assert!(validate_approval_readback(&readback, &"b".repeat(40)).is_err());
        assert!(
            plan_jeryu_approval_request("jeryu/example", Some("7"), Some("short"), None).is_err()
        );
    }

    #[test]
    fn non_owner_is_forbidden_from_disabling_branch_protection() {
        let mut policy = immutable_main_readback("example/required");
        policy["enforce_admins"]["enabled"] = json!(false);
        assert!(
            validate_protection_policy(&policy, "jeryu/example", "main", "example/required")
                .is_err()
        );

        policy["enforce_admins"]["enabled"] = json!(true);
        validate_protection_policy(&policy, "jeryu/example", "main", "example/required").unwrap();

        for invalid_url in [
            "/repos/jeryu/other/branches/main/protection",
            "/repos/jeryu/example/branches/release/protection",
            "/repos/jeryu%2Fexample/branches/main/protection",
            "/repos/jeryu/example/branches/main/%70rotection",
            "//repos/jeryu/example/branches/main/protection",
        ] {
            let mut wrong_subject = immutable_main_readback("example/required");
            wrong_subject["url"] = json!(invalid_url);
            assert!(validate_protection_policy(
                &wrong_subject,
                "jeryu/example",
                "main",
                "example/required"
            )
            .is_err());
        }

        policy["required_status_checks"]["contexts"] =
            json!(["example/required", "unexpected/required"]);
        assert!(
            validate_protection_policy(&policy, "jeryu/example", "main", "example/required")
                .is_err()
        );
        policy["required_status_checks"]["contexts"] = json!(["example/required"]);
        policy["required_pull_request_reviews"]["required_approving_review_count"] = json!(2);
        assert!(
            validate_protection_policy(&policy, "jeryu/example", "main", "example/required")
                .is_err()
        );

        policy = immutable_main_readback("example/required");
        policy["required_pull_request_reviews"]["bypass_pull_request_allowances"] =
            json!({"users": ["admin"]});
        assert!(
            validate_protection_policy(&policy, "jeryu/example", "main", "example/required")
                .is_err()
        );
        policy = immutable_main_readback("example/required");
        policy["restrictions"] = json!({"users": []});
        assert!(
            validate_protection_policy(&policy, "jeryu/example", "main", "example/required")
                .is_err()
        );
        policy = immutable_main_readback("example/required");
        policy["required_signatures"]["enabled"] = json!(true);
        assert!(
            validate_protection_policy(&policy, "jeryu/example", "main", "example/required")
                .is_err()
        );
    }

    #[test]
    fn jeryu_lifecycle_dry_run_needs_no_token_and_writes_only_explicit_evidence() {
        let root = TestDir::new("jeryu-dry-run");
        let before = fs::read_dir(root.path()).unwrap().count();
        jeryu_lifecycle(vec![
            "pr-ready".to_owned(),
            "--repo".to_owned(),
            "jeryu/example".to_owned(),
            "--number".to_owned(),
            "3".to_owned(),
        ])
        .unwrap();
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), before);

        let receipt = root.path().join("ready.json");
        jeryu_lifecycle(vec![
            "pr-ready".to_owned(),
            "--repo".to_owned(),
            "jeryu/example".to_owned(),
            "--number".to_owned(),
            "3".to_owned(),
            "--evidence-out".to_owned(),
            receipt.display().to_string(),
        ])
        .unwrap();
        let report = read_json(&receipt);
        assert_eq!(report["mode"], "dry-run");
        assert_eq!(report["action"], "would-apply");
        assert_eq!(report["status"], "pass");
    }

    #[test]
    fn managed_release_identity_is_closed_pending_or_exact_bound() {
        let pending: toml::Value = r#"identity_status = "pending""#.parse().unwrap();
        validate_managed_release_identity(&pending, "repo[example]", "example", "current_tag")
            .unwrap();

        let mut stale_pending = pending.clone();
        stale_pending.as_table_mut().unwrap().insert(
            "release_commit".to_owned(),
            toml::Value::String("0".repeat(40)),
        );
        assert!(validate_managed_release_identity(
            &stale_pending,
            "repo[example]",
            "example",
            "current_tag"
        )
        .unwrap_err()
        .contains("must omit every bound-only identity field"));

        let bound: toml::Value = format!(
            r#"
identity_status = "bound"
product_version = "{RELEASE_VERSION}"
tag_revision = 3
current_tag = "example-v{RELEASE_VERSION}-split.3"
release_commit = "{}"
release_tree = "{}"
release_checksum_sha256 = "{}"
"#,
            "1".repeat(40),
            "2".repeat(40),
            "3".repeat(64),
        )
        .parse()
        .unwrap();
        validate_managed_release_identity(&bound, "repo[example]", "example", "current_tag")
            .unwrap();

        let mut wrong_tag = bound;
        wrong_tag["current_tag"] =
            toml::Value::String(format!("example-v{RELEASE_VERSION}-split.2"));
        assert!(validate_managed_release_identity(
            &wrong_tag,
            "repo[example]",
            "example",
            "current_tag"
        )
        .unwrap_err()
        .contains("split.3"));
    }

    #[test]
    fn source_inventory_is_exact_and_ignores_dirty_worktree_bytes() {
        let root = TestDir::new("source-inventory");
        let (source, source_sha) = init_source(root.path());
        fs::write(source.join("payload.txt"), "dirty working tree\n").unwrap();
        fs::write(source.join("untracked.txt"), "must not enter authority\n").unwrap();
        let files = git_files(&source, &source_sha).unwrap();
        assert_eq!(files, vec!["payload.txt"]);
        let bytes = render_source_inventory(&files, &source_sha).unwrap();
        let manifest = root.path().join("repos.manifest.toml");
        fs::write(
            &manifest,
            format!(
                r#"source_sha = "{source_sha}"
source_inventory = "authority/source-paths.txt"
source_inventory_count = 1
source_inventory_sha256 = "{}"
"#,
                sha256_bytes(&bytes)
            ),
        )
        .unwrap();
        seal_source_inventory_command(vec![
            "--manifest".to_owned(),
            manifest.display().to_string(),
            "--source-root".to_owned(),
            source.display().to_string(),
            "--apply".to_owned(),
        ])
        .unwrap();
        let data: toml::Value = fs::read_to_string(&manifest).unwrap().parse().unwrap();
        let (_, sealed, digest) = read_source_inventory(&data, &manifest).unwrap();
        assert_eq!(sealed, vec!["payload.txt"]);
        assert_eq!(digest, sha256_bytes(&bytes));
    }

    #[test]
    fn source_inventory_rejects_non_normalized_duplicate_or_unsealed_paths() {
        let source_sha = "1".repeat(40);
        let header = format!(
            "# Generated by: splitctl seal-source-inventory\n\
# DO NOT EDIT BY HAND\n\
# Source: immutable Git tree {source_sha}\n\
# Regenerate: cargo run --locked --quiet -- seal-source-inventory --manifest repos.manifest.toml --source-root SOURCE_ROOT --apply\n"
        );
        for body in ["a/../b\n", "a//b\n", "a\\b\n", "a\na\n", "b\na\n", "a"] {
            let invalid = format!("{header}{body}");
            assert!(parse_source_inventory(invalid.as_bytes(), &source_sha).is_err());
        }
        assert!(parse_source_inventory(b"payload.txt\n", &source_sha).is_err());
    }

    #[test]
    fn canonical_manifest_is_candidate_only_and_pending_projections_are_non_writable() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("repos.manifest.toml");
        let canonical: toml::Value = fs::read_to_string(&path).unwrap().parse().unwrap();
        validate_manifest_data(&canonical, &path, false).unwrap();
        assert!(derived_manifest_is_pending(&canonical, "portal").unwrap());
        assert!(derived_manifest_is_pending(&canonical, "deploy").unwrap());
        let jankurai_tui_custody = canonical["excluded_path"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|row| string(row, "name").as_deref() == Some("jankurai-tools-tui"))
            .collect::<Vec<_>>();
        assert_eq!(jankurai_tui_custody.len(), 1);
        assert_eq!(
            string(jankurai_tui_custody[0], "path").as_deref(),
            Some("/home/ubuntu/jain-split/jankurai-tools-tui")
        );
        assert_eq!(
            string(jankurai_tui_custody[0], "owner").as_deref(),
            Some("jankurai")
        );
        assert_eq!(
            string(jankurai_tui_custody[0], "reason").as_deref(),
            Some(
                "temporary source custody only; requires governed creation and onboarding through \
                 corrected AUTH-006/007/008, then removal from exclusions"
            )
        );
        validate_derived_manifest(
            Path::new("/definitely/missing/held-projection.toml"),
            "not-used-while-pending",
            &canonical,
            &path,
            "portal",
        )
        .unwrap();

        for (field, invalid, expected) in [
            (
                "status",
                toml::Value::String("ga".to_owned()),
                "status must be candidate",
            ),
            (
                "formal_ga",
                toml::Value::Boolean(true),
                "formal_ga must be false",
            ),
            (
                "sagemaker",
                toml::Value::String("ready".to_owned()),
                "sagemaker must be N/A",
            ),
            (
                "rollback_target",
                toml::Value::String("8.0.0".to_owned()),
                "rollback_target must be 7.0.6",
            ),
        ] {
            let mut invalid_manifest = canonical.clone();
            invalid_manifest
                .as_table_mut()
                .unwrap()
                .insert(field.to_owned(), invalid);
            assert!(validate_manifest_data(&invalid_manifest, &path, false)
                .unwrap_err()
                .to_string()
                .contains(expected));
        }

        let mut stale_control_owner = canonical.clone();
        stale_control_owner["control_plane"]["forge_owner"] =
            toml::Value::String("jeryu".to_owned());
        assert!(validate_manifest_data(&stale_control_owner, &path, false)
            .unwrap_err()
            .to_string()
            .contains("control_plane.forge_owner must be veox"));
    }

    #[test]
    fn nested_engine_topology_accepts_exact_legacy_and_child_physical_families() {
        for release in ["8.0.0", "8.0.1"] {
            let root = TestDir::new(&format!("redline-topology-{release}"));
            let (data, topology) = synthetic_nested_engine_topology(root.path(), release);
            validate_nested_engine_topology_paths(&topology).unwrap();
            let nested: toml::Value = fs::read_to_string(&topology.manifest_path)
                .unwrap()
                .parse()
                .unwrap();
            assert_eq!(
                declared_nested_lock_path(&topology, &nested).unwrap(),
                topology.control_plane_path.join("redline.lock.toml")
            );
            let mut errors = Vec::new();
            validate_nested_family_local(&data, true, &mut errors).unwrap();
            assert!(errors.is_empty(), "{release}: {errors:?}");
        }
    }

    #[test]
    fn nested_topology_accounts_for_closed_pending_repositories_in_the_canonical_container() {
        let root = TestDir::new("redline-topology-pending-repository");
        let (mut data, topology) = synthetic_nested_engine_topology(root.path(), "8.0.1");
        let central = topology.container_path.join("redline-central");
        standalone_physical_clone(root.path(), "pending-central-source", &central);
        let pending: toml::Value = r#"
name = "redline-central"
remote = "http://127.0.0.1:8787/git/jeryu/redline-central.git"
required_check = "redline-central/required"
identity_status = "pending"
"#
        .parse()
        .unwrap();
        data["nested_families"]["redline"]
            .as_table_mut()
            .unwrap()
            .insert(
                "pending_repository".to_owned(),
                toml::Value::Array(vec![pending]),
            );

        let topology = nested_engine_topology(&data).unwrap();
        assert_eq!(topology.pending_repositories.len(), 1);
        let nested: toml::Value = fs::read_to_string(&topology.manifest_path)
            .unwrap()
            .parse()
            .unwrap();
        let paths = validated_nested_repository_paths(&topology, &nested).unwrap();
        assert_eq!(paths.get("redline-central"), Some(&central));
        let mut errors = Vec::new();
        validate_nested_family_local(&data, true, &mut errors).unwrap();
        assert!(errors.is_empty(), "{errors:?}");

        data["nested_families"]["redline"]["pending_repository"]
            .as_array_mut()
            .unwrap()[0]
            .as_table_mut()
            .unwrap()
            .insert(
                "path".to_owned(),
                toml::Value::String("../redline-central".to_owned()),
            );
        assert!(nested_engine_topology(&data)
            .unwrap_err()
            .contains("must omit path and every bound-only identity field"));
    }

    #[test]
    fn nested_engine_topology_rejects_mixed_alias_and_unsupported_mode_tuples() {
        let root = TestDir::new("redline-topology-invalid");
        let (legacy, _) = synthetic_nested_engine_topology(root.path(), "8.0.0");

        let mut mixed = legacy.clone();
        mixed["nested_families"]["redline"]["control_plane"] = toml::Value::String(
            root.path()
                .join("jain-redline/redline-split-ops")
                .display()
                .to_string(),
        );
        let mixed = nested_engine_topology(&mixed).unwrap();
        assert!(validate_nested_engine_topology_paths(&mixed)
            .unwrap_err()
            .contains("nested control plane"));

        let mut alias = legacy.clone();
        alias["nested_families"]["redline"]["manifest_path"] = toml::Value::String(
            root.path()
                .join("redline-split-ops/../redline-split-ops/repos.manifest.toml")
                .display()
                .to_string(),
        );
        assert!(nested_engine_topology(&alias)
            .unwrap_err()
            .contains("manifest_path"));

        let mut mode = legacy;
        mode["nested_families"]["redline"]
            .as_table_mut()
            .unwrap()
            .insert(
                "authority_mode".to_owned(),
                toml::Value::String("parent".to_owned()),
            );
        assert!(nested_engine_topology(&mode)
            .unwrap_err()
            .contains("authority_mode must be child when present"));

        let child_root = TestDir::new("redline-topology-invalid-child-mode");
        let (mut child, _) = synthetic_nested_engine_topology(child_root.path(), "8.0.1");
        child["nested_families"]["redline"]["authority_mode"] =
            toml::Value::String("parent".to_owned());
        assert!(nested_engine_topology(&child)
            .unwrap_err()
            .contains("authority_mode must be child"));
    }

    #[test]
    fn nested_engine_topology_binds_engine_tag_or_requires_explicit_paired_pending_identity() {
        let root = TestDir::new("redline-topology-engine-identity");
        let (data, _) = synthetic_nested_engine_topology(root.path(), "8.0.1");

        let mut missing = data.clone();
        missing["nested_families"]["redline"]
            .as_table_mut()
            .unwrap()
            .remove("engine_tag");
        assert!(nested_engine_topology(&missing)
            .unwrap_err()
            .contains("must match external_dependencies.redline.immutable_tag"));

        let mut wrong = data.clone();
        wrong["nested_families"]["redline"]["engine_tag"] =
            toml::Value::String("redline-core-v4.1.0-jain.5".to_owned());
        assert!(nested_engine_topology(&wrong)
            .unwrap_err()
            .contains("must match external_dependencies.redline.immutable_tag"));

        let mut pending = data.clone();
        let external = pending["external_dependencies"]["redline"]
            .as_table_mut()
            .unwrap();
        for key in [
            "immutable_tag",
            "product_version",
            "tag_revision",
            "release_commit",
            "release_tree",
            "release_checksum_sha256",
        ] {
            external.remove(key);
        }
        external.insert(
            "identity_status".to_owned(),
            toml::Value::String("pending".to_owned()),
        );
        let nested = pending["nested_families"]["redline"]
            .as_table_mut()
            .unwrap();
        nested.remove("engine_tag");
        nested.remove("engine_release_tree");
        nested.insert(
            "engine_identity_status".to_owned(),
            toml::Value::String("pending".to_owned()),
        );
        let mut unpaired = pending.clone();
        unpaired["nested_families"]["redline"]
            .as_table_mut()
            .unwrap()
            .remove("engine_identity_status");
        assert!(nested_engine_topology(&unpaired)
            .unwrap_err()
            .contains("identity statuses must be paired"));
        nested_engine_topology(&pending).unwrap();

        let mut numeric_external_tag = pending.clone();
        numeric_external_tag["external_dependencies"]["redline"]
            .as_table_mut()
            .unwrap()
            .insert("immutable_tag".to_owned(), toml::Value::Integer(4));
        assert!(nested_engine_topology(&numeric_external_tag)
            .unwrap_err()
            .contains("external_dependencies.redline.immutable_tag must be a string"));

        let mut numeric_engine_tag = pending.clone();
        numeric_engine_tag["nested_families"]["redline"]
            .as_table_mut()
            .unwrap()
            .insert("engine_tag".to_owned(), toml::Value::Integer(4));
        assert!(nested_engine_topology(&numeric_engine_tag)
            .unwrap_err()
            .contains("nested_families.redline.engine_tag must be a string"));

        let mut stale_pending = pending;
        stale_pending["external_dependencies"]["redline"]
            .as_table_mut()
            .unwrap()
            .insert(
                "release_commit".to_owned(),
                toml::Value::String("0".repeat(40)),
            );
        assert!(nested_engine_topology(&stale_pending)
            .unwrap_err()
            .contains("must omit every bound-only identity field"));

        let mut explicit_bound = data.clone();
        explicit_bound["external_dependencies"]["redline"]
            .as_table_mut()
            .unwrap()
            .insert(
                "identity_status".to_owned(),
                toml::Value::String("bound".to_owned()),
            );
        explicit_bound["nested_families"]["redline"]
            .as_table_mut()
            .unwrap()
            .insert(
                "engine_identity_status".to_owned(),
                toml::Value::String("bound".to_owned()),
            );
        nested_engine_topology(&explicit_bound).unwrap();

        let mut malformed_tag = data.clone();
        malformed_tag["external_dependencies"]["redline"]["immutable_tag"] =
            toml::Value::String("redline-core-v4.1-jain.4".to_owned());
        malformed_tag["nested_families"]["redline"]["engine_tag"] =
            toml::Value::String("redline-core-v4.1-jain.4".to_owned());
        assert!(nested_engine_topology(&malformed_tag)
            .unwrap_err()
            .contains("must match repository, product_version, and tag_revision"));

        let mut missing_commit = data.clone();
        missing_commit["external_dependencies"]["redline"]
            .as_table_mut()
            .unwrap()
            .remove("release_commit");
        assert!(nested_engine_topology(&missing_commit)
            .unwrap_err()
            .contains("release_commit must be 40 lowercase hex"));

        let mut missing_tree = data.clone();
        missing_tree["external_dependencies"]["redline"]
            .as_table_mut()
            .unwrap()
            .remove("release_tree");
        assert!(nested_engine_topology(&missing_tree)
            .unwrap_err()
            .contains("release_tree must be 40 lowercase hex"));

        let mut wrong_nested_tree = data.clone();
        wrong_nested_tree["nested_families"]["redline"]["engine_release_tree"] =
            toml::Value::String("0".repeat(40));
        assert!(nested_engine_topology(&wrong_nested_tree)
            .unwrap_err()
            .contains("engine_release_tree must match"));

        let mut typed_external_status = data.clone();
        typed_external_status["external_dependencies"]["redline"]
            .as_table_mut()
            .unwrap()
            .insert("identity_status".to_owned(), toml::Value::Boolean(false));
        assert!(nested_engine_topology(&typed_external_status)
            .unwrap_err()
            .contains("external_dependencies.redline.identity_status must be a string"));

        let mut typed_engine_status = data;
        typed_engine_status["nested_families"]["redline"]
            .as_table_mut()
            .unwrap()
            .insert(
                "engine_identity_status".to_owned(),
                toml::Value::Boolean(false),
            );
        assert!(nested_engine_topology(&typed_engine_status)
            .unwrap_err()
            .contains("nested_families.redline.engine_identity_status must be a string"));
    }

    #[test]
    fn nested_engine_identity_is_generic_and_matches_one_child_authority_row() {
        let root = TestDir::new("nested-engine-authority-binding");
        let (mut data, _) = synthetic_nested_engine_topology(root.path(), "8.0.1");
        let external = data["external_dependencies"]
            .as_table_mut()
            .unwrap()
            .remove("redline")
            .unwrap();
        data["external_dependencies"]
            .as_table_mut()
            .unwrap()
            .insert("database".to_owned(), external);
        let nested_family = data["nested_families"]
            .as_table_mut()
            .unwrap()
            .remove("redline")
            .unwrap();
        data["nested_families"]
            .as_table_mut()
            .unwrap()
            .insert("database".to_owned(), nested_family);
        data["nested_families"]["database"]["family"] =
            toml::Value::String("storage-family".to_owned());

        let topology = nested_engine_topology(&data).unwrap();
        assert_eq!(topology.dependency_name, "database");
        assert_eq!(topology.family, "storage-family");
        let mut nested: toml::Value = fs::read_to_string(&topology.manifest_path)
            .unwrap()
            .parse()
            .unwrap();
        nested["family"] = toml::Value::String("storage-family".to_owned());
        validate_child_family_authority(&topology, &nested).unwrap();
        validate_child_engine_authority(&topology, &nested).unwrap();
        let lock = render_nested_lock_section(&topology).unwrap();
        assert!(lock.starts_with("[nested.database]\nfamily = \"storage-family\"\n"));
        assert!(lock.contains(&format!(
            "tree = \"{}\"",
            topology.bound_identity.as_ref().unwrap().release_tree
        )));
        assert!(lock.contains("required_check = \"redline-core/required\""));

        let mut wrong_family = nested.clone();
        wrong_family["family"] = toml::Value::String("different-family".to_owned());
        assert!(validate_child_family_authority(&topology, &wrong_family)
            .unwrap_err()
            .contains("must match parent declaration storage-family"));

        let mut immutable_wins = nested.clone();
        immutable_wins["repo"][0].as_table_mut().unwrap().insert(
            "immutable_tag".to_owned(),
            toml::Value::String(topology.bound_identity.as_ref().unwrap().tag.clone()),
        );
        immutable_wins["repo"][0]["current_tag"] =
            toml::Value::String("redline-core-v4.1.0-jain.999".to_owned());
        validate_child_engine_authority(&topology, &immutable_wins).unwrap();
        assert_eq!(
            declared_release_tag(&immutable_wins["repo"][0]),
            Some(topology.bound_identity.as_ref().unwrap().tag.clone())
        );

        let mut mismatch = nested.clone();
        mismatch["repo"][0]["release_commit"] = toml::Value::String("0".repeat(40));
        assert!(validate_child_engine_authority(&topology, &mismatch)
            .unwrap_err()
            .contains("must exactly match"));

        let mut tree_mismatch = nested.clone();
        tree_mismatch["repo"][0]["release_tree"] = toml::Value::String("0".repeat(40));
        assert!(validate_child_engine_authority(&topology, &tree_mismatch)
            .unwrap_err()
            .contains("must exactly match"));

        let mut missing = nested.clone();
        missing
            .get_mut("repo")
            .and_then(toml::Value::as_array_mut)
            .unwrap()
            .remove(0);
        assert!(validate_child_engine_authority(&topology, &missing)
            .unwrap_err()
            .contains("exactly one engine repository row"));

        let mut duplicate = nested;
        let engine = duplicate["repo"][0].clone();
        duplicate
            .get_mut("repo")
            .and_then(toml::Value::as_array_mut)
            .unwrap()
            .push(engine);
        assert!(validate_child_engine_authority(&topology, &duplicate)
            .unwrap_err()
            .contains("exactly one engine repository row"));
    }

    #[test]
    fn nested_family_validation_binds_child_family_and_immutable_tag_tree() {
        let root = TestDir::new("nested-family-and-tree-binding");
        let (data, topology) = synthetic_nested_engine_topology(root.path(), "8.0.1");
        let engine_path = topology.container_path.join(&topology.engine_repository);
        let bound = topology.bound_identity.as_ref().unwrap();
        run_git_strict(&engine_path, &["tag", &bound.tag, &bound.release_commit]).unwrap();

        let failures = external_dependency_failures(&data);
        assert!(
            failures
                .iter()
                .all(|failure| !failure.contains("tree must resolve locally")),
            "{failures:?}"
        );

        let mut wrong_tree = data.clone();
        wrong_tree["external_dependencies"]["redline"]["release_tree"] =
            toml::Value::String("0".repeat(40));
        wrong_tree["nested_families"]["redline"]["engine_release_tree"] =
            toml::Value::String("0".repeat(40));
        let failures = external_dependency_failures(&wrong_tree);
        assert!(failures
            .iter()
            .any(|failure| failure.contains("tree must resolve locally")));

        let mut nested: toml::Value = fs::read_to_string(&topology.manifest_path)
            .unwrap()
            .parse()
            .unwrap();
        nested["family"] = toml::Value::String("wrong-family".to_owned());
        fs::write(
            &topology.manifest_path,
            toml::to_string_pretty(&nested).unwrap(),
        )
        .unwrap();
        let mut errors = Vec::new();
        validate_nested_family_local(&data, true, &mut errors).unwrap();
        assert!(errors
            .iter()
            .any(|error| error.contains("must match parent declaration redline-split")));
    }

    #[test]
    fn nested_engine_topology_rejects_coherent_split_control_and_repo_lexical_aliases() {
        let split_root = TestDir::new("redline-topology-split-alias");
        let (split_data, _) = synthetic_nested_engine_topology(split_root.path(), "8.0.1");
        let raw_root = split_root.path().display().to_string();
        let (root_parent, root_name) = raw_root.rsplit_once('/').unwrap();
        for aliased_root in [
            format!("{raw_root}/."),
            format!("{raw_root}/"),
            format!("/{raw_root}"),
            format!("{root_parent}//{root_name}"),
            format!("{raw_root}/../{root_name}"),
        ] {
            let mut split_alias = split_data.clone();
            split_alias["split_root"] = toml::Value::String(aliased_root.clone());
            for (key, suffix) in [
                (
                    "manifest_path",
                    "jain-redline/redline-split-ops/repos.manifest.toml",
                ),
                ("container_path", "jain-redline"),
                ("control_plane", "jain-redline/redline-split-ops"),
            ] {
                split_alias["nested_families"]["redline"][key] =
                    toml::Value::String(format!("{aliased_root}/{suffix}"));
            }
            assert!(nested_engine_topology(&split_alias)
                .unwrap_err()
                .contains("exact normalized absolute spelling"));
        }

        let control_root = TestDir::new("redline-topology-control-alias");
        let (control_data, control_topology) =
            synthetic_nested_engine_topology(control_root.path(), "8.0.1");
        let mut control_manifest: toml::Value = fs::read_to_string(&control_topology.manifest_path)
            .unwrap()
            .parse()
            .unwrap();
        control_manifest["control_plane"]["path"] = toml::Value::String("./.".to_owned());
        assert!(
            validated_nested_repository_paths(&control_topology, &control_manifest)
                .unwrap_err()
                .contains("must be exactly .")
        );
        nested_engine_topology(&control_data).unwrap();

        let repo_root = TestDir::new("redline-topology-repo-alias");
        let (_, repo_topology) = synthetic_nested_engine_topology(repo_root.path(), "8.0.1");
        let mut repo_manifest: toml::Value = fs::read_to_string(&repo_topology.manifest_path)
            .unwrap()
            .parse()
            .unwrap();
        repo_manifest["repo"][0]["path"] =
            toml::Value::String("../redline-web/../redline-core".to_owned());
        assert!(
            validated_nested_repository_paths(&repo_topology, &repo_manifest)
                .unwrap_err()
                .contains("path must be exactly ../redline-core")
        );
    }

    #[test]
    fn redline_local_validation_rejects_missing_and_undeclared_git_roots() {
        let root = TestDir::new("redline-topology-path-failures");
        let (data, topology) = synthetic_nested_engine_topology(root.path(), "8.0.1");

        let missing = topology.container_path.join("redline-web");
        fs::remove_dir_all(&missing).unwrap();
        let mut errors = Vec::new();
        validate_nested_family_local(&data, true, &mut errors).unwrap();
        assert!(errors.iter().any(|error| error.contains("redline-web")));

        standalone_physical_clone(root.path(), "redline-web-replacement", &missing);
        standalone_physical_clone(
            root.path(),
            "undeclared-source",
            &topology.container_path.join("undeclared"),
        );
        errors.clear();
        validate_nested_family_local(&data, true, &mut errors).unwrap();
        assert!(errors
            .iter()
            .any(|error| error.contains("undeclared nested-family Git root")));
    }

    #[test]
    fn redline_local_validation_rejects_out_of_container_before_canonicalization() {
        let root = TestDir::new("redline-topology-physical-identity");
        let (data, topology) = synthetic_nested_engine_topology(root.path(), "8.0.1");
        let elsewhere = root.path().join("elsewhere/redline-core");
        standalone_physical_clone(root.path(), "same-basename-source", &elsewhere);
        let mut nested: toml::Value = fs::read_to_string(&topology.manifest_path)
            .unwrap()
            .parse()
            .unwrap();
        nested["repo"][0]["path"] = toml::Value::String("../../elsewhere/redline-core".to_owned());
        fs::write(
            &topology.manifest_path,
            toml::to_string_pretty(&nested).unwrap(),
        )
        .unwrap();
        let mut errors = Vec::new();
        validate_nested_family_local(&data, true, &mut errors).unwrap();
        assert!(errors.iter().any(|error| error
            .contains("redline-core: nested repository path must be exactly ../redline-core")));
    }

    #[test]
    fn managed_repository_view_includes_both_control_planes_and_nested_family() {
        let root = TestDir::new("managed-repos");
        let nested = root.path().join("redline-split-ops/repos.manifest.toml");
        standalone_physical_clone(root.path(), "managed-control", nested.parent().unwrap());
        standalone_physical_clone(
            root.path(),
            "managed-core",
            &root.path().join("redline-split/redline-core"),
        );
        let engine_path = root.path().join("redline-split/redline-core");
        let engine_commit = git_query(&engine_path, &["rev-parse", "HEAD"]).unwrap();
        let engine_tree = git_query(&engine_path, &["rev-parse", "HEAD^{tree}"]).unwrap();
        let engine_checksum = git_archive_sha256(&engine_path, &engine_commit).unwrap();
        fs::write(
            &nested,
            format!(
                r#"
family = "redline-split"
[control_plane]
name = "redline-split-ops"
path = "."
remote = "http://127.0.0.1:8787/git/jeryu/redline-split-ops.git"
required_check = "redline-split-ops/required"
[[repo]]
name = "redline-core"
path = "../redline-split/redline-core"
jeryu_slug = "jeryu/redline-core"
required_check = "redline-core/required"
default_branch = "main"
role = "canonical-engine"
product_version = "4.1.0"
tag_revision = 1
current_tag = "redline-core-v4.1.0-jain.1"
release_commit = "{engine_commit}"
release_tree = "{engine_tree}"
release_checksum_sha256 = "{engine_checksum}"
"#,
            ),
        )
        .unwrap();
        let canonical = format!(
            r#"
release_version = "8.0.0"
repo_family = "jain-split"
split_root = "{}"
[control_plane]
name = "jain-split-ops"
path = "{}/jain-split-ops"
remote = "http://127.0.0.1:8787/git/veox/jain-split-ops.git"
required_check = "jain-split-ops/required"
[external_dependencies.redline]
repository = "redline-core"
remote = "http://127.0.0.1:8787/git/jeryu/redline-core.git"
immutable_tag = "redline-core-v4.1.0-jain.1"
product_version = "4.1.0"
tag_revision = 1
release_commit = "{engine_commit}"
release_tree = "{engine_tree}"
release_checksum_sha256 = "{engine_checksum}"
[nested_families.redline]
family = "redline-split"
manifest_path = "{}"
container_path = "{}/redline-split"
control_plane = "{}/redline-split-ops"
required = true
engine_repository = "redline-core"
engine_remote = "http://127.0.0.1:8787/git/jeryu/redline-core.git"
engine_tag = "redline-core-v4.1.0-jain.1"
engine_release_tree = "{engine_tree}"
[[infrastructure_repo]]
name = "jain-smartcluster"
path = "{}/jain-smartcluster"
profile = "rust-workspace"
remote = "http://127.0.0.1:8787/git/veox/jain-smartcluster.git"
required_check = "jain-smartcluster/required"
default_branch = "main"
immutable_tag = "jain-smartcluster-v8.0.0-split.0"
kind = "required-infrastructure"
family_registered = true
[[repo]]
name = "jain"
path = "{}/jain"
profile = "custom"
jeryu_slug = "veox/jain"
required_check = "jain/required"
default_branch = "main"
current_tag = "jain-v8.0.0-split.0"
"#,
            root.path().display(),
            root.path().display(),
            nested.display(),
            root.path().display(),
            root.path().display(),
            root.path().display(),
            root.path().display(),
        );
        let data: toml::Value = canonical.parse().unwrap();
        let repos = managed_repositories(&data, &root.path().join("repos.manifest.toml")).unwrap();
        assert_eq!(repos.len(), 5);
        assert!(repos.iter().any(|repo| repo.name == "jain-split-ops"));
        assert!(repos.iter().any(|repo| repo.name == "redline-split-ops"));
        assert!(repos.iter().any(|repo| repo.name == "redline-core"));
        assert!(
            repos
                .iter()
                .find(|repo| repo.name == "jain-smartcluster")
                .unwrap()
                .family_registered
        );

        let original_nested = fs::read_to_string(&nested).unwrap();
        let mut out_of_container: toml::Value = original_nested.parse().unwrap();
        out_of_container["repo"][0]["path"] =
            toml::Value::String("../../elsewhere/redline-core".to_owned());
        fs::write(&nested, toml::to_string_pretty(&out_of_container).unwrap()).unwrap();
        assert!(
            managed_repositories(&data, &root.path().join("repos.manifest.toml"))
                .unwrap_err()
                .to_string()
                .contains("path must be exactly ../redline-split/redline-core")
        );

        fs::write(&nested, &original_nested).unwrap();
    }

    #[test]
    fn derived_manifest_is_exact_unless_subset_is_declared() {
        let root = TestDir::new("derived-manifest");
        let manifest = root.path().join("repos.manifest.toml");
        fs::write(&manifest, "schema_version = \"1\"\n").unwrap();
        let canonical: toml::Value = r#"
schema_version = "1"
release_version = "8.0.0"
split_root = "/tmp/example"
required_repos = ["one", "two"]
[[repo]]
name = "one"
[[repo]]
name = "two"
"#
        .parse()
        .unwrap();
        let rendered = render_derived_manifest(&canonical, &manifest, "portal", "abc").unwrap();
        let derived: toml::Value = rendered.parse().unwrap();
        assert_eq!(family_repos(&derived).unwrap().len(), 2);
        assert_eq!(
            string(&derived, "canonical_manifest_sha256").as_deref(),
            Some("abc")
        );
        assert_eq!(
            string(&derived, "derived_manifest_target").as_deref(),
            Some("portal")
        );

        let subset: toml::Value = r#"
schema_version = "1"
release_version = "8.0.0"
split_root = "/tmp/example"
required_repos = ["one", "two"]
[derived_manifests.portal]
repos = ["two"]
[[repo]]
name = "one"
[[repo]]
name = "two"
"#
        .parse()
        .unwrap();
        let rendered = render_derived_manifest(&subset, &manifest, "portal", "def").unwrap();
        let derived: toml::Value = rendered.parse().unwrap();
        assert_eq!(family_repos(&derived).unwrap().len(), 1);
        assert_eq!(
            string(family_repos(&derived).unwrap()[0], "name").as_deref(),
            Some("two")
        );
        assert_eq!(strings(&derived, "required_repos"), vec!["two"]);
    }

    #[test]
    fn derived_manifest_sync_is_dry_run_by_default_and_apply_is_explicit() {
        assert!(release_evidence_path("receipt.json")
            .ends_with("docs/release-evidence/8.0.1/receipt.json"));
        let root = TestDir::new("derived-sync");
        let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("repos.manifest.toml");
        let mut canonical: toml::Value = fs::read_to_string(source).unwrap().parse().unwrap();
        let external = canonical["external_dependencies"]["redline"]
            .as_table_mut()
            .unwrap();
        external.remove("immutable_tag");
        external.insert(
            "identity_status".to_owned(),
            toml::Value::String("pending".to_owned()),
        );
        let nested = canonical["nested_families"]["redline"]
            .as_table_mut()
            .unwrap();
        nested.remove("engine_tag");
        nested.insert(
            "engine_identity_status".to_owned(),
            toml::Value::String("pending".to_owned()),
        );
        let portal = root.path().join("portal.toml");
        let deploy = root.path().join("deploy.toml");
        let mut targets = toml::map::Map::new();
        for (name, path) in [("portal", &portal), ("deploy", &deploy)] {
            let mut target = toml::map::Map::new();
            target.insert(
                "path".to_owned(),
                toml::Value::String(path.display().to_string()),
            );
            targets.insert(name.to_owned(), toml::Value::Table(target));
        }
        canonical
            .as_table_mut()
            .unwrap()
            .insert("derived_manifests".to_owned(), toml::Value::Table(targets));
        let manifest = root.path().join("repos.manifest.toml");
        fs::write(&manifest, toml::to_string_pretty(&canonical).unwrap()).unwrap();
        let receipt = root.path().join("receipt.json");
        let args = || {
            vec![
                "--manifest".to_owned(),
                manifest.display().to_string(),
                "--receipt".to_owned(),
                receipt.display().to_string(),
            ]
        };
        sync_derived_manifests_command(args()).unwrap();
        assert!(!portal.exists());
        assert!(!deploy.exists());
        assert!(read_json(&receipt)["derived_manifests"]
            .as_array()
            .unwrap()
            .iter()
            .all(|row| row["action"] == "would-update"));

        let mut apply = args();
        apply.push("--apply".to_owned());
        sync_derived_manifests_command(apply).unwrap();
        assert!(portal.is_file());
        assert!(deploy.is_file());
        let portal_data: toml::Value = fs::read_to_string(portal).unwrap().parse().unwrap();
        assert_eq!(
            string(&portal_data, "canonical_manifest_sha256"),
            Some(manifest_sha256(&manifest).unwrap())
        );

        canonical["derived_manifests"]["portal"]
            .as_table_mut()
            .unwrap()
            .insert(
                "identity_status".to_owned(),
                toml::Value::String("bound".to_owned()),
            );
        canonical["derived_manifests"]["deploy"]
            .as_table_mut()
            .unwrap()
            .insert(
                "identity_status".to_owned(),
                toml::Value::String("pending".to_owned()),
            );
        fs::write(&manifest, toml::to_string_pretty(&canonical).unwrap()).unwrap();
        fs::write(&deploy, b"pending deploy bytes must remain unchanged\n").unwrap();
        let deploy_before = fs::read(&deploy).unwrap();
        let targeted = vec![
            "--manifest".to_owned(),
            manifest.display().to_string(),
            "--receipt".to_owned(),
            receipt.display().to_string(),
            "--target".to_owned(),
            "portal".to_owned(),
            "--apply".to_owned(),
        ];
        sync_derived_manifests_command(targeted).unwrap();
        assert_eq!(fs::read(&deploy).unwrap(), deploy_before);
        let rows = read_json(&receipt)["derived_manifests"]
            .as_array()
            .unwrap()
            .clone();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["target"], "portal");
        assert_eq!(rows[0]["action"], "updated");

        let mut unknown = args();
        unknown.extend(["--target".to_owned(), "unknown".to_owned()]);
        assert!(sync_derived_manifests_command(unknown)
            .unwrap_err()
            .to_string()
            .contains("unknown derived manifest targets"));
        let mut duplicate = args();
        duplicate.extend([
            "--target".to_owned(),
            "portal".to_owned(),
            "--target".to_owned(),
            "portal".to_owned(),
        ]);
        assert!(sync_derived_manifests_command(duplicate)
            .unwrap_err()
            .to_string()
            .contains("duplicate derived manifest target"));
    }

    #[test]
    fn path_checks_only_require_onboarded_repository_checkouts() {
        let default_repo: toml::Value = "name = \"core\"".parse().unwrap();
        let excluded_repo: toml::Value = "name = \"python\"\nonboarded = false".parse().unwrap();
        assert!(repo_is_onboarded(&default_repo));
        assert!(!repo_is_onboarded(&excluded_repo));
    }

    #[test]
    fn host_ci_request_snapshot_is_bounded_and_rejects_special_inodes() {
        let root = TestDir::new("host-ci-snapshot");
        let source = root.path().join("request.json");
        fs::write(&source, b"{\"request\":true}\n").unwrap();
        fs::set_permissions(&source, fs::Permissions::from_mode(0o600)).unwrap();
        let metadata = fs::metadata(&source).unwrap();
        let destination = root.path().join("snapshot.json");
        snapshot_host_ci_request(
            &source,
            &destination,
            metadata.uid(),
            metadata.gid(),
            65_536,
        )
        .unwrap();
        assert_eq!(fs::read(&destination).unwrap(), fs::read(&source).unwrap());

        let oversized = root.path().join("oversized.json");
        fs::write(&oversized, vec![b'x'; 65_537]).unwrap();
        fs::set_permissions(&oversized, fs::Permissions::from_mode(0o600)).unwrap();
        assert!(snapshot_host_ci_request(
            &oversized,
            &root.path().join("oversized-copy"),
            metadata.uid(),
            metadata.gid(),
            65_536,
        )
        .is_err());

        let hardlink = root.path().join("request-hardlink");
        fs::hard_link(&source, &hardlink).unwrap();
        assert!(snapshot_host_ci_request(
            &source,
            &root.path().join("hardlink-copy"),
            metadata.uid(),
            metadata.gid(),
            65_536,
        )
        .is_err());
        fs::remove_file(hardlink).unwrap();

        let fifo = root.path().join("request-fifo");
        assert!(Command::new("mkfifo")
            .args(["-m", "0600"])
            .arg(&fifo)
            .status()
            .unwrap()
            .success());
        assert!(snapshot_host_ci_request(
            &fifo,
            &root.path().join("fifo-copy"),
            metadata.uid(),
            metadata.gid(),
            65_536,
        )
        .is_err());
    }
}
