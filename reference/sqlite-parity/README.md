# SQLite parity reference snapshots

This directory preserves two historical Rust-wrapper snapshots of the
1,127-case SQLite parity corpus. Neither snapshot is compiled or loaded at
runtime. The active bench catalog is
`crates/bench/sqlite_parity/generated_manifest.json`, embedded by
`crates/bench/src/sqlite_parity/catalog.rs`.

- `redline-core-owned-cases/` is the ownership snapshot introduced by commit
  `7ba2f349489448885be593261553c0bdd55821bd`.
- `legacy-diagnostic-cases/` is the earlier diagnostic snapshot, finalized by
  commit `53db3c5e08eb033b5a7d66720cd12cdfceb41eef` from wrappers originally
  introduced by commit `2f87d2c0b4ff55ac6ce1f3f566d4192dc3cba5fe`.

The snapshots have the same manifest IDs and folders. Their preserved
differences are the case-001 audit annotation and the diagnostic spelling and
source comment for case 150.

`asset-receipt.toml` records deterministic SHA-256 tree digests. The
`sqlite_parity_reference_assets` Rust integration test verifies both trees,
their provenance metadata, every manifest mapping, the runtime manifest
source, and release packaging of the canonical C ABI headers.
