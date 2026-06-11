# ops/ — agent guide

This directory contains all CI and operations scripts for the RedlineDB hub.

## Owns

- `ops/ci/lib.sh` — shared bash helpers sourced by all CI scripts
- `ops/ci/pr-ci.sh` — the full PR gate (mirrors `.github/workflows/ci.yml`)
- `ops/ci/security.sh` — secret scan and supply-chain checks
- `ops/ci/release.sh` — build, package, and publish binary assets
- `ops/git-hooks/pre-push` — pre-push hook that runs the PR gate locally

## Forbidden

- No engine source (Rust/SQL/RQL). All engine code belongs in `redline-core`.
- Do not inline CI logic in workflows; delegate to these scripts instead.
- Do not store credentials or tokens here.

## Proof lane

The `just check` / `bash ops/ci/pr-ci.sh` command is the single authoritative gate.
Run it before every push. The `ops/git-hooks/pre-push` hook automates this.

See [docs/architecture.md](../docs/architecture.md) for the full CI layout.
