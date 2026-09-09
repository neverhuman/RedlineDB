use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use serde::Deserialize;
use sha2::{Digest, Sha256};

const RECEIPT_PATH: &str = "reference/sqlite-parity/asset-receipt.toml";
const OWNED_PATH: &str = "reference/sqlite-parity/redline-core-owned-cases";
const LEGACY_PATH: &str = "reference/sqlite-parity/legacy-diagnostic-cases";
const OWNED_SOURCE_COMMIT: &str = "7ba2f349489448885be593261553c0bdd55821bd";
const LEGACY_SOURCE_COMMIT: &str = "53db3c5e08eb033b5a7d66720cd12cdfceb41eef";

#[derive(Debug, Deserialize)]
struct AssetReceipt {
    schema_version: String,
    classification: String,
    digest_algorithm: String,
    manifest: ManifestReceipt,
    snapshots: SnapshotReceipts,
}

#[derive(Debug, Deserialize)]
struct ManifestReceipt {
    path: String,
    case_count: usize,
    sha256: String,
}

#[derive(Debug, Deserialize)]
struct SnapshotReceipts {
    redline_core_owned: SnapshotReceipt,
    legacy_diagnostic: SnapshotReceipt,
}

#[derive(Debug, Deserialize)]
struct SnapshotReceipt {
    path: String,
    source_commit: String,
    case_count: usize,
    tree_sha256: String,
}

#[derive(Debug, Deserialize)]
struct ManifestCase {
    id: u64,
    folder: String,
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("bench crate must be two levels below the workspace root")
        .to_path_buf()
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn verify_snapshot(
    root: &Path,
    receipt: &SnapshotReceipt,
    manifest: &BTreeMap<String, u64>,
) -> String {
    let directory = root.join(&receipt.path);
    let mut entries = fs::read_dir(&directory)
        .unwrap_or_else(|error| panic!("read {}: {error}", directory.display()))
        .collect::<Result<Vec<_>, _>>()
        .unwrap_or_else(|error| panic!("enumerate {}: {error}", directory.display()));
    entries.sort_by_key(|entry| entry.file_name());

    let mut actual = BTreeMap::new();
    let mut digest = Sha256::new();

    for entry in entries {
        let file_type = entry
            .file_type()
            .unwrap_or_else(|error| panic!("inspect {}: {error}", entry.path().display()));
        assert!(
            file_type.is_file() && !file_type.is_symlink(),
            "reference snapshot contains a non-regular file: {}",
            entry.path().display()
        );

        let filename = entry.file_name().into_string().unwrap_or_else(|_| {
            panic!(
                "reference filename is not UTF-8: {}",
                entry.path().display()
            )
        });
        assert!(
            filename.ends_with(".rs"),
            "unexpected reference asset: {filename}"
        );

        let bytes = fs::read(entry.path())
            .unwrap_or_else(|error| panic!("read {}: {error}", entry.path().display()));
        let contents = std::str::from_utf8(&bytes)
            .unwrap_or_else(|error| panic!("decode {}: {error}", entry.path().display()));
        let mut lines = contents.lines();
        assert!(
            lines
                .next()
                .is_some_and(|line| line.starts_with("// Auto-generated SQLite parity case.")),
            "unexpected wrapper header in {filename}"
        );
        let source = lines
            .next()
            .and_then(|line| line.strip_prefix("// Source: "))
            .and_then(|line| line.split_whitespace().next())
            .unwrap_or_else(|| panic!("missing source folder in {filename}"));
        let id = *manifest
            .get(source)
            .unwrap_or_else(|| panic!("{filename} names unknown manifest folder {source}"));
        assert!(
            contents.contains(&format!("        id: {id},")),
            "{filename} does not contain manifest id {id}"
        );
        assert!(
            contents.contains(&format!("        folder: r\"{source}\",")),
            "{filename} does not contain manifest folder {source}"
        );
        assert!(
            actual.insert(source.to_owned(), id).is_none(),
            "duplicate manifest folder {source} in {}",
            receipt.path
        );

        digest.update(filename.as_bytes());
        digest.update([0]);
        digest.update(&bytes);
        digest.update([0]);
    }

    assert_eq!(
        actual.len(),
        receipt.case_count,
        "{} case count",
        receipt.path
    );
    assert_eq!(&actual, manifest, "{} manifest coverage", receipt.path);
    format!("{:x}", digest.finalize())
}

#[test]
fn reference_snapshots_match_manifest_and_receipt() {
    let root = workspace_root();
    let receipt_bytes = fs::read(root.join(RECEIPT_PATH)).expect("read reference asset receipt");
    let receipt: AssetReceipt =
        toml::from_str(std::str::from_utf8(&receipt_bytes).expect("receipt must be UTF-8"))
            .expect("parse reference asset receipt");

    assert_eq!(receipt.schema_version, "1.0.0");
    assert_eq!(receipt.classification, "historical-reference-evidence");
    assert_eq!(
        receipt.digest_algorithm,
        "sha256(sorted UTF-8 filename + NUL + file bytes + NUL)"
    );
    assert_eq!(
        receipt.manifest.path,
        "crates/bench/sqlite_parity/generated_manifest.json"
    );
    assert_eq!(receipt.snapshots.redline_core_owned.path, OWNED_PATH);
    assert_eq!(receipt.snapshots.legacy_diagnostic.path, LEGACY_PATH);
    assert_eq!(
        receipt.snapshots.redline_core_owned.source_commit,
        OWNED_SOURCE_COMMIT
    );
    assert_eq!(
        receipt.snapshots.legacy_diagnostic.source_commit,
        LEGACY_SOURCE_COMMIT
    );

    let manifest_bytes = fs::read(root.join(&receipt.manifest.path)).expect("read parity manifest");
    let cases: Vec<ManifestCase> =
        serde_json::from_slice(&manifest_bytes).expect("parse parity manifest");
    assert_eq!(cases.len(), receipt.manifest.case_count);

    let mut ids = BTreeSet::new();
    let mut manifest = BTreeMap::new();
    for case in cases {
        assert!(ids.insert(case.id), "duplicate manifest id {}", case.id);
        assert!(
            manifest.insert(case.folder.clone(), case.id).is_none(),
            "duplicate manifest folder {}",
            case.folder
        );
    }

    let owned_digest = verify_snapshot(&root, &receipt.snapshots.redline_core_owned, &manifest);
    let legacy_digest = verify_snapshot(&root, &receipt.snapshots.legacy_diagnostic, &manifest);
    assert_eq!(
        (sha256(&manifest_bytes), owned_digest, legacy_digest,),
        (
            receipt.manifest.sha256,
            receipt.snapshots.redline_core_owned.tree_sha256,
            receipt.snapshots.legacy_diagnostic.tree_sha256,
        )
    );
}

#[test]
fn runtime_and_release_use_canonical_sources() {
    let root = workspace_root();
    assert!(!root.join("crates/bench/sqlite_parity/cases").exists());
    assert!(
        !root
            .join("crates/bench/sqlite_parity/generated_cases")
            .exists()
    );
    assert!(!root.join("crates/ffi/include/sqlite3.h").exists());

    let catalog = fs::read_to_string(root.join("crates/bench/src/sqlite_parity/catalog.rs"))
        .expect("read parity runtime catalog");
    assert!(catalog.contains("include_str!(\"../../sqlite_parity/generated_manifest.json\")"));
    assert!(!catalog.contains("reference/sqlite-parity"));

    let release_build =
        fs::read_to_string(root.join("ops/ci/release-build.sh")).expect("read release build");
    assert!(release_build.contains("cp \"contracts/c-abi/sqlite3.h\" \"${PKG_DIR}/include/\""));
    assert!(!release_build.contains("crates/ffi/include/sqlite3.h"));

    let sqlite_header =
        fs::read_to_string(root.join("contracts/c-abi/sqlite3.h")).expect("read sqlite3 shim");
    assert!(sqlite_header.contains("#include \"redlinedb.h\""));
}

#[test]
fn release_packaging_honors_cargo_target_dir() {
    let root = workspace_root();
    let release_build =
        fs::read_to_string(root.join("ops/ci/release-build.sh")).expect("read release build");

    assert!(release_build.contains("TARGET_DIR=\"${CARGO_TARGET_DIR:-target}\""));
    assert!(release_build.contains("RELEASE_DIR=\"${TARGET_DIR}/${TARGET}/release\""));
    assert!(release_build.contains("cp \"${RELEASE_DIR}/redlinedb-cli\""));
    assert!(release_build.contains("cp \"${RELEASE_DIR}/libredlinedb.a\""));
    assert!(!release_build.contains("\"target/${TARGET}/release/"));
}
