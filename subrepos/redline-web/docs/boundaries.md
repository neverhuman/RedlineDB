# Boundaries

The machine-readable boundary map is `agent/boundaries.toml`. This doc explains
the three runtime boundaries and how each is proven.

## 1. HTTP API (`CONTRACT.md`)

`CONTRACT.md` is the source of truth. The Rust DTOs (`apps/api/src/model.rs`,
serde `camelCase`) and the TypeScript DTOs (`apps/web/src/api/types.ts`) mirror
it exactly. Untyped values crossing the network are validated before they are
narrowed:

- **Rust** — typed serde structs; handlers only ever see decoded `Json<T>`.
- **TypeScript** — `apps/web/src/api/decode.ts` validates raw JSON/SSE frames
  (e.g. `parseMetricsFrame`) and throws `DecodeError` on a bad shape. Never
  `as`-cast an untyped boundary value.

Proof: `cargo test --workspace --all-targets --locked` and
`cd apps/web && npm run test`.

## 2. SQL execution (`apps/api/src/connector/`)

- Identifiers are escaped with `quote_ident` (double-quote doubling) before any
  interpolation; values are passed as bound parameters.
- `--read-only` connections enforce isolation: `is_read_only_sql` rejects any
  non read-only leading statement, and SQLite is opened `SQLITE_OPEN_READ_ONLY`.
- Output is capped at `--max-rows` and every query is bounded by
  `--query-timeout-ms`.

Proof: `apps/api/tests/property.rs` (input-boundary + read-only invariants).
See [security.md](security.md).

## 3. Embedded SPA (`apps/api/src/static_assets.rs`)

`apps/web/dist` is embedded via `rust-embed`. `/api/*` paths never resolve to
the SPA; unmatched non-API GET paths return `index.html` for client routing.

Proof: `apps/api/tests/api.rs`.
