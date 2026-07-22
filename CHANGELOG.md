# Changelog

## 4.1.0-jain.4 candidate — 2026-07-16

- Preserve the independently reviewed two-commit Redline Central history.
- Establish the native Redline 4.1.0 identity and protected local-Jeryu review flow.
- Add Rust behavior tests and blocking required, security, score, contract,
  artifact, and coverage lanes.
- Pin the governed Jankurai 1.6.11 path and digest.
- Rotate that closed auditor identity to
  `v1.6.11-deadlang-precision-split.2` at source revision `4dfbdfa` and bind
  its reproducible release-binary digest without changing the legacy release
  tag schema.
- Replace named production dispatch with an owned backend-neutral contract, isolated compile-time
  Redline/SQLite/Postgres adapters, and the governed `db-shim.used-operations/v2` corpus with
  explicit portable null types and truthful success-only execution.
