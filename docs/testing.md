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
| ship-gate | `cargo run -p xtask -- ship-gate` | prospective SQLite cases self-compare; required contract cases cannot be removed |
| compatibility | `cargo run --locked -- run --contract <id> --cases <selector> --mode diagnostic\|release` | deterministic selection and strict fail-closed evidence; release requires `--cases all` |
| beyond-postgres | `cargo test --locked --features pg-embedded -p redline-testing beyond_sqlite::oracle::tests::postgres_self_compare_all_published_cases -- --ignored --exact` | every published case passes the `psql` ↔ `psql` oracle self-compare with zero skips |

### Forge proof parity

Before pushing a proof-control successor, reproduce the local forge's exact
changed-head decision with the regular, non-symlink governed binary:

```bash
/home/ubuntu/.jeryu/bin/jankurai diff-audit . \
  --base-ref origin/main \
  --json target/jankurai/forge-parity-diff-audit.json \
  --advisory-only
```

Acceptance is score at least 85, zero hard findings, and zero applied caps.
The full-tree ratchet is a separate gate and cannot waive this result. The
forge runs the same decision in an automatically removed standalone clone;
CI and local validation never use `git worktree`.

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
Those structured errors are the CLI telemetry boundary: command status,
elapsed-time metrics, case identifiers, and repair hints remain machine
readable without a remote observability service. A repair receipt records the
exact head, command, exit status, and artifact digest; never infer success from
an absent or null receipt. The stable machine contract is
[`schemas/repair-receipt.schema.json`](../schemas/repair-receipt.schema.json),
which is packaged and integrity-hashed with every release. Jankurai's JSONL
repair tasks are separately described by
[`schemas/repair-queue.schema.json`](../schemas/repair-queue.schema.json), and
[`schemas/repo-score.schema.json`](../schemas/repo-score.schema.json) defines
the score attestation fields that must never be silently defaulted.

## Cost budget

`redline-testing` is a zero-spend, offline harness — no paid API, no metered
network egress. `agent/cost-budget.toml` declares the budgets, quota caps, and
stop conditions (all zero / fail-closed); `bash ops/ci/cost-budget.sh` proves
them and writes `target/jankurai/cost-budget.json`.
The explicit kill switch is `REDLINE_TESTING_KILL_SWITCH`; a set switch,
unknown paid tool, missing receipt, or nonzero quota stops the lane. This is a
hard budget and rate-limit boundary, not advisory accounting.

## Release readiness

Launch gates for a tarball release are documented in
[`docs/release.md`](release.md) and [`docs/operations.md`](operations.md), and
proven by `bash ops/ci/release-readiness.sh` (writes
`target/jankurai/release-readiness.json`). The gate checks that the following
launch-gate evidence is present before a tag is published:

- **security**: `bash ops/ci/security.sh` (gitleaks, cargo-audit, cargo-deny,
  zizmor, SBOM) plus local custody and family-CI **provenance** receipts.
- **backups**: the corpus + manifest are content-addressed (SHA-256) and the
  release tarball is the immutable backup of every shipped artifact.
- **monitoring**: RedlineDB CI consumes the pinned tarball and reports
  regressions; the `release_manifest_integrity` test monitors artifact drift.
- **rollback**: ship a higher version restoring prior behavior — old tags stay
  immutable (see [`docs/release.md`](release.md#rollback)).
- **abuse controls**: the runner only drives allowlisted local subprocess
  shells with bounded timeouts; no untrusted network input is accepted.

The authoritative local host-CI security lane requires
`JAIN_CARGO_DENY_ADVISORY_DB` to name the exact physical cargo-deny database
under its request-local Cargo home. In isolated release CI, `advisory-dbs` must
be an exact root-owned read-only mountpoint and the database must be recursively
non-writable before cargo-deny starts. This prevents a same-commit database
from being swapped in during the scan and the original inode restored before
the post-scan identity check. Developer runs retain exact commit/tree and
pre/post physical-identity validation but do not claim that immutable mount
boundary.
