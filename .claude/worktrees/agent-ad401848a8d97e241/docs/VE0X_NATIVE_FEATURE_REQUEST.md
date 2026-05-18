# veox-native feature request for RedlineDB

The xdoug native runtime is now designed around a Redline-shaped session store.
This document records the upstream RedlineDB work needed to make that path real
instead of local stand-in behavior.

Local phase11 follow-up status: the ephemeral database API, caller-owned temp
roots, queue contract, and xdoug UPDATE-subquery regression are implemented in
the workspace tree; this request remains as the external-facing checklist.

Required work:

1. Ephemeral shared in-memory database support.
   - `Database::create_in_memory(options: OpenOptions) -> Result<Database>`
   - `Database::create_ephemeral(session_name: &str, options: OpenOptions) -> Result<Database>`
   - multiple connections see the same state
   - state drops when the owning database/session drops

2. Caller-owned temp/spill roots.
   - explicit roots for query spill, sort spill, vector spill, and temp artifacts
   - no surprise page/WAL/checkpoint/control files unless configured

3. Tokio/pooling contract.
   - document `Send`/`Sync` expectations for `Database`, `Connection`, and `Statement`
   - provide the recommended `spawn_blocking` / pool pattern

4. Veox task queue contract.
   - atomic claim under contention
   - priority desc + created_at asc ordering
   - complete/fail only from claimed state
   - no duplicate claims under load

5. SQL compatibility for xdoug.
   - `BEGIN IMMEDIATE`
   - `UPDATE ... RETURNING`
   - `INSERT ... ON CONFLICT DO UPDATE`
   - `INSERT OR IGNORE`
   - `INSERT OR REPLACE`
   - `BLOB`, `TEXT`, `INTEGER`, nullable columns
   - JSON scalar functions used by direct tests
   - partial indexes or a documented replacement
   - `ORDER BY ... LIMIT 1` subquery inside `UPDATE`

6. MSRV resolution.
   - xdoug declares Rust 1.92
   - record the compatibility choice explicitly

Acceptance target:
- the native runtime can depend on RedlineDB without dragging in a Postgres
  client or requiring a persistent SQLite fallback
