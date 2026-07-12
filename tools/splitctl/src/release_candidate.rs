use serde_json::{json, Value as JsonValue};
use sha2::{Digest, Sha256};
use std::{
    env, fs,
    fs::File,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Instant, SystemTime, UNIX_EPOCH},
};

const RELEASE_VERSION: &str = "8.0.0";
const PENDING: &str = "PENDING";
const DEFAULT_MAX_AGE_HOURS: u64 = 24;
const USAGE: &str = "usage: release-candidate [--plan] [--repo NAME]... [--from-wave N] [--through-wave N] [--force] [--no-tags] [--no-atomicsoul] [--max-age-hours N] [--manifest PATH] [--evidence-dir PATH]";

#[derive(Debug, Clone)]
struct Options {
    manifest: PathBuf,
    evidence_dir: PathBuf,
    plan: bool,
    force: bool,
    apply_tags: bool,
    atomicsoul: bool,
    selected: Vec<String>,
    from_wave: u64,
    through_wave: u64,
    max_age_hours: u64,
}

#[derive(Debug, Clone)]
struct FleetRepo {
    name: String,
    path: PathBuf,
    remote: String,
    required_check: String,
    expected_branch: String,
    tag: String,
    release_commit: String,
    wave: u64,
    kind: String,
    policy_sha256: String,
}

pub(crate) fn run(args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    if help_requested(&args) {
        println!("{USAGE}");
        return Ok(());
    }
    let options = parse_options(args)?;
    reject_unsafe_environment()?;
    let manifest_bytes = fs::read(&options.manifest)?;
    let manifest_sha256 = sha256_bytes(&manifest_bytes);
    let manifest: toml::Value = std::str::from_utf8(&manifest_bytes)?.parse()?;
    validate_candidate_header(&manifest)?;
    validate_rollout_dependencies(&manifest)?;
    let mut repositories = fleet_repositories(&manifest, &options.manifest)?;
    repositories.retain(|repo| selected(&options, repo));
    repositories.sort_by(|left, right| (left.wave, &left.name).cmp(&(right.wave, &right.name)));

    fs::create_dir_all(&options.evidence_dir)?;
    let aggregate_path = options
        .evidence_dir
        .join(aggregate_receipt_name(options.plan));
    let mut report = json!({
        "schema_version": "jain.release-candidate-runner/v1",
        "release_version": RELEASE_VERSION,
        "release_status": "candidate",
        "formal_ga": false,
        "sagemaker": "N/A",
        "mode": if options.plan {"plan"} else {"execute"},
        "manifest": options.manifest,
        "manifest_sha256": manifest_sha256,
        "atomicsoul_push": false,
        "production_applied": false,
        "external_production_mutations": [],
        "repository_count": repositories.len(),
        "status": "pending",
        "started_at_unix": now_unix(),
        "steps": [],
    });
    report["plan"] = json!(repositories.iter().map(repo_plan).collect::<Vec<_>>());
    if options.plan {
        report["status"] = json!("planned");
        report["finished_at_unix"] = json!(now_unix());
        write_json(&aggregate_path, &report)?;
        println!("release candidate plan: {}", aggregate_path.display());
        return Ok(());
    }

    let mut steps = Vec::new();
    let executable = env::current_exe()?;
    let control_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    let mut validate_manifest = Command::new(&executable);
    validate_manifest
        .arg("validate-manifest")
        .arg("--manifest")
        .arg(&options.manifest)
        .arg("--check-paths");
    if options.selected.is_empty() {
        validate_manifest.arg("--check-derived");
    }
    steps.push(run_control_step(
        "validate-manifest",
        &mut validate_manifest,
        &options.evidence_dir,
        "fail",
    )?);
    steps.push(run_control_step(
        "python-boundary",
        Command::new(&executable)
            .arg("python-boundary")
            .arg("--receipt")
            .arg(options.evidence_dir.join("python-boundary.json")),
        &options.evidence_dir,
        "fail",
    )?);

    let mut prerequisites_green = steps.iter().all(step_green);
    if (options.selected.is_empty() && options.from_wave == 0)
        || options.selected.iter().any(|name| name == "redline")
    {
        let redline_ci = run_redline_family(&manifest, &options, false, false)?;
        prerequisites_green &= step_green(&redline_ci);
        steps.push(redline_ci);
        if options.apply_tags {
            let redline_tags = if prerequisites_green {
                run_redline_tags(&manifest, &options, &executable)?
            } else {
                vec![blocked_step(
                    "redline-family-tags",
                    "canonical validation and pre-tag Redline family CI must pass first",
                )]
            };
            prerequisites_green &= redline_tags.iter().all(step_green);
            steps.extend(redline_tags);
            if prerequisites_green {
                let mut post_tag = run_redline_family(&manifest, &options, false, true)?;
                post_tag["name"] = json!("redline-family-ci-post-tag");
                prerequisites_green &= step_green(&post_tag);
                steps.push(post_tag);
            }
        }
    }

    let waves = repositories
        .iter()
        .map(|repo| repo.wave)
        .collect::<std::collections::BTreeSet<_>>();
    for wave in waves {
        let wave_repositories = repositories
            .iter()
            .filter(|repo| repo.wave == wave)
            .collect::<Vec<_>>();
        let mut wave_ci = std::collections::BTreeMap::new();
        for repo in &wave_repositories {
            let step = run_repo_ci(repo, &options, &manifest_sha256, &control_root)?;
            wave_ci.insert(
                repo.name.clone(),
                step["status"].as_str().unwrap_or("fail").to_owned(),
            );
            steps.push(step);
        }
        let wave_ci_green = wave_ci
            .values()
            .all(|status| matches!(status.as_str(), "pass" | "cached"));
        if options.apply_tags {
            let mut wave_bound_identity = false;
            for repo in &wave_repositories {
                let tag_steps = if prerequisites_green && wave_ci_green {
                    run_repo_tag_sequence(
                        repo,
                        wave_ci
                            .get(&repo.name)
                            .map(String::as_str)
                            .unwrap_or("fail"),
                        &options,
                        &executable,
                    )?
                } else {
                    vec![blocked_step(
                        &format!("tag:{}", repo.name),
                        "all prior waves and every CI lane in this wave must pass first",
                    )]
                };
                wave_bound_identity |= tag_steps.iter().any(|step| {
                    step["name"] == format!("bind-identity:{}", repo.name) && step_green(step)
                });
                prerequisites_green &= tag_steps.iter().all(step_green);
                steps.extend(tag_steps);
            }
            if wave_bound_identity {
                let sync_receipt = options
                    .evidence_dir
                    .join(format!("sync-derived-wave-{wave}.json"));
                let sync = run_control_step(
                    &format!("sync-derived-wave-{wave}"),
                    Command::new(&executable)
                        .arg("sync-derived-manifests")
                        .arg("--manifest")
                        .arg(&options.manifest)
                        .arg("--receipt")
                        .arg(sync_receipt)
                        .arg("--apply"),
                    &options.evidence_dir,
                    "fail",
                )?;
                prerequisites_green &= step_green(&sync);
                steps.push(sync);
            }
        } else {
            prerequisites_green &= wave_ci_green;
        }
    }

    if options.selected.is_empty() {
        let preflight_path = options.evidence_dir.join("release-preflight.json");
        steps.push(run_control_step(
            "release-preflight",
            Command::new(&executable)
                .arg("release-preflight")
                .arg("--manifest")
                .arg(&options.manifest)
                .arg("--json")
                .arg(&preflight_path),
            &options.evidence_dir,
            "blocked",
        )?);
    } else {
        steps.push(json!({
            "name": "release-preflight",
            "status": "skipped",
            "reason": "global preflight is deferred for a selective repository invocation",
        }));
    }

    let ready_for_rollout = steps
        .iter()
        .all(|step| matches!(step["status"].as_str(), Some("pass" | "cached" | "skipped")));
    if options.atomicsoul && options.selected.is_empty() && ready_for_rollout {
        steps.extend(run_atomicsoul(&control_root, &options.evidence_dir)?);
    } else {
        steps.push(json!({
            "name": "atomicsoul-dry-run",
            "status": if options.atomicsoul && options.selected.is_empty() {"blocked"} else {"skipped"},
            "reason": if options.atomicsoul && options.selected.is_empty() {
                "all manifest, fleet CI, tag, and preflight steps must pass first"
            } else {
                "disabled for this invocation"
            },
            "production_applied": false,
            "external_mutations": [],
        }));
    }

    let overall = overall_status(&steps);
    report["steps"] = json!(steps);
    report["status"] = json!(overall);
    let final_manifest_sha256 = sha256_file(&options.manifest)?;
    let manifest_changed = report["manifest_sha256"] != final_manifest_sha256;
    report["final_manifest_sha256"] = json!(final_manifest_sha256);
    report["manifest_changed"] = json!(manifest_changed);
    report["finished_at_unix"] = json!(now_unix());
    write_json(&aggregate_path, &report)?;
    println!("release candidate {overall}: {}", aggregate_path.display());
    if overall == "pass" {
        Ok(())
    } else {
        Err(format!("release candidate is {overall}; see the aggregate receipt").into())
    }
}

fn run_repo_tag_sequence(
    repo: &FleetRepo,
    ci_status: &str,
    options: &Options,
    executable: &Path,
) -> Result<Vec<JsonValue>, Box<dyn std::error::Error>> {
    if repo.release_commit != PENDING {
        return Ok(vec![run_repo_tag(repo, ci_status, options, executable)?]);
    }
    let (bound, identity) = bind_reviewed_identity(repo, options)?;
    let Some(bound) = bound else {
        return Ok(vec![identity]);
    };
    let tag = run_repo_tag(&bound, ci_status, options, executable)?;
    Ok(vec![identity, tag])
}

fn bind_reviewed_identity(
    repo: &FleetRepo,
    options: &Options,
) -> Result<(Option<FleetRepo>, JsonValue), Box<dyn std::error::Error>> {
    let step_name = format!("bind-identity:{}", repo.name);
    if repo.kind == "control-plane" {
        return Ok((
            None,
            blocked_step(
                &step_name,
                "the control plane cannot self-bind its own commit inside the commit it identifies",
            ),
        ));
    }
    let branch = git_output(&repo.path, &["branch", "--show-current"])?;
    let head = git_output(&repo.path, &["rev-parse", "--verify", "HEAD^{commit}"])?;
    let porcelain = git_output(
        &repo.path,
        &["status", "--porcelain=v1", "--untracked-files=all"],
    )?;
    let dirty_paths = release_relevant_dirty_paths(repo, &porcelain);
    if branch != repo.expected_branch || !dirty_paths.is_empty() {
        return Ok((
            None,
            json!({
                "name": step_name,
                "repository": repo.name,
                "status": "blocked",
                "reason": "identity binding requires a clean checkout on the reviewed branch",
                "branch": branch,
                "expected_branch": repo.expected_branch,
                "dirty_paths": dirty_paths,
            }),
        ));
    }
    let remote = remote_branch_commit(&repo.path, &repo.remote, &repo.expected_branch)?;
    if remote.as_deref() != Some(head.as_str()) {
        return Ok((
            None,
            json!({
                "name": step_name,
                "repository": repo.name,
                "status": "blocked",
                "reason": "reviewed checkout HEAD does not equal live forge main",
                "head": head,
                "remote_main": remote,
            }),
        ));
    }
    let checksum = super::release_tree_checksum(&repo.path, &head)?;
    update_manifest_identity(&options.manifest, &repo.name, &head, &checksum)?;
    let mut bound = repo.clone();
    bound.release_commit.clone_from(&head);
    let receipt = options
        .evidence_dir
        .join("identities")
        .join(format!("{}.json", repo.name));
    let step = json!({
        "schema_version": "jain.release-identity-binding/v1",
        "name": step_name,
        "repository": repo.name,
        "status": "pass",
        "manifest": options.manifest,
        "release_commit": head,
        "release_checksum_sha256": checksum,
        "source": "clean-reviewed-forge-main",
        "timestamp_unix": now_unix(),
    });
    write_json(&receipt, &step)?;
    Ok((Some(bound), step))
}

fn update_manifest_identity(
    manifest: &Path,
    repo_name: &str,
    commit: &str,
    checksum: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let source = fs::read_to_string(manifest)?;
    let mut lines = source
        .split_inclusive('\n')
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let starts = lines
        .iter()
        .enumerate()
        .filter_map(|(index, line)| {
            matches!(
                line.trim(),
                "[[repo]]" | "[[infrastructure_repo]]" | "[control_plane]"
            )
            .then_some(index)
        })
        .collect::<Vec<_>>();
    let mut target = None;
    for (position, start) in starts.iter().copied().enumerate() {
        let end = starts.get(position + 1).copied().unwrap_or(lines.len());
        let name_line = format!("name = \"{repo_name}\"");
        if lines[start..end]
            .iter()
            .any(|line| line.trim() == name_line)
        {
            target = Some((start, end));
            break;
        }
    }
    let (start, end) = target.ok_or_else(|| {
        format!("canonical manifest has no mutable repository block for {repo_name}")
    })?;
    let mut replaced_commit = false;
    let mut replaced_checksum = false;
    for line in &mut lines[start..end] {
        let trimmed = line.trim();
        if trimmed.starts_with("release_commit = ") {
            validate_replaceable_identity(trimmed, "release_commit", commit)?;
            *line = format!("release_commit = \"{commit}\"\n");
            replaced_commit = true;
        } else if trimmed.starts_with("release_checksum_sha256 = ") {
            validate_replaceable_identity(trimmed, "release_checksum_sha256", checksum)?;
            *line = format!("release_checksum_sha256 = \"{checksum}\"\n");
            replaced_checksum = true;
        }
    }
    if !replaced_commit || !replaced_checksum {
        return Err(format!("{repo_name} identity fields are incomplete in the manifest").into());
    }
    let staging = manifest.with_extension(format!("tmp-{}", std::process::id()));
    fs::write(&staging, lines.concat())?;
    fs::rename(staging, manifest)?;
    Ok(())
}

fn validate_replaceable_identity(
    line: &str,
    key: &str,
    replacement: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let current = line
        .split_once('=')
        .map(|(_, value)| value.trim().trim_matches('"'))
        .ok_or_else(|| format!("malformed {key} identity line"))?;
    if current != PENDING && current != replacement {
        return Err(format!(
            "refusing to replace immutable {key} value {current} with {replacement}"
        )
        .into());
    }
    Ok(())
}

fn remote_branch_commit(
    repo: &Path,
    remote: &str,
    branch: &str,
) -> Result<Option<String>, Box<dyn std::error::Error>> {
    let reference = format!("refs/heads/{branch}");
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["ls-remote", remote, &reference])
        .output()?;
    if !output.status.success() {
        return Err(format!(
            "git ls-remote failed for {}: {}",
            repo.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        )
        .into());
    }
    Ok(String::from_utf8(output.stdout)?
        .lines()
        .find_map(|line| line.split_whitespace().next().map(str::to_owned)))
}

fn parse_options(args: Vec<String>) -> Result<Options, Box<dyn std::error::Error>> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut options = Options {
        manifest: root.join("repos.manifest.toml"),
        evidence_dir: root.join("docs/release-evidence/8.0.0/orchestrator"),
        plan: false,
        force: false,
        apply_tags: true,
        atomicsoul: true,
        selected: Vec::new(),
        from_wave: 0,
        through_wave: u64::MAX,
        max_age_hours: DEFAULT_MAX_AGE_HOURS,
    };
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--manifest" => {
                options.manifest = PathBuf::from(iter.next().ok_or("--manifest needs a path")?)
            }
            "--evidence-dir" => {
                options.evidence_dir =
                    PathBuf::from(iter.next().ok_or("--evidence-dir needs a path")?)
            }
            "--repo" => options
                .selected
                .push(iter.next().ok_or("--repo needs a name")?),
            "--from-wave" => {
                options.from_wave = iter.next().ok_or("--from-wave needs a number")?.parse()?
            }
            "--through-wave" => {
                options.through_wave = iter
                    .next()
                    .ok_or("--through-wave needs a number")?
                    .parse()?
            }
            "--max-age-hours" => {
                options.max_age_hours = iter
                    .next()
                    .ok_or("--max-age-hours needs a number")?
                    .parse()?
            }
            "--plan" => options.plan = true,
            "--force" => options.force = true,
            "--no-tags" => options.apply_tags = false,
            "--no-atomicsoul" => options.atomicsoul = false,
            value => return Err(format!("unknown release-candidate argument: {value}").into()),
        }
    }
    if options.from_wave > options.through_wave {
        return Err("--from-wave cannot exceed --through-wave".into());
    }
    Ok(options)
}

fn help_requested(args: &[String]) -> bool {
    args.iter()
        .any(|arg| matches!(arg.as_str(), "--help" | "-h"))
}

fn aggregate_receipt_name(plan: bool) -> &'static str {
    if plan {
        "release-candidate-plan.json"
    } else {
        "release-candidate.json"
    }
}

fn reject_unsafe_environment() -> Result<(), Box<dyn std::error::Error>> {
    if env::var("ATOMICSOUL_PUSH").is_ok_and(|value| value != "0") {
        return Err("release-candidate refuses ATOMICSOUL_PUSH other than 0".into());
    }
    if env::var("JAIN_RELEASE_VERSION").is_ok_and(|value| value != RELEASE_VERSION) {
        return Err(format!("JAIN_RELEASE_VERSION must be {RELEASE_VERSION}").into());
    }
    Ok(())
}

fn validate_candidate_header(manifest: &toml::Value) -> Result<(), Box<dyn std::error::Error>> {
    let string = |key: &str| manifest.get(key).and_then(toml::Value::as_str);
    if string("release_version") != Some(RELEASE_VERSION)
        || string("status") != Some("candidate")
        || manifest.get("formal_ga").and_then(toml::Value::as_bool) != Some(false)
        || string("sagemaker") != Some("N/A")
    {
        return Err(
            "manifest must declare release 8.0.0, candidate, formal_ga=false, sagemaker=N/A".into(),
        );
    }
    Ok(())
}

fn validate_rollout_dependencies(manifest: &toml::Value) -> Result<(), Box<dyn std::error::Error>> {
    let mut repositories = std::collections::BTreeMap::new();
    for key in ["repo", "infrastructure_repo"] {
        for entry in manifest
            .get(key)
            .and_then(toml::Value::as_array)
            .into_iter()
            .flatten()
        {
            let name = entry
                .get("name")
                .and_then(toml::Value::as_str)
                .ok_or_else(|| format!("{key} entry is missing name"))?;
            let wave = entry
                .get("rollout_wave")
                .and_then(toml::Value::as_integer)
                .and_then(|value| u64::try_from(value).ok())
                .ok_or_else(|| format!("{name} is missing a valid rollout_wave"))?;
            let pending = entry
                .get("release_commit")
                .and_then(toml::Value::as_str)
                .is_some_and(|commit| commit == PENDING);
            if repositories
                .insert(name.to_owned(), (wave, pending))
                .is_some()
            {
                return Err(format!("duplicate release repository {name}").into());
            }
        }
    }

    let external_repositories = manifest
        .get("external_dependencies")
        .and_then(toml::Value::as_table)
        .into_iter()
        .flat_map(toml::map::Map::values)
        .filter_map(|dependency| dependency.get("repository"))
        .filter_map(toml::Value::as_str)
        .collect::<std::collections::BTreeSet<_>>();

    for entry in manifest
        .get("repo")
        .and_then(toml::Value::as_array)
        .into_iter()
        .flatten()
    {
        let name = entry.get("name").and_then(toml::Value::as_str).unwrap();
        let (wave, pending) = repositories[name];
        let dependencies = entry
            .get("cross_repo_deps")
            .and_then(toml::Value::as_array)
            .ok_or_else(|| format!("{name} is missing cross_repo_deps"))?;
        for dependency in dependencies {
            let dependency = dependency
                .as_str()
                .ok_or_else(|| format!("{name} has a non-string cross_repo_deps entry"))?;
            if let Some((dependency_wave, dependency_pending)) = repositories.get(dependency) {
                if pending && *dependency_pending && *dependency_wave >= wave {
                    return Err(format!(
                        "{name} wave {wave} requires unresolved {dependency} wave {dependency_wave}; unresolved dependencies must be in an earlier wave"
                    )
                    .into());
                }
            } else if !external_repositories.contains(dependency) {
                return Err(format!("{name} references unknown dependency {dependency}").into());
            }
        }
    }

    for entry in manifest
        .get("infrastructure_repo")
        .and_then(toml::Value::as_array)
        .into_iter()
        .flatten()
    {
        let name = entry.get("name").and_then(toml::Value::as_str).unwrap();
        let (wave, pending) = repositories[name];
        let dependants = entry
            .get("dependency_edges")
            .and_then(toml::Value::as_array)
            .ok_or_else(|| format!("{name} is missing dependency_edges"))?;
        for dependant in dependants {
            let dependant = dependant
                .as_str()
                .ok_or_else(|| format!("{name} has a non-string dependency_edges entry"))?;
            let (dependant_wave, dependant_pending) = repositories
                .get(dependant)
                .ok_or_else(|| format!("{name} references unknown dependant {dependant}"))?;
            if pending && *dependant_pending && *dependant_wave <= wave {
                return Err(format!(
                    "{dependant} wave {dependant_wave} requires unresolved {name} wave {wave}; infrastructure dependants must be in a later wave"
                )
                .into());
            }
        }
    }
    Ok(())
}

fn fleet_repositories(
    manifest: &toml::Value,
    manifest_path: &Path,
) -> Result<Vec<FleetRepo>, Box<dyn std::error::Error>> {
    let mut rows = Vec::new();
    for (key, kind) in [
        ("repo", "family"),
        ("infrastructure_repo", "infrastructure"),
    ] {
        for entry in manifest
            .get(key)
            .and_then(toml::Value::as_array)
            .into_iter()
            .flatten()
        {
            rows.push(fleet_repo(entry, kind, manifest_path)?);
        }
    }
    if let Some(control) = manifest.get("control_plane") {
        let mut repo = fleet_repo(control, "control-plane", manifest_path)?;
        repo.wave = 10;
        rows.push(repo);
    }
    Ok(rows)
}

fn fleet_repo(
    entry: &toml::Value,
    kind: &str,
    manifest_path: &Path,
) -> Result<FleetRepo, Box<dyn std::error::Error>> {
    let text = |key: &str| {
        entry
            .get(key)
            .and_then(toml::Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| format!("{kind} entry is missing {key}"))
    };
    let name = text("name")?;
    let path = PathBuf::from(text("path")?);
    let tag = entry
        .get("current_tag")
        .or_else(|| entry.get("immutable_tag"))
        .and_then(toml::Value::as_str)
        .ok_or_else(|| format!("{name} is missing an immutable tag"))?
        .to_owned();
    let mut policy = toml::to_string(entry)?;
    policy.push_str(&fs::read_to_string(
        manifest_path
            .parent()
            .unwrap_or(Path::new("."))
            .join("ops/ci/split-host-ci.sh"),
    )?);
    Ok(FleetRepo {
        name,
        path,
        remote: text("remote")?,
        required_check: text("required_check")?,
        expected_branch: entry
            .get("default_branch")
            .or_else(|| entry.get("branch"))
            .and_then(toml::Value::as_str)
            .unwrap_or("main")
            .to_owned(),
        tag,
        release_commit: text("release_commit")?,
        wave: entry
            .get("rollout_wave")
            .and_then(toml::Value::as_integer)
            .and_then(|value| u64::try_from(value).ok())
            .unwrap_or(10),
        kind: kind.to_owned(),
        policy_sha256: sha256_bytes(policy.as_bytes()),
    })
}

fn selected(options: &Options, repo: &FleetRepo) -> bool {
    (options.selected.is_empty() || options.selected.iter().any(|name| name == &repo.name))
        && repo.wave >= options.from_wave
        && repo.wave <= options.through_wave
}

fn repo_plan(repo: &FleetRepo) -> JsonValue {
    json!({
        "name": repo.name,
        "wave": repo.wave,
        "kind": repo.kind,
        "path": repo.path,
        "remote": repo.remote,
        "required_check": repo.required_check,
        "expected_branch": repo.expected_branch,
        "release_commit": repo.release_commit,
        "tag": repo.tag,
        "policy_sha256": repo.policy_sha256,
    })
}

fn run_repo_ci(
    repo: &FleetRepo,
    options: &Options,
    manifest_sha256: &str,
    control_root: &Path,
) -> Result<JsonValue, Box<dyn std::error::Error>> {
    let receipt = options
        .evidence_dir
        .join("ci")
        .join(format!("{}.json", repo.name));
    let log = options
        .evidence_dir
        .join("ci")
        .join(format!("{}.log", repo.name));
    fs::create_dir_all(receipt.parent().unwrap())?;
    let head = git_output(&repo.path, &["rev-parse", "--verify", "HEAD^{commit}"])?;
    let branch = git_output(&repo.path, &["branch", "--show-current"])?;
    let dirty_paths = git_output(
        &repo.path,
        &["status", "--porcelain=v1", "--untracked-files=all"],
    )?;
    let dirty_paths = release_relevant_dirty_paths(repo, &dirty_paths);
    let dirty = !dirty_paths.is_empty();
    let commit = if repo.release_commit == PENDING {
        head.clone()
    } else {
        repo.release_commit.clone()
    };
    if dirty {
        let step = json!({
            "name": format!("ci:{}", repo.name),
            "repository": repo.name,
            "wave": repo.wave,
            "commit": commit,
            "checkout_head": head,
            "branch": branch,
            "dirty_paths": dirty_paths,
            "status": "blocked",
            "reason": "working tree is dirty; preserve and commit/review it before detached release CI",
            "receipt": receipt,
            "log": log,
        });
        write_json(&receipt, &step)?;
        return Ok(step);
    }
    if !options.force {
        if let Some(mut cached) = cached_step(
            &receipt,
            &log,
            &commit,
            &repo.policy_sha256,
            options.max_age_hours,
        )? {
            cached["name"] = json!(format!("ci:{}", repo.name));
            cached["status"] = json!("cached");
            return Ok(cached);
        }
    }
    let owner = remote_owner(&repo.remote)?;
    let host = control_root.join("ops/ci/split-host-ci.sh");
    let started = Instant::now();
    let mut command = Command::new("bash");
    command
        .arg(&host)
        .arg(&owner)
        .arg(&repo.name)
        .arg(&commit)
        .arg(&repo.path)
        .arg(&repo.required_check)
        .env("JAIN_RELEASE_CI", "1")
        .env("JAIN_RELEASE_VERSION", RELEASE_VERSION)
        .env("ATOMICSOUL_PUSH", "0")
        .env(
            "JAIN_SPLIT_ROOT",
            control_root.parent().ok_or("control root has no parent")?,
        )
        .env_remove("RUSTUP_TOOLCHAIN")
        .env_remove("RUSTUP_OVERRIDE");
    let status = run_logged(&mut command, &log)?;
    let step = json!({
        "schema_version": "jain.release-candidate-ci/v1",
        "name": format!("ci:{}", repo.name),
        "repository": repo.name,
        "wave": repo.wave,
        "kind": repo.kind,
        "commit": commit,
        "checkout_head": head,
        "branch": branch,
        "manifest_sha256": manifest_sha256,
        "policy_sha256": repo.policy_sha256,
        "required_check": repo.required_check,
        "status": if status.success() {"pass"} else {"fail"},
        "exit_code": status.code(),
        "duration_millis": started.elapsed().as_millis(),
        "timestamp_unix": now_unix(),
        "log": log,
        "log_sha256": sha256_file(&log)?,
        "forge_status_posted": true,
    });
    write_json(&receipt, &step)?;
    Ok(step)
}

fn release_relevant_dirty_paths(repo: &FleetRepo, porcelain: &str) -> Vec<String> {
    porcelain
        .lines()
        .filter_map(|line| line.get(3..))
        .map(|path| {
            path.rsplit_once(" -> ")
                .map(|(_, path)| path)
                .unwrap_or(path)
        })
        .filter(|path| {
            repo.name != "jain-split-ops" || !path.starts_with("docs/release-evidence/8.0.0/")
        })
        .map(str::to_owned)
        .collect()
}

fn run_repo_tag(
    repo: &FleetRepo,
    ci_status: &str,
    options: &Options,
    executable: &Path,
) -> Result<JsonValue, Box<dyn std::error::Error>> {
    if !matches!(ci_status, "pass" | "cached") {
        return Ok(json!({
            "name": format!("tag:{}", repo.name),
            "repository": repo.name,
            "status": "blocked",
            "reason": "exact-commit release CI has not passed",
        }));
    }
    if repo.release_commit == PENDING {
        return Ok(json!({
            "name": format!("tag:{}", repo.name),
            "repository": repo.name,
            "status": "blocked",
            "reason": "canonical manifest release_commit is PENDING",
        }));
    }
    let receipt = options
        .evidence_dir
        .join("tags")
        .join(format!("{}.json", repo.name));
    let log = options
        .evidence_dir
        .join("tags")
        .join(format!("{}.log", repo.name));
    fs::create_dir_all(receipt.parent().unwrap())?;
    let mut command = Command::new(executable);
    command
        .arg("immutable-tag")
        .arg("--manifest")
        .arg(&options.manifest)
        .arg("--repo")
        .arg(&repo.path)
        .arg("--remote")
        .arg(&repo.remote)
        .arg("--tag")
        .arg(&repo.tag)
        .arg("--commit")
        .arg(&repo.release_commit)
        .arg("--receipt")
        .arg(&receipt)
        .arg("--apply")
        .env("ATOMICSOUL_PUSH", "0")
        .env_remove("RUSTUP_TOOLCHAIN")
        .env_remove("RUSTUP_OVERRIDE");
    let status = run_logged(&mut command, &log)?;
    Ok(json!({
        "name": format!("tag:{}", repo.name),
        "repository": repo.name,
        "commit": repo.release_commit,
        "tag": repo.tag,
        "status": if status.success() {"pass"} else {"fail"},
        "exit_code": status.code(),
        "receipt": receipt,
        "log": log,
        "log_sha256": sha256_file(&log)?,
    }))
}

fn run_redline_family(
    manifest: &toml::Value,
    options: &Options,
    force: bool,
    require_verified_tags: bool,
) -> Result<JsonValue, Box<dyn std::error::Error>> {
    let nested = manifest
        .get("nested_families")
        .and_then(|value| value.get("redline"))
        .ok_or("manifest is missing nested_families.redline")?;
    let control = PathBuf::from(
        nested
            .get("control_plane")
            .and_then(toml::Value::as_str)
            .ok_or("Redline control_plane is missing")?,
    );
    let nested_manifest = PathBuf::from(
        nested
            .get("manifest_path")
            .and_then(toml::Value::as_str)
            .ok_or("Redline manifest_path is missing")?,
    );
    let nested_hash = sha256_file(&nested_manifest)?;
    let release_root = options
        .evidence_dir
        .parent()
        .ok_or("orchestrator evidence directory has no release root")?;
    let receipt = release_root.join("redline-family-ci.json");
    let log = options.evidence_dir.join("redline-family-ci.log");
    if !force
        && !options.force
        && cached_family(
            &receipt,
            &nested_hash,
            options.max_age_hours,
            require_verified_tags,
        )?
    {
        return Ok(json!({
            "name": "redline-family-ci",
            "status": "cached",
            "manifest": nested_manifest,
            "manifest_sha256": nested_hash,
            "receipt": receipt,
        }));
    }
    let dirty = !git_output(
        &control,
        &["status", "--porcelain=v1", "--untracked-files=all"],
    )?
    .is_empty();
    if dirty {
        return Ok(json!({
            "name": "redline-family-ci",
            "status": "blocked",
            "reason": "Redline control-plane worktree is dirty",
            "manifest": nested_manifest,
        }));
    }
    let mut command = Command::new(control.join("redlinectl"));
    command
        .current_dir(&control)
        .arg("family-ci")
        .arg("--receipt")
        .arg(&receipt)
        .env("ATOMICSOUL_PUSH", "0")
        .env_remove("RUSTUP_TOOLCHAIN")
        .env_remove("RUSTUP_OVERRIDE");
    let started = Instant::now();
    let status = run_logged(&mut command, &log)?;
    Ok(json!({
        "name": "redline-family-ci",
        "status": if status.success() {"pass"} else {"fail"},
        "manifest": nested_manifest,
        "manifest_sha256": nested_hash,
        "receipt": receipt,
        "log": log,
        "log_sha256": sha256_file(&log)?,
        "exit_code": status.code(),
        "duration_millis": started.elapsed().as_millis(),
        "timestamp_unix": now_unix(),
    }))
}

fn run_redline_tags(
    manifest: &toml::Value,
    options: &Options,
    executable: &Path,
) -> Result<Vec<JsonValue>, Box<dyn std::error::Error>> {
    let nested_manifest = PathBuf::from(
        manifest
            .get("nested_families")
            .and_then(|value| value.get("redline"))
            .and_then(|value| value.get("manifest_path"))
            .and_then(toml::Value::as_str)
            .ok_or("manifest is missing nested_families.redline.manifest_path")?,
    );
    let nested: toml::Value = fs::read_to_string(&nested_manifest)?.parse()?;
    let base = nested_manifest.parent().unwrap_or(Path::new("."));
    let mut steps = Vec::new();
    for entry in nested
        .get("repo")
        .and_then(toml::Value::as_array)
        .into_iter()
        .flatten()
    {
        let field = |key: &str| {
            entry
                .get(key)
                .and_then(toml::Value::as_str)
                .ok_or_else(|| format!("Redline repository entry is missing {key}"))
        };
        let name = field("name")?;
        let commit = field("release_commit")?;
        if commit == PENDING {
            steps.push(blocked_step(
                &format!("tag:{name}"),
                "Redline manifest release_commit is PENDING",
            ));
            continue;
        }
        let repo = base.join(field("path")?);
        let remote = field("remote")?;
        let tag = field("current_tag")?;
        let receipt = options
            .evidence_dir
            .join("tags")
            .join(format!("{name}.json"));
        let log = options
            .evidence_dir
            .join("tags")
            .join(format!("{name}.log"));
        fs::create_dir_all(receipt.parent().unwrap())?;
        let mut command = Command::new(executable);
        command
            .arg("immutable-tag")
            .arg("--manifest")
            .arg(&nested_manifest)
            .arg("--repo")
            .arg(&repo)
            .arg("--remote")
            .arg(remote)
            .arg("--tag")
            .arg(tag)
            .arg("--commit")
            .arg(commit)
            .arg("--receipt")
            .arg(&receipt)
            .arg("--apply")
            .env("ATOMICSOUL_PUSH", "0")
            .env_remove("RUSTUP_TOOLCHAIN")
            .env_remove("RUSTUP_OVERRIDE");
        let status = run_logged(&mut command, &log)?;
        steps.push(json!({
            "name": format!("tag:{name}"),
            "repository": name,
            "commit": commit,
            "tag": tag,
            "status": if status.success() {"pass"} else {"fail"},
            "exit_code": status.code(),
            "receipt": receipt,
            "log": log,
            "log_sha256": sha256_file(&log)?,
        }));
    }
    Ok(steps)
}

fn run_control_step(
    name: &str,
    command: &mut Command,
    evidence_dir: &Path,
    failure_status: &str,
) -> Result<JsonValue, Box<dyn std::error::Error>> {
    let log = evidence_dir.join(format!("{name}.log"));
    command
        .env("JAIN_RELEASE_VERSION", RELEASE_VERSION)
        .env("ATOMICSOUL_PUSH", "0")
        .env_remove("RUSTUP_TOOLCHAIN")
        .env_remove("RUSTUP_OVERRIDE");
    let started = Instant::now();
    let status = run_logged(command, &log)?;
    Ok(json!({
        "name": name,
        "status": if status.success() {"pass"} else {failure_status},
        "exit_code": status.code(),
        "duration_millis": started.elapsed().as_millis(),
        "log": log,
        "log_sha256": sha256_file(&log)?,
    }))
}

fn run_atomicsoul(
    control_root: &Path,
    evidence_dir: &Path,
) -> Result<Vec<JsonValue>, Box<dyn std::error::Error>> {
    let deploy = control_root
        .parent()
        .ok_or("control root has no parent")?
        .join("jain-deploy");
    let atomicsoul_dir = evidence_dir.join("atomicsoul");
    let contract = run_control_step(
        "atomicsoul-contract",
        Command::new("bash").arg(deploy.join("scripts/test-atomicsoul-dry-run.sh")),
        evidence_dir,
        "fail",
    )?;
    if contract["status"] != "pass" {
        return Ok(vec![
            contract,
            json!({
                "name": "atomicsoul-dry-run",
                "status": "blocked",
                "reason": "AtomicSoul wrapper contract failed",
                "production_applied": false,
            }),
        ]);
    }
    let mut command = Command::new("bash");
    command
        .current_dir(&deploy)
        .arg(deploy.join("scripts/atomicsoul-dry-run.sh"))
        .arg("--evidence-dir")
        .arg(&atomicsoul_dir)
        .env("JAIN_RELEASE_VERSION", RELEASE_VERSION)
        .env("ATOMICSOUL_PUSH", "0");
    let dry_run = run_control_step("atomicsoul-dry-run", &mut command, evidence_dir, "fail")?;
    if dry_run["status"] == "pass" {
        validate_atomicsoul_receipt(&atomicsoul_dir.join("atomicsoul-dry-run.receipt.json"))?;
    }
    Ok(vec![contract, dry_run])
}

fn validate_atomicsoul_receipt(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let receipt: JsonValue = serde_json::from_slice(&fs::read(path)?)?;
    if receipt["release_version"] != RELEASE_VERSION
        || receipt["status"] != "planned"
        || receipt["release_status"] != "candidate"
        || receipt["formal_ga"] != false
        || receipt["atomicsoul_push"] != false
        || receipt["production_applied"] != false
        || receipt["external_mutations"] != false
        || receipt["rollback_target"] != "7.0.6"
        || receipt["sagemaker"] != "N/A"
    {
        return Err("AtomicSoul receipt violates the v8 dry-run contract".into());
    }
    Ok(())
}

fn run_logged(
    command: &mut Command,
    log: &Path,
) -> Result<std::process::ExitStatus, Box<dyn std::error::Error>> {
    if let Some(parent) = log.parent() {
        fs::create_dir_all(parent)?;
    }
    let stdout = File::create(log)?;
    let stderr = stdout.try_clone()?;
    Ok(command
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr))
        .status()?)
}

fn cached_step(
    receipt: &Path,
    log: &Path,
    commit: &str,
    policy_sha256: &str,
    max_age_hours: u64,
) -> Result<Option<JsonValue>, Box<dyn std::error::Error>> {
    if !receipt.is_file() || !log.is_file() {
        return Ok(None);
    }
    let value: JsonValue = serde_json::from_slice(&fs::read(receipt)?)?;
    let fresh = value["timestamp_unix"].as_u64().is_some_and(|timestamp| {
        now_unix().saturating_sub(timestamp) <= max_age_hours.saturating_mul(3600)
    });
    let valid = value["status"] == "pass"
        && value["commit"] == commit
        && value["policy_sha256"] == policy_sha256
        && value["log_sha256"].as_str() == Some(&sha256_file(log)?)
        && fresh;
    Ok(valid.then_some(value))
}

fn cached_family(
    receipt: &Path,
    manifest_sha256: &str,
    max_age_hours: u64,
    require_verified_tags: bool,
) -> Result<bool, Box<dyn std::error::Error>> {
    if !receipt.is_file() {
        return Ok(false);
    }
    let value: JsonValue = serde_json::from_slice(&fs::read(receipt)?)?;
    let generated = value["generated_at"].as_str().is_some();
    let modified = fs::metadata(receipt)?
        .modified()?
        .duration_since(UNIX_EPOCH)?
        .as_secs();
    let tags_verified = value["repositories"]
        .as_array()
        .is_some_and(|repositories| {
            !repositories.is_empty()
                && repositories
                    .iter()
                    .all(|repo| repo["tag_state"] == "verified")
        });
    Ok(value["status"] == "pass"
        && value["manifest_sha256"] == manifest_sha256
        && generated
        && (!require_verified_tags || tags_verified)
        && now_unix().saturating_sub(modified) <= max_age_hours.saturating_mul(3600))
}

fn remote_owner(remote: &str) -> Result<String, Box<dyn std::error::Error>> {
    let slug = remote
        .split_once("/git/")
        .map(|(_, slug)| slug)
        .and_then(|slug| slug.strip_suffix(".git"))
        .ok_or_else(|| format!("unsupported local Jeryu remote: {remote}"))?;
    let (owner, _) = slug
        .split_once('/')
        .ok_or_else(|| format!("remote has no owner/repository slug: {remote}"))?;
    Ok(owner.to_owned())
}

fn git_output(repo: &Path, args: &[&str]) -> Result<String, Box<dyn std::error::Error>> {
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

fn overall_status(steps: &[JsonValue]) -> &'static str {
    if steps.iter().any(|step| step["status"] == "fail") {
        "fail"
    } else if steps.iter().any(|step| step["status"] == "blocked") {
        "blocked"
    } else {
        "pass"
    }
}

fn step_green(step: &JsonValue) -> bool {
    matches!(step["status"].as_str(), Some("pass" | "cached" | "skipped"))
}

fn blocked_step(name: &str, reason: &str) -> JsonValue {
    json!({
        "name": name,
        "status": "blocked",
        "reason": reason,
    })
}

fn write_json(path: &Path, value: &JsonValue) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let staging = path.with_extension(format!("tmp-{}", std::process::id()));
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    fs::write(&staging, bytes)?;
    fs::rename(staging, path)?;
    Ok(())
}

fn sha256_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn sha256_file(path: &Path) -> Result<String, Box<dyn std::error::Error>> {
    Ok(sha256_bytes(&fs::read(path)?))
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_jeryu_owner_is_derived_without_guessing() {
        assert_eq!(
            remote_owner("http://127.0.0.1:8787/git/jain-split/jain-smartcluster.git").unwrap(),
            "jain-split"
        );
        assert!(remote_owner("https://github.com/neverhuman/jain.git").is_err());
    }

    #[test]
    fn candidate_header_is_fail_closed() {
        let valid: toml::Value = r#"
release_version = "8.0.0"
status = "candidate"
formal_ga = false
sagemaker = "N/A"
"#
        .parse()
        .unwrap();
        validate_candidate_header(&valid).unwrap();
        let invalid: toml::Value = r#"
release_version = "8.0.0"
status = "ga"
formal_ga = true
sagemaker = "passed"
"#
        .parse()
        .unwrap();
        assert!(validate_candidate_header(&invalid).is_err());
    }

    #[test]
    fn rollout_dependencies_are_ordered_and_known() {
        let valid: toml::Value = r#"
[external_dependencies.redline]
repository = "redline-core"

[[infrastructure_repo]]
name = "smartcluster"
rollout_wave = 5
release_commit = "PENDING"
dependency_edges = ["web"]

[[repo]]
name = "llm"
rollout_wave = 4
release_commit = "PENDING"
cross_repo_deps = []

[[repo]]
name = "agent"
rollout_wave = 5
release_commit = "PENDING"
cross_repo_deps = ["llm", "redline-core"]

[[repo]]
name = "web"
rollout_wave = 6
release_commit = "PENDING"
cross_repo_deps = ["agent"]

[[repo]]
name = "released-contracts"
rollout_wave = 1
release_commit = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
cross_repo_deps = ["web"]
"#
        .parse()
        .unwrap();
        validate_rollout_dependencies(&valid).unwrap();

        let same_wave: toml::Value = r#"
[[repo]]
name = "llm"
rollout_wave = 4
release_commit = "PENDING"
cross_repo_deps = []

[[repo]]
name = "agent"
rollout_wave = 4
release_commit = "PENDING"
cross_repo_deps = ["llm"]
"#
        .parse()
        .unwrap();
        assert!(validate_rollout_dependencies(&same_wave).is_err());

        let unknown: toml::Value = r#"
[[repo]]
name = "agent"
rollout_wave = 5
release_commit = "PENDING"
cross_repo_deps = ["typo"]
"#
        .parse()
        .unwrap();
        assert!(validate_rollout_dependencies(&unknown).is_err());
    }

    #[test]
    fn aggregate_status_prefers_fail_then_blocked() {
        assert_eq!(overall_status(&[json!({"status":"pass"})]), "pass");
        assert_eq!(
            overall_status(&[json!({"status":"pass"}), json!({"status":"blocked"})]),
            "blocked"
        );
        assert_eq!(
            overall_status(&[json!({"status":"blocked"}), json!({"status":"fail"})]),
            "fail"
        );
    }

    #[test]
    fn help_and_plan_cannot_replace_the_execution_receipt() {
        assert!(help_requested(&["--help".to_owned()]));
        assert!(help_requested(&["-h".to_owned()]));
        assert!(!help_requested(&["--plan".to_owned()]));
        assert_eq!(aggregate_receipt_name(true), "release-candidate-plan.json");
        assert_eq!(aggregate_receipt_name(false), "release-candidate.json");
    }

    #[test]
    fn control_plane_ignores_only_its_generated_release_receipts() {
        let repo = FleetRepo {
            name: "jain-split-ops".to_owned(),
            path: PathBuf::new(),
            remote: String::new(),
            required_check: String::new(),
            expected_branch: "main".to_owned(),
            tag: String::new(),
            release_commit: PENDING.to_owned(),
            wave: 10,
            kind: "control-plane".to_owned(),
            policy_sha256: String::new(),
        };
        assert_eq!(
            release_relevant_dirty_paths(
                &repo,
                "?? docs/release-evidence/8.0.0/orchestrator/run.json\n M tools/splitctl/src/main.rs\n"
            ),
            vec!["tools/splitctl/src/main.rs"]
        );
        let mut member = repo;
        member.name = "jain-core".to_owned();
        assert_eq!(
            release_relevant_dirty_paths(
                &member,
                "?? docs/release-evidence/8.0.0/orchestrator/run.json\n"
            ),
            vec!["docs/release-evidence/8.0.0/orchestrator/run.json"]
        );
    }

    #[test]
    fn identity_binding_replaces_only_pending_fields_and_never_moves_identity() {
        let root =
            env::temp_dir().join(format!("jain-release-identity-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let manifest = root.join("repos.manifest.toml");
        fs::write(
            &manifest,
            r#"[[repo]]
name = "one"
release_commit = "PENDING"
release_checksum_sha256 = "PENDING"

[[repo]]
name = "two"
release_commit = "PENDING"
release_checksum_sha256 = "PENDING"
"#,
        )
        .unwrap();
        let commit = "a".repeat(40);
        let checksum = "b".repeat(64);
        update_manifest_identity(&manifest, "two", &commit, &checksum).unwrap();
        let updated = fs::read_to_string(&manifest).unwrap();
        assert!(updated.contains("name = \"one\"\nrelease_commit = \"PENDING\""));
        assert!(updated.contains(&format!(
            "name = \"two\"\nrelease_commit = \"{commit}\"\nrelease_checksum_sha256 = \"{checksum}\""
        )));
        assert!(update_manifest_identity(&manifest, "two", &"c".repeat(40), &checksum).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
