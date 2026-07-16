# RedlineDB

[![Jankurai governed: 1.6.11](assets/jankurai-governed.svg)](docs/audit-rubric.md)

`redline` is the public hub for the Redline family. The embedded engine lives
only in [`redline-core`](../redline-core); the conformance harness lives in
[`redline-testing`](../redline-testing); and the observability console lives in
[`redline-web`](../redline-web).

This repository intentionally contains no Cargo workspace and no engine
source. The family is independently managed by
[`redline-split-ops`](../redline-split-ops) and pinned by
[`redline.lock.toml`](../redline.lock.toml). Use the control-plane commands
from the family root to clone, update, validate, or run all child checks.
Read [`AGENTS.md`](AGENTS.md) before making changes; it defines this hub's
ownership, provenance, and proof requirements.

## Quick start

From this repository, validate the thin-hub boundary and then produce the
governed, clean-HEAD audit receipt:

```sh
just check
just score
```

## Public entry points

- Engine API and CLI: [`redline-core`](../redline-core)
- SQLite-parity, RQL, memory, and beyond-SQLite evidence:
  [`redline-testing`](../redline-testing)
- SQL console and metrics dashboard: [`redline-web`](../redline-web)
- Family manifest: [`repos.manifest.toml`](../repos.manifest.toml)

Run `scripts/guard-no-duplicate-engine.sh` from this repository before
publishing a hub change. It fails if engine crates or a Cargo workspace are
reintroduced here.
