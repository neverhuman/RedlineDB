# Testing, proof lanes, and release readiness

This repo's proof surface is deterministic and local-first: every CI lane is a
script under `ops/ci/` that you can run by hand, so local == CI.

## One-command setup and validation

```bash
bash scripts/setup.sh        # install toolchain + build + fetch deps
bash ops/ci/pr-ci.sh         # the single validate command (fmt, check, test, package)
```

## Proof lanes

| Lane | Command | What it proves |
|---|---|---|
| fast / validate | `bash ops/ci/pr-ci.sh` | fmt, `cargo check`, `cargo test --locked`, release packaging |
| security | `bash ops/ci/security.sh` | offline gitleaks, pinned local RustSec `cargo audit --no-fetch`, `cargo deny`, blocking zizmor, and no-update SBOM |
| jankurai | `bash ops/ci/jankurai.sh` | fail-closed audit/proof using exact regular non-symlink Jankurai 1.6.11 bytes at `/home/ubuntu/.jeryu/bin/jankurai`; target-only copy-code, rust-witness, security, cost, and release evidence |
| ship-gate | `cargo run -p xtask -- ship-gate` | each SQLite-parity shard self-compares against `sqlite3` |
| beyond-postgres | `cargo test --locked --features pg-embedded -p redline-testing beyond_sqlite::oracle::tests::postgres_self_compare_all_published_cases -- --ignored --exact` | every published case passes the `psql` ↔ `psql` oracle self-compare with zero skips |

### Unit, integration, and property tests

- Unit tests live next to the code in `src/**`.
- Integration tests live in `tests/*.rs` and run through `cargo test --locked`.
- Property/invariant tests use `proptest` (in-module `prop_tests` under
  `src/sqlite_parity/`, e.g. `text.rs`) to fuzz pure transforms such as
  identifier sanitization.

## Observability and repair receipts

Runtime errors are modeled by the typed `HarnessError` surface in
`src/exceptions.rs`. Every variant carries a `purpose`, `reason`,
`common_fixes`, `docs_url`, and `repair_hint`, so a failing run tells the next
agent exactly where to rerun proof. Jankurai evidence (audit score, security
evidence, copy-code, witness graph) is written under `target/jankurai/**`.

## Cost budget

`redline-testing` is a zero-spend, offline harness — no paid API, no metered
network egress. `agent/cost-budget.toml` declares the budgets, quota caps, and
stop conditions (all zero / fail-closed); `bash ops/ci/cost-budget.sh` proves
them and writes `target/jankurai/cost-budget.json`.

## Release readiness

Launch gates for a tarball release are documented in
[`docs/release.md`](release.md) and [`docs/operations.md`](operations.md), and
proven by `bash ops/ci/release-readiness.sh` (writes
`target/jankurai/release-readiness.json`). The gate checks that the following
launch-gate evidence is present before a tag is published:

- **security**: `bash ops/ci/security.sh` (gitleaks, cargo-audit, cargo-deny,
  zizmor, SBOM) plus Sigstore **provenance** attestation of the tarball.
- **backups**: the corpus + manifest are content-addressed (SHA-256) and the
  release tarball is the immutable backup of every shipped artifact.
- **monitoring**: RedlineDB CI consumes the pinned tarball and reports
  regressions; the `release_manifest_integrity` test monitors artifact drift.
- **rollback**: ship a higher version restoring prior behavior — old tags stay
  immutable (see [`docs/release.md`](release.md#rollback)).
- **abuse controls**: the runner only drives allowlisted local subprocess
  shells with bounded timeouts; no untrusted network input is accepted.
