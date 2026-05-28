# RedlineDB Engineered-Intelligence Stage Surface Specification

**Target repository:** `neverhuman/RedlineDB`  
**Spec status:** implementation-ready proposal with a conservative additive patch  
**Primary objective:** expose the maximum practical number of high-leverage database-kernel decision surfaces as standardized, swappable stages without allowing arbitrary stage combinations to corrupt correctness, durability, or SQL compatibility.

---

## 1. Executive summary

RedlineDB already has the right shape for engineered intelligence. The repository is not a monolith: the storage kernel is separated from SQL and adapters, and the kernel itself is decomposed into `catalog`, `engine`, `format`, `heap`, `index`, `storage`, `txn`, `wal`, `vector`, `json`, `integrity`, and `telemetry`. The most promising evolutionary unit is therefore not a replacement for the entire engine. It is a replacement for narrow decision functions that sit beside correctness-critical code.

The maximum safe stage count I would standardize now is **16 stages**:

1. `WalSchedule`
2. `WalCombine`
3. `CommitBarrier`
4. `RecoveryReplay`
5. `BufferResidency`
6. `CheckpointBatch`
7. `Prefetch`
8. `HeapPlacement`
9. `Visibility`
10. `LockAdmission`
11. `IndexAccess`
12. `IndexMutation`
13. `CatalogSnapshot`
14. `VectorDistance`
15. `VectorTopK`
16. `JsonPath`

That number is deliberately not much larger. There are more interesting functions in the codebase, but many are not safe evolutionary surfaces because they own durable facts, byte-level compatibility, commit publication order, or page-format invariants. Those should be protected by guards and tests, not opened as free-form genes.

The attached diff implements the first safe slice: it adds `redlinedb-kernel::stages`, a standardized stage contract module, and a detailed repository-local design document. It does **not** change runtime behavior yet. This is intentional. The first merge should create stable interfaces and default decisions, then later patches can route existing `wal::policy`, `storage::policy`, planner, heap, index, vector, and JSON call sites through the stage contracts one at a time.

---

## 2. Design principle: maximize replaceability, minimize stillbirth

A stage is valid only when all of these are true:

1. **The input is a bounded snapshot.** A stage receives facts, not mutable engine ownership.
2. **The output is a decision, transform, ranking, or hint.** It does not directly write pages, publish commits, mutate the catalog, or acknowledge durability.
3. **The kernel remains the guard.** The caller clamps batch sizes, validates visibility, verifies page/WAL checksums, enforces unique constraints, and preserves commit ordering.
4. **The default implementation is behavior-preserving.** A genome containing only defaults should be semantically identical to current behavior.
5. **The failure mode is safe.** A bad stage should be rejected, clamped, or degraded to default rather than producing a corrupt database.

This gives agents room to invent stage implementations while keeping the database alive across arbitrary valid stage combinations.

---

## 3. Why these 16 stages are the practical maximum

### 3.1 Stage candidates accepted now

| # | Stage | Existing area | Why it is safe | Why it is high-leverage |
|---|---|---|---|---|
| 1 | `WalSchedule` | `crates/kernel/src/wal/policy.rs`, WAL writer loop | Bounded scheduling decision; does not change record bytes or durability semantics | Controls group-commit fan-in, latency, write batching, and fsync cadence |
| 2 | `WalCombine` | `wal/combiner.rs` placeholder, logical WAL payload stream | Pure/guarded transform; kernel can re-encode and checksum results | Can reduce WAL volume and replay cost |
| 3 | `CommitBarrier` | `engine/runtime/commit.rs` | Returns required barrier only; kernel still executes `flush_until`/`write_until` | Lets evolution explore strict/normal latency tradeoffs without weakening strict mode |
| 4 | `RecoveryReplay` | `engine/recovery.rs`, WAL scan reports | Produces replay plan; recovery still validates WAL/page images | Can reduce startup time by choosing replay windows and checkpoints |
| 5 | `BufferResidency` | `storage/buffer.rs`, `storage/policy.rs` | Victim selection can be clamped to unpinned/durable frames | Directly impacts read hot-set behavior and write amplification |
| 6 | `CheckpointBatch` | dirty-frame batching in `storage/buffer.rs` and engine checkpointing | Chooses batch size/order; kernel still persists pages and control file safely | Large effect on checkpoint stalls and cache churn |
| 7 | `Prefetch` | buffer prefetch advisory path | Pure hint; correctness must not depend on it | Can improve index scans, sequential scans, and vector search locality |
| 8 | `HeapPlacement` | page-backed heap lanes and row allocation | Chooses lane/hint; heap still owns row ID allocation and tuple layout | Impacts multi-writer append contention |
| 9 | `Visibility` | MVCC snapshot checks | Must be guarded by invariant tests; returns boolean only | Central read-path cost in every query |
| 10 | `LockAdmission` | row lock FIFO queues and timeout behavior | Does not unlock/commit; only grants, queues, spins, or times out under lock manager guard | Impacts write concurrency and tail latency |
| 11 | `IndexAccess` | index cursor selection, range scans, row fetch paths | Chooses access path; executor still validates predicates | Large query latency impact |
| 12 | `IndexMutation` | B-tree insert/delete/split/vacuum policy | Chooses split/vacuum heuristics, not cell encoding | Impacts write path and index shape |
| 13 | `CatalogSnapshot` | catalog persistence and DDL publication | Chooses when/how to persist/publish after commit; kernel validates schema epoch and commit state | DDL-heavy and startup paths benefit |
| 14 | `VectorDistance` | scalar/SIMD vector distance dispatch | Pure numeric kernel; result can be compared against scalar oracle | High performance gain potential |
| 15 | `VectorTopK` | flat top-k, HNSW/DiskANN candidate ranking | Pure ranking; correctness can be bounded by exact fallback tests | Search latency and memory profile |
| 16 | `JsonPath` | JSONB extraction and path dispatch | Pure extraction strategy; output verified against reference path semantics | Important for JSON-heavy generated parity cases |

### 3.2 Candidates rejected for now

These are tempting but too dangerous as evolutionary genes in the first generation:

| Rejected surface | Reason |
|---|---|
| Raw page header/page tuple binary encoding | Must remain byte-compatible with persisted data; wrong output corrupts files |
| WAL record header encoding/checksum | A gene here can destroy recovery globally |
| `publish_commit` ordering | Commit visibility and durability ordering are correctness-critical |
| Control-file A/B generation swap | A wrong policy can make recovery choose stale state |
| Catalog type coercion and affinity semantics | Exposed to SQLite parity; better handled by isolated expression/codec stages later |
| FFI ABI behavior | External contract, not kernel performance policy |
| SQL parser syntax acceptance | Compatibility surface; stage after parsed/bound representation is safer |
| Arbitrary transaction isolation semantics | Serializable/read-committed rules should be kernel-level invariants |

---

## 4. Standard interface

Every stage implements the same conceptual interface:

```rust
pub trait Stage<I, O>: Send + Sync + 'static {
    fn descriptor(&self) -> StageDescriptor;
    fn evaluate(&self, input: I) -> Result<O>;
}
```

A stage also provides:

- `StageDescriptor`: identity, version, kind, purity, determinism, and notes.
- Input struct: immutable, serializable-friendly facts.
- Output struct: a decision or transform.
- Guard rules: caller-side clamping and invariant enforcement.
- Default implementation: current behavior.

This gives agents one uniform pattern while allowing each database surface to expose domain-specific inputs and outputs.

---

## 5. Genome model

A genome is a map from `StageKind` to one implementation:

```text
Genome {
  WalSchedule      -> wal_tail_latency_v7
  WalCombine       -> wal_noop_default
  CommitBarrier    -> strict_default
  RecoveryReplay   -> checkpoint_window_v3
  BufferResidency  -> hot_scan_clock_v12
  ...
}
```

A genome is valid when:

1. It has exactly one implementation for every required stage.
2. Every implementation advertises the expected `StageKind`.
3. Every implementation passes its local stage audit.
4. The composed genome passes smoke tests, crash tests, and SQL parity tests.
5. The stage set does not request forbidden capabilities.

The current patch only defines the contract layer. A future patch should add `StageGenome` and `EngineConfig::stages` once two or more call sites have been routed through the interface.

---

## 6. Stage-by-stage engineering contracts

### 6.1 `WalSchedule`

**Input:** pending bytes, pending records, flush gap, configured write batch, configured group delay, max group bytes.  
**Output:** write batch bytes, group commit delay, whether to resample flush target, drain batch bytes.  
**Guard:** minimum batch size is `1`; delay cannot exceed configured delay; strict durability still flushes before acknowledgment.

**Evolution ideas:**

- Tail-latency mode that disables delay for single commits.
- Fan-in adaptive mode for high-concurrency workloads.
- Checkpoint-friendly mode that widens groups during checkpoint pressure.
- IO-depth-aware mode using recent sync counters.

### 6.2 `WalCombine`

**Input:** ordered logical WAL payloads for a transaction or drain batch.  
**Output:** replacement payload sequence.  
**Guard:** output must preserve transaction ID, commit semantics, record ordering constraints, and WAL decodeability.

**Evolution ideas:**

- Collapse update chains on the same row before commit.
- Remove insert/delete pairs for aborted or internally canceled mutations.
- Combine adjacent logical events into coarser redo payloads.

### 6.3 `CommitBarrier`

**Input:** durability mode, end LSN, written LSN, durable LSN.  
**Output:** required barrier: flush, write, or volatile acknowledgment.  
**Guard:** strict mode cannot be downgraded below `FlushUntil(end_lsn)`.

**Evolution ideas:**

- Allow normal mode to switch between write and flush based on group state.
- Add optional budget-aware barriers for bulk import.

### 6.4 `RecoveryReplay`

**Input:** valid WAL end, checkpoint LSN, target LSN/CSN, torn-tail bit, scanned record count.  
**Output:** replay-from LSN, stop condition, whether torn tail is acceptable.  
**Guard:** replay cannot start after checkpoint if required pages are absent; torn tail acceptance requires checksum-valid prefix.

**Evolution ideas:**

- Faster point-in-time recovery target selection.
- Skip replay records provably dominated by later page images.
- Parallel redo chunk selection.

### 6.5 `BufferResidency`

**Input:** frame pin count, dirty bit, usage count, page LSN, durable LSN, residency pressure.  
**Output:** evictable, victim score, whether cold prefetch load is allowed.  
**Guard:** pinned frames and dirty pages newer than durable LSN are never evictable.

**Evolution ideas:**

- Scan-resistant CLOCK variants.
- Dirty-page-aware eviction during WAL backlog.
- Workload-adaptive hot read policy.

### 6.6 `CheckpointBatch`

**Input:** resident pages, dirty pages, durable LSN.  
**Output:** page limit and ordering key.  
**Guard:** page limit is clamped to available dirty pages; only durable pages can be flushed.

**Evolution ideas:**

- Sort by page LSN for sequential WAL/checkpoint behavior.
- Sort by page ID for fewer disk seeks.
- Smaller batches during latency-sensitive reads.

### 6.7 `Prefetch`

**Input:** residency probe result, contention state, resident pages, capacity pages.  
**Output:** drop hint, count hit, or cold-load.  
**Guard:** prefetch is advisory and never returns an error to the caller.

**Evolution ideas:**

- Disable cold-load under high residency pressure.
- Aggressive prefetch for range scans and vector search.
- Conservative prefetch for random point lookups.

### 6.8 `HeapPlacement`

**Input:** relation ID, optional row ID hint, payload length, lane count, writer ID.  
**Output:** target lane and optional accepted row ID hint.  
**Guard:** lane is modulo-clamped; heap remains source of truth for row IDs.

**Evolution ideas:**

- Payload-size-aware lane assignment.
- Relation-hotness-aware lane spreading.
- Bulk-import append-lane selection.

### 6.9 `Visibility`

**Input:** snapshot CSN, current transaction, tuple creator/deleter metadata.  
**Output:** visible/not visible.  
**Guard:** invariant tests compare against existing MVCC implementation across transaction state tables.

**Evolution ideas:**

- Branch-minimized fast path for committed visible tuples.
- Read-committed refresh specialization.
- SIMD/batch visibility for scans.

### 6.10 `LockAdmission`

**Input:** row key, requester, optional holder, waiter count, timeout.  
**Output:** grant, enqueue, spin, or timeout.  
**Guard:** lock manager retains exclusive ownership of holder state and unlock behavior.

**Evolution ideas:**

- Adaptive spin for very short lock holds.
- Writer convoy avoidance.
- Fairness tuning for mixed point updates and scans.

### 6.11 `IndexAccess`

**Input:** equality terms, range terms, ordering need, row estimate, covering index availability.  
**Output:** table scan, point lookup, range scan, covering range scan, or skip.  
**Guard:** executor rechecks predicates; planner never trusts a stage to prove correctness alone.

**Evolution ideas:**

- Better ORDER BY/LIMIT decisions.
- Covering index preference.
- Selectivity-driven fallback to table scan.

### 6.12 `IndexMutation`

**Input:** page fill, page capacity, incoming cell size, uniqueness facts.  
**Output:** split-before-insert and duplicate rejection hint.  
**Guard:** uniqueness is enforced by existing lock/table checks; page format remains owned by B-tree code.

**Evolution ideas:**

- Fill-factor tuning.
- Right-growth split avoidance.
- Bulk-load split policy.

### 6.13 `CatalogSnapshot`

**Input:** schema epoch, serialized size, volatile/in-memory flag.  
**Output:** persist atomically and publish after commit flags.  
**Guard:** schema publication occurs only after transaction commit.

**Evolution ideas:**

- Avoid redundant saves when schema epoch did not change.
- Batch DDL catalog snapshots.
- Faster cold-start metadata path.

### 6.14 `VectorDistance`

**Input:** dimensions, metric, SIMD availability.  
**Output:** scalar, SIMD, or approximate kernel choice.  
**Guard:** numeric tests compare against scalar oracle within tolerance.

**Evolution ideas:**

- Dimension thresholds for SIMD dispatch.
- Metric-specific approximations with exact fallback.
- CPU-feature-aware vector lanes.

### 6.15 `VectorTopK`

**Input:** candidate count, K, heap budget.  
**Output:** fixed heap, partial sort, full sort, or approximate.  
**Guard:** exact mode must match sorted oracle; approximate mode must be explicitly benchmarked separately.

**Evolution ideas:**

- Small-K fixed heap.
- Large-K partial sort.
- Budget-aware approximate ranking.

### 6.16 `JsonPath`

**Input:** JSON document length, path length, extraction count.  
**Output:** interpreter, compiled path, or cached path.  
**Guard:** output is compared against existing JSON-path semantics.

**Evolution ideas:**

- Cache repeated JSON paths.
- Compile long paths.
- Short-circuit scalar extraction.

---

## 7. Integration plan

### Phase 0 — Add contracts only

Included in the diff:

- Add `crates/kernel/src/stages/mod.rs`.
- Export `pub mod stages;` from `crates/kernel/src/lib.rs`.
- Add this stage-surface spec under `docs/architecture/`.
- Add lightweight invariant tests for default stage outputs.

No runtime behavior changes.

### Phase 1 — Wire existing policy surfaces

Recommended next patch:

1. Make `wal::policy::WalSchedulePolicy` an adapter around `stages::WalSchedule`.
2. Make `storage::policy::BufferPolicy` an adapter around `stages::BufferResidency`, `CheckpointBatch`, and `Prefetch`.
3. Add `EngineConfig::stage_registry: StageRegistry` only after the first two call sites are behavior-preserving.
4. Keep defaults monomorphized where possible to avoid dyn-dispatch overhead in hot paths.

### Phase 2 — Add genome loading

Add a `StageGenome` manifest:

```toml
[stages.wal_schedule]
name = "wal_tail_latency_v7"
abi = "kernel-stage-v1"
sha256 = "..."

[stages.buffer_residency]
name = "scan_resistant_clock_v3"
abi = "kernel-stage-v1"
sha256 = "..."
```

For Rust-native experiments, this can first be compile-time feature selection. For agent-generated experiments, prefer build-time codegen into a temporary crate over dynamic loading; this keeps Rust type checking and avoids unsafe plugin boundaries.

### Phase 3 — Evolution harness

Every genome run should produce:

- Build result.
- Unit tests.
- Stage audit results.
- Crash/failpoint matrix result.
- SQLite parity result.
- Microbenchmarks per stage.
- End-to-end DB benchmark.
- Genome manifest and git SHA.
- Reproducibility seed.

A genome should be eligible for mutation only if it passes all correctness gates. Performance-only ranking starts after correctness.

---

## 8. Stage guard checklist

Each stage implementation must pass:

1. **Panic safety:** no panic on valid input.
2. **Bounded output:** no zero batch sizes unless allowed; no lane out of range; no unsafe durability downgrade.
3. **Determinism declaration:** deterministic stages must produce same output for same input.
4. **Metamorphic tests:** monotonic input changes must not break obvious invariants.
5. **Default equivalence:** default stage equals current behavior.
6. **Composition tests:** random valid combinations must complete smoke SQL workload.
7. **Crash safety:** no lost acknowledged commits under failpoints.
8. **Parity:** generated SQLite corpus still passes.
9. **Regression budget:** performance can be worse in exploration, but correctness cannot.

---

## 9. Why the initial diff is additive

It is important that the first stage patch not change commit behavior, WAL flush behavior, or buffer eviction behavior. This repository already claims crash recovery, SQLite parity, and a failpoint matrix. Those are precious invariants.

The additive patch creates an architectural ratchet: future agents have a stable target, and future refactors can move one call site at a time behind the contract. That is the safest way to reach many stages without producing stillborn genomes.

---

## 10. Recommended immediate next PR after this diff

After merging the contract layer, I would do the following small PR:

1. Change `wal::policy::ActiveWalSchedulePolicy` to call `DefaultWalScheduleStage`.
2. Add a golden test that current `WalScheduleDefault` equals `DefaultWalScheduleStage` for a matrix of contexts.
3. Add two experimental scheduler implementations behind tests only.
4. Benchmark `sqlite-parity` and WAL-heavy write workloads.
5. Only then expose a genome selector.

This gives the evolution system its first real gene while protecting commit durability.
