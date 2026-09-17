# db — agent guide

Read the root `AGENTS.md` first. This cell documents the database boundary.

- **Owns:** the database boundary contract. redline-web has **no checked-in
  migrations** — it serves whatever SQLite database is supplied at runtime
  (`--db`/`--target-bin`). Durable truth lives in that database.
- **Forbidden:** application logic, transport routing, or UI concerns here; any
  database access from outside the `apps/api/src/connector` adapter.
- **Proof lane:** the connector's negative proofs —
  `cargo test -p redline-web-server --test connector` and `--test property`
  (read-only isolation + identifier input-boundary).

The only code that touches the database is the `Connector` adapter
(`apps/api/src/connector/`). The web layer never accesses a database directly;
it sends user-authored SQL text over HTTP to `POST /api/query`. The demo seed
(`apps/api/src/connector/seed_demo.sql`) is a fixed, checked-in script. See
[`../docs/boundaries.md`](../docs/boundaries.md) and `agent/boundaries.toml`.
