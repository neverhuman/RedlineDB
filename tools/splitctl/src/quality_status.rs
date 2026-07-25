use crate::jeryu_client::{JeryuClient, JeryuRequest};
use serde_json::{json, Value as JsonValue};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    thread,
};

const DEFAULT_JOBS: usize = 4;
const MAX_JOBS: usize = 32;
const MAX_POLICY_BYTES: u64 = 1024 * 1024;

#[derive(Clone, Debug)]
struct RepoSpec {
    name: String,
    path: PathBuf,
    forge_repo: String,
    required_check: String,
    family: String,
    kind: String,
    phase: i64,
    wave: i64,
}

#[derive(Clone, Debug)]
struct Policy {
    minimum_score: f64,
    floor_enforced: bool,
    baseline_score: Option<f64>,
}

#[derive(Clone, Debug)]
struct RepoContext {
    spec: RepoSpec,
    head: String,
    policy: Policy,
}

pub(crate) fn command(args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    let mut manifest = None;
    let mut token_file = None;
    let mut selected = Vec::new();
    let mut jobs = DEFAULT_JOBS;
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--manifest" => {
                manifest = Some(PathBuf::from(iter.next().ok_or("--manifest needs a path")?))
            }
            "--token-file" => {
                token_file = Some(PathBuf::from(
                    iter.next().ok_or("--token-file needs a path")?,
                ))
            }
            "--repo" => selected.push(iter.next().ok_or("--repo needs a name")?),
            "--jobs" => {
                jobs = iter
                    .next()
                    .ok_or("--jobs needs a value")?
                    .parse::<usize>()?;
                if !(1..=MAX_JOBS).contains(&jobs) {
                    return Err(format!("--jobs must be between 1 and {MAX_JOBS}").into());
                }
            }
            value => return Err(format!("unknown quality-status argument: {value}").into()),
        }
    }
    let manifest = manifest.ok_or("quality-status requires --manifest")?;
    let token_file = token_file.ok_or("quality-status requires --token-file")?;
    let manifest_bytes = fs::read(&manifest)?;
    let manifest_data: toml::Value = std::str::from_utf8(&manifest_bytes)?.parse()?;
    let managed = super::managed_repositories(&manifest_data, &manifest)?;
    let mut specs = managed
        .iter()
        .enumerate()
        .map(|(index, repo)| {
            let (phase, wave) = manifest_order(&manifest_data, &repo.name, &repo.family, index);
            Ok(RepoSpec {
                name: repo.name.clone(),
                path: repo.path.clone(),
                forge_repo: forge_slug(&repo.remote)?,
                required_check: repo.required_check.clone(),
                family: repo.family.clone(),
                kind: repo.kind.clone(),
                phase,
                wave,
            })
        })
        .collect::<Result<Vec<_>, Box<dyn std::error::Error>>>()?;

    let selected_set = selected.iter().cloned().collect::<BTreeSet<_>>();
    if selected_set.len() != selected.len() {
        return Err("quality-status rejects duplicate --repo selections".into());
    }
    if !selected_set.is_empty() {
        let known = specs
            .iter()
            .map(|repo| repo.name.clone())
            .collect::<BTreeSet<_>>();
        let unknown = selected_set.difference(&known).cloned().collect::<Vec<_>>();
        if !unknown.is_empty() {
            return Err(format!("unknown managed repositories: {}", unknown.join(", ")).into());
        }
        specs.retain(|repo| selected_set.contains(&repo.name));
    }
    specs.sort_by(|left, right| {
        (left.phase, left.wave, &left.name).cmp(&(right.phase, right.wave, &right.name))
    });
    let mut redline_members = specs
        .iter_mut()
        .filter(|repo| repo.family == "redline-split" && repo.kind == "nested-family")
        .collect::<Vec<_>>();
    redline_members.sort_by(|left, right| left.name.cmp(&right.name));
    for (index, repo) in redline_members.into_iter().enumerate() {
        repo.wave = index as i64 + 1;
    }
    for repo in &mut specs {
        if repo.family == "redline-split" && repo.kind == "nested-control-plane" {
            repo.wave = 15;
        }
    }
    specs.sort_by(|left, right| {
        (left.phase, left.wave, &left.name).cmp(&(right.phase, right.wave, &right.name))
    });

    let queue = Arc::new(Mutex::new(
        specs
            .into_iter()
            .enumerate()
            .collect::<VecDeque<(usize, RepoSpec)>>(),
    ));
    let results = Arc::new(Mutex::new(Vec::new()));
    thread::scope(|scope| {
        for _ in 0..jobs {
            let queue = Arc::clone(&queue);
            let results = Arc::clone(&results);
            let token_file = &token_file;
            scope.spawn(move || {
                let mut client = None;
                loop {
                    let Some((index, spec)) = queue.lock().unwrap().pop_front() else {
                        break;
                    };
                    let result = scan_repo(spec, token_file, &mut client);
                    results.lock().unwrap().push((index, result));
                }
            });
        }
    });
    let mut results = Arc::try_unwrap(results)
        .map_err(|_| "quality-status result workers did not release state")?
        .into_inner()
        .map_err(|_| "quality-status result state was poisoned")?;
    results.sort_by_key(|(index, _)| *index);
    let repositories = results
        .into_iter()
        .map(|(_, result)| result)
        .collect::<Vec<_>>();
    let report = build_report(
        &manifest,
        &format!("{:x}", Sha256::digest(&manifest_bytes)),
        jobs,
        repositories,
    );
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

fn scan_repo(spec: RepoSpec, token_file: &Path, client: &mut Option<JeryuClient>) -> JsonValue {
    let head = match local_head(&spec.path) {
        Ok(head) => head,
        Err(error) => return result(&spec, None, None, "blocked", vec![error], None),
    };
    match local_dirty(&spec.path) {
        Ok(false) => {}
        Ok(true) => {
            return result(
                &spec,
                Some(&head),
                None,
                "blocked",
                vec!["checkout is dirty at the scanned exact HEAD".to_owned()],
                None,
            )
        }
        Err(error) => return result(&spec, Some(&head), None, "blocked", vec![error], None),
    }
    let policy = match load_policy(&spec.path) {
        Ok(policy) => policy,
        Err(error) => return result(&spec, Some(&head), None, "blocked", vec![error], None),
    };
    let context = RepoContext {
        spec: spec.clone(),
        head: head.clone(),
        policy: policy.clone(),
    };
    if client.is_none() {
        match JeryuClient::from_token_file(token_file) {
            Ok(created) => *client = Some(created),
            Err(error) => {
                return result(
                    &spec,
                    Some(&head),
                    Some(&policy),
                    "blocked",
                    vec![format!("authenticated forge checks unavailable: {error}")],
                    None,
                )
            }
        }
    }
    let request = match JeryuRequest::checks(&spec.forge_repo, &head) {
        Ok(request) => request,
        Err(error) => {
            return result(
                &spec,
                Some(&head),
                Some(&policy),
                "blocked",
                vec![format!(
                    "cannot construct exact-head check request: {error}"
                )],
                None,
            )
        }
    };
    match client.as_ref().unwrap().execute(&request) {
        Ok(response) => classify_checks(&context, &response),
        Err(error) => classify_forge_error(&context, &error.to_string()),
    }
}

fn local_head(path: &Path) -> Result<String, String> {
    let output =
        super::secure_git_output_bytes_bounded(Some(path), &["rev-parse", "--verify", "HEAD"], 128)
            .map_err(|error| format!("cannot resolve local HEAD: {error}"))?;
    let head = std::str::from_utf8(&output)
        .map_err(|_| "local HEAD is not UTF-8".to_owned())?
        .trim();
    if !lower_hex(head, 40) {
        return Err("local HEAD is not a full lowercase Git SHA".to_owned());
    }
    Ok(head.to_owned())
}

fn local_dirty(path: &Path) -> Result<bool, String> {
    super::secure_git_output_bytes_bounded(
        Some(path),
        &["status", "--porcelain=v1", "--untracked-files=all"],
        1024 * 1024,
    )
    .map(|output| !output.is_empty())
    .map_err(|error| format!("cannot inspect checkout dirtiness: {error}"))
}

fn load_policy(root: &Path) -> Result<Policy, String> {
    let policy_path = root.join("agent/audit-policy.toml");
    let policy_bytes = bounded_regular(&policy_path, MAX_POLICY_BYTES)
        .map_err(|error| format!("governed audit policy unavailable: {error}"))?;
    let policy: toml::Value = std::str::from_utf8(&policy_bytes)
        .map_err(|_| "governed audit policy is not UTF-8".to_owned())?
        .parse()
        .map_err(|error| format!("governed audit policy is malformed: {error}"))?;
    let minimum_score = numeric(policy.get("minimum_score"))
        .ok_or_else(|| "governed audit policy has no numeric minimum_score".to_owned())?;
    let floor_enforced = policy
        .get("floor_enforced")
        .map(|value| {
            value
                .as_bool()
                .ok_or_else(|| "floor_enforced must be boolean".to_owned())
        })
        .transpose()?
        .unwrap_or(true);
    if policy
        .get("hard_findings_allowed")
        .is_some_and(|value| value.as_integer() != Some(0))
    {
        return Err("governed audit policy must allow zero hard findings".to_owned());
    }

    let baseline_path = root.join("agent/jankurai-baseline.json");
    let baseline_score = if baseline_path.exists() || baseline_path.is_symlink() {
        let bytes = bounded_regular(&baseline_path, MAX_POLICY_BYTES)
            .map_err(|error| format!("governed Jankurai baseline unavailable: {error}"))?;
        let baseline: JsonValue = serde_json::from_slice(&bytes)
            .map_err(|error| format!("governed Jankurai baseline is malformed: {error}"))?;
        let hard = baseline.get("hard_findings").or_else(|| {
            baseline
                .get("decision")
                .and_then(|value| value.get("hard_findings"))
        });
        let caps = baseline
            .get("caps")
            .or_else(|| baseline.get("caps_applied"));
        let zero_hard = hard.is_some_and(zero_count);
        let zero_caps = caps.is_some_and(zero_count);
        if !zero_hard || !zero_caps {
            return Err("governed Jankurai baseline has an unsafe contract".to_owned());
        }
        Some(
            json_number(baseline.get("score"))
                .ok_or_else(|| "governed Jankurai baseline has no numeric score".to_owned())?,
        )
    } else {
        None
    };
    if !floor_enforced && baseline_score.is_none() {
        return Err("repository waives its absolute floor without a governed baseline".to_owned());
    }
    Ok(Policy {
        minimum_score,
        floor_enforced,
        baseline_score,
    })
}

fn classify_checks(context: &RepoContext, response: &JsonValue) -> JsonValue {
    let Some(check_runs) = response.get("check_runs").and_then(JsonValue::as_array) else {
        return assessed(
            context,
            "unproven",
            vec!["forge response has no check_runs array".to_owned()],
            None,
        );
    };
    if response
        .get("total_count")
        .and_then(JsonValue::as_u64)
        .is_some_and(|count| count as usize != check_runs.len())
    {
        return assessed(
            context,
            "blocked",
            vec!["forge check count disagrees with the returned inventory".to_owned()],
            None,
        );
    }
    let mut ids = BTreeSet::new();
    for run in check_runs {
        let Some(id) = run.get("id").and_then(JsonValue::as_str) else {
            return assessed(
                context,
                "blocked",
                vec!["forge check run is missing its ID".to_owned()],
                None,
            );
        };
        if !ids.insert(id) {
            return assessed(
                context,
                "blocked",
                vec![format!("forge check inventory contains duplicate ID {id}")],
                None,
            );
        }
    }

    let required = match unique_latest(check_runs, &context.spec.required_check, &context.head) {
        Ok(Some(run)) => run,
        Ok(None) => {
            return assessed(
                context,
                "unproven",
                vec![format!(
                    "missing exact-head {} check",
                    context.spec.required_check
                )],
                None,
            )
        }
        Err(error) => return assessed(context, "blocked", vec![error], None),
    };
    let proof = match unique_latest(check_runs, "jankurai/proof", &context.head) {
        Ok(Some(run)) => run,
        Ok(None) => {
            return assessed(
                context,
                "unproven",
                vec!["missing exact-head jankurai/proof check".to_owned()],
                None,
            )
        }
        Err(error) => return assessed(context, "blocked", vec![error], None),
    };
    let selected = json!({
        "required": selected_check(required),
        "proof": selected_check(proof),
    });
    let failed = [
        (context.spec.required_check.as_str(), required),
        ("jankurai/proof", proof),
    ]
    .into_iter()
    .filter_map(|(name, run)| {
        let status = run.get("status").and_then(JsonValue::as_str);
        let conclusion = run.get("conclusion").and_then(JsonValue::as_str);
        (status != Some("completed") || conclusion != Some("success")).then(|| {
            format!(
                "latest {name} result is status={} conclusion={}",
                status.unwrap_or("<missing>"),
                conclusion.unwrap_or("<missing>")
            )
        })
    })
    .collect::<Vec<_>>();
    if !failed.is_empty() {
        return assessed(context, "red", failed, Some(selected));
    }

    let required_summary = summary(required);
    let proof_summary = summary(proof);
    let (Some(required_summary), Some(proof_summary)) = (required_summary, proof_summary) else {
        return assessed(
            context,
            "unproven",
            vec!["latest successful checks lack proof summaries".to_owned()],
            Some(selected),
        );
    };
    if required_summary != proof_summary {
        return assessed(
            context,
            "unproven",
            vec!["required and proof summaries do not match exactly".to_owned()],
            Some(selected),
        );
    }
    let markers = match summary_markers(proof_summary) {
        Ok(markers) => markers,
        Err(error) => {
            return assessed(context, "unproven", vec![error], Some(selected));
        }
    };
    for digest in ["receipt_sha256", "auditor_sha256"] {
        if !markers
            .get(digest)
            .is_some_and(|value| lower_hex(value, 64) && value.bytes().any(|byte| byte != b'0'))
        {
            return assessed(
                context,
                "unproven",
                vec![format!("proof summary has an invalid {digest}")],
                Some(selected),
            );
        }
    }
    if !markers.get("attempt_id").is_some_and(|value| {
        !value.is_empty()
            && value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    }) {
        return assessed(
            context,
            "unproven",
            vec!["proof summary has an invalid attempt_id".to_owned()],
            Some(selected),
        );
    }
    let score = match markers
        .get("score")
        .and_then(|value| value.parse::<f64>().ok())
        .filter(|value| value.is_finite() && (0.0..=100.0).contains(value))
    {
        Some(score) => score,
        None => {
            return assessed(
                context,
                "unproven",
                vec!["proof summary has an invalid score".to_owned()],
                Some(selected),
            )
        }
    };
    let hard = marker_u64(&markers, "hard_findings");
    let caps = marker_u64(&markers, "caps_applied");
    let (Some(hard), Some(caps)) = (hard, caps) else {
        return assessed(
            context,
            "unproven",
            vec!["proof summary has invalid hard_findings or caps_applied".to_owned()],
            Some(selected),
        );
    };

    let mut red = Vec::new();
    if markers.get("proof_status").map(String::as_str) != Some("pass") {
        red.push("proof_status is not pass".to_owned());
    }
    if context.policy.floor_enforced && score < context.policy.minimum_score {
        red.push(format!(
            "score {score} is below governed floor {}",
            context.policy.minimum_score
        ));
    }
    if context
        .policy
        .baseline_score
        .is_some_and(|baseline| score < baseline)
    {
        red.push(format!(
            "score {score} regresses governed baseline {}",
            context.policy.baseline_score.unwrap()
        ));
    }
    if hard != 0 {
        red.push(format!("hard_findings={hard}"));
    }
    if caps != 0 {
        red.push(format!("caps_applied={caps}"));
    }
    let evidence = json!({
        "selected_checks": selected,
        "score": score,
        "hard_findings": hard,
        "caps_applied": caps,
        "proof_status": markers.get("proof_status"),
        "attempt_id": markers.get("attempt_id"),
        "receipt_sha256": markers.get("receipt_sha256"),
        "auditor_sha256": markers.get("auditor_sha256"),
    });
    if red.is_empty() {
        assessed(context, "green", Vec::new(), Some(evidence))
    } else {
        assessed(context, "red", red, Some(evidence))
    }
}

fn unique_latest<'a>(
    check_runs: &'a [JsonValue],
    name: &str,
    head: &str,
) -> Result<Option<&'a JsonValue>, String> {
    let mut candidates = Vec::new();
    for run in check_runs
        .iter()
        .filter(|run| run.get("name").and_then(JsonValue::as_str) == Some(name))
    {
        if run.get("head_sha").and_then(JsonValue::as_str) != Some(head) {
            return Err(format!("{name} check does not bind the scanned exact HEAD"));
        }
        let timestamp = run
            .get("completed_at")
            .and_then(JsonValue::as_str)
            .or_else(|| run.get("started_at").and_then(JsonValue::as_str))
            .ok_or_else(|| format!("{name} check has no selection timestamp"))?;
        candidates.push((parse_utc_timestamp(timestamp)?, run));
    }
    let Some(latest) = candidates
        .iter()
        .map(|(timestamp, _)| timestamp.clone())
        .max()
    else {
        return Ok(None);
    };
    let latest_runs = candidates
        .iter()
        .filter(|(timestamp, _)| *timestamp == latest)
        .map(|(_, run)| *run)
        .collect::<Vec<_>>();
    if latest_runs.len() != 1 {
        return Err(format!(
            "{name} has {} runs tied for the latest timestamp",
            latest_runs.len()
        ));
    }
    Ok(latest_runs.into_iter().next())
}

fn parse_utc_timestamp(value: &str) -> Result<(String, u32), String> {
    let Some(body) = value.strip_suffix('Z') else {
        return Err(format!("check timestamp is not UTC: {value}"));
    };
    let (whole, fraction) = body.split_once('.').unwrap_or((body, ""));
    let bytes = whole.as_bytes();
    if bytes.len() != 19
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || bytes[10] != b'T'
        || bytes[13] != b':'
        || bytes[16] != b':'
        || bytes
            .iter()
            .enumerate()
            .any(|(index, byte)| !matches!(index, 4 | 7 | 10 | 13 | 16) && !byte.is_ascii_digit())
        || fraction.len() > 9
        || !fraction.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(format!("check timestamp is malformed: {value}"));
    }
    let mut nanos = fraction.to_owned();
    while nanos.len() < 9 {
        nanos.push('0');
    }
    Ok((
        whole.to_owned(),
        if nanos.is_empty() {
            0
        } else {
            nanos
                .parse::<u32>()
                .map_err(|_| format!("check timestamp fractional seconds are malformed: {value}"))?
        },
    ))
}

fn selected_check(run: &JsonValue) -> JsonValue {
    json!({
        "id": run.get("id"),
        "name": run.get("name"),
        "status": run.get("status"),
        "conclusion": run.get("conclusion"),
        "started_at": run.get("started_at"),
        "completed_at": run.get("completed_at"),
    })
}

fn summary(run: &JsonValue) -> Option<&str> {
    run.get("output")?
        .get("summary")
        .and_then(JsonValue::as_str)
}

fn summary_markers(summary: &str) -> Result<BTreeMap<String, String>, String> {
    let mut markers = BTreeMap::new();
    for token in summary.split_ascii_whitespace() {
        let Some((key, value)) = token.split_once('=') else {
            return Err("proof summary contains a token without key=value shape".to_owned());
        };
        if key.is_empty()
            || value.is_empty()
            || !key
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
        {
            return Err("proof summary contains an unsafe key=value marker".to_owned());
        }
        if markers.insert(key.to_owned(), value.to_owned()).is_some() {
            return Err(format!("proof summary repeats marker {key}"));
        }
    }
    for required in [
        "receipt_sha256",
        "attempt_id",
        "auditor_sha256",
        "proof_status",
        "score",
        "hard_findings",
        "caps_applied",
    ] {
        if !markers.contains_key(required) {
            return Err(format!("proof summary is missing marker {required}"));
        }
    }
    Ok(markers)
}

fn marker_u64(markers: &BTreeMap<String, String>, key: &str) -> Option<u64> {
    markers.get(key)?.parse().ok()
}

fn classify_forge_error(context: &RepoContext, error: &str) -> JsonValue {
    if error.contains("unknown or ambiguous")
        || error.contains("repository not found")
        || error.contains("HTTP 404")
    {
        assessed(
            context,
            "unpublished",
            vec!["local exact HEAD is unknown to the authenticated forge".to_owned()],
            None,
        )
    } else {
        assessed(
            context,
            "blocked",
            vec![format!("authenticated forge checks unavailable: {error}")],
            None,
        )
    }
}

fn assessed(
    context: &RepoContext,
    status: &str,
    reasons: Vec<String>,
    evidence: Option<JsonValue>,
) -> JsonValue {
    result(
        &context.spec,
        Some(&context.head),
        Some(&context.policy),
        status,
        reasons,
        evidence,
    )
}

fn result(
    spec: &RepoSpec,
    head: Option<&str>,
    policy: Option<&Policy>,
    status: &str,
    reasons: Vec<String>,
    evidence: Option<JsonValue>,
) -> JsonValue {
    json!({
        "name": spec.name,
        "family": spec.family,
        "phase": spec.phase,
        "wave": spec.wave,
        "path": spec.path,
        "forge_repo": spec.forge_repo,
        "required_check": spec.required_check,
        "head_sha": head,
        "status": status,
        "reasons": reasons,
        "policy": policy.map(|policy| json!({
            "minimum_score": policy.minimum_score,
            "floor_enforced": policy.floor_enforced,
            "baseline_score": policy.baseline_score,
        })),
        "evidence": evidence,
    })
}

fn build_report(
    manifest: &Path,
    manifest_sha256: &str,
    jobs: usize,
    mut repositories: Vec<JsonValue>,
) -> JsonValue {
    repositories.sort_by(|left, right| {
        (
            left["phase"].as_i64().unwrap_or(i64::MAX),
            left["wave"].as_i64().unwrap_or(i64::MAX),
            left["name"].as_str().unwrap_or_default(),
        )
            .cmp(&(
                right["phase"].as_i64().unwrap_or(i64::MAX),
                right["wave"].as_i64().unwrap_or(i64::MAX),
                right["name"].as_str().unwrap_or_default(),
            ))
    });
    let mut counts = BTreeMap::from([
        ("blocked", 0_u64),
        ("green", 0),
        ("red", 0),
        ("unproven", 0),
        ("unpublished", 0),
    ]);
    for repo in &repositories {
        if let Some(count) = repo
            .get("status")
            .and_then(JsonValue::as_str)
            .and_then(|status| counts.get_mut(status))
        {
            *count += 1;
        }
    }
    let repair_queue = repositories
        .iter()
        .filter(|repo| repo["status"] != "green")
        .enumerate()
        .map(|(index, repo)| {
            json!({
                "position": index + 1,
                "phase": repo["phase"],
                "wave": repo["wave"],
                "name": repo["name"],
                "head_sha": repo["head_sha"],
                "status": repo["status"],
                "reasons": repo["reasons"],
            })
        })
        .collect::<Vec<_>>();
    json!({
        "schema_version": "jain.split.quality-status/v1",
        "manifest": manifest,
        "manifest_sha256": manifest_sha256,
        "jobs": jobs,
        "repository_count": repositories.len(),
        "category_counts": counts,
        "repositories": repositories,
        "repair_queue": repair_queue,
    })
}

fn manifest_order(manifest: &toml::Value, name: &str, family: &str, fallback: usize) -> (i64, i64) {
    for key in ["repo", "infrastructure_repo"] {
        if let Some(raw) = manifest
            .get(key)
            .and_then(toml::Value::as_array)
            .into_iter()
            .flatten()
            .find(|raw| raw.get("name").and_then(toml::Value::as_str) == Some(name))
        {
            return (
                1,
                raw.get("rollout_wave")
                    .and_then(toml::Value::as_integer)
                    .unwrap_or(fallback as i64),
            );
        }
    }
    if manifest
        .get("control_plane")
        .and_then(|value| value.get("name"))
        .and_then(toml::Value::as_str)
        == Some(name)
    {
        return (0, 0);
    }
    if let Some(nested) = manifest.get("nested_families").and_then(|value| {
        value.as_table().and_then(|families| {
            families.values().find(|registration| {
                registration.get("family").and_then(toml::Value::as_str) == Some(family)
            })
        })
    }) {
        let phase = nested
            .get("release_phase")
            .and_then(toml::Value::as_integer)
            .unwrap_or(0);
        if nested
            .get("control_plane_name")
            .and_then(toml::Value::as_str)
            == Some(name)
        {
            return (
                phase,
                nested
                    .get("control_plane_rollout_wave")
                    .and_then(toml::Value::as_integer)
                    .unwrap_or(10_000),
            );
        }
        if let Some(raw) = nested
            .get("repository")
            .and_then(toml::Value::as_array)
            .into_iter()
            .flatten()
            .find(|raw| raw.get("name").and_then(toml::Value::as_str) == Some(name))
        {
            return (
                phase,
                raw.get("rollout_wave")
                    .and_then(toml::Value::as_integer)
                    .unwrap_or(fallback as i64),
            );
        }
    }
    (if family == "jain-split" { 1 } else { 0 }, fallback as i64)
}

fn forge_slug(remote: &str) -> Result<String, Box<dyn std::error::Error>> {
    let slug = remote
        .strip_prefix("http://127.0.0.1:8787/git/")
        .and_then(|value| value.strip_suffix(".git"))
        .ok_or("managed repository remote is not a canonical local-Jeryu URL")?;
    let mut parts = slug.split('/');
    if !matches!(
        (parts.next(), parts.next(), parts.next()),
        (Some(owner), Some(repo), None)
            if !owner.is_empty()
                && !repo.is_empty()
                && [owner, repo].into_iter().all(|part| part.bytes().all(|byte| {
                    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.')
                }))
    ) {
        return Err("managed repository remote has an unsafe forge slug".into());
    }
    Ok(slug.to_owned())
}

fn bounded_regular(path: &Path, limit: u64) -> Result<Vec<u8>, String> {
    let before =
        fs::symlink_metadata(path).map_err(|error| format!("{}: {error}", path.display()))?;
    if !before.file_type().is_file() || before.len() > limit {
        return Err(format!("{} is not a bounded regular file", path.display()));
    }
    let bytes = fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let after =
        fs::symlink_metadata(path).map_err(|error| format!("{}: {error}", path.display()))?;
    if bytes.len() as u64 != before.len()
        || before.len() != after.len()
        || before.modified().ok() != after.modified().ok()
    {
        return Err(format!("{} changed while being read", path.display()));
    }
    Ok(bytes)
}

fn numeric(value: Option<&toml::Value>) -> Option<f64> {
    value?
        .as_float()
        .or_else(|| value?.as_integer().map(|value| value as f64))
}

fn json_number(value: Option<&JsonValue>) -> Option<f64> {
    value?.as_f64()
}

fn zero_count(value: &JsonValue) -> bool {
    value.as_u64() == Some(0) || value.as_array().is_some_and(|entries| entries.is_empty())
}

fn lower_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_ID: AtomicU64 = AtomicU64::new(1);

    fn spec() -> RepoSpec {
        RepoSpec {
            name: "jain-test".to_owned(),
            path: PathBuf::from("/test/jain-test"),
            forge_repo: "veox/jain-test".to_owned(),
            required_check: "jain-test/required".to_owned(),
            family: "jain-split".to_owned(),
            kind: "family".to_owned(),
            phase: 1,
            wave: 2,
        }
    }

    fn context(baseline: Option<f64>) -> RepoContext {
        RepoContext {
            spec: spec(),
            head: "a".repeat(40),
            policy: Policy {
                minimum_score: 85.0,
                floor_enforced: true,
                baseline_score: baseline,
            },
        }
    }

    fn summary(score: f64, hard: u64, caps: u64) -> String {
        format!(
            "receipt_sha256={} attempt_id=attempt-1 auditor_sha256={} proof_status=pass score={score} hard_findings={hard} caps_applied={caps}",
            "b".repeat(64),
            "c".repeat(64)
        )
    }

    fn run(id: &str, name: &str, time: &str, conclusion: &str, summary: Option<&str>) -> JsonValue {
        json!({
            "id": id,
            "name": name,
            "head_sha": "a".repeat(40),
            "status": "completed",
            "conclusion": conclusion,
            "started_at": time,
            "completed_at": time,
            "output": summary.map(|summary| json!({"summary": summary})),
        })
    }

    fn response(runs: Vec<JsonValue>) -> JsonValue {
        json!({"total_count": runs.len(), "check_runs": runs})
    }

    #[test]
    fn latest_success_supersedes_failure_and_latest_failure_wins() {
        let proof = summary(90.0, 0, 0);
        let success = response(vec![
            run(
                "1",
                "jain-test/required",
                "2026-07-25T01:00:00Z",
                "failure",
                None,
            ),
            run(
                "2",
                "jankurai/proof",
                "2026-07-25T02:00:00Z",
                "success",
                Some(&proof),
            ),
            run(
                "3",
                "jain-test/required",
                "2026-07-25T02:00:01Z",
                "success",
                Some(&proof),
            ),
        ]);
        assert_eq!(
            classify_checks(&context(Some(88.0)), &success)["status"],
            "green"
        );

        let mut later_failure = success;
        later_failure["check_runs"]
            .as_array_mut()
            .unwrap()
            .push(run(
                "4",
                "jain-test/required",
                "2026-07-25T03:00:00Z",
                "failure",
                None,
            ));
        later_failure["total_count"] = json!(4);
        assert_eq!(
            classify_checks(&context(Some(88.0)), &later_failure)["status"],
            "red"
        );
    }

    #[test]
    fn missing_malformed_and_mismatched_proof_is_unproven() {
        assert_eq!(
            classify_checks(&context(None), &response(Vec::new()))["status"],
            "unproven"
        );
        let proof = summary(90.0, 0, 0);
        let malformed = response(vec![
            run(
                "1",
                "jankurai/proof",
                "2026-07-25T02:00:00Z",
                "success",
                Some("malformed"),
            ),
            run(
                "2",
                "jain-test/required",
                "2026-07-25T02:00:01Z",
                "success",
                Some(&proof),
            ),
        ]);
        assert_eq!(
            classify_checks(&context(None), &malformed)["status"],
            "unproven"
        );
        let wrong_context = response(vec![run(
            "3",
            "other/required",
            "2026-07-25T02:00:00Z",
            "success",
            Some(&proof),
        )]);
        assert_eq!(
            classify_checks(&context(None), &wrong_context)["status"],
            "unproven"
        );
    }

    #[test]
    fn score_regression_caps_and_hard_findings_are_red() {
        for proof in [
            summary(87.0, 0, 0),
            summary(90.0, 1, 0),
            summary(90.0, 0, 1),
        ] {
            let checks = response(vec![
                run(
                    "1",
                    "jankurai/proof",
                    "2026-07-25T02:00:00Z",
                    "success",
                    Some(&proof),
                ),
                run(
                    "2",
                    "jain-test/required",
                    "2026-07-25T02:00:01Z",
                    "success",
                    Some(&proof),
                ),
            ]);
            assert_eq!(
                classify_checks(&context(Some(88.0)), &checks)["status"],
                "red"
            );
        }
    }

    #[test]
    fn ambiguous_timestamps_and_duplicate_ids_are_blocked() {
        let proof = summary(90.0, 0, 0);
        let tied = response(vec![
            run(
                "1",
                "jankurai/proof",
                "2026-07-25T02:00:00Z",
                "success",
                Some(&proof),
            ),
            run(
                "2",
                "jankurai/proof",
                "2026-07-25T02:00:00Z",
                "success",
                Some(&proof),
            ),
            run(
                "3",
                "jain-test/required",
                "2026-07-25T02:00:01Z",
                "success",
                Some(&proof),
            ),
        ]);
        assert_eq!(classify_checks(&context(None), &tied)["status"], "blocked");

        let duplicate = response(vec![
            run(
                "1",
                "jankurai/proof",
                "2026-07-25T02:00:00Z",
                "success",
                Some(&proof),
            ),
            run(
                "1",
                "jain-test/required",
                "2026-07-25T02:00:01Z",
                "success",
                Some(&proof),
            ),
        ]);
        assert_eq!(
            classify_checks(&context(None), &duplicate)["status"],
            "blocked"
        );
    }

    #[test]
    fn forge_errors_distinguish_unpublished_from_blocked() {
        assert_eq!(
            classify_forge_error(
                &context(None),
                "HTTP 422: commit reference is unknown or ambiguous"
            )["status"],
            "unpublished"
        );
        assert_eq!(
            classify_forge_error(&context(None), "HTTP 401 authentication failed")["status"],
            "blocked"
        );
    }

    #[test]
    fn dirty_checkout_is_detected_without_forge_evidence() {
        let root = std::env::temp_dir().join(format!(
            "splitctl-quality-dirty-{}-{}",
            std::process::id(),
            NEXT_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let initialized = std::process::Command::new("git")
            .args(["init", "--quiet"])
            .current_dir(&root)
            .status()
            .unwrap();
        assert!(initialized.success());
        fs::write(root.join("untracked.txt"), b"dirty\n").unwrap();
        assert!(local_dirty(&root).unwrap());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn report_order_is_deterministic() {
        let mut first = result(
            &spec(),
            Some(&"a".repeat(40)),
            None,
            "red",
            vec!["red".to_owned()],
            None,
        );
        first["name"] = json!("z");
        first["wave"] = json!(9);
        let mut second = first.clone();
        second["name"] = json!("a");
        second["wave"] = json!(1);
        let left = build_report(
            Path::new("manifest"),
            &"d".repeat(64),
            4,
            vec![first.clone(), second.clone()],
        );
        let right = build_report(
            Path::new("manifest"),
            &"d".repeat(64),
            4,
            vec![second, first],
        );
        assert_eq!(left, right);
        assert_eq!(left["repair_queue"][0]["name"], "a");
    }
}
