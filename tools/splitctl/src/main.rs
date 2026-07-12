// Repository-local release and Jeryu control-plane CLI.
use serde_json::{json, Map, Value as JsonValue};
use sha2::{Digest, Sha256};
use std::{
    env, fs, io,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

const RELEASE_VERSION: &str = "8.0.0";
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
        Some("regenerate-lock") => regenerate_lock(args.collect())?,
        Some("release-preflight") => release_preflight(args.collect())?,
        Some("release-snapshot") => release_snapshot(args.collect())?,
        Some("release-status") => release_status(args.collect())?,
        Some("bootstrap-main") => bootstrap_main_command(args.collect())?,
        Some("immutable-tag") => immutable_tag_command(args.collect())?,
        Some("defer-worktree") => defer_worktree_command(args.collect())?,
        Some("verify-worktrees") => verify_worktrees_command(args.collect())?,
        Some("reconcile") => reconcile(args.collect())?,
        Some("bump-version") => bump_version(args.collect())?,
        Some("--version") | Some("version") => println!("splitctl 0.1.0"),
        _ => return Err("usage: splitctl refresh-ci-contract [--repo NAME]... | materialize [--repo NAME]... | manifest [--manifest PATH] [--json] | managed-repos [--manifest PATH] --json | release-cargo-commands [--manifest PATH] --repo NAME | sync-derived-manifests [--manifest PATH] [--receipt PATH] [--apply] | validate-manifest [--manifest PATH] [--check-paths] [--check-derived] | validate-local-jeryu [--manifest PATH] [--skip-remotes] | validate-family [--manifest PATH] [--json PATH] | validate-family-lock [--manifest PATH] [--lock PATH] | regenerate-lock [--manifest PATH] [--output PATH] --apply | release-preflight [--manifest PATH] [--json PATH] | release-snapshot [--manifest PATH] [--json PATH] | release-status [--manifest PATH] [--json PATH] | bootstrap-main --repo PATH --remote URL --reviewed-commit SHA [--receipt PATH] [--apply] | immutable-tag --repo PATH --remote URL --tag TAG --commit SHA [--receipt PATH] [--apply] | defer-worktree --repo PATH --destination PATH --expected-head SHA [--receipt PATH] [--apply] | verify-worktrees [--manifest PATH] [--receipt PATH] | preflight [--manifest PATH] [--json PATH] | source-coverage [--manifest PATH] [--json] | python-boundary [--receipt PATH] | jeryu-doctor [--manifest PATH] | reconcile [--manifest PATH] [--base-ref REF] [--apply] [--json PATH] | bump-version [--manifest PATH] --from VERSION --new VERSION --rewrite-split-tags".into()),
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
    if names.contains("jain-python")
        || repos
            .iter()
            .any(|raw| string(raw, "profile").as_deref() == Some("python"))
    {
        errors.push(
            "product Python repositories are forbidden; Python is limited to Rust parity tests"
                .to_owned(),
        );
    }
    if !strings(data, "retired_paths")
        .iter()
        .any(|path| path == "python/ai-service/**")
    {
        errors.push("retired_paths must include python/ai-service/**".to_owned());
    }
    if repos.len() != 25 {
        errors.push(format!(
            "expected 25 Rust/parity-policy family repositories, found {}",
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
    validate_tag_release_identity(repo, tag, &reviewed, report)?;
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

fn validate_tag_release_identity(
    repo: &Path,
    tag: &str,
    commit: &str,
    report: &mut JsonValue,
) -> Result<(), Box<dyn std::error::Error>> {
    let expected = product_version_from_tag(tag)?;
    if tag.contains("-split.") && expected != RELEASE_VERSION {
        report["release_identity"] = json!({
            "status": "fail",
            "expected_product_version": RELEASE_VERSION,
            "tag_product_version": expected,
        });
        return Err(format!(
            "split tag {tag} encodes product version {expected}, expected {RELEASE_VERSION}"
        )
        .into());
    }

    let version_file = git_file_at_commit(repo, commit, "VERSION")?;
    if let Some(value) = version_file.as_deref() {
        if value.trim() != tag {
            report["release_identity"] = json!({
                "status": "fail",
                "expected_product_version": expected,
                "version_file": value.trim(),
                "expected_version_file": tag,
            });
            return Err(format!(
                "VERSION at {commit} is {:?}, expected exact immutable tag {tag:?}",
                value.trim()
            )
            .into());
        }
    }

    let Some(root_manifest) = git_file_at_commit(repo, commit, "Cargo.toml")? else {
        report["release_identity"] = json!({
            "status": "pass",
            "expected_product_version": expected,
            "version_file": version_file.as_deref().map(str::trim),
            "cargo_packages": [],
        });
        return Ok(());
    };
    let root: toml::Value = root_manifest.parse()?;
    let workspace_version = root
        .get("workspace")
        .and_then(|value| value.get("package"))
        .and_then(|value| value.get("version"))
        .and_then(toml::Value::as_str)
        .map(str::to_owned);
    if let Some(version) = workspace_version.as_deref() {
        if version != expected {
            report["release_identity"] = json!({
                "status": "fail",
                "expected_product_version": expected,
                "workspace_package_version": version,
            });
            return Err(format!(
                "workspace.package.version at {commit} is {version}, expected {expected}"
            )
            .into());
        }
    }

    let tracked = strict_git_output(repo, &["ls-tree", "-r", "--name-only", commit])?;
    let members = root
        .get("workspace")
        .and_then(|value| value.get("members"))
        .and_then(toml::Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(toml::Value::as_str)
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let mut manifests = Vec::new();
    if root.get("package").is_some() {
        manifests.push(("Cargo.toml".to_owned(), root));
    }
    for path in tracked.lines().filter(|path| path.ends_with("/Cargo.toml")) {
        let directory = path.trim_end_matches("/Cargo.toml");
        if !members
            .iter()
            .any(|pattern| workspace_member_matches(pattern, directory))
        {
            continue;
        }
        let content = git_file_at_commit(repo, commit, path)?
            .ok_or_else(|| format!("workspace member manifest disappeared at {commit}:{path}"))?;
        manifests.push((path.to_owned(), content.parse()?));
    }
    let mut packages = Vec::new();
    for (path, manifest) in manifests {
        let Some(package) = manifest.get("package") else {
            continue;
        };
        let name = package
            .get("name")
            .and_then(toml::Value::as_str)
            .unwrap_or("<unnamed>");
        let declared = package
            .get("version")
            .ok_or_else(|| format!("{path}: package {name} has no version"))?;
        let version = if let Some(value) = declared.as_str() {
            value
        } else if declared.get("workspace").and_then(toml::Value::as_bool) == Some(true) {
            workspace_version.as_deref().ok_or_else(|| {
                format!("{path}: package {name} inherits a missing workspace.package.version")
            })?
        } else {
            return Err(
                format!("{path}: package {name} has an unsupported version declaration").into(),
            );
        };
        packages.push(json!({"manifest": path.clone(), "name": name, "version": version}));
        if version != expected {
            report["release_identity"] = json!({
                "status": "fail",
                "expected_product_version": expected,
                "version_file": version_file.as_deref().map(str::trim),
                "cargo_packages": packages,
                "mismatch": {"manifest": path, "name": name, "version": version},
            });
            return Err(format!(
                "{path}: package {name} version is {version}, expected {expected} from tag {tag}"
            )
            .into());
        }
    }
    report["release_identity"] = json!({
        "status": "pass",
        "expected_product_version": expected,
        "version_file": version_file.as_deref().map(str::trim),
        "workspace_package_version": workspace_version,
        "cargo_packages": packages,
    });
    Ok(())
}

fn product_version_from_tag(tag: &str) -> Result<&str, Box<dyn std::error::Error>> {
    let suffix = ["-split.", "-jain."]
        .iter()
        .filter_map(|marker| tag.rfind(marker))
        .max()
        .ok_or_else(|| format!("release tag has no supported -split.N or -jain.N suffix: {tag}"))?;
    let prefix = &tag[..suffix];
    let version_start = prefix
        .rfind("-v")
        .map(|index| index + 2)
        .ok_or_else(|| format!("release tag has no product version: {tag}"))?;
    let version = &tag[version_start..suffix];
    if version.is_empty()
        || !version
            .split('.')
            .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
    {
        return Err(format!("release tag has an invalid product version: {tag}").into());
    }
    Ok(version)
}

fn git_file_at_commit(
    repo: &Path,
    commit: &str,
    path: &str,
) -> Result<Option<String>, Box<dyn std::error::Error>> {
    let object = format!("{commit}:{path}");
    let exists = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["cat-file", "-e", &object])
        .output()?;
    if !exists.status.success() {
        return Ok(None);
    }
    Ok(Some(strict_git_output(repo, &["show", &object])?))
}

fn workspace_member_matches(pattern: &str, directory: &str) -> bool {
    let pattern = pattern.trim_end_matches('/');
    let pattern_parts = pattern.split('/').collect::<Vec<_>>();
    let directory_parts = directory.split('/').collect::<Vec<_>>();
    pattern_parts.len() == directory_parts.len()
        && pattern_parts
            .iter()
            .zip(directory_parts)
            .all(|(expected, actual)| *expected == "*" || *expected == actual)
}

fn defer_worktree_command(args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    let mut repo = None;
    let mut destination = None;
    let mut expected_head = None;
    let mut receipt = None;
    let mut apply = false;
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--repo" => repo = Some(PathBuf::from(iter.next().ok_or("--repo needs a path")?)),
            "--destination" => {
                destination = Some(PathBuf::from(
                    iter.next().ok_or("--destination needs a path")?,
                ))
            }
            "--expected-head" => {
                expected_head = Some(iter.next().ok_or("--expected-head needs a SHA")?)
            }
            "--receipt" => {
                receipt = Some(PathBuf::from(iter.next().ok_or("--receipt needs a path")?))
            }
            "--apply" => apply = true,
            value => return Err(format!("unknown defer-worktree argument: {value}").into()),
        }
    }
    let repo = repo.ok_or("defer-worktree requires --repo")?;
    let destination = destination.ok_or("defer-worktree requires --destination")?;
    let expected_head = expected_head.ok_or("defer-worktree requires --expected-head")?;
    let receipt = receipt.unwrap_or_else(|| release_evidence_path("deferred-worktree.json"));
    let mut report = receipt_header("jain.deferred-worktree/v1", "defer-worktree", apply);
    report["source"] = json!(repo);
    report["destination"] = json!(destination);
    report["expected_head"] = json!(expected_head);
    let result = defer_worktree(&repo, &destination, &expected_head, apply, &mut report);
    finish_receipted_operation(&receipt, &mut report, result)
}

fn defer_worktree(
    repo: &Path,
    destination: &Path,
    expected_head: &str,
    apply: bool,
    report: &mut JsonValue,
) -> Result<(), Box<dyn std::error::Error>> {
    if destination.starts_with(repo) {
        return Err("deferred destination must not be inside the source worktree".into());
    }
    if !repo.exists() {
        if !destination.exists() {
            return Err("source and deferred destination are both absent".into());
        }
        let existing = worktree_identity(destination)?;
        require_expected_worktree_head(&existing, expected_head)?;
        report["action"] = json!("verified-existing");
        report["before"] = JsonValue::Null;
        report["after"] = existing;
        return Ok(());
    }
    if destination.exists() {
        return Err(format!(
            "refusing to overwrite existing deferred destination {}",
            destination.display()
        )
        .into());
    }
    if !repo.join(".git").exists() {
        return Err(format!("source is not a Git worktree: {}", repo.display()).into());
    }
    let before = worktree_identity(repo)?;
    require_expected_worktree_head(&before, expected_head)?;
    report["before"] = before.clone();
    if !apply {
        report["action"] = json!("would-move");
        report["after"] = JsonValue::Null;
        return Ok(());
    }
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::rename(repo, destination)?;
    let after = match worktree_identity(destination) {
        Ok(identity) => identity,
        Err(error) => {
            let rollback = fs::rename(destination, repo);
            return Err(format!(
                "deferred worktree verification failed: {error}; rollback {}",
                if rollback.is_ok() {
                    "succeeded"
                } else {
                    "failed"
                }
            )
            .into());
        }
    };
    if before != after {
        let rollback = fs::rename(destination, repo);
        return Err(format!(
            "deferred worktree identity changed during move; rollback {}",
            if rollback.is_ok() {
                "succeeded"
            } else {
                "failed"
            }
        )
        .into());
    }
    report["action"] = json!("moved-and-verified");
    report["after"] = after;
    Ok(())
}

fn require_expected_worktree_head(
    identity: &JsonValue,
    expected: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let actual = identity["head"].as_str().unwrap_or("<missing>");
    if actual == expected {
        Ok(())
    } else {
        Err(format!("worktree HEAD is {actual}, expected {expected}").into())
    }
}

fn worktree_identity(repo: &Path) -> Result<JsonValue, Box<dyn std::error::Error>> {
    let head = resolve_commit(repo, "HEAD")?;
    let branch_output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["symbolic-ref", "--quiet", "--short", "HEAD"])
        .output()?;
    let branch = if branch_output.status.success() {
        Some(String::from_utf8(branch_output.stdout)?.trim().to_owned())
    } else {
        None
    };
    let status = strict_git_output(repo, &["status", "--porcelain=v2", "--branch"])?;
    let remotes = strict_git_output(repo, &["remote", "-v"])?;
    let refs = strict_git_output(
        repo,
        &[
            "for-each-ref",
            "--sort=refname",
            "--format=%(objectname) %(refname)",
            "refs/heads",
            "refs/tags",
        ],
    )?;
    Ok(json!({
        "head": head,
        "branch": branch,
        "status_porcelain_v2": status,
        "remotes": remotes,
        "refs_sha256": sha256_bytes(refs.as_bytes()),
    }))
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

fn python_boundary(args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    let mut receipt = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/python-boundary.json");
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--receipt" => receipt = PathBuf::from(iter.next().ok_or("--receipt needs a path")?),
            value => return Err(format!("unknown python-boundary argument: {value}").into()),
        }
    }
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
        if let Some((purpose, rust_evidence)) = python_parity_declaration(&rel) {
            if root.join(rust_evidence).exists() {
                declared.push(json!({
                    "path": rel,
                    "purpose": purpose,
                    "rust_evidence": rust_evidence,
                }));
            } else {
                unexpected.push(format!(
                    "{rel} (declared Rust evidence is missing: {rust_evidence})"
                ));
            }
        } else {
            unexpected.push(rel);
        }
    }
    let mut runtime_files = Vec::new();
    collect_python_runtime_files(&root, &mut runtime_files)?;
    let mut allowed_invocations = Vec::new();
    let mut unexpected_invocations = Vec::new();
    for path in runtime_files {
        let rel = path
            .strip_prefix(&root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        let content = fs::read_to_string(&path)?;
        for (index, line) in content.lines().enumerate() {
            if !line_has_python_runtime(line) {
                continue;
            }
            let row = json!({"path": rel, "line": index + 1, "text": line.trim()});
            if python_parity_invocation_allowed(&rel, line) {
                allowed_invocations.push(row);
            } else {
                unexpected_invocations.push(row);
            }
        }
    }
    let mut report = receipt_header(
        "jain.rust-parity-python-boundary/v1",
        "python-boundary",
        false,
    );
    report["policy"] =
        json!("Python is permitted only for parity testing against Rust implementations");
    report["allowed"] = json!(declared);
    report["unexpected"] = json!(unexpected);
    report["allowed_runtime_invocations"] = json!(allowed_invocations);
    report["unexpected_runtime_invocations"] = json!(unexpected_invocations);
    report["allowed_count"] = json!(report["allowed"].as_array().map_or(0, Vec::len));
    report["unexpected_count"] = json!(report["unexpected"].as_array().map_or(0, Vec::len));
    report["allowed_runtime_invocation_count"] = json!(report["allowed_runtime_invocations"]
        .as_array()
        .map_or(0, Vec::len));
    report["unexpected_runtime_invocation_count"] = json!(report["unexpected_runtime_invocations"]
        .as_array()
        .map_or(0, Vec::len));
    let result =
        if report["unexpected_count"] == 0 && report["unexpected_runtime_invocation_count"] == 0 {
            println!(
                "python boundary ok: {} Rust-parity files; {} approved interpreter invocations",
                report["allowed_count"], report["allowed_runtime_invocation_count"]
            );
            Ok(())
        } else {
            let mut violations = report["unexpected"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(JsonValue::as_str)
                .map(str::to_owned)
                .collect::<Vec<_>>();
            violations.extend(
                report["unexpected_runtime_invocations"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|row| {
                        format!(
                            "{}:{}: {}",
                            row["path"].as_str().unwrap_or("<unknown>"),
                            row["line"].as_u64().unwrap_or(0),
                            row["text"].as_str().unwrap_or("<unknown>")
                        )
                    }),
            );
            Err(format!(
                "Python outside the Rust-parity boundary:\n{}",
                violations.join("\n")
            )
            .into())
        };
    finish_receipted_operation(&receipt, &mut report, result)
}

fn python_parity_declaration(path: &str) -> Option<(&'static str, &'static str)> {
    const ALLOWED: &[(&str, &str, &str)] = &[
        (
            "jain-deploy/ops/ci/testdata/invention-export/model.py",
            "fixture consumed by Rust export compatibility tests",
            "jain-deploy/ops/ci/testdata/invention-export/model.rs",
        ),
        (
            "jain-model-zoo/ops/parity/parity_suite.py",
            "Rust model parity harness",
            "jain-model-zoo/ops/parity/run.sh",
        ),
        (
            "jain-model-zoo/ops/parity/plot_errors.py",
            "Rust model parity report renderer",
            "jain-model-zoo/ops/parity/parity_errors.json",
        ),
        (
            "jain-model-zoo/reference/ported/tabicl/oracle/dump_oracle.py",
            "frozen oracle for a Rust model port",
            "jain-model-zoo/reference/ported/tabicl/tests/e2e_parity.rs",
        ),
        (
            "jain-model-zoo/reference/ported/tabdpt_classifier/oracle/dump_oracle.py",
            "frozen oracle for a Rust model port",
            "jain-model-zoo/reference/ported/tabdpt_classifier/src/bin/verify_oracle.rs",
        ),
        (
            "jain-model-zoo/reference/ported/tabdpt_regressor/oracle/dump_oracle.py",
            "frozen oracle for a Rust model port",
            "jain-model-zoo/reference/ported/tabdpt_regressor/src/bin/verify_oracle.rs",
        ),
        (
            "jain-model-zoo/reference/ported/nm_copula_log_density_estimate_ridge/parity/oracle.py",
            "frozen oracle for a Rust model port",
            "jain-model-zoo/reference/ported/nm_copula_log_density_estimate_ridge/tests/known_values.rs",
        ),
        (
            "jain-model-zoo/reference/ported/nm_copula_gaussian_loglik_ridge/parity/oracle.py",
            "frozen oracle for a Rust model port",
            "jain-model-zoo/reference/ported/nm_copula_gaussian_loglik_ridge/tests/known_values.rs",
        ),
        (
            "jain-model-zoo/reference/ported/_nm/parity/regression_oracle.py",
            "frozen oracle for a Rust model port",
            "jain-model-zoo/reference/ported/_nm/tests/nm_regression_report.rs",
        ),
        (
            "jain-model-zoo/reference/ported/_nm/parity/oracle.py",
            "frozen oracle for a Rust model port",
            "jain-model-zoo/reference/ported/_nm/tests/nm_compressed_rank_parity.rs",
        ),
        (
            "jain-model-zoo/reference/ported/_nm/parity/nm_xform1_oracle.py",
            "frozen oracle for a Rust model port",
            "jain-model-zoo/reference/ported/_nm/tests/nm_xform1_parity.rs",
        ),
        (
            "jain-model-zoo/reference/ported/_nm/parity/nm_xform2_oracle.py",
            "frozen oracle for a Rust model port",
            "jain-model-zoo/reference/ported/_nm/tests/nm_xform2_parity.rs",
        ),
        (
            "jain-model-zoo/reference/ported/_nm/parity/nm_xform3_oracle.py",
            "frozen oracle for a Rust model port",
            "jain-model-zoo/reference/ported/_nm/tests/nm_xform3_parity.rs",
        ),
        (
            "jain-model-zoo/reference/ported/_nm/parity/nm_xform4_oracle.py",
            "frozen oracle for a Rust model port",
            "jain-model-zoo/reference/ported/_nm/tests/nm_xform4_parity.rs",
        ),
    ];
    ALLOWED
        .iter()
        .find(|(allowed, _, _)| *allowed == path)
        .map(|(_, purpose, evidence)| (*purpose, *evidence))
}

fn collect_python_runtime_files(root: &Path, out: &mut Vec<PathBuf>) -> io::Result<()> {
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
            collect_python_runtime_files(&path, out)?;
            continue;
        }
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        let extension = path.extension().and_then(|value| value.to_str());
        let runtime_source = matches!(extension, Some("sh" | "bash" | "yml" | "yaml" | "just"))
            || name == "Justfile"
            || name == "Makefile"
            || name.starts_with("Dockerfile")
            || name == "pre-commit"
            || name.starts_with("pre-push")
            || name == "generated-zones.toml"
            || name == "test-map.json";
        if runtime_source {
            out.push(path);
        }
    }
    Ok(())
}

fn line_has_python_runtime(line: &str) -> bool {
    let trimmed = line.trim_start();
    if trimmed.starts_with('#') {
        return false;
    }
    if trimmed.starts_with("python_command=") {
        return false;
    }
    if line.contains("actions/setup-python@") {
        return true;
    }
    line.split(|character: char| {
        character.is_whitespace()
            || matches!(
                character,
                '\'' | '"'
                    | '('
                    | ')'
                    | '{'
                    | '}'
                    | '['
                    | ']'
                    | ';'
                    | '|'
                    | '&'
                    | '!'
                    | ','
                    | ':'
                    | '\\'
                    | '$'
                    | '='
            )
    })
    .any(|token| {
        matches!(
            token,
            "python" | "python3" | "python3-pip" | "pytest" | "ruff" | "pip" | "pip3"
        )
    })
}

fn python_parity_invocation_allowed(path: &str, line: &str) -> bool {
    (path == "jain-model-zoo/ops/parity/run.sh" && line.contains("ops/parity/parity_suite.py"))
        || ((path == "jain-model-zoo/reference/ported/_nm/parity/verify_parity.sh"
            || path == "jain-model-zoo/reference/ported/_nm/parity/verify_parity_apex67.sh")
            && line.contains("_nm/parity/oracle.py"))
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
    let source_command = if !repo.cargo_members.is_empty() || repo.name == "jain-web" {
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
    fn immutable_tag_refuses_mismatched_cargo_package_identity() {
        let root = TestDir::new("immutable-tag-package-identity");
        let (repo, _) = init_source(root.path());
        fs::create_dir_all(repo.join("crates/example")).unwrap();
        fs::write(
            repo.join("Cargo.toml"),
            "[workspace]\nmembers = [\"crates/*\"]\nresolver = \"2\"\n\n[workspace.package]\nversion = \"8.0.0\"\n",
        )
        .unwrap();
        fs::write(
            repo.join("crates/example/Cargo.toml"),
            "[package]\nname = \"example\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )
        .unwrap();
        fs::write(repo.join("crates/example/src.rs"), "pub fn example() {}\n").unwrap();
        fs::write(repo.join("VERSION"), "example-v8.0.0-split.0\n").unwrap();
        run_git_strict(&repo, &["add", "Cargo.toml", "VERSION", "crates/example"]).unwrap();
        run_git_strict(&repo, &["commit", "-m", "candidate metadata"]).unwrap();
        let reviewed = resolve_commit(&repo, "HEAD").unwrap();
        let remote = init_bare(root.path());
        let main_refspec = format!("{reviewed}:refs/heads/main");
        run_git_strict(&repo, &["push", remote.to_str().unwrap(), &main_refspec]).unwrap();
        let receipt = root.path().join("tag.json");
        let result = immutable_tag_command(vec![
            "--repo".to_owned(),
            repo.display().to_string(),
            "--remote".to_owned(),
            remote.display().to_string(),
            "--tag".to_owned(),
            "example-v8.0.0-split.0".to_owned(),
            "--commit".to_owned(),
            reviewed,
            "--receipt".to_owned(),
            receipt.display().to_string(),
            "--apply".to_owned(),
        ]);
        assert!(result.is_err());
        let report = read_json(&receipt);
        assert_eq!(report["status"], "fail");
        assert_eq!(report["release_identity"]["mismatch"]["version"], "0.1.0");
        assert_eq!(
            local_ref_commit(&repo, "refs/tags/example-v8.0.0-split.0").unwrap(),
            None
        );
        assert_eq!(
            ls_remote_ref(
                &repo,
                remote.to_str().unwrap(),
                "refs/tags/example-v8.0.0-split.0"
            )
            .unwrap(),
            None
        );
    }

    #[test]
    fn python_boundary_allows_only_rust_parity_surfaces() {
        assert_eq!(
            python_parity_declaration("jain-model-zoo/ops/parity/parity_suite.py"),
            Some((
                "Rust model parity harness",
                "jain-model-zoo/ops/parity/run.sh"
            ))
        );
        assert_eq!(
            python_parity_declaration(
                "jain-model-zoo/reference/ported/tabicl/oracle/dump_oracle.py"
            ),
            Some((
                "frozen oracle for a Rust model port",
                "jain-model-zoo/reference/ported/tabicl/tests/e2e_parity.rs"
            ))
        );
        assert_eq!(
            python_parity_declaration("jain-deploy/ops/ci/testdata/invention-export/model.py"),
            Some((
                "fixture consumed by Rust export compatibility tests",
                "jain-deploy/ops/ci/testdata/invention-export/model.rs"
            ))
        );
        assert_eq!(
            python_parity_declaration("jain-python/python/ai-service/src/client.py"),
            None
        );
        assert_eq!(
            python_parity_declaration("redline-split/redline-core/scripts/perf/diff.py"),
            None
        );
        assert_eq!(
            python_parity_declaration("anything/oracle/production.py"),
            None
        );
        assert!(line_has_python_runtime("python3 - <<'PY'"));
        assert!(line_has_python_runtime("if command -v python3; then"));
        assert!(line_has_python_runtime(
            "actual=\"$(python3 -c 'print(1)')\""
        ));
        assert!(line_has_python_runtime("pip install example"));
        assert!(line_has_python_runtime("uses: actions/setup-python@v5"));
        assert!(!line_has_python_runtime("# python3 is forbidden here"));
        assert!(!line_has_python_runtime("python_command=\"python\"\"3\""));
        assert!(!line_has_python_runtime(
            "rm -rf vendor/xgboost/python-package"
        ));
        assert!(python_parity_invocation_allowed(
            "jain-model-zoo/ops/parity/run.sh",
            "exec python3 \"$REPO/ops/parity/parity_suite.py\" \"$@\""
        ));
        assert!(!python_parity_invocation_allowed(
            "somewhere/else.sh",
            "python3 parity_suite.py"
        ));
    }

    #[test]
    fn defer_worktree_is_dry_run_idempotent_and_preserves_dirty_identity() {
        let root = TestDir::new("defer-worktree");
        let (repo, head) = init_source(root.path());
        fs::write(repo.join("preserved-untracked.txt"), "keep me\n").unwrap();
        let destination = root.path().join("deferred/source");
        let receipt = root.path().join("defer.json");
        let args = || {
            vec![
                "--repo".to_owned(),
                repo.display().to_string(),
                "--destination".to_owned(),
                destination.display().to_string(),
                "--expected-head".to_owned(),
                head.clone(),
                "--receipt".to_owned(),
                receipt.display().to_string(),
            ]
        };
        defer_worktree_command(args()).unwrap();
        assert!(repo.exists());
        assert!(!destination.exists());
        assert_eq!(read_json(&receipt)["action"], "would-move");

        let mut apply = args();
        apply.push("--apply".to_owned());
        defer_worktree_command(apply.clone()).unwrap();
        assert!(!repo.exists());
        assert!(destination.join("preserved-untracked.txt").is_file());
        let moved = read_json(&receipt);
        assert_eq!(moved["action"], "moved-and-verified");
        assert_eq!(moved["before"], moved["after"]);

        defer_worktree_command(apply).unwrap();
        let repeated = read_json(&receipt);
        assert_eq!(repeated["action"], "verified-existing");
        assert_eq!(repeated["after"]["head"], head);
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
}
