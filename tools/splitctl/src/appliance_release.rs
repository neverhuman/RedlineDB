//! Fail-closed conductor for the 9.0.0-appliance.1 production hotfix.
//!
//! This module owns release identity and authorization, not deployment logic.
//! A separately reviewed, digest-bound Rust operator performs an apply after
//! splitctl has verified the immutable spec and two-owner action envelope.

use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, OpenOptions},
    io::{Read, Write},
    os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{SystemTime, UNIX_EPOCH},
};

type AnyError = Box<dyn std::error::Error>;

const RELEASE: &str = "9.0.0-appliance.1";
const STATUS: &str = "production-hotfix";
const PUBLIC_HOST: &str = "www.neverhuman.org";
const AUTHORITY_FILE: &str = "authority/jain-9.0.0-appliance.1.production-hotfix.toml";
const EVIDENCE_ROOT: &str = "docs/release-evidence/9.0.0-appliance.1";
const AUTHORITY_SCHEMA: &str = "jain.appliance-production-hotfix-authority/v1";
const SPEC_SCHEMA: &str = "jain.appliance-release-spec/v1";
const PLAN_SCHEMA: &str = "jain.appliance-release-plan/v1";
const ACTION_SCHEMA: &str = "jain.appliance-release-action/v1";
const OPERATOR_PROTOCOL: &str = "jain.appliance-operator-stdin/v1";
const OPERATOR_RESULT_SCHEMA: &str = "jain.appliance-operator-result/v1";
const RECEIPT_SCHEMA: &str = "jain.appliance-release-receipt/v1";
const INDEX_MEDIA_TYPE: &str = "application/vnd.jain.appliance.release.v1+json";
const INDEX_PREFIX: &str = "image.neverhuman.org/jain/appliance@sha256:";
const NONCE_ROOT: &str = "/var/lib/jain-split-ops/appliance-release/consumed-nonces";
const SIGNATURE_SCRATCH_ROOT: &str = "/var/lib/jain-split-ops/appliance-release/signature-scratch";
const MINIMUM_STAGE_SOAK_SECONDS: u64 = 6 * 60 * 60;
const MINIMUM_CANARY_SECONDS: u64 = 60 * 60;
const MAX_DOCUMENT_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Operation {
    Plan,
    Build,
    Publish,
    Stage,
    Qualify,
    Canary,
    Promote,
    Rollback,
}

impl Operation {
    fn parse(value: &str) -> Result<Self, AnyError> {
        Ok(match value {
            "plan" => Self::Plan,
            "build" => Self::Build,
            "publish" => Self::Publish,
            "stage" => Self::Stage,
            "qualify" => Self::Qualify,
            "canary" => Self::Canary,
            "promote" => Self::Promote,
            "rollback" => Self::Rollback,
            _ => return Err(format!("unsupported appliance-release operation: {value}").into()),
        })
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Plan => "plan",
            Self::Build => "build",
            Self::Publish => "publish",
            Self::Stage => "stage",
            Self::Qualify => "qualify",
            Self::Canary => "canary",
            Self::Promote => "promote",
            Self::Rollback => "rollback",
        }
    }
}

#[derive(Debug)]
struct Args {
    operation: Operation,
    authority: Option<PathBuf>,
    evidence_root: Option<PathBuf>,
    spec: Option<PathBuf>,
    action_envelope: Option<PathBuf>,
    host: Option<String>,
    cohort: Option<String>,
    duration_seconds: Option<u64>,
    minimum_soak_seconds: Option<u64>,
    expected_caddy_etag: Option<String>,
    to_receipt: Option<PathBuf>,
    apply: bool,
}

#[derive(Debug)]
struct Authority {
    path: PathBuf,
    sha256: String,
    evidence_root: PathBuf,
    signature_max_age_seconds: u64,
}

#[derive(Debug)]
struct LoadedSpec {
    path: PathBuf,
    value: Value,
    sha256: String,
    evidence_root: PathBuf,
}

trait SignatureVerifier {
    fn verify(&self, public_key: &[u8], payload: &[u8], signature: &str) -> Result<(), AnyError>;
}

struct CosignVerifier;

struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

pub fn run(args: Vec<String>, control_root: &Path) -> Result<(), AnyError> {
    let args = parse_args(args)?;
    let authority_path = args
        .authority
        .clone()
        .unwrap_or_else(|| control_root.join(AUTHORITY_FILE));
    let authority = load_authority(control_root, &authority_path)?;

    if args.operation == Operation::Plan {
        let evidence_root = args
            .evidence_root
            .as_ref()
            .ok_or("appliance-release plan requires --evidence-root")?;
        let evidence_root = resolve_existing_beneath(control_root, evidence_root)?;
        if evidence_root != authority.evidence_root {
            return Err("--evidence-root differs from the production-hotfix authority".into());
        }
        let spec = load_spec(
            &authority.evidence_root.join("release-spec.json"),
            &authority,
        )?;
        let blockers = readiness_blockers(&spec, &authority, Operation::Build)?;
        let plan = finish_plan(json!({
            "schema_version": PLAN_SCHEMA,
            "release": RELEASE,
            "status": STATUS,
            "formal_ga": false,
            "operation": "plan",
            "mode": "dry-run",
            "authority_path": relative_display(control_root, &authority.path)?,
            "authority_sha256": authority.sha256,
            "evidence_root": relative_display(control_root, &authority.evidence_root)?,
            "spec_path": relative_display(control_root, &spec.path)?,
            "spec_sha256": spec.sha256,
            "next_operation": "build",
            "ready": blockers.is_empty(),
            "blockers": blockers,
            "external_state_changed": false,
            "apply_requires_action_envelope": true,
            "required_signatures": 2,
        }))?;
        println!("{}", serde_json::to_string_pretty(&plan)?);
        return Ok(());
    }

    let spec_path = args
        .spec
        .as_ref()
        .ok_or("appliance-release operation requires --spec")?;
    let spec = load_spec(spec_path, &authority)?;
    let scope = operation_scope(&args, &spec)?;
    let mut blockers = readiness_blockers(&spec, &authority, args.operation)?;
    if args.operation == Operation::Rollback && scope["to_receipt_sha256"].is_null() {
        blockers.push("requested pre-v9 rollback receipt is absent".into());
    }
    blockers.sort();
    blockers.dedup();
    let plan = finish_plan(json!({
        "schema_version": PLAN_SCHEMA,
        "release": RELEASE,
        "status": STATUS,
        "formal_ga": false,
        "operation": args.operation.as_str(),
        "mode": if args.apply { "apply" } else { "dry-run" },
        "authority_sha256": authority.sha256,
        "spec_sha256": spec.sha256,
        "scope": scope,
        "ready": blockers.is_empty(),
        "blockers": blockers,
        "external_state_changed": false,
        "apply_requires_action_envelope": true,
        "required_signatures": 2,
    }))?;

    if !args.apply {
        println!("{}", serde_json::to_string_pretty(&plan)?);
        return Ok(());
    }
    if !plan["blockers"].as_array().is_some_and(Vec::is_empty) {
        return Err(
            "appliance release apply is blocked by the reported immutable-spec gates".into(),
        );
    }
    let envelope_path = args
        .action_envelope
        .as_ref()
        .ok_or("appliance release --apply requires --action-envelope")?;
    let verifier = CosignVerifier;
    let verified = load_and_verify_envelope(
        envelope_path,
        &plan,
        &spec,
        &authority,
        &verifier,
        now_seconds()?,
    )?;
    consume_nonce(&spec, &verified.nonce, &verified.file_sha256)?;
    match execute_operator(&spec, &plan, &verified) {
        Ok(result) => {
            let receipt = write_receipt(&spec, &authority, &plan, &verified, result)?;
            println!("{}", receipt.display());
            Ok(())
        }
        Err(error) => {
            let failure = json!({
                "schema_version": OPERATOR_RESULT_SCHEMA,
                "release": RELEASE,
                "operation": plan["operation"],
                "spec_sha256": spec.sha256,
                "pass": false,
                "external_state_changed": "unknown",
                "error": error.to_string(),
            });
            let receipt = write_receipt(&spec, &authority, &plan, &verified, failure)?;
            Err(format!(
                "appliance operator failed after nonce consumption; failure receipt: {}",
                receipt.display()
            )
            .into())
        }
    }
}

fn parse_args(values: Vec<String>) -> Result<Args, AnyError> {
    let mut iter = values.into_iter();
    let operation = Operation::parse(&iter.next().ok_or(
        "appliance-release needs plan|build|publish|stage|qualify|canary|promote|rollback",
    )?)?;
    let mut parsed = Args {
        operation,
        authority: None,
        evidence_root: None,
        spec: None,
        action_envelope: None,
        host: None,
        cohort: None,
        duration_seconds: None,
        minimum_soak_seconds: None,
        expected_caddy_etag: None,
        to_receipt: None,
        apply: false,
    };
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--authority" => parsed.authority = Some(PathBuf::from(next_arg(&mut iter, &arg)?)),
            "--evidence-root" => {
                parsed.evidence_root = Some(PathBuf::from(next_arg(&mut iter, &arg)?))
            }
            "--spec" => parsed.spec = Some(PathBuf::from(next_arg(&mut iter, &arg)?)),
            "--action-envelope" => {
                parsed.action_envelope = Some(PathBuf::from(next_arg(&mut iter, &arg)?))
            }
            "--host" => parsed.host = Some(next_arg(&mut iter, &arg)?),
            "--cohort" => parsed.cohort = Some(next_arg(&mut iter, &arg)?),
            "--duration" => {
                parsed.duration_seconds = Some(parse_duration(&next_arg(&mut iter, &arg)?)?)
            }
            "--minimum-soak" => {
                parsed.minimum_soak_seconds = Some(parse_duration(&next_arg(&mut iter, &arg)?)?)
            }
            "--expected-caddy-etag" => {
                parsed.expected_caddy_etag = Some(next_arg(&mut iter, &arg)?)
            }
            "--to-receipt" => parsed.to_receipt = Some(PathBuf::from(next_arg(&mut iter, &arg)?)),
            "--apply" => {
                if parsed.apply {
                    return Err("duplicate --apply".into());
                }
                parsed.apply = true;
            }
            value => return Err(format!("unknown appliance-release argument: {value}").into()),
        }
    }
    validate_arg_shape(&parsed)?;
    Ok(parsed)
}

fn next_arg(iter: &mut impl Iterator<Item = String>, flag: &str) -> Result<String, AnyError> {
    iter.next()
        .ok_or_else(|| format!("{flag} needs a value").into())
}

fn validate_arg_shape(args: &Args) -> Result<(), AnyError> {
    if args.operation == Operation::Plan {
        if args.apply
            || args.spec.is_some()
            || args.action_envelope.is_some()
            || args.host.is_some()
            || args.cohort.is_some()
            || args.duration_seconds.is_some()
            || args.minimum_soak_seconds.is_some()
            || args.expected_caddy_etag.is_some()
            || args.to_receipt.is_some()
        {
            return Err(
                "appliance-release plan accepts only --authority and --evidence-root".into(),
            );
        }
        if args.authority.is_none() || args.evidence_root.is_none() {
            return Err("appliance-release plan requires --authority and --evidence-root".into());
        }
        return Ok(());
    }
    if args.authority.is_some() || args.evidence_root.is_some() || args.spec.is_none() {
        return Err("non-plan operations accept --spec, not --authority or --evidence-root".into());
    }
    if args.action_envelope.is_some() && !args.apply {
        return Err("--action-envelope is accepted only with --apply".into());
    }
    if args.apply && args.action_envelope.is_none() {
        return Err("every appliance-release --apply requires --action-envelope".into());
    }
    let shape_ok = match args.operation {
        Operation::Build | Operation::Publish => {
            args.host.is_none()
                && args.cohort.is_none()
                && args.duration_seconds.is_none()
                && args.minimum_soak_seconds.is_none()
                && args.expected_caddy_etag.is_none()
                && args.to_receipt.is_none()
        }
        Operation::Stage => {
            args.host.as_deref() == Some("atomicsoul")
                && args.cohort.is_none()
                && args.duration_seconds.is_none()
                && args.minimum_soak_seconds.is_none()
                && args.expected_caddy_etag.is_none()
                && args.to_receipt.is_none()
        }
        Operation::Qualify => {
            args.minimum_soak_seconds
                .is_some_and(|value| value >= MINIMUM_STAGE_SOAK_SECONDS)
                && args.host.is_none()
                && args.cohort.is_none()
                && args.duration_seconds.is_none()
                && args.expected_caddy_etag.is_none()
                && args.to_receipt.is_none()
        }
        Operation::Canary => {
            args.host.as_deref() == Some(PUBLIC_HOST)
                && args.cohort.as_deref() == Some("signed-cookie")
                && args.duration_seconds == Some(MINIMUM_CANARY_SECONDS)
                && args.minimum_soak_seconds.is_none()
                && args.expected_caddy_etag.is_none()
                && args.to_receipt.is_none()
        }
        Operation::Promote => {
            args.host.as_deref() == Some(PUBLIC_HOST)
                && args
                    .expected_caddy_etag
                    .as_deref()
                    .is_some_and(valid_strong_etag)
                && args.cohort.is_none()
                && args.duration_seconds.is_none()
                && args.minimum_soak_seconds.is_none()
                && args.to_receipt.is_none()
        }
        Operation::Rollback => {
            args.host.as_deref() == Some(PUBLIC_HOST)
                && args.to_receipt.is_some()
                && args.cohort.is_none()
                && args.duration_seconds.is_none()
                && args.minimum_soak_seconds.is_none()
                && args.expected_caddy_etag.is_none()
        }
        Operation::Plan => false,
    };
    if !shape_ok {
        return Err(format!("invalid or incomplete {} scope", args.operation.as_str()).into());
    }
    Ok(())
}

fn parse_duration(value: &str) -> Result<u64, AnyError> {
    let split = value
        .find(|ch: char| !ch.is_ascii_digit())
        .ok_or("duration needs an s, m, or h suffix")?;
    let (number, suffix) = value.split_at(split);
    if number.is_empty() || number.starts_with('0') && number != "0" {
        return Err("duration must be a canonical non-negative integer".into());
    }
    let number = number.parse::<u64>()?;
    let multiplier = match suffix {
        "s" => 1,
        "m" => 60,
        "h" => 3600,
        _ => return Err("duration needs an s, m, or h suffix".into()),
    };
    number
        .checked_mul(multiplier)
        .ok_or_else(|| "duration overflows seconds".into())
}

fn load_authority(control_root: &Path, requested: &Path) -> Result<Authority, AnyError> {
    let expected = resolve_existing_beneath(control_root, &control_root.join(AUTHORITY_FILE))?;
    let requested = resolve_existing_beneath(control_root, requested)?;
    if requested != expected {
        return Err("only the canonical appliance production-hotfix authority is accepted".into());
    }
    let bytes = secure_read(&requested, MAX_DOCUMENT_BYTES)?;
    let text = std::str::from_utf8(&bytes)?;
    let value: toml::Value = text.parse()?;
    let table = value
        .as_table()
        .ok_or("appliance authority must be a TOML table")?;
    exact_toml_keys(
        table,
        &[
            "schema_version",
            "release",
            "status",
            "formal_ga",
            "runtime_roster",
            "runtime_roles",
            "public_host",
            "evidence_root",
            "alpha6_authority",
            "release_index",
            "installer",
            "rollback",
            "operation_policy",
            "soak_policy",
        ],
        "authority",
    )?;
    expect_toml_str(table, "schema_version", AUTHORITY_SCHEMA)?;
    expect_toml_str(table, "release", RELEASE)?;
    expect_toml_str(table, "status", STATUS)?;
    expect_toml_bool(table, "formal_ga", false)?;
    expect_toml_str(table, "runtime_roster", "appliance-v9")?;
    expect_toml_str(table, "public_host", PUBLIC_HOST)?;
    expect_toml_str(table, "evidence_root", EVIDENCE_ROOT)?;
    expect_toml_bool(table, "alpha6_authority", false)?;
    let roles = table
        .get("runtime_roles")
        .and_then(toml::Value::as_array)
        .ok_or("authority.runtime_roles must be an array")?
        .iter()
        .map(|role| role.as_str().ok_or("authority role must be a string"))
        .collect::<Result<Vec<_>, _>>()?;
    if roles != ["caddy", "jainhub", "jainnode"] {
        return Err("authority runtime roster must be exactly caddy, jainhub, jainnode".into());
    }
    validate_authority_tables(table)?;
    let evidence_root = resolve_existing_beneath(control_root, &control_root.join(EVIDENCE_ROOT))?;
    let signature_max_age_seconds = table["operation_policy"]["signature_max_age_seconds"]
        .as_integer()
        .and_then(|value| u64::try_from(value).ok())
        .ok_or("authority signature age must be a positive integer")?;
    Ok(Authority {
        path: requested,
        sha256: sha256(&bytes),
        evidence_root,
        signature_max_age_seconds,
    })
}

fn validate_authority_tables(table: &toml::value::Table) -> Result<(), AnyError> {
    let index = exact_toml_table(table, "release_index", &["repository", "media_type"])?;
    expect_toml_str(index, "repository", "image.neverhuman.org/jain/appliance")?;
    expect_toml_str(index, "media_type", INDEX_MEDIA_TYPE)?;
    let installer = exact_toml_table(
        table,
        "installer",
        &["url", "release", "profile", "gpu", "port"],
    )?;
    expect_toml_str(
        installer,
        "url",
        "https://www.neverhuman.org/install-jain.sh",
    )?;
    expect_toml_str(installer, "release", RELEASE)?;
    expect_toml_str(installer, "profile", "local")?;
    expect_toml_str(installer, "gpu", "auto")?;
    if installer.get("port").and_then(toml::Value::as_integer) != Some(4180) {
        return Err("authority installer port must be 4180".into());
    }
    let rollback = exact_toml_table(
        table,
        "rollback",
        &[
            "capture_status",
            "caddy_container",
            "legacy_guest_upstream",
            "landing_upstream",
            "legacy_matchers",
            "fresh_caddy_capture_required",
            "containers_required",
            "image_ids_required",
            "source_tag_required",
            "configuration_required",
        ],
    )?;
    expect_toml_str(rollback, "capture_status", "required-before-first-apply")?;
    expect_toml_str(rollback, "caddy_container", "proxy-caddy-1")?;
    expect_toml_str(rollback, "legacy_guest_upstream", "192.168.68.54:8082")?;
    expect_toml_str(rollback, "landing_upstream", "jain-landing:80")?;
    for key in [
        "fresh_caddy_capture_required",
        "containers_required",
        "image_ids_required",
        "source_tag_required",
        "configuration_required",
    ] {
        expect_toml_bool(rollback, key, true)?;
    }
    let matchers = rollback
        .get("legacy_matchers")
        .and_then(toml::Value::as_array)
        .ok_or("authority rollback matchers must be an array")?
        .iter()
        .map(|value| value.as_str().ok_or("rollback matcher must be a string"))
        .collect::<Result<Vec<_>, _>>()?;
    if matchers != ["/api/public/*", "/try", "/live/gNN/*", "fallback"] {
        return Err(
            "authority rollback matchers differ from the preserved production route".into(),
        );
    }
    let policy = exact_toml_table(
        table,
        "operation_policy",
        &[
            "plan_only_without_apply",
            "apply_requires_action_envelope",
            "required_signatures",
            "signature_max_age_seconds",
            "nonce_root",
            "signature_scratch_root",
            "single_use_nonce",
            "gated_operations",
        ],
    )?;
    for key in [
        "plan_only_without_apply",
        "apply_requires_action_envelope",
        "single_use_nonce",
    ] {
        expect_toml_bool(policy, key, true)?;
    }
    if policy
        .get("required_signatures")
        .and_then(toml::Value::as_integer)
        != Some(2)
    {
        return Err("authority requires exactly two signatures".into());
    }
    let max_age = policy
        .get("signature_max_age_seconds")
        .and_then(toml::Value::as_integer)
        .unwrap_or_default();
    if !(1..=3600).contains(&max_age) {
        return Err("authority signature max age must be 1..=3600 seconds".into());
    }
    expect_toml_str(policy, "nonce_root", NONCE_ROOT)?;
    expect_toml_str(policy, "signature_scratch_root", SIGNATURE_SCRATCH_ROOT)?;
    let operations = policy
        .get("gated_operations")
        .and_then(toml::Value::as_array)
        .ok_or("authority gated_operations must be an array")?
        .iter()
        .map(|value| value.as_str().ok_or("gated operation must be a string"))
        .collect::<Result<Vec<_>, _>>()?;
    if operations
        != [
            "build", "publish", "stage", "qualify", "canary", "promote", "rollback",
        ]
    {
        return Err("authority action-envelope operation set is not exact".into());
    }
    let soak = exact_toml_table(table, "soak_policy", &["stage_seconds", "canary_seconds"])?;
    if soak.get("stage_seconds").and_then(toml::Value::as_integer)
        != Some(MINIMUM_STAGE_SOAK_SECONDS as i64)
        || soak.get("canary_seconds").and_then(toml::Value::as_integer)
            != Some(MINIMUM_CANARY_SECONDS as i64)
    {
        return Err("authority soak durations must be six hours and one hour".into());
    }
    Ok(())
}

fn load_spec(path: &Path, authority: &Authority) -> Result<LoadedSpec, AnyError> {
    let path = resolve_evidence_path(&authority.evidence_root, path)?;
    if path != authority.evidence_root.join("release-spec.json") {
        return Err(
            "--spec must be the canonical authority evidence-root release-spec.json".into(),
        );
    }
    let bytes = secure_read(&path, MAX_DOCUMENT_BYTES)?;
    let value: Value = serde_json::from_slice(&bytes)?;
    validate_spec_structure(&value, authority)?;
    Ok(LoadedSpec {
        path,
        value,
        sha256: sha256(&bytes),
        evidence_root: authority.evidence_root.clone(),
    })
}

fn validate_spec_structure(spec: &Value, authority: &Authority) -> Result<(), AnyError> {
    let root = exact_object(
        spec,
        &[
            "schema_version",
            "release",
            "status",
            "formal_ga",
            "runtime_roster",
            "public_host",
            "evidence_root",
            "authority_sha256",
            "release_index",
            "runtime_roles",
            "payloads",
            "configuration",
            "source_freeze",
            "rollback",
            "qualification",
            "action_policy",
            "operator",
        ],
        "release spec",
    )?;
    expect_json_str(root, "schema_version", SPEC_SCHEMA)?;
    expect_json_str(root, "release", RELEASE)?;
    expect_json_str(root, "status", STATUS)?;
    expect_json_bool(root, "formal_ga", false)?;
    expect_json_str(root, "runtime_roster", "appliance-v9")?;
    expect_json_str(root, "public_host", PUBLIC_HOST)?;
    expect_json_str(root, "evidence_root", EVIDENCE_ROOT)?;
    expect_json_str(root, "authority_sha256", &authority.sha256)?;

    let index = exact_object(
        &root["release_index"],
        &["media_type", "reference"],
        "release_index",
    )?;
    expect_json_str(index, "media_type", INDEX_MEDIA_TYPE)?;
    validate_optional_index(&index["reference"])?;

    let roles = root["runtime_roles"]
        .as_array()
        .ok_or("runtime_roles must be an array")?;
    if roles.len() != 3 {
        return Err("runtime_roles must contain exactly three entries".into());
    }
    let mut role_names = Vec::new();
    for role in roles {
        let role = exact_object(
            role,
            &["role", "image_digest", "source_commit"],
            "runtime role",
        )?;
        let name = role["role"]
            .as_str()
            .ok_or("runtime role name must be a string")?;
        role_names.push(name);
        validate_optional_digest(&role["image_digest"], true)?;
        validate_optional_oid(&role["source_commit"])?;
    }
    if role_names != ["caddy", "jainhub", "jainnode"] {
        return Err("runtime role order and identity must be caddy, jainhub, jainnode".into());
    }

    let payloads = exact_object(
        &root["payloads"],
        &[
            "web_assets",
            "platform_adapter",
            "jope_model_pack",
            "typst_font_tool_pack",
        ],
        "payloads",
    )?;
    for value in payloads.values() {
        validate_optional_digest(value, false)?;
    }
    let configuration = exact_object(
        &root["configuration"],
        &[
            "local_compose_sha256",
            "production_compose_sha256",
            "config_sha256",
        ],
        "configuration",
    )?;
    for value in configuration.values() {
        validate_optional_digest(value, false)?;
    }
    validate_source_freeze(&root["source_freeze"])?;
    validate_rollback(&root["rollback"])?;
    validate_qualification(&root["qualification"])?;
    validate_action_policy(&root["action_policy"])?;
    let operator = exact_object(
        &root["operator"],
        &["protocol", "path", "sha256"],
        "operator",
    )?;
    expect_json_str(operator, "protocol", OPERATOR_PROTOCOL)?;
    match (&operator["path"], &operator["sha256"]) {
        (Value::Null, Value::Null) => {}
        (Value::String(path), Value::String(digest)) => {
            if !Path::new(path).is_absolute() || !is_sha256(digest) {
                return Err("operator path/digest binding is invalid".into());
            }
        }
        _ => return Err("operator path and sha256 must be both null or both exact".into()),
    }
    Ok(())
}

fn validate_source_freeze(value: &Value) -> Result<(), AnyError> {
    let source = exact_object(
        value,
        &["status", "closure_sha256", "entries"],
        "source_freeze",
    )?;
    let status = source["status"]
        .as_str()
        .ok_or("source freeze status must be a string")?;
    if !["pending", "complete"].contains(&status) {
        return Err("source freeze status must be pending or complete".into());
    }
    validate_optional_digest(&source["closure_sha256"], false)?;
    let entries = source["entries"]
        .as_array()
        .ok_or("source freeze entries must be an array")?;
    let mut repositories = BTreeSet::new();
    for entry in entries {
        let entry = exact_object(
            entry,
            &["repository", "tag", "commit", "tree", "archive_sha256"],
            "source freeze entry",
        )?;
        let repository = entry["repository"]
            .as_str()
            .ok_or("source repository must be a string")?;
        let canonical_owner =
            repository.starts_with("veox/") || repository.starts_with("jeryu/redline-");
        if !canonical_owner || !repositories.insert(repository) {
            return Err(
                "source repositories must be unique canonical veox or Redline identities".into(),
            );
        }
        if entry["tag"].as_str().is_none_or(str::is_empty)
            || !entry["commit"].as_str().is_some_and(is_oid)
            || !entry["tree"].as_str().is_some_and(is_oid)
            || !entry["archive_sha256"].as_str().is_some_and(is_sha256)
        {
            return Err("source freeze entry identity is incomplete or malformed".into());
        }
    }
    if status == "complete" && (entries.is_empty() || source["closure_sha256"].is_null()) {
        return Err("complete source freeze requires entries and a closure digest".into());
    }
    if status == "complete" {
        let calculated = sha256(&canonical_json(&source["entries"])?);
        if source["closure_sha256"].as_str() != Some(&calculated) {
            return Err("source freeze closure digest does not bind the canonical entries".into());
        }
        for required in [
            "veox/jain",
            "veox/jain-battle-gpu",
            "veox/jain-core",
            "veox/jain-deploy",
            "veox/jain-platform",
            "veox/jain-report",
            "veox/jain-shard",
            "veox/jain-smartcluster",
            "veox/jain-starforge",
            "veox/jain-web",
            "veox/jain-zyal",
        ] {
            if !repositories.contains(required) {
                return Err(
                    format!("source freeze omits required hot-release input {required}").into(),
                );
            }
        }
    }
    Ok(())
}

fn validate_rollback(value: &Value) -> Result<(), AnyError> {
    let rollback = exact_object(
        value,
        &[
            "capture_status",
            "capture_receipt",
            "caddy_routes_sha256",
            "non_jain_routes_sha256",
            "configuration_sha256",
            "source_tag",
            "containers",
        ],
        "rollback",
    )?;
    let status = rollback["capture_status"]
        .as_str()
        .ok_or("rollback status must be a string")?;
    if !["pending", "complete"].contains(&status) {
        return Err("rollback capture_status must be pending or complete".into());
    }
    validate_optional_receipt(&rollback["capture_receipt"])?;
    for key in [
        "caddy_routes_sha256",
        "non_jain_routes_sha256",
        "configuration_sha256",
    ] {
        validate_optional_digest(&rollback[key], false)?;
    }
    if !rollback["source_tag"].is_null()
        && rollback["source_tag"].as_str().is_none_or(str::is_empty)
    {
        return Err("rollback source_tag must be null or nonempty".into());
    }
    let containers = rollback["containers"]
        .as_array()
        .ok_or("rollback containers must be an array")?;
    let mut names = BTreeSet::new();
    for container in containers {
        let container = exact_object(container, &["name", "image_id"], "rollback container")?;
        let name = container["name"]
            .as_str()
            .ok_or("rollback container name must be a string")?;
        let image = container["image_id"]
            .as_str()
            .ok_or("rollback image id must be a string")?;
        if name.is_empty()
            || !names.insert(name)
            || !image.starts_with("sha256:")
            || !is_sha256(&image[7..])
        {
            return Err("rollback container/image identity is malformed or duplicated".into());
        }
    }
    if status == "complete"
        && (rollback["capture_receipt"].is_null()
            || rollback["caddy_routes_sha256"].is_null()
            || rollback["non_jain_routes_sha256"].is_null()
            || rollback["configuration_sha256"].is_null()
            || rollback["source_tag"].is_null()
            || containers.is_empty())
    {
        return Err("complete rollback capture omits required production identity".into());
    }
    Ok(())
}

fn validate_qualification(value: &Value) -> Result<(), AnyError> {
    let qualification = exact_object(
        value,
        &[
            "minimum_stage_soak_seconds",
            "minimum_canary_seconds",
            "stage_soak_started_unix_seconds",
            "stage_soak_ended_unix_seconds",
            "canary_started_unix_seconds",
            "canary_ended_unix_seconds",
            "receipts",
        ],
        "qualification",
    )?;
    if qualification["minimum_stage_soak_seconds"].as_u64() != Some(MINIMUM_STAGE_SOAK_SECONDS)
        || qualification["minimum_canary_seconds"].as_u64() != Some(MINIMUM_CANARY_SECONDS)
    {
        return Err("qualification durations differ from authority".into());
    }
    for key in [
        "stage_soak_started_unix_seconds",
        "stage_soak_ended_unix_seconds",
        "canary_started_unix_seconds",
        "canary_ended_unix_seconds",
    ] {
        if !qualification[key].is_null()
            && qualification[key].as_u64().is_none_or(|value| value == 0)
        {
            return Err(format!("qualification {key} must be null or a positive epoch").into());
        }
    }
    let receipts = exact_object(
        &qualification["receipts"],
        &[
            "build", "publish", "stage", "qualify", "canary", "promote", "rollback",
        ],
        "qualification receipts",
    )?;
    for receipt in receipts.values() {
        validate_optional_receipt(receipt)?;
    }
    Ok(())
}

fn validate_action_policy(value: &Value) -> Result<(), AnyError> {
    let policy = exact_object(
        value,
        &[
            "required_signatures",
            "signature_max_age_seconds",
            "nonce_root",
            "signature_scratch_root",
            "owner_keys",
        ],
        "action_policy",
    )?;
    if policy["required_signatures"].as_u64() != Some(2) {
        return Err("action policy must require exactly two signatures".into());
    }
    if policy["signature_max_age_seconds"]
        .as_u64()
        .is_none_or(|value| !(1..=3600).contains(&value))
    {
        return Err("action signature max age must be 1..=3600 seconds".into());
    }
    if policy["nonce_root"].as_str() != Some(NONCE_ROOT)
        || policy["signature_scratch_root"].as_str() != Some(SIGNATURE_SCRATCH_ROOT)
    {
        return Err("action storage roots differ from authority".into());
    }
    let keys = policy["owner_keys"]
        .as_array()
        .ok_or("owner_keys must be an array")?;
    let mut fingerprints = BTreeSet::new();
    for key in keys {
        let key = exact_object(key, &["fingerprint", "path", "sha256"], "owner key")?;
        let digest = key["sha256"]
            .as_str()
            .ok_or("owner key sha256 must be a string")?;
        let fingerprint = key["fingerprint"]
            .as_str()
            .ok_or("owner fingerprint must be a string")?;
        let path = key["path"]
            .as_str()
            .ok_or("owner key path must be a string")?;
        if !is_sha256(digest)
            || fingerprint != format!("sha256:{digest}")
            || Path::new(path).is_absolute()
            || path
                .split('/')
                .any(|part| part.is_empty() || part == "." || part == "..")
            || !fingerprints.insert(fingerprint)
        {
            return Err("owner key identity/path is malformed or duplicated".into());
        }
    }
    Ok(())
}

fn operation_scope(args: &Args, spec: &LoadedSpec) -> Result<Value, AnyError> {
    Ok(match args.operation {
        Operation::Build | Operation::Publish => json!({}),
        Operation::Stage => json!({"host": "atomicsoul"}),
        Operation::Qualify => json!({"minimum_soak_seconds": args.minimum_soak_seconds}),
        Operation::Canary => json!({
            "host": PUBLIC_HOST,
            "cohort": "signed-cookie",
            "duration_seconds": args.duration_seconds,
        }),
        Operation::Promote => json!({
            "host": PUBLIC_HOST,
            "expected_caddy_etag": args.expected_caddy_etag,
        }),
        Operation::Rollback => {
            let path = args
                .to_receipt
                .as_ref()
                .ok_or("rollback scope requires --to-receipt")?;
            let resolved = resolve_evidence_path(&spec.evidence_root, path);
            match resolved {
                Ok(resolved) => json!({
                    "host": PUBLIC_HOST,
                    "to_receipt": relative_display(&spec.evidence_root, &resolved)?,
                    "to_receipt_sha256": sha256(&secure_read(&resolved, MAX_DOCUMENT_BYTES)?),
                }),
                Err(_) => json!({
                    "host": PUBLIC_HOST,
                    "to_receipt": path.display().to_string(),
                    "to_receipt_sha256": null,
                }),
            }
        }
        Operation::Plan => return Err("plan has no executable operation scope".into()),
    })
}

fn readiness_blockers(
    spec: &LoadedSpec,
    authority: &Authority,
    operation: Operation,
) -> Result<Vec<String>, AnyError> {
    let mut blockers = Vec::new();
    let root = spec
        .value
        .as_object()
        .ok_or("validated spec is not an object")?;
    let operator = root["operator"]
        .as_object()
        .ok_or("validated operator is not an object")?;
    if operator["path"].is_null() || operator["sha256"].is_null() {
        blockers.push("reviewed appliance operator binary is not digest-bound".into());
    } else if let Err(error) = validate_operator_binding(operator) {
        blockers.push(format!("appliance operator binding: {error}"));
    }
    let policy = root["action_policy"]
        .as_object()
        .ok_or("validated action policy is not an object")?;
    let nonce_root = Path::new(
        policy["nonce_root"]
            .as_str()
            .ok_or("validated action nonce root is absent")?,
    );
    if let Err(error) = require_root_private_dir(nonce_root) {
        blockers.push(format!("single-use nonce root: {error}"));
    }
    if let Err(error) = require_root_private_dir(Path::new(SIGNATURE_SCRATCH_ROOT)) {
        blockers.push(format!("signature scratch root: {error}"));
    }
    let keys = policy["owner_keys"]
        .as_array()
        .ok_or("validated owner keys are not an array")?;
    if keys.len() != 2 {
        blockers.push("exactly two approved owner public keys are not bound".into());
    } else {
        for key in keys {
            if let Err(error) = validate_owner_key_file(&spec.evidence_root, key) {
                blockers.push(format!("owner key: {error}"));
            }
        }
    }
    if policy["signature_max_age_seconds"].as_u64() != Some(authority.signature_max_age_seconds) {
        blockers.push("spec signature age differs from authority".into());
    }
    let source = root["source_freeze"]
        .as_object()
        .ok_or("validated source freeze is not an object")?;
    if source["status"] != "complete" || source["entries"].as_array().is_none_or(Vec::is_empty) {
        blockers.push("deterministic transitive source freeze is incomplete".into());
    }

    match operation {
        Operation::Build => {}
        Operation::Publish => {
            require_release_pins(root, &mut blockers)?;
            require_receipt(spec, "build", &mut blockers);
        }
        Operation::Stage => {
            require_release_pins(root, &mut blockers)?;
            require_receipt(spec, "publish", &mut blockers);
        }
        Operation::Qualify => {
            require_receipt(spec, "stage", &mut blockers);
            require_elapsed(
                root,
                "stage_soak_started_unix_seconds",
                "stage_soak_ended_unix_seconds",
                MINIMUM_STAGE_SOAK_SECONDS,
                "six-hour unchanged stage soak",
                &mut blockers,
            )?;
        }
        Operation::Canary => require_receipt(spec, "qualify", &mut blockers),
        Operation::Promote => {
            require_receipt(spec, "canary", &mut blockers);
            require_elapsed(
                root,
                "canary_started_unix_seconds",
                "canary_ended_unix_seconds",
                MINIMUM_CANARY_SECONDS,
                "one-hour signed-cookie canary",
                &mut blockers,
            )?;
        }
        Operation::Rollback => {
            let rollback = root["rollback"]
                .as_object()
                .ok_or("validated rollback is not an object")?;
            if rollback["capture_status"] != "complete" {
                blockers.push("fresh complete pre-v9 rollback identity is absent".into());
            }
        }
        Operation::Plan => return Err("plan has no apply readiness state".into()),
    }
    blockers.sort();
    blockers.dedup();
    Ok(blockers)
}

fn require_release_pins(
    root: &Map<String, Value>,
    blockers: &mut Vec<String>,
) -> Result<(), AnyError> {
    if root["release_index"]["reference"].is_null() {
        blockers.push("immutable release-index reference is not pinned".into());
    }
    for role in root["runtime_roles"]
        .as_array()
        .ok_or("validated runtime roles are not an array")?
    {
        if role["image_digest"].is_null() || role["source_commit"].is_null() {
            blockers.push(format!(
                "{} image/source identity is not pinned",
                role["role"].as_str().unwrap_or("runtime role")
            ));
        }
    }
    for (name, value) in root["payloads"]
        .as_object()
        .ok_or("validated payloads are not an object")?
    {
        if value.is_null() {
            blockers.push(format!("{name} payload digest is not pinned"));
        }
    }
    for (name, value) in root["configuration"]
        .as_object()
        .ok_or("validated configuration is not an object")?
    {
        if value.is_null() {
            blockers.push(format!("{name} is not pinned"));
        }
    }
    Ok(())
}

fn require_receipt(spec: &LoadedSpec, name: &str, blockers: &mut Vec<String>) {
    let receipt = &spec.value["qualification"]["receipts"][name];
    if receipt.is_null() {
        blockers.push(format!("successful immutable {name} receipt is absent"));
        return;
    }
    if let Err(error) = validate_receipt_file(&spec.evidence_root, receipt, name, &spec.sha256) {
        blockers.push(format!("{name} receipt: {error}"));
    }
}

fn require_elapsed(
    root: &Map<String, Value>,
    start_name: &str,
    end_name: &str,
    minimum: u64,
    label: &str,
    blockers: &mut Vec<String>,
) -> Result<(), AnyError> {
    let qualification = root["qualification"]
        .as_object()
        .ok_or("validated qualification is not an object")?;
    let elapsed = qualification[start_name]
        .as_u64()
        .zip(qualification[end_name].as_u64())
        .and_then(|(start, end)| end.checked_sub(start));
    if elapsed.is_none_or(|elapsed| elapsed < minimum) {
        blockers.push(format!("{label} is incomplete"));
    }
    Ok(())
}

fn finish_plan(mut plan: Value) -> Result<Value, AnyError> {
    plan.as_object_mut()
        .ok_or("release plan must be an object")?
        .remove("plan_sha256");
    let binding = json!({
        "schema_version": "jain.appliance-release-plan-binding/v1",
        "release": plan["release"],
        "status": plan["status"],
        "formal_ga": plan["formal_ga"],
        "operation": plan["operation"],
        "authority_sha256": plan["authority_sha256"],
        "spec_sha256": plan["spec_sha256"],
        "scope": plan.get("scope").cloned().unwrap_or(Value::Null),
    });
    let digest = sha256(&canonical_json(&binding)?);
    plan.as_object_mut()
        .ok_or("validated release plan object disappeared")?
        .insert("plan_sha256".into(), json!(digest));
    Ok(plan)
}

struct VerifiedEnvelope {
    nonce: String,
    file_sha256: String,
    signed_payload_sha256: String,
    verified_signers: Vec<String>,
}

fn load_and_verify_envelope(
    path: &Path,
    plan: &Value,
    spec: &LoadedSpec,
    authority: &Authority,
    verifier: &dyn SignatureVerifier,
    now: u64,
) -> Result<VerifiedEnvelope, AnyError> {
    if !path.is_absolute() {
        return Err("action envelope path must be absolute".into());
    }
    let bytes = secure_read(path, 1024 * 1024)?;
    let mut value: Value = serde_json::from_slice(&bytes)?;
    let root = exact_object(
        &value,
        &[
            "schema_version",
            "release",
            "operation",
            "authority_sha256",
            "spec_sha256",
            "plan_sha256",
            "issued_unix_seconds",
            "expires_unix_seconds",
            "nonce",
            "signatures",
        ],
        "action envelope",
    )?;
    expect_json_str(root, "schema_version", ACTION_SCHEMA)?;
    expect_json_str(root, "release", RELEASE)?;
    expect_json_str(
        root,
        "operation",
        plan["operation"].as_str().unwrap_or_default(),
    )?;
    expect_json_str(root, "authority_sha256", &authority.sha256)?;
    expect_json_str(root, "spec_sha256", &spec.sha256)?;
    expect_json_str(
        root,
        "plan_sha256",
        plan["plan_sha256"].as_str().unwrap_or_default(),
    )?;
    let issued = root["issued_unix_seconds"]
        .as_u64()
        .ok_or("action issued time must be an epoch")?;
    let expires = root["expires_unix_seconds"]
        .as_u64()
        .ok_or("action expiry must be an epoch")?;
    if issued > now
        || expires <= now
        || expires <= issued
        || now.saturating_sub(issued) > authority.signature_max_age_seconds
        || expires - issued > authority.signature_max_age_seconds
    {
        return Err(
            "action envelope is future-dated, expired, or exceeds its authority window".into(),
        );
    }
    let nonce = root["nonce"]
        .as_str()
        .ok_or("action nonce must be a string")?
        .to_owned();
    validate_nonce(&nonce)?;
    let signatures = root["signatures"]
        .as_array()
        .ok_or("action signatures must be an array")?
        .clone();
    if signatures.len() != 2 {
        return Err("action envelope requires exactly two signatures".into());
    }
    value["signatures"] = Value::Array(Vec::new());
    let payload = canonical_json(&value)?;
    let payload_sha256 = sha256(&payload);
    let key_values = spec.value["action_policy"]["owner_keys"]
        .as_array()
        .ok_or("validated owner keys are not an array")?;
    let mut keys = BTreeMap::new();
    for key in key_values {
        let fingerprint = key["fingerprint"]
            .as_str()
            .ok_or("validated owner fingerprint is absent")?
            .to_owned();
        if keys.insert(fingerprint, key).is_some() {
            return Err("validated owner fingerprint is duplicated".into());
        }
    }
    let mut verified = Vec::new();
    for signature in &signatures {
        let signature = exact_object(
            signature,
            &["signer_fingerprint", "signature", "signed_over_sha256"],
            "action signature",
        )?;
        let fingerprint = signature["signer_fingerprint"]
            .as_str()
            .ok_or("signature fingerprint must be a string")?;
        if signature["signed_over_sha256"].as_str() != Some(&payload_sha256) {
            return Err("action signature does not bind the canonical envelope payload".into());
        }
        let encoded = signature["signature"]
            .as_str()
            .ok_or("signature must be a string")?;
        if encoded.trim().is_empty() {
            return Err("action signature is empty".into());
        }
        let key = keys
            .get(fingerprint)
            .ok_or("action signature uses an unknown owner key")?;
        let key_bytes = load_owner_key(&spec.evidence_root, key)?;
        verifier.verify(&key_bytes, &payload, encoded)?;
        verified.push(fingerprint.to_owned());
    }
    if verified.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err("action signatures must be sorted and from distinct owners".into());
    }
    Ok(VerifiedEnvelope {
        nonce,
        file_sha256: sha256(&bytes),
        signed_payload_sha256: payload_sha256,
        verified_signers: verified,
    })
}

impl SignatureVerifier for CosignVerifier {
    fn verify(&self, public_key: &[u8], payload: &[u8], signature: &str) -> Result<(), AnyError> {
        if unsafe { libc::geteuid() } != 0 {
            return Err("appliance release apply and signature verification require root".into());
        }
        let root = Path::new(SIGNATURE_SCRATCH_ROOT);
        require_root_private_dir(root)?;
        let scratch = allocate_scratch(root)?;
        let key = scratch.0.join("owner.pub");
        let message = scratch.0.join("payload.json");
        let signature_path = scratch.0.join("signature");
        write_new_private(&key, public_key)?;
        write_new_private(&message, payload)?;
        write_new_private(&signature_path, signature.trim().as_bytes())?;
        let output = Command::new("/usr/bin/cosign")
            .env_clear()
            .args([
                "verify-blob",
                "--insecure-ignore-tlog=true",
                "--key",
                key.to_str().ok_or("cosign key path is not UTF-8")?,
                "--signature",
                signature_path
                    .to_str()
                    .ok_or("cosign signature path is not UTF-8")?,
                message.to_str().ok_or("cosign payload path is not UTF-8")?,
            ])
            .output()?;
        if !output.status.success() {
            return Err("cosign rejected an appliance action signature".into());
        }
        Ok(())
    }
}

fn allocate_scratch(root: &Path) -> Result<Scratch, AnyError> {
    for attempt in 0..64 {
        let path = root.join(format!(
            "verify-{}-{}-{attempt}",
            std::process::id(),
            now_seconds()?
        ));
        match fs::create_dir(&path) {
            Ok(()) => {
                fs::set_permissions(&path, fs::Permissions::from_mode(0o700))?;
                return Ok(Scratch(path));
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        }
    }
    Err("cannot allocate signature scratch directory".into())
}

fn consume_nonce(spec: &LoadedSpec, nonce: &str, envelope_sha256: &str) -> Result<(), AnyError> {
    if unsafe { libc::geteuid() } != 0 {
        return Err("appliance release apply requires root".into());
    }
    let root = PathBuf::from(
        spec.value["action_policy"]["nonce_root"]
            .as_str()
            .ok_or("validated nonce root is absent")?,
    );
    require_root_private_dir(&root)?;
    let path = root.join(nonce);
    let body = format!("{envelope_sha256}\n");
    write_new_private(&path, body.as_bytes())
        .map_err(|error| format!("action nonce is already consumed or unavailable: {error}").into())
}

fn execute_operator(
    spec: &LoadedSpec,
    plan: &Value,
    envelope: &VerifiedEnvelope,
) -> Result<Value, AnyError> {
    let operator = spec.value["operator"]
        .as_object()
        .ok_or("validated operator is not an object")?;
    validate_operator_binding(operator)?;
    let path = PathBuf::from(
        operator["path"]
            .as_str()
            .ok_or("validated operator path is absent")?,
    );
    let request = json!({
        "schema_version": OPERATOR_PROTOCOL,
        "release": RELEASE,
        "operation": plan["operation"],
        "authority_sha256": plan["authority_sha256"],
        "spec_path": spec.path,
        "spec_sha256": spec.sha256,
        "plan": plan,
        "action_envelope_sha256": envelope.file_sha256,
        "action_payload_sha256": envelope.signed_payload_sha256,
        "action_nonce": envelope.nonce,
        "verified_signers": envelope.verified_signers,
    });
    let mut child = Command::new(&path)
        .env_clear()
        .arg("appliance-release-apply")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    child
        .stdin
        .take()
        .ok_or("operator stdin is unavailable")?
        .write_all(&canonical_json(&request)?)?;
    let output = child.wait_with_output()?;
    if output.stdout.len() > 1024 * 1024 || output.stderr.len() > 1024 * 1024 {
        return Err("appliance operator output exceeded one MiB".into());
    }
    if !output.status.success() {
        return Err(format!(
            "appliance operator failed after nonce consumption; stderr sha256={}",
            sha256(&output.stderr)
        )
        .into());
    }
    let result: Value = serde_json::from_slice(&output.stdout)?;
    let result_object = exact_object(
        &result,
        &[
            "schema_version",
            "release",
            "operation",
            "spec_sha256",
            "pass",
            "external_state_changed",
            "receipt_path",
            "receipt_sha256",
        ],
        "operator result",
    )?;
    expect_json_str(result_object, "schema_version", OPERATOR_RESULT_SCHEMA)?;
    expect_json_str(result_object, "release", RELEASE)?;
    expect_json_str(
        result_object,
        "operation",
        plan["operation"].as_str().unwrap_or_default(),
    )?;
    expect_json_str(result_object, "spec_sha256", &spec.sha256)?;
    if result_object["pass"].as_bool() != Some(true)
        || result_object["external_state_changed"].as_bool().is_none()
        || result_object["receipt_path"]
            .as_str()
            .is_none_or(str::is_empty)
        || !result_object["receipt_sha256"]
            .as_str()
            .is_some_and(is_sha256)
    {
        return Err("appliance operator result is not a successful exact receipt".into());
    }
    let receipt_path = Path::new(
        result_object["receipt_path"]
            .as_str()
            .ok_or("validated operator receipt path is absent")?,
    );
    if !receipt_path.is_absolute() || receipt_path.starts_with("/tmp") {
        return Err("operator receipt path must be absolute and outside /tmp".into());
    }
    let receipt_bytes = secure_read(receipt_path, MAX_DOCUMENT_BYTES)?;
    if sha256(&receipt_bytes)
        != result_object["receipt_sha256"]
            .as_str()
            .ok_or("validated operator receipt digest is absent")?
    {
        return Err("operator receipt readback differs from the reported digest".into());
    }
    Ok(result)
}

fn write_receipt(
    spec: &LoadedSpec,
    authority: &Authority,
    plan: &Value,
    envelope: &VerifiedEnvelope,
    operator_result: Value,
) -> Result<PathBuf, AnyError> {
    let operation = plan["operation"]
        .as_str()
        .ok_or("plan operation is absent")?;
    let root = spec.evidence_root.join("operations").join(operation);
    fs::create_dir_all(&root)?;
    let root = resolve_existing_beneath(&spec.evidence_root, &root)?;
    let path = root.join(format!("{}-{}.json", now_seconds()?, envelope.nonce));
    let receipt = json!({
        "schema_version": RECEIPT_SCHEMA,
        "release": RELEASE,
        "status": STATUS,
        "formal_ga": false,
        "operation": operation,
        "authority_sha256": authority.sha256,
        "spec_sha256": spec.sha256,
        "plan_sha256": plan["plan_sha256"],
        "action_envelope_sha256": envelope.file_sha256,
        "action_payload_sha256": envelope.signed_payload_sha256,
        "action_nonce": envelope.nonce,
        "verified_signers": envelope.verified_signers,
        "operator_result": operator_result,
        "emitted_unix_seconds": now_seconds()?,
    });
    write_new_private(&path, &serde_json::to_vec_pretty(&receipt)?)?;
    Ok(path)
}

fn validate_operator_binding(operator: &Map<String, Value>) -> Result<(), AnyError> {
    let path = PathBuf::from(
        operator["path"]
            .as_str()
            .ok_or("operator path is not bound")?,
    );
    let expected = operator["sha256"]
        .as_str()
        .ok_or("operator digest is not bound")?;
    let bytes = secure_read(&path, 256 * 1024 * 1024)?;
    let metadata = fs::metadata(&path)?;
    if metadata.uid() != 0
        || metadata.permissions().mode() & 0o022 != 0
        || metadata.permissions().mode() & 0o111 == 0
        || sha256(&bytes) != expected
    {
        return Err(
            "operator must be exact, executable, root-owned, and not group/world-writable".into(),
        );
    }
    Ok(())
}

fn validate_owner_key_file(root: &Path, key: &Value) -> Result<(), AnyError> {
    load_owner_key(root, key).map(|_| ())
}

fn load_owner_key(root: &Path, key: &Value) -> Result<Vec<u8>, AnyError> {
    let path = key["path"].as_str().ok_or("owner key path is absent")?;
    let resolved = resolve_existing_beneath(root, &root.join(path))?;
    let bytes = secure_read(&resolved, 1024 * 1024)?;
    if sha256(&bytes) != key["sha256"].as_str().unwrap_or_default() {
        return Err("owner key bytes differ from their bound digest".into());
    }
    Ok(bytes)
}

fn validate_receipt_file(
    root: &Path,
    receipt: &Value,
    action: &str,
    _spec_sha: &str,
) -> Result<(), AnyError> {
    let path = receipt["path"].as_str().ok_or("receipt path is absent")?;
    let resolved = resolve_existing_beneath(root, &root.join(path))?;
    let bytes = secure_read(&resolved, MAX_DOCUMENT_BYTES)?;
    if sha256(&bytes) != receipt["sha256"].as_str().unwrap_or_default() {
        return Err("receipt bytes differ from the bound digest".into());
    }
    let value: Value = serde_json::from_slice(&bytes)?;
    if value["schema_version"] != RECEIPT_SCHEMA
        || value["release"] != RELEASE
        || value["operation"] != action
        || !value["spec_sha256"].as_str().is_some_and(is_sha256)
        || value["operator_result"]["pass"] != true
    {
        return Err("receipt schema/release/action/spec/pass binding is invalid".into());
    }
    Ok(())
}

fn validate_optional_receipt(value: &Value) -> Result<(), AnyError> {
    if value.is_null() {
        return Ok(());
    }
    let receipt = exact_object(value, &["path", "sha256"], "receipt reference")?;
    let path = receipt["path"]
        .as_str()
        .ok_or("receipt path must be a string")?;
    if Path::new(path).is_absolute()
        || path
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
        || !receipt["sha256"].as_str().is_some_and(is_sha256)
    {
        return Err("receipt reference path or digest is invalid".into());
    }
    Ok(())
}

fn validate_optional_index(value: &Value) -> Result<(), AnyError> {
    if value.is_null() {
        return Ok(());
    }
    let reference = value
        .as_str()
        .ok_or("release index reference must be a string")?;
    if !reference.starts_with(INDEX_PREFIX) || !is_sha256(&reference[INDEX_PREFIX.len()..]) {
        return Err("release index must be one immutable nonzero OCI digest reference".into());
    }
    Ok(())
}

fn validate_optional_digest(value: &Value, image: bool) -> Result<(), AnyError> {
    if value.is_null() {
        return Ok(());
    }
    let value = value.as_str().ok_or("digest must be a string or null")?;
    let digest = if image {
        value
            .strip_prefix("sha256:")
            .ok_or("image digest needs sha256: prefix")?
    } else {
        value
    };
    if !is_sha256(digest) {
        return Err("digest must be lowercase, nonzero SHA-256".into());
    }
    Ok(())
}

fn validate_optional_oid(value: &Value) -> Result<(), AnyError> {
    if value.is_null() || value.as_str().is_some_and(is_oid) {
        Ok(())
    } else {
        Err("commit must be null or a lowercase nonzero full Git OID".into())
    }
}

fn secure_read(path: &Path, max_bytes: u64) -> Result<Vec<u8>, AnyError> {
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)?;
    let metadata = file.metadata()?;
    if !metadata.file_type().is_file() || metadata.nlink() != 1 || metadata.len() > max_bytes {
        return Err(format!(
            "{} must be one bounded physical regular file",
            path.display()
        )
        .into());
    }
    let mut bytes = Vec::with_capacity(usize::try_from(metadata.len())?);
    file.read_to_end(&mut bytes)?;
    if file.metadata()?.len() != metadata.len() || bytes.len() as u64 != metadata.len() {
        return Err(format!("{} changed during read", path.display()).into());
    }
    Ok(bytes)
}

fn write_new_private(path: &Path, bytes: &[u8]) -> Result<(), AnyError> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

fn require_root_private_dir(path: &Path) -> Result<(), AnyError> {
    let metadata = fs::symlink_metadata(path)?;
    if fs::canonicalize(path)? != path {
        return Err(format!(
            "{} contains a symlink or non-canonical component",
            path.display()
        )
        .into());
    }
    if !metadata.file_type().is_dir()
        || metadata.file_type().is_symlink()
        || metadata.uid() != 0
        || metadata.permissions().mode() & 0o077 != 0
    {
        return Err(format!(
            "{} must be a root-owned physical 0700 directory",
            path.display()
        )
        .into());
    }
    Ok(())
}

fn resolve_existing_beneath(root: &Path, requested: &Path) -> Result<PathBuf, AnyError> {
    let root = fs::canonicalize(root)?;
    let lexical = if requested.is_absolute() {
        requested.to_path_buf()
    } else {
        root.join(requested)
    };
    if lexical.components().any(|component| {
        matches!(
            component,
            std::path::Component::CurDir | std::path::Component::ParentDir
        )
    }) {
        return Err(format!(
            "{} contains a relative traversal component",
            lexical.display()
        )
        .into());
    }
    let candidate = fs::canonicalize(&lexical)?;
    if candidate != lexical {
        return Err(format!(
            "{} contains a symlink or non-canonical component",
            lexical.display()
        )
        .into());
    }
    if !candidate.starts_with(&root) {
        return Err(format!("{} escapes {}", candidate.display(), root.display()).into());
    }
    Ok(candidate)
}

fn resolve_evidence_path(root: &Path, requested: &Path) -> Result<PathBuf, AnyError> {
    let requested = if requested.is_absolute() {
        requested.to_path_buf()
    } else if requested.starts_with(EVIDENCE_ROOT) {
        std::env::current_dir()?.join(requested)
    } else {
        root.join(requested)
    };
    resolve_existing_beneath(root, &requested)
}

fn relative_display(root: &Path, path: &Path) -> Result<String, AnyError> {
    Ok(path
        .strip_prefix(fs::canonicalize(root)?)?
        .to_str()
        .ok_or("relative path is not UTF-8")?
        .to_owned())
}

fn exact_object<'a>(
    value: &'a Value,
    keys: &[&str],
    label: &str,
) -> Result<&'a Map<String, Value>, AnyError> {
    let object = value
        .as_object()
        .ok_or_else(|| format!("{label} must be an object"))?;
    let actual = object.keys().map(String::as_str).collect::<BTreeSet<_>>();
    let expected = keys.iter().copied().collect::<BTreeSet<_>>();
    if actual != expected {
        return Err(format!("{label} has missing or unknown fields").into());
    }
    Ok(object)
}

fn exact_toml_table<'a>(
    parent: &'a toml::value::Table,
    key: &str,
    keys: &[&str],
) -> Result<&'a toml::value::Table, AnyError> {
    let table = parent
        .get(key)
        .and_then(toml::Value::as_table)
        .ok_or_else(|| format!("authority.{key} must be a table"))?;
    exact_toml_keys(table, keys, key)?;
    Ok(table)
}

fn exact_toml_keys(table: &toml::value::Table, keys: &[&str], label: &str) -> Result<(), AnyError> {
    let actual = table.keys().map(String::as_str).collect::<BTreeSet<_>>();
    let expected = keys.iter().copied().collect::<BTreeSet<_>>();
    if actual != expected {
        return Err(format!("authority {label} has missing or unknown fields").into());
    }
    Ok(())
}

fn expect_toml_str(table: &toml::value::Table, key: &str, expected: &str) -> Result<(), AnyError> {
    if table.get(key).and_then(toml::Value::as_str) != Some(expected) {
        return Err(format!("authority {key} must be {expected:?}").into());
    }
    Ok(())
}

fn expect_toml_bool(table: &toml::value::Table, key: &str, expected: bool) -> Result<(), AnyError> {
    if table.get(key).and_then(toml::Value::as_bool) != Some(expected) {
        return Err(format!("authority {key} must be {expected}").into());
    }
    Ok(())
}

fn expect_json_str(table: &Map<String, Value>, key: &str, expected: &str) -> Result<(), AnyError> {
    if table.get(key).and_then(Value::as_str) != Some(expected) {
        return Err(format!("{key} differs from authority").into());
    }
    Ok(())
}

fn expect_json_bool(table: &Map<String, Value>, key: &str, expected: bool) -> Result<(), AnyError> {
    if table.get(key).and_then(Value::as_bool) != Some(expected) {
        return Err(format!("{key} differs from authority").into());
    }
    Ok(())
}

fn canonical_json(value: &Value) -> Result<Vec<u8>, AnyError> {
    fn canonical(value: &Value) -> Value {
        match value {
            Value::Array(values) => Value::Array(values.iter().map(canonical).collect()),
            Value::Object(values) => {
                let mut keys = values.keys().collect::<Vec<_>>();
                keys.sort();
                let mut object = Map::new();
                for key in keys {
                    object.insert(key.clone(), canonical(&values[key]));
                }
                Value::Object(object)
            }
            scalar => scalar.clone(),
        }
    }
    Ok(serde_json::to_vec(&canonical(value))?)
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && !value.bytes().all(|byte| byte == b'0')
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn is_oid(value: &str) -> bool {
    value.len() == 40
        && !value.bytes().all(|byte| byte == b'0')
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn validate_nonce(value: &str) -> Result<(), AnyError> {
    if !(16..=128).contains(&value.len())
        || value
            .bytes()
            .any(|byte| !(byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-')))
    {
        return Err("action nonce must be 16..=128 safe filename characters".into());
    }
    Ok(())
}

fn valid_strong_etag(value: &str) -> bool {
    let bytes = value.as_bytes();
    (3..=256).contains(&bytes.len())
        && bytes.first() == Some(&b'"')
        && bytes.last() == Some(&b'"')
        && bytes[1..bytes.len() - 1]
            .iter()
            .all(|byte| byte.is_ascii_graphic() && *byte != b'"' && *byte != b'\\')
}

fn now_seconds() -> Result<u64, AnyError> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT: AtomicU64 = AtomicU64::new(0);

    struct Temp(PathBuf);

    impl Temp {
        fn new(label: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "splitctl-appliance-{label}-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    struct AcceptVerifier;

    impl SignatureVerifier for AcceptVerifier {
        fn verify(
            &self,
            _public_key: &[u8],
            _payload: &[u8],
            _signature: &str,
        ) -> Result<(), AnyError> {
            Ok(())
        }
    }

    #[test]
    fn command_shapes_are_closed_and_apply_always_needs_an_envelope() {
        assert!(parse_args(vec!["build".into(), "--spec".into(), "s".into()]).is_ok());
        assert!(parse_args(vec![
            "stage".into(),
            "--host".into(),
            "atomicsoul".into(),
            "--spec".into(),
            "s".into()
        ])
        .is_ok());
        assert!(parse_args(vec![
            "qualify".into(),
            "--spec".into(),
            "s".into(),
            "--minimum-soak".into(),
            "6h".into()
        ])
        .is_ok());
        assert!(parse_args(vec![
            "canary".into(),
            "--host".into(),
            PUBLIC_HOST.into(),
            "--cohort".into(),
            "signed-cookie".into(),
            "--duration".into(),
            "1h".into(),
            "--spec".into(),
            "s".into()
        ])
        .is_ok());
        assert!(parse_args(vec![
            "build".into(),
            "--spec".into(),
            "s".into(),
            "--apply".into()
        ])
        .is_err());
        assert!(parse_args(vec![
            "publish".into(),
            "--spec".into(),
            "s".into(),
            "--action-envelope".into(),
            "e".into()
        ])
        .is_err());
        assert!(parse_args(vec![
            "canary".into(),
            "--host".into(),
            PUBLIC_HOST.into(),
            "--cohort".into(),
            "signed-cookie".into(),
            "--duration".into(),
            "59m".into(),
            "--spec".into(),
            "s".into()
        ])
        .is_err());
    }

    #[test]
    fn zero_and_floating_release_pins_are_rejected() {
        assert!(
            validate_optional_index(&json!(format!("{INDEX_PREFIX}{}", "0".repeat(64)))).is_err()
        );
        assert!(
            validate_optional_index(&json!("image.neverhuman.org/jain/appliance:latest")).is_err()
        );
        assert!(
            validate_optional_digest(&json!(format!("sha256:{}", "0".repeat(64))), true).is_err()
        );
        assert!(validate_optional_digest(&json!("a".repeat(64)), false).is_ok());
    }

    #[test]
    fn signatures_are_excluded_from_the_canonical_action_payload() {
        let base = json!({
            "schema_version": ACTION_SCHEMA,
            "release": RELEASE,
            "operation": "build",
            "authority_sha256": "a".repeat(64),
            "spec_sha256": "b".repeat(64),
            "plan_sha256": "c".repeat(64),
            "issued_unix_seconds": 10,
            "expires_unix_seconds": 20,
            "nonce": "1234567890abcdef",
            "signatures": [],
        });
        let before = canonical_json(&base).unwrap();
        let mut with_signatures = base;
        with_signatures["signatures"] = json!([{"untrusted": "bytes"}]);
        with_signatures["signatures"] = Value::Array(Vec::new());
        assert_eq!(before, canonical_json(&with_signatures).unwrap());
    }

    #[test]
    fn secure_read_rejects_hardlinked_documents() {
        let root = Temp::new("hardlink");
        let first = root.0.join("first");
        let second = root.0.join("second");
        fs::write(&first, b"bytes").unwrap();
        fs::hard_link(&first, &second).unwrap();
        assert!(secure_read(&first, 100).is_err());
    }

    #[test]
    fn canonical_plan_hash_changes_with_scope() {
        let base = || {
            json!({
                "release": RELEASE,
                "status": STATUS,
                "formal_ga": false,
                "operation": "stage",
                "authority_sha256": "a".repeat(64),
                "spec_sha256": "b".repeat(64),
            })
        };
        let mut first_input = base();
        first_input["scope"] = json!({"host": "atomicsoul"});
        first_input["mode"] = json!("dry-run");
        let mut second_input = base();
        second_input["scope"] = json!({"host": "other"});
        second_input["mode"] = json!("dry-run");
        let first = finish_plan(first_input).unwrap();
        let second = finish_plan(second_input).unwrap();
        assert_ne!(first["plan_sha256"], second["plan_sha256"]);
    }

    #[test]
    fn plan_hash_is_identical_for_dry_run_and_apply_mode() {
        let base = json!({
            "release": RELEASE,
            "status": STATUS,
            "formal_ga": false,
            "operation": "promote",
            "authority_sha256": "a".repeat(64),
            "spec_sha256": "b".repeat(64),
            "scope": {"host": PUBLIC_HOST, "expected_caddy_etag": "\"fresh\""},
            "ready": true,
            "blockers": [],
            "external_state_changed": false,
        });
        let mut dry = base.clone();
        dry["mode"] = json!("dry-run");
        let mut apply = base;
        apply["mode"] = json!("apply");
        assert_eq!(
            finish_plan(dry).unwrap()["plan_sha256"],
            finish_plan(apply).unwrap()["plan_sha256"]
        );
    }

    #[test]
    fn verifier_trait_is_injectable_without_launching_cosign() {
        AcceptVerifier
            .verify(b"key", b"payload", "signature")
            .unwrap();
    }

    #[test]
    fn action_envelope_binds_exact_plan_and_two_sorted_owner_keys() {
        let root = Temp::new("envelope");
        fs::create_dir(root.0.join("keys")).unwrap();
        let mut owner_keys = Vec::new();
        for (name, bytes) in [
            ("a.pub", b"owner-a".as_slice()),
            ("b.pub", b"owner-b".as_slice()),
        ] {
            fs::write(root.0.join("keys").join(name), bytes).unwrap();
            let digest = sha256(bytes);
            owner_keys.push(json!({
                "fingerprint": format!("sha256:{digest}"),
                "path": format!("keys/{name}"),
                "sha256": digest,
            }));
        }
        owner_keys.sort_by(|left, right| {
            left["fingerprint"]
                .as_str()
                .cmp(&right["fingerprint"].as_str())
        });
        let spec_sha = "b".repeat(64);
        let authority_sha = "a".repeat(64);
        let spec = LoadedSpec {
            path: root.0.join("release-spec.json"),
            value: json!({"action_policy": {"owner_keys": owner_keys}}),
            sha256: spec_sha.clone(),
            evidence_root: root.0.clone(),
        };
        let authority = Authority {
            path: root.0.join("authority.toml"),
            sha256: authority_sha.clone(),
            evidence_root: root.0.clone(),
            signature_max_age_seconds: 100,
        };
        let plan = finish_plan(json!({
            "release": RELEASE,
            "status": STATUS,
            "formal_ga": false,
            "operation": "build",
            "authority_sha256": authority_sha,
            "spec_sha256": spec_sha,
            "scope": {},
            "mode": "dry-run",
        }))
        .unwrap();
        let mut envelope = json!({
            "schema_version": ACTION_SCHEMA,
            "release": RELEASE,
            "operation": "build",
            "authority_sha256": authority.sha256,
            "spec_sha256": spec.sha256,
            "plan_sha256": plan["plan_sha256"],
            "issued_unix_seconds": 100,
            "expires_unix_seconds": 150,
            "nonce": "1234567890abcdef",
            "signatures": [],
        });
        let signed_over = sha256(&canonical_json(&envelope).unwrap());
        envelope["signatures"] = Value::Array(
            spec.value["action_policy"]["owner_keys"]
                .as_array()
                .unwrap()
                .iter()
                .map(|key| {
                    json!({
                        "signer_fingerprint": key["fingerprint"],
                        "signature": "accepted-by-test-verifier",
                        "signed_over_sha256": signed_over,
                    })
                })
                .collect(),
        );
        let envelope_path = root.0.join("envelope.json");
        fs::write(
            &envelope_path,
            serde_json::to_vec_pretty(&envelope).unwrap(),
        )
        .unwrap();
        let verified = load_and_verify_envelope(
            &envelope_path,
            &plan,
            &spec,
            &authority,
            &AcceptVerifier,
            120,
        )
        .unwrap();
        assert_eq!(verified.verified_signers.len(), 2);

        let mut other_plan = plan;
        other_plan["scope"] = json!({"unexpected": true});
        other_plan = finish_plan(other_plan).unwrap();
        assert!(load_and_verify_envelope(
            &envelope_path,
            &other_plan,
            &spec,
            &authority,
            &AcceptVerifier,
            120,
        )
        .is_err());
    }

    #[test]
    fn promotion_scope_requires_a_fresh_strong_etag_shape() {
        assert!(valid_strong_etag("\"fresh-etag\""));
        assert!(!valid_strong_etag("ef219b857dcfa6a2"));
        assert!(!valid_strong_etag("W/\"weak\""));
        assert!(!valid_strong_etag("\"contains\\\\escape\""));
    }
}
