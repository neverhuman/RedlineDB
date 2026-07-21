use std::fs;
use std::path::Path;

#[test]
fn docker_topology_is_internal_pinned_and_socket_free() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let compose = fs::read_to_string(root.join("docker/compose.yaml")).unwrap();
    let dockerfile = fs::read_to_string(root.join("docker/Dockerfile")).unwrap();
    let images = fs::read_to_string(root.join("docker/images.lock.toml")).unwrap();

    assert_eq!(images.matches("@sha256:").count(), 2);
    assert!(!images.contains(":latest"));
    assert!(dockerfile.lines().next().unwrap().contains("@sha256:"));
    assert!(dockerfile.contains("WORKDIR /opt/redline/share"));
    assert!(!dockerfile.contains("RUN "));
    assert!(compose.contains("internal: true"));
    assert!(compose.contains("pull_policy: never"));
    assert!(compose.contains("read_only: true"));
    assert!(compose.contains("cap_drop:"));
    assert!(!compose.contains("network_mode: host"));
    assert!(!compose.contains("docker.sock"));
    assert!(
        !compose
            .lines()
            .any(|line| line.trim_start().starts_with("ports:"))
    );
    assert!(!compose.contains("build:"));
}

#[test]
fn github_workflows_are_not_parity_authorities() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let workflows = root.join(".github/workflows");
    if !workflows.exists() {
        return;
    }
    for entry in fs::read_dir(workflows).unwrap() {
        let path = entry.unwrap().path();
        let body = fs::read_to_string(path).unwrap();
        assert!(!body.contains("REDLINE_TESTING_POSTGRES_URL"));
        assert!(!body.contains("redline.docker-parity-evidence/v1"));
    }
}
