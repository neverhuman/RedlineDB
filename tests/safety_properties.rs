use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_CASE: AtomicU64 = AtomicU64::new(0);

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "splitctl-integration-{}-{}",
            std::process::id(),
            NEXT_CASE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).expect("create scratch directory");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn splitctl(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_splitctl"))
        .args(args)
        .output()
        .expect("run splitctl")
}

fn git(root: &Path, args: &[&str]) -> Output {
    Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .expect("run git")
}

fn bound_authority(path: &Path) -> toml::Value {
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("repos.manifest.toml");
    let mut authority: toml::Value = fs::read_to_string(source)
        .expect("read canonical authority")
        .parse()
        .expect("parse canonical authority");
    let redline = authority
        .get_mut("external_dependencies")
        .and_then(|value| value.get_mut("redline"))
        .and_then(toml::Value::as_table_mut)
        .expect("Redline authority table");
    redline.insert(
        "identity_status".to_owned(),
        toml::Value::String("bound".to_owned()),
    );
    redline.insert(
        "immutable_tag".to_owned(),
        toml::Value::String("redline-core-v4.1.0-jain.4".to_owned()),
    );
    let nested = authority
        .get_mut("nested_families")
        .and_then(|value| value.get_mut("redline"))
        .and_then(toml::Value::as_table_mut)
        .expect("nested Redline table");
    nested.insert(
        "engine_identity_status".to_owned(),
        toml::Value::String("bound".to_owned()),
    );
    nested.insert(
        "engine_tag".to_owned(),
        toml::Value::String("redline-core-v4.1.0-jain.4".to_owned()),
    );
    fs::write(
        path,
        toml::to_string_pretty(&authority).expect("render authority"),
    )
    .expect("write authority");
    authority
}

fn declared_test_remote(repo: &toml::Value) -> String {
    if let Some(remote) = repo.get("remote").and_then(toml::Value::as_str) {
        return remote.to_owned();
    }
    let slug = repo
        .get("jeryu_slug")
        .and_then(toml::Value::as_str)
        .expect("Jeryu slug");
    format!("http://127.0.0.1:8787/git/{slug}.git")
}

fn valid_deploy_lock(authority: &toml::Value, manifest: &Path) -> toml::Value {
    let digest = format!(
        "{:x}",
        Sha256::digest(fs::read(manifest).expect("read authority bytes"))
    );
    let family = authority
        .get("repo")
        .and_then(toml::Value::as_array)
        .expect("family repositories");
    let infrastructure = authority
        .get("infrastructure_repo")
        .and_then(toml::Value::as_array)
        .expect("infrastructure repositories");
    let pin = |repo: &toml::Value| {
        let name = repo.get("name").and_then(toml::Value::as_str).unwrap();
        let tag = repo
            .get("current_tag")
            .or_else(|| repo.get("immutable_tag"))
            .and_then(toml::Value::as_str)
            .unwrap();
        let mut table = toml::map::Map::new();
        table.insert("repo".to_owned(), toml::Value::String(name.to_owned()));
        table.insert("tag".to_owned(), toml::Value::String(tag.to_owned()));
        table.insert(
            "commit".to_owned(),
            toml::Value::String("1111111111111111111111111111111111111111".to_owned()),
        );
        table.insert(
            "jeryu".to_owned(),
            toml::Value::String(declared_test_remote(repo)),
        );
        table.insert(
            "required_check".to_owned(),
            toml::Value::String(
                repo.get("required_check")
                    .and_then(toml::Value::as_str)
                    .unwrap()
                    .to_owned(),
            ),
        );
        toml::Value::Table(table)
    };
    let mut root = toml::map::Map::new();
    root.insert(
        "schema_version".to_owned(),
        toml::Value::String("1.0.0".to_owned()),
    );
    root.insert("family".to_owned(), toml::Value::String("jain".to_owned()));
    root.insert(
        "release".to_owned(),
        toml::Value::String("8.0.1-split.0".to_owned()),
    );
    root.insert(
        "source_manifest_sha256".to_owned(),
        toml::Value::String(digest),
    );
    root.insert(
        "family_repo_count".to_owned(),
        toml::Value::Integer(family.len() as i64),
    );
    root.insert(
        "infrastructure_repo_count".to_owned(),
        toml::Value::Integer(infrastructure.len() as i64),
    );
    root.insert(
        "repo".to_owned(),
        toml::Value::Array(family.iter().map(pin).collect()),
    );
    let infrastructure_pins = infrastructure
        .iter()
        .map(|repo| {
            let mut pin = pin(repo);
            let table = pin.as_table_mut().unwrap();
            table.insert(
                "kind".to_owned(),
                toml::Value::String("required-infrastructure".to_owned()),
            );
            table.insert(
                "forge_owner".to_owned(),
                toml::Value::String("jain-split".to_owned()),
            );
            table.insert("family_registered".to_owned(), toml::Value::Boolean(true));
            pin
        })
        .collect();
    root.insert(
        "infrastructure_repo".to_owned(),
        toml::Value::Array(infrastructure_pins),
    );
    let commit = "2222222222222222222222222222222222222222";
    let mut redline = toml::map::Map::new();
    redline.insert(
        "family".to_owned(),
        toml::Value::String("redline-split".to_owned()),
    );
    redline.insert(
        "engine_tag".to_owned(),
        toml::Value::String("redline-core-v4.1.0-jain.4".to_owned()),
    );
    redline.insert(
        "engine_commit".to_owned(),
        toml::Value::String(commit.to_owned()),
    );
    redline.insert(
        "proof_lock_id".to_owned(),
        toml::Value::String(format!("redline-proof/v2/4.1.0/{commit}")),
    );
    let mut nested = toml::map::Map::new();
    nested.insert("redline".to_owned(), toml::Value::Table(redline));
    root.insert("nested".to_owned(), toml::Value::Table(nested));
    toml::Value::Table(root)
}

#[test]
fn property_rejects_unsafe_repository_slugs() {
    for slug in ["example", "../example", "owner/name/extra", "owner/na me"] {
        let output = splitctl(&["jeryu-local", "pr-ready", "--repo", slug, "--number", "1"]);
        assert!(!output.status.success(), "unsafe slug was accepted: {slug}");
    }
}

#[test]
fn dry_run_pr_lifecycle_writes_a_plan_without_credentials() {
    let scratch = Scratch::new();
    let receipt = scratch.path().join("ready.json");
    let output = splitctl(&[
        "jeryu-local",
        "pr-ready",
        "--repo",
        "jeryu/example",
        "--number",
        "7",
        "--receipt",
        receipt.to_str().expect("UTF-8 receipt path"),
    ]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value =
        serde_json::from_slice(&fs::read(receipt).expect("read receipt")).expect("parse receipt");
    assert_eq!(report["mode"], "dry-run");
    assert_eq!(report["action"], "would-apply");
    assert_eq!(report["request"]["body"]["draft"], false);
}

#[test]
fn non_owner_is_forbidden_from_planning_an_admin_bypass() {
    let rejected = splitctl(&[
        "jeryu-local",
        "protection-apply",
        "--repo",
        "jeryu/example",
        "--required-check",
        "example/required",
        "--enforce-admins=false",
    ]);
    assert!(!rejected.status.success());

    let scratch = Scratch::new();
    let receipt = scratch.path().join("protection.json");
    let planned = splitctl(&[
        "jeryu-local",
        "protection-apply",
        "--repo",
        "jeryu/example",
        "--required-check",
        "example/required",
        "--receipt",
        receipt.to_str().expect("UTF-8 receipt path"),
    ]);
    assert!(
        planned.status.success(),
        "{}",
        String::from_utf8_lossy(&planned.stderr)
    );
    let report: Value =
        serde_json::from_slice(&fs::read(receipt).expect("read receipt")).expect("parse receipt");
    assert_eq!(report["request"]["body"]["enforce_admins"], true);
}

#[test]
fn cli_reports_the_pinned_control_plane_version() {
    let output = splitctl(&["--version"]);
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "splitctl 0.1.0"
    );
}

#[test]
fn release_cargo_commands_exposes_the_canonical_feature_matrix() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("repos.manifest.toml");
    let output = splitctl(&[
        "release-cargo-commands",
        "--manifest",
        manifest.to_str().expect("UTF-8 manifest path"),
        "--repo",
        "jain-battle-gpu",
    ]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let policy: Value = serde_json::from_slice(&output.stdout).expect("parse release policy");
    assert_eq!(policy["mode"], "feature-matrix");
    assert_eq!(policy["commands"].as_array().unwrap().len(), 4);
    assert_eq!(policy["commands"][0]["label"], "build-feature-set-1");
    assert_eq!(policy["commands"][1]["label"], "test-feature-set-1");
    assert_eq!(
        policy["commands"][3]["args"][5],
        "battle-gpu/gpu-dynamic-linking"
    );
}

#[test]
fn cutover_receipt_derives_active_release_and_preserves_rollback() {
    let scratch = Scratch::new();
    let manifest = scratch.path().join("repos.manifest.toml");
    let authority = bound_authority(&manifest);
    let split = scratch.path().join("split");
    let deploy = split.join("jain-deploy");
    let binary = deploy.join("target/release/jain");
    fs::create_dir_all(binary.parent().unwrap()).expect("create binary directory");
    fs::write(&binary, "#!/usr/bin/env bash\nprintf 'jain 8.0.1\\n'\n")
        .expect("write fake release binary");
    let mut permissions = fs::metadata(&binary)
        .expect("binary metadata")
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&binary, permissions).expect("make binary executable");
    let lock = deploy.join("jain-split.lock.toml");
    fs::write(
        &lock,
        toml::to_string_pretty(&valid_deploy_lock(&authority, &manifest))
            .expect("render deploy lock"),
    )
    .expect("write deploy lock fixture");
    let receipt = scratch.path().join("cutover.json");
    let script = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("ops/split/cutover.sh");
    let output = Command::new("bash")
        .arg(script)
        .args(["--dry-run", "--receipt"])
        .arg(&receipt)
        .env("JAIN_SPLIT_ROOT", &split)
        .env("JAIN_SPLIT_MANIFEST", &manifest)
        .env("JAIN_SPLITCTL", env!("CARGO_BIN_EXE_splitctl"))
        .output()
        .expect("run cutover dry-run");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value =
        serde_json::from_slice(&fs::read(receipt).expect("read receipt")).expect("parse receipt");
    assert_eq!(report["release"], "8.0.1");
    assert_eq!(report["rollback_target"], "7.0.6");
    assert_eq!(report["redline"]["tag"], "redline-core-v4.1.0-jain.4");
    assert_eq!(
        report["redline"]["commit"],
        "2222222222222222222222222222222222222222"
    );
    assert_eq!(report["external_mutations"], serde_json::json!([]));
}

#[test]
fn deploy_lock_rejects_authority_pin_and_redline_mismatches() {
    let scratch = Scratch::new();
    let manifest = scratch.path().join("repos.manifest.toml");
    let authority = bound_authority(&manifest);
    let lock_path = scratch.path().join("jain-split.lock.toml");

    let mut cases = Vec::new();
    let mut digest = valid_deploy_lock(&authority, &manifest);
    digest.as_table_mut().unwrap().insert(
        "source_manifest_sha256".to_owned(),
        toml::Value::String("0".repeat(64)),
    );
    cases.push((digest, "source_manifest_sha256"));

    let mut pin = valid_deploy_lock(&authority, &manifest);
    pin.get_mut("repo")
        .and_then(toml::Value::as_array_mut)
        .unwrap()[0]
        .as_table_mut()
        .unwrap()
        .insert(
            "tag".to_owned(),
            toml::Value::String("jain-v8.0.1-split.9".to_owned()),
        );
    cases.push((pin, "lock tag does not match authority"));

    let mut infrastructure = valid_deploy_lock(&authority, &manifest);
    infrastructure
        .get_mut("infrastructure_repo")
        .and_then(toml::Value::as_array_mut)
        .unwrap()[0]
        .as_table_mut()
        .unwrap()
        .insert("family_registered".to_owned(), toml::Value::Boolean(false));
    cases.push((
        infrastructure,
        "infrastructure lock metadata is not fail-closed",
    ));

    let mut redline = valid_deploy_lock(&authority, &manifest);
    redline
        .get_mut("nested")
        .and_then(|value| value.get_mut("redline"))
        .and_then(toml::Value::as_table_mut)
        .unwrap()
        .insert(
            "engine_tag".to_owned(),
            toml::Value::String("redline-core-v4.1.0-jain.3".to_owned()),
        );
    cases.push((redline, "engine_tag must match authority"));

    for (invalid, expected) in cases {
        fs::write(
            &lock_path,
            toml::to_string_pretty(&invalid).expect("render invalid lock"),
        )
        .expect("write invalid lock");
        let output = splitctl(&[
            "validate-deploy-lock",
            "--manifest",
            manifest.to_str().unwrap(),
            "--lock",
            lock_path.to_str().unwrap(),
        ]);
        assert!(!output.status.success(), "invalid lock was accepted");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(expected),
            "expected {expected}, got {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn manifest_validation_rejects_a_non_maximal_release_feature_set() {
    let scratch = Scratch::new();
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("repos.manifest.toml");
    let canonical = fs::read_to_string(source).expect("read canonical manifest");
    let invalid = canonical.replace(
        "release_feature_sets = [\n  [\"gpu\", \"gpu-dynamic-loading\"],\n  [\"gpu-dynamic-linking\"],\n]",
        "release_feature_sets = [\n  [\"gpu\"],\n  [\"gpu\", \"gpu-dynamic-loading\"],\n]",
    );
    assert_ne!(canonical, invalid, "Battle matrix fixture must be replaced");
    let manifest = scratch.path().join("repos.manifest.toml");
    fs::write(&manifest, invalid).expect("write invalid manifest");

    let output = splitctl(&[
        "validate-manifest",
        "--manifest",
        manifest.to_str().expect("UTF-8 manifest path"),
    ]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("is not maximal"));
}

#[test]
fn bare_mirror_refresh_is_dry_run_by_default_and_verifies_applied_refs() {
    let scratch = Scratch::new();
    let source = scratch.path().join("source");
    let split_root = scratch.path().join("split");
    fs::create_dir_all(&source).expect("create source");
    assert!(git(&source, &["init", "-b", "main"]).status.success());
    fs::write(source.join("README.md"), "reviewed\n").expect("write fixture");
    assert!(git(&source, &["add", "README.md"]).status.success());
    assert!(git(
        &source,
        &[
            "-c",
            "user.name=Split Test",
            "-c",
            "user.email=split-test@example.invalid",
            "commit",
            "-m",
            "reviewed",
        ],
    )
    .status
    .success());
    assert!(git(&source, &["tag", "example-v8.0.0-split.0"])
        .status
        .success());

    let manifest = scratch.path().join("repos.manifest.toml");
    fs::write(
        &manifest,
        format!(
            "split_root = {:?}\n\n[[repo]]\nname = \"example\"\npath = {:?}\nprofile = \"rust\"\n",
            split_root, source
        ),
    )
    .expect("write manifest");
    let dry_receipt = scratch.path().join("mirror-dry.json");
    let dry = splitctl(&[
        "refresh-bare-mirrors",
        "--manifest",
        manifest.to_str().expect("UTF-8 manifest"),
        "--receipt",
        dry_receipt.to_str().expect("UTF-8 receipt"),
    ]);
    assert!(
        dry.status.success(),
        "{}",
        String::from_utf8_lossy(&dry.stderr)
    );
    assert!(!split_root.join("target/bare-mirrors/example.git").exists());
    let dry_report: Value =
        serde_json::from_slice(&fs::read(dry_receipt).expect("read dry-run mirror receipt"))
            .expect("parse dry-run mirror receipt");
    assert_eq!(dry_report["mode"], "dry-run");
    assert_eq!(dry_report["repositories"][0]["action"], "would-create");

    let apply_receipt = scratch.path().join("mirror-apply.json");
    let applied = splitctl(&[
        "refresh-bare-mirrors",
        "--manifest",
        manifest.to_str().expect("UTF-8 manifest"),
        "--receipt",
        apply_receipt.to_str().expect("UTF-8 receipt"),
        "--apply",
    ]);
    assert!(
        applied.status.success(),
        "{}",
        String::from_utf8_lossy(&applied.stderr)
    );
    let apply_report: Value =
        serde_json::from_slice(&fs::read(apply_receipt).expect("read applied mirror receipt"))
            .expect("parse applied mirror receipt");
    assert_eq!(apply_report["status"], "pass");
    assert_eq!(apply_report["repositories"][0]["refs_verified"], true);
    let mirror = split_root.join("target/bare-mirrors/example.git");
    let source_head =
        String::from_utf8(git(&source, &["rev-parse", "HEAD"]).stdout).expect("source head UTF-8");
    let mirror_head = String::from_utf8(git(&mirror, &["rev-parse", "refs/heads/main"]).stdout)
        .expect("mirror head UTF-8");
    assert_eq!(source_head.trim(), mirror_head.trim());
}
