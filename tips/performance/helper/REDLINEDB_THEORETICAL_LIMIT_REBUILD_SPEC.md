# RedlineDB Theoretical-Limit Performance Rebuild Spec

**Target repository:** `neverhuman/RedlineDB`  
**Date:** 2026-05-25  
**Goal:** Make RedlineDB the fastest Rust-native SQLite-parity database, while preserving compatibility and using the existing benchmark suite as the first acceptance gate.

---

## 1. Diagnosis from the current benchmark outliers

The current published parity metrics show 1,127 SQLite-parity cases, 1,123 passed, 4 skipped, and 0 failed. Median latency is currently about **1.90× SQLite** and median RSS is hundreds of times higher than the SQLite measurement. The worst latency outliers are dominated by CLI tempfile cases such as `.cd`, `.once`, `.output`, `.read`, and `-append`; the worst RSS outliers cluster around window functions, expression indexes, aggregates, recursive CTEs, and `WINDOW_FRAMES_ROWS`.

The key observation is that the top latency outliers are not actually heavy database workloads. For example, `DOT_CD_TEMPFILE` is effectively:

```sql
.cd {{TMP}}
.print cd-ok
```

That means a 20–28 ms RedlineDB result is mostly fixed shell/database-startup overhead, not filesystem throughput. RedlineDB is currently paying for a full database open, engine construction, ephemeral temp-root creation, lock/table/cache setup, SQL parser setup, query-memory broker setup, and output plumbing even for scripts that either do not need a database or only run constant scalar statements.

The RSS problem has two layers:

1. The benchmark reports process high-water RSS, so the Rust binary, sqlparser, engine scaffolding, default caches, and allocator arenas all show up even when the query itself is tiny.
2. Several SQL paths are truly over-materialized: window evaluation stores row sets, partition layouts, per-window-call result arrays, per-frame temporary vectors, and projected rows; grouped aggregation repeatedly scans groups for each aggregate expression; recursive CTE and ORDER BY/DISTINCT paths copy `Vec<SqlValue>` rows many times.

---

## 2. Priority-zero changes: eliminate fixed shell cost

### 2.1 Lazy database open for the CLI

**Problem:** The shell opens `:memory:` before it knows whether the input needs a database. `.cd`, `.print`, `.help`, `.mode`, `.headers`, `.separator`, `.output`, `.once`, and several pure shell commands do not need an engine.

**Change:** Split `CliState` into:

```rust
struct ShellState {
    db: LazyDatabase,
    output: OutputTarget,
    mode: OutputMode,
    // shell-only fields
}

enum LazyDatabase {
    Unopened { filename: PathBuf, options: OpenOptions },
    Opened { db: Database, conn: Connection, db_path: PathBuf },
}
```

Only force `LazyDatabase::open()` when executing SQL or a dot-command that needs schema/data. This should turn `DOT_CD_TEMPFILE` from a full DB-open benchmark into a true shell operation.

**Expected latency impact:**

| Case | Current RedlineDB | SQLite | Target RedlineDB | Expected speedup |
|---|---:|---:|---:|---:|
| `DOT_CD_TEMPFILE` | 24.28 ms | 1.20 ms | 0.7–1.5 ms | 16–35× |
| shell-only dot commands | 4–25 ms | 1–3 ms | 0.5–1.5 ms | 3–20× |

### 2.2 DB-free constant SELECT path for one-shot CLI argv

**Problem:** Cases like `-append {{TMP}}/append.db SELECT 1;` open an entire database even though the SQL is fromless and pure.

**Change:** Before opening a database in argv mode, detect safe scalar-fromless statements:

- `SELECT <literal/expression>[, ...];`
- no `FROM`, no subquery, no function requiring database/session state, no parameters, no pragmas, no readfile/writefile, no random/time functions unless SQLite-compatible deterministic handling is implemented.

Evaluate with the existing scalar evaluator against `RowContext::Empty`, write through the normal renderer, and exit without database open.

**Expected latency impact:** `OPT_APPEND_TEMPFILE` should drop from 20.56 ms to roughly 0.5–1.2 ms, a 17–40× speedup, and should beat SQLite for the one-shot constant-select shape.

### 2.3 Buffered `.output FILE`

**Problem:** `.output` stores a raw `File`; `write_all` on cell values and newlines becomes many tiny writes.

**Change in proposed diff:** `OutputTarget::File` becomes `BufWriter<File>` with a 64 KiB buffer. `.once` already uses a `BufWriter`, so this makes `.output` symmetric.

**Expected latency impact:** 1.2–2.5× on `.output`/dump-heavy shell paths, and less syscall noise in benchmark traces.

---

## 3. Priority-one changes: lean ephemeral profile

### 3.1 Shrink private-memory defaults

**Problem:** Private `:memory:` sessions inherit defaults sized for a long-lived database: 16 MiB public cache, 8 MiB query work memory, 128 statement cache entries, multi-lane heap, and many lock shards. This dominates high-water RSS in tiny parity tests.

**Change in proposed diff:** Add `OpenOptions::lean_ephemeral()` and apply lean overrides only when caller fields are still default. The profile uses:

- `memory.cache_bytes = 1 MiB`
- `query_memory.work_mem_bytes = 256 KiB`
- `query_memory.max_spill_bytes = 64 MiB`
- `query_memory.batch_rows = 128`
- `statement_cache_capacity = 32`
- `busy_timeout = 50 ms`
- private-memory SQL options clamp engine buffer pool to 16–64 pages, heap lanes to 1, lock shards to 1–4, unique locks to 1–16, and stats samples/histograms to tiny values.

**Expected RSS impact:** Top RSS outliers should move from ~9.7–10.0 MiB to roughly 2–4 MiB without deeper row-arena work. With allocator tuning and lazy DB open, shell-only cases should not pay database RSS at all.

### 3.2 Lazy spill-root creation

**Problem:** `QueryMemoryBroker::new` touches the filesystem (`create_dir_all`) even when a query never spills.

**Change in proposed diff:** Move `create_dir_all` to `ensure_spill_file()`.

**Expected latency impact:** Small but broad: removes 1–3 syscalls from every SELECT runtime path.

---

## 4. Priority-two changes: window engine rebuild

### 4.1 Current behavior

The current window evaluator is correct-oriented and materialized:

- materialize all source rows;
- partition rows into `Vec<Vec<usize>>`;
- order each partition with `Vec<(usize, Vec<SqlValue>)>`;
- compute `Vec<SqlValue>` result arrays per window call;
- for some value functions allocate a temporary `Vec<usize>` per row for frame positions;
- clone window results into projected rows.

This is why `WINDOW_PARTITION_SUM_*` dominates RSS.

### 4.2 Proposed immediate diff

The proposed diff removes one high-cost allocation in partitioning and removes per-row frame-position allocations for `first_value`, `last_value`, and `nth_value`.

### 4.3 Full rebuild target

Build a dedicated `WindowProgram` at plan time:

```rust
struct WindowProgram {
    layouts: Vec<WindowLayoutId>,
    functions: Vec<WindowFunctionOp>,
    projection_slots: Vec<ProjectSlot>,
}

enum WindowFunctionOp {
    RowNumber,
    Rank,
    DenseRank,
    PrefixSum { arg_slot: ExprSlot, numeric: NumericKind },
    PrefixCount { arg_slot: Option<ExprSlot> },
    SlidingSum { arg_slot: ExprSlot, frame: FrameSpec },
    LagLead { arg_slot: ExprSlot, offset: usize, default_slot: Option<ExprSlot> },
    FirstLastNth { arg_slot: ExprSlot, selector: FrameSelector },
}
```

Execution strategy:

1. Precompile partition/order expressions into slot evaluators.
2. Store rows in an arena: fixed columns in columnar `Vec<SqlValue>` or typed vectors, row references as `u32` indexes.
3. For `PARTITION BY` keys, encode directly into a reusable scratch buffer; insert encoded keys into `AHashMap<Vec<u8>, PartitionId>` only on group miss.
4. For `SUM/COUNT/AVG/TOTAL` with prefix frames, compute one prefix array per partition and answer each row in O(1).
5. For bounded ROWS frames, use sliding accumulators with add/remove where aggregate supports inverse transition.
6. For RANGE/GROUPS frames, precompute peer ranges once and use prefix arrays over peer groups.
7. Produce projected rows directly, without `Vec<Vec<Vec<SqlValue>>>` intermediates.

**Expected impact:**

| Workload | Latency target | RSS target |
|---|---:|---:|
| `WINDOW_PARTITION_SUM_*` | 2–6× faster | 3–8× lower |
| `WINDOW_FRAMES_ROWS` | 2–5× faster | 3–6× lower |
| rank/dense_rank/row_number | 1.5–3× faster | 2–4× lower |

---

## 5. Priority-three changes: aggregate and GROUP BY fusion

### 5.1 Current behavior

Grouped projection evaluates aggregate expressions by scanning a group for each aggregate call. Expression caches help, but the shape is still interpreted and per-aggregate/per-group.

### 5.2 Rebuild target

Build an `AggProgram` per SELECT:

```rust
struct AggProgram {
    group_key_slots: Vec<ExprSlot>,
    aggregates: Vec<AggOp>,
    having: Option<ExprSlot>,
    projection: Vec<ProjectSlot>,
}

enum AggOp {
    CountStar,
    Count { arg: ExprSlot, distinct: Option<DistinctSpec> },
    SumI64 { arg: ExprSlot },
    SumF64 { arg: ExprSlot },
    Avg { arg: ExprSlot },
    Min { arg: ExprSlot, collation: Collation },
    Max { arg: ExprSlot, collation: Collation },
    GroupConcat { arg: ExprSlot, sep: ExprSlot },
}
```

Execution strategy:

1. One scan over source rows.
2. Encode group key once per row into scratch.
3. Hash to group state.
4. Update every aggregate accumulator in that group in one pass.
5. Evaluate HAVING and projection from accumulator slots.

**Expected impact:**

| Workload | Current symptom | Target |
|---|---|---|
| `AGG_GROUP_HAVING_*` | ~10 MiB RSS, repeated scans | 2–6× faster, 2–4 MiB RSS |
| multiple aggregate SELECTs | per-aggregate group scans | O(rows × aggregates) → O(rows + groups × aggregates) |
| DISTINCT aggregates | per-call hash sets | shared encoded-key arena per group/op |

---

## 6. Priority-four changes: ORDER BY / LIMIT / DML executor

The `DML_WHERE_ORDER_LIMIT_*` cases should not materialize and sort full candidate sets when an index can supply the order and limit.

### Target operator

```rust
PhysicalPlan::OrderedRowIdLimit {
    table: TableId,
    index: IndexId,
    range: KeyRange,
    order: ScanDirection,
    limit: usize,
    offset: usize,
    residual: Option<ExprSlot>,
}
```

Use this operator for:

- `SELECT ... WHERE ... ORDER BY indexed_col LIMIT n`
- `UPDATE ... WHERE ... ORDER BY indexed_col LIMIT n`
- `DELETE ... WHERE ... ORDER BY indexed_col LIMIT n`

The operator streams rowids from the B-tree in order, applies residual predicates, stops after `offset + limit`, then the DML path mutates only those rowids.

**Expected impact:** 2–5× on `DML_WHERE_ORDER_LIMIT_*`, larger if tables grow beyond benchmark sizes.

---

## 7. Priority-five changes: recursive CTE worktable

Recursive CTEs need a queue/worktable executor rather than repeated `Vec<Vec<SqlValue>>` clones.

Target design:

```rust
struct RecursiveWorkTable {
    queue: VecDeque<RowId32>,
    arena: RowArena,
    seen: Option<AHashSet<RowKeyBytes>>, // UNION vs UNION ALL
}
```

Execution:

1. Seed anchor rows into arena and queue.
2. Pop queue in batches.
3. Evaluate recursive branch against batch.
4. Deduplicate with encoded row keys for `UNION`.
5. Append new rows and continue until fixpoint or limit.

**Expected impact:** 2–4× latency, 2–5× RSS reduction on `CTE_RECURSIVE_MATRIX_*`.

---

## 8. Priority-six changes: planner correctness/performance surfaces

### 8.1 `INDEXED BY`

`INDEXED BY idx` must become a hard planner constraint, not a hint. If the named index cannot satisfy the table access, fail with SQLite-compatible error text. If it can, only enumerate that access path. This removes planner search and prevents accidental heap scans.

**Expected impact:** 2–3× on `INDEXED_BY`, plus better parity semantics.

### 8.2 Covering index projection

For `SELECT indexed_cols FROM t WHERE indexed_col ...`, project directly from index leaf payload where possible. Avoid heap row fetch and MVCC recheck only when visibility can be proven from index entry metadata; otherwise batch heap rechecks.

**Expected impact:** 1.5–4× on index-heavy SELECT, lower cache pressure.

### 8.3 Trigger/view generated paths

Compile triggers/views into cached `PreparedTemplate` programs keyed by schema epoch. Do not parse/bind trigger body SQL per row.

**Expected impact:** 2–4× on `VIEW_TRIGGER_GENERATED_*`.

---

## 9. Storage/HPC work for “beyond SQLite” wins

SQLite is extremely hard to beat on single-process, one-shot, tiny queries because its startup/memory footprint is tiny and mature. RedlineDB’s realistic advantage should come from places where SQLite’s architecture is constrained:

1. Multi-writer row-level concurrency.
2. Group-commit WAL throughput under concurrent write load.
3. Vector/JSONB/index extensions.
4. Covering index and top-K operators that avoid full materialization.
5. Native Rust specialization with PGO/BOLT and CPU-specific builds.

Recommended build stack:

```toml
[profile.release-max]
inherits = "release"
opt-level = 3
lto = "fat"
codegen-units = 1
panic = "abort"
strip = "symbols"
```

Training pipeline:

```bash
# 1. Instrument
RUSTFLAGS="-Cprofile-generate=/tmp/rldb-pgo" cargo build --profile release-native -p redlinedb-cli

# 2. Train on representative workloads
cargo run --profile release-native -p redlinedb-bench -- sqlite-parity --cases top-slow,window,agg,dml,index,cte
cargo run --profile release-native -p redlinedb-bench -- oltp --writers 1,4,16,64

# 3. Merge profile
llvm-profdata merge -o /tmp/rldb.profdata /tmp/rldb-pgo

# 4. Rebuild
RUSTFLAGS="-Cprofile-use=/tmp/rldb.profdata -Cllvm-args=-pgo-warn-missing-function" cargo build --profile release-native

# 5. Optional post-link BOLT
llvm-bolt target/release/redlinedb -o target/release/redlinedb.bolt -data=/tmp/rldb.fdata -reorder-blocks=ext-tsp -reorder-functions=hfsort -split-functions
```

Allocator strategy:

- Benchmark `mimalloc`, `jemalloc`, and system allocator separately.
- For query execution, allocator choice is second-best to arena design; first remove row/value clone storms.
- Add `--features allocator-mimalloc` and `--features allocator-jemalloc` for reproducible CI comparisons.

---

## 10. Acceptance gates

A change is accepted only if all pass:

1. SQLite parity pass count does not regress.
2. Top 10 latency ratios improve or remain neutral.
3. Top 10 RSS ratios improve or remain neutral.
4. `EXPLAIN`/planner expected outputs remain SQLite-compatible where currently tested.
5. Crash/failpoint matrix remains clean for storage changes.
6. New operator paths include fallback to current conservative executor.

Performance gates:

```text
P0 shell gate:
  DOT_CD_TEMPFILE <= 1.5 ms
  DOT_OUTPUT_TEMPFILE <= 4.0 ms
  DOT_ONCE_TEMPFILE <= 4.0 ms
  OPT_APPEND_TEMPFILE <= 2.0 ms when db-free scalar path applies

P1 memory gate:
  WINDOW_PARTITION_SUM_* RedlineDB RSS <= 3.0 MiB in lean CLI mode
  AGG_GROUP_HAVING_* RedlineDB RSS <= 4.0 MiB in lean CLI mode

P2 median gate:
  SQLite parity median latency <= 1.10× SQLite
  SQLite parity p75 latency <= 1.25× SQLite

P3 beyond-SQLite gate:
  16-writer disjoint-row insert/update throughput >= 3× SQLite WAL baseline
```

---

## 11. Expected aggregate outcome

With only the attached surgical diff, expect meaningful but not final improvement:

- CLI `.output`/`.once` fixed-cost and syscall reduction: 1.2–3× on affected cases.
- Private-memory RSS reduction: ~2–4× on many one-shot and window/aggregate tests.
- Window partition allocation reduction: 1.1–1.5× immediately, larger after full WindowProgram rebuild.
- DISTINCT ON: O(n²) duplicate detection removed; large-case improvement can be dramatic.

With the full rebuild:

| Area | Target improvement |
|---|---:|
| Top CLI tempfile latency outliers | 7–40× |
| Window partition/frames RSS | 3–8× lower |
| Aggregate/group/having latency | 2–6× |
| DML WHERE ORDER LIMIT | 2–5× |
| Recursive CTE | 2–4× |
| Median SQLite parity latency | 1.90× SQLite → 0.90–1.15× SQLite |
| Multi-writer OLTP throughput | plausibly 3–10× SQLite on disjoint writes |

Can RedlineDB be faster than SQLite? Yes, but not by trying to out-SQLite SQLite on every one-shot scalar CLI query with a fully initialized engine. The winning strategy is to (1) avoid opening the engine when the shell does not need one, (2) specialize transient cases, (3) remove materialization in SQL operators, and (4) exploit RedlineDB’s architectural advantage in concurrent writes and extensible native operators.
