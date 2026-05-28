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
- The stray `benchmark-results/sqlite-parity/baselines/v4.0.9-post-a1-a5.jsonl` was deleted. The promoted baseline bundle remains the canonical copy for that evidence family.

Pending local cleanup:
- Re-run `just score` after the commit so the jankurai snapshot reflects the new state.
- Commit the source change, regression test, and this status file together.

Notes:
- The touched source files are limited to `crates/sql/src/json/scalar.rs`, `crates/sql/src/json/jsonb.rs`, `crates/sql/src/exec/agg_eval.rs`, `crates/sql/src/exec/expr/json_dispatch.rs`, `crates/sql/src/exec/expr/program.rs`, `crates/sql/tests/parity_coverage.rs`, `crates/sql/tests/parity_negative.rs`, and `crates/sql/tests/scalar_program_vm.rs`.
- The current fix is intentionally narrow: it rejects JSON BLOB inputs at the shared builder helper, keeps lower/upper ASCII-only to match SQLite parity, and splits `NUMERIC` casts by syntax kind so SQLite `CAST` and PG `::` stay separate.

Current slice:
- `SQL_AUTOINCREMENT` cases `10062` and `10063` now pass on the freshly built `target/release/redlinedb`.
- The fix is intentionally limited to ordinary `INTEGER PRIMARY KEY` rowid reuse after delete/delete-all; the true `AUTOINCREMENT` keyword path is still part of the remaining plan.

Current AUTOINCREMENT slice:
- `sqlite_sequence` is now visible in batch-mode CLI and library paths after creating an `AUTOINCREMENT` table, and AUTOINCREMENT rowid allocation is monotonic off the per-table sequence state.
- Added CLI coverage in `crates/cli/tests/dot_commands.rs` for the batch shell path, plus the existing library regression in `crates/sql/tests/jeryu_schema_compat.rs`.

Verification:
- `cargo test -p redlinedb-cli --test dot_commands batch_autoincrement_exposes_sqlite_sequence --quiet --locked`
- `cargo test -p redlinedb-sql --test jeryu_schema_compat autoincrement --quiet --locked`
- `cargo test -p redlinedb-sql --test phase10_sqlc_conflict_matrix integer_pk_reuses_deleted_max_rowid --quiet --locked`
- `cargo test -p redlinedb-cli --test dot_commands --quiet --locked`
- batch repro on `target/release/redlinedb` and `target/release/redlinedb-cli` now yields `0` then `t|1` for `SELECT count(*) FROM sqlite_sequence;` / `SELECT name, seq FROM sqlite_sequence;`

Remaining next work:
- continue with the remaining SQL parity gaps outside AUTOINCREMENT; W4/W6/W7/W2 still dominate the open plan lanes.

Verification:
- Targeted `phase10_sqlc_conflict_matrix` lane passed.
- Latest official `sqlite_parity` run on the fresh CLI binary finished with `50` remaining failures out of `2445` total cases.

Next safe phase:
- Continue with the remaining SQL parity gaps outside `SQL_AUTOINCREMENT`, with W4/W6/W7/W2 still the dominant open plan lanes.

Current attach slice:
- `.databases` now lists attached aliases using the same `PRAGMA database_list` data path the shell already trusts.
- `PRAGMA aux.user_version` and `PRAGMA aux.schema_version` now route to the attached sidecar instead of the main database.
- Official `sqlite_parity` on the rebuilt CLI binary is down to `47` remaining failures out of `2445`.

Verification:
- `cargo test -p redlinedb-cli --test dot_commands --quiet --locked`
- `cargo test -p redlinedb-sql --test parity_attach --quiet --locked`
- Fresh official run on `target/release/redlinedb`:
  - `10062`, `10063`, `10381`, `10385`, and `10387` are fixed
  - `10388` (`ALIAS_QUALIFIED_UPDATE_DELETE`) still remains

Remaining attach gap:
- Alias-qualified UPDATE/DELETE across attached databases is still a larger cross-db DML routing problem and should be claimed separately, not as part of this shell/pragma slice.

Current DDL pattern slice:
- `SQL_PATTERN` case `10605` (`LIKE_INSIDE_CHECK_CONSTRAINT`) is now fixed on the fresh release binary.
- `CHECK(x LIKE 'a%')` compiles through the kernel DDL expression path and validates rows at write time using the active session's `case_sensitive_like` setting.

Verification:
- `cargo test -p redlinedb-sql --test phase10_sqlc_conflict_matrix --quiet --locked`
- `cargo test -p redlinedb-sql --test parity_negative --quiet --locked`
- `cargo check -p redlinedb-sql --quiet --locked`
- Fresh official `sqlite_parity` run on `target/release/redlinedb`: `26` remaining failures out of `2445`

Open follow-up after this slice:
- `SQL_MATH` `cosh`/`exp` precision cases still remain.
- The main unresolved plan lanes are still W4, W6, W7, and W2, plus the remaining attach/DML and AUTOINCREMENT gaps in the parity corpus.

Current sqlite_sequence source slice:
- AUTOINCREMENT source WIP is commit-ready after MCP audit fixes.
- `sqlite_sequence` is database-scoped across sibling connections, transaction-local for rollback/savepoint reads, visible in `sqlite_schema`, and supports aliases.
- Failed statements inside explicit transactions restore sequence state.
- Rowid UPDATE no longer visibly bumps `sqlite_sequence`; the next omitted AUTOINCREMENT insert still considers the live max rowid.

Verification:
- `cargo check -p redlinedb-sql --quiet --locked`
- `cargo test -p redlinedb-kernel --tests --quiet --locked`
- `cargo test -p redlinedb-sql --test jeryu_schema_compat --quiet --locked`
- `cargo test -p redlinedb-sql --test phase10_sqlb --quiet --locked`
- `cargo test -p redlinedb-sql --test phase10_sqlc_conflict_matrix integer_pk_reuses_deleted_max_rowid --quiet --locked`
- `cargo test -p redlinedb-cli --test dot_commands batch_autoincrement_exposes_sqlite_sequence --quiet --locked`
- `git diff --check` on the intended source/test set

Next safe slice:
- `SQL_AUTOINCREMENT` case `10070` (`changes()` / `total_changes()`), isolated to scalar dispatch and `smoke_misc` coverage.

Current changes/total_changes slice:
- `SQL_AUTOINCREMENT` case `10070` is fixed on latest `redline-testing 1.0.1`.
- `changes()` and `total_changes()` now resolve as zero-argument scalar functions.
- DML row counters now match SQLite's last-statement and cumulative row-change semantics; DDL no longer increments them.

Verification:
- `cargo test -p redlinedb-sql --test smoke_misc changes_and_total_changes_scalar_functions_track_dml --quiet --locked`
- `cargo test -p redlinedb-sql --test smoke_misc --quiet --locked`
- `cargo test -p redlinedb-sql --test phase10_sqlb --quiet --locked`
- `cargo check -p redlinedb-sql --quiet --locked`
- `cargo build -p redlinedb-cli --release --locked`
- Latest full `redline-testing run --suite sqlite_parity` on `target/release/redlinedb`: `10070` passed; total failures now `16 / 2445`.

Next safe slice:
- SQL_MATH precision cluster: `11037`, `11038`, `11045`.
