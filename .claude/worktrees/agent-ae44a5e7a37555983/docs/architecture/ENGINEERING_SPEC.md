# RedlineDB Engineering Specification

**Version:** 1.0.1  
**Status:** Living document — updated with each release cycle  
**Scope:** Internal design reference for researchers, external reviewers, and agents performing architectural analysis. This is not a user guide.

---

## Abstract

RedlineDB is a 100% safe-Rust embedded SQL engine that targets the SQLite API contract on the documented compatibility surface — the C ABI shim, the SQL surface, and the embedded deployment model — while replacing SQLite's single-writer WAL storage core with MVCC, a concurrent B-tree, real group-commit WAL, and deterministic crash recovery.

The key design claims:

- **Multi-writer concurrency.** Row-level locks with snapshot isolation allow disjoint-row writes to proceed without serialization at the SQL level.
- **Crash safety.** Every committed transaction is durable before the application receives acknowledgement. The failpoint matrix (24 cases) and recovery matrix (36 cases) verify zero lost acked commits across every crash injection point.
- **SQLite compatibility shim.** `crates/ffi` exports `sqlite3_open`, `sqlite3_prepare_v2`, `sqlite3_step`, and the documented `sqlite3_*` aliases that the compatibility tests cover. The shim is intended for incremental integration and link-time testing, not a blanket claim that every SQLite-linked program can relink without review.
- **Deterministic proof.** The 1,728-child certification matrix produces byte-comparable artifacts across runs given the same git SHA, Docker image digest, and seed.

The codebase is 34,999 active source lines across 8 workspace crates. Test count: 928 passing.

---

## Table of Contents

1. [Crate Architecture & Dependency DAG](#1-crate-architecture--dependency-dag)
2. [Annotated File Tree](#2-annotated-file-tree)
3. [Storage Layer — Pages, Slots, Buffer Pool](#3-storage-layer--pages-slots-buffer-pool)
4. [MVCC & Version Chains](#4-mvcc--version-chains)
5. [WAL & Group Commit](#5-wal--group-commit)
6. [B-Tree Index](#6-b-tree-index)
7. [Catalog & Schema Management](#7-catalog--schema-management)
8. [Transaction Lifecycle](#8-transaction-lifecycle)
9. [Concurrency Control](#9-concurrency-control)
10. [Crash Recovery](#10-crash-recovery)
11. [SQL Pipeline — Parser, Planner, Executor](#11-sql-pipeline--parser-planner-executor)
12. [Connection & Session Model](#12-connection--session-model)
13. [FFI C ABI Surface](#13-ffi-c-abi-surface)
14. [Vector Index](#14-vector-index)
15. [Certification & Proof Infrastructure](#15-certification--proof-infrastructure)
16. [Design Tradeoffs & Known Limitations](#16-design-tradeoffs--known-limitations)
17. [Key Constants Reference](#17-key-constants-reference)

---

## 1. Crate Architecture & Dependency DAG

The workspace is a strict DAG. No back-edges are permitted. `bench` reaches all product crates; no product crate depends on `bench`.

```
redlinedb-domain
      │
redlinedb-kernel
      │
redlinedb-sql
      │
  redlinedb          (public Rust facade)
  ┌────┴─────┐
ffi         cli    server

redlinedb-bench ──► all of the above (dev/test only)
```

### Crate Inventory

| Crate | Active LOC | Responsibility |
|-------|-----------|----------------|
| `redlinedb-domain` | — | Policy-free cross-crate types. Only `DomainError`. No dependencies on other workspace crates. |
| `redlinedb-kernel` | 12,883 | Pages, WAL, MVCC version chains, B-tree index, catalog, crash recovery, vector search, JSONB, failpoints. |
| `redlinedb-sql` | 11,615 | SQL parser (sqlparser-rs SQLiteDialect), cost-based query planner, vectorized executor, index undo log, connection/session model. |
| `redlinedb` | 2,975 | Public Rust facade: `Database`, `Connection`, `Statement`, `Row`, `OpenOptions`. Narrows the kernel+sql surface. |
| `redlinedb-ffi` | 1,478 | C ABI shim exporting `rldb_*` and `sqlite3_*` symbols for ecosystem compatibility. |
| `redlinedb-cli` | — | Interactive REPL (`rustyline`) and batch SQL execution. Eight output modes. |
| `redlinedb-server` | — | TCP server with custom binary protocol (JSON serialization over framed stream). |
| `redlinedb-bench` | 5,144 | Certification harness, failpoint matrix, recovery matrix, cross-engine compat, chaos workloads. |

### Boundary Rules (`agent/boundaries.toml`)

- **No back-edges.** `kernel` must not import `sql`; `sql` must not import `redlinedb`.
- **Forbidden stdlib in kernel/sql.** `std::fs`, `std::net`, `std::time::SystemTime`, `std::process` are blocked to keep the kernel portable and mockable.
- **Forbidden third-party in product crates.** `rand`, `sqlx`, `diesel`, `reqwest`, `rdkafka`, `tracing`, `log` are not allowed as product dependencies.
- **Domain types are leaves.** `redlinedb-domain` has zero workspace dependencies.
- **FFI is one-way.** Only `crates/ffi` is allowed to use `#[no_mangle] extern "C"`.

---

## 2. Annotated File Tree

### `crates/kernel/src/`

```
lib.rs                       Public kernel API; re-exports Engine, Txn, Snapshot, BtreeIndex
catalog/
  mod.rs                     Public catalog interface
  ids.rs                     Typed IDs: ColumnId, ConstraintId, IndexId, TableId, ObjectId, RelId
  schema.rs                  SchemaSnapshot, TableDef, IndexDef, ColumnDef, ConstraintDef, ClassKind
  manager.rs                 CatalogManager: ArcSwap<SchemaSnapshot> + AtomicU64 epoch + DDL mutex
  bootstrap.rs               Initial schema creation on new database
  ddl.rs                     DDL operation specs: CreateTableSpec, CreateIndexSpec, AlterTableSpec
  ops.rs                     DDL application: apply_create_table, apply_drop_index, etc.
  store.rs                   Catalog persistence: encode/decode snapshots to .redline_catalog
  key.rs                     IndexKeyDef, encode_index_key, NullOrder
  record.rs                  Row encoding/decoding in storage format
  affinity.rs                SQL type affinity coercion rules
  expr.rs                    CompiledExpr, eval_expr — expression compilation for index key extraction
  value.rs                   OwnedValue, ValueRef, StorageClass
  names.rs                   DbName, QualifiedName qualified identifiers
  codec.rs                   Value binary encoding/decoding
  stats.rs                   TableStats, ColumnStats structures
  system.rs                  System catalog table definitions
engine/
  mod.rs                     Engine struct: config + buffer + heap + catalog + locks + wal + control
  tx.rs                      ConcurrentTxStatus: sharded tx state, CSN frontier, active snapshot set
  recovery.rs                Crash recovery: WAL scan, redo pass, catalog restore, frontier restore
  lock.rs                    RowLockManager: per-row FIFO Condvar queue, timeout, Phase 11 telemetry
  page_heap.rs               PageBackedHeap: row directories, append lanes, reusable page lists
  page_heap/
    directory.rs             RowId → TuplePtr mapping (per-lane HashMap)
    mutation.rs              Insert/update/delete operations
    mutation/read.rs         Read path (fetch tuple by RowId)
    mutation/write.rs        Write path (apply mutations, create undo records)
  concurrent_heap.rs         ConcurrentHeap: multi-writer wrapper around PageBackedHeap
  runtime.rs                 Transaction execution: commit/rollback lifecycle
  catalog_ops.rs             DDL operations routed through Engine
  maintenance.rs             Vacuum and dead-tuple cleanup
format/
  mod.rs                     Format module re-exports
  ids.rs                     PageId, RelId, RowId, TxId, Csn, Lsn, UndoPtr, TuplePtr, WalSegmentNo
  page.rs                    Page structure: PageHeader (64B), PageKind, PageState, slot array, CRC32
  tuple.rs                   TupleVersion (72B header + payload): visibility fields, undo chain link
  bytes.rs                   Byte-level read/write helpers, CRC32c checksums
storage/
  mod.rs                     Storage module interface
  page_file.rs               PageFile: pread/pwrite abstraction over on-disk page file (Mutex)
  buffer.rs                  BufferPool: CLOCK eviction, frame pinning, dirty tracking, stats
  control.rs                 ControlFile: dual A/B generation write, CRC32, checkpoint_lsn
  tx_status_checkpoint.rs    TxStatusCheckpoint: next_tx, next_csn, published_csn, committed entries
wal/
  mod.rs                     WAL module re-exports
  manager.rs                 WalCoordinator: group-commit interface, WalSyncCounters
  manager/
    coordinator.rs           Core state machine: reserved/written/durable LSN, pending queue, condvar
    storage.rs               Persistent WAL segment file management
  record.rs                  WalRecord: 48B header + payload, CRC32, WalRecordKind enum
  payload.rs                 WalPayload enum: HeapInsert/Update/Delete, IndexInsert/Delete, Commit, etc.
  segment.rs                 SealedWalSegment metadata, segment numbering, seal file format
  lanes.rs                   WalLaneCoordinator: per-thread WAL lanes (Lane GC, Phase 10)
  combiner.rs                Semantic WAL record combining (currently no-op placeholder)
txn/
  mod.rs                     Transaction module re-exports
  status.rs                  Snapshot, TxStatusTable, TxState (InProgress/Committed/Aborted), Isolation
  undo.rs                    UndoRecord: kind, tx_id, row_id, prev_undo chain, before_image payload
index/
  mod.rs                     BtreeIndex main interface
  cells.rs                   LeafCell, InternalCell encoding; physical key format
  cursor.rs                  IndexCursor: streaming left-to-right leaf-chain scan, KeyRange, SnapshotView
  cursor/raw.rs              Raw cursor implementation (page pin, slot iteration)
  mutate.rs                  Insert/delete with split logic
  scan.rs                    Range scan wrappers, pre-cursor equivalence test
  lookup.rs                  Key lookup, search path descent
  maintenance.rs             Index vacuuming
  locks.rs                   UniqueKeyLockTable: per-key FIFO lock for unique constraint enforcement
heap/
  mod.rs                     Heap module
  mem.rs                     In-memory heap implementation (ephemeral databases)
vector/
  mod.rs                     VectorMetric enum (L2, Cosine, InnerProduct), encode/decode_vector
  distance.rs                Scalar reference kernels: l2_distance_scalar, cosine_distance_scalar, inner_product_scalar
  codec.rs                   Wire format: varint LEB128 dimension + element kind byte + LE f32 bytes
  simd.rs                    AVX2/NEON SIMD dispatcher with runtime feature detection + scalar fallback
  flat.rs                    flat_top_k: fixed-capacity max-heap brute-force top-K
  hnsw/                      Hierarchical Navigable Small World index (Lane V2)
  diskann/                   DiskANN sector-aligned disk index (Lane V3)
json/
  mod.rs                     JSONB binary format, JSON-path extraction
failpoints/
  mod.rs                     Failpoint registry, ci_soft_gate integration
  macros.rs                  fail_point! macro (zero-cost when feature=off)
io/                          Filesystem abstraction layer
integrity/                   Checksum verification, page validation cross-checks
telemetry/                   Phase 11 performance counters (WAL sync, lock contention, index ops)
```

### `crates/sql/src/`

```
lib.rs                       SQL crate public API
parser.rs                    Entry points: split_first_statement, split_statements, is_blank_sql
parser/
  bind.rs                    SQL binder: AST → PreparedTemplate after name resolution
  ddl.rs                     CREATE/DROP TABLE/INDEX, ALTER TABLE parsing
  dml.rs                     INSERT/UPDATE/DELETE/SELECT parsing and binding
  pragma.rs                  PRAGMA statement parsing
  savepoint.rs               SAVEPOINT/RELEASE/ROLLBACK TO parsing
  select.rs                  Complex SELECT: joins, aggregates, subqueries, window functions, CTEs
  helpers/
    mod.rs                   Shared parser utilities
    ddl.rs                   DDL conversion helpers (column/constraint conversion)
    expr.rs                  Expression normalization and parameter binding
    table.rs                 Table resolution, column ordinal lookup, join binding
planner.rs                   Query planner entry point; Cost struct definition
planner/
  build.rs                   SelectPlan → PhysicalPlan with access path selection
  access.rs                  Access path enumeration: TableScan, IndexPointLookup, IndexRangeScan, etc.
  optimize.rs                Cost comparison, join reordering, predicate pushdown heuristics
  helpers.rs                 Statistics lookup, affinity resolution, selectivity estimation
exec/
  mod.rs                     Execute dispatcher: routes PreparedKind → execute_* functions
  expr/
    mod.rs                   Expression evaluation architecture
    scalar/
      mod.rs                 Scalar function dispatch
      math.rs                ABS, ROUND, SQRT, SIGN, CEIL, FLOOR, POWER
      pattern.rs             LIKE, GLOB, MATCH pattern matching
      value.rs               Value construction and literal evaluation
      row.rs                 Row value operations (IN, CASE, coercion)
    coerce.rs                SQL type affinity coercion rules
    json_dispatch.rs         JSON function routing (json_extract, json_type, etc.)
    window.rs                Window function evaluation (PARTITION BY, ORDER BY frames)
  agg.rs                     Aggregate function registry (COUNT, SUM, AVG, MIN, MAX, GROUP_CONCAT)
  agg_eval.rs                Aggregate accumulation during streaming
  insert.rs                  INSERT execution: row collection, upsert conflict handling, RETURNING
  select_top.rs              SELECT TOP N optimization and LIMIT/OFFSET application
  tail_rows.rs               Fetch next batch from table/index scan
  tail_stats.rs              Accumulate aggregates during streaming
  tail_conflict.rs           Detect unique violations during insert
  index_access.rs            Index cursor management: seek, range scan, row-ID fetch
  index_batch.rs             Batch-oriented index operations
  index_dml.rs               Per-transaction index undo log for DML atomicity
  vec/
    mod.rs                   Vectorized execution engine
    select.rs                Vectorized SELECT
    hash_agg.rs              Hash-based aggregation for GROUP BY
    sort.rs                  Vectorized sort
    topk.rs                  Vectorized TOP-K
    spill.rs                 Spill-to-disk for memory overflow
batch.rs                     RowBatch, MaterializeNode, ExecContext, QueryMemoryBroker
statement.rs                 PreparedTemplate, PreparedKind enum, Step result type
connection/
  mod.rs                     Connection module organization
  session.rs                 Connection: prepare_v2, execute, begin/commit/rollback, savepoint API
  database.rs                Database: owns Engine, StatementCache, StatsSnapshot, OptimizerConfig
  cache.rs                   Two-level statement cache (per-connection LRU + database-wide shared)
  options.rs                 DbOptions, OptimizerConfig, StatsConfig, QueryMemoryConfig
  tests.rs                   Connection integration tests
session.rs                   SessionState: per-connection tx journal, savepoint stack, unique locks
error.rs                     SqlError enum and conversion mappings (to DomainError, FFI codes)
value.rs                     SqlValue/SqlValueRef aliases to OwnedValue/ValueRef; comparison, coercion
datetime.rs                  Date/time parsing and formatting
collation.rs                 String comparison and collation rules
regexp.rs                    REGEXP operator implementation
json/
  mod.rs                     JSON value representation
  path.rs                    JSON path parsing ($.key[0].nested)
  scalar.rs                  JSON scalar function implementations
```

### `crates/ffi/src/`

```
lib.rs                       FFI module re-exports of C ABI symbols
types.rs                     C-visible types: rldb, rldb_stmt, rldb_backup, rldb_config (#[repr(C)])
sqlite3_api.rs               sqlite3_* aliases delegating to rldb_* entry points
lifecycle.rs                 rldb_open, rldb_open_v2, rldb_close, rldb_close_v2
stmt.rs                      rldb_prepare_v2, rldb_step, rldb_reset, rldb_finalize, rldb_clear_bindings
bind.rs                      rldb_bind_null/int64/double/text/blob/parameter_index
column.rs                    rldb_column_count/name/type/int64/double/text/blob/bytes
exec.rs                      rldb_exec with callback support
config.rs                    rldb_busy_timeout, rldb_changes, rldb_last_insert_rowid, rldb_stats_json
error.rs                     rldb_errcode, rldb_errmsg, rldb_free, rldb_interrupt
snapshot.rs                  rldb_backup_init/step/finish/close/remaining/pagecount
util.rs                      api() panic wrapper, open_handle(), map_error(), record_status()
tests.rs                     FFI unit tests
tests/
  exec_input_boundary.rs     SQL injection, multi-byte UTF-8, NULL byte in BLOB
  safety_invariants.rs       NULL pointer, oversize SQL, double-close, OOB parameter index
  error_paths.rs             Error condition coverage
```

### `crates/bench/src/`

```
lib.rs                       Bench crate public interface
certify.rs                   CertificationReport, CertificationManifest, manifest serialization
certify/
  scheduler.rs               1728-child bin-packing parallel scheduler
config.rs                    TOML-based RunSpec, CertifyArgs, FailpointMatrixArgs
workload.rs                  OLTP/chaos workload generators, run_once entry point
failpoint_matrix.rs          Crash injection certification: FailpointMatrixReport, FailpointMatrixRun
recover.rs                   Recovery matrix: RecoveryMatrixReport, RecoveryMatrixRun
checksum.rs                  Three-axis dataset fingerprint: row_count, key_xor, payload_hash
report.rs                    RunRecord, MetricsSummary, RunEnvironment, write_json/append_jsonl
process_metrics.rs           RSS/syscall tracking via libc::getrusage + strace aggregation
metrics.rs                   Latency histograms, throughput counters, failure kind tracking
gates.rs                     Certification result validation, markdown report generation
cross_engine.rs              RedlineDB ↔ SQLite query result comparison harness
feature_workloads.rs         JSON/vector-specific workload generators
spill.rs                     Sort spill overflow measurement
queue.rs                     Queue semantics validation workloads
connection_limit.rs          Max stable connections binary search
phase11.rs                   OLTP gap measurement workloads
strace_capture.rs            strace -c syscall aggregation
```

---

## 3. Storage Layer — Pages, Slots, Buffer Pool

### Page Format

All pages are 16,384 bytes (16 KB). The header occupies the first 64 bytes (`PAGE_HEADER_LEN = 64`).

```
Offset  Size  Field               Notes
------  ----  ------------------  ------------------------------------------------
0       4B    magic               0x5244_5047 ("RDPG")
4       2B    format_version      = 1
6       2B    kind                PageKind enum (see below)
8       4B    checksum            CRC32c over full 16 KB page (checksum field = 0 during compute)
12      4B    [reserved]
16      8B    page_id             Logical page identifier (u64)
24      8B    rel_id              Relation (table or index) identifier (u64)
32      8B    page_lsn            Most recent WAL LSN applied to this page
40      2B    lower               Byte offset to end of slot array (grows down from 64)
42      2B    upper               Byte offset to start of free space (grows up from end)
44      2B    special             Byte offset to special region (page_size − special_len)
46      2B    flags               Page-level flags
48      4B    generation          Incremented each time this page is reused (PageState: Reusable→Active)
52      1B    state               PageState enum (see below)
53      1B    free_class_hint     Free space class for allocation heuristics
54      2B    dead_bytes_hint     Approximate dead bytes (vacuum optimization hint)
56      8B    horizon_csn_hint    Oldest visible CSN on this page (vacuum optimization)
64      var   [slot array]        Grows downward; each slot = 4 bytes
        var   [free space]        Between slot array bottom and tuple data top
        var   [tuple/cell data]   Grows upward from page end toward free space
        var   [special region]    Fixed-size index metadata at page end
```

**PageKind enum:**

| Value | Name | Purpose |
|-------|------|---------|
| 1 | Meta | Database-level metadata page |
| 2 | Heap | Tuple storage (TupleVersion records) |
| 3 | Undo | Before-image undo records for rollback |
| 4 | TxnStatus | Transaction status checkpoint snapshot |
| 5 | FreeSpace | Free space map (vacuum / allocation hints) |
| 6 | Visibility | Visibility map (all-visible page flags) |
| 7 | BtreeInternal | B-tree internal (separator key + child pointer) |
| 8 | BtreeLeaf | B-tree leaf (index entries with create/delete tx) |
| 9 | BtreeMeta | B-tree root pointer + tree height |

**PageState enum:** `NeverAllocated`, `Reusable`, `Active`, `Retired`, `Quarantined`, `Invalid`.

### Slot Array

Each slot is 4 bytes: `[offset: u16, length: u16]` pointing to a cell in the page body. The array grows downward from byte 64. The number of live slots is `(header.lower - PAGE_HEADER_LEN) / SLOT_LEN` where `SLOT_LEN = 4`. Deleted cells leave their slot in place with offset=0, length=0 (dead slot); vacuum reclaims them.

### Heap File Structure

- One page file per relation (e.g., `data.redline`).
- Physical byte offset of page `P` = `P.id × 16384`.
- Pages are allocated sequentially; `BufferPool` tracks `next_page_id: AtomicU64`.
- Page reuse: `generation` counter (u32) increments when state transitions `Reusable → Active`. The `TuplePtr` struct encodes `{page_id, slot, generation}` — stale pointers with wrong generation are rejected.

### Buffer Pool

```rust
struct BufferPool {
    frames: Vec<Frame>,            // Fixed array of pinned pages
    clock_hand: AtomicUsize,       // CLOCK eviction hand position
    page_map: Mutex<HashMap<PageId, usize>>,  // PageId → frame index
    stats: BufferPoolStats {
        reads: AtomicU64,
        writes: AtomicU64,
        evictions: AtomicU64,
        checkpoint_flushes: AtomicU64,
    },
}
```

Eviction policy: CLOCK (second-chance approximation of LRU). Each frame carries a `referenced: AtomicBool`; on eviction pass, referenced frames get one more chance (bit cleared), unreferenced frames are evicted. Pinned frames are never evicted.

### Control File

Two copies (`CONTROL_A`, `CONTROL_B`) alternate writes using a generation counter:

```rust
struct ControlFile {
    generation: u64,      // Monotonically increasing; higher = more recent
    checkpoint_lsn: Lsn,  // WAL LSN of latest durable checkpoint
    page_count: u64,       // Number of allocated pages at checkpoint time
    crc32: u32,            // Covers the above fields
}
```

On startup: read both copies, pick the one with higher valid generation. If both corrupt: recover from WAL beginning.

> **Design note — critique surface:** The 16 KB page size is inherited from SQLite's default. There is no support for variable page sizes. Large BLOBs that do not fit in one page require spanning across multiple pages without an explicit large-object mechanism. The CLOCK eviction approximation may allow recently-accessed pages to be evicted under pathological access patterns where the reference bit is always set.

---

## 4. MVCC & Version Chains

### TupleVersion Header (72 bytes)

```
Offset  Size  Field            Notes
------  ----  ---------------  --------------------------------------------------
0       8B    row_id           Logical row identifier (RowId = u64)
8       8B    begin_tx         TxId that created this version
16      8B    end_tx           TxId that ended this version (TxId::ZERO = still alive)
24      8B    begin_csn_hint   Cached CSN of begin_tx (avoid status table re-lookup)
32      8B    end_csn_hint     Cached CSN of end_tx
40      8B    undo_head        UndoPtr: (page_id << 16) | slot; points to before-image chain
48      2B    flags            TUPLE_FLAG_DELETED = bit 0 (soft delete marker)
50      22B   [reserved]
72      var   payload          Encoded column values
```

### CSN (Commit Sequence Number)

- 64-bit monotonic counter held in `ConcurrentTxStatus`.
- On commit begin: atomically reserve CSN (`fetch_add`), add to pending set.
- On commit complete: publish CSN (remove from pending, add to committed, advance `published_csn` frontier).
- `published_csn` is the highest CSN such that all CSNs ≤ it are committed or aborted — vacuum can safely reclaim versions with `end_csn ≤ published_csn`.

### Snapshot

```rust
struct Snapshot {
    visible_csn: Csn,          // Committed txs with csn ≤ visible_csn are visible
    xmin: TxId,                // Smallest active TxId at snapshot time
    xmax: TxId,                // Largest assigned TxId + 1 at snapshot time
    active: BTreeSet<TxId>,   // In-progress transactions at snapshot time
}
```

Snapshot is captured atomically at `BEGIN` (Snapshot isolation) or at each statement (ReadCommitted).

### Visibility Decision (4-case rule)

```
fn is_visible(tuple: &TupleVersion, status: &ConcurrentTxStatus, snap: &Snapshot, owner: TxId) -> Visibility {
    // 1. Creator not visible → this version doesn't exist yet for this snapshot
    if !is_tx_visible(tuple.begin_tx, snap, owner, status) { return Invisible }

    // 2. Ender is visible → this version was superseded or deleted
    if tuple.end_tx != TxId::ZERO && is_tx_visible(tuple.end_tx, snap, owner, status) {
        return Invisible
    }

    // 3. Soft-delete marker
    if tuple.flags & TUPLE_FLAG_DELETED != 0 { return Deleted }

    // 4. Visible
    Visible
}
```

`is_tx_visible(tx, snap, owner, status)`:
- If `tx == owner`: always true (own writes are visible)
- If `status.state(tx) == Committed(csn)` and `csn <= snap.visible_csn`: true
- Otherwise: false (InProgress or Aborted)

### Tuple Lifecycle

**INSERT:**
1. Create `TupleVersion` with `begin_tx = tx_id`, `end_tx = TxId::ZERO`, no undo record.
2. Assign new `RowId`.
3. Write to heap, update B-tree index with `(logical_key || row_id)`.
4. Append `HeapInsert` WAL record.

**UPDATE:**
1. Find existing version (acquire row lock).
2. Create `UndoRecord { kind: UpdateBeforeImage, tx_id, row_id, before_image: old_payload, prev_undo: old.undo_head }`.
3. Create new `TupleVersion` with `begin_tx = tx_id`, `end_tx = TxId::ZERO`.
4. Set old version's `end_tx = tx_id`, `undo_head = new_undo_ptr`.
5. Update B-tree: delete old entry, insert new entry.
6. Append `HeapUpdate` + `IndexDelete` + `IndexInsert` WAL records.

**DELETE:**
1. Acquire row lock.
2. Create `UndoRecord { kind: DeleteBeforeImage, full before-image }`.
3. Set `end_tx = tx_id` on existing version, set `TUPLE_FLAG_DELETED`.
4. Delete B-tree entry.
5. Append `HeapDelete` + `IndexDelete` WAL records.

**ROLLBACK (for each mutated row):**
1. Walk undo chain from `undo_head`.
2. For `UpdateBeforeImage`: restore old payload, reset `end_tx = TxId::ZERO`, clear `undo_head`.
3. For `DeleteBeforeImage`: restore tuple, clear `TUPLE_FLAG_DELETED`, reset `end_tx`.
4. For `InsertDelete`: mark original tuple dead (set `TUPLE_FLAG_DELETED`, `end_tx = aborted_tx`).
5. Vacuum cleans up aborted versions during maintenance.

### Undo Record Format

```rust
struct UndoRecord {
    kind: UndoKind,        // InsertDelete=1, UpdateBeforeImage=2, DeleteBeforeImage=3
    tx_id: TxId,
    row_id: RowId,
    prev_undo: UndoPtr,    // Links to earlier undo record for same row
    before_image: Vec<u8>, // Encoded column values of previous version
}
```

Undo records are stored on dedicated `Undo` pages. The chain terminates when `prev_undo` encodes a null pointer sentinel.

### Isolation Levels

```rust
enum Isolation {
    ReadCommitted,  // Snapshot refreshed at each statement start
    Snapshot,       // Snapshot taken at BEGIN; consistent through transaction (default)
    Serializable,   // Enum variant exists; NOT enforced by kernel — application responsibility
}
```

> **Design note — critique surface:** Snapshot isolation allows write skew anomalies (e.g., two concurrent transactions each read a row, decide to update based on what they see, and both commit — producing a result neither would have chosen with full knowledge). There is no Serializable Snapshot Isolation (SSI), no cycle detection, and no predicate locking. Applications requiring strict serializability must implement application-level locking. The `BTreeSet<TxId>` in the snapshot grows with the number of concurrent transactions — for workloads with many long-running transactions, snapshot acquisition and visibility checks have O(active set) cost.

---

## 5. WAL & Group Commit

### WAL Record Header (48 bytes)

```
Offset  Size  Field         Notes
------  ----  ------------  -------------------------------------------------
0       4B    magic         0x5244_574c ("RDWL")
4       2B    version       = 1
6       2B    kind          WalRecordKind enum (see below)
8       4B    crc32         CRC32c over entire record (header + payload; field = 0 during compute)
12      4B    payload_len   Length of variable-length payload in bytes
16      8B    lsn           Log Sequence Number: byte offset in WAL stream
24      8B    prev_lsn      LSN of previous record (for backward scan)
32      8B    tx_id         Transaction ID (TxId::ZERO for non-transactional records)
40      8B    [reserved]
48      var   [payload]     Encoded WalPayload
```

**WalRecordKind:**

| Value | Name | Description |
|-------|------|-------------|
| 1 | Begin | Transaction start |
| 2 | PageImage | Full 16 KB page snapshot (redo safety net) |
| 3 | PageDelta | Partial page modification (reserved) |
| 4 | UndoAppend | Undo record written to WAL (reserved) |
| 5 | Commit | Transaction committed with CSN |
| 6 | Abort | Transaction aborted |
| 7 | CheckpointBegin | Checkpoint started |
| 8 | CheckpointEnd | Checkpoint completed |
| 9 | SegmentSeal | WAL segment rotation marker |

**WalPayload variants (selected):**

```
HeapInsert   { tx_id, rel_id, row_id, payload: Vec<u8> }
HeapUpdate   { tx_id, rel_id, row_id, payload: Vec<u8> }
HeapDelete   { tx_id, rel_id, row_id }
IndexInsert  { tx_id, index_id, logical_key: Vec<u8>, row: IndexRowRef }
IndexDelete  { tx_id, index_id, logical_key: Vec<u8>, row: IndexRowRef }
Commit       { tx_id, csn: Csn }
PageImage    { page_id, page_lsn: Lsn, page_bytes: [u8; 16384] }
SegmentSeal  { timeline, segment_no, first_lsn, last_lsn, crc32 }
CatalogSnapshot { tx_id, schema_epoch, snapshot: Vec<u8> }
```

### WAL Coordinator State Machine

`kernel/src/wal/manager/coordinator.rs`:

```rust
struct CoordinatorState {
    reserved_lsn: Lsn,           // Next LSN to assign (fetch_add on each record)
    written_lsn: Lsn,            // Successfully pwrite'd to segment file
    durable_lsn: Lsn,            // Successfully fdatasync'd
    pending: VecDeque<WalRecord>, // Records waiting for writer thread
    pending_bytes: usize,         // Total bytes in pending queue
    flush_requested_lsn: Lsn,    // Caller wants durability up to this LSN
    segment_bytes_written: u64,   // Bytes written to current segment
}
```

### Group Commit Flow

1. **Append (called by transaction on each mutation):**
   - Lock coordinator state.
   - Assign LSN from `reserved_lsn` (fetch_add encoded_len).
   - Push record to `pending` queue; bump `pending_bytes`.
   - Wake writer thread via `Condvar::notify_all()`.
   - Return `WalAppend { start_lsn, end_lsn }`.

2. **Writer thread loop (`wal_writer_loop`):**
   - Wait on condvar.
   - Check flush condition: `pending_bytes > GROUP_COMMIT_MAX_BATCH_BYTES` OR `flush_requested_lsn > written_lsn` OR `group_commit_delay_us` elapsed.
   - Drain `pending` queue into write buffer.
   - `pwrite()` buffer to current segment at `written_lsn` offset.
   - Rotate segment if `segment_bytes_written >= segment_size`.
   - `fdatasync()` segment file.
   - Advance `durable_lsn` atomically.
   - Wake all threads waiting on `flush_until()`.
   - Update telemetry: `group_commits_issued++`, histogram bucket, `batch_bytes_sum`.

3. **Flush (called by committing transaction):**
   - Set `flush_requested_lsn = commit_lsn`.
   - Notify writer thread.
   - Block on condvar until `durable_lsn >= commit_lsn`.

### Group Commit Parameters

| Parameter | Default | Effect |
|-----------|---------|--------|
| `group_commit_delay_us` | 200 µs | Maximum wait before forced flush |
| `group_commit_max_batch_bytes` | 4 MB | Batch size trigger for early flush |
| `wal_segment_bytes` | 64 MB | WAL file rotation threshold |

### WalSyncCounters (Phase 11 telemetry)

```rust
struct WalSyncCounters {
    fsyncs_issued: AtomicU64,
    fdatasyncs_issued: AtomicU64,
    pwrites_issued: AtomicU64,
    group_commits_issued: AtomicU64,
    group_commit_batch_bytes_sum: AtomicU64,
    group_commit_batch_record_count_sum: AtomicU64,
    group_commit_batch_buckets: [AtomicU64; 16],  // Power-of-two size histogram
}
```

### Segment Files

- Filename: `{segment_no:020}.wal` (20-digit zero-padded decimal)
- Seal file: `{segment_no:020}.seal` (text key=value pairs: `timeline=`, `segment_no=`, `first_lsn=`, `last_lsn=`, `byte_len=`, `crc32=`)
- Seal is written atomically after rotation before the next segment receives any writes.

### CommitDurability Modes

```rust
enum CommitDurability {
    Strict,    // Block until WAL fdatasync before returning to application
    Normal,    // Same as Strict (currently identical; reserved for async ack path)
    UnsafeDev, // No fsync — data loss on crash. Testing only.
}
```

> **Design note — critique surface:** The 200 µs group-commit delay is a hard latency floor for individual transactions under low concurrency — a single-writer workload always pays this tax even when there is no batching benefit. The 4 MB batch size may cause write stalls on slow storage (NVMe handles this gracefully; spinning disk may not). There is a single WAL writer thread; all concurrent transactions serialize their append through one `VecDeque`. High-throughput workloads with many small transactions may saturate the writer thread. There is no parallel WAL (Postgres-style WAL buffer with multiple inserters) — all appends are serialized through the coordinator state lock.

---

## 6. B-Tree Index

### Physical Key Format

Every index entry is keyed by a `KeyBuf` that appends the row locator to the user-visible key:

```
KeyBuf = [logical_key_bytes] ++ [row_id: 8B] ++ [page_id: 8B] ++ [slot: 2B] ++ [generation: 4B]
                                 \_________________ 22-byte suffix (IndexRowRef) _________________/
```

The row locator suffix guarantees key uniqueness — even a non-unique index has distinct physical keys for each row. This property:
- Eliminates phantom duplicates in the leaf chain.
- Ensures split midpoint calculation is always well-defined.
- Allows range scans to strip the suffix when comparing against user-supplied bounds.

### Leaf Entry Encoding

```
varint(logical_key.len)
logical_key bytes (variable)
row_id:     8B LE
page_id:    8B LE
slot:       2B LE
generation: 4B LE
create_tx:  8B LE
delete_tx:  8B LE   (TxId::ZERO = entry is alive)
```

### Internal Cell Encoding

```
varint(separator.len)
separator bytes (full physical key including row_id suffix)
child_page_id: 8B LE
```

### Page Special Region (256 bytes, `INDEX_SPECIAL_LEN`)

For **BtreeMeta** pages:

```
Offset  Size  Field
------  ----  --------------------
0       2B    page_kind
2       2B    level
8       8B    root_page_id
16      2B    root_level (tree height; 0 = root is leaf)
18      1B    uniqueness (1 = Unique, 0 = NonUnique)
```

For **BtreeLeaf** and **BtreeInternal** pages:

```
Offset  Size  Field
------  ----  --------------------
0       2B    page_kind
2       2B    level (0 = leaf)
8       8B    index_id
18      8B    left_sibling  (PageId::MAX = no left neighbor)
26      8B    right_sibling (PageId::MAX = no right neighbor)
34      2B    high_key_len
36      var   high_key bytes (separator toward right sibling)
```

### Split Algorithm

1. Leaf page is full (cannot accommodate new cell).
2. Compute split point: median entry by byte offset.
3. Allocate new right-sibling leaf page (`BufferPool::alloc_page`).
4. Move entries above split point to new page.
5. Extract separator key (first key of new page, full physical form).
6. Promote separator to parent: `parent.insert_internal_cell(separator, new_page_id)`.
7. If parent is full: recursively split parent (bottom-up cascade).
8. If root splits: allocate new root, install two children, increment `root_level`.
9. Update `left_sibling` / `right_sibling` links atomically under `structure_lock`.

### Concurrent Writer Behavior

All structural modifications (insert, delete, split) are protected by `structure_lock: Mutex<()>` on `BtreeIndex.inner`. The lock is acquired for the duration of the tree descent + leaf modification + split cascade. This ensures structural integrity but serializes all writers:

- Two writers inserting into **different** leaf pages still serialize at the `structure_lock`.
- The lock is **not** held across `pwrite()` to the buffer pool — structural changes are staged in memory and flushed by the buffer pool separately.

### Range Scan

1. Descend from root to leftmost leaf that may contain `start_key`.
2. Pin the leaf page. Iterate slots:
   - Extract `logical_key` (strip row_id suffix).
   - Check: `logical_key >= start_bound` and `logical_key < end_bound`.
   - If entry is visible (snapshot-filtered via `create_tx`/`delete_tx`): yield to cursor.
3. Follow `right_sibling` link to next leaf.
4. Stop when `leaf.high_key >= end_bound` (all remaining entries are out of range) or `right_sibling == PageId::MAX`.

### Entry Visibility

Mirroring heap tuple visibility:

```
fn entry_visible(entry, status, snap, owner) -> bool {
    if !is_tx_visible(entry.create_tx, snap, owner, status) { return false }
    if entry.delete_tx != TxId::ZERO && is_tx_visible(entry.delete_tx, snap, owner, status) {
        return false
    }
    true
}
```

### UniqueKeyLockTable

Prevents the TOCTOU race: "writer A sees no duplicate, writer B sees no duplicate, both insert same unique key, both commit."

Each INSERT on a unique-indexed column acquires a `UniqueKeyGuard` on the normalized key. The guard is held until the transaction commits or rolls back. A second INSERT on the same key blocks until the guard is released, then checks for a committed version.

> **Design note — critique surface:** `structure_lock` is a single global mutex over the entire B-tree. All insertions serialize at the leaf modification point, even for logically disjoint keys on different leaf pages. Under high concurrent-insert workloads this becomes a bottleneck — the 15.89× win over SQLite on `writers-disjoint` is achieved despite this lock because SQLite's single-writer WAL is even more restrictive. The Lehman-Yao B-link tree protocol (1981) allows concurrent structural modifications without a global structure lock by using right-links and a "move right" algorithm — this would eliminate the serialization but requires significant redesign of the split path.

---

## 7. Catalog & Schema Management

### CatalogManager

```rust
struct CatalogManager {
    current: ArcSwap<SchemaSnapshot>,  // Atomic pointer to current schema; zero-copy reads
    version: AtomicU64,                // SchemaEpoch; incremented on every DDL
    ddl_lock: Mutex<()>,               // Serializes all DDL (one DDL operation at a time)
}
```

Reads do not acquire any lock: `manager.current()` atomically clones the `Arc<SchemaSnapshot>` via ArcSwap. DDL writers acquire `ddl_lock`, mutate a clone of the snapshot, increment the epoch, then `ArcSwap::store(new_snapshot)`. Concurrent readers see either the old or new snapshot atomically — no partial state.

### SchemaSnapshot

```rust
struct SchemaSnapshot {
    meta: CatalogMeta {
        format_version: u64,
        schema_epoch: SchemaEpoch,     // u64 epoch, matches CatalogManager::version
        next_object_id: ObjectId,
        next_relation_id: RelId,
        database_uuid: [u8; 16],
    },
    namespaces: HashMap<SchemaId, NamespaceDef>,
    relations: HashMap<RelId, (ClassKind, ObjectId)>,  // heap or index
    tables: HashMap<TableId, TableDef>,
    columns: HashMap<ColumnId, ColumnDef>,
    indexes: HashMap<IndexId, IndexDef>,
    constraints: HashMap<ConstraintId, ConstraintDef>,
}
```

### Key Type Definitions

**TableDef:**
```rust
struct TableDef {
    table_id: TableId,
    relation_id: RelId,           // Points to the heap PageFile
    name: Box<str>,
    schema_id: SchemaId,
    columns: Vec<ColumnId>,
    constraints: Vec<ConstraintId>,
    primary_key: Option<IndexId>, // Auto-created B-tree for PRIMARY KEY
}
```

**IndexDef:**
```rust
struct IndexDef {
    index_id: IndexId,
    table_id: TableId,
    relation_id: RelId,              // Points to the B-tree PageFile
    meta_page_id: Option<PageId>,    // Root of B-tree (None until first insert)
    name: Box<str>,
    unique: bool,
    primary: bool,
    origin: IndexOrigin,             // CreatedExplicitly | ByUniqueConstraint | ByPrimaryKey
    keys: Vec<IndexKeyDef {          // Column ordinal + sort direction + null ordering
        column_ordinal: usize,
        descending: bool,
        nulls_first: bool,
    }>,
}
```

### Catalog Persistence

- Snapshots are encoded as binary blobs and stored in `.redline_catalog` (an embedded SQLite database).
- Every DDL transaction appends a `CatalogSnapshot` WAL record containing the serialized new snapshot.
- On startup: `CatalogStore::load()` reads the latest snapshot from the catalog store.
- Recovery: the WAL scan also extracts the most recent `CatalogSnapshot` record; this is authoritative after a crash.

### DDL Lifecycle

1. Acquire `ddl_lock` (serializes concurrent DDL).
2. Clone current `SchemaSnapshot` from ArcSwap.
3. Apply mutation (add table, add index, etc.) to the clone.
4. Increment `schema_epoch`.
5. Persist: append `CatalogSnapshot` WAL record, fsync, write to catalog store.
6. `ArcSwap::store(Arc::new(new_snapshot))`.
7. Bump `schema_epoch` in `CatalogManager::version`.
8. Release `ddl_lock`.
9. All connections with cached statements see stale `schema_epoch` on next prepare and re-plan.

> **Design note — critique surface:** The catalog is persisted in an embedded SQLite database (`.redline_catalog`). This creates a bootstrapping dependency: RedlineDB uses SQLite to store its own schema. If SQLite's catalog file is corrupted, schema recovery must fall back to WAL replay of `CatalogSnapshot` records. Any DDL operation increments `schema_epoch` and invalidates all statement caches globally across all connections — there is no fine-grained cache invalidation (e.g., invalidating only connections that reference the changed table). High-DDL workloads (schema storms) will cause repeated re-planning.

---

## 8. Transaction Lifecycle

### Begin

```rust
fn begin_txn(isolation: Isolation) -> Txn {
    let tx_id = txs.next_tx.fetch_add(1);           // Assign TxId
    let snap = txs.snapshot();                       // Capture visible_csn + active set
    txs.register_active_snapshot(tx_id, snap.visible_csn);
    Txn { tx_id, isolation, snapshot: snap, ... }
}
```

For `IMMEDIATE`/`EXCLUSIVE` BEGIN modes: call `engine.reserve_begin_lock(tx)` which acquires a writer lock (blocking concurrent BEGINs of the same mode).

### Write Phase

For each `INSERT`/`UPDATE`/`DELETE`:

1. `locks.lock(rel_id, row_id, tx_id)` — acquire row lock (FIFO queue, blocks if contended, timeout enforced).
2. Apply heap mutation (create/update TupleVersion, create UndoRecord if needed).
3. Apply index mutations (insert/delete B-tree entries).
4. Append WAL records (HeapInsert/Update/Delete + IndexInsert/Delete).
5. Record mutation in `SessionState::journal` for savepoint replay.

### Commit

```rust
fn commit(tx: Txn) -> CommitOutcome {
    // 1. Reserve CSN atomically
    let csn = txs.reserve_csn();    // fetch_add(1), add to pending frontier set

    // 2. Append Commit record to WAL
    wal.append(WalPayload::Commit { tx_id: tx.tx_id, csn })?;

    // 3. Flush WAL to disk (group commit: may batch with other committers)
    wal.flush_until(commit_lsn)?;   // blocks until durable_lsn >= commit_lsn

    // 4. Publish commit (update ConcurrentTxStatus)
    txs.publish_commit(tx.tx_id, csn);
    // - Sets tx state to Committed(csn)
    // - Removes csn from pending frontier
    // - Advances published_csn if csn is at frontier
    // - Unregisters active snapshot (releases vacuum horizon pressure)

    // 5. Release row locks (wake FIFO waiters)
    for key in tx.drain_row_locks() {
        locks.unlock(key);
    }

    CommitOutcome::Committed(csn)
}
```

### Rollback

```rust
fn rollback(tx: Txn) {
    // Walk undo chain for each modified row
    for undo_ptr in tx.undo_chain() {
        let record = read_undo_record(undo_ptr);
        match record.kind {
            UpdateBeforeImage => restore_heap_tuple(record.row_id, &record.before_image),
            DeleteBeforeImage => restore_heap_tuple(record.row_id, &record.before_image),
            InsertDelete      => mark_heap_tuple_dead(record.row_id),
        }
        restore_index_entry(record);
    }

    // Optionally append Abort WAL record
    wal.append(WalPayload::Abort { tx_id: tx.tx_id })?;

    txs.publish_abort(tx.tx_id);
    for key in tx.drain_row_locks() { locks.unlock(key); }
}
```

### CommitOutcome

```rust
enum CommitOutcome {
    Committed(Csn),   // Success; CSN is the durable commit point
    RolledBack,       // Application called rollback
    MaybeCommitted,   // WAL fsync succeeded but publish_commit failed (edge case)
}
```

`MaybeCommitted` is surfaced as an error to the application. It indicates that the transaction's WAL record is durable but the in-memory frontier was not updated — the transaction is recoverable on next startup but the application cannot rely on the current session seeing the committed data.

> **Design note — critique surface:** `MaybeCommitted` requires application-level idempotency. If the application retries a `MaybeCommitted` transaction, the retry will either find the committed data (after recovery) or insert a duplicate (if it does not check first). The CSN frontier is bounded below by the oldest active snapshot — a long-running `SELECT` (read transaction) holds `visible_csn` constant for that snapshot, which blocks vacuum from advancing `published_csn`, which means dead tuple storage is not reclaimed.

---

## 9. Concurrency Control

### RowLockManager

```rust
struct RowLockManager {
    shards: Vec<LockShard>,           // 64 shards (hash of (rel_id, row_id) % 64)
    timeout: RwLock<Duration>,
    phase11: RwLock<Option<Arc<Phase11Counters>>>,
}

struct LockShard {
    rows: Mutex<HashMap<RowKey, RowLockState>>,
}

struct RowLockState {
    owner: Option<TxId>,              // Current holder (None = free)
    waiters: VecDeque<Arc<Condvar>>,  // FIFO queue of parked waiters
}
```

**Acquire:**
1. Hash `(rel_id, row_id)` → shard index.
2. Lock shard mutex.
3. Fast path: `owner == None` → set `owner = Some(tx_id)`, return `Ok(guard)`.
4. Slow path: push `Arc<Condvar>` to `waiters`, unlock shard, park on condvar with timeout.
5. On wake: re-check ownership (spurious wakes possible); if still not owner, re-park.
6. On timeout: return `Err(LockTimeout)`.

**Release:**
1. Lock shard mutex.
2. Clear `owner = None`.
3. If `waiters` non-empty: pop front condvar, `notify_one()`.

### Conflict Scenarios

| Scenario | Mechanism | Behavior |
|----------|-----------|----------|
| Two writers, same row | RowLockManager | Second writer blocks; released when first commits/rolls back |
| Two writers, disjoint rows | RowLockManager | No conflict; proceed in parallel |
| Writer + reader, same row | None (MVCC) | Reader sees pre-write snapshot; no blocking |
| Two writers, same unique key | UniqueKeyLockTable | Second writer blocks; checks committed version on wake |
| Two writers, same B-tree path | `structure_lock` | Serialize at leaf modification; brief hold |

### What Is NOT Enforced

- **Predicate locks**: no protection against phantoms in range queries under Snapshot isolation.
- **Serializable conflicts**: no cycle detection, no SSI.
- **CHECK / FOREIGN KEY constraints**: parsed but not enforced at engine level.
- **Deadlock detection**: lock acquisition has a timeout (`busy_timeout_ms`), not a deadlock graph. On timeout, the transaction receives `RLDB_BUSY`.

> **Design note — critique surface:** The absence of deadlock detection means that a deadlock cycle between two transactions will not be broken — both will time out after `busy_timeout_ms`. For high-contention workloads, careful application-level lock ordering is required. The FIFO wake ordering in `RowLockManager` prevents starvation but does not guarantee fairness between transactions of different priorities.

---

## 10. Crash Recovery

### Recovery Algorithm

`Engine::open_with_recovery_report` runs on every database open (fast-path when no crash).

**Step 1 — WAL scan:**
```rust
let scan = WalReader::new(&wal_dir, config).scan_report()?;
// scan.valid_end_lsn: highest LSN with valid CRC32
// scan.torn_tail: bytes after valid_end_lsn (truncated on re-open)
```

**Step 2 — Control file:**
```rust
let control = ControlFile::load_latest(&control_dir)?;
// Reads CONTROL_A and CONTROL_B; picks highest valid generation.
// If both corrupt: recover from WAL beginning (checkpoint_lsn = 0).
```

**Step 3 — Transaction status checkpoint:**
```rust
let tx_status = TxStatusStore::load(control.generation)?;
// Contains: next_tx, next_csn, published_csn, Vec<(TxId, TxState)> committed entries
```

**Step 4 — Redo pass 1 (collect commits):**
```rust
let committed: HashMap<TxId, Csn> = wal_records
    .filter(|r| r.kind == Commit && r.lsn > control.checkpoint_lsn)
    .map(|r| (r.tx_id, r.payload.csn))
    .collect();
```

**Step 5 — Redo pass 2 (apply mutations):**
```rust
for record in wal_records.filter(|r| r.lsn > control.checkpoint_lsn) {
    if committed.contains_key(&record.tx_id) {
        match record.payload {
            HeapInsert { .. } | HeapUpdate { .. } | HeapDelete { .. } => apply_heap(record),
            IndexInsert { .. } | IndexDelete { .. }                    => apply_index(record),
            PageImage { page_id, page_bytes, .. }                      => restore_page(page_id, page_bytes),
            CatalogSnapshot { .. }                                      => update_catalog(record),
            _ => {}
        }
    }
    // Uncommitted tx_ids are silently skipped (no undo phase)
}
```

**Step 6 — Idempotency guard:**
Page redo is skipped if `page.page_lsn >= record.lsn` — the page already reflects this WAL record (was flushed as part of a checkpoint after the WAL was written).

**Step 7 — Frontier restoration:**
```rust
txs.restore_frontier(tx_status.next_tx, tx_status.next_csn, tx_status.published_csn);
// Advances atomic counters so new transactions get fresh IDs
```

### No Undo Phase

Incomplete (uncommitted) transactions are ignored during recovery. Their tuple versions are physically present on heap pages but visibility-hidden (their `begin_tx` will never appear in `committed`, so no snapshot will consider them visible). Vacuum is responsible for reclaiming the storage.

### Point-In-Time Recovery (PITR)

```rust
enum RecoveryTarget {
    Latest,      // Recover all committed transactions up to valid_end_lsn
    Lsn(Lsn),    // Recover committed transactions up to but not including this LSN
    Csn(Csn),    // Recover committed transactions up to but not including this CSN
}
```

PITR stops redo at the target boundary. Useful for recovering to a known-good state after a logical data error.

### Durability Guarantee

A transaction is durable if and only if its `Commit` WAL record has been `fdatasync()`'d to the WAL file before the application received `RLDB_OK`. This is enforced by `WalCoordinator::flush_until()` which blocks the committing thread until `durable_lsn >= commit_lsn`.

> **Design note — critique surface:** The recovery algorithm is redo-only (no undo phase). This simplifies recovery but means aborted or in-flight transactions leave ghost versions on heap pages that must be cleaned by vacuum. Under a crash with many large in-flight transactions, the vacuum backlog can be substantial. `PageImage` records in the WAL are full 16 KB pages — this ensures redo idempotency but at significant WAL amplification for workloads with many small mutations to large pages (e.g., 100 single-byte updates to one page writes 100 × 16 KB = 1.6 MB of WAL). A page-delta record format (storing only changed bytes) would reduce WAL amplification but requires careful handling of partial page images during recovery.

---

## 11. SQL Pipeline — Parser, Planner, Executor

### Parser

Uses `sqlparser-rs` crate with `SQLiteDialect`. Input SQL string → `Vec<Statement>` (sqlparser AST) → `PreparedKind` (RedlineDB internal IR).

**PreparedKind enum:**
```rust
enum PreparedKind {
    Begin(BeginMode),                    // IMMEDIATE | DEFERRED | EXCLUSIVE
    Commit, Rollback,
    Savepoint(String), Release(String), RollbackTo(String),
    Pragma(PragmaPlan),
    CreateTable(CreateTableSpec),
    CreateIndex(CreateIndexSpec),
    DropTable(DropTableSpec),
    DropIndex(DropIndexSpec),
    AlterTable(AlterTableSpec),
    Analyze(AnalyzePlan),
    Explain(ExplainPlan),
    Select(SelectPlan),
    Insert(InsertPlan),
    Update(UpdatePlan),
    Delete(DeletePlan),
}
```

### Supported SQL Surface

| Category | Supported | Not Supported |
|----------|-----------|---------------|
| SELECT | FROM, WHERE, GROUP BY, HAVING, ORDER BY, LIMIT/OFFSET, DISTINCT, UNION/INTERSECT/EXCEPT, JOIN (INNER/LEFT), window functions (OVER), limited CTEs | RIGHT/FULL JOIN, recursive CTEs, lateral joins, table-valued functions |
| INSERT | VALUES, SELECT source, ON CONFLICT upsert, RETURNING | INSERT OR REPLACE (use upsert), multi-row VALUES with different param counts |
| UPDATE | WHERE, RETURNING | ORDER BY, LIMIT, UPDATE with FROM clause |
| DELETE | WHERE, RETURNING | ORDER BY, LIMIT |
| DDL | CREATE/DROP TABLE, CREATE/DROP INDEX, ALTER TABLE (add column) | CREATE TABLE AS SELECT, partial/expression indexes, RENAME TABLE |
| Constraints | NOT NULL, UNIQUE, PRIMARY KEY (stored), DEFAULT | CHECK (parsed, not enforced), FOREIGN KEY (parsed, not enforced) |
| Types | INTEGER, REAL, TEXT, BLOB, NULL | No strict type system (SQLite affinity rules apply) |
| Other | PRAGMA (FOREIGN_KEYS, USER_VERSION), SAVEPOINT, EXPLAIN, ANALYZE | VIEW, TRIGGER, TEMPORARY TABLE, ATTACH |

### Parameter Binding

```rust
struct ParamLayout {
    slots: Vec<Option<String>>,     // 1-indexed; slot 0 = reserved (SQLite contract)
    named: HashMap<String, usize>,  // Named parameter → slot index
}
```

Marker styles: `?` (anonymous), `?1`/`?2` (positional), `:name`/`@name`/`$name` (named). All are 1-indexed.

### Query Planner

`sql/src/planner/` converts `SelectPlan` → `PhysicalPlan`.

**Cost model constants:**

| Constant | Value | Purpose |
|----------|-------|---------|
| `SEQ_PAGE_COST` | 1.0 | Sequential page fetch cost unit |
| `RANDOM_PAGE_COST` | 4.0 | Random page access penalty |
| `CPU_TUPLE_COST` | 0.01 | Per-tuple CPU processing |
| `CPU_OPERATOR_COST` | 0.0025 | Per-operator overhead |
| `INDEX_PROBE_STARTUP` | 2.0 | B-tree descent startup cost |
| `UNKNOWN_EQ_SELECTIVITY` | 0.10 | Default selectivity for `=` predicate |
| `UNKNOWN_RANGE_SELECTIVITY` | 0.33 | Default selectivity for range predicates |

**Access path types:**

| Path | When Used | Cost Estimate |
|------|-----------|---------------|
| `TableScan` | No usable index, or index worse than seq scan | `page_count × SEQ_PAGE_COST + rows × CPU_TUPLE_COST` |
| `IndexPointLookup` | `=` predicate on indexed column | `INDEX_PROBE_STARTUP + selectivity × rows × RANDOM_PAGE_COST` |
| `IndexRangeScan` | `<`, `>`, `<=`, `>=`, `BETWEEN` | `INDEX_PROBE_STARTUP + range_selectivity × rows × RANDOM_PAGE_COST` |
| `CoveringIndexScan` | All projected columns in index | Eliminates heap fetch; removes `RANDOM_PAGE_COST` per row |
| `MultiIndexOr` | `col=1 OR col=2` | Union of two IndexPointLookup costs |
| `MultiIndexAnd` | Two predicates, two indexes | Intersection cost model |

**PhysicalPlan node:**
```rust
struct PhysicalPlan {
    kind: PhysicalKind,
    relation: Option<String>,
    index: Option<String>,
    index_probe_kind: Option<&'static str>,    // "unique" | "range" | "covering"
    ordered_index_scan_limit: Option<usize>,   // Early stop on sorted scan + LIMIT
    estimated_rows: f64,
    cost: Cost { startup, total, rows, width, memory_bytes, spill_bytes },
    access_predicates: Vec<String>,            // Pushed into index scan
    residual_predicates: Vec<String>,          // Evaluated after row fetch
    output_order: Vec<String>,                 // Guaranteed sort order
    projected_columns: Vec<String>,
    memory_budget: usize,
    // EXPLAIN ANALYZE fields (populated at runtime):
    actual_rows: Option<usize>,
    elapsed_ms: Option<f64>,
    peak_memory_bytes: Option<usize>,
    spill_bytes: Option<usize>,
    children: Vec<PhysicalPlan>,
}
```

### Executor

`sql/src/exec/mod.rs` dispatches on `PreparedKind`:

- DDL → `execute_ddl` → kernel catalog ops
- `INSERT` → `execute_insert` → row collection, conflict check, heap + index write, RETURNING
- `UPDATE` → `execute_update` → predicate scan, row lock, heap + index update, RETURNING
- `DELETE` → `execute_delete` → predicate scan, row lock, heap + index delete, RETURNING
- `SELECT` → `execute_select` → `SelectRuntime` driving `PhysicalPlan`

**Vectorized executor** (`exec/vec/`):
- Processes rows in `RowBatch` chunks (not row-at-a-time).
- `hash_agg.rs`: spills to disk when `QueryMemoryBroker` budget exceeded.
- `sort.rs`: external sort with disk spill.
- `topk.rs`: maintains a heap of size K rather than sorting all rows.

### Statement Cache

Two-level LRU:
1. **Per-connection local cache** — fast path for repeated `prepare()`.
2. **Database-wide shared cache** — allows prepared statements to be reused across connections.

Cache key:
```rust
struct StatementCacheKey {
    schema_epoch: u64,     // Invalidate on DDL
    stats_epoch: u64,      // Invalidate on ANALYZE
    optimizer_hash: u64,   // Invalidate on OptimizerConfig change
    sql: Arc<str>,         // Normalized SQL text
}
```

> **Design note — critique surface:** The planner has no histogram-based cardinality estimation. All selectivity is estimated from constants (`UNKNOWN_EQ_SELECTIVITY = 0.10`, `UNKNOWN_RANGE_SELECTIVITY = 0.33`). These constants are wrong for skewed distributions (e.g., a column with 90% nulls and 10% non-null values). `ANALYZE` collects stats (`TableStats`, `ColumnStats`) but the planner may not use histogram buckets — only approximate row counts. There is no join reordering for 3+ table joins; the join order from the SQL text is used directly. The `HashAggregate` spill path exists but its completeness in edge cases (nested aggregates, very large groups) is tracked as a feature gap. The `ordered_index_scan_limit` optimization (early stop on a sorted index scan + LIMIT) only applies to single-column ORDER BY matching an index prefix.

---

## 12. Connection & Session Model

### Object Hierarchy

```
Database
  ├── Engine (Arc)
  ├── StatementCache (shared, database-wide)
  ├── OptimizerConfig
  └── UniqueLockTable (Arc)

Connection (Arc<Database>)
  ├── SessionState (Mutex)
  └── StatementCache (per-connection LRU)

Statement (Arc<Connection>)
  ├── PreparedTemplate
  └── column_names: Vec<String>
```

### SessionState

```rust
struct SessionState {
    tx: Option<Txn>,                     // Active kernel transaction; None between transactions
    failed: bool,                        // tx must ROLLBACK before any new statement
    changes: usize,                      // Rows changed in current transaction
    total_changes: usize,                // Cumulative changes (across transactions)
    foreign_keys: bool,                  // PRAGMA FOREIGN_KEYS state
    last_insert_rowid: Option<i64>,     // Last INSERT rowid
    unique_guards: Vec<UniqueKeyGuard>,  // SQL-layer unique guards held by this session
    kernel_unique_guards: Vec<KernelUniqueKeyGuard>,
    journal: Vec<JournalEntry>,          // Ordered SQL statements for savepoint replay
    savepoints: Vec<SavepointFrame>,     // Active savepoint stack
    replay_in_progress: bool,            // True during ROLLBACK TO replay
}
```

### Transaction Flow Through Session

**BEGIN:**
1. Reject if `session.tx.is_some()` (no nested transactions).
2. `engine.begin(Isolation::Snapshot)` → assign TxId, capture Snapshot.
3. For IMMEDIATE/EXCLUSIVE: acquire writer lock.
4. Set `session.tx = Some(tx)`, `session.failed = false`.

**Statement execution:**
1. Reject if `session.failed` ("must ROLLBACK").
2. Execute via appropriate `execute_*` function.
3. Update `session.changes`, `session.last_insert_rowid`.
4. Append to `session.journal` (for ROLLBACK TO replay).

**COMMIT:**
1. Take `session.tx` (error if None).
2. Check `!session.failed`.
3. `engine.commit(tx)` → group-commit WAL, publish CSN.
4. Clear journal, savepoints, unique guards.
5. `session.tx = None`.

**ROLLBACK:**
1. Take `session.tx`.
2. `engine.rollback(tx)` → restore undo chain, publish abort.
3. Clear all session state.
4. `session.tx = None`, `session.failed = false`.

### Sync Facade Integration Pattern

The public Rust API is synchronous. `Database` is the shared handle and may be cloned or shared across threads; each worker that performs blocking SQL work should open its own `Connection`. `Connection` owns session state and is the unit that prepares and executes statements. Prepared statements are borrow-bound to a connection unless the code explicitly needs an owned form that can outlive that borrow.

For async runtimes, the integration pattern is:

1. Keep `Database` in shared application state.
2. Open one `Connection` inside each blocking worker.
3. Execute SQL work inside `tokio::task::spawn_blocking` or an equivalent blocking pool.
4. Drop the `Connection` with the worker or task scope so session state does not cross thread boundaries unintentionally.

### Savepoint Mechanism

Each `SAVEPOINT name` pushes:

```rust
struct SavepointFrame {
    name: String,
    journal_len: usize,      // Prefix of session.journal to replay on ROLLBACK TO
    changes: usize,          // session.changes snapshot
    total_changes: usize,
    last_insert_rowid: Option<i64>,
    implicit_tx: bool,       // True if this SAVEPOINT opened the transaction
}
```

**ROLLBACK TO name:**
1. Find frame; record `journal_len` prefix.
2. Take current `session.tx`; call `engine.rollback(tx)`.
3. Open fresh `engine.begin(Snapshot)`.
4. Set `session.replay_in_progress = true`.
5. Re-execute `session.journal[..journal_len]` on fresh tx.
6. Set `session.replay_in_progress = false`.
7. Restore `changes`, `last_insert_rowid` from frame.
8. Frame remains on stack (per SQLite contract — `ROLLBACK TO` does not pop).

**RELEASE name:**
1. Find frame; pop it and all frames above.
2. If `implicit_tx` and savepoint stack is now empty: auto-commit.

> **Design note — critique surface:** Savepoint rollback re-executes SQL in the SQL layer (not WAL redo). This relies on statement-level idempotency. Non-deterministic functions — `RANDOM()`, `datetime('now')`, external UDFs — will produce different values on replay, yielding a different result set than the original execution. This is a semantic difference from PostgreSQL's savepoints, which replay WAL page images (exact). Applications using savepoints with randomized or time-dependent logic must account for this behavior. Additionally, `session.journal` grows unboundedly with the number of statements in a transaction — long transactions with thousands of statements accumulate large journals.

---

## 13. FFI C ABI Surface

### Exported Symbols

All functions are declared in `contracts/c-abi/redlinedb.h` and implemented in `crates/ffi/src/`.

```c
// Lifecycle
int rldb_open(const char *path, rldb **out);
int rldb_open_v2(const char *path, const rldb_config *cfg, rldb **out);
int rldb_close(rldb *db);
int rldb_close_v2(rldb *db);

// Prepared statements
int rldb_prepare_v2(rldb *db, const char *sql, int nbytes, rldb_stmt **out, const char **tail);
int rldb_step(rldb_stmt *stmt);
int rldb_reset(rldb_stmt *stmt);
int rldb_finalize(rldb_stmt *stmt);
int rldb_clear_bindings(rldb_stmt *stmt);

// Parameter binding (1-indexed)
int rldb_bind_null(rldb_stmt *stmt, int idx);
int rldb_bind_int64(rldb_stmt *stmt, int idx, int64_t val);
int rldb_bind_double(rldb_stmt *stmt, int idx, double val);
int rldb_bind_text(rldb_stmt *stmt, int idx, const char *val, int nbytes);
int rldb_bind_blob(rldb_stmt *stmt, int idx, const void *val, int nbytes);
int rldb_bind_parameter_index(rldb_stmt *stmt, const char *name);

// Column access
int rldb_column_count(rldb_stmt *stmt);
const char *rldb_column_name(rldb_stmt *stmt, int col);
int rldb_column_type(rldb_stmt *stmt, int col);
int64_t rldb_column_int64(rldb_stmt *stmt, int col);
double rldb_column_double(rldb_stmt *stmt, int col);
const unsigned char *rldb_column_text(rldb_stmt *stmt, int col);
const void *rldb_column_blob(rldb_stmt *stmt, int col);
int rldb_column_bytes(rldb_stmt *stmt, int col);

// Configuration & introspection
int rldb_busy_timeout(rldb *db, int ms);
int rldb_changes(rldb *db);
int64_t rldb_last_insert_rowid(rldb *db);
int rldb_checkpoint(rldb *db);
int rldb_vacuum(rldb *db);
int rldb_stats_json(rldb *db, char **out_json);

// Error
int rldb_errcode(rldb *db);
const char *rldb_errmsg(rldb *db);
void rldb_free(void *ptr);
void rldb_interrupt(rldb *db);

// Backup
int rldb_backup_init(rldb *src, const char *dst, const rldb_config *cfg, rldb_backup **out);
int rldb_backup_step(rldb_backup *backup, int batches);
int rldb_backup_finish(rldb_backup *backup);
int rldb_backup_close(rldb_backup *backup);
int rldb_backup_remaining(rldb_backup *backup);
int rldb_backup_pagecount(rldb_backup *backup);

// Multi-statement execution
typedef int (*rldb_exec_callback)(void*, int, char**, char**);
int rldb_exec(rldb *db, const char *sql, rldb_exec_callback cb, void *ctx, char **errmsg);

// SQLite-compatible aliases (sqlite3_api.rs)
typedef rldb sqlite3;
typedef rldb_stmt sqlite3_stmt;
// ... all rldb_* functions also exported as sqlite3_* equivalents
```

### C-Visible Types (`#[repr(C)]`)

```rust
#[repr(C)]
pub struct rldb_config {
    pub struct_size: u32,           // sizeof(rldb_config) for forward-compat
    pub flags: u32,                 // SQLITE_OPEN_* flag bits
    pub durability: u32,            // 0=Normal, 1=Strict, 2=UnsafeDev
    pub cache_bytes: u64,           // Buffer pool size
    pub work_mem_bytes: u64,        // Per-query memory budget
    pub max_spill_bytes: u64,       // Sort/hash spill limit
    pub statement_cache_capacity: u32,
    pub busy_timeout_ms: u32,
}

#[repr(C)]
pub struct rldb {
    pub(crate) db: Arc<Database>,
    pub(crate) conn: Arc<Connection>,
    pub(crate) path: PathBuf,
    pub(crate) path_text: CString,
    pub(crate) last_code: AtomicI32,
    pub(crate) last_message: Mutex<CString>,
    pub(crate) interrupted: AtomicBool,
    pub(crate) active_statements: AtomicUsize,  // Count of live rldb_stmt for this db
}
```

### Result Codes

| Code | Value | Meaning |
|------|-------|---------|
| `RLDB_OK` | 0 | Success |
| `RLDB_ERROR` | 1 | SQL error or misuse |
| `RLDB_INTERNAL` | 2 | Internal error (caught panic) |
| `RLDB_BUSY` | 5 | Row lock timeout |
| `RLDB_LOCKED` | 6 | Write conflict |
| `RLDB_READONLY` | 8 | Write on read-only |
| `RLDB_INTERRUPT` | 9 | `rldb_interrupt()` called |
| `RLDB_IOERR` | 10 | I/O error |
| `RLDB_SCHEMA` | 17 | Schema changed (statement cache miss) |
| `RLDB_CONSTRAINT` | 19 | Constraint violation |
| `RLDB_MISMATCH` | 20 | Type mismatch or invalid UTF-8 |
| `RLDB_MISUSE` | 21 | API misuse (NULL pointer, OOB index) |
| `RLDB_RANGE` | 25 | Parameter/column index out of range |
| `RLDB_NOTADB` | 26 | Object not found |
| `RLDB_ROW` | 100 | Row available (`rldb_step` result) |
| `RLDB_DONE` | 101 | Execution complete |

### Ownership Model

Every C handle is a heap-allocated Rust struct:

| Handle | Created by | Destroyed by | Guard |
|--------|-----------|--------------|-------|
| `*mut rldb` | `Box::into_raw` in `open_handle()` | `Box::from_raw` in `rldb_close()` | `active_statements == 0` check |
| `*mut rldb_stmt` | `Box::into_raw` in `rldb_prepare_v2()` | `Box::from_raw` in `rldb_finalize()` | Parent `db` pointer valid check |
| `*mut rldb_backup` | `Box::into_raw` in `rldb_backup_init()` | `Box::from_raw` in `rldb_backup_close()` | — |

`rldb_stmt.db` is a non-owning `*mut rldb` — the caller must not call `rldb_close(db)` while any `rldb_stmt` is live. `active_statements` count enforces this: `rldb_close` returns `RLDB_MISUSE` if `active_statements > 0`.

### Panic Safety Wrapper

Every exported C function is wrapped in:

```rust
pub(crate) fn api<T>(f: impl FnOnce() -> T) -> c_int {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(code) => code,
        Err(_)   => RLDB_INTERNAL,  // Panic converted to error code, never UB
    }
}
```

### Unsafe Sites (FFI crate, 46 of 89 total)

All unsafe blocks in the FFI crate are ledgered in `agent/unsafe-ledger.toml`. The primary patterns:

| Pattern | Count | Justification |
|---------|-------|---------------|
| `Box::from_raw(db)` | 3 | Matches `Box::into_raw` in `open_handle`; active_statements guard |
| `Box::from_raw(stmt)` | 2 | Matches `Box::into_raw` in `rldb_prepare_v2`; parent db lifetime |
| `*out_ptr = Box::into_raw(boxed)` | 5 | Ownership transfer to C caller; caller responsible for finalize |
| `caller_buffer(ptr, nbytes)` | 8 | Explicit-length buffer from C; nbytes=-1 = null-terminated, ≥0 = length-prefixed |
| Pointer dereference with null check | 28 | All guarded by preceding null check returning `RLDB_MISUSE` |

### Input Boundary Tests

`crates/ffi/tests/exec_input_boundary.rs`:
- `sql_injection_classic_quote_escape` — bound parameter with `' OR 1=1 --`; assert treated as data
- `sql_injection_stacked_statement` — `SELECT 1; DROP TABLE t;`; assert RLDB_NOTADB for missing table, seeded table survives
- `multi_byte_utf8_handled` — café, 汉字, 😀 (2/3/4-byte codepoints); byte-exact round-trip
- `null_byte_in_bound_parameter` — `[0xDE, 0xAD, 0x00, 0xBE, 0xEF]` as BLOB; length-prefixed nbytes prevents truncation

`crates/ffi/tests/safety_invariants.rs`:
- NULL `*mut rldb` → `RLDB_MISUSE` for all functions
- NULL SQL pointer → `RLDB_MISUSE`
- Invalid UTF-8 in SQL → `RLDB_MISMATCH` (not panic)
- Double-close via NULL → `RLDB_MISUSE`
- Oversize SQL (~1.5 MB) → error code (not panic)
- Parameter index out of range → `RLDB_RANGE`

> **Design note — critique surface:** `active_statements` prevents `rldb_close` while statements are outstanding, but relies on the caller nulling their stmt pointer after `rldb_finalize`. If the caller holds the `*mut rldb_stmt` pointer after finalize and calls any stmt function, the result is undefined behavior (use-after-free). The `sqlite3_api.rs` compatibility shim may drift from upstream SQLite ABI as SQLite adds new entry points (e.g., `sqlite3_prepare_v3`, `sqlite3_bind_pointer`) — each new SQLite ABI addition requires an explicit stub. There is no automated ABI compatibility test against a live SQLite version.

---

## 14. Vector Index

### Metric Definitions

```rust
pub enum VectorMetric {
    L2,            // Squared Euclidean: Σ(a_i − b_i)²  (no sqrt — preserves ranking)
    Cosine,        // 1 − dot(a,b) / (‖a‖ · ‖b‖)  — returns 1.0 if either vector is zero
    InnerProduct,  // −dot(a,b)  — negative so "smaller = closer" convention matches pgvector <#>
}
```

### Scalar Reference Kernels (`vector/distance.rs`)

```rust
fn l2_distance_scalar(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| (x - y) * (x - y)).sum()
}

fn cosine_distance_scalar(a: &[f32], b: &[f32]) -> f32 {
    let (dot, na, nb) = a.iter().zip(b).fold((0.0, 0.0, 0.0), |(d, na, nb), (x, y)| {
        (d + x * y, na + x * x, nb + y * y)
    });
    let denom = na.sqrt() * nb.sqrt();
    if denom == 0.0 { 1.0 } else { 1.0 - dot / denom }
}

fn inner_product_scalar(a: &[f32], b: &[f32]) -> f32 {
    -a.iter().zip(b).map(|(x, y)| x * y).sum::<f32>()
}
```

### Wire Codec

```
+--------------------+----------+------------------+
| dim: varint (LEB128)| kind: 1B | data: 4*dim bytes |
+--------------------+----------+------------------+
```

- `kind = 0x01`: F32 (only implemented kind)
- `kind = 0x02`: F16 (reserved)
- `kind = 0x03`: I8 (reserved)
- All floats in little-endian byte order (cross-platform byte equality).

### SIMD Dispatch (`vector/simd.rs`)

```rust
fn dispatch_l2(a: &[f32], b: &[f32]) -> f32 {
    #[cfg(target_arch = "x86_64")]
    if is_x86_feature_detected!("avx2") {
        return unsafe { l2_avx2(a, b) };
    }
    #[cfg(target_arch = "aarch64")]
    if std::arch::is_aarch64_feature_detected!("neon") {
        return unsafe { l2_neon(a, b) };
    }
    l2_distance_scalar(a, b)
}
```

Runtime feature detection on first call (no cost after warm-up).

### Flat Scan (`vector/flat.rs`)

```rust
pub fn flat_top_k<P, I>(query: &[f32], metric: VectorMetric, k: usize, candidates: I)
    -> Vec<FlatScanHit<P>>
where I: IntoIterator<Item = (P, Vec<f32>)>
```

Uses a fixed-capacity max-heap of size `k`. For each candidate: compute distance, maintain k smallest (evict maximum). Returns sorted ascending by distance. O(n·d·log k) time, O(k) space.

### ANN Indexes

- **HNSW** (`vector/hnsw/`): Hierarchical Navigable Small World graph (Lane V2). Configurable `ef_construction`, `ef_search`, `m` (max connections per node).
- **DiskANN** (`vector/diskann/`): Sector-aligned on-disk graph index (Lane V3). Uses `unsafe { slice::align_to::<f32>() }` for alignment validation (ledgered in `agent/unsafe-ledger.toml`).

### `vector_v1_unmerged` Feature Flag

Gates distance.rs items during the Lane V1 → V2 transition period. Currently defaults to enabled (`default = ["vector_v1_unmerged"]` in kernel). Will be deleted in the fusing commit that makes Lane V1 scalar kernels superseded by Lane V2 SIMD. Not user-visible.

> **Design note — critique surface:** `l2_distance_scalar` returns *squared* Euclidean distance, not Euclidean distance. This is correct for nearest-neighbor ranking (ordering is preserved) but will surprise applications that expect a metric satisfying the triangle inequality — squared L2 is not a proper metric. There is no schema-level dimensionality constraint on vector columns: inserting a 512-dim vector into a column that has only ever seen 128-dim vectors is a runtime error, not a DDL error. The HNSW index has no persistence guarantee in the current Lane V2 implementation — index state may require rebuild after restart (verify in `hnsw/` before relying on durability).

---

## 15. Certification & Proof Infrastructure

### 1728-Child Certification Matrix

`crates/bench/src/certify/scheduler.rs`

**Build job queue:**
```rust
fn build_job_queue(config: &CertifyConfig, warmup: usize, measured: usize) -> VecDeque<Job>
```
Generates `(engine × workload × durability × threads) × (warmup + measured)` jobs in deterministic FIFO order. Warmup rounds run first; measured rounds follow.

**Parallel dispatch:**
```rust
fn dispatch_parallel(jobs: VecDeque<Job>, out_dir: &Path, with_strace: bool)
```
- `available_cores()`: `num_cpus::get() - RESERVED_CORES` (RESERVED_CORES = 4).
- Honors `REDLINEDB_BENCH_MAX_PARALLEL_THREADS` env var override.
- Greedy bin-packing: spawn next job when a slot frees (100ms poll).
- Kill-switch: `REDLINEDB_BENCH_KILL=1` → exit at next workload boundary.

**Scale breakdown (certification.toml):**

| Axis | Values | Count |
|------|--------|-------|
| Threads | 1, 2, 4, 8, 16, 32, 64 | 7 (but certification uses subset per workload) |
| Workloads | 9 core workloads | 9 |
| Durabilities | Normal, Strict, UnsafeDev | 3 |
| Repetitions per combo | 5 measured + 1 warmup | 6 |
| **Total child processes** | **1728** | |

### Workload Catalog

| Category | Workload | Description |
|----------|----------|-------------|
| OLTP write | `SingleRowInsert` | One INSERT per transaction |
| OLTP write | `BatchedInsert100` | 100 INSERTs per transaction |
| OLTP write | `WritersDisjoint` | Concurrent writers on non-overlapping key ranges |
| OLTP mixed | `MixedOLTP-95/5` | 95% reads, 5% writes |
| OLTP mixed | `MixedOLTP-80/20` | 80% reads, 20% writes |
| OLTP mixed | `MixedOLTP-50/50` | 50% reads, 50% writes |
| Read | `PointReadPk` | Single-row by primary key |
| Read | `SecondaryIndexRead` | Point lookup on secondary index |
| Read | `SecondaryIndexRange` | Range scan on secondary index |
| Read | `HotRowUpdate` | Repeated update of same row (lock contention) |
| Feature | `JsonPathExtract` | `json_extract()` workload |
| Feature | `VectorFlatSearch` | Brute-force top-K vector search |
| Feature | `VectorAnnSearch` | Approximate nearest-neighbor search |
| Chaos | `ChaosLockConvoy` | Many writers competing for same row lock |
| Chaos | `ChaosConnectionChurn` | Rapid open/close cycles |
| Chaos | `ChaosCheckpointThrash` | Checkpoint triggered mid-transaction |
| Chaos | `ChaosIndexHammer` | Concurrent inserts into same B-tree leaf |
| Chaos | `ChaosSortSpillConvoy` | Memory-limited sort spills on concurrent queries |
| Chaos | `ChaosSchemaStorm` | Rapid DDL changes invalidating statement caches |

### Fsynced-Ack Durability Protocol

Used by both failpoint matrix and recovery matrix:

```
Child process:
  1. INSERT INTO log VALUES (key, payload)
  2. COMMIT (group-commit WAL, fsync)
  3. append "key\n" to ack_log file
  4. fsync(ack_log)
  [process killed here by parent]

Parent process (after restart):
  1. Open fresh database
  2. acknowledged = count lines in ack_log
  3. recovered = SELECT COUNT(*) FROM log
  4. ASSERT recovered >= acknowledged
  5. ASSERT lost_acked_commits == 0  ← hard gate
```

Any `recovered < acknowledged` is an immediate test FAIL. Zero tolerance.

### Three-Axis Dataset Checksum

`crates/bench/src/checksum.rs`

```rust
struct DatasetChecksum {
    row_count: u64,        // Exact count (detects missing/extra rows)
    key_xor: u64,          // XOR of SHA-256(key) per row (order-independent; detects missing keys)
    payload_hash: u64,     // SHA-256 with per-row framing (order-sensitive; detects value corruption)
}
```

The three axes are independent: row_count catches count drift, key_xor catches key drift without caring about order, payload_hash catches value corruption in a specific row. A result where all three match across two runs gives high confidence in dataset equivalence.

### Certification Manifest (JSON)

```json
{
  "out_dir": "target/bench/xbabe1/certification",
  "git_sha": "...",
  "git_dirty": false,
  "image_digest": "sha256:...",
  "pragmas": {
    "redline": { "page_size": "16384", "wal_autocheckpoint": "1000" },
    "sqlite":  { "page_size": "4096",  "wal_autocheckpoint": "1000" }
  },
  "checksums": {
    "runs.jsonl": "sha256:...",
    "summary.csv": "sha256:..."
  },
  "process_metrics_per_run": [
    { "rss_bytes": 45678912, "fdatasync_count": 142, "pwrite_count": 1847 }
  ],
  "warmup_runs_per_combo": 1,
  "measured_runs_per_combo": 5
}
```

Same `git_sha` + `image_digest` + `seed` → byte-comparable output files (verified via SHA-256 checksums).

### Current Certification Results (xbabe1, 128 vCPU, Linux 6.8, Rust 1.95)

| Workload | RedlineDB | SQLite | Ratio |
|----------|-----------|--------|-------|
| writers-disjoint (64T) | 1,256 qps | 79 qps | **15.89×** |
| mixed-95/5 (64T) | 24,763 qps | 1,680 qps | **14.74×** |
| mixed-80/20 (64T) | 6,154 qps | 405 qps | **15.21×** |
| point-read-pk (64T) | 121,268 qps | 122,221 qps | **0.99×** |

Crash certification: **36/36** recovery cases, **24/24** failpoint cases — zero lost acked commits.

SQL compat: the focused SQLite parity suites currently cover **121 passing tests** with **0 ignored parity tests**. This is not a full SQLite claim; [docs/sqlite-parity.md](../sqlite-parity.md) tracks remaining `fail` and `not-started` rows such as SQLite file format, broad `sqlite3_*` API coverage, views, triggers, CTE execution, window functions, generated columns, partial/expression indexes, and broader PRAGMA/function coverage.

### Failpoint Placement Summary

Strategic kernel failpoints (all via `fail_point!` macro, zero cost when feature=off):

| Failpoint name | Location | Crash scenario |
|----------------|----------|----------------|
| `wal::flush_until` | `coordinator.rs` | Crash before WAL durability signal |
| `storage::control::write` | `control.rs` | Crash between ControlFile write and fsync |
| `engine::commit::before_publish` | `runtime.rs` | Crash after WAL fsync, before ConcurrentTxStatus update |
| `index::split::after_alloc` | `mutate.rs` | Crash during B-tree split after new page allocated |
| `heap::insert::after_wal` | `mutation/write.rs` | Crash after HeapInsert WAL record, before in-memory state |

---

## 16. Design Tradeoffs & Known Limitations

This section consolidates the critique surfaces from each component section.

### Concurrency & Isolation

| Limitation | Impact | Mitigation |
|------------|--------|-----------|
| Snapshot isolation only (no SSI) | Write skew anomalies possible | Application-level locking; explicit `SELECT ... FOR UPDATE` pattern |
| No serializable cycle detection | True ACID serializable not guaranteed | Document as limitation; users needing serializability must use advisory locks |
| B-tree `structure_lock` serializes all inserts | High write concurrency to same table serializes | B-link tree protocol would eliminate; accepted tradeoff for implementation simplicity |
| Row lock timeout (no deadlock detection) | Deadlock cycle → both transactions time out | Application must impose lock ordering |

### Storage & WAL

| Limitation | Impact | Mitigation |
|------------|--------|-----------|
| Fixed 16 KB page size | Large BLOBs inefficient; not tunable | Variable page size requires format change |
| CLOCK eviction (approximate LRU) | May evict recently-accessed pages under adversarial patterns | LRU/ARC would improve; CLOCK is simpler |
| Single WAL writer thread | High-throughput append workloads may saturate | Parallel WAL inserters (Postgres-style WAL buffer) would scale; significant redesign |
| 200 µs group-commit floor | Latency floor for single-writer workloads | Configurable per deployment; reduce for low-latency at expense of throughput |
| PageImage records (full 16 KB) | High WAL amplification for small mutations | Page-delta records would reduce amplification; requires careful recovery logic |

### Recovery

| Limitation | Impact | Mitigation |
|------------|--------|-----------|
| No undo phase | Aborted ghost versions remain until vacuum | Vacuum must run; high abort rate accumulates storage debt |
| Redo-only recovery | Simple implementation; correct | Fundamental design choice |
| Catalog in `.redline_catalog` (SQLite) | Bootstrap dependency; catalog recovery via WAL fallback | Consider native catalog page format for cleaner recovery story |

### SQL Engine

| Limitation | Impact | Mitigation |
|------------|--------|-----------|
| No histogram cardinality estimation | Bad plans for skewed distributions | ANALYZE collects stats; planner should use histogram buckets |
| No join reordering for 3+ tables | Suboptimal plans for multi-table joins | Dynamic programming join enumeration (Postgres-style) |
| No partial / expression indexes | Cannot index `lower(email)` etc. | Not planned |
| CHECK / FOREIGN KEY not enforced | Applications must enforce application-side | Known gap; tracked in FEATURE_GAPS.md |
| Savepoint replay is SQL-level | Non-deterministic functions yield different results on replay | Document; PostgreSQL WAL-level savepoints would be more correct |
| Statement cache invalidated globally on any DDL | Schema storms cause re-planning storm | Table-specific cache invalidation would scope the impact |

### Vector

| Limitation | Impact | Mitigation |
|------------|--------|-----------|
| L2 returns squared distance | Not a proper metric; may surprise users | Document clearly; rename to `l2_squared_distance` is a possible API change |
| No dimension constraint at schema level | Runtime error on dimension mismatch | Add DDL `VECTOR(dim)` type with schema-level validation |
| HNSW durability (Lane V2) | Index may not survive restart without rebuild | Verify persistence semantics in `hnsw/` before production use |

### FFI / ABI

| Limitation | Impact | Mitigation |
|------------|--------|-----------|
| `sqlite3_api.rs` may drift from upstream SQLite | New SQLite entry points not automatically available | Automated ABI diff testing against sqlite3.h |
| No thread-safety guarantees across handles | Concurrent access to same `*mut rldb` from multiple threads is undefined | Document: one `*mut rldb` per thread, or external mutex |
| `active_statements` prevents close but not use-after-finalize | Caller holding stmt ptr post-finalize is UB | Cannot enforce in C ABI; document contract |

---

## 17. Key Constants Reference

| Constant | Value | Location | Description |
|----------|-------|----------|-------------|
| `PAGE_SIZE` | 16,384 bytes | `format/page.rs` | All page I/O unit |
| `PAGE_HEADER_LEN` | 64 bytes | `format/page.rs` | Fixed header at page start |
| `SLOT_LEN` | 4 bytes | `format/page.rs` | Per-slot overhead in slot array |
| `INDEX_SPECIAL_LEN` | 256 bytes | `index/cells.rs` | B-tree special region per page |
| `TUPLE_HEADER_LEN` | 72 bytes | `format/tuple.rs` | TupleVersion fixed header |
| `WAL_RECORD_HEADER_LEN` | 48 bytes | `wal/record.rs` | WalRecord fixed header |
| `WAL_SEGMENT_BYTES` | 64 MB | `wal/manager.rs` | WAL file rotation threshold |
| `GROUP_COMMIT_DELAY_US` | 200 µs | `wal/manager.rs` | Group commit batch collection window |
| `GROUP_COMMIT_MAX_BATCH_BYTES` | 4 MB | `wal/manager.rs` | Group commit size trigger |
| `GROUP_COMMIT_HISTOGRAM_BUCKETS` | 16 | `wal/manager.rs` | Power-of-two WAL batch size histogram |
| `RESERVED_CORES` | 4 | `bench/certify/scheduler.rs` | CPU cores reserved for OS during bench |
| `BENCH_POLL_INTERVAL_MS` | 100 | `bench/certify/scheduler.rs` | Child process reap poll interval |
| `UNKNOWN_EQ_SELECTIVITY` | 0.10 | `sql/planner/helpers.rs` | Default `=` predicate selectivity |
| `UNKNOWN_RANGE_SELECTIVITY` | 0.33 | `sql/planner/helpers.rs` | Default range predicate selectivity |
| `SEQ_PAGE_COST` | 1.0 | `sql/planner.rs` | Sequential page fetch cost unit |
| `RANDOM_PAGE_COST` | 4.0 | `sql/planner.rs` | Random page access cost unit |
| `CPU_TUPLE_COST` | 0.01 | `sql/planner.rs` | Per-tuple processing cost |
| `INDEX_PROBE_STARTUP` | 2.0 | `sql/planner.rs` | B-tree descent startup cost |
| `RLDB_OK` | 0 | `ffi/types.rs` | C ABI success code |
| `RLDB_ROW` | 100 | `ffi/types.rs` | C ABI "row available" code |
| `RLDB_DONE` | 101 | `ffi/types.rs` | C ABI "execution complete" code |
| `RLDB_MISUSE` | 21 | `ffi/types.rs` | C ABI NULL pointer / API misuse |
| `TUPLE_FLAG_DELETED` | `0x0001` | `format/tuple.rs` | Soft-delete flag in TupleVersion |
| `WAL_MAGIC` | `0x5244_574c` | `wal/record.rs` | "RDWL" magic bytes |
| `PAGE_MAGIC` | `0x5244_5047` | `format/page.rs` | "RDPG" magic bytes |
| `VECTOR_KIND_F32` | `0x01` | `kernel/vector/codec.rs` | F32 element kind in vector wire format |

---

*This document is maintained alongside the codebase. On any change that alters a data structure, format, algorithm, or constant described here, update the corresponding section. File is exempt from `scripts/check_file_sizes.sh` line-count limit (generated spec artifact, see exemption in line ~25 of that script).*
