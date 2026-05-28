# RedlineDB Speed Recovery - Live Pending Work

Canonical plan:
- [speed_up_workplan_FINAL.md](./speed_up_workplan_FINAL.md)

Realtime coordination:
- [AGENT_CHAT.md](./AGENT_CHAT.md)

Completed local slice:
- `SQL_ERROR_MESSAGES` cases `10555` and `10556` now fail at prepare time with `unknown column` instead of silently stepping past an empty table.

Verification:
- Targeted `parity_negative`, `smoke_select`, `jeryu_compat`, `parity_coverage`, and `parser_proptest` lanes all passed.
- Latest `redline-testing` smoke on `target/release/redlinedb` and SQLite 3.53.1 passed the missing-column cases and finished with `62` remaining failures out of `2445` total cases.

Open master-plan work that remains after this slice:
- W4 morsel/vector routing on the default SQL path.
- W6 aggregation / CTE / window / subquery runtime work.
- W7 CLI startup, output rendering, and RSS work.
- W2 build/profile/allocator strategy and the remaining W3 RQL native-path work.

Pending local cleanup:
- Re-run `just score` after the commit so the jankurai snapshot reflects the new state.
- Commit the source change, regression test, and this status file together.

Notes:
- The touched source files are limited to `crates/sql/src/parser/helpers/table/projection.rs`, `crates/sql/src/parser/helpers/table.rs`, `crates/sql/src/parser/select.rs`, and `crates/sql/tests/parity_negative.rs`.
- The current fix is intentionally narrow: it validates direct projection identifiers on single-table SELECTs and leaves correlated subquery and non-table sources on the existing execution path.
