# RedlineDB engineered-intelligence stage surfaces

## Executive summary

This spec defines the maximum stage set I think is safe to standardize in the current RedlineDB codebase without making random genome combinations likely to be stillborn. The result is intentionally not a plugin ABI and not a trait-object runtime dispatch layer. RedlineDB’s hot path is a Rust storage kernel; the safest genome unit is a compile-time, zero-overhead policy seam with an audited input/output contract, a default implementation, and a manifest entry that reasoning agents can target.

The best practical maximum is **nine stage families**:

1. SQL planner choice policy
2. WAL schedule policy
3. Heap placement policy
4. Undo read policy
5. Index cursor policy
6. Leaf split policy
7. Commit barrier policy
8. Row lock policy
9. Transaction admission policy

The first six already exist in partial form as policy modules. The patch formalizes them as stage surfaces and adds a kernel stage manifest. The last three are new seams around the engine runtime and lock manager. I intentionally do **not** expose tuple visibility, WAL record encoding, page layout, B-tree key ordering, checksum selection, or recovery replay semantics as free genome genes yet, because those surfaces can silently corrupt data or produce combinations that only fail under crash/recovery.

## Objective

The engineered-intelligence goal is to make important RedlineDB behaviors replaceable by agents while preserving correctness by construction. A stage should be:

- **Small enough** that an agent can invent an alternative implementation.
- **Important enough** to affect performance on real workloads.
- **Abstract enough** that future implementations are not forced into the current algorithm.
- **Constrained enough** that arbitrary combinations of valid stage implementations keep producing a working database.
- **Observable enough** that benchmarks can attribute wins and regressions.

## Current architecture observations

RedlineDB is already layered in a way that supports this approach. Public API code is in `crates/redlinedb`, SQL parser/planner/executor code is in `crates/sql`, and the storage kernel owns catalog, page storage, WAL, MVCC, and recovery. That split makes the kernel and planner the right initial places for stage standardization.

Inside the kernel, several modules already contain policy-style seams:

- `crates/kernel/src/wal/policy.rs` controls WAL batch sizing, group-commit delay, target resampling, and drain sizing.
- `crates/kernel/src/engine/page_heap/policy.rs` controls heap lane placement, reusable page decisions, undo prefetch, and undo depth hints.
- `crates/kernel/src/index/policy.rs` controls cursor batching, right-sibling prefetch, early range termination, leaf split points, and duplicate handling.
- `crates/sql/src/planner/policy.rs` controls join, aggregate, and ordering choices.

Those surfaces are valuable, but they are not yet standardized as one “stage genome” model, and some important runtime surfaces are still hard-coded inside `Engine::commit`, `Engine::begin`, and `RowLockManager`.

## Definition: stage, gene, genome

A **stage family** is a stable interface and invariant set around one replaceable decision surface.

A **gene** is one implementation of a stage family, such as `WalScheduleTailLatency` for `WalSchedulePolicy`.

A **genome** is one assignment of active gene per stage family. In this patch, genome composition remains compile-time and Rust-native through `type ActiveFooPolicy = FooDefaultPolicy`. That is deliberate. The immediate goal is to safely create the search space. Runtime hot-swapping can be layered later using the same contracts once the benchmark and crash-validation harness can reject bad genes reliably.

## Stage eligibility rules

A code surface is eligible when it passes these filters:

1. **Inputs are value/context only.** The stage receives snapshots of state, not mutable access to kernel internals, unless it is already the owner of that state.
2. **Outputs are bounded choices.** The stage returns an enum, bounded number, boolean, or existing plan kind. It does not return arbitrary encoded bytes, raw pointers, or page mutations.
3. **Persistent format is unchanged.** A stage may change placement, scheduling, or algorithm selection. It may not change on-disk page/WAL/catalog encoding in this phase.
4. **Correctness is clampable.** The call site can reject, clamp, or safely interpret invalid outputs.
5. **Composition is independent.** Combining a valid WAL schedule with a valid heap placement and a valid planner policy should not require pairwise compatibility rules.

## Stage safety classes

The patch adds `StageSafetyClass` so future tooling can rank mutation risk.

### `PurePolicy`

The stage changes only a local choice and has no direct side effects. Most stages belong here. Examples: cursor batch size, join kind, leaf split point.

### `ClampedPolicy`

The stage can affect durability or concurrency latency, so the call site must clamp unsafe outputs. Examples: commit barrier action, row-lock wake mode.

### `CriticalSideEffect`

The stage directly mutates persistent state or recovery behavior. This spec does not expose any new stage in this class. Future work may add such stages only behind crash-consistency proof gates.

## Proposed stage families

### 1. SQL planner choice policy

**Current location:** `crates/sql/src/planner/policy.rs`

**Interface:** `PlannerPolicy`

**Controls:**

- Join kind selection: nested loop, index nested loop, hash, cross.
- Aggregate algorithm: streaming or hash.
- Ordering algorithm: full sort or top-N.

**Why it matters:** Planner decisions can dominate query latency, especially as table sizes and indexes vary.

**Safety argument:** The policy can only return existing `JoinKind` and `PhysicalKind` variants. It cannot bypass expression evaluation, change row visibility, or mutate storage.

**Required invariants:**

- Join kind must be one of the supported join variants.
- Aggregate kind must be `StreamingAggregate` or `HashAggregate`.
- Ordering kind must be `Sort` or `TopN`.
- The planner must preserve residual predicates when choosing an access path.

**Gene examples already present:**

- `SqlCurrentPolicy`
- `SqlIndexJoinBiasPolicy`
- `SqlVectorBatchPolicy`
- `SqlHashThroughputPolicy`

### 2. WAL schedule policy

**Current location:** `crates/kernel/src/wal/policy.rs`

**Interface:** `WalSchedulePolicy`

**Controls:**

- WAL writer batch byte target.
- Group-commit delay.
- Whether to resample the flush target after the group-commit window.
- Drain bytes before fsync.

**Why it matters:** Commit throughput and tail latency are heavily shaped by batching and fsync grouping.

**Safety argument:** The policy cannot mark records durable, skip WAL writes, change record bytes, or publish commit CSNs. It only influences when the existing writer drains and flushes.

**Required invariants:**

- Write batch bytes must be at least 1.
- Drain batch bytes must be at least 1.
- Group delay must not exceed configured delay unless the policy explicitly documents why and the call site accepts it.
- Resampling may widen the flush target, but durability still occurs before waiters are released.

**Gene examples already present:**

- `WalScheduleDefault`
- `WalScheduleTailLatency`
- `WalScheduleFanInAdaptive`
- `WalScheduleCheckpointFriendly`

### 3. Heap placement policy

**Current location:** `crates/kernel/src/engine/page_heap/policy.rs`

**Interface:** `HeapPlacementPolicy`

**Controls:**

- Mapping row ids to row-directory and append lanes.
- Mapping relation ids to relation-directory lanes.
- Preference for reusing a page versus allocating fresh.

**Why it matters:** Lane placement and page reuse affect contention, cache locality, page churn, and vacuum behavior.

**Safety argument:** The stage returns only lane indexes and a reuse/fresh decision. The caller still owns the actual page allocation, directory update, and page-state transitions.

**Required invariants:**

- Returned lane must be less than lane count.
- Lane count zero must be treated as one effective lane.
- Reusable-page decisions must not fabricate page ids.
- Page reuse must only consume pages already marked reusable by the heap.

**Gene examples already present:**

- `HeapModuloPolicy`
- `HeapHashStripePolicy`
- `HeapReuseConservativePolicy`
- `HeapUndoPrefetchPolicy`

### 4. Undo read policy

**Current location:** `crates/kernel/src/engine/page_heap/policy.rs`

**Interface:** `UndoReadPolicy`

**Controls:**

- Whether to prefetch the next undo record.
- Optional undo-chain depth limit hint.

**Why it matters:** MVCC reads and write-conflict checks can traverse undo chains. Prefetch and guard rails affect latency under update-heavy workloads.

**Safety argument:** The stage never decides tuple visibility. Visibility remains in `ConcurrentVisibility` and transaction status logic. The depth hint is the dangerous part; it must be treated as a guard rail, not a semantic cutoff for normal operation.

**Required invariants:**

- Prefetch decisions must be advisory only.
- Depth limit must be `None` or large enough that normal undo chains are not truncated.
- The policy must never claim a tuple is visible or invisible directly.

**Patch recommendation:** Keep this as a stage, but enforce a conservative audit threshold in future test harnesses. The existing `HeapUndoPrefetchPolicy` uses `Some(4096)`, which is acceptable as a defensive upper bound.

### 5. Index cursor policy

**Current location:** `crates/kernel/src/index/policy.rs`

**Interface:** `IndexCursorPolicy`

**Controls:**

- Batch size for vector wrapper cursors.
- Batch size for raw cursors.
- Right-sibling prefetch decision.
- Early stop after a leaf when the range bound is satisfied.

**Why it matters:** Index scan throughput and tail latency depend on cursor batching and leaf traversal behavior.

**Safety argument:** The stage cannot skip predicate checks or mutate B-tree structure. It only controls iteration shape and advisory prefetch.

**Required invariants:**

- Batch constants must be greater than zero.
- `stop_after_leaf` must be monotonic with respect to the end bound.
- Prefetch must be advisory.
- The cursor must still evaluate visibility and range semantics outside the policy.

**Gene examples already present:**

- `IndexCurrentPolicy`
- `IndexLargeBatchPolicy`
- `IndexDuplicateHeavyPolicy`
- `IndexLowLatencyPolicy`

### 6. Leaf split policy

**Current location:** `crates/kernel/src/index/policy.rs`

**Interface:** `LeafSplitPolicy`

**Controls:**

- Split point selection when a B-tree leaf overflows.
- Duplicate-run strategy.

**Why it matters:** Split balance and duplicate handling affect write amplification, tree height, scan locality, and point lookup cost.

**Safety argument:** The stage chooses only a split boundary and duplicate mode. The B-tree code still rewrites pages, records WAL, and validates page structure.

**Required invariants:**

- Split point must be in `1..entries.len()` for splittable pages.
- Duplicate mode must be one of the supported variants.
- The policy must not reorder entries.
- Separator semantics must remain consistent with encoded key ordering.

### 7. Commit barrier policy

**New location:** `crates/kernel/src/engine/policy.rs`

**Interface:** `CommitBarrierPolicy`

**Controls:**

- Whether a strict commit can publish before the durability barrier on the special no-side-effect fast path.
- Whether a commit waits for flush, write, or skips the barrier based on configured durability.
- Whether WAL flushes on shutdown.
- Whether catalog sidecar sync is durable.

**Why it matters:** Commit latency and throughput are central database performance metrics. The existing code has an important strict fast path and three durability modes; this makes that surface explicit and evolvable.

**Safety argument:** The policy returns a `CommitBarrierAction`, but the call site can clamp any unsafe choice. `Strict` must not skip the barrier, and schema/index side-effect commits must not publish before their barrier.

**Required invariants:**

- `Strict` durability must map to flush unless the call site is in the existing no-side-effect fast path and still flushes immediately after publish.
- `Normal` durability must map to at least write acknowledgement.
- `UnsafeDev` is the only mode allowed to skip.
- Catalog sync must stay durable for `Strict` and `Normal`.
- Commit outcome semantics must remain `Committed`, `RolledBack`, or `MaybeCommitted` exactly as before.

### 8. Row lock policy

**New location:** `crates/kernel/src/engine/policy.rs`

**Interface:** `RowLockPolicy`

**Controls:**

- Mapping `(rel_id, row_id)` to a lock shard.
- Wake mode on unlock: targeted front-of-FIFO handoff or broadcast.

**Why it matters:** Row lock sharding and wake behavior affect contention, fairness, scheduler churn, and tail latency under concurrent writes.

**Safety argument:** The policy cannot grant locks directly or bypass owner checks. It only chooses a shard index and wake style. The lock manager still enforces ownership, FIFO queueing, timeout, and release.

**Required invariants:**

- Shard index must be less than shard count after clamping.
- Wake mode must be one of the supported enum variants.
- Lock ownership remains exclusive.
- Timeout behavior must remain enforced by the lock manager.

### 9. Transaction admission policy

**New location:** `crates/kernel/src/engine/policy.rs`

**Interface:** `TransactionAdmissionPolicy`

**Controls:**

- Admission/rejection of requested isolation mode at transaction start.

**Why it matters:** This is not the hottest stage today, but it is a safe hook for future concurrency experiments such as read-only routing, snapshot mode selection, admission throttling, and workload-aware transaction shaping.

**Safety argument:** The current implementation preserves existing behavior: `Serializable` is rejected; all supported modes are admitted. Future implementations must not admit an isolation level that the rest of the engine cannot enforce.

**Required invariants:**

- Unsupported isolation levels must be rejected.
- Admitted transactions must still be allocated by `ConcurrentTxStatus`.
- The policy must not create transaction ids or snapshots directly.

## Surfaces intentionally not exposed yet

### Tuple visibility

Do not expose `ConcurrentVisibility` as a free gene yet. It is compact and tempting, but one wrong variant can violate MVCC correctness while appearing fast in non-adversarial benchmarks.

### WAL/page/catalog encoding

Do not allow agents to mutate WAL record format, checksums, page headers, tuple encodings, or catalog snapshot encoding in this stage system. These require compatibility and recovery proof obligations.

### Recovery replay semantics

Recovery target selection and replay filtering are too crash-critical for the initial genome. Recovery can later gain stages for scheduling, scan prefetch, or parallel replay after deterministic crash tests exist.

### B-tree key ordering and physical suffix encoding

Changing the ordering contract would invalidate existing indexes and corrupt range scans. Key normalization can become a future stage only if it is versioned per index and persisted in catalog metadata.

### SQL parser behavior

Parser changes are broad compatibility changes, not performance-stage genes. They should remain separate.

## Genome composition model

The initial genome is compile-time:

```rust
type ActiveWalSchedulePolicy = WalScheduleDefault;
type ActiveHeapPlacementPolicy = HeapModuloPolicy;
type ActiveUndoReadPolicy = HeapModuloPolicy;
type ActiveIndexCursorPolicy = IndexCurrentPolicy;
type ActiveLeafSplitPolicy = IndexCurrentPolicy;
type ActivePlannerPolicy = SqlCurrentPolicy;
type ActiveCommitBarrierPolicy = CommitBarrierCurrentPolicy;
type ActiveRowLockPolicy = RowLockCurrentPolicy;
type ActiveTransactionAdmissionPolicy = TransactionAdmissionCurrentPolicy;
```

Reasoning agents generate candidate implementations that satisfy the same trait. Evolution swaps the active aliases, builds the crate, runs the invariant tests, then runs benchmark and crash-validation suites.

This avoids runtime dispatch in the hot path and avoids dynamic ABI compatibility problems while still giving agents a clean target.

## Validation gates for generated genes

Every generated gene should pass the following gates before benchmarking:

1. **Static compile gate:** `cargo check --workspace --all-targets`.
2. **Stage invariant tests:** existing and new `*_drop_ins_preserve_basic_invariants` tests.
3. **Fast correctness gate:** `just fast` or equivalent crate tests.
4. **SQL compatibility gate:** SQLite parity suite for planner/executor-affecting genes.
5. **Crash gate:** recovery/failpoint tests for WAL, commit, heap, and index-affecting genes.
6. **Benchmark gate:** microbench plus macrobench with at least p50/p95/p99 and throughput.
7. **Regression gate:** reject wins that reduce correctness coverage or fail determinism under repeated runs.

## Benchmark attribution

Each benchmark result should record:

- Active stage family -> gene mapping.
- Git commit and build profile.
- Workload name, seed, and dataset scale.
- Durability mode and WAL settings.
- Buffer pool size, page size, heap lanes, lock shards.
- Throughput, p50, p95, p99, max latency.
- WAL fsync/write counters.
- Lock wait buckets.
- Index leaf visits / cursor batches.
- Recovery time if the gene affects WAL, heap, or index mutation.

## Why nine is the practical maximum now

The repo already has four policy modules that cover six meaningful surfaces. Adding commit barrier, row lock, and transaction admission reaches the boundary of what can be isolated safely without changing persistent formats or allowing stages to decide core MVCC truth.

More stages are possible later, but they require additional protection:

- Executor vectorization stages need a stable batch/expression ABI.
- Recovery stages need deterministic crash harnessing.
- Page layout stages need versioned on-disk format negotiation.
- Index key stages need catalog-persisted comparator identity.
- Buffer replacement stages need a narrow frame-selection API in `BufferPool`.

## Patch summary

The accompanying diff does the following:

1. Adds `crates/kernel/src/stages.rs` with shared stage metadata, stage families, safety classes, invariant descriptors, and a kernel manifest.
2. Exposes the manifest from `crates/kernel/src/lib.rs`.
3. Adds `crates/kernel/src/engine/policy.rs` with commit barrier, row lock, and transaction admission policy traits and default genes.
4. Wires `Engine::begin` through `TransactionAdmissionPolicy`.
5. Wires `Engine::commit`, catalog sync, and WAL shutdown flush through `CommitBarrierPolicy` while preserving default behavior.
6. Wires `RowLockManager` shard selection and unlock wake behavior through `RowLockPolicy` while preserving default FIFO handoff.
7. Widens existing index policy traits from `pub(super)` to `pub(crate)` so they can participate in crate-level audits and manifests.
8. Adds `crates/sql/src/stages.rs` with an SQL stage manifest and exports it from `crates/sql/src/lib.rs`.
9. Adds this architecture spec under `docs/architecture/ENGINEERED_INTELLIGENCE_STAGES.md`.

## Implementation note

The diff is intentionally conservative. It standardizes and names the surfaces without changing default behavior. The default active policies reproduce the current engine behavior: strict commits still flush, normal commits still wait for WAL write, unsafe dev commits still skip the barrier, serializable isolation is still rejected, and row locks still use targeted FIFO handoff.

