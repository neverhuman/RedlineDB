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
        "bash scripts/ci-family.sh all",
        "jankurai security run . --strict --profile ci --script ops/ci/security-family.sh",
        "bash ops/ci/audit-family.sh",
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
        "tools/security-lane.sh",
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

    let workflow = read(".github/workflows/ci.yml");
    assert!(workflow.contains("cargo install cargo-deny --locked --version 0.19.8"));
    let audit = read("ops/ci/audit-family.sh");
    assert!(audit.contains("--mode ratchet --baseline"));
    assert!(audit.contains(".jankurai/baselines/main.repo-score.json"));
    assert!(!audit.contains("--mode advisory"));
    assert!(workflow.contains("RedlineDB/required"));
    assert!(workflow.contains("all(.value.result == \"success\")"));
    assert!(!workflow.contains("cargo-deny --locked --version 0.18.0"));

    let security_marker = read("tools/security-lane.sh");
    assert!(security_marker.contains("bash \"$ROOT/ops/ci/security-family.sh\""));
    assert!(security_marker.contains("bash \"$ROOT/ops/ci/dependency-review.sh\""));
    assert!(!security_marker.contains("soft-gated"));
    assert!(!security_marker.contains("ci-soft-gate-ledger"));
}

#[test]
fn operator_surfaces_route_to_the_protected_release_contract() {
    let justfile = read("justfile");
    assert!(justfile.contains("\nrequired:\n"));
    assert!(justfile.contains("./scripts/ci-local.sh required"));

    let readme = read("README.md");
    for route in [
        "`just required`",
        "docs/architecture.md",
        "docs/testing.md",
        "docs/release.md",
    ] {
        assert!(
            readme.contains(route),
            "README must route operators to {route}"
        );
    }

    let release = read("docs/release.md");
    assert!(release.contains("full-graph dependency review"));
    assert!(release.contains("none is advisory or soft-gated"));

    assert!(repository_root().join("agent/boundaries.toml").is_file());
    assert!(!repository_root().join(".jankurai/boundaries.toml").exists());
    assert_eq!(
        read("agent/generated-zones.toml"),
        read(".jankurai/generated-zones.toml"),
        "current and compatibility generated-zone manifests must be byte-identical"
    );
    for path in [
        ".jankurai/JANKURAI_STANDARD.md",
        "docs/architecture.md",
        "docs/architecture/ENGINEERING_SPEC.md",
        "docs/boundaries.md",
    ] {
        assert!(
            !read(path).contains(".jankurai/boundaries.toml"),
            "{path} must route to the canonical agent boundary manifest"
        );
    }
}
