use super::*;

const JOURNAL_SCHEMA: &str = "jain.release-candidate-journal/v1";
const PLAN_SCHEMA: &str = "jain.release-candidate-plan/v1";
const MAX_JOURNAL_BYTES: u64 = 8 * 1024 * 1024;
const MAX_TRANSITIONS_PER_REPOSITORY: u64 = 16;
#[cfg(test)]
const PLAN_KEYS: [&str; 12] = [
    "ci_jobs",
    "fleet_jobs",
    "formal_ga",
    "manifest",
    "manifest_sha256",
    "release",
    "release_status",
    "repositories",
    "rollback_target",
    "schema_version",
    "selected_repositories",
    "status",
];
#[cfg(test)]
const PLAN_ROW_REQUIRED_KEYS: [&str; 17] = [
    "binding_bound",
    "blocked_reasons",
    "family",
    "identity_status",
    "kind",
    "name",
    "path",
    "phase",
    "remote",
    "repo_slug",
    "required_check",
    "selected",
    "state",
    "status",
    "tag",
    "tag_prefix",
    "wave",
];
const JOURNAL_KEYS: [&str; 16] = [
    "ci_jobs",
    "created_at_unix",
    "fleet_jobs",
    "formal_ga",
    "generation",
    "journal_id",
    "lifecycle_status",
    "manifest",
    "manifest_sha256",
    "release",
    "release_status",
    "repositories",
    "rollback_target",
    "schema_version",
    "selected_repositories",
    "updated_at_unix",
];
const JOURNAL_ROW_KEYS: [&str; 20] = [
    "binding_bound",
    "name",
    "path",
    "pending_action",
    "phase",
    "pr_number",
    "release_checksum_sha256",
    "release_tree",
    "remote",
    "repo_slug",
    "required_check",
    "selected",
    "source_branch",
    "source_head",
    "source_tree",
    "state",
    "tag",
    "tag_prefix",
    "transition_count",
    "wave",
];

#[derive(Clone)]
struct AuthorityMeta {
    phase: i64,
    wave: i64,
    identity_status: String,
    tag_prefix: Option<String>,
    release_commit: Option<String>,
    release_tree: Option<String>,
    release_checksum: Option<String>,
}

pub(super) fn command(args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    reject_legacy_jeryu_environment()?;
    if env::var_os("ATOMICSOUL_PUSH").is_some_and(|value| value != "0") {
        return Err("release-candidate requires ATOMICSOUL_PUSH to be unset or exactly 0".into());
    }
    // Deployment entrypoints may export JAIN_RELEASE_VERSION.  Treat a
    // disagreement as a hard failure so a candidate cannot be journaled with
    // an image/installer version different from the manifest authority.
    if let Some(value) = env::var_os("JAIN_RELEASE_VERSION") {
        let value = value
            .into_string()
            .map_err(|_| "JAIN_RELEASE_VERSION must be valid UTF-8")?;
        if value != RELEASE_VERSION {
            return Err(format!(
                "JAIN_RELEASE_VERSION must match release-candidate authority {RELEASE_VERSION}"
            )
            .into());
        }
    }

    let root = control_plane_root();
    let mut manifest = root.join("repos.manifest.toml");
    let mut selected = Vec::new();
    let mut journal = None;
    let mut receipt = None;
    let mut token_file = None;
    let mut apply = false;
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--manifest" => manifest = PathBuf::from(iter.next().ok_or("--manifest needs a path")?),
            "--repo" => selected.push(iter.next().ok_or("--repo needs a name")?),
            "--journal" => {
                journal = Some(PathBuf::from(iter.next().ok_or("--journal needs a path")?))
            }
            "--receipt" => {
                receipt = Some(PathBuf::from(iter.next().ok_or("--receipt needs a path")?))
            }
            "--token-file" => {
                token_file = Some(PathBuf::from(
                    iter.next().ok_or("--token-file needs a path")?,
                ))
            }
            "--apply" => apply = true,
            value => return Err(format!("unknown release-candidate argument: {value}").into()),
        }
    }
    if !apply && (journal.is_some() || token_file.is_some()) {
        return Err("release-candidate dry-run does not accept --journal or --token-file".into());
    }
    if apply && receipt.is_some() {
        return Err(
            "release-candidate --apply journals every transition and does not accept --receipt"
                .into(),
        );
    }

    let manifest = fs::canonicalize(&manifest)?;
    physical_regular_file(&manifest, "release-candidate manifest")?;
    let data: toml::Value = fs::read_to_string(&manifest)?.parse()?;
    validate_manifest_data(&data, &manifest, false)?;
    validate_candidate_authority(&data)?;
    let plan = build_plan(&data, &manifest, &selected)?;

    if !apply {
        let result = require_plan_pass(&plan);
        let mut report = receipt_header(PLAN_SCHEMA, "release-candidate", false);
        report["plan"] = plan;
        return finish_optional_evidence(receipt.as_deref(), &mut report, result);
    }

    let journal_path = journal.ok_or("release-candidate --apply requires --journal")?;
    let token_file = token_file.ok_or("release-candidate --apply requires --token-file")?;
    drop(JeryuClient::from_token_file(&token_file)?);
    let _lock = lock_journal(&journal_path)?;
    let mut journal = if journal_path.exists() {
        read_journal(&journal_path)?
    } else {
        initialize_journal(&plan, &manifest)?
    };
    validate_journal(&mut journal, &data, &manifest, &selected)?;
    write_journal(&journal_path, &journal)?;
    if campaign_complete(&journal)? {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "schema_version": "jain.release-candidate-transition/v1",
                "status": "pass",
                "journal": journal_path,
                "generation": journal["generation"],
                "lifecycle_status": "complete",
            }))?
        );
        return Ok(());
    }

    let (index, action) = next_action(&journal)?;
    prepare_action(&mut journal, index, &action, &token_file)?;
    mark_pending(&mut journal, index, &action)?;
    write_journal(&journal_path, &journal)?;
    let result = execute_action(&mut journal, index, &action, &token_file, &journal_path);
    match result {
        Ok(()) => {
            complete_transition(&mut journal, index, &action)?;
            write_journal(&journal_path, &journal)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "schema_version": "jain.release-candidate-transition/v1",
                    "status": "pass",
                    "journal": journal_path,
                    "generation": journal["generation"],
                    "repository": journal["repositories"][index]["name"],
                    "action": action,
                    "lifecycle_status": journal["lifecycle_status"],
                }))?
            );
            Ok(())
        }
        Err(error) => {
            eprintln!("release-candidate stopped with pending action {action}: {error}");
            Err(error)
        }
    }
}

fn require_plan_pass(plan: &JsonValue) -> Result<(), Box<dyn std::error::Error>> {
    if plan["status"] == "pass" {
        Ok(())
    } else {
        Err("release-candidate plan is blocked".into())
    }
}

fn validate_candidate_authority(data: &toml::Value) -> Result<(), Box<dyn std::error::Error>> {
    if string(data, "release_version").as_deref() != Some(RELEASE_VERSION)
        || string(data, "status").as_deref() != Some(RELEASE_STATUS)
        || data.get("formal_ga").and_then(toml::Value::as_bool) != Some(false)
        || string(data, "rollback_target").as_deref() != Some(ROLLBACK_TARGET)
    {
        return Err("release-candidate authority must match the compiled candidate version/status, formal_ga=false, and rollback target".into());
    }
    Ok(())
}

fn authority_metadata(
    data: &toml::Value,
) -> Result<BTreeMap<String, AuthorityMeta>, Box<dyn std::error::Error>> {
    let mut result = BTreeMap::new();
    for raw in manifest_repos(data)? {
        let name = string(raw, "name").ok_or("managed repository is missing its name")?;
        let wave = raw
            .get("rollout_wave")
            .and_then(toml::Value::as_integer)
            .ok_or_else(|| format!("{name} is missing rollout_wave"))?;
        insert_authority_meta(&mut result, &name, raw, 1, wave)?;
    }

    if data.get("nested_families").is_some() {
        let topology = nested_engine_topology(data)?;
        let nested: toml::Value = fs::read_to_string(&topology.manifest_path)?.parse()?;
        let mut wave = 0_i64;
        for raw in nested
            .get("repo")
            .and_then(toml::Value::as_array)
            .into_iter()
            .flatten()
        {
            let name = string(raw, "name").ok_or("nested repository is missing its name")?;
            insert_authority_meta(&mut result, &name, raw, 0, wave)?;
            wave += 1;
        }
        for pending in &topology.pending_repositories {
            result.entry(pending.name.clone()).or_insert(AuthorityMeta {
                phase: 0,
                wave,
                identity_status: "pending".to_owned(),
                tag_prefix: None,
                release_commit: None,
                release_tree: None,
                release_checksum: None,
            });
            wave += 1;
        }
        let control = nested
            .get("control_plane")
            .ok_or("nested manifest is missing control_plane")?;
        let name = string(control, "name").ok_or("nested control plane is missing its name")?;
        insert_authority_meta(&mut result, &name, control, 0, wave)?;
    }

    for (family_name, family) in registered_nested_families(data) {
        let phase = family
            .get("release_phase")
            .and_then(toml::Value::as_integer)
            .ok_or_else(|| format!("nested_families.{family_name} is missing release_phase"))?;
        for raw in registered_nested_projection(family) {
            let name = string(raw, "name").ok_or_else(|| {
                format!("nested_families.{family_name} repository is missing name")
            })?;
            let wave = raw
                .get("rollout_wave")
                .and_then(toml::Value::as_integer)
                .ok_or_else(|| {
                    format!(
                        "nested_families.{family_name}.repository[{name}] is missing rollout_wave"
                    )
                })?;
            insert_authority_meta(&mut result, &name, raw, phase, wave)?;
        }
        let control_name = string(family, "control_plane_name").ok_or_else(|| {
            format!("nested_families.{family_name} is missing control_plane_name")
        })?;
        let control_wave = family
            .get("control_plane_rollout_wave")
            .and_then(toml::Value::as_integer)
            .ok_or_else(|| {
                format!("nested_families.{family_name} is missing control_plane_rollout_wave")
            })?;
        let control = toml::Value::Table(
            [
                (
                    "identity_status".to_owned(),
                    family
                        .get("control_plane_identity_status")
                        .cloned()
                        .ok_or_else(|| {
                            format!(
                                "nested_families.{family_name} is missing control-plane identity"
                            )
                        })?,
                ),
                (
                    "current_tag".to_owned(),
                    family
                        .get("control_plane_current_tag")
                        .cloned()
                        .ok_or_else(|| {
                            format!("nested_families.{family_name} is missing control-plane tag")
                        })?,
                ),
            ]
            .into_iter()
            .collect(),
        );
        insert_authority_meta(&mut result, &control_name, &control, phase, control_wave)?;
    }

    let control = data
        .get("control_plane")
        .ok_or("manifest is missing control_plane")?;
    let name = string(control, "name").ok_or("control plane is missing its name")?;
    insert_authority_meta(&mut result, &name, control, 2, 0)?;
    Ok(result)
}

fn insert_authority_meta(
    result: &mut BTreeMap<String, AuthorityMeta>,
    name: &str,
    raw: &toml::Value,
    phase: i64,
    wave: i64,
) -> Result<(), Box<dyn std::error::Error>> {
    let meta = AuthorityMeta {
        phase,
        wave,
        identity_status: string(raw, "identity_status").unwrap_or_else(|| "pending".to_owned()),
        tag_prefix: candidate_tag_prefix(name, raw, phase)?,
        release_commit: string(raw, "release_commit"),
        release_tree: string(raw, "release_tree"),
        release_checksum: string(raw, "release_checksum_sha256"),
    };
    if result.insert(name.to_owned(), meta).is_some() {
        return Err(format!("duplicate release-candidate authority row: {name}").into());
    }
    Ok(())
}

fn candidate_tag_prefix(
    name: &str,
    raw: &toml::Value,
    phase: i64,
) -> Result<Option<String>, Box<dyn std::error::Error>> {
    if let Some(current) = declared_release_tag(raw) {
        let (prefix, revision) = current
            .rsplit_once('.')
            .ok_or_else(|| format!("{name}: release tag has no numeric revision"))?;
        if revision.is_empty() || !revision.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(format!("{name}: release tag has no numeric revision").into());
        }
        return Ok(Some(format!("{prefix}.")));
    }
    if phase == 0 {
        return Ok(None);
    }
    Ok(Some(format!("{name}-v{RELEASE_VERSION}-split.")))
}

fn build_plan(
    data: &toml::Value,
    manifest: &Path,
    selected_names: &[String],
) -> Result<JsonValue, Box<dyn std::error::Error>> {
    let managed = managed_repositories(data, manifest)?;
    if managed.is_empty() || managed.len() > 64 {
        return Err("release-candidate managed repository count must be from 1 through 64".into());
    }
    let metadata = authority_metadata(data)?;
    let mut selected = BTreeSet::new();
    for name in selected_names {
        if !selected.insert(name.clone()) {
            return Err(format!("release-candidate received duplicate --repo {name}").into());
        }
    }
    let managed_names = managed
        .iter()
        .map(|repo| repo.name.clone())
        .collect::<BTreeSet<_>>();
    if let Some(unknown) = selected.iter().find(|name| !managed_names.contains(*name)) {
        return Err(format!("release-candidate repository is not managed: {unknown}").into());
    }
    let select_all = selected.is_empty();
    let mut rows = Vec::new();
    for repo in managed {
        let meta = metadata
            .get(&repo.name)
            .ok_or_else(|| format!("{} has no release ordering authority", repo.name))?;
        let selected_row = select_all || selected.contains(&repo.name);
        rows.push(inspect_repository(repo, meta, selected_row)?);
    }
    rows.sort_by(|left, right| {
        left["phase"]
            .as_i64()
            .unwrap_or(i64::MAX)
            .cmp(&right["phase"].as_i64().unwrap_or(i64::MAX))
            .then_with(|| {
                left["wave"]
                    .as_i64()
                    .unwrap_or(i64::MAX)
                    .cmp(&right["wave"].as_i64().unwrap_or(i64::MAX))
            })
            .then_with(|| {
                left["name"]
                    .as_str()
                    .unwrap_or_default()
                    .cmp(right["name"].as_str().unwrap_or_default())
            })
    });
    let selected_repositories = rows
        .iter()
        .filter(|row| row["selected"] == true)
        .map(|row| row["name"].clone())
        .collect::<Vec<_>>();
    for index in 0..rows.len() {
        if rows[index]["selected"] != true {
            continue;
        }
        let phase = rows[index]["phase"].as_i64().unwrap_or(i64::MAX);
        let wave = rows[index]["wave"].as_i64().unwrap_or(i64::MAX);
        let blocked = rows.iter().any(|prior| {
            let prior_key = (
                prior["phase"].as_i64().unwrap_or(i64::MAX),
                prior["wave"].as_i64().unwrap_or(i64::MAX),
            );
            prior_key < (phase, wave) && prior["selected"] != true && prior["state"] != "bound"
        });
        if blocked {
            block_row(
                &mut rows[index],
                "earlier manifest waves are not authority-bound",
            )?;
        }
    }
    let status = if rows
        .iter()
        .filter(|row| row["selected"] == true)
        .all(|row| row["status"] == "pass")
    {
        "pass"
    } else {
        "blocked"
    };
    Ok(json!({
        "schema_version": PLAN_SCHEMA,
        "release": RELEASE_VERSION,
        "release_status": RELEASE_STATUS,
        "formal_ga": false,
        "rollback_target": ROLLBACK_TARGET,
        "manifest": manifest,
        "manifest_sha256": manifest_sha256(manifest)?,
        "fleet_jobs": required_job_count(data, "fleet_jobs")?,
        "ci_jobs": required_job_count(data, "ci_jobs")?,
        "selected_repositories": selected_repositories,
        "repositories": rows,
        "status": status,
    }))
}

fn block_row(row: &mut JsonValue, reason: &str) -> Result<(), Box<dyn std::error::Error>> {
    row["status"] = json!("blocked");
    let reasons = row["blocked_reasons"]
        .as_array_mut()
        .ok_or("release-candidate row has invalid blocked_reasons")?;
    if !reasons.iter().any(|value| value.as_str() == Some(reason)) {
        reasons.push(json!(reason));
    }
    Ok(())
}

fn required_job_count(data: &toml::Value, key: &str) -> Result<i64, Box<dyn std::error::Error>> {
    data.get(key)
        .and_then(toml::Value::as_integer)
        .filter(|jobs| (1..=64).contains(jobs))
        .ok_or_else(|| {
            format!("release authority {key} must be an integer from 1 through 64").into()
        })
}

fn inspect_repository(
    repo: ManagedRepo,
    meta: &AuthorityMeta,
    selected: bool,
) -> Result<JsonValue, Box<dyn std::error::Error>> {
    let slug = fixed_jeryu_git_slug(&repo.remote)?;
    let mut row = json!({
        "name": repo.name,
        "repo_slug": slug,
        "path": repo.path,
        "remote": repo.remote,
        "required_check": repo.required_check,
        "kind": repo.kind,
        "family": repo.family,
        "phase": meta.phase,
        "wave": meta.wave,
        "identity_status": meta.identity_status,
        "tag": JsonValue::Null,
        "tag_prefix": meta.tag_prefix,
        "selected": selected,
        "status": "blocked",
        "blocked_reasons": [],
        "binding_bound": false,
        "state": "unavailable",
    });
    let result = inspect_repository_git(&repo, meta, &mut row);
    if let Err(error) = result {
        row["blocked_reasons"] = json!([error.to_string()]);
    } else {
        row["status"] = json!("pass");
    }
    Ok(row)
}

fn inspect_repository_git(
    repo: &ManagedRepo,
    meta: &AuthorityMeta,
    row: &mut JsonValue,
) -> Result<(), Box<dyn std::error::Error>> {
    if meta.tag_prefix.is_none() {
        return Err("repository has no manifest-authorized release tag series".into());
    }
    physical_directory(&repo.path, "release-candidate checkout")?;
    physical_directory(&repo.path.join(".git"), "release-candidate Git database")?;
    if repo.path.join(".git/worktrees").exists() {
        return Err("auxiliary Git worktree registration metadata is forbidden".into());
    }
    if secure_git_output(Some(&repo.path), &["remote"])? != "origin"
        || secure_git_output(Some(&repo.path), &["remote", "get-url", "--all", "origin"])?
            != repo.remote
    {
        return Err("checkout does not have the sole exact manifest origin".into());
    }
    let status = secure_git_output(
        Some(&repo.path),
        &["status", "--porcelain=v1", "--untracked-files=all"],
    )?;
    if !status.is_empty() {
        return Err("checkout is dirty".into());
    }
    let branch = secure_git_output(Some(&repo.path), &["branch", "--show-current"])?;
    let head = secure_git_output(Some(&repo.path), &["rev-parse", "HEAD^{commit}"])?;
    let tree = secure_git_output(Some(&repo.path), &["rev-parse", "HEAD^{tree}"])?;
    let main = secure_git_output(
        Some(&repo.path),
        &["rev-parse", "--verify", "refs/remotes/origin/main^{commit}"],
    )?;
    if !is_full_sha(&head) || !is_full_sha(&tree) || !is_full_sha(&main) {
        return Err("checkout contains a noncanonical Git identity".into());
    }
    let state = if branch == repo.branch {
        if head != main {
            return Err("main checkout does not equal the authenticated tracking main".into());
        }
        let tags = match meta.tag_prefix.as_deref() {
            Some(prefix) => local_candidate_tags_at_head(&repo.path, prefix, &head)?,
            None => Vec::new(),
        };
        if tags.len() > 1 {
            return Err("multiple release-series tags resolve to the protected main".into());
        }
        let discovered_tag = tags.first().cloned();
        let authority_tag_exact = repo.tag.as_deref() == discovered_tag.as_deref();
        let tree_exact = meta
            .release_tree
            .as_deref()
            .is_none_or(|release_tree| release_tree == tree);
        let actual_checksum = if discovered_tag.is_some() || meta.release_checksum.is_some() {
            Some(secure_git_archive_sha256(&repo.path, &head)?)
        } else {
            None
        };
        let checksum_exact = match (meta.release_checksum.as_deref(), actual_checksum.as_deref()) {
            (Some(expected), Some(actual)) if is_lower_hex(expected, 64) => actual == expected,
            _ => false,
        };
        let binding_exact = meta.release_commit.as_deref() == Some(head.as_str())
            && tree_exact
            && checksum_exact
            && authority_tag_exact;
        row["tag"] = json!(discovered_tag);
        row["release_tree"] = if row["tag"].is_string() {
            json!(tree)
        } else {
            JsonValue::Null
        };
        row["release_checksum_sha256"] = json!(actual_checksum);
        row["binding_bound"] = json!(binding_exact);
        if !binding_exact {
            return Err(
                "protected main lacks an exact release authority binding; refusing to infer prior PR, review, and check evidence"
                    .into(),
            );
        }
        "bound"
    } else {
        validate_release_branch(&branch)?;
        if head == main
            || !secure_git_status(
                Some(&repo.path),
                &["merge-base", "--is-ancestor", &main, &head],
            )?
        {
            return Err("release branch is not a strict descendant of origin/main".into());
        }
        if let Some(prefix) = meta.tag_prefix.as_deref() {
            if !local_candidate_tags_at_head(&repo.path, prefix, &head)?.is_empty() {
                return Err("a release-series tag resolves to a non-main source head".into());
            }
        }
        row["tag"] = JsonValue::Null;
        row["binding_bound"] = json!(false);
        "source-ready"
    };
    row["source_branch"] = json!(branch);
    row["source_head"] = json!(head);
    row["source_tree"] = json!(tree);
    row["protected_main"] = json!(main);
    row["state"] = json!(state);
    Ok(())
}

fn is_lower_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn validate_candidate_tag(tag: &str, prefix: &str) -> Result<u64, Box<dyn std::error::Error>> {
    let revision = tag
        .strip_prefix(prefix)
        .ok_or("candidate tag is outside its manifest-authorized series")?;
    if revision.is_empty()
        || !revision.bytes().all(|byte| byte.is_ascii_digit())
        || (revision.len() > 1 && revision.starts_with('0'))
    {
        return Err("candidate tag revision must be canonical unsigned decimal".into());
    }
    let revision = revision
        .parse::<u64>()
        .map_err(|_| "candidate tag revision exceeds its bound")?;
    let reference = format!("refs/tags/{tag}");
    secure_git_output(None, &["check-ref-format", &reference])?;
    Ok(revision)
}

fn next_candidate_tag(
    repo: &str,
    prefix: &str,
    token_file: &Path,
) -> Result<String, Box<dyn std::error::Error>> {
    validate_candidate_tag(&format!("{prefix}0"), prefix)?;
    let remote = fixed_jeryu_git_remote(repo)?;
    let pattern = format!("refs/tags/{prefix}*");
    let mut command = secure_git_authenticated_command(None, token_file)?;
    command
        .command
        .args(["ls-remote", "--refs", &remote, &pattern]);
    let output = bounded_command_output(&mut command.command, 1024 * 1024, "remote tag inventory")?;
    next_candidate_tag_from_inventory(prefix, &output)
}

fn next_candidate_tag_from_inventory(
    prefix: &str,
    output: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let mut revisions = BTreeSet::new();
    for (index, line) in output.lines().filter(|line| !line.is_empty()).enumerate() {
        if index >= 4096 {
            return Err("remote candidate tag inventory exceeds its row bound".into());
        }
        let (commit, reference) = line
            .split_once('\t')
            .ok_or("remote candidate tag inventory contains a malformed row")?;
        if !is_lower_hex(commit, 40) {
            return Err("remote candidate tag inventory contains a noncanonical commit".into());
        }
        let tag = reference
            .strip_prefix("refs/tags/")
            .ok_or("remote candidate tag inventory contains a non-tag ref")?;
        let revision = validate_candidate_tag(tag, prefix)?;
        if !revisions.insert(revision) {
            return Err("remote candidate tag inventory contains a duplicate revision".into());
        }
    }
    let revision = revisions.last().copied().map_or(Ok(0), |revision| {
        revision
            .checked_add(1)
            .ok_or("candidate tag revision overflow")
    })?;
    Ok(format!("{prefix}{revision}"))
}

fn local_candidate_tags_at_head(
    path: &Path,
    prefix: &str,
    head: &str,
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let pattern = format!("refs/tags/{prefix}*");
    let mut command = secure_git_command(Some(path));
    command.args([
        "for-each-ref",
        "--format=%(refname:strip=2) %(objectname)",
        &pattern,
    ]);
    let output = bounded_command_output(&mut command, 512 * 1024, "local tag inventory")?;
    let mut tags = Vec::new();
    for (index, line) in output.lines().filter(|line| !line.is_empty()).enumerate() {
        if index >= 4096 {
            return Err("local candidate tag inventory exceeds its row bound".into());
        }
        let (tag, commit) = line
            .split_once(' ')
            .ok_or("candidate tag inventory contains a malformed row")?;
        validate_candidate_tag(tag, prefix)?;
        if !is_lower_hex(commit, 40) {
            return Err("candidate tag inventory contains a noncanonical commit".into());
        }
        if commit == head {
            tags.push(tag.to_owned());
        }
    }
    tags.sort();
    Ok(tags)
}

fn bounded_command_output(
    command: &mut Command,
    max_bytes: u64,
    kind: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    let stdout = child.stdout.take().ok_or("bounded command has no stdout")?;
    let mut bytes = Vec::with_capacity(max_bytes.min(64 * 1024) as usize);
    if let Err(error) = stdout.take(max_bytes + 1).read_to_end(&mut bytes) {
        let _ = child.kill();
        let _ = child.wait();
        return Err(error.into());
    }
    if bytes.len() as u64 > max_bytes {
        let _ = child.kill();
        let _ = child.wait();
        return Err(format!("{kind} exceeds its byte bound").into());
    }
    if !child.wait()?.success() {
        return Err(format!("{kind} command failed").into());
    }
    Ok(String::from_utf8(bytes).map_err(|_| format!("{kind} is not UTF-8"))?)
}

fn secure_git_archive_sha256(
    path: &Path,
    commit: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let mut child = secure_git_command(Some(path))
        .args(["archive", "--format=tar", commit])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    let mut stdout = child.stdout.take().ok_or("Git archive has no stdout")?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = match stdout.read(&mut buffer) {
            Ok(read) => read,
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error.into());
            }
        };
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    if !child.wait()?.success() {
        return Err("streaming Git archive failed".into());
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn initialize_journal(
    plan: &JsonValue,
    manifest: &Path,
) -> Result<JsonValue, Box<dyn std::error::Error>> {
    if plan["status"] != "pass" {
        return Err(
            "release-candidate cannot initialize an apply journal from a blocked plan".into(),
        );
    }
    let now = unix_time();
    let repositories = plan["repositories"]
        .as_array()
        .ok_or("release-candidate plan has no repositories")?
        .iter()
        .map(|row| {
            let state = row["state"].as_str().unwrap_or("unavailable");
            json!({
                "name": row["name"],
                "repo_slug": row["repo_slug"],
                "path": row["path"],
                "remote": row["remote"],
                "required_check": row["required_check"],
                "phase": row["phase"],
                "wave": row["wave"],
                "selected": row["selected"],
                "source_branch": row.get("source_branch").cloned().unwrap_or(JsonValue::Null),
                "source_head": row.get("source_head").cloned().unwrap_or(JsonValue::Null),
                "source_tree": row.get("source_tree").cloned().unwrap_or(JsonValue::Null),
                "release_checksum_sha256": row.get("release_checksum_sha256").cloned().unwrap_or(JsonValue::Null),
                "release_tree": row.get("release_tree").cloned().unwrap_or(JsonValue::Null),
                "tag": row["tag"],
                "tag_prefix": row["tag_prefix"],
                "binding_bound": row["binding_bound"],
                "state": state,
                "pending_action": JsonValue::Null,
                "pr_number": JsonValue::Null,
                "transition_count": 0,
            })
        })
        .collect::<Vec<_>>();
    let lifecycle_status = lifecycle_status_for_rows(&repositories);
    let journal_id = journal_identity(&plan["manifest_sha256"], &repositories)?;
    Ok(json!({
        "schema_version": JOURNAL_SCHEMA,
        "journal_id": journal_id,
        "release": RELEASE_VERSION,
        "release_status": RELEASE_STATUS,
        "formal_ga": false,
        "rollback_target": ROLLBACK_TARGET,
        "manifest": manifest,
        "manifest_sha256": plan["manifest_sha256"],
        "fleet_jobs": plan["fleet_jobs"],
        "ci_jobs": plan["ci_jobs"],
        "selected_repositories": plan["selected_repositories"],
        "generation": 0,
        "created_at_unix": now,
        "updated_at_unix": now,
        "lifecycle_status": lifecycle_status,
        "repositories": repositories,
    }))
}

fn lifecycle_status_for_rows(rows: &[JsonValue]) -> &'static str {
    let selected = rows
        .iter()
        .filter(|row| row["selected"] == true)
        .collect::<Vec<_>>();
    if !selected.is_empty() && selected.iter().all(|row| row["state"] == "bound") {
        "complete"
    } else if selected
        .iter()
        .any(|row| row["state"] == "binding-required")
    {
        "binding-required"
    } else {
        "active"
    }
}

fn journal_identity(
    manifest_sha: &JsonValue,
    repositories: &[JsonValue],
) -> Result<String, Box<dyn std::error::Error>> {
    let mut digest = Sha256::new();
    digest.update(
        manifest_sha
            .as_str()
            .ok_or("journal manifest digest is missing")?,
    );
    for row in repositories {
        for key in ["name", "source_branch", "source_head", "source_tree"] {
            digest.update(row[key].as_str().unwrap_or("<unavailable>"));
            digest.update([0]);
        }
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn validate_journal(
    journal: &mut JsonValue,
    data: &toml::Value,
    manifest: &Path,
    selected: &[String],
) -> Result<(), Box<dyn std::error::Error>> {
    require_exact_keys(journal, &JOURNAL_KEYS, "release-candidate journal")?;
    let current_manifest_sha = manifest_sha256(manifest)?;
    if journal["schema_version"] != JOURNAL_SCHEMA
        || journal["release"] != RELEASE_VERSION
        || journal["release_status"] != RELEASE_STATUS
        || journal["formal_ga"] != false
        || journal["rollback_target"] != ROLLBACK_TARGET
        || journal["manifest"] != json!(manifest)
        || journal["fleet_jobs"] != required_job_count(data, "fleet_jobs")?
        || journal["ci_jobs"] != required_job_count(data, "ci_jobs")?
    {
        return Err("release-candidate journal authority drift".into());
    }
    let expected_selected = if selected.is_empty() {
        managed_repositories(data, manifest)?
            .into_iter()
            .map(|repo| repo.name)
            .collect::<BTreeSet<_>>()
    } else {
        selected.iter().cloned().collect::<BTreeSet<_>>()
    };
    let actual_selected = journal["selected_repositories"]
        .as_array()
        .ok_or("journal selected_repositories is not an array")?
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or("journal selected repository is not a string")
        })
        .collect::<Result<BTreeSet<_>, _>>()?;
    if actual_selected != expected_selected {
        return Err("release-candidate selection differs from the resume journal".into());
    }
    let rows = journal["repositories"]
        .as_array()
        .ok_or("journal repositories is not an array")?;
    let expected_rows = managed_repositories(data, manifest)?;
    let metadata = authority_metadata(data)?;
    if rows.len() != expected_rows.len() {
        return Err("release-candidate journal repository count drift".into());
    }
    let mut expected_order = expected_rows
        .iter()
        .map(|repo| {
            let authority = metadata
                .get(&repo.name)
                .ok_or("managed repository has no ordering authority")?;
            Ok((authority.phase, authority.wave, repo.name.as_str()))
        })
        .collect::<Result<Vec<_>, Box<dyn std::error::Error>>>()?;
    expected_order.sort();
    if rows
        .iter()
        .map(|row| row["name"].as_str())
        .ne(expected_order.iter().map(|(_, _, name)| Some(*name)))
    {
        return Err("release-candidate journal repository order or uniqueness drift".into());
    }
    let mut externally_bound = BTreeSet::new();
    for row in rows {
        require_exact_keys(row, &JOURNAL_ROW_KEYS, "release-candidate journal row")?;
        let name = row["name"].as_str().ok_or("journal row has no name")?;
        let expected = expected_rows
            .iter()
            .find(|repo| repo.name == name)
            .ok_or("journal contains an unmanaged repository")?;
        if row["path"] != json!(expected.path)
            || row["remote"] != expected.remote
            || row["required_check"] != expected.required_check
            || row["repo_slug"] != fixed_jeryu_git_slug(&expected.remote)?
            || row["selected"] != actual_selected.contains(name)
        {
            return Err(format!("{name}: release-candidate journal static authority drift").into());
        }
        let authority = metadata
            .get(name)
            .ok_or("journal row has no ordering authority")?;
        if row["phase"] != authority.phase
            || row["wave"] != authority.wave
            || row["tag_prefix"] != json!(authority.tag_prefix)
        {
            return Err(format!("{name}: release ordering or tag-series authority drift").into());
        }
        validate_journal_tag(row, authority)?;
        validate_journal_release_binding(row)?;
        let expected_binding = row["source_head"].as_str().is_some_and(|head| {
            authority.release_commit.as_deref() == Some(head)
                && authority
                    .release_tree
                    .as_deref()
                    .is_none_or(|tree| row["source_tree"].as_str() == Some(tree))
                && expected.tag.as_deref() == row["tag"].as_str()
                && authority.release_checksum.as_deref().is_some_and(|digest| {
                    is_lower_hex(digest, 64)
                        && row["release_checksum_sha256"].as_str() == Some(digest)
                })
        });
        if row["state"] == "binding-required" && expected_binding {
            externally_bound.insert(name.to_owned());
        } else if row["binding_bound"] != expected_binding {
            return Err(format!("{name}: release binding authority drift").into());
        }
        match (row["source_head"].as_str(), row["source_tree"].as_str()) {
            (Some(head), Some(tree)) => {
                validate_journal_checkout(expected)?;
                if !is_full_sha(head)
                    || !is_full_sha(tree)
                    || secure_git_output(
                        Some(&expected.path),
                        &["rev-parse", &format!("{head}^{{tree}}")],
                    )? != tree
                {
                    return Err(format!(
                        "{name}: journaled source identity is unavailable or changed"
                    )
                    .into());
                }
            }
            (None, None) if row["selected"] == false && row["state"] == "unavailable" => {}
            _ => {
                return Err(
                    format!("{name}: selected journal row has no exact source identity").into(),
                )
            }
        }
        let transitions = row["transition_count"]
            .as_u64()
            .ok_or("journal transition count is invalid")?;
        if transitions > MAX_TRANSITIONS_PER_REPOSITORY
            || !valid_state(row["state"].as_str())
            || !valid_action(row["pending_action"].as_str())
        {
            return Err(format!("{name}: journal state or transition bound is invalid").into());
        }
    }
    if journal["journal_id"] != journal_identity(&journal["manifest_sha256"], rows)? {
        return Err("release-candidate journal identity mismatch".into());
    }
    if journal["manifest_sha256"] != current_manifest_sha && externally_bound.is_empty() {
        return Err("release-candidate journal authority drift".into());
    }
    if !externally_bound.is_empty() {
        let rows = journal["repositories"]
            .as_array_mut()
            .ok_or("journal repositories is not an array")?;
        for row in rows.iter_mut().filter(|row| {
            row["name"]
                .as_str()
                .is_some_and(|name| externally_bound.contains(name))
        }) {
            row["state"] = json!("bound");
            row["binding_bound"] = json!(true);
            row["pending_action"] = JsonValue::Null;
            row["transition_count"] = json!(row["transition_count"]
                .as_u64()
                .ok_or("journal transition count is invalid")?
                .checked_add(1)
                .ok_or("journal transition count overflow")?);
        }
        journal["manifest_sha256"] = json!(current_manifest_sha);
        journal["journal_id"] = json!(journal_identity(
            &journal["manifest_sha256"],
            journal["repositories"]
                .as_array()
                .ok_or("journal repositories is not an array")?,
        )?);
        journal["lifecycle_status"] = json!(lifecycle_status_for_rows(
            journal["repositories"]
                .as_array()
                .ok_or("journal repositories is not an array")?,
        ));
        bump_generation(journal)?;
    }
    Ok(())
}

fn validate_journal_checkout(expected: &ManagedRepo) -> Result<(), Box<dyn std::error::Error>> {
    physical_directory(&expected.path, "release-candidate checkout")?;
    physical_directory(
        &expected.path.join(".git"),
        "release-candidate Git database",
    )?;
    if expected.path.join(".git/worktrees").exists()
        || secure_git_output(Some(&expected.path), &["remote"])? != "origin"
        || secure_git_output(
            Some(&expected.path),
            &["remote", "get-url", "--all", "origin"],
        )? != expected.remote
        || !secure_git_output(
            Some(&expected.path),
            &["status", "--porcelain=v1", "--untracked-files=all"],
        )?
        .is_empty()
    {
        return Err(format!(
            "{}: canonical checkout origin, cleanliness, or worktree registration drift",
            expected.name
        )
        .into());
    }
    Ok(())
}

fn validate_journal_tag(
    row: &JsonValue,
    authority: &AuthorityMeta,
) -> Result<(), Box<dyn std::error::Error>> {
    let state = row["state"]
        .as_str()
        .ok_or("release-candidate journal row has no state")?;
    let tag = row["tag"].as_str();
    let tag_required = matches!(state, "tagged" | "binding-required" | "bound")
        || row["pending_action"] == "immutable-tag";
    if tag_required != tag.is_some() {
        return Err("release-candidate journal tag does not match its lifecycle state".into());
    }
    if let Some(tag) = tag {
        let prefix = authority
            .tag_prefix
            .as_deref()
            .ok_or("release-candidate row has no tag-series authority")?;
        validate_candidate_tag(tag, prefix)?;
    }
    if (state == "bound") != (row["binding_bound"] == true) {
        return Err("release-candidate bound state disagrees with its authority binding".into());
    }
    Ok(())
}

fn validate_journal_release_binding(row: &JsonValue) -> Result<(), Box<dyn std::error::Error>> {
    let state = row["state"]
        .as_str()
        .ok_or("release-candidate journal row has no state")?;
    let binding_ready = matches!(state, "tagged" | "binding-required" | "bound")
        || row["pending_action"] == "authority-binding";
    if binding_ready {
        if row["release_tree"].as_str() != row["source_tree"].as_str()
            || !row["release_checksum_sha256"]
                .as_str()
                .is_some_and(|digest| is_lower_hex(digest, 64))
        {
            return Err("release-candidate tag binding evidence is missing or invalid".into());
        }
    } else if !row["release_tree"].is_null() || !row["release_checksum_sha256"].is_null() {
        return Err("release-candidate pre-tag row contains premature binding evidence".into());
    }
    Ok(())
}

fn require_exact_keys(
    value: &JsonValue,
    expected: &[&str],
    kind: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let object = value
        .as_object()
        .ok_or_else(|| format!("{kind} is not an object"))?;
    if object.len() != expected.len() || expected.iter().any(|key| !object.contains_key(*key)) {
        return Err(format!("{kind} contains missing or unknown fields").into());
    }
    Ok(())
}

fn valid_state(state: Option<&str>) -> bool {
    state.is_some_and(|state| {
        matches!(
            state,
            "unavailable"
                | "source-ready"
                | "branch-published"
                | "pr-open"
                | "protected"
                | "checks-green"
                | "reviewed"
                | "merged"
                | "main-reconciled"
                | "tagged"
                | "binding-required"
                | "bound"
        )
    })
}

fn valid_action(action: Option<&str>) -> bool {
    action.is_none()
        || action.is_some_and(|action| {
            matches!(
                action,
                "branch-push"
                    | "pr-open"
                    | "protection-apply"
                    | "checks-readback"
                    | "approval-readback"
                    | "pr-merge"
                    | "main-reconcile"
                    | "immutable-tag"
                    | "authority-binding"
            )
        })
}

fn next_action(journal: &JsonValue) -> Result<(usize, String), Box<dyn std::error::Error>> {
    let rows = journal["repositories"]
        .as_array()
        .ok_or("journal repositories is not an array")?;
    for (index, row) in rows
        .iter()
        .enumerate()
        .filter(|(_, row)| row["selected"] == true)
    {
        let state = row["state"].as_str().ok_or("journal row has no state")?;
        if state == "bound" {
            continue;
        }
        let phase = row["phase"].as_i64().ok_or("journal phase is invalid")?;
        let wave = row["wave"].as_i64().ok_or("journal wave is invalid")?;
        if rows.iter().any(|prior| {
            let prior_key = (
                prior["phase"].as_i64().unwrap_or(i64::MAX),
                prior["wave"].as_i64().unwrap_or(i64::MAX),
            );
            prior_key < (phase, wave) && prior["state"] != "bound"
        }) {
            return Err(format!(
                "{} is blocked until every earlier manifest wave is authority-bound",
                row["name"].as_str().unwrap_or("repository")
            )
            .into());
        }
        if state == "binding-required" {
            return Err(format!(
                "{} requires a separate protected authority binding PR before resume",
                row["name"].as_str().unwrap_or("repository")
            )
            .into());
        }
        let action = row["pending_action"]
            .as_str()
            .map(str::to_owned)
            .unwrap_or_else(|| {
                match state {
                    "source-ready" => "branch-push",
                    "branch-published" => "pr-open",
                    "pr-open" => "protection-apply",
                    "protected" => "checks-readback",
                    "checks-green" => "approval-readback",
                    "reviewed" => "pr-merge",
                    "merged" => "main-reconcile",
                    "main-reconciled" => "immutable-tag",
                    "tagged" => "authority-binding",
                    _ => "invalid",
                }
                .to_owned()
            });
        if !valid_action(Some(&action)) {
            return Err("release-candidate journal cannot derive a valid next action".into());
        }
        return Ok((index, action));
    }
    Err("release-candidate has no remaining selected source lifecycle action".into())
}

fn campaign_complete(journal: &JsonValue) -> Result<bool, Box<dyn std::error::Error>> {
    let rows = journal["repositories"]
        .as_array()
        .ok_or("journal repositories is not an array")?;
    let selected = rows
        .iter()
        .filter(|row| row["selected"] == true)
        .collect::<Vec<_>>();
    let all_bound = !selected.is_empty() && selected.iter().all(|row| row["state"] == "bound");
    if (journal["lifecycle_status"] == "complete") != all_bound {
        return Err("release-candidate lifecycle status disagrees with selected rows".into());
    }
    Ok(all_bound)
}

fn prepare_action(
    journal: &mut JsonValue,
    index: usize,
    action: &str,
    token_file: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    if action != "immutable-tag" || journal["repositories"][index]["tag"].is_string() {
        return Ok(());
    }
    let prefix = journal["repositories"][index]["tag_prefix"]
        .as_str()
        .ok_or("repository has no manifest-authorized release tag series")?;
    let repo = journal["repositories"][index]["repo_slug"]
        .as_str()
        .ok_or("journal row has no repository slug")?;
    let tag = next_candidate_tag(repo, prefix, token_file)?;
    journal["repositories"][index]["tag"] = json!(tag);
    bump_generation(journal)
}

fn mark_pending(
    journal: &mut JsonValue,
    index: usize,
    action: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if !valid_action(Some(action)) {
        return Err("invalid release-candidate action".into());
    }
    if journal["repositories"][index]["pending_action"].is_null() {
        journal["repositories"][index]["pending_action"] = json!(action);
    } else if journal["repositories"][index]["pending_action"] != action {
        return Err("release-candidate pending action drift".into());
    }
    bump_generation(journal)
}

fn complete_transition(
    journal: &mut JsonValue,
    index: usize,
    action: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let next = match action {
        "branch-push" => "branch-published",
        "pr-open" => "pr-open",
        "protection-apply" => "protected",
        "checks-readback" => "checks-green",
        "approval-readback" => "reviewed",
        "pr-merge" => "merged",
        "main-reconcile" => "main-reconciled",
        "immutable-tag" => "tagged",
        "authority-binding" => "binding-required",
        _ => return Err("unknown release-candidate transition".into()),
    };
    let transitions = journal["repositories"][index]["transition_count"]
        .as_u64()
        .unwrap_or(0)
        + 1;
    if transitions > MAX_TRANSITIONS_PER_REPOSITORY {
        return Err("release-candidate transition bound exceeded".into());
    }
    journal["repositories"][index]["state"] = json!(next);
    journal["repositories"][index]["pending_action"] = JsonValue::Null;
    journal["repositories"][index]["transition_count"] = json!(transitions);
    journal["lifecycle_status"] = json!(lifecycle_status_for_rows(
        journal["repositories"]
            .as_array()
            .ok_or("journal repositories is not an array")?,
    ));
    bump_generation(journal)
}

fn bump_generation(journal: &mut JsonValue) -> Result<(), Box<dyn std::error::Error>> {
    let generation = journal["generation"]
        .as_u64()
        .ok_or("journal generation is invalid")?;
    journal["generation"] = json!(generation
        .checked_add(1)
        .ok_or("journal generation overflow")?);
    journal["updated_at_unix"] = json!(unix_time());
    Ok(())
}

fn execute_action(
    journal: &mut JsonValue,
    index: usize,
    action: &str,
    token_file: &Path,
    journal_path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let row = journal["repositories"][index].clone();
    let text = |key: &str| -> Result<String, Box<dyn std::error::Error>> {
        row[key]
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| format!("journal row is missing {key}").into())
    };
    let repo = text("repo_slug")?;
    let path = PathBuf::from(text("path")?);
    let branch = text("source_branch")?;
    let head = text("source_head")?;
    let required_check = text("required_check")?;
    let client = JeryuClient::from_token_file(token_file)?;
    match action {
        "branch-push" => jeryu_branch_push(vec![
            "branch-push".to_owned(),
            "--repo".to_owned(),
            repo,
            "--repo-path".to_owned(),
            path.to_string_lossy().into_owned(),
            "--branch".to_owned(),
            branch,
            "--expected-head".to_owned(),
            head,
            "--token-file".to_owned(),
            token_file.to_string_lossy().into_owned(),
            "--apply".to_owned(),
        ]),
        "pr-open" => {
            let number = find_or_open_pr(
                &client,
                &repo,
                &branch,
                &head,
                journal["release"]
                    .as_str()
                    .ok_or("journal has no release version")?,
            )?;
            journal["repositories"][index]["pr_number"] = json!(number);
            Ok(())
        }
        "protection-apply" => {
            let response = client.execute(&JeryuRequest::protection(
                &repo,
                "main",
                Some(immutable_main_policy(&required_check)),
            )?)?;
            let readback = client.execute(&JeryuRequest::protection(&repo, "main", None)?)?;
            validate_protection_policy(&readback, &repo, "main", &required_check)?;
            if response.is_null() {
                return Err("Jeryu protection apply returned no response".into());
            }
            Ok(())
        }
        "checks-readback" => {
            let number = pr_number(&row)?;
            let details = client.execute(&JeryuRequest::pr_details(&repo, number)?)?;
            validate_pr_open_readback(&details, number, &branch, &head, "main")?;
            let protection = client.execute(&JeryuRequest::protection(&repo, "main", None)?)?;
            validate_protection_policy(&protection, &repo, "main", &required_check)?;
            let response = client.execute(&JeryuRequest::checks(&repo, &head)?)?;
            require_green_checks(&response, &head, &required_check)
        }
        "approval-readback" => {
            let number = pr_number(&row)?;
            let details = client.execute(&JeryuRequest::pr_details(&repo, number)?)?;
            validate_pr_open_readback(&details, number, &branch, &head, "main")?;
            let response = client.execute(&JeryuRequest::pr_readback(&repo, number)?)?;
            validate_approval_readback(&response, &head)
        }
        "pr-merge" => merge_or_verify(
            &client,
            &repo,
            pr_number(&row)?,
            &branch,
            &head,
            &required_check,
            token_file,
        ),
        "main-reconcile" => reconcile_main(&path, &text("remote")?, &head, token_file),
        "immutable-tag" => {
            let tag = row["tag"]
                .as_str()
                .ok_or("authority manifest has no immutable tag for the merged repository")?;
            let receipt =
                journal_path.with_extension(format!("tag-{}.json", receipt_component(tag)));
            immutable_tag_command(vec![
                "--repo".to_owned(),
                path.to_string_lossy().into_owned(),
                "--remote".to_owned(),
                text("remote")?,
                "--tag".to_owned(),
                tag.to_owned(),
                "--commit".to_owned(),
                head.clone(),
                "--token-file".to_owned(),
                token_file.to_string_lossy().into_owned(),
                "--receipt".to_owned(),
                receipt.to_string_lossy().into_owned(),
                "--apply".to_owned(),
            ])?;
            journal["repositories"][index]["release_tree"] = json!(text("source_tree")?);
            journal["repositories"][index]["release_checksum_sha256"] =
                json!(secure_git_archive_sha256(&path, &head)?);
            Ok(())
        }
        "authority-binding" => {
            let tag = row["tag"]
                .as_str()
                .ok_or("journal has no immutable tag for authority binding")?;
            let remote = text("remote")?;
            let source_tree = text("source_tree")?;
            let archive_checksum = secure_git_archive_sha256(&path, &head)?;
            if secure_local_ref_commit(&path, &format!("refs/tags/{tag}"))?.as_deref()
                != Some(head.as_str())
                || secure_ls_remote_at(&remote, &format!("refs/tags/{tag}"), token_file)?.as_deref()
                    != Some(head.as_str())
                || secure_ls_remote(&repo, "refs/heads/main", token_file)?.as_deref()
                    != Some(head.as_str())
                || row["release_tree"].as_str() != Some(source_tree.as_str())
                || row["release_checksum_sha256"].as_str() != Some(archive_checksum.as_str())
            {
                return Err("immutable tag binding evidence no longer matches exact main".into());
            }
            Ok(())
        }
        _ => Err("unsupported release-candidate action".into()),
    }
}

fn find_or_open_pr(
    client: &JeryuClient,
    repo: &str,
    branch: &str,
    head: &str,
    release: &str,
) -> Result<u64, Box<dyn std::error::Error>> {
    let list = client.execute(&JeryuRequest::pr_list(repo, "open")?)?;
    let rows = list
        .as_array()
        .ok_or("Jeryu open PR list is not an array")?;
    if rows.len() > 128 {
        return Err("Jeryu open PR list exceeds the release-candidate bound".into());
    }
    let same_branch = rows
        .iter()
        .filter(|row| row.pointer("/head/ref").and_then(JsonValue::as_str) == Some(branch))
        .collect::<Vec<_>>();
    if same_branch
        .iter()
        .any(|row| row.pointer("/head/sha").and_then(JsonValue::as_str) != Some(head))
    {
        return Err("an open PR for the journaled branch advertises a different head".into());
    }
    let exact = same_branch
        .into_iter()
        .filter(|row| {
            row.pointer("/head/sha").and_then(JsonValue::as_str) == Some(head)
                && row.pointer("/base/ref").and_then(JsonValue::as_str) == Some("main")
                && row.get("state").and_then(JsonValue::as_str) == Some("open")
        })
        .collect::<Vec<_>>();
    if exact.len() > 1 {
        return Err("multiple open PRs match the exact release-candidate head".into());
    }
    if let Some(row) = exact.first() {
        return row
            .get("number")
            .and_then(JsonValue::as_u64)
            .ok_or_else(|| "matching PR has no number".into());
    }
    let title = format!(
        "release({}): candidate {}",
        repo.rsplit('/').next().unwrap_or(repo),
        release
    );
    let body = "Manifest-bound release-candidate source lifecycle. Candidate only: no GA metadata, registry publication, installation, or traffic routing.";
    let response = client.execute(&JeryuRequest::pr_open(
        repo, &title, branch, head, "main", body, false, "codex",
    )?)?;
    let number = response
        .get("number")
        .and_then(JsonValue::as_u64)
        .ok_or("Jeryu PR-open response has no PR number")?;
    let readback = client.execute(&JeryuRequest::pr_details(repo, number)?)?;
    validate_pr_open_readback(&readback, number, branch, head, "main")?;
    Ok(number)
}

fn require_green_checks(
    response: &JsonValue,
    head: &str,
    required_check: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let runs = response
        .get("check_runs")
        .and_then(JsonValue::as_array)
        .ok_or("Jeryu checks response has no check_runs array")?;
    if runs.len() > 128
        || response.get("total_count").and_then(JsonValue::as_u64) != Some(runs.len() as u64)
    {
        return Err("Jeryu checks response count is missing, inconsistent, or unbounded".into());
    }
    for required in [required_check, "jankurai/proof"] {
        let matching = runs
            .iter()
            .filter(|run| {
                run.get("name").and_then(JsonValue::as_str) == Some(required)
                    && run.get("head_sha").and_then(JsonValue::as_str) == Some(head)
            })
            .collect::<Vec<_>>();
        let mut ids = BTreeSet::new();
        let parsed = matching
            .iter()
            .map(|run| {
                let id = run
                    .get("id")
                    .and_then(JsonValue::as_str)
                    .filter(|value| !value.is_empty() && value.len() <= 128)
                    .ok_or("check run has no bounded ID")?;
                if !ids.insert(id) {
                    return Err("check run history contains a duplicate ID");
                }
                let started = run
                    .get("started_at")
                    .and_then(JsonValue::as_str)
                    .and_then(parse_utc_timestamp)
                    .ok_or("check run has no canonical UTC start time")?;
                Ok((started, *run))
            })
            .collect::<Result<Vec<_>, &str>>();
        let Ok(parsed) = parsed else {
            return Err(format!("exact-head {required} check history is incomplete").into());
        };
        let Some(latest_time) = parsed.iter().map(|(started, _)| started).max() else {
            return Err(format!("exact-head {required} check history is incomplete").into());
        };
        let latest = parsed
            .iter()
            .filter(|(started, _)| started == latest_time)
            .collect::<Vec<_>>();
        if latest.len() != 1 {
            return Err(format!("latest exact-head {required} check is ambiguous").into());
        }
        let run = latest[0].1;
        let green = run.get("status").and_then(JsonValue::as_str) == Some("completed")
            && run.get("conclusion").and_then(JsonValue::as_str) == Some("success");
        if !green {
            return Err(format!("latest exact-head {required} check is not successful").into());
        }
    }
    Ok(())
}

fn parse_utc_timestamp(value: &str) -> Option<(u16, u8, u8, u8, u8, u8, u32)> {
    if value.len() < 20 || value.len() > 30 || !value.ends_with('Z') {
        return None;
    }
    let body = value.strip_suffix('Z')?;
    let (base, fraction) = body
        .split_once('.')
        .map_or((body, None), |(base, fraction)| (base, Some(fraction)));
    if base.len() != 19
        || base.as_bytes()[4] != b'-'
        || base.as_bytes()[7] != b'-'
        || base.as_bytes()[10] != b'T'
        || base.as_bytes()[13] != b':'
        || base.as_bytes()[16] != b':'
    {
        return None;
    }
    let year = base.get(0..4)?.parse::<u16>().ok()?;
    let component = |start: usize, end: usize| base.get(start..end)?.parse::<u8>().ok();
    let month = component(5, 7)?;
    let day = component(8, 10)?;
    let hour = component(11, 13)?;
    let minute = component(14, 16)?;
    let second = component(17, 19)?;
    let leap_year =
        year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400));
    let month_days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap_year => 29,
        2 => 28,
        _ => return None,
    };
    if year == 0 || day == 0 || day > month_days || hour > 23 || minute > 59 || second > 59 {
        return None;
    }
    let nanos = match fraction {
        None => 0,
        Some(fraction)
            if !fraction.is_empty()
                && fraction.len() <= 9
                && fraction.bytes().all(|byte| byte.is_ascii_digit()) =>
        {
            fraction
                .parse::<u32>()
                .ok()?
                .checked_mul(10_u32.pow(9 - fraction.len() as u32))?
        }
        Some(_) => return None,
    };
    Some((year, month, day, hour, minute, second, nanos))
}

fn validate_pr_merged_readback(
    response: &JsonValue,
    number: u64,
    branch: &str,
    head: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let valid = response.get("number").and_then(JsonValue::as_u64) == Some(number)
        && response.get("state").and_then(JsonValue::as_str) == Some("closed")
        && response.get("merged").and_then(JsonValue::as_bool) == Some(true)
        && response.get("mergeable_state").and_then(JsonValue::as_str) == Some("merged")
        && response.get("merge_commit_sha").and_then(JsonValue::as_str) == Some(head)
        && response.pointer("/head/ref").and_then(JsonValue::as_str) == Some(branch)
        && response.pointer("/head/sha").and_then(JsonValue::as_str) == Some(head)
        && response.pointer("/base/ref").and_then(JsonValue::as_str) == Some("main");
    if valid {
        Ok(())
    } else {
        Err("Jeryu merged PR readback does not prove the exact head and base".into())
    }
}

fn pr_number(row: &JsonValue) -> Result<u64, Box<dyn std::error::Error>> {
    row["pr_number"]
        .as_u64()
        .filter(|number| *number > 0)
        .ok_or_else(|| "release-candidate journal has no PR number".into())
}

fn merge_or_verify(
    client: &JeryuClient,
    repo: &str,
    number: u64,
    branch: &str,
    head: &str,
    required_check: &str,
    token_file: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let main = secure_ls_remote(repo, "refs/heads/main", token_file)?;
    let details = client.execute(&JeryuRequest::pr_details(repo, number)?)?;
    let approval = client.execute(&JeryuRequest::pr_readback(repo, number)?)?;
    validate_approval_readback(&approval, head)?;
    let checks = client.execute(&JeryuRequest::checks(repo, head)?)?;
    require_green_checks(&checks, head, required_check)?;
    let protection = client.execute(&JeryuRequest::protection(repo, "main", None)?)?;
    validate_protection_policy(&protection, repo, "main", required_check)?;
    if main.as_deref() == Some(head) {
        return validate_pr_merged_readback(&details, number, branch, head);
    }
    validate_pr_open_readback(&details, number, branch, head, "main")?;
    let response = client.execute(&JeryuRequest::pr_merge(repo, number, head)?)?;
    if response.get("merged").and_then(JsonValue::as_bool) != Some(true)
        || response.get("sha").and_then(JsonValue::as_str) != Some(head)
        || secure_ls_remote(repo, "refs/heads/main", token_file)?.as_deref() != Some(head)
    {
        return Err("release-candidate merge did not read back the exact protected main".into());
    }
    let merged = client.execute(&JeryuRequest::pr_details(repo, number)?)?;
    validate_pr_merged_readback(&merged, number, branch, head)
}

fn reconcile_main(
    path: &Path,
    remote: &str,
    head: &str,
    token_file: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut report = receipt_header(
        "jain.release-candidate-main-reconcile/v1",
        "release-candidate main-reconcile",
        true,
    );
    fetch_authenticated_main(path, remote, token_file, Some(head), true, &mut report)?;
    if !secure_git_output(
        Some(path),
        &["status", "--porcelain=v1", "--untracked-files=all"],
    )?
    .is_empty()
    {
        return Err("main reconciliation requires a clean checkout".into());
    }
    if secure_git_output(Some(path), &["branch", "--show-current"])? != "main"
        && !secure_git_status(Some(path), &["switch", "main"])?
    {
        return Err("cannot switch the canonical checkout to main".into());
    }
    if !secure_git_status(
        Some(path),
        &["merge", "--ff-only", "refs/remotes/origin/main"],
    )? || secure_git_output(Some(path), &["rev-parse", "HEAD^{commit}"])? != head
        || secure_git_output(Some(path), &["branch", "--show-current"])? != "main"
        || !secure_git_output(
            Some(path),
            &["status", "--porcelain=v1", "--untracked-files=all"],
        )?
        .is_empty()
    {
        return Err(
            "canonical main reconciliation did not produce the clean exact merged head".into(),
        );
    }
    Ok(())
}

fn lock_journal(path: &Path) -> Result<fs::File, Box<dyn std::error::Error>> {
    let parent = validate_journal_parent(path)?;
    let lock_path = path.with_extension("lock");
    if lock_path == path || lock_path.parent() != Some(parent) {
        return Err("release-candidate lock path is ambiguous".into());
    }
    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(0o400000)
        .open(&lock_path)?;
    let metadata = file.metadata()?;
    if !metadata.file_type().is_file()
        || metadata.nlink() != 1
        || metadata.uid() != unsafe { libc::geteuid() }
        || metadata.mode() & 0o7777 != 0o600
    {
        return Err("release-candidate lock inode is unsafe".into());
    }
    if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
        return Err(format!(
            "release-candidate journal is already locked: {}",
            io::Error::last_os_error()
        )
        .into());
    }
    Ok(file)
}

fn validate_journal_parent(path: &Path) -> Result<&Path, Box<dyn std::error::Error>> {
    if !path.is_absolute() || path.file_name().is_none() {
        return Err("release-candidate journal path must be an absolute file path".into());
    }
    let parent = path
        .parent()
        .ok_or("release-candidate journal has no parent")?;
    if fs::canonicalize(parent)? != parent {
        return Err("release-candidate journal parent must be an exact physical path".into());
    }
    let metadata = physical_directory(parent, "release-candidate journal parent")?;
    if metadata.uid() != unsafe { libc::geteuid() } || metadata.mode() & 0o022 != 0 {
        return Err("release-candidate journal parent must be current-user-owned and not group/world-writable".into());
    }
    Ok(parent)
}

fn read_journal(path: &Path) -> Result<JsonValue, Box<dyn std::error::Error>> {
    if !path.is_absolute() {
        return Err("release-candidate journal path must be absolute".into());
    }
    let file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(0o400000)
        .open(path)?;
    let metadata = file.metadata()?;
    if !metadata.file_type().is_file()
        || metadata.nlink() != 1
        || metadata.uid() != unsafe { libc::geteuid() }
        || metadata.mode() & 0o7777 != 0o600
        || metadata.len() > MAX_JOURNAL_BYTES
    {
        return Err(
            "release-candidate journal inode, owner, mode, link count, or size is unsafe".into(),
        );
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.take(MAX_JOURNAL_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 != metadata.len() || bytes.len() as u64 > MAX_JOURNAL_BYTES {
        return Err(
            "release-candidate journal changed or exceeded its byte limit while reading".into(),
        );
    }
    Ok(serde_json::from_slice(&bytes)?)
}

fn write_journal(path: &Path, journal: &JsonValue) -> Result<(), Box<dyn std::error::Error>> {
    let parent = validate_journal_parent(path)?;
    match fs::symlink_metadata(path) {
        Ok(metadata)
            if metadata.file_type().is_file()
                && metadata.uid() == unsafe { libc::geteuid() }
                && metadata.mode() & 0o7777 == 0o600
                && metadata.nlink() == 1 => {}
        Ok(_) => return Err("existing release-candidate journal inode is unsafe".into()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    let mut bytes = serde_json::to_vec_pretty(journal)?;
    bytes.push(b'\n');
    if bytes.len() as u64 > MAX_JOURNAL_BYTES {
        return Err("release-candidate journal exceeds its byte limit".into());
    }
    let name = path
        .file_name()
        .and_then(OsStr::to_str)
        .ok_or("journal filename is not UTF-8")?;
    let stage = parent.join(format!(
        ".{name}.stage-{}-{}",
        std::process::id(),
        unix_nanos()
    ));
    let mut output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(0o400000)
        .open(&stage)?;
    let write_result = (|| -> Result<(), Box<dyn std::error::Error>> {
        output.write_all(&bytes)?;
        output.sync_all()?;
        let metadata = output.metadata()?;
        if !metadata.file_type().is_file()
            || metadata.nlink() != 1
            || metadata.mode() & 0o7777 != 0o600
            || metadata.uid() != unsafe { libc::geteuid() }
            || metadata.len() != bytes.len() as u64
        {
            return Err("release-candidate staged journal inode is unsafe".into());
        }
        fs::rename(&stage, path)?;
        fs::File::open(parent)?.sync_all()?;
        Ok(())
    })();
    if write_result.is_err() {
        let _ = fs::remove_file(&stage);
    }
    write_result
}

fn unix_time() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn unix_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let path = env::temp_dir().join(format!(
                "jain-split-ops-release-candidate-test-{}-{}",
                std::process::id(),
                unix_nanos()
            ));
            fs::create_dir_all(&path).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    fn row(name: &str, phase: i64, wave: i64, state: &str, selected: bool) -> JsonValue {
        json!({
            "name": name, "phase": phase, "wave": wave, "state": state,
            "selected": selected, "pending_action": JsonValue::Null,
        })
    }

    #[test]
    fn wave_order_and_resume_are_fail_closed() {
        let mut journal = json!({
            "repositories": [
                row("redline", 0, 0, "binding-required", false),
                row("jain-domain", 1, 1, "source-ready", true),
            ]
        });
        assert!(next_action(&journal)
            .unwrap_err()
            .to_string()
            .contains("earlier manifest wave"));
        journal["repositories"][0]["state"] = json!("bound");
        assert_eq!(
            next_action(&journal).unwrap(),
            (1, "branch-push".to_owned())
        );
        journal["repositories"][1]["pending_action"] = json!("pr-open");
        assert_eq!(next_action(&journal).unwrap(), (1, "pr-open".to_owned()));
    }

    #[test]
    fn completed_campaign_returns_success_without_deriving_another_action() {
        let manifest = control_plane_root().join("repos.manifest.toml");
        let plan = json!({
            "status": "pass",
            "manifest_sha256": "a".repeat(64),
            "fleet_jobs": 4,
            "ci_jobs": 4,
            "selected_repositories": ["jain-domain"],
            "repositories": [{
                "name":"jain-domain", "repo_slug":"veox/jain-domain", "path":"/jain-domain",
                "remote":"http://127.0.0.1:8787/git/veox/jain-domain.git",
                "required_check":"jain-domain/required", "phase":1, "wave":1,
                "selected":true, "source_branch":"release/jain-domain",
                "source_head":"b".repeat(40), "source_tree":"c".repeat(40),
                "release_checksum_sha256":"d".repeat(64), "release_tree":"c".repeat(40),
                "tag":format!("jain-domain-v{RELEASE_VERSION}-split.1"),
                "tag_prefix":format!("jain-domain-v{RELEASE_VERSION}-split."),
                "binding_bound":true, "state":"bound"
            }]
        });
        let journal = initialize_journal(&plan, &manifest).unwrap();
        assert_eq!(journal["lifecycle_status"], "complete");
        assert!(campaign_complete(&journal).unwrap());
        assert!(next_action(&journal).is_err());

        let inconsistent = json!({
            "lifecycle_status": "complete",
            "repositories": [row("jain-domain", 1, 1, "source-ready", true)],
        });
        assert!(campaign_complete(&inconsistent).is_err());
    }

    #[test]
    fn blocked_plans_fail_and_keep_every_blocking_reason() {
        assert!(require_plan_pass(&json!({"status": "pass"})).is_ok());
        assert!(require_plan_pass(&json!({"status": "blocked"})).is_err());

        let mut row = json!({
            "status": "blocked",
            "blocked_reasons": ["repository has no manifest-authorized release tag series"]
        });
        block_row(&mut row, "earlier manifest waves are not authority-bound").unwrap();
        assert_eq!(
            row["blocked_reasons"],
            json!([
                "repository has no manifest-authorized release tag series",
                "earlier manifest waves are not authority-bound"
            ])
        );
        block_row(&mut row, "earlier manifest waves are not authority-bound").unwrap();
        assert_eq!(row["blocked_reasons"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn checks_require_exact_latest_required_and_jankurai_success() {
        let head = "a".repeat(40);
        let response = json!({"total_count": 2, "check_runs": [
            {"id":"required-1","name":"repo/required","head_sha":head,"status":"completed","conclusion":"success","started_at":"2026-01-01T00:00:00Z","completed_at":"2026-01-01T00:00:01Z"},
            {"id":"proof-1","name":"jankurai/proof","head_sha":head,"status":"completed","conclusion":"success","started_at":"2026-01-01T00:00:00Z","completed_at":"2026-01-01T00:00:01Z"}
        ]});
        assert!(require_green_checks(&response, &head, "repo/required").is_ok());
        let mut failed = response.clone();
        failed["check_runs"].as_array_mut().unwrap().push(json!({
            "id":"proof-2","name":"jankurai/proof","head_sha":head,"status":"completed","conclusion":"failure",
            "started_at":"2026-01-02T00:00:00Z","completed_at":"2026-01-02T00:00:01Z"
        }));
        failed["total_count"] = json!(3);
        assert!(require_green_checks(&failed, &head, "repo/required").is_err());
        let mut running = response.clone();
        running["check_runs"].as_array_mut().unwrap().push(json!({
            "id":"required-2","name":"repo/required","head_sha":head,"status":"in_progress","conclusion":null,
            "started_at":"2026-01-03T00:00:00Z","completed_at":null
        }));
        running["total_count"] = json!(3);
        assert!(require_green_checks(&running, &head, "repo/required").is_err());

        let mut ambiguous = response;
        ambiguous["check_runs"].as_array_mut().unwrap().push(json!({
            "id":"required-tie","name":"repo/required","head_sha":head,"status":"completed","conclusion":"success",
            "started_at":"2026-01-01T00:00:00Z","completed_at":"2026-01-01T00:00:02Z"
        }));
        ambiguous["total_count"] = json!(3);
        assert!(require_green_checks(&ambiguous, &head, "repo/required").is_err());
    }

    #[test]
    fn check_timestamps_and_merged_pr_readback_are_canonical() {
        assert!(
            parse_utc_timestamp("2026-01-01T00:00:00.1Z")
                > parse_utc_timestamp("2026-01-01T00:00:00Z")
        );
        assert!(
            parse_utc_timestamp("2026-01-01T00:00:00.9Z")
                > parse_utc_timestamp("2026-01-01T00:00:00.10Z")
        );
        assert!(parse_utc_timestamp("2026-01-01T00:00:00+00:00").is_none());
        assert!(parse_utc_timestamp("2026-13-01T00:00:00Z").is_none());
        assert!(parse_utc_timestamp("2026-02-29T00:00:00Z").is_none());
        assert!(parse_utc_timestamp("2028-02-29T00:00:00Z").is_some());

        let head = "a".repeat(40);
        let merged = json!({
            "number": 7, "state": "closed", "merged": true,
            "mergeable_state": "merged", "merge_commit_sha": head,
            "head": {"ref": "release/example", "sha": head},
            "base": {"ref": "main"}
        });
        assert!(validate_pr_merged_readback(&merged, 7, "release/example", &head).is_ok());
        let mut wrong = merged;
        wrong["merge_commit_sha"] = json!("b".repeat(40));
        assert!(validate_pr_merged_readback(&wrong, 7, "release/example", &head).is_err());
    }

    #[test]
    fn journal_shape_rejects_unknown_fields_and_invalid_states() {
        let value = json!({"one": 1, "two": 2});
        assert!(require_exact_keys(&value, &["one", "two"], "test").is_ok());
        assert!(require_exact_keys(&value, &["one"], "test").is_err());
        assert!(valid_state(Some("checks-green")));
        assert!(!valid_state(Some("published")));
        assert!(valid_action(None));
        assert!(!valid_action(Some("route-production")));
    }

    #[test]
    fn journal_initialization_retains_unavailable_unselected_rows() {
        let manifest = control_plane_root().join("repos.manifest.toml");
        let plan = json!({
            "status": "pass",
            "manifest_sha256": "a".repeat(64),
            "fleet_jobs": 4,
            "ci_jobs": 4,
            "selected_repositories": ["selected"],
            "repositories": [
                {
                    "name":"unavailable", "repo_slug":"veox/unavailable", "path":"/unavailable",
                    "remote":"http://127.0.0.1:8787/git/veox/unavailable.git",
                    "required_check":"unavailable/required", "phase":0, "wave":0,
                    "selected":false, "tag":null, "tag_prefix":null,
                    "binding_bound":false, "state":"unavailable"
                },
                {
                    "name":"selected", "repo_slug":"veox/selected", "path":"/selected",
                    "remote":"http://127.0.0.1:8787/git/veox/selected.git",
                    "required_check":"selected/required", "phase":1, "wave":0,
                    "selected":true, "source_branch":"release/selected", "source_head":"b".repeat(40),
                    "source_tree":"c".repeat(40), "tag":null,
                    "tag_prefix":format!("selected-v{RELEASE_VERSION}-split."), "binding_bound":false,
                    "state":"source-ready"
                }
            ]
        });
        let journal = initialize_journal(&plan, &manifest).unwrap();
        assert_eq!(journal["repositories"][0]["state"], "unavailable");
        assert_eq!(journal["repositories"][0]["binding_bound"], false);
        assert!(journal["repositories"][0]["source_head"].is_null());
        assert_eq!(journal["repositories"][1]["state"], "source-ready");
    }

    #[test]
    fn candidate_tag_inventory_is_strict_and_monotonic() {
        let prefix = "example-v9.0.0-split.";
        let inventory = format!(
            "{}\trefs/tags/{prefix}0\n{}\trefs/tags/{prefix}2",
            "a".repeat(40),
            "b".repeat(40)
        );
        assert_eq!(
            next_candidate_tag_from_inventory(prefix, &inventory).unwrap(),
            format!("{prefix}3")
        );
        assert!(next_candidate_tag_from_inventory(
            prefix,
            &format!("{}\trefs/tags/{prefix}02", "a".repeat(40))
        )
        .is_err());
        assert!(next_candidate_tag_from_inventory(
            prefix,
            &format!("{}\trefs/heads/main", "a".repeat(40))
        )
        .is_err());
    }

    #[test]
    fn command_output_enforces_its_byte_bound() {
        let mut exact = Command::new("/usr/bin/printf");
        exact.arg("1234");
        assert_eq!(
            bounded_command_output(&mut exact, 4, "test output").unwrap(),
            "1234"
        );

        let mut oversized = Command::new("/usr/bin/printf");
        oversized.arg("1234");
        assert!(bounded_command_output(&mut oversized, 3, "test output")
            .unwrap_err()
            .to_string()
            .contains("byte bound"));
    }

    #[test]
    fn tag_series_requires_explicit_nested_product_authority() {
        let empty = toml::Value::Table(toml::map::Map::new());
        assert_eq!(
            candidate_tag_prefix("redline-new", &empty, 0).unwrap(),
            None
        );
        assert_eq!(
            candidate_tag_prefix("jain-new", &empty, 1).unwrap(),
            Some(format!("jain-new-v{RELEASE_VERSION}-split."))
        );
        let declared: toml::Value = "current_tag = \"redline-v4.1.0-jain.7\"".parse().unwrap();
        assert_eq!(
            candidate_tag_prefix("redline", &declared, 0).unwrap(),
            Some("redline-v4.1.0-jain.".to_owned())
        );
    }

    #[test]
    fn parent_authority_orders_registered_nested_family_before_jain_products() {
        let data: toml::Value = fs::read_to_string(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("repos.manifest.toml"),
        )
        .unwrap()
        .parse()
        .unwrap();
        let metadata = authority_metadata(&data).unwrap();
        assert_eq!((metadata["jeryu"].phase, metadata["jeryu"].wave), (0, 16));
        assert_eq!(
            (
                metadata["jeryu-tool-finder"].phase,
                metadata["jeryu-tool-finder"].wave
            ),
            (0, 24)
        );
        assert_eq!(
            (
                metadata["jeryu-release-ops"].phase,
                metadata["jeryu-release-ops"].wave
            ),
            (0, 26)
        );
        assert_eq!(
            (metadata["jain-domain"].phase, metadata["jain-domain"].wave),
            (1, 1)
        );
        assert_eq!(
            (
                metadata["jain-split-ops"].phase,
                metadata["jain-split-ops"].wave
            ),
            (2, 0)
        );
    }

    #[test]
    fn journal_io_is_atomic_private_and_rejects_aliases_and_lock_races() {
        let directory = TestDirectory::new();
        let path = directory.0.join("campaign.json");
        let value = json!({"schema_version": JOURNAL_SCHEMA, "generation": 7});
        write_journal(&path, &value).unwrap();
        let metadata = fs::metadata(&path).unwrap();
        assert_eq!(metadata.mode() & 0o7777, 0o600);
        assert_eq!(metadata.nlink(), 1);
        assert_eq!(read_journal(&path).unwrap(), value);

        let first_lock = lock_journal(&path).unwrap();
        assert!(lock_journal(&path).is_err());
        drop(first_lock);

        let alias = directory.0.join("alias.json");
        fs::hard_link(&path, &alias).unwrap();
        assert!(read_journal(&path).is_err());
    }

    #[test]
    fn published_contract_matches_the_runtime_journal_shape() {
        let schema: JsonValue = serde_json::from_str(include_str!(
            "../../../contracts/release-candidate.schema.json"
        ))
        .unwrap();
        assert_eq!(
            schema.pointer("/$defs/plan/properties/schema_version/const"),
            Some(&json!(PLAN_SCHEMA))
        );
        assert_eq!(
            schema.pointer("/$defs/journal/properties/schema_version/const"),
            Some(&json!(JOURNAL_SCHEMA))
        );
        let required = |pointer: &str| {
            schema
                .pointer(pointer)
                .unwrap()
                .as_array()
                .unwrap()
                .iter()
                .map(|value| value.as_str().unwrap())
                .collect::<BTreeSet<_>>()
        };
        assert_eq!(
            required("/$defs/plan/required"),
            PLAN_KEYS.into_iter().collect()
        );
        assert_eq!(
            required("/$defs/planRepository/required"),
            PLAN_ROW_REQUIRED_KEYS.into_iter().collect()
        );
        for definition in ["plan", "planRepository", "journal", "journalRepository"] {
            assert_eq!(
                schema.pointer(&format!("/$defs/{definition}/additionalProperties")),
                Some(&json!(false))
            );
        }
        assert_eq!(
            required("/$defs/journal/required"),
            JOURNAL_KEYS.into_iter().collect()
        );
        assert_eq!(
            required("/$defs/journalRepository/required"),
            JOURNAL_ROW_KEYS.into_iter().collect()
        );
    }
}
