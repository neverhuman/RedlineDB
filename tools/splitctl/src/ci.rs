//! Typed CI planning, execution, and performance evidence.
//!
//! The optimized runner is deliberately inactive until release-full
//! equivalence and the protected root-broker boundary are proven. The root
//! broker owns profile selection and plan creation; v1 plans cannot execute.

use super::{
    control_plane_root, is_full_sha, managed_repositories, manifest_sha256, release_cargo_policy,
    release_repo_entry, sha256_regular_file, write_json_receipt,
};
use serde_json::{json, Map, Value as JsonValue};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    env, fs,
    io::Read,
    os::unix::{
        fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
        process::CommandExt,
    },
    path::{Component, Path, PathBuf},
    process::{Command, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const PLAN_SCHEMA: &str = "jain.ci-plan/v1";
const LANE_RESULT_SCHEMA: &str = "jain.ci-lane-result/v1";
const RUN_SCHEMA: &str = "jain.host-ci-result/v6";
const HOST_EVIDENCE_SCHEMA: &str = "jain.host-ci-evidence/v6";
const PERFORMANCE_SCHEMA: &str = "jain.ci-performance/v1";
const MAX_PLAN_BYTES: u64 = 4 * 1024 * 1024;
const MAX_EVIDENCE_BYTES: u64 = 8 * 1024 * 1024;
const MAX_EVIDENCE_FILES: usize = 4096;
const MAX_LANES: usize = 64;
const MAX_CANONICAL_ID: usize = 96;
const MAX_CHANGED_PATHS: usize = 20_000;
const MAX_STRING: usize = 16 * 1024;
const MAX_LANE_DURATION_MS: u64 = 3_660_000;
const PRESUBMIT_LIMIT_MS: u64 = 5 * 60 * 1000;
const RELEASE_FULL_LIMIT_MS: u64 = 15 * 60 * 1000;
const MIN_PERFORMANCE_SAMPLES: usize = 20;
const MIN_AVAILABLE_MEMORY_BYTES: u64 = 20 * 1024 * 1024 * 1024;
const SCCACHE_MAX_BYTES: u64 = 40 * 1024 * 1024 * 1024;
const EXECUTION_BOUNDARY: &str = "root-systemd-filesystem-v1";
const EVIDENCE_SEALER: &str = "root-create-only-v1";
const FLEET_SCHEDULER: &str = "root-fleet-lease-v1";
const MEASUREMENT_SOURCE: &str = "continuous-cgroup-v1";
const SOURCE_POSTCONDITION: &str = "exact-tree-clean-v1";
const TOOL_MANIFEST: &str = "root-readonly-tool-manifest-v1";
const CALIBRATION_SETTINGS: [(u64, u64); 20] = [
    (4, 2),
    (8, 2),
    (16, 2),
    (24, 2),
    (32, 2),
    (4, 4),
    (8, 4),
    (16, 4),
    (24, 4),
    (32, 4),
    (4, 6),
    (8, 6),
    (16, 6),
    (24, 6),
    (32, 6),
    (4, 8),
    (8, 8),
    (16, 8),
    (24, 8),
    (32, 8),
];

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Profile {
    Presubmit,
    ReleaseFull,
}

impl Profile {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "presubmit" => Ok(Self::Presubmit),
            "release-full" => Ok(Self::ReleaseFull),
            _ => Err("CI profile must be exactly presubmit or release-full".to_owned()),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Presubmit => "presubmit",
            Self::ReleaseFull => "release-full",
        }
    }
}

pub(crate) fn plan_command(args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    if unsafe { libc::geteuid() } != 0 {
        return Err("ci-plan profile selection is restricted to the root broker".into());
    }
    let root = control_plane_root();
    let mut manifest = root.join("repos.manifest.toml");
    let mut repository = None;
    let mut head = None;
    let mut base = None;
    let mut profile = None;
    let mut output = None;
    let mut calibration = false;
    let mut build_jobs = None;
    let mut fleet_concurrency = None;
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--manifest" => manifest = PathBuf::from(iter.next().ok_or("--manifest needs a path")?),
            "--repo" => repository = Some(iter.next().ok_or("--repo needs a name")?),
            "--head" => head = Some(iter.next().ok_or("--head needs a SHA")?),
            "--base" => base = Some(iter.next().ok_or("--base needs a SHA")?),
            "--profile" => profile = Some(iter.next().ok_or("--profile needs a value")?),
            "--output" => output = Some(PathBuf::from(iter.next().ok_or("--output needs a path")?)),
            "--calibration" => calibration = true,
            "--build-jobs" => {
                build_jobs = Some(
                    iter.next()
                        .ok_or("--build-jobs needs a value")?
                        .parse::<u64>()?,
                )
            }
            "--fleet-concurrency" => {
                fleet_concurrency = Some(
                    iter.next()
                        .ok_or("--fleet-concurrency needs a value")?
                        .parse::<u64>()?,
                )
            }
            value => return Err(format!("unknown ci-plan argument: {value}").into()),
        }
    }
    let profile = Profile::parse(&profile.ok_or("ci-plan requires --profile")?)?;
    let calibration = match (calibration, build_jobs, fleet_concurrency) {
        (false, None, None) => None,
        (true, Some(build_jobs), Some(fleet_concurrency)) => {
            if ![4, 8, 16, 24, 32].contains(&build_jobs)
                || ![2, 4, 6, 8].contains(&fleet_concurrency)
            {
                return Err("calibration settings must use build jobs 4/8/16/24/32 and fleet concurrency 2/4/6/8".into());
            }
            Some((build_jobs, fleet_concurrency))
        }
        _ => return Err("--calibration requires both --build-jobs and --fleet-concurrency".into()),
    };
    let repository = repository.ok_or("ci-plan requires --repo")?;
    let head = head.ok_or("ci-plan requires --head")?;
    let base = base.ok_or("ci-plan requires --base")?;
    let plan = build_plan(PlanRequest {
        root: &root,
        manifest: &manifest,
        repository: &repository,
        head: &head,
        base: &base,
        requested_profile: profile,
        broker_uid: 0,
        calibration,
    })?;
    if let Some(path) = output {
        write_json_receipt(&path, &plan)?;
        let mut permissions = fs::metadata(&path)?.permissions();
        permissions.set_mode(0o444);
        fs::set_permissions(&path, permissions)?;
        println!("wrote {}", path.display());
    } else {
        println!("{}", serde_json::to_string_pretty(&plan)?);
    }
    Ok(())
}

struct PlanRequest<'a> {
    root: &'a Path,
    manifest: &'a Path,
    repository: &'a str,
    head: &'a str,
    base: &'a str,
    requested_profile: Profile,
    broker_uid: u32,
    calibration: Option<(u64, u64)>,
}

fn build_plan(request: PlanRequest<'_>) -> Result<JsonValue, Box<dyn std::error::Error>> {
    let PlanRequest {
        root,
        manifest,
        repository,
        head,
        base,
        requested_profile,
        broker_uid,
        calibration,
    } = request;
    require_lower_full_sha(head, "head")?;
    require_lower_full_sha(base, "base")?;
    let manifest = physical_regular_path(manifest, "CI authority manifest")?;
    let data: toml::Value = fs::read_to_string(&manifest)?.parse()?;
    let managed = managed_repositories(&data, &manifest)?;
    let matches = managed
        .iter()
        .filter(|candidate| candidate.name == repository)
        .collect::<Vec<_>>();
    if matches.len() != 1 {
        return Err(format!("repository {repository} is absent or ambiguous in authority").into());
    }
    let managed_repo = matches[0];
    if managed_repo.inventory_status != "active" {
        return Err(format!("repository {repository} is not active").into());
    }
    let repo = physical_checkout(&managed_repo.path)?;
    let remotes = git_lines(&repo, &["remote"])?;
    if remotes != ["origin"]
        || git_text(&repo, &["remote", "get-url", "origin"])? != managed_repo.remote
    {
        return Err("CI planning requires the sole canonical origin".into());
    }
    if !git_text(
        &repo,
        &["status", "--porcelain=v1", "--untracked-files=all"],
    )?
    .is_empty()
    {
        return Err("CI planning requires an exact clean checkout".into());
    }
    let actual_head = git_commit(&repo, "HEAD")?;
    if actual_head != head {
        return Err(format!("checkout HEAD drifted: expected {head}, found {actual_head}").into());
    }
    if git_commit(&repo, head)? != head || git_commit(&repo, base)? != base {
        return Err("CI head or base does not resolve exactly".into());
    }
    let tracking_main = git_commit(&repo, "refs/remotes/origin/main")?;
    if tracking_main != base {
        return Err(format!("stale CI base: origin/main is {tracking_main}, not {base}").into());
    }
    if !git_success(&repo, &["merge-base", "--is-ancestor", base, head])? {
        return Err("CI base is not an ancestor of the exact head".into());
    }
    let head_tree = git_text(
        &repo,
        &["rev-parse", "--verify", &format!("{head}^{{tree}}")],
    )?;
    let base_tree = git_text(
        &repo,
        &["rev-parse", "--verify", &format!("{base}^{{tree}}")],
    )?;
    require_lower_full_sha(&head_tree, "head tree")?;
    require_lower_full_sha(&base_tree, "base tree")?;
    reject_prohibited_tree_modes(&repo, head)?;

    let changed_paths = git_nul_paths(&repo, &["diff", "--name-only", "-z", base, head])?;
    if changed_paths.len() > MAX_CHANGED_PATHS {
        return Err("changed-path count exceeds the bounded CI plan limit".into());
    }
    let tracked_paths = git_nul_paths(&repo, &["ls-tree", "-r", "--name-only", "-z", head])?;
    let raw = release_repo_entry(&data, repository)?;
    let cross_repo_dependencies = toml_string_array(raw, "cross_repo_deps")?;
    let rust_packages = affected_rust_packages(&repo, head, &tracked_paths, &changed_paths)?;
    let node_packages = affected_node_packages(&tracked_paths, &changed_paths);
    let generated_zones = generated_zone_paths(&repo, head, &tracked_paths)?;
    let widening_reasons = widening_reasons(&changed_paths, &generated_zones);
    let effective_profile =
        if requested_profile == Profile::ReleaseFull || !widening_reasons.is_empty() {
            Profile::ReleaseFull
        } else {
            Profile::Presubmit
        };
    if effective_profile == Profile::Presubmit && changed_paths.is_empty() {
        return Err("presubmit requires a non-empty exact base..head change".into());
    }

    let cargo_configuration = validate_cargo_configuration(&repo, head, &tracked_paths)?;
    let executables = bound_executables(
        &repo,
        head,
        &tracked_paths,
        effective_profile,
        !node_packages.is_empty(),
    )?;
    let lanes = if effective_profile == Profile::ReleaseFull {
        release_full_lanes(repository, raw, &tracked_paths, &executables)?
    } else {
        presubmit_lanes(
            &repo,
            head,
            &changed_paths,
            &rust_packages,
            &node_packages,
            &cross_repo_dependencies,
            raw,
            &executables,
        )?
    };
    validate_generated_lanes(
        &lanes,
        effective_profile,
        !rust_packages.is_empty(),
        !node_packages.is_empty(),
        presubmit_requires_contract(&changed_paths, &cross_repo_dependencies),
        !cross_repo_dependencies.is_empty(),
    )?;

    let authority_sha256 = manifest_sha256(&manifest)?;
    let lockfiles = bound_inputs(&repo, head, &tracked_paths, is_lockfile)?;
    let toolchain_files = bound_inputs(&repo, head, &tracked_paths, is_toolchain_file)?;
    let manifest_ci_jobs = data
        .get("ci_jobs")
        .and_then(toml::Value::as_integer)
        .filter(|value| (1..=64).contains(value))
        .ok_or("authority manifest has no bounded ci_jobs")? as u64;
    let manifest_fleet_jobs =
        data.get("fleet_jobs")
            .and_then(toml::Value::as_integer)
            .filter(|value| (1..=64).contains(value))
            .ok_or("authority manifest has no bounded fleet_jobs")? as u64;
    let (build_jobs, fleet_concurrency) =
        calibration.unwrap_or((manifest_ci_jobs, manifest_fleet_jobs));
    let max_parallel = build_jobs.min(8);
    let host_network_namespace = fs::metadata("/proc/self/ns/net")?;
    let contract_root = root.join("contracts");
    let cache = match effective_profile {
        Profile::Presubmit => json!({
            "mode": "content-addressed-local-only",
            "enabled": executables.contains_key("sccache"),
            "root": "/var/cache/jain-ci/sccache",
            "max_bytes": SCCACHE_MAX_BYTES,
            "compiler_cache_forbidden_for_release": true
        }),
        Profile::ReleaseFull => json!({
            "mode": "forbidden",
            "enabled": false,
            "root": null,
            "max_bytes": 0,
            "compiler_cache_forbidden_for_release": true
        }),
    };
    let coverage_scope = json!({
        "changed_paths": changed_paths,
        "rust_packages": rust_packages,
        "node_packages": node_packages,
        "contract_consumers": cross_repo_dependencies,
        "scope": if effective_profile == Profile::ReleaseFull { "complete" } else { "changed-surface" }
    });
    let plan = json!({
        "schema_version": PLAN_SCHEMA,
        "repository": repository,
        "repository_path": repo,
        "required_check": managed_repo.required_check,
        "head_sha": head,
        "head_tree": head_tree,
        "base_sha": base,
        "base_tree": base_tree,
        "authority": {
            "manifest_path": manifest,
            "manifest_sha256": authority_sha256,
            "plan_schema_sha256": sha256_regular_file(&contract_root.join("ci-plan.schema.json"), "CI plan schema")?,
            "lane_result_schema_sha256": sha256_regular_file(&contract_root.join("ci-lane-result.schema.json"), "CI lane-result schema")?,
            "performance_schema_sha256": sha256_regular_file(&contract_root.join("ci-performance.schema.json"), "CI performance schema")?,
            "host_result_schema_sha256": sha256_regular_file(&contract_root.join("host-ci-result.schema.json"), "host CI result schema")?,
            "host_evidence_schema_sha256": sha256_regular_file(&contract_root.join("host-ci-evidence.schema.json"), "host CI evidence schema")?
        },
        "profile": {
            "requested": requested_profile.as_str(),
            "effective": effective_profile.as_str(),
            "widened": requested_profile != effective_profile,
            "widening_reasons": widening_reasons,
            "selected_by_uid": broker_uid
        },
        "source_scope": coverage_scope,
        "cross_repo_dependencies": cross_repo_dependencies,
        "cargo_configuration": cargo_configuration,
        "toolchains": toolchain_files,
        "lockfiles": lockfiles,
        "executables": executables.values().cloned().collect::<Vec<_>>(),
        "resources": {
            "max_parallel_lanes": max_parallel,
            "build_jobs": build_jobs,
            "fleet_concurrency": fleet_concurrency,
            "authority_build_jobs": manifest_ci_jobs,
            "authority_fleet_concurrency": manifest_fleet_jobs,
            "host_network_namespace_device": host_network_namespace.dev(),
            "host_network_namespace_inode": host_network_namespace.ino(),
            "calibration": calibration.is_some(),
            "minimum_available_memory_bytes": MIN_AVAILABLE_MEMORY_BYTES,
            "swap_in_pages_max": 0,
            "io_wait_percent_max_exclusive": 15.0,
            "load1_max_exclusive": 96.0,
            "network": "denied"
        },
        "cache": cache,
        "lanes": lanes,
        "execution": {
            "allowed": false,
            "activation_state": "blocked",
            "activation_requires": "release-full-equivalence",
            "required_boundary": EXECUTION_BOUNDARY,
            "required_evidence_sealer": EVIDENCE_SEALER,
            "required_fleet_scheduler": FLEET_SCHEDULER,
            "required_measurement_source": MEASUREMENT_SOURCE,
            "required_source_postcondition": SOURCE_POSTCONDITION,
            "required_tool_manifest": TOOL_MANIFEST
        },
        "publication": {
            "mode": "shadow-equivalence",
            "allowed": false,
            "required_equivalence_profile": "release-full",
            "required_check": managed_repo.required_check
        }
    });
    validate_plan_value(&plan)?;
    Ok(plan)
}

fn physical_regular_path(path: &Path, kind: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    if !path.is_absolute() {
        return Err(format!("{kind} path must be absolute").into());
    }
    let canonical = fs::canonicalize(path)?;
    if canonical != path {
        return Err(format!("{kind} path is not canonical").into());
    }
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.file_type().is_file() || metadata.file_type().is_symlink() || metadata.nlink() != 1
    {
        return Err(format!("{kind} is not a physical single-link file").into());
    }
    Ok(canonical)
}

fn physical_checkout(path: &Path) -> Result<PathBuf, Box<dyn std::error::Error>> {
    if !path.is_absolute() {
        return Err("repository path must be absolute".into());
    }
    let canonical = fs::canonicalize(path)?;
    if canonical != path {
        return Err("repository path is not canonical".into());
    }
    let root = fs::symlink_metadata(path)?;
    let dot_git = fs::symlink_metadata(path.join(".git"))?;
    if !root.file_type().is_dir()
        || root.file_type().is_symlink()
        || !dot_git.file_type().is_dir()
        || dot_git.file_type().is_symlink()
    {
        return Err("repository must own a physical Git directory".into());
    }
    for forbidden in [
        path.join(".git/commondir"),
        path.join(".git/worktrees"),
        path.join(".git/objects/info/alternates"),
    ] {
        if fs::symlink_metadata(&forbidden).is_ok() {
            return Err(format!(
                "repository contains forbidden Git metadata: {}",
                forbidden.display()
            )
            .into());
        }
    }
    Ok(canonical)
}

fn git_command(repo: &Path) -> Command {
    let mut command = Command::new("/usr/bin/git");
    command
        .env_clear()
        .env("LC_ALL", "C")
        .args([
            "-c",
            "core.fsmonitor=false",
            "-c",
            "core.hooksPath=/dev/null",
            "-c",
            "core.untrackedCache=false",
            "-c",
            "diff.external=",
            "-c",
            &format!("safe.directory={}", repo.display()),
            "-C",
        ])
        .arg(repo);
    command
}

fn git_bytes(repo: &Path, args: &[&str]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let output = git_command(repo).args(args).output()?;
    if !output.status.success() {
        return Err(format!(
            "credentialless git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        )
        .into());
    }
    Ok(output.stdout)
}

fn git_text(repo: &Path, args: &[&str]) -> Result<String, Box<dyn std::error::Error>> {
    Ok(String::from_utf8(git_bytes(repo, args)?)?.trim().to_owned())
}

fn git_lines(repo: &Path, args: &[&str]) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    Ok(git_text(repo, args)?.lines().map(str::to_owned).collect())
}

fn git_success(repo: &Path, args: &[&str]) -> Result<bool, Box<dyn std::error::Error>> {
    Ok(git_command(repo)
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?
        .success())
}

fn git_commit(repo: &Path, value: &str) -> Result<String, Box<dyn std::error::Error>> {
    let resolved = git_text(
        repo,
        &["rev-parse", "--verify", &format!("{value}^{{commit}}")],
    )?;
    require_lower_full_sha(&resolved, "Git commit")?;
    Ok(resolved)
}

fn git_nul_paths(repo: &Path, args: &[&str]) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let bytes = git_bytes(repo, args)?;
    if !bytes.is_empty() && !bytes.ends_with(&[0]) {
        return Err("Git path list is not NUL terminated".into());
    }
    let mut paths = Vec::new();
    for raw in bytes.split(|byte| *byte == 0).filter(|raw| !raw.is_empty()) {
        let path = std::str::from_utf8(raw)?.to_owned();
        validate_relative_path(&path)?;
        paths.push(path);
    }
    if paths.iter().collect::<BTreeSet<_>>().len() != paths.len() {
        return Err("Git path list contains duplicates".into());
    }
    Ok(paths)
}

fn require_lower_full_sha(value: &str, kind: &str) -> Result<(), Box<dyn std::error::Error>> {
    if !is_full_sha(value) || value.chars().any(|ch| ch.is_ascii_uppercase()) {
        return Err(format!("{kind} must be a lowercase full 40-character SHA").into());
    }
    Ok(())
}

fn validate_relative_path(value: &str) -> Result<(), Box<dyn std::error::Error>> {
    if value.is_empty()
        || value.len() > 4096
        || value.bytes().any(|byte| byte.is_ascii_control())
        || Path::new(value).is_absolute()
        || Path::new(value)
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(format!("unsafe repository-relative path: {value:?}").into());
    }
    Ok(())
}

fn reject_prohibited_tree_modes(repo: &Path, head: &str) -> Result<(), Box<dyn std::error::Error>> {
    let tree = git_bytes(repo, &["ls-tree", "-r", "-z", head])?;
    for row in tree.split(|byte| *byte == 0).filter(|row| !row.is_empty()) {
        let row = std::str::from_utf8(row)?;
        let mode = row.split_ascii_whitespace().next().unwrap_or_default();
        if mode != "100644" && mode != "100755" {
            return Err(format!("CI head contains prohibited Git mode {mode}").into());
        }
    }
    Ok(())
}

fn toml_string_array(raw: &toml::Value, key: &str) -> Result<Vec<String>, String> {
    let Some(value) = raw.get(key) else {
        return Ok(Vec::new());
    };
    let array = value
        .as_array()
        .ok_or_else(|| format!("{key} must be an array"))?;
    let mut result = Vec::with_capacity(array.len());
    for item in array {
        let value = item
            .as_str()
            .ok_or_else(|| format!("{key} entries must be strings"))?;
        if value.is_empty() || value.len() > 128 {
            return Err(format!("{key} contains an unsafe string"));
        }
        result.push(value.to_owned());
    }
    if result.iter().collect::<BTreeSet<_>>().len() != result.len() {
        return Err(format!("{key} contains duplicates"));
    }
    Ok(result)
}

fn is_lockfile(path: &str) -> bool {
    matches!(
        Path::new(path).file_name().and_then(|name| name.to_str()),
        Some(
            "Cargo.lock"
                | "pnpm-lock.yaml"
                | "package-lock.json"
                | "yarn.lock"
                | "bun.lockb"
                | "deno.lock"
        )
    )
}

fn is_toolchain_file(path: &str) -> bool {
    matches!(
        path,
        "rust-toolchain" | "rust-toolchain.toml" | ".node-version" | ".nvmrc" | "mise.toml"
    )
}

fn is_cargo_configuration(path: &str) -> bool {
    path == ".cargo/config"
        || path == ".cargo/config.toml"
        || path.ends_with("/.cargo/config")
        || path.ends_with("/.cargo/config.toml")
}

fn validate_cargo_configuration(
    repo: &Path,
    head: &str,
    tracked: &[String],
) -> Result<Vec<JsonValue>, Box<dyn std::error::Error>> {
    let inputs = bound_inputs(repo, head, tracked, is_cargo_configuration)?;
    for input in &inputs {
        let path = input["path"].as_str().unwrap();
        let bytes = git_bytes(repo, &["show", &format!("{head}:{path}")])?;
        let value: toml::Value = std::str::from_utf8(&bytes)?.parse()?;
        reject_unsafe_cargo_configuration(&value, "")?;
    }
    Ok(inputs)
}

fn reject_unsafe_cargo_configuration(
    value: &toml::Value,
    parent: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let Some(table) = value.as_table() else {
        return Ok(());
    };
    for (key, child) in table {
        let path = if parent.is_empty() {
            key.clone()
        } else {
            format!("{parent}.{key}")
        };
        if matches!(
            key.as_str(),
            "alias"
                | "ar"
                | "credential-provider"
                | "global-credential-providers"
                | "linker"
                | "replace-with"
                | "runner"
                | "rustc"
                | "rustc-wrapper"
                | "rustc-workspace-wrapper"
                | "rustflags"
        ) {
            return Err(
                format!("Cargo configuration contains forbidden execution key {path}").into(),
            );
        }
        reject_unsafe_cargo_configuration(child, &path)?;
    }
    Ok(())
}

fn bound_inputs(
    repo: &Path,
    head: &str,
    tracked: &[String],
    predicate: fn(&str) -> bool,
) -> Result<Vec<JsonValue>, Box<dyn std::error::Error>> {
    let mut result = Vec::new();
    for path in tracked.iter().filter(|path| predicate(path)) {
        let bytes = git_bytes(repo, &["show", &format!("{head}:{path}")])?;
        result.push(json!({
            "path": path,
            "sha256": format!("{:x}", Sha256::digest(&bytes)),
            "bytes": bytes.len()
        }));
    }
    Ok(result)
}

fn generated_zone_paths(
    repo: &Path,
    head: &str,
    tracked: &[String],
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    if !tracked
        .iter()
        .any(|path| path == "agent/generated-zones.toml")
    {
        return Ok(Vec::new());
    }
    let bytes = git_bytes(
        repo,
        &["show", &format!("{head}:agent/generated-zones.toml")],
    )?;
    if bytes.len() > MAX_PLAN_BYTES as usize {
        return Err("generated-zone authority exceeds the bounded parser limit".into());
    }
    let value: toml::Value = std::str::from_utf8(&bytes)?.parse()?;
    let zones = value
        .get("zone")
        .and_then(toml::Value::as_array)
        .ok_or("generated-zone authority has no zone array")?;
    let mut paths = BTreeSet::new();
    for zone in zones {
        let path = zone
            .get("path")
            .and_then(toml::Value::as_str)
            .ok_or("generated zone has no path")?;
        let normalized = path.strip_suffix('/').unwrap_or(path);
        validate_relative_path(normalized)?;
        if !paths.insert(path.to_owned()) {
            return Err("generated-zone authority contains duplicate paths".into());
        }
    }
    Ok(paths.into_iter().collect())
}

fn widening_reasons(changed: &[String], generated_zones: &[String]) -> Vec<String> {
    let mut reasons = BTreeSet::new();
    for path in changed {
        let reason = if is_lockfile(path) {
            Some("lockfile")
        } else if path == "repos.manifest.toml" || path == "Cargo.toml" || path == "Justfile" {
            Some("control")
        } else if path.starts_with("ops/ci/")
            || path.starts_with("scripts/ci-")
            || path.starts_with(".github/")
        {
            Some("ci-control")
        } else if path.starts_with("tools/splitctl/") {
            Some("control-plane-source")
        } else if path.starts_with("contracts/") {
            Some("contract")
        } else if path.starts_with("agent/")
            || generated_zones.iter().any(|zone| {
                zone.strip_suffix('/').map_or(path == zone, |prefix| {
                    path == prefix || path.starts_with(&format!("{prefix}/"))
                })
            })
        {
            Some("generated-or-source-policy")
        } else if matches!(
            Path::new(path).extension().and_then(|value| value.to_str()),
            Some("c" | "cc" | "cpp" | "cxx" | "h" | "hh" | "hpp" | "hxx" | "s" | "S")
        ) {
            Some("native-code")
        } else {
            None
        };
        if let Some(reason) = reason {
            reasons.insert(reason.to_owned());
        }
    }
    reasons.into_iter().collect()
}

#[derive(Debug)]
struct CargoPackage {
    name: String,
    directory: String,
    dependencies: BTreeSet<String>,
}

fn affected_rust_packages(
    repo: &Path,
    head: &str,
    tracked: &[String],
    changed: &[String],
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let mut packages = Vec::new();
    for manifest in tracked.iter().filter(|path| path.ends_with("Cargo.toml")) {
        let bytes = git_bytes(repo, &["show", &format!("{head}:{manifest}")])?;
        let parsed: toml::Value = std::str::from_utf8(&bytes)?.parse()?;
        let Some(name) = parsed
            .get("package")
            .and_then(|package| package.get("name"))
            .and_then(toml::Value::as_str)
        else {
            continue;
        };
        let directory = Path::new(manifest)
            .parent()
            .and_then(Path::to_str)
            .unwrap_or_default()
            .to_owned();
        let mut dependencies = BTreeSet::new();
        collect_dependency_names(&parsed, &mut dependencies);
        packages.push(CargoPackage {
            name: name.to_owned(),
            directory,
            dependencies,
        });
    }
    let rust_changed = changed.iter().any(|path| {
        path.ends_with(".rs")
            || path.ends_with("Cargo.toml")
            || path.ends_with("Cargo.lock")
            || path.starts_with("build.rs")
            || path.ends_with("/build.rs")
    });
    if !rust_changed {
        return Ok(Vec::new());
    }
    let mut selected = BTreeSet::new();
    for path in changed {
        let mut candidates = packages
            .iter()
            .filter(|package| {
                package.directory.is_empty() || path.starts_with(&format!("{}/", package.directory))
            })
            .collect::<Vec<_>>();
        candidates.sort_by_key(|package| package.directory.len());
        if let Some(package) = candidates.last() {
            selected.insert(package.name.clone());
        }
    }
    if selected.is_empty() {
        selected.extend(packages.iter().map(|package| package.name.clone()));
    }
    loop {
        let before = selected.len();
        for package in &packages {
            if package
                .dependencies
                .iter()
                .any(|dependency| selected.contains(dependency))
            {
                selected.insert(package.name.clone());
            }
        }
        if selected.len() == before {
            break;
        }
    }
    Ok(selected.into_iter().collect())
}

fn collect_dependency_names(value: &toml::Value, names: &mut BTreeSet<String>) {
    let Some(table) = value.as_table() else {
        return;
    };
    for (key, value) in table {
        if matches!(
            key.as_str(),
            "dependencies" | "dev-dependencies" | "build-dependencies"
        ) {
            if let Some(dependencies) = value.as_table() {
                for (name, specification) in dependencies {
                    let actual = specification
                        .as_table()
                        .and_then(|table| table.get("package"))
                        .and_then(toml::Value::as_str)
                        .unwrap_or(name);
                    names.insert(actual.to_owned());
                }
            }
        } else if key == "target" {
            collect_dependency_names(value, names);
        }
    }
}

fn affected_node_packages(tracked: &[String], changed: &[String]) -> Vec<String> {
    let mut package_roots = tracked
        .iter()
        .filter(|path| path.ends_with("package.json"))
        .map(|path| {
            Path::new(path)
                .parent()
                .and_then(Path::to_str)
                .unwrap_or_default()
                .to_owned()
        })
        .collect::<Vec<_>>();
    package_roots.sort_by_key(|root| root.len());
    let mut selected = BTreeSet::new();
    for path in changed.iter().filter(|path| {
        matches!(
            Path::new(path).extension().and_then(|value| value.to_str()),
            Some("js" | "jsx" | "ts" | "tsx" | "mjs" | "cjs" | "vue" | "svelte")
        ) || matches!(
            Path::new(path).file_name().and_then(|value| value.to_str()),
            Some("package.json" | "pnpm-lock.yaml" | "package-lock.json" | "yarn.lock")
        )
    }) {
        if let Some(root) = package_roots
            .iter()
            .rfind(|root| root.is_empty() || path.starts_with(&format!("{root}/")))
        {
            selected.insert(if root.is_empty() {
                ".".to_owned()
            } else {
                root.clone()
            });
        }
    }
    selected.into_iter().collect()
}

fn bound_executables(
    repo: &Path,
    head: &str,
    tracked: &[String],
    profile: Profile,
    has_node: bool,
) -> Result<BTreeMap<String, JsonValue>, Box<dyn std::error::Error>> {
    let rust_channel = tracked
        .iter()
        .find(|path| path.as_str() == "rust-toolchain.toml")
        .map(|path| -> Result<String, Box<dyn std::error::Error>> {
            let bytes = git_bytes(repo, &["show", &format!("{head}:{path}")])?;
            let value: toml::Value = std::str::from_utf8(&bytes)?.parse()?;
            value
                .get("toolchain")
                .and_then(|toolchain| toolchain.get("channel"))
                .and_then(toml::Value::as_str)
                .map(str::to_owned)
                .ok_or_else(|| "rust-toolchain.toml has no exact channel".into())
        })
        .transpose()?
        .or_else(|| {
            tracked
                .iter()
                .find(|path| path.as_str() == "rust-toolchain")
                .and_then(|path| git_text(repo, &["show", &format!("{head}:{path}")]).ok())
        })
        .unwrap_or_else(|| "1.96.0".to_owned());
    if rust_channel.is_empty()
        || rust_channel.len() > 64
        || !rust_channel
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-'))
    {
        return Err("Rust toolchain channel is unsafe".into());
    }
    let toolchain_directory = if rust_channel.ends_with("-x86_64-unknown-linux-gnu") {
        rust_channel.clone()
    } else {
        format!("{rust_channel}-x86_64-unknown-linux-gnu")
    };
    let toolchain_bin = PathBuf::from(format!(
        "/home/ubuntu/.rustup/toolchains/{toolchain_directory}/bin"
    ));
    let cargo_path = toolchain_bin.join("cargo");
    let mut candidates = BTreeMap::from([
        ("bash", PathBuf::from("/usr/bin/bash")),
        ("cargo", cargo_path),
        ("clippy-driver", toolchain_bin.join("clippy-driver")),
        ("rustc", toolchain_bin.join("rustc")),
        ("rustdoc", toolchain_bin.join("rustdoc")),
        ("rustfmt", toolchain_bin.join("rustfmt")),
        ("time", PathBuf::from("/usr/bin/time")),
    ]);
    if tracked.iter().any(|path| path == "Cargo.toml") {
        candidates.insert(
            "cargo-nextest",
            PathBuf::from("/home/ubuntu/.cargo/bin/cargo-nextest"),
        );
        candidates.insert(
            "cargo-llvm-cov",
            PathBuf::from("/home/ubuntu/.cargo/bin/cargo-llvm-cov"),
        );
    }
    if profile == Profile::Presubmit {
        candidates.insert("sccache", PathBuf::from("/usr/local/libexec/jain/sccache"));
    }
    if has_node {
        candidates.insert(
            "node",
            PathBuf::from("/home/ubuntu/.nvm/versions/node/v26.1.0/bin/node"),
        );
        candidates.insert(
            "pnpm",
            PathBuf::from("/home/ubuntu/.npm-global/lib/node_modules/pnpm/bin/pnpm.mjs"),
        );
    }
    let mut result = BTreeMap::new();
    for (name, path) in candidates {
        let canonical = physical_executable(&path, &format!("CI executable {name}"))?;
        result.insert(
            name.to_owned(),
            json!({
                "name": name,
                "path": canonical,
                "sha256": sha256_regular_file(&canonical, "CI executable")?
            }),
        );
    }
    Ok(result)
}

fn physical_executable(path: &Path, kind: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let canonical = physical_regular_path(path, kind)?;
    let metadata = fs::metadata(&canonical)?;
    if metadata.mode() & 0o111 == 0 {
        return Err(format!("{kind} is not executable").into());
    }
    Ok(canonical)
}

fn executable_path(
    executables: &BTreeMap<String, JsonValue>,
    name: &str,
) -> Result<String, String> {
    executables
        .get(name)
        .and_then(|value| value.get("path"))
        .and_then(JsonValue::as_str)
        .map(str::to_owned)
        .ok_or_else(|| format!("missing bound executable {name}"))
}

// Keeping every closed-schema lane field adjacent makes obligation review at
// the call site substantially clearer than a stateful builder.
#[allow(clippy::too_many_arguments)]
fn lane(
    id: &str,
    kind: &str,
    program: &str,
    args: Vec<String>,
    dependencies: Vec<String>,
    obligations: Vec<String>,
    coverage: Vec<String>,
    timeout_seconds: u64,
    cache_policy: &str,
) -> JsonValue {
    json!({
        "id": id,
        "kind": kind,
        "program": program,
        "args": args,
        "environment": {},
        "dependencies": dependencies,
        "obligations": obligations,
        "coverage": coverage,
        "expected_outputs": [
            format!(".ci-evidence/lanes/{id}.log"),
            format!(".ci-evidence/lanes/{id}.json")
        ],
        "timeout_seconds": timeout_seconds,
        "cache_policy": cache_policy,
        "network": "denied"
    })
}

fn release_full_lanes(
    repository: &str,
    raw: &toml::Value,
    tracked: &[String],
    executables: &BTreeMap<String, JsonValue>,
) -> Result<Vec<JsonValue>, Box<dyn std::error::Error>> {
    let bash = executable_path(executables, "bash")?;
    let cargo = executable_path(executables, "cargo")?;
    let has_cargo = tracked.iter().any(|path| path == "Cargo.toml");
    if !tracked
        .iter()
        .any(|path| path == "ops/ci/typed-required-non-test.sh")
    {
        return Err("release-full requires an explicit typed non-test required mapping".into());
    }
    let mut lanes = vec![lane(
        "required-pre-rust-tests",
        "required",
        &bash,
        vec![
            "ops/ci/typed-required-non-test.sh".to_owned(),
            "pre-rust-tests".to_owned(),
        ],
        vec![],
        vec![
            "compatibility".to_owned(),
            "conformance".to_owned(),
            "repository-specific".to_owned(),
        ],
        vec!["complete-product".to_owned()],
        900,
        "forbidden",
    )];
    let mut rust_qualification_ids = Vec::new();
    if has_cargo {
        let llvm_cov = executable_path(executables, "cargo-llvm-cov")?;
        let policy = release_cargo_policy(repository, raw)?;
        let commands = policy
            .get("commands")
            .and_then(JsonValue::as_array)
            .ok_or("release Cargo policy has no commands")?;
        let mut build_ids = Vec::new();
        let mut emitted_complete_coverage = false;
        for (index, command) in commands.iter().enumerate() {
            let label = command
                .get("label")
                .and_then(JsonValue::as_str)
                .ok_or("release Cargo command has no label")?;
            let original = command
                .get("args")
                .and_then(JsonValue::as_array)
                .ok_or("release Cargo command has no args")?
                .iter()
                .map(|value| {
                    value
                        .as_str()
                        .map(str::to_owned)
                        .ok_or("release Cargo argument is not a string")
                })
                .collect::<Result<Vec<_>, _>>()?;
            let Some(subcommand) = original.first().map(String::as_str) else {
                return Err("release Cargo command has empty args".into());
            };
            let id = format!("release-cargo-{}-{}", index + 1, safe_id(label));
            match subcommand {
                "build" => {
                    build_ids.push(id.clone());
                    lanes.push(lane(
                        &id,
                        "rust-build",
                        &cargo,
                        original,
                        vec!["required-pre-rust-tests".to_owned()],
                        vec![format!("release-build:{label}")],
                        vec!["all-rust-packages".to_owned()],
                        900,
                        "forbidden",
                    ));
                }
                "test" => {
                    let mut args = vec!["llvm-cov".to_owned(), "nextest".to_owned()];
                    args.extend(original.into_iter().skip(1));
                    args.extend([
                        "--lcov".to_owned(),
                        "--output-path".to_owned(),
                        format!("target/llvm-cov/{id}.lcov.info"),
                    ]);
                    let mut obligations = vec![
                        format!("release-test:{label}"),
                        format!("complete-coverage:{label}"),
                    ];
                    if !emitted_complete_coverage {
                        obligations.push("complete-coverage".to_owned());
                        emitted_complete_coverage = true;
                    }
                    rust_qualification_ids.push(id.clone());
                    lanes.push(lane(
                        &id,
                        "rust-test-and-coverage",
                        &llvm_cov,
                        args,
                        build_ids.clone(),
                        obligations,
                        vec!["all-rust-packages".to_owned()],
                        900,
                        "forbidden",
                    ));
                }
                _ => return Err("release Cargo policy contains an unsupported subcommand".into()),
            }
        }
        if !emitted_complete_coverage {
            return Err("release Cargo policy has no typed test-and-coverage command".into());
        }
    }
    let mut post_obligations = vec!["required".to_owned()];
    if !has_cargo {
        post_obligations.push("complete-coverage".to_owned());
    }
    let post_dependencies = if rust_qualification_ids.is_empty() {
        vec!["required-pre-rust-tests".to_owned()]
    } else {
        rust_qualification_ids
    };
    lanes.push(lane(
        "required-post-rust-tests",
        "required",
        &bash,
        vec![
            "ops/ci/typed-required-non-test.sh".to_owned(),
            "post-rust-tests".to_owned(),
        ],
        post_dependencies,
        post_obligations,
        vec!["complete-product".to_owned()],
        900,
        "forbidden",
    ));
    for (id, kind, mode, obligation, dependency) in [
        ("static-security", "security", "security", "security", None),
        (
            "contract-drift",
            "contract",
            "contract-drift",
            "contract-drift",
            None,
        ),
        (
            "jankurai-conformance",
            "jankurai",
            "score",
            "jankurai",
            None,
        ),
        (
            "artifact-qualification",
            "artifact",
            "artifact-support",
            "artifact",
            Some("required-post-rust-tests"),
        ),
    ] {
        lanes.push(lane(
            id,
            kind,
            &bash,
            vec!["scripts/ci-local.sh".to_owned(), mode.to_owned()],
            dependency.into_iter().map(str::to_owned).collect(),
            vec![obligation.to_owned()],
            vec!["complete-product".to_owned()],
            900,
            "forbidden",
        ));
    }
    let dependencies = toml_string_array(raw, "cross_repo_deps")?;
    let commands = toml_string_array(raw, "typed_contract_consumer_commands")?;
    if dependencies.len() != commands.len() {
        return Err(
            "release-full cross-repository consumers require one bound typed command each".into(),
        );
    }
    for (index, (dependency, command)) in dependencies.iter().zip(commands).enumerate() {
        let mut obligations = vec![format!("contract-consumer:{dependency}")];
        if index == 0 {
            obligations.push("contract-consumers".to_owned());
        }
        lanes.push(lane(
            &format!("contract-consumer-{}-{}", index + 1, safe_id(dependency)),
            "contract-consumer",
            &bash,
            vec!["-lc".to_owned(), command],
            vec!["contract-drift".to_owned()],
            obligations,
            vec![dependency.clone()],
            900,
            "forbidden",
        ));
    }
    Ok(lanes)
}

#[allow(clippy::too_many_arguments)]
fn presubmit_lanes(
    repo: &Path,
    head: &str,
    changed: &[String],
    rust_packages: &[String],
    node_packages: &[String],
    cross_repo_dependencies: &[String],
    raw: &toml::Value,
    executables: &BTreeMap<String, JsonValue>,
) -> Result<Vec<JsonValue>, Box<dyn std::error::Error>> {
    let bash = executable_path(executables, "bash")?;
    let cargo = executable_path(executables, "cargo")?;
    let mut lanes = vec![lane(
        "static-security",
        "security",
        &bash,
        vec!["scripts/ci-local.sh".to_owned(), "security".to_owned()],
        vec![],
        vec!["static-security".to_owned()],
        changed.to_vec(),
        300,
        "presubmit-only",
    )];
    if !rust_packages.is_empty() {
        let llvm_cov = executable_path(executables, "cargo-llvm-cov")?;
        let mut package_args = Vec::new();
        for package in rust_packages {
            package_args.extend(["-p".to_owned(), package.clone()]);
        }
        let mut check_args = vec!["check".to_owned(), "--locked".to_owned()];
        check_args.extend(package_args.clone());
        lanes.push(lane(
            "changed-rust-check",
            "rust-check",
            &cargo,
            check_args,
            vec![],
            vec!["changed-rust-build".to_owned()],
            rust_packages.to_vec(),
            300,
            "presubmit-only",
        ));
        let mut coverage_args = vec![
            "llvm-cov".to_owned(),
            "nextest".to_owned(),
            "--locked".to_owned(),
        ];
        coverage_args.extend(package_args);
        coverage_args.extend([
            "--lcov".to_owned(),
            "--output-path".to_owned(),
            "target/llvm-cov/lcov.info".to_owned(),
        ]);
        lanes.push(lane(
            "changed-rust-coverage",
            "coverage",
            &llvm_cov,
            coverage_args,
            vec!["changed-rust-check".to_owned()],
            vec![
                "changed-rust-tests".to_owned(),
                "changed-surface-coverage".to_owned(),
            ],
            rust_packages.to_vec(),
            300,
            "presubmit-only",
        ));
    }
    let test_map = read_json_at(repo, head, "agent/test-map.json")?;
    let tests = test_map
        .get("tests")
        .and_then(JsonValue::as_object)
        .ok_or("agent/test-map.json has no tests object")?;
    let mut routed = BTreeMap::<(String, String), BTreeSet<String>>::new();
    for path in changed {
        for (pattern, route) in tests {
            if test_map_matches(pattern, path) {
                let command = route
                    .get("command")
                    .and_then(JsonValue::as_str)
                    .ok_or("test-map command must be a string")?;
                let lane_name = route
                    .get("lane")
                    .and_then(JsonValue::as_str)
                    .ok_or("test-map lane must be a string")?;
                if node_packages
                    .iter()
                    .any(|package| path_is_within_node_package(package, path))
                    && is_broad_required_route(command)
                {
                    return Err(format!(
                        "affected Node path {path} is routed through broad required instead of an explicit node-test lane"
                    )
                    .into());
                }
                routed
                    .entry((lane_name.to_owned(), command.to_owned()))
                    .or_default()
                    .insert(path.clone());
            }
        }
    }
    if !node_packages.is_empty() {
        let pnpm = executable_path(executables, "pnpm")?;
        for (index, package) in node_packages.iter().enumerate() {
            let command = node_test_command(package);
            let route_key = ("node-test".to_owned(), command.clone());
            let paths = routed.remove(&route_key).ok_or_else(|| {
                format!(
                    "affected Node package {package} requires test-map lane=node-test command={command:?}"
                )
            })?;
            if !paths
                .iter()
                .any(|path| path_is_within_node_package(package, path))
            {
                return Err(format!(
                    "node-test route for {package} does not cover an affected package path"
                )
                .into());
            }
            let package_id = match safe_id(package).as_str() {
                "" => "root".to_owned(),
                value => value.to_owned(),
            };
            let mut obligations = vec![format!("node-package-test:{package_id}")];
            if index == 0 {
                obligations.push("affected-node-tests".to_owned());
            }
            lanes.push(lane(
                &format!("affected-node-{}-{package_id}", index + 1),
                "node-test",
                &pnpm,
                vec![
                    "--dir".to_owned(),
                    package.clone(),
                    "run".to_owned(),
                    "test".to_owned(),
                ],
                vec![],
                obligations,
                paths.into_iter().collect(),
                300,
                "presubmit-only",
            ));
        }
        if routed.keys().any(|(kind, _)| kind == "node-test") {
            return Err("node-test routes must bind exactly one affected package through the governed pnpm command".into());
        }
    }
    let mut route_index = 0;
    for ((kind, command), paths) in routed {
        if command == "just security" || command.ends_with("scripts/ci-local.sh security") {
            continue;
        }
        route_index += 1;
        lanes.push(lane(
            &format!("mapped-{}-{}", route_index, safe_id(&kind)),
            &kind,
            &bash,
            vec!["-lc".to_owned(), command],
            vec![],
            vec![format!("mapped-route:{route_index}")],
            paths.into_iter().collect(),
            300,
            "presubmit-only",
        ));
    }
    if presubmit_requires_contract(changed, cross_repo_dependencies) {
        lanes.push(lane(
            "contract-drift",
            "contract",
            &bash,
            vec![
                "scripts/ci-local.sh".to_owned(),
                "contract-drift".to_owned(),
            ],
            vec![],
            vec!["contract-drift".to_owned()],
            vec!["local-contracts".to_owned()],
            300,
            "presubmit-only",
        ));
        let commands = toml_string_array(raw, "typed_contract_consumer_commands")?;
        if commands.len() != cross_repo_dependencies.len() {
            return Err(
                "presubmit cross-repository consumers require one bound typed command each".into(),
            );
        }
        for (index, (dependency, command)) in
            cross_repo_dependencies.iter().zip(commands).enumerate()
        {
            let mut obligations = vec![format!("contract-consumer:{dependency}")];
            if index == 0 {
                obligations.push("contract-consumers".to_owned());
            }
            lanes.push(lane(
                &format!("contract-consumer-{}-{}", index + 1, safe_id(dependency)),
                "contract-consumer",
                &bash,
                vec!["-lc".to_owned(), command],
                vec!["contract-drift".to_owned()],
                obligations,
                vec![dependency.clone()],
                300,
                "presubmit-only",
            ));
        }
    }
    Ok(lanes)
}

fn read_json_at(
    repo: &Path,
    head: &str,
    path: &str,
) -> Result<JsonValue, Box<dyn std::error::Error>> {
    let bytes = git_bytes(repo, &["show", &format!("{head}:{path}")])?;
    if bytes.len() > MAX_PLAN_BYTES as usize {
        return Err(format!("{path} exceeds the bounded CI parser limit").into());
    }
    Ok(serde_json::from_slice(&bytes)?)
}

fn test_map_matches(pattern: &str, path: &str) -> bool {
    if let Some(prefix) = pattern.strip_suffix("/**") {
        path == prefix || path.starts_with(&format!("{prefix}/"))
    } else if pattern.ends_with('/') {
        path.starts_with(pattern)
    } else {
        pattern == path
    }
}

fn path_is_within_node_package(package: &str, path: &str) -> bool {
    package == "." || path == package || path.starts_with(&format!("{package}/"))
}

fn node_test_command(package: &str) -> String {
    format!("pnpm --dir {package} run test")
}

fn is_broad_required_route(command: &str) -> bool {
    let command = command.trim();
    command == "just required"
        || command.ends_with("ops/ci/required.sh")
        || command.ends_with("scripts/ci-local.sh required")
}

fn presubmit_requires_contract(changed: &[String], cross_repo_dependencies: &[String]) -> bool {
    changed.iter().any(|path| path.starts_with("contracts/"))
        || (!cross_repo_dependencies.is_empty()
            && changed
                .iter()
                .any(|path| path.contains("contract") || path.ends_with(".proto")))
}

fn safe_id(value: &str) -> String {
    let mut result = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>();
    while result.contains("--") {
        result = result.replace("--", "-");
    }
    result
        .trim_matches('-')
        .chars()
        .take(MAX_CANONICAL_ID)
        .collect()
}

fn validate_generated_lanes(
    lanes: &[JsonValue],
    profile: Profile,
    has_changed_rust: bool,
    has_changed_node: bool,
    requires_contract: bool,
    has_contract_consumers: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if lanes.is_empty() || lanes.len() > MAX_LANES {
        return Err("CI plan must contain one through 64 lanes".into());
    }
    let mut ids = BTreeSet::new();
    let mut obligations = BTreeSet::new();
    let mut outputs = BTreeSet::new();
    for lane in lanes {
        let object = exact_object(
            lane,
            &[
                "args",
                "cache_policy",
                "coverage",
                "dependencies",
                "environment",
                "expected_outputs",
                "id",
                "kind",
                "network",
                "obligations",
                "program",
                "timeout_seconds",
            ],
            "CI lane",
        )?;
        let id = bounded_string(&object["id"], "lane id", MAX_CANONICAL_ID)?;
        if safe_id(id) != id || !ids.insert(id.to_owned()) {
            return Err(format!("lane id is unsafe or duplicated: {id}").into());
        }
        bounded_string(&object["kind"], "lane kind", 96)?;
        let program = bounded_string(&object["program"], "lane program", 4096)?;
        if !Path::new(program).is_absolute() {
            return Err("lane program must be an absolute bound executable".into());
        }
        let args = bounded_string_array(&object["args"], "lane args", 128)?;
        if args.iter().any(|arg| arg.as_bytes().contains(&0)) {
            return Err("lane arguments may not contain NUL".into());
        }
        let environment = object["environment"]
            .as_object()
            .ok_or("lane environment must be an object")?;
        if !environment.is_empty() {
            return Err("v1 root-generated lanes do not admit caller environment entries".into());
        }
        bounded_string_array(&object["coverage"], "lane coverage", MAX_CHANGED_PATHS)?;
        let dependencies =
            bounded_string_array(&object["dependencies"], "lane dependencies", MAX_LANES)?;
        if dependencies.iter().collect::<BTreeSet<_>>().len() != dependencies.len()
            || dependencies.contains(&id)
        {
            return Err(format!("lane {id} has duplicate or self dependencies").into());
        }
        for obligation in bounded_string_array(&object["obligations"], "lane obligations", 64)? {
            if !obligations.insert(obligation.to_owned()) {
                return Err(format!("CI obligation appears more than once: {obligation}").into());
            }
        }
        let expected_outputs = [
            format!(".ci-evidence/lanes/{id}.json"),
            format!(".ci-evidence/lanes/{id}.log"),
        ]
        .into_iter()
        .collect::<BTreeSet<_>>();
        let mut lane_outputs = BTreeSet::new();
        for output in bounded_string_array(&object["expected_outputs"], "lane outputs", 4)? {
            validate_relative_path(output)?;
            if !output.starts_with(&format!(".ci-evidence/lanes/{id}."))
                || !lane_outputs.insert(output.to_owned())
                || !outputs.insert(output.to_owned())
            {
                return Err(format!("lane {id} has unsafe or duplicate expected output").into());
            }
        }
        if lane_outputs != expected_outputs {
            return Err(format!("lane {id} does not bind its exact log and result outputs").into());
        }
        let timeout = object["timeout_seconds"]
            .as_u64()
            .filter(|value| (1..=3600).contains(value))
            .ok_or("lane timeout must be an integer from 1 through 3600")?;
        let _ = timeout;
        let expected_cache = if profile == Profile::ReleaseFull {
            "forbidden"
        } else {
            "presubmit-only"
        };
        if object["cache_policy"].as_str() != Some(expected_cache)
            || object["network"].as_str() != Some("denied")
        {
            return Err(format!("lane {id} has invalid cache or network policy").into());
        }
    }
    for lane in lanes {
        for dependency in lane["dependencies"].as_array().unwrap() {
            if !ids.contains(dependency.as_str().unwrap_or_default()) {
                return Err(format!(
                    "lane {} depends on an unknown lane",
                    lane["id"].as_str().unwrap_or_default()
                )
                .into());
            }
        }
    }
    validate_dag(lanes)?;
    let mut required = match profile {
        Profile::Presubmit => vec!["static-security"],
        Profile::ReleaseFull => vec![
            "required",
            "security",
            "complete-coverage",
            "contract-drift",
            "jankurai",
            "artifact",
            "compatibility",
            "conformance",
            "repository-specific",
        ],
    };
    if profile == Profile::Presubmit && has_changed_rust {
        required.extend([
            "changed-rust-build",
            "changed-rust-tests",
            "changed-surface-coverage",
        ]);
    }
    if profile == Profile::Presubmit && has_changed_node {
        required.push("affected-node-tests");
    }
    if profile == Profile::Presubmit && requires_contract {
        required.push("contract-drift");
    }
    if has_contract_consumers
        && (profile == Profile::ReleaseFull || (profile == Profile::Presubmit && requires_contract))
    {
        required.push("contract-consumers");
    }
    for requirement in required {
        if !obligations.contains(requirement) {
            return Err(format!("CI plan omits required obligation {requirement}").into());
        }
    }
    Ok(())
}

fn validate_dag(lanes: &[JsonValue]) -> Result<(), Box<dyn std::error::Error>> {
    let mut indegree = BTreeMap::<String, usize>::new();
    let mut dependents = BTreeMap::<String, Vec<String>>::new();
    for lane in lanes {
        let id = lane["id"].as_str().unwrap().to_owned();
        let dependencies = lane["dependencies"].as_array().unwrap();
        indegree.insert(id.clone(), dependencies.len());
        for dependency in dependencies {
            dependents
                .entry(dependency.as_str().unwrap().to_owned())
                .or_default()
                .push(id.clone());
        }
    }
    let mut ready = indegree
        .iter()
        .filter_map(|(id, count)| (*count == 0).then_some(id.clone()))
        .collect::<VecDeque<_>>();
    let mut visited = 0;
    while let Some(id) = ready.pop_front() {
        visited += 1;
        for dependent in dependents.get(&id).into_iter().flatten() {
            let count = indegree.get_mut(dependent).unwrap();
            *count -= 1;
            if *count == 0 {
                ready.push_back(dependent.clone());
            }
        }
    }
    if visited != lanes.len() {
        return Err("CI lane dependency graph contains a cycle".into());
    }
    Ok(())
}

fn validate_plan_value(plan: &JsonValue) -> Result<(), Box<dyn std::error::Error>> {
    let object = exact_object(
        plan,
        &[
            "authority",
            "base_sha",
            "base_tree",
            "cache",
            "cargo_configuration",
            "cross_repo_dependencies",
            "execution",
            "executables",
            "head_sha",
            "head_tree",
            "lanes",
            "lockfiles",
            "profile",
            "publication",
            "repository",
            "repository_path",
            "required_check",
            "resources",
            "schema_version",
            "source_scope",
            "toolchains",
        ],
        "CI plan",
    )?;
    if object["schema_version"].as_str() != Some(PLAN_SCHEMA) {
        return Err("unsupported CI plan schema".into());
    }
    let repository = bounded_string(&object["repository"], "repository", MAX_CANONICAL_ID)?;
    if safe_id(repository) != repository {
        return Err("CI repository name is not canonical".into());
    }
    let repo_path = PathBuf::from(bounded_string(
        &object["repository_path"],
        "repository path",
        4096,
    )?);
    if !repo_path.is_absolute() {
        return Err("CI repository path must be absolute".into());
    }
    bounded_string(&object["required_check"], "required check", 256)?;
    for field in ["head_sha", "head_tree", "base_sha", "base_tree"] {
        require_lower_full_sha(bounded_string(&object[field], field, 40)?, field)?;
    }
    let authority = exact_object(
        &object["authority"],
        &[
            "lane_result_schema_sha256",
            "host_evidence_schema_sha256",
            "host_result_schema_sha256",
            "manifest_path",
            "manifest_sha256",
            "performance_schema_sha256",
            "plan_schema_sha256",
        ],
        "CI plan authority",
    )?;
    let authority_path = PathBuf::from(bounded_string(
        &authority["manifest_path"],
        "authority path",
        4096,
    )?);
    if !authority_path.is_absolute() {
        return Err("CI authority path must be absolute".into());
    }
    for field in [
        "manifest_sha256",
        "plan_schema_sha256",
        "lane_result_schema_sha256",
        "performance_schema_sha256",
        "host_result_schema_sha256",
        "host_evidence_schema_sha256",
    ] {
        require_sha256(bounded_string(&authority[field], field, 64)?, field)?;
    }
    let profile = exact_object(
        &object["profile"],
        &[
            "effective",
            "requested",
            "selected_by_uid",
            "widened",
            "widening_reasons",
        ],
        "CI profile",
    )?;
    let requested = Profile::parse(bounded_string(
        &profile["requested"],
        "requested profile",
        32,
    )?)?;
    let effective = Profile::parse(bounded_string(
        &profile["effective"],
        "effective profile",
        32,
    )?)?;
    if profile["selected_by_uid"].as_u64() != Some(0)
        || profile["widened"].as_bool() != Some(requested != effective)
    {
        return Err(
            "CI profile was not selected by the root broker or has inconsistent widening".into(),
        );
    }
    let reasons = bounded_string_array(&profile["widening_reasons"], "widening reasons", 16)?;
    if reasons.iter().collect::<BTreeSet<_>>().len() != reasons.len()
        || (requested != effective) == reasons.is_empty()
        || requested == Profile::ReleaseFull && !reasons.is_empty()
    {
        return Err("CI profile widening reasons are inconsistent".into());
    }
    let source_scope = exact_object(
        &object["source_scope"],
        &[
            "changed_paths",
            "contract_consumers",
            "node_packages",
            "rust_packages",
            "scope",
        ],
        "CI source scope",
    )?;
    let changed = bounded_string_array(
        &source_scope["changed_paths"],
        "changed paths",
        MAX_CHANGED_PATHS,
    )?;
    for path in &changed {
        validate_relative_path(path)?;
    }
    let contract_consumers = bounded_string_array(
        &source_scope["contract_consumers"],
        "contract consumers",
        128,
    )?;
    let node_packages =
        bounded_string_array(&source_scope["node_packages"], "node packages", 1024)?;
    let rust_packages =
        bounded_string_array(&source_scope["rust_packages"], "Rust packages", 1024)?;
    for (kind, values) in [
        ("changed paths", &changed),
        ("contract consumers", &contract_consumers),
        ("node packages", &node_packages),
        ("Rust packages", &rust_packages),
    ] {
        if values.iter().copied().collect::<BTreeSet<_>>().len() != values.len() {
            return Err(format!("CI source scope contains duplicate {kind}").into());
        }
    }
    let expected_scope = if effective == Profile::ReleaseFull {
        "complete"
    } else {
        "changed-surface"
    };
    if source_scope["scope"].as_str() != Some(expected_scope) {
        return Err("CI coverage scope differs from the effective profile".into());
    }
    let cross_repo_dependencies = bounded_string_array(
        &object["cross_repo_dependencies"],
        "cross-repository dependencies",
        128,
    )?;
    if cross_repo_dependencies != contract_consumers {
        return Err("CI contract-consumer scope differs from cross-repository authority".into());
    }
    validate_bound_file_array(&object["toolchains"], "toolchains")?;
    validate_bound_file_array(&object["lockfiles"], "lockfiles")?;
    validate_bound_file_array(&object["cargo_configuration"], "Cargo configuration")?;
    validate_executables(&object["executables"])?;
    let executable_paths = object["executables"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["path"].as_str().unwrap())
        .collect::<BTreeSet<_>>();
    let resources = exact_object(
        &object["resources"],
        &[
            "authority_build_jobs",
            "authority_fleet_concurrency",
            "build_jobs",
            "calibration",
            "fleet_concurrency",
            "host_network_namespace_device",
            "host_network_namespace_inode",
            "io_wait_percent_max_exclusive",
            "load1_max_exclusive",
            "max_parallel_lanes",
            "minimum_available_memory_bytes",
            "network",
            "swap_in_pages_max",
        ],
        "CI resources",
    )?;
    let max_parallel = resources["max_parallel_lanes"]
        .as_u64()
        .filter(|value| (1..=64).contains(value))
        .ok_or("max_parallel_lanes is not bounded")?;
    let build_jobs = resources["build_jobs"]
        .as_u64()
        .filter(|value| (1..=64).contains(value))
        .ok_or("build_jobs is not bounded")?;
    let fleet_concurrency = resources["fleet_concurrency"]
        .as_u64()
        .filter(|value| (1..=64).contains(value))
        .ok_or("fleet_concurrency is not bounded")?;
    let authority_build_jobs = resources["authority_build_jobs"]
        .as_u64()
        .filter(|value| (1..=64).contains(value))
        .ok_or("authority_build_jobs is not bounded")?;
    let authority_fleet_concurrency = resources["authority_fleet_concurrency"]
        .as_u64()
        .filter(|value| (1..=64).contains(value))
        .ok_or("authority_fleet_concurrency is not bounded")?;
    let calibration = resources["calibration"]
        .as_bool()
        .ok_or("calibration must be a boolean")?;
    if max_parallel != build_jobs.min(8)
        || (!calibration
            && (build_jobs != authority_build_jobs
                || fleet_concurrency != authority_fleet_concurrency))
        || (calibration
            && (![4, 8, 16, 24, 32].contains(&build_jobs)
                || ![2, 4, 6, 8].contains(&fleet_concurrency)))
        || resources["minimum_available_memory_bytes"].as_u64() != Some(MIN_AVAILABLE_MEMORY_BYTES)
        || resources["host_network_namespace_device"]
            .as_u64()
            .is_none()
        || resources["host_network_namespace_inode"]
            .as_u64()
            .filter(|inode| *inode > 0)
            .is_none()
        || resources["swap_in_pages_max"].as_u64() != Some(0)
        || resources["io_wait_percent_max_exclusive"].as_f64() != Some(15.0)
        || resources["load1_max_exclusive"].as_f64() != Some(96.0)
        || resources["network"].as_str() != Some("denied")
    {
        return Err("CI resource policy differs from the v1 authority".into());
    }
    let cache = exact_object(
        &object["cache"],
        &[
            "compiler_cache_forbidden_for_release",
            "enabled",
            "max_bytes",
            "mode",
            "root",
        ],
        "CI cache",
    )?;
    if cache["compiler_cache_forbidden_for_release"].as_bool() != Some(true) {
        return Err("CI cache policy must forbid release compiler caches".into());
    }
    match effective {
        Profile::ReleaseFull => {
            if cache["mode"].as_str() != Some("forbidden")
                || cache["enabled"].as_bool() != Some(false)
                || !cache["root"].is_null()
                || cache["max_bytes"].as_u64() != Some(0)
            {
                return Err("release-full plan admits a compiler cache".into());
            }
        }
        Profile::Presubmit => {
            if cache["mode"].as_str() != Some("content-addressed-local-only")
                || cache["max_bytes"].as_u64() != Some(SCCACHE_MAX_BYTES)
                || cache["root"].as_str() != Some("/var/cache/jain-ci/sccache")
                || cache["enabled"].as_bool().is_none()
            {
                return Err("presubmit cache policy is malformed".into());
            }
        }
    }
    let lanes = object["lanes"]
        .as_array()
        .ok_or("CI lanes must be an array")?;
    if lanes.iter().any(|lane| {
        lane["program"]
            .as_str()
            .is_none_or(|program| !executable_paths.contains(program))
    }) {
        return Err("CI lane invokes an executable absent from the bound toolchain set".into());
    }
    let changed_owned = changed
        .iter()
        .map(|path| (*path).to_owned())
        .collect::<Vec<_>>();
    let contract_consumers_owned = contract_consumers
        .iter()
        .map(|name| (*name).to_owned())
        .collect::<Vec<_>>();
    validate_generated_lanes(
        lanes,
        effective,
        !rust_packages.is_empty(),
        !node_packages.is_empty(),
        presubmit_requires_contract(&changed_owned, &contract_consumers_owned),
        !contract_consumers.is_empty(),
    )?;
    let execution = exact_object(
        &object["execution"],
        &[
            "activation_requires",
            "activation_state",
            "allowed",
            "required_boundary",
            "required_evidence_sealer",
            "required_fleet_scheduler",
            "required_measurement_source",
            "required_source_postcondition",
            "required_tool_manifest",
        ],
        "CI execution activation",
    )?;
    if execution["allowed"].as_bool() != Some(false)
        || execution["activation_state"].as_str() != Some("blocked")
        || execution["activation_requires"].as_str() != Some("release-full-equivalence")
        || execution["required_boundary"].as_str() != Some(EXECUTION_BOUNDARY)
        || execution["required_evidence_sealer"].as_str() != Some(EVIDENCE_SEALER)
        || execution["required_fleet_scheduler"].as_str() != Some(FLEET_SCHEDULER)
        || execution["required_measurement_source"].as_str() != Some(MEASUREMENT_SOURCE)
        || execution["required_source_postcondition"].as_str() != Some(SOURCE_POSTCONDITION)
        || execution["required_tool_manifest"].as_str() != Some(TOOL_MANIFEST)
    {
        return Err("typed CI v1 execution is not fail-closed".into());
    }
    let publication = exact_object(
        &object["publication"],
        &[
            "allowed",
            "mode",
            "required_check",
            "required_equivalence_profile",
        ],
        "CI publication",
    )?;
    if publication["mode"].as_str() != Some("shadow-equivalence")
        || publication["allowed"].as_bool() != Some(false)
        || publication["required_equivalence_profile"].as_str() != Some("release-full")
        || publication["required_check"] != object["required_check"]
    {
        return Err("optimized CI publication is enabled before equivalence".into());
    }
    Ok(())
}

fn exact_object<'a>(
    value: &'a JsonValue,
    keys: &[&str],
    kind: &str,
) -> Result<&'a Map<String, JsonValue>, Box<dyn std::error::Error>> {
    let object = value
        .as_object()
        .ok_or_else(|| format!("{kind} must be an object"))?;
    let actual = object.keys().map(String::as_str).collect::<BTreeSet<_>>();
    let expected = keys.iter().copied().collect::<BTreeSet<_>>();
    if actual != expected {
        return Err(format!("{kind} has missing or unknown fields").into());
    }
    Ok(object)
}

fn bounded_string<'a>(
    value: &'a JsonValue,
    kind: &str,
    max: usize,
) -> Result<&'a str, Box<dyn std::error::Error>> {
    value
        .as_str()
        .filter(|value| !value.is_empty() && value.len() <= max && !value.as_bytes().contains(&0))
        .ok_or_else(|| format!("{kind} must be a bounded non-empty string").into())
}

fn bounded_string_array<'a>(
    value: &'a JsonValue,
    kind: &str,
    max_items: usize,
) -> Result<Vec<&'a str>, Box<dyn std::error::Error>> {
    let array = value
        .as_array()
        .filter(|array| array.len() <= max_items)
        .ok_or_else(|| format!("{kind} must be a bounded array"))?;
    array
        .iter()
        .map(|value| bounded_string(value, kind, MAX_STRING))
        .collect()
}

fn require_sha256(value: &str, kind: &str) -> Result<(), Box<dyn std::error::Error>> {
    if value.len() != 64
        || !value.bytes().all(|byte| byte.is_ascii_hexdigit())
        || value.bytes().any(|byte| byte.is_ascii_uppercase())
    {
        return Err(format!("{kind} must be a lowercase SHA-256").into());
    }
    Ok(())
}

fn validate_bound_file_array(
    value: &JsonValue,
    kind: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let array = value
        .as_array()
        .filter(|array| array.len() <= 4096)
        .ok_or_else(|| format!("{kind} must be a bounded array"))?;
    let mut paths = BTreeSet::new();
    for entry in array {
        let entry = exact_object(entry, &["bytes", "path", "sha256"], kind)?;
        let path = bounded_string(&entry["path"], kind, 4096)?;
        validate_relative_path(path)?;
        if !paths.insert(path) {
            return Err(format!("{kind} contains duplicate paths").into());
        }
        require_sha256(bounded_string(&entry["sha256"], kind, 64)?, kind)?;
        if entry["bytes"]
            .as_u64()
            .filter(|bytes| *bytes <= MAX_EVIDENCE_BYTES)
            .is_none()
        {
            return Err(format!("{kind} byte count is invalid").into());
        }
    }
    Ok(())
}

fn validate_executables(value: &JsonValue) -> Result<(), Box<dyn std::error::Error>> {
    let array = value
        .as_array()
        .filter(|array| !array.is_empty() && array.len() <= 16)
        .ok_or("executables must be a bounded non-empty array")?;
    let mut names = BTreeSet::new();
    let mut paths = BTreeSet::new();
    for executable in array {
        let executable = exact_object(executable, &["name", "path", "sha256"], "CI executable")?;
        let name = bounded_string(&executable["name"], "executable name", 64)?;
        let path = bounded_string(&executable["path"], "executable path", 4096)?;
        if safe_id(name) != name
            || !Path::new(path).is_absolute()
            || !names.insert(name)
            || !paths.insert(path)
        {
            return Err("CI executable identity is unsafe or duplicated".into());
        }
        require_sha256(
            bounded_string(&executable["sha256"], "executable digest", 64)?,
            "executable digest",
        )?;
    }
    for required in [
        "bash",
        "cargo",
        "clippy-driver",
        "rustc",
        "rustdoc",
        "rustfmt",
        "time",
    ] {
        if !names.contains(required) {
            return Err(format!("CI plan omits bound executable {required}").into());
        }
    }
    Ok(())
}

#[derive(Clone, Debug)]
struct LaneSpec {
    id: String,
    kind: String,
    program: PathBuf,
    args: Vec<String>,
    dependencies: Vec<String>,
    obligations: Vec<String>,
    timeout_seconds: u64,
}

#[derive(Clone, Debug)]
struct RunContext {
    repo: PathBuf,
    lane_root: PathBuf,
    cargo_home: PathBuf,
    target_dir: PathBuf,
    command_path: String,
    writable_root: Option<PathBuf>,
    profile: Profile,
    plan_sha256: String,
    build_jobs: u64,
    sccache_path: Option<PathBuf>,
}

pub(crate) fn run_command(args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    let mut plan_path = None;
    let mut receipt = None;
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--plan" => plan_path = Some(PathBuf::from(iter.next().ok_or("--plan needs a path")?)),
            "--receipt" => {
                receipt = Some(PathBuf::from(iter.next().ok_or("--receipt needs a path")?))
            }
            value => return Err(format!("unknown ci-run argument: {value}").into()),
        }
    }
    if unsafe { libc::geteuid() } == 0 {
        return Err("ci-run refuses to execute product commands as root".into());
    }
    let plan_path = plan_path.ok_or("ci-run requires --plan")?;
    let receipt = receipt.ok_or("ci-run requires --receipt")?;
    let plan_bytes = read_root_owned_plan(&plan_path)?;
    let plan_sha256 = format!("{:x}", Sha256::digest(&plan_bytes));
    let plan: JsonValue = serde_json::from_slice(&plan_bytes)?;
    validate_plan_value(&plan)?;
    if plan["execution"]["allowed"].as_bool() != Some(true) {
        return Err("ci-run is intentionally inactive until the protected root systemd boundary, broker sealer, fleet lease, continuous cgroup sampler, tool manifest, post-run source proof, and release-full equivalence are installed".into());
    }
    if !network_namespace_isolated(
        plan["resources"]["host_network_namespace_device"]
            .as_u64()
            .unwrap(),
        plan["resources"]["host_network_namespace_inode"]
            .as_u64()
            .unwrap(),
    )? {
        return Err(
            "ci-run requires a network-empty worker namespace distinct from the broker".into(),
        );
    }
    execute_plan(&plan, &plan_sha256, &receipt, true)?;
    println!("wrote {}", receipt.display());
    Ok(())
}

fn read_root_owned_plan(path: &Path) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let path = physical_regular_path(path, "CI plan")?;
    const O_NONBLOCK: i32 = 0o4000;
    const O_NOFOLLOW: i32 = 0o400000;
    let mut file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(O_NONBLOCK | O_NOFOLLOW)
        .open(&path)?;
    let before = file.metadata()?;
    if before.uid() != 0
        || before.gid() != 0
        || before.nlink() != 1
        || before.mode() & 0o022 != 0
        || before.len() == 0
        || before.len() > MAX_PLAN_BYTES
    {
        return Err("CI plan must be root-owned, single-link, immutable, and bounded".into());
    }
    let mut bytes = Vec::with_capacity(before.len() as usize);
    (&mut file)
        .take(MAX_PLAN_BYTES + 1)
        .read_to_end(&mut bytes)?;
    let after = file.metadata()?;
    let path_after = fs::symlink_metadata(&path)?;
    if bytes.len() as u64 != before.len()
        || !same_metadata(&before, &after)
        || !same_metadata(&before, &path_after)
    {
        return Err("CI plan changed while being read".into());
    }
    Ok(bytes)
}

fn same_metadata(left: &fs::Metadata, right: &fs::Metadata) -> bool {
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

fn network_namespace_isolated(
    broker_device: u64,
    broker_inode: u64,
) -> Result<bool, Box<dyn std::error::Error>> {
    let current = fs::metadata("/proc/self/ns/net")?;
    if current.dev() == broker_device && current.ino() == broker_inode {
        return Ok(false);
    }
    let mut interfaces = fs::read_dir("/sys/class/net")?
        .map(|entry| entry.map(|entry| entry.file_name().to_string_lossy().into_owned()))
        .collect::<Result<Vec<_>, _>>()?;
    interfaces.sort();
    if interfaces != ["lo"] {
        return Ok(false);
    }
    let loopback_flags = fs::read_to_string("/sys/class/net/lo/flags")?;
    let loopback_flags = u32::from_str_radix(loopback_flags.trim_start_matches("0x").trim(), 16)?;
    if loopback_flags & 1 != 0 {
        return Ok(false);
    }
    let ipv4_routes = fs::read_to_string("/proc/net/route")?;
    if ipv4_routes
        .lines()
        .skip(1)
        .any(|line| !line.trim().is_empty())
    {
        return Ok(false);
    }
    let ipv6_routes = fs::read_to_string("/proc/net/ipv6_route")?;
    Ok(!ipv6_routes.lines().any(|line| {
        line.split_whitespace()
            .next_back()
            .is_some_and(|interface| interface != "lo")
    }))
}

#[derive(Debug)]
struct TemporaryCheckout {
    path: PathBuf,
    writable_root: PathBuf,
}

impl Drop for TemporaryCheckout {
    fn drop(&mut self) {
        let safe_name = self
            .path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("typed-ci-checkout-"));
        if safe_name && self.path.parent() == Some(self.writable_root.as_path()) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

fn worker_writable_root() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let path = PathBuf::from(
        env::var_os("JAIN_HOST_CI_WRITABLE_ROOT")
            .ok_or("ci-run requires the root-broker writable-root binding")?,
    );
    if !path.is_absolute() {
        return Err("root-broker writable root must be absolute".into());
    }
    let canonical = fs::canonicalize(&path)?;
    if canonical != path {
        return Err("root-broker writable root must be canonical".into());
    }
    let metadata = fs::symlink_metadata(&path)?;
    if !metadata.file_type().is_dir()
        || metadata.file_type().is_symlink()
        || metadata.uid() != unsafe { libc::geteuid() }
        || metadata.mode() & 0o002 != 0
    {
        return Err("root-broker writable root has unsafe custody or mode".into());
    }
    Ok(canonical)
}

fn validate_worker_cargo_home(path: &Path) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let canonical = fs::canonicalize(path)?;
    if canonical != path {
        return Err("worker Cargo home must be canonical".into());
    }
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.file_type().is_dir()
        || metadata.file_type().is_symlink()
        || metadata.uid() != unsafe { libc::geteuid() }
        || metadata.mode() & 0o002 != 0
    {
        return Err("worker Cargo home has unsafe custody or mode".into());
    }
    for credential in ["credentials", "credentials.toml"] {
        if path.join(credential).exists() {
            return Err("worker Cargo home contains ambient registry credentials".into());
        }
    }
    Ok(canonical)
}

fn materialize_temporary_checkout(
    source: &Path,
    writable_root: &Path,
    head: &str,
    expected_tree: &str,
    canonical_remote: &str,
    plan_sha256: &str,
) -> Result<TemporaryCheckout, Box<dyn std::error::Error>> {
    let destination = writable_root.join(format!("typed-ci-checkout-{}", &plan_sha256[..16]));
    if destination.exists() {
        return Err("typed CI standalone checkout path already exists".into());
    }
    let checkout = TemporaryCheckout {
        path: destination,
        writable_root: writable_root.to_path_buf(),
    };
    let output = Command::new("/usr/bin/git")
        .env_clear()
        .env("LC_ALL", "C")
        .env("HOME", "/var/empty")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_TERMINAL_PROMPT", "0")
        .args(["clone", "--no-local", "--no-checkout", "--quiet", "--"])
        .arg(source)
        .arg(&checkout.path)
        .output()?;
    if !output.status.success() {
        return Err(format!(
            "cannot create the standalone typed CI checkout: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    for args in [
        vec!["checkout", "--quiet", "--detach", head],
        vec!["remote", "set-url", "origin", canonical_remote],
    ] {
        let output = git_command(&checkout.path).args(args).output()?;
        if !output.status.success() {
            return Err(format!(
                "cannot finalize the standalone typed CI checkout: {}",
                String::from_utf8_lossy(&output.stderr)
            )
            .into());
        }
    }
    physical_checkout(&checkout.path)?;
    if git_commit(&checkout.path, "HEAD")? != head
        || git_text(&checkout.path, &["rev-parse", "--verify", "HEAD^{tree}"])? != expected_tree
        || !git_text(
            &checkout.path,
            &["status", "--porcelain=v1", "--untracked-files=all"],
        )?
        .is_empty()
    {
        return Err("standalone typed CI checkout differs from the sealed plan".into());
    }
    Ok(checkout)
}

fn execute_plan(
    plan: &JsonValue,
    plan_sha256: &str,
    receipt: &Path,
    enforce_runtime_boundary: bool,
) -> Result<JsonValue, Box<dyn std::error::Error>> {
    require_sha256(plan_sha256, "plan digest")?;
    if enforce_runtime_boundary {
        return Err(
            "unsealed worker prototype cannot cross the protected root-broker boundary".into(),
        );
    }
    let canonical_remote = revalidate_plan_inputs(plan)?;
    let writable_root = if enforce_runtime_boundary {
        Some(worker_writable_root()?)
    } else {
        None
    };
    let receipt_parent = prepare_receipt_parent(receipt, writable_root.as_deref())?;
    let evidence_root = receipt_parent.join(".ci-evidence");
    if evidence_root.exists() {
        return Err("CI evidence directory already exists".into());
    }
    fs::create_dir(&evidence_root)?;
    fs::set_permissions(&evidence_root, fs::Permissions::from_mode(0o700))?;
    let lane_root = evidence_root.join("lanes");
    if lane_root.exists() {
        return Err("CI lane evidence directory already exists".into());
    }
    fs::create_dir(&lane_root)?;
    fs::set_permissions(&lane_root, fs::Permissions::from_mode(0o700))?;
    let profile = Profile::parse(plan["profile"]["effective"].as_str().unwrap())?;
    let cache_enabled = plan["cache"]["enabled"].as_bool().unwrap();
    let executable_map = plan["executables"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| {
            (
                entry["name"].as_str().unwrap().to_owned(),
                PathBuf::from(entry["path"].as_str().unwrap()),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let sccache_path = executable_map.get("sccache").cloned();
    let cargo_directory = executable_map
        .get("cargo")
        .and_then(|path| path.parent())
        .ok_or("bound Cargo executable has no parent directory")?;
    let command_path = format!("{}:/usr/local/bin:/usr/bin:/bin", cargo_directory.display());
    if profile == Profile::Presubmit && (!cache_enabled || sccache_path.is_none()) {
        return Err(
            "presubmit execution requires the reviewed root-installed sccache boundary".into(),
        );
    }
    if profile == Profile::ReleaseFull && (cache_enabled || sccache_path.is_some()) {
        return Err("release-full execution admitted a compiler cache".into());
    }
    let checkout = writable_root
        .as_ref()
        .map(|writable_root| {
            materialize_temporary_checkout(
                Path::new(plan["repository_path"].as_str().unwrap()),
                writable_root,
                plan["head_sha"].as_str().unwrap(),
                plan["head_tree"].as_str().unwrap(),
                &canonical_remote,
                plan_sha256,
            )
        })
        .transpose()?;
    let cargo_home = if let Some(writable_root) = &writable_root {
        validate_worker_cargo_home(&writable_root.join("cargo-home"))?
    } else {
        receipt_parent.join("cargo-home")
    };
    let target_dir = receipt_parent.join("cargo-target");
    if writable_root.is_none() {
        if cargo_home.exists() {
            return Err(format!(
                "fresh CI directory already exists: {}",
                cargo_home.display()
            )
            .into());
        }
        fs::create_dir(&cargo_home)?;
        fs::set_permissions(&cargo_home, fs::Permissions::from_mode(0o700))?;
    }
    if target_dir.exists() {
        return Err(format!(
            "fresh CI directory already exists: {}",
            target_dir.display()
        )
        .into());
    }
    fs::create_dir(&target_dir)?;
    fs::set_permissions(&target_dir, fs::Permissions::from_mode(0o700))?;
    let context = RunContext {
        repo: checkout.as_ref().map_or_else(
            || PathBuf::from(plan["repository_path"].as_str().unwrap()),
            |checkout| checkout.path.clone(),
        ),
        lane_root,
        cargo_home,
        target_dir,
        command_path,
        writable_root: writable_root.clone(),
        profile,
        plan_sha256: plan_sha256.to_owned(),
        build_jobs: plan["resources"]["build_jobs"].as_u64().unwrap(),
        sccache_path,
    };
    let lanes = parsed_lanes(plan)?;
    let max_parallel = plan["resources"]["max_parallel_lanes"].as_u64().unwrap() as usize;
    let system_before = SystemSnapshot::read()?;
    let started_at_unix_ms = unix_ms()?;
    let started = Instant::now();
    let cache_before = cache_stats(&context)?;
    let results = run_dag(&lanes, &context, max_parallel)?;
    let cache_after = cache_stats(&context)?;
    let system_after = SystemSnapshot::read()?;
    let finished_at_unix_ms = unix_ms()?;
    let duration_ms = started.elapsed().as_millis() as u64;
    let result_values = lanes
        .iter()
        .map(|lane| {
            results
                .get(&lane.id)
                .cloned()
                .ok_or_else(|| format!("missing result for lane {}", lane.id))
        })
        .collect::<Result<Vec<_>, _>>()?;
    for result in &result_values {
        validate_lane_result(result, plan_sha256)?;
    }
    let status = if result_values
        .iter()
        .all(|result| result["status"].as_str() == Some("pass"))
    {
        "pass"
    } else {
        "fail"
    };
    let metrics = aggregate_metrics(
        &result_values,
        &lanes,
        &system_before,
        &system_after,
        cache_before.as_ref(),
        cache_after.as_ref(),
    )?;
    let result_set_sha256 = format!("{:x}", Sha256::digest(serde_json::to_vec(&result_values)?));
    let metrics_sha256 = format!("{:x}", Sha256::digest(serde_json::to_vec(&metrics)?));
    let host_evidence = json!({
        "schema_version": HOST_EVIDENCE_SCHEMA,
        "plan_sha256": plan_sha256,
        "lane_result_schema_version": LANE_RESULT_SCHEMA,
        "result_set_sha256": result_set_sha256,
        "metrics_sha256": metrics_sha256,
        "profile": profile.as_str(),
        "measurement_source": MEASUREMENT_SOURCE,
        "publication_allowed": false
    });
    validate_host_evidence(
        &host_evidence,
        plan_sha256,
        &result_set_sha256,
        &metrics_sha256,
        profile,
    )?;
    let configuration = json!({
        "build_jobs": plan["resources"]["build_jobs"],
        "fleet_concurrency": plan["resources"]["fleet_concurrency"],
        "calibration": plan["resources"]["calibration"]
    });
    let run = json!({
        "schema_version": RUN_SCHEMA,
        "plan_schema_version": PLAN_SCHEMA,
        "plan_sha256": plan_sha256,
        "repository": plan["repository"],
        "head_sha": plan["head_sha"],
        "head_tree": plan["head_tree"],
        "base_sha": plan["base_sha"],
        "authority_sha256": plan["authority"]["manifest_sha256"],
        "configuration": configuration,
        "profile": profile.as_str(),
        "started_at_unix_ms": started_at_unix_ms,
        "finished_at_unix_ms": finished_at_unix_ms,
        "duration_ms": duration_ms,
        "status": status,
        "publication_allowed": false,
        "publication_mode": "shadow-equivalence",
        "lane_results": result_values,
        "result_set_sha256": result_set_sha256,
        "metrics": metrics,
        "host_evidence": host_evidence,
        "sealed": true
    });
    validate_run_receipt(&run)?;
    if receipt.exists() {
        return Err("CI receipt already exists".into());
    }
    write_json_receipt(receipt, &run)?;
    if status == "fail" {
        return Err("one or more typed CI lanes failed or were canceled".into());
    }
    Ok(run)
}

fn prepare_receipt_parent(
    receipt: &Path,
    writable_root: Option<&Path>,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    if !receipt.is_absolute() {
        return Err("CI receipt path must be absolute".into());
    }
    if receipt.exists() {
        return Err("CI receipt is create-only".into());
    }
    let parent = receipt.parent().ok_or("CI receipt has no parent")?;
    if writable_root.is_some_and(|root| parent != root && !parent.starts_with(root)) {
        return Err("CI receipt must remain inside the root-broker writable boundary".into());
    }
    if !parent.exists() {
        if writable_root.is_some() {
            return Err("root broker must pre-create the CI receipt directory".into());
        }
        fs::create_dir_all(parent)?;
    }
    let canonical = fs::canonicalize(parent)?;
    if canonical != parent {
        return Err("CI receipt parent must be canonical".into());
    }
    if writable_root.is_some_and(|root| canonical != root && !canonical.starts_with(root)) {
        return Err("CI receipt must remain inside the root-broker writable boundary".into());
    }
    let metadata = fs::symlink_metadata(parent)?;
    if !metadata.file_type().is_dir() || metadata.file_type().is_symlink() {
        return Err("CI receipt parent must be a physical directory".into());
    }
    Ok(canonical)
}

fn revalidate_plan_inputs(plan: &JsonValue) -> Result<String, Box<dyn std::error::Error>> {
    let repo = physical_checkout(Path::new(plan["repository_path"].as_str().unwrap()))?;
    if git_commit(&repo, "HEAD")? != plan["head_sha"].as_str().unwrap()
        || git_text(&repo, &["rev-parse", "--verify", "HEAD^{tree}"])?
            != plan["head_tree"].as_str().unwrap()
        || git_commit(&repo, "refs/remotes/origin/main")? != plan["base_sha"].as_str().unwrap()
        || !git_text(
            &repo,
            &["status", "--porcelain=v1", "--untracked-files=all"],
        )?
        .is_empty()
    {
        return Err("CI checkout moved or became dirty after planning".into());
    }
    if !git_success(
        &repo,
        &[
            "merge-base",
            "--is-ancestor",
            plan["base_sha"].as_str().unwrap(),
            plan["head_sha"].as_str().unwrap(),
        ],
    )? {
        return Err("CI base ancestry changed after planning".into());
    }
    let manifest = physical_regular_path(
        Path::new(plan["authority"]["manifest_path"].as_str().unwrap()),
        "CI authority manifest",
    )?;
    if manifest_sha256(&manifest)? != plan["authority"]["manifest_sha256"].as_str().unwrap() {
        return Err("CI authority manifest changed after planning".into());
    }
    let authority: toml::Value = fs::read_to_string(&manifest)?.parse()?;
    let managed = managed_repositories(&authority, &manifest)?;
    let matches = managed
        .iter()
        .filter(|candidate| candidate.name == plan["repository"].as_str().unwrap())
        .collect::<Vec<_>>();
    if matches.len() != 1
        || matches[0].path != repo
        || matches[0].inventory_status != "active"
        || git_lines(&repo, &["remote"])? != ["origin"]
        || git_text(&repo, &["remote", "get-url", "origin"])? != matches[0].remote
    {
        return Err("CI repository identity or canonical remote changed after planning".into());
    }
    let canonical_remote = matches[0].remote.clone();
    let root = control_plane_root();
    for (field, path) in [
        (
            "plan_schema_sha256",
            root.join("contracts/ci-plan.schema.json"),
        ),
        (
            "lane_result_schema_sha256",
            root.join("contracts/ci-lane-result.schema.json"),
        ),
        (
            "performance_schema_sha256",
            root.join("contracts/ci-performance.schema.json"),
        ),
        (
            "host_result_schema_sha256",
            root.join("contracts/host-ci-result.schema.json"),
        ),
        (
            "host_evidence_schema_sha256",
            root.join("contracts/host-ci-evidence.schema.json"),
        ),
    ] {
        if sha256_regular_file(&path, "CI schema")? != plan["authority"][field].as_str().unwrap() {
            return Err(format!("CI schema changed after planning: {field}").into());
        }
    }
    let tracked = git_nul_paths(
        &repo,
        &[
            "ls-tree",
            "-r",
            "--name-only",
            "-z",
            plan["head_sha"].as_str().unwrap(),
        ],
    )?;
    if json!(bound_inputs(
        &repo,
        plan["head_sha"].as_str().unwrap(),
        &tracked,
        is_lockfile
    )?) != plan["lockfiles"]
        || json!(bound_inputs(
            &repo,
            plan["head_sha"].as_str().unwrap(),
            &tracked,
            is_toolchain_file,
        )?) != plan["toolchains"]
    {
        return Err("CI lockfile or toolchain binding changed after planning".into());
    }
    let changed = git_nul_paths(
        &repo,
        &[
            "diff",
            "--name-only",
            "-z",
            plan["base_sha"].as_str().unwrap(),
            plan["head_sha"].as_str().unwrap(),
        ],
    )?;
    if json!(changed) != plan["source_scope"]["changed_paths"] {
        return Err("CI changed-surface binding drifted after planning".into());
    }
    for executable in plan["executables"].as_array().unwrap() {
        let path = Path::new(executable["path"].as_str().unwrap());
        let canonical = physical_executable(path, "bound CI executable")?;
        if sha256_regular_file(&canonical, "bound CI executable")?
            != executable["sha256"].as_str().unwrap()
        {
            return Err(format!("bound CI executable changed: {}", path.display()).into());
        }
    }
    Ok(canonical_remote)
}

fn parsed_lanes(plan: &JsonValue) -> Result<Vec<LaneSpec>, Box<dyn std::error::Error>> {
    plan["lanes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|lane| {
            Ok(LaneSpec {
                id: lane["id"].as_str().unwrap().to_owned(),
                kind: lane["kind"].as_str().unwrap().to_owned(),
                program: PathBuf::from(lane["program"].as_str().unwrap()),
                args: lane["args"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|arg| arg.as_str().unwrap().to_owned())
                    .collect(),
                dependencies: lane["dependencies"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|dependency| dependency.as_str().unwrap().to_owned())
                    .collect(),
                obligations: lane["obligations"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|obligation| obligation.as_str().unwrap().to_owned())
                    .collect(),
                timeout_seconds: lane["timeout_seconds"].as_u64().unwrap(),
            })
        })
        .collect()
}

fn run_dag(
    lanes: &[LaneSpec],
    context: &RunContext,
    max_parallel: usize,
) -> Result<BTreeMap<String, JsonValue>, Box<dyn std::error::Error>> {
    let (sender, receiver) = mpsc::channel::<(String, Result<JsonValue, String>)>();
    let mut pending = lanes
        .iter()
        .map(|lane| (lane.id.clone(), lane.clone()))
        .collect::<BTreeMap<_, _>>();
    let mut running = BTreeSet::new();
    let mut results = BTreeMap::<String, JsonValue>::new();
    let mut evidence_errors = Vec::new();
    while results.len() < lanes.len() {
        let pending_ids = pending.keys().cloned().collect::<Vec<_>>();
        let mut progressed = false;
        for id in pending_ids {
            if running.len() >= max_parallel {
                break;
            }
            let lane = pending.get(&id).unwrap();
            if lane
                .dependencies
                .iter()
                .any(|dependency| !results.contains_key(dependency))
            {
                continue;
            }
            if lane
                .dependencies
                .iter()
                .any(|dependency| results[dependency]["status"].as_str() != Some("pass"))
            {
                let lane = pending.remove(&id).unwrap();
                let result = canceled_lane_result(&lane, context, "dependency-failed")?;
                results.insert(id, result);
                progressed = true;
                continue;
            }
            let lane = pending.remove(&id).unwrap();
            running.insert(id.clone());
            let sender = sender.clone();
            let thread_id = id.clone();
            let context = context.clone();
            thread::spawn(move || {
                let outcome = run_lane(&lane, &context).map_err(|error| error.to_string());
                let _ = sender.send((thread_id, outcome));
            });
            progressed = true;
        }
        if !running.is_empty() {
            let (id, outcome) = receiver.recv()?;
            running.remove(&id);
            match outcome {
                Ok(result) => {
                    results.insert(id, result);
                }
                Err(error) => {
                    evidence_errors.push(format!("lane {id} evidence failed: {error}"));
                    results.insert(id, json!({"status": "fail"}));
                }
            }
            continue;
        }
        if !progressed && !pending.is_empty() {
            return Err("CI DAG scheduler made no progress".into());
        }
    }
    if !evidence_errors.is_empty() {
        evidence_errors.sort();
        return Err(evidence_errors.join("\n").into());
    }
    Ok(results)
}

fn run_lane(
    lane: &LaneSpec,
    context: &RunContext,
) -> Result<JsonValue, Box<dyn std::error::Error>> {
    let log_path = context.lane_root.join(format!("{}.log", lane.id));
    let metrics_path = context.lane_root.join(format!("{}.time", lane.id));
    let result_path = context.lane_root.join(format!("{}.json", lane.id));
    for path in [&log_path, &metrics_path, &result_path] {
        if path.exists() {
            return Err(format!("lane output already exists: {}", path.display()).into());
        }
    }
    let log = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&log_path)?;
    let stderr = log.try_clone()?;
    let started_at_unix_ms = unix_ms()?;
    let started = Instant::now();
    let format = "wall_seconds=%e\nuser_seconds=%U\nsystem_seconds=%S\npeak_rss_kib=%M\nfs_inputs=%I\nfs_outputs=%O";
    let mut command = Command::new("/usr/bin/time");
    command
        .env_clear()
        .current_dir(&context.repo)
        .args(["-f", format, "-o"])
        .arg(&metrics_path)
        .arg("--")
        .arg(&lane.program)
        .args(&lane.args)
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(stderr))
        .env("LC_ALL", "C")
        .env("PATH", &context.command_path)
        .env("HOME", "/var/empty")
        .env("RUSTUP_HOME", "/home/ubuntu/.rustup")
        .env("CARGO_HOME", &context.cargo_home)
        .env("CARGO_TARGET_DIR", &context.target_dir)
        .env("CARGO_BUILD_JOBS", context.build_jobs.to_string())
        .env("JAIN_CI_JOBS", context.build_jobs.to_string())
        .env("CMAKE_BUILD_PARALLEL_LEVEL", context.build_jobs.to_string())
        .env("RAYON_NUM_THREADS", context.build_jobs.to_string())
        .env("MAKEFLAGS", format!("-j{}", context.build_jobs))
        .env("CARGO_NET_OFFLINE", "true")
        .env("CARGO_REGISTRIES_CRATES_IO_PROTOCOL", "sparse")
        .env("JAIN_TYPED_CI_PROFILE", context.profile.as_str());
    if let Some(writable_root) = &context.writable_root {
        command
            .env("JAIN_HOST_CI_WRITABLE_ROOT", writable_root)
            .env("JAIN_HOST_CI_NETWORK_ISOLATED", "1");
    }
    if context.profile == Profile::ReleaseFull {
        command.env("JAIN_RELEASE_CI", "1");
    }
    match context.profile {
        Profile::Presubmit => {
            let sccache = context
                .sccache_path
                .as_ref()
                .ok_or("presubmit lane has no bound sccache")?;
            command
                .env("RUSTC_WRAPPER", sccache)
                .env("SCCACHE_DIR", "/var/cache/jain-ci/sccache")
                .env("SCCACHE_CACHE_SIZE", "40G");
        }
        Profile::ReleaseFull => {
            command
                .env_remove("RUSTC_WRAPPER")
                .env_remove("SCCACHE_DIR")
                .env_remove("SCCACHE_CACHE_SIZE");
        }
    }
    unsafe {
        command.pre_exec(|| {
            if libc::setpgid(0, 0) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut child = command.spawn()?;
    let deadline = Instant::now() + Duration::from_secs(lane.timeout_seconds);
    let (exit_code, reason) = loop {
        if let Some(status) = child.try_wait()? {
            break (
                status
                    .code()
                    .and_then(|code| u8::try_from(code).ok())
                    .map(u64::from),
                if status.success() {
                    None
                } else {
                    Some("command-failed")
                },
            );
        }
        if Instant::now() >= deadline {
            unsafe {
                libc::kill(-(child.id() as i32), libc::SIGKILL);
            }
            let _ = child.wait();
            break (None, Some("timeout"));
        }
        thread::sleep(Duration::from_millis(100));
    };
    let finished_at_unix_ms = unix_ms()?;
    let duration_ms = started.elapsed().as_millis() as u64;
    let time_metrics = match parse_time_metrics(&metrics_path, duration_ms) {
        Ok(metrics) => metrics,
        Err(_) if reason == Some("timeout") => zero_time_metrics(duration_ms),
        Err(error) => return Err(error),
    };
    let log_sha256 = sha256_regular_file(&log_path, "CI lane log")?;
    let command_sha256 = command_digest(lane)?;
    let status = if exit_code == Some(0) && reason.is_none() {
        "pass"
    } else {
        "fail"
    };
    let result = json!({
        "schema_version": LANE_RESULT_SCHEMA,
        "plan_sha256": context.plan_sha256,
        "lane_id": lane.id,
        "kind": lane.kind,
        "status": status,
        "reason": reason,
        "started_at_unix_ms": started_at_unix_ms,
        "finished_at_unix_ms": finished_at_unix_ms,
        "duration_ms": duration_ms,
        "exit_code": exit_code,
        "log_path": log_path,
        "log_sha256": log_sha256,
        "command_sha256": command_sha256,
        "dependencies": lane.dependencies,
        "obligations": lane.obligations,
        "metrics": time_metrics
    });
    validate_lane_result(&result, &context.plan_sha256)?;
    write_json_receipt(&result_path, &result)?;
    Ok(result)
}

fn canceled_lane_result(
    lane: &LaneSpec,
    context: &RunContext,
    reason: &str,
) -> Result<JsonValue, Box<dyn std::error::Error>> {
    let log_path = context.lane_root.join(format!("{}.log", lane.id));
    let result_path = context.lane_root.join(format!("{}.json", lane.id));
    if log_path.exists() || result_path.exists() {
        return Err("canceled lane output already exists".into());
    }
    let timestamp = unix_ms()?;
    fs::write(&log_path, format!("lane {} canceled: {reason}\n", lane.id))?;
    fs::set_permissions(&log_path, fs::Permissions::from_mode(0o600))?;
    let result = json!({
        "schema_version": LANE_RESULT_SCHEMA,
        "plan_sha256": context.plan_sha256,
        "lane_id": lane.id,
        "kind": lane.kind,
        "status": "canceled",
        "reason": reason,
        "started_at_unix_ms": timestamp,
        "finished_at_unix_ms": timestamp,
        "duration_ms": 0,
        "exit_code": null,
        "log_path": log_path,
        "log_sha256": sha256_regular_file(&log_path, "canceled CI lane log")?,
        "command_sha256": command_digest(lane)?,
        "dependencies": lane.dependencies,
        "obligations": lane.obligations,
        "metrics": {
            "wall_ms": 0,
            "cpu_user_ms": 0,
            "cpu_system_ms": 0,
            "peak_rss_bytes": 0,
            "fs_inputs": 0,
            "fs_outputs": 0
        }
    });
    validate_lane_result(&result, &context.plan_sha256)?;
    write_json_receipt(&result_path, &result)?;
    Ok(result)
}

fn command_digest(lane: &LaneSpec) -> Result<String, Box<dyn std::error::Error>> {
    Ok(format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&json!({
            "program": lane.program,
            "args": lane.args,
            "environment": {},
            "timeout_seconds": lane.timeout_seconds
        }))?)
    ))
}

fn parse_time_metrics(
    path: &Path,
    fallback_wall_ms: u64,
) -> Result<JsonValue, Box<dyn std::error::Error>> {
    let text = fs::read_to_string(path)?;
    if text.len() > 4096 {
        return Err("GNU time metrics exceed the bounded parser limit".into());
    }
    let mut values = BTreeMap::new();
    let mut status_line_seen = false;
    for line in text.lines() {
        let Some((key, value)) = line.split_once('=') else {
            if !status_line_seen
                && (line.starts_with("Command exited with non-zero status ")
                    || line.starts_with("Command terminated by signal "))
            {
                status_line_seen = true;
                continue;
            }
            return Err("malformed GNU time metric".into());
        };
        if values.insert(key, value).is_some() {
            return Err("duplicate GNU time metric".into());
        }
    }
    if values.keys().copied().collect::<BTreeSet<_>>()
        != [
            "fs_inputs",
            "fs_outputs",
            "peak_rss_kib",
            "system_seconds",
            "user_seconds",
            "wall_seconds",
        ]
        .into_iter()
        .collect()
    {
        return Err("GNU time metrics have missing or unknown fields".into());
    }
    let parse_seconds = |key: &str| -> Result<u64, Box<dyn std::error::Error>> {
        let value = values[key].parse::<f64>()?;
        if !value.is_finite() || !(0.0..=3600.0).contains(&value) {
            return Err(format!("invalid timing value for {key}").into());
        }
        Ok((value * 1000.0).round() as u64)
    };
    let parse_integer =
        |key: &str| -> Result<u64, Box<dyn std::error::Error>> { Ok(values[key].parse::<u64>()?) };
    Ok(json!({
        "wall_ms": parse_seconds("wall_seconds")?.max(fallback_wall_ms),
        "cpu_user_ms": parse_seconds("user_seconds")?,
        "cpu_system_ms": parse_seconds("system_seconds")?,
        "peak_rss_bytes": parse_integer("peak_rss_kib")?.checked_mul(1024).ok_or("peak RSS overflow")?,
        "fs_inputs": parse_integer("fs_inputs")?,
        "fs_outputs": parse_integer("fs_outputs")?
    }))
}

fn zero_time_metrics(wall_ms: u64) -> JsonValue {
    json!({
        "wall_ms": wall_ms,
        "cpu_user_ms": 0,
        "cpu_system_ms": 0,
        "peak_rss_bytes": 0,
        "fs_inputs": 0,
        "fs_outputs": 0
    })
}

fn validate_lane_result(
    result: &JsonValue,
    plan_sha256: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let object = exact_object(
        result,
        &[
            "command_sha256",
            "dependencies",
            "duration_ms",
            "exit_code",
            "finished_at_unix_ms",
            "kind",
            "lane_id",
            "log_path",
            "log_sha256",
            "metrics",
            "obligations",
            "plan_sha256",
            "reason",
            "schema_version",
            "started_at_unix_ms",
            "status",
        ],
        "CI lane result",
    )?;
    if object["schema_version"].as_str() != Some(LANE_RESULT_SCHEMA)
        || object["plan_sha256"].as_str() != Some(plan_sha256)
    {
        return Err("CI lane result schema or plan binding is invalid".into());
    }
    let lane_id = bounded_string(&object["lane_id"], "lane-result id", MAX_CANONICAL_ID)?;
    if safe_id(lane_id) != lane_id {
        return Err("CI lane-result id is unsafe".into());
    }
    bounded_string(&object["kind"], "lane-result kind", 96)?;
    let status = object["status"]
        .as_str()
        .filter(|status| matches!(*status, "pass" | "fail" | "canceled"))
        .ok_or("CI lane-result status is invalid")?;
    let reason = object["reason"].as_str();
    if !object["reason"].is_null() {
        bounded_string(&object["reason"], "lane-result reason", 1024)?;
    }
    if (status == "pass") != object["reason"].is_null()
        || status == "pass" && object["exit_code"].as_u64() != Some(0)
        || status == "canceled" && !object["exit_code"].is_null()
        || status != "pass" && reason.is_none()
        || !object["exit_code"].is_null() && object["exit_code"].as_u64().is_none()
        || object["exit_code"]
            .as_u64()
            .is_some_and(|exit_code| exit_code > 255)
    {
        return Err("CI lane-result conclusion fields are inconsistent".into());
    }
    let start = object["started_at_unix_ms"]
        .as_u64()
        .filter(|value| *value > 0)
        .ok_or("invalid lane start")?;
    let finish = object["finished_at_unix_ms"]
        .as_u64()
        .filter(|value| *value >= start)
        .ok_or("invalid lane finish")?;
    let duration = object["duration_ms"]
        .as_u64()
        .filter(|value| *value <= MAX_LANE_DURATION_MS)
        .ok_or("invalid lane duration")?;
    if finish - start > duration.saturating_add(2000) {
        return Err("CI lane timing fields are inconsistent".into());
    }
    let log_path = Path::new(bounded_string(&object["log_path"], "lane log path", 4096)?);
    if !log_path.is_absolute() || !log_path.is_file() {
        return Err("CI lane log is absent or not absolute".into());
    }
    for field in ["log_sha256", "command_sha256"] {
        require_sha256(bounded_string(&object[field], field, 64)?, field)?;
    }
    if sha256_regular_file(log_path, "CI lane log")? != object["log_sha256"].as_str().unwrap() {
        return Err("CI lane log digest mismatch".into());
    }
    for (kind, values) in [
        (
            "lane-result dependencies",
            bounded_string_array(&object["dependencies"], "lane-result dependencies", 64)?,
        ),
        (
            "lane-result obligations",
            bounded_string_array(&object["obligations"], "lane-result obligations", 64)?,
        ),
    ] {
        if values.iter().copied().collect::<BTreeSet<_>>().len() != values.len() {
            return Err(format!("{kind} contain duplicates").into());
        }
    }
    let metrics = exact_object(
        &object["metrics"],
        &[
            "cpu_system_ms",
            "cpu_user_ms",
            "fs_inputs",
            "fs_outputs",
            "peak_rss_bytes",
            "wall_ms",
        ],
        "lane metrics",
    )?;
    for field in ["cpu_system_ms", "cpu_user_ms", "wall_ms"] {
        if metrics[field]
            .as_u64()
            .filter(|value| *value <= MAX_LANE_DURATION_MS)
            .is_none()
        {
            return Err(format!("invalid lane metric {field}").into());
        }
    }
    for field in ["fs_inputs", "fs_outputs", "peak_rss_bytes"] {
        if metrics[field].as_u64().is_none() {
            return Err(format!("invalid lane metric {field}").into());
        }
    }
    Ok(())
}

#[derive(Clone, Debug)]
struct SystemSnapshot {
    mem_available_bytes: u64,
    swap_in_pages: u64,
    oom_kills: u64,
    cpu_total_ticks: u64,
    io_wait_ticks: u64,
    load1: f64,
}

impl SystemSnapshot {
    fn read() -> Result<Self, Box<dyn std::error::Error>> {
        let meminfo = fs::read_to_string("/proc/meminfo")?;
        let vmstat = fs::read_to_string("/proc/vmstat")?;
        let stat = fs::read_to_string("/proc/stat")?;
        let load = fs::read_to_string("/proc/loadavg")?;
        let mem_available_kib = proc_key_u64(&meminfo, "MemAvailable:")?;
        let swap_in_pages = proc_key_u64(&vmstat, "pswpin")?;
        let oom_kills = proc_key_u64(&vmstat, "oom_kill").unwrap_or(0);
        let cpu = stat.lines().next().ok_or("/proc/stat has no CPU row")?;
        let fields = cpu.split_whitespace().collect::<Vec<_>>();
        if fields.first().copied() != Some("cpu") || fields.len() < 6 {
            return Err("/proc/stat CPU row is malformed".into());
        }
        let ticks = fields[1..]
            .iter()
            .map(|value| value.parse::<u64>())
            .collect::<Result<Vec<_>, _>>()?;
        let load1 = load
            .split_whitespace()
            .next()
            .ok_or("/proc/loadavg is empty")?
            .parse::<f64>()?;
        if !load1.is_finite() || load1 < 0.0 {
            return Err("load1 is invalid".into());
        }
        Ok(Self {
            mem_available_bytes: mem_available_kib
                .checked_mul(1024)
                .ok_or("available memory overflow")?,
            swap_in_pages,
            oom_kills,
            cpu_total_ticks: ticks.iter().sum(),
            io_wait_ticks: ticks[4],
            load1,
        })
    }
}

fn proc_key_u64(text: &str, key: &str) -> Result<u64, Box<dyn std::error::Error>> {
    let matches = text
        .lines()
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            (fields.next() == Some(key))
                .then(|| fields.next())
                .flatten()
        })
        .collect::<Vec<_>>();
    if matches.len() != 1 {
        return Err(format!("proc metric {key} is absent or ambiguous").into());
    }
    Ok(matches[0].parse()?)
}

fn cache_stats(context: &RunContext) -> Result<Option<JsonValue>, Box<dyn std::error::Error>> {
    if context.profile == Profile::ReleaseFull {
        return Ok(None);
    }
    let sccache = context
        .sccache_path
        .as_ref()
        .ok_or("presubmit sccache is absent")?;
    let output = Command::new(sccache)
        .env_clear()
        .env("SCCACHE_DIR", "/var/cache/jain-ci/sccache")
        .args(["--show-stats", "--stats-format", "json"])
        .output()?;
    if !output.status.success() || output.stdout.len() > 1024 * 1024 {
        return Err("cannot read bounded sccache statistics".into());
    }
    Ok(Some(serde_json::from_slice(&output.stdout)?))
}

fn aggregate_metrics(
    results: &[JsonValue],
    lanes: &[LaneSpec],
    before: &SystemSnapshot,
    after: &SystemSnapshot,
    cache_before: Option<&JsonValue>,
    cache_after: Option<&JsonValue>,
) -> Result<JsonValue, Box<dyn std::error::Error>> {
    let total_ticks = after.cpu_total_ticks.saturating_sub(before.cpu_total_ticks);
    let io_ticks = after.io_wait_ticks.saturating_sub(before.io_wait_ticks);
    let io_wait_percent = if total_ticks == 0 {
        0.0
    } else {
        (io_ticks as f64 * 100.0) / total_ticks as f64
    };
    if !io_wait_percent.is_finite() || !(0.0..=100.0).contains(&io_wait_percent) {
        return Err("aggregate I/O wait metric is malformed".into());
    }
    let peak_rss_bytes = results
        .iter()
        .filter_map(|result| result["metrics"]["peak_rss_bytes"].as_u64())
        .max()
        .unwrap_or(0);
    let cpu_user_ms = results
        .iter()
        .filter_map(|result| result["metrics"]["cpu_user_ms"].as_u64())
        .sum::<u64>();
    let cpu_system_ms = results
        .iter()
        .filter_map(|result| result["metrics"]["cpu_system_ms"].as_u64())
        .sum::<u64>();
    let critical_path_ms = critical_path(lanes, results)?;
    let (cache_requests, cache_hits, cache_hit_rate) = cache_delta(cache_before, cache_after)?;
    Ok(json!({
        "minimum_available_memory_bytes": before.mem_available_bytes.min(after.mem_available_bytes),
        "swap_in_delta_pages": after.swap_in_pages.saturating_sub(before.swap_in_pages),
        "oom_kill_delta": after.oom_kills.saturating_sub(before.oom_kills),
        "io_wait_percent": io_wait_percent,
        "load1_max": before.load1.max(after.load1),
        "peak_rss_bytes": peak_rss_bytes,
        "cpu_user_ms": cpu_user_ms,
        "cpu_system_ms": cpu_system_ms,
        "critical_path_ms": critical_path_ms,
        "cache_requests": cache_requests,
        "cache_hits": cache_hits,
        "cache_hit_rate": cache_hit_rate
    }))
}

fn critical_path(
    lanes: &[LaneSpec],
    results: &[JsonValue],
) -> Result<u64, Box<dyn std::error::Error>> {
    let durations = results
        .iter()
        .map(|result| {
            (
                result["lane_id"].as_str().unwrap().to_owned(),
                result["duration_ms"].as_u64().unwrap(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut longest = BTreeMap::<String, u64>::new();
    let mut remaining = lanes.iter().collect::<Vec<_>>();
    while !remaining.is_empty() {
        let before = remaining.len();
        remaining.retain(|lane| {
            if lane
                .dependencies
                .iter()
                .all(|dependency| longest.contains_key(dependency))
            {
                let parent = lane
                    .dependencies
                    .iter()
                    .map(|dependency| longest[dependency])
                    .max()
                    .unwrap_or(0);
                longest.insert(lane.id.clone(), parent.saturating_add(durations[&lane.id]));
                false
            } else {
                true
            }
        });
        if remaining.len() == before {
            return Err("cannot calculate critical path for cyclic lanes".into());
        }
    }
    Ok(longest.values().copied().max().unwrap_or(0))
}

fn cache_delta(
    before: Option<&JsonValue>,
    after: Option<&JsonValue>,
) -> Result<(JsonValue, JsonValue, JsonValue), Box<dyn std::error::Error>> {
    match (before, after) {
        (None, None) => Ok((JsonValue::Null, JsonValue::Null, JsonValue::Null)),
        (Some(before), Some(after)) => {
            let requests_before = sccache_counter(before, "compile_requests")?;
            let requests_after = sccache_counter(after, "compile_requests")?;
            let hits_before = sccache_counter(before, "cache_hits")?;
            let hits_after = sccache_counter(after, "cache_hits")?;
            let requests = requests_after.saturating_sub(requests_before);
            let hits = hits_after.saturating_sub(hits_before);
            let rate = if requests == 0 {
                0.0
            } else {
                hits as f64 / requests as f64
            };
            Ok((json!(requests), json!(hits), json!(rate)))
        }
        _ => Err("sccache metric boundary is incomplete".into()),
    }
}

fn sccache_counter(value: &JsonValue, key: &str) -> Result<u64, Box<dyn std::error::Error>> {
    if key == "cache_hits" {
        if let Some(object) = value.get(key).and_then(JsonValue::as_object) {
            return Ok(object.values().filter_map(JsonValue::as_u64).sum());
        }
    }
    value
        .get(key)
        .and_then(JsonValue::as_u64)
        .ok_or_else(|| format!("sccache statistic {key} is missing").into())
}

fn validate_run_receipt(run: &JsonValue) -> Result<(), Box<dyn std::error::Error>> {
    let object = exact_object(
        run,
        &[
            "authority_sha256",
            "base_sha",
            "broker_seal",
            "configuration",
            "duration_ms",
            "finished_at_unix_ms",
            "head_sha",
            "head_tree",
            "host_evidence",
            "lane_results",
            "metrics",
            "plan_schema_version",
            "plan_sha256",
            "profile",
            "publication_allowed",
            "publication_mode",
            "repository",
            "result_set_sha256",
            "schema_version",
            "sealed",
            "started_at_unix_ms",
            "status",
        ],
        "CI run receipt",
    )?;
    if object["schema_version"].as_str() != Some(RUN_SCHEMA)
        || object["plan_schema_version"].as_str() != Some(PLAN_SCHEMA)
        || object["publication_allowed"].as_bool() != Some(false)
        || object["publication_mode"].as_str() != Some("shadow-equivalence")
        || object["sealed"].as_bool() != Some(true)
    {
        return Err("CI run receipt violates shadow publication or sealing policy".into());
    }
    for field in ["plan_sha256", "authority_sha256", "result_set_sha256"] {
        require_sha256(bounded_string(&object[field], field, 64)?, field)?;
    }
    for field in ["head_sha", "head_tree", "base_sha"] {
        require_lower_full_sha(bounded_string(&object[field], field, 40)?, field)?;
    }
    Profile::parse(bounded_string(&object["profile"], "run profile", 32)?)?;
    let configuration = exact_object(
        &object["configuration"],
        &["build_jobs", "calibration", "fleet_concurrency"],
        "run configuration",
    )?;
    let build_jobs = configuration["build_jobs"]
        .as_u64()
        .filter(|value| (1..=64).contains(value))
        .ok_or("run build_jobs is invalid")?;
    let fleet_concurrency = configuration["fleet_concurrency"]
        .as_u64()
        .filter(|value| (1..=64).contains(value))
        .ok_or("run fleet_concurrency is invalid")?;
    let calibration = configuration["calibration"]
        .as_bool()
        .ok_or("run calibration marker is invalid")?;
    if calibration
        && (![4, 8, 16, 24, 32].contains(&build_jobs) || ![2, 4, 6, 8].contains(&fleet_concurrency))
    {
        return Err("run calibration setting is outside the reviewed matrix".into());
    }
    bounded_string(&object["repository"], "run repository", 128)?;
    let start = object["started_at_unix_ms"]
        .as_u64()
        .filter(|value| *value > 0)
        .ok_or("run start is invalid")?;
    let finish = object["finished_at_unix_ms"]
        .as_u64()
        .filter(|value| *value >= start)
        .ok_or("run finish is invalid")?;
    let duration = object["duration_ms"]
        .as_u64()
        .filter(|value| *value <= 86_400_000)
        .ok_or("run duration is invalid")?;
    if finish - start > duration.saturating_add(2000) {
        return Err("run timing fields are inconsistent".into());
    }
    let results = object["lane_results"]
        .as_array()
        .filter(|values| !values.is_empty() && values.len() <= MAX_LANES)
        .ok_or("run lane-result set is invalid")?;
    let status = object["status"]
        .as_str()
        .filter(|value| matches!(*value, "pass" | "fail"))
        .ok_or("run status is invalid")?;
    if (status == "pass")
        != results
            .iter()
            .all(|result| result["status"].as_str() == Some("pass"))
    {
        return Err("run status differs from its lane results".into());
    }
    if format!("{:x}", Sha256::digest(serde_json::to_vec(results)?))
        != object["result_set_sha256"].as_str().unwrap()
    {
        return Err("run result-set digest is invalid".into());
    }
    let metrics = exact_object(
        &object["metrics"],
        &[
            "aggregate_peak_rss_bytes",
            "cache_hit_rate",
            "cache_hits",
            "cache_requests",
            "cpu_system_ms",
            "cpu_user_ms",
            "critical_path_ms",
            "io_wait_percent",
            "load1_max",
            "measurement_sample_count",
            "measurement_source",
            "minimum_available_memory_bytes",
            "oom_kill_delta",
            "swap_in_delta_pages",
        ],
        "run metrics",
    )?;
    for field in [
        "cpu_system_ms",
        "cpu_user_ms",
        "critical_path_ms",
        "aggregate_peak_rss_bytes",
        "measurement_sample_count",
        "minimum_available_memory_bytes",
        "oom_kill_delta",
        "swap_in_delta_pages",
    ] {
        if metrics[field].as_u64().is_none() {
            return Err(format!("run metric {field} is invalid").into());
        }
    }
    if metrics["measurement_source"].as_str() != Some(MEASUREMENT_SOURCE)
        || metrics["measurement_sample_count"]
            .as_u64()
            .filter(|count| *count >= 2)
            .is_none()
    {
        return Err("run metrics are not continuous root-cgroup measurements".into());
    }
    if !metrics["io_wait_percent"]
        .as_f64()
        .is_some_and(|value| value.is_finite() && (0.0..=100.0).contains(&value))
        || !metrics["load1_max"]
            .as_f64()
            .is_some_and(|value| value.is_finite() && value >= 0.0)
    {
        return Err("run I/O-wait or load metric is invalid".into());
    }
    match Profile::parse(object["profile"].as_str().unwrap())? {
        Profile::ReleaseFull => {
            if !metrics["cache_requests"].is_null()
                || !metrics["cache_hits"].is_null()
                || !metrics["cache_hit_rate"].is_null()
            {
                return Err("release-full run contains compiler-cache metrics".into());
            }
        }
        Profile::Presubmit => {
            if metrics["cache_requests"].as_u64().is_none()
                || metrics["cache_hits"].as_u64().is_none()
                || !metrics["cache_hit_rate"]
                    .as_f64()
                    .is_some_and(|value| value.is_finite() && (0.0..=1.0).contains(&value))
            {
                return Err("presubmit run has malformed cache metrics".into());
            }
        }
    }
    let metrics_sha256 = format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&object["metrics"])?)
    );
    let profile = Profile::parse(object["profile"].as_str().unwrap())?;
    let plan_sha256 = object["plan_sha256"].as_str().unwrap();
    let mut lane_ids = BTreeSet::new();
    for result in results {
        validate_lane_result(result, plan_sha256)?;
        let lane_id = result["lane_id"].as_str().unwrap();
        if !lane_ids.insert(lane_id) {
            return Err("run receipt contains duplicate lane results".into());
        }
    }
    validate_host_evidence(
        &object["host_evidence"],
        plan_sha256,
        object["result_set_sha256"].as_str().unwrap(),
        &metrics_sha256,
        profile,
    )?;
    validate_broker_seal(&object["broker_seal"], plan_sha256, fleet_concurrency)?;
    Ok(())
}

fn validate_broker_seal(
    seal: &JsonValue,
    plan_sha256: &str,
    fleet_concurrency: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    let object = exact_object(
        seal,
        &[
            "boundary",
            "evidence_sealer",
            "fleet_lease_id",
            "fleet_slot",
            "fleet_slot_count",
            "measurement_source",
            "plan_path",
            "plan_sha256",
            "sealed_by_uid",
            "source_postcondition",
            "tool_manifest_path",
            "tool_manifest_sha256",
        ],
        "CI broker seal",
    )?;
    if object["boundary"].as_str() != Some(EXECUTION_BOUNDARY)
        || object["evidence_sealer"].as_str() != Some(EVIDENCE_SEALER)
        || object["measurement_source"].as_str() != Some(MEASUREMENT_SOURCE)
        || object["source_postcondition"].as_str() != Some(SOURCE_POSTCONDITION)
        || object["sealed_by_uid"].as_u64() != Some(0)
        || object["plan_sha256"].as_str() != Some(plan_sha256)
    {
        return Err("CI broker seal is not bound to the protected v1 boundary".into());
    }
    require_sha256(
        bounded_string(&object["fleet_lease_id"], "fleet lease", 64)?,
        "fleet lease",
    )?;
    require_sha256(
        bounded_string(&object["tool_manifest_sha256"], "tool manifest digest", 64)?,
        "tool manifest digest",
    )?;
    for field in ["plan_path", "tool_manifest_path"] {
        if !Path::new(bounded_string(&object[field], field, 4096)?).is_absolute() {
            return Err(format!("CI broker seal {field} is not absolute").into());
        }
    }
    let slot_count = object["fleet_slot_count"]
        .as_u64()
        .filter(|count| (1..=64).contains(count))
        .ok_or("CI broker fleet slot count is invalid")?;
    let slot = object["fleet_slot"]
        .as_u64()
        .filter(|slot| *slot < slot_count)
        .ok_or("CI broker fleet slot is invalid")?;
    let _ = slot;
    if slot_count != fleet_concurrency {
        return Err("CI broker fleet lease differs from the planned concurrency".into());
    }
    Ok(())
}

fn validate_host_evidence(
    evidence: &JsonValue,
    plan_sha256: &str,
    result_set_sha256: &str,
    metrics_sha256: &str,
    expected_profile: Profile,
) -> Result<(), Box<dyn std::error::Error>> {
    let object = exact_object(
        evidence,
        &[
            "lane_result_schema_version",
            "measurement_source",
            "metrics_sha256",
            "plan_sha256",
            "profile",
            "publication_allowed",
            "result_set_sha256",
            "schema_version",
        ],
        "host CI evidence v6",
    )?;
    if object["schema_version"].as_str() != Some(HOST_EVIDENCE_SCHEMA)
        || object["lane_result_schema_version"].as_str() != Some(LANE_RESULT_SCHEMA)
        || object["plan_sha256"].as_str() != Some(plan_sha256)
        || object["result_set_sha256"].as_str() != Some(result_set_sha256)
        || object["metrics_sha256"].as_str() != Some(metrics_sha256)
        || object["measurement_source"].as_str() != Some(MEASUREMENT_SOURCE)
        || object["publication_allowed"].as_bool() != Some(false)
    {
        return Err("host CI evidence v6 binding or shadow policy is invalid".into());
    }
    if Profile::parse(bounded_string(
        &object["profile"],
        "host evidence profile",
        32,
    )?)? != expected_profile
    {
        return Err("host CI evidence profile differs from its result".into());
    }
    Ok(())
}

fn unix_ms() -> Result<u64, Box<dyn std::error::Error>> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as u64)
}

pub(crate) fn performance_command(args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    let mut evidence_root = None;
    let mut output = None;
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--evidence-root" => {
                evidence_root = Some(PathBuf::from(
                    iter.next().ok_or("--evidence-root needs a path")?,
                ))
            }
            "--output" => output = Some(PathBuf::from(iter.next().ok_or("--output needs a path")?)),
            value => return Err(format!("unknown ci-performance argument: {value}").into()),
        }
    }
    let evidence_root = evidence_root.ok_or("ci-performance requires --evidence-root")?;
    let report = performance_report(&evidence_root)?;
    if let Some(path) = output {
        write_json_receipt(&path, &report)?;
        println!("wrote {}", path.display());
    } else {
        println!("{}", serde_json::to_string_pretty(&report)?);
    }
    Ok(())
}

#[derive(Clone, Debug)]
struct PerformanceSample {
    profile: Profile,
    repository: String,
    duration_ms: u64,
    valid: bool,
    build_jobs: u64,
    fleet_concurrency: u64,
    calibration: bool,
}

fn performance_report(evidence_root: &Path) -> Result<JsonValue, Box<dyn std::error::Error>> {
    if !evidence_root.is_absolute() {
        return Err("CI performance evidence root must be absolute".into());
    }
    let canonical = fs::canonicalize(evidence_root)?;
    if canonical != evidence_root {
        return Err("CI performance evidence root must be canonical".into());
    }
    let metadata = fs::symlink_metadata(evidence_root)?;
    if !metadata.file_type().is_dir() || metadata.file_type().is_symlink() {
        return Err("CI performance evidence root must be a physical directory".into());
    }
    let mut files = Vec::new();
    collect_json_files(evidence_root, 0, &mut files)?;
    files.sort();
    if files.len() > MAX_EVIDENCE_FILES {
        return Err("CI performance evidence file count exceeds the bounded limit".into());
    }
    let mut samples = Vec::<PerformanceSample>::new();
    for path in files {
        let bytes = read_bounded_evidence(&path)?;
        let value: JsonValue = match serde_json::from_slice(&bytes) {
            Ok(value) => value,
            Err(_) => continue,
        };
        if value.get("schema_version").and_then(JsonValue::as_str) != Some(RUN_SCHEMA) {
            continue;
        }
        let sealed_bytes = read_root_sealed_file(&path, MAX_EVIDENCE_BYTES, "typed CI result")?;
        if sealed_bytes != bytes {
            return Err(format!(
                "typed CI result changed while being classified: {}",
                path.display()
            )
            .into());
        }
        validate_run_receipt(&value)
            .map_err(|error| format!("malformed typed CI evidence {}: {error}", path.display()))?;
        validate_performance_sample_custody(&value)?;
        if value["status"].as_str() != Some("pass") {
            continue;
        }
        let profile = Profile::parse(value["profile"].as_str().unwrap())?;
        let metrics = &value["metrics"];
        let valid = metrics["minimum_available_memory_bytes"].as_u64().unwrap()
            >= MIN_AVAILABLE_MEMORY_BYTES
            && metrics["swap_in_delta_pages"].as_u64() == Some(0)
            && metrics["oom_kill_delta"].as_u64() == Some(0)
            && metrics["io_wait_percent"]
                .as_f64()
                .is_some_and(|value| value < 15.0)
            && metrics["load1_max"]
                .as_f64()
                .is_some_and(|value| value < 96.0);
        samples.push(PerformanceSample {
            profile,
            repository: value["repository"].as_str().unwrap().to_owned(),
            duration_ms: value["duration_ms"].as_u64().unwrap(),
            valid,
            build_jobs: value["configuration"]["build_jobs"].as_u64().unwrap(),
            fleet_concurrency: value["configuration"]["fleet_concurrency"]
                .as_u64()
                .unwrap(),
            calibration: value["configuration"]["calibration"].as_bool().unwrap(),
        });
    }
    let presubmit = performance_profile(
        samples
            .iter()
            .filter(|sample| sample.profile == Profile::Presubmit)
            .map(|sample| (sample.duration_ms, sample.valid))
            .collect(),
        PRESUBMIT_LIMIT_MS,
    );
    let release_full = performance_profile(
        samples
            .iter()
            .filter(|sample| sample.profile == Profile::ReleaseFull)
            .map(|sample| (sample.duration_ms, sample.valid))
            .collect(),
        RELEASE_FULL_LIMIT_MS,
    );
    let calibration = calibration_report(&samples)?;
    let statuses = [
        presubmit["status"].as_str().unwrap(),
        release_full["status"].as_str().unwrap(),
    ];
    let status = if statuses.iter().all(|status| *status == "green") {
        "green"
    } else if statuses.contains(&"red") {
        "red"
    } else {
        "calibrating"
    };
    let report = json!({
        "schema_version": PERFORMANCE_SCHEMA,
        "evidence_root": canonical,
        "generated_at_unix": unix_ms()? / 1000,
        "sample_count": presubmit["observed_sample_count"].as_u64().unwrap()
            + release_full["observed_sample_count"].as_u64().unwrap(),
        "status": status,
        "calibration": calibration,
        "profiles": {
            "presubmit": presubmit,
            "release-full": release_full
        }
    });
    validate_performance_report(&report)?;
    Ok(report)
}

fn validate_performance_sample_custody(run: &JsonValue) -> Result<(), Box<dyn std::error::Error>> {
    let seal = &run["broker_seal"];
    let plan_path = Path::new(seal["plan_path"].as_str().unwrap());
    let plan_bytes = read_root_sealed_file(plan_path, MAX_PLAN_BYTES, "typed CI plan")?;
    let plan_sha256 = format!("{:x}", Sha256::digest(&plan_bytes));
    if plan_sha256 != run["plan_sha256"].as_str().unwrap()
        || seal["plan_sha256"].as_str() != Some(&plan_sha256)
    {
        return Err("root broker seal does not bind the exact immutable CI plan".into());
    }
    let plan: JsonValue = serde_json::from_slice(&plan_bytes)?;
    validate_plan_value(&plan)?;
    if plan["execution"]["allowed"].as_bool() != Some(true) {
        return Err("inactive typed CI v1 plans cannot contribute performance evidence".into());
    }
    for (run_field, plan_field) in [
        ("repository", "repository"),
        ("head_sha", "head_sha"),
        ("head_tree", "head_tree"),
        ("base_sha", "base_sha"),
    ] {
        if run[run_field] != plan[plan_field] {
            return Err("typed CI result identity differs from its sealed plan".into());
        }
    }
    if run["authority_sha256"] != plan["authority"]["manifest_sha256"]
        || run["profile"] != plan["profile"]["effective"]
        || run["configuration"]["build_jobs"] != plan["resources"]["build_jobs"]
        || run["configuration"]["fleet_concurrency"] != plan["resources"]["fleet_concurrency"]
        || run["configuration"]["calibration"] != plan["resources"]["calibration"]
    {
        return Err("typed CI result configuration differs from its sealed plan".into());
    }
    let tool_manifest_path = Path::new(seal["tool_manifest_path"].as_str().unwrap());
    let tool_manifest = read_root_sealed_file(
        tool_manifest_path,
        MAX_EVIDENCE_BYTES,
        "typed CI tool manifest",
    )?;
    if format!("{:x}", Sha256::digest(&tool_manifest))
        != seal["tool_manifest_sha256"].as_str().unwrap()
    {
        return Err("root broker seal has a drifting tool manifest".into());
    }
    Ok(())
}

fn calibration_report(
    samples: &[PerformanceSample],
) -> Result<JsonValue, Box<dyn std::error::Error>> {
    const REQUIRED: [&str; 4] = [
        "jain-split-ops",
        "jain-web",
        "jain-smartcluster",
        "redline-core",
    ];
    let mut grouped = BTreeMap::<(u64, u64), Vec<&PerformanceSample>>::new();
    for sample in samples.iter().filter(|sample| sample.calibration) {
        grouped
            .entry((sample.build_jobs, sample.fleet_concurrency))
            .or_default()
            .push(sample);
    }
    let mut candidates = Vec::<JsonValue>::new();
    for ((build_jobs, fleet_concurrency), group) in grouped {
        let repositories = group
            .iter()
            .map(|sample| sample.repository.as_str())
            .collect::<BTreeSet<_>>();
        let missing_repositories = REQUIRED
            .iter()
            .filter(|repository| !repositories.contains(**repository))
            .copied()
            .collect::<Vec<_>>();
        let invalid_sample_count = group.iter().filter(|sample| !sample.valid).count();
        let mut durations = group
            .iter()
            .filter_map(|sample| sample.valid.then_some(sample.duration_ms))
            .collect::<Vec<_>>();
        durations.sort_unstable();
        candidates.push(json!({
            "build_jobs": build_jobs,
            "fleet_concurrency": fleet_concurrency,
            "observed_sample_count": group.len(),
            "valid_sample_count": durations.len(),
            "invalid_sample_count": invalid_sample_count,
            "repositories": repositories,
            "missing_repositories": missing_repositories,
            "p95_ms": nearest_rank_p95(&durations),
            "improvement_percent": null,
            "setting_valid": false
        }));
    }
    candidates.sort_by_key(|candidate| {
        let setting = (
            candidate["build_jobs"].as_u64().unwrap(),
            candidate["fleet_concurrency"].as_u64().unwrap(),
        );
        CALIBRATION_SETTINGS
            .iter()
            .position(|candidate| *candidate == setting)
            .unwrap()
    });
    for index in 0..candidates.len() {
        let setting = (
            candidates[index]["build_jobs"].as_u64().unwrap(),
            candidates[index]["fleet_concurrency"].as_u64().unwrap(),
        );
        let order_index = CALIBRATION_SETTINGS
            .iter()
            .position(|candidate| *candidate == setting)
            .unwrap();
        let baseline = order_index
            .checked_sub(1)
            .and_then(|previous| {
                candidates.iter().find(|candidate| {
                    candidate["build_jobs"].as_u64() == Some(CALIBRATION_SETTINGS[previous].0)
                        && candidate["fleet_concurrency"].as_u64()
                            == Some(CALIBRATION_SETTINGS[previous].1)
                })
            })
            .and_then(|candidate| candidate["p95_ms"].as_u64());
        let p95 = candidates[index]["p95_ms"].as_u64();
        let improvement = match (baseline, p95) {
            (Some(baseline), Some(current)) if baseline > 0 => {
                Some((baseline as f64 - current as f64) * 100.0 / baseline as f64)
            }
            _ => None,
        };
        let complete = candidates[index]["missing_repositories"]
            .as_array()
            .is_some_and(Vec::is_empty);
        let resources_valid = candidates[index]["invalid_sample_count"].as_u64() == Some(0);
        let improvement_valid = baseline.is_some()
            && improvement.is_some_and(|percent| percent.is_finite() && percent >= 5.0);
        candidates[index]["improvement_percent"] =
            improvement.map_or(JsonValue::Null, |value| json!(value));
        candidates[index]["setting_valid"] =
            json!(complete && resources_valid && p95.is_some() && improvement_valid);
    }
    let selected = candidates
        .iter()
        .filter(|candidate| candidate["setting_valid"].as_bool() == Some(true))
        .min_by(|left, right| {
            (
                left["p95_ms"].as_u64().unwrap(),
                left["build_jobs"].as_u64().unwrap(),
                left["fleet_concurrency"].as_u64().unwrap(),
            )
                .cmp(&(
                    right["p95_ms"].as_u64().unwrap(),
                    right["build_jobs"].as_u64().unwrap(),
                    right["fleet_concurrency"].as_u64().unwrap(),
                ))
        })
        .map(|candidate| {
            json!({
                "build_jobs": candidate["build_jobs"],
                "fleet_concurrency": candidate["fleet_concurrency"],
                "p95_ms": candidate["p95_ms"]
            })
        });
    Ok(json!({
        "required_repositories": REQUIRED,
        "minimum_improvement_percent": 5.0,
        "candidates": candidates,
        "selected": selected
    }))
}

fn collect_json_files(
    root: &Path,
    depth: usize,
    files: &mut Vec<PathBuf>,
) -> Result<(), Box<dyn std::error::Error>> {
    if depth > 16 {
        return Err("CI evidence directory depth exceeds the bounded limit".into());
    }
    let mut entries = fs::read_dir(root)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() {
            return Err(format!(
                "CI evidence contains a forbidden symlink: {}",
                path.display()
            )
            .into());
        }
        if metadata.file_type().is_dir() {
            collect_json_files(&path, depth + 1, files)?;
        } else if metadata.file_type().is_file() {
            if path.extension().and_then(|extension| extension.to_str()) == Some("json") {
                files.push(path);
                if files.len() > MAX_EVIDENCE_FILES {
                    return Err("CI evidence file count exceeds the bounded limit".into());
                }
            }
        } else {
            return Err(format!("CI evidence contains a special node: {}", path.display()).into());
        }
    }
    Ok(())
}

fn read_bounded_evidence(path: &Path) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    const O_NONBLOCK: i32 = 0o4000;
    const O_NOFOLLOW: i32 = 0o400000;
    let mut file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(O_NONBLOCK | O_NOFOLLOW)
        .open(path)?;
    let before = file.metadata()?;
    if !before.file_type().is_file() || before.nlink() != 1 || before.len() > MAX_EVIDENCE_BYTES {
        return Err(format!("unsafe or oversized CI evidence: {}", path.display()).into());
    }
    let mut bytes = Vec::with_capacity(before.len() as usize);
    (&mut file)
        .take(MAX_EVIDENCE_BYTES + 1)
        .read_to_end(&mut bytes)?;
    let after = file.metadata()?;
    let path_after = fs::symlink_metadata(path)?;
    if bytes.len() as u64 != before.len()
        || !same_metadata(&before, &after)
        || !same_metadata(&before, &path_after)
    {
        return Err(format!("CI evidence changed while reading: {}", path.display()).into());
    }
    Ok(bytes)
}

fn read_root_sealed_file(
    path: &Path,
    maximum_bytes: u64,
    kind: &str,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let path = physical_regular_path(path, kind)?;
    const O_NONBLOCK: i32 = 0o4000;
    const O_NOFOLLOW: i32 = 0o400000;
    let mut file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(O_NONBLOCK | O_NOFOLLOW)
        .open(&path)?;
    let before = file.metadata()?;
    if before.uid() != 0
        || before.gid() != 0
        || before.nlink() != 1
        || before.mode() & 0o777 != 0o444
        || before.len() == 0
        || before.len() > maximum_bytes
    {
        return Err(
            format!("{kind} must be root-owned, single-link, mode 0444, and bounded").into(),
        );
    }
    let mut bytes = Vec::with_capacity(before.len() as usize);
    (&mut file)
        .take(maximum_bytes + 1)
        .read_to_end(&mut bytes)?;
    let after = file.metadata()?;
    let path_after = fs::symlink_metadata(&path)?;
    if bytes.len() as u64 != before.len()
        || !same_metadata(&before, &after)
        || !same_metadata(&before, &path_after)
    {
        return Err(format!("{kind} changed while being read").into());
    }
    Ok(bytes)
}

fn performance_profile(samples: Vec<(u64, bool)>, limit_ms: u64) -> JsonValue {
    let observed = samples.len();
    let invalid = samples.iter().filter(|(_, valid)| !*valid).count();
    let mut valid = samples
        .into_iter()
        .filter_map(|(duration, valid)| valid.then_some(duration))
        .collect::<Vec<_>>();
    valid.sort_unstable();
    let p95 = nearest_rank_p95(&valid);
    let status = if invalid > 0 || p95.is_some_and(|value| value > limit_ms) {
        "red"
    } else if valid.len() < MIN_PERFORMANCE_SAMPLES {
        "calibrating"
    } else {
        "green"
    };
    json!({
        "observed_sample_count": observed,
        "sample_count": valid.len(),
        "invalid_sample_count": invalid,
        "p95_ms": p95,
        "limit_ms": limit_ms,
        "status": status
    })
}

fn nearest_rank_p95(sorted: &[u64]) -> Option<u64> {
    if sorted.is_empty() {
        return None;
    }
    let rank = (95 * sorted.len()).div_ceil(100);
    sorted.get(rank.saturating_sub(1)).copied()
}

fn validate_performance_report(report: &JsonValue) -> Result<(), Box<dyn std::error::Error>> {
    let object = exact_object(
        report,
        &[
            "calibration",
            "evidence_root",
            "generated_at_unix",
            "profiles",
            "sample_count",
            "schema_version",
            "status",
        ],
        "CI performance report",
    )?;
    if object["schema_version"].as_str() != Some(PERFORMANCE_SCHEMA) {
        return Err("unsupported CI performance schema".into());
    }
    let root = Path::new(bounded_string(
        &object["evidence_root"],
        "performance evidence root",
        4096,
    )?);
    if !root.is_absolute() {
        return Err("performance evidence root is not absolute".into());
    }
    if object["generated_at_unix"]
        .as_u64()
        .filter(|value| *value > 0)
        .is_none()
        || object["sample_count"]
            .as_u64()
            .filter(|value| *value <= MAX_EVIDENCE_FILES as u64)
            .is_none()
    {
        return Err("performance report counters are invalid".into());
    }
    let overall = object["status"]
        .as_str()
        .filter(|value| matches!(*value, "green" | "calibrating" | "red"))
        .ok_or("performance report status is invalid")?;
    let profiles = exact_object(
        &object["profiles"],
        &["presubmit", "release-full"],
        "performance profiles",
    )?;
    let mut observed_total = 0_u64;
    validate_calibration_report(&object["calibration"])?;
    let mut statuses = Vec::new();
    for (name, limit) in [
        ("presubmit", PRESUBMIT_LIMIT_MS),
        ("release-full", RELEASE_FULL_LIMIT_MS),
    ] {
        let profile = exact_object(
            &profiles[name],
            &[
                "invalid_sample_count",
                "limit_ms",
                "observed_sample_count",
                "p95_ms",
                "sample_count",
                "status",
            ],
            "performance profile",
        )?;
        let observed = profile["observed_sample_count"]
            .as_u64()
            .ok_or("observed sample count is invalid")?;
        let valid = profile["sample_count"]
            .as_u64()
            .ok_or("valid sample count is invalid")?;
        let invalid = profile["invalid_sample_count"]
            .as_u64()
            .ok_or("invalid sample count is invalid")?;
        if observed != valid + invalid || profile["limit_ms"].as_u64() != Some(limit) {
            return Err("performance profile counters or limit are inconsistent".into());
        }
        observed_total += observed;
        let p95 = profile["p95_ms"].as_u64();
        if (valid == 0) != profile["p95_ms"].is_null() {
            return Err("performance p95 presence differs from sample count".into());
        }
        let status = profile["status"]
            .as_str()
            .filter(|value| matches!(*value, "green" | "calibrating" | "red"))
            .ok_or("performance profile status is invalid")?;
        let expected = if invalid > 0 || p95.is_some_and(|value| value > limit) {
            "red"
        } else if valid < MIN_PERFORMANCE_SAMPLES as u64 {
            "calibrating"
        } else {
            "green"
        };
        if status != expected {
            return Err("performance profile status is inconsistent".into());
        }
        statuses.push(status);
    }
    let expected_overall = if statuses.iter().all(|status| *status == "green") {
        "green"
    } else if statuses.contains(&"red") {
        "red"
    } else {
        "calibrating"
    };
    if object["sample_count"].as_u64() != Some(observed_total) || overall != expected_overall {
        return Err("overall performance status or count is inconsistent".into());
    }
    Ok(())
}

fn validate_calibration_report(value: &JsonValue) -> Result<(), Box<dyn std::error::Error>> {
    let object = exact_object(
        value,
        &[
            "candidates",
            "minimum_improvement_percent",
            "required_repositories",
            "selected",
        ],
        "CI calibration report",
    )?;
    if object["minimum_improvement_percent"].as_f64() != Some(5.0)
        || object["required_repositories"]
            != json!([
                "jain-split-ops",
                "jain-web",
                "jain-smartcluster",
                "redline-core"
            ])
    {
        return Err("CI calibration authority differs from the reviewed matrix".into());
    }
    let candidates = object["candidates"]
        .as_array()
        .filter(|candidates| candidates.len() <= 20)
        .ok_or("CI calibration candidates are invalid")?;
    let mut settings = BTreeSet::new();
    let mut last_order = None;
    for candidate in candidates {
        let candidate = exact_object(
            candidate,
            &[
                "build_jobs",
                "fleet_concurrency",
                "improvement_percent",
                "invalid_sample_count",
                "missing_repositories",
                "observed_sample_count",
                "p95_ms",
                "repositories",
                "setting_valid",
                "valid_sample_count",
            ],
            "CI calibration candidate",
        )?;
        let build_jobs = candidate["build_jobs"]
            .as_u64()
            .ok_or("calibration build jobs missing")?;
        let fleet = candidate["fleet_concurrency"]
            .as_u64()
            .ok_or("calibration fleet missing")?;
        if ![4, 8, 16, 24, 32].contains(&build_jobs)
            || ![2, 4, 6, 8].contains(&fleet)
            || !settings.insert((build_jobs, fleet))
        {
            return Err("CI calibration setting is invalid or duplicated".into());
        }
        let order = CALIBRATION_SETTINGS
            .iter()
            .position(|setting| *setting == (build_jobs, fleet))
            .unwrap();
        if last_order.is_some_and(|previous| order <= previous) {
            return Err("CI calibration candidates are not in the governed sequence".into());
        }
        last_order = Some(order);
        let observed = candidate["observed_sample_count"]
            .as_u64()
            .ok_or("calibration observed count missing")?;
        let valid = candidate["valid_sample_count"]
            .as_u64()
            .ok_or("calibration valid count missing")?;
        let invalid = candidate["invalid_sample_count"]
            .as_u64()
            .ok_or("calibration invalid count missing")?;
        if observed != valid + invalid
            || candidate["setting_valid"].as_bool().is_none()
            || !(candidate["p95_ms"].is_null() || candidate["p95_ms"].as_u64().is_some())
            || !(candidate["improvement_percent"].is_null()
                || candidate["improvement_percent"]
                    .as_f64()
                    .is_some_and(f64::is_finite))
        {
            return Err("CI calibration candidate metrics are inconsistent".into());
        }
        let repositories =
            bounded_string_array(&candidate["repositories"], "calibration repositories", 64)?;
        let missing = bounded_string_array(
            &candidate["missing_repositories"],
            "calibration missing repositories",
            4,
        )?;
        if repositories.iter().collect::<BTreeSet<_>>().len() != repositories.len()
            || missing.iter().collect::<BTreeSet<_>>().len() != missing.len()
        {
            return Err("CI calibration repository sets contain duplicates".into());
        }
        let expected_missing = [
            "jain-split-ops",
            "jain-web",
            "jain-smartcluster",
            "redline-core",
        ]
        .into_iter()
        .filter(|required| !repositories.contains(required))
        .collect::<Vec<_>>();
        if missing != expected_missing || (valid == 0) != candidate["p95_ms"].is_null() {
            return Err("CI calibration coverage or p95 presence is inconsistent".into());
        }
        let predecessor = order.checked_sub(1).and_then(|previous| {
            candidates.iter().find(|other| {
                other["build_jobs"].as_u64() == Some(CALIBRATION_SETTINGS[previous].0)
                    && other["fleet_concurrency"].as_u64() == Some(CALIBRATION_SETTINGS[previous].1)
            })
        });
        let expected_improvement = predecessor
            .and_then(|previous| previous["p95_ms"].as_u64())
            .zip(candidate["p95_ms"].as_u64())
            .and_then(|(previous, current)| {
                (previous > 0)
                    .then_some((previous as f64 - current as f64) * 100.0 / previous as f64)
            });
        let actual_improvement = candidate["improvement_percent"].as_f64();
        if expected_improvement.is_some() != actual_improvement.is_some()
            || expected_improvement
                .zip(actual_improvement)
                .is_some_and(|(expected, actual)| (expected - actual).abs() > 1e-9)
        {
            return Err("CI calibration improvement is not bound to its predecessor".into());
        }
        let expected_valid = predecessor.is_some()
            && missing.is_empty()
            && invalid == 0
            && candidate["p95_ms"].as_u64().is_some()
            && expected_improvement.is_some_and(|improvement| improvement >= 5.0);
        if candidate["setting_valid"].as_bool() != Some(expected_valid) {
            return Err("CI calibration validity is not sequential and fail-closed".into());
        }
    }
    let expected_selected = candidates
        .iter()
        .filter(|candidate| candidate["setting_valid"].as_bool() == Some(true))
        .min_by_key(|candidate| {
            (
                candidate["p95_ms"].as_u64().unwrap(),
                candidate["build_jobs"].as_u64().unwrap(),
                candidate["fleet_concurrency"].as_u64().unwrap(),
            )
        });
    if let Some(selected) = object["selected"].as_object() {
        let selected = exact_object(
            &JsonValue::Object(selected.clone()),
            &["build_jobs", "fleet_concurrency", "p95_ms"],
            "selected CI calibration",
        )?
        .clone();
        let setting = (
            selected["build_jobs"]
                .as_u64()
                .ok_or("selected build jobs missing")?,
            selected["fleet_concurrency"]
                .as_u64()
                .ok_or("selected fleet missing")?,
        );
        let p95 = selected["p95_ms"].as_u64().ok_or("selected p95 missing")?;
        if !expected_selected.is_some_and(|candidate| {
            candidate["build_jobs"].as_u64() == Some(setting.0)
                && candidate["fleet_concurrency"].as_u64() == Some(setting.1)
                && candidate["p95_ms"].as_u64() == Some(p95)
        }) {
            return Err("selected CI calibration is not the fastest valid candidate".into());
        }
    } else if !object["selected"].is_null() || expected_selected.is_some() {
        return Err("selected CI calibration must be an object or null".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_ID: AtomicU64 = AtomicU64::new(1);

    struct TestDir(PathBuf);

    impl TestDir {
        fn new(label: &str) -> Self {
            let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
            let path = env::temp_dir().join(format!(
                "jain-typed-ci-test-{}-{}-{id}",
                std::process::id(),
                safe_id(label)
            ));
            assert!(!path.exists());
            fs::create_dir(&path).unwrap();
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            assert!(self.0.starts_with(env::temp_dir()));
            assert!(self
                .0
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("jain-typed-ci-test-")));
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn git(repo: &Path, args: &[&str]) -> String {
        let output = Command::new("/usr/bin/git")
            .env_clear()
            .env("LC_ALL", "C")
            .args(["-C"])
            .arg(repo)
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap().trim().to_owned()
    }

    fn write(path: &Path, contents: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }

    fn rust_fixture() -> (TestDir, String, String, Vec<String>, Vec<String>) {
        let root = TestDir::new("rust-routing");
        git(root.path(), &["init", "-q", "-b", "main"]);
        git(root.path(), &["config", "user.name", "CI test"]);
        git(root.path(), &["config", "user.email", "ci-test@invalid"]);
        write(
            &root.path().join("Cargo.toml"),
            "[workspace]\nmembers=[\"crates/a\",\"crates/b\"]\nresolver=\"2\"\n",
        );
        write(
            &root.path().join("crates/a/Cargo.toml"),
            "[package]\nname=\"fixture-a\"\nversion=\"0.1.0\"\nedition=\"2021\"\n",
        );
        write(&root.path().join("crates/a/src/lib.rs"), "pub fn a() {}\n");
        write(
            &root.path().join("crates/b/Cargo.toml"),
            "[package]\nname=\"fixture-b\"\nversion=\"0.1.0\"\nedition=\"2021\"\n[dependencies]\nfixture-a={path=\"../a\"}\n",
        );
        write(&root.path().join("crates/b/src/lib.rs"), "pub fn b() {}\n");
        git(root.path(), &["add", "."]);
        git(root.path(), &["commit", "-q", "-m", "base"]);
        let base = git(root.path(), &["rev-parse", "HEAD"]);
        write(
            &root.path().join("crates/a/src/lib.rs"),
            "pub fn a() { assert!(true); }\n",
        );
        git(root.path(), &["add", "."]);
        git(root.path(), &["commit", "-q", "-m", "change a"]);
        let head = git(root.path(), &["rev-parse", "HEAD"]);
        let tracked =
            git_nul_paths(root.path(), &["ls-tree", "-r", "--name-only", "-z", &head]).unwrap();
        let changed =
            git_nul_paths(root.path(), &["diff", "--name-only", "-z", &base, &head]).unwrap();
        (root, base, head, tracked, changed)
    }

    fn node_fixture(command: &str, lane_name: &str) -> (TestDir, String, Vec<String>) {
        let root = TestDir::new("node-routing");
        git(root.path(), &["init", "-q", "-b", "main"]);
        git(root.path(), &["config", "user.name", "CI test"]);
        git(root.path(), &["config", "user.email", "ci-test@invalid"]);
        write(
            &root.path().join("apps/web/package.json"),
            "{\"name\":\"fixture-web\",\"scripts\":{\"test\":\"node --test\"}}\n",
        );
        write(
            &root.path().join("apps/web/src/app.ts"),
            "export const value = 1;\n",
        );
        write(
            &root.path().join("agent/test-map.json"),
            &serde_json::to_string_pretty(&json!({
                "tests": {
                    "apps/web/**": {
                        "command": command,
                        "lane": lane_name,
                        "purpose": "fixture"
                    }
                }
            }))
            .unwrap(),
        );
        git(root.path(), &["add", "."]);
        git(root.path(), &["commit", "-q", "-m", "base"]);
        write(
            &root.path().join("apps/web/src/app.ts"),
            "export const value = 2;\n",
        );
        git(root.path(), &["add", "."]);
        git(root.path(), &["commit", "-q", "-m", "change web"]);
        let head = git(root.path(), &["rev-parse", "HEAD"]);
        (root, head, vec!["apps/web/src/app.ts".to_owned()])
    }

    fn presubmit_fixture_executables() -> BTreeMap<String, JsonValue> {
        let executable =
            |name: &str, path: &str| json!({"name": name, "path": path, "sha256": "a".repeat(64)});
        BTreeMap::from([
            ("bash".to_owned(), executable("bash", "/usr/bin/bash")),
            ("cargo".to_owned(), executable("cargo", "/usr/bin/cargo")),
            ("pnpm".to_owned(), executable("pnpm", "/usr/bin/pnpm")),
        ])
    }

    fn simple_lane(id: &str, dependencies: Vec<&str>, obligations: Vec<&str>) -> JsonValue {
        lane(
            id,
            "test",
            "/usr/bin/bash",
            vec!["-c".to_owned(), "true".to_owned()],
            dependencies.into_iter().map(str::to_owned).collect(),
            obligations.into_iter().map(str::to_owned).collect(),
            vec!["fixture".to_owned()],
            30,
            "presubmit-only",
        )
    }

    fn validation_plan(profile: Profile) -> JsonValue {
        let cache_policy = if profile == Profile::ReleaseFull {
            "forbidden"
        } else {
            "presubmit-only"
        };
        let obligations = if profile == Profile::ReleaseFull {
            vec![
                "required",
                "security",
                "complete-coverage",
                "contract-drift",
                "jankurai",
                "artifact",
                "compatibility",
                "conformance",
                "repository-specific",
            ]
        } else {
            vec!["static-security"]
        };
        let lanes = vec![lane(
            "required",
            "test",
            "/usr/bin/bash",
            vec!["-c".to_owned(), "true".to_owned()],
            vec![],
            obligations.into_iter().map(str::to_owned).collect(),
            vec!["fixture".to_owned()],
            30,
            cache_policy,
        )];
        let cache = if profile == Profile::ReleaseFull {
            json!({
                "mode": "forbidden", "enabled": false, "root": null, "max_bytes": 0,
                "compiler_cache_forbidden_for_release": true
            })
        } else {
            json!({
                "mode": "content-addressed-local-only", "enabled": true,
                "root": "/var/cache/jain-ci/sccache", "max_bytes": SCCACHE_MAX_BYTES,
                "compiler_cache_forbidden_for_release": true
            })
        };
        json!({
            "schema_version": PLAN_SCHEMA,
            "repository": "fixture",
            "repository_path": "/tmp/fixture",
            "required_check": "fixture/required",
            "head_sha": "a".repeat(40),
            "head_tree": "b".repeat(40),
            "base_sha": "c".repeat(40),
            "base_tree": "d".repeat(40),
            "authority": {
                "manifest_path": "/tmp/manifest",
                "manifest_sha256": "a".repeat(64),
                "plan_schema_sha256": "b".repeat(64),
                "lane_result_schema_sha256": "c".repeat(64),
                "performance_schema_sha256": "d".repeat(64),
                "host_result_schema_sha256": "e".repeat(64),
                "host_evidence_schema_sha256": "f".repeat(64)
            },
            "profile": {
                "requested": profile.as_str(), "effective": profile.as_str(),
                "selected_by_uid": 0, "widened": false, "widening_reasons": []
            },
            "source_scope": {
                "changed_paths": ["src/lib.rs"], "contract_consumers": [],
                "node_packages": [], "rust_packages": [],
                "scope": if profile == Profile::ReleaseFull { "complete" } else { "changed-surface" }
            },
            "cross_repo_dependencies": [],
            "cargo_configuration": [], "toolchains": [], "lockfiles": [],
            "executables": [
                {"name":"bash","path":"/usr/bin/bash","sha256":"1".repeat(64)},
                {"name":"cargo","path":"/usr/bin/cargo","sha256":"2".repeat(64)},
                {"name":"clippy-driver","path":"/usr/bin/clippy-driver","sha256":"3".repeat(64)},
                {"name":"rustc","path":"/usr/bin/rustc","sha256":"4".repeat(64)},
                {"name":"rustdoc","path":"/usr/bin/rustdoc","sha256":"5".repeat(64)},
                {"name":"rustfmt","path":"/usr/bin/rustfmt","sha256":"6".repeat(64)},
                {"name":"time","path":"/usr/bin/time","sha256":"7".repeat(64)}
            ],
            "resources": {
                "authority_build_jobs": 4, "authority_fleet_concurrency": 4,
                "build_jobs": 4, "fleet_concurrency": 4, "calibration": false,
                "host_network_namespace_device": 1, "host_network_namespace_inode": 1,
                "max_parallel_lanes": 4, "minimum_available_memory_bytes": MIN_AVAILABLE_MEMORY_BYTES,
                "swap_in_pages_max": 0, "io_wait_percent_max_exclusive": 15.0,
                "load1_max_exclusive": 96.0, "network": "denied"
            },
            "cache": cache,
            "lanes": lanes,
            "execution": {
                "allowed": false, "activation_state": "blocked",
                "activation_requires": "release-full-equivalence",
                "required_boundary": EXECUTION_BOUNDARY,
                "required_evidence_sealer": EVIDENCE_SEALER,
                "required_fleet_scheduler": FLEET_SCHEDULER,
                "required_measurement_source": MEASUREMENT_SOURCE,
                "required_source_postcondition": SOURCE_POSTCONDITION,
                "required_tool_manifest": TOOL_MANIFEST
            },
            "publication": {
                "mode": "shadow-equivalence", "allowed": false,
                "required_equivalence_profile": "release-full", "required_check": "fixture/required"
            }
        })
    }

    #[test]
    fn rejects_abbreviated_and_uppercase_object_ids() {
        assert!(require_lower_full_sha("abc123", "head").is_err());
        assert!(require_lower_full_sha(&"A".repeat(40), "head").is_err());
        assert!(require_lower_full_sha(&"a".repeat(40), "head").is_ok());
    }

    #[test]
    fn sensitive_surfaces_widen_presubmit_to_full() {
        for (path, expected) in [
            ("Cargo.lock", "lockfile"),
            ("ops/ci/runner.sh", "ci-control"),
            ("contracts/protocol.json", "contract"),
            ("agent/generated-zones.toml", "generated-or-source-policy"),
            ("agent/audit-policy.toml", "generated-or-source-policy"),
            ("authority/source-paths.txt", "generated-or-source-policy"),
            ("native/engine.cpp", "native-code"),
            ("tools/splitctl/src/ci.rs", "control-plane-source"),
            ("repos.manifest.toml", "control"),
        ] {
            assert!(widening_reasons(
                &[path.to_owned()],
                &["authority/source-paths.txt".to_owned()]
            )
            .contains(&expected.to_owned()));
        }
        assert!(widening_reasons(&["crates/demo/src/lib.rs".to_owned()], &[]).is_empty());
    }

    #[test]
    fn rust_routing_includes_changed_package_and_reverse_dependents() {
        let (root, _base, head, tracked, changed) = rust_fixture();
        assert_eq!(
            affected_rust_packages(root.path(), &head, &tracked, &changed).unwrap(),
            vec!["fixture-a".to_owned(), "fixture-b".to_owned()]
        );
    }

    #[test]
    fn standalone_checkout_is_not_a_worktree_and_is_removed_on_drop() {
        let (source, _base, head, _tracked, _changed) = rust_fixture();
        let writable = TestDir::new("standalone-checkout");
        let tree = git(source.path(), &["rev-parse", "HEAD^{tree}"]);
        let worktrees_before = git(source.path(), &["worktree", "list", "--porcelain"]);
        let checkout = materialize_temporary_checkout(
            source.path(),
            writable.path(),
            &head,
            &tree,
            "http://127.0.0.1:8787/git/veox/fixture.git",
            &"a".repeat(64),
        )
        .unwrap();
        let checkout_path = checkout.path.clone();
        assert_eq!(
            git(&checkout_path, &["rev-parse", "--git-common-dir"]),
            ".git"
        );
        assert_eq!(
            git(&checkout_path, &["remote", "get-url", "origin"]),
            "http://127.0.0.1:8787/git/veox/fixture.git"
        );
        assert_eq!(
            git(source.path(), &["worktree", "list", "--porcelain"]),
            worktrees_before
        );
        drop(checkout);
        assert!(!checkout_path.exists());
    }

    #[test]
    fn node_routing_selects_the_nearest_package() {
        let tracked = vec![
            "package.json".to_owned(),
            "apps/web/package.json".to_owned(),
            "apps/admin/package.json".to_owned(),
        ];
        assert_eq!(
            affected_node_packages(&tracked, &["apps/web/src/app.tsx".to_owned()]),
            vec!["apps/web".to_owned()]
        );
    }

    #[test]
    fn affected_node_routes_bind_direct_pnpm_tests_and_reject_broad_required() {
        let (root, head, changed) = node_fixture("pnpm --dir apps/web run test", "node-test");
        let raw: toml::Value = "name = \"fixture\"".parse().unwrap();
        let lanes = presubmit_lanes(
            root.path(),
            &head,
            &changed,
            &[],
            &["apps/web".to_owned()],
            &[],
            &raw,
            &presubmit_fixture_executables(),
        )
        .unwrap();
        let node_lane = lanes
            .iter()
            .find(|lane| lane["kind"] == "node-test")
            .unwrap();
        assert_eq!(node_lane["program"], "/usr/bin/pnpm");
        assert_eq!(
            node_lane["args"],
            json!(["--dir", "apps/web", "run", "test"])
        );
        assert!(node_lane["obligations"]
            .as_array()
            .unwrap()
            .contains(&json!("affected-node-tests")));
        assert!(!lanes.iter().any(|lane| {
            lane["args"]
                .as_array()
                .is_some_and(|args| args.contains(&json!("just required")))
        }));

        let (broad, broad_head, broad_changed) = node_fixture("just required", "required");
        assert!(presubmit_lanes(
            broad.path(),
            &broad_head,
            &broad_changed,
            &[],
            &["apps/web".to_owned()],
            &[],
            &raw,
            &presubmit_fixture_executables(),
        )
        .is_err());
    }

    #[test]
    fn affected_node_routes_reject_untyped_or_drifting_commands() {
        let raw: toml::Value = "name = \"fixture\"".parse().unwrap();
        for (command, lane_name) in [
            ("pnpm --dir apps/web run test", "required"),
            ("pnpm --dir apps/web run test:coverage", "node-test"),
            ("just required", "node-test"),
        ] {
            let (root, head, changed) = node_fixture(command, lane_name);
            assert!(presubmit_lanes(
                root.path(),
                &head,
                &changed,
                &[],
                &["apps/web".to_owned()],
                &[],
                &raw,
                &presubmit_fixture_executables(),
            )
            .is_err());
        }
    }

    #[test]
    fn test_map_matching_is_bounded_and_exact() {
        assert!(test_map_matches("crates/**", "crates/a/src/lib.rs"));
        assert!(test_map_matches("apps/", "apps/web/a.ts"));
        assert!(test_map_matches("Cargo.toml", "Cargo.toml"));
        assert!(!test_map_matches("Cargo.toml", "nested/Cargo.toml"));
        assert!(!test_map_matches("crates/**", "crate/a.rs"));
    }

    #[test]
    fn duplicate_missing_and_cyclic_lanes_fail_closed() {
        let duplicate = vec![
            simple_lane("static-security", vec![], vec!["static-security"]),
            simple_lane("static-security", vec![], vec!["other"]),
        ];
        assert!(validate_generated_lanes(
            &duplicate,
            Profile::Presubmit,
            false,
            false,
            false,
            false
        )
        .is_err());
        let missing = vec![simple_lane("other", vec![], vec!["other"])];
        assert!(
            validate_generated_lanes(&missing, Profile::Presubmit, false, false, false, false)
                .is_err()
        );
        let cycle = vec![
            simple_lane("static-security", vec!["second"], vec!["static-security"]),
            simple_lane("second", vec!["static-security"], vec!["other"]),
        ];
        assert!(
            validate_generated_lanes(&cycle, Profile::Presubmit, false, false, false, false)
                .is_err()
        );
    }

    #[test]
    fn release_lanes_reject_presubmit_cache_policy() {
        let lanes = vec![simple_lane(
            "required",
            vec![],
            vec![
                "required",
                "security",
                "complete-coverage",
                "contract-drift",
                "jankurai",
                "artifact",
                "compatibility",
                "conformance",
                "repository-specific",
            ],
        )];
        assert!(
            validate_generated_lanes(&lanes, Profile::ReleaseFull, false, false, false, false)
                .is_err()
        );
    }

    #[test]
    fn closed_plan_rejects_unknown_fields_profile_forgery_and_cache_poisoning() {
        let plan = validation_plan(Profile::ReleaseFull);
        validate_plan_value(&plan).unwrap();

        let mut unknown = plan.clone();
        unknown["caller_profile"] = json!("presubmit");
        assert!(validate_plan_value(&unknown).is_err());

        let mut forged = plan.clone();
        forged["profile"]["selected_by_uid"] = json!(1000);
        assert!(validate_plan_value(&forged).is_err());

        let mut activated = plan.clone();
        activated["execution"]["allowed"] = json!(true);
        activated["execution"]["activation_state"] = json!("active");
        assert!(validate_plan_value(&activated).is_err());

        let mut cache = plan.clone();
        cache["cache"] = json!({
            "mode": "content-addressed-local-only", "enabled": true,
            "root": "/var/cache/jain-ci/sccache", "max_bytes": SCCACHE_MAX_BYTES,
            "compiler_cache_forbidden_for_release": true
        });
        assert!(validate_plan_value(&cache).is_err());

        let mut missing_coverage = plan.clone();
        missing_coverage["lanes"][0]["obligations"]
            .as_array_mut()
            .unwrap()
            .retain(|value| value.as_str() != Some("complete-coverage"));
        assert!(validate_plan_value(&missing_coverage).is_err());

        let mut missing_output = plan.clone();
        missing_output["lanes"][0]["expected_outputs"]
            .as_array_mut()
            .unwrap()
            .pop();
        assert!(validate_plan_value(&missing_output).is_err());

        let mut unbound_program = plan.clone();
        unbound_program["lanes"][0]["program"] = json!("/bin/true");
        assert!(validate_plan_value(&unbound_program).is_err());

        let mut missing_changed_rust = validation_plan(Profile::Presubmit);
        missing_changed_rust["source_scope"]["rust_packages"] = json!(["fixture"]);
        assert!(validate_plan_value(&missing_changed_rust).is_err());

        let mut compiler_mismatch = plan;
        compiler_mismatch["executables"][1]["sha256"] = json!("short");
        assert!(validate_plan_value(&compiler_mismatch).is_err());
    }

    #[test]
    fn cargo_configuration_rejects_execution_and_credential_hooks() {
        let safe: toml::Value = "[net]\noffline = true\n".parse().unwrap();
        reject_unsafe_cargo_configuration(&safe, "").unwrap();
        for hostile in [
            "[build]\nrustc-wrapper = \"/tmp/exec\"\n",
            "[alias]\nci = \"run --bin hostile\"\n",
            "[source.crates-io]\nreplace-with = \"hostile\"\n",
            "[registry]\nglobal-credential-providers = [\"cargo:token\"]\n",
            "[target.x86_64-unknown-linux-gnu]\nrunner = \"/tmp/exec\"\n",
        ] {
            let value: toml::Value = hostile.parse().unwrap();
            assert!(reject_unsafe_cargo_configuration(&value, "").is_err());
        }
    }

    #[test]
    fn release_dag_runs_each_rust_test_configuration_once_with_coverage() {
        let raw: toml::Value = "name = \"fixture\"".parse().unwrap();
        let executable =
            |name: &str, path: &str| json!({"name": name, "path": path, "sha256": "a".repeat(64)});
        let executables = BTreeMap::from([
            ("bash".to_owned(), executable("bash", "/usr/bin/bash")),
            ("cargo".to_owned(), executable("cargo", "/usr/bin/cargo")),
            (
                "cargo-llvm-cov".to_owned(),
                executable("cargo-llvm-cov", "/usr/bin/cargo-llvm-cov"),
            ),
        ]);
        let lanes = release_full_lanes(
            "fixture",
            &raw,
            &[
                "Cargo.toml".to_owned(),
                "ops/ci/typed-required-non-test.sh".to_owned(),
            ],
            &executables,
        )
        .unwrap();
        assert_eq!(
            lanes
                .iter()
                .filter(|lane| lane["kind"] == "rust-test-and-coverage")
                .count(),
            1
        );
        assert!(!lanes.iter().any(|lane| lane["id"] == "required-product"));
        let obligations = lanes
            .iter()
            .flat_map(|lane| lane["obligations"].as_array().unwrap())
            .filter_map(JsonValue::as_str)
            .collect::<Vec<_>>();
        assert_eq!(
            obligations.iter().copied().collect::<BTreeSet<_>>().len(),
            obligations.len()
        );
        assert!(obligations.contains(&"complete-coverage"));
        assert!(obligations.contains(&"contract-drift"));
    }

    #[test]
    fn failure_cancels_only_dependents_and_preserves_logs() {
        let root = TestDir::new("dag-cancellation");
        let lane_root = root.path().join("lanes");
        let cargo_home = root.path().join("cargo-home");
        let target_dir = root.path().join("target");
        for directory in [&lane_root, &cargo_home, &target_dir] {
            fs::create_dir(directory).unwrap();
        }
        let context = RunContext {
            repo: root.path().to_path_buf(),
            lane_root: lane_root.clone(),
            cargo_home,
            target_dir,
            command_path: "/usr/bin:/bin".to_owned(),
            writable_root: None,
            profile: Profile::ReleaseFull,
            plan_sha256: "a".repeat(64),
            build_jobs: 2,
            sccache_path: None,
        };
        let lanes = vec![
            LaneSpec {
                id: "fails".to_owned(),
                kind: "test".to_owned(),
                program: PathBuf::from("/usr/bin/bash"),
                args: vec![
                    "-c".to_owned(),
                    "printf 'failure log\\n'; exit 7".to_owned(),
                ],
                dependencies: vec![],
                obligations: vec!["failure".to_owned()],
                timeout_seconds: 10,
            },
            LaneSpec {
                id: "dependent".to_owned(),
                kind: "test".to_owned(),
                program: PathBuf::from("/usr/bin/bash"),
                args: vec!["-c".to_owned(), "exit 0".to_owned()],
                dependencies: vec!["fails".to_owned()],
                obligations: vec!["dependent".to_owned()],
                timeout_seconds: 10,
            },
            LaneSpec {
                id: "independent".to_owned(),
                kind: "test".to_owned(),
                program: PathBuf::from("/usr/bin/bash"),
                args: vec!["-c".to_owned(), "printf 'independent log\\n'".to_owned()],
                dependencies: vec![],
                obligations: vec!["independent".to_owned()],
                timeout_seconds: 10,
            },
        ];
        let results = run_dag(&lanes, &context, 2).unwrap();
        assert_eq!(results["fails"]["status"], "fail");
        assert_eq!(results["dependent"]["status"], "canceled");
        assert_eq!(results["dependent"]["reason"], "dependency-failed");
        assert_eq!(results["independent"]["status"], "pass");
        assert!(fs::read_to_string(lane_root.join("fails.log"))
            .unwrap()
            .contains("failure log"));
        assert!(fs::read_to_string(lane_root.join("dependent.log"))
            .unwrap()
            .contains("canceled"));
        assert!(run_dag(&lanes, &context, 2).is_err());
    }

    #[test]
    fn malformed_timing_data_is_rejected() {
        let root = TestDir::new("malformed-timing");
        let path = root.path().join("time");
        fs::write(&path, "wall_seconds=NaN\n").unwrap();
        assert!(parse_time_metrics(&path, 1).is_err());
    }

    #[test]
    fn host_result_requires_broker_seal_and_continuous_aggregate_metrics() {
        let root = TestDir::new("host-result");
        let log_path = root.path().join("lane.log");
        fs::write(&log_path, "pass\n").unwrap();
        let plan_sha256 = "a".repeat(64);
        let lane_result = json!({
            "schema_version": LANE_RESULT_SCHEMA,
            "plan_sha256": plan_sha256,
            "lane_id": "required",
            "kind": "required",
            "status": "pass",
            "reason": null,
            "started_at_unix_ms": 1000,
            "finished_at_unix_ms": 1001,
            "duration_ms": 1,
            "exit_code": 0,
            "log_path": log_path,
            "log_sha256": sha256_regular_file(&log_path, "test log").unwrap(),
            "command_sha256": "b".repeat(64),
            "dependencies": [],
            "obligations": ["required"],
            "metrics": {
                "wall_ms": 1,
                "cpu_user_ms": 0,
                "cpu_system_ms": 0,
                "peak_rss_bytes": 1,
                "fs_inputs": 0,
                "fs_outputs": 0
            }
        });
        let lane_results = vec![lane_result];
        let result_set_sha256 = format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&lane_results).unwrap())
        );
        let metrics = json!({
            "aggregate_peak_rss_bytes": 1,
            "cache_hit_rate": null,
            "cache_hits": null,
            "cache_requests": null,
            "cpu_system_ms": 0,
            "cpu_user_ms": 0,
            "critical_path_ms": 1,
            "io_wait_percent": 0.0,
            "load1_max": 1.0,
            "measurement_sample_count": 2,
            "measurement_source": MEASUREMENT_SOURCE,
            "minimum_available_memory_bytes": MIN_AVAILABLE_MEMORY_BYTES,
            "oom_kill_delta": 0,
            "swap_in_delta_pages": 0
        });
        let metrics_sha256 = format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&metrics).unwrap())
        );
        let mut run = json!({
            "schema_version": RUN_SCHEMA,
            "plan_schema_version": PLAN_SCHEMA,
            "plan_sha256": plan_sha256,
            "repository": "fixture",
            "head_sha": "c".repeat(40),
            "head_tree": "d".repeat(40),
            "base_sha": "e".repeat(40),
            "authority_sha256": "f".repeat(64),
            "configuration": {"build_jobs": 4, "fleet_concurrency": 2, "calibration": true},
            "profile": "release-full",
            "started_at_unix_ms": 1000,
            "finished_at_unix_ms": 1001,
            "duration_ms": 1,
            "status": "pass",
            "publication_allowed": false,
            "publication_mode": "shadow-equivalence",
            "lane_results": lane_results,
            "result_set_sha256": result_set_sha256,
            "metrics": metrics,
            "host_evidence": {
                "schema_version": HOST_EVIDENCE_SCHEMA,
                "plan_sha256": plan_sha256,
                "lane_result_schema_version": LANE_RESULT_SCHEMA,
                "result_set_sha256": result_set_sha256,
                "metrics_sha256": metrics_sha256,
                "profile": "release-full",
                "measurement_source": MEASUREMENT_SOURCE,
                "publication_allowed": false
            },
            "broker_seal": {
                "boundary": EXECUTION_BOUNDARY,
                "evidence_sealer": EVIDENCE_SEALER,
                "fleet_lease_id": "1".repeat(64),
                "fleet_slot": 0,
                "fleet_slot_count": 2,
                "measurement_source": MEASUREMENT_SOURCE,
                "plan_path": "/run/jain-ci/plan.json",
                "plan_sha256": plan_sha256,
                "sealed_by_uid": 0,
                "source_postcondition": SOURCE_POSTCONDITION,
                "tool_manifest_path": "/run/jain-ci/tools.json",
                "tool_manifest_sha256": "2".repeat(64)
            },
            "sealed": true
        });
        validate_run_receipt(&run).unwrap();
        run["metrics"]["measurement_sample_count"] = json!(1);
        assert!(validate_run_receipt(&run).is_err());
        run["metrics"]["measurement_sample_count"] = json!(2);
        run.as_object_mut().unwrap().remove("broker_seal");
        assert!(validate_run_receipt(&run).is_err());
    }

    #[test]
    fn nearest_rank_p95_and_health_thresholds_are_exact() {
        let samples = (1..=20).map(|value| (value * 10, true)).collect::<Vec<_>>();
        let profile = performance_profile(samples, 190);
        assert_eq!(profile["p95_ms"], 190);
        assert_eq!(profile["status"], "green");
        let invalid = performance_profile(vec![(1, false); 20], 190);
        assert_eq!(invalid["status"], "red");
        assert_eq!(nearest_rank_p95(&[1, 2, 3, 4, 5]), Some(5));
    }

    #[test]
    fn calibration_requires_representative_repositories_and_five_percent_gain() {
        let repositories = [
            "jain-split-ops",
            "jain-web",
            "jain-smartcluster",
            "redline-core",
        ];
        let mut samples = Vec::new();
        for (build_jobs, duration) in [(4, 1000), (8, 940), (16, 920)] {
            for repository in repositories {
                samples.push(PerformanceSample {
                    profile: Profile::ReleaseFull,
                    repository: repository.to_owned(),
                    duration_ms: duration,
                    valid: true,
                    build_jobs,
                    fleet_concurrency: 2,
                    calibration: true,
                });
            }
        }
        let report = calibration_report(&samples).unwrap();
        validate_calibration_report(&report).unwrap();
        assert_eq!(report["candidates"][0]["setting_valid"], false);
        assert!(report["candidates"][0]["improvement_percent"].is_null());
        assert_eq!(report["selected"]["build_jobs"], 8);
        assert_eq!(report["selected"]["fleet_concurrency"], 2);
        assert_eq!(report["selected"]["p95_ms"], 940);
        assert_eq!(report["candidates"][2]["setting_valid"], false);
        let mut forged = report;
        forged["candidates"][0]["setting_valid"] = json!(true);
        assert!(validate_calibration_report(&forged).is_err());
    }

    #[test]
    fn published_contracts_match_runtime_shape() {
        for (bytes, schema) in [
            (
                include_str!("../../../contracts/ci-plan.schema.json"),
                PLAN_SCHEMA,
            ),
            (
                include_str!("../../../contracts/ci-lane-result.schema.json"),
                LANE_RESULT_SCHEMA,
            ),
            (
                include_str!("../../../contracts/ci-performance.schema.json"),
                PERFORMANCE_SCHEMA,
            ),
            (
                include_str!("../../../contracts/host-ci-result.schema.json"),
                RUN_SCHEMA,
            ),
            (
                include_str!("../../../contracts/host-ci-evidence.schema.json"),
                HOST_EVIDENCE_SCHEMA,
            ),
        ] {
            let value: JsonValue = serde_json::from_str(bytes).unwrap();
            assert_eq!(value["additionalProperties"], false);
            let encoded = serde_json::to_string(&value).unwrap();
            assert!(encoded.contains(schema));
        }
        let plan_schema: JsonValue =
            serde_json::from_str(include_str!("../../../contracts/ci-plan.schema.json")).unwrap();
        let required = plan_schema["required"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(JsonValue::as_str)
            .collect::<BTreeSet<_>>();
        for field in [
            "cargo_configuration",
            "execution",
            "lanes",
            "profile",
            "resources",
            "source_scope",
        ] {
            assert!(required.contains(field));
        }
        assert_eq!(plan_schema["$defs"]["stringSet"]["maxItems"], 128);
        assert_eq!(plan_schema["$defs"]["packageSet"]["maxItems"], 1024);
        assert_eq!(
            plan_schema["$defs"]["name"]["pattern"],
            "^[a-z0-9][a-z0-9-]{0,95}$"
        );
        assert_eq!(
            plan_schema["$defs"]["lane"]["properties"]["id"]["pattern"],
            "^[a-z0-9][a-z0-9-]{0,95}$"
        );
        let lane_result_schema: JsonValue = serde_json::from_str(include_str!(
            "../../../contracts/ci-lane-result.schema.json"
        ))
        .unwrap();
        assert_eq!(
            lane_result_schema["properties"]["lane_id"]["pattern"],
            "^[a-z0-9][a-z0-9-]{0,95}$"
        );
        assert_eq!(
            safe_id(&"a".repeat(MAX_CANONICAL_ID)),
            "a".repeat(MAX_CANONICAL_ID)
        );
        assert_ne!(
            safe_id(&"a".repeat(MAX_CANONICAL_ID + 1)),
            "a".repeat(MAX_CANONICAL_ID + 1)
        );
        assert_eq!(
            plan_schema["$defs"]["execution"]["properties"]["allowed"]["const"],
            false
        );
        let host_schema: JsonValue = serde_json::from_str(include_str!(
            "../../../contracts/host-ci-result.schema.json"
        ))
        .unwrap();
        assert!(host_schema["required"]
            .as_array()
            .unwrap()
            .contains(&json!("broker_seal")));
        assert_eq!(
            host_schema["$defs"]["metrics"]["properties"]["measurement_source"]["const"],
            MEASUREMENT_SOURCE
        );
        assert_eq!(
            host_schema["$defs"]["metrics"]["properties"]["measurement_sample_count"]["minimum"],
            2
        );
        let packages = (0..1024)
            .map(|index| json!(format!("package-{index}")))
            .collect::<Vec<_>>();
        assert_eq!(
            bounded_string_array(&json!(packages), "packages", 1024)
                .unwrap()
                .len(),
            1024
        );
        assert!(bounded_string_array(
            &json!((0..1025)
                .map(|index| format!("package-{index}"))
                .collect::<Vec<_>>()),
            "packages",
            1024
        )
        .is_err());
    }

    #[test]
    fn unsafe_paths_and_profile_names_are_rejected() {
        for path in ["../escape", "/absolute", "a/../b", "bad\npath"] {
            assert!(validate_relative_path(path).is_err());
        }
        assert!(Profile::parse("release").is_err());
        assert!(Profile::parse("Release-Full").is_err());
    }
}
