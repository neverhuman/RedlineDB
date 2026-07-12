# jain-split-ops

![local required](https://img.shields.io/badge/local_required-passing-brightgreen)
![jankurai score](https://img.shields.io/badge/jankurai_score-89-brightgreen)

Control plane for the Jain split family under `/home/ubuntu/jain-split`.

This repository owns the family manifest, Rust contract materializer, local Jeryu host CI
runner, validation scripts, and release/tag orchestration helpers. It is
declared separately as the family control plane in `repos.manifest.toml`.

Start with [AGENTS.md](AGENTS.md) for agent instructions.

## Quick Start

```bash
./release-candidate.sh
```

That single resumable command reads the canonical manifest, runs Redline and
every selected Jain repository from detached exact commits in rollout-wave
order, applies only manifest-bound compare-and-swap tags, and finishes with the
AtomicSoul 8.0.0 dry run. It records every result under
`docs/release-evidence/8.0.0/orchestrator/` and stops on dirty work, missing
review identities, failed gates, or host capability blockers. Use
`./release-candidate.sh --plan` for a no-execution inventory.

Operational remotes and internal Cargo Git dependencies use local Jeryu:
`http://127.0.0.1:8787/git/jeryu/<repo>.git`.

Do not point agents at `~/jeryu-split`, public GitHub, or `target/bare-mirrors`
for Jain development sources. `~/.jeryu` is only local credential/client state.
Bare mirrors are only a CI cache created by the host runner through a temporary
`GIT_CONFIG_GLOBAL`.

## Key Files

- `repos.manifest.toml`: live split-family repo map and tags.
- `tools/splitctl/src/main.rs` (`splitctl`): Rust control-plane contract refresh, authored-repo refresh, local-Jeryu validation, source coverage, version consistency, and bare-mirror refresh.
- `release-candidate.sh`: thin one-command entrypoint into the Rust resumable fleet runner.
- `cargo run --locked -- materialize`: Rust contract materializer for the family.
- `cargo run --locked -- jeryu-doctor`: local forge/remotes health check.
- `cargo run --locked -- jeryu-local`: local PR/check REST wrapper for agents.
- `cargo run --locked -- validate-local-jeryu`: local-source policy validator.
- `ops/ci/split-host-ci.sh`: detached local Jeryu required-check worker invoked by the Rust fleet runner.
- `docs/local-jeryu-forge-agent-workflow.md`: PR and status workflow.
- `docs/release-runbook.md`: canonical v8.0.0 release, SmartCluster, Redline,
  artifact, canary, promotion, and rollback workflow.

SmartCluster is managed as required infrastructure under the declared
`jain-split/jain-smartcluster` namespace. Redline source remains in the local
`redline-split/` nested family and is validated as an immutable release
dependency; neither is copied from an external workspace.
