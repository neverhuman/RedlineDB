# Changelog

## Unreleased — Phase 10 (long-range closure)

### Kernel

- `CommitOutcome::MaybeCommitted` propagated through engine + SQL so
  post-fsync failures are no longer reported as ordinary rollback.
- Index format v2 with per-entry `(create_tx, delete_tx)` MVCC tags
  replacing the boolean `dead` flag; `point_lookup_visible` and
  `range_scan_visible` accept `(ConcurrentTxStatus, Snapshot)` for
  three-valued visibility.
- v1 → v2 index migration on `Engine::open`.
- Transactional index-handle queueing in `Txn` so rollback never exposes
  uninstalled indexes.
- Group-commit telemetry: 16-bucket batch-size histogram + p50/p95/p99/max
  on `WalSyncCounters`; opt-in per-core lane coordinator (default 1 lane);
  semantic counter combiner stub (gated, `unimplemented!()`).
- New `crates/kernel/src/integrity/{heap,index,equivalence,page_csum}.rs`:
  visible-row heap walk, full index tree dump, heap↔index cross-check,
  page checksum verifier, LSN monotonicity audit.
- New `crates/kernel/src/json/{wire,encode,decode,path_bytecode,simd_key}.rs`:
  binary JSONB format (magic 0x96, format-v1, type tags 0x00..0x08, LEB128
  varints, zig-zag i64), SIMD path-key compare, compiled path bytecode.
- New `crates/kernel/src/vector/{mod,distance,simd,codec,flat}.rs`:
  VECTOR type with AVX2/NEON/scalar dispatch, L2 / Cosine / InnerProduct,
  exact flat top-K scan.
- New `crates/kernel/src/vector/hnsw/{builder,searcher,storage,levels}.rs`:
  HNSW index (M=32, efC=200, recall@10 = 0.95 at efS=64).
- New `crates/kernel/src/vector/diskann/{builder,searcher,sectors,prune}.rs`:
  DiskANN-style Vamana graph (R=64, alpha=1.2, recall@10 = 0.99).

### SQL

- SAVEPOINT / RELEASE / ROLLBACK TO via journal-and-replay.
- Multi-statement parser + `Connection::prepare_v2` returning unconsumed
  remainder; FFI `sqlite3_prepare_v2` + `pzTail`; multi-stmt
  `sqlite3_exec`; errmsg via `CString::into_raw` + `sqlite3_free`.
- Centralized SQLite ON CONFLICT matrix:
  `INSERT OR ABORT/FAIL/IGNORE/REPLACE/ROLLBACK` with NOT NULL / CHECK /
  UNIQUE / PK; `INTEGER PRIMARY KEY` AUTOINCREMENT-style high-water-mark
  through delete + recovery; UPSERT `DO UPDATE` / `DO NOTHING`.
- Wrong-result fixes: SELECT ALL, NOT IN NULL three-valued, NULL || x,
  divide / modulo by zero return NULL, scalar function NULL propagation,
  CAST follows SQLite truncation/prefix-parse, GLOB bracket / range /
  negation, grouped + DISTINCT ORDER BY honors keys.
- New `crates/sql/src/json/`: full SQLite JSON1 surface — json,
  json_array, json_array_length, json_object, json_extract, json_set,
  json_insert, json_replace, json_remove, json_patch (RFC 7396),
  json_type, json_valid, json_quote, json_minify; `->` / `->>` operators.
- New `crates/sql/src/exec/vec/`: vectorized executor scaffolding —
  selection vectors, top-K min-heap (k≤64 from `MaterializedTopN`),
  hash aggregation with spill, external merge-sort with spill.
- VECTOR(d[, f32]) column type + `<=>` cosine-distance overload;
  `vector_*` scalar functions backed by `kernel::vector`.
- Tier-1 SQLite surface: REGEXP, date/time (date, time, datetime,
  julianday, strftime, unixepoch + modifiers), collations
  (BINARY/NOCASE/RTRIM).
- Tier-1 parser-only with execute-time errors: FK declarations,
  ALTER TABLE DROP COLUMN, partial indexes, expression indexes.
- Tier-2/3 parser-only: CTEs, CREATE VIEW, CREATE TRIGGER, window
  functions, generated columns.
- New PRAGMAs: `redline_index_check`, `redline_full_check`.
- `user_version` persisted to `user_version.redline` sidecar.
- SQL-side index undo log removed; mutations ride kernel index MVCC.

### Bench

- New `crates/bench/src/checksum.rs`: deterministic `DatasetChecksum`
  (`row_count`, `key_xor`, `payload_hash`) replacing the `MAX(k)` /
  `COUNT(*)` placeholder. Manifest `checksums` field consumes the new
  struct.
- `large-sort-spill` workload registered (Lane VE).
- WAL group-commit batch histogram + per-core lane counters surfaced
  through `WalSyncCountersSnapshot`.

### Tests

691 passing, 3 ignored (vs 241 wave-7-fused; +450 phase-10 tests).

### Tags

`phase10-baseline`, `phase10-wave1-partial`, `phase10-wave2-fused`.

## Earlier

- Repository hygiene and agent-readiness updates.
- Workspace proof lanes, contribution guidance, and file-size policy tightening.
