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
fn jankurai_lane_routes_deletions_and_classifies_existing_paths() {
    let lane = repo_file("ops/ci/jankurai.sh");

    assert!(lane.contains("proof . --changed-from \"$base_ref\""));
    assert!(lane.contains("git diff --diff-filter=d --name-only -z"));
    assert!(lane.contains("proofbind verify . \"${proofbind_changed_args[@]}\""));
    assert!(lane.contains("proofmark rust . \"${proofbind_changed_args[@]}\""));
    assert!(lane.contains("fail \"proofbind has no existing changed paths to verify\""));
}
