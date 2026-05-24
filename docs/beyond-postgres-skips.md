# Beyond-Postgres skip-list policy

The redline-testing corpus's `beyond_sqlite` suite (265 cases as of 0.1.3) drives redlinedb against psql as the oracle. Most cases close cleanly — generate_series, FILTER (WHERE), LATERAL joins, regex via `crates/sql/src/regexp.rs`, JSONB via `crates/sql/src/json/scalar.rs`. A small set do not, because they require type-system or semantics surfaces that don't exist in SQLite's shape and would derail the engine's coherence.

## When to add an entry to the skip list

Add `(case_id, rationale, target_release)` to `metadata/beyond_sqlite/skip-list.toml` iff **all** are true:
1. The case requires a feature that has no SQLite-shape analog (e.g. `int[]` arrays as a first-class type, `LOCK TABLE` semantics, role-based `GRANT`).
2. Implementing it would require either (a) a new top-level type, (b) a new privilege/session model, or (c) a new system-catalog surface that doesn't fit `sqlite_master`-style introspection.
3. The author has confirmed there's no acceptable lower-fidelity port (e.g. arrays-as-blobs, GRANT-as-noop) that would still pass the case.

## When NOT to add an entry

If the case is:
- A `generate_series` / `string_agg` / `array_to_string` / regex / JSONB / LATERAL / FILTER variant — these are closable. Open them in `crates/sql/src/exec/`.
- A `FOR UPDATE` / `FOR SHARE` lock hint — closable as no-op since redlinedb's tx model handles it differently.
- A `\d` psql meta-command — these are CLI-shell concerns; either implement or document but don't skip-list.

## Cap

≤ 30 entries (~1% of 2,710 corpus). If we cross 30, the gap-closure pass has missed something — revisit scope rather than grow the list.

## How the skip list is enforced

The gap-ledger tooling (`target/redline-testing/gap-ledger.md`) reads `metadata/beyond_sqlite/skip-list.toml` and counts skipped-by-design cases separately from failing cases. Final verification (`just redline-testing-official` + report-update) expects:
- `failing_in_scope == 0`
- `skipped_by_design <= 30`
- All skip entries have non-empty `rationale`.
