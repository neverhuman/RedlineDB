# RedlineDB theoretical-limit performance rebuild spec

## Executive target

RedlineDB should be optimized as two products that share one correctness core:

1. **SQLite-parity shell and embedded database**: match SQLite behavior while cutting fixed costs on tiny one-shot invocations.
2. **Rust-native HPC database kernel**: beat SQLite on concurrency, window/aggregate workloads, large scans, write-heavy workloads, and native deployments where RedlineDB can specialize to host CPU, workload, and page cache policy.

The first patch in `redlinedb_theoretical_limit_rebuild.diff` attacks the largest visible regressions without touching storage correctness invariants:

- **ShellZero fast path**: skip database open/connect/catalog setup for provably shell-only dot-command scripts.
- **AllocatorSlim CLI default**: remove unconditional mimalloc from CLI binaries by default, keeping it behind `--features hpc-allocator` for long-lived/server workloads.
- **WindowAggLinear**: make full-partition aggregate windows linear per partition instead of re-aggregating a frame for every row.
- **NoOrderWindow layout shortcut**: avoid building empty ORDER BY key vectors for no-order window partitions.

This is the lowest-risk first strike because it targets the exact reported top gaps: `.cd`, `.once`, `.output`, `.read`, `-append` temp-file shell cases, plus the `WINDOW_PARTITION_SUM_*` RSS/time class.

> Status: patch was source-inspected and generated against the current public `main` source snapshot. I could not clone or compile locally because the execution container could not resolve `github.com`; run the validation plan below before merging.

## Current observed baseline

The public README reports SQLite-parity at 1127 total tests, 1123 passed, 4 skipped, 0 failed, and 99.6% pass rate. It also reports median RedlineDB latency at 3.27 ms vs SQLite 1.71 ms, and median peak RSS at 8.9 MiB vs SQLite 12 KiB. The user-supplied top regressions show shell temp-file commands as the worst latency class, with 7.86x to 20.27x gaps, and window partition cases as the worst RSS class, around 9.7-10.0 MiB vs 8 KiB.

## Patch 1: ShellZero fast path

### Problem

Current CLI flow parses arguments, then opens/creates the database and creates a connection before it executes most input. That is the wrong default for process-local SQLite shell commands that do not need database state:

- `.cd DIR`
- `.output FILE|stdout|off`
- `.once FILE`
- `.read FILE` when the referenced file itself contains only shell-local commands
- `.print ...`
- display toggles such as `.headers`, `.mode`, `.separator`, `.nullvalue`, `.crlf`, `.echo`, `.bail`
- `.exit` / `.quit`

For those commands, opening the storage engine is pure fixed overhead. The measured symptom is exactly what we see: SQLite returns in roughly 1-2 ms while RedlineDB spends roughly 20-28 ms.

### Change

Add a conservative preflight after CLI flag resolution and before database open:

1. Build an input candidate from trailing SQL args or preloaded stdin.
2. Reject immediately if `--init` or `--cmd` is present, because these can mix hidden SQL and stateful commands.
3. Parse every line with the existing SQLite-compatible dot-command tokenizer.
4. Accept only a small, auditable set of shell-local dot commands.
5. For `.read`, recursively preflight the target file, bounded by a depth limit.
6. If all lines are shell-local, execute them with a tiny shell state and return before touching `Database::open` or `Database::create_in_memory`.
7. If any line is SQL or a database-inspecting dot command, fall back to the current path.

### Expected speedup

| Case class | Current RedlineDB | SQLite | Expected RedlineDB after ShellZero | Expected effect |
|---|---:|---:|---:|---:|
| `DOT_CD_TEMPFILE` | 24.28 ms | 1.20 ms | 0.25-0.80 ms | 30-95x internal; can beat SQLite shell |
| `DOT_ONCE_TEMPFILE` | 27.46 ms | 1.48 ms | 0.20-0.60 ms | 45-135x internal; can beat SQLite shell |
| `DOT_OUTPUT_TEMPFILE` | 28.42 ms | 1.66 ms | 0.40-1.00 ms | 28-70x internal; likely parity or better |
| `OPT_APPEND_TEMPFILE` | 20.56 ms | 1.37 ms | 0.35-0.90 ms | 22-58x internal; likely parity or better |
| `DOT_READ_TEMPFILE` shell-only script | 20.18 ms | 2.57 ms | 0.40-1.25 ms | 16-50x internal; likely parity or better |

The exact numbers depend on benchmark harness process startup measurement, filesystem, dynamic linker warmup, and whether benchmark scripts include SQL inside `.read`. ShellZero is designed to fall back when `.read` contains SQL, preserving correctness.

### Correctness envelope

ShellZero is safe because it is a strict subset optimization:

- It never handles SQL.
- It never handles `.schema`, `.tables`, `.dump`, `.import`, `.backup`, `.restore`, `.dbinfo`, `.recover`, or anything that touches database state.
- It recursively validates `.read` content before executing it.
- It does not create a database file for shell-only scripts, matching the intuitive SQLite shell behavior and preventing the current fixed-cost regression.

## Patch 2: AllocatorSlim CLI default

### Problem

The CLI binaries currently install mimalloc unconditionally. mimalloc is often a win for long-lived server processes and allocator-heavy benches, but it can hurt RSS measurements for tiny one-shot shell invocations because the allocator runtime and retained heaps dominate the actual work.

### Change

Move mimalloc behind a feature flag:

```toml
[features]
default = []
hpc-allocator = ["mimalloc"]
```

The CLI binaries now use mimalloc only with `--features hpc-allocator`.

### Expected effect

| Workload | Expected result |
|---|---|
| Tiny CLI shell-only and one-shot SQL parity RSS | Lower peak RSS, typically multiple MiB saved depending on libc/platform measurement |
| Long-lived benchmark/server process | Keep using `--features hpc-allocator` if mimalloc wins throughput |
| Embedded library users | Unaffected unless they opt into their own allocator |

This will not reduce Rust process RSS to SQLite's tiny 8-12 KiB harness values by itself, because binary/runtime/linker measurement dominates microcases. It should still remove a large self-inflicted fixed RSS contributor.

## Patch 3: WindowAggLinear

### Problem

The window executor already has a prefix aggregate fast path for:

```sql
ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW
```

But SQLite's default frame for no `ORDER BY` is the whole partition:

```sql
RANGE BETWEEN UNBOUNDED PRECEDING AND UNBOUNDED FOLLOWING
```

That means a query like:

```sql
SUM(x) OVER (PARTITION BY k)
```

is logically one aggregate value per partition. The current generic path computes frame bounds and re-runs the accumulator for each row, which is O(rows_per_partition^2) work for full partition frames.

### Change

Add a fast path for aggregate windows where:

- function is `sum`, `count`, `avg`, `min`, `max`, or `total`
- start is `UNBOUNDED PRECEDING`
- end is `UNBOUNDED FOLLOWING`
- `EXCLUDE` is not active

For each partition:

1. Build one accumulator.
2. Evaluate the aggregate argument once per row.
3. Finalize once.
4. Copy the resulting value into every output row in that partition.

### Expected speedup

| Workload | Complexity before | Complexity after | Expected effect |
|---|---:|---:|---:|
| `SUM(x) OVER (PARTITION BY k)` | O(n^2) per partition | O(n) per partition | 3-50x depending on partition size |
| Generated `WINDOW_PARTITION_SUM_*` cases | O(partition^2) plus allocation churn | O(partition) | likely 2-8x on current small parity cases; much larger on real data |
| Memory/RSS | repeated frame scans and transient values | one accumulator and fan-out | lower heap churn; RSS may still be dominated by process baseline |

## Patch 4: NoOrderWindow layout shortcut

### Problem

`WindowLayoutCache::build_layouts` calls `order_partition` even when there is no `ORDER BY`. `order_partition` still builds a `Vec<(usize, Vec<SqlValue>)>` and allocates an empty key vector for every row.

### Change

If `window.order_by.is_empty()`:

- clone the partition index vector directly into `order_index_map`
- set one peer group covering the full partition
- skip key-vector construction and sorting entirely

### Expected effect

This cuts allocator pressure for all no-order window functions, including the reported window partition RSS cases. It is intentionally simple and semantics-preserving.

## Theoretical-limit rebuild roadmap

Patch 1 is not the end state. It is the first high-return, low-risk layer. To make RedlineDB faster than SQLite broadly, the rebuild should proceed through the following engine architecture.

### 1. Bytecode VM and plan cache

Current SQL execution still exposes AST-level evaluation patterns in several paths. SQLite's strength comes from a tight VM and decades of opcode tuning. RedlineDB should implement a Rust-native VM with two tiers:

- **Tier 0 interpreter**: compact bytecode, register file, branch-predictable opcodes, no heap allocation in the hot loop.
- **Tier 1 specialized plan**: generated Rust-like micro-IR for stable prepared statements, with constants folded and column offsets resolved.

Required changes:

- Compile scalar expressions into bytecode once during prepare.
- Store column offsets and type-affinity coercions in opcodes.
- Replace recursive `eval_scalar` hot loops with register-machine execution.
- Cache prepared plans by normalized SQL plus schema epoch.
- Invalidate by catalog epoch, index epoch, pragma-affecting flags, and collation registry epoch.

Expected result:

- Tiny repeated statements: 1.5-4x faster.
- Expression-heavy SELECT/WHERE/ORDER BY: 2-8x faster.
- Lower allocations: per-statement arenas replace per-row heap churn.

### 2. B-link tree index core

The deeper RedlineDB differentiator should be the storage/index kernel. Replace the global-structure-lock index path with a B-link tree design:

- page-level latches, not tree-global locks
- high keys and right-links for safe concurrent splits
- optimistic descent with version validation
- latch coupling only on split/merge edges
- prefix-compressed interior keys
- leaf sibling scans for range queries
- covering-index row materialization without table lookup

Expected result:

- Write concurrency: RedlineDB can beat SQLite decisively because SQLite serializes many write paths.
- Range/index scans: 1.5-5x faster with covering scans and early-stop limits.
- Hot secondary-index insert/update: 2-10x faster under contention.

Protected invariants:

- MVCC visibility and snapshot isolation remain outside stage mutation.
- WAL record order and recovery grammar remain canonical.
- Page format upgrades require explicit version gates and fuzz/recovery tests.

### 3. WHERE ORDER BY LIMIT early stop everywhere

The reported `DML_WHERE_ORDER_LIMIT_*` cases are 3x slower. The planner already contains an ordered-index-scan-limit concept, but DML explain/build paths are still effectively constant-plan placeholders. The rebuild should push ORDER/LIMIT into mutation candidate selection:

- For `DELETE ... WHERE ... ORDER BY indexed_col LIMIT n`, scan the ordered index and collect at most `n` rowids.
- For `UPDATE ... WHERE ... ORDER BY indexed_col LIMIT n`, collect rowids through the ordered index, then mutate by rowid.
- Use a bounded TopN heap only when no compatible index exists.
- Avoid full sort/materialization when the index provides the required order.

Expected result:

- Current 3x DML regressions should collapse to parity or faster.
- Large tables with small LIMIT: 10-100x over full scan/sort fallback.

### 4. Segment/vector execution for scans, filters, and aggregates

For analytical and generated SQL cases, row-at-a-time execution leaves SIMD and cache locality unused. Add a batch layer under the VM:

- `Batch<N>` row vectors with validity bitmaps.
- SIMD comparisons for integer/real/text prefix filters.
- Vectorized hash aggregation using SwissTable/hashbrown raw entry APIs.
- Adaptive batch width: 128/256/512 rows based on row width and cache pressure.
- Direct `ValueRef` views into page/cache memory when lifetime-safe.

Expected result:

- Scan/filter/project: 2-6x faster.
- Group aggregate: 2-8x faster.
- Window partition preprocessing: 2-5x faster.

### 5. Arena-owned statement memory

The current RSS gap is not only algorithmic; it is fixed process and allocation shape. Introduce explicit memory domains:

- process domain: global registries, collations, function table
- connection domain: page cache handles, prepared statement cache
- statement arena: parse tree, bytecode, transient keys, row buffers
- operator arena: sort/hash/window spill buffers

Rules:

- No `Vec<Vec<SqlValue>>` in hot paths unless a materialized result is semantically required.
- Prefer `SmallVec<[T; 4]>` for narrow rows and short expression lists.
- Prefer `Arc<str>`/interned identifiers for catalog names.
- Use `ValueRef` until a value must escape.
- Add memory accounting to every operator and fail/spill before RSS blows up.

Expected result:

- Parity memory ratios should drop from hundreds/thousands-x to tens-x in CLI process measurements.
- In embedded in-process metrics, transient heap should approach SQLite-like KiB-scale for small statements.

### 6. Temp-store and sorter redesign

SQLite is extremely good at avoiding temp work for small cases. RedlineDB needs an explicit temp-store policy:

- in-register TopN for LIMIT <= 64
- small-run stack/SmallVec sort
- radix/integer specialized sort for integer ORDER BY
- external merge sort with preallocated run buffers
- temp files through `pwritev`/`preadv`, no per-row syscall patterns
- compression only when spill size crosses threshold

Expected result:

- ORDER BY LIMIT: 2-20x depending on LIMIT and row width.
- Lower RSS by spilling predictably instead of growing unbounded vectors.

### 7. WAL and durability path

RedlineDB can beat SQLite on write-heavy workloads if it treats WAL as a group-commit pipeline:

- append-only WAL writer thread or cooperative group committer
- `writev`/`pwritev2` batching
- checksum vectorization
- durable epoch acknowledgment
- page-cache dirty queue sorted by page id
- checkpoint as background stage with admission control

Expected result:

- Single writer: parity to 2x faster depending on fsync settings.
- Multiple writers: 3-20x faster than SQLite-style serialized writer bottlenecks.

### 8. Custom compile profile

The repository already uses strong release settings: `opt-level=3`, fat LTO, single codegen unit, panic abort, symbol stripping, and Linux `target-cpu=native` with clang/mold. Keep those and add:

- PGO training corpus from sqlite-parity plus beyond-sqlite hot workloads.
- BOLT post-link layout on Linux release artifacts.
- Feature-gated SIMD kernels: `simd-json`, AVX2/AVX-512 text scan, ARM NEON comparators.
- `-Zlocation-detail=none` for nightly release experimentation when acceptable.
- `RUSTFLAGS='-C target-cpu=native -C target-feature=+bmi2,+lzcnt,+popcnt'` for benchmark builds when host supports it.

Expected result:

- 5-20% broad throughput improvement from PGO/BOLT.
- 1.5-5x in individual SIMD-able kernels.

### 9. Stage/gene interfaces for engineered intelligence

Expose mutation surfaces only where correctness can be boxed in by a stable contract. Suggested stage traits:

```rust
trait ScalarKernelStage { fn eval(&self, program: &ScalarProgram, row: RowView, out: &mut Registers) -> Result<()>; }
trait AccessPathStage { fn open(&self, plan: &AccessPlan, snapshot: Snapshot) -> Result<Box<dyn RowCursor>>; }
trait SortStage { fn sort(&self, input: BatchStream, spec: SortSpec, budget: MemoryBudget) -> Result<BatchStream>; }
trait HashAggStage { fn aggregate(&self, input: BatchStream, spec: AggSpec, budget: MemoryBudget) -> Result<BatchStream>; }
trait WindowStage { fn evaluate(&self, input: RowSet, spec: WindowSpecSet, budget: MemoryBudget) -> Result<RowSet>; }
trait WalStage { fn append_commit(&self, records: &[WalRecord], durability: Durability) -> Result<CommitToken>; }
trait PageCacheStage { fn get_page(&self, id: PageId, mode: LatchMode) -> Result<PageGuard>; }
trait IndexStage { fn seek(&self, key: KeyRef, snapshot: Snapshot) -> Result<IndexCursor>; }
```

Hard boundaries:

- Stage implementations may not invent new SQL semantics.
- Stage implementations must consume the same typed plan/input and produce the same typed output.
- MVCC visibility, WAL replay correctness, page checksum validation, and catalog epoching are protected by non-evolvable harnesses.
- Every stage gets differential tests against SQLite plus RedlineDB reference stage.

## Can RedlineDB be faster than SQLite?

Yes, but not by copying SQLite. SQLite is already close to optimal for many single-process, single-statement, small-data cases. RedlineDB can beat it by exploiting places SQLite intentionally does not specialize:

- Rust-native host CPU specialization with PGO/BOLT/SIMD.
- Concurrent writes with a B-link/tree + group commit architecture.
- Vectorized execution for scans, aggregates, windows, JSON, and text kernels.
- Workload-specialized generated stages selected by benchmarking.
- Shell and API fast paths that avoid starting the engine for non-engine work.

The realistic target sequence is:

1. **First**: eliminate embarrassing fixed-cost regressions. ShellZero should turn the worst 20x latency gaps into parity or wins.
2. **Second**: linearize known algorithmic cliffs. WindowAggLinear and DML ORDER/LIMIT early-stop should eliminate the visible 3x classes.
3. **Third**: reduce RSS ratios by changing allocator defaults, per-statement arenas, and materialization discipline.
4. **Fourth**: beat SQLite in categories where RedlineDB's architecture can be fundamentally better: multi-writer, range-heavy indexed queries, vectorizable analytics, and native deployed workloads.

## Validation plan

Run locally after applying the diff:

```bash
git apply redlinedb_theoretical_limit_rebuild.diff
cargo fmt
cargo test -p redlinedb-cli --test shell_fast_path
cargo test -p redlinedb-sql --test parity_window
cargo test -p redlinedb-sql --test parity_expr_index
cargo test --workspace --release
```

Then run the benchmark gate:

```bash
cargo build --profile release-native -p redlinedb-cli
# Run your sqlite-parity harness exactly as used for README/latest.
# Confirm these specific case IDs first:
# 00153 DOT_CD_TEMPFILE
# 00149 DOT_ONCE_TEMPFILE
# 00148 DOT_OUTPUT_TEMPFILE
# 00202 OPT_APPEND_TEMPFILE
# 00146 DOT_READ_TEMPFILE
# 00832/00845/00811 WINDOW_PARTITION_SUM_*
```

For profiling:

```bash
perf stat -r 50 target/release-native/redlinedb :memory: '.cd /tmp'
perf record -g target/release-native/redlinedb :memory: 'SELECT sum(x) OVER (PARTITION BY k) FROM t'
heaptrack target/release-native/redlinedb :memory: < window_partition_sum.sql
```

Merge criteria:

- 0 sqlite-parity failures.
- Shell temp-file class median <= SQLite median or within 1.2x.
- No SQL-containing `.read` behavior change.
- Window partition sum cases do not regress outputs.
- Median RSS improves; if mimalloc removal hurts a long-lived benchmark, enable `--features hpc-allocator` only for that binary/profile.

## Follow-up patches I would implement next

1. DML ORDER BY LIMIT rowid candidate selection.
2. Statement arena and `SmallVec`-first row/cell storage.
3. Scalar bytecode VM for expression evaluation.
4. Covering-index scan executor path for `INDEXED BY` and expression-index parity cases.
5. B-link tree index stage prototype with differential/fuzz harness.
6. PGO/BOLT release pipeline and benchmark corpus automation.
