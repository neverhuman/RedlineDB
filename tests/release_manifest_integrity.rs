//! Integration test: when `just release-local` has produced a dist/<package>
//! directory, every file inside must be enumerated in release-manifest.json's
//! `artifact_hashes` with a correct SHA-256. This is the silent-omission
//! detector that prevents the release tarball from shipping a file that
//! isn't covered by the Sigstore attestation.
//!
//! Skips gracefully when the dist directory hasn't been produced yet (running
//! `cargo test` in a fresh checkout, before invoking `just release-local`).

use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn package_dir() -> PathBuf {
    repo_root().join(format!(
        "dist/redline-testing-{}-linux-x86_64",
        env!("CARGO_PKG_VERSION")
    ))
}

fn sha256_of(path: &Path) -> String {
    let bytes = fs::read(path).expect("read for hash");
    format!("{:x}", Sha256::digest(&bytes))
}

#[test]
fn release_manifest_enumerates_every_bundled_file() {
    let pkg = package_dir();
    if !pkg.is_dir() {
        eprintln!(
            "skipping release_manifest_integrity: no exact package directory {pkg:?}; \
             run `just release-local` first"
        );
        return;
    }
    let manifest_path = pkg.join("release-manifest.json");
    let manifest_raw = match fs::read_to_string(&manifest_path) {
        Ok(s) => s,
        Err(_) => {
            eprintln!("skipping: release-manifest.json absent in {pkg:?}");
            return;
        }
    };
    let manifest: serde_json::Value =
        serde_json::from_str(&manifest_raw).expect("parse release-manifest.json");
    let declared = manifest
        .get("artifact_hashes")
        .and_then(|v| v.as_object())
        .expect("release-manifest.json missing artifact_hashes object");
    assert_eq!(manifest["version"], env!("CARGO_PKG_VERSION"));
    assert!(
        manifest["release_commit"]
            .as_str()
            .is_some_and(|value| value.len() == 40)
    );
    assert!(
        manifest["release_tree"]
            .as_str()
            .is_some_and(|value| value.len() == 40)
    );
    assert!(
        manifest["source_archive_sha256"]
            .as_str()
            .is_some_and(|value| value.len() == 64)
    );
    assert!(matches!(
        manifest["release_tag_state"].as_str(),
        Some("planned" | "live")
    ));
    assert_eq!(
        manifest["release_tag"],
        format!(
            "redline-testing-v{}-jain.{}",
            env!("CARGO_PKG_VERSION"),
            manifest["tag_revision"]
        )
    );
    assert!(
        manifest["tag_revision"]
            .as_u64()
            .is_some_and(|value| value > 0),
        "release manifest requires a positive corrective tag revision"
    );

    let bin = pkg.join("bin");
    let bin_entries: Vec<_> = fs::read_dir(&bin)
        .expect("read closed bin inventory")
        .collect::<Result<_, _>>()
        .expect("read every bin entry");
    assert_eq!(bin_entries.len(), 1, "bin inventory must contain one entry");
    assert_eq!(
        bin_entries[0].file_name(),
        "redline-testing",
        "unexpected binary inventory member"
    );
    let binary_metadata = fs::symlink_metadata(bin.join("redline-testing"))
        .expect("stat bin/redline-testing without following links");
    assert!(
        binary_metadata.file_type().is_file(),
        "binary must be a physical regular file"
    );
    assert_eq!(binary_metadata.nlink(), 1, "binary must be single-link");

    // Walk every file under dist/<package>/ EXCEPT release-manifest.json itself
    // and the exactly-one binary (which is hashed via binary_sha256).
    let mut on_disk: Vec<(String, String)> = Vec::new();
    walk(&pkg, &pkg, &mut on_disk);
    for (rel, sha) in &on_disk {
        if rel == "release-manifest.json" || rel.starts_with("bin/") {
            continue;
        }
        let declared_sha = declared
            .get(rel)
            .and_then(|v| v.as_str())
            .unwrap_or_else(|| panic!("artifact_hashes missing entry for {rel}"));
        assert_eq!(
            declared_sha, sha,
            "SHA-256 mismatch for {rel}: declared={declared_sha} actual={sha}"
        );
    }
    // Inverse check: every declared file must exist on disk.
    for (declared_rel, declared_sha) in declared {
        let p = pkg.join(declared_rel);
        assert!(
            p.is_file(),
            "artifact_hashes declares {declared_rel} but file is missing in {pkg:?}"
        );
        let actual = sha256_of(&p);
        assert_eq!(
            actual,
            declared_sha.as_str().unwrap_or(""),
            "declared SHA for {declared_rel} differs from on-disk content"
        );
    }
}

fn walk(root: &Path, dir: &Path, acc: &mut Vec<(String, String)>) {
    for entry in fs::read_dir(dir).expect("read dist dir") {
        let entry = entry.expect("dist entry");
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path).expect("lstat dist entry");
        assert!(
            !metadata.file_type().is_symlink(),
            "symlink is forbidden in release inventory: {path:?}"
        );
        if metadata.is_dir() {
            walk(root, &path, acc);
        } else if metadata.is_file() {
            assert_eq!(
                metadata.nlink(),
                1,
                "hard link is forbidden in release inventory: {path:?}"
            );
            let rel = path
                .strip_prefix(root)
                .expect("strip prefix")
                .to_string_lossy()
                .into_owned();
            let sha = sha256_of(&path);
            acc.push((rel, sha));
        } else {
            panic!("special file is forbidden in release inventory: {path:?}");
        }
    }
}
