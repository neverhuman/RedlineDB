# redline-web — agent guide

## Zero-worktree policy — absolute

- Do not create or move a Git worktree anywhere, including under `/tmp`. Never
  run `git worktree add` or `git worktree move`.
- Work only in the existing primary checkout. If it is dirty, busy, or held by
  another owner, stop and wait for a clean stopped-head handoff.
- Existing worktrees are cleanup inputs only. Never force-remove or prune one.
  Removal requires separate authority, proof that no regression or unmerged
  work would be lost, and a fresh recursive scan proving the path has no
  symlink.
- Exact-SHA CI may use only an automatically removed standalone clone/sandbox
  that is not registered with `git worktree`.

Rust (Axum) backend in `apps/api/` + Vite/TS/React frontend in `apps/web/`. It is
a SQL console + observability dashboard over any SQLite-compatible database.

Per-cell guides: [`apps/api/AGENTS.md`](apps/api/AGENTS.md),
[`apps/web/AGENTS.md`](apps/web/AGENTS.md), [`ops/AGENTS.md`](ops/AGENTS.md).
Durable detail lives in `docs/` (architecture, boundaries, testing, security,
operations, release).

## Ground rules

- **`CONTRACT.md` is the source of truth** for every endpoint and DTO. Change it
  first, then the backend (`apps/api/src`) and the frontend (`apps/web/src`)
  together.
- **No engine coupling.** Do not depend on `redline-core`'s internal `redlinedb-*`
  crates. Talk to databases via the `Connector` trait (SQLite file or
  `--target-bin` CLI).
- **Stay independent.** No workspace spanning sibling repos. `ops/ci/pr-ci.sh` is
  the green gate.
- **jankurai standard.** Audit only with governed
  `/home/ubuntu/.jeryu/bin/jankurai`, which must report `jankurai 1.6.11` and
  SHA-256 `fdb42e5fa7d9851c0729e59bf1e582c895aa9cfc03a7175b420c6025d2fd014e`.
  Never select an auditor through ambient `PATH`, `~/.cargo/bin`, or
  `~/.local/bin`. `just score`.
- **MR-only.** Land via a jeryu PR (`gh pr create` → `jeryu.propose_patch`);
  `main` advances on forge merge and mirrors to `github.com/neverhuman/redline-web`.

## Layout

```
apps/api/src/
  api/        axum routers + handlers (health, schema, query, metrics)
  connector/  Connector trait + sqlite.rs + target_bin.rs
  metrics/    in-process registry (counters, latency, slow-query ring)
  model.rs    serde DTOs (camelCase) — mirror apps/web/src/api/types.ts
  repair.rs   typed agent-readable exception surface (RepairHint)
apps/web/src/
  api/        typed client + types + runtime decoders (decode.ts)
  components/  SchemaTree, QueryConsole, ResultsGrid, TableBrowser, MetricsDashboard
  e2e/        Playwright smoke (../e2e) boots the built binary
```
