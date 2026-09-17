# RedlineDB Stage Genome Engineering Specification

**Version:** 0.1
**Target repository:** `neverhuman/RedlineDB`, branch `main`
**Authoring intent:** standardize the largest practical set of RedlineDB hot-path decision surfaces into safe, drop-in “stages” that can be replaced by generated code without producing stillborn databases.
**Core conclusion:** the safe v1 maximum is **39 stage families**: **24 kernel stages** and **15 SQL stages**. This is deliberately *not* “everything that might affect performance.” It is the largest set I would expose now while keeping the storage, WAL, MVCC, schema, and SQLite-compatibility invariants host-enforced.

---

## 1. Executive summary

RedlineDB is already well-shaped for engineered intelligence: the code is split into a kernel crate with storage, MVCC, WAL, catalog, recovery, indexes, JSON, vector search, telemetry, and transactional runtime, and a SQL crate with parser, planner, executor, connection/session, value, collation, UDF, and compatibility layers. The tempting mistake would be to expose every module boundary as a gene. That would generate many stillborn combinations because page layout, WAL ordering, transaction publication, schema epoching, and SQLite semantics are not independent.

The proposed design protects a **small correctness kernel** and exposes **many bounded decisions** around it. Stages may choose lanes, batching, ordering, victim selection, cost estimates, memory budgets, access-path enumeration, and local execution strategy. Stages may not own raw files, publish transaction commit status, forge LSN/CSN values, alter durable encodings, mutate schema directly, change SQL semantics, or bypass host validation.

This spec proposes:

1. A shared stage vocabulary: `StageKind`, `StageFamily`, `StageGenome`, `StageBinding`, and `StageAuditReport`.
2. A compatibility contract: every stage has a contract version, deterministic behavior, bounded outputs, and a host-validated fallback.
3. A staged rollout: first add metadata and audit surfaces, then wire existing policy seams, then move SQL planner/executor policy, then allow external generated crates.
4. A concrete patch that adds public stage manifests to `redlinedb-kernel` and `redlinedb-sql` without changing behavior. This makes the first patch low-risk and gives agents a standard interface to target.

---

## 2. Design principle: expose decisions, not authority

A RedlineDB stage is not an arbitrary plugin. It is a replaceable implementation of one bounded decision surface. The host retains authority over all durable and semantic invariants.

### 2.1 Stage may do

A stage may:

- Choose between already-valid strategies.
- Return a bounded numeric decision, such as lane index, batch size, prefetch depth, spill threshold, or cost.
- Reorder independent work only when the host has declared the work commutative.
- Decline and ask the host to use the built-in default.
- Use read-only metadata passed through a typed context.
- Emit observability metadata under a host-controlled budget.

### 2.2 Stage may not do

A stage may not:

- Write a page, WAL segment, control file, catalog snapshot, or transaction status table directly.
- Invent row IDs, transaction IDs, CSNs, LSNs, schema epochs, or page IDs.
- Encode/decode durable records except through host APIs.
- Hold locks or latches across host calls.
- Observe raw pointer/lifetime-sensitive state.
- Change SQL result semantics, collation semantics, or constraint semantics.
- Panic as control flow.
- Block indefinitely.
- Allocate unbounded memory.
- Return an output that cannot be clamped or validated.

### 2.3 Non-stillborn genome rule

A genome is valid if:

1. It has exactly one enabled binding for every required stage kind in its declared scope.
2. Every binding advertises the current stage contract version.
3. Every binding passes its stage-specific audit.
4. Every decision is deterministic for the same inputs and stage seed.
5. Every decision is host-clamped before use.
6. Any stage failure maps to the built-in default decision without corrupting state.
7. The composition passes the invariant matrix described in §10.

The host should assume every generated stage is adversarially wrong until validated. The interface should make “wrong” mean “slower” or “falls back,” not “corrupts the database.”

---

## 3. The protected kernel

The following invariants must remain outside stage control:

### 3.1 WAL and durability

Protected:

- WAL record encoding and checksums.
- LSN allocation.
- Segment header and seal format.
- `append_commit` atomicity.
- The commit publication rule: a transaction is not visible until host-published.
- Recovery end-LSN detection and torn-tail handling.
- Control-file dual-write generation and checksum.

Stages may influence batching, flush timing, segment rotation thresholds, and semantic combining only through host-audited decisions.

### 3.2 MVCC and transaction visibility

Protected:

- Transaction ID allocation.
- Snapshot construction.
- CSN allocation, publish, cancel, and abort.
- Visibility rules for begin/end transaction IDs.
- Tuple deletion semantics.
- Serialization/conflict errors.
- Undo record durable format.

Stages may influence refresh timing, undo traversal prefetch, and pruning candidate selection, but the host decides visibility.

### 3.3 Page and heap layout

Protected:

- Page header and cell layout.
- Page magic, kind, generation, checksum, and LSN.
- Tuple/undo encoding.
- Row directory correctness.
- Page reinitialization.
- Dirty marking and flush safety.

Stages may choose heap lane, page reuse preference, victim candidates, and flush batches. The host pins pages, writes pages, and validates generations.

### 3.4 Index correctness

Protected:

- Key encoding and comparison semantics.
- Unique constraint outcome.
- B-tree page format.
- Structural split/merge validity.
- Cursor visibility checks.
- Index DML undo/repair semantics.

Stages may choose search/split heuristics, scan batching, and DML batching order, but the host checks keys and row visibility.

### 3.5 SQL semantics

Protected:

- Parser grammar accepted by the public API.
- Name resolution correctness.
- Type affinity rules.
- Collation result for committed built-in collations.
- Constraint enforcement.
- Trigger/view semantics.
- Result ordering when SQL requires ordering.
- SQLite-shaped FFI ABI.

Stages may optimize, cache, estimate, reorder legal joins, select access paths, and choose execution strategy only when semantics are preserved.

---

## 4. Proposed stage taxonomy

The safe v1 maximum is 39 stages. Anything beyond this should wait until the stage harness proves these work under evolution.

### 4.1 Kernel stages: 24

| # | Stage kind | Current code area | Decision exposed | Host-enforced invariant |
|---:|---|---|---|---|
| K01 | `EngineAdmission` | `kernel::engine` config/open paths | Normalize or reject config values; choose safe defaults | Engine opens only with legal page size, buffer size, lock shard count, and WAL settings |
| K02 | `TxAdmission` | `engine::runtime::begin` | Admit, queue, or reject based on isolation and load | Unsupported isolation and transaction ID rules remain host-owned |
| K03 | `SnapshotRefresh` | read-committed refresh points | Decide when to refresh read-committed snapshots | Host constructs the snapshot and visibility rules |
| K04 | `RowLockPlacement` | `engine::lock` | Choose lock shard/key stripe | Same logical row maps to a host-validated lock key |
| K05 | `RowLockWait` | `engine::lock` | Backoff, wake, timeout hint | FIFO/safety and timeout caps remain host-owned |
| K06 | `CommitBarrier` | `engine::runtime::commit` | Strict/normal/dev barrier plan within config | Commit publish and CSN rules remain host-owned |
| K07 | `WalBatching` | `wal::manager` | Group-commit batch size and wake policy | WAL order, LSN allocation, and flush result validation |
| K08 | `WalCombiner` | `wal::combiner` | Combine/drop redundant pre-commit records if safe | Commit records and durability cannot be removed |
| K09 | `WalSegmentRotation` | `wal::segment` / storage | Rotate/recycle thresholds | Segment format and replay scan remain host-owned |
| K10 | `RecoveryReplay` | `engine::recovery` | Replay scheduling and prefetch | Recovery target and WAL validity remain host-owned |
| K11 | `BufferReplacement` | `storage::buffer` | Victim preference among eligible frames | Pinned/dirty-before-durable frames are never evicted |
| K12 | `DirtyFlush` | `storage::buffer`, checkpoint | Flush batch size/order | Page LSN <= durable LSN rule remains host-owned |
| K13 | `PageReuse` | `engine::page_heap` | Prefer reusable page or allocate fresh | Page kind, generation, and free-space check enforced |
| K14 | `HeapLanePlacement` | `engine::page_heap` | Append/read lane index | Lane is clamped; row directory updated by host |
| K15 | `UndoRead` | `engine::page_heap::mutation::read` | Prefetch/depth-limit hints | Visibility result comes from host MVCC |
| K16 | `VacuumPrune` | `engine::maintenance`, index maintenance | Pick candidates and batch size | Oldest-active snapshot horizon remains host-owned |
| K17 | `IndexSearch` | `index::lookup`, `index::cursor` | Descent/search strategy | Key comparison and page validation remain host-owned |
| K18 | `IndexSplit` | `index::mutate` | Split pivot/fill-factor hints | B-tree structural invariants remain host-owned |
| K19 | `IndexScan` | `index::cursor`, SQL index access | Cursor batch/prefetch policy | Range bounds and row visibility remain host-owned |
| K20 | `UniqueKeyLock` | `index::locks` | Unique-key lock shard/wait hint | Unique outcome and conflict semantics remain host-owned |
| K21 | `VectorDispatch` | `vector::distance`, `vector::simd` | Scalar/SIMD/kernel selector | Distance semantics and NaN handling remain host-owned |
| K22 | `VectorTopK` | `vector::flat`, HNSW/DiskANN | Candidate batch/pruning hint | Final top-k must be host-verified |
| K23 | `JsonPath` | `kernel::json` | JSON-path lookup acceleration | JSON semantic result remains host-owned |
| K24 | `Telemetry` | `telemetry` | Sampling, counter routing, histograms | Counters never affect database correctness |

### 4.2 SQL stages: 15

| # | Stage kind | Current code area | Decision exposed | Host-enforced invariant |
|---:|---|---|---|---|
| S01 | `SqlTextSplit` | `sql::parser.rs` | Fast path for blank/single statement/splitting | Parser remains final authority |
| S02 | `StatementCache` | `connection::cache` | Cache admission/eviction | Prepared statement semantics remain host-owned |
| S03 | `BinderMemo` | `parser::bind` | Memoization and lookup order | Binder still validates names and schema epoch |
| S04 | `PredicateNormalize` | `planner`, `exec::expr` | Constant folding and predicate ordering | Three-valued SQL semantics preserved |
| S05 | `AccessPathEnumerate` | `planner::access` | Which legal access paths to consider | Host checks schema/index compatibility |
| S06 | `SelectivityEstimate` | `planner::helpers`, catalog stats | Selectivity/cardinality estimate | Estimate never determines correctness alone |
| S07 | `JoinOrder` | `planner::optimize` | Join search strategy and pruning | Only associative/commutative legal reorderings |
| S08 | `PlanCost` | `planner` | Cost/tie-breaker model | Plan must be executable and semantically equivalent |
| S09 | `QueryMemory` | `batch`, `exec::vec::spill` | Batch rows/spill thresholds | Host enforces memory caps and spill safety |
| S10 | `Projection` | `exec::tail_build`, `exec::expr` | Materialize vs late projection | Output shape/order remains host-owned |
| S11 | `Aggregate` | `exec::agg`, `exec::vec::hash_agg` | Streaming/hash/sort aggregate choice | Aggregate semantics remain host-owned |
| S12 | `SortTopK` | `exec::vec::sort`, `select_top` | Full sort vs heap/top-k choice | ORDER BY and LIMIT semantics remain host-owned |
| S13 | `IndexDmlBatch` | `exec::index_dml`, `index_batch` | Batch/order index mutations | Transaction undo/repair remains host-owned |
| S14 | `ForeignKeySchedule` | `exec::fk*` | Lookup and cascade scheduling | FK constraint outcome remains host-owned |
| S15 | `WindowFrame` | `exec::window` | Frame execution strategy | Window result semantics remain host-owned |

### 4.3 Deliberately excluded surfaces

These should not be v1 stages:

- Parser grammar mutation.
- FFI ABI layout or return-code mapping.
- Page, tuple, WAL, control-file, catalog, or index durable encodings.
- Transaction status publication.
- Checksum implementations.
- System catalog semantics.
- Collation semantics for built-in collations.
- SQL function semantics, except as separately versioned UDFs.
- Arbitrary filesystem and network IO.

---

## 5. Standard interface

The patch adds a minimal manifest/audit interface first. Subsequent phases can wire trait objects into runtime hot paths without changing the vocabulary.

### 5.1 Core types

```rust
pub const STAGE_CONTRACT_VERSION: u16 = 1;

pub enum StageFamily {
    Kernel,
    Sql,
}

pub enum StageKind {
    EngineAdmission,
    TxAdmission,
    SnapshotRefresh,
    // ...
    WindowFrame,
}

pub enum StageGenomeScope {
    Kernel,
    Sql,
    Full,
}

pub struct StageBinding {
    pub kind: StageKind,
    pub id: String,
    pub contract_version: u16,
    pub enabled: bool,
}

pub struct StageGenome {
    pub scope: StageGenomeScope,
    pub bindings: Vec<StageBinding>,
}

pub struct StageAuditReport {
    pub valid: bool,
    pub errors: Vec<StageAuditError>,
}
```

The interface is intentionally metadata-first. A generated stage can target a stable `StageKind` before the hot loop is wired. The audit tool can then reject duplicate, missing, wrong-version, empty-ID, or out-of-scope bindings.

### 5.2 Decision types

Each runtime stage should expose a small decision type with a clamp/validate method. Examples:

```rust
pub struct LaneDecision {
    pub lane: usize,
}

impl LaneDecision {
    pub fn clamp(self, lane_count: usize) -> Self {
        Self { lane: self.lane % lane_count.max(1) }
    }
}
```

```rust
pub struct FlushDecision {
    pub max_pages: usize,
    pub prefer_oldest_lsn: bool,
}
```

```rust
pub struct CommitBarrierDecision {
    pub flush_wal: bool,
    pub wait_for_write: bool,
}
```

A stage should never return a raw page pointer, WAL handle, transaction object, or mutable reference into engine state.

### 5.3 Stage trait pattern

Each family should have a marker trait plus narrow subtraits:

```rust
pub trait KernelStage: Send + Sync + 'static {
    fn binding(&self) -> StageBinding;
}

pub trait HeapLanePlacementStage: KernelStage {
    fn choose_heap_lane(&self, input: HeapLaneInput) -> LaneDecision;
}
```

SQL stages follow the same pattern:

```rust
pub trait SqlStage: Send + Sync + 'static {
    fn binding(&self) -> StageBinding;
}
```

The host registry can later store typed `Arc<dyn HeapLanePlacementStage>`, `Arc<dyn PlanCostStage>`, etc.

---

## 6. Composition model

### 6.1 Genome shape

A genome is a set of `StageBinding`s with a declared scope. For production database opens, a `Full` genome must bind every kernel and SQL stage. For kernel-only benches, a `Kernel` genome is enough. For planner-only experiments, a `Sql` genome is enough.

### 6.2 Default genome

The default genome binds every stage kind to `redlinedb::builtin::<canonical-name>`. This is the baseline control group for benchmarking.

### 6.3 Compatibility

The `StageKind` list is a stable vocabulary. A breaking change increments `STAGE_CONTRACT_VERSION`. A non-breaking addition can add a new `StageKind`, but a full genome audit must then fail until the genome explicitly binds it. That prevents silent partial experiments.

### 6.4 Determinism

Stages may receive a deterministic seed from the harness, but should not use process randomness. Evolution can explore randomized implementations by materializing the random choices into stage parameters, not by using runtime entropy.

### 6.5 Fallback

The host must always have a built-in fallback. Stage invocation should be wrapped:

1. call stage;
2. validate/clamp output;
3. if invalid, increment telemetry and use fallback;
4. optionally mark the genome as degraded.

No runtime stage failure should corrupt persistent state.

---

## 7. Stage-by-stage contract detail

### K01 EngineAdmission

**Input:** requested `EngineConfig`, filesystem capabilities, volatile/durable mode.
**Output:** normalized config or rejection reason.
**Evolution target:** lock shard count, heap lanes, page cache size, WAL settings.
**Invariant:** page size and file names are host-validated; unsupported values are rejected before open.

### K02 TxAdmission

**Input:** isolation level, active transaction count, lock pressure, WAL backlog.
**Output:** admit now, queue hint, or fail-fast hint.
**Evolution target:** adaptive admission under contention.
**Invariant:** unsupported isolation cannot be smuggled through.

### K03 SnapshotRefresh

**Input:** current isolation, read/write operation kind, observed epoch.
**Output:** refresh now / keep snapshot.
**Evolution target:** reduce read-committed refresh overhead while preserving visibility.
**Invariant:** only the host constructs snapshots.

### K04 RowLockPlacement

**Input:** relation ID, row ID, lock shard count.
**Output:** shard index.
**Evolution target:** contention-aware hashing.
**Invariant:** shard is clamped; logical row identity remains unchanged.

### K05 RowLockWait

**Input:** wait depth, elapsed time, busy timeout, observed contention counters.
**Output:** sleep/yield/spin hint and next wait cap.
**Evolution target:** lower tail latency under multi-writer workloads.
**Invariant:** host enforces timeout and queue correctness.

### K06 CommitBarrier

**Input:** durability mode, append end-LSN, pending schema/index work.
**Output:** write-only, flush, or unsafe-dev barrier plan within config.
**Evolution target:** group commit latency/throughput tradeoff.
**Invariant:** publish happens only after host-approved barrier.

### K07 WalBatching

**Input:** pending WAL queue length, oldest LSN, durability mode, observed fsync cost.
**Output:** batch size and wake policy.
**Evolution target:** amortize fsync without harming latency too much.
**Invariant:** WAL order and LSN allocation are host-owned.

### K08 WalCombiner

**Input:** transaction-local WAL record sequence before commit.
**Output:** safe combined sequence.
**Evolution target:** drop overwritten pre-commit deltas, combine repeated page images.
**Invariant:** no commit record is removed; replay equivalence is verified by host.

### K09 WalSegmentRotation

**Input:** current segment size, checkpoint horizon, filesystem hints.
**Output:** rotate/recycle hint.
**Evolution target:** reduce segment churn and recovery scan cost.
**Invariant:** segment format and replay validity remain host-owned.

### K10 RecoveryReplay

**Input:** scanned WAL record metadata and recovery target.
**Output:** replay order/prefetch hints.
**Evolution target:** faster recovery by grouping page images or relations.
**Invariant:** host applies only valid records up to target.

### K11 BufferReplacement

**Input:** frame metadata, pin counts, dirty status, LSN, access counters.
**Output:** victim preference.
**Evolution target:** beat CLOCK under mixed OLTP/scan workloads.
**Invariant:** pinned and unsafe dirty frames cannot be evicted.

### K12 DirtyFlush

**Input:** dirty page list metadata, durable LSN, checkpoint goal.
**Output:** flush batch size/order hint.
**Evolution target:** reduce write amplification and checkpoint stalls.
**Invariant:** host filters by durable LSN and validates flush result.

### K13 PageReuse

**Input:** page kind, encoded length, reusable queue depth.
**Output:** prefer reusable / allocate fresh.
**Evolution target:** improve locality and reduce fragmentation.
**Invariant:** page kind, generation, and free-space check remain host-owned.

### K14 HeapLanePlacement

**Input:** row ID, relation ID, lane count, operation kind.
**Output:** lane index.
**Evolution target:** reduce append lane contention and false sharing.
**Invariant:** host clamps lane and updates row directories.

### K15 UndoRead

**Input:** undo pointer, depth, snapshot shape, owner transaction.
**Output:** prefetch next and depth-limit hint.
**Evolution target:** speed long undo-chain reads.
**Invariant:** host determines visibility.

### K16 VacuumPrune

**Input:** vacuum horizon, relation/index stats, dirty budget.
**Output:** candidate batch and order.
**Evolution target:** reduce bloat without hurting foreground latency.
**Invariant:** host checks oldest-active snapshot horizon.

### K17 IndexSearch

**Input:** key range, tree height, page fanout stats.
**Output:** descent/prefetch strategy.
**Evolution target:** fewer cache misses in lookup/range scans.
**Invariant:** host does key comparisons and validates pages.

### K18 IndexSplit

**Input:** page occupancy, insertion key position, workload counters.
**Output:** split pivot and fill-factor hint.
**Evolution target:** improve write amplification and scan locality.
**Invariant:** host enforces B-tree structural validity.

### K19 IndexScan

**Input:** range bounds, limit, projected columns, scan counters.
**Output:** batch size and prefetch hint.
**Evolution target:** reduce per-row cursor overhead.
**Invariant:** range bounds and tuple visibility remain host-owned.

### K20 UniqueKeyLock

**Input:** encoded key hash, table/index ID, contention counters.
**Output:** lock shard and wait hint.
**Evolution target:** lower unique-insert contention.
**Invariant:** unique conflict result remains host-owned.

### K21 VectorDispatch

**Input:** metric, dimension, CPU feature flags, batch shape.
**Output:** scalar/SIMD/kernel selection.
**Evolution target:** choose best vector kernel by workload and CPU.
**Invariant:** host verifies distance semantics in debug/shadow modes.

### K22 VectorTopK

**Input:** k, candidate count, metric, filter selectivity.
**Output:** candidate batch/pruning hint.
**Evolution target:** reduce brute-force top-k cost.
**Invariant:** final result can be host-verified exactly when configured.

### K23 JsonPath

**Input:** JSONB shape metadata and path tokens.
**Output:** lookup accelerator hint.
**Evolution target:** reduce repeated JSON path traversal.
**Invariant:** host returns canonical JSON result.

### K24 Telemetry

**Input:** counter event metadata, sample budget.
**Output:** sample/drop/histogram bucket.
**Evolution target:** lower telemetry overhead while preserving useful evidence.
**Invariant:** telemetry cannot influence data-state correctness.

### S01 SqlTextSplit

**Input:** SQL text.
**Output:** blank/single/multiple preliminary split hint.
**Evolution target:** reduce parser calls for common statements.
**Invariant:** parser remains final authority.

### S02 StatementCache

**Input:** SQL fingerprint, schema epoch, parameter shape, cache pressure.
**Output:** cache admit/evict hint.
**Evolution target:** improve hit rate and reduce stale work.
**Invariant:** prepared template is schema-validated before use.

### S03 BinderMemo

**Input:** schema epoch, table/column identifiers, search path.
**Output:** memo hit/candidate hint.
**Evolution target:** speed repeated prepares.
**Invariant:** binder verifies all names.

### S04 PredicateNormalize

**Input:** expression tree metadata and available stats.
**Output:** reordered/folded candidate expression.
**Evolution target:** unlock index access and short-circuit wins.
**Invariant:** host checks three-valued logic equivalence for allowed rewrites.

### S05 AccessPathEnumerate

**Input:** table, predicates, available indexes, projection.
**Output:** legal access path candidates.
**Evolution target:** include better candidate paths without exploding search.
**Invariant:** host validates each candidate against schema.

### S06 SelectivityEstimate

**Input:** column stats, predicate kind, histograms/MCVs.
**Output:** cardinality/selectivity estimate.
**Evolution target:** better plans.
**Invariant:** estimate affects performance, not correctness.

### S07 JoinOrder

**Input:** join graph, legal join predicates, stats, search budget.
**Output:** candidate join order.
**Evolution target:** avoid bad join orders under complex queries.
**Invariant:** host checks legality of reordering.

### S08 PlanCost

**Input:** access path costs, cardinalities, memory, index shape.
**Output:** numeric cost and tie-breaker.
**Evolution target:** better plan selection.
**Invariant:** selected plan must be executable and equivalent.

### S09 QueryMemory

**Input:** configured work memory, active operators, spill state.
**Output:** batch rows and spill threshold hints.
**Evolution target:** reduce spills and memory stalls.
**Invariant:** host enforces caps.

### S10 Projection

**Input:** projection list, row width, predicate pushdown, operator shape.
**Output:** eager/late/materialize choice.
**Evolution target:** reduce copying.
**Invariant:** output columns and order remain host-owned.

### S11 Aggregate

**Input:** grouping keys, aggregate functions, estimated rows, memory.
**Output:** streaming/hash/sort strategy.
**Evolution target:** choose faster aggregate algorithm.
**Invariant:** aggregate semantics and NULL handling remain host-owned.

### S12 SortTopK

**Input:** ordering keys, limit/offset, row count estimate, memory.
**Output:** full-sort, partial-sort, heap top-k, or streaming hint.
**Evolution target:** reduce ORDER BY/LIMIT cost.
**Invariant:** ORDER BY semantics remain host-owned.

### S13 IndexDmlBatch

**Input:** pending row changes, index count, uniqueness constraints.
**Output:** batch order and chunk size.
**Evolution target:** lower index DML overhead.
**Invariant:** undo/repair and uniqueness remain host-owned.

### S14 ForeignKeySchedule

**Input:** FK graph, pending DML, deferrability, cascade depth.
**Output:** lookup/cascade schedule.
**Evolution target:** reduce redundant FK checks.
**Invariant:** constraint outcome remains host-owned.

### S15 WindowFrame

**Input:** partition shape, ordering, frame type, memory.
**Output:** streaming/materialized frame strategy.
**Evolution target:** reduce memory and repeated frame scans.
**Invariant:** window function semantics remain host-owned.

---

## 8. Implementation plan

### Phase 0: manifest and audit only

This patch implements Phase 0:

- Add `crates/kernel/src/stage.rs`.
- Export `pub mod stage;` from `redlinedb-kernel`.
- Add `crates/sql/src/stage.rs`.
- Export `pub mod stage;` from `redlinedb-sql`.
- Add this engineering spec under `docs/architecture/STAGE_GENOME_SPEC.md`.

No runtime behavior changes in Phase 0. That is intentional. It creates stable names and audit mechanics before agents start writing replacement code.

### Phase 1: wire existing kernel policy seams

Wire the lowest-risk kernel stages first:

1. `HeapLanePlacement`
2. `PageReuse`
3. `UndoRead`
4. `DirtyFlush`
5. `CommitBarrier`
6. `WalBatching`
7. `BufferReplacement`

The existing `engine/page_heap/policy.rs` already resembles this pattern and should be migrated behind the public stage vocabulary.

### Phase 2: planner and executor stages

Wire SQL stages that cannot change semantics:

1. `StatementCache`
2. `PredicateNormalize`
3. `SelectivityEstimate`
4. `AccessPathEnumerate`
5. `JoinOrder`
6. `PlanCost`
7. `QueryMemory`
8. `SortTopK`
9. `Aggregate`

For early evolution, planner stages are the best performance target because wrong choices are usually slow, not corrupting.

### Phase 3: generated stage crates

Introduce a generated-stage crate ABI:

- `redlinedb-stage-api` or re-exported kernel/sql stage APIs.
- One crate per candidate gene.
- Compile-time registration first.
- Dynamic loading only after a sandbox model exists.

### Phase 4: genome benchmark harness

Extend `crates/bench` to accept:

```text
--stage-genome genome.json
--stage-report out/stage-report.json
--stage-shadow builtin
```

The harness should emit:

- audit report,
- per-stage fallback count,
- per-stage invocation count,
- benchmark latency/throughput,
- correctness suite result,
- crash/failpoint result,
- deterministic replay hash.

---

## 9. Safety and validation matrix

Every stage candidate must pass these gates.

### 9.1 Static gate

- `cargo check --workspace`
- `cargo test -p redlinedb-kernel stage`
- `cargo test -p redlinedb-sql stage`
- boundary lint: no forbidden imports in generated stage code
- no `unsafe` unless separately approved and quarantined
- no filesystem/network/process access
- no nondeterministic time or random source

### 9.2 Stage audit gate

- genome has required stage kinds
- exactly one enabled binding per kind
- contract version matches
- binding IDs are non-empty and canonical
- any stage-specific parameter ranges are valid
- declared exclusions do not conflict

### 9.3 Property gate

Per stage:

- output is always within host-clampable range
- deterministic for same input
- fallback equivalence holds
- panics are caught in tests as failures
- bounded allocation and bounded runtime

### 9.4 SQL correctness gate

- parser/binder equivalence for statement-cache and split stages
- SQLite parity corpus
- generated DML/query suites
- join/order/aggregate/window semantic tests
- result-order tests where ordering is required

### 9.5 Kernel correctness gate

- MVCC isolation tests
- row-lock contention tests
- crash-recovery matrix
- failpoint matrix
- WAL torn-tail tests
- index uniqueness and vacuum tests
- page/checksum/integrity checks

### 9.6 Benchmark gate

A candidate should not be promoted unless it improves at least one target class without a significant regression in protected classes. Suggested target classes:

- point lookup
- insert-only
- update hot row
- update disjoint rows
- range scan
- index lookup
- join-heavy
- aggregate-heavy
- ORDER BY/LIMIT
- JSON path
- vector top-k
- crash recovery time

---

## 10. Genome evolution workflow

1. **Baseline.** Build and benchmark default genome.
2. **Mutation.** Agent generates one or more stage candidates targeting a single `StageKind`.
3. **Compile.** Candidate crate compiles against the stage API.
4. **Audit.** Candidate and composed genome pass manifest audit.
5. **Microbench.** Candidate runs against stage-specific microbench.
6. **Correctness.** Full correctness matrix runs.
7. **Shadow.** Candidate runs in shadow mode beside built-in for selected decisions.
8. **Macrobench.** End-to-end workloads run.
9. **Rank.** Genome is ranked by performance, correctness, determinism, and stability.
10. **Archive.** Candidate metadata and results are stored for future recombination.

Important: start with one mutated stage per genome until the harness is stable. Then allow pairwise combinations, then N-way combinations.

---

## 11. Patch overview

The attached diff implements the safe foundation:

- `docs/architecture/STAGE_GENOME_SPEC.md`: this specification.
- `crates/kernel/src/stage.rs`: stable stage taxonomy, genome model, audit, and kernel-stage decision wrappers.
- `crates/kernel/src/lib.rs`: exports `stage`.
- `crates/sql/src/stage.rs`: SQL-stage facade and SQL-specific decision wrappers.
- `crates/sql/src/lib.rs`: exports `stage`.

The diff does not wire stages into runtime paths yet. That is intentional. The first merge should create the standard vocabulary and make it impossible for agents to invent incompatible surfaces.

---

## 12. Why not expose more than 39 now?

There are more than 39 possible optimization points, but many are either aliases of the same decision or unsafe in combination. For example:

- “tuple encoding stage” and “WAL encoding stage” look attractive but create corruption risk.
- “collation stage” changes SQL semantics.
- “parser grammar stage” changes the accepted language and can break compatibility.
- “CSN allocation stage” breaks MVCC and recovery.
- “checkpoint file writer stage” risks losing committed data.
- “B-tree compare stage” risks index corruption.

The proposed 39 maximize performance-relevant degrees of freedom while keeping correctness centralized.

---

## 13. Expected performance opportunities

Highest-value early targets:

1. `WalBatching` + `CommitBarrier`: group commit throughput and p99 latency.
2. `BufferReplacement` + `DirtyFlush`: scan/write mixed workloads.
3. `HeapLanePlacement` + `PageReuse`: concurrent insert/update locality.
4. `SelectivityEstimate` + `JoinOrder` + `PlanCost`: complex query performance.
5. `SortTopK` + `Aggregate`: analytical queries and SQLite parity laggards.
6. `IndexDmlBatch`: multi-index DML workloads.
7. `VectorDispatch` + `VectorTopK`: vector workloads.

Lowest-risk first evolutionary playground: SQL planner cost/estimate stages. Highest-risk but high reward: WAL/commit and buffer replacement.

---

## 14. Acceptance criteria for Phase 0 patch

- The new stage modules compile.
- Built-in kernel, SQL, and full genomes pass audit.
- Duplicate stage bindings fail audit.
- Missing required stage bindings fail audit.
- Wrong contract versions fail audit.
- No runtime behavior changes.
- No public existing API is broken except adding new modules.
- Documentation clearly identifies protected invariants.

---

## 15. Acceptance criteria for Phase 1 wiring

For each wired stage:

- Built-in stage exactly preserves current behavior.
- Stage output is clamped and validated before use.
- Stage failures fall back to built-in.
- At least one test demonstrates fallback on invalid output.
- At least one benchmark includes per-stage counters.
- Shadow-mode comparison exists for correctness-sensitive decisions.

---

## 16. Suggested JSON genome format

```json
{
  "scope": "Full",
  "contract_version": 1,
  "bindings": [
    {
      "kind": "HeapLanePlacement",
      "id": "redlinedb::builtin::heap-lane-placement",
      "contract_version": 1,
      "enabled": true
    },
    {
      "kind": "PlanCost",
      "id": "agent-lab::plan-cost::v17",
      "contract_version": 1,
      "enabled": true
    }
  ]
}
```

The JSON loader should reject duplicate stage kinds and missing required kinds. It should also support a “patch genome” that overlays a few bindings onto the default genome for experiments, but persisted benchmark results should always store the expanded full genome.

---

## 17. Summary recommendation

Merge the Phase 0 patch first. Then wire stages in this order:

1. Kernel heap policy seams: `HeapLanePlacement`, `PageReuse`, `UndoRead`.
2. SQL planner seams: `SelectivityEstimate`, `JoinOrder`, `PlanCost`, `AccessPathEnumerate`.
3. WAL/buffer seams: `WalBatching`, `CommitBarrier`, `BufferReplacement`, `DirtyFlush`.
4. Executor seams: `QueryMemory`, `SortTopK`, `Aggregate`, `Projection`.
5. Index/vector/JSON specialized seams.

This creates a large enough genome to evolve real database performance while keeping the dangerous invariants out of generated code.
