# RedlineDB

Release status: **candidate** (`formal_ga=false`; production promotion is not
authorized by this repository).

`redline` is the public hub for the Redline family. The embedded engine lives
only in [`redline-core`](../redline-core); the conformance harness lives in
[`redline-testing`](../redline-testing); and the observability console lives in
[`redline-web`](../redline-web).

This repository intentionally contains no Cargo workspace and no engine
source. The family is independently managed by
[`redline-split-ops`](../redline-split-ops) and pinned by
[`redline.lock.toml`](../redline.lock.toml). Use the control-plane commands
from the family root to clone, update, validate, or run all child checks.

## Public entry points

- Engine API and CLI: [`redline-core`](../redline-core)
- SQLite-parity, RQL, memory, and beyond-SQLite evidence:
  [`redline-testing`](../redline-testing)
- SQL console and metrics dashboard: [`redline-web`](../redline-web)
- Family manifest: [`repos.manifest.toml`](../repos.manifest.toml)

Run `scripts/guard-no-duplicate-engine.sh` from this repository before
publishing a hub change. It fails if engine crates or a Cargo workspace are
reintroduced here.

## Agent and release navigation

- Repository agent entrypoint: [`AGENTS.md`](AGENTS.md)
- Architecture and ownership: [`docs/architecture.md`](docs/architecture.md)
- Runtime and repository boundaries: [`docs/boundaries.md`](docs/boundaries.md)
- Proof lanes and repair evidence: [`docs/testing.md`](docs/testing.md)
- Generated zones and audit rules:
  [`.jankurai/generated-zones.toml`](.jankurai/generated-zones.toml) and
  [`docs/audit-rubric.md`](docs/audit-rubric.md)
- Candidate release process, evidence, and rollback:
  [`docs/release.md`](docs/release.md)

## Quick start

Run the deterministic protected-source proof from the repository root:

```sh
bash scripts/ci-local.sh required
```

For a hub source change, run `bash scripts/ci-local.sh required`, then the
hard-gated `security`, `score`, `contract-drift`, and `artifact-support`
routes. The artifact route publishes local unsigned review evidence only; it
does not sign, tag, push, or authorize a release.
