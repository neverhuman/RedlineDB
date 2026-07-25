# jain-split-ops

![local required](https://img.shields.io/badge/local_required-passing-brightgreen)
![jankurai score](https://img.shields.io/badge/jankurai_score-89-brightgreen)

Control plane for the Jain split family under `/home/ubuntu/jain-split`.

This repository owns the family manifest, Rust contract materializer, local Jeryu host CI
runner, validation scripts, and release/tag orchestration helpers. It is not a
product member repo and is not listed in `repos.manifest.toml`.

Start with [AGENTS.md](AGENTS.md) for agent instructions.

## Quick Start

```bash
just jeryu-ready
just jeryu-repos
just coordination-status
just quality-status /home/ubuntu/.jeryu/secrets/veox-owner-token
just fast
just required
just score
```

Operational remotes use the local Jeryu forge under the canonical `veox`
owner: `http://127.0.0.1:8787/git/veox/<repo>.git`. Historical internal Cargo
pins retain their existing owner spelling until an explicit dependency re-pin.

Do not point agents at `~/jeryu-split`, public GitHub, or `target/bare-mirrors`
for Jain development sources. `~/.jeryu` is only local credential/client state.
Bare mirrors are only a CI cache created by the host runner through a temporary
`GIT_CONFIG_GLOBAL`.

## Key Files

- `repos.manifest.toml`: live split-family repo map and tags.
- `src/main.rs` (`splitctl`): Rust control-plane contract refresh, authored-repo refresh, local-Jeryu validation, source coverage, version consistency, and bare-mirror refresh.
- `cargo run --locked -- materialize`: Rust contract materializer for the family.
- `cargo run --locked -- jeryu-doctor`: local forge/remotes health check.
- `cargo run --locked -- jeryu-local`: local PR/check REST wrapper for agents.
- `cargo run --locked -- coordination-ledger`: frozen-prefix validation,
  three-ledger synchronization reporting, and guarded all-lock append.
- `cargo run --locked -- quality-status`: authenticated exact-local-HEAD
  required/proof selection, ratchet validation, and deterministic repair queue.
- `cargo run --locked -- validate-local-jeryu`: local-source policy validator.
- `ops/ci/split-host-ci.sh`: local Jeryu required-check runner.
- `ops/ci/HOST_CI_BOUNDARY.md`: root-owned sandbox, proof-evidence, result-seal,
  and publication contract.
- `docs/local-jeryu-forge-agent-workflow.md`: PR and status workflow.
- `docs/release.md`: control-plane version, integrity, installation, and rollback policy.
- `docs/release-runbook.md`: canonical v8.0.1 candidate, SmartCluster, Redline,
  artifact, canary, promotion, and rollback workflow.

SmartCluster is managed as required infrastructure under the declared
`veox/jain-smartcluster` namespace. Redline ground truth lives only in the
local `jain-redline/` nested family and is validated as an immutable release
dependency. The top-level legacy Redline paths are preservation inputs, not
authority or release sources; a future copy-out is a separately authorized export.

## Protected host-CI authority

Required-check publication is a protected-host operation, not a capability of
an ordinary source checkout. The protected boundary runs the product gate and governed
Jankurai audit in separate network-isolated units, validates exact-SHA evidence
under root-owned storage, and publishes and reads back `jankurai/proof` before
publishing and reading back `<repo>/required`, then publishing and reading back
its commit status.

Install or update that boundary only from a clean protected-merged
`jain-split-ops` commit using the reviewed procedure in
[HOST_CI_BOUNDARY.md](ops/ci/HOST_CI_BOUNDARY.md). An unmerged PR may run its
tests, but it must not install or execute the publisher, migrate the forge
credential, or publish product checks.
