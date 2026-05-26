# RedlineDB theoretical-limit rebuild plan

Generated from inspection of `neverhuman/RedlineDB` current `main` through the GitHub connector. The sandbox could not perform a direct `git clone` because DNS resolution is disabled, so the attached patch is not compile-tested here. It is written against the source files fetched from current `main` and should be treated as an aggressive first patch plus a larger engineering roadmap.

## Executive verdict

RedlineDB can beat SQLite on selected workloads, especially multi-writer, high-concurrency, covering-index, vectorized scan, top-K, and large analytical-window workloads. It should not try to beat SQLite by cloning SQLite’s exact row-at-a-time VDBE architecture. The winning strategy is:

1. Keep the SQLite surface contract and differential tests.
2. Replace row-at-a-time internals with streaming/vectorized Rust kernels.
3. Avoid opening the full storage engine when the shell workload is pure CLI or fromless scalar SQL.
4. Push LIMIT, ORDER BY, projection, and aggregate state as early as possible.
5. Treat every allocation in the executor as a bug unless it survives a memory-budget justification.

The current top slow cases are not one problem. They are four separate bottleneck families:

| Family | Evidence | Root cause | Fix class |
|---|---:|---|---|
| CLI tempfile | `.cd`, `.output`, `.once`, `.read`, `-append` up to 20× slower | full DB/engine setup for shell-only or scalar work; unbuffered file output; `.read` materializes whole script; hex encoding via formatting | ShellLite pre-open path, buffered output, streaming `.read`, LUT hex |
| Index/ORDER/LIMIT | `INDEXED_BY`, generated `WHERE ORDER LIMIT` about 3× slower | conservative index access, no expression-index matching, late LIMIT, row materialization before top-K | required-index planner lane, streaming top-K, covering expression indexes |
| Window memory | window partition SUM cases ~10 MiB vs 8 KiB reference | materialized full row sets, full partition vectors, per-window result vectors, generic O(n²) frame aggregation | partition-total aggregate, prefix/sliding window kernels, compressed peer layouts |
| Aggregate/CTE memory | GROUP/HAVING and recursive CTE around 9.7–9.9 MiB | hash agg overcharges memory and spills unnecessarily; recursive CTE materialization instead of queue streaming | accurate group accounting, inline keys, streaming recursive work queue |

## Patch included in `redlinedb_theoretical_limit.diff`

The diff implements the safest first wave:

### 1. Buffered CLI file output

`OutputTarget::File` becomes `BufWriter<File>` and is opened through `OutputTarget::file(path, append)`. `.output`, `.once`, and readonly sidecar dumping now use the buffered path. This removes a large syscall tax from the tiny tempfile cases.

Expected impact:

| Case | Current | Expected after patch | Expected after ShellLite phase |
|---|---:|---:|---:|
| DOT_OUTPUT_TEMPFILE | 28.42 ms | 10–18 ms | 1.2–2.0 ms |
| DOT_ONCE_TEMPFILE | 27.46 ms | 9–16 ms | 1.2–2.0 ms |
| DOT_READ_TEMPFILE | 20.18 ms | 8–14 ms | 2.0–3.0 ms |
| DOT_CD_TEMPFILE | 24.28 ms | mostly unchanged | 0.8–1.5 ms |

The patch does not pretend buffered output alone fixes `.cd`: `.cd` is slow because RedlineDB opens a full database before it knows the workload is only `.cd` + `.print`. That requires the ShellLite phase below.

### 2. Streaming `.read`

`run_script_file` now uses `BufReader` and `run_input_reader`, avoiding `fs::read_to_string` plus a second scan of the whole file. This matters for both the benchmark and real `.read` scripts.

### 3. Fast `readfile()` hex rendering

`SELECT hex(readfile(...))` now uses a byte lookup table rather than `write!` formatting per byte.

Expected impact: small for tiny files, large for blob/file verification cases. For large files, hex rendering becomes memory-bandwidth bound instead of formatter-bound.

### 4. Whole-partition window aggregate fast path

The current window evaluator resolves `OVER (...)` calls against already materialized rows and computes partitions/order/frames in memory. The default frame for `OVER (PARTITION BY ...)` without `ORDER BY` is the entire partition. The generic path recomputes `SUM`/`COUNT`/`AVG`/`MIN`/`MAX` for each row, which is O(n²) per partition. The patch adds `partition_total_aggregate_window`: accumulate once per partition, then fan out the final value.

Expected impact:

| Workload | Current shape | New shape | Speedup | Memory reduction |
|---|---|---|---:|---:|
| `SUM(x) OVER (PARTITION BY k)` | O(n²) frame accumulation | O(n) accumulation + O(n) fanout | 10–100× for large partitions | 20–60% immediately |
| `COUNT/AVG/MIN/MAX/TOTAL` same frame | O(n²) | O(n) | 10–100× | 20–60% immediately |

RSS will not drop to SQLite’s reported 8 KiB in the current harness because a Rust process plus allocator baseline is already multiple MiB. The correct long-term memory metric should be baseline-subtracted delta RSS plus allocator allocation counters.

### 5. Window partition key materialization reduction

`partition_rows` no longer builds `Vec<Vec<SqlValue>>` for every partition key before hashing. It evaluates and hashes each row’s key in one pass. This reduces the peak memory of partition-heavy windows and avoids one full extra copy of group keys.

### 6. Hash aggregate memory accounting fix

`HashAggregator::observe` previously increased `table_bytes` on every input row, even when the row hit an existing group. That can trigger false spills for small hot group counts. The patch charges memory only when inserting a new group.

Expected impact:

| Case family | Expected speedup | Expected memory reduction |
|---|---:|---:|
| GROUP BY with few groups, many rows | 1.5–5× if spills were false | 30–80% spill reduction |
| GROUP/HAVING generated cases | 1.2–2.5× | 10–50% |

### 7. Streaming no-ORDER LIMIT and streaming top-K

`order_and_project_rows_with_distinct_on` now returns immediately for `LIMIT 0`, streams no-ORDER `LIMIT/OFFSET`, and uses a streaming top-K heap before building the full filtered row vector. This aligns with SQLite’s early LIMIT behavior and reduces work for generated `WHERE ORDER LIMIT` cases.

Expected impact:

| Case family | Current | Expected |
|---|---:|---:|
| `SELECT ... WHERE ... LIMIT n` | materializes every passing row | stops after offset+n passing rows |
| `ORDER BY ... LIMIT small_n` | filters all rows into a vector, then top-K | pushes directly into top-K heap |
| DML verification queries | 3× SQLite in worst cases | 1.1–1.8× SQLite after patch; faster after index ORDER BY lanes |

## Required second wave: ShellLite pre-open executor

The top CLI tempfile cases will not be solved completely until the CLI can answer shell-only and fromless scalar workloads before opening the database engine. Current behavior opens a `Database`, creates a `Connection`, initializes state, and only then executes `.cd`, `.print`, `.output`, or `SELECT 1`.

Design:

```text
argv/stdin
  -> classify input
  -> if ShellLite-compatible, execute with no Database open
  -> else open Database and run current path
```

ShellLite-compatible commands:

- `.cd DIR`
- `.print ...`
- `.output FILE|stdout|off`
- `.once FILE`
- `.mode`, `.headers`, `.separator`, `.nullvalue` state changes
- fromless deterministic scalar SQL: `SELECT <literal/arithmetic/function-without-db-state>`
- `SELECT hex(readfile('path'))`
- `-version`, `-help`, `-interactive` smoke paths

Non-compatible commands fall back to the full engine:

- any `FROM`, schema access, DML, DDL, transaction, PRAGMA requiring engine state
- UDFs that consult connection state
- authorizer/collation/parameter features requiring `Connection`

Expected impact:

| Case | Current | Target after ShellLite |
|---|---:|---:|
| DOT_CD_TEMPFILE | 24.28 ms | 0.8–1.5 ms |
| DOT_OUTPUT_TEMPFILE | 28.42 ms | 1.2–2.0 ms |
| DOT_ONCE_TEMPFILE | 27.46 ms | 1.2–2.0 ms |
| OPT_APPEND_TEMPFILE | 20.56 ms | 1.0–1.8 ms when SQL is fromless |
| DOT_READ_TEMPFILE | 20.18 ms | 2.0–3.0 ms |

Implementation notes:

- Put this behind an internal `shell_lite` module, not benchmark-specific branches.
- Use the same renderer functions as the full CLI where possible.
- Add differential tests that compare full-engine and ShellLite output for every supported subset.
- Count this as a semantic optimization: if a statement has no database dependencies, opening the database is unnecessary work.

## Required third wave: index and planner rebuild

### A. `INDEXED BY` as a hard planning contract

SQLite’s `INDEXED BY` is not a hint. It requires the named index to be usable, and statement preparation fails if it cannot be used. RedlineDB should model this as a hard access-path contract:

```rust
enum RequiredAccessPath {
    Any,
    NoIndex,
    Index(IndexId),
}
```

Plan rules:

- Parse `INDEXED BY idx` into `RequiredAccessPath::Index(idx)`.
- If the named index cannot satisfy the access predicate, return a prepare error instead of silently scanning.
- If the index covers the projection, use the covering path.
- If not covering, use index rowids + heap lookup.
- If `NOT INDEXED`, disallow secondary indexes but still allow rowid lookup.

Expected impact for `00073 INDEXED_BY`:

- Current: 5.08 ms vs SQLite 1.56 ms.
- Target: 1.1–1.8 ms for tiny memory case.
- With ShellLite/open-cost reduction and statement cache: <= SQLite for repeated prepared statements.

### B. Expression-index access

The current index access code skips expression-source keys. That explains both the `EXPRESSION_INDEX` memory offender and the planner conservatism. Add:

```rust
trait IndexKeyExpr {
    fn expr_fingerprint(&self) -> ExprFingerprint;
    fn eval_key(&self, row: &RowContext, bindings: &[Option<SqlValue>]) -> Result<SqlValue>;
}
```

Rules:

- Canonicalize expressions during CREATE INDEX.
- Store expression fingerprint and bytecode/lowered AST with the index key.
- During planning, match WHERE/ORDER expressions by fingerprint.
- During DML, compute expression keys once per inserted/updated row.
- During SELECT, use expression index for equality/range/order where safe.

Expected impact:

| Case | Expected speedup | Memory impact |
|---|---:|---:|
| EXPRESSION_INDEX | 2–8× | removes full-scan/sort allocations |
| ORDER BY expression LIMIT | 5–30× | streaming index order |

### C. Reverse cursors and ORDER BY LIMIT without WHERE

Current ORDER BY LIMIT index path depends on `try_match_index_access`, meaning it usually needs a WHERE predicate first. Add full-index ordered scans:

```rust
IndexProbe::FullForward
IndexProbe::FullReverse
```

Then plan:

- `ORDER BY indexed_col ASC LIMIT n` -> forward index scan early stop.
- `ORDER BY indexed_col DESC LIMIT n` -> reverse index scan early stop.
- If projection covered by index, never touch heap.
- If projection not covered, rowid batch load only the top K rows.

Expected impact: 3–20× for ordered LIMIT queries over indexed columns.

## Required fourth wave: executor memory architecture

The current executor still frequently turns query results into `Vec<Vec<SqlValue>>` or `Vec<SqlRow>`. That shape is easy to implement but fundamentally loses to SQLite on tiny memory cases and loses to vector engines on large scans.

Target architecture:

```text
PlanNode::next_batch(&mut self, &mut RowBatch) -> Result<BatchState>

ScanNode          -> fills RowBatch from heap/index
FilterNode        -> bitmap selection vector
ProjectNode       -> column vectors / late materialization
TopKNode          -> fixed-size heap
HashAggNode       -> group table with inline small keys
WindowNode        -> partition streaming kernels
SortNode          -> bounded in-memory or external merge sort
RenderNode        -> writes directly to output buffer
```

Rules:

- No `Vec<Vec<SqlValue>>` in hot paths.
- Rows crossing node boundaries must be batches, not heap-allocated row vectors.
- `LIMIT` must be a physical operator that can stop upstream nodes.
- Projection should be late: do not decode columns not needed by the query.
- Text/blob values should be borrowed where possible and copied only at output or spill boundaries.

## Required fifth wave: window kernels

Window execution should be split by frame class:

| Frame class | Kernel | Complexity |
|---|---|---:|
| whole partition | one aggregate per partition + fanout | O(n) |
| prefix unbounded preceding to current row | running accumulator | O(n) |
| bounded ROWS frame | sliding accumulator with inverse function | O(n) |
| ranking functions | peer-run compression | O(n) |
| lag/lead | direct indexed read | O(n) |
| arbitrary RANGE/GROUPS | peer-range prefix sums or fallback | O(n log n) or fallback |

Use a `WindowLayout` with compressed partitions:

```rust
struct WindowLayout {
    rowids: Vec<u32>,          // row order in partition order
    partition_offsets: Vec<u32>,
    peer_offsets: Vec<u32>,
}
```

Do not store `Vec<usize>` for every partition and peer array unless row count requires `usize`.

## Required sixth wave: hash aggregation rebuild

Patch wave fixes false spill accounting. The theoretical version should also:

- Use inline small keys: `[u8; 24]` or `SmallVec<[u8; 32]>` for common integer/text-short groups.
- Use aggregate states stored in a struct-of-arrays layout for SIMD-friendly updates.
- Avoid cloning `SqlValue` for min/max if the source value can be referenced until finalization.
- Add `HAVING` pushdown when it can be evaluated from aggregate state.
- Add sorted aggregation when input is already ordered by group key.

Expected impact: 2–10× on GROUP BY workloads depending on group cardinality.

## Required seventh wave: storage/kernel theoretical limit

SQLite is extremely optimized, but it is intentionally constrained by a single WAL writer. RedlineDB’s advertised advantage is MVCC + concurrent B-tree + group-commit WAL. To actually beat SQLite beyond microcases:

1. Replace coarse B-tree structure locks with B-link pages and per-page latches.
2. Use optimistic reads with epoch/hazard protection.
3. Make secondary-index maintenance append-friendly and batchable.
4. Implement group commit with fsync coalescing.
5. Use per-core WAL reservation lanes that merge into a durable sequence.
6. Add page-cache admission/eviction tuned for mixed OLTP + analytics.
7. Keep readers latch-free after snapshot acquisition.

Expected impact:

| Workload | Target vs SQLite |
|---|---:|
| single-row point read | parity to 1.2× faster |
| indexed range scan, covering | 1.5–5× faster |
| many concurrent writers | 5–50× faster throughput if conflict-free |
| analytical scan with vector filters | 2–20× faster |
| tiny CLI cold start | only faster if ShellLite skips engine open |

## Measurement rebuild

The current RSS ratios are useful for ranking but misleading in absolute terms. A Rust process plus allocator baseline can exceed several MiB before query allocations. SQLite’s 8 KiB number appears to be a near-zero delta case, not the total resident footprint of a full shell process.

Add these metrics:

```text
process_start_rss
post_open_rss
pre_query_rss
peak_query_rss
post_query_rss
query_delta_peak = peak_query_rss - pre_query_rss
allocation_count
allocated_bytes
freed_bytes
spill_bytes
syscall_count
```

Tools:

- Linux: `perf stat`, `perf record`, `heaptrack`, `massif`, `/proc/self/smaps_rollup`.
- Rust: `dhat`, `tikv-jemallocator` or `mimalloc`, optional allocation hooks in benchmark builds.
- CI: fail PR when a case regresses >10% latency or >64 KiB query-delta RSS.

## Can RedlineDB be faster than SQLite?

Yes, but not by trying to be a Rust clone of SQLite’s VDBE. The fastest route is to be SQLite-compatible at the boundary and more modern internally:

- faster than SQLite on concurrent writes by not being single-writer constrained;
- faster on covering-index reads by avoiding heap loads;
- faster on ORDER BY LIMIT by index-order early stop or streaming top-K;
- faster on windows/aggregates by O(n) kernels rather than repeated row-frame scans;
- faster on large scans by vectorization and SIMD;
- faster on tiny CLI cases only by skipping the engine entirely when no database state is needed.

The realistic target after the first two waves:

| Metric | Current README | 30-day target | 90-day target |
|---|---:|---:|---:|
| Median parity latency | 1.90× SQLite | 1.15–1.35× | 0.80–1.05× |
| Worst CLI tempfile ratio | 20.27× | 3–8× | 0.8–1.5× |
| Worst SQL ratio in top 10 | 3.25× | 1.3–2.0× | 0.8–1.2× |
| Median peak RSS ratio | 763× | 200–400× raw / <2× delta | raw baseline accepted / <= SQLite on delta for many cases |
| Window partition SUM | ~10 MiB raw | 5–7 MiB raw | baseline + <128 KiB query delta |

## Correctness gates

Every optimization must pass:

1. SQLite parity corpus.
2. sqllogictest randomized corpus.
3. SQLancer-style fuzzing for SELECT/DML/index/window queries.
4. Crash/recovery fuzzing with torn WAL and random process kill.
5. MVCC isolation tests under concurrent readers/writers.
6. Deterministic plan tests for `INDEXED BY` and `NOT INDEXED`.
7. Allocation budget tests for top memory cases.

## Risk register

| Risk | Mitigation |
|---|---|
| ShellLite diverges from full engine rendering | Share render functions and differential-test ShellLite vs full engine |
| Window fast path mishandles EXCLUDE/RANGE semantics | Only enable for exact unbounded/no-exclude frames; fallback otherwise |
| Hash agg memory undercount | Charge new groups by encoded key + value width + state width; add allocation tests |
| Top-K changes collation behavior | Disable streaming top-K for custom/UINT comparator cases; fallback to existing comparator path |
| Expression index mismatch | Use canonical expression fingerprints and require deterministic functions only |
| MVCC index visibility bugs | Keep heap visibility recheck until index entries carry sufficient MVCC metadata |

## Immediate landing order

1. Land attached patch and run full parity.
2. Add allocation/RSS delta instrumentation.
3. Implement ShellLite pre-open path for CLI-only and fromless scalar cases.
4. Add `INDEXED BY` hard access contract tests.
5. Add expression-index matching for equality and covering projection.
6. Add full forward/reverse ordered index scans for ORDER BY LIMIT.
7. Replace window result materialization with batch/streaming kernels.
8. Replace recursive CTE materialization with queue streaming.
9. Move storage concurrency work behind new kernel-stage benchmarks.

