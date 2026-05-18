# Work-Order Ledger

Agent-authored tracking file. Entries are pending documentation of observed gaps only; no product fixes are bundled here.

## <pending> WO-001: Make Full SQL Parity Fatal On Divergence

- Area: SQL parity gate
- Severity: High
- Confidence: High
- Evidence: `crates/sql/tests/parity_oracle.rs:13` says per-file mismatches are intentionally non-fatal; `crates/sql/tests/parity_oracle.rs:93` records diffs into `report.failures`; `crates/sql/tests/parity_oracle.rs:157` only asserts corpus floors, not zero divergences.
- Change: Tighten the full parity oracle so `sql-parity-full` fails on any mismatch while preserving the raw divergence receipt.
- Test: `rtk cargo test -p redlinedb-sql --test parity_oracle --quiet --locked`

## Completed WO-002: Make FFI Symbol-Diff Lane Run Ignored Test

- Area: FFI parity gate
- Severity: High
- Confidence: High
- Evidence: `crates/ffi/tests/symbol_diff.rs:164` says the proof lane must run `cargo test ... -- --ignored`; the only test is `#[ignore]` at `crates/ffi/tests/symbol_diff.rs:169`; `scripts/just/run.sh:131` invokes `symbol_diff` without `--ignored`.
- Change: Updated `ffi-symbol-diff` to generate its reference symbols, build the FFI cdylib, pass `-- --ignored`, and keep the lane failing on uncovered/unallowlisted symbols.
- Test: `rtk env CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=cc RUSTFLAGS= just ffi-symbol-diff` passed.
- Published: PR `#16`, commit `2a9fcb7` (`fix ffi parity gate`).

## Completed WO-003: Replace Missing FFI Parity-Oracle Target

- Area: FFI parity gate
- Severity: High
- Confidence: High
- Evidence: `scripts/just/run.sh:128` references `redlinedb-ffi --test parity_oracle`; `rtk rg --files crates/ffi/tests` lists no `parity_oracle.rs` test target.
- Change: Repointed `ffi-parity-full` to the existing FFI ABI suite plus the dedicated symbol-diff lane, removing the missing `parity_oracle` target reference.
- Test: `rtk env CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=cc RUSTFLAGS= just ffi-parity-full` passed.
- Published: PR `#16`, commit `2a9fcb7` (`fix ffi parity gate`).

## <pending> WO-004: Prevent Fuzz Parity From Blessing First-Run Divergences

- Area: Fuzz parity gate
- Severity: High
- Confidence: High
- Evidence: `crates/bench/tests/fuzz_parity.rs:292` treats a missing baseline as first run; `crates/bench/tests/fuzz_parity.rs:408` documents that a pristine repo records the current rate and passes; `crates/bench/tests/fuzz_parity.rs:410` sets `gate_failed` to `false` when no baseline exists.
- Change: Require zero divergences or a checked-in/explicitly provided baseline before the fuzz lane can pass.
- Test: `rtk just fuzz-parity`

## <pending> WO-005: Remove Fuzz Skips For Implemented CTE And Compound SELECT

- Area: Fuzz parity coverage
- Severity: Medium
- Confidence: High
- Evidence: `crates/bench/tests/fuzz_parity.rs:260` skips `WITH`; `crates/bench/tests/fuzz_parity.rs:264` skips compound SELECT variants; `docs/sqlite-parity.md:32` and `docs/sqlite-parity.md:33` now mark CTEs and compound SELECT as pass.
- Change: Delete stale known-skip cases and let fuzz parity exercise the implemented SQL surfaces.
- Test: `rtk just fuzz-parity`

## <pending> WO-006: Refill sqlite_full_parity Known-Gap Sentinel

- Area: SQL parity traceability
- Severity: Medium
- Confidence: High
- Evidence: `crates/sql/tests/sqlite_full_parity.rs:201` keeps a known-gap test, but `crates/sql/tests/sqlite_full_parity.rs:212` says the fixture list is empty; `docs/sqlite-parity.md:55`, `docs/sqlite-parity.md:73`, and `docs/sqlite-parity.md:79` still list fail/not-started rows.
- Change: Add executable sentinel cases for every active fail/not-started row, or remove stale ledger rows only after proof.
- Test: `rtk cargo test -p redlinedb-sql --test sqlite_full_parity --quiet --locked`

## <pending> WO-007: Reconcile README Compatibility Claims With Parity Ledger

- Area: Documentation
- Severity: Medium
- Confidence: High
- Evidence: `README.md:328` says views, triggers, CTE execution, window functions, generated columns, partial/expression indexes, and broad FFI remain fail/not-started; `docs/sqlite-parity.md:32` through `docs/sqlite-parity.md:41` mark several of those rows pass.
- Change: Update README/docs to separate proven pass rows from remaining SQLite drop-in gaps without overstating compatibility.
- Test: `rtk just score`

## <pending> WO-008: Generate Required SQLite Parity Receipts In The Gate

- Area: Proof receipts
- Severity: High
- Confidence: High
- Evidence: `docs/sqlite-parity.md:91` requires receipts under `target/proof/sqlite-full-parity/`; `scripts/just/run.sh:122` through `scripts/just/run.sh:151` run test commands but do not generate those receipt files.
- Change: Add a receipt-generation step to the parity gate and fail when required receipts are missing or stale.
- Test: `rtk just parity-full`

## <pending> WO-009: Start SQLite Native File-Format Compatibility

- Area: Storage compatibility
- Severity: High
- Confidence: High
- Evidence: `docs/sqlite-parity.md:79` marks SQLite database header/pages/btrees/records as not-started and states current files use RedlineDB-native formats, not `SQLite format 3`.
- Change: Define whether native SQLite file format is in scope; if yes, implement/read proof for SQLite headers, pages, btrees, and records.
- Test: Add cross-engine file-format fixtures, then run `rtk just parity-full`.

## <pending> WO-010: Start SQLite Rollback Journal And WAL Byte-Format Compatibility

- Area: Durability compatibility
- Severity: High
- Confidence: High
- Evidence: `docs/sqlite-parity.md:80` says no SQLite rollback-journal reader/writer exists; `docs/sqlite-parity.md:81` says RedlineDB has a native group-commit WAL, not SQLite WAL frames.
- Change: Define journal/WAL byte-format scope and implement explicit compatibility or documented non-goal proof.
- Test: Add rollback-journal/WAL corpus tests, then run `rtk just parity-full`.

## <pending> WO-011: Support Cross-Opening SQLite And RedlineDB Files

- Area: File compatibility
- Severity: High
- Confidence: High
- Evidence: `docs/sqlite-parity.md:82` and `docs/sqlite-parity.md:83` mark both cross-open directions not-started.
- Change: Implement cross-open support or explicitly gate docs/API claims so file-level compatibility is not implied.
- Test: Add `sqlite3` CLI cross-open fixtures and a RedlineDB-open-SQLite fixture, then run `rtk just parity-full`.

## <pending> WO-012: Generate Full PRAGMA Corpus From Reference Build

- Area: PRAGMA parity
- Severity: Medium
- Confidence: High
- Evidence: `docs/sqlite-parity.md:73` says the full reference-build PRAGMA set needs a generated corpus from `PRAGMA compile_options`; `crates/sql/tests/sqlite_full_parity.rs:154` only prints version/compile options metadata.
- Change: Generate a reference PRAGMA corpus from the bundled SQLite build and compare RedlineDB behavior row by row.
- Test: `rtk cargo test -p redlinedb-sql --test sqlite_full_parity --quiet --locked`

## <pending> WO-013: Finish Built-In Collation Parity

- Area: SQL expressions
- Severity: Medium
- Confidence: High
- Evidence: `docs/sqlite-parity.md:55` marks collations as fail; `crates/sql/src/collation.rs:1` scopes the current implementation to BINARY/NOCASE/RTRIM and `crates/sql/src/collation.rs:37` implements local text comparison.
- Change: Diff built-in collation behavior against SQLite, including ordering/equality edge cases, and either fix divergences or ledger exact limits.
- Test: `rtk cargo test -p redlinedb-sql --test phase10_sqld_collation --quiet --locked`

## <pending> WO-014: Materialize Views At Runtime For Cached Statements

- Area: SQL views and statement cache
- Severity: High
- Confidence: High
- Evidence: `crates/sql/src/exec/view.rs:10` says view rows live for the prepared statement duration and cached statements can observe cached rows; `crates/sql/src/connection/session.rs:595` avoids shared cache for view materialization but still inserts into the local cache at `crates/sql/src/connection/session.rs:609`.
- Change: Move view materialization to execution time or invalidate local cached view statements when underlying data changes.
- Test: Add a prepared-view stale-row regression to `crates/sql/tests/parity_view.rs`, then run that test.

## <pending> WO-015: Add INSTEAD OF Triggers On Views

- Area: SQL triggers and views
- Severity: Medium
- Confidence: High
- Evidence: `docs/sqlite-parity.md:36` lists `INSTEAD OF` triggers on views as followup; `crates/sql/src/parser/ddl.rs:409` documents the deferral and `crates/sql/src/parser/ddl.rs:435` rejects the syntax.
- Change: Implement `INSTEAD OF` trigger parsing, catalog storage, and view-DML dispatch semantics.
- Test: Add rusqlite-oracle cases to `crates/sql/tests/parity_trigger.rs`.

## <pending> WO-016: Support Cross-Database Writes Through ATTACH

- Area: ATTACH/DETACH
- Severity: Medium
- Confidence: High
- Evidence: `docs/sqlite-parity.md:41` says cross-database writes are rejected; `crates/sql/src/parser/helpers/table/bind.rs:31` returns `cross-database writes are not yet supported`; `crates/sql/tests/parity_attach.rs:209` asserts that rejection.
- Change: Implement INSERT/UPDATE/DELETE over attached aliases with transaction and durability semantics, or keep the compatibility ledger at partial.
- Test: Extend `crates/sql/tests/parity_attach.rs` with write parity cases.

## <pending> WO-017: Implement CREATE TABLE AS SELECT

- Area: SQL DDL
- Severity: Medium
- Confidence: High
- Evidence: `crates/sql/src/parser/ddl.rs:8` rejects `create_table.query` with `CREATE TABLE AS SELECT is not supported`.
- Change: Implement CTAS schema inference and data population semantics matching SQLite.
- Test: Add CTAS oracle cases to `crates/sql/tests/parity_coverage.rs`.

## <pending> WO-018: Complete SQLite-Relevant ALTER TABLE Variants

- Area: SQL DDL
- Severity: Medium
- Confidence: High
- Evidence: `docs/sqlite-parity.md:25` says add/drop-column variants remain partial; `crates/sql/src/parser/ddl.rs:221` permits only one ALTER operation; `crates/sql/src/parser/ddl.rs:250` and `crates/sql/src/parser/ddl.rs:298` reject several ADD/DROP variants.
- Change: Fill SQLite-supported ALTER forms or enumerate exact unsupported variants in parity tests and docs.
- Test: Extend `crates/sql/tests/phase10_sqld_alter.rs` and `crates/sql/tests/parity_coverage.rs`.

## <pending> WO-019: Complete UPDATE OR/FROM And DELETE ORDER BY/LIMIT

- Area: SQL DML
- Severity: Medium
- Confidence: High
- Evidence: `crates/sql/src/parser/dml.rs:141` rejects `UPDATE OR ...`; `crates/sql/src/parser/dml.rs:146` rejects `UPDATE ... FROM`; `crates/sql/src/parser/dml.rs:236` rejects `DELETE ORDER BY`; `crates/sql/src/parser/dml.rs:241` rejects `DELETE LIMIT`.
- Change: Implement these SQLite DML variants or keep explicit negative tests and compatibility docs aligned.
- Test: Add rusqlite-oracle DML cases to `crates/sql/tests/parity_coverage.rs`.

## <pending> WO-020: Allow Parameters In Compound SELECTs

- Area: SQL planner/executor
- Severity: Medium
- Confidence: High
- Evidence: `crates/sql/src/parser/select.rs:310` rejects parameters in compound branches; `crates/sql/src/parser/select.rs:368` rejects parameters in compound tail ORDER/LIMIT.
- Change: Thread parameter layouts through compound SELECT branches and tail expressions.
- Test: Add parameterized UNION/INTERSECT/EXCEPT cases to `crates/sql/tests/parity_compound_select.rs`.

## <pending> WO-021: Support Nested SELECT Wrappers With ORDER BY Or LIMIT

- Area: SQL parser/planner
- Severity: Medium
- Confidence: High
- Evidence: `crates/sql/src/parser/select.rs:40` rejects nested query wrappers when wrapper-level ORDER BY or LIMIT is present.
- Change: Preserve wrapper ORDER BY/LIMIT semantics through binding rather than rejecting them.
- Test: Add nested wrapper cases to `crates/sql/tests/parity_order_by_ordinal.rs` or a new parity test.

## <pending> WO-022: Implement NATURAL JOIN

- Area: SQL joins
- Severity: Medium
- Confidence: High
- Evidence: `docs/sqlite-parity.md:29` says natural joins are explicitly rejected; `crates/sql/src/parser/helpers/table/select.rs:358` returns `NATURAL joins are not supported`.
- Change: Expand NATURAL JOIN into SQLite-compatible USING predicates and output-column behavior.
- Test: Add NATURAL JOIN oracle cases to `crates/sql/tests/parity_coverage.rs`.

## <pending> WO-023: Implement Named Windows

- Area: SQL window functions
- Severity: Medium
- Confidence: High
- Evidence: `crates/sql/src/exec/expr/window_eval.rs:254` rejects any non-inline window spec with `named windows are not supported`.
- Change: Parse, bind, and evaluate named WINDOW clauses against SQLite behavior.
- Test: Extend `crates/sql/tests/parity_window.rs` with named-window cases.

## <pending> WO-024: Support Aggregate Expressions Inside CASE

- Area: SQL aggregates
- Severity: Medium
- Confidence: High
- Evidence: `crates/sql/tests/parity_negative.rs:197` asserts `CASE WHEN count(*)` is unsupported; `crates/sql/src/exec/agg_eval.rs:271` returns `aggregate expressions in CASE are not supported`.
- Change: Teach aggregate analysis/evaluation to handle aggregates nested inside CASE expressions.
- Test: Move the negative test into positive rusqlite parity coverage.

## <pending> WO-025: Support Bind Parameters In Table-Valued Function Arguments

- Area: SQL table-valued functions
- Severity: Medium
- Confidence: High
- Evidence: `crates/sql/src/exec/table_valued.rs:67` says bind parameters are not allowed in TVF arguments because bindings are unavailable at that stage; `crates/sql/src/exec/table_valued.rs:122` rejects unsupported argument expressions.
- Change: Lower TVF arguments after parameter binding or carry parameter slots through TVF materialization.
- Test: Add `json_each(?)` and table-valued PRAGMA parameter cases to parity tests.

## <pending> WO-026: Implement True INSERT OR FAIL And OR ROLLBACK Semantics

- Area: SQL conflict handling
- Severity: High
- Confidence: High
- Evidence: `crates/sql/src/exec/tail_conflict.rs:297` documents that `OR FAIL` and `OR ROLLBACK` currently behave like `OR ABORT`; `crates/sql/src/exec/tail_conflict.rs:493` maps ABORT/FAIL/ROLLBACK to the same constraint error path.
- Change: Add statement-level partial commit and explicit-transaction error classification matching SQLite.
- Test: Extend `crates/sql/tests/phase10_sqlc_conflict_matrix.rs` with strict rusqlite comparisons.

## <pending> WO-027: Unignore Unicode SQL Literal Parser Panic Test

- Area: Parser robustness
- Severity: High
- Confidence: High
- Evidence: `crates/sql/tests/parser_proptest.rs:194` documents a known lexer panic on `SELECT '\u{80}'`; the property is ignored at `crates/sql/tests/parser_proptest.rs:204`.
- Change: Wrap or fix the parser boundary so arbitrary UTF-8 literals return `Ok`/`Err` without panicking, then unignore the property.
- Test: `rtk cargo test -p redlinedb-sql --test parser_proptest --quiet --locked -- --ignored`

## <pending> WO-028: Strictly Compare JSON Invalid-Input Oracle Behavior

- Area: JSON parity
- Severity: Medium
- Confidence: High
- Evidence: `crates/sql/tests/parity_json_table.rs:160` tests invalid JSON, but `crates/sql/tests/parity_json_table.rs:166` allows prepare-vs-step variation and `crates/sql/tests/parity_json_table.rs:177` only requires at least one engine to error before separately requiring RedlineDB rejection.
- Change: Normalize prepare/step error timing and compare oracle error class/content strictly enough to catch behavioral drift.
- Test: `rtk cargo test -p redlinedb-sql --test parity_json_table --quiet --locked`

## <pending> WO-029: Complete Planner Matching For Expression Indexes

- Area: SQL planner/indexes
- Severity: Medium
- Confidence: Medium
- Evidence: `docs/sqlite-parity.md:39` says expression indexes are maintained by DML; `crates/sql/src/exec/index_partial.rs:113` limits expression-index matching to leading-key equality and `crates/sql/src/exec/index_partial.rs:116` skips multi-key expression indexes.
- Change: Expand expression-index read matching across multi-key and richer predicate forms.
- Test: Extend `crates/sql/tests/parity_expr_index.rs` with EXPLAIN and read-path coverage.

## <pending> WO-030: Improve Partial-Index Predicate Implication

- Area: SQL planner/indexes
- Severity: Low
- Confidence: High
- Evidence: `crates/sql/src/exec/index_partial.rs:3` says partial indexes are used only when the query provably implies the predicate; `crates/sql/src/exec/index_partial.rs:5` limits that to identical normalized predicates and names richer implication as follow-up.
- Change: Add safe implication rules such as range strengthening and equality-to-IN cases without advertising unsafe plans.
- Test: Extend `crates/sql/tests/parity_partial_index.rs` with planner coverage.

## <pending> WO-031: Make Multi-Index OR/AND Executable Or Remove Dead Variants

- Area: SQL planner/indexes
- Severity: Medium
- Confidence: High
- Evidence: `crates/sql/src/planner/access.rs:65` says `CoveringIndexScan`, `MultiIndexOr`, and `MultiIndexAnd` are not advertised; `crates/sql/src/planner/access.rs:81` marks those variants not consumable; `crates/sql/tests/smoke_select.rs:506` asserts multi-index AND/OR stay off.
- Change: Implement executable multi-index union/intersection plans or keep them out of exposed planner output and docs.
- Test: Add EXPLAIN plus result parity tests for OR/AND predicates over multiple indexes.

## <pending> WO-032: Finish Covering-Index Planner Integration

- Area: SQL planner/indexes
- Severity: Medium
- Confidence: Medium
- Evidence: `crates/sql/src/planner/access.rs:65` still says covering scans are not advertised; `crates/sql/src/exec/select_top.rs:88` has a special-case covering fast path; `crates/sql/src/exec/select_top.rs:911` limits covering scans to plain column indexes.
- Change: Integrate covering indexes into normal access-path selection with correct EXPLAIN and broaden support where safe.
- Test: Extend covering-index result and EXPLAIN tests in `crates/sql/tests/smoke_select.rs`.

## <pending> WO-033: Remove Fixed Low-Concurrency WAL Group-Commit Latency Floor

- Area: WAL performance
- Severity: Medium
- Confidence: High
- Evidence: `crates/kernel/src/wal/manager/config.rs:7` sets a default group-commit delay of 200 microseconds; `crates/kernel/src/wal/manager/coordinator/helpers.rs:60` waits for that delay unless batch thresholds short-circuit.
- Change: Make group-commit delay adaptive or bypass it for latency-sensitive low-concurrency commits.
- Test: Add group-commit latency regression coverage and run `rtk cargo test -p redlinedb-kernel --test group_commit_tests --quiet --locked`.

## <pending> WO-034: Reduce Single WAL Writer Queue Serialization

- Area: WAL concurrency
- Severity: Medium
- Confidence: High
- Evidence: `crates/kernel/src/wal/manager/coordinator/methods.rs:33` creates one shared pending `VecDeque`; `crates/kernel/src/wal/manager/coordinator/methods.rs:55` spawns one `redlinedb-wal-writer`; `crates/kernel/src/wal/manager/config.rs:47` defaults `lanes` to 1.
- Change: Wire multi-lane WAL mode into the engine or otherwise reduce single-queue serialization without compromising recovery.
- Test: Run group-commit tests plus high-concurrency benchmark proof for WAL fan-in.

## <pending> WO-035: Wire Semantic WAL Combiner Into Real WAL Records

- Area: WAL write amplification
- Severity: Medium
- Confidence: High
- Evidence: `crates/kernel/src/wal/combiner.rs:27` exposes a pure-data helper; `crates/kernel/src/wal/manager/config.rs:31` says the combiner is wired as a no-op today; `rtk rg maybe_combine_pending crates/kernel/src` finds no coordinator call site outside `combiner.rs`.
- Change: Encode combinable real WAL records and invoke the combiner from the pending queue only under a completed safety proof.
- Test: Extend `crates/kernel/tests/group_commit_tests.rs` with recovery and visibility cases for combined records.

## <pending> WO-036: Replace Global B-Tree Structure Lock With Finer-Grained Structural Concurrency

- Area: Kernel index concurrency
- Severity: Medium
- Confidence: High
- Evidence: `crates/kernel/src/index/mod.rs:170` stores one `structure_lock`; `crates/kernel/src/index/mutate/insert.rs:101` takes it for leaf split work; `crates/kernel/src/index/mod.rs:742` tracks contention on that single mutex.
- Change: Move structural modification concurrency to latch-coupled or page/subtree-scoped locking.
- Test: Add concurrent split stress tests and run `rtk just kernel-cursor` plus kernel index tests.

## <pending> WO-037: Complete Range-Scan Warm-Leaf And Reverse-Iteration Work

- Area: Kernel index scans
- Severity: Medium
- Confidence: Medium
- Evidence: `README.md:431` still calls range scans the biggest gap and says they lack prefetch/warm-leaf reuse; current `crates/kernel/src/index/cursor/raw/range.rs:21` tracks forward `current_leaf`/`next_leaf` and `crates/kernel/src/index/cursor/raw/range.rs:67` only prefetches the next-next leaf.
- Change: Reconcile stale docs with the current advisory prefetch, then implement missing warm-leaf reuse and reverse range iteration if still absent.
- Test: Extend `crates/kernel/tests/index_cursor_prefetch.rs` with reverse and warm-cache reuse coverage.

## <pending> WO-038: Reduce Snapshot Acquisition And Visibility Cost

- Area: MVCC
- Severity: Medium
- Confidence: High
- Evidence: `crates/kernel/src/txn/status.rs:65` builds each snapshot by iterating `states`; `crates/kernel/src/txn/status.rs:66` collects all in-progress transactions into a `BTreeSet`; visibility then checks `snapshot.active.contains` at `crates/kernel/src/txn/status.rs:88`.
- Change: Track active transaction ranges or epochs so snapshot acquisition and visibility do not scale with all active transactions.
- Test: Add MVCC snapshot microbenchmarks and concurrency stress tests.

## <pending> WO-039: Narrow Statement Cache Invalidation Beyond Global Schema Epoch

- Area: SQL statement cache
- Severity: Medium
- Confidence: High
- Evidence: `crates/sql/src/connection/cache.rs:10` keys cached statements by global `schema_epoch`; `crates/kernel/src/catalog/ops.rs:359`, `crates/kernel/src/catalog/ops.rs:380`, and `crates/kernel/src/catalog/ops.rs:472` bump that epoch for broad DDL changes; `crates/sql/src/statement.rs:552` reparses when epochs differ.
- Change: Add dependency-aware invalidation so unrelated table/index/view DDL does not flush every cached statement.
- Test: Add cache-hit/invalidation tests around unrelated DDL in `crates/sql/src/connection/tests.rs`.

## <pending> WO-040: Finish Benchmark Telemetry And Claim Gating

- Area: Benchmark proof
- Severity: High
- Confidence: High
- Evidence: `README.md:446` says bench claims must include manifests with git/image/host/artifact hashes; `docs/WORKPLAN_slam.md:815` says raw SQLite VFS/fsync/RSS/IO metrics are still incomplete; `docs/WORKPLAN_slam.md:816` says benchmark interpretation needs stronger review before headline claims.
- Change: Make headline claims fail closed unless the required telemetry and interpretation receipts are present.
- Test: Run the relevant certify lane and verify generated manifests/hashes before docs updates.

## <pending> WO-041: Fail Or Mark CLI .dump Round-Trip When sqlite3 Is Missing

- Area: CLI parity tests
- Severity: Low
- Confidence: High
- Evidence: `crates/cli/tests/dot_commands.rs:5` says the `.dump` round-trip test is silently skipped without `sqlite3`; `crates/cli/tests/dot_commands.rs:147` returns early when `sqlite3 --version` fails.
- Change: Treat missing `sqlite3` as an explicit skipped proof receipt or CI prerequisite instead of a silent pass.
- Test: `rtk cargo test -p redlinedb-cli --test dot_commands --quiet --locked`

## Completed WO-042: Convert parity_scalar_funcs Hardcoded Assertions To Oracle Comparisons

- Area: SQL parity coverage
- Severity: High
- Confidence: High
- Evidence: `crates/sql/tests/parity_scalar_funcs.rs` runs ~30 tests (e.g. `substr_basic_1based`, `trim_whitespace`, `printf_hex_placeholder`, `iif_true_branch`, `unicode_basic`, `zeroblob_correct_length`) that assert hand-written expected values rather than diffing against the bundled rusqlite oracle; only `lower_upper_of_real_keeps_trailing_zero` and `cast_real_as_text_keeps_trailing_zero` route through the oracle. `docs/sqlite-parity.md:48` marks the row as pass on the basis of this file.
- Change: Routed scalar-function cases through the shared oracle harness. Kept `randomblob` deterministic by asserting `typeof(...)` and `length(...)` instead of byte equality; aligned exposed scalar behavior for `length(BLOB)`, `zeroblob(NULL)`, `randomblob(NULL/nonpositive)`, and SQLite `trim(X, Y)` parsing so the oracle-backed tests are green.
- Branch: `codex/sql-oracle-coverage-hardening`, stacked on `gapcheck`.
- PR intent: Oracle-backed SQL coverage hardening.
- Proof:
  - `rtk cargo test -p redlinedb-sql --test parity_scalar_funcs --quiet --locked` exited 0; 46 tests passed.
  - `rtk just sql-parity` exited 0; 99 tests passed across 5 suites.
  - `rtk just fast` exited 0.
- Debug receipt: initial scalar oracle run exited 101; failing tests were `randomblob_correct_length`, `randomblob_produces_blob_of_right_size`, `zeroblob_null_propagates`, and `trim_custom_chars`; raw log `~/.local/share/rtk/tee/1779131686_cargo_test.log`.

## Completed WO-043: Convert phase10_j1_compat JSON Self-Tests To Oracle Comparisons

- Area: JSON parity coverage
- Severity: High
- Confidence: High
- Evidence: `crates/sql/tests/phase10_j1_compat.rs` contains 30+ tests (e.g. `phase10_j1_json_minifies_whitespace`, `phase10_j1_json_array_*`, `phase10_j1_json_array_length_*`) that compare RedlineDB output to hardcoded literals such as `r#"{"a":1}"#`; the oracle-backed file `crates/sql/tests/parity_json1.rs` is significantly smaller. `docs/sqlite-parity.md:53` cites both files when marking JSON scalar functions as pass.
- Change: Converted deterministic JSON scalar behavior in `phase10_j1_compat.rs` to `harness::assert_parity(sql)` and malformed JSON/path cases to `harness::check_parity(sql)` error-class comparisons. Retained fuzz/no-panic coverage as robustness tests, explicitly separate from the parity proof.
- Branch: `codex/sql-oracle-coverage-hardening`, stacked on `gapcheck`.
- PR intent: Oracle-backed SQL coverage hardening.
- Proof:
  - `rtk cargo test -p redlinedb-sql --test phase10_j1_compat --quiet --locked` exited 0; 42 tests passed.
  - `rtk cargo test -p redlinedb-sql --test parity_json1 --quiet --locked` exited 0; 32 tests passed.
  - `rtk just sql-parity` exited 0; 99 tests passed across 5 suites.
  - `rtk just fast` exited 0.

## Completed WO-044: Pin Error Class On parity_negative `assert_errors` Tests

- Area: Negative parity coverage
- Severity: Medium
- Confidence: High
- Evidence: `crates/sql/tests/parity_negative.rs:28` defines `assert_errors` that only checks `result.expect_err(...)`; nine tests use it without pinning a message fragment, including `update_or_conflict_is_unsupported` (line 52), `delete_using_is_unsupported` (line 61), `delete_order_by_is_unsupported` (line 81), `insert_set_syntax_is_unsupported` (line 90), `insert_on_duplicate_key_update_is_unsupported` (line 100), `alter_table_only_is_unsupported` (line 120), `alter_table_add_column_after_is_unsupported` (line 128), `alter_table_drop_multiple_columns_is_unsupported` (line 136), `group_by_all_is_unsupported` (line 175).
- Change: Replaced the WO-listed broad `assert_errors(result)` calls with `assert_unsupported(result, "fragment")`. Added narrow parser-front unsupported classification for `INSERT ... SET`, `ALTER TABLE ADD COLUMN ... AFTER`, and multi-`DROP COLUMN` forms so the intended stable boundary messages are reached instead of raw parser errors.
- Branch: `codex/sql-oracle-coverage-hardening`, stacked on `gapcheck`.
- PR intent: Oracle-backed SQL coverage hardening.
- Proof:
  - `rtk cargo test -p redlinedb-sql --test parity_negative --quiet --locked` exited 0; 23 tests passed.
  - `rtk just sql-parity` exited 0; 99 tests passed across 5 suites.
  - `rtk just fast` exited 0.
- Debug receipt: initial negative proof exited 101; failing tests were `alter_table_add_column_after_is_unsupported`, `alter_table_drop_multiple_columns_is_unsupported`, and `insert_set_syntax_is_unsupported`; raw log `~/.local/share/rtk/tee/1779131849_cargo_test.log`.

## <pending> WO-045: Stop Sort-Splitting `group_concat` Results In Aggregate Parity Tests

- Area: Aggregate parity
- Severity: Medium
- Confidence: High
- Evidence: `crates/sql/tests/parity_agg_funcs.rs` (`group_concat_basic_default_separator`, `group_concat_custom_separator`, `group_concat_with_group_by`) split the output on `,` and sort the parts before asserting; the differential lab (`crates/sql/tests/differential_lab.rs::diff_aggregate_matrix`) explicitly skips `group_concat` for the same reason. Hidden order divergence cannot be caught by either harness.
- Change: Switch tests to `group_concat(expr ORDER BY ...)` (the SQLite-supported deterministic form) and assert the exact string.
- Test: `rtk cargo test -p redlinedb-sql --test parity_agg_funcs --quiet --locked`

## <pending> WO-046: Expand Cross-Engine Compat Corpus Beyond 8 Query Cases

- Area: SQL compatibility corpus
- Severity: High
- Confidence: High
- Evidence: `crates/bench/compat/orm/migration.sqlt`, `crates/bench/compat/orm/queries.sqlt`, and `crates/bench/compat/slt/smoke.sqlt` total 40 directives: 32 setup statements (`CREATE`, `INSERT`, `DROP`) and 8 actual query validations against 1-3-row tables; `README.md` advertises "40 / 40 SQL compatibility cases pass" on the basis of this count.
- Change: Import a subset of SQLite's public sqllogictest corpus, or generate new cases covering LEFT/RIGHT/CROSS JOIN, GROUP BY/HAVING, NULL-heavy predicates, LIMIT/OFFSET, deeply-nested correlated subqueries, and compound SELECT with mismatched types; target ≥500 distinct query cases.
- Test: `rtk cargo run -p redlinedb-bench --release -- compat --engine both --test-dir crates/bench/compat --seed 7 --out target/bench/compat.json`

## <pending> WO-047: Expand `differential_lab` Matrices Beyond String/Logic/Agg/Join

- Area: SQL parity coverage
- Severity: Medium
- Confidence: High
- Evidence: `crates/sql/tests/differential_lab.rs` ships four matrices totalling ~28 paired queries: `diff_scalar_string_matrix` (~11), `diff_scalar_math_and_logic_matrix` (~4), `diff_aggregate_matrix` (~5, `group_concat` skipped), `diff_join_and_subquery_matrix` (~8 INNER plus IN/EXISTS). There is no LEFT/RIGHT/CROSS JOIN matrix, no GROUP BY/HAVING matrix, no window-function differential, no NULL-semantics matrix.
- Change: Add `diff_outer_join_matrix`, `diff_groupby_having_matrix`, `diff_window_function_matrix`, and `diff_null_semantics_matrix`, each with ≥10 paired queries against rusqlite.
- Test: `rtk cargo test -p redlinedb-sql --test differential_lab --quiet --locked`

## <pending> WO-048: Raise parity_corpus Floor Above 5 Files Per Tag

- Area: SQL parity corpus
- Severity: Medium
- Confidence: High
- Evidence: Every tag under `crates/sql/tests/parity_corpus/{basic,compound,cte,window,view,trigger,index,fk,pragma,json,datetime}` contains exactly five short `.sql` files (~199 LOC total across 55 files). The five-per-tag count is the floor `crates/sql/tests/parity_oracle.rs:173` asserts; corpus growth has stalled at that floor. The `json` tag is five one-liners and nothing else.
- Change: Raise the floor to ≥25 files per tag, and seed each tag with at least one NULL-heavy case, one type-coercion case, and one nested-expression case.
- Test: After WO-001 lands, `rtk cargo test -p redlinedb-sql --test parity_oracle --quiet --locked` runs the expanded corpus.

## <pending> WO-049: Demote Catalog Rows That Are `pass` With Admitted Limitations

- Area: SQL parity traceability
- Severity: High
- Confidence: High
- Evidence: `docs/sqlite-parity.md` marks the following rows as `pass` while their own Notes column admits a gap: line 25 (ALTER TABLE add/drop column "remain partial"), line 27 (UPSERT conflict matrix "still incomplete"), line 35 (Views "Followup: runtime materialization"), line 36 (Triggers "Recursion cap … well under SQLite's 1000" and "Followup: INSTEAD OF"), line 23 (CREATE TABLE/DROP TABLE "SQLite metadata compatibility is not complete").
- Change: Split each row into a covered-subset `pass` entry and an uncovered-subset `fail` entry with linked test paths, or introduce a `partial` status and demote these rows to it.
- Test: Add a catalog-lint script (new) that walks the markdown table and rejects any `pass` row whose Notes contain `incomplete`, `partial`, `followup`, or `still`; run it as part of `just fast`.

## <pending> WO-050: Stop Labelling Rejected PRAGMAs As Parity Pass

- Area: PRAGMA parity traceability
- Severity: Medium
- Confidence: High
- Evidence: `docs/sqlite-parity.md:63` marks `PRAGMA auto_vacuum` as pass; `docs/sqlite-parity.md:64` marks `PRAGMA wal_checkpoint(MODE)` as pass; `docs/sqlite-parity.md:70` marks "Unknown PRAGMA names" as pass. All three are implemented as `UnsupportedSql` rejections (`crates/sql/tests/parity_pragma_tv.rs:364`, `:384`, `:404`), while SQLite accepts them.
- Change: Introduce a `rejects-by-design` status separate from `pass`, or move these rows under an "Intentionally Unsupported" section so they are not counted toward the parity tally.
- Test: Catalog lint (see WO-049) flags `pass` rows that delegate to rejection-asserting tests.

## <pending> WO-051: Replace Fixed 10,000-Iteration Recursive CTE Cap

- Area: SQL recursive CTEs
- Severity: Medium
- Confidence: High
- Evidence: `crates/sql/src/exec/cte_recursive.rs:20` declares `pub(super) const RECURSIVE_CTE_ITERATION_LIMIT: usize = 10_000`; the loop at `crates/sql/src/exec/cte_recursive.rs:89` enforces it; `:160` raises a synthetic error on the boundary. SQLite has no equivalent hard cap.
- Change: Replace the fixed cap with a memory-budget guard backed by a spillable temp row store, or expose `PRAGMA redlinedb_recursive_cte_cap` so the cap is at least raisable per-connection.
- Test: A 50k-iteration recursive graph-walk CTE runs to completion against the rusqlite oracle; add the case to `crates/sql/tests/parity_cte.rs`.

## <pending> WO-052: Raise Trigger Recursion Cap From 8 Toward SQLite's 1000

- Area: SQL triggers
- Severity: High
- Confidence: High
- Evidence: `crates/sql/src/exec/trigger.rs:43` sets `pub(crate) const TRIGGER_DEPTH_CAP: u32 = 8`. `crates/sql/src/exec/trigger.rs:35` comments that SQLite's default `SQLITE_MAX_TRIGGER_DEPTH` is 1000. The catalog (`docs/sqlite-parity.md:36`) cites "default 32 in debug" — itself wrong relative to the source.
- Change: Raise the constant to 1000, expose it as a configurable PRAGMA, or gate a lower debug-only cap behind `cfg(debug_assertions)` and update the catalog to match the actual value.
- Test: A 128-deep trigger chain runs to completion against the rusqlite oracle; a 1001-deep chain returns a SQLite-shaped error.

## <pending> WO-053: Accept `PRAGMA journal_mode = WAL` As A Truthful No-Op

- Area: PRAGMA compatibility
- Severity: High
- Confidence: High
- Evidence: `docs/sqlite-parity.md:65` lists supported `journal_mode` values as the `memory/off/delete` subset; `wal/truncate/persist` are rejected with a message naming the value. SQLite's default for most modern applications (rusqlite, Python `sqlite3`, common ORM stacks) is `WAL`, so the first PRAGMA on open against RedlineDB fails.
- Change: Accept `WAL` as an alias that round-trips — RedlineDB always runs a WAL-style group-commit log, so the acknowledgement is truthful even though the on-disk format differs.
- Test: `PRAGMA journal_mode = WAL` returns `wal`; reading back `PRAGMA journal_mode` returns `wal`; both diff-clean against rusqlite. Add the case to `crates/sql/tests/parity_pragma_tv.rs`.

## <pending> WO-054: Accept CTE `MATERIALIZED` / `NOT MATERIALIZED` Hints

- Area: SQL CTE syntax
- Severity: Low
- Confidence: Medium
- Evidence: SQLite accepts `WITH foo AS MATERIALIZED (...)` and `WITH foo AS NOT MATERIALIZED (...)`. No handling for either keyword is present under `crates/sql/src/parser/`; queries that include them parse-error.
- Change: Parse and accept both keywords as no-ops; record the hint on the CTE node so a future planner pass can consume it without re-parsing.
- Test: Round-trip a CTE with each hint against rusqlite in `crates/sql/tests/parity_cte.rs`.

## <pending> WO-055: Implement Recursive CTE `CYCLE` Clause

- Area: SQL recursive CTEs
- Severity: Medium
- Confidence: High
- Evidence: SQL:1999 syntax `WITH RECURSIVE foo(x) AS (...) CYCLE x SET mark USING path` deduplicates cycles during traversal; SQLite supports it. `crates/sql/src/exec/cte_recursive.rs` contains no CYCLE handling — graph queries either iteration-cap (see WO-051) or non-terminate.
- Change: Parse the CYCLE clause, track the visited set on the configured columns, and surface the mark/path columns in output.
- Test: Add a cyclic-graph fixture to `crates/sql/tests/parity_cte.rs` and diff against rusqlite.

## <pending> WO-056: Cache Partial-Index Predicates Across DML Rows

- Area: SQL planner/indexes (perf)
- Severity: Low
- Confidence: High
- Evidence: `docs/sqlite-parity.md:38` says "DML re-parses and evaluates the predicate per row" for partial indexes; `crates/sql/src/exec/index_predicate.rs` carries that responsibility. Bulk INSERT against a partial-indexed table pays a parser tax per row.
- Change: Parse and bind the predicate once at DDL time, cache the bound expression on `IndexDef`, and re-bind only on schema-epoch change (see WO-039 for the broader invalidation work).
- Test: Micro-bench bulk INSERT (100k rows) into a partial-indexed table before/after; expect ≥2× throughput.

## <pending> WO-057: Parse Window-Frame `EXCLUDE` Modes

- Area: SQL window functions
- Severity: Medium
- Confidence: Medium
- Evidence: `crates/sql/src/exec/expr/window_eval/frame.rs:79-126` (`frame_bounds`) computes ROWS/RANGE/GROUPS bounds but contains no handling for SQLite's `EXCLUDE TIES | GROUP | CURRENT ROW | NO OTHERS` modifiers. Queries that use them fail at parse or planner stage.
- Change: Extend frame parsing and the `frame_bounds` computation to honour each EXCLUDE mode by post-filtering the row indices that contribute to the aggregate.
- Test: Differential against rusqlite for each EXCLUDE mode crossed with ROWS/RANGE/GROUPS framing in `crates/sql/tests/parity_window.rs`.

## <pending> WO-058: Parse Aggregate `FILTER (WHERE ...)` In Window Position

- Area: SQL window functions
- Severity: Medium
- Confidence: Medium
- Evidence: SQLite accepts `SUM(x) FILTER (WHERE y > 0) OVER (...)`. No FILTER handling is wired into the window-eval path (`crates/sql/src/exec/expr/window_eval.rs`).
- Change: Parse the FILTER clause attached to a window-function call and evaluate the predicate per row to gate aggregate contribution.
- Test: Differential against rusqlite for SUM/COUNT/AVG/MIN/MAX with FILTER inside an OVER clause in `crates/sql/tests/parity_window.rs`.

## <pending> WO-059: Honour `COLLATE` Clause In ORDER BY

- Area: SQL collations
- Severity: Medium
- Confidence: Medium
- Evidence: SQLite accepts `ORDER BY name COLLATE NOCASE`. No per-key collation handling is visible in `crates/sql/src/parser/helpers/order_by*` or in the sort-key path. WO-013 covers the broader collation work, but this specific clause is worth its own gate.
- Change: Parse the per-key `COLLATE` clause and route the chosen collation through the sort-key encoder.
- Test: `ORDER BY name COLLATE NOCASE` diff-cleans against rusqlite on a mixed-case TEXT column in `crates/sql/tests/parity_coverage.rs`.

## <pending> WO-060: Provide A Virtual-Table Interface (`CREATE VIRTUAL TABLE` / `sqlite3_module`)

- Area: SQL extensibility
- Severity: High
- Confidence: High
- Evidence: SQLite's virtual-table interface (the `sqlite3_module` callback surface) is the foundation for FTS3/4/5, R*Tree, JSON1 `json_each` / `json_tree`, CSV reader, and most third-party data-source adapters. `docs/sqlite-parity.md` does not list `CREATE VIRTUAL TABLE` at all — silent omission. No code under `crates/sql/src/parser/` or `crates/sql/src/exec/` recognises the syntax.
- Change: Design a Rust-native virtual-table trait, plus a C-ABI shim exposing the `sqlite3_module` shape. Wire `CREATE VIRTUAL TABLE` parsing through to a registry of module handlers.
- Test: A minimal `generate_series` virtual table registers via both the Rust API and the FFI and produces matching rows against rusqlite.

## <pending> WO-061: Document Or Implement Absent SQLite Extensions (FTS, R*Tree, RBU)

- Area: Extension coverage
- Severity: Medium
- Confidence: High
- Evidence: SQLite's official extensions FTS3/4/5, R*Tree, and RBU are not mentioned in `docs/sqlite-parity.md` and have no presence in the source tree. Applications depending on these will fail outright on RedlineDB.
- Change: Either scope the extensions as a separate phase (depends on WO-060) or add a "Known absent extensions" subsection to the README "Limitations and roadmap" so users discover the omission before they port.
- Test: `crates/ffi/tests/symbol_diff.rs` extended to note the absent extension symbol families; smoke tests when the extensions land.

## <pending> WO-062: Close The `sqlite3_*` C ABI Coverage Gap

- Area: FFI coverage
- Severity: High
- Confidence: High
- Evidence: A grep across `crates/ffi/src/sqlite3_api/*.rs` finds approximately 54 `pub extern "C"` symbols (`bind.rs` ~7, `column.rs` ~8, `core.rs` ~10, `meta.rs` ~18, `stmt.rs` ~8, others ~3). SQLite publishes ~300 public functions. `docs/sqlite-parity.md:85` marks "Broad sqlite3_* API surface" as pass on the strength of "36 additional symbols". The gap means rusqlite, Python `sqlite3`, and Go drivers cannot dynamically link without missing-symbol errors.
- Change: Generate a symbol-coverage report from `sqlite3.h`. For each missing symbol, either implement it or stub it as a documented `SQLITE_MISUSE`-returning shim so dynamic linkage succeeds. Surface the coverage percentage as a CI gate (must not regress).
- Test: Extend `crates/ffi/tests/symbol_diff.rs` to enumerate the full documented set and fail the build on uncovered symbols (paired with WO-002 to make the lane run).

## <pending> WO-063: Reconcile Duplicate Snapshot Implementations And Verify Active-Set Visibility

- Area: Kernel MVCC correctness
- Severity: High
- Confidence: Medium
- Evidence: Two snapshot implementations exist. `crates/kernel/src/txn/status.rs:65-89` builds the snapshot with a populated `active: BTreeSet<TxId>` and `is_tx_visible` correctly tests `csn <= snapshot.visible_csn && !snapshot.active.contains(&tx)`. `crates/kernel/src/engine/tx/status.rs:140-148` builds the snapshot as `Snapshot { visible_csn, xmin, xmax, active: BTreeSet::new() }` (active set unconditionally empty) and visibility at `:154-162` is just `csn <= snapshot.visible_csn`. If the engine path is live and publication is non-monotonic across threads (or CSNs are assigned before commit), a snapshot can observe writes from transactions that committed after it was taken.
- Change: Determine which path is live for MVCC visibility (likely `engine/tx/status.rs`), populate `Snapshot::active` from the existing `active_snapshots` Mutex, and unify the two implementations or delete the dead one. If the empty-active path is provably safe under the current publish ordering, document why.
- Test: Property test: thread A snapshots, thread B commits at higher CSN, thread A must not see B's write. Add to `crates/kernel/tests/` or a new MVCC isolation test.

## <pending> WO-064: Detect Row-Level Write Conflicts For Non-Indexed UPDATEs

- Area: Kernel MVCC correctness
- Severity: High
- Confidence: Medium
- Evidence: `crates/kernel/src/index/mutate/insert.rs:85,152` raises `WriteConflict` only for unique-index slot collisions. The heap UPDATE path appears to append new versions without checking whether the row's latest committed CSN exceeds the writer's snapshot (`crates/kernel/src/engine/runtime/mutation.rs` and `crates/kernel/src/engine/concurrent_heap.rs` did not surface a CSN check during the audit). Under contention on a table with no unique constraint, this permits lost updates and write skew — classic snapshot-isolation holes.
- Change: At UPDATE time, compare the row's latest committed CSN against `snapshot.visible_csn`; return `WriteConflict` when it is newer. Cover the delete-vs-update race symmetrically.
- Test: Two-thread concurrent UPDATE on the same row of a table without a unique index — second commit must fail with `WriteConflict`. Two-thread write-skew test: tx1 reads (X, Y) and writes X = f(Y); tx2 reads (X, Y) and writes Y = g(X); both must not commit successfully.

## <pending> WO-065: Drop The Page-Cache Eviction Mutex Before Issuing I/O

- Area: Kernel buffer pool concurrency
- Severity: High
- Confidence: High
- Evidence: `crates/kernel/src/storage/buffer.rs:375-392` (`ensure_capacity`) acquires the global `eviction` Mutex at `:380-382` and holds it across the while-loop that calls `evict_one` at `:385`; `evict_one` (`:394-442`) flushes dirty pages via `flush_frame_if_durable` at `:425`, which issues synchronous `pwrite`. Every other pinner blocks on the eviction Mutex until disk I/O completes — a dominant tail-latency contributor under write pressure.
- Change: Drop the eviction Mutex before issuing fsync/pwrite. Coordinate via per-frame atomic state (claimed → flushing → flushed → unpinnable) so multiple evictions can be in flight on disjoint frames.
- Test: Latency histogram during sustained eviction shows the long tail shortening; mixed read/write throughput improves on the existing bench harness. Add a per-page lock-wait counter to surface regressions.

## <pending> WO-066: Skip Dead MVCC Versions During Index Cursor Descent, Not After Materialization

- Area: Kernel index scans (perf and write-amp)
- Severity: High
- Confidence: High
- Evidence: `crates/kernel/src/index/cursor.rs:128-137` and `crates/kernel/src/index/cells.rs:175-183` apply `leaf_entry_visible` after the cursor has already materialized the leaf entry. On a hot-updated secondary index where most leaf entries are dead versions, every dead entry is visited and parsed before being discarded. This is the most plausible root cause of the README-documented `secondary-index-range` ratio (0.048× at 64 threads). WO-037 covers warm-leaf prefetch; this is the orthogonal visibility-filter issue.
- Change: Carry an MVCC tombstone bitmap or visibility hint in the cell header or per-leaf summary so the cursor can skip dead version runs without parsing each cell. Tighten vacuum cadence on hot indexes.
- Test: `secondary-index-range` cell ratio improves from 0.048× toward ≥0.5× on the existing bench. B-tree correctness tests and the recovery matrix stay green.

## <pending> WO-067: Extend Failpoint Coverage Beyond WAL/Commit/Recovery

- Area: Kernel correctness testing
- Severity: Medium
- Confidence: High
- Evidence: `grep -rn fp_inject crates/kernel/src/` returns ~68 sites, concentrated in WAL append, commit publish, and recovery replay. None hook page-cache eviction-after-flush, B-tree post-split-pre-parent-update, catalog rename mid-fsync, or vacuum mid-shard. The 24/24 failpoint-matrix headline only guarantees safety on the paths the failpoints can interrupt.
- Change: Add failpoints at: (a) `flush_frame_if_durable` post-write-pre-page-state-update, (b) B-tree split between leaf write and parent pointer update, (c) catalog DDL between temp write and rename, (d) vacuum between shard purges. Extend `crates/bench/bench/failpoint-matrix.toml` with the new cases.
- Test: `rtk cargo run -p redlinedb-bench --release -- failpoint-matrix --config crates/bench/bench/failpoint-matrix.toml --out target/bench/failpoint.json --seed 7` — matrix moves from 24/24 to 28+/28+ green with zero lost acked commits.
