# RedlineDB Hub — Architecture

## Overview

This repository is the **fusion hub** (front-door) for RedlineDB. It holds no engine
code; its job is distribution, documentation, and release orchestration.

```
neverhuman/RedlineDB  (this repo — hub)
  install.sh          one-line installer, fetches binary from Releases
  family.json         machine-readable family manifest
  FAMILY.md           human-readable family map
  .github/workflows/
    ci.yml            lint + shellcheck + security scan on every PR
    release.yml       builds binary from a pinned redline-core tag, publishes here
  ops/ci/             shell CI scripts mirrored to CI (ci-local parity)
  docs/               agent-readable documentation
  assets/             branding and diagrams
```

## The family

| Repo | Role | Public URL |
|------|------|-----------|
| `redline-core` | SQL/RQL engine, benchmarks, conformance test suite | `neverhuman/redline-core` |
| `redline-testing` | Official performance evidence runner | `neverhuman/redline-testing` |
| `redline-web` | Observability dashboard and web console | `neverhuman/redline-web` |
| `RedlineDB` | Hub / front-door (this repo) | `neverhuman/RedlineDB` |

See [FAMILY.md](../FAMILY.md) and [family.json](../family.json) for the full map including
internal jeryu mirrors.

## Release flow

1. A maintainer tags `redline-core` (e.g. `v1.2.3`) after passing its CI gate.
2. The tag is pushed to `RedlineDB` with the same version string.
3. `.github/workflows/release.yml` triggers, checks out `redline-core@v1.2.3`, builds
   the `redline` CLI binary for each target platform via `ops/ci/release.sh`.
4. `install.sh` fetches the asset from this repo's GitHub Releases.

## CI structure

All CI logic lives in `ops/ci/*.sh` so that local runs are identical to GitHub Actions:

| Script | Purpose |
|--------|---------|
| `ops/ci/lib.sh` | Shared helpers (require_tool, log_step, repo_root) |
| `ops/ci/pr-ci.sh` | Full PR gate: shellcheck, pointer checks, security, jankurai |
| `ops/ci/security.sh` | Secret/supply-chain scan (gitleaks + install.sh URL audit) |
| `ops/ci/release.sh` | Build, package, and publish binary assets |

Run locally: `just check` (same as CI) or `bash scripts/ci-local.sh [lane]`.
