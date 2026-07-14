// Repository-local release and Jeryu control-plane CLI.
use serde_json::{json, Map, Value as JsonValue};
use sha2::{Digest, Sha256};
use std::{
    env, fs, io,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

const RELEASE_VERSION: &str = "8.0.1";
const ROLLBACK_TARGET: &str = "7.0.6";
const LOCAL_JERYU_BASE: &str = "http://127.0.0.1:8787";
const FAMILY_REMOTE_PREFIX: &str = "http://127.0.0.1:8787/git/jeryu/";
const INFRA_REMOTE_PREFIX: &str = "http://127.0.0.1:8787/git/jain-split/";

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
struct JeryuRequest {
    method: &'static str,
    path: String,
    body: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ReleaseFeatureMatrix {
    package: String,
    feature_sets: Vec<Vec<String>>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
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
        Some("python-boundary") => python_boundary(args.collect())?,
        Some("jeryu-doctor") => {
            let mut manifest = None;
            let mut skip_remotes = false;
            let mut fix_remotes = false;
            let mut register_family = false;
            while let Some(arg) = args.next() {
                match arg.as_str() {
                    "--manifest" => manifest = Some(PathBuf::from(args.next().ok_or("--manifest needs a path")?)),
                    "--skip-remotes" => skip_remotes = true,
                    "--fix-remotes" => fix_remotes = true,
                    "--register-family" => register_family = true,
                    value => return Err(format!("unknown argument: {value}").into()),
                }
            }
            if fix_remotes {
                fix_local_remotes(manifest.clone())?;
            }
            if register_family {
                let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
                let manifest_path = manifest
                    .clone()
                    .unwrap_or_else(|| root.join("repos.manifest.toml"));
                let status = Command::new("bash")
                    .arg(root.join("ops/split/register-family.sh"))
                    .arg("--manifest")
                    .arg(&manifest_path)
                    .status()?;
                if !status.success() {
                    return Err("family registration failed".into());
                }
            }
            validate_local_jeryu(manifest, skip_remotes)?;
        }
        Some("jeryu-local") => jeryu_local(args.collect())?,
        Some("manifest") => manifest_command(args.collect())?,
        Some("managed-repos") => managed_repos_command(args.collect())?,
        Some("release-cargo-commands") => release_cargo_commands_command(args.collect())?,
        Some("sync-derived-manifests") => sync_derived_manifests_command(args.collect())?,
        Some("validate-manifest") => validate_manifest_command(args.collect())?,
        Some("validate-family") => preflight(args.collect())?,
        Some("validate-family-lock") => validate_family_lock(args.collect())?,
        Some("validate-deploy-lock") => validate_deploy_lock_command(args.collect())?,
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
        _ => return Err("usage: splitctl refresh-ci-contract [--repo NAME]... | materialize [--repo NAME]... | manifest [--manifest PATH] [--json] | managed-repos [--manifest PATH] --json | release-cargo-commands [--manifest PATH] --repo NAME | sync-derived-manifests [--manifest PATH] [--receipt PATH] [--apply] | validate-manifest [--manifest PATH] [--check-paths] [--check-derived] | validate-local-jeryu [--manifest PATH] [--skip-remotes] | validate-family [--manifest PATH] [--json PATH] | validate-family-lock [--manifest PATH] [--lock PATH] | validate-deploy-lock [--manifest PATH] [--lock PATH] | regenerate-lock [--manifest PATH] [--output PATH] --apply | release-preflight [--manifest PATH] [--json PATH] | release-snapshot [--manifest PATH] [--json PATH] | release-status [--manifest PATH] [--json PATH] | bootstrap-main --repo PATH --remote URL --reviewed-commit SHA [--receipt PATH] [--apply] | immutable-tag --repo PATH --remote URL --tag TAG --commit SHA [--receipt PATH] [--apply] | verify-worktrees [--manifest PATH] [--receipt PATH] | preflight [--manifest PATH] [--json PATH] | source-coverage [--manifest PATH] [--json] | python-boundary [--manifest PATH] | jeryu-doctor [--manifest PATH] | reconcile [--manifest PATH] [--base-ref REF] [--apply] [--json PATH] | bump-version [--manifest PATH] --from VERSION --new VERSION --rewrite-split-tags".into()),
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
        if check_paths {
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
    if string(data, "status").as_deref() != Some("candidate") {
        errors.push("status must be candidate".to_owned());
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
    if string(data, "repo_family").as_deref() != Some("jain-split") {
        errors.push("repo_family must be jain-split".to_owned());
    }
    match string(data, "dependency_tag_suffix") {
        Some(value) => {
            if let Err(error) = validate_split_tag(&value, "v", RELEASE_VERSION) {
                errors.push(format!("dependency_tag_suffix: {error}"));
            }
        }
        None => errors.push("dependency_tag_suffix is required".to_owned()),
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
        match string(raw, "current_tag") {
            Some(value) => {
                if let Err(error) =
                    validate_split_tag(&value, &format!("{name}-v"), RELEASE_VERSION)
                {
                    errors.push(format!("{name}: current_tag: {error}"));
                }
            }
            None => errors.push(format!("{name}: current_tag is required")),
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
        if check_paths {
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
        ] {
            if string(raw, key).as_deref() != Some(expected) {
                errors.push(format!("jain-smartcluster: {key} must be {expected}"));
            }
        }
        match string(raw, "immutable_tag") {
            Some(value) => {
                if let Err(error) =
                    validate_split_tag(&value, "jain-smartcluster-v", RELEASE_VERSION)
                {
                    errors.push(format!("jain-smartcluster: immutable_tag: {error}"));
                }
            }
            None => errors.push("jain-smartcluster: immutable_tag is required".to_owned()),
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
    if string(redline, "owner").as_deref() != Some("jeryu")
        || string(redline, "repository").as_deref() != Some("redline-core")
        || string(redline, "remote").as_deref()
            != Some("http://127.0.0.1:8787/git/jeryu/redline-core.git")
        || redline.get("required").and_then(toml::Value::as_bool) != Some(true)
    {
        errors.push("redline dependency must be the required local-Jeryu redline-core".to_owned());
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
    ] {
        if string(nested, key).as_deref() != Some(expected) {
            errors.push(format!("nested_families.redline.{key} must be {expected}"));
        }
    }
    if nested.get("required").and_then(toml::Value::as_bool) != Some(true) {
        errors.push("nested_families.redline.required must be true".to_owned());
    }
    match string(redline, "identity_status").as_deref() {
        Some("pending") => {
            if redline.get("immutable_tag").is_some() || nested.get("engine_tag").is_some() {
                errors.push(
                    "pending Redline identity must not fabricate immutable_tag or engine_tag"
                        .to_owned(),
                );
            }
            if string(nested, "engine_identity_status").as_deref() != Some("pending") {
                errors.push(
                    "nested_families.redline.engine_identity_status must be pending".to_owned(),
                );
            }
        }
        Some("bound") => {
            let tag = string(redline, "immutable_tag").unwrap_or_default();
            if let Err(error) = validate_redline_tag(&tag) {
                errors.push(error.to_string());
            }
            if string(nested, "engine_identity_status").as_deref() != Some("bound") {
                errors.push(
                    "nested_families.redline.engine_identity_status must be bound".to_owned(),
                );
            }
            if string(nested, "engine_tag").as_deref() != Some(tag.as_str()) {
                errors.push(
                    "nested_families.redline.engine_tag must match the bound Redline identity"
                        .to_owned(),
                );
            }
        }
        Some(other) => errors.push(format!(
            "external_dependencies.redline.identity_status is invalid: {other}"
        )),
        None => errors.push("external_dependencies.redline.identity_status is required".to_owned()),
    }
    for (target, consumer_repo) in [("portal", "jain"), ("deploy", "jain-deploy")] {
        let declaration = data
            .get("derived_manifests")
            .and_then(|value| value.get(target));
        let expected_review = format!("derived-manifests/{target}.toml");
        let expected_consumer = split_root
            .as_ref()
            .map(|root| root.join(consumer_repo).join("repos.manifest.toml"));
        if declaration
            .and_then(|value| string(value, "path"))
            .as_deref()
            != Some(expected_review.as_str())
        {
            errors.push(format!(
                "derived_manifests.{target}.path must be the tracked review artifact {expected_review}"
            ));
        }
        if declaration
            .and_then(|value| string(value, "consumer_path"))
            .map(PathBuf::from)
            != expected_consumer
        {
            errors.push(format!(
                "derived_manifests.{target}.consumer_path must be {}",
                expected_consumer
                    .as_deref()
                    .unwrap_or(Path::new("<missing split_root>"))
                    .display()
            ));
        }
    }
    validate_rollout_waves(data, &mut errors)?;
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

fn validate_rollout_waves(
    data: &toml::Value,
    errors: &mut Vec<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut order = Vec::new();
    let mut order_seen = std::collections::BTreeSet::new();
    match data
        .get("rollout_wave_order")
        .and_then(toml::Value::as_array)
    {
        Some(values) => {
            for value in values {
                let Some(wave) = value.as_integer() else {
                    errors.push("rollout_wave_order must contain only integers".to_owned());
                    continue;
                };
                if wave < 0 {
                    errors.push(format!("rollout_wave_order contains negative wave {wave}"));
                }
                if !order_seen.insert(wave) {
                    errors.push(format!("rollout_wave_order repeats wave {wave}"));
                }
                order.push(wave);
            }
        }
        None => errors.push("rollout_wave_order is required".to_owned()),
    }
    if order.windows(2).any(|pair| pair[0] >= pair[1]) {
        errors.push("rollout_wave_order must be strictly increasing".to_owned());
    }

    let mut waves = std::collections::BTreeMap::new();
    let mut dependencies = std::collections::BTreeMap::new();
    for raw in manifest_repos(data)? {
        let Some(name) = string(raw, "name") else {
            continue;
        };
        let Some(wave) = raw.get("rollout_wave").and_then(toml::Value::as_integer) else {
            errors.push(format!("{name}: rollout_wave is required"));
            continue;
        };
        if wave < 0 {
            errors.push(format!("{name}: rollout_wave must be nonnegative"));
        }
        waves.insert(name, wave);
        dependencies.insert(
            string(raw, "name").unwrap_or_default(),
            strings(raw, "cross_repo_deps"),
        );
    }
    let external_rows = data
        .get("external_dependencies")
        .and_then(toml::Value::as_table)
        .into_iter()
        .flat_map(toml::map::Map::values)
        .collect::<Vec<_>>();
    for raw in external_rows {
        let Some(name) = string(raw, "repository") else {
            errors.push("external dependency is missing repository".to_owned());
            continue;
        };
        let Some(wave) = raw.get("rollout_wave").and_then(toml::Value::as_integer) else {
            errors.push(format!("{name}: external rollout_wave is required"));
            continue;
        };
        if wave < 0 {
            errors.push(format!("{name}: rollout_wave must be nonnegative"));
        }
        if waves.insert(name.clone(), wave).is_some() {
            errors.push(format!("duplicate rollout node: {name}"));
        }
        dependencies.insert(name.clone(), Vec::new());
    }

    let used_waves = waves
        .values()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    for wave in &used_waves {
        if !order_seen.contains(wave) {
            errors.push(format!(
                "rollout wave {wave} is not reachable from rollout_wave_order"
            ));
        }
    }
    for wave in &order_seen {
        if !used_waves.contains(wave) {
            errors.push(format!("rollout_wave_order contains unused wave {wave}"));
        }
    }
    let order_index = order
        .iter()
        .enumerate()
        .map(|(index, wave)| (*wave, index))
        .collect::<std::collections::BTreeMap<_, _>>();

    for raw in family_repos(data)? {
        let Some(name) = string(raw, "name") else {
            continue;
        };
        let Some(wave) = waves.get(&name).copied() else {
            continue;
        };
        for dependency in strings(raw, "cross_repo_deps") {
            match waves.get(&dependency).copied() {
                Some(dependency_wave)
                    if order_index.get(&dependency_wave) >= order_index.get(&wave) =>
                {
                    errors.push(format!(
                        "{name}: dependency {dependency} must be in an earlier rollout wave ({dependency_wave} before {wave})"
                    ))
                }
                Some(_) => {}
                None => errors.push(format!("{name}: unknown cross_repo_deps entry {dependency}")),
            }
        }
    }

    for raw in data
        .get("infrastructure_repo")
        .and_then(toml::Value::as_array)
        .into_iter()
        .flatten()
    {
        let Some(name) = string(raw, "name") else {
            continue;
        };
        let Some(wave) = waves.get(&name).copied() else {
            continue;
        };
        for dependant in strings(raw, "dependency_edges") {
            match waves.get(&dependant).copied() {
                Some(dependant_wave)
                    if order_index.get(&dependant_wave) <= order_index.get(&wave) =>
                {
                    errors.push(format!(
                        "{name}: dependant {dependant} must be in a later rollout wave ({dependant_wave} after {wave})"
                    ))
                }
                Some(_) => {}
                None => errors.push(format!("{name}: unknown dependency_edges entry {dependant}")),
            }
        }
    }

    let mut completed = std::collections::BTreeSet::new();
    for wave in &order {
        let wave_nodes = waves
            .iter()
            .filter_map(|(name, node_wave)| (node_wave == wave).then_some(name.clone()))
            .collect::<Vec<_>>();
        for name in &wave_nodes {
            for dependency in dependencies.get(name).into_iter().flatten() {
                if !completed.contains(dependency) {
                    errors.push(format!(
                        "all-PENDING rollout cannot reach {name} in wave {wave}: {dependency} is not complete"
                    ));
                }
            }
        }
        completed.extend(wave_nodes);
    }
    if completed.len() != waves.len() {
        let missing = waves
            .keys()
            .filter(|name| !completed.contains(*name))
            .cloned()
            .collect::<Vec<_>>();
        errors.push(format!(
            "all-PENDING rollout leaves unreachable nodes: {}",
            missing.join(", ")
        ));
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
    let expected_release = string(&data, "dependency_tag_suffix")
        .and_then(|suffix| suffix.strip_prefix('v').map(str::to_owned))
        .unwrap_or_default();
    if string(&lock_data, "release").as_deref() != Some(expected_release.as_str()) {
        errors.push(format!("lock release is not {expected_release}"));
    }
    let family = family_repos(&data)?;
    let lock_repos = lock_data
        .get("repo")
        .and_then(toml::Value::as_array)
        .cloned()
        .unwrap_or_default();
    if lock_data
        .get("family_repo_count")
        .and_then(toml::Value::as_integer)
        != Some(family.len() as i64)
    {
        errors.push(format!("family_repo_count must be {}", family.len()));
    }
    if lock_repos.len() != family.len() {
        errors.push(format!(
            "lock has {} family entries, expected {}",
            lock_repos.len(),
            family.len()
        ));
    }
    let infrastructure = data
        .get("infrastructure_repo")
        .and_then(toml::Value::as_array)
        .cloned()
        .unwrap_or_default();
    let lock_infrastructure = lock_data
        .get("infrastructure_repo")
        .and_then(toml::Value::as_array);
    if lock_data
        .get("infrastructure_repo_count")
        .and_then(toml::Value::as_integer)
        != Some(infrastructure.len() as i64)
    {
        errors.push(format!(
            "infrastructure_repo_count must be {}",
            infrastructure.len()
        ));
    }
    validate_lock_pin_group(&family, Some(&lock_repos), "repo", &mut errors);
    validate_lock_pin_group(
        &infrastructure.iter().collect::<Vec<_>>(),
        lock_infrastructure,
        "infrastructure_repo",
        &mut errors,
    );
    let redline = data
        .get("external_dependencies")
        .and_then(|value| value.get("redline"));
    let nested = lock_data
        .get("nested")
        .and_then(|value| value.get("redline"));
    match redline.and_then(|value| string(value, "identity_status")) {
        Some(status) if status == "bound" => {
            let expected_tag = redline
                .and_then(|value| string(value, "immutable_tag"))
                .unwrap_or_default();
            if nested.and_then(|value| string(value, "tag")).as_deref()
                != Some(expected_tag.as_str())
            {
                errors.push(format!(
                    "nested.redline.tag must match authority tag {expected_tag}"
                ));
            }
            let commit = nested
                .and_then(|value| string(value, "commit"))
                .unwrap_or_default();
            if !is_hex(&commit, 40) {
                errors.push("nested.redline.commit must be a 40-character SHA".to_owned());
            }
        }
        _ => errors.push(
            "authority Redline identity is not bound; family lock cannot be released".to_owned(),
        ),
    }
    if !errors.is_empty() {
        return Err(format!("family lock validation failed:\n{}", errors.join("\n")).into());
    }
    println!("family lock valid: {}", lock.display());
    Ok(())
}

fn validate_deploy_lock_command(args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut manifest = root.join("repos.manifest.toml");
    let mut lock = None;
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--manifest" => manifest = PathBuf::from(iter.next().ok_or("--manifest needs a path")?),
            "--lock" => lock = Some(PathBuf::from(iter.next().ok_or("--lock needs a path")?)),
            value => return Err(format!("unknown validate-deploy-lock argument: {value}").into()),
        }
    }
    let data: toml::Value = fs::read_to_string(&manifest)?.parse()?;
    validate_manifest_data(&data, &manifest, false)?;
    let lock = lock.unwrap_or_else(|| {
        PathBuf::from(string(&data, "split_root").unwrap_or_default())
            .join("jain-deploy/jain-split.lock.toml")
    });
    let lock_data: toml::Value = fs::read_to_string(&lock)?.parse()?;
    validate_deploy_lock_data(&data, &manifest, &lock_data, &lock)?;
    println!("deploy lock valid: {}", lock.display());
    Ok(())
}

fn validate_deploy_lock_data(
    authority: &toml::Value,
    manifest: &Path,
    lock: &toml::Value,
    lock_path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut errors = Vec::new();
    let expected_hash = manifest_sha256(manifest)?;
    if string(lock, "source_manifest_sha256").as_deref() != Some(expected_hash.as_str()) {
        errors.push(format!(
            "source_manifest_sha256 does not match authority digest {expected_hash}"
        ));
    }
    if string(lock, "schema_version").as_deref() != Some("1.0.0") {
        errors.push("schema_version must be 1.0.0".to_owned());
    }
    if string(lock, "family").as_deref() != Some("jain") {
        errors.push("family must be jain".to_owned());
    }
    let expected_release = string(authority, "dependency_tag_suffix")
        .and_then(|suffix| suffix.strip_prefix('v').map(str::to_owned))
        .unwrap_or_default();
    if string(lock, "release").as_deref() != Some(expected_release.as_str()) {
        errors.push(format!("release must be {expected_release}"));
    }

    let family = family_repos(authority)?;
    let infrastructure = authority
        .get("infrastructure_repo")
        .and_then(toml::Value::as_array)
        .cloned()
        .unwrap_or_default();
    if lock
        .get("family_repo_count")
        .and_then(toml::Value::as_integer)
        != Some(family.len() as i64)
    {
        errors.push(format!("family_repo_count must be {}", family.len()));
    }
    if lock
        .get("infrastructure_repo_count")
        .and_then(toml::Value::as_integer)
        != Some(infrastructure.len() as i64)
    {
        errors.push(format!(
            "infrastructure_repo_count must be {}",
            infrastructure.len()
        ));
    }
    validate_lock_pin_group(
        &family,
        lock.get("repo").and_then(toml::Value::as_array),
        "repo",
        &mut errors,
    );
    validate_lock_pin_group(
        &infrastructure.iter().collect::<Vec<_>>(),
        lock.get("infrastructure_repo")
            .and_then(toml::Value::as_array),
        "infrastructure_repo",
        &mut errors,
    );

    let redline = authority
        .get("external_dependencies")
        .and_then(|value| value.get("redline"));
    let nested = lock.get("nested").and_then(|value| value.get("redline"));
    match redline.and_then(|value| string(value, "identity_status")) {
        Some(status) if status == "bound" => {
            let expected_tag = redline
                .and_then(|value| string(value, "immutable_tag"))
                .unwrap_or_default();
            if nested.and_then(|value| string(value, "family")).as_deref() != Some("redline-split")
            {
                errors.push("nested.redline.family must be redline-split".to_owned());
            }
            if nested
                .and_then(|value| string(value, "engine_tag"))
                .as_deref()
                != Some(expected_tag.as_str())
            {
                errors.push(format!(
                    "nested.redline.engine_tag must match authority tag {expected_tag}"
                ));
            }
            let commit = nested
                .and_then(|value| string(value, "engine_commit"))
                .unwrap_or_default();
            if !is_hex(&commit, 40) {
                errors.push("nested.redline.engine_commit must be a 40-character SHA".to_owned());
            }
            let expected_lock_id = format!("redline-proof/v2/4.1.0/{commit}");
            if nested
                .and_then(|value| string(value, "proof_lock_id"))
                .as_deref()
                != Some(expected_lock_id.as_str())
            {
                errors.push(format!(
                    "nested.redline.proof_lock_id must bind engine commit {commit}"
                ));
            }
        }
        _ => errors.push(
            "authority Redline identity is not bound; deploy cutover remains blocked".to_owned(),
        ),
    }

    if !errors.is_empty() {
        return Err(format!(
            "deploy lock validation failed ({}):\n{}",
            lock_path.display(),
            errors.join("\n")
        )
        .into());
    }
    Ok(())
}

fn validate_lock_pin_group(
    expected: &[&toml::Value],
    actual: Option<&Vec<toml::Value>>,
    label: &str,
    errors: &mut Vec<String>,
) {
    let actual = actual.map(Vec::as_slice).unwrap_or_default();
    if actual.len() != expected.len() {
        errors.push(format!(
            "{label} pin count is {}, expected {}",
            actual.len(),
            expected.len()
        ));
    }
    let mut seen = std::collections::BTreeSet::new();
    for entry in actual {
        let name = string(entry, "repo")
            .or_else(|| string(entry, "name"))
            .unwrap_or_default();
        if name.is_empty() || !seen.insert(name.clone()) {
            errors.push(format!(
                "{label} contains a missing or duplicate repository name: {name}"
            ));
        }
    }
    for raw in expected {
        let name = string(raw, "name").unwrap_or_default();
        let Some(entry) = actual.iter().find(|entry| {
            string(entry, "repo")
                .or_else(|| string(entry, "name"))
                .as_deref()
                == Some(name.as_str())
        }) else {
            errors.push(format!("{name}: missing from {label} pins"));
            continue;
        };
        let expected_tag = string(raw, "immutable_tag").or_else(|| string(raw, "current_tag"));
        if string(entry, "tag") != expected_tag {
            errors.push(format!("{name}: lock tag does not match authority"));
        }
        if string(entry, "jeryu") != declared_remote(raw) {
            errors.push(format!(
                "{name}: lock Jeryu remote does not match authority"
            ));
        }
        if string(entry, "required_check") != string(raw, "required_check") {
            errors.push(format!(
                "{name}: lock required_check does not match authority"
            ));
        }
        let commit = string(entry, "commit").unwrap_or_default();
        if !is_hex(&commit, 40) {
            errors.push(format!("{name}: lock commit must be a 40-character SHA"));
        }
        if let Some(checksum) = string(entry, "checksum_sha256") {
            if !is_hex(&checksum, 64) {
                errors.push(format!(
                    "{name}: checksum_sha256 must be a 64-character digest"
                ));
            }
        }
        if label == "infrastructure_repo"
            && (string(entry, "kind") != string(raw, "kind")
                || string(entry, "forge_owner") != string(raw, "forge_owner")
                || entry
                    .get("family_registered")
                    .and_then(toml::Value::as_bool)
                    != Some(true))
        {
            errors.push(format!(
                "{name}: infrastructure lock metadata is not fail-closed"
            ));
        }
    }
}

fn is_hex(value: &str, length: usize) -> bool {
    value.len() == length && value.chars().all(|ch| ch.is_ascii_hexdigit())
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
    let redline = data
        .get("external_dependencies")
        .and_then(|value| value.get("redline"))
        .ok_or("manifest missing external_dependencies.redline")?;
    if string(redline, "identity_status").as_deref() != Some("bound") {
        return Err(
            "cannot regenerate a release lock while the Redline reviewed identity is pending"
                .into(),
        );
    }
    let manifest_hash = manifest_sha256(&manifest)?;
    let release = string(&data, "dependency_tag_suffix")
        .and_then(|suffix| suffix.strip_prefix('v').map(str::to_owned))
        .ok_or("manifest dependency_tag_suffix is invalid")?;
    let mut text = format!(
        "schema_version = \"1.0.0\"\nfamily = \"jain-split\"\nrelease = \"{release}\"\ngenerator_version = \"splitctl 0.1.0\"\nsource = \"{}\"\nsource_manifest_sha256 = \"{manifest_hash}\"\nfamily_repo_count = {}\ninfrastructure_repo_count = {}\ndependency_resolution = \"immutable-git-tag\"\n\n",
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
        let infrastructure_metadata = if table == "infrastructure_repo" {
            format!(
                "kind = \"{}\"\nforge_owner = \"{}\"\nfamily_registered = true\n",
                string(raw, "kind").unwrap_or_default(),
                string(raw, "forge_owner").unwrap_or_default()
            )
        } else {
            String::new()
        };
        text.push_str(&format!(
            "[[{table}]]\nrepo = \"{}\"\ntag = \"{tag}\"\ncommit = \"{commit}\"\njeryu = \"{}\"\nrequired_check = \"{}\"\n{infrastructure_metadata}\n",
            repo.name,
            declared_remote(raw).ok_or_else(|| format!("{} missing remote", repo.name))?,
            string(raw, "required_check").unwrap_or_else(|| format!("{}/required", repo.name))
        ));
    }
    let redline_tag = string(redline, "immutable_tag").unwrap_or_default();
    let redline_core = data
        .get("nested_families")
        .and_then(|value| value.get("redline"))
        .and_then(|value| string(value, "container_path"))
        .map(PathBuf::from)
        .ok_or("nested Redline container path is missing")?
        .join("redline-core");
    let redline_ref = format!("refs/tags/{redline_tag}^{{}}");
    let redline_commit = git_query(&redline_core, &["rev-parse", &redline_ref])
        .ok_or_else(|| format!("Redline core is missing immutable tag {redline_tag}"))?;
    text.push_str(&format!(
        "[nested.redline]\nfamily = \"redline-split\"\nremote = \"{}\"\ntag = \"{redline_tag}\"\ncommit = \"{redline_commit}\"\n\n",
        string(redline, "remote").unwrap_or_default(),
    ));
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
        "release_status": "candidate",
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
        "sagemaker": "N/A",
        "rollback_target": ROLLBACK_TARGET,
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
        .strip_prefix(&format!("{LOCAL_JERYU_BASE}/git/"))
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
    let mut command = Command::new("git");
    command.arg("-C").arg(repo).args(args);
    if args.first() == Some(&"push") {
        if let Ok(token) = local_jeryu_token() {
            command
                .env("GIT_CONFIG_COUNT", "1")
                .env("GIT_CONFIG_KEY_0", "http.extraHeader")
                .env(
                    "GIT_CONFIG_VALUE_0",
                    format!("Authorization: Bearer {token}"),
                );
        }
    }
    let output = command.output()?;
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
    let base = env::var("JERYU_BASE").unwrap_or_else(|_| LOCAL_JERYU_BASE.to_owned());
    let token = local_jeryu_token()?;
    let json_output = args.iter().any(|arg| arg == "--json");
    let value = |flag: &str| -> Result<String, Box<dyn std::error::Error>> {
        args.iter()
            .position(|arg| arg == flag)
            .and_then(|index| args.get(index + 1))
            .cloned()
            .ok_or_else(|| format!("{flag} needs a value").into())
    };
    let (method, path, body) = match command {
        "repo-list" => ("GET", "/api/v1/repos?host=jeryu".to_owned(), None),
        "pr-list" => {
            let repo = value("--repo")?;
            (
                "GET",
                format!(
                    "/repos/{repo}/pulls?state={}",
                    value("--state").unwrap_or_else(|_| "open".to_owned())
                ),
                None,
            )
        }
        "pr-open" => {
            let repo = value("--repo")?;
            let payload = json!({
                "title": value("--title")?,
                "head": value("--head")?,
                "base": value("--base").unwrap_or_else(|_| "main".to_owned()),
                "body": value("--body").unwrap_or_default(),
                "draft": args.iter().any(|arg| arg == "--draft"),
                "actor": value("--actor").unwrap_or_else(|_| "codex".to_owned())
            });
            (
                "POST",
                format!("/repos/{repo}/pulls"),
                Some(payload.to_string()),
            )
        }
        "checks" => {
            let repo = value("--repo")?;
            (
                "GET",
                format!("/repos/{repo}/commits/{}/check-runs", value("--sha")?),
                None,
            )
        }
        _ => return Err(format!("unsupported jeryu-local command: {command}").into()),
    };
    let mut curl = Command::new("curl");
    curl.args([
        "-fsS",
        "--max-time",
        "15",
        "-H",
        "accept: application/json",
        "-H",
        &format!("authorization: Bearer {token}"),
    ]);
    if let Some(body) = body {
        curl.args([
            "-H",
            "content-type: application/json",
            "-X",
            method,
            "--data",
            &body,
        ]);
    } else if method != "GET" {
        curl.args(["-X", method]);
    }
    let output = curl
        .arg(format!("{}{}", base.trim_end_matches('/'), path))
        .output()?;
    if !output.status.success() {
        return Err(format!(
            "local Jeryu request failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )
        .into());
    }
    let raw = String::from_utf8(output.stdout)?;
    if json_output {
        println!(
            "{}",
            serde_json::from_str::<JsonValue>(&raw)
                .map(|value| serde_json::to_string_pretty(&value).unwrap_or(raw.clone()))
                .unwrap_or(raw)
        );
    } else {
        println!("{}", raw.trim());
    }
    Ok(())
}

fn local_jeryu_token() -> Result<String, Box<dyn std::error::Error>> {
    env::var("JERYU_MERGE_TOKEN")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            let path = env::var_os("JERYU_MERGE_TOKEN_FILE")
                .map(PathBuf::from)
                .unwrap_or_else(|| {
                    PathBuf::from(env::var_os("HOME").unwrap_or_default())
                        .join(".jeryu/secrets/merge-token")
                });
            fs::read_to_string(path)
                .ok()
                .map(|value| value.trim().to_owned())
        })
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "local Jeryu API credential is unavailable; repair it through the supported Jeryu token workflow".into())
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
    let mut receipt = None;
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
            "--receipt" => {
                receipt = Some(PathBuf::from(iter.next().ok_or("--receipt needs a path")?))
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
    let receipt = match receipt {
        Some(path) => path,
        None => release_evidence_path(&format!(
            "jeryu-{}-{}.json",
            receipt_component(&command),
            receipt_component(&repo)
        )),
    };
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
        let base = env::var("JERYU_BASE").unwrap_or_else(|_| LOCAL_JERYU_BASE.to_owned());
        let token = local_jeryu_token()?;
        let response = execute_jeryu_request(&base, &token, &request)?;
        report["response"] = response.clone();
        match command.as_str() {
            "pr-ready" if response.get("draft").and_then(JsonValue::as_bool) != Some(false) => {
                return Err("Jeryu PR ready readback still reports draft=true".into())
            }
            "pr-close" if response.get("state").and_then(JsonValue::as_str) != Some("closed") => {
                return Err("Jeryu PR close readback does not report state=closed".into())
            }
            "pr-approve" => {
                let readback = JeryuRequest {
                    method: "GET",
                    path: request.path.trim_end_matches("/reviews").to_owned(),
                    body: None,
                };
                let approval = execute_jeryu_request(&base, &token, &readback)?;
                validate_approval_readback(
                    &approval,
                    expected_head.as_deref().ok_or("missing expected head")?,
                )?;
                report["readback"] = approval;
            }
            "pr-merge"
                if response.get("merged").and_then(JsonValue::as_bool) != Some(true)
                    && response.get("merged_at").is_none()
                    && response.get("sha").is_none() =>
            {
                return Err("Jeryu PR merge response does not prove a merged commit".into())
            }
            "protection-apply" => {
                let readback = JeryuRequest {
                    method: "GET",
                    path: request.path.clone(),
                    body: None,
                };
                let policy = execute_jeryu_request(&base, &token, &readback)?;
                report["readback"] = policy.clone();
                validate_protection_policy(
                    &policy,
                    required_check.as_deref().ok_or("missing required check")?,
                )?;
            }
            "protection-readback" => validate_protection_policy(
                &response,
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
    finish_receipted_operation(&receipt, &mut report, result)
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
    let repo_id = repo.replace('/', "%2F");
    Ok(JeryuRequest {
        method: "POST",
        path: format!("/api/v1/repos/{repo_id}/pulls/{number}/reviews"),
        body: Some(
            json!({
                "verdict": "approve",
                "expected_head_sha": expected_head,
                "body_markdown": body,
                "thread_comments": [],
                "evidence": JsonValue::Null,
            })
            .to_string(),
        ),
    })
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
    branch: &str,
    required_check: Option<&str>,
) -> Result<JeryuRequest, Box<dyn std::error::Error>> {
    match command {
        "pr-ready" | "pr-close" | "pr-merge" => {
            let number = number.ok_or("PR lifecycle command requires --number")?;
            number
                .parse::<u64>()
                .map_err(|_| "--number must be a positive integer")?;
            Ok(JeryuRequest {
                method: if command == "pr-merge" {
                    "PUT"
                } else {
                    "PATCH"
                },
                path: if command == "pr-merge" {
                    format!("/repos/{repo}/pulls/{number}/merge")
                } else {
                    format!("/repos/{repo}/pulls/{number}")
                },
                body: Some(match command {
                    "pr-ready" => json!({"draft": false}).to_string(),
                    "pr-close" => json!({"state": "closed"}).to_string(),
                    _ => "{}".to_owned(),
                }),
            })
        }
        "protection-apply" | "protection-readback" => {
            let required_check =
                required_check.ok_or("protection command requires --required-check")?;
            if required_check.trim().is_empty() {
                return Err("--required-check must not be empty".into());
            }
            Ok(JeryuRequest {
                method: if command == "protection-apply" {
                    "PUT"
                } else {
                    "GET"
                },
                path: format!("/repos/{repo}/branches/{branch}/protection"),
                body: if command == "protection-apply" {
                    Some(immutable_main_policy(required_check).to_string())
                } else {
                    None
                },
            })
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
        "method": request.method,
        "path": request.path,
        "body": request.body.as_deref().and_then(|body| serde_json::from_str::<JsonValue>(body).ok()),
    })
}

fn execute_jeryu_request(
    base: &str,
    token: &str,
    request: &JeryuRequest,
) -> Result<JsonValue, Box<dyn std::error::Error>> {
    let mut curl = Command::new("curl");
    curl.args([
        "-fsS",
        "--max-time",
        "15",
        "-H",
        "accept: application/json",
        "-H",
        &format!("authorization: Bearer {token}"),
    ]);
    if let Some(body) = &request.body {
        curl.args([
            "-H",
            "content-type: application/json",
            "-X",
            request.method,
            "--data",
            body,
        ]);
    } else if request.method != "GET" {
        curl.args(["-X", request.method]);
    }
    let output = curl
        .arg(format!("{}{}", base.trim_end_matches('/'), request.path))
        .output()?;
    if !output.status.success() {
        return Err(format!(
            "local Jeryu request failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )
        .into());
    }
    let raw = String::from_utf8(output.stdout)?;
    if raw.trim().is_empty() {
        Ok(JsonValue::Null)
    } else {
        serde_json::from_str(&raw).map_err(Into::into)
    }
}

fn validate_protection_policy(
    policy: &JsonValue,
    required_check: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let checks = policy.get("required_status_checks");
    let contexts = checks
        .and_then(JsonValue::as_array)
        .or_else(|| checks.and_then(|value| value.get("contexts"))?.as_array());
    let required_present = contexts.is_some_and(|values| {
        values
            .iter()
            .any(|value| value.as_str() == Some(required_check))
    });
    let approvals = policy
        .get("required_approving_review_count")
        .and_then(JsonValue::as_u64)
        .or_else(|| {
            policy
                .get("required_pull_request_reviews")?
                .get("required_approving_review_count")?
                .as_u64()
        })
        .unwrap_or(0);
    let boolean = |name: &str| {
        policy.get(name).and_then(|value| {
            value
                .as_bool()
                .or_else(|| value.get("enabled").and_then(JsonValue::as_bool))
        })
    };
    if required_present
        && approvals >= 1
        && boolean("required_linear_history") == Some(true)
        && boolean("enforce_admins") == Some(true)
        && boolean("allow_force_pushes") == Some(false)
        && boolean("allow_deletions") == Some(false)
    {
        Ok(())
    } else {
        Err("branch protection readback does not satisfy the immutable-main policy".into())
    }
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
    let mut rewrite_tags = false;
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--manifest" => manifest = PathBuf::from(iter.next().ok_or("--manifest needs a path")?),
            "--from" => from_version = Some(iter.next().ok_or("--from needs a version")?),
            "--new" => new = Some(iter.next().ok_or("--new needs a version")?),
            "--update-lock-shas" => {
                return Err("--update-lock-shas was removed: bump-version may only update its explicit authority manifest; use regenerate-lock with an explicit output and --apply".into())
            }
            "--rewrite-split-tags" => rewrite_tags = true,
            value => return Err(format!("unknown bump-version argument: {value}").into()),
        }
    }
    let from_version = from_version.ok_or("--from is required")?;
    let new = new.ok_or("--new is required")?;
    validate_release_version(&from_version)?;
    validate_release_version(&new)?;
    if from_version == new {
        return Err("--from and --new must differ".into());
    }
    if !rewrite_tags {
        return Err(
            "refusing to rewrite split tag pins by default; pass --rewrite-split-tags".into(),
        );
    }
    let original = fs::read_to_string(&manifest)?;
    let changed = bump_manifest_version(&original, &from_version, &new)?;
    if changed != original {
        write_atomic_bytes(&manifest, changed.as_bytes())?;
        println!("changed: {}", manifest.display());
    } else {
        println!("already current: {}", manifest.display());
    }
    Ok(())
}

fn validate_release_version(version: &str) -> Result<(), Box<dyn std::error::Error>> {
    let components = version.split('.').collect::<Vec<_>>();
    if components.len() != 3
        || components.iter().any(|component| {
            component.is_empty() || !component.chars().all(|ch| ch.is_ascii_digit())
        })
    {
        return Err(format!("release version must be numeric MAJOR.MINOR.PATCH: {version}").into());
    }
    Ok(())
}

fn parse_revision(value: &str, label: &str) -> Result<u64, Box<dyn std::error::Error>> {
    if value.is_empty()
        || !value.chars().all(|ch| ch.is_ascii_digit())
        || (value.len() > 1 && value.starts_with('0'))
    {
        return Err(format!("{label} must be a canonical nonnegative integer: {value}").into());
    }
    value
        .parse::<u64>()
        .map_err(|_| format!("{label} is outside the supported integer range: {value}").into())
}

fn validate_split_tag(
    value: &str,
    expected_prefix: &str,
    expected_version: &str,
) -> Result<u64, Box<dyn std::error::Error>> {
    let prefix = format!("{expected_prefix}{expected_version}-split.");
    let revision = value
        .strip_prefix(&prefix)
        .ok_or_else(|| format!("split tag must start with {prefix}: {value}"))?;
    parse_revision(revision, "split tag revision")
}

fn validate_redline_tag(value: &str) -> Result<u64, Box<dyn std::error::Error>> {
    let revision = value
        .strip_prefix("redline-core-v4.1.0-jain.")
        .ok_or_else(|| format!("invalid reviewed Redline core tag: {value}"))?;
    parse_revision(revision, "Redline tag revision")
}

fn bump_manifest_version(
    original: &str,
    from_version: &str,
    new: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let data: toml::Value = original.parse()?;
    let current = string(&data, "release_version").ok_or("manifest missing release_version")?;
    if current != from_version && current != new {
        return Err(format!(
            "manifest release_version is {current}, expected --from {from_version} or already-current --new {new}"
        )
        .into());
    }

    let mut output = String::with_capacity(original.len());
    let mut release_fields = 0usize;
    let mut dependency_suffixes = 0usize;
    for line in original.split_inclusive('\n') {
        let mut rewritten = line.to_owned();
        if let Some(range) = toml_string_value_range(line, "release_version") {
            release_fields += 1;
            rewritten.replace_range(range, new);
        } else if let Some(range) = toml_string_value_range(line, "dependency_tag_suffix") {
            dependency_suffixes += 1;
            let normalized = normalize_split_tag(&line[range.clone()], from_version, new)?
                .ok_or("dependency_tag_suffix must use the split tag namespace")?;
            rewritten.replace_range(range, &normalized);
        } else {
            for key in ["current_tag", "immutable_tag"] {
                let Some(range) = toml_string_value_range(line, key) else {
                    continue;
                };
                if let Some(normalized) =
                    normalize_split_tag(&line[range.clone()], from_version, new)?
                {
                    rewritten.replace_range(range, &normalized);
                }
                break;
            }
        }
        output.push_str(&rewritten);
    }
    if release_fields != 1 {
        return Err(format!(
            "manifest must contain exactly one release_version, found {release_fields}"
        )
        .into());
    }
    if dependency_suffixes != 1 {
        return Err(format!(
            "manifest must contain exactly one dependency_tag_suffix, found {dependency_suffixes}"
        )
        .into());
    }
    let _: toml::Value = output.parse()?;
    Ok(output)
}

fn toml_string_value_range(line: &str, key: &str) -> Option<std::ops::Range<usize>> {
    let indentation = line.len() - line.trim_start().len();
    let trimmed = &line[indentation..];
    let remainder = trimmed.strip_prefix(key)?;
    if !remainder.starts_with(|ch: char| ch.is_ascii_whitespace() || ch == '=') {
        return None;
    }
    let equals = remainder.find('=')?;
    let after_equals = indentation + key.len() + equals + 1;
    let quote = line[after_equals..].find('"')? + after_equals;
    let value_start = quote + 1;
    let value_end = line[value_start..].find('"')? + value_start;
    Some(value_start..value_end)
}

fn normalize_split_tag(
    value: &str,
    from_version: &str,
    new: &str,
) -> Result<Option<String>, Box<dyn std::error::Error>> {
    let Some(marker) = value.rfind("-split.") else {
        return Ok(None);
    };
    let revision = &value[marker + "-split.".len()..];
    parse_revision(revision, "split tag revision")?;
    let before_revision = &value[..marker];
    let version_start = before_revision
        .rfind("-v")
        .map(|index| index + 2)
        .or_else(|| before_revision.starts_with('v').then_some(1))
        .ok_or_else(|| format!("invalid split tag: {value}"))?;
    let version = &before_revision[version_start..];
    if version != from_version && version != new {
        return Err(format!(
            "split tag {value} encodes {version}, expected {from_version} or {new}"
        )
        .into());
    }
    Ok(Some(format!(
        "{}{new}-split.{revision}",
        &value[..version_start]
    )))
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

fn python_boundary(args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    let control_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut manifest = control_root.join("repos.manifest.toml");
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--manifest" => manifest = PathBuf::from(iter.next().ok_or("--manifest needs a path")?),
            value => return Err(format!("unknown python-boundary argument: {value}").into()),
        }
    }
    let data: toml::Value = fs::read_to_string(&manifest)?.parse()?;
    let root = env::var_os("JAIN_SPLIT_ROOT")
        .map(PathBuf::from)
        .or_else(|| string(&data, "split_root").map(PathBuf::from))
        .ok_or("manifest split_root is unavailable")?;
    if !root.is_dir() {
        return Err(format!("declared split root is not a directory: {}", root.display()).into());
    }
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
    let redline = data
        .get("external_dependencies")
        .and_then(|value| value.get("redline"));
    let nested = data
        .get("nested_families")
        .and_then(|value| value.get("redline"));
    let Some(redline) = redline else {
        return vec!["missing external_dependencies.redline".to_owned()];
    };
    match string(redline, "identity_status").as_deref() {
        Some("pending") => {
            return vec![
                "Redline reviewed successor identity is pending; bind the immutable tag explicitly after its protected merge and tag"
                    .to_owned(),
            ]
        }
        Some("bound") => {}
        Some(other) => {
            return vec![format!("invalid Redline identity_status: {other}")];
        }
        None => return vec!["missing Redline identity_status".to_owned()],
    }
    let tag = string(redline, "immutable_tag").unwrap_or_default();
    if let Err(error) = validate_redline_tag(&tag) {
        failures.push(error.to_string());
    }
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
            if string(&lock, "engine_tag").as_deref() != Some(tag.as_str()) {
                failures.push(format!(
                    "Redline proof lock engine_tag does not match authority tag {tag}"
                ));
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
                    ".git"
                        | "target"
                        | ".stage"
                        | ".venv"
                        | "vendor"
                        | "node_modules"
                        | ".worktrees"
                        | ".work"
                        | ".ci-worktrees"
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
        string(value, "jeryu_slug").map(|slug| format!("{LOCAL_JERYU_BASE}/git/{slug}.git"))
    })
}

fn string(value: &toml::Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(toml::Value::as_str)
        .map(ToOwned::to_owned)
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
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

    struct TestDir(PathBuf);

    impl TestDir {
        fn new(label: &str) -> Self {
            let path = env::temp_dir().join(format!(
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
    fn bump_version_is_manifest_only_revision_preserving_and_idempotent() {
        let root = TestDir::new("bump-version");
        let product = root.path().join("product");
        fs::create_dir_all(&product).unwrap();
        let version_file = product.join("VERSION");
        let cargo_file = product.join("Cargo.toml");
        let changelog = product.join("CHANGELOG.md");
        fs::write(&version_file, "example-v8.0.0-split.1\n").unwrap();
        fs::write(
            &cargo_file,
            "[package]\nname = \"example\"\nversion = \"8.0.0\"\n",
        )
        .unwrap();
        fs::write(&changelog, "# Existing history\n").unwrap();

        let manifest = root.path().join("repos.manifest.toml");
        fs::write(
            &manifest,
            format!(
                r#"release_version = "8.0.0"
dependency_tag_suffix = "v8.0.0-split.1"

[external_dependencies.redline]
immutable_tag = "redline-core-v4.1.0-jain.3"

[[infrastructure_repo]]
name = "infra"
immutable_tag = "infra-v8.0.0-split.1"

[[repo]]
name = "example"
path = {:?}
current_tag = "example-v8.0.0-split.0"

[[repo]]
name = "already-bumped"
path = {:?}
current_tag = "already-bumped-v8.0.1-split.2"
"#,
                product, product
            ),
        )
        .unwrap();
        let args = || {
            vec![
                "--manifest".to_owned(),
                manifest.display().to_string(),
                "--from".to_owned(),
                "8.0.0".to_owned(),
                "--new".to_owned(),
                "8.0.1".to_owned(),
                "--rewrite-split-tags".to_owned(),
            ]
        };

        bump_version(args()).unwrap();
        let first = fs::read_to_string(&manifest).unwrap();
        assert!(first.contains("release_version = \"8.0.1\""));
        assert!(first.contains("dependency_tag_suffix = \"v8.0.1-split.1\""));
        assert!(first.contains("immutable_tag = \"infra-v8.0.1-split.1\""));
        assert!(first.contains("current_tag = \"example-v8.0.1-split.0\""));
        assert!(first.contains("current_tag = \"already-bumped-v8.0.1-split.2\""));
        assert!(first.contains("immutable_tag = \"redline-core-v4.1.0-jain.3\""));
        assert_eq!(
            fs::read_to_string(&version_file).unwrap(),
            "example-v8.0.0-split.1\n"
        );
        assert!(fs::read_to_string(&cargo_file)
            .unwrap()
            .contains("version = \"8.0.0\""));
        assert_eq!(
            fs::read_to_string(&changelog).unwrap(),
            "# Existing history\n"
        );

        bump_version(args()).unwrap();
        assert_eq!(fs::read_to_string(&manifest).unwrap(), first);
    }

    #[test]
    fn bump_version_rejects_malformed_revisions_without_writing() {
        let root = TestDir::new("bump-version-malformed");
        let manifest = root.path().join("repos.manifest.toml");
        let original = "release_version = \"8.0.0\"\ndependency_tag_suffix = \"v8.0.0-split.01\"\n";
        fs::write(&manifest, original).unwrap();
        let result = bump_version(vec![
            "--manifest".to_owned(),
            manifest.display().to_string(),
            "--from".to_owned(),
            "8.0.0".to_owned(),
            "--new".to_owned(),
            "8.0.1".to_owned(),
            "--rewrite-split-tags".to_owned(),
        ]);
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("canonical nonnegative integer"));
        assert_eq!(fs::read_to_string(manifest).unwrap(), original);
    }

    #[test]
    fn bump_version_rejects_implicit_lock_and_live_repo_updates() {
        let root = TestDir::new("bump-version-lock");
        let manifest = root.path().join("repos.manifest.toml");
        let original = "release_version = \"8.0.0\"\ndependency_tag_suffix = \"v8.0.0-split.0\"\n";
        fs::write(&manifest, original).unwrap();
        let result = bump_version(vec![
            "--manifest".to_owned(),
            manifest.display().to_string(),
            "--update-lock-shas".to_owned(),
        ]);
        assert!(result.unwrap_err().to_string().contains("was removed"));
        assert_eq!(fs::read_to_string(manifest).unwrap(), original);
    }

    #[test]
    fn canonical_rollout_topology_orders_contracts_and_battle_gpu_before_core_consumers() {
        let manifest: toml::Value = fs::read_to_string(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("repos.manifest.toml"),
        )
        .unwrap()
        .parse()
        .unwrap();
        let mut errors = Vec::new();
        validate_rollout_waves(&manifest, &mut errors).unwrap();
        assert!(errors.is_empty(), "{}", errors.join("\n"));

        let core = release_repo_entry(&manifest, "jain-core").unwrap();
        assert!(strings(core, "cross_repo_deps").contains(&"jain-battle-gpu".to_owned()));
        let core_wave = core
            .get("rollout_wave")
            .and_then(toml::Value::as_integer)
            .unwrap();
        let contracts = release_repo_entry(&manifest, "jain-contracts").unwrap();
        let contracts_wave = contracts
            .get("rollout_wave")
            .and_then(toml::Value::as_integer)
            .unwrap();
        assert!(contracts_wave > core_wave);
    }

    #[test]
    fn rollout_validation_rejects_negative_unreachable_and_all_pending_deadlocks() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("repos.manifest.toml");
        let canonical: toml::Value = fs::read_to_string(path).unwrap().parse().unwrap();

        let mut negative = canonical.clone();
        negative
            .get_mut("rollout_wave_order")
            .and_then(toml::Value::as_array_mut)
            .unwrap()[0] = toml::Value::Integer(-1);
        let mut errors = Vec::new();
        validate_rollout_waves(&negative, &mut errors).unwrap();
        assert!(errors.iter().any(|error| error.contains("negative wave")));

        let mut unreachable = canonical.clone();
        unreachable
            .get_mut("rollout_wave_order")
            .and_then(toml::Value::as_array_mut)
            .unwrap()
            .retain(|value| value.as_integer() != Some(4));
        let mut errors = Vec::new();
        validate_rollout_waves(&unreachable, &mut errors).unwrap();
        assert!(errors
            .iter()
            .any(|error| error.contains("not reachable from rollout_wave_order")));

        let mut deadlocked = canonical;
        let contracts = deadlocked
            .get_mut("repo")
            .and_then(toml::Value::as_array_mut)
            .unwrap()
            .iter_mut()
            .find(|repo| string(repo, "name").as_deref() == Some("jain-contracts"))
            .unwrap();
        contracts
            .as_table_mut()
            .unwrap()
            .insert("rollout_wave".to_owned(), toml::Value::Integer(3));
        let mut errors = Vec::new();
        validate_rollout_waves(&deadlocked, &mut errors).unwrap();
        assert!(errors
            .iter()
            .any(|error| error.contains("all-PENDING rollout cannot reach jain-contracts")));
    }

    #[test]
    fn canonical_manifest_metadata_is_candidate_only_and_rollback_bound() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("repos.manifest.toml");
        let canonical: toml::Value = fs::read_to_string(&path).unwrap().parse().unwrap();
        validate_manifest_data(&canonical, &path, false).unwrap();

        for (field, invalid, expected_error) in [
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
            let mut data = canonical.clone();
            data.as_table_mut()
                .unwrap()
                .insert(field.to_owned(), invalid);
            let error = validate_manifest_data(&data, &path, false).unwrap_err();
            assert!(error.to_string().contains(expected_error));
        }
    }

    #[test]
    fn pending_redline_identity_is_structural_but_blocks_release_operations() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("repos.manifest.toml");
        let canonical: toml::Value = fs::read_to_string(&path).unwrap().parse().unwrap();
        validate_manifest_data(&canonical, &path, false).unwrap();
        assert_eq!(
            external_dependency_failures(&canonical),
            vec!["Redline reviewed successor identity is pending; bind the immutable tag explicitly after its protected merge and tag"]
        );

        let root = TestDir::new("pending-redline-lock");
        let manifest = root.path().join("repos.manifest.toml");
        fs::write(&manifest, toml::to_string_pretty(&canonical).unwrap()).unwrap();
        let output = root.path().join("family.lock");
        let error = regenerate_lock(vec![
            "--manifest".to_owned(),
            manifest.display().to_string(),
            "--output".to_owned(),
            output.display().to_string(),
            "--apply".to_owned(),
        ])
        .unwrap_err();
        assert!(error
            .to_string()
            .contains("Redline reviewed identity is pending"));
        assert!(!output.exists());

        let mut malformed = canonical;
        let redline = malformed
            .get_mut("external_dependencies")
            .and_then(|value| value.get_mut("redline"))
            .and_then(toml::Value::as_table_mut)
            .unwrap();
        redline.insert(
            "identity_status".to_owned(),
            toml::Value::String("bound".to_owned()),
        );
        redline.insert(
            "immutable_tag".to_owned(),
            toml::Value::String("redline-core-v4.1.0-jain.04".to_owned()),
        );
        let nested = malformed
            .get_mut("nested_families")
            .and_then(|value| value.get_mut("redline"))
            .and_then(toml::Value::as_table_mut)
            .unwrap();
        nested.insert(
            "engine_identity_status".to_owned(),
            toml::Value::String("bound".to_owned()),
        );
        nested.insert(
            "engine_tag".to_owned(),
            toml::Value::String("redline-core-v4.1.0-jain.04".to_owned()),
        );
        assert!(validate_manifest_data(&malformed, &path, false)
            .unwrap_err()
            .to_string()
            .contains("canonical nonnegative integer"));
    }

    #[test]
    fn manifest_accepts_lawful_split_revisions_and_rejects_malformed_ones() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("repos.manifest.toml");
        let canonical: toml::Value = fs::read_to_string(&path).unwrap().parse().unwrap();
        let mut revised = canonical.clone();
        revised.as_table_mut().unwrap().insert(
            "dependency_tag_suffix".to_owned(),
            toml::Value::String("v8.0.1-split.2".to_owned()),
        );
        revised
            .get_mut("repo")
            .and_then(toml::Value::as_array_mut)
            .unwrap()[0]
            .as_table_mut()
            .unwrap()
            .insert(
                "current_tag".to_owned(),
                toml::Value::String("jain-v8.0.1-split.1".to_owned()),
            );
        revised
            .get_mut("infrastructure_repo")
            .and_then(toml::Value::as_array_mut)
            .unwrap()[0]
            .as_table_mut()
            .unwrap()
            .insert(
                "immutable_tag".to_owned(),
                toml::Value::String("jain-smartcluster-v8.0.1-split.2".to_owned()),
            );
        validate_manifest_data(&revised, &path, false).unwrap();

        let mut malformed = revised;
        malformed
            .get_mut("repo")
            .and_then(toml::Value::as_array_mut)
            .unwrap()[0]
            .as_table_mut()
            .unwrap()
            .insert(
                "current_tag".to_owned(),
                toml::Value::String("jain-v8.0.1-split.01".to_owned()),
            );
        assert!(validate_manifest_data(&malformed, &path, false)
            .unwrap_err()
            .to_string()
            .contains("canonical nonnegative integer"));
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
    fn jeryu_lifecycle_plans_exact_requests_and_policy() {
        let ready =
            plan_jeryu_lifecycle_request("pr-ready", "jeryu/example", Some("7"), "main", None)
                .unwrap();
        assert_eq!(ready.method, "PATCH");
        assert_eq!(ready.path, "/repos/jeryu/example/pulls/7");
        assert_eq!(
            serde_json::from_str::<JsonValue>(ready.body.as_deref().unwrap()).unwrap(),
            json!({"draft": false})
        );
        let close =
            plan_jeryu_lifecycle_request("pr-close", "jeryu/example", Some("7"), "main", None)
                .unwrap();
        assert_eq!(
            serde_json::from_str::<JsonValue>(close.body.as_deref().unwrap()).unwrap(),
            json!({"state": "closed"})
        );
        let merge =
            plan_jeryu_lifecycle_request("pr-merge", "jeryu/example", Some("7"), "main", None)
                .unwrap();
        assert_eq!(merge.method, "PUT");
        assert_eq!(merge.path, "/repos/jeryu/example/pulls/7/merge");
        assert_eq!(merge.body.as_deref(), Some("{}"));
        let protection = plan_jeryu_lifecycle_request(
            "protection-apply",
            "jeryu/example",
            None,
            "main",
            Some("example/required"),
        )
        .unwrap();
        let policy: JsonValue = serde_json::from_str(protection.body.as_deref().unwrap()).unwrap();
        validate_protection_policy(&policy, "example/required").unwrap();
        assert!(validate_protection_policy(&json!({}), "example/required").is_err());
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
        assert_eq!(request.method, "POST");
        assert_eq!(
            request.path,
            "/api/v1/repos/jeryu%2Fexample/pulls/7/reviews"
        );
        let body: JsonValue = serde_json::from_str(request.body.as_deref().unwrap()).unwrap();
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
        let mut policy = immutable_main_policy("example/required");
        policy["enforce_admins"] = json!(false);
        assert!(validate_protection_policy(&policy, "example/required").is_err());

        policy["enforce_admins"] = json!(true);
        validate_protection_policy(&policy, "example/required").unwrap();
    }

    #[test]
    fn jeryu_lifecycle_dry_run_needs_no_token_and_writes_receipt() {
        let root = TestDir::new("jeryu-dry-run");
        let receipt = root.path().join("ready.json");
        jeryu_lifecycle(vec![
            "pr-ready".to_owned(),
            "--repo".to_owned(),
            "jeryu/example".to_owned(),
            "--number".to_owned(),
            "3".to_owned(),
            "--receipt".to_owned(),
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
current_tag = "redline-core-v4.1.0-jain.3"
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
            .ends_with("docs/release-evidence/8.0.1/receipt.json"));
        let root = TestDir::new("derived-sync");
        let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("repos.manifest.toml");
        let mut canonical: toml::Value = fs::read_to_string(source).unwrap().parse().unwrap();
        let portal = root.path().join("derived-manifests/portal.toml");
        let deploy = root.path().join("derived-manifests/deploy.toml");
        let mut targets = toml::map::Map::new();
        for (name, consumer) in [
            ("portal", "/home/ubuntu/jain-split/jain/repos.manifest.toml"),
            (
                "deploy",
                "/home/ubuntu/jain-split/jain-deploy/repos.manifest.toml",
            ),
        ] {
            let mut target = toml::map::Map::new();
            target.insert(
                "path".to_owned(),
                toml::Value::String(format!("derived-manifests/{name}.toml")),
            );
            target.insert(
                "consumer_path".to_owned(),
                toml::Value::String(consumer.to_owned()),
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
}
