use std::{fs, path::PathBuf};

fn repo_file(path: &str) -> String {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    fs::read_to_string(root.join(path)).unwrap_or_else(|error| {
        panic!("failed to read {path}: {error}");
    })
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
fn required_lane_runs_security_with_strict_tool_checks() {
    let required = repo_file("ops/ci/pr-ci.sh");

    assert!(
        required
            .contains("ci_run env REDLINE_STRICT_TOOLS=1 bash \"$repo_root/ops/ci/security.sh\"")
    );
}
