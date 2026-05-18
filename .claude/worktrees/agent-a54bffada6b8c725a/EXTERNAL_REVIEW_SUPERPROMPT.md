# RedlineDB External Expert Audit Brief

You are reviewing RedlineDB, a 100% Rust embedded database intended to become a SQLite drop-in replacement for supported APIs and SQL behavior, while ultimately being faster, lower-memory, more concurrent, richer in data types, and more capable than SQLite.

Repository to pull:

```bash
git clone https://github.com/jeppsontaylor/RedlineDB.git
cd RedlineDB
```

Your job is not a lightweight code review. Treat this as an expert audit and product/architecture upgrade review. Push hard. Look for correctness bugs, missing SQLite behavior, hidden performance limits, missing public API expectations, benchmark flaws, crash-recovery gaps, memory issues, type-system gaps, and major feature opportunities.

## Product Target

RedlineDB should become:

- A SQLite-style drop-in replacement for supported SQL and API behavior.
- 100% Rust in implementation.
- Faster than SQLite on meaningful concurrent workloads.
- Lower memory than SQLite for comparable workloads where possible.
- Much higher concurrent connection capacity before failure or severe tail latency.
- Safer under crash/recovery and corruption-prone scenarios.
- More capable over time: richer type support, JSON/BJSON or JSONB-style binary JSON, vectorized execution, vector search/indexing, modern observability, and robust tooling.
- Usable as a compiled tool through CLI/server/FFI in the ways users expect from a SQLite-like embedded database.

Do not assume current claims are true. Verify them. Do not make benchmark headline claims unless raw artifacts prove them.

SQLite “drop-in” here means C API and SQL behavior compatibility for the supported subset. It does not mean SQLite file-format compatibility unless you propose that as a future feature.

## First Commands

If you do not have `rtk`, run the raw `cargo` commands without the `rtk` prefix.

```bash
cargo --version
cargo fmt --check
./scripts/check_file_sizes.sh
cargo check --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --quiet --locked
cargo test -p redlinedb-kernel --quiet --locked
cargo test -p redlinedb-sql --quiet --locked
cargo test -p redlinedb-bench --quiet --locked
cargo test -p redlinedb-ffi --quiet --locked
cargo test -p redlinedb --quiet --locked
```

Benchmark/proof smoke commands:

```bash
cargo run -p redlinedb-bench -- compat --engine both --test-dir crates/bench/compat --seed 7 --out target/bench/compat-full.json

cargo run -p redlinedb-bench -- failpoint-matrix \
  --config crates/bench/bench/failpoint-matrix.toml \
  --out target/bench/failpoint-matrix.json \
  --seed 7

cargo run -p redlinedb-bench -- certify \
  --config crates/bench/bench/smoke.toml \
  --out-dir target/bench/certify-smoke \
  --seed 7 --repetitions 1 --warmup 0
```

Remote/Docker benchmark scripts to inspect:

```bash
scripts/bench/xbabe1_sync.sh
scripts/bench/xbabe1_run.sh
scripts/bench/xbabe1_fetch.sh
crates/bench/docker/Dockerfile
```

## Current Context

Important: some docs may lag the latest commits. Treat the current source tree, tests, `docs/WORKPLAN_slam.md`, and `docs/WORKPLAN_CLAUDE.md` as the starting evidence, but verify everything.

Recent work includes:

- SQL parser split with `crates/sql/src/parser.rs` as the entry point plus submodules under `crates/sql/src/parser/`.
- Catalog durability through `WalPayload::CatalogSnapshot`.
- Relation-qualified heap/table row access.
- Physical B-tree index lifecycle, SQL DML maintenance, index reads, and planner/EXPLAIN support.
- Unique index handling with SQLite NULL uniqueness behavior.
- Failpoint infrastructure and a failpoint matrix with fsynced ack oracle.
- Benchmark `certify` lane with child processes, warmup accounting, parallel scheduling/bin-packing, telemetry fields, strace support, and connection-limit workload.
- xbabe1 Docker scripts for remote benchmarking.
- Compatibility tests under `crates/bench/compat`.

Previously identified risk areas that should still be re-reviewed:

- SQL index undo/rollback behavior on commit failure.
- Failpoint action validation and whether the failpoint matrix really proves non-vacuous crash safety.
- Planner only advertising paths the executor really uses.
- B-tree duplicate splits and range-scan termination.
- Benchmark scheduler, warmup, artifact hashes, git SHA/dirty capture, data byte accounting, fsync/pwrite metrics, and connection-limit telemetry.
- SQLite compatibility surface is still intentionally incomplete.

## High-Value Files To Study

Repository routing/proof metadata:

- `AGENTS.md`
- `Cargo.toml`
- `README.md`
- `agent/owner-map.json`
- `agent/test-map.json`
- `agent/proof-lanes.toml`
- `agent/generated-zones.toml`
- `agent/unsafe-ledger.toml`
- `agent/proof-receipt-template.md`
- `docs/WORKPLAN_CLAUDE.md`
- `docs/WORKPLAN_slam.md`

Kernel/storage/catalog/WAL:

- `crates/kernel/src/lib.rs`
- `crates/kernel/src/error.rs`
- `crates/kernel/src/catalog/schema.rs`
- `crates/kernel/src/catalog/store.rs`
- `crates/kernel/src/catalog/ops.rs`
- `crates/kernel/src/catalog/record.rs`
- `crates/kernel/src/catalog/value.rs`
- `crates/kernel/src/catalog/key.rs`
- `crates/kernel/src/catalog/stats.rs`
- `crates/kernel/src/engine/mod.rs`
- `crates/kernel/src/engine/page_heap.rs`
- `crates/kernel/src/engine/concurrent_heap.rs`
- `crates/kernel/src/engine/lock.rs`
- `crates/kernel/src/engine/tx.rs`
- `crates/kernel/src/index/mod.rs`
- `crates/kernel/src/wal/manager.rs`
- `crates/kernel/src/wal/payload.rs`
- `crates/kernel/src/wal/record.rs`
- `crates/kernel/src/wal/segment.rs`
- `crates/kernel/src/storage/buffer.rs`
- `crates/kernel/src/storage/control.rs`
- `crates/kernel/src/storage/page_file.rs`
- `crates/kernel/src/storage/tx_status_checkpoint.rs`
- `crates/kernel/src/txn/status.rs`
- `crates/kernel/src/txn/undo.rs`
- `crates/kernel/src/format/page.rs`
- `crates/kernel/src/format/tuple.rs`
- `crates/kernel/src/failpoints/mod.rs`
- `crates/kernel/src/failpoints/macros.rs`

SQL parser/planner/executor:

- `crates/sql/src/lib.rs`
- `crates/sql/src/parser.rs`
- `crates/sql/src/parser/ddl.rs`
- `crates/sql/src/parser/dml.rs`
- `crates/sql/src/parser/helpers.rs`
- `crates/sql/src/parser/pragma.rs`
- `crates/sql/src/parser/select.rs`
- `crates/sql/src/planner.rs`
- `crates/sql/src/planner/helpers.rs`
- `crates/sql/src/exec.rs`
- `crates/sql/src/exec/tail.rs`
- `crates/sql/src/exec/expr.rs`
- `crates/sql/src/exec/index_access.rs`
- `crates/sql/src/exec/index_dml.rs`
- `crates/sql/src/connection.rs`
- `crates/sql/src/session.rs`
- `crates/sql/src/statement.rs`
- `crates/sql/src/value.rs`
- `crates/sql/src/batch.rs`
- `crates/sql/src/error.rs`

Public Rust facade:

- `crates/redlinedb/src/lib.rs`
- `crates/redlinedb/src/options.rs`
- `crates/redlinedb/src/value.rs`
- `crates/redlinedb/src/params.rs`
- `crates/redlinedb/src/error.rs`
- `crates/redlinedb/src/backup.rs`
- `crates/redlinedb/src/registry.rs`
- `crates/redlinedb/src/machine.rs`

C ABI / SQLite compatibility layer:

- `crates/ffi/include/sqlite3.h`
- `crates/ffi/include/redlinedb.h`
- `crates/ffi/src/lib.rs`

CLI/server/tooling:

- `crates/cli/src/main.rs`
- `crates/server/src/main.rs`
- `justfile`
- `scripts/check_file_sizes.sh`
- `scripts/bench/xbabe1_sync.sh`
- `scripts/bench/xbabe1_run.sh`
- `scripts/bench/xbabe1_fetch.sh`

Benchmark harness:

- `crates/bench/src/config.rs`
- `crates/bench/src/certify.rs`
- `crates/bench/src/workload.rs`
- `crates/bench/src/engine/redline.rs`
- `crates/bench/src/engine/sqlite.rs`
- `crates/bench/src/failpoint_matrix.rs`
- `crates/bench/src/recover.rs`
- `crates/bench/src/compat.rs`
- `crates/bench/src/gates.rs`
- `crates/bench/src/report.rs`
- `crates/bench/src/process_metrics.rs`
- `crates/bench/src/strace_capture.rs`
- `crates/bench/bench/smoke.toml`
- `crates/bench/bench/certification.toml`
- `crates/bench/bench/failpoint-matrix.toml`
- `crates/bench/bench/recovery-matrix.toml`
- `crates/bench/compat/orm/*.sqlt`
- `crates/bench/compat/slt/*.sqlt`

Tests to read:

- `crates/kernel/tests/*.rs`
- `crates/sql/tests/sql_smoke.rs`
- `crates/bench/tests/*.rs`
- `crates/redlinedb/tests` if present
- `crates/ffi` tests and C ABI coverage

## Review Lanes

Please split the audit into focused expert lanes. Each lane should produce findings with file/line references, severity, reproduction steps, and concrete implementation recommendations.

### Lane 1: SQLite Compatibility And Drop-In Surface

Audit expected SQLite behavior:

- SQL syntax support and missing grammar.
- Type affinity and dynamic typing rules.
- NULL semantics, boolean behavior, integer/real/text/blob conversions.
- Collations, ordering, comparison, LIKE/GLOB/REGEXP behavior.
- Constraints: PRIMARY KEY, UNIQUE, CHECK, NOT NULL, FOREIGN KEY.
- Transactions: BEGIN modes, COMMIT, ROLLBACK, SAVEPOINT/RELEASE/ROLLBACK TO.
- Conflict algorithms: ABORT, FAIL, IGNORE, REPLACE, ROLLBACK.
- UPSERT behavior.
- PRAGMA coverage and expected side effects.
- Views, triggers, indexes, partial indexes, expression indexes.
- ALTER TABLE behavior.
- ATTACH/DETACH.
- VACUUM/analyze/statistics.
- Date/time functions.
- Aggregate/window functions.
- CTEs and recursive CTEs.
- Subqueries, joins, ORDER BY, GROUP BY, HAVING, LIMIT/OFFSET.
- Generated columns.
- WITHOUT ROWID.
- AUTOINCREMENT.
- SQLite shell/user expectations.

Deliverable: a compatibility matrix with `supported`, `partial`, `missing`, `wrong`, and `test needed`.

### Lane 2: SQL Parser, Planner, Executor Correctness

Study `crates/sql`.

Look for:

- Parser gaps and ambiguous syntax.
- Incorrect binding/name resolution.
- Incorrect query results versus SQLite.
- Planner advertising paths executor does not use.
- Index access bugs.
- Predicate residual filtering bugs.
- Snapshot visibility bugs.
- JOIN, aggregation, ordering, grouping, DISTINCT, LIMIT/OFFSET issues.
- Prepared statement parameter behavior.
- Error mapping and diagnostics.
- Memory growth in query execution.

Add SQLLogicTest-style cases where possible.

### Lane 3: Storage, WAL, Crash Recovery, Corruption Resistance

Study `crates/kernel`.

Look for:

- WAL append/flush/fsync correctness.
- Commit protocol correctness.
- Page image recovery gaps.
- Catalog snapshot durability issues.
- Checkpoint/control-file atomicity.
- LSN sentinel misuse.
- Recovery idempotence.
- Torn writes and partial write handling.
- Directory fsync gaps.
- Data loss after acknowledged commit.
- Page format validation gaps.
- Checksums/CRC coverage.
- Corruption detection and error behavior.
- Cross-platform filesystem assumptions.

Run and extend:

```bash
cargo test -p redlinedb-kernel --quiet --locked
cargo test -p redlinedb-kernel --features failpoints --quiet --locked
cargo run -p redlinedb-bench -- recover-matrix --config crates/bench/bench/recovery-matrix.toml --out target/bench/recovery-matrix.json --seed 7
cargo run -p redlinedb-bench -- failpoint-matrix --config crates/bench/bench/failpoint-matrix.toml --out target/bench/failpoint-matrix.json --seed 7
```

### Lane 4: Indexes, MVCC, Locks, Concurrency

Study:

- `crates/kernel/src/index/mod.rs`
- `crates/sql/src/exec/index_dml.rs`
- `crates/sql/src/exec/index_access.rs`
- `crates/sql/src/session.rs`
- `crates/sql/src/connection.rs`
- `crates/kernel/src/engine/lock.rs`

Look for:

- Stale index entries.
- Rollback/commit failure corruption.
- Unique race conditions.
- Duplicate key split/range scan bugs.
- Visibility recheck gaps.
- Lock timeout behavior.
- Deadlocks and starvation.
- Multi-connection behavior.
- High concurrent connection scaling.
- Snapshot isolation correctness.
- Index maintenance under UPDATE/DELETE/REPLACE.
- Future need for MVCC-tagged index entries instead of physical undo logs.

### Lane 5: C ABI And SQLite API Drop-In

Study:

- `crates/ffi/include/sqlite3.h`
- `crates/ffi/include/redlinedb.h`
- `crates/ffi/src/lib.rs`

Compare against SQLite C API expectations:

- `sqlite3_open`, close, prepare, step, finalize, reset.
- Error codes and messages.
- Bind APIs.
- Column APIs.
- Busy timeout/handler.
- Extended result codes.
- Threading mode expectations.
- Memory ownership rules.
- Blob/text lifetime rules.
- `sqlite3_exec`.
- Backup API.
- Serialization/deserialization if expected.
- Extension loading strategy.
- Build/link story for real C/C++ apps.

Deliverable: list the minimal API set required to compile common SQLite-using libraries against RedlineDB.

### Lane 6: Benchmarking, Performance, xbabe1, Claims

Study `crates/bench` and `scripts/bench`.

Do not accept benchmark claims without artifacts.

Audit:

- Fairness versus SQLite PRAGMAs.
- Warmup and measurement separation.
- Parallel scheduler/bin-packing correctness.
- Thread count versus core count.
- Child process isolation.
- Disk/cache reuse.
- Strace overhead and sampling.
- RSS/proc I/O/fsync/pwrite counters.
- SQLite checkpoint stats and integrity checks.
- Redline data/WAL byte accounting.
- Artifact hashes and manifest completeness.
- `connection-limit` workload.
- Throughput, p50/p95/p99/p999/max latency.
- BUSY/LOCKED/timeout split.
- Maximum stable concurrent connections before failure.

Design a benchmark suite that can credibly show:

- Latency.
- Query/sec.
- Tail latency.
- Memory.
- Disk writes/fsyncs.
- Concurrent connection scaling.
- Failure threshold.
- Recovery after crash.
- Redline versus SQLite under identical Docker/Linux conditions.

Preferred report wording until proven:

> SQLite contention and tail-latency evidence

Use stronger claims only when raw artifacts prove them.

### Lane 7: Data Types, JSON/BJSON, Vectorization, Extensions

Propose major product upgrades:

- JSON text functions.
- Binary JSON / BJSON / JSONB-style storage.
- JSON path indexes.
- Generated columns over JSON paths.
- Vector data type for embeddings.
- Vector indexes: HNSW, IVF, flat SIMD scan.
- Vectorized query execution.
- SIMD filtering/projection.
- Columnar side caches for analytical scans.
- User-defined scalar/aggregate functions.
- Extension/plugin API in Rust and C.
- Date/time, decimal, UUID, duration, enum, array/map support.
- Full-text search.
- Compression/encryption at page/WAL level.

For each proposal, specify:

- API shape.
- Storage format.
- Query syntax.
- Indexing strategy.
- Backward compatibility.
- Tests.
- Performance risks.

### Lane 8: Safety, Security, Fuzzing, Dependencies

Audit:

- `unsafe` usage.
- FFI memory safety.
- Panic boundaries.
- Poisoned locks.
- Error handling around I/O.
- Corrupt file handling.
- Dependency risk.
- Cargo features.
- Miri suitability.
- Loom/concurrency testing opportunities.
- Fuzz targets for parser, page decode, WAL decode, catalog decode, tuple decode, SQL execution.
- Proptest coverage.

Suggested commands:

```bash
cargo audit
cargo deny check
cargo test --workspace --locked
grep -R "unsafe" -n crates
grep -R "unwrap\\|expect\\|panic!" -n crates
```

### Lane 9: CLI, Server, Packaging, Tool Behavior

Study:

- `crates/cli/src/main.rs`
- `crates/server/src/main.rs`
- public crate APIs
- Dockerfile and scripts

Ask:

- Does the compiled CLI behave like users expect from a SQLite-like tool?
- Are errors readable?
- Are backups/stats/checkpoint commands useful?
- Is there a usable shell?
- Is server protocol documented and testable?
- Are release artifacts easy to build?
- Can a C app link against the FFI cleanly?
- Is there a migration story from SQLite?
- Are docs accurate?

### Lane 10: Roadmap And Product Strategy

Produce a prioritized roadmap to make RedlineDB a much better database than SQLite for its target use cases.

Include:

- P0 correctness blockers.
- P1 SQLite drop-in blockers.
- P1 benchmark/proof blockers.
- P2 performance architecture upgrades.
- P2 data type/vector/JSON roadmap.
- P3 polish/tooling/docs.
- Suggested parallel work lanes.
- Estimated risk and implementation complexity.

## Stress Tests We Want Designed

Design tests that often expose bugs in embedded databases. Be precise and fair. Do not claim SQLite corrupts under normal correct usage. Instead, compare behavior under documented durability modes and record results.

Required test families:

- Multi-process writer contention with many connections.
- Long reader plus checkpoint pressure.
- Hot-row update storms.
- Unique index races.
- CREATE INDEX while writers/readers are active, if supported.
- Crash after commit before publish.
- Crash before/after WAL fsync.
- Crash during catalog save/rename/parent fsync.
- Crash during index mutation.
- Disk-full or injected I/O error.
- Torn WAL/page writes.
- Partial/truncated files.
- DDL plus DML churn.
- Random SQL differential testing against SQLite for supported syntax.
- Index consistency checks after random INSERT/UPDATE/DELETE/ROLLBACK.
- Connection-limit binary search.
- Prepared statement lifecycle abuse: reset/finalize/rebind edge cases.
- C API misuse/failure-mode tests.
- Memory/RSS ceiling tests.

Each test should state:

- What invariant it proves.
- Whether SQLite is the oracle, a comparison target, or only a baseline.
- Expected Redline behavior.
- Expected SQLite behavior.
- How to reproduce.
- What artifact proves the result.

## Required Report Format

Please produce a Markdown report with this structure:

```markdown
# RedlineDB External Audit Report

## Executive Summary
- Top 10 blockers.
- Top 10 opportunities.
- Whether RedlineDB is currently safe to market as SQLite-compatible.

## P0 Findings
Each finding:
- Severity: P0/P1/P2/P3
- Area:
- Files/lines:
- Reproduction:
- Expected:
- Actual:
- Why it matters:
- Suggested fix:
- Tests to add:

## SQLite Compatibility Matrix
Table: feature, status, files, tests, notes.

## Performance And Benchmark Review
- Harness issues.
- Fairness concerns.
- Missing metrics.
- Suggested benchmark matrix.
- Claims that are currently justified.
- Claims that are not justified.

## Crash/Recovery/Corruption Review
- Invariants reviewed.
- Gaps.
- New failpoints/tests.

## API/FFI/Tooling Review
- Rust API.
- C ABI.
- CLI/server behavior.
- Packaging.

## Major Upgrade Roadmap
- JSON/BJSON/JSONB.
- Vector search/vectorized execution.
- Higher concurrency.
- Extensions/UDFs.
- Observability.
- Replication/backup if relevant.

## Suggested PR Plan
List concrete PRs in dependency order.
```

## Quality Bar

Be direct. If something is weak, say so. If a claim is unsupported, call it out. If RedlineDB is missing behavior every SQLite user expects, list it. If a benchmark is misleading, explain how to fix it.

The goal is not to defend the current repo. The goal is to turn it into a substantially better embedded database.
