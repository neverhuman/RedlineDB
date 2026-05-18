# SQLite Parity Traceability Ledger

This ledger records RedlineDB's current SQLite-facing compatibility status.
The reference oracle is the SQLite library bundled with `rusqlite` in the test
harness. Proof artifacts under `target/proof/sqlite-full-parity/` should record
`sqlite_version()`, `PRAGMA compile_options`, ignored tests, `UnsupportedSql`
sites, and the SQLLogicTest inventory for each parity pass.

Status values are deliberately narrow:

| Status | Meaning |
|---|---|
| `pass` | Covered by an executable test and expected to match the bundled SQLite reference for the stated row. |
| `fail` | SQLite supports the row, but RedlineDB currently rejects it or implements only a documented subset. |
| `not-started` | No production implementation exists yet, or the current implementation intentionally uses RedlineDB-native behavior instead of SQLite behavior. |

## SQL Surface

| Feature row | Status | Test path | Owner | Notes |
|---|---|---|---|---|
| Basic `SELECT` projection/filter/order | pass | `crates/sql/tests/smoke_select.rs`, `crates/sql/tests/differential_lab.rs` | sql-parser-planner-executor | Covered for scalar values and simple predicates. |
| `INSERT`, `UPDATE`, `DELETE` basics | pass | `crates/sql/tests/smoke_dml.rs`, `crates/bench/compat/**` | sql-parser-planner-executor | Cross-engine SQLLogicTest corpus covers representative DML. |
| `CREATE TABLE`, `DROP TABLE` basics | pass | `crates/bench/compat/**`, `crates/sql/tests/parity_coverage.rs` | sql-parser-planner-executor | SQLite metadata compatibility is not complete. |
| `CREATE INDEX`, `DROP INDEX` basics | pass | `crates/sql/tests/parity_coverage.rs` | sql-parser-planner-executor | Basic B-tree-backed indexes only. |
| `ALTER TABLE RENAME TABLE/COLUMN` | pass | `crates/sql/tests/parity_coverage.rs` | sql-parser-planner-executor | Add/drop-column variants remain partial. |
| `RETURNING` expressions | pass | `crates/sql/tests/parity_coverage.rs` | sql-parser-planner-executor | Insert/update expressions covered. |
| UPSERT `ON CONFLICT DO UPDATE` representative path | pass | `crates/bench/compat/orm/migration.sqlt`, `crates/sql/tests/phase10_sqlc_conflict_matrix.rs` | sql-parser-planner-executor | Full SQLite conflict matrix is still incomplete. |
| Savepoints | pass | `crates/sql/tests/parity_coverage.rs`, `crates/sql/tests/phase10_sqlb.rs` | sql-parser-planner-executor | Nested savepoint behavior has focused coverage. |
| Joins and left joins | pass | `crates/sql/tests/parity_coverage.rs`, `crates/bench/compat/orm/queries.sqlt` | sql-parser-planner-executor | Natural joins are explicitly rejected. |
| Row-value `IN` subqueries | pass | `crates/sql/tests/parity_coverage.rs`, `crates/sql/tests/differential_lab.rs` | sql-parser-planner-executor | Representative row-value `IN`/`NOT IN` covered. |
| Correlated subqueries | pass | `crates/sql/tests/differential_lab.rs` | sql-parser-planner-executor | A7: thread-local outer-row stack resolves qualified outer-scope references; correlated `EXISTS` / `NOT EXISTS` and correlated scalar subqueries in projection covered against the rusqlite oracle. |
| CTEs (`WITH`, recursive and non-recursive) | pass | `crates/sql/tests/parity_cte.rs` | sql-parser-planner-executor | A3: non-recursive + recursive (UNION / UNION ALL) materialized into thread-local row store; supports JOIN against CTE via synthetic TableDef. Iteration cap 10_000. |
| Compound `SELECT` (`UNION`, `INTERSECT`, `EXCEPT`) | pass | `crates/sql/tests/parity_compound_select.rs`, `crates/sql/tests/parity_order_by_ordinal.rs` | sql-parser-planner-executor | A2: UNION / UNION ALL / INTERSECT / EXCEPT all wired through the parser and exec layers; column-arity and type-class mismatches return diagnostic errors that match SQLite's error class. Top-level integer literals in trailing `ORDER BY` resolve as 1-based output-column references (single-branch and compound), matching SQLite. |
| Window functions and frames | pass | `crates/sql/tests/parity_window.rs` | sql-parser-planner-executor | A4: ROW_NUMBER / RANK / DENSE_RANK / NTILE / LAG / LEAD / FIRST_VALUE / LAST_VALUE / NTH_VALUE / PERCENT_RANK / CUME_DIST + aggregate-OVER (SUM/COUNT/AVG/MIN/MAX/TOTAL) with ROWS / RANGE / GROUPS frames. |
| Views | pass | `crates/sql/tests/parity_view.rs` | sql-parser-planner-executor | A5-views: `CREATE [TEMP] VIEW [IF NOT EXISTS]` + `DROP VIEW [IF EXISTS]` supported. Views persist in the catalog (format_version 4) and expand at FROM-binding time as derived row sources. DML on a view returns a SQLite-style "cannot modify view" error. Followup: runtime (not bind-time) materialization so cached prepared statements observe fresh rows after data changes. |
| Triggers | pass | `crates/sql/tests/parity_trigger.rs` | sql-parser-planner-executor | A5-triggers: `CREATE TRIGGER {BEFORE\|AFTER} {INSERT\|UPDATE [OF col,...]\|DELETE} ON table FOR EACH ROW [WHEN expr] BEGIN body END` + `DROP TRIGGER`. Persisted in the catalog (format_version 6). Fire-hook in INSERT/UPDATE/DELETE runs every matching trigger in `rowid` order; `OLD`/`NEW` rows are bound as outer-row contexts so the body resolves `OLD.col`/`NEW.col` via the qualified-identifier path. UPDATE OF column filter skips firing when no listed column changed. Recursion cap on `Txn::trigger_depth` (default 32 in debug, well under SQLite's 1000 but matched to Rust's debug stack). Followup: `INSTEAD OF` on views; raise the depth cap with stack-grown threads in release builds. |
| Generated columns | pass | `crates/sql/tests/parity_generated_col.rs` | sql-parser-planner-executor | A6 SQL-D: `STORED` and `VIRTUAL` generated columns parse, persist (catalog format_version 7), recompute on INSERT/UPDATE (STORED), and evaluate on read (VIRTUAL). Writes targeting generated columns are rejected with a SQLite-class error. |
| Partial indexes | pass | `crates/sql/tests/parity_partial_index.rs` | sql-parser-planner-executor | A6 SQL-D: `CREATE INDEX ... WHERE <predicate>` persists the predicate as verbatim SQL on `IndexDef.predicate_sql`; DML re-parses and evaluates the predicate per row (`crates/sql/src/exec/index_predicate.rs`) so partial indexes track in/out membership precisely, and reads only use the index when the query predicate implies the index predicate (`crates/sql/src/exec/index_partial.rs`). |
| Expression indexes | pass | `crates/sql/tests/parity_expr_index.rs` | sql-parser-planner-executor | A6 SQL-D: `CREATE INDEX ... ON t(expr(col))` stashes the expression SQL on `IndexKeySource::Expression`; DML re-evaluates the expression to compute the index key, and UPDATE re-emits when any referenced column changes. |
| Foreign keys | pass | `crates/sql/tests/parity_fk_enforce.rs`, `crates/sql/tests/phase10_sqld_fk.rs` | sql-parser-planner-executor | A6-fk: PRAGMA-gated enforcement on INSERT/UPDATE/DELETE; ON DELETE/UPDATE {NO ACTION, RESTRICT, CASCADE, SET NULL, SET DEFAULT}; DEFERRABLE INITIALLY DEFERRED checked at COMMIT. Cascade depth bounded. |
| ATTACH / DETACH | pass | `crates/sql/tests/parity_attach.rs` | sql-parser-planner-executor | A2: per-connection alias map (`crate::exec::attach::AttachMap`) opens a sidecar [`Database`] per `ATTACH DATABASE 'file' AS alias`; DETACH drops the handle. Reserved aliases `main` / `temp` return a SQLite-style error class. Cross-database `SELECT` and JOIN over `alias.table` resolve through `crate::exec::cross_db::try_resolve_cross_db_bound_table`, which materializes the sidecar rows at bind time (same registry as views/CTEs). Cross-database writes are rejected with a clear "not yet supported" error pending a follow-up. |

## Expressions And Functions

| Feature row | Status | Test path | Owner | Notes |
|---|---|---|---|---|
| NULL comparison and `IN`/`NOT IN` edge cases | pass | `crates/sql/tests/parity_coverage.rs` | sql-parser-planner-executor | SQLite three-valued logic has focused coverage. |
| Core string scalars (`substr`, `trim`, `instr`, `replace`, case conversion, length) | pass | `crates/sql/tests/parity_scalar_funcs.rs`, `crates/sql/tests/differential_lab.rs` | sql-parser-planner-executor | Representative SQLite behavior covered. |
| Formatting and conditional scalars (`printf`, `format`, `iif`, `sign`) | pass | `crates/sql/tests/parity_scalar_funcs.rs`, `crates/sql/tests/differential_lab.rs` | sql-parser-planner-executor | Does not imply exhaustive SQLite format coverage. |
| Blob/character helpers (`zeroblob`, `randomblob`, `char`, `unicode`) | pass | `crates/sql/tests/parity_scalar_funcs.rs` | sql-parser-planner-executor | Randomness is shape-tested, not value-matched. |
| Aggregate functions (`count`, `sum`, `total`, `min`, `max`) | pass | `crates/sql/tests/parity_agg_funcs.rs`, `crates/sql/tests/differential_lab.rs` | sql-parser-planner-executor | Representative grouped and NULL behavior covered. |
| JSON aggregate functions | pass | `crates/sql/tests/parity_agg_funcs.rs` | phase10-json1-surface | JSON aggregate rows covered. |
| JSON scalar functions | pass | `crates/sql/tests/phase10_j1_compat.rs`, `crates/sql/tests/parity_json1.rs`, `crates/sql/src/json/scalar.rs` | phase10-json1-surface | `parity_json1.rs` runs a rusqlite-oracle differential pass over `json()` / `json_array[_length]` / `json_object` / `json_extract` / `json_type` / `json_valid` / `json_quote` / `json_set` / `json_insert` / `json_replace` / `json_remove` / `json_patch` and the `->` / `->>` arrow operators. |
| Date/time functions | pass | `crates/sql/tests/phase10_sqld_datetime.rs`, `crates/sql/src/datetime.rs` | phase10-datetime | A8: `date()` / `time()` / `datetime()` / `julianday()` / `unixepoch()` / `strftime()` accept the SQLite time-string formats and modifiers (`'now'`, `'+/-N {days,hours,...}'`, `'start of {day,month,year}'`, `'weekday N'`, `'utc'`, `'localtime'`) through the shared modifier pipeline in `crates/sql/src/datetime/modifiers.rs`; differential parity against the rusqlite oracle is wired and green. |
| Collations | fail | `crates/sql/tests/phase10_sqld_collation.rs` | phase10-collations | Built-in collation behavior is partial. |
| User-defined SQL functions/collations | pass | `crates/ffi/tests/udf_register.rs`, `crates/ffi/tests/collation_register.rs` | c-abi | B2: `sqlite3_create_function{,_v2,16}` registers scalar UDFs; `sqlite3_create_collation*` + `sqlite3_collation_needed` register collation callbacks. B4 dispatch path routes the registered C callbacks through the SQL evaluator so user-defined functions and collations are invoked end-to-end from prepared statements. |

## PRAGMAs

| Feature row | Status | Test path | Owner | Notes |
|---|---|---|---|---|
| `PRAGMA integrity_check` / `quick_check` | pass | `crates/sql/tests/parity_coverage.rs` | sql-parser-planner-executor | Current checks are RedlineDB-native integrity summaries. |
| `PRAGMA auto_vacuum` (rejected) | pass | `crates/sql/tests/parity_pragma_tv.rs` | sql-parser-planner-executor | Previously returned a fabricated `0` row; now `UnsupportedSql`. |
| `PRAGMA wal_checkpoint(MODE)` (rejected) | pass | `crates/sql/tests/parity_pragma_tv.rs` | storage-and-catalog | Previously returned fabricated `(busy, log, checkpointed)`; RedlineDB has no WAL journal so the PRAGMA is rejected naming WAL as the missing prerequisite. |
| `PRAGMA journal_mode` (memory/off/delete) | pass | `crates/sql/tests/parity_pragma_tv.rs` | sql-parser-planner-executor | Round-trip on the supported subset; `wal`/`truncate`/`persist` rejected with a message naming the value. |
| `PRAGMA synchronous` | pass | `crates/sql/tests/parity_pragma_tv.rs` | sql-parser-planner-executor | Accepts `OFF`/`NORMAL`/`FULL`/`EXTRA` plus integer aliases; stored for read-back. |
| `PRAGMA temp_store` | pass | `crates/sql/tests/parity_pragma_tv.rs` | sql-parser-planner-executor | Accepts `DEFAULT`/`FILE`/`MEMORY` plus integer aliases. |
| `PRAGMA cache_size` | pass | `crates/sql/tests/parity_pragma_tv.rs` | sql-parser-planner-executor | Signed integer round-trip (negative = KiB, positive = pages). |
| `PRAGMA query_only` | pass | `crates/sql/tests/parity_pragma_tv.rs` | sql-parser-planner-executor | When ON, blocks every write-side statement with a message containing `query_only`. |
| Unknown PRAGMA names | pass | `crates/sql/tests/parity_pragma_tv.rs` | sql-parser-planner-executor | All unsupported PRAGMAs now return `UnsupportedSql` naming the PRAGMA. |
| `PRAGMA foreign_keys` | pass | `crates/sql/tests/parity_fk_enforce.rs`, `crates/sql/tests/phase10_sqld_fk.rs` | sql-parser-planner-executor | A6: per-connection toggle now gates the FK enforcement layer end-to-end; OFF skips every check (matches SQLite's bundled default). |
| Table-valued PRAGMAs | pass | `crates/sql/tests/parity_pragma_tv.rs` | sql-parser-planner-executor | C2: `PRAGMA table_info(...)`, `PRAGMA index_list(...)`, `PRAGMA index_info(...)`, `PRAGMA foreign_key_list(...)` and the rest of the table-valued PRAGMA family produce row sets with column names and ordering that match the rusqlite oracle. |
| Full reference-build PRAGMA set | not-started | `crates/sql/tests/sqlite_full_parity.rs` metadata only | sql-parser-planner-executor | Needs generated corpus from `PRAGMA compile_options` and SQLite docs. |

## File Format, Durability, C API, And CLI

| Feature row | Status | Test path | Owner | Notes |
|---|---|---|---|---|
| SQLite database header/pages/btrees/records | not-started | none | storage-and-catalog | Current files use RedlineDB-native page/control/WAL formats, not `SQLite format 3`. |
| SQLite rollback journal compatibility | not-started | none | storage-and-catalog | No SQLite rollback-journal reader/writer exists. |
| SQLite WAL compatibility and recovery | not-started | none | storage-and-catalog | RedlineDB has a native group-commit WAL, not SQLite WAL frames. |
| Cross-open RedlineDB-created files with SQLite CLI | not-started | none | storage-and-catalog | Requires SQLite file-format writer. |
| Cross-open SQLite-created files with RedlineDB | not-started | none | storage-and-catalog | Requires SQLite pager/btree/record reader. |
| Covered `sqlite3_*` open/prepare/step/finalize aliases | pass | `crates/ffi/tests/**`, `contracts/c-abi/redlinedb.h` | c-abi | Covered ABI subset only. Header edits require an exception receipt. |
| Broad `sqlite3_*` API surface | pass | `crates/ffi/src/sqlite3_api/**`, `crates/ffi/tests/**` | c-abi | B1-B5: 36 additional `sqlite3_*` symbols implemented across `sqlite3_api/{result,value,context,udf,collation,blob,hooks,hooks_fire,bind,column,core,exec,meta,stmt}.rs`. Covers UDF context + result family, value extraction, blob I/O, collation registration, per-connection hooks (`commit`, `rollback`, `update`, `trace`, `profile`, `busy_handler`, `set_authorizer`), and the remaining stmt/meta/exec aliases. Symbol-allowlist tests (`crates/ffi/tests/symbol_diff.rs`) enforce the surface. |
| CLI one-shot query/stats/backup commands | pass | `crates/cli` smoke lanes via `agent/test-map.json` | cli-shell | SQLite shell scripting compatibility is not complete. |
| SQLite shell dot-command compatibility | pass | `crates/cli/tests/dot_commands.rs` | cli-shell | 25 dot-commands wired through `crates/cli/src/dot/{mod,control,display,io_cmd,schema,parameter}.rs`; adds `.fullschema` (schema + `sqlite_master`), `.once FILE` (one-shot redirect plumbed through `run_query_with_state`), and `.parameter set|unset|init|clear|list` (bindings applied via `bind_named` on the next statement). |

## Required Receipts

For any parity change, keep or regenerate:

- `target/proof/sqlite-full-parity/git-status.txt`
- `target/proof/sqlite-full-parity/diff-stat.txt`
- `target/proof/sqlite-full-parity/rusqlite-reference.txt`
- `target/proof/sqlite-full-parity/unsupported-sql-sites.txt`
- `target/proof/sqlite-full-parity/ignored-tests.txt`
- `target/proof/sqlite-full-parity/sqllogictest-inventory.txt`
- `target/proof/sqlite-full-parity/sql-parity-tests.txt`
