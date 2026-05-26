# RedlineDB Engineered Intelligence Stage Architecture

**Repository studied:** `neverhuman/RedlineDB`, `main` branch  
**Goal:** maximize the number of high-value, swappable performance stages while preserving a database correctness membrane strong enough that arbitrary stage combinations do not produce stillborn engines.

## 1. Executive recommendation

RedlineDB is already shaped around narrow kernel modules: SQL, storage, MVCC, WAL, recovery, B-tree, buffer pool, catalog, telemetry, and vector/JSON subsystems. The safest decomposition is **not** to let agents rewrite MVCC visibility, WAL durability, page encodings, catalog publication, or SQL semantics directly. Those are the *membrane*. The evolutionary surface should instead expose **policy stages** that can influence placement, scheduling, batching, search, prefetch, split points, lock strategy, and pure-function kernels. Each stage returns a bounded decision; the engine validates, clamps, or ignores the decision before it touches durable state.

My strongest recommendation is a two-layer design:

1. **Stage traits:** narrow standard interfaces that agents can implement.
2. **Stage guards:** invariant-preserving wrappers that normalize outputs before core code observes them.

This patch implements the first foundation: a central `kernel::stage` module and guard-wrapped adoption for the three highest leverage stage families that already exist in the codebase: buffer policy, heap policy, and index policy. It also documents the next stage surfaces and explicitly labels the hard invariants that must not become free genes.

## 2. Core principle: every gene is advisory until validated

A reasoning agent can invent a new stage, but the stage does not get authority over correctness. It gets authority over a bounded decision, for example:

- Which buffer frame is *eligible* for eviction.
- How many dirty pages to flush in one batch.
- Which append lane to use for a row.
- Whether to prefer a reusable heap page.
- How large an index scan batch should be.
- Where to split a B-tree leaf.
- Whether to prefetch a right sibling.

The surrounding engine remains responsible for:

- MVCC visibility rules.
- WAL-before-data ordering.
- commit CSN publication ordering.
- page checksums and on-disk format.
- catalog snapshot atomicity.
- B-tree ordering and structural validity.
- SQL-result equivalence.

The practical rule is: **stages may choose among legal options; they may not define legality.**

## 3. Terms

| Term | Meaning |
| --- | --- |
| Stage | A small replacement unit with a standard interface and bounded output. |
| Gene | One implementation of one stage interface. |
| Genome | A set of selected genes across stage surfaces. |
| Membrane | Guard code that validates/clamps stage decisions before durable/core code consumes them. |
| Stillborn genome | A stage combination that cannot compile, cannot open a DB, violates invariants, panics on ordinary use, or fails smoke correctness. |
| Fitness harness | Benchmark plus correctness gates used to rank genomes. |

## 4. What this patch changes

The diff adds:

- `crates/kernel/src/stage/mod.rs`: central stage descriptors, safety labels, standard input structs, and guard functions.
- `pub mod stage;` in `crates/kernel/src/lib.rs`.
- Guard adoption in:
  - `crates/kernel/src/storage/policy.rs`
  - `crates/kernel/src/engine/page_heap/policy.rs`
  - `crates/kernel/src/index/policy.rs`
- `docs/engineered-intelligence-stages.md`: this engineering spec in-repo.

The patch is intentionally conservative. It does **not** add dynamic dispatch, plugin loading, or runtime-selected genomes yet. It standardizes the stage *contract* first so later dynamic genomes can be added without widening unsafe surfaces.

## 5. The maximum safe stage inventory

I would standardize **22 stage surfaces**. The first 9 are included in the initial patch foundation; the rest should be wired after invariant-specific tests exist.

### 5.1 Implemented/wrapped now

| ID | Stage surface | Interface shape | Why it is safe | Guard/membrane |
| --- | --- | --- | --- | --- |
| S01 | Buffer victim scoring | `FrameMeta -> Option<u32>` | Chooses among already-eligible frames. | Pinned, referenced, loading, write-in-progress, and not-durable dirty frames are forced to `None`. |
| S02 | Dirty flush batch sizing | `(resident_pages, dirty_pages) -> usize` | Only changes throughput/latency tradeoff. | Clamped to `0` if no dirty pages, otherwise `1..=dirty_pages`. |
| S03 | Dirty flush ordering | `&mut [DirtyFrameMeta]` | Reorders a legal dirty set. | All candidates are pre-filtered by durable LSN. Future guard can verify permutation. |
| S04 | Buffer prefetch cold-load policy | `(resident_pages, capacity) -> bool` | Advisory only; correctness never depends on warming. | Prefetch is ignored on shard contention or load error. |
| S05 | Heap row/relation append lane | `(RowId/RelId, lane_count) -> usize` | Changes contention distribution only. | Lane index is modulo-clamped into range. |
| S06 | Heap page reuse preference | `(PageKind, encoded_len, queued_pages) -> PreferReusable/AllocateFresh` | Chooses between reusable and fresh pages. | Cannot prefer reusable when queue is empty. Page insert still validates capacity. |
| S07 | Undo traversal prefetch | `(depth, UndoPtr) -> bool` | Advisory only; read result does not depend on prefetch. | Suppressed for null pointers and excessive depth. |
| S08 | Undo depth-limit hint | `(depth, UndoPtr) -> Option<usize>` | May cap pathological chains but cannot fabricate visibility. | Lower bound is current depth + 1; upper bound is a hard global max. |
| S09 | Index cursor batch/prefetch/stop | batch constants plus range checks | Affects scan granularity and early stop. | Batch size clamped; right-prefetch suppressed without a right sibling; stop function has a default exact comparator. |
| S10 | B-tree leaf split point/duplicate mode | `entries, body_capacity -> split` | Chooses legal split of an already sorted vector. | Split clamped to legal `1..len-1`; duplicate mode cannot skip validation. |

### 5.2 Next safe stage surfaces to wire

| ID | Stage surface | Interface shape | Required guard before enabling evolution |
| --- | --- | --- | --- |
| S11 | Index descent child selection | `(internal entries, key, high_key, siblings) -> child` | Verify selected child is consistent with separators/high keys; otherwise fall back to reference descent. |
| S12 | Unique-key lock striping/hash | `(logical_key, shard_count) -> shard` | Modulo-clamp shard; canonical byte equality check still enforces uniqueness. |
| S13 | Row-lock backoff/acquire policy | `(contention, elapsed, timeout) -> wait/yield/retry` | Must never exceed timeout or skip required lock. |
| S14 | Commit barrier plan | `(configured durability, append_lsn, tx metadata) -> flush/write/unsafe` | May not lower configured durability; can only strengthen or choose existing legal barrier. |
| S15 | WAL group-commit batch formation | `(pending appends, durability) -> batch` | Maintain per-transaction LSN order and flush-until semantics. |
| S16 | Checkpoint page scheduler | `(dirty pages, durable_lsn) -> flush order/batches` | Only pages with `page_lsn <= durable_lsn`; final control-file write remains core. |
| S17 | Recovery replay scheduler | `(wal records, target) -> replay order/filter` | Must preserve LSN order and include all records required by target; reference replay diff gate. |
| S18 | Vacuum candidate selection | `(row heads, undo chains, horizon) -> candidates` | Candidate rechecked under current oldest-active-snapshot horizon before prune. |
| S19 | Catalog snapshot compaction/save plan | `(schema snapshot, tx state) -> write plan` | Commit publication order remains core; atomic save and publish verified. |
| S20 | SQL plan micro-policy | `(AST/logical plan/stats) -> physical plan choice` | Differential SQLite/RedlineDB corpus equality and parameterized replay. |
| S21 | JSON/datetime scalar kernels | pure-function scalar operator replacements | Golden corpus equality, null/type/error compatibility. |
| S22 | Vector distance/filter kernels | pure-function numeric kernels | Tolerance-bounded equality and determinism across CPU features. |

## 6. Surfaces deliberately excluded as free genes

The following should not become unconstrained stages:

1. **MVCC visibility itself.** It can be optimized only as a fast path with exact fallback, because wrong visibility corrupts isolation.
2. **WAL record encoding and LSN assignment.** Agents may batch or schedule, but not redefine durable ordering.
3. **Page format/checksum.** Agents may choose placement, but page construction/validation remains fixed.
4. **Commit CSN frontier logic.** Agents may choose barrier scheduling; they cannot publish CSNs out of order.
5. **Catalog publication semantics.** Agents can optimize encoding/compaction, not atomic schema visibility.
6. **SQL semantic compatibility.** Agents can choose plans and kernels only behind corpus/equivalence gates.

## 7. Stage interface design

Each stage should implement one narrow trait and, when compile-time selected, provide descriptors through `StageSpec`:

```rust
pub trait StageSpec {
    fn stage_descriptors() -> &'static [StageDescriptor];
}
```

Descriptors are metadata for agents and genome manifests. They include:

```rust
pub struct StageDescriptor {
    pub surface: StageSurface,
    pub stage_name: &'static str,
    pub safety: StageSafety,
    pub contract: &'static str,
}
```

Safety levels:

- `Pure`: deterministic, no side effects.
- `Advisory`: can be ignored without changing correctness.
- `Guarded`: output is clamped/verified before use.
- `InvariantCritical`: may exist as an internal stage only with strong post-verification.

## 8. Genome composition rule

A genome is valid only if every selected gene satisfies:

1. It compiles under the exact trait signature.
2. Its output is accepted or clamped by the guard layer.
3. It passes stage-local invariant tests.
4. It passes kernel smoke tests.
5. It passes SQL parity/differential tests for any SQL-facing gene.
6. It passes crash/recovery tests for any WAL/checkpoint/recovery gene.

Invalid stage output should not make the engine stillborn. The guard should either:

- clamp to the nearest legal value,
- fall back to the reference implementation, or
- return a normal database error before durable mutation.

## 9. Recommended genome manifest shape

This is a future runtime-selection format; the current patch is compile-time only.

```toml
[genome]
id = "candidate-2026-05-25-a17"
base = "main"

[stage.buffer_victim]
crate = "redlinedb_stage_buffer_hot_read"
symbol = "HotReadVictim"

[stage.heap_lane]
crate = "redlinedb_stage_heap_hash_lane"
symbol = "HashStripeLane"

[stage.index_leaf_split]
crate = "redlinedb_stage_index_duplicate_heavy"
symbol = "DuplicateHeavySplit"

[gates]
require_kernel = true
require_sql_parity = true
require_recovery = true
```

## 10. Evolution harness

A genome should be scored only after a staged gate ladder:

1. **Compile gate:** `cargo check --workspace`.
2. **Unit invariant gate:** all stage policy tests.
3. **Kernel smoke gate:** page, buffer, heap, index, tx, recovery tests.
4. **Crash gate:** failpoints and replay after simulated drops.
5. **SQL differential gate:** SQLite-shaped corpus equality.
6. **Benchmark gate:** compare against baseline with enough repetitions to smooth noise.
7. **Regression budget:** reject if speed improves one scenario but violates latency/correctness budgets elsewhere.

Fitness should be multi-objective:

- median query latency,
- p95/p99 latency,
- write throughput,
- recovery time,
- memory footprint,
- syscalls/fsyncs,
- code-shape/complexity penalty,
- flake rate.

## 11. Why the initial patch starts with buffer/heap/index

These three areas are the best first target because they are high-performance, high-leverage, and already partly separated by policy modules. They influence contention, cache residency, page reuse, scan batching, and B-tree shape, while the page/MVCC/WAL/B-tree validators remain in control.

The patch does **not** claim that these are the only valuable surfaces. It creates the membrane and proves the pattern in the places where the code already has natural seams. After this lands, the next highest-value wiring is:

1. checkpoint scheduler,
2. WAL group-commit scheduler,
3. row-lock backoff,
4. index descent verified-choice,
5. SQL micro-planner stage.

## 12. Concrete invariants added by the patch

The central `stage` module defines reusable guard functions:

- `guard_lane`: a stage can never return an out-of-range lane.
- `guard_index_cursor_batch`: a stage can never return `0` or an absurdly large batch.
- `guard_leaf_split_point`: a stage can never split a non-trivial leaf at `0` or `len`.
- `guard_dirty_batch_pages`: a stage can never flush more pages than were selected.
- `guard_buffer_victim_score`: a stage can never evict pinned, referenced, or not-durable dirty frames.
- `guard_heap_reuse`: a stage can never request reuse from an empty reuse queue.
- `guard_undo_prefetch`: a stage can never prefetch a null undo pointer.
- `guard_undo_depth_limit`: a stage can never return a limit below the current traversal point.
- `guard_index_sibling_prefetch`: a stage can never prefetch a non-existent sibling.
- `default_stop_after_leaf`: exact reference range-bound stopping logic.

## 13. Agent authoring guide

When generating a new gene, agents should produce:

1. One small implementation file or module.
2. A `StageSpec` descriptor.
3. A local invariant test.
4. A short rationale: expected performance win, workload hypothesis, risk.
5. A benchmark selector: which benchmark should show the effect.
6. A rollback path: reference stage fallback.

Agents should not edit page format, WAL encoding, or MVCC visibility to win a benchmark unless the task is explicitly a kernel correctness redesign.

## 14. Suggested follow-up patch sequence

### Patch 2: compile-time genome feature selection

Add Cargo features such as:

- `stage-buffer-clean-first`
- `stage-buffer-checkpoint-throughput`
- `stage-heap-hash-stripe`
- `stage-index-duplicate-heavy`

Then replace `type Active... = ...` aliases with feature-gated aliases. This enables cheap static genomes.

### Patch 3: runtime manifest selection

Add a `GenomeConfig` parsed from TOML/JSON and stored in `EngineConfig`. Use enum-dispatched built-in genes first. Avoid dynamic library loading until the invariant harness is mature.

### Patch 4: external stage crates

Expose only stable input/output structs from `redlinedb-kernel::stage`. External stage crates compile against this public surface, not against internal page/tx structs.

### Patch 5: evolutionary runner

Add a benchmark harness that builds genome candidates, runs gate ladders, records results, and keeps the Pareto frontier.

## 15. Validation notes for this artifact

I could inspect the repository through GitHub/web interfaces, but this sandbox could not clone GitHub directly (`Could not resolve host: github.com`) and does not have `rustc`/`cargo` installed. I therefore could not run `cargo fmt`, `cargo check`, or tests here. The patch is written conservatively against the inspected source and should be applied locally with:

```bash
git apply redlinedb_engineered_intelligence_stages.diff
cargo fmt
cargo test -p redlinedb-kernel
just fast
```

The most likely adjustments, if any, are minor formatting or visibility fixes around `StageSpec` imports. The architecture and guard boundaries are the core recommendation.
