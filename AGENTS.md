# redline-web — agent guide

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
- **jankurai standard.** Audit only with the regular non-symlink `jankurai`
  selected from the root-governed release `PATH`, at version 1.6.11 and its
  pinned digest. A caller-selected substitute never becomes evidence merely
  because it has the same name. `just score`.
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
