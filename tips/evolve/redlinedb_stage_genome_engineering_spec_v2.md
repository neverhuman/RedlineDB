# RedlineDB Stage Genome Engineering Spec

**Repository:** `neverhuman/RedlineDB`  
**Target:** first-epoch engineered-intelligence stage genome for performance evolution  
**Patch companion:** `redlinedb_stage_genome_patch_v2.diff`

---

## 1. Executive decision

The maximum number of first-epoch stages I can reasonably protect is **8**.

These are the largest surfaces that appear both important for performance and narrow enough to standardize without giving generated code ownership of database correctness:

1. **WAL scheduling / group commit**
2. **Buffer residency / dirty flush / prefetch policy**
3. **Heap append-lane and reusable-page placement**
4. **Undo traversal advisory policy**
5. **Index scan batching / sibling prefetch policy**
6. **B-tree leaf split policy**
7. **SQL physical planning policy**
8. **SQL query-memory / vectorization policy**

I would not expand the first epoch past these 8. The next attractive candidates, lock wait/backoff and commit barrier behavior, should stay behind a second-epoch gate because mistakes there create deadlocks, false `MaybeCommitted` behavior, or misleading durability semantics.

The guiding rule is:

> A stage may choose among correct strategies. A stage may not redefine correctness.

Therefore stages are not allowed to reinterpret page bytes, WAL bytes, checksums, MVCC visibility, transaction status transitions, lock ownership, recovery ordering, catalog schema rules, expression truth tables, or SQL parser semantics.

---

## 2. Repository observations

RedlineDB already has the right shape for this. The workspace separates `crates/kernel`, `crates/sql`, `crates/bench`, client/public crates, FFI, CLI, and server. The kernel crate declares modules for catalog, engine, format, heap, index, integrity, I/O, JSON, storage, telemetry, transactions, vectors, and WAL. The SQL crate separately owns connection/session, parser/planner, execution, statements, values, and UDFs.

Inside the kernel, `Engine` is the composition root. It aggregates transaction status, buffer pool, page-backed heap, catalog, row locks, WAL coordinator, checkpoint state, telemetry counters, and B-tree handles. That makes the engine the place to install a genome, but not the place to let generated stages mutate invariants directly.

The strongest signal is that RedlineDB already contains policy modules:

- `crates/kernel/src/wal/policy.rs`
- `crates/kernel/src/storage/policy.rs`
- `crates/kernel/src/engine/page_heap/policy.rs`
- `crates/kernel/src/index/policy.rs`
- `crates/sql/src/planner/policy.rs`

Those are already embryonic stage surfaces. The proposed patch standardizes them into a common ABI and adds audits/manifests so generated stages can be composed into genomes and rejected before benchmarking.

---

## 3. Stage selection criteria

A candidate surface becomes a stage only if it satisfies all of these:

1. **Performance relevance:** it plausibly affects latency, throughput, cache hit rate, write amplification, memory pressure, contention, scan cost, or plan quality.
2. **Small context:** it can receive a bounded immutable context and return a small decision.
3. **No invariant ownership:** it cannot corrupt page/WAL/MVCC/recovery/catalog semantics.
4. **Auditable output:** its output can be clamped or rejected by a generic validator.
5. **Composable:** every combination with other valid stages should build and remain semantically valid.
6. **Default equivalent:** the default stage reproduces current behavior closely enough to establish a baseline genome.

---

## 4. Proposed first-epoch stages

### 4.1 WAL scheduling / group commit

**Current surface:** `crates/kernel/src/wal/policy.rs` exposes write batch size, group commit delay, target resampling, and drain batch decisions.

**Standard input:** pending bytes, pending record count, flush gap, configured batch bytes, configured group delay, configured max group bytes.

**Standard output:** write batch bytes, group delay, resample boolean, drain batch bytes.

**Protected invariants:** stages do not reserve LSNs, serialize records, choose WAL record kinds, write file bytes, fsync directly, publish durable LSN, or alter commit ordering.

**Evolution opportunity:** adapt group commit fan-in, tail latency, drain aggressiveness, checkpoint-friendly draining, and write batching.

**Audit rules:** batch sizes must be nonzero and capped; group delay must not exceed the configured delay; resampling cannot move durability backward.

**Primary benchmarks:** write-heavy inserts, concurrent commits, strict vs normal durability, fsync count, p99 commit latency, WAL bytes per transaction.

---

### 4.2 Buffer residency / dirty flush / prefetch policy

**Current surface:** `crates/kernel/src/storage/policy.rs` exposes victim score, dirty batch size, dirty frame ordering, and prefetch cold-load behavior.

**Standard input:** frame metadata, dirty frame metadata, resident page count, dirty page count, capacity.

**Standard output:** optional victim score, dirty batch page count, dirty frame sort, prefetch cold-load boolean.

**Protected invariants:** stages cannot evict pinned frames, cannot flush pages whose LSN is ahead of durable WAL, cannot inspect page bytes, cannot write storage, and cannot mutate frame state.

**Evolution opportunity:** workload-aware eviction, clean-first eviction, checkpoint throughput, scan-resistant policies, and prefetch aggressiveness.

**Audit rules:** clean unpinned frames must remain evictable; pinned frames and undurable dirty frames must never be evictable; dirty batch size must be nonzero when dirty pages exist.

**Primary benchmarks:** read-hit ratio, eviction rate, checkpoint throughput, mixed read/write latency, cold scan behavior.

---

### 4.3 Heap append-lane and reusable-page placement

**Current surface:** `crates/kernel/src/engine/page_heap/policy.rs` exposes row lane, relation lane, and reusable-page choice.

**Standard input:** row id, relation id, lane count, page kind, encoded length, queued reusable pages.

**Standard output:** lane indexes and reusable-page decision.

**Protected invariants:** stages cannot create row IDs, rewrite tuple pointers, mutate row directories, decide visibility, or allocate pages directly.

**Evolution opportunity:** better lane striping, hot-relation isolation, mixed relation distribution, reusable page conservation, append contention reduction.

**Audit rules:** every lane decision must be `< lane_count`; zero lane counts are normalized before decision use; reusable-page output is an enum only.

**Primary benchmarks:** insert/update throughput, multi-relation writes, append-lane contention, page reuse rate.

---

### 4.4 Undo traversal advisory policy

**Current surface:** `crates/kernel/src/engine/page_heap/policy.rs` has undo prefetch and depth-limit hooks.

**Standardized safe subset:** advisory prefetch only. I intentionally do **not** standardize early termination as a generated decision in the first epoch. Returning “not visible” before the true visible version is found can be a correctness bug, not just a performance choice.

**Standard input:** undo depth and next undo pointer.

**Standard output:** prefetch-next boolean.

**Protected invariants:** stages cannot decide tuple visibility, skip required undo records, manufacture payloads, or terminate traversal.

**Evolution opportunity:** prefetch windows, cold-chain heuristics, latency/throughput tradeoffs for update-heavy workloads.

**Audit rules:** prefetch must be advisory; ignored prefetch must preserve correctness; stage output must not be used as a stop condition.

**Primary benchmarks:** update-heavy point reads, long undo chains, snapshot reads after concurrent writes.

---

### 4.5 Index scan batching / sibling prefetch policy

**Current surface:** `crates/kernel/src/index/policy.rs` exposes cursor batch constants, right-sibling prefetch, and stop-after-leaf logic.

**Standardized safe subset:** batch sizing and sibling prefetch only. I intentionally do **not** expose “stop after leaf” as a generated correctness decision in first epoch. Range-bound termination must remain core logic.

**Standard input:** entries in leaf, right sibling availability.

**Standard output:** vector wrapper batch size, raw cursor batch size, prefetch-right-sibling boolean.

**Protected invariants:** stages cannot choose search paths, compare keys, apply MVCC visibility, stop range scans, or decode index cells.

**Evolution opportunity:** low-latency cursor batching, high-throughput cursor batching, sibling prefetch windows, scan/read locality.

**Audit rules:** batches must be nonzero and bounded; prefetch is advisory only.

**Primary benchmarks:** range scan throughput, point lookup latency, ordered index scans, cache behavior on adjacent leaves.

---

### 4.6 B-tree leaf split policy

**Current surface:** `crates/kernel/src/index/policy.rs` exposes split point and duplicate split mode.

**Standard input:** logical key summaries and page body capacity.

**Standard output:** split point and duplicate mode.

**Protected invariants:** stages cannot encode cells, write pages, assign child pointers, update root/meta pages, compare physical tuple refs incorrectly, or bypass validation.

**Evolution opportunity:** duplicate-heavy splits, write-optimized splits, low-latency splits, split hysteresis, improved fill factor.

**Audit rules:** split point must be inside `1..entries.len()` for multi-entry pages; duplicate mode must be one of the known safe modes; default behavior keeps distinct-key boundaries where possible.

**Primary benchmarks:** index build time, insert into indexed tables, duplicate-heavy indexes, range scan shape after heavy inserts.

---

### 4.7 SQL physical planning policy

**Current surface:** `crates/sql/src/planner/policy.rs` exposes join kind, aggregate kind, and ordering kind decisions.

**Standard input:** estimated rows, equality/indexability flags, selection presence, group columns, input ordering, and limit.

**Standard output:** executor-supported join, aggregate, and ordering operators.

**Protected invariants:** stages cannot parse SQL, rewrite expressions, remove predicates, change projection semantics, or emit unsupported executor operators.

**Evolution opportunity:** join bias, hash vs nested loop thresholds, top-N threshold, streaming aggregate preference, index nested-loop aggressiveness.

**Audit rules:** join kind must be executor-supported; `Cross` is allowed only when no join selection exists; aggregate/order choices must be supported physical operators.

**Primary benchmarks:** analytical selects, joins with/without indexes, top-N queries, group-by workloads.

---

### 4.8 SQL query-memory / vectorization policy

**Current surface:** `crates/sql/src/connection/options.rs` contains query-memory knobs: work memory, max spill bytes, and batch rows. Execution code also has vectorized/top-K behavior referenced from planner policy.

**Standard input:** configured memory, spill cap, configured batch rows, estimated rows, estimated row width, operator class.

**Standard output:** work memory for an operator, batch rows, spill permission.

**Protected invariants:** stages cannot drop rows, reorder unordered output into ordered output, change expression results, or bypass max spill caps.

**Evolution opportunity:** adaptive batch sizing, hash join memory allocation, aggregate memory pressure control, top-K tuning, spill avoidance.

**Audit rules:** batch rows must be nonzero and capped; operator memory must be <= configured work memory; spill cap must be respected.

**Primary benchmarks:** hash joins, hash aggregates, sorts/top-N, large result scans, memory-pressure tests.

---

## 5. Explicit non-stages

These areas should **not** be evolved in the first epoch:

- Page layout and page checksums
- WAL binary record format
- WAL recovery replay order
- CSN/LSN ordering semantics
- MVCC visibility truth table
- Transaction state transition rules
- Lock ownership and unlock correctness
- Catalog DDL semantics
- SQL parser/AST correctness
- Expression truth tables and collation semantics
- FFI ABI and public client API

Those surfaces are correctness contracts. Performance may eventually be improved near them, but only by adding safe policy hooks around them, not by making them genes.

---

## 6. Genome model

A genome is a manifest plus one implementation per stage family.

```text
Genome {
  kernel: {
    wal_schedule,
    buffer_policy,
    heap_placement,
    undo_traversal,
    index_cursor,
    leaf_split,
  },
  sql: {
    planner,
    query_memory,
  },
  metadata: {
    id,
    parent_ids,
    generator,
    seed,
    stage_versions,
  }
}
```

Missing stages use defaults. Invalid stages are rejected before compilation or before benchmark registration, depending on how they are supplied.

---

## 7. Validation pipeline

Every generated stage should pass this sequence:

1. **Static ABI check:** implements the exact stage trait.
2. **Audit check:** bounded outputs and allowed decisions.
3. **Unit tests:** per-stage invariant tests.
4. **Composition smoke tests:** all selected stages together with defaults for the rest.
5. **Correctness suite:** SQL logic, MVCC, WAL/recovery, integrity check, index validation.
6. **Crash/failpoint suite:** commit, WAL flush, page image redo, checkpoint/vacuum.
7. **Microbench:** isolated read/write/index/planner scenarios.
8. **Macrobench:** full database workload suite.

No generated genome should be benchmarked for speed until it passes correctness and integrity.

---

## 8. Patch strategy

The companion diff is intentionally a **stage ABI foundation patch**:

- Adds `crates/kernel/src/stages/mod.rs` with stage families, manifests, audits, default kernel stages, and a `KernelStageSet`.
- Adds `crates/sql/src/stages.rs` with SQL stage families, manifests, audits, default SQL stages, and a `SqlStageSet`.
- Exposes the modules from `crates/kernel/src/lib.rs` and `crates/sql/src/lib.rs`.
- Adds a checked-in architecture document.

This patch does not yet rewrite every hot call site to use trait objects because that should be a second patch after the ABI is reviewed. The safe sequence is:

1. Standardize ABI and audits.
2. Wire defaults through existing policy modules without behavior change.
3. Add genome selection and benchmark registration.
4. Allow generated replacement stages.
5. Add evolutionary benchmark loop.

---

## 9. Why not more than 8?

More stages are tempting, but most extra candidates cross into correctness ownership. For example:

- A “visibility stage” could easily return wrong rows.
- A “WAL encoding stage” could create unrecoverable files.
- A “lock ownership stage” could unlock another transaction’s row.
- A “recovery strategy stage” could skip required redo.
- A “parser stage” could alter SQL semantics.

The point is not to maximize the count numerically. It is to maximize evolutionary surface area while keeping generated combinations non-stillborn. Eight is the practical maximum I would defend for first-epoch RedlineDB.

---

## 10. Second-epoch candidates

After the first stage genome is working and well-tested, add:

1. **Lock wait/backoff policy** — only if lock ownership remains core-owned and the stage only chooses wait/yield/backoff duration.
2. **Commit barrier profile** — only if the stage selects among existing durability profiles and cannot publish commits or durable LSNs directly.

Both should require stress tests and failpoint tests before inclusion in evolution.
