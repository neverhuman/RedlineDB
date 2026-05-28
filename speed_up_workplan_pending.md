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

Current SQL_MATH slice:
- `SQL_MATH` cases `11037`, `11038`, and `11045` now pass on the latest `redline-testing 1.0.1` runner.
- `cosh()` and `exp()` now use the host `f64` implementations on the scalar dispatch path, matching the SQLite reference formatting for the current official binary.
- Added scalar parity coverage for the exact `cosh(1.0)`, `cosh(-1.0)`, and `exp(1.0)` precision cases.

Verification:
- `cargo test -p redlinedb-sql --test parity_scalar_funcs math1_exp_cosh_match_sqlite_3531_rendering --quiet --locked`
- `cargo test -p redlinedb-sql --test parity_scalar_funcs math1 --quiet --locked`
- `cargo check -p redlinedb-sql --quiet --locked`
- `cargo build -p redlinedb-cli --release --locked`
- Latest full `redline-testing run --suite sqlite_parity` on `target/release/redlinedb`: `13` remaining failures out of `2445`; `11037`, `11038`, and `11045` all passed with matching stdout hashes.

Next safe slices identified by read-only MCP survey:
- `10105` (`STRICT, WITHOUT ROWID` option ordering), likely low risk in parser normalization plus SQL test coverage.
- `10407` (`pragma_table_list` temp schema / `sqlite_temp_master` introspection), low-medium risk in pragma table-valued path.

Current STRICT/WITHOUT ROWID slice:
- `SQL_STRICT_TABLES` case `10105` now passes on the latest `redline-testing 1.0.1` runner.
- The SQLite table-option forms `STRICT, WITHOUT ROWID` and `WITHOUT ROWID, STRICT` are normalized to the parser's accepted internal order without rewriting quoted strings or comments.
- Added parity coverage for both option orders and a literal-safety regression.

Verification:
- `cargo test -p redlinedb-sql --test parity_scale_p0 strict_without_rowid_combo_accepts_sqlite_option_orders --quiet --locked`
- `cargo test -p redlinedb-sql --test parity_scale_p0 without_rowid_rewrite_ignores_strict_inside_literals --quiet --locked`
- `cargo test -p redlinedb-sql --test parity_scale_p0 --quiet --locked`
- `cargo test -p redlinedb-sql --test parser_proptest identifier_quoting_roundtrip --quiet --locked`
- `cargo check -p redlinedb-sql --quiet --locked`
- `cargo build -p redlinedb-cli --release --locked`
- Latest full `redline-testing run --suite sqlite_parity` on `target/release/redlinedb`: `12` remaining failures out of `2445`; `10105` passed with matching stdout hash.

Remaining latest-runner failures after this slice:
- `10234`, `10339`, `10340`, `10379`, `10388`, `10396`, `10407`, `10445`, `10451`, `10456`, `10466`, `10476`.

Current temp schema introspection slice:
- `SQL_SCHEMA_INTROSPECTION` case `10407` now passes on the latest `redline-testing 1.0.1` runner.
- `sqlite_temp_master`, `sqlite_temp_schema`, `temp.sqlite_master`, `temp.sqlite_schema`, `temp.sqlite_temp_master`, and `temp.sqlite_temp_schema` now route to the temp schema pseudo-table path.
- `pragma_table_list` now reports `temp|sqlite_temp_schema` and session temp tables, while avoiding duplicate `main` rows for temp tables stored in the shared catalog.

Verification:
- `cargo test -p redlinedb-sql --test smoke_pragma pragma_table_list_and_temp_master_report_temp_tables --quiet --locked`
- `cargo test -p redlinedb-sql --test smoke_pragma pragma_table_list_reports_without_rowid_and_strict_bits_separately --quiet --locked`
- `cargo test -p redlinedb-sql --test smoke_pragma --quiet --locked`
- `cargo check -p redlinedb-sql --quiet --locked`
- `cargo build -p redlinedb-cli --release --locked`
- Latest full `redline-testing run --suite sqlite_parity` on `target/release/redlinedb`: `11` remaining failures out of `2445`; `10407` passed with matching stdout hash.

Remaining latest-runner failures after this slice:
- `10234`, `10339`, `10340`, `10379`, `10388`, `10396`, `10445`, `10451`, `10456`, `10466`, `10476`.

Current sqlite_stat1 slice:
- `SQL_SCHEMA_INTROSPECTION` case `10396` now passes on the latest `redline-testing 1.0.1` runner.
- `sqlite_stat1` now binds as a read-only pseudo-table after `ANALYZE` publishes stats.
- Rows are generated from the stats snapshot as `(tbl, idx, stat)`, with rowid primary-key autoindexes hidden and unindexed tables exposed as `idx=NULL`.

Verification:
- `cargo test -p redlinedb-sql --test smoke_misc analyze_and_explain_return_rows --quiet --locked`
- `cargo test -p redlinedb-sql --test smoke_misc sqlite_stat1_reports_index_stats_after_analyze --quiet --locked`
- `cargo test -p redlinedb-sql --test smoke_misc --quiet --locked`
- `cargo check -p redlinedb-sql --quiet --locked`
- `cargo build -p redlinedb-cli --release --locked`
- Latest full `redline-testing run --suite sqlite_parity` on `target/release/redlinedb`: `10` remaining failures out of `2445`; `10396` passed with matching stdout hash.

Remaining latest-runner failures after this slice:
- `10234`, `10339`, `10340`, `10379`, `10388`, `10445`, `10451`, `10456`, `10466`, `10476`.

Current compound set-op slice:
- `SQL_COMPOUND` case `10476` now passes on the latest `redline-testing 1.0.1` runner.
- Unparenthesized mixed compound operations are normalized to SQLite left-to-right grouping before binding.
- Explicit `SetExpr::Query` wrappers remain boundaries, so parenthesized query grouping is not rewritten.

Verification:
- `cargo test -p redlinedb-sql --test parity_compound_select --quiet --locked`
- `cargo test -p redlinedb-sql --test smoke_select --quiet --locked`
- `cargo check -p redlinedb-sql --quiet --locked`
- `rustfmt --edition 2024 --check crates/sql/src/parser/select.rs crates/sql/tests/parity_compound_select.rs`
- `cargo build -p redlinedb-cli --release --locked`
- Direct batch replay of `SELECT 1 UNION SELECT 2 INTERSECT SELECT 2;` on `target/release/redlinedb`: output `2`.
- Latest full `redline-testing run --suite sqlite_parity` on `target/release/redlinedb`: `9` remaining failures out of `2445`; `10476` passed with matching stdout hash.

Proof-lane caveat:
- `just fast` was attempted and failed on a workspace rustfmt check in pre-existing files outside this semantic slice. Raw log: `~/.local/share/rtk/tee/1779976046_just_fast.log`.

Remaining latest-runner failures after this slice:
- `10234`, `10339`, `10340`, `10379`, `10388`, `10445`, `10451`, `10456`, `10466`.

Current rowid qualifier slice:
- `SQL_JOIN` case `10456` now passes on the latest `redline-testing 1.0.1` runner.
- The rowid equality fast path is qualifier-aware, so a correlated outer reference such as `a.id` is not mistaken for the scanned inner table `b`'s rowid alias while planning `WHERE b.a_id = a.id`.
- Added focused differential coverage for the official correlated-subquery shape.

Verification:
- `cargo test -p redlinedb-sql --test differential_lab diff_correlated_subquery_outer_pk_is_not_inner_rowid_alias --quiet --locked`
- `cargo test -p redlinedb-sql --test differential_lab diff_subquery_matrix --quiet --locked`
- `cargo test -p redlinedb-sql --test smoke_select --quiet --locked`
- `cargo check -p redlinedb-sql --quiet --locked`
- `jankurai audit-file . --path crates/sql/src/planner/helpers.rs --mode save-gate`
- `jankurai audit-file . --path crates/sql/tests/differential_lab.rs --mode save-gate`
- `cargo build -p redlinedb-cli --release --locked`
- Direct batch replay of the official correlated-subquery shape on `target/release/redlinedb`: outputs `1|one|101`, `2|two|200`, `3|three|NULL`.
- Latest full `redline-testing run --suite sqlite_parity` on `target/release/redlinedb`: `8` remaining failures out of `2445`; `10456` passed with matching stdout hash.
- `just fast`

Remaining latest-runner failures after this slice:
- `10234`, `10339`, `10340`, `10379`, `10388`, `10445`, `10451`, `10466`.

Current CLI deserialize slice:
- `CLI_OPTION` case `10234` now passes on the latest `redline-testing 1.0.1` runner.
- `redlinedb -deserialize :memory:` now mirrors SQLite's legacy `Error: out of memory` stderr while still executing successfully.
- The compatibility warning is limited to explicit `:memory:` deserialize mode; `-deserialize ''` remains quiet, matching SQLite.
- Added CLI subprocess coverage for the official shape.

Verification:
- `cargo test -p redlinedb-cli --test dot_commands deserialize_memory_mode_emits_sqlite_oom_warning_and_continues --quiet --locked`
- `cargo test -p redlinedb-cli --test dot_commands --quiet --locked`
- `cargo check -p redlinedb-cli --quiet --locked`
- `cargo build -p redlinedb-cli --release --locked`
- Direct release-binary replay for `-deserialize :memory:`: stdout `1`, stderr `Error: out of memory`, exit `0`.
- Direct release-binary replay for `-deserialize ''`: stdout `1`, empty stderr, exit `0`.
- Latest full `redline-testing run --suite sqlite_parity` on `target/release/redlinedb`: `7` remaining failures out of `2445`; `10234` passed with matching stdout and stderr hashes.

Remaining latest-runner failures after this slice:
- `10339`, `10340`, `10379`, `10388`, `10445`, `10451`, `10466`.

Current attach update/delete slice:
- `SQL_ATTACH` case `10388` now passes on the latest `redline-testing 1.0.1` runner.
- Simple alias-qualified `UPDATE aux.table ...` and `DELETE FROM aux.table ...` route through the existing attached-sidecar `CrossDbSql` template before local DML binding rejects cross-database writes.
- Routing is limited to direct alias-qualified targets without `RETURNING`; broader cross-db write shapes still fall back to the existing unsupported path.
- Added attach parity coverage proving main rows remain untouched while aux rows are updated/deleted.

Verification:
- `cargo test -p redlinedb-sql --test parity_attach alias_qualified_update_delete_routes_to_attached_database --quiet --locked`
- `cargo test -p redlinedb-sql --test parity_attach --quiet --locked`
- `cargo check -p redlinedb-sql --quiet --locked`
- `cargo build -p redlinedb-cli --release --locked`
- Latest full `redline-testing run --suite sqlite_parity` on `target/release/redlinedb`: `6` remaining failures out of `2445`; `10388` passed with matching stdout and stderr hashes.

Remaining latest-runner failures after this slice:
- `10339`, `10340`, `10379`, `10445`, `10451`, `10466`.

Current attach insert-select slice:
- `SQL_ATTACH` case `10379` now passes on the latest `redline-testing 1.0.1` runner.
- Added a narrow `CrossDbInsertSelect` plan for `INSERT INTO aux.table [cols] SELECT ...`; the source SELECT materializes on the main connection and inserts into the attached sidecar with bound values inside one sidecar transaction.
- Added safety coverage for parameter binding, empty-source arity validation, active transaction/savepoint rejection, unsupported modifier rejection, and `last_insert_rowid()` mirroring.
- Existing sidecar SQL routing remains responsible for `INSERT aux.t VALUES (...)`, DDL, update/delete; UPSERT, RETURNING, and broader cross-db planning remain out of scope.

Verification:
- `cargo fmt --all --check`
- `cargo test -p redlinedb-sql --test parity_attach cross_db_insert_select --quiet --locked`
- `cargo test -p redlinedb-sql --test parity_attach --quiet --locked`
- `cargo check -p redlinedb-sql --quiet --locked`
- `cargo build -p redlinedb-cli --release --locked`
- `jankurai audit-file` save-gates on `crates/sql/src/statement.rs`, `crates/sql/src/parser/templates.rs`, `crates/sql/src/exec/mod.rs`, `crates/sql/src/planner.rs`, and `crates/sql/tests/parity_attach.rs`
- Latest full `redline-testing run --suite sqlite_parity` on `target/release/redlinedb`: `5` remaining failures out of `2445`; `10379` passed with matching stdout and stderr hashes.
- Raw result: `target/redline-testing/attach-insert-select-v2/sqlite_parity.raw.jsonl`, sha256 `b890eddb15f50bfb1f1ff1b19140ca512fba2b04fbfe9f9370b93442d759e0cb`

Remaining latest-runner failures after this slice:
- `10339`, `10340`, `10445`, `10451`, `10466`.

Current UPSERT ordered-arm slice:
- `SQL_UPSERT` case `10339` now passes on the latest `redline-testing 1.0.1` runner.
- Chained `ON CONFLICT` arms are preserved in source order and executor dispatch applies the first arm whose target matches the actual unique conflict.
- Targetless `ON CONFLICT` arms are rejected unless final; final targetless arms still catch otherwise-unmatched unique conflicts.
- Anonymous parameter slots stay in SQL text order across VALUES/source, skipped arms, matching arms, arm WHERE predicates, and RETURNING.
- The ON CONFLICT pre-parser scanner is now whitespace/comment tolerant, ignores quoted/commented `on conflict` text, and is byte-safe for non-ASCII SQL literals.

Verification:
- `cargo fmt --all --check`
- `cargo test -p redlinedb-sql --test phase10_sqlc_conflict_matrix multiple_on_conflict_clauses --quiet --locked`
- `cargo test -p redlinedb-sql --test phase10_sqlc_conflict_matrix --quiet --locked`
- `cargo test -p redlinedb-sql --test parity_scalar_funcs --quiet --locked`
- `cargo test -p redlinedb-sql --test smoke_dml upsert_and_conflict_algorithms_work --quiet --locked`
- `cargo test -p redlinedb-sql --test smoke_select --quiet --locked`
- `cargo check -p redlinedb-sql --quiet --locked`
- `cargo build -p redlinedb-cli --release --locked`
- `just fast`
- `jankurai audit-file` save-gates on all touched source/test files
- Latest full `redline-testing run --suite sqlite_parity` on `target/release/redlinedb`: `4` remaining failures out of `2445`; `10339` passed.
- Raw result: `target/redline-testing/upsert-ordered-arms-v6/sqlite_parity.raw.jsonl`, sha256 `4d2de4e4d46bbedca8bba9a02927b2b96ce14beefd5dafa729851c13766522be`.

Remaining latest-runner failures after this slice:
- `10340`, `10445`, `10451`, `10466`.

Next safe slice:
- NATURAL/USING join merged-column output and bare-name lookup for `10445`, `10451`, and `10466`.
- Defer `10340` until a dedicated collated UNIQUE index key slice; it is not safe as an UPSERT-only patch.
