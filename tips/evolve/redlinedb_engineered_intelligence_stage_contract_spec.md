# RedlineDB Engineered Intelligence Stage Contract Spec

## 0. Goal

This spec proposes a first-generation **stage genome** for RedlineDB: a set of isolated, replaceable decision surfaces in the kernel that can be evolved independently while keeping the database non-stillborn under arbitrary gene combinations.

The key idea is not to let generated code own database truth. A stage may choose *order, batching, placement, scheduling, or hints*. It must not own page encodings, WAL encodings, MVCC visibility truth, commit publication order, catalog meaning, or SQL semantics. Those are the protected core. The patch accompanying this spec adds a runtime `StageGenome` with guarded dispatch into the currently hard-coded or compile-time-policy areas of the kernel.

## 1. Repository baseline and current architecture observations

The repository is a Rust workspace with a dedicated kernel crate plus SQL, benchmark, FFI, CLI, and server crates. The current README describes RedlineDB as a Rust-native embedded SQL engine with a SQLite-shaped API, and positions the core around MVCC, concurrent B-tree indexing, group-commit WAL, and crash recovery.

The kernel crate exposes modules for catalog, engine, format, heap, index, integrity, storage, transaction, vector, and WAL code. Its crate-level documentation explicitly frames the kernel as the correctness-critical foundation: typed storage IDs, explicit on-disk page/WAL encodings, checksums, and MVCC visibility rules.

The important existing seam is that the repo already contains several private policy embryos:

- `crates/kernel/src/storage/policy.rs` defines buffer-pool victim, dirty-batch, dirty-order, and prefetch policies, but the active choice is a compile-time alias.
- `crates/kernel/src/engine/page_heap/policy.rs` defines heap placement/reuse/undo-read policy shapes, also selected by compile-time alias.
- `crates/kernel/src/wal/policy.rs` defines WAL write-batch, group-commit, flush-resample, and drain-batch policies, again selected by compile-time alias.

Those are exactly the right class of stage surface, but they are not yet a standard runtime genome. The patch promotes these surfaces into a central `StageGenome`, normalizes decisions through safety clamps, and wires engine creation/opening so benchmark runners can generate/drop in new genomes without editing page/WAL/MVCC code.

## 2. Design rule: stage decisions vs. protected invariants

A database evolution harness will produce many candidate genes. Most candidates will be wrong unless the contract prevents them from corrupting state. The kernel should therefore expose only “bounded decisions,” not raw mechanisms.

### 2.1 Protected invariants

These invariants are *not* stage-owned:

1. **Tuple identity and MVCC truth.** A stage may choose where row-directory shards live or whether to prefetch undo pages. It may not decide whether a tuple version is visible. `begin_tx`, `end_tx`, owner visibility, delete flags, and snapshot rules remain in the fixed kernel.
2. **Commit atomicity and CSN publication.** A stage may choose a stronger or policy-equivalent durability barrier. It may not publish a transaction before the minimum barrier required by `CommitDurability`.
3. **WAL record correctness.** A stage may choose writer batch size, group delay, and late flush resampling. It may not reorder record LSNs, rewrite record bytes, skip commit records, change segment arithmetic, or weaken recovery scan semantics.
4. **On-disk page correctness.** A stage may choose page flush order, buffer eviction preference, and heap-page reuse preference. It may not change page header layout, checksum rules, generation checks, slot encoding, or page LSN rules.
5. **Catalog/schema meaning.** A stage may not reinterpret relation IDs, index IDs, schema epochs, or catalog snapshots.
6. **SQL result semantics.** Planner/executor stages should be added later only behind equivalence checks. The first patch is kernel-only.

### 2.2 Allowed stage outputs

A stage output must be one of these safe classes:

- A bounded integer: batch size, checkpoint flush batch, group delay, drain limit.
- A stable ordering key: dirty-frame order, row-directory lane, relation-directory lane.
- A yes/no hint: cold prefetch allowed, reusable page preferred, WAL flush target may widen.
- A protected enum: commit barrier action clamped by configured durability.

The wrapper owns clamps. Generated genes never return unchecked raw values to kernel machinery.

## 3. Maximum safe stage decomposition

I divide candidate stage surfaces into four tiers.

### Tier A — wired by the patch: 14 replacement-ready kernel stage genes

These are decision points that can be safely combined now. The patch wires all 14 through `StageGenome`.

| # | Gene | Current code area | Stage decision | Protected by |
|---:|---|---|---|---|
| 1 | `runtime.commit_barrier` | `engine/runtime/commit.rs` | Commit barrier action: flush/write/none | Cannot weaken `Strict`; cannot weaken `Normal` below WAL write |
| 2 | `maintenance.checkpoint_batch` | `engine/maintenance.rs` | Dirty-page checkpoint batch size | Clamped to `1..=4096` |
| 3 | `buffer.victim` | `storage/buffer.rs` | Whether an unpinned resident frame is eligible for eviction | Pinned/write-in-progress/too-new-dirty frames are always rejected |
| 4 | `buffer.dirty_batch` | `storage/buffer.rs` | Dirty frames per flush pass | Clamped to at least one and no more than dirty frame count |
| 5 | `buffer.dirty_order` | `storage/buffer.rs` | Flush ordering key for dirty durable pages | Only reorders already-durable dirty pages |
| 6 | `buffer.cold_prefetch` | `storage/buffer.rs` | Whether advisory prefetch may load a cold page | Prefetch remains best-effort and error-swallowing |
| 7 | `heap.row_lane` | `engine/page_heap/directory/heads.rs` | Row-directory/append-lane stripe for a row ID | Lane index always modulo lane count |
| 8 | `heap.relation_lane` | `engine/page_heap/directory/heads.rs` | Relation-directory stripe for a relation ID | Lane index always modulo lane count |
| 9 | `heap.reusable_page` | `engine/page_heap/directory/vacuum.rs`, `mutation/write/append.rs` | Whether to pop from reusable page queue | Cannot pop when queue is empty; page kind remains validated |
| 10 | `heap.undo_prefetch` | `engine/page_heap/mutation/read.rs` | Whether to issue an undo-chain prefetch hint | Hint only; never truncates visibility scan |
| 11 | `wal.write_batch` | `wal/manager/coordinator/writer.rs` | Writer-thread pending WAL bytes drained per normal pass | Clamped to at least one byte |
| 12 | `wal.group_delay` | `wal/manager/coordinator/helpers.rs` | Group-commit wait duration | Clamped to configured max delay |
| 13 | `wal.resample_flush_target` | `wal/manager/coordinator/helpers.rs` | Whether late-arriving flush targets join current fsync train | Only widens monotonically; never lowers target |
| 14 | `wal.drain_batch` | `wal/manager/coordinator/writer.rs` | Extra WAL bytes drained before fsync | Clamped to at least one byte |

Why this is the maximum I would wire in the first patch: each stage can be tested by local invariants and does not require a semantic proof across SQL, index contents, or WAL replay. More importantly, any arbitrary combination of these genes should still construct an engine, run transactions, recover, and pass integrity checks because the wrapper clamps unsafe outputs.

### Tier B — safe, but requires coordinated adapter work

These should be next because they are likely high-leverage but need more scaffolding than a single wrapper.

| Candidate | Why it matters | Why it is deferred |
|---|---|---|
| Row-lock shard hashing | Hot rows and hash collisions can dominate write workloads | Current `RowLockManager` embeds `DefaultHasher`; stage needs a reusable `LockKeyRouter` plus tests for FIFO handoff |
| Lock wait handoff strategy | Tail latency under write contention | Current FIFO per-row condvar is correctness-sensitive; alternative waiters need starvation tests |
| B-tree descent/prefetch policy | Index lookup/scan hot path | Need inspect and wrap B-tree cursor/latch internals without exposing page mutation order |
| B-tree split/fill-factor policy | Index write amplification and fanout | Must preserve structural validation and crash replay; should start as bounded split thresholds only |
| Index cursor batching | Range-scan throughput | Need standard cursor result contract and cursor equivalence tests |
| Recovery replay batching | Crash recovery time | Must preserve commit visibility and page-image precedence; needs replay work queue proof |
| Heap vacuum row ordering | Vacuum throughput/cache locality | Must not race with head-pointer changes; needs explicit vacuum snapshot contract |
| SQL predicate evaluation order | CPU efficiency | Requires expression purity classification and error-order compatibility rules |
| SQL join enumeration budget | Query planning latency/perf | Existing options already define optimizer limits; stage needs plan-equivalence fallback |
| Executor batch sizing/spill threshold | Analytical throughput and memory pressure | Belongs in SQL crate, not first kernel patch |
| Vector search parameters | ANN performance | Only safe when recall/ordering contracts are explicitly metric-bounded |

### Tier C — hook-only observer stages

These are useful for instrumentation and evolution feedback but should not change behavior initially:

- Page hotness observer.
- WAL fan-in observer.
- Lock wait observer.
- Heap chain length observer.
- Index leaf visit observer.
- SQL operator timing observer.

The existing `Phase11Counters` surface is close to this class.

### Tier D — explicitly unsafe to stage directly

Do **not** make these replaceable genes without a much stronger proof system:

- Page header/cell encoding.
- WAL record encoding/checksum/segment numbering.
- MVCC visibility truth table.
- Transaction state publication and CSN frontier movement.
- Catalog snapshot encoding/decoding.
- SQL expression semantics and error behavior.

These can be optimized internally, but generated genes should not be allowed to swap them independently.

## 4. Patch architecture

The patch adds `crates/kernel/src/stages.rs` and exports it from `lib.rs`.

### 4.1 `StageGenome`

`StageGenome` is a compact, copyable manifest of stage choices:

```rust
pub struct StageGenome {
    pub runtime: RuntimeStageGenome,
    pub maintenance: MaintenanceStageGenome,
    pub buffer: BufferStageGenome,
    pub heap: HeapStageGenome,
    pub wal: WalStageGenome,
}
```

Each sub-genome contains small enums, not trait objects. This is intentional for generation one:

- It keeps genome values easy to serialize, hash, mutate, and benchmark.
- It avoids dynamic dispatch on extremely hot paths unless later benchmarks prove it worthwhile.
- It makes arbitrary combinations finite and exhaustively auditable.
- It still gives reasoning agents a standard surface: invent a new enum variant implementation behind the same method contract, then benchmark it as a gene.

A later plugin ABI can wrap the same method contracts with trait objects or WASM, but the protected core should stay the same.

### 4.2 Safety clamp pattern

Every stage method follows this pattern:

```rust
let raw = match selected_gene { ... };
clamp_or_reject(raw)
```

Examples:

- `buffer_victim_score` always rejects pinned frames, frames being written, and dirty frames whose page LSN exceeds durable WAL LSN.
- `commit_barrier_action` always returns `Flush` for strict durability, regardless of gene choice.
- `heap_reusable_page` returns `false` when `queued_pages == 0`, regardless of gene choice.
- `wal_group_commit_delay_us` never returns a delay larger than the configured delay.

This is what makes random genomes non-stillborn.

## 5. Engineering contracts for generated stage implementations

Any future gene implementation must satisfy these contracts.

### 5.1 Buffer stage contract

Inputs:

- Frame metadata: pin count, dirty flag, usage count, write-in-progress flag, page LSN, durable LSN.
- Dirty-frame metadata: page ID and page LSN.
- Resident/capacity counters.

Allowed outputs:

- Optional victim score.
- Dirty batch limit.
- Dirty sort key.
- Cold-prefetch boolean.

Hard rules:

- Never evict a pinned frame.
- Never evict a frame being written.
- Never flush/evict a dirty page newer than durable WAL.
- Never make prefetch required for correctness.

### 5.2 Heap stage contract

Inputs:

- Row ID, relation ID, lane count.
- Page kind, encoded tuple length, reusable queue length.
- Undo chain depth and next undo pointer.

Allowed outputs:

- Lane index.
- Reusable-page preference.
- Undo prefetch hint.

Hard rules:

- Lane index must be in range.
- Reusable queue pop is impossible when empty.
- Undo prefetch cannot truncate or skip the visibility chain.
- Tuple visibility remains fixed in `ConcurrentVisibility` and transaction snapshot code.

### 5.3 WAL stage contract

Inputs:

- Pending bytes and record count.
- Flush gap bytes.
- Configured batch/delay thresholds.

Allowed outputs:

- Write batch byte limit.
- Group delay.
- Flush-resample boolean.
- Drain batch byte limit.

Hard rules:

- LSN reservation remains monotonic.
- Record order in the pending queue remains FIFO by reserved LSN.
- Flush target can widen only upward.
- A commit cannot be acknowledged before the configured durability class is satisfied.

### 5.4 Runtime/maintenance stage contract

Allowed outputs:

- Commit barrier action, clamped by `CommitDurability`.
- Checkpoint dirty-page batch size.

Hard rules:

- Strict durability always uses `flush_until`.
- Normal durability never weakens below `write_until`.
- UnsafeDev may skip barriers but only because the user selected that durability mode.
- Checkpoint still flushes WAL first and writes tx-status/catalog/control state in the existing order.

## 6. Suggested evolutionary harness flow

1. Generate a `StageGenome` candidate.
2. Run a cheap construction smoke test: create/open, begin/insert/commit/get, checkpoint, reopen, get.
3. Run kernel invariants: page heap, recovery, failpoint smoke, index validation, integrity full check.
4. Run SQL parity smoke: inserts, updates, deletes, indexed scans, joins, transactions.
5. Run crash/failpoint matrix for commit/checkpoint/WAL barriers.
6. Only then benchmark.
7. Store the exact `StageGenome::manifest()` and benchmark stats with the result.

A candidate that fails any correctness stage gets zero fitness and is not benchmarked.

## 7. Validation plan

After applying the diff, run:

```bash
cargo fmt --all
cargo test -p redlinedb-kernel
cargo test -p redlinedb-sql
cargo test -p redlinedb-bench --lib
```

Then run targeted matrices:

```bash
cargo test -p redlinedb-kernel recovery -- --nocapture
cargo test -p redlinedb-kernel failpoint --features failpoints -- --nocapture
cargo test -p redlinedb-kernel integrity -- --nocapture
```

Finally, generate a small grid over stage genomes:

- default genome;
- latency-oriented WAL + small checkpoint;
- throughput WAL + large checkpoint;
- hash-striped heap + conservative reuse;
- clean-first buffer + LSN dirty order;
- random valid combinations.

Every genome must pass correctness before performance numbers count.

## 8. Why this is the right first cut

The repo already had the beginnings of policy separation, but those decisions were static and scattered. The proposed patch centralizes them into a single inspectable stage genome while keeping the correctness-critical kernel sealed. This gives the evolutionary system enough genes to make meaningful performance discoveries without letting a generated stage accidentally rewrite durability, MVCC visibility, or file formats.

The first-generation maximum I recommend is therefore:

- **14 wired kernel genes now**;
- **11 more coordinated-replacement genes after adapter work**;
- **6 observer-only genes for telemetry and fitness explanations**;
- **6 protected areas that should not be directly staged**.

That is a high enough decomposition to support real search, but narrow enough that arbitrary combinations should still produce valid working databases.
