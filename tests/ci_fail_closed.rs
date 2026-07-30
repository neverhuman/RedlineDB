use std::{fs, path::PathBuf, process::Command};

fn repo_file(path: &str) -> String {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    fs::read_to_string(root.join(path)).unwrap_or_else(|error| {
        panic!("failed to read {path}: {error}");
    })
}

#[test]
fn control_plane_test_maps_are_identical() {
    assert_eq!(
        repo_file(".jankurai/test-map.json"),
        repo_file("agent/test-map.json")
    );
}

#[test]
fn audit_policy_mirrors_bind_the_governed_floor_and_tool() {
    let canonical = repo_file(".jankurai/audit-policy.toml");
    assert_eq!(canonical, repo_file("agent/audit-policy.toml"));

    for declaration in [
        "mode = \"advisory\"",
        "minimum_score = 85",
        "required_tool = \"jankurai\"",
        "required_tool_version = \"1.6.11\"",
    ] {
        assert_eq!(
            canonical
                .lines()
                .filter(|line| *line == declaration)
                .count(),
            1,
            "audit policy must contain exactly one `{declaration}` declaration"
        );
    }

    let baseline: serde_json::Value =
        serde_json::from_str(&repo_file(".jankurai/baselines/accepted-baseline.json"))
            .expect("valid accepted baseline");
    assert!(baseline["score"].as_u64().is_some_and(|score| score >= 86));
    assert_eq!(baseline["decision"]["minimum_score"].as_u64(), Some(85));
    assert_eq!(baseline["decision"]["hard_findings"].as_u64(), Some(0));
    assert_eq!(
        baseline["decision"]["ratchet"]["allowed_drop"].as_u64(),
        Some(0)
    );
    assert_eq!(baseline["caps_applied"].as_array().map(Vec::len), Some(0));
    assert_eq!(
        baseline["decision"]["ratchet"]["new_caps"]
            .as_array()
            .map(Vec::len),
        Some(0)
    );
    assert_eq!(
        baseline["decision"]["ratchet"]["new_hard_findings"]
            .as_array()
            .map(Vec::len),
        Some(0)
    );
}

#[test]
fn agent_proof_lanes_cover_every_test_route() {
    let test_map: serde_json::Value =
        serde_json::from_str(&repo_file("agent/test-map.json")).expect("valid agent test map");
    let tests = test_map["tests"]
        .as_object()
        .expect("agent test map has a tests object");
    let proof_lanes = repo_file("agent/proof-lanes.toml");

    assert!(proof_lanes.contains("[[lane]]"));
    for (route, spec) in tests {
        let command = spec["command"]
            .as_str()
            .unwrap_or_else(|| panic!("test route {route} has a command"));
        let declaration = format!("command = {}", serde_json::to_string(command).unwrap());
        assert!(
            proof_lanes.lines().any(|line| line == declaration),
            "test route {route} is missing from agent/proof-lanes.toml"
        );
    }
}

#[test]
fn retired_gitlab_pipeline_stays_absent() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    assert!(!root.join(".gitlab-ci.yml").exists());
}

#[test]
fn security_workflow_is_blocking_and_delegates_to_the_local_lane() {
    let workflow = repo_file(".github/workflows/security-audit.yml");

    assert!(workflow.contains("pull_request:"));
    assert!(workflow.contains("merge_group:"));
    assert!(!workflow.contains("continue-on-error"));
    assert!(workflow.contains("REDLINE_STRICT_TOOLS: \"1\""));
    assert!(workflow.contains("run: bash ops/ci/security.sh"));
    assert!(workflow.contains("run: bash ops/ci/jankurai-audit.sh"));
}

#[test]
fn release_workflow_delegates_to_the_canonical_ops_lanes() {
    let workflow = repo_file(".github/workflows/release.yml");

    assert!(workflow.contains("redline-testing-v*-jain.*"));
    assert!(!workflow.contains("- \"v*\""));
    assert!(workflow.contains("validate-release-tag --tag \"$GITHUB_REF_NAME\""));
    assert!(workflow.contains("run: bash ops/ci/pr-ci.sh"));
    assert!(workflow.contains("run: bash ops/ci/release.sh"));
}

#[test]
fn badge_lane_uses_the_tested_rust_updater() {
    let workflow = repo_file(".github/workflows/ci.yml");

    assert!(workflow.contains("cargo run --locked --quiet -p xtask -- update-badge"));
    assert!(!workflow.contains("scripts/update-badge.py"));
}

#[test]
fn repository_has_no_python_runtime_surface() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let output = Command::new("git")
        .args(["ls-files", "-z"])
        .current_dir(&root)
        .output()
        .expect("run git ls-files");
    assert!(output.status.success(), "git ls-files must succeed");

    for relative in output.stdout.split(|byte| *byte == 0) {
        if relative.is_empty() {
            continue;
        }
        let relative = std::str::from_utf8(relative).expect("tracked path is UTF-8");
        let path = root.join(relative);
        if !path.is_file() {
            continue;
        }
        assert_ne!(
            path.extension().and_then(|extension| extension.to_str()),
            Some("py"),
            "tracked Python file remains: {relative}"
        );

        if relative.starts_with("ops/")
            || relative.starts_with("scripts/")
            || relative.starts_with(".github/workflows/")
        {
            let Ok(body) = fs::read_to_string(&path) else {
                continue;
            };
            for invocation in ["python3", "python -", "env python"] {
                assert!(
                    !body.contains(invocation),
                    "Python runtime invocation `{invocation}` remains in {relative}"
                );
            }
        }
    }
}

#[test]
fn required_lane_runs_security_with_strict_tool_checks() {
    let required = repo_file("ops/ci/pr-ci.sh");

    assert!(
        required
            .contains("ci_run env REDLINE_STRICT_TOOLS=1 bash \"$repo_root/ops/ci/security.sh\"")
    );
}

#[test]
fn local_dispatcher_exposes_the_standard_release_lanes() {
    let dispatcher = repo_file("scripts/ci-local.sh");

    for (lane, script) in [
        ("score", "ops/ci/score.sh"),
        ("contract-drift", "ops/ci/contract-drift.sh"),
        ("artifact-support", "ops/ci/artifact_support.sh"),
    ] {
        assert!(
            dispatcher.contains(&format!("{lane})")),
            "dispatcher is missing the {lane} route"
        );
        assert!(
            dispatcher.contains(&format!("bash \"$repo_root/{script}\"")),
            "dispatcher does not delegate {lane} to {script}"
        );
        assert!(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join(script)
                .is_file(),
            "release lane script is missing: {script}"
        );
    }
}

#[test]
fn score_lane_is_version_pinned_and_fail_closed() {
    let lane = repo_file("ops/ci/score.sh");

    for invariant in [
        "jankurai 1.6.11",
        "--mode ratchet",
        "--baseline .jankurai/baselines/accepted-baseline.json",
        "--policy agent/audit-policy.toml",
        ".score >= 85",
        ".decision.status == \"pass\"",
        ".decision.hard_findings == 0",
        ".decision.ratchet.allowed_drop == 0",
        ".decision.ratchet.passed == true",
        ".caps_applied | type == \"array\" and length == 0",
        "(.blockers // []) | type == \"array\" and length == 0",
    ] {
        assert!(lane.contains(invariant), "score lane lost `{invariant}`");
    }
    assert!(!lane.contains("|| true"));
}

#[test]
fn contract_drift_lane_binds_the_exact_release_package() {
    let lane = repo_file("ops/ci/contract-drift.sh");

    for invariant in [
        "expected_version=\"1.0.1\"",
        "REDLINE_TESTING_RELEASE_TAG",
        "git ls-files 'schemas/*.json'",
        "scripts/release-package.sh",
        "verify-release-inventory.sh",
        "validate-release-manifest.sh",
        "release_manifest_integrity",
        "sha256sum -c \"$2\"",
        "release package is not reproducible",
        "release inventory mismatch",
        "redline.testing.contract-drift/v2",
        "release_tag_state",
        "source_archive_sha256",
    ] {
        assert!(
            lane.contains(invariant),
            "contract-drift lane lost `{invariant}`"
        );
    }
    assert!(!lane.contains("|| true"));
}

#[test]
fn artifact_support_is_unsigned_network_free_review_evidence() {
    let lane = repo_file("ops/ci/artifact_support.sh");

    for forbidden in [
        "SIGNRAIL",
        "SignRail",
        "ED25519_SEED",
        "cargo install",
        "https://github.com",
        "sign-release",
    ] {
        assert!(
            !lane.contains(forbidden),
            "artifact-support must not contain `{forbidden}`"
        );
    }
    for deterministic in [
        "source-identity.sh snapshot",
        "source-identity.sh verify",
        "git show -s --format=%cI HEAD",
        "git show -s --format=%ct HEAD",
        "tar --sort=name",
        "gzip -n",
    ] {
        assert!(
            lane.contains(deterministic),
            "artifact-support lost `{deterministic}`"
        );
    }
    assert!(lane.contains("source-identity.json"));
}

#[test]
fn release_package_requires_bound_identity_and_closed_inventory() {
    let lane = repo_file("scripts/release-package.sh");

    for invariant in [
        "set GITHUB_REF_NAME or REDLINE_TESTING_RELEASE_TAG explicitly",
        "source-identity.sh snapshot",
        "source-identity.sh verify",
        "release-tag-identity.sh",
        "verify-release-inventory.sh",
        "release-source-projection.sh",
        "validate-release-manifest.sh",
        "env -i",
        "CARGO_ENCODED_RUSTFLAGS",
        "--remap-path-prefix=",
        "release-path-remap/v1",
        "sanitized-no-local",
        "build_inputs",
        "release_tree",
        "release_tag_state",
        "source_archive_sha256",
    ] {
        assert!(
            lane.contains(invariant),
            "release package lost `{invariant}`"
        );
    }
    assert!(!lane.contains("jain.1}}"));
}

#[test]
fn required_lane_runs_hostile_release_fixtures() {
    let required = repo_file("ops/ci/pr-ci.sh");
    assert!(required.contains("tests/release_lanes_hostile.sh"));
}

#[test]
fn one_command_verify_runs_complete_release_proof() {
    let verify = repo_file("justfile");

    for lane in [
        "ops/ci/pr-ci.sh",
        "ops/ci/contract-drift.sh",
        "ops/ci/artifact_support.sh",
        "ops/ci/jankurai.sh",
        "ops/ci/score.sh",
    ] {
        assert!(verify.contains(lane), "verify target lost `{lane}`");
    }
    assert!(verify.contains("REDLINE_STRICT_TOOLS=1"));
    assert!(verify.contains("REDLINE_TESTING_RELEASE_TAG="));
}

#[test]
fn jankurai_lane_routes_existing_paths_from_diffs_or_clean_snapshots() {
    let lane = repo_file("ops/ci/jankurai.sh");

    assert!(lane.contains("--full"));
    assert!(lane.contains("--mode ratchet"));
    assert!(lane.contains("--baseline .jankurai/baselines/accepted-baseline.json"));
    assert!(lane.contains("--policy agent/audit-policy.toml"));
    assert!(lane.contains("--no-score-history"));
    assert!(lane.contains("proof . \"${proofbind_changed_args[@]}\""));
    assert!(lane.contains("git diff --diff-filter=d --name-only -z"));
    assert!(lane.contains("git diff-tree --no-commit-id --name-only -r -z --diff-filter=d HEAD"));
    assert!(lane.contains("proofbind verify . \"${proofbind_changed_args[@]}\""));
    assert!(lane.contains("proofmark rust . \"${proofbind_changed_args[@]}\""));
    assert!(lane.contains("fail \"proofbind has no existing changed paths to verify\""));
}
