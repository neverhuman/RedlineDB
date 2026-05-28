# RedlineDB Speed Recovery - Live Pending Work

Canonical plan:
- [speed_up_workplan_FINAL.md](./speed_up_workplan_FINAL.md)

Realtime coordination:
- [AGENT_CHAT.md](./AGENT_CHAT.md)

Completed local slice:
- `SQL_ERROR_MESSAGES` cases `10555` and `10556` now fail at prepare time with `unknown column` instead of silently stepping past an empty table.
- `SQL_BLOB` case `10630` now fails with `JSON cannot hold BLOB values`.
- `SQL_STRING` case `11410` now follows SQLite ASCII-only `lower`/`upper` semantics.
- `SQL_CAST` / `TYPE_AFFINITY` `CAST(... AS NUMERIC)` cases now return SQLite numeric storage classes, while `::numeric` keeps the PG decimal path.

Verification:
- Targeted `parity_negative`, `smoke_select`, `jeryu_compat`, `parity_coverage`, and `parser_proptest` lanes all passed.
- Latest `redline-testing` smoke on the fresh `target/fresh-cli/release/redlinedb` and SQLite 3.53.1 passed the missing-column, JSON blob, lower/upper, and SQLite `CAST(... AS NUMERIC)` cases and finished with `52` remaining failures out of `2445` total cases.

Open master-plan work that remains after this slice:
- W4 morsel/vector routing on the default SQL path.
- W6 aggregation / CTE / window / subquery runtime work.
- W7 CLI startup, output rendering, and RSS work.
- W2 build/profile/allocator strategy and the remaining W3 RQL native-path work.
- The stray `benchmark-results/sqlite-parity/baselines/v4.0.9-post-a1-a5.jsonl` should be either promoted as a named baseline artifact or deleted; it should not coexist as a second unlabeled source of truth for the same evidence family.

Pending local cleanup:
- Re-run `just score` after the commit so the jankurai snapshot reflects the new state.
- Commit the source change, regression test, and this status file together.

Notes:
- The touched source files are limited to `crates/sql/src/json/scalar.rs`, `crates/sql/src/json/jsonb.rs`, `crates/sql/src/exec/agg_eval.rs`, `crates/sql/src/exec/expr/json_dispatch.rs`, `crates/sql/src/exec/expr/program.rs`, `crates/sql/tests/parity_coverage.rs`, `crates/sql/tests/parity_negative.rs`, and `crates/sql/tests/scalar_program_vm.rs`.
- The current fix is intentionally narrow: it rejects JSON BLOB inputs at the shared builder helper, keeps lower/upper ASCII-only to match SQLite parity, and splits `NUMERIC` casts by syntax kind so SQLite `CAST` and PG `::` stay separate.
