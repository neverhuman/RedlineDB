# FEATURE GAPS

Append-only tracker for gaps surfaced by veox-native integration work.

## 2026-05-15

Historical gaps that were closed in the phase11 follow-up:

- Shared ephemeral in-memory DB support is implemented via
  `Database::create_in_memory(options: OpenOptions) -> Result<Database>` and
  `Database::create_ephemeral(session_name: &str, options: OpenOptions) -> Result<Database>`.
- Caller-owned temp/spill roots are threaded through query spill, sort spill,
  vector spill, and temp-artifact paths.
- `Send`/`Sync` and pooling semantics for `Database`, `Connection`, and
  `Statement` are documented.
- Veox queue semantics are covered by the SQL-only claim path and regression
  tests.
- xdoug SQL compatibility coverage now includes the `BEGIN IMMEDIATE`,
  `UPDATE ... RETURNING`, `INSERT ... ON CONFLICT DO UPDATE`,
  `INSERT OR IGNORE`, `INSERT OR REPLACE`, JSON scalar, and
  `ORDER BY ... LIMIT 1` regression cases; partial indexes remain parser-only
  with the documented replacement.
- MSRV remains Rust `1.95` for this workspace.

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
