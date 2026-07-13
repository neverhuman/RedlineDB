use std::{fs, path::PathBuf};

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(path: &str) -> String {
    let path = repository_root().join(path);
    fs::read_to_string(&path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
}

#[test]
fn protected_required_lane_runs_every_hard_gate_in_order() {
    let required = read("ops/ci/pr-ci.sh");
    let mut remainder = required.as_str();

    for command in [
        "bash ops/ci/fast.sh",
        "bash ops/ci/security.sh",
        "bash ops/ci/dependency-review.sh",
        "bash ops/ci/jankurai-audit.sh",
    ] {
        let offset = remainder
            .find(command)
            .unwrap_or_else(|| panic!("protected required lane is missing `{command}`"));
        remainder = &remainder[offset + command.len()..];
    }

    assert!(
        !required.contains("|| true"),
        "protected required lane must not absorb a gate failure"
    );
    assert!(
        !required.contains("--mode advisory"),
        "protected required lane must not substitute an advisory audit"
    );
}

#[test]
fn release_security_surfaces_have_no_active_soft_gate() {
    for path in [
        "ops/ci/security.sh",
        "ops/ci/dependency-review.sh",
        "ops/ci/jankurai-audit.sh",
        "ops/ci/lib.sh",
    ] {
        assert!(
            !read(path).contains("ci_soft_gate"),
            "{path} must not invoke or define a soft-gate escape"
        );
    }

    let ledger = read(".jankurai/ci-soft-gate-ledger.toml");
    assert!(
        !ledger.contains("[[entry]]"),
        "release security policy must not retain an active soft-gate row"
    );

    let workflow = read(".github/workflows/jankurai.yml");
    assert!(workflow.contains("cargo install cargo-deny --locked --version 0.19.8"));
    assert!(workflow.contains(
        "jankurai audit . --mode ratchet --baseline target/jankurai/accepted-baseline.json"
    ));
    assert!(!workflow.contains("cargo-deny --locked --version 0.18.0"));
}
