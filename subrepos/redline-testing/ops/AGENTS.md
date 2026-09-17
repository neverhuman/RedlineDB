# ops — Agent guidance

## Owns

- `ops/ci/*.sh` — the CI lane scripts (fast/validate, security, jankurai,
  language-bad-behavior, cost-budget, release-readiness, release).
- `ops/ci/lib.sh` — shared lane helpers (`log`/`has`/`run_if_has`/tool pins).
- `ops/git-hooks/*` — the mandatory local pre-push gate.
- `ops/observability/` — repair-receipt and telemetry notes.

## Forbidden

- Do not put suite/runtime/report behavior under `ops/`; that lives in `src/`.
- Do not inline CI commands in `.github/workflows/*`. Workflows stay thin and
  delegate to `bash ops/ci/<lane>.sh` so local runs == CI.
- Do not weaken a security scan to non-blocking, echo secrets, or unpin a
  GitHub Action from its 40-hex commit SHA.

## Proof lane

- Validate: `bash ops/ci/pr-ci.sh`
- Security: `bash ops/ci/security.sh`
- Jankurai evidence: `bash ops/ci/jankurai.sh` (artifacts under `target/jankurai/**`)
- Doctor: `bash scripts/ci-doctor.sh`
