use std::collections::BTreeSet;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::custody;

const RECEIPT_SCHEMA: &str = "redline.docker-custody-receipt/v1";
const IMAGE_SCHEMA: &str = "redline.docker-images/v1";

#[derive(Debug, Deserialize)]
struct ImageManifest {
    schema_version: String,
    runner_base: LockedImage,
    postgres: LockedImage,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct LockedImage {
    reference: String,
    image_id: String,
}

#[derive(Debug, Serialize)]
struct RuntimeArtifact {
    kind: String,
    path: String,
    sha256: String,
}

#[allow(clippy::too_many_arguments)]
pub fn stage(
    repo_root: &Path,
    source_cargo_home: &Path,
    source_custody_sha256: &str,
    cargo_home: &Path,
    sqlite_bin: &Path,
    psql_bin: &Path,
    postgres_bin: &Path,
    target_bin: &Path,
    core_commit: &str,
    core_tree: &str,
    image_manifest_path: &Path,
    out_dir: &Path,
) -> Result<()> {
    validate_sha1(core_commit, "Core commit")?;
    validate_sha1(core_tree, "Core tree")?;
    ensure!(
        source_custody_sha256.len() == 64
            && source_custody_sha256
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase()),
        "source Cargo custody SHA-256 is malformed"
    );
    ensure!(
        !out_dir.exists(),
        "Docker stage output must not already exist"
    );
    let workspace = workspace_root(repo_root)?;
    ensure!(
        out_dir.starts_with(&workspace),
        "Docker stage output must remain in-tree"
    );
    fs::create_dir_all(out_dir)?;
    let out_dir = fs::canonicalize(out_dir)?;
    let cargo_custody_dir = out_dir.join("cargo-custody");
    custody::stage(
        repo_root,
        source_cargo_home,
        cargo_home,
        sqlite_bin,
        psql_bin,
        postgres_bin,
        &cargo_custody_dir,
    )?;

    let cargo = resolve_executable("cargo")?;
    let build = Command::new(&cargo)
        .current_dir(repo_root)
        .args([
            "build",
            "--release",
            "--locked",
            "--offline",
            "-p",
            "redline-testing",
        ])
        .env("CARGO_HOME", cargo_home)
        .env("CARGO_NET_OFFLINE", "true")
        .status()
        .context("build release Docker parity runner offline")?;
    ensure!(build.success(), "offline release runner build failed");

    let image_manifest_path = if image_manifest_path.is_absolute() {
        image_manifest_path.to_path_buf()
    } else {
        repo_root.join(image_manifest_path)
    };
    let image_manifest_body = fs::read_to_string(&image_manifest_path)?;
    let images = validate_image_manifest(&image_manifest_body)?;
    verify_local_image(&images.runner_base)?;
    verify_local_image(&images.postgres)?;

    let context = out_dir.join("context");
    let rootfs = context.join("rootfs");
    let bin_dir = rootfs.join("opt/redline/bin");
    let share_dir = rootfs.join("opt/redline/share");
    let custody_dir = share_dir.join("custody");
    fs::create_dir_all(&bin_dir)?;
    fs::create_dir_all(&custody_dir)?;

    let runner_source = repo_root.join("target/release/redline-testing");
    let runner = bin_dir.join("redline-testing");
    let target = bin_dir.join("redlinedb");
    let sqlite = bin_dir.join("sqlite3");
    let psql = bin_dir.join("psql");
    copy_executable(&runner_source, &runner)?;
    copy_executable(target_bin, &target)?;
    copy_executable(sqlite_bin, &sqlite)?;
    copy_executable(psql_bin, &psql)?;

    let mut libraries = BTreeSet::new();
    for binary in [&runner_source, target_bin, sqlite_bin, psql_bin] {
        libraries.extend(ldd_closure(binary)?);
    }
    for library in libraries {
        let relative = library
            .strip_prefix("/")
            .with_context(|| format!("ELF dependency is not absolute: {}", library.display()))?;
        copy_physical(&library, &rootfs.join(relative))?;
    }

    for (source, relative) in [
        (repo_root.join("contracts"), "contracts"),
        (repo_root.join("corpus"), "corpus"),
        (repo_root.join("metadata"), "metadata"),
        (repo_root.join("schemas"), "schemas"),
        (repo_root.join("docker"), "docker"),
    ] {
        copy_tree(&source, &share_dir.join(relative))?;
    }
    copy_physical(
        &repo_root.join("docker/Dockerfile"),
        &context.join("Dockerfile"),
    )?;

    let cargo_receipt_source = cargo_custody_dir.join("custody-receipt.json");
    let cargo_receipt = custody_dir.join("cargo-custody-receipt.json");
    copy_physical(&cargo_receipt_source, &cargo_receipt)?;
    let cargo_receipt_value: Value = serde_json::from_slice(&fs::read(&cargo_receipt)?)?;

    let testing_commit = git(repo_root, &["rev-parse", "HEAD"])?;
    let testing_tree = git(repo_root, &["rev-parse", "HEAD^{tree}"])?;
    validate_sha1(&testing_commit, "Testing commit")?;
    validate_sha1(&testing_tree, "Testing tree")?;
    let docker_engine = docker_engine_identity()?;
    let image_manifest_runtime = share_dir.join("docker/images.lock.toml");
    let dockerfile_runtime = share_dir.join("docker/Dockerfile");
    let compose_runtime = share_dir.join("docker/compose.yaml");
    let contracts = json!({
        "manifest_sha256": sha256_file(&share_dir.join("contracts/compatibility-v1.toml"))?,
        "sqlite_corpus_sha256": tree_digest(&share_dir.join("corpus/sqlite_parity"))?,
        "postgres_corpus_sha256": sha256_file(&share_dir.join("corpus/beyond_sqlite/generated_manifest.json"))?,
        "postgres_exclusions_sha256": sha256_file(&share_dir.join("metadata/beyond_sqlite/skip-list.toml"))?,
    });

    let mut artifacts = runtime_artifacts(&rootfs)?;
    artifacts.sort_by(|left, right| left.path.cmp(&right.path));
    let receipt = json!({
        "schema_version": RECEIPT_SCHEMA,
        "status": "pass",
        "provenance_source": "local-jeryu",
        "testing": {
            "commit": testing_commit,
            "tree": testing_tree,
            "binary": {"path": "/opt/redline/bin/redline-testing", "sha256": sha256_file(&runner)?},
        },
        "core": {
            "commit": core_commit,
            "tree": core_tree,
            "binary": {"path": "/opt/redline/bin/redlinedb", "sha256": sha256_file(&target)?},
        },
        "images": {
            "manifest": {"path": "/opt/redline/share/docker/images.lock.toml", "sha256": sha256_file(&image_manifest_runtime)?},
            "runner_base": images.runner_base,
            "postgres": images.postgres,
        },
        "docker_engine": docker_engine,
        "docker_inputs": {
            "dockerfile": {"path": "/opt/redline/share/docker/Dockerfile", "sha256": sha256_file(&dockerfile_runtime)?},
            "compose": {"path": "/opt/redline/share/docker/compose.yaml", "sha256": sha256_file(&compose_runtime)?},
        },
        "cargo_custody": {
            "path": "/opt/redline/share/custody/cargo-custody-receipt.json",
            "sha256": sha256_file(&cargo_receipt)?,
            "cargo_lock_sha256": cargo_receipt_value.get("cargo_lock_sha256"),
            "dependency_closure_sha256": cargo_receipt_value.get("dependency_closure_sha256"),
            "rustc": cargo_receipt_value.get("rustc"),
            "cargo": cargo_receipt_value.get("cargo"),
            "source_cargo_custody_sha256": source_custody_sha256,
        },
        "contracts": contracts,
        "runtime_artifacts": artifacts,
        "network_policy": "local image inspect only; no registry lookup or pull; Cargo locked+offline; image build must use --network=none --pull=false",
    });
    let receipt_path = custody_dir.join("docker-custody-receipt.json");
    write_checksummed_json(&receipt_path, &receipt)?;
    require_symlink_free(&context)?;
    println!("Docker context: {}", context.display());
    println!("Docker custody receipt: {}", receipt_path.display());
    Ok(())
}

fn validate_image_manifest(body: &str) -> Result<ImageManifest> {
    let manifest: ImageManifest = toml::from_str(body)?;
    ensure!(
        manifest.schema_version == IMAGE_SCHEMA,
        "unknown Docker image manifest schema"
    );
    for (name, image) in [
        ("runner_base", &manifest.runner_base),
        ("postgres", &manifest.postgres),
    ] {
        ensure!(
            image.reference.matches("@sha256:").count() == 1,
            "{name} image reference is not digest-qualified"
        );
        ensure!(
            !image.reference.contains(":latest") && !image.reference.ends_with("latest"),
            "{name} image reference uses latest"
        );
        let digest = image
            .reference
            .split_once("@sha256:")
            .map(|(_, digest)| digest)
            .context("image reference lacks digest")?;
        ensure!(
            digest.len() == 64
                && digest
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase()),
            "{name} image digest is malformed"
        );
        ensure!(
            image.image_id == format!("sha256:{digest}"),
            "{name} image ID must equal its locked repository digest"
        );
    }
    Ok(manifest)
}

fn verify_local_image(image: &LockedImage) -> Result<()> {
    let output = Command::new("docker")
        .args(["image", "inspect", "--format", "{{.Id}}", &image.reference])
        .output()
        .with_context(|| format!("inspect local Docker image {}", image.reference))?;
    ensure!(
        output.status.success(),
        "locked Docker image is not present locally: {}",
        image.reference
    );
    let actual = String::from_utf8(output.stdout)?.trim().to_owned();
    ensure!(
        actual == image.image_id,
        "local Docker image identity mismatch for {}: expected={} actual={actual}",
        image.reference,
        image.image_id
    );
    Ok(())
}

fn docker_engine_identity() -> Result<Value> {
    let output = Command::new("docker")
        .args(["version", "--format", "{{json .}}"])
        .output()
        .context("capture Docker engine identity")?;
    ensure!(output.status.success(), "docker version failed");
    let value: Value = serde_json::from_slice(&output.stdout)?;
    Ok(json!({
        "client_version": value.pointer("/Client/Version").and_then(Value::as_str).context("Docker client version is missing")?,
        "client_commit": value.pointer("/Client/GitCommit").and_then(Value::as_str).context("Docker client commit is missing")?,
        "server_version": value.pointer("/Server/Version").and_then(Value::as_str).context("Docker server version is missing")?,
        "server_commit": value.pointer("/Server/GitCommit").and_then(Value::as_str).context("Docker server commit is missing")?,
        "server_arch": value.pointer("/Server/Arch").and_then(Value::as_str).context("Docker server arch is missing")?,
    }))
}

fn ldd_closure(binary: &Path) -> Result<BTreeSet<PathBuf>> {
    let output = Command::new("ldd")
        .arg(binary)
        .output()
        .with_context(|| format!("inspect ELF closure for {}", binary.display()))?;
    ensure!(
        output.status.success(),
        "ldd failed for {}: {}",
        binary.display(),
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout)?;
    parse_ldd(&text)
}

fn parse_ldd(text: &str) -> Result<BTreeSet<PathBuf>> {
    ensure!(!text.contains("not found"), "ELF dependency is unavailable");
    let mut paths = BTreeSet::new();
    for line in text.lines() {
        let trimmed = line.trim();
        let candidate = if let Some((_, value)) = trimmed.split_once("=>") {
            value.split_whitespace().next().unwrap_or_default()
        } else {
            trimmed.split_whitespace().next().unwrap_or_default()
        };
        if candidate.starts_with('/') {
            paths.insert(PathBuf::from(candidate));
        }
    }
    ensure!(!paths.is_empty(), "ELF shared-library closure is empty");
    Ok(paths)
}

fn runtime_artifacts(rootfs: &Path) -> Result<Vec<RuntimeArtifact>> {
    let mut files = Vec::new();
    collect_files(rootfs, &mut files)?;
    let mut artifacts = Vec::new();
    for path in files {
        let relative = path.strip_prefix(rootfs)?;
        let runtime_path = Path::new("/").join(relative);
        let kind = if runtime_path.starts_with("/opt/redline/bin") {
            "binary"
        } else if runtime_path.starts_with("/opt/redline/share") {
            "custody-input"
        } else {
            "elf-library"
        };
        artifacts.push(RuntimeArtifact {
            kind: kind.to_owned(),
            path: runtime_path.display().to_string(),
            sha256: sha256_file(&path)?,
        });
    }
    Ok(artifacts)
}

fn copy_executable(source: &Path, destination: &Path) -> Result<()> {
    copy_physical(source, destination)?;
    fs::set_permissions(destination, fs::Permissions::from_mode(0o555))?;
    Ok(())
}

fn copy_physical(source: &Path, destination: &Path) -> Result<()> {
    let resolved = fs::canonicalize(source)
        .with_context(|| format!("resolve custody input {}", source.display()))?;
    let metadata = fs::symlink_metadata(&resolved)?;
    ensure!(
        metadata.is_file(),
        "custody input is not a regular file: {}",
        source.display()
    );
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::copy(&resolved, destination)?;
    Ok(())
}

fn copy_tree(source: &Path, destination: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(source)?;
    ensure!(
        metadata.is_dir() && !metadata.file_type().is_symlink(),
        "Docker custody source is not a physical directory: {}",
        source.display()
    );
    fs::create_dir_all(destination)?;
    let mut entries = fs::read_dir(source)?.collect::<std::io::Result<Vec<_>>>()?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        let target = destination.join(entry.file_name());
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() {
            bail!(
                "Docker custody source contains a symlink: {}",
                path.display()
            );
        }
        if metadata.is_dir() {
            copy_tree(&path, &target)?;
        } else if metadata.is_file() {
            copy_physical(&path, &target)?;
        } else {
            bail!(
                "Docker custody source contains a special node: {}",
                path.display()
            );
        }
    }
    Ok(())
}

fn tree_digest(root: &Path) -> Result<String> {
    let mut files = Vec::new();
    collect_files(root, &mut files)?;
    files.sort();
    let mut hasher = Sha256::new();
    for path in files {
        hasher.update(path.strip_prefix(root)?.as_os_str().as_encoded_bytes());
        hasher.update([0]);
        hasher.update(fs::read(path)?);
        hasher.update([0]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn collect_files(path: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() {
        bail!("Docker stage contains a symlink: {}", path.display());
    }
    if metadata.is_file() {
        files.push(path.to_path_buf());
        return Ok(());
    }
    ensure!(
        metadata.is_dir(),
        "Docker stage contains a special node: {}",
        path.display()
    );
    for entry in fs::read_dir(path)? {
        collect_files(&entry?.path(), files)?;
    }
    Ok(())
}

fn require_symlink_free(path: &Path) -> Result<()> {
    let mut files = Vec::new();
    collect_files(path, &mut files)
}

fn write_checksummed_json(path: &Path, value: &Value) -> Result<()> {
    let mut body = serde_json::to_vec_pretty(value)?;
    body.push(b'\n');
    fs::write(path, body)?;
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("receipt.json");
    fs::write(
        path.with_file_name(format!("{name}.sha256")),
        format!("{}  {name}\n", sha256_file(path)?),
    )?;
    Ok(())
}

fn sha256_file(path: &Path) -> Result<String> {
    Ok(format!("{:x}", Sha256::digest(fs::read(path)?)))
}

fn validate_sha1(value: &str, label: &str) -> Result<()> {
    ensure!(
        value.len() == 40
            && value
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase()),
        "{label} is not a lowercase full SHA"
    );
    Ok(())
}

fn git(root: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()?;
    ensure!(
        output.status.success(),
        "Git command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(String::from_utf8(output.stdout)?.trim().to_owned())
}

fn resolve_executable(name: &str) -> Result<PathBuf> {
    for directory in std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()) {
        let candidate = directory.join(name);
        if candidate.is_file() {
            return Ok(candidate);
        }
    }
    bail!("required executable is unavailable: {name}")
}

fn workspace_root(repo_root: &Path) -> Result<PathBuf> {
    repo_root
        .parent()
        .and_then(Path::parent)
        .context("resolve jain-split workspace")?
        .canonicalize()
        .context("canonicalize jain-split workspace")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locked_images_reject_latest_and_digest_mismatch() {
        let good = r#"
schema_version = "redline.docker-images/v1"
[runner_base]
reference = "ubuntu@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
image_id = "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
[postgres]
reference = "postgres@sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
image_id = "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
"#;
        assert!(validate_image_manifest(good).is_ok());
        assert!(
            validate_image_manifest(&good.replace(
                "ubuntu@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "ubuntu:latest"
            ))
            .is_err()
        );
        assert!(validate_image_manifest(&good.replace(
            "image_id = \"sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\"",
            "image_id = \"sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc\""
        )).is_err());
    }

    #[test]
    fn ldd_parser_rejects_missing_and_collects_loader() {
        let parsed = parse_ldd(
            "libc.so.6 => /lib/x86_64-linux-gnu/libc.so.6 (0x1)\n/lib64/ld-linux-x86-64.so.2 (0x2)\n",
        )
        .unwrap();
        assert!(parsed.contains(Path::new("/lib/x86_64-linux-gnu/libc.so.6")));
        assert!(parsed.contains(Path::new("/lib64/ld-linux-x86-64.so.2")));
        assert!(parse_ldd("libmissing.so => not found\n").is_err());
    }
}
