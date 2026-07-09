# jain-split-ops

![local required](https://img.shields.io/badge/local_required-passing-brightgreen)
![jankurai score](https://img.shields.io/badge/jankurai_score-89-brightgreen)

Control plane for the Jain split family under `/home/ubuntu/jain-split`.

This repository owns the family manifest, materializer, local Jeryu host CI
runner, validation scripts, and release/tag orchestration helpers. It is not a
product member repo and is not listed in `repos.manifest.toml`.

Start with [AGENTS.md](AGENTS.md) for agent instructions.

## Quick Start

```bash
just jeryu-ready
just jeryu-repos
just fast
just required
just score
```

Operational remotes and internal Cargo Git dependencies use local Jeryu:
`http://127.0.0.1:8787/git/jeryu/<repo>.git`.

Do not point agents at `~/jeryu-split`, public GitHub, or `target/bare-mirrors`
for Jain development sources. `~/.jeryu` is only local credential/client state.
Bare mirrors are only a CI cache created by the host runner through a temporary
`GIT_CONFIG_GLOBAL`.

## Key Files

- `repos.manifest.toml`: live split-family repo map and tags.
- `ops/split/materialize.py`: generator for member repo standards.
- `ops/split/jeryu-doctor.py`: one-command local forge and remote setup.
- `ops/split/jeryu-local.py`: local PR/check REST wrapper for agents.
- `ops/split/validate-local-jeryu.py`: local-source policy validator.
- `ops/ci/split-host-ci.sh`: local Jeryu required-check runner.
- `docs/local-jeryu-forge-agent-workflow.md`: PR and status workflow.
