# apps/api — agent guide

Read the root `AGENTS.md` first. This cell is the Rust + Axum backend.

- **Owns:** `apps/api/` — Axum routers/handlers, the `Connector` trait and its
  transports, the metrics registry, serde DTOs (`model.rs`), the typed repair
  surface (`repair.rs`), and the embedded-SPA handler (`static_assets.rs`).
- **Forbidden:** UI-only concerns; depending on `redline-core`'s internal
  `redlinedb-*` crates; ad-hoc SQL string-building from the edge (escape
  identifiers via `quote_ident`, bind values); bypassing the `--read-only`
  guard; drifting DTOs from `CONTRACT.md`.
- **Proof lane:** edge handler / contract + property tests —
  `cargo test --workspace --all-targets --locked` (`tests/api.rs`,
  `tests/property.rs`).

`CONTRACT.md` is the source of truth; change it first, then the Rust DTOs and
`apps/web/src/api/types.ts` together. The release binary embeds
`apps/web/dist`, so build the frontend before the backend.
