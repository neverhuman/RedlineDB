# RedlineDB Extreme Performance Rebuild Spec

Date: 2026-05-25  
Target: make RedlineDB the fastest SQLite-compatible, Rust-native embedded SQL database where the design is allowed to exceed SQLite internally while preserving SQLite-facing behavior.

## Executive diagnosis

The current public README reports strong semantic parity but large performance gaps in specific clusters:

- SQLite parity: 1127 total, 1123 passed, 4 skipped, 0 failed, 99.6% pass rate.
- Median parity latency: RedlineDB 3.27 ms vs SQLite 1.71 ms, or 1.90× slower.
- Median peak RSS: RedlineDB 8.9 MiB vs SQLite 12 KiB, or 763× higher.
- Worst latency cluster: CLI tempfile cases from 7.86× to 20.27× slower.
- Worst memory cluster: window/aggregate/CTE/index cases near 9.7-10.0 MiB while SQLite reports 8 KiB.

My read is that the main losses are not “Rust is slow.” They are concrete architecture choices:

1. File output in the CLI is unbuffered for `.output`, `.dump` sidecars, and normal `OutputTarget::File`, causing one syscall per tiny write in several tempfile benchmarks.
2. `.read` loads the entire script into a `String` and then reparses line control from memory.
3. `SELECT hex(readfile(...))` reads the whole file and builds a full hex `String`.
4. Window execution materializes input rows, precomputes every window call into `Vec<Vec<Vec<SqlValue>>>`, then materializes the final projection. Its fallback aggregate path rebuilds an accumulator over the frame for every output row.
5. Planner/executor support for index range, covering scans, top-K, and ordered index limit already exists, but it is conservative: only simple leading-column cases are handled, no reverse cursor path, no composite ORDER BY satisfaction, expression index matching is explicitly skipped, and `INDEXED BY` is not visible in the searched code paths.
6. The process baseline is high relative to SQLite’s benchmark-reported 8 KiB RSS. Some of that can be reduced, but a Rust binary using clap, sqlparser, a Rust allocator, and the current engine will not honestly reach an 8 KiB total RSS floor. The right measurement is baseline-adjusted delta RSS and allocations per statement.

## Immediate patch lane

The accompanying diff is intentionally surgical and targets the highest-confidence fixes first.

### 1. Buffer all file output

Change `OutputTarget::File` from `File` to `BufWriter<File>` with a 256 KiB buffer and route `.output`, `.once`, and sidecar dumps through the same helper.

Expected impact:

| Case | Current RedlineDB | SQLite | Expected after patch | Improvement |
|---|---:|---:|---:|---:|
| DOT_OUTPUT_TEMPFILE | 28.42 ms | 1.66 ms | 1.6-3.0 ms | 9-18× |
| DOT_ONCE_TEMPFILE | 27.46 ms | 1.48 ms | 1.5-2.8 ms | 10-18× |
| OPT_APPEND_TEMPFILE | 20.56 ms | 1.37 ms | 1.6-3.2 ms | 6-13× |

The remaining gap after buffering will mostly be process startup, clap parsing, database/session open, and compatibility setup.

### 2. Stream `.read` and file-to-hex

Replace `.read`’s full `read_to_string` with a `BufReader` that feeds the same shell state machine line-by-line. Replace `SELECT hex(readfile(...))` full-file `Vec<u8>` + full hex `String` with chunked read + stack-buffered hex encoding.

Expected impact:

| Case | Current RedlineDB | SQLite | Expected after patch | Improvement |
|---|---:|---:|---:|---:|
| DOT_READ_TEMPFILE | 20.18 ms | 2.57 ms | 3.0-5.5 ms | 3.7-6.7× |
| readfile/hex variants | workload-dependent | workload-dependent | 2-20× less allocation | large files benefit most |

### 3. Window aggregate fast paths

Add partition-wide and peer-prefix aggregate fast paths:

- `SUM/COUNT/AVG/MIN/MAX/TOTAL(...) OVER (PARTITION BY ...)`
- default no-ORDER frame: `RANGE UNBOUNDED PRECEDING ... UNBOUNDED FOLLOWING`
- default ORDER frame: `RANGE UNBOUNDED PRECEDING ... CURRENT ROW`
- existing `ROWS UNBOUNDED PRECEDING ... CURRENT ROW` fast path remains

This changes common partition sum shapes from O(partition_size²) frame rescans to O(partition_size).

Expected impact:

| Cluster | Current RSS | Expected dynamic RSS | Expected latency |
|---|---:|---:|---:|
| WINDOW_PARTITION_SUM_* | ~9.9-10.0 MiB | 2-5 MiB total RSS, much lower delta RSS | 3-30× faster depending partition size |
| WINDOW_FRAMES_ROWS | ~9.7 MiB | 3-6 MiB total RSS | 2-12× faster for prefix frames |

This does not fully solve window memory. It removes the worst common O(n²) path; the full rebuild below eliminates the extra materialization layers.

## The aggressive rebuild

### A. Shell and parity harness

The CLI currently pays a large cold-start tax in tests that are trying to measure small shell features. Keep the CLI compatible, but add a persistent in-process shell benchmark harness for performance work so we can separate shell-cold-start from engine performance.

Required changes:

1. `CliSession` API:
   - owns `CliState`
   - accepts `&[u8]` input chunks
   - emits into a caller-provided `Write`
   - exposes counters: syscalls, bytes written, statements prepared, rows emitted, allocations if allocator instrumentation is on

2. `OutputTarget`:
   - `Stdout(BufWriter<Stdout>)`
   - `File(BufWriter<File>)`
   - `Null`
   - future: `Bytes(Vec<u8>)` for in-process tests

3. Dot command parser:
   - keep SQLite-compatible quoting semantics
   - add zero-allocation fast parser for common unquoted forms:
     - `.cd X`
     - `.read X`
     - `.output X`
     - `.once X`
     - `.mode X`
     - `.headers on|off`

4. CLI binary split:
   - `redlinedb-cli-min`: no rustyline, no maintenance subcommands, no server deps, minimal clap replacement or lexopt
   - `redlinedb`: full user shell

Expected: tempfile shell cases move from 7.86-20.27× slower to 0.8-2.0× SQLite. Some cases can beat SQLite if output is large enough for buffered Rust writes to dominate startup.

### B. Planner/index rebuild

The code already has index probe and ordered index limit machinery, but the current shape is deliberately conservative. To beat SQLite consistently, make the planner’s access path layer a first-class subsystem.

Implement `AccessPath`:

```rust
enum AccessPath {
    RowIdPoint { rowid: RowId },
    IndexPoint { index_id: IndexId, key: EncodedKey, covering: CoveringMap },
    IndexRange {
        index_id: IndexId,
        start: EncodedKey,
        end: EncodedKey,
        dir: ScanDir,
        covering: CoveringMap,
        order_satisfies: OrderSatisfaction,
        hard_limit: Option<usize>,
    },
    MultiIndexAnd { children: SmallVec<[AccessPath; 4]> },
    MultiIndexOr { children: SmallVec<[AccessPath; 4]> },
    TableScan,
}
```

Required capabilities:

1. `INDEXED BY` semantics:
   - parse and attach forced index to table source
   - if forced index cannot be used, error like SQLite instead of silently scanning
   - allow forced expression indexes

2. Expression indexes:
   - canonicalize expression AST
   - match WHERE and ORDER BY expressions against index expression keys
   - store expression result in index leaf payload for covering expression scans

3. Composite ORDER BY:
   - support multi-column index order satisfaction
   - support ASC/DESC and reverse cursor
   - support null ordering rules exactly

4. WHERE + ORDER BY + LIMIT:
   - push `limit + offset` to index cursor
   - stop before heap loads
   - if projection is covering, never touch heap
   - if not covering, heap load in rowid/page order when ORDER BY does not need stable cursor order; otherwise preserve cursor order

5. Cost model:
   - use `sqlite_stat1`-like stats plus Redline-specific page density, MVCC visibility rate, and covering ratio
   - track actual row counts and feed them back into stats after ANALYZE

Expected impact:

| Case family | Expected speedup vs current |
|---|---:|
| INDEXED_BY | 2-5× |
| DML_WHERE_ORDER_LIMIT_* | 2-6× |
| expression index memory case | 2-10× lower allocations depending expression width |
| top-K ORDER BY small LIMIT | 2-20× when it avoids full materialization/sort |

### C. Window executor rebuild

The real fix is to stop treating window execution as a post-processing vector transform.

Current structure:

```text
collect base rows
filter
window_values: Vec<projection_item -> window_call -> row_value>
projected: Vec<row -> projected_values>
sort projected
skip/take
StaticRows runtime
```

Target structure:

```text
source cursor -> partitioner -> peer/order layout -> vectorized window kernels -> projection sink
```

Design:

1. `WindowPlan`:
   - group identical window specs once
   - assign each function call to a layout
   - precompile argument expressions into evaluators
   - detect fast kernels:
     - partition constant aggregate
     - prefix aggregate
     - peer-prefix aggregate
     - sliding ROWS frame with inverse transition
     - rank/row_number/ntile direct formulas
     - lag/lead offset lookup

2. `WindowPartition` memory:
   - store row references and only columns needed by window args, partition keys, order keys, projection, and final ORDER BY
   - use `SmallVec<[SqlValue; 4]>` for small row vectors
   - intern repeated partition/order keys
   - spill partitions above memory budget

3. Aggregate kernels:
   - `sum/count/total/avg`: prefix arrays or running accumulator
   - sliding `ROWS BETWEEN N PRECEDING AND CURRENT ROW`: inverse accumulator
   - `min/max`: monotonic deque for sliding frames
   - `first/last/nth`: index arithmetic, no `Vec<usize>` per frame

4. Remove per-row frame allocation:
   - replace `enumerate_frame_positions -> Vec<usize>` with iterator/fold callbacks
   - never allocate frame positions for first/last/nth/aggregates

Expected: the worst window memory cluster should no longer allocate 3 layers of row vectors. Latency for partition sums should become linear and memory should become proportional to one partition’s required columns, not all projected values plus all window values.

### D. Row/value memory model

`SqlValue` cloning is currently too easy. For theoretical limit, split value representation by lifetime:

```rust
enum ValueRef<'a> { Null, Integer(i64), Real(f64), Text(&'a str), Blob(&'a [u8]) }
enum ValueOwned { Null, Integer(i64), Real(f64), Text(Arc<str>), Blob<Arc<[u8]>) }
struct RowView<'a> { cols: &'a [ValueRef<'a>] }
struct RowArena { strings: Bump, blobs: Bump, rows: Vec<RowSlot> }
```

Rules:

1. Operators consume `RowView` and only materialize `ValueOwned` at API boundaries, sort spill, or storage writes.
2. Projection into list/csv/tabs writes directly from `ValueRef`.
3. Table/box/markdown modes materialize only because widths require two-pass layout.
4. Per-statement arena reset after execution.
5. Schema column names interned per connection/schema epoch.
6. Avoid `Vec<Vec<SqlValue>>`; use `RowBatch` + fixed-layout columns.

Expected: 30-80% fewer allocations on scalar/select paths; 2-10× lower dynamic RSS in window/aggregate/CTE cases.

### E. Execution engine

Move from row-at-a-time to morsel/vector execution where semantics permit:

1. Batch size: 256 or 1024 rows; tune per CPU cache.
2. Operators:
   - filter: vectorized boolean mask
   - projection: expression kernels over column vectors
   - hash aggregate: SIMD hash/fingerprint, partitioned hash table
   - sort: radix/integer fast path, timsort/string fallback, top-K heap
3. Expression specialization:
   - cache compiled expression trees by SQL hash + schema epoch
   - specialize common types after first batch
   - avoid dynamic dispatch in inner loops
4. SQL parser/prepare cache:
   - connection LRU keyed by SQL bytes + schema epoch + parameter count
   - store normalized statement and lowered plan

Expected: 1.5-5× on generated SQL workloads after correctness stabilization.

### F. Storage/kernel

SQLite is extremely strong on single-process, single-writer, cache-hot workloads. RedlineDB can beat it by exploiting the design goal SQLite does not optimize for: multi-writer MVCC.

1. WAL:
   - group commit by default
   - commit combiner thread optional
   - separate durable commit from visible commit with configured sync policy
2. B-tree:
   - latch coupling with optimistic read validation
   - prefix-compressed keys
   - page-local binary search with SIMD key prefix compare
   - prefetch next leaf during range scan
3. Buffer pool:
   - sharded clock-pro or tinylfu admission
   - hot root/internal pages pinned
   - per-core page caches
4. Index:
   - covering payload support
   - reverse cursor
   - bulk-build index path
5. Concurrency:
   - no global engine mutex
   - table/index-level write conflict detection
   - MVCC snapshots are immutable and cheap

Expected: RedlineDB can beat SQLite substantially on concurrent write/read workloads and selected analytic/window/index workloads. On tiny one-shot CLI invocations, beating SQLite everywhere is much less likely because SQLite’s C shell and process footprint are extremely small.

## Build and HPC policy

The root already uses release `opt-level=3`, `lto="fat"`, `codegen-units=1`, `panic="abort"`, and stripped symbols. `.cargo/config.toml` already uses `target-cpu=native` and mold on Linux.

Add:

1. PGO:
   - train on the full parity suite plus the top 50 slowest cases repeated 100×
   - separate training for CLI and library
2. BOLT:
   - post-link optimize `redlinedb` and core benchmark binaries on Linux
3. CPU feature lanes:
   - baseline portable
   - x86_64-v3
   - x86_64-v4
   - native
4. Allocator lanes:
   - mimalloc default for CLI
   - optional snmalloc/jemalloc/tikv-jemallocator benchmark variants
   - allocator telemetry in benchmark harness
5. Instrumentation gates:
   - instructions/op
   - branches/op
   - branch-misses/op
   - L1/L2/LLC misses
   - syscalls/op
   - allocations/op and allocated bytes/op
   - peak and baseline-adjusted RSS

## Validation gates

Every optimization must pass three gates:

1. SQLite parity gate:
   - all 1127 parity cases remain pass/skip-compatible
   - targeted slow cases must not regress
2. Differential fuzzing:
   - sqllogictest-style random generation against SQLite
   - seed-preserving minimized failures
3. Performance gate:
   - top 10 current slow latency cases
   - top 10 current RSS cases
   - top 10 current RedlineDB wins, to defend existing wins
   - concurrent write benchmark where RedlineDB should beat SQLite

Suggested hard performance targets:

| Metric | Near-term | Aggressive |
|---|---:|---:|
| median parity latency ratio | <=1.25× SQLite | <=0.90× SQLite |
| top tempfile latency ratio | <=2.0× SQLite | <=1.1× SQLite |
| SQL index/order-limit cases | <=1.2× SQLite | <=0.7× SQLite |
| window partition sum latency | <=1.0× SQLite | <=0.5× SQLite |
| baseline-adjusted RSS ratio | <=5× SQLite | <=2× SQLite |
| total process RSS vs SQLite 8 KiB cases | cannot honestly hit 8 KiB with this Rust binary | minimize and report delta |

## Can RedlineDB be faster than SQLite?

Yes, but not uniformly in the naive one-shot CLI microbenchmark sense.

RedlineDB can beat SQLite where its architecture is allowed to matter:

- concurrent writers
- read/write overlap
- covering index scans
- ORDER BY LIMIT with early stop
- vectorized expressions
- window/aggregate kernels
- Rust-native API without C ABI/shell overhead
- custom compiled `target-cpu=native` builds

SQLite will remain hard to beat on:

- tiny one-shot shell invocations
- total process RSS floor
- decades-hardened single-threaded B-tree paths
- edge-case SQL semantics where every fast path must preserve historical quirks

The correct goal is not “win every row in the current CSV immediately.” The correct goal is:

1. close the obvious self-inflicted gaps,
2. beat SQLite on core engine paths,
3. preserve wins already present,
4. expose fair benchmark dimensions where RedlineDB’s concurrent MVCC design dominates,
5. keep SQLite parity non-negotiable.
