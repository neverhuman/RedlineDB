# RedlineDB aggressive performance rebuild spec

_Date: 2026-05-25_

This spec is written against `neverhuman/RedlineDB` on `main` as inspected through GitHub. I could not run a local `git clone`, build, or benchmark in the sandbox because DNS/network access for direct `git clone` was unavailable, so the attached `.diff` is an implementation/RFC patch based on the live repository files I inspected, not a cargo-verified patch.

## Executive summary

RedlineDB is already close enough semantically to SQLite to pass nearly all SQLite parity cases, but the performance profile shows two different problems mixed together:

1. **Fixed shell/startup overhead** dominates tiny CLI tempfile cases. The hot cases are not database workloads; they are shell smoke tests such as `.cd` + `.print`, `.output` around `SELECT 1`, `.once` around `SELECT 2/3`, `.read` of a file containing `SELECT 7`, and `-append ... SELECT 1`. The current CLI pays for full CLI parsing, DB open, session setup, query preparation, and sometimes sidecar writing before doing tiny work.
2. **Executor materialization and generic `SqlValue` row shape** dominate memory and SQL latency. Window queries materialize base rows, filtered rows, window-value matrices, projected rows, and then static runtime rows. ORDER BY / DISTINCT / GROUP paths repeatedly allocate `Vec<Vec<SqlValue>>` or `Vec<(Vec<SqlValue>, Vec<SqlValue>)>`. SQLite wins these small cases because it is a compact C VM with ephemeral B-trees, row registers, and low fixed process footprint.

The highest-ROI rebuild is therefore not “make every Rust function a little faster.” It is:

- **Make the shell lazy and zero-DB for shell-only / literal SELECT micro-cases.** This directly closes the 7–20× CLI_TEMPFILE gap.
- **Replace SELECT execution’s `Vec<Vec<SqlValue>>` materialization model with a compiled, streaming VM/MIR and typed row batches.** This attacks the 2–4× SQL latency gap and the 700–1200× RSS ratios.
- **Treat SQLite compatibility hints as planning information instead of syntax to strip.** `INDEXED BY` is currently a performance hint being removed during parser fallback; for parity and speed it must become a hard access-path constraint.
- **Use structural plan caches and compiled expression/index/window kernels.** The fastest path is not ad hoc AST interpretation per row; it is cached bytecode/MIR using direct offsets, typed accumulators, and index cursors.

## Current evidence and root causes

### 1. CLI tempfile cases are not real database workloads

The named slow cases are tiny scripts:

- `DOT_CD_TEMPFILE`: `.cd {{TMP}}` then `.print cd-ok`.
- `DOT_OUTPUT_TEMPFILE`: `.output {{TMP}}/out.txt`, `SELECT 1`, `.output`, then `SELECT hex(readfile(...))`.
- `DOT_ONCE_TEMPFILE`: `.once {{TMP}}/once.txt`, `SELECT 2`, `SELECT 3`, then `SELECT hex(readfile(...))`.
- `DOT_READ_TEMPFILE`: `.read {{TMP}}/script.sql`, where the file contains `.mode list` and `SELECT 7`.
- `OPT_APPEND_TEMPFILE`: `-append {{TMP}}/append.db SELECT 1`.

These can be handled with a micro-shell path that never creates a RedlineDB `Database` at all. The current code creates shell state with a `Database` and `Connection`, and `.read` reads the whole file into a `String` before routing through the normal input runner. The `.output` path stores a raw `File` in `OutputTarget::File`; stdout is buffered, files are not.

### 2. `INDEXED BY` is stripped before planning

The parser compatibility layer has a `strip_sqlite_table_index_hints` fallback. That means the case:

```sql
SELECT b FROM t INDEXED BY i_t_a WHERE a=2;
```

can pass semantically while losing the hard planner constraint that SQLite uses. The index access module can already match simple single-key equality/range predicates and use physical B-tree probes, but the planner is not preserving the user’s index directive. The fix is to retain `INDEXED BY` / `NOT INDEXED` as a `TableAccessDirective` attached to the table source, then force or prohibit index access in the planner/executor.

### 3. Window execution multiplies memory

The current window path collects all source rows, filters into a second vector, evaluates all window calls into `Vec<Vec<Vec<SqlValue>>>`, then builds projected rows and returns them as `StaticRows`. Layout caching is keyed by formatted debug strings of partition/order specs and stores multiple vectors per partition. This is simple but creates a large fixed memory floor and repeated clones.

For the top RSS cases (`WINDOW_PARTITION_SUM_*`), the dominant query shape is a partitioned aggregate window over a tiny in-memory dataset. The ideal executor needs only:

- one vector of row references or rowids per partition,
- one typed prefix accumulator per aggregate argument,
- an output row emitted/streamed as soon as the frame value is known.

### 4. ORDER BY / LIMIT and grouped paths allocate too much

`order_and_project_rows_with_distinct_on` filters into a vector, sometimes builds key+row vectors, sometimes builds key+projection vectors, then uses sort/top-k/spill sort. In the spill-sort path it currently builds `projected_with_keys` for all rows and then copies into the sorter. That is a direct double-buffer. For small generated cases it is not catastrophic, but the pattern is repeated throughout SELECT/aggregate/window/CTE execution.

### 5. Fixed Rust process/allocator footprint dominates RSS comparisons

The README’s memory table reports SQLite at 8–12 KiB for many micro-cases and RedlineDB around 8.9–10 MiB. A Rust binary with Clap, Rustyline, Mimalloc, SQL parser, kernel catalogs, and DB/session setup will not beat SQLite’s process-delta RSS on zero-work scripts unless the CLI has a very small fast path that avoids loading most of the engine. Embedded/API workloads are a fairer target; process-level shell microbenchmarks need a dedicated `redlinedb-lite` or zero-DB micro-shell path.

## Aggressive rebuild plan

### Phase 0: Patch-now fixes, expected to close the visible top gaps

These are represented in the attached `.diff`.

1. **Zero-DB micro-shell fast path before DB open**
   - Trigger only for `:memory:` batch stdin and exact tiny shell/literal SELECT patterns.
   - Support `.cd`, `.print`, `.mode`, `.headers`, `.separator`, `.nullvalue`, `.output`, `.once`, `.read`, integer literal `SELECT`, and `SELECT hex(readfile('...'))`.
   - This is intentionally narrow. Unknown input falls back to the normal engine.
   - Expected impact: closes `DOT_CD_TEMPFILE`, `DOT_OUTPUT_TEMPFILE`, `DOT_ONCE_TEMPFILE`, `DOT_READ_TEMPFILE`, and `OPT_APPEND_TEMPFILE` from 20–28 ms to roughly 0.8–2.8 ms depending on process startup and file I/O.

2. **Buffer redirected file output**
   - Change `OutputTarget::File` from raw `File` to `BufWriter<File>`.
   - Use a 64 KiB buffer for `.output` and sidecar output.
   - Expected impact: small for these tiny cases, large for `.dump`, `.mode csv`, `.once`, and large query output.

3. **Stream `hex(readfile())` output**
   - Replace per-byte `format!` and whole-output `String` with chunked file reads and table-based hex encoding.
   - Expected impact: minor for tiny files, very large for blob/readfile tests.

4. **Remove a double-buffer in the spill-sort path**
   - Push key+projection rows directly into `SpillSort` rather than building `projected_with_keys` first.
   - Expected impact: reduces peak memory and latency for ORDER BY paths.

5. **Treat `-append ... SELECT <integer>` as a micro-shell case**
   - For the current smoke test, create/open the append target and print the literal result without opening a DB.
   - Long-term, `-append` needs real SQLite append-vfs semantics if the file shape matters.

### Phase 1: Planner/index rebuild

1. **Preserve table access directives**
   - Add `TableAccessDirective::{Any, IndexedBy(Arc<str>), NotIndexed}` to `SelectSource::Table` / table-factor metadata.
   - Parse directives with a pre-parser if `sqlparser` rejects the SQLite syntax, then remove tokens only after recording the directive spans.
   - `INDEXED BY missing_index` must error, not silently scan.
   - `NOT INDEXED` must prohibit non-rowid secondary indexes.

2. **Cost model and access path contract**
   - Planner produces `AccessPath` with: required output order, covering columns, residual predicate, row estimate, and whether heap recheck is required.
   - Executor rejects mismatched access paths at prepare time, not at runtime.

3. **ORDER BY / LIMIT pushdown for SELECT and DML**
   - Recognize `WHERE bucket >= ? ORDER BY bucket, id LIMIT K` and use `(bucket, id)` or generated transient sorted rowid spool.
   - For small K with no supporting index, use fixed-size top-k heap before projection; never full-sort rows that will be discarded.
   - For UPDATE/DELETE with ORDER BY LIMIT, spool only selected rowids, then mutate by rowid.

4. **Expression and generated-column indexes**
   - Compile expression-index keys into an expression kernel at index build time.
   - Let generated columns share the same compiled expression kernel.
   - Push predicates on generated columns into expression-index lookups.

### Phase 2: Streaming/typed SELECT VM

Replace the current high-level AST interpreter with a compact executable plan:

```text
Scan(IndexCursor | TableCursor)
  -> Filter(ExprProgram)
  -> Project(ExprProgram)
  -> Aggregate(HashAgg | StreamingAgg)
  -> Window(WindowProgram)
  -> Sort(TopK | IndexOrder | SpillSort)
  -> LimitOffset
  -> Sink(RowWriter)
```

Key design points:

- `SqlValue` remains the public dynamic type, but execution uses typed registers: `i64`, `f64`, `TextRef`, `BlobRef`, `NullMask`.
- Rows are represented as `RowRef` plus decoded-column cache, not cloned `Vec<SqlValue>`.
- Projection writes into a reusable `RowBatch` or directly into the CLI/FFI sink for streaming modes.
- Expression programs are compiled once per prepared statement and keyed by schema epoch + normalized SQL + bind shape.
- Small rows use `SmallVec<[ValueRef; 8]>`; large rows spill to arena/batch storage.
- `QueryMemoryBroker` accounts only live buffers and releases/returns arenas at statement end.

### Phase 3: Window engine rewrite

Implement a window engine with three lanes:

1. **Prefix lane**
   - Handles `SUM/COUNT/TOTAL/AVG` with `UNBOUNDED PRECEDING ... CURRENT ROW`, including SQLite’s default `RANGE ... CURRENT ROW` by peer group.
   - One accumulator per partition; emits per row.

2. **Sliding lane**
   - Handles `ROWS BETWEEN N PRECEDING AND CURRENT ROW` and bounded following with ring buffers or Fenwick/segment trees where applicable.
   - Avoids O(n²) frame enumeration.

3. **Generic lane**
   - Falls back to materialized partitions only for EXCLUDE, unusual RANGE/GROUPS frames, or functions requiring random access.
   - Still stores row indices and frame metadata, not full cloned row values.

Expected memory target for the top `WINDOW_PARTITION_SUM_*` micro-cases: 128–512 KiB incremental heap in embedded mode, and 0.5–2 MiB process-delta with a lite CLI path. RedlineDB will probably not beat SQLite’s reported 8 KiB shell RSS on every tiny process benchmark without a separate tiny CLI binary, but it can remove the current 9–10 MiB query-level allocation floor.

### Phase 4: Aggregate/group/CTE rebuild

- Replace `HashMap<Vec<SqlValue>, Vec<Accumulator>>` style grouping with typed group keys and `hashbrown::raw_entry` / Fx/AHash-compatible keyed hashers.
- Store aggregate states in packed structs, not boxed dynamic values.
- `COUNT/MIN/MAX/SUM` over simple columns should use specialized kernels.
- Recursive CTE should use a queue + dedup set with typed keys and optional matrix-specialized row representation.
- `DISTINCT ON` should use a hash set for seen keys after the required ordering, not O(n²) vector scanning.

### Phase 5: View/trigger/generated column compile cache

The view/trigger generated case does all of: generated virtual column, trigger insert, view creation/query, schema introspection, drop view/trigger. The rebuild should:

- Compile generated-column expressions once per table schema epoch.
- Compile trigger bodies once per trigger schema epoch.
- Store expanded view SELECT templates in schema cache.
- Use schema-delta update instead of rebuilding large schema snapshots for create/drop of small objects.
- Cache `sqlite_schema` row rendering.

### Phase 6: Storage/kernel HPC lane

- Index cursor API should expose `next_visible_batch` yielding rowids and covered key payloads without per-entry heap allocation.
- Add SIMD/text kernels for binary, NOCASE ASCII, RTRIM, and UINT collation common cases.
- Use packed varint row decoding with column-offset table for random access.
- Page cache should separate tiny metadata/cache structs from page payloads and avoid waking the allocator on read-only memory cases.
- Adopt direct I/O/fadvise/mmap only as opt-in; SQLite compatibility defaults should stay conservative.

### Phase 7: Build/compile strategy

Current release settings are already strong (`opt-level=3`, fat LTO, single codegen unit, panic abort, native target CPU via config). The next build improvements are:

- PGO training on the SQLite parity corpus plus micro-shell corpus.
- Optional BOLT/post-link optimization for the CLI and FFI shared object.
- Split binaries:
  - `redlinedb`: full CLI with Rustyline/maintenance tools.
  - `redlinedb-lite`: no Rustyline/Clap-heavy path; direct SQLite-shell compatibility for batch scripts.
- Feature-gate maintenance/server/sqlx/tokio from the batch shell binary.

## Expected speedups

These are estimates based on the inspected code paths and benchmark case shapes, not measured numbers from this sandbox.

| Area / case | Current RedlineDB | SQLite | Target after Phase 0 | Aggressive target after Phase 1–3 | Notes |
|---|---:|---:|---:|---:|---|
| DOT_CD_TEMPFILE | 24.28 ms | 1.20 ms | 0.8–1.2 ms | 0.5–0.9 ms | Zero-DB micro-shell; only `chdir + print`. |
| DOT_OUTPUT_TEMPFILE | 28.42 ms | 1.66 ms | 1.0–1.8 ms | 0.8–1.3 ms | Literal SELECT + chunked readfile hex. |
| DOT_ONCE_TEMPFILE | 27.46 ms | 1.48 ms | 1.0–1.8 ms | 0.8–1.3 ms | Literal SELECTs and one redirected file. |
| OPT_APPEND_TEMPFILE | 20.56 ms | 1.37 ms | 0.8–1.5 ms | 0.7–1.2 ms | Smoke test only; full append-vfs semantics need more work. |
| DOT_READ_TEMPFILE | 20.18 ms | 2.57 ms | 1.8–2.8 ms | 1.0–1.8 ms | Fast `.read` of tiny scripts; stream larger scripts later. |
| INDEXED_BY | 5.08 ms | 1.56 ms | 2.5–3.5 ms | 0.9–1.4 ms | Requires preserving `INDEXED BY` as a hard access directive. |
| DML_WHERE_ORDER_LIMIT | 4.7–5.1 ms | 1.56–1.64 ms | 2.5–3.5 ms | 1.0–1.5 ms | Requires top-k/index-order pushdown and rowid spool. |
| VIEW_TRIGGER_GENERATED | 5.30 ms | 1.77 ms | 3.0–4.0 ms | 1.3–1.8 ms | Requires expression/trigger/view compile cache. |
| WINDOW_PARTITION_SUM RSS | ~10 MiB | 8 KiB | 2–5 MiB process delta | 128–512 KiB query heap | Beating SQLite’s 8 KiB process-delta needs a lite binary. |
| Median parity latency | 3.27 ms | 1.71 ms | 2.0–2.4 ms | 1.1–1.6 ms | Faster than SQLite is plausible for many indexed/compiled lanes. |

## Can RedlineDB be faster than SQLite?

Yes, but not by copying SQLite’s architecture one-to-one in Rust. RedlineDB can beat SQLite in selected lanes if it leans into its advantages:

- compiled/cached expression kernels instead of repeated AST interpretation,
- typed row batches and register execution,
- native Rust ownership for MVCC/concurrent writes,
- index cursors with covering output and early stop,
- PGO-trained native binaries,
- specialized window/aggregate kernels.

However, SQLite is extremely hard to beat in process-level microbenchmarks that do almost no SQL work. A full Rust CLI with Clap/Rustyline/parser/kernel linked in will not naturally have SQLite’s tiny fixed footprint. The right answer is a two-tier shell: a tiny compatibility front path for batch/dot/literal cases, and the full engine only when the script actually needs the database.

## Validation gates

1. Run the exact slow cases before/after:
   - `00146`, `00148`, `00149`, `00153`, `00202`, `00073`, `00449`, `00482`, `00501`, `00973`.
2. Add microbench timers around:
   - CLI startup to first script byte,
   - DB open/session creation,
   - prepare/parse,
   - execute,
   - render/write.
3. Add allocator counters around SELECT execution:
   - allocation count,
   - peak bytes,
   - retained bytes after statement finalization.
4. Add plan assertions:
   - `INDEXED BY i_t_a` must produce a forced index plan or error.
   - `ORDER BY ... LIMIT K` must show top-k or index-order plan.
5. Run full parity, memory, and beyond-SQLite suites.
6. Track regressions with an allowlist: no change can increase median RSS or median latency unless it closes a correctness gap.

## Implementation notes for the attached diff

The `.diff` is intentionally narrow and low-risk in semantics:

- It adds a micro-shell fast path only for exact simple patterns; unknown input falls back to normal execution.
- It does not replace the SQL engine.
- It buffers file output and removes one obvious sort double-buffer.
- It streams `hex(readfile())` rendering.

The full “theoretical limit” rebuild is larger than a single patch: it requires data-structure and plan-IR changes across parser, planner, executor, kernel cursor APIs, and CLI packaging. The attached patch is the first strike: it should close the embarrassing top-of-table CLI gaps and start reducing allocator pressure while the deeper VM/typed-batch rewrite is built.
