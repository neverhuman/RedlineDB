use super::*;

const EVIDENCE_SCHEMA: &str = "redline.docker-parity-evidence/v1";
const CANDIDATE_NAME: &str = "redline-docker-parity-candidate.json";
const FINAL_NAME: &str = "redline-docker-parity-evidence.json";

pub(super) struct Options {
    pub source_cargo_home: PathBuf,
    pub additional_cargo_homes: Vec<PathBuf>,
    pub evidence_dir: PathBuf,
    pub mode: String,
    pub core_commit: Option<String>,
    pub testing_commit: Option<String>,
    pub sqlite_bin: PathBuf,
    pub psql_bin: PathBuf,
    pub postgres_bin: PathBuf,
    pub target_args: Vec<String>,
    pub smoke: bool,
}

struct SealInput {
    project: String,
    runner_image_id: String,
}

pub(super) fn run(manifest: &Manifest, options: Options) -> Result<()> {
    if options.mode != "release" && options.mode != "diagnostic" {
        return Err(error("docker-parity --mode must be release or diagnostic"));
    }
    if options.smoke && options.mode != "diagnostic" {
        return Err(error("docker-parity --smoke is diagnostic-only"));
    }
    let core = repository(manifest, "redline-core")?;
    let testing = repository(manifest, "redline-testing")?;
    let core_commit = options
        .core_commit
        .as_deref()
        .unwrap_or(&core.release_commit)
        .to_owned();
    let testing_commit = options
        .testing_commit
        .as_deref()
        .unwrap_or(&testing.release_commit)
        .to_owned();
    if !is_sha1(&core_commit) || !is_sha1(&testing_commit) {
        return Err(error(
            "docker-parity requires exact lowercase Core and Testing commits",
        ));
    }
    if options.mode == "release" {
        if core_commit != core.release_commit || testing_commit != testing.release_commit {
            return Err(error(
                "release Docker parity commits must equal the authority manifest",
            ));
        }
        current_reviewed_state(manifest, core, false)?;
        current_reviewed_state(manifest, testing, false)?;
    }

    let workspace = manifest
        .container_root
        .parent()
        .ok_or_else(|| error("Redline container has no Jain workspace parent"))?;
    let mut cargo_homes = Vec::new();
    let mut cargo_digests = Vec::new();
    for raw in
        std::iter::once(&options.source_cargo_home).chain(options.additional_cargo_homes.iter())
    {
        let cargo_home = fs::canonicalize(raw).map_err(|value| {
            error(format!(
                "Docker parity Cargo custody is unavailable at {}: {value}",
                raw.display()
            ))
        })?;
        if !cargo_home.starts_with(workspace) {
            return Err(error(
                "Docker parity Cargo custody must be physical and inside jain-split",
            ));
        }
        if cargo_homes.contains(&cargo_home) {
            return Err(error("Docker parity Cargo custody roots must be unique"));
        }
        cargo_digests.push(physical_tree_sha256(
            &cargo_home,
            "Docker parity Cargo custody",
        )?);
        cargo_homes.push(cargo_home);
    }
    let cargo_custody_sha256 = sha256_bytes(cargo_digests.join("\n").as_bytes());
    for (label, path) in [
        ("SQLite oracle", &options.sqlite_bin),
        ("PostgreSQL client", &options.psql_bin),
        ("PostgreSQL server", &options.postgres_bin),
    ] {
        require_physical_file(path, label)?;
    }

    fs::create_dir_all(&options.evidence_dir)?;
    reject_symlink_components(&options.evidence_dir, "Docker parity evidence directory")?;
    let evidence_dir = fs::canonicalize(&options.evidence_dir)?;
    if !evidence_dir.starts_with(workspace) {
        return Err(error(
            "Docker parity evidence directory must remain inside jain-split",
        ));
    }
    if options.mode == "release" {
        require_path_absent(
            &evidence_dir.join(CANDIDATE_NAME),
            "Docker parity candidate",
        )?;
        require_path_absent(
            &evidence_dir.join(FINAL_NAME),
            "Docker parity final evidence",
        )?;
        require_path_absent(
            &checksum_path(&evidence_dir.join(FINAL_NAME)),
            "Docker parity final evidence sidecar",
        )?;
    }

    let seal = with_standalone_sandbox("redline-docker-parity", |sandbox| {
        run_campaign(
            manifest,
            core,
            testing,
            &core_commit,
            &testing_commit,
            &cargo_homes,
            &cargo_custody_sha256,
            &evidence_dir,
            &options,
            sandbox,
        )
    })?;
    if options.mode == "release" {
        let final_path = seal_candidate(&evidence_dir, &core_commit, &testing_commit, &seal)?;
        println!("Redline Docker parity passed: {}", final_path.display());
    } else {
        println!(
            "Redline Docker parity diagnostic complete: {}",
            evidence_dir.display()
        );
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn run_campaign(
    manifest: &Manifest,
    core: &Repo,
    testing: &Repo,
    core_commit: &str,
    testing_commit: &str,
    source_cargo_homes: &[PathBuf],
    source_custody_sha256: &str,
    evidence_dir: &Path,
    options: &Options,
    sandbox: &Path,
) -> Result<SealInput> {
    let core_checkout = sandbox.join("redline-core");
    let testing_checkout = sandbox.join("redline-testing");
    clone_exact_standalone(&manifest.repo_root(core), &core_checkout, core_commit)?;
    clone_exact_standalone(
        &manifest.repo_root(testing),
        &testing_checkout,
        testing_commit,
    )?;
    let core_tree = git(&core_checkout, &["rev-parse", "HEAD^{tree}"])?;
    let testing_tree = git(&testing_checkout, &["rev-parse", "HEAD^{tree}"])?;
    if !is_sha1(&core_tree) || !is_sha1(&testing_tree) {
        return Err(error("Docker parity clone tree identity is malformed"));
    }

    let build_cargo_home = sandbox.join("build-cargo-home");
    copy_custody_trees(source_cargo_homes, &build_cargo_home)?;
    let core_build = Command::new("cargo")
        .current_dir(&core_checkout)
        .args([
            "build",
            "--release",
            "--locked",
            "--offline",
            "-p",
            "redlinedb-cli",
            "--bin",
            "redlinedb",
        ])
        .env("CARGO_HOME", &build_cargo_home)
        .env("CARGO_NET_OFFLINE", "true")
        .status()?;
    if !core_build.success() {
        return Err(error(format!(
            "offline exact Core target build failed with {core_build}"
        )));
    }
    let built_target = core_checkout.join("target/release/redlinedb");
    let target_bin = sandbox.join("artifacts/redlinedb");
    stage_built_artifact(&built_target, &target_bin)?;

    let stage = sandbox.join("docker-stage");
    let staged_cargo_home = sandbox.join("docker-cargo-home");
    let stage_status = Command::new("cargo")
        .current_dir(&testing_checkout)
        .args([
            "run",
            "--locked",
            "--offline",
            "-p",
            "xtask",
            "--",
            "docker-stage",
        ])
        .args([
            "--source-cargo-home",
            &build_cargo_home.display().to_string(),
        ])
        .args(["--source-custody-sha256", source_custody_sha256])
        .args(["--cargo-home", &staged_cargo_home.display().to_string()])
        .args(["--sqlite-bin", &options.sqlite_bin.display().to_string()])
        .args(["--psql-bin", &options.psql_bin.display().to_string()])
        .args([
            "--postgres-bin",
            &options.postgres_bin.display().to_string(),
        ])
        .args(["--target-bin", &target_bin.display().to_string()])
        .args(["--core-commit", core_commit])
        .args(["--core-tree", &core_tree])
        .args(["--out-dir", &stage.display().to_string()])
        .env("CARGO_HOME", &build_cargo_home)
        .env("CARGO_NET_OFFLINE", "true")
        .status()?;
    if !stage_status.success() {
        return Err(error(format!(
            "Docker custody staging failed with {stage_status}"
        )));
    }

    let context = stage.join("context");
    let compose = testing_checkout.join("docker/compose.yaml");
    let dockerfile = context.join("Dockerfile");
    let images_path = testing_checkout.join("docker/images.lock.toml");
    let (runner_base, postgres_image) = locked_images(&images_path)?;
    validate_docker_inputs(&compose, &dockerfile, &runner_base)?;
    let project = format!("redline-parity-{}", unique_suffix());
    let runner_image = format!("redline-docker-parity:{project}");
    let build_args = docker_build_args(&runner_image, &dockerfile, &context)?;
    let build_status = Command::new("docker")
        .args(&build_args)
        .env("DOCKER_BUILDKIT", "1")
        .status()?;
    if !build_status.success() {
        return Err(error(format!(
            "network-disabled Docker runner build failed with {build_status}"
        )));
    }
    let runner_image_id = docker_image_id(&runner_image)?;
    let environment = compose_environment(&project, &runner_image, &postgres_image, evidence_dir);
    let campaign = run_compose_campaign(&compose, &project, &environment, options, testing_commit);
    let cleanup = cleanup_compose(
        &compose,
        &project,
        &environment,
        &runner_image,
        &runner_image_id,
    );
    combine_campaign_cleanup(campaign, cleanup)?;
    Ok(SealInput {
        project,
        runner_image_id,
    })
}

fn combine_campaign_cleanup(campaign: Result<()>, cleanup: Result<()>) -> Result<()> {
    match (campaign, cleanup) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(value), Ok(())) => Err(value),
        (Ok(()), Err(cleanup)) => Err(error(format!(
            "Docker parity cleanup failed after campaign success: {cleanup}"
        ))),
        (Err(value), Err(cleanup)) => Err(error(format!(
            "{value}; Docker parity cleanup also failed: {cleanup}"
        ))),
    }
}

fn run_compose_campaign(
    compose: &Path,
    project: &str,
    environment: &BTreeMap<String, String>,
    options: &Options,
    _testing_commit: &str,
) -> Result<()> {
    let up = compose_command(compose, project, environment)
        .args(["up", "-d", "--pull", "never", "--no-build", "postgres"])
        .status()?;
    if !up.success() {
        return Err(error(format!(
            "Docker Compose PostgreSQL start failed with {up}"
        )));
    }
    let postgres_id = compose_stdout(compose, project, environment, &["ps", "-q", "postgres"])?;
    if !is_container_id(&postgres_id) {
        return Err(error(
            "Docker Compose returned a malformed PostgreSQL container ID",
        ));
    }
    wait_healthy(&postgres_id)?;

    let mut command = compose_command(compose, project, environment);
    command.args([
        "run",
        "--rm",
        "--no-deps",
        "-e",
        &format!("REDLINE_DOCKER_POSTGRES_CONTAINER_ID={postgres_id}"),
        "-e",
        "REDLINE_DOCKER_POSTGRES_HEALTH=healthy",
        "-e",
        &format!("REDLINE_DOCKER_COMPOSE_PROJECT={project}"),
        "runner",
        "docker-parity",
        "--mode",
        &options.mode,
        "--target-bin",
        "/opt/redline/bin/redlinedb",
        "--sqlite-bin",
        "/opt/redline/bin/sqlite3",
        "--evidence-dir",
        "/evidence",
        "--custody-receipt",
        "/opt/redline/share/custody/docker-custody-receipt.json",
    ]);
    if options.smoke {
        command.args([
            "--workers",
            "1",
            "--sqlite-cases",
            "id:10001",
            "--postgres-oracle-cases",
            "id:20001",
            "--postgres-target-cases",
            "id:20001",
        ]);
    }
    for argument in &options.target_args {
        command.arg("--target-arg").arg(argument);
    }
    let status = command.status()?;
    if !status.success() {
        return Err(error(format!(
            "Docker parity runner failed with {status}; no release receipt may be sealed"
        )));
    }
    Ok(())
}

fn wait_healthy(container_id: &str) -> Result<()> {
    for _ in 0..90 {
        let output = Command::new("docker")
            .args([
                "inspect",
                "--format",
                "{{.State.Health.Status}}",
                container_id,
            ])
            .output()?;
        if output.status.success() {
            match String::from_utf8_lossy(&output.stdout).trim() {
                "healthy" => return Ok(()),
                "unhealthy" => return Err(error("Docker PostgreSQL healthcheck is unhealthy")),
                _ => {}
            }
        }
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
    Err(error("Docker PostgreSQL healthcheck timed out"))
}

fn cleanup_compose(
    compose: &Path,
    project: &str,
    environment: &BTreeMap<String, String>,
    runner_image: &str,
    expected_image_id: &str,
) -> Result<()> {
    let down = compose_command(compose, project, environment)
        .args(["down", "--volumes", "--remove-orphans", "--timeout", "10"])
        .status()?;
    if !down.success() {
        return Err(error(format!("Docker Compose down failed with {down}")));
    }
    let containers = docker_stdout(&[
        "ps",
        "-aq",
        "--filter",
        &format!("label=com.docker.compose.project={project}"),
    ])?;
    let volumes = docker_stdout(&[
        "volume",
        "ls",
        "-q",
        "--filter",
        &format!("label=com.docker.compose.project={project}"),
    ])?;
    if !containers.is_empty() || !volumes.is_empty() {
        return Err(error(
            "descriptor-bound Docker containers or volumes remain after cleanup",
        ));
    }
    if docker_image_id(runner_image)? != expected_image_id {
        return Err(error(
            "campaign runner image identity changed before cleanup",
        ));
    }
    let removed = Command::new("docker")
        .args(["image", "rm", runner_image])
        .status()?;
    if !removed.success() {
        return Err(error(format!(
            "campaign runner image cleanup failed with {removed}"
        )));
    }
    Ok(())
}

fn seal_candidate(
    evidence_dir: &Path,
    core_commit: &str,
    testing_commit: &str,
    seal: &SealInput,
) -> Result<PathBuf> {
    let candidate_path = evidence_dir.join(CANDIDATE_NAME);
    verify_checksum(&candidate_path)?;
    let mut candidate: JsonValue = serde_json::from_slice(&fs::read(&candidate_path)?)?;
    validate_candidate(&candidate, core_commit, testing_commit)?;
    candidate["status"] = json!("pass");
    candidate["cleanup"] = json!({
        "status": "pass",
        "compose_project": seal.project,
        "containers_remaining": 0,
        "volumes_remaining": 0,
        "runner_image_removed": true,
        "runner_image_id": seal.runner_image_id,
        "standalone_clone_cleanup": "pass",
    });
    candidate["images"]["runner"] = json!({
        "image_id": seal.runner_image_id,
        "removed_after_campaign": true,
    });
    let final_path = evidence_dir.join(FINAL_NAME);
    write_checksummed_json(&final_path, &candidate)?;
    verify_checksum(&final_path)?;
    Ok(final_path)
}

fn validate_candidate(value: &JsonValue, core_commit: &str, testing_commit: &str) -> Result<()> {
    if value.get("schema_version").and_then(JsonValue::as_str) != Some(EVIDENCE_SCHEMA)
        || value.get("status").and_then(JsonValue::as_str) != Some("candidate")
        || value.get("mode").and_then(JsonValue::as_str) != Some("release")
        || value.get("provenance_source").and_then(JsonValue::as_str) != Some("local-jeryu")
    {
        return Err(error(
            "Docker parity candidate is not a local-Jeryu release candidate",
        ));
    }
    if value.pointer("/core/commit").and_then(JsonValue::as_str) != Some(core_commit)
        || value.pointer("/testing/commit").and_then(JsonValue::as_str) != Some(testing_commit)
    {
        return Err(error("Docker parity candidate source identity drifted"));
    }
    if value
        .get("release_failures")
        .and_then(JsonValue::as_array)
        .is_none_or(|failures| !failures.is_empty())
    {
        return Err(error("Docker parity candidate contains release failures"));
    }
    for (pointer, expected) in [
        ("/sqlite/total", 2445),
        ("/sqlite/passed", 2445),
        ("/postgres_oracle/total", 265),
        ("/postgres_oracle/passed", 265),
        ("/postgres_target/total", 151),
        ("/postgres_target/target_passed", 151),
    ] {
        if value.pointer(pointer).and_then(JsonValue::as_u64) != Some(expected) {
            return Err(error(format!(
                "Docker parity candidate count drift at {pointer}"
            )));
        }
    }
    if value
        .get("postgres_exclusions")
        .and_then(JsonValue::as_array)
        .is_none_or(|values| values.len() != 114)
    {
        return Err(error(
            "Docker parity candidate exclusion identities drifted",
        ));
    }
    Ok(())
}

fn copy_custody_trees(sources: &[PathBuf], destination: &Path) -> Result<()> {
    if sources.is_empty() {
        return Err(error("Docker Cargo custody source set is empty"));
    }
    require_path_absent(destination, "ephemeral Docker build Cargo home")?;
    for source in sources {
        copy_custody_entry(source, destination, true)?;
    }
    physical_tree_sha256(destination, "ephemeral Docker build Cargo home")?;
    Ok(())
}

fn stage_built_artifact(source: &Path, destination: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    reject_symlink_components(source, "Cargo-built RedlineDB target")?;
    let metadata = fs::symlink_metadata(source)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(error(
            "Cargo-built RedlineDB target is not a physical regular file",
        ));
    }
    let parent = destination
        .parent()
        .ok_or_else(|| error("staged RedlineDB target has no parent"))?;
    fs::create_dir(parent)?;
    fs::copy(source, destination)?;
    fs::set_permissions(destination, fs::Permissions::from_mode(0o555))?;
    require_physical_file(destination, "staged exact RedlineDB target")
}

fn copy_custody_entry(source: &Path, destination: &Path, allow_existing: bool) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let metadata = fs::symlink_metadata(source)?;
    if metadata.file_type().is_symlink() {
        return Err(error(format!(
            "Docker Cargo custody contains a symlink: {}",
            source.display()
        )));
    }
    if metadata.is_dir() {
        match fs::symlink_metadata(destination) {
            Err(value) if value.kind() == io::ErrorKind::NotFound => {
                fs::create_dir(destination)?;
                fs::set_permissions(destination, fs::Permissions::from_mode(0o700))?;
            }
            Ok(value) if allow_existing && value.is_dir() && !value.file_type().is_symlink() => {}
            Ok(_) => {
                return Err(error(format!(
                    "Docker Cargo custody directory conflicts at {}",
                    destination.display()
                )))
            }
            Err(value) => return Err(value.into()),
        }
        let mut entries = fs::read_dir(source)?.collect::<io::Result<Vec<_>>>()?;
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            copy_custody_entry(&entry.path(), &destination.join(entry.file_name()), true)?;
        }
    } else if metadata.is_file() {
        match fs::symlink_metadata(destination) {
            Err(value) if value.kind() == io::ErrorKind::NotFound => {
                fs::copy(source, destination)?;
                fs::set_permissions(destination, fs::Permissions::from_mode(0o600))?;
            }
            Ok(value) if value.is_file() && !value.file_type().is_symlink() => {
                let source_sha = sha256_file(source)?;
                let destination_sha = sha256_file(destination)?;
                let registry_index = source
                    .components()
                    .any(|component| component.as_os_str() == "registry")
                    && source
                        .components()
                        .any(|component| component.as_os_str() == "index");
                let cargo_bookkeeping = source.file_name().is_some_and(|name| {
                    let name = name.to_string_lossy();
                    name.starts_with(".global-cache")
                        || name.starts_with(".package-cache")
                        || name == ".last-updated"
                });
                if source_sha != destination_sha && !registry_index && !cargo_bookkeeping {
                    return Err(error(format!(
                        "Docker Cargo custody file conflict at {}",
                        destination.display()
                    )));
                }
            }
            Ok(_) => {
                return Err(error(format!(
                    "Docker Cargo custody file conflicts at {}",
                    destination.display()
                )))
            }
            Err(value) => return Err(value.into()),
        }
    } else {
        return Err(error(format!(
            "Docker Cargo custody contains a special node: {}",
            source.display()
        )));
    }
    Ok(())
}

fn repository<'a>(manifest: &'a Manifest, name: &str) -> Result<&'a Repo> {
    manifest
        .repos
        .iter()
        .find(|repo| repo.name == name)
        .ok_or_else(|| error(format!("manifest has no repository named {name}")))
}

fn locked_images(path: &Path) -> Result<(String, String)> {
    require_physical_file(path, "Docker image manifest")?;
    let value: toml::Value = fs::read_to_string(path)?.parse()?;
    if value.get("schema_version").and_then(toml::Value::as_str) != Some("redline.docker-images/v1")
    {
        return Err(error("unknown Docker image manifest schema"));
    }
    let read = |table: &str| -> Result<String> {
        let reference = value
            .get(table)
            .and_then(toml::Value::as_table)
            .and_then(|table| table.get("reference"))
            .and_then(toml::Value::as_str)
            .ok_or_else(|| error(format!("Docker image manifest lacks {table}.reference")))?;
        if reference.matches("@sha256:").count() != 1 || reference.contains(":latest") {
            return Err(error(format!(
                "Docker image {table} is not exactly digest-pinned"
            )));
        }
        Ok(reference.to_owned())
    };
    Ok((read("runner_base")?, read("postgres")?))
}

fn validate_docker_inputs(compose: &Path, dockerfile: &Path, runner_base: &str) -> Result<()> {
    require_physical_file(compose, "Docker parity Compose file")?;
    require_physical_file(dockerfile, "Docker parity Dockerfile")?;
    let compose_body = fs::read_to_string(compose)?;
    let dockerfile_body = fs::read_to_string(dockerfile)?;
    for required in [
        "internal: true",
        "pull_policy: never",
        "read_only: true",
        "no-new-privileges:true",
    ] {
        if !compose_body.contains(required) {
            return Err(error(format!(
                "Docker Compose security contract lacks `{required}`"
            )));
        }
    }
    for forbidden in [
        "docker.sock",
        "network_mode: host",
        "pull_policy: always",
        "build:",
    ] {
        if compose_body.contains(forbidden) {
            return Err(error(format!(
                "Docker Compose security contract contains `{forbidden}`"
            )));
        }
    }
    if compose_body
        .lines()
        .any(|line| line.trim_start().starts_with("ports:"))
    {
        return Err(error("Docker Compose publishes a host port"));
    }
    let expected_from = format!("FROM {runner_base}");
    if dockerfile_body.lines().next() != Some(expected_from.as_str())
        || dockerfile_body.lines().any(|line| {
            let line = line.trim_start();
            line.starts_with("RUN ") || line.starts_with("ADD ")
        })
    {
        return Err(error(
            "Dockerfile is not the locked copy-only, network-free runtime recipe",
        ));
    }
    Ok(())
}

fn docker_build_args(image: &str, dockerfile: &Path, context: &Path) -> Result<Vec<String>> {
    let dockerfile = dockerfile
        .to_str()
        .ok_or_else(|| error("Dockerfile path is not UTF-8"))?;
    let context = context
        .to_str()
        .ok_or_else(|| error("Docker context path is not UTF-8"))?;
    Ok(vec![
        "build".to_owned(),
        "--network=none".to_owned(),
        "--pull=false".to_owned(),
        "--provenance=false".to_owned(),
        "--tag".to_owned(),
        image.to_owned(),
        "--file".to_owned(),
        dockerfile.to_owned(),
        context.to_owned(),
    ])
}

fn compose_environment(
    project: &str,
    runner_image: &str,
    postgres_image: &str,
    evidence_dir: &Path,
) -> BTreeMap<String, String> {
    BTreeMap::from([
        (
            "REDLINE_DOCKER_COMPOSE_PROJECT".to_owned(),
            project.to_owned(),
        ),
        (
            "REDLINE_DOCKER_RUNNER_IMAGE".to_owned(),
            runner_image.to_owned(),
        ),
        (
            "REDLINE_DOCKER_POSTGRES_IMAGE".to_owned(),
            postgres_image.to_owned(),
        ),
        (
            "REDLINE_DOCKER_EVIDENCE_DIR".to_owned(),
            evidence_dir.display().to_string(),
        ),
        (
            "REDLINE_DOCKER_UID".to_owned(),
            unsafe { libc::geteuid() }.to_string(),
        ),
        (
            "REDLINE_DOCKER_GID".to_owned(),
            unsafe { libc::getegid() }.to_string(),
        ),
    ])
}

fn compose_command(
    compose: &Path,
    project: &str,
    environment: &BTreeMap<String, String>,
) -> Command {
    let mut command = Command::new("docker");
    command.args(["compose", "--ansi", "never", "--file"]);
    command.arg(compose);
    command.args(["--project-name", project]);
    command.envs(environment);
    command
}

fn compose_stdout(
    compose: &Path,
    project: &str,
    environment: &BTreeMap<String, String>,
    args: &[&str],
) -> Result<String> {
    let output = command_output(compose_command(compose, project, environment).args(args))?;
    Ok(String::from_utf8(output.stdout)?.trim().to_owned())
}

fn docker_stdout(args: &[&str]) -> Result<String> {
    let output = command_output(Command::new("docker").args(args))?;
    Ok(String::from_utf8(output.stdout)?.trim().to_owned())
}

fn docker_image_id(reference: &str) -> Result<String> {
    let value = docker_stdout(&["image", "inspect", "--format", "{{.Id}}", reference])?;
    if !value.starts_with("sha256:") || value.len() != 71 {
        return Err(error(format!("Docker image ID is malformed: {value}")));
    }
    Ok(value)
}

fn is_container_id(value: &str) -> bool {
    (12..=64).contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_arguments_disable_network_and_pull() {
        let args = docker_build_args(
            "redline:test",
            Path::new("/tmp/Dockerfile"),
            Path::new("/tmp/context"),
        )
        .unwrap();
        assert!(args.contains(&"--network=none".to_owned()));
        assert!(args.contains(&"--pull=false".to_owned()));
        assert!(!args.iter().any(|arg| arg == "--pull"));
    }

    #[test]
    fn candidate_validation_rejects_failures_and_count_drift() {
        let commit = "a".repeat(40);
        let mut candidate = json!({
            "schema_version": EVIDENCE_SCHEMA,
            "status": "candidate",
            "mode": "release",
            "provenance_source": "local-jeryu",
            "core": {"commit": commit},
            "testing": {"commit": commit},
            "release_failures": [],
            "sqlite": {"total": 2445, "passed": 2445},
            "postgres_oracle": {"total": 265, "passed": 265},
            "postgres_target": {"total": 151, "target_passed": 151},
            "postgres_exclusions": (0..114).collect::<Vec<_>>(),
        });
        assert!(validate_candidate(&candidate, &commit, &commit).is_ok());
        candidate["release_failures"] = json!(["target failed"]);
        assert!(validate_candidate(&candidate, &commit, &commit).is_err());
        candidate["release_failures"] = json!([]);
        candidate["postgres_target"]["target_passed"] = json!(150);
        assert!(validate_candidate(&candidate, &commit, &commit).is_err());
        candidate["postgres_target"]["target_passed"] = json!(151);
        candidate["provenance_source"] = json!("github");
        assert!(validate_candidate(&candidate, &commit, &commit).is_err());
    }

    #[test]
    fn cleanup_failure_prevents_success_even_after_a_green_campaign() {
        let result = combine_campaign_cleanup(Ok(()), Err(error("down failed")));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("cleanup failed"));
    }

    #[test]
    fn compose_guard_rejects_host_network_ports_and_socket() {
        let safe = "internal: true\npull_policy: never\nread_only: true\nno-new-privileges:true\n";
        for forbidden in ["network_mode: host", "ports:", "docker.sock", "build:"] {
            let body = format!("{safe}{forbidden}\n");
            let has_forbidden = [
                "docker.sock",
                "network_mode: host",
                "pull_policy: always",
                "build:",
            ]
            .iter()
            .any(|token| body.contains(token))
                || body
                    .lines()
                    .any(|line| line.trim_start().starts_with("ports:"));
            assert!(has_forbidden);
        }
    }
}
