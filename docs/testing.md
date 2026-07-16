# Testing & proof lanes

One command validates everything, locally and in CI (ci-local parity):

```bash
bash ops/ci/pr-ci.sh
```

It runs the same lanes `.github/workflows/ci.yml` runs. Run a single lane with
`bash scripts/ci-local.sh <lane>` (`fast|web|backend|security|e2e|jankurai|…`),
and check your toolchain with `bash scripts/ci-doctor.sh`.

## Lanes

| Lane | Script | What it proves |
|---|---|---|
| fast | `ops/ci/fast.sh` | shell syntax, `cargo fmt`/`check`, npm lockfile, actionlint |
| frontend | `ops/ci/web.sh` | `tsc`, eslint, vitest unit/component tests, `vite build` |
| backend | `ops/ci/backend.sh` | `cargo fmt`/`clippy -D warnings`/`test`, release build |
| security | `ops/ci/security.sh` | gitleaks, cargo-audit, cargo-deny, npm audit, zizmor, SBOM |
| web e2e | `ops/ci/e2e.sh` | Playwright smoke against the **built binary** + axe a11y |
| jankurai | `ops/ci/jankurai.sh` | exact governed Jankurai 1.6.11, clean-head score/proof plus exact-head Playwright/Axe evidence → ignored `target/jankurai/**` evidence |
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

## Agent-readable failures

Every blocking lane must explain a failure as a structured exception with its
`purpose`, concrete `reason`, bounded `common fixes`, local `docs_url`, and a
copyable `repair_hint`. The phase completion receipt records the exact-head
artifact and rerun command so the next owner can reproduce the failure without
guessing or using network services.
