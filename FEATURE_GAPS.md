# FEATURE GAPS

Append-only tracker for gaps surfaced by veox-native integration work.

## 2026-05-15

- Need true shared ephemeral in-memory DB support:
  - `Database::create_in_memory(options: OpenOptions) -> Result<Database>`
  - `Database::create_ephemeral(session_name: &str, options: OpenOptions) -> Result<Database>`
  - multiple connections must see the same state
  - state must drop when the owning session/database drops
- Need explicit caller-owned temp/spill roots for query spill, sort spill,
  vector spill, and temp artifacts.
- Need documented `Send`/`Sync` and pooling contract for `Database`,
  `Connection`, and `Statement`.
- Need atomic veox task-queue semantics under contention:
  - claim once
  - priority desc + created_at asc
  - complete/fail only from claimed state
  - no duplicate claims
- Need xdoug SQL compatibility coverage for:
  - `BEGIN IMMEDIATE`
  - `UPDATE ... RETURNING`
  - `INSERT ... ON CONFLICT DO UPDATE`
  - `INSERT OR IGNORE`
  - `INSERT OR REPLACE`
  - `BLOB`, `TEXT`, `INTEGER`, nullable columns
  - JSON scalar functions used by direct tests
  - partial indexes or a documented replacement
  - `ORDER BY ... LIMIT 1` subquery inside `UPDATE`
- Need an explicit MSRV compatibility decision for the xdoug target.

## 2026-05-15 phase11 follow-up

- Added `Database::create_in_memory(options)` and
  `Database::create_ephemeral(session_name, options)` with owned temp-root
  cleanup on final drop.
- Threaded caller-owned temp roots through SQL query spill, sort spill, and
  vector spill helpers.
- Added SQL-only queue claim coverage using a single-statement atomic claim
  shape plus claimed-state completion/failure checks.
- Added the missing `UPDATE ... SET x = (SELECT ... ORDER BY ... LIMIT 1)`
  compatibility regression.
- Kept partial indexes parser-only and left the documented replacement in
  place.
- Confirmed the workspace MSRV stays at Rust `1.95`.
