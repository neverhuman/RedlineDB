# Testing & proof lanes

One command validates everything, locally and in CI (ci-local parity):

```bash
bash ops/ci/pr-ci.sh
```

It runs the same lanes `.github/workflows/ci.yml` runs. Run a single lane with
`bash scripts/ci-local.sh <lane>` (`fast|web|backend|security|e2e|jankurai|…`),
and check your toolchain with `bash scripts/ci-doctor.sh`.

## Governed Jankurai 1.6.11

Every active Jankurai command routes through `bash ops/ci/run-jankurai.sh`.
It accepts only `/home/ubuntu/.jeryu/bin/jankurai` with version
`jankurai 1.6.11`, binary SHA-256
`fdb42e5fa7d9851c0729e59bf1e582c895aa9cfc03a7175b420c6025d2fd014e`,
and a content-addressed production receipt for local-forge tag
`v1.6.11-deadlang-precision-split.1` at commit
`dface7397fe24d46b0b1885ddd5782c34edbff49`. Missing or mismatched identity
fails the lane; no PATH, environment, or per-repository install fallback is
allowed.

## Lanes

| Lane | Script | What it proves |
|---|---|---|
| fast | `ops/ci/fast.sh` | shell syntax, `cargo fmt`/`check`, npm lockfile, actionlint |
| frontend | `ops/ci/web.sh` | `tsc`, eslint, vitest unit/component tests, `vite build` |
| backend | `ops/ci/backend.sh` | `cargo fmt`/`clippy -D warnings`/`test`, release build |
| security | `ops/ci/security.sh` | gitleaks, cargo-audit, cargo-deny, npm audit, zizmor, SBOM |
| web e2e | `ops/ci/e2e.sh` | Playwright smoke against the **built binary** + axe a11y |
| jankurai | `ops/ci/jankurai.sh` | the jankurai tool suite → `target/jankurai/**` evidence |
| cost-budget | `ops/ci/cost-budget.sh` | zero-spend budget + stop conditions receipt |
| release-readiness | `ops/ci/release-readiness.sh` | release evidence surface receipt |

## Test kinds

- **Unit / integration (Rust):** `apps/api/tests/api.rs` drives the real router;
  module `#[cfg(test)]` blocks cover the connector.
- **Property (Rust):** `apps/api/tests/property.rs` uses `proptest` for the
  input-boundary (`quote_ident`) and read-only-authz (`is_read_only_sql`)
  invariants — the deterministic negative proofs.
- **Unit / component (web):** vitest + Testing Library under `apps/web/src`.
- **E2E (web):** `apps/web/e2e/smoke.spec.ts` boots the embedded binary, asserts
  the UI renders, runs a live `/api/query` round-trip, and scans accessibility.

## Budgets

Every lane is local and deterministic; there is **no paid or networked work**.
The cost budget (`agent/cost-budget.toml`, `target/jankurai/cost-budget.json`)
caps `local_ci_minutes` at 30 and sets every spend and quota to zero with
explicit stop conditions and a kill switch. See [release.md](release.md) for
rollback guidance and the release version source.

## Launch Gate Evidence

Release readiness needs more than command names. The release gate points at
artifact-backed proof of every launch concern:

- security evidence in `docs/security.md` and
  `target/jankurai/security/evidence.json` (blocking in CI);
- backup and restore expectations in `docs/operations.md`;
- monitoring receipts: `/api/metrics`, `/metrics`, plus
  `target/jankurai/repo-score.md` and `target/jankurai/repair-queue.jsonl`;
- cost-budget and stop-condition receipts in `agent/cost-budget.toml` and
  `target/jankurai/cost-budget.json`;
- release readiness in `target/jankurai/release-readiness.json`;
- rollback guidance in `docs/release.md` and `CHANGELOG.md`;
- abuse controls from the `--read-only` isolation boundary and the
  identifier/input-boundary proofs in `apps/api/tests/property.rs`.

The release-readiness lane (`bash ops/ci/release-readiness.sh`) asserts this
surface and writes `target/jankurai/release-readiness.json`.
