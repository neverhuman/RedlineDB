# RedlineDB theoretical-limit performance rebuild spec

Generated for `neverhuman/RedlineDB` mainline after inspecting the current public repository and the SQLite-parity benchmark table.

## 1. Executive diagnosis

The top regressions split into two different classes:

1. **CLI startup / tempfile microbenchmarks are not storage-engine problems.**
   The worst five latency cases are `.cd`, `.once`, `.output`, `-append`, and `.read` smoke tests. Their measured work is dominated by process startup, argument parsing, eager database open/connection creation, unbuffered file output, whole-file readback, and full SQL-engine bootstrap for scripts that often do not need the database at all.

2. **The huge RSS ratios are mostly baseline-residency problems, not query-cardinality problems.**
   Several worst RSS cases are 9-row generated window queries but report about 10 MiB peak RSS. That is not the working set required to compute the result. The CLI currently hard-wires `mimalloc` as the global allocator, and the engine defaults are optimized for a resident database process rather than short-lived SQLite shell parity microcases. The correct fix is to separate “micro-CLI parity” allocator policy from “long-running throughput” allocator policy.

3. **The SQL engine already has important fast lanes but needs more early-exit and late-materialization discipline.**
   Current `select_top.rs` has fromless `SELECT` fast paths, covering index paths, ordered-index+limit, and Top-K paths. The remaining 3x SQL cases are consistent with missed early LIMIT propagation, insufficient covering ordered index usage, row materialization before sort/projection, and small-vector allocation overhead.

## 2. Patch delivered

The accompanying `redlinedb-theoretical-limit.diff` implements a first-stage, high-impact patch:

### 2.1 CLI allocator policy

`mimalloc` becomes optional behind the `fast-allocator` feature. Default CLI builds use the platform allocator, which should substantially reduce peak RSS in short-lived parity/memory runs. Long-running throughput builds can still opt in:

```bash
cargo build -p redlinedb-cli --release --features fast-allocator
```

### 2.2 Buffered file output

`.output` and sidecar dump file targets now use `BufWriter<File>` with a 64 KiB buffer. `.once` already used a `BufWriter`; the patch keeps that and adds the same treatment to persistent file output.

### 2.3 Streaming `hex(readfile())`

The CLI special-case for `SELECT hex(readfile('...'))` no longer does:

```rust
std::fs::read(path) -> Vec<u8>
format! per byte -> String
write whole rendered string
```

It now streams through a fixed 8 KiB input buffer and 16 KiB output buffer using a static uppercase hex lookup table. This cuts temporary allocation to O(1), removes `fmt::Write` in the byte loop, and improves the `.output` / `.once` verification cases.

### 2.4 No-DB fast CLI lane

The patch adds a conservative fast lane for scripts made entirely of:

- `.mode`, `.headers`, `.separator`, `.nullvalue`
- `.cd`, `.print`, `.output`, `.once`, `.read`
- fromless scalar `SELECT <integer>;`
- `SELECT hex(readfile('...'));`

It validates the entire script before executing side effects. Unsupported scripts fall back to the full shell. This targets the worst tempfile cases without weakening SQLite compatibility for real SQL.

### 2.5 Fromless argv fast lane

The patch also allows simple argv SQL such as `redlinedb -append /tmp/append.db 'SELECT 1;'` to avoid full engine bootstrap while still touching/creating the database file so shell smoke semantics are preserved.

### 2.6 Window accumulator allocation removal

`Accumulator` no longer stores `kind: String` and no longer clones the full accumulator for every prefix value. It uses a compact `AccumulatorKind` enum, and `value()` computes from borrowed state. This is a low-risk hot-loop fix for prefix window aggregates.

## 3. Expected speed and memory impact

These are engineering estimates, not measured results, because the execution container could not resolve `github.com` and therefore could not perform a full checkout/compile/benchmark run. The estimates are based on the code paths inspected and the benchmark magnitudes in the README.

| Case | Current RedlineDB | SQLite | Target after patch | Expected improvement | Why |
|---|---:|---:|---:|---:|---|
| `00153 DOT_CD_TEMPFILE` | 24.28 ms | 1.20 ms | 1.2–2.0 ms | 12–20x | No DB open, no SQL setup, direct `.cd` + `.print`. |
| `00149 DOT_ONCE_TEMPFILE` | 27.46 ms | 1.48 ms | 2.0–3.5 ms | 8–14x | No DB boot for `SELECT 2/3`, buffered once file, streaming readfile verification. |
| `00148 DOT_OUTPUT_TEMPFILE` | 28.42 ms | 1.66 ms | 2.0–3.2 ms | 9–14x | Buffered `.output`, no DB boot for scalar select, no whole-file hex allocation. |
| `00202 OPT_APPEND_TEMPFILE` | 20.56 ms | 1.37 ms | 1.8–3.5 ms | 6–11x | Fromless argv fast lane touches DB file and emits scalar output without engine open. |
| `00146 DOT_READ_TEMPFILE` | 20.18 ms | 2.57 ms | 2.5–4.0 ms | 5–8x | Validated `.read` fast lane executes trivial script without DB boot. |
| Window RSS top cases | ~9.7–10.0 MiB | 8 KiB | ~0.7–2.5 MiB | 4–14x RSS reduction | Optional allocator removes MiB-scale allocator baseline; query itself is tiny. |
| Prefix window aggregate CPU | baseline | SQLite | 1.05–1.30x faster | modest on tiny cases, larger on long partitions | No string kind allocation; no accumulator clone per output row. |

The memory target still does **not** promise 8 KiB RSS. A Rust CLI plus SQL engine will normally have a larger process baseline than SQLite’s C shell. The realistic near-term goal is to eliminate the artificial 10 MiB floor and make the memory suite measure query/data structures rather than allocator residency.

## 4. Can RedlineDB be faster than SQLite?

Yes, but not by treating SQLite as a single scalar. SQLite is extremely hard to beat in tiny single-statement CLI startup microcases because it is a mature C shell with decades of hot-path compression. RedlineDB can beat SQLite in targeted lanes where the design is allowed to diverge:

- concurrent writers and MVCC workloads;
- append-heavy ingest with group commit;
- covering-index queries with late materialization;
- vectorized scans and hash aggregation;
- top-K ORDER BY LIMIT queries;
- server/embedded long-running workloads where startup cost is amortized.

The README already shows several cases where RedlineDB beats SQLite by large margins in specific CLI categories, so the goal should be: **match SQLite on shell microcases, beat it on modern engine workloads, and prevent the shell from polluting engine benchmark signals.**

## 5. Next rebuild stages to reach the theoretical limit

### Stage A — benchmark hygiene and hard gates

1. Split benchmark reports into:
   - process startup cost;
   - shell parsing cost;
   - database open/connect cost;
   - prepare/execute/fetch cost;
   - renderer/file I/O cost.
2. Track allocator baseline separately from query RSS.
3. Run every parity case under:
   - default allocator;
   - `fast-allocator`;
   - `jemalloc` if added later;
   - system allocator with `MALLOC_ARENA_MAX=1` on Linux.
4. Add a CI redline: no new case may allocate >2x SQLite RSS unless it touches enough rows/pages to justify it.

### Stage B — shell and FFI hot path

1. Keep database open lazy for scripts until a command actually needs storage.
2. Replace Clap on hot SQLite-compatible hot path with a tiny pre-parser for common flags, falling back to Clap only for uncommon options.
3. Intern common separators and null strings.
4. Use `SmallVec<[SqlValue; 8]>` for rows under 8 columns.
5. Make scalar fromless SELECT a first-class shell expression lane, not a sidecar special case.
6. Stream all list/csv/tabs output. Do not materialize rows for display unless layout modes require width measurement.

### Stage C — query executor core

1. Make row values borrow wherever possible: `ValueRef` through projection, render, and comparison.
2. Replace debug-string window layout cache keys with structural keys.
3. Add inverse aggregate window accumulators for sliding ROWS frames:
   - `sum`, `count`, `avg`, `total` O(1) per row;
   - `min/max` monotonic deque per partition;
   - `first_value/last_value/nth_value` direct frame-bound indexing without allocating frame-position vectors.
4. Convert `partition_rows` to encode partition keys directly into a scratch buffer; avoid `Vec<Vec<SqlValue>>` key storage for partitioning.
5. Add projected-row arenas for short-lived SELECT output to avoid per-cell heap churn.

### Stage D — index/order/limit dominance

The 3x `DML_WHERE_ORDER_LIMIT` and `INDEXED_BY` cases should be handled by making the planner/executor aggressively prefer:

1. covering ordered index scans for `WHERE + ORDER BY + LIMIT`;
2. LIMIT pushdown into index cursors;
3. heap fetch only after row survives WHERE/ORDER/LIMIT;
4. exact `INDEXED BY` enforcement with a no-fallback path;
5. reverse index cursor support for DESC without post-sort;
6. multi-column key prefix range scans.

Target: bring the 3x cases to 1.0–1.4x SQLite first, then optimize hot comparators to beat SQLite in integer/text key scans.

### Stage E — storage/kernel HPC

1. Page cache: sharded, lock-light, fixed-size slabs, hot-page pinning.
2. WAL: group commit with batched fsync and checksum SIMD.
3. B-tree: prefix-compressed keys, branchless binary search inside page, SIMD memcmp for fixed-width/integer keys.
4. Serialization: zero-copy row decode for projected columns; lazy blob/text decode.
5. Temp/spill: preallocated spill files, O_DIRECT optional, mmap option for large read-only sorts.
6. CPU builds: keep `target-cpu=native`, fat LTO, PGO, and add BOLT/autofdo for official “native max” artifacts.

## 6. Build matrix

Recommended build modes:

```bash
# Parity/memory suite: small RSS, realistic SQLite shell comparison
cargo build -p redlinedb-cli --release

# Long-running throughput / server-style profile
cargo build -p redlinedb-cli --release --features fast-allocator

# Native PGO profile after training on parity + TPC-H-ish + write-heavy corpus
./scripts/perf/pgo.sh
cargo build -p redlinedb-cli --profile release-pgo --features fast-allocator
```

## 7. Validation plan

1. Run the five CLI tempfile cases individually before/after.
2. Run the top 10 latency cases before/after.
3. Run the top 20 RSS cases under `/usr/bin/time -v` and the existing memory harness.
4. Confirm byte-for-byte parity for stdout/stderr/exit code.
5. Add specific unit tests for:
   - `.output` buffered file flush/restore;
   - `.once` exactly one statement;
   - `.read` recursive script fast lane fallback;
   - `SELECT hex(readfile(...))` streaming output;
   - default allocator build and `fast-allocator` build.

## 8. Risk notes

- The fast CLI lane must remain conservative. Any unsupported dot command, non-fromless query, multi-line SQL, parameter binding, or database-dependent function should fall back to the full engine.
- Optional allocator changes can shift long-running throughput. That is why `fast-allocator` remains available.
- The patch is generated from repository inspection through GitHub because local checkout failed in the execution environment. It should be applied and compiled before merging.
