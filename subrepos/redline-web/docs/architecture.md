# Architecture

redline-web is a two-cell application with one HTTP contract between them.

```
apps/api/   Rust + Axum backend  ── embeds ──▶ apps/web/dist (built SPA)
apps/web/   Vite + TS + React frontend
CONTRACT.md single source of truth for every endpoint and DTO
```

## Backend (`apps/api/`)

- `src/main.rs` — clap entry point; parses `Config`, calls `build_app`.
- `src/lib.rs` — `build_app(config)` selects a connector and wires the router.
- `src/api/` — Axum routers + handlers (`health`, `schema`, `query`, `metrics`).
  Unmatched non-`/api` GET paths resolve to the embedded SPA shell.
- `src/connector/` — the `Connector` trait and its two transports:
  - `sqlite.rs` — bundled `rusqlite` over a SQLite file (primary).
  - `target_bin.rs` — drives an external SQLite-compatible CLI (`--target-bin`).
- `src/metrics/` — in-process registry (counters, latency histogram, slow-query
  ring) streamed over SSE and exposed at `/metrics`.
- `src/model.rs` — serde DTOs (`camelCase`), mirroring `apps/web/src/api/types.ts`.
- `src/repair.rs` — the typed, agent-readable exception surface (`RepairHint`).
- `src/static_assets.rs` — `apps/web/dist` embedded via `rust-embed`.

## Frontend (`apps/web/`)

- `src/api/` — typed client (`client.ts`), DTO types (`types.ts`), and the
  runtime boundary decoders (`decode.ts`) that validate untyped JSON/SSE frames.
- `src/components/` — `SchemaTree`, `QueryConsole`, `ResultsGrid`,
  `TableBrowser`, `MetricsDashboard`, `ConnectionBar`.
- `src/hooks/useMetricsStream.ts` — SSE subscription with a polling fallback.
- `src/tokens.css` — design tokens (the visual single source of truth).

## Build/embed flow

`apps/web` builds to `apps/web/dist`; the backend's `rust-embed` folder is
`$CARGO_MANIFEST_DIR/../web/dist` (i.e. `apps/api/../web/dist` →
`apps/web/dist`), so the release binary serves the real built UI. See
[boundaries.md](boundaries.md) and [testing.md](testing.md).
