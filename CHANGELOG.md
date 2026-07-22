# Changelog

This project follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
and uses [Semantic Versioning](https://semver.org/spec/v2.0.0.html). The
release surface is the locally attested runner tarball; see
[docs/release.md](docs/release.md) for the protected Jeryu flow.

## [Unreleased]

### Added

- Versioned SQLite/PostgreSQL compatibility contracts, deterministic selectors,
  strict release evidence, and the compatibility major gate.
- Offline dependency/oracle custody receipts for the local release path.

### Changed

- Bound Jain release packaging to the Cargo product version through the Rust
  `xtask validate-release-tag` gate and immutable corrective tag scheme.
- Removed GitHub release/download/attestation dependencies from the
  authoritative build and release path.
- Removed the remaining Python CI helper in favor of the tested Rust xtask
  implementation.
- Required CI now anchors compatibility counts to the reviewed v1 contract;
  release evidence requires the complete case selection, and custody receipts
  bind the exact physical packaged artifact.

## [1.0.1] - 2026-05-26

### Fixed

- Restored the GitHub CI mirror and GitHub release workflow alongside the
  GitLab validation pipeline.
- Re-aligned the release docs, ownership maps, and CI guardrails so the
  public release host and local proof surface are explicit again.

## [1.0.0] - 2026-05-25

### Added — exhaustive corpus expansion

- **24 hand-authored SQLite-parity shards** under `corpus/sqlite_parity/cases/`
  (630 cases, IDs 10001–10630, P0 priority). Categories: NULL semantics, NULL
  ordering, aggregate-NULL, autoincrement, strict tables, CLI output mode, CLI
  dot-command, CLI option, PRAGMA P0, foreign keys, transactions, UPSERT,
  RETURNING, ATTACH, schema introspection, ALTER, recursive CTE, JOIN, compound,
  subquery, aggregate-advanced, error messages, pattern, BLOB.
- **8 xtask matrix-generated shards** under `corpus/sqlite_parity/cases/gen_*`
  (688 cases, IDs 11000–12037). Generators in `xtask/src/generators.rs`:
  `math` (109), `cast` (45), `affinity` (64), `string` (52), `datetime` (240),
  `json_path` (67), `window` (73), `pragma_sweep` (38). `cargo run -p xtask --
  generate --check` is the drift guard.
- **12 beyond-SQLite oracle shards** under `corpus/beyond_sqlite/generated_manifest.json`
  (265 cases, IDs 20001–20444) covering all 12 ranked feature areas in
  `metadata/beyond_sqlite/features.json`. Validated via psql ↔ psql self-compare
  in `src/beyond_sqlite/oracle.rs`; 253 passed, 12 graceful skips, 0 failures.

### Added — runner / infrastructure

- `xtask` workspace member with `generate` and `ship-gate` subcommands. Not
  shipped in the release tarball.
- `build.rs` enumerates `corpus/sqlite_parity/cases/*.json` at compile time so
  `include_str!` picks up new shards automatically.
- `src/beyond_sqlite/` module split: `mod.rs`, `case.rs`, `engine.rs`,
  `normalize.rs`, `oracle.rs`, `taxonomy.rs`. Postgres resolver returns
  `Configured(_) | Unavailable(_)`; SQLite-parity suite is mathematically
  incapable of depending on Postgres.
- Extended capability enum (`fts5`, `rtree`, `jsonb`, `math1`, `generate_series`,
  `json_pretty`, `jsonb_array_insert`) + probes in
  `src/sqlite_parity/engine.rs`.
- 5 new Rust integration tests under `tests/`: shard schema, beyond manifest,
  capability probe, release-manifest integrity, runner JSONL invariants.
- `scripts/release-package.sh` replaces the monoline justfile recipe — every
  file under `dist/<package>/` is glob-hashed into `artifact_hashes`.
- `justfile`: new `check`, `test`, `verify`, `generate-check`, `ship-gate`
  recipes alongside `pr-ci` and `release-local`.
- `.github/workflows/ci.yml`: optional `beyond-postgres` job (push +
  workflow_dispatch + `exhaustive` branch) that spins up a postgres:16
  service container and runs the oracle path. The default `pr-ci` job stays
  Postgres-free, preserving the SQLite-parity invariant.

### Changed

- `.jankurai/audit-policy.toml` — adopted with `mode = "advisory"`, scan
  exclusions for `tests/` integration files and `xtask/`. Mirror in
  `agent/audit-policy.toml`.
- `Cargo.toml` — workspace `[".", "xtask"]`, `default-members = ["."]`,
  resolver 3, `[features] pg-embedded` (declaration only; dep not yet wired).
- README.md / AGENTS.md — document the corpus development flow, ship contract,
  oracle gating, and new tarball contents.

### Notes

- The pinned upstream manifest `corpus/sqlite_parity/generated_manifest.json`
  (1,127 cases) stays byte-identical.
- New cases start at ID 10001 (parity) / 20001 (beyond), reserving 1128–9999
  for upstream growth.
- Ship contract: a case lands on `main` only after passing reference
  self-compare (`sqlite3 ↔ sqlite3` for parity, `psql ↔ psql` for beyond).

## [0.1.3] - 2026-05-24

Baseline release before the exhaustive expansion. See git history for the
prior runner + report-gate work.

[1.0.0]: https://github.com/neverhuman/redline-testing/compare/v0.1.3...v1.0.0
[1.0.1]: https://github.com/neverhuman/redline-testing/compare/v1.0.0...v1.0.1
[0.1.3]: https://github.com/neverhuman/redline-testing/releases/tag/v0.1.3
