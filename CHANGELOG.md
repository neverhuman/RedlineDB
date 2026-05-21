# Changelog

## Unreleased

## [1.0.21] - 2026-05-21

SQLite shell parity push 5.

### Changed

- SQLite parity CI coverage now approves 1049 generated cases, including
  shell terminators, additional dot-command smoke cases, typed CLI
  parameters, selected tempfile shell workflows, and generated scalar
  null/coalesce cases.
- Workspace package metadata and lockfile entries now target `1.0.21`.

## [1.0.20] - 2026-05-21

Release-only version bump for the current SQLite parity branch.

### Changed

- SQLite parity coverage was expanded on this branch, and the latest parity
  report artifacts remain aligned with the approved CI allowlist.
- Workspace package metadata and lockfile entries now target `1.0.20`.

## [1.0.19] - 2026-05-20

Latency pass 3 for volatile SQLite parity cases.

### Changed

- Private in-memory databases now use an internal volatile engine path that
  skips WAL writer startup, WAL segment creation, catalog sidecar writes, and
  user-version sidecar writes while keeping persistent databases on the
  durable path.
- CLI `list`, `tabs`, and `csv` output modes now stream rows directly from
  stepped statements instead of materializing full result sets first.
- `OpenOptions::statement_cache_capacity` now flows into the SQL statement
  caches, and private in-memory opens use smaller default lock/cache/heap
  sizing for one-shot scripts.
- SQLite parity latency report artifacts were regenerated on 2026-05-20 after
  the volatile fixed-cost reductions.
- Workspace package metadata and lockfile entries now target `1.0.19`.

## [1.0.18] - 2026-05-20

Latency round 2 for volatile SQLite parity cases.

### Changed

- Private volatile databases now honor explicit `OpenOptions::temp_dir` roots
  and otherwise prefer `/dev/shm/redlinedb-ephemeral` when writable before
  using the process scratch directory. This brings default `:memory:` backing
  roots closer to SQLite memory-profile latency on Linux.
- Nested SELECT, scalar subquery, and `IN (SELECT ...)` evaluation now reuse
  the enclosing SELECT transaction snapshot when one exists.
- `EXISTS (SELECT ...)` now stops after the first matching subquery row instead
  of materializing every row.
- SQLite parity latency report artifacts were regenerated on 2026-05-20. The
  previous `JOIN_SUBQUERY_EXISTS` and P0 memory gaps are materially reduced.
- Workspace package metadata and lockfile entries now target `1.0.18`.

## [1.0.17] - 2026-05-20

SQLite dynamic-default compatibility and release version alignment.

### Fixed

- `CURRENT_DATE`, `CURRENT_TIME`, and `CURRENT_TIMESTAMP` column defaults now
  parse, persist through catalog reopen, evaluate at insert time, and appear in
  `PRAGMA table_info` output using SQLite-compatible default text.
- `redlinedb --version` now identifies the RedlineDB release version while
  still reporting SQLite 3.45.1 compatibility, instead of printing only the
  SQLite compatibility version.

### Added

- SQLite parity coverage for current date/time defaults, including the Jansu
  `cluster` table default shape used by storage integration smoke tests.

### Changed

- Workspace package metadata and lockfile entries now target `1.0.17`.

## [1.0.16] - 2026-05-20

Release-readiness pass for CI and local proof lanes.

### Fixed

- Nightly fuzz CI installs `mold` before running `ops/ci/nightly-fuzz.sh`,
  matching the linker expected by the release fuzz lane.

### Changed

- Fast CI now smoke-tests the checksum-verified RedlineDB `v1.0.1` Linux
  release binary from the project GitHub release before current-branch tests.
- CI and local jankurai gates now install the pinned `jankurai` `v1.5.1`
  GitHub release binary, verify its `.sha256` file, and install runtime schema
  data for the release binary instead of building jankurai from source.
- Workspace package metadata and lockfile entries now target `1.0.16`.

SQLite parity truth pass + faster, blocking jankurai pre-commit hook.

### Added

- **SQLite CASE aggregate parity**: grouped `CASE` expressions now evaluate
  aggregate-containing conditions and branches instead of rejecting them, so
  queries like `CASE WHEN count(*) > 2 THEN ... END` match SQLite. Simple
  `CASE` now also follows SQLite null semantics for `CASE NULL WHEN NULL`.

### Added

- **SQL ingress compatibility hardening**:
  - `PRAGMA journal_mode = WAL` now round-trips as `wal` for RedlineDB's
    WAL-style journal, while `truncate` / `persist` stay rejected.
  - Compound `SELECT` now shares parameter slots across branches and tail
    `ORDER BY` / `LIMIT` wrappers.
  - Nested `SELECT` wrappers with trailing `ORDER BY` / `LIMIT` now bind
    correctly instead of rejecting the wrapper form.
  - `WITH ... AS MATERIALIZED` / `AS NOT MATERIALIZED` CTE hints are
    accepted as no-op syntax.
  - The parser boundary now catches upstream `sqlparser` panics and
    converts them into `Error::Parse`.
- **SQLx attach mode**: `redlinedb-sqlx` now parses `mode=rwc` / `mode=ro`
  on RedlineDB URLs. Owning/server processes keep the existing owner-lock
  behavior with `mode=rwc`; dashboard/TUI/inspection clients can attach
  read-only to a live file-backed database with `mode=ro` and get a read-only
  error on writes.
- **SQLite parity coverage expansion**: `sqlite_full_parity.rs` now writes a
  reference-build PRAGMA corpus from bundled SQLite metadata and asserts the
  remaining unsupported PRAGMAs and SQLite-native file-format gaps explicitly;
  `parity_oracle` now requires 25 seed files per tag.
- **SQLite parity receipts**: `just sql-parity-full` now regenerates the
  required `target/proof/sqlite-full-parity/` receipts for git status, diff
  stat, rusqlite reference metadata, unsupported SQL sites, ignored tests,
  sqllogictest inventory, and SQL parity test inventory.
- **SQLite parity ledger lint**: the fast preflight lane rejects `pass` rows in
  `docs/sqlite-parity.md` whose notes admit known gaps, and prevents rejected
  PRAGMA rows from being counted as parity passes.
- **PRAGMA truth pass**: real implementations for `PRAGMA journal_mode`
  (`memory`/`off`/`delete`), `synchronous`, `temp_store`, `cache_size`,
  `query_only` round-trip on the session; `query_only` additionally blocks
  every write-side statement (Insert/Update/Delete/CreateTable/AlterTable
  /Drop*/CreateIndex/CreateView/CreateTrigger) with
  `attempt to write while PRAGMA query_only is set`.
- **JSON1 oracle parity** (`crates/sql/tests/parity_json1.rs`): 32
  rusqlite-oracle tests covering `json()`, `json_array[_length]`,
  `json_object`, `json_extract`, `json_type`, `json_valid`, `json_quote`,
  `json_set`/`json_insert`/`json_replace`/`json_remove`, `json_patch`,
  and the `->` / `->>` arrow operators. JSON1 row in
  `docs/sqlite-parity.md` flips from `fail` to `pass`.
- **Operator parity lock-in** (`crates/sql/tests/parity_operators.rs`):
  oracle-compared `||`, `REGEXP` operator/UDF, `LIKE`, and
  `INSERT/UPDATE/DELETE ... RETURNING`. `ILIKE` is RedlineDB-only
  (positive tests on our side); `ILIKE ANY` stays a negative test.
- **CLI dot commands**:
  - `.fullschema [PATTERN]` — `.schema` plus `SELECT * FROM sqlite_master`.
  - `.once FILE` — one-shot redirect for the next statement.
  - `.parameter set|unset|init|clear|list` — named-parameter binding
    applied to the next prepared statement via `bind_named`.
- **Fast staged-files pre-commit hook**
  (`tools/jankurai-hooks/pre-commit`): runs `jankurai audit-file`
  per staged file in save-gate mode with the HEAD revision (or empty file
  for new paths) as the baseline. Blocks on any new hard finding.
  Typical commits now run <2 s instead of 10–60 s.
  `JANKURAI_SKIP_HOOKS=1` and `JANKURAI_PRE_COMMIT_CHAIN` still work.
- **CI staged-gate** (`.github/workflows/jankurai.yml`,
  `ops/ci/jankurai-staged-gate.sh`): PR runs the same per-file save-gate
  against `origin/main`'s merge base so PRs can't sneak past local
  bypasses.
- **Hook integration test**
  (`tools/jankurai-hooks/tests/pre_commit_blocks.sh`).

### Changed (potentially BREAKING for callers that probe unknown PRAGMAs)

- `sql-parity-full` now fails on any SQLite parity corpus divergence after
  writing `baseline-divergence.txt`; the corpus is no longer a non-fatal
  baseline recorder.
- The fuzz parity gate no longer skips implemented CTE or compound SELECT
  forms, and a missing fuzz baseline only passes when the current run observes
  zero divergences.
- SQLite parity documentation now distinguishes `pass`, `partial`, `fail`,
  `not-started`, and `rejects-by-design` so covered subsets and intentional
  PRAGMA rejections are not counted as full parity.
- `PRAGMA auto_vacuum` and `PRAGMA wal_checkpoint(MODE)` previously
  returned fabricated rows; they now return `UnsupportedSql`. Callers
  that branched on the row shape need to handle the error instead.
- Any PRAGMA RedlineDB does not implement now returns
  `UnsupportedSql("PRAGMA <name> is not supported by RedlineDB")` rather
  than silently falling through.
- `redlinedb-cli`'s query runner now writes through an `io::Write` sink
  so `.once` can redirect a single statement; default sink stays
  `io::stdout()` so behaviour is unchanged for non-`.once` callers.

### Notes

- Jankurai 1.4.3 is the supported version.

## [1.0.8] - 2026-05-18

### Added

- `redlinedb-sqlx` now registers both SQLx `Any` URL schemes used by Jeryu
  autonomy ledgers: canonical `redline://` and compatibility alias
  `redlinedb://`. Mixed-case inputs such as `redlineDB://` are accepted after
  URL scheme normalization.

### Notes for Jeryu consumers

- Preferred autonomy ledger URL:
  `redline:///absolute/path/to/target/jeryu/autonomy.redlineDB`.
- Compatibility alias:
  `redlineDB:///absolute/path/to/target/jeryu/autonomy.redlineDB`.

## [1.0.2] - 2026-05-17

New crate **`redlinedb-tokio`** — a tokio async adapter that wraps the sync
`Database`/`Connection` core in a sqlx::Pool-shaped surface. Lets async
tokio crates (e.g. jeryu) consume RedlineDB without writing
`spawn_blocking` by hand.

### Added

- `crates/redlinedb-tokio/` — new workspace member.
  - `Pool` — clone-cheap async pool; bounded by a tokio semaphore.
    - `Pool::open(path)` / `Pool::open_in_memory()` constructors.
    - `Pool::execute / fetch_one / fetch_optional / fetch_all` async methods
      mirroring `sqlx::Pool` ergonomics.
    - `Pool::with_connection(closure)` for multi-step ops on one connection.
    - `Pool::transaction(closure)` — auto BEGIN/COMMIT/ROLLBACK.
  - `AsyncRow` — owned, `Send + Sync + Clone` row materialized from the
    borrowed `redlinedb::Row` so it survives `.await` boundaries.
  - `PoolBuilder` — fluent config (max_connections, busy_timeout).
- 9 integration test files covering smoke, concurrent writes (16 producers /
  100 inserts each / no lost rows), transaction commit + rollback, params
  binding for every `Value` variant, error propagation across `.await`,
  builder settings, persistent file-backed pools, clone semantics, and
  multi-step closures.
- One example: `cargo run --example async_round_trip -p redlinedb-tokio`.

### Changed

- All workspace crate versions bumped 1.0.0 → 1.0.2 in sync (no source
  changes outside of `redlinedb-tokio` and the workspace `Cargo.toml`).
- Workspace member list now includes `crates/redlinedb-tokio`.

### Notes for downstream consumers

- The new crate is additive; existing `redlinedb` callers are unaffected.
- `redlinedb-tokio` re-exports the common types (`Database`, `Connection`,
  `Error`, `Value`, `params!`, etc.) so migrating callers can `use
  redlinedb_tokio::*` without pulling `redlinedb` directly.

## [1.0.1] - 2026-05-16
Jankurai score repair cycle, CI hardening, and install-story improvements.
No FFI ABI break; downstream consumers unaffected.

### Score motion

- Final score: 88 → 91 (0 caps, 2 medium findings both disabled in policy)
- Tool adoption: 26 → 61/100 (16/16 tools configured, 7/16 with CI evidence)
- Workspace tests: 928 passing

### CI / install

- Inlined all `jankurai` steps directly in `.github/workflows/jankurai.yml`;
  scanner now sees `run: jankurai ...` YAML patterns (was dispatching to
  shell script, invisible to tool-adoption scanner)
- Fixed `CI_JANKURAI_GIT` URL typo in `ops/ci/lib.sh`
  (`jepsontaylor` → `jeppsontaylor`)
- Added `proofbind`, `proofmark-rust`, `copy-code` to
  `.jankurai/tool-adoption.toml` (13 → 16 tools configured)
- Committed `.jankurai/baselines/main.repo-score.json`; CI baseline step now
  falls back to local copy on first-commit of the file
- Exempted `.jankurai/baselines/*` from `scripts/check_file_sizes.sh` 2000-line
  hard limit (generated score artifacts, same class as `.jankurai/repo-score.json`)
- README install section expanded: exact version-pin examples for Cargo,
  `VERSION=v1.0.x` for CLI script, `cargo install --version --locked`,
  and `--git --tag --locked`
- Added `[features]` to `crates/redlinedb/Cargo.toml` with `failpoints`
  routing through to kernel+sql (clearly marked internal/test-only)

### Caps lifted (9)

- `repo-rot-bad-behavior` (B): renamed `certification-phase10-v3*.toml`,
  rewrote `backup.rs:1` doc comments.
- `python-direct-product-truth-or-db-ownership` (B): ported
  `scripts/bench/dick_head_choas_report.py` to `crates/bench/src/bin/chaos_report/`.
- `no-agent-friendly-exception-pattern` (F): added typed `DomainError` in
  `crates/domain/`, wired one kernel error path through it.
- `missing-agent-readable-docs` (F): authored `docs/{audit-rubric,
  language-bad-behavior,testing,release,architecture,boundaries}.md`.
- `vibe-placeholders-in-product-code` + `future-hostile-dead-language-in-product-code`
  (C1–C4): renamed dead-marker terms across bench, kernel, sql, ffi.
- `release-readiness-gap` (H): authored `docs/release.md`,
  `.jankurai/cost-budget.toml`; wired security CI gates.
- `non-optimal-product-language-found` (J4): relocated
  `crates/ffi/include/redlinedb.h` → `contracts/c-abi/redlinedb.h`.
- `fallback-soup-in-product-code` (J1a–d + followups): collapsed ~237
  closure-form `unwrap_or_else` / `ok_or_else` / `or_else` chains into
  explicit `match` blocks across sql, kernel, bench, ffi, redlinedb,
  server, cli.

### Caps still applied (4)

- `severe-duplication-in-product-code` (70): one cross-file structural
  duplicate at `crates/kernel/src/catalog/ops.rs:61/91` (early-return
  after duplicate-check pattern). Lifting requires substantive refactor.
- `authz-or-data-isolation-gap` (78): tests in
  `crates/bench/tests/tenant_isolation.rs` + `security-policy.toml`
  proof routes added; auditor's HLT-022 detector link unclear.
- `input-boundary-gap` (78): tests in
  `crates/ffi/tests/{safety_invariants,exec_input_boundary}.rs`; same
  detector-link gap as authz.
- `rust-bad-behavior` (72): jankurai 0.8.16's `rust.unsafe.raw-parts`
  hard rule fires unconditionally on `Box::from_raw` / `from_raw_parts`
  regardless of SAFETY comments or ledger entries. Five FFI ownership-
  transfer sites are intrinsic to the C ABI; lifting requires upstream
  jankurai patch.

### Code shape

- Split `crates/sql/src/connection.rs` (972 LOC) → `connection/{mod,
  cache,options,database,session,tests}.rs` (G).
- Split `crates/sql/src/exec/expr/scalar.rs` (957 LOC) → `scalar/{mod,
  math,pattern,value,row,tests}.rs` (J6a).
- Split `crates/bench/src/bin/chaos_report.rs` (1148 LOC) →
  `chaos_report/{main,args,read,normalize,compare,write}.rs` (J6b).
- Split `crates/bench/src/chaos.rs` → `chaos/{mod,helpers,lock_convoy,
  connection_churn,checkpoint_thrash,index_hammer,sort_spill_convoy,
  schema_storm,tests}.rs` (J2).

### FFI surface

- Renamed module `crates/ffi/src/sqlite3_compat.rs` →
  `sqlite3_api.rs` (`pub use sqlite3_api as sqlite3_compat;` keeps
  internal Rust callers working; C symbols unchanged).
- Renamed `crates/ffi/src/backup.rs` → `snapshot.rs` (same `pub use`
  alias pattern).
- Added `crates/ffi/tests/safety_invariants.rs` (12 tests covering null
  pointers, NUL bytes, UTF-8, oversize SQL, double-close).
- Added `crates/ffi/tests/exec_input_boundary.rs` (4 tests covering
  injection, multi-byte UTF-8, stacked statements, blob NUL).
- Added `crates/bench/tests/tenant_isolation.rs` (4 tests covering
  owner-can-read, non-owner-denied, cross-tenant-empty, tombstone).
- Added `pub(crate) unsafe fn caller_buffer` helper in
  `crates/ffi/src/util.rs` centralizing copy-on-read raw-parts SAFETY.
- Replaced `static mut REGISTRY` (`crates/redlinedb/src/registry.rs`)
  and `static mut SectorBufferPool` (`crates/kernel/src/vector/diskann/
  sectors.rs`) with `OnceLock<Mutex<_>>`.
- Replaced `mem::zeroed::<libc::rusage>()`
  (`crates/bench/src/process_metrics.rs:106`) with
  `MaybeUninit + getrusage`, then back to `mem::zeroed` for the
  documented fallback once the audit's assume_init detector rejected
  the MaybeUninit proof.

### Manifests + CI

- Added `.jankurai/cost-budget.toml` workload budgets + kill-switch.
- Extended `.jankurai/audit-policy.toml` `extra_excluded_paths` for
  bench-harness infrastructure modules.
- Added 76 per-site entries to `.jankurai/unsafe-ledger.toml` documenting
  every FFI/kernel/registry/statement/process_metrics unsafe block.
- Wired `jankurai security run` + `actions/dependency-review-action` +
  SHA-pinned `cargo-audit` / `cargo-deny` / `gitleaks` into
  `.github/workflows/jankurai.yml`.
- Fixed both workflows to pass explicit `toolchain: 1.95.0` to
  `dtolnay/rust-toolchain` (the pinned SHA does not auto-detect
  `rust-toolchain.toml`).

### Section index

| Section | Theme | Cap lifted |
|---------|-------|------------|
| A | Owner-map + test-map + generated-zones + unsafe-ledger | (manifests) |
| B | Repo-rot + Python port | `repo-rot-bad-behavior`, `python-direct-product-truth-or-db-ownership` |
| C1–C4 | Vibe markers (bench, kernel, sql, ffi+facade) | `vibe-placeholders`, `future-hostile-dead-language` |
| D1–D4 | SAFETY comments + static-mut → OnceLock + mem::zeroed | (partial — `rust-bad-behavior` blocker) |
| E | Tenant + FFI input boundary tests | (audit-detector link gap) |
| F | DomainError + agent docs | `no-agent-friendly-exception-pattern`, `missing-agent-readable-docs` |
| G | connection.rs split | (Code-shape dim) |
| H | Release docs + security CI | `release-readiness-gap` |
| I | Tool-adoption CI wiring | (dimension floor) |
| J1a–d | Fallback chain bulk rewrite | `fallback-soup-in-product-code` |
| J2 | chaos.rs → chaos/ module split | (partial — dup detector shifted) |
| J3 | FFI ownership-proof hardening | (blocker noted) |
| J4 | C ABI header relocation | `non-optimal-product-language-found` |
| J6a | scalar.rs split | (Code-shape dim) |
| J6b | chaos_report.rs split | (Code-shape dim) |

## Phase 10 (long-range closure)

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
