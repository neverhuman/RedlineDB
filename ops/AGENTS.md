# ops — agent guide

Owns CI helpers, security/quality lanes, local proof scripts, git hooks, and
GitHub Actions / local parity.

- **Owns:** `ops/ci/*.sh`, `ops/git-hooks/*`, `.github/workflows/*`.
- **Forbidden:** product/runtime behavior (no domain, API, or UI logic under
  `ops/`); secret echo in workflows; mutable action refs (pin every external
  action to a 40-hex SHA); non-blocking security scans.
- **Proof lane:** `bash ops/ci/security.sh` and `actionlint`/`zizmor`. The whole
  gate is `bash ops/ci/pr-ci.sh`, which CI mirrors lane-for-lane.

Keep workflow logic in `ops/ci/*.sh`; GitHub Actions only call those scripts.
Security and audit receipts belong under `.artifacts/security/` or
`target/jankurai/`.
