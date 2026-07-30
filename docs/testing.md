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
| security | `bash ops/ci/security.sh` | gitleaks secret scan, `cargo audit`, `cargo deny`, zizmor, SBOM |
| score | `bash ops/ci/score.sh` | governed Jankurai 1.6.11 score, floor, findings, caps, blockers, and accepted-baseline ratchet |
| contract drift | `bash ops/ci/contract-drift.sh` | tracked schemas plus exact 1.0.1 package, local/forge immutable live-or-planned tag identity, sanitized build inputs, repeated deterministic archive identity, schema-valid evidence, closed binary/archive content, and digests |
| artifact support | `bash ops/ci/artifact_support.sh` | clean commit/tree-bound unsigned review evidence with a deterministic source-archive digest and stable pre/post identity checks; release signing remains a separate release workflow gate |
| jankurai | `REDLINE_STRICT_TOOLS=1 bash ops/ci/jankurai.sh` | exact-head routed receipts, non-vacuous required Proofbind, covered/mutated Proofmark, closed receipt validation, and the audit/copy/security/readiness evidence |
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
Strict runs also require `target/jankurai/proofbind/obligations.json` to bind
the full source head with at least one fully satisfied obligation and a
`pass` verdict. Proofmark must cover every changed Rust line, kill every
focused receipt-contract mutation, and include the governed negative proofs
for input-boundary and tool-supply obligations. The closed validator rejects
unknown fields, substituted heads, vacuous obligations, review verdicts,
unavailable coverage/mutation, and missing negative evidence.
Artifact-support review evidence is intentionally unsigned and network-free;
release artifact signing and provenance remain governed by the release
workflow and are not synthesized by a required-check worker. The lane refuses
dirty tracked, staged, or untracked source and revalidates the exact HEAD, tree,
and deterministic source-archive digest after CI and evidence generation.

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
