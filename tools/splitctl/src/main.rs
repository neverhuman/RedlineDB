// Repository-local release and Jeryu control-plane CLI.
mod jeryu_client;

use jeryu_client::{write_token_for_askpass, HostCiPublication, JeryuClient, JeryuRequest};
use serde_json::{json, Map, Value as JsonValue};
use sha2::{Digest, Sha256};
use std::{
    env,
    ffi::OsStr,
    fs,
    io::{self, Read, Write},
    os::{
        fd::AsRawFd,
        unix::fs::{MetadataExt, OpenOptionsExt},
    },
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

const RELEASE_VERSION: &str = "8.0.0";
const LOCAL_JERYU_ORIGIN: &str = "http://127.0.0.1:8787";
const FAMILY_REMOTE_PREFIX: &str = "http://127.0.0.1:8787/git/jeryu/";
const INFRA_REMOTE_PREFIX: &str = "http://127.0.0.1:8787/git/jain-split/";
const JERYU_ASKPASS_MODE: &str = "JAIN_SPLITCTL_JERYU_ASKPASS";
const JERYU_ASKPASS_TOKEN_FILE: &str = "JAIN_SPLITCTL_JERYU_TOKEN_FILE";
const JERYU_GIT_USERNAME: &str = "x-access-token";

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
        Some("validate-local-jeryu") => {
            let mut manifest = None;
            let mut skip_remotes = false;
            while let Some(arg) = args.next() {
                match arg.as_str() {
                    "--manifest" => manifest = Some(PathBuf::from(args.next().ok_or("--manifest needs a path")?)),
                    "--skip-remotes" => skip_remotes = true,
                    value => return Err(format!("unknown argument: {value}").into()),
                }
            }
            validate_local_jeryu(manifest, skip_remotes)?;
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
            validate_local_jeryu(manifest, skip_remotes)?;
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
        Some("release-cargo-commands") => release_cargo_commands_command(args.collect())?,
        Some("sync-derived-manifests") => sync_derived_manifests_command(args.collect())?,
        Some("jankurai-evidence") => jankurai_evidence_command(args.collect())?,
        Some("validate-manifest") => validate_manifest_command(args.collect())?,
        Some("validate-family") => preflight(args.collect())?,
        Some("validate-family-lock") => validate_family_lock(args.collect())?,
        Some("regenerate-lock") => regenerate_lock(args.collect())?,
        Some("release-preflight") => release_preflight(args.collect())?,
        Some("release-snapshot") => release_snapshot(args.collect())?,
        Some("release-status") => release_status(args.collect())?,
        Some("bootstrap-main") => bootstrap_main_command(args.collect())?,
        Some("immutable-tag") => immutable_tag_command(args.collect())?,
        Some("verify-worktrees") => verify_worktrees_command(args.collect())?,
        Some("reconcile") => reconcile(args.collect())?,
        Some("bump-version") => bump_version(args.collect())?,
        Some("--version") | Some("version") => println!("splitctl 0.1.0"),
        _ => return Err("usage: splitctl refresh-ci-contract [--repo NAME]... | materialize [--repo NAME]... | host-ci-snapshot-request --source PATH --destination PATH --expected-uid UID --expected-gid GID --max-bytes BYTES | manifest [--manifest PATH] [--json] | managed-repos [--manifest PATH] --json | release-cargo-commands [--manifest PATH] --repo NAME | sync-derived-manifests [--manifest PATH] [--receipt PATH] [--apply] | jankurai-evidence --repository NAME --commit SHA --worktree PATH --report-root PATH --report PATH --auditor PATH --attempt-id ID --lane-conclusion success|failure [--lane-failure-reason REASON] --clean-tracked-tree-start BOOL --receipt PATH | validate-manifest [--manifest PATH] [--check-paths] [--check-derived] | validate-local-jeryu [--manifest PATH] [--skip-remotes] | validate-family [--manifest PATH] [--json PATH] | validate-family-lock [--manifest PATH] [--lock PATH] | regenerate-lock [--manifest PATH] [--output PATH] --apply | release-preflight [--manifest PATH] [--json PATH] | release-snapshot [--manifest PATH] [--json PATH] | release-status [--manifest PATH] [--json PATH] | bootstrap-main --repo PATH --remote URL --reviewed-commit SHA [--receipt PATH] [--apply] | immutable-tag --repo PATH --remote URL --tag TAG --commit SHA [--receipt PATH] [--apply] | verify-worktrees [--manifest PATH] [--receipt PATH] | preflight [--manifest PATH] [--json PATH] | source-coverage [--manifest PATH] [--json] | python-boundary | jeryu-doctor [--manifest PATH] | reconcile [--manifest PATH] [--base-ref REF] [--apply] [--json PATH] | bump-version [--manifest PATH] --from VERSION --new VERSION --rewrite-split-tags".into()),
    }
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
            "current_tag",
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
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
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
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "schema_version": "jain.managed-repositories/v1",
            "manifest": manifest,
            "repositories": repos.iter().map(managed_repo_json).collect::<Vec<_>>(),
            "repository_count": repos.len(),
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
    })
}

fn release_cargo_commands_command(args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut manifest = root.join("repos.manifest.toml");
    let mut repo_name = None;
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--manifest" => manifest = PathBuf::from(iter.next().ok_or("--manifest needs a path")?),
            "--repo" => repo_name = Some(iter.next().ok_or("--repo needs a name")?),
            value => return Err(format!("unknown release-cargo-commands argument: {value}").into()),
        }
    }
    let repo_name = repo_name.ok_or("release-cargo-commands requires --repo")?;
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
    Err(format!(
        "repository {repo_name} is not declared by the canonical manifest"
    ))
}

fn release_cargo_policy(repo_name: &str, raw: &toml::Value) -> Result<JsonValue, String> {
    let Some(matrix) = release_feature_matrix(raw)? else {
        return Ok(json!({
            "schema_version": "jain.split.release-cargo-commands/v1",
            "repo": repo_name,
            "mode": "all-features",
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
        "release_package": matrix.package,
        "release_feature_sets": matrix.feature_sets,
        "commands": commands,
    }))
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

fn managed_repositories(
    data: &toml::Value,
    manifest: &Path,
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
            tag: string(raw, "immutable_tag").or_else(|| string(raw, "current_tag")),
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
        });
    }

    let control = data
        .get("control_plane")
        .ok_or("manifest is missing its control_plane")?;
    let control_name = string(control, "name").ok_or("control plane is missing its name")?;
    let release = string(data, "release_version").unwrap_or_else(|| RELEASE_VERSION.to_owned());
    managed.push(ManagedRepo {
        name: control_name.clone(),
        path: PathBuf::from(string(control, "path").ok_or("control plane is missing its path")?),
        remote: declared_remote(control).ok_or("control plane is missing its remote")?,
        required_check: string(control, "required_check")
            .ok_or("control plane is missing its required check")?,
        branch: string(control, "branch").unwrap_or_else(|| "main".to_owned()),
        tag: string(control, "immutable_tag")
            .or_else(|| string(control, "current_tag"))
            .or_else(|| Some(format!("{control_name}-v{release}-split.0"))),
        kind: "control-plane".to_owned(),
        family: family.clone(),
        family_registered: true,
    });

    if let Some(nested_path) = declared_nested_manifest_path(data, manifest) {
        let nested: toml::Value = fs::read_to_string(&nested_path)?.parse()?;
        let nested_family = string(&nested, "family").ok_or("nested manifest is missing family")?;
        let nested_dir = nested_path.parent().unwrap_or(Path::new("."));
        for raw in nested
            .get("repo")
            .and_then(toml::Value::as_array)
            .into_iter()
            .flatten()
        {
            let name = string(raw, "name").ok_or("nested repository is missing its name")?;
            let raw_path =
                PathBuf::from(string(raw, "path").ok_or("nested repository is missing its path")?);
            let resolved_path = if raw_path.is_absolute() {
                raw_path
            } else {
                nested_dir.join(raw_path)
            };
            managed.push(ManagedRepo {
                name,
                path: fs::canonicalize(&resolved_path).unwrap_or(resolved_path),
                remote: declared_remote(raw).ok_or("nested repository is missing its remote")?,
                required_check: string(raw, "required_check")
                    .ok_or("nested repository is missing its required check")?,
                branch: string(raw, "default_branch").unwrap_or_else(|| "main".to_owned()),
                tag: string(raw, "immutable_tag").or_else(|| string(raw, "current_tag")),
                kind: "nested-family".to_owned(),
                family: nested_family.clone(),
                family_registered: true,
            });
        }
        let nested_control = nested
            .get("control_plane")
            .ok_or("nested manifest is missing its control_plane")?;
        let nested_declaration = data
            .get("nested_families")
            .and_then(|value| value.get("redline"))
            .ok_or("manifest is missing nested_families.redline")?;
        let nested_control_name =
            string(nested_control, "name").ok_or("nested control plane is missing its name")?;
        managed.push(ManagedRepo {
            name: nested_control_name,
            path: PathBuf::from(
                string(nested_declaration, "control_plane")
                    .ok_or("nested control plane path is not declared")?,
            ),
            remote: declared_remote(nested_control)
                .ok_or("nested control plane is missing its remote")?,
            required_check: string(nested_control, "required_check")
                .ok_or("nested control plane is missing its required check")?,
            branch: string(nested_control, "branch").unwrap_or_else(|| "main".to_owned()),
            tag: string(nested_control, "immutable_tag")
                .or_else(|| string(nested_control, "current_tag")),
            kind: "nested-control-plane".to_owned(),
            family: nested_family,
            family_registered: true,
        });
    }

    let mut names = std::collections::BTreeSet::new();
    let mut paths = std::collections::BTreeSet::new();
    for repo in &managed {
        if !names.insert(repo.name.clone()) {
            return Err(format!("duplicate managed repository name: {}", repo.name).into());
        }
        if !paths.insert(repo.path.clone()) {
            return Err(
                format!("duplicate managed repository path: {}", repo.path.display()).into(),
            );
        }
    }
    Ok(managed)
}

fn declared_nested_manifest_path(data: &toml::Value, manifest: &Path) -> Option<PathBuf> {
    let raw = data
        .get("nested_families")?
        .get("redline")?
        .get("manifest_path")?
        .as_str()?;
    let path = PathBuf::from(raw);
    Some(if path.is_absolute() {
        path
    } else {
        manifest.parent().unwrap_or(Path::new(".")).join(path)
    })
}

fn sync_derived_manifests_command(args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut manifest = root.join("repos.manifest.toml");
    let mut receipt = None;
    let mut apply = false;
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--manifest" => manifest = PathBuf::from(iter.next().ok_or("--manifest needs a path")?),
            "--receipt" => {
                receipt = Some(PathBuf::from(iter.next().ok_or("--receipt needs a path")?))
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
        for (target, path) in derived_manifest_targets(&data, &manifest)? {
            let rendered = render_derived_manifest(&data, &manifest, &target, &canonical_hash)?;
            let expected_sha256 = sha256_bytes(rendered.as_bytes());
            let current = fs::read(&path).ok();
            let current_sha256 = current.as_deref().map(sha256_bytes);
            let changed = current.as_deref() != Some(rendered.as_bytes());
            if apply && changed {
                write_atomic_bytes(&path, rendered.as_bytes())?;
            }
            rows.push(json!({
                "target": target,
                "path": path,
                "changed": changed,
                "current_sha256": current_sha256,
                "expected_sha256": expected_sha256,
                "action": if apply && changed {"updated"} else if changed {"would-update"} else {"verified"},
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
    let auditor_output = Command::new(&auditor).arg("--version").output()?;
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
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--manifest" => path = PathBuf::from(iter.next().ok_or("--manifest needs a path")?),
            "--check-paths" => check_paths = true,
            "--check-derived" => check_derived = true,
            value => return Err(format!("unknown validate-manifest argument: {value}").into()),
        }
    }
    let data: toml::Value = fs::read_to_string(&path)?.parse()?;
    validate_manifest_data(&data, &path, check_paths)?;
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
            "current_tag",
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
        if string(raw, "current_tag").as_deref()
            != Some(format!("{name}-v{RELEASE_VERSION}-split.0").as_str())
        {
            errors.push(format!(
                "{name}: current_tag must be {name}-v{RELEASE_VERSION}-split.0"
            ));
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
            ("forge_owner", "jain-split"),
            ("forge_slug", "jain-split/jain-smartcluster"),
            ("required_check", "jain-smartcluster/required"),
            ("default_branch", "main"),
            ("immutable_tag", "jain-smartcluster-v8.0.0-split.0"),
        ] {
            if string(raw, key).as_deref() != Some(expected) {
                errors.push(format!("jain-smartcluster: {key} must be {expected}"));
            }
        }
        let expected_infra_remote = format!("{INFRA_REMOTE_PREFIX}jain-smartcluster.git");
        if declared_remote(raw).as_deref() != Some(expected_infra_remote.as_str()) {
            errors.push("jain-smartcluster: remote must use the jain-split namespace".to_owned());
        }
        if raw.get("family_registered").and_then(toml::Value::as_bool) != Some(true) {
            errors.push("jain-smartcluster: family_registered must be true".to_owned());
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
    let redline = data
        .get("external_dependencies")
        .and_then(|value| value.get("redline"))
        .ok_or("manifest must declare external_dependencies.redline")?;
    if string(redline, "immutable_tag").as_deref() != Some("redline-core-v4.1.0-jain.1")
        || string(redline, "remote").as_deref()
            != Some("http://127.0.0.1:8787/git/jeryu/redline-core.git")
    {
        errors.push("redline dependency must use the immutable local-Jeryu v4.1.0 tag".to_owned());
    }
    let nested = data
        .get("nested_families")
        .and_then(|value| value.get("redline"))
        .ok_or("manifest must declare nested_families.redline")?;
    for (key, expected) in [
        ("family", "redline-split"),
        (
            "manifest_path",
            "/home/ubuntu/jain-split/redline-split-ops/repos.manifest.toml",
        ),
        ("container_path", "/home/ubuntu/jain-split/redline-split"),
        ("control_plane", "/home/ubuntu/jain-split/redline-split-ops"),
        ("engine_repository", "redline-core"),
        (
            "engine_remote",
            "http://127.0.0.1:8787/git/jeryu/redline-core.git",
        ),
        ("engine_tag", "redline-core-v4.1.0-jain.1"),
    ] {
        if string(nested, key).as_deref() != Some(expected) {
            errors.push(format!("nested_families.redline.{key} must be {expected}"));
        }
    }
    if nested.get("required").and_then(toml::Value::as_bool) != Some(true) {
        errors.push("nested_families.redline.required must be true".to_owned());
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
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
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
        errors.push("lock release is not 8.0.0-split.0".to_owned());
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
        let tag = string(raw, "immutable_tag").or_else(|| string(raw, "current_tag"));
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
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
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
        let tag = string(raw, "immutable_tag")
            .or_else(|| string(raw, "current_tag"))
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
    if let Some(redline) = data
        .get("external_dependencies")
        .and_then(|value| value.get("redline"))
    {
        text.push_str(&format!(
            "[nested.redline]\nfamily = \"redline-split\"\nremote = \"{}\"\ntag = \"{}\"\n\n",
            string(redline, "remote").unwrap_or_default(),
            string(redline, "immutable_tag").unwrap_or_default()
        ));
    }
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&output, text)?;
    println!("regenerated family lock: {}", output.display());
    Ok(())
}

fn release_preflight(args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    preflight(args)
}

fn release_snapshot(args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
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

fn release_status(args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut manifest = root.join("repos.manifest.toml");
    let mut output = None;
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--manifest" => manifest = PathBuf::from(iter.next().ok_or("--manifest needs a path")?),
            "--json" => output = Some(PathBuf::from(iter.next().ok_or("--json needs a path")?)),
            value => return Err(format!("unknown release-status argument: {value}").into()),
        }
    }
    let data: toml::Value = fs::read_to_string(&manifest)?.parse()?;
    let report = json!({
        "schema_version": "jain.release.status/v1",
        "release": RELEASE_VERSION,
        "status": "candidate",
        "formal_ga": false,
        "manifest_sha256": manifest_sha256(&manifest)?,
        "family_repo_count": family_repos(&data)?.len(),
        "infrastructure_repo_count": data.get("infrastructure_repo").and_then(toml::Value::as_array).map_or(0, Vec::len),
        "reason": "GA requires a clean reviewed snapshot, immutable tags, green CI, artifact evidence, and production promotion receipts",
    });
    if let Some(path) = output {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, serde_json::to_vec_pretty(&report)?)?;
        println!("wrote {}", path.display());
    } else {
        println!("{}", serde_json::to_string_pretty(&report)?);
    }
    Ok(())
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
        let tag = string(raw, "immutable_tag")
            .or_else(|| string(raw, "current_tag"))
            .unwrap_or_default();
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
    let mut repo = None;
    let mut remote = None;
    let mut tag = None;
    let mut commit = None;
    let mut receipt = None;
    let mut apply = false;
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--repo" => repo = Some(PathBuf::from(iter.next().ok_or("--repo needs a path")?)),
            "--remote" => remote = Some(iter.next().ok_or("--remote needs a URL")?),
            "--tag" => tag = Some(iter.next().ok_or("--tag needs a name")?),
            "--commit" => commit = Some(iter.next().ok_or("--commit needs a SHA")?),
            "--receipt" => {
                receipt = Some(PathBuf::from(iter.next().ok_or("--receipt needs a path")?))
            }
            "--apply" => apply = true,
            value => return Err(format!("unknown immutable-tag argument: {value}").into()),
        }
    }
    let repo = repo.ok_or("immutable-tag requires --repo")?;
    let remote = remote.ok_or("immutable-tag requires --remote")?;
    let tag = tag.ok_or("immutable-tag requires --tag")?;
    let commit = commit.ok_or("immutable-tag requires --commit")?;
    let receipt = match receipt {
        Some(path) => path,
        None => release_evidence_path(&format!("immutable-tag-{}.json", receipt_component(&tag))),
    };
    let mut report = receipt_header("jain.immutable-tag/v1", "immutable-tag", apply);
    report["repository"] = json!(repo);
    report["remote"] = json!(remote);
    report["tag"] = json!(tag);
    report["commit_input"] = json!(commit);
    let result = create_or_verify_immutable_tag(&repo, &remote, &tag, &commit, apply, &mut report);
    finish_receipted_operation(&receipt, &mut report, result)
}

fn create_or_verify_immutable_tag(
    repo: &Path,
    remote: &str,
    tag: &str,
    commit: &str,
    apply: bool,
    report: &mut JsonValue,
) -> Result<(), Box<dyn std::error::Error>> {
    let tag_ref = format!("refs/tags/{tag}");
    run_git_strict(repo, &["check-ref-format", &tag_ref])?;
    let reviewed = resolve_commit(repo, commit)?;
    report["commit"] = json!(reviewed);
    let remote_main = ls_remote_ref(repo, remote, "refs/heads/main")?;
    report["remote_main"] = json!(remote_main);
    if remote_main.as_deref() != Some(reviewed.as_str()) {
        report["action"] = json!("refused-non-main-tag");
        return Err(format!(
            "refusing to tag {reviewed}: remote main resolves to {}",
            remote_main.as_deref().unwrap_or("<absent>")
        )
        .into());
    }
    let local_before = local_ref_commit(repo, &tag_ref)?;
    let remote_before = ls_remote_ref(repo, remote, &tag_ref)?;
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
    if local_before.is_none() {
        run_git_strict(
            repo,
            &[
                "update-ref",
                &tag_ref,
                &reviewed,
                "0000000000000000000000000000000000000000",
            ],
        )?;
    }
    if remote_before.is_none() {
        let lease = format!("--force-with-lease={tag_ref}:");
        let refspec = format!("{tag_ref}:{tag_ref}");
        run_git_strict(repo, &["push", "--porcelain", &lease, remote, &refspec])?;
    }
    let local_after = local_ref_commit(repo, &tag_ref)?;
    let remote_after = ls_remote_ref(repo, remote, &tag_ref)?;
    report["action"] = json!(if local_before.is_some() && remote_before.is_some() {
        "verified-existing"
    } else {
        "created-and-verified"
    });
    report["after"] = json!({"local": local_after, "remote": remote_after});
    if local_after.as_deref() == Some(reviewed.as_str())
        && remote_after.as_deref() == Some(reviewed.as_str())
    {
        Ok(())
    } else {
        Err("immutable tag verification did not resolve to the reviewed commit".into())
    }
}

fn verify_worktrees_command(args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
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
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("docs/release-evidence/8.0.0")
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
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
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
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
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

fn nested_manifest_path(manifest: &Path) -> Option<PathBuf> {
    manifest
        .parent()
        .map(|parent| parent.join("../redline-split-ops/repos.manifest.toml"))
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

fn secure_git_authenticated_command(
    repo: Option<&Path>,
    token_file: &Path,
) -> Result<AuthenticatedGitCommand, Box<dyn std::error::Error>> {
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
    // pins these exact running bytes across Git's child chain until it invokes askpass.
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
    let executable_path = format!("/proc/self/fd/{descriptor}");
    let mut command = secure_git_command(repo);
    command
        .env("GIT_ASKPASS", &executable_path)
        .env(JERYU_ASKPASS_MODE, "v1")
        .env(JERYU_ASKPASS_TOKEN_FILE, token_file)
        .args([
            "-c",
            "credential.username=x-access-token",
            "-c",
            "credential.useHttpPath=false",
            "-c",
            "http.followRedirects=false",
            "-c",
            "http.maxRequests=1",
        ]);
    Ok(AuthenticatedGitCommand {
        command,
        _askpass_executable: executable,
    })
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

fn validate_physical_git_checkout(path: &Path) -> Result<PathBuf, Box<dyn std::error::Error>> {
    if !path.is_absolute() {
        return Err("--repo-path must be absolute".into());
    }
    let canonical = fs::canonicalize(path)?;
    if canonical != path {
        return Err("--repo-path must already be canonical".into());
    }
    let split_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("splitctl root has no parent")?;
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
            if !matches!(name, "core" | "remote" | "branch" | "user") {
                return Err(format!(
                    "local Git configuration section is forbidden for branch publication: {name}"
                )
                .into());
            }
            section = Some(name.to_owned());
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

fn jeryu_branch_push(args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
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
    let path =
        validate_physical_git_checkout(&repo_path.ok_or("branch-push requires --repo-path")?)?;
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
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
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
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
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

fn source_coverage(manifest: &Path, json_output: bool) -> Result<(), Box<dyn std::error::Error>> {
    let data: toml::Value = fs::read_to_string(manifest)?.parse()?;
    let source_root =
        PathBuf::from(string(&data, "source_root").ok_or("manifest missing source_root")?);
    let source_sha = string(&data, "source_sha").ok_or("manifest missing source_sha")?;
    let files = git_files(&source_root, &source_sha)?;
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

fn python_boundary() -> Result<(), Box<dyn std::error::Error>> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
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
            || rel.starts_with("jain-python/python/ai-service/tests/");
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
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
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
            if string(raw, "forge_owner").as_deref() != Some("jain-split") {
                failures.push("infrastructure forge_owner must be jain-split".to_owned());
            }
            if string(raw, "forge_slug").as_deref() != Some("jain-split/jain-smartcluster") {
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

        let tag = string(raw, "immutable_tag")
            .or_else(|| string(raw, "current_tag"))
            .unwrap_or_default();
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
    if string(&data, "release_version").as_deref() != Some("8.0.0") {
        manifest_failures.push("release_version must be 8.0.0".to_owned());
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
    let redline = data
        .get("external_dependencies")
        .and_then(|value| value.get("redline"));
    let nested = data
        .get("nested_families")
        .and_then(|value| value.get("redline"));
    let Some(redline) = redline else {
        return vec!["missing external_dependencies.redline".to_owned()];
    };
    let tag = string(redline, "immutable_tag").unwrap_or_default();
    let remote = string(redline, "remote").unwrap_or_default();
    let core_path = nested
        .and_then(|value| string(value, "container_path"))
        .map(|path| PathBuf::from(path).join("redline-core"));
    let Some(core_path) = core_path else {
        return vec!["nested Redline core path is not declared".to_owned()];
    };
    if !core_path.join(".git").exists() {
        failures.push(format!(
            "redline-core checkout missing at {}",
            core_path.display()
        ));
        return failures;
    }
    if git_query(&core_path, &["remote", "get-url", "origin"]).as_deref() != Some(remote.as_str()) {
        failures.push(format!("redline-core origin must be {remote}"));
    }
    let tag_ref = format!("refs/tags/{tag}^{{}}");
    let local_commit = git_query(&core_path, &["rev-parse", &tag_ref]);
    if local_commit.is_none() {
        failures.push(format!(
            "redline-core immutable tag {tag} is absent locally"
        ));
    }
    let remote_tag = git_query(&core_path, &["ls-remote", "origin", &tag_ref]);
    if remote_tag.is_none() {
        failures.push(format!(
            "redline-core immutable tag {tag} is absent from Jeryu"
        ));
    }
    let lock_path = core_path
        .parent()
        .and_then(|parent| parent.parent())
        .map(|root| root.join("redline-split-ops/redline.lock.toml"));
    if let Some(lock_path) = lock_path.filter(|path| path.is_file()) {
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
                failures.push("Redline proof lock is not cutover_eligible".to_owned());
            }
            if let (Some(expected), Some(actual)) = (string(&lock, "engine_commit"), local_commit) {
                if expected != actual {
                    failures.push(format!(
                        "redline-core tag commit {actual} differs from lock {expected}"
                    ));
                }
            }
        } else {
            failures.push(format!(
                "unable to parse Redline lock {}",
                lock_path.display()
            ));
        }
    } else {
        failures.push("Redline family lock is missing".to_owned());
    }
    failures
}

fn git_query(root: &Path, args: &[&str]) -> Option<String> {
    git_query_with_status(root, args).filter(|value| !value.is_empty())
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
            .is_some_and(|name| {
                matches!(
                    name,
                    ".git" | "target" | ".stage" | ".venv" | "vendor" | "node_modules"
                )
            })
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
    let output = Command::new("git")
        .args([
            "-C",
            root.to_str().ok_or("source root is not UTF-8")?,
            "ls-tree",
            "-r",
            "--name-only",
            sha,
        ])
        .output()?;
    if !output.status.success() {
        return Err(format!("git ls-tree failed for {sha}").into());
    }
    Ok(String::from_utf8(output.stdout)?
        .lines()
        .filter(|line| !line.is_empty())
        .map(ToOwned::to_owned)
        .collect())
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
) -> Result<(), Box<dyn std::error::Error>> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
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
    if let Some(nested_path) = nested_manifest_path(&manifest_path) {
        if nested_path.is_file() {
            let nested: toml::Value = fs::read_to_string(&nested_path)?.parse()?;
            for raw in nested
                .get("repo")
                .and_then(toml::Value::as_array)
                .into_iter()
                .flatten()
            {
                let mut repo = repo_from(raw)?;
                if repo.path.is_relative() {
                    repo.path = nested_path
                        .parent()
                        .unwrap_or(Path::new("."))
                        .join(&repo.path);
                }
                if !repo.path.join(".git").exists() {
                    errors.push(format!("{}: missing nested git checkout", repo.name));
                    continue;
                }
                if !skip_remotes {
                    let expected = declared_remote(raw)
                        .ok_or_else(|| format!("{} is missing a declared remote", repo.name))?;
                    let remotes = git_remotes(&repo.path)?;
                    if remotes.len() != 1 || remotes.get("origin") != Some(&vec![expected.clone()])
                    {
                        errors.push(format!(
                            "{}: nested remotes must contain exactly origin -> {}",
                            repo.name, expected
                        ));
                    }
                }
                check_cargo_sources(&repo, &mut errors)?;
            }
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
                || line.contains("http://127.0.0.1:8787/git/jeryu/redline-core/")
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
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
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
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
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

    impl TestDir {
        fn new(label: &str) -> Self {
            let parent = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/test-tmp");
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
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
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

        let fixture = JankuraiFixture::new("jankurai-linked-policy");
        let external_policy = fixture._root.path().join("external-policy.toml");
        fs::copy(&fixture.policy, &external_policy).unwrap();
        fs::remove_file(&fixture.policy).unwrap();
        symlink(&external_policy, &fixture.policy).unwrap();
        let error = fixture
            .validate(&fixture.valid_report(), true)
            .unwrap_err()
            .to_string();
        assert!(error.contains("governed repository policy"));

        let fixture = JankuraiFixture::new("jankurai-linked-baseline");
        let external_baseline = fixture._root.path().join("external-baseline.json");
        fs::copy(&fixture.baseline, &external_baseline).unwrap();
        fs::remove_file(&fixture.baseline).unwrap();
        symlink(&external_baseline, &fixture.baseline).unwrap();
        let error = fixture
            .validate(&fixture.valid_report(), true)
            .unwrap_err()
            .to_string();
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

        let generic: toml::Value = "name = \"example\"".parse().unwrap();
        let generic = release_cargo_policy("example", &generic).unwrap();
        assert_eq!(generic["mode"], "all-features");
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
        let main_refspec = format!("{reviewed}:refs/heads/main");
        run_git_strict(&repo, &["push", remote.to_str().unwrap(), &main_refspec]).unwrap();
        let receipt = root.path().join("tag.json");
        let args = || {
            vec![
                "--repo".to_owned(),
                repo.display().to_string(),
                "--remote".to_owned(),
                remote.display().to_string(),
                "--tag".to_owned(),
                "example-v8.0.0-split.0".to_owned(),
                "--commit".to_owned(),
                reviewed.clone(),
                "--receipt".to_owned(),
                receipt.display().to_string(),
            ]
        };
        immutable_tag_command(args()).unwrap();
        assert_eq!(
            local_ref_commit(&repo, "refs/tags/example-v8.0.0-split.0").unwrap(),
            None
        );
        let mut apply = args();
        apply.push("--apply".to_owned());
        immutable_tag_command(apply.clone()).unwrap();
        immutable_tag_command(apply).unwrap();
        assert_eq!(read_json(&receipt)["action"], "verified-existing");

        let different = commit_next(&repo);
        let mut refuse = args();
        let index = refuse.iter().position(|value| value == "--commit").unwrap();
        refuse[index + 1] = different;
        refuse.push("--apply".to_owned());
        assert!(immutable_tag_command(refuse).is_err());
        assert_eq!(
            local_ref_commit(&repo, "refs/tags/example-v8.0.0-split.0").unwrap(),
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
        create_or_verify_immutable_tag(
            &repo,
            remote.to_str().unwrap(),
            "example-v8.0.0-split.0",
            &reviewed,
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
        let root = TestDir::new("jeryu-askpass");
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
    fn branch_push_apply_requires_an_explicit_token_before_remote_access() {
        let root = TestDir::new("branch-push-token-required");
        let (repo, head) = init_source(root.path());
        let error = jeryu_branch_push(vec![
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
        ])
        .unwrap_err();
        assert!(error.to_string().contains("requires --token-file"));
    }

    #[test]
    fn branch_push_dry_run_is_local_read_only_and_rejects_config_injection() {
        let root = TestDir::new("branch-push-dry-run");
        let (repo, head) = init_source(root.path());
        let before = strict_git_output(&repo, &["status", "--porcelain=v1"]).unwrap();
        jeryu_branch_push(vec![
            "branch-push".to_owned(),
            "--repo".to_owned(),
            "jeryu/example".to_owned(),
            "--repo-path".to_owned(),
            repo.display().to_string(),
            "--branch".to_owned(),
            "main".to_owned(),
            "--expected-head".to_owned(),
            head.clone(),
        ])
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
        assert!(jeryu_branch_push(vec![
            "branch-push".to_owned(),
            "--repo".to_owned(),
            "jeryu/example".to_owned(),
            "--repo-path".to_owned(),
            repo.display().to_string(),
            "--branch".to_owned(),
            "main".to_owned(),
            "--expected-head".to_owned(),
            head.clone(),
        ])
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
    fn managed_repository_view_includes_both_control_planes_and_nested_family() {
        let root = TestDir::new("managed-repos");
        let nested = root.path().join("redline-split-ops/repos.manifest.toml");
        fs::create_dir_all(nested.parent().unwrap()).unwrap();
        fs::write(
            &nested,
            r#"
family = "redline-split"
[control_plane]
name = "redline-split-ops"
remote = "http://127.0.0.1:8787/git/jeryu/redline-split-ops.git"
required_check = "redline-split-ops/required"
[[repo]]
name = "redline-core"
path = "../redline-split/redline-core"
jeryu_slug = "jeryu/redline-core"
required_check = "redline-core/required"
default_branch = "main"
current_tag = "redline-core-v4.1.0-jain.1"
"#,
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
remote = "http://127.0.0.1:8787/git/jeryu/jain-split-ops.git"
required_check = "jain-split-ops/required"
[nested_families.redline]
manifest_path = "{}"
control_plane = "{}/redline-split-ops"
[[infrastructure_repo]]
name = "jain-smartcluster"
path = "{}/jain-smartcluster"
profile = "rust-workspace"
remote = "http://127.0.0.1:8787/git/jain-split/jain-smartcluster.git"
required_check = "jain-smartcluster/required"
default_branch = "main"
immutable_tag = "jain-smartcluster-v8.0.0-split.0"
kind = "required-infrastructure"
family_registered = true
[[repo]]
name = "jain"
path = "{}/jain"
profile = "custom"
jeryu_slug = "jeryu/jain"
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
            .ends_with("docs/release-evidence/8.0.0/receipt.json"));
        let root = TestDir::new("derived-sync");
        let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("repos.manifest.toml");
        let mut canonical: toml::Value = fs::read_to_string(source).unwrap().parse().unwrap();
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

        let symlink = root.path().join("request-symlink");
        std::os::unix::fs::symlink(&source, &symlink).unwrap();
        assert!(snapshot_host_ci_request(
            &symlink,
            &root.path().join("symlink-copy"),
            metadata.uid(),
            metadata.gid(),
            65_536,
        )
        .is_err());

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
