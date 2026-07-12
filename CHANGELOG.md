# Changelog

All notable changes to redline-web are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); this project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html). The authoritative
version source is `apps/api/Cargo.toml`.

## [Unreleased]

### Changed

- Migrated to the canonical reference-profile layout: `server/` → `apps/api/`,
  `web/` → `apps/web/`. The release binary still embeds the built SPA.

### Added

- A locked, isolated Rust release-control tool that enforces the repository's
  interpreter boundary in local and hosted CI.
- ci-local parity: `ops/ci/*.sh` lanes sourced by a single `ops/ci/pr-ci.sh`
  gate, mirrored 1:1 by `.github/workflows/ci.yml`; pre-push hook, ci-doctor,
  and ci-local runner.
- Security lane (`ops/ci/security.sh`): gitleaks, cargo-audit, cargo-deny, npm
  audit, zizmor, SBOM — blocking in CI.
- jankurai tool-suite evidence lane and CI artifact upload.
- Web e2e Playwright smoke + rendered-UX QA lane; design tokens.
- `proptest` property tests for the input-boundary and read-only-authz guards.
- Typed agent-readable exception surface (`RepairHint`) and `docs/`.

## [0.1.0] - 2026-06-10

### Added

- Initial SQL console + live observability dashboard over SQLite / `--target-bin`.
