use serde_json::Value;
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
    fs::write(&manifest, "release_version = \"8.0.1\"\n").expect("write manifest");
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
    fs::write(deploy.join("jain-split.lock.toml"), "release = \"8.0.1\"\n")
        .expect("write deploy lock fixture");
    let receipt = scratch.path().join("cutover.json");
    let script = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("ops/split/cutover.sh");
    let output = Command::new("bash")
        .arg(script)
        .args(["--dry-run", "--receipt"])
        .arg(&receipt)
        .env("JAIN_SPLIT_ROOT", &split)
        .env("JAIN_SPLIT_MANIFEST", &manifest)
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
    assert_eq!(report["external_mutations"], serde_json::json!([]));
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
