use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Deserialize)]
struct Lockfile {
    package: Vec<LockedPackage>,
}

#[derive(Debug, Deserialize)]
struct LockedPackage {
    name: String,
    version: String,
    source: Option<String>,
    checksum: Option<String>,
}

#[derive(Debug, Serialize)]
struct DependencyIdentity {
    name: String,
    version: String,
    sha256: String,
    archive: String,
}

#[derive(Debug, Serialize)]
struct ToolIdentity {
    path: String,
    sha256: String,
    version: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ArtifactIdentity {
    path: String,
    sha256: String,
}

#[derive(Debug, Serialize)]
struct CustodyReceipt {
    schema_version: &'static str,
    status: &'static str,
    artifact_path: String,
    artifact_sha256: String,
    cargo_lock_sha256: String,
    cargo_home: String,
    dependency_count: usize,
    dependency_closure_sha256: String,
    dependencies: Vec<DependencyIdentity>,
    rustc: ToolIdentity,
    cargo: ToolIdentity,
    oracles: BTreeMap<&'static str, ToolIdentity>,
    build_command: &'static str,
    network_policy: &'static str,
}

pub(crate) struct StageRequest<'a> {
    pub(crate) repo_root: &'a Path,
    pub(crate) artifact: &'a Path,
    pub(crate) source_cargo_home: &'a Path,
    pub(crate) cargo_home: &'a Path,
    pub(crate) sqlite_bin: &'a Path,
    pub(crate) postgres_client_bin: &'a Path,
    pub(crate) postgres_server_bin: &'a Path,
    pub(crate) out_dir: &'a Path,
}

pub(crate) fn stage(request: StageRequest<'_>) -> Result<()> {
    let StageRequest {
        repo_root,
        artifact,
        source_cargo_home,
        cargo_home,
        sqlite_bin,
        postgres_client_bin,
        postgres_server_bin,
        out_dir,
    } = request;
    let workspace = workspace_root(repo_root)?;
    let artifact_before = physical_artifact_identity(artifact, &workspace)?;
    let source_cargo_home = fs::canonicalize(source_cargo_home)?;
    let selected = resolved_registry_packages(repo_root, &source_cargo_home)?;
    let cargo_home = prepare_output(cargo_home, &workspace)?;
    let lock_path = repo_root.join("Cargo.lock");
    stage_dependency_custody(&selected, &lock_path, &source_cargo_home, &cargo_home)?;
    let out_dir = prepare_output(out_dir, &workspace)?;
    let dependencies = dependency_closure(&lock_path, &cargo_home, &selected)?;
    let resolved_from_custody = resolved_registry_packages(repo_root, &cargo_home)?;
    ensure!(
        resolved_from_custody == selected,
        "staged Cargo custody resolves a different host closure"
    );

    let oracle_dir = out_dir.join("oracles");
    fs::create_dir_all(&oracle_dir)?;
    let sqlite = stage_tool(sqlite_bin, &oracle_dir.join("sqlite3"), &["--version"])?;
    let postgres_client = stage_tool(
        postgres_client_bin,
        &oracle_dir.join("psql"),
        &["--version"],
    )?;
    let postgres_server = stage_tool(
        postgres_server_bin,
        &oracle_dir.join("postgres"),
        &["--version"],
    )?;

    let cargo_path = resolve_executable("cargo")?;
    let rustc_path = resolve_executable("rustc")?;
    let cargo = tool_identity(&cargo_path, &["-V"])?;
    let rustc = tool_identity(&rustc_path, &["-vV"])?;
    let status = Command::new(&cargo_path)
        .current_dir(repo_root)
        .args(["build", "--workspace", "--locked", "--offline"])
        .env("CARGO_HOME", &cargo_home)
        .env("CARGO_NET_OFFLINE", "true")
        .status()
        .context("run offline custody build")?;
    ensure!(
        status.success(),
        "offline custody build failed with {status}"
    );
    let artifact_after = physical_artifact_identity(artifact, &workspace)?;
    ensure!(
        artifact_after == artifact_before,
        "release artifact identity changed during offline custody build"
    );

    let dependency_closure_sha256 = dependency_digest(&dependencies);
    let mut oracles = BTreeMap::new();
    oracles.insert("postgres_client", postgres_client);
    oracles.insert("postgres_server", postgres_server);
    oracles.insert("sqlite", sqlite);
    let receipt = CustodyReceipt {
        schema_version: "redline.custody-receipt/v1",
        status: "pass",
        artifact_path: artifact_after.path,
        artifact_sha256: artifact_after.sha256,
        cargo_lock_sha256: sha256_file(&lock_path)?,
        cargo_home: cargo_home.display().to_string(),
        dependency_count: dependencies.len(),
        dependency_closure_sha256,
        dependencies,
        rustc,
        cargo,
        oracles,
        build_command: "cargo build --workspace --locked --offline",
        network_policy: "CARGO_NET_OFFLINE=true; dependency and oracle bytes are in-tree",
    };
    let receipt_path = out_dir.join("custody-receipt.json");
    let mut body = serde_json::to_vec_pretty(&receipt)?;
    body.push(b'\n');
    write_new_receipt(&receipt_path, &body)?;
    println!("custody receipt: {}", receipt_path.display());
    Ok(())
}

fn dependency_closure(
    lock_path: &Path,
    cargo_home: &Path,
    selected: &BTreeMap<(String, String), String>,
) -> Result<Vec<DependencyIdentity>> {
    let lock: Lockfile = toml::from_str(&fs::read_to_string(lock_path)?)?;
    let cache_root = cargo_home.join("registry/cache");
    let mut archives = Vec::new();
    collect_regular_files(&cache_root, &mut archives)?;
    let mut result = Vec::new();
    for package in lock.package.into_iter().filter(|package| {
        package
            .source
            .as_deref()
            .is_some_and(|source| source.starts_with("registry+"))
            && selected.contains_key(&(package.name.clone(), package.version.clone()))
    }) {
        let expected = package
            .checksum
            .as_deref()
            .context("registry package is missing its lockfile checksum")?;
        ensure!(
            selected
                .get(&(package.name.clone(), package.version.clone()))
                .is_some_and(|checksum| checksum == expected),
            "Cargo metadata and lockfile checksum disagree for {} {}",
            package.name,
            package.version
        );
        let filename = format!("{}-{}.crate", package.name, package.version);
        let candidates = archives
            .iter()
            .filter(|path| {
                path.file_name()
                    .is_some_and(|name| name == filename.as_str())
            })
            .collect::<Vec<_>>();
        ensure!(
            !candidates.is_empty(),
            "offline Cargo custody is missing {filename}"
        );
        let mut matching = Vec::new();
        for candidate in candidates {
            let actual = sha256_file(candidate)?;
            ensure!(
                actual == expected,
                "Cargo custody checksum mismatch for {}: expected {expected}, found {actual}",
                candidate.display()
            );
            matching.push(candidate);
        }
        matching.sort();
        let archive = matching[0];
        result.push(DependencyIdentity {
            name: package.name,
            version: package.version,
            sha256: expected.to_owned(),
            archive: archive.display().to_string(),
        });
    }
    result.sort_by(|left, right| {
        (&left.name, &left.version, &left.sha256).cmp(&(&right.name, &right.version, &right.sha256))
    });
    ensure!(
        !result.is_empty(),
        "Cargo.lock has no registry dependency closure"
    );
    Ok(result)
}

fn stage_dependency_custody(
    selected: &BTreeMap<(String, String), String>,
    lock_path: &Path,
    source_home: &Path,
    destination_home: &Path,
) -> Result<()> {
    let source_cache = source_home.join("registry/cache");
    let source_index = source_home.join("registry/index");
    let mut cache_files = Vec::new();
    collect_regular_files(&source_cache, &mut cache_files)?;
    let mut index_files = Vec::new();
    collect_regular_files(&source_index, &mut index_files)?;

    for ((name, version), checksum) in selected {
        let archive_name = format!("{name}-{version}.crate");
        let archive = cache_files
            .iter()
            .find(|path| {
                path.file_name()
                    .is_some_and(|file| file == archive_name.as_str())
            })
            .with_context(|| format!("source Cargo cache lacks {archive_name}"))?;
        ensure!(
            sha256_file(archive)? == *checksum,
            "source cache checksum mismatch for {archive_name}"
        );
        let relative = archive.strip_prefix(source_home)?;
        copy_physical(archive, &destination_home.join(relative))?;
    }

    let lock: Lockfile = toml::from_str(&fs::read_to_string(lock_path)?)?;
    let locked_names = lock
        .package
        .into_iter()
        .filter(|package| {
            package
                .source
                .as_deref()
                .is_some_and(|source| source.starts_with("registry+"))
        })
        .map(|package| package.name)
        .collect::<std::collections::BTreeSet<_>>();
    for name in locked_names {
        let index = index_files
            .iter()
            .find(|path| {
                path.file_name().is_some_and(|file| file == name.as_str())
                    && path
                        .components()
                        .any(|component| component.as_os_str() == ".cache")
            })
            .with_context(|| format!("source sparse index lacks {name}"))?;
        let relative = index.strip_prefix(source_home)?;
        copy_physical(index, &destination_home.join(relative))?;
    }
    for config in index_files
        .iter()
        .filter(|path| path.file_name().is_some_and(|name| name == "config.json"))
    {
        let relative = config.strip_prefix(source_home)?;
        copy_physical(config, &destination_home.join(relative))?;
    }
    Ok(())
}

fn copy_physical(source: &Path, destination: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(source)?;
    ensure!(
        metadata.is_file() && !metadata.file_type().is_symlink(),
        "custody source is not a physical regular file: {}",
        source.display()
    );
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::copy(source, destination)?;
    Ok(())
}

fn resolved_registry_packages(
    repo_root: &Path,
    cargo_home: &Path,
) -> Result<BTreeMap<(String, String), String>> {
    let lock: Lockfile = toml::from_str(&fs::read_to_string(repo_root.join("Cargo.lock"))?)?;
    let lock_checksums = lock
        .package
        .into_iter()
        .filter_map(|package| {
            package
                .checksum
                .map(|checksum| ((package.name, package.version), checksum))
        })
        .collect::<BTreeMap<_, _>>();
    let rustc = resolve_executable("rustc")?;
    let rustc_output = Command::new(rustc).arg("-vV").output()?;
    ensure!(rustc_output.status.success(), "rustc -vV failed");
    let rustc_version = format!(
        "{}{}",
        String::from_utf8_lossy(&rustc_output.stdout),
        String::from_utf8_lossy(&rustc_output.stderr)
    );
    let host = rustc_version
        .lines()
        .find_map(|line| line.strip_prefix("host: "))
        .context("rustc -vV lacks host triple")?
        .to_owned();
    let cargo = resolve_executable("cargo")?;
    let output = Command::new(cargo)
        .current_dir(repo_root)
        .args([
            "metadata",
            "--format-version=1",
            "--locked",
            "--offline",
            "--filter-platform",
            &host,
        ])
        .env("CARGO_HOME", cargo_home)
        .env("CARGO_NET_OFFLINE", "true")
        .output()
        .context("resolve host Cargo dependency closure")?;
    ensure!(
        output.status.success(),
        "offline cargo metadata failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let metadata: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    let selected_ids = metadata
        .pointer("/resolve/nodes")
        .and_then(|value| value.as_array())
        .context("cargo metadata lacks resolve nodes")?
        .iter()
        .filter_map(|node| node.get("id").and_then(|value| value.as_str()))
        .collect::<std::collections::BTreeSet<_>>();
    let mut selected = BTreeMap::new();
    for package in metadata
        .get("packages")
        .and_then(|value| value.as_array())
        .context("cargo metadata lacks packages")?
    {
        let id = package
            .get("id")
            .and_then(|value| value.as_str())
            .unwrap_or_default();
        let source = package
            .get("source")
            .and_then(|value| value.as_str())
            .unwrap_or_default();
        if !selected_ids.contains(id) || !source.starts_with("registry+") {
            continue;
        }
        let name = package
            .get("name")
            .and_then(|value| value.as_str())
            .context("metadata package lacks name")?;
        let version = package
            .get("version")
            .and_then(|value| value.as_str())
            .context("metadata package lacks version")?;
        let key = (name.to_owned(), version.to_owned());
        let checksum = lock_checksums
            .get(&key)
            .with_context(|| format!("Cargo.lock lacks checksum for {name} {version}"))?;
        selected.insert(key, checksum.to_owned());
    }
    ensure!(
        !selected.is_empty(),
        "host Cargo dependency closure is empty"
    );
    Ok(selected)
}

fn dependency_digest(dependencies: &[DependencyIdentity]) -> String {
    let mut hasher = Sha256::new();
    for dependency in dependencies {
        hasher.update(dependency.name.as_bytes());
        hasher.update([0]);
        hasher.update(dependency.version.as_bytes());
        hasher.update([0]);
        hasher.update(dependency.sha256.as_bytes());
        hasher.update([0]);
    }
    format!("{:x}", hasher.finalize())
}

fn stage_tool(source: &Path, destination: &Path, version_args: &[&str]) -> Result<ToolIdentity> {
    let metadata = fs::symlink_metadata(source)
        .with_context(|| format!("inspect oracle artifact {}", source.display()))?;
    ensure!(
        metadata.is_file() && !metadata.file_type().is_symlink(),
        "oracle artifact must be a physical regular file: {}",
        source.display()
    );
    fs::copy(source, destination).with_context(|| {
        format!(
            "stage oracle artifact {} -> {}",
            source.display(),
            destination.display()
        )
    })?;
    tool_identity(destination, version_args)
}

fn tool_identity(path: &Path, version_args: &[&str]) -> Result<ToolIdentity> {
    let output = Command::new(path)
        .args(version_args)
        .output()
        .with_context(|| format!("capture version from {}", path.display()))?;
    ensure!(
        output.status.success(),
        "{} version command failed",
        path.display()
    );
    let mut version = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if version.is_empty() {
        version = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    }
    Ok(ToolIdentity {
        path: fs::canonicalize(path)?.display().to_string(),
        sha256: sha256_file(path)?,
        version,
    })
}

fn prepare_output(path: &Path, workspace: &Path) -> Result<PathBuf> {
    fs::create_dir_all(path)?;
    let path = fs::canonicalize(path)?;
    let metadata = fs::symlink_metadata(&path)?;
    ensure!(metadata.is_dir() && !metadata.file_type().is_symlink());
    ensure!(
        path.starts_with(workspace),
        "custody output must remain in-tree"
    );
    require_tree(&path, workspace, "custody output")
}

fn require_tree(path: &Path, workspace: &Path, label: &str) -> Result<PathBuf> {
    let path = fs::canonicalize(path).with_context(|| format!("resolve {label}"))?;
    ensure!(path.starts_with(workspace), "{label} must remain in-tree");
    let mut files = Vec::new();
    collect_regular_files(&path, &mut files)?;
    Ok(path)
}

fn physical_artifact_identity(artifact: &Path, workspace: &Path) -> Result<ArtifactIdentity> {
    ensure!(
        artifact.is_absolute(),
        "release artifact path must be absolute"
    );
    let workspace = fs::canonicalize(workspace).context("resolve custody workspace")?;
    ensure!(
        artifact.starts_with(&workspace),
        "release artifact must remain in-tree"
    );

    let relative = artifact
        .strip_prefix(&workspace)
        .context("resolve release artifact beneath workspace")?;
    let mut cursor = workspace.clone();
    for component in relative.components() {
        let std::path::Component::Normal(name) = component else {
            bail!("release artifact path contains a non-normal component");
        };
        cursor.push(name);
        let metadata = fs::symlink_metadata(&cursor)
            .with_context(|| format!("inspect release artifact path {}", cursor.display()))?;
        ensure!(
            !metadata.file_type().is_symlink(),
            "release artifact path contains a symlink: {}",
            cursor.display()
        );
    }

    let metadata = fs::symlink_metadata(artifact)
        .with_context(|| format!("inspect release artifact {}", artifact.display()))?;
    ensure!(
        metadata.is_file() && !metadata.file_type().is_symlink(),
        "release artifact must be a physical regular file"
    );
    ensure!(
        metadata.nlink() == 1,
        "release artifact must have exactly one hard link"
    );
    let canonical = fs::canonicalize(artifact)?;
    ensure!(
        canonical.starts_with(&workspace),
        "release artifact resolves outside the custody workspace"
    );
    Ok(ArtifactIdentity {
        path: canonical.display().to_string(),
        sha256: sha256_file(&canonical)?,
    })
}

fn write_new_receipt(path: &Path, body: &[u8]) -> Result<()> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .with_context(|| format!("create new custody receipt {}", path.display()))?;
    file.write_all(body)?;
    file.sync_all()?;
    let metadata = file.metadata()?;
    ensure!(
        metadata.is_file() && metadata.nlink() == 1,
        "custody receipt must be a physical single-link regular file"
    );
    Ok(())
}

fn collect_regular_files(path: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() {
        bail!("custody tree contains a symlink: {}", path.display());
    }
    if metadata.is_file() {
        files.push(path.to_path_buf());
        return Ok(());
    }
    ensure!(
        metadata.is_dir(),
        "custody tree contains a special node: {}",
        path.display()
    );
    for entry in fs::read_dir(path)? {
        collect_regular_files(&entry?.path(), files)?;
    }
    Ok(())
}

fn workspace_root(repo_root: &Path) -> Result<PathBuf> {
    repo_root
        .parent()
        .and_then(Path::parent)
        .context("resolve jain-split workspace")?
        .canonicalize()
        .context("canonicalize jain-split workspace")
}

fn resolve_executable(name: &str) -> Result<PathBuf> {
    for directory in std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()) {
        let candidate = directory.join(name);
        if candidate.is_file() {
            // Preserve the argv[0] tool name for rustup-style dispatchers.
            // The receipt still hashes the canonical backing executable.
            return Ok(candidate);
        }
    }
    bail!("required executable is unavailable: {name}")
}

fn sha256_file(path: &Path) -> Result<String> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    std::io::copy(&mut file, &mut hasher)?;
    Ok(format!("{:x}", hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

    struct Scratch(PathBuf);

    impl Scratch {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "redline-testing-custody-artifact-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .expect("clock")
                    .as_nanos()
            ));
            fs::create_dir(&path).expect("scratch directory");
            Self(path)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn dependency_digest_is_order_sensitive_and_stable() {
        let dependency = DependencyIdentity {
            name: "demo".into(),
            version: "1.2.3".into(),
            sha256: "a".repeat(64),
            archive: "/ignored/demo.crate".into(),
        };
        assert_eq!(dependency_digest(&[dependency]).len(), 64);
    }

    #[test]
    fn release_artifact_identity_rejects_foreign_linked_and_symlinked_files() {
        let scratch = Scratch::new();
        let workspace = scratch.0.join("workspace");
        fs::create_dir(&workspace).unwrap();
        let artifact = workspace.join("redline-testing.tar.gz");
        fs::write(&artifact, b"reviewed artifact\n").unwrap();

        let identity = physical_artifact_identity(&artifact, &workspace).unwrap();
        assert_eq!(identity.path, artifact.display().to_string());
        assert_eq!(identity.sha256, sha256_file(&artifact).unwrap());

        let hard_link = workspace.join("hard-link.tar.gz");
        fs::hard_link(&artifact, &hard_link).unwrap();
        assert!(physical_artifact_identity(&artifact, &workspace).is_err());
        fs::remove_file(&hard_link).unwrap();

        let symlink_path = workspace.join("symlink.tar.gz");
        symlink(&artifact, &symlink_path).unwrap();
        assert!(physical_artifact_identity(&symlink_path, &workspace).is_err());

        let foreign = scratch.0.join("foreign.tar.gz");
        fs::write(&foreign, b"foreign artifact\n").unwrap();
        assert!(physical_artifact_identity(&foreign, &workspace).is_err());
    }

    #[test]
    fn custody_receipt_creation_refuses_existing_and_symlink_paths() {
        let scratch = Scratch::new();
        let receipt = scratch.0.join("custody-receipt.json");
        write_new_receipt(&receipt, b"{\"status\":\"pass\"}\n").unwrap();
        assert!(write_new_receipt(&receipt, b"replacement\n").is_err());
        assert_eq!(fs::read(&receipt).unwrap(), b"{\"status\":\"pass\"}\n");

        let target = scratch.0.join("foreign.json");
        fs::write(&target, b"foreign\n").unwrap();
        let linked = scratch.0.join("linked-receipt.json");
        symlink(&target, &linked).unwrap();
        assert!(write_new_receipt(&linked, b"replacement\n").is_err());
        assert_eq!(fs::read(&target).unwrap(), b"foreign\n");
    }
}
