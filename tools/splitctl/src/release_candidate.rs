use serde_json::{json, Value as JsonValue};
use sha2::{Digest, Sha256};
use std::{
    env, fs,
    fs::File,
    io::Write,
    os::fd::AsRawFd,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Instant, SystemTime, UNIX_EPOCH},
};

const RELEASE_VERSION: &str = "8.0.0";
const TOOL_VERSION: &str = "splitctl 0.1.0";
const PENDING: &str = "PENDING";
const DEFAULT_MAX_AGE_HOURS: u64 = 24;
const MAX_ROLLOUT_WAVE: u64 = 10;
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

struct RunLock(File);

impl RunLock {
    fn acquire(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut file = File::options()
            .create(true)
            .read(true)
            .write(true)
            .open(path)?;
        let result = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
        if result != 0 {
            return Err(format!(
                "another release-candidate runner owns {}; wait for it to finish",
                path.display()
            )
            .into());
        }
        file.set_len(0)?;
        write!(
            file,
            "{{\"pid\":{},\"started_at_unix\":{}}}\n",
            std::process::id(),
            now_unix()
        )?;
        file.sync_all()?;
        Ok(Self(file))
    }
}

impl Drop for RunLock {
    fn drop(&mut self) {
        unsafe {
            libc::flock(self.0.as_raw_fd(), libc::LOCK_UN);
        }
    }
}

pub(crate) fn run(args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    if help_requested(&args) {
        println!("{USAGE}");
        return Ok(());
    }
    let options = parse_options(args)?;
    let aggregate_path = options
        .evidence_dir
        .join(aggregate_receipt_name(options.plan));

    // Planning is intentionally read-only apart from its dedicated plan
    // receipt. It must not acquire the execution lock or touch the execution
    // aggregate.
    reject_unsafe_environment()?;
    let manifest_bytes = fs::read(&options.manifest)?;
    let manifest_sha256 = sha256_bytes(&manifest_bytes);
    let manifest: toml::Value = std::str::from_utf8(&manifest_bytes)?.parse()?;
    validate_candidate_header(&manifest)?;
    validate_rollout_dependencies(&manifest)?;
    let mut repositories = fleet_repositories(&manifest, &options.manifest)?;
    validate_selection(&options, &repositories)?;
    validate_selective_dependencies(&options, &manifest, &repositories)?;
    repositories.retain(|repo| selected(&options, repo));
    if repositories.is_empty() && !options.selected.iter().any(|name| name == "redline") {
        return Err("release selection matched no repositories".into());
    }
    repositories.sort_by(|left, right| (left.wave, &left.name).cmp(&(right.wave, &right.name)));
    let policy_sha256 = sha256_file(
        &options
            .manifest
            .parent()
            .unwrap_or(Path::new("."))
            .join("ops/ci/split-host-ci.sh"),
    )?;

    let mut report = json!({
        "schema_version": "jain.release-candidate-runner/v1",
        "release_version": RELEASE_VERSION,
        "release_status": "candidate",
        "formal_ga": false,
        "sagemaker": "N/A",
        "mode": if options.plan {"plan"} else {"execute"},
        "manifest": options.manifest,
        "manifest_sha256": manifest_sha256,
        "policy_sha256": policy_sha256,
        "atomicsoul_push": false,
        "production_applied": false,
        "external_production_mutations": [],
        "rollback_target": "7.0.6",
        "blocked_steps": 0,
        "failed_steps": 0,
        "repository_count": repositories.len(),
        "status": "pending",
        "started_at_unix": now_unix(),
        "tool_version": TOOL_VERSION,
        "rerun_command": rerun_command(&options),
        "last_completed_step": JsonValue::Null,
        "last_attempted_step": JsonValue::Null,
        "last_log_path": JsonValue::Null,
        "steps": [],
    });
    report["plan"] = json!(repositories.iter().map(repo_plan).collect::<Vec<_>>());
    if options.plan {
        fs::create_dir_all(&options.evidence_dir)?;
        report["status"] = json!("planned");
        report["finished_at_unix"] = json!(now_unix());
        write_json(&aggregate_path, &report)?;
        println!("release candidate plan: {}", aggregate_path.display());
        return Ok(());
    }
    fs::create_dir_all(&options.evidence_dir)?;
    let lock_path = options.evidence_dir.join(".release-candidate.lock");
    let _run_lock = match RunLock::acquire(&lock_path) {
        Ok(lock) => lock,
        Err(error) => {
            let lock_receipt = options.evidence_dir.join("release-candidate-lock.json");
            let receipt = json!({
                "schema_version": "jain.release-candidate-lock/v1",
                "status": "blocked",
                "reason": "another release-candidate runner owns the process-wide lock",
                "lock": lock_path,
                "owner": fs::read_to_string(&lock_path).unwrap_or_default(),
                "rerun_command": rerun_command(&options),
                "timestamp_unix": now_unix(),
            });
            write_json(&lock_receipt, &receipt)?;
            return Err(error);
        }
    };
    reject_unsafe_environment()?;
    report["status"] = json!("running");
    report["last_updated_at_unix"] = json!(now_unix());
    write_json(&aggregate_path, &report)?;

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
    checkpoint_report(&aggregate_path, &mut report, &steps)?;
    let host_prerequisites = validate_host_prerequisites(&manifest, &options, &control_root)?;
    steps.push(host_prerequisites);
    checkpoint_report(&aggregate_path, &mut report, &steps)?;
    let mut prerequisites_green = steps.iter().all(step_green);
    if !prerequisites_green {
        return finish_run(&aggregate_path, &mut report, steps, &options.manifest);
    }

    let python_boundary = run_control_step(
        "python-boundary",
        Command::new(&executable)
            .arg("python-boundary")
            .arg("--receipt")
            .arg(options.evidence_dir.join("python-boundary.json")),
        &options.evidence_dir,
        "fail",
    )?;
    prerequisites_green &= step_green(&python_boundary);
    steps.push(python_boundary);
    checkpoint_report(&aggregate_path, &mut report, &steps)?;
    if !prerequisites_green {
        return finish_run(&aggregate_path, &mut report, steps, &options.manifest);
    }

    let full_run = options.selected.is_empty()
        && options.from_wave == 0
        && options.through_wave == MAX_ROLLOUT_WAVE;
    if full_run || options.selected.iter().any(|name| name == "redline") {
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
            if prerequisites_green {
                let cutover = run_redline_cutover(&manifest, &options)?;
                prerequisites_green &= cutover.iter().all(step_green);
                steps.extend(cutover);
            }
        }
    }
    checkpoint_report(&aggregate_path, &mut report, &steps)?;

    if prerequisites_green {
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
            let current_manifest_sha256 = sha256_file(&options.manifest)?;
            for repo in &wave_repositories {
                let step = run_repo_ci(repo, &options, &current_manifest_sha256, &control_root)?;
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
                        step["name"] == format!("tag:{}", repo.name) && step_green(step)
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
            checkpoint_report(&aggregate_path, &mut report, &steps)?;
            if !prerequisites_green {
                break;
            }
        }
    } else {
        steps.push(blocked_step(
            "jain-dependency-waves",
            "Redline preconditions must pass before Jain CI or tagging",
        ));
    }

    if options.selected.is_empty() && prerequisites_green {
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
    } else if options.selected.is_empty() {
        steps.push(json!({
            "name": "release-preflight",
            "status": "blocked",
            "reason": "Jain dependency waves did not complete; release preflight is fail-closed",
        }));
    } else {
        steps.push(json!({
            "name": "release-preflight",
            "status": "skipped",
            "reason": "global preflight is deferred for a selective repository invocation",
        }));
    }
    checkpoint_report(&aggregate_path, &mut report, &steps)?;

    let ready_for_rollout = steps
        .iter()
        .all(|step| matches!(step["status"].as_str(), Some("pass" | "cached" | "skipped")));
    if full_run && ready_for_rollout {
        steps.extend(run_staged_artifact(&control_root, &options.evidence_dir)?);
    } else if full_run {
        steps.push(blocked_step(
            "staged-artifact-image",
            "all manifest, Redline, dependency-wave, and preflight gates must pass first",
        ));
    } else {
        steps.push(json!({
            "name": "staged-artifact-image",
            "status": "skipped",
            "reason": "artifact build is reserved for the complete release-candidate invocation",
        }));
    }
    checkpoint_report(&aggregate_path, &mut report, &steps)?;

    let ready_for_atomic = steps
        .iter()
        .all(|step| matches!(step["status"].as_str(), Some("pass" | "cached" | "skipped")));
    if options.atomicsoul && full_run && ready_for_atomic {
        steps.extend(run_atomicsoul(&control_root, &options.evidence_dir)?);
    } else {
        steps.push(json!({
            "name": "atomicsoul-dry-run",
            "status": if options.atomicsoul && full_run {"blocked"} else {"skipped"},
            "reason": if options.atomicsoul && full_run {
                "all manifest, fleet CI, artifact, tag, and preflight steps must pass first"
            } else {
                "disabled for this invocation"
            },
            "production_applied": false,
            "external_mutations": [],
        }));
    }
    checkpoint_report(&aggregate_path, &mut report, &steps)?;

    if full_run && steps.iter().all(step_green) {
        steps.extend(run_final_validation(&control_root, &options)?);
        checkpoint_report(&aggregate_path, &mut report, &steps)?;
    }
    finish_run(&aggregate_path, &mut report, steps, &options.manifest)
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
    Ok(vec![json!({
        "name": format!("tag:{}", repo.name),
        "repository": repo.name,
        "status": "blocked",
        "reason": "reviewed manifest update required: commit the exact reviewed main identity and checksum through the Jeryu lifecycle before tagging",
        "release_commit": PENDING,
        "tag": repo.tag,
    })])
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
        through_wave: MAX_ROLLOUT_WAVE,
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
    if options.from_wave > MAX_ROLLOUT_WAVE || options.through_wave > MAX_ROLLOUT_WAVE {
        return Err(format!("wave selectors must be between 0 and {MAX_ROLLOUT_WAVE}").into());
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

fn validate_selection(
    options: &Options,
    repositories: &[FleetRepo],
) -> Result<(), Box<dyn std::error::Error>> {
    for selected in &options.selected {
        if selected.trim().is_empty() {
            return Err("release repository selectors must not be empty".into());
        }
        if selected != "redline" && !repositories.iter().any(|repo| &repo.name == selected) {
            return Err(format!("unknown release repository selector: {selected}").into());
        }
    }
    Ok(())
}

fn validate_selective_dependencies(
    options: &Options,
    manifest: &toml::Value,
    repositories: &[FleetRepo],
) -> Result<(), Box<dyn std::error::Error>> {
    let selective = !options.selected.is_empty()
        || options.from_wave != 0
        || options.through_wave != MAX_ROLLOUT_WAVE;
    if !selective {
        return Ok(());
    }

    let active = repositories
        .iter()
        .map(|repo| (repo.name.as_str(), repo))
        .collect::<std::collections::BTreeMap<_, _>>();
    let selected = repositories
        .iter()
        .filter(|repo| selected(options, repo))
        .map(|repo| repo.name.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let external = manifest
        .get("external_dependencies")
        .and_then(toml::Value::as_table)
        .into_iter()
        .flat_map(toml::map::Map::values)
        .filter_map(|dependency| dependency.get("repository"))
        .filter_map(toml::Value::as_str)
        .collect::<std::collections::BTreeSet<_>>();

    for name in &selected {
        let Some(repo) = active.get(name) else {
            continue;
        };
        let raw = manifest_entry(manifest, name)
            .ok_or_else(|| format!("selected repository {name} has no manifest entry"))?;
        let dependencies = raw
            .get("cross_repo_deps")
            .and_then(toml::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(toml::Value::as_str);
        for dependency in dependencies {
            if external.contains(dependency) {
                continue;
            }
            let dependency_repo = active.get(dependency).ok_or_else(|| {
                format!("{name} references unknown selective dependency {dependency}")
            })?;
            if dependency_repo.release_commit == PENDING && !selected.contains(dependency) {
                return Err(format!(
                    "selective release of {name} is dependency-incomplete: {dependency} is unresolved; include --repo {dependency} or run the complete wave range"
                )
                .into());
            }
            if dependency_repo.wave > repo.wave && dependency_repo.release_commit == PENDING {
                return Err(format!(
                    "selective release of {name} requires later unresolved dependency {dependency}"
                )
                .into());
            }
        }
    }

    for repo in repositories {
        if !selected.contains(repo.name.as_str()) {
            continue;
        }
        for infrastructure in manifest
            .get("infrastructure_repo")
            .and_then(toml::Value::as_array)
            .into_iter()
            .flatten()
        {
            let edges = infrastructure
                .get("dependency_edges")
                .and_then(toml::Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(toml::Value::as_str)
                .collect::<Vec<_>>();
            if !edges.contains(&repo.name.as_str()) {
                continue;
            }
            let infrastructure_name = infrastructure
                .get("name")
                .and_then(toml::Value::as_str)
                .ok_or("infrastructure dependency is missing name")?;
            let infrastructure_repo = active.get(infrastructure_name).ok_or_else(|| {
                format!("unknown infrastructure dependency {infrastructure_name}")
            })?;
            if infrastructure_repo.release_commit == PENDING
                && !selected.contains(infrastructure_name)
            {
                return Err(format!(
                    "selective release of {} is dependency-incomplete: unresolved infrastructure {infrastructure_name}; include --repo {infrastructure_name}",
                    repo.name
                )
                .into());
            }
        }
    }
    Ok(())
}

fn manifest_entry<'a>(manifest: &'a toml::Value, name: &str) -> Option<&'a toml::Value> {
    ["repo", "infrastructure_repo"]
        .into_iter()
        .filter_map(|key| manifest.get(key).and_then(toml::Value::as_array))
        .flatten()
        .find(|entry| entry.get("name").and_then(toml::Value::as_str) == Some(name))
        .or_else(|| {
            manifest
                .get("control_plane")
                .filter(|entry| entry.get("name").and_then(toml::Value::as_str) == Some(name))
        })
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

fn rerun_command(options: &Options) -> Vec<String> {
    let mut command = vec!["./release-candidate.sh".to_owned()];
    if options.force {
        command.push("--force".to_owned());
    }
    if !options.apply_tags {
        command.push("--no-tags".to_owned());
    }
    if !options.atomicsoul {
        command.push("--no-atomicsoul".to_owned());
    }
    for repo in &options.selected {
        command.extend(["--repo".to_owned(), repo.clone()]);
    }
    if options.from_wave != 0 {
        command.extend(["--from-wave".to_owned(), options.from_wave.to_string()]);
    }
    if options.through_wave != MAX_ROLLOUT_WAVE {
        command.extend([
            "--through-wave".to_owned(),
            options.through_wave.to_string(),
        ]);
    }
    if options.max_age_hours != DEFAULT_MAX_AGE_HOURS {
        command.extend([
            "--max-age-hours".to_owned(),
            options.max_age_hours.to_string(),
        ]);
    }
    if options.manifest != PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("repos.manifest.toml") {
        command.extend([
            "--manifest".to_owned(),
            options.manifest.display().to_string(),
        ]);
    }
    if options.evidence_dir
        != PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("docs/release-evidence/8.0.0/orchestrator")
    {
        command.extend([
            "--evidence-dir".to_owned(),
            options.evidence_dir.display().to_string(),
        ]);
    }
    command
}

fn validate_host_prerequisites(
    manifest: &toml::Value,
    options: &Options,
    control_root: &Path,
) -> Result<JsonValue, Box<dyn std::error::Error>> {
    let split_root = manifest
        .get("split_root")
        .and_then(toml::Value::as_str)
        .map(PathBuf::from)
        .ok_or("manifest is missing split_root")?;
    let mut checks = Vec::new();
    for tool in [
        "rustc",
        "cargo",
        "git",
        "curl",
        "docker",
        "jq",
        "syft",
        "grype",
        "cargo-audit",
        "cosign",
    ] {
        let available = executable_available(tool);
        checks.push(json!({
            "name": tool,
            "status": if available {"pass"} else {"blocked"},
            "detail": if available {"executable available"} else {"install or expose the required executable on PATH"},
        }));
    }

    let buildx = command_succeeds("docker", &["buildx", "version"]);
    checks.push(json!({
        "name": "docker-buildx",
        "status": if buildx {"pass"} else {"blocked"},
        "detail": if buildx {"docker buildx is available"} else {"docker buildx is unavailable; install the Docker buildx plugin"},
    }));

    let jeryu_health = command_succeeds(
        "curl",
        &["-fsS", "--max-time", "5", "http://127.0.0.1:8787/health"],
    );
    checks.push(json!({
        "name": "jeryu-health",
        "status": if jeryu_health {"pass"} else {"blocked"},
        "detail": if jeryu_health {"local Jeryu health endpoint is ready"} else {"start or repair local Jeryu at http://127.0.0.1:8787"},
    }));

    let token_path = env::var_os("JERYU_MERGE_TOKEN_FILE")
        .map(PathBuf::from)
        .or_else(|| {
            env::var_os("HOME").map(|home| PathBuf::from(home).join(".jeryu/secrets/merge-token"))
        });
    let token_available = env::var("JERYU_MERGE_TOKEN")
        .ok()
        .is_some_and(|token| !token.is_empty())
        || token_path.as_deref().is_some_and(Path::is_file);
    checks.push(json!({
        "name": "jeryu-merge-token",
        "status": if token_available {"pass"} else {"blocked"},
        "detail": if token_available {"Jeryu write credential is available"} else {"configure JERYU_MERGE_TOKEN or JERYU_MERGE_TOKEN_FILE; token contents are never recorded"},
    }));

    let mirror_root = split_root.join("target/bare-mirrors");
    let mirror_names = if options.selected.is_empty() {
        ["repo", "infrastructure_repo"]
            .into_iter()
            .filter_map(|key| manifest.get(key).and_then(toml::Value::as_array))
            .flatten()
            .filter_map(|entry| entry.get("name").and_then(toml::Value::as_str))
            .map(str::to_owned)
            .collect::<Vec<_>>()
    } else {
        options.selected.clone()
    };
    let missing_mirrors = mirror_names
        .iter()
        .filter(|name| !mirror_root.join(format!("{name}.git")).is_dir())
        .cloned()
        .collect::<Vec<_>>();
    let mirrors_ready = mirror_root.is_dir() && missing_mirrors.is_empty();
    checks.push(json!({
        "name": "local-bare-mirrors",
        "status": if mirrors_ready {"pass"} else {"blocked"},
        "path": mirror_root,
        "missing": missing_mirrors,
        "detail": if mirrors_ready {"selected repository mirrors are present"} else {"run the reviewed local mirror refresh before release CI"},
    }));

    let disk_free_kib = disk_free_kib(&split_root).unwrap_or(0);
    let disk_ready = disk_free_kib >= 10_000_000;
    checks.push(json!({
        "name": "disk-space",
        "status": if disk_ready {"pass"} else {"blocked"},
        "free_kib": disk_free_kib,
        "minimum_kib": 10_000_000,
        "detail": if disk_ready {"sufficient workspace disk is available"} else {"free at least 10 GiB in the Jain workspace filesystem"},
    }));

    let cgroup = Path::new("/sys/fs/cgroup").is_dir();
    let psi = Path::new("/proc/pressure").is_dir();
    checks.push(json!({
        "name": "linux-cgroup",
        "status": if cgroup {"pass"} else {"blocked"},
        "detail": if cgroup {"Linux cgroup controls are visible"} else {"run on a Linux host with /sys/fs/cgroup mounted"},
    }));
    checks.push(json!({
        "name": "linux-psi",
        "status": if psi {"pass"} else {"blocked"},
        "detail": if psi {"Linux PSI is visible"} else {"run on a Linux host exposing /proc/pressure"},
    }));

    let status = if checks.iter().all(|check| check["status"] == "pass") {
        "pass"
    } else {
        "blocked"
    };
    let receipt = options.evidence_dir.join("host-prerequisites.json");
    let report = json!({
        "schema_version": "jain.release-host-prerequisites/v1",
        "name": "host-prerequisites",
        "status": status,
        "release_version": RELEASE_VERSION,
        "manifest": options.manifest,
        "manifest_sha256": sha256_file(&options.manifest)?,
        "tool_version": TOOL_VERSION,
        "control_root": control_root,
        "checks": checks,
        "timestamp_unix": now_unix(),
        "receipt": receipt,
    });
    write_json(&receipt, &report)?;
    Ok(report)
}

fn executable_available(name: &str) -> bool {
    if name.contains('/') {
        return Path::new(name).is_file();
    }
    env::var_os("PATH")
        .into_iter()
        .flat_map(|path| env::split_paths(&path).collect::<Vec<_>>())
        .any(|directory| directory.join(name).is_file())
}

fn command_succeeds(program: &str, args: &[&str]) -> bool {
    Command::new(program)
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

fn disk_free_kib(path: &Path) -> Option<u64> {
    let output = Command::new("df")
        .args(["-Pk", path.to_str()?])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout)
        .ok()?
        .lines()
        .last()?
        .split_whitespace()
        .nth(3)?
        .parse()
        .ok()
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
    if branch != repo.expected_branch {
        let step = json!({
            "name": format!("ci:{}", repo.name),
            "repository": repo.name,
            "wave": repo.wave,
            "commit": commit,
            "checkout_head": head,
            "branch": branch,
            "expected_branch": repo.expected_branch,
            "status": "blocked",
            "reason": "release CI requires the canonical branch before detached exact-commit execution",
            "receipt": receipt,
            "log": log,
        });
        write_json(&receipt, &step)?;
        return Ok(step);
    }
    if repo.release_commit != PENDING && head != commit {
        let step = json!({
            "name": format!("ci:{}", repo.name),
            "repository": repo.name,
            "wave": repo.wave,
            "commit": commit,
            "checkout_head": head,
            "status": "blocked",
            "reason": "checkout HEAD does not equal the reviewed manifest release_commit",
            "receipt": receipt,
            "log": log,
        });
        write_json(&receipt, &step)?;
        return Ok(step);
    }
    let forge_main = match remote_branch_commit(&repo.path, &repo.remote, &repo.expected_branch) {
        Ok(value) => value,
        Err(error) => {
            let step = json!({
                "name": format!("ci:{}", repo.name),
                "repository": repo.name,
                "wave": repo.wave,
                "commit": commit,
                "checkout_head": head,
                "status": "blocked",
                "reason": "could not read the reviewed forge branch before release CI",
                "detail": error.to_string(),
                "receipt": receipt,
                "log": log,
            });
            write_json(&receipt, &step)?;
            return Ok(step);
        }
    };
    if forge_main.as_deref() != Some(commit.as_str()) {
        let step = json!({
            "name": format!("ci:{}", repo.name),
            "repository": repo.name,
            "wave": repo.wave,
            "commit": commit,
            "checkout_head": head,
            "forge_main": forge_main,
            "status": "blocked",
            "reason": "reviewed forge main does not resolve to the exact release CI commit",
            "receipt": receipt,
            "log": log,
        });
        write_json(&receipt, &step)?;
        return Ok(step);
    }
    if repo.release_commit != PENDING {
        let local_tag = git_output(
            &repo.path,
            &["rev-parse", &format!("refs/tags/{}^{{}}", repo.tag)],
        )
        .ok();
        if local_tag.as_deref() != Some(commit.as_str()) {
            let step = json!({
                "name": format!("ci:{}", repo.name),
                "repository": repo.name,
                "wave": repo.wave,
                "commit": commit,
                "checkout_head": head,
                "local_tag": local_tag,
                "tag": repo.tag,
                "status": "blocked",
                "reason": "reviewed immutable tag does not resolve locally to the release CI commit",
                "receipt": receipt,
                "log": log,
            });
            write_json(&receipt, &step)?;
            return Ok(step);
        }
    }
    if !options.force {
        if let Some(mut cached) = cached_step(
            &receipt,
            &log,
            &commit,
            manifest_sha256,
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
    let status = run_logged(&format!("ci:{}", repo.name), &mut command, &log)?;
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
        "tool_version": TOOL_VERSION,
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
    let status = run_logged(&format!("tag:{}", repo.name), &mut command, &log)?;
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
    let runner_cache = release_root.join("redline-family-ci.runner.json");
    if !force
        && !options.force
        && cached_family(
            &receipt,
            &log,
            &runner_cache,
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
    let status = run_logged("redline-family-ci", &mut command, &log)?;
    if status.success() {
        write_json(
            &runner_cache,
            &json!({
                "schema_version": "jain.release-candidate-cache/v1",
                "tool_version": TOOL_VERSION,
                "manifest_sha256": nested_hash,
                "family_receipt_sha256": sha256_file(&receipt)?,
                "log_sha256": sha256_file(&log)?,
                "timestamp_unix": now_unix(),
            }),
        )?;
    }
    Ok(json!({
        "name": "redline-family-ci",
        "status": if status.success() {"pass"} else {"fail"},
        "manifest": nested_manifest,
        "manifest_sha256": nested_hash,
        "receipt": receipt,
        "log": log,
        "log_sha256": sha256_file(&log)?,
        "tool_version": TOOL_VERSION,
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
        let status = run_logged(&format!("tag:{name}"), &mut command, &log)?;
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

fn run_redline_cutover(
    manifest: &toml::Value,
    options: &Options,
) -> Result<Vec<JsonValue>, Box<dyn std::error::Error>> {
    let control = PathBuf::from(
        manifest
            .get("nested_families")
            .and_then(|value| value.get("redline"))
            .and_then(|value| value.get("control_plane"))
            .and_then(toml::Value::as_str)
            .ok_or("manifest is missing nested_families.redline.control_plane")?,
    );
    let release_root = options
        .evidence_dir
        .parent()
        .ok_or("orchestrator evidence directory has no release root")?;
    let family = release_root.join("redline-family-ci.json");
    let jain = release_root.join("redline-consumer-jain-split.json");
    let jeryu = release_root.join("redline-consumer-jeryu-split.json");
    let consumer_checks = vec![
        validate_consumer_evidence(&jain, "jain-split", &family)?,
        validate_consumer_evidence(&jeryu, "jeryu-split", &family)?,
    ];
    if consumer_checks.iter().any(|step| !step_green(step)) {
        return Ok(consumer_checks);
    }

    let proof_receipt = release_root.join("redline-proof-refresh.json");
    let mut proof = Command::new(control.join("redlinectl"));
    proof
        .current_dir(&control)
        .arg("proof-refresh")
        .arg("--family-ci")
        .arg(&family)
        .arg("--jain-evidence")
        .arg(&jain)
        .arg("--jeryu-evidence")
        .arg(&jeryu)
        .arg("--receipt")
        .arg(&proof_receipt);
    let proof_step = run_control_step(
        "redline-proof-refresh",
        &mut proof,
        &options.evidence_dir,
        "fail",
    )?;
    if !step_green(&proof_step) {
        return Ok(vec![proof_step]);
    }

    let cutover = run_control_step(
        "redline-cutover-verify",
        Command::new(control.join("redlinectl"))
            .current_dir(&control)
            .arg("cutover-verify"),
        &options.evidence_dir,
        "fail",
    )?;
    if !step_green(&cutover) {
        return Ok([consumer_checks, vec![proof_step, cutover]].concat());
    }
    let lock_check = verify_redline_lock_pair(manifest, &release_root)?;
    Ok([consumer_checks, vec![proof_step, cutover, lock_check]].concat())
}

fn validate_consumer_evidence(
    path: &Path,
    consumer: &str,
    family_ci: &Path,
) -> Result<JsonValue, Box<dyn std::error::Error>> {
    let mut failures = Vec::new();
    let sidecar = path.with_extension("json.sha256");
    let mut digest = None;
    let value = if !path.is_file() {
        failures.push("consumer evidence file is missing".to_owned());
        JsonValue::Null
    } else {
        let bytes = fs::read(path)?;
        digest = Some(sha256_bytes(&bytes));
        if !sidecar.is_file() {
            failures.push("consumer evidence SHA-256 sidecar is missing".to_owned());
        } else {
            let declared = fs::read_to_string(&sidecar)?;
            if declared.split_whitespace().next() != digest.as_deref() {
                failures.push("consumer evidence SHA-256 sidecar does not match bytes".to_owned());
            }
        }
        match serde_json::from_slice::<JsonValue>(&bytes) {
            Ok(value) => value,
            Err(error) => {
                failures.push(format!("consumer evidence is not valid JSON: {error}"));
                JsonValue::Null
            }
        }
    };
    let family_digest = sha256_file(family_ci).unwrap_or_default();
    if value["schema_version"] != "redline.consumer-evidence/v1" {
        failures.push("consumer evidence schema is invalid".to_owned());
    }
    if value["consumer"] != consumer {
        failures.push("consumer evidence identity is invalid".to_owned());
    }
    if value["status"] != "pass" {
        failures.push("consumer evidence status is not pass".to_owned());
    }
    for field in ["source_commit", "engine_commit"] {
        let valid = value[field]
            .as_str()
            .is_some_and(|sha| sha.len() == 40 && sha.bytes().all(|byte| byte.is_ascii_hexdigit()));
        if !valid {
            failures.push(format!(
                "consumer evidence {field} is not an immutable commit"
            ));
        }
    }
    for field in ["engine_tag", "proof_lock_id", "generated_at"] {
        if value[field].as_str().is_none_or(str::is_empty) {
            failures.push(format!("consumer evidence is missing {field}"));
        }
    }
    let expected_check = format!("{consumer}/redline-consumer");
    if value["required_check"] != expected_check {
        failures.push(format!(
            "consumer evidence required_check must be {expected_check}"
        ));
    }
    if value["family_ci_receipt_sha256"] != family_digest {
        failures.push("consumer evidence is not bound to the exact family receipt".to_owned());
    }
    if let (Ok(evidence_mtime), Ok(family_mtime)) = (
        fs::metadata(path).and_then(|metadata| metadata.modified()),
        fs::metadata(family_ci).and_then(|metadata| metadata.modified()),
    ) {
        if evidence_mtime < family_mtime {
            failures.push("consumer evidence predates the post-tag family receipt".to_owned());
        }
    }
    Ok(json!({
        "name": format!("redline-consumer-evidence:{consumer}"),
        "consumer": consumer,
        "status": if failures.is_empty() {"pass"} else {"blocked"},
        "evidence": path,
        "sidecar": sidecar,
        "evidence_sha256": digest,
        "family_ci_receipt": family_ci,
        "family_ci_receipt_sha256": family_digest,
        "engine_tag": value["engine_tag"],
        "engine_commit": value["engine_commit"],
        "proof_lock_id": value["proof_lock_id"],
        "source_commit": value["source_commit"],
        "generated_at": value["generated_at"],
        "failures": failures,
    }))
}

fn verify_redline_lock_pair(
    manifest: &toml::Value,
    release_root: &Path,
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
    let container = PathBuf::from(
        nested
            .get("container_path")
            .and_then(toml::Value::as_str)
            .ok_or("Redline container_path is missing")?,
    );
    let primary = control.join("redline.lock.toml");
    let mirror = container.join("redline.lock.toml");
    let primary_bytes = fs::read(&primary).unwrap_or_default();
    let mirror_bytes = fs::read(&mirror).unwrap_or_default();
    let parsed: Option<toml::Value> = std::str::from_utf8(&primary_bytes)
        .ok()
        .and_then(|text| text.parse().ok());
    let eligible = parsed
        .as_ref()
        .and_then(|lock| lock.get("proof"))
        .and_then(|proof| proof.get("cutover_eligible"))
        .and_then(toml::Value::as_bool)
        == Some(true);
    let equal = !primary_bytes.is_empty() && primary_bytes == mirror_bytes;
    let status = if equal && eligible { "pass" } else { "blocked" };
    let receipt = release_root.join("redline-lock-pair.json");
    let report = json!({
        "schema_version": "jain.redline-lock-pair/v1",
        "name": "redline-lock-pair",
        "status": status,
        "primary": primary,
        "mirror": mirror,
        "primary_sha256": sha256_bytes(&primary_bytes),
        "mirror_sha256": sha256_bytes(&mirror_bytes),
        "byte_identical": equal,
        "cutover_eligible": eligible,
        "receipt": receipt,
        "timestamp_unix": now_unix(),
    });
    write_json(&receipt, &report)?;
    Ok(report)
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
    let status = run_logged(name, command, &log)?;
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
        if let Err(error) =
            validate_atomicsoul_receipt(&atomicsoul_dir.join("atomicsoul-dry-run.receipt.json"))
        {
            let mut failed = dry_run;
            failed["status"] = json!("fail");
            failed["reason"] = json!(error.to_string());
            return Ok(vec![contract, failed]);
        }
    }
    Ok(vec![contract, dry_run])
}

fn run_staged_artifact(
    control_root: &Path,
    evidence_dir: &Path,
) -> Result<Vec<JsonValue>, Box<dyn std::error::Error>> {
    let workspace = control_root.parent().ok_or("control root has no parent")?;
    let deploy = workspace.join("jain-deploy");
    let artifact_dir = evidence_dir.join("artifact");
    let image_repo =
        env::var("ATOMICSOUL_IMAGE_REPO").unwrap_or_else(|_| "jain-candidate/local".to_owned());
    let native_root = env::var_os("JAIN_NATIVE_SOURCE_ROOT").map(PathBuf::from);
    let vendor_root = env::var_os("JAIN_VENDOR_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| artifact_dir.join("native-vendor"));
    let mut build = Command::new("bash");
    build
        .current_dir(&deploy)
        .arg(deploy.join("scripts/release-atomicsoul.sh"))
        .env("ATOMICSOUL_PUSH", "0")
        .env("ATOMICSOUL_IMAGE_REPO", &image_repo)
        .env("ATOMICSOUL_IMAGE_TAG", RELEASE_VERSION)
        .env("ATOMICSOUL_EVIDENCE_DIR", &artifact_dir)
        .env("JAIN_VENDOR_ROOT", &vendor_root);
    if let Some(native_root) = native_root {
        build.env("JAIN_NATIVE_SOURCE_ROOT", native_root);
    }
    let build_step = run_control_step("staged-artifact-image", &mut build, evidence_dir, "fail")?;
    if !step_green(&build_step) {
        return Ok(vec![
            build_step,
            blocked_step(
                "artifact-security-evidence",
                "the staged candidate image must build before local SBOM, vulnerability, and audit evidence can run",
            ),
        ]);
    }

    let image = format!("{image_repo}:{RELEASE_VERSION}");
    let sbom = artifact_dir.join("sbom.spdx.json");
    let grype = artifact_dir.join("grype.json");
    let sbom_step = run_control_step(
        "artifact-sbom",
        Command::new("syft")
            .arg(&image)
            .arg("-o")
            .arg(format!("spdx-json={}", sbom.display())),
        evidence_dir,
        "fail",
    )?;
    let grype_step = if step_green(&sbom_step) {
        run_control_step(
            "artifact-vulnerability-scan",
            Command::new("grype")
                .arg(format!("sbom:{}", sbom.display()))
                .arg("-o")
                .arg(format!("json={}", grype.display()))
                .arg("--fail-on")
                .arg("high"),
            evidence_dir,
            "fail",
        )?
    } else {
        blocked_step("artifact-vulnerability-scan", "SBOM generation failed")
    };
    let audit_step = run_control_step(
        "artifact-cargo-audit",
        Command::new("cargo")
            .current_dir(&deploy)
            .args(["audit", "--locked"]),
        evidence_dir,
        "fail",
    )?;
    let signature_receipt = artifact_dir.join("signature-verification.json");
    let signature = json!({
        "schema_version": "jain.release-signature/v1",
        "status": "pass",
        "mode": "non-pushing-candidate",
        "verified": false,
        "reason": "ATOMICSOUL_PUSH=0; no production registry signature is expected for a local candidate",
        "production_push": false,
        "timestamp_unix": now_unix(),
    });
    write_json(&signature_receipt, &signature)?;
    let security_status = if [
        step_green(&sbom_step),
        step_green(&grype_step),
        step_green(&audit_step),
    ]
    .into_iter()
    .all(|value| value)
    {
        "pass"
    } else {
        "fail"
    };
    let security_receipt = artifact_dir.join("security-evidence.json");
    write_json(
        &security_receipt,
        &json!({
            "schema_version": "jain.release-artifact-security/v1",
            "status": security_status,
            "release_version": RELEASE_VERSION,
            "atomicsoul_push": false,
            "sbom": sbom,
            "vulnerability_report": grype,
            "cargo_audit": evidence_dir.join("artifact-cargo-audit.log"),
            "signature": signature_receipt,
            "timestamp_unix": now_unix(),
        }),
    )?;
    Ok(vec![
        build_step,
        sbom_step,
        grype_step,
        audit_step,
        json!({
            "name": "artifact-security-evidence",
            "status": security_status,
            "receipt": security_receipt,
        }),
    ])
}

fn run_final_validation(
    control_root: &Path,
    options: &Options,
) -> Result<Vec<JsonValue>, Box<dyn std::error::Error>> {
    let snapshot = options.evidence_dir.join("release-snapshot.json");
    let snapshot_step = run_control_step(
        "final-snapshot",
        Command::new(env::current_exe()?)
            .arg("release-snapshot")
            .arg("--manifest")
            .arg(&options.manifest)
            .arg("--json")
            .arg(&snapshot)
            .arg("--apply"),
        &options.evidence_dir,
        "fail",
    )?;
    let rollback_receipt = options.evidence_dir.join("rollback.json");
    let deploy = control_root
        .parent()
        .ok_or("control root has no parent")?
        .join("jain-deploy");
    let dry_run = deploy.join("scripts/atomicsoul-dry-run.sh");
    let rollback_contract = fs::read_to_string(&dry_run)
        .map(|contents| {
            contents.contains("rollback_target=\"7.0.6\"") && contents.contains("rollback --to")
        })
        .unwrap_or(false);
    let rollback = json!({
        "schema_version": "jain.release-rollback/v1",
        "status": if rollback_contract {"pass"} else {"blocked"},
        "target": "7.0.6",
        "command": "deployctl rollback --to 7.0.6",
        "production_applied": false,
        "source": dry_run,
        "reason": if rollback_contract {"dry-run rollback contract is present"} else {"the 7.0.6 rollback contract is missing"},
        "timestamp_unix": now_unix(),
    });
    write_json(&rollback_receipt, &rollback)?;
    Ok(vec![
        snapshot_step,
        json!({
            "name": "rollback-validation",
            "status": rollback["status"],
            "receipt": rollback_receipt,
        }),
    ])
}

fn finish_run(
    aggregate_path: &Path,
    report: &mut JsonValue,
    steps: Vec<JsonValue>,
    manifest: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let overall = overall_status(&steps);
    let blocked_steps = steps
        .iter()
        .filter(|step| step["status"] == "blocked")
        .count();
    let failed_steps = steps.iter().filter(|step| step["status"] == "fail").count();
    report["steps"] = json!(steps);
    report["status"] = json!(overall);
    report["blocked_steps"] = json!(blocked_steps);
    report["failed_steps"] = json!(failed_steps);
    report["rollback_target"] = json!("7.0.6");
    let final_manifest_sha256 = sha256_file(manifest)?;
    let manifest_changed =
        report["manifest_sha256"].as_str() != Some(final_manifest_sha256.as_str());
    report["final_manifest_sha256"] = json!(final_manifest_sha256);
    report["manifest_changed"] = json!(manifest_changed);
    report["finished_at_unix"] = json!(now_unix());
    write_json(aggregate_path, report)?;
    println!("release candidate {overall}: {}", aggregate_path.display());
    if overall == "pass" {
        Ok(())
    } else {
        Err(format!("release candidate is {overall}; see the aggregate receipt").into())
    }
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
    label: &str,
    command: &mut Command,
    log: &Path,
) -> Result<std::process::ExitStatus, Box<dyn std::error::Error>> {
    if let Some(parent) = log.parent() {
        fs::create_dir_all(parent)?;
    }
    let stdout = File::create(log)?;
    let stderr = stdout.try_clone()?;
    let program = command.get_program().to_string_lossy().into_owned();
    let started = Instant::now();
    eprintln!("START {label}: {program} (log: {})", log.display());
    let status = command
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr))
        .status()?;
    eprintln!(
        "{} {label}: {program} in {} ms (log: {})",
        if status.success() { "PASS" } else { "FAIL" },
        started.elapsed().as_millis(),
        log.display()
    );
    Ok(status)
}

fn checkpoint_report(
    aggregate_path: &Path,
    report: &mut JsonValue,
    steps: &[JsonValue],
) -> Result<(), Box<dyn std::error::Error>> {
    report["steps"] = json!(steps);
    report["status"] = json!("running");
    if let Some(step) = steps.last() {
        report["last_attempted_step"] = step["name"].clone();
        if let Some(log) = step.get("log") {
            report["last_log_path"] = log.clone();
        }
    }
    if let Some(step) = steps
        .iter()
        .rev()
        .find(|step| matches!(step["status"].as_str(), Some("pass" | "cached" | "skipped")))
    {
        report["last_completed_step"] = step["name"].clone();
        if let Some(log) = step.get("log") {
            report["last_log_path"] = log.clone();
        }
    }
    report["last_updated_at_unix"] = json!(now_unix());
    write_json(aggregate_path, report)
}

fn cached_step(
    receipt: &Path,
    log: &Path,
    commit: &str,
    manifest_sha256: &str,
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
        && value["manifest_sha256"] == manifest_sha256
        && value["policy_sha256"] == policy_sha256
        && value["tool_version"] == TOOL_VERSION
        && value["log_sha256"].as_str() == Some(&sha256_file(log)?)
        && fresh;
    Ok(valid.then_some(value))
}

fn cached_family(
    receipt: &Path,
    log: &Path,
    runner_cache: &Path,
    manifest_sha256: &str,
    max_age_hours: u64,
    require_verified_tags: bool,
) -> Result<bool, Box<dyn std::error::Error>> {
    if !receipt.is_file() || !log.is_file() || !runner_cache.is_file() {
        return Ok(false);
    }
    let value: JsonValue = serde_json::from_slice(&fs::read(receipt)?)?;
    let cache: JsonValue = serde_json::from_slice(&fs::read(runner_cache)?)?;
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
        && now_unix().saturating_sub(modified) <= max_age_hours.saturating_mul(3600)
        && cache["tool_version"] == TOOL_VERSION
        && cache["manifest_sha256"] == manifest_sha256
        && cache["family_receipt_sha256"].as_str() == Some(&sha256_file(receipt)?)
        && cache["log_sha256"].as_str() == Some(&sha256_file(log)?))
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
}
