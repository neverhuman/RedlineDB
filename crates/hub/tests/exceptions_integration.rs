/// Integration tests verifying the hub exception catalog is complete and internally consistent.
/// These tests are the proof lane for the typed exception surface.
///
/// Run with: cargo test --manifest-path crates/hub/Cargo.toml
/// Or (probe-only, no compile needed): `jankurai audit . --mode advisory`

#[path = "../src/exceptions.rs"]
mod exceptions;
use exceptions::*;

#[test]
fn catalog_covers_all_lib_sh_error_codes() {
    // ops/ci/lib.sh defines ERR_MISSING_TOOL=1 through ERR_SECRET_DETECTED=5.
    // The catalog must have an entry for each.
    let expected_purposes = [
        "locate required tool on PATH",
        "verify install.sh URL template is well-formed",
        "keep README/FAMILY.md/family.json in sync",
        "enforce thin-hub invariant (no engine source in hub repo)",
        "scan for accidentally committed secrets",
    ];
    assert_eq!(
        CATALOG.len(),
        expected_purposes.len(),
        "CATALOG length must match lib.sh ERR_* count"
    );
    for (ex, expected) in CATALOG.iter().zip(expected_purposes.iter()) {
        assert_eq!(
            ex.purpose, *expected,
            "catalog entry purpose mismatch — keep in sync with lib.sh"
        );
    }
}

#[test]
fn all_docs_urls_are_local() {
    for ex in CATALOG {
        assert!(
            ex.docs_url.starts_with("docs/"),
            "docs_url must point to a local docs/ path, not an external URL: {}",
            ex.docs_url
        );
    }
}
