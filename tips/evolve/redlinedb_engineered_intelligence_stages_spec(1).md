
# RedlineDB Engineered-Intelligence Stage Standardization Spec

**Target repo:** `neverhuman/RedlineDB` / default branch `main`  
**Purpose:** introduce a safe, evolvable “stage genome” for RedlineDB so reasoning agents can generate replacement implementations behind standardized interfaces, then benchmark combinations without producing invalid/stillborn databases.  
**Patch stance:** conservative first implementation. It standardizes the interfaces, names, compatibility classes, and open-time genome fingerprinting while leaving the current implementation as the default genome. This keeps all existing behavior unchanged and prevents untracked stage mixes from sharing a live process-global database entry.

---

## 1. Executive recommendation

The maximum safe decomposition I recommend for RedlineDB right now is **39 stage slots**, grouped into **24 gene families**.

That is the practical upper bound because below these boundaries the code starts crossing into invariants that cannot be composed independently: page bytes, WAL record bytes, MVCC truth tables, schema epoch publication, and recovery replay must move together or be versioned together. Splitting beneath those seams would generate many candidate stages, but too many would be stillborn because they could create data that another stage cannot read, replay, lock, or make visible.

The correct architecture is therefore two-layered:

1. **Replacement-ready stage slots:** narrow policies already represented by current private traits or constants. These can be swapped aggressively because their outputs are bounded and easy to validate.
2. **Coordinated-replacement stage slots:** important performance surfaces that can become stages, but only if the genome declares a compatibility family. These can still evolve, but they must be replaced as a coordinated bundle or guarded by a format/semantic epoch.

This patch adds the foundation for that architecture:

- `redlinedb_kernel::stages::KernelStageGenome`
- `redlinedb_sql::stages::SqlStageGenome`
- public `redlinedb::stages::StageGenome`
- per-stage `StageGene` metadata with safety class, contract version, implementation ID, and parameter hash
- open-time/fingerprint plumbing so a process cannot accidentally reuse an already-open database with a different genome
- `OpenOptions::with_stage_genome(...)`
- public `Database::stage_genome()` for benchmark manifests
- documented stage contracts and evolution gates

Default behavior remains exactly the current implementation. The default genome only names the existing behavior.

---

## 2. What I found in the repo

RedlineDB is already shaped in a way that supports stage extraction. The important split is:

- `crates/kernel`: storage kernel primitives, catalog, engine, format, heap, index, IO, storage, telemetry, txn, vector, WAL.
- `crates/sql`: batch, connection, exec, parser, planner, statement, session, value, UDF surfaces.
- `crates/redlinedb`: public facade, open options, registry, handle, pool, snapshot, value, metrics, phase8 APIs.

The most important observation is that several “future stage” seams already exist privately:

- heap placement / undo read policy in `crates/kernel/src/engine/page_heap/policy.rs`
- buffer policy in `crates/kernel/src/storage/policy.rs`
- WAL scheduling in `crates/kernel/src/wal/policy.rs`
- SQL planner policy in `crates/sql/src/planner/policy.rs`
- SQL executor batch policy in `crates/sql/src/exec/policy.rs`

Those are exactly the right first genes. The patch promotes their identity into a standard, benchmarkable genome without immediately hot-swapping arbitrary code pointers at runtime.

---

## 3. Non-stillborn definition

A stage combination is **non-stillborn** if all of the following are true:

1. It compiles against the standard trait/interface surface.
2. It does not change page bytes or WAL bytes unless it declares a new format epoch.
3. It does not make a tuple visible if the current MVCC contract would make it invisible, unless the whole MVCC/recovery/commit compatibility group is replaced together.
4. It never returns an invalid page, row ID, LSN, CSN, tuple pointer, undo pointer, catalog epoch, or index key range.
5. It can open, recover, checkpoint, vacuum, and run the SQLite parity corpus with the same SQL-visible behavior as the default genome.
6. It can be uniquely identified in benchmark output and process-global open fingerprints.

The key lesson: **“drop-in replacement” does not mean “any code can replace any function.”** It means a replacement implements a stable contract with typed inputs, bounded outputs, and declared compatibility.

---

## 4. The 39 safe stage slots

### 4.1 Replacement-ready slots

These are narrow and can be evolved immediately because they choose among bounded alternatives and do not own durable bytes.

| # | Stage slot | Current anchor | Standard input | Standard output | Hard invariant |
|---:|---|---|---|---|---|
| 1 | `heap.row_lane` | `page_heap/policy.rs` / `row_lane` | `RowId`, lane count | lane index | `lane < lane_count` |
| 2 | `heap.relation_lane` | `page_heap/policy.rs` / `relation_lane` | `RelId`, lane count | lane index | `lane < lane_count` |
| 3 | `heap.reusable_page_choice` | `page_heap/policy.rs` / `reusable_page` | page kind, encoded length, queue depth | reuse/fresh | cannot return non-heap/undo reusable page |
| 4 | `heap.undo_read_policy` | `page_heap/mutation/read.rs` | undo depth, pointer | depth limit and prefetch hint | cannot skip visible tuple versions |
| 5 | `buffer.victim_scoring` | `storage/policy.rs` | frame metadata | optional victim score | cannot evict pinned, loading, or WAL-unsafe dirty frame |
| 6 | `buffer.prefetch_gate` | `storage/buffer.rs` / `prefetch` | resident pages, capacity | allow/drop cold load | hint-only; correctness cannot depend on warming |
| 7 | `buffer.dirty_batch_size` | `storage/policy.rs` | resident/dirty counts | batch page count | must be at least 1 when dirty pages exist |
| 8 | `buffer.dirty_flush_order` | `storage/policy.rs` | dirty frames | sorted dirty frames | cannot drop or duplicate frames |
| 9 | `wal.write_batch_bytes` | `wal/policy.rs` | pending bytes/records/config | write batch bytes | positive, bounded by memory guard |
| 10 | `wal.group_commit_delay` | `wal/policy.rs` | flush gap/pending/config | delay μs | never exceeds configured max delay |
| 11 | `wal.flush_resample` | `wal/policy.rs` | WAL schedule context | boolean | hint-only; cannot move durable LSN backwards |
| 12 | `wal.drain_batch_bytes` | `wal/policy.rs` | WAL schedule context | drain bytes | positive, bounded by max batch |
| 13 | `planner.join_kind` | `planner/policy.rs` | join cardinality/indexability hints | join kind | output must be supported by executor |
| 14 | `planner.aggregate_kind` | `planner/policy.rs` | input rows/group/order | physical aggregate kind | output must be supported by executor |
| 15 | `planner.ordering_kind` | `planner/policy.rs` | limit/order hints | sort vs TopN | output must preserve SQL order semantics |
| 16 | `exec.row_batch_policy` | `exec/policy.rs` | memory/config | row/index batch sizes | positive, bounded by work memory |
| 17 | `exec.materialize_capacity` | `exec/policy.rs` | row width and memory budget | row count | at least 1, no budget overflow |

### 4.2 Coordinated-replacement slots

These are valuable performance surfaces, but they must be coordinated. An agent can invent replacements here, but the genome must declare that related semantic/format families match.

| # | Stage slot | Why it matters | Required compatibility group |
|---:|---|---|---|
| 18 | `mvcc.tuple_visibility` | hot-path reads, update/write visibility | MVCC + tx status + recovery |
| 19 | `txn.status_frontier` | snapshot construction, CSN reservation, publish visibility | MVCC + commit + recovery |
| 20 | `mvcc.write_conflict_resolution` | update/delete contention and serialization failures | MVCC + row locking |
| 21 | `locks.row_keying` | row-lock shard distribution | lock manager + tx row-lock tracking |
| 22 | `locks.wait_backoff` | tail latency under contention | lock manager only, but must preserve timeout semantics |
| 23 | `heap.row_id_allocation` | insert throughput and locality | heap directory + SQL rowid semantics |
| 24 | `heap.append_page_selection` | page density, write amplification | heap page format + buffer allocation |
| 25 | `heap.undo_record_semantics` | undo chain pruning/readability | MVCC + vacuum + recovery |
| 26 | `commit.durability_barrier` | strict/normal/unsafe commit latency | WAL + tx publish ordering |
| 27 | `recovery.commit_filter` | point-in-time recovery and torn-tail handling | WAL payload + tx status |
| 28 | `recovery.heap_redo` | crash recovery speed | heap page image/delta semantics |
| 29 | `recovery.index_redo` | index crash consistency | index WAL + index maintenance |
| 30 | `recovery.catalog_replay` | schema recovery | catalog snapshot codec + commit filter |
| 31 | `catalog.snapshot_codec` | schema load/save speed and size | catalog format epoch |
| 32 | `catalog.schema_epoch_publish` | DDL visibility | catalog manager + tx publish |
| 33 | `index.probe_strategy` | SELECT/index lookup speed | B-tree comparator + row visibility recheck |
| 34 | `index.maintenance_delta` | INSERT/UPDATE/DELETE index cost | index WAL + recovery |
| 35 | `checkpoint.batch_pacing` | checkpoint stalls and writeback shape | buffer + WAL durable LSN |
| 36 | `vacuum.prune_and_reuse` | space reuse and undo cleanup | MVCC horizon + heap reuse |
| 37 | `sql.access_path_selection` | scan/index/multi-index choice | planner + executor index-access support |

### 4.3 Hook-only slots

These are important to name, but not safe to replace independently yet. They are hook-only until RedlineDB adds explicit durable format epochs.

| # | Hook | Why not replacement-ready yet | Safe near-term use |
|---:|---|---|---|
| 38 | `format.page_codec` | page headers, checksums, tuple pointers, generation, and direct page writes must agree globally | observe/validate/feature-gate only |
| 39 | `wal.record_codec` | recovery, segment scanning, commit records, page images, and logical payloads must agree globally | observe/validate/feature-gate only |

---

## 5. Gene families in the patch

The patch groups the 39 slots into gene families because this avoids over-fragmentation while preserving the important replacement boundaries.

### Kernel genome families

`KernelStageGenome` includes:

1. `tuple_visibility`
2. `tx_status_frontier`
3. `write_conflict`
4. `row_locking`
5. `row_id_allocation`
6. `heap_placement`
7. `reusable_page`
8. `undo_read`
9. `buffer_residency`
10. `dirty_flush`
11. `wal_schedule`
12. `durability_barrier`
13. `recovery_replay`
14. `catalog_codec`
15. `index_access`
16. `index_maintenance`
17. `checkpoint_pacing`
18. `page_format_hook`
19. `wal_format_hook`

### SQL genome families

`SqlStageGenome` includes the kernel genome plus:

1. `access_path_selection`
2. `join_choice`
3. `aggregate_choice`
4. `ordering_choice`
5. `exec_batch`
6. `materialization`
7. `analyze_sampling`
8. `statement_dispatch`

The SQL genome carries the kernel genome because the public open path is SQL-first: `redlinedb::OpenOptions` becomes `redlinedb_sql::DbOptions`, which then constructs `redlinedb_kernel::EngineConfig`.

---

## 6. Why the patch fingerprints the genome at open time

RedlineDB has a process-global registry that reuses live database entries for the same path. That is correct, but stage evolution changes the meaning of performance decisions and eventually could change semantics. Therefore the stage genome must participate in the open fingerprint.

The diff adds `stage_genome` to:

- public `OpenOptions`
- registry `OpenFingerprint`
- SQL `DbOptions`
- kernel `EngineConfig`

This prevents a benchmark worker from doing this accidentally:

1. Open `/tmp/db` with genome A.
2. Open `/tmp/db` with genome B in the same process.
3. Reuse the old live engine silently.
4. Attribute genome A’s result to genome B.

With the patch, that becomes an incompatible-open error unless the genomes match.

---

## 7. Replacement contract rules for agents

Every generated stage must include:

```rust
StageGene {
    family: "kernel.wal_schedule",
    implementation: "agent42.tail_latency.v3",
    contract_version: 1,
    params_hash: 0x...,
    safety: StageSafetyClass::ReplacementReady,
}
```

The `family` says what slot/group the implementation is claiming to replace. The `implementation` says which algorithm is being benchmarked. `contract_version` lets interfaces evolve without ambiguous comparison. `params_hash` allows parameter evolution without inventing a new type name. `safety` states whether the gene can be freely mixed or must move with other compatibility groups.

### Replacement-ready stage acceptance

A replacement-ready stage must pass:

- trait-level audit tests
- boundary tests for min/max inputs
- randomized fuzz tests for outputs inside valid ranges
- full SQLite parity corpus
- RedlineDB crash/recovery tests where applicable
- benchmark smoke run with manifest output

### Coordinated stage acceptance

A coordinated replacement must also declare a compatibility group. For example:

- `mvcc-v1`: tuple visibility, write conflict, tx status, commit publish, recovery filter
- `heap-v1`: append page selection, undo semantics, vacuum, recovery heap redo
- `index-v1`: index probe, maintenance delta, recovery index redo
- `catalog-v1`: catalog codec, schema epoch publish, catalog replay

The harness must reject mixed coordinated genes unless their compatibility group matches.

---

## 8. Benchmark manifest shape

Every benchmark result should record:

```json
{
  "redlinedb_stage_genome": {
    "fingerprint": "0x0123456789abcdef",
    "kernel": {
      "wal_schedule": "kernel.wal_schedule/current.wal_schedule/v1/0/replacement_ready"
    },
    "sql": {
      "join_choice": "sql.join_choice/current.planner/v1/0/replacement_ready"
    }
  },
  "engine": "redline",
  "workload": "sqlite-parity/memory",
  "latency_p50_us": 0,
  "latency_p99_us": 0,
  "rows_per_second": 0,
  "wal_fsyncs": 0,
  "buffer_evictions": 0,
  "correctness": "pass"
}
```

The public `Database::stage_genome()` added by the patch is there to make that manifest easy to populate.

---

## 9. Why I did not recommend more than 39 slots

There are many functions in the codebase that look swappable. Most should not become independent stages. Examples:

- page header encode/decode fields
- WAL record payload layout
- tuple pointer generation semantics
- undo pointer encoding
- commit record meaning
- catalog snapshot byte format
- index key comparator and WAL delta encoding
- checksum and torn-tail interpretation

These are not performance knobs; they are shared language. They can be optimized, but only behind an explicit format epoch and coordinated compatibility group. Making them independent genes would create many combinations that compile but cannot recover or cannot read their own data after restart.

The proposed 39-stage boundary is therefore deliberately maximum-safe, not maximum-count.

---

## 10. Diff summary

The accompanying diff does the following:

1. Adds `crates/kernel/src/stages.rs` with kernel stage gene metadata, safety classes, and kernel genome defaults.
2. Exports `redlinedb_kernel::stages` from `crates/kernel/src/lib.rs`.
3. Adds `stage_genome` to `EngineConfig` and exposes `Engine::stage_genome()`.
4. Adds `crates/sql/src/stages.rs` with SQL stage genome defaults and kernel genome nesting.
5. Exports `redlinedb_sql::stages` from `crates/sql/src/lib.rs`.
6. Adds `stage_genome` to SQL `DbOptions` and propagates kernel genes into `EngineConfig` at database create/open time.
7. Adds `crates/redlinedb/src/stages.rs` as the public re-export surface.
8. Adds `stage_genome` to public `OpenOptions` with `with_stage_genome(...)` setter.
9. Adds `stage_genome` to the registry fingerprint to prevent silent live-engine reuse across genomes.
10. Adds convenience public constructors and `Database::stage_genome()`.

---

## 11. Next implementation waves

### Wave A: make existing private policies implement public stage traits

Start with the existing five policy modules:

- heap placement / undo read
- buffer policy
- WAL schedule
- planner policy
- exec batch policy

Add explicit trait impls and trait-level tests for every current alternative. This will let agents generate new policy implementations and wire them with only a type alias change.

### Wave B: static genome selector

Add cargo features or a generated Rust module such as:

```rust
pub type ActiveHeapPlacementPolicy = generated_genome::HeapPlacement;
pub type ActiveBufferPolicy = generated_genome::BufferPolicy;
```

This gives near-zero runtime overhead while still allowing the evolution harness to produce many genomes.

### Wave C: dynamic registry only for safe hint stages

Use runtime trait objects only for hint-like stages where dispatch overhead is tolerable and correctness cannot depend on side effects: prefetch, batch sizing, dirty flush ordering, WAL delay. Keep MVCC, recovery, and codec stages compile-time or epoch-bound.

### Wave D: coordinated compatibility groups

Only after enough parity/crash tests exist, introduce non-default coordinated MVCC, heap, index, catalog, or WAL families.

---

## 12. Validation plan

The minimum validation gate for any stage candidate:

```bash
cargo fmt --all
cargo test -p redlinedb_kernel
cargo test -p redlinedb_sql
cargo test -p redlinedb
cargo run -p sqlite-parity --release -- --engine both --suite memory
cargo run -p redlinedb-bench --release -- --emit-stage-genome
```

For coordinated stages, add:

```bash
cargo test -p redlinedb_kernel recovery
cargo test -p redlinedb_kernel checkpoint
cargo test -p redlinedb_kernel wal
cargo test -p redlinedb_kernel integrity
```

For format-hook stages, add explicit create/open/recover across old/new binaries before allowing replacement status.

---

## 13. Risk assessment

| Risk | Mitigation in patch |
|---|---|
| Silent benchmark contamination by registry reuse | genome participates in open fingerprint |
| Stillborn random combinations | safety class and coordinated/hook-only labeling |
| Runtime overhead in hot paths | default patch only stores/fingerprints metadata; no dynamic dispatch on tuple/buffer/WAL hot path |
| External API instability | new fields have defaults and fluent setter; existing constructors behave the same |
| Agents changing durable bytes accidentally | page/WAL codecs are hook-only |
| MVCC/recovery mismatch | tuple visibility/tx/recovery are coordinated, not freely mixed |

---

## 14. Bottom line

The best first move is **not** to make every internal function a runtime plugin. The best first move is to make RedlineDB’s current internal policy seams explicit, name every safe future stage, fingerprint the genome, and block untracked cross-genome reuse. That gives the evolution system stable surfaces to target while keeping the default database correct.

This patch gives you that foundation and identifies the maximum safe stage map for the next waves.
