# RedlineDB Kernel Stage Genome Engineering Spec

## 1. Objective

This spec proposes the largest stage surface I believe can be safely standardized in `crates/kernel` without turning future generated candidates into mostly stillborn combinations.

The goal is not to make the database dynamically plugin-driven at runtime. The goal is to create a compile-time genome: a small set of audited Rust policy traits with identical interfaces, explicit invariants, and one active type alias per stage. Reasoning agents can then generate new implementations of those traits, swap the active aliases, run correctness gates, and benchmark the resulting genome.

This is intentionally conservative. RedlineDB is an embedded storage engine with crash recovery, WAL, MVCC, a page file, B-tree indexes, row locks, and checkpoint sidecars. A stage that can reorder commit publication, change WAL record semantics, alter page encoding, or redefine MVCC visibility is not safe as an evolutionary gene unless it is surrounded by a much larger proof harness. The surfaces below are chosen because they strongly affect performance but can be constrained to be advisory, monotonic, or locally validated.

## 2. Repository facts used

The repository is already shaped for this. The public README describes RedlineDB as an embedded SQL engine whose storage core owns MVCC, a concurrent B-tree, group-commit WAL, and crash recovery. The architecture map states the workspace DAG as `domain → kernel → sql → redlinedb → {ffi, cli, server}`, and names `crates/kernel` as the owner of pages, WAL, MVCC, catalogs, integrity, vector, and JSONB.

The kernel already contains four ad-hoc policy seams:

- `crates/kernel/src/storage/policy.rs`
- `crates/kernel/src/wal/policy.rs`
- `crates/kernel/src/engine/page_heap/policy.rs`
- `crates/kernel/src/index/policy.rs`

Those files prove the codebase is already partially staged, but the seams are not yet standardized. This patch turns them into first-class kernel stages and adds the two missing safe stage families: row-lock wait behavior and checkpoint flush behavior.

## 3. Stage definition

A kernel stage is a compile-time Rust policy type that implements:

```rust
pub trait KernelStage {
    const SPEC: StageSpec;
}
```

Each stage family then adds a narrower behavior trait such as `WalSchedulePolicy`, `BufferPolicy`, or `LeafSplitPolicy`.

A generated stage must satisfy all of these rules:

1. It must be deterministic for identical inputs.
2. It must not own global mutable state.
3. It must not perform I/O.
4. It must not spawn threads.
5. It must not publish transaction state, mutate page bytes directly, or bypass WAL.
6. It must return bounded values where bounded values are required.
7. It must preserve all semantic invariants documented on the trait.

The active genome is the set of `type Active...Policy = ...` aliases. Evolution changes those aliases and/or adds new policy implementations.

## 4. Why compile-time stages first

A runtime plugin registry sounds attractive, but it would be the wrong first cut for this database.

Compile-time stages give:

- zero dynamic dispatch overhead in hot paths after monomorphization/inlining;
- direct access to Rust type checking and unit tests;
- simple bisection of a bad gene by reverting one alias;
- no ABI commitment across experiments;
- no unsafe dynamic loading boundary;
- no need to serialize internal page, tuple, and WAL metadata into generic plugin objects.

A later runtime registry can be layered on top once the winning stage contracts stabilize. The standard interface should be designed now, but the execution model should stay static until the proof machinery is stronger.

## 5. Proposed protected stage set

This is the maximum set I believe is defensible now: eight active performance stages plus the standard metadata wrapper.

### S0. Stage metadata wrapper

**File:** `crates/kernel/src/stage.rs`

This is not a performance stage. It is the common identity and invariant surface used by all stages.

**Interface:**

```rust
pub struct StageSpec {
    pub id: &'static str,
    pub domain: StageDomain,
    pub version: u16,
    pub summary: &'static str,
    pub invariants: &'static [&'static str],
}

pub trait KernelStage {
    const SPEC: StageSpec;
}
```

**Why it matters:** generated code must be machine-auditable. Every candidate can be required to provide a stable id, domain, version, and invariant list before it can enter a genome.

---

### S1. WAL schedule stage

**File:** `crates/kernel/src/wal/policy.rs`

**Existing interface standardized:**

```rust
pub(crate) trait WalSchedulePolicy: KernelStage {
    fn write_batch_bytes(ctx: WalScheduleContext) -> usize;
    fn group_commit_delay_us(ctx: WalScheduleContext) -> u64;
    fn resample_flush_target(ctx: WalScheduleContext) -> bool;
    fn drain_batch_bytes(ctx: WalScheduleContext) -> usize;
}
```

**Performance levers:**

- group-commit fan-in;
- fdatasync frequency;
- write-batch size;
- latency/throughput tradeoff;
- how much late-arriving WAL work gets folded into one durable train.

**Protected invariant boundary:**

This stage never chooses WAL record contents and never publishes commit state. It only chooses bounded scheduling parameters. Correctness still lives in the coordinator and writer.

**Non-stillborn rule:** any implementation must return non-zero write/drain sizes and must not exceed configured group-commit delay.

---

### S2. Buffer residency and flush stage

**File:** `crates/kernel/src/storage/policy.rs`

**Existing interface standardized:**

```rust
pub(crate) trait BufferPolicy: KernelStage {
    fn victim_score(meta: FrameMeta) -> Option<u32>;
    fn dirty_batch_pages(resident_pages: usize, dirty_pages: usize) -> usize;
    fn sort_dirty_frames(frames: &mut [DirtyFrameMeta]);
    fn prefetch_cold_load(resident_pages: usize, capacity: usize) -> bool;
}
```

**Performance levers:**

- eviction preference;
- dirty-page flush batching;
- checkpoint/writeback locality;
- read prefetch aggressiveness.

**Protected invariant boundary:**

The buffer pool still enforces pin count, dirty LSN durability, and page validation. A policy may rank or decline candidates, but cannot evict pinned pages or flush non-durable pages.

**Non-stillborn rule:** a replacement must return `None` for pinned frames and for dirty frames whose page LSN exceeds the durable WAL LSN.

---

### S3. Heap placement and reusable-page stage

**File:** `crates/kernel/src/engine/page_heap/policy.rs`

**Existing interface standardized:**

```rust
pub(super) trait HeapPlacementPolicy: KernelStage {
    fn row_lane(row_id: RowId, lane_count: usize) -> usize;
    fn relation_lane(rel_id: RelId, lane_count: usize) -> usize;
    fn reusable_page(kind: PageKind, encoded_len: usize, queued_pages: usize) -> ReuseDecision;
}
```

**Performance levers:**

- append-lane striping;
- row-directory shard contention;
- relation-directory shard contention;
- page reuse versus fresh allocation.

**Protected invariant boundary:**

The heap still owns tuple encoding, undo encoding, page insertion, and head-pointer mutation. The stage only maps work to lanes and chooses whether a queued reusable page is preferred.

**Non-stillborn rule:** returned lanes must always be `< lane_count.max(1)`.

---

### S4. Undo-chain read stage

**File:** `crates/kernel/src/engine/page_heap/policy.rs`

**Existing interface standardized:**

```rust
pub(super) trait UndoReadPolicy: KernelStage {
    fn prefetch_next(ctx: UndoReadContext) -> bool;
    fn depth_limit_hint(ctx: UndoReadContext) -> Option<usize>;
}
```

**Performance levers:**

- long undo-chain traversal behavior;
- speculative prefetch of prior undo pages;
- pathological chain cap hints.

**Protected invariant boundary:**

This stage cannot decide tuple visibility. It can only hint about traversal. Returning a depth limit is allowed only as a defensive cap; the default remains unbounded.

**Non-stillborn rule:** `depth_limit_hint` must either be `None` or greater than the current depth for ordinary progress.

---

### S5. Index cursor scan stage

**File:** `crates/kernel/src/index/policy.rs`

**Existing interface standardized:**

```rust
pub(super) trait IndexCursorPolicy: KernelStage {
    const VEC_WRAPPER_BATCH: usize;
    const RAW_CURSOR_BATCH: usize;

    fn prefetch_right_sibling(entries_in_leaf: usize, has_right: bool) -> bool;
    fn stop_after_leaf(last_logical_key: Option<&[u8]>, end: &Bound<Vec<u8>>) -> bool;
}
```

**Performance levers:**

- raw cursor batch size;
- vector-wrapper batch size;
- sibling prefetch;
- early range-scan stop.

**Protected invariant boundary:**

The cursor still validates page type and walks B-link siblings. The stage only decides advisory prefetch and stop-after-leaf decisions based on already-seen ordered keys.

**Non-stillborn rule:** batch constants must be non-zero; `stop_after_leaf` must not stop before an inclusive/exclusive bound has been passed.

---

### S6. Index leaf split stage

**File:** `crates/kernel/src/index/policy.rs`

**Existing interface standardized:**

```rust
pub(super) trait LeafSplitPolicy: KernelStage {
    fn split_point(entries: &[Entry], body_capacity: usize) -> usize;
    fn duplicate_mode(entries: &[Entry], split: usize) -> DuplicateSplitMode;
}
```

**Performance levers:**

- split balance;
- duplicate-heavy key handling;
- separator strategy;
- future page-fill heuristics.

**Protected invariant boundary:**

The B-tree still owns physical encoding, latches, WAL logging, and page rewrite. The stage only chooses a split point and duplicate handling mode.

**Non-stillborn rule:** for a splittable leaf, split must be `> 0` and `< entries.len()`.

---

### S7. Row-lock wait stage

**File:** `crates/kernel/src/engine/lock/policy.rs`

**New interface:**

```rust
pub(crate) trait LockWaitPolicy: KernelStage {
    fn park_duration(ctx: LockWaitContext) -> Option<Duration>;
    fn timed_out(ctx: LockWaitContext) -> bool;
}
```

**Performance levers:**

- short-slice versus deadline parking;
- timeout responsiveness;
- contention tail latency;
- future spin-then-park strategies.

**Protected invariant boundary:**

The lock manager still owns FIFO waiter queues and ownership transfer. The stage only chooses how long the current waiter parks before rechecking.

**Non-stillborn rule:** timeout must not occur before the configured busy timeout has elapsed.

---

### S8. Checkpoint flush stage

**File:** `crates/kernel/src/engine/maintenance/policy.rs`

**New interface:**

```rust
pub(crate) trait CheckpointPolicy: KernelStage {
    fn flush_batch_pages(ctx: CheckpointContext) -> usize;
    fn prune_wal_after_control(ctx: CheckpointContext) -> bool;
}
```

**Performance levers:**

- checkpoint dirty-page batch size;
- yielding behavior through batch size;
- WAL retention/pruning aggressiveness after a valid control file lands.

**Protected invariant boundary:**

The engine still flushes WAL before flushing pages, writes tx-status and catalog sidecars, then writes the control file. This stage cannot reorder the crash-consistency sequence.

**Non-stillborn rule:** `flush_batch_pages` must be non-zero; skipping WAL prune is safe but may increase disk use.

## 6. Explicitly rejected stage surfaces

These are intentionally *not* staged in this patch:

### Page format and checksum encoding

Tempting, but unsafe. `Page::from_bytes`, `Page::validate`, slot layout, special bytes, and checksum semantics are compatibility and recovery contracts. A generated stage here can easily make old files unreadable or corrupt recovery.

### WAL payload encoding and record scan semantics

Not safe yet. Changing payload shape, record length, or scan rules can invalidate crash recovery. WAL scheduling is staged; WAL meaning is not.

### MVCC tuple visibility

Very high leverage, but too dangerous. Visibility decides whether rows exist. A generated visibility policy can silently violate isolation. Only undo-chain traversal hints are staged.

### Commit publish ordering

Do not stage until model-checked. `Engine::commit` has a delicate order: append commit record, force/write barrier based on durability, publish CSN, release locks, and handle maybe-committed failpoints. Reordering this can create unrecoverable acknowledged transactions.

### Catalog snapshot semantics

DDL/catalog correctness needs stronger schema-evolution proofs. Catalog I/O sync policy may be parameterized later, but catalog content and epoch semantics should not be a gene yet.

### Recovery replay acceptance

Replay filtering by LSN/CSN and committed transaction set is correctness-critical. This should only become a stage after deterministic crash-fuzzing can prove equivalence across recovery targets.

### B-tree page format/latch protocol

Split heuristics and cursor behavior are staged. Page header layout, B-link latch coupling, and WAL logging are not.

## 7. Genome composition model

A genome is a single Rust build with one active policy alias per stage family:

```rust
type ActiveWalSchedulePolicy = WalScheduleDefault;
type ActiveBufferPolicy = BufferClockPolicy;
type ActiveHeapPlacementPolicy = HeapModuloPolicy;
type ActiveUndoReadPolicy = HeapModuloPolicy;
type ActiveIndexCursorPolicy = IndexCurrentPolicy;
type ActiveLeafSplitPolicy = IndexCurrentPolicy;
type ActiveLockWaitPolicy = LockWaitDeadlinePolicy;
type ActiveCheckpointPolicy = CheckpointBalancedPolicy;
```

A generated candidate may add a new type, for example:

```rust
pub(crate) struct WalScheduleBurstCoalesce;

impl KernelStage for WalScheduleBurstCoalesce { ... }
impl WalSchedulePolicy for WalScheduleBurstCoalesce { ... }
```

Then the genome mutates by changing the alias:

```rust
pub(crate) type ActiveWalSchedulePolicy = WalScheduleBurstCoalesce;
```

## 8. Candidate acceptance protocol

A generated stage must pass the following before it can be benchmarked:

1. `cargo fmt --all --check`
2. `cargo check -p redlinedb-kernel`
3. Kernel unit tests for the modified policy module.
4. Crash/recovery tests if WAL, buffer, heap, checkpoint, or lock stages changed.
5. SQLite parity lane before any published performance claim.
6. Benchmark comparison with the exact genome manifest recorded.

## 9. Genome manifest recommendation

This patch adds metadata but does not yet add a manifest generator. The next patch should produce a `kernel-genome.json` like:

```json
{
  "wal_schedule": "wal.schedule.default@1",
  "buffer": "buffer.clock@1",
  "heap_placement": "heap.modulo@1",
  "undo_read": "heap.modulo@1",
  "index_cursor": "index.current@1",
  "leaf_split": "index.current@1",
  "lock_wait": "lock.wait.deadline@1",
  "checkpoint": "checkpoint.balanced@1"
}
```

That manifest should be emitted with each benchmark result so evolutionary search can correlate latency improvements with exact stage combinations.

## 10. Expected first experiments

The first useful agent-generated genes should target:

1. WAL scheduling for latency versus throughput.
2. Buffer dirty-frame ordering for checkpoint throughput.
3. Heap lane hashing under multi-writer contention.
4. Index split policy for duplicate-heavy workloads.
5. Cursor batch sizing for range scans and index-backed SQL.
6. Row-lock wait slicing for high-contention writes.
7. Checkpoint flush batch sizing for p99 latency.

Those stages have enough performance leverage to matter and enough local invariants to protect correctness.
