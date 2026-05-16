<p align="center">
  <img src="assets/redlinedb-banner.png" alt="RedlineDB" width="100%">
</p>

<h1 align="center">RedlineDB</h1>

<p align="center">
  <em>A Rust-native, concurrent-write embedded SQL engine that stays SQLite-compatible without inheriting its concurrency cliff.</em>
</p>

<p align="center">
  <a href="agent/repo-score.md"><img src="https://img.shields.io/badge/jankurai-85%2F100%20pass-brightgreen" alt="jankurai score"></a>
  <a href="#status"><img src="https://img.shields.io/badge/tests-928%20passing-brightgreen" alt="tests"></a>
  <a href="#bench-headlines"><img src="https://img.shields.io/badge/xbabe1%20cert-1728%2F1728%20%E2%9C%93-brightgreen" alt="cert"></a>
  <a href="#crash-and-failpoint-certification"><img src="https://img.shields.io/badge/recovery-36%2F36%20%E2%9C%93-brightgreen" alt="recovery"></a>
  <a href="#crash-and-failpoint-certification"><img src="https://img.shields.io/badge/failpoint-24%2F24%20%E2%9C%93-brightgreen" alt="failpoint"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-Apache--2.0-blue" alt="license"></a>
  <a href="rust-toolchain.toml"><img src="https://img.shields.io/badge/rust-1.95-orange" alt="rust"></a>
  <img src="https://img.shields.io/badge/version-1.0.0-blue" alt="version">
</p>

---

## Install

### Rust library

Add to `Cargo.toml`. Use an exact pin for production:

```toml
[dependencies]
redlinedb = "=1.0.0"   # exact pin — recommended for production
# redlinedb = "1"      # any compatible 1.x (fine for libraries)
```

`Cargo.lock` locks the resolved version for binary crates. Run `cargo update -p redlinedb` to upgrade on your schedule.

### CLI binary — Linux & macOS

Latest release:

```bash
curl -LsSf https://raw.githubusercontent.com/neverhuman/RedlineDB/main/scripts/install.sh | bash
```

**Pin to a specific version** (recommended for CI and reproducible environments):

```bash
VERSION=v1.0.0 curl -LsSf https://raw.githubusercontent.com/neverhuman/RedlineDB/main/scripts/install.sh | bash
```

Custom install prefix:

```bash
VERSION=v1.0.0 PREFIX=~/.local curl -LsSf https://raw.githubusercontent.com/neverhuman/RedlineDB/main/scripts/install.sh | bash
```

The script verifies a SHA-256 checksum from the release before installing.

### cargo install (from source, version-pinned)

```bash
cargo install redlinedb-cli --version 1.0.0 --locked
# or from a specific git tag:
cargo install --git https://github.com/neverhuman/RedlineDB.git --tag v1.0.0 --package redlinedb-cli --locked
```

`--locked` enforces the committed `Cargo.lock` — ensures you get the exact dependency tree that was tested.

### Direct download

Pre-built tarballs on the [releases page](https://github.com/neverhuman/RedlineDB/releases):

| Platform | File |
|---|---|
| Linux x86_64 | `redlinedb-v1.0.0-linux-x86_64.tar.gz` |
| macOS Apple Silicon | `redlinedb-v1.0.0-macos-arm64.tar.gz` |
| macOS Intel | `redlinedb-v1.0.0-macos-x86_64.tar.gz` |

Each tarball has a matching `.sha256` checksum file.

---

## Why RedlineDB

SQLite is the most-deployed database engine on Earth — billions of installs, decades of stability, a public-domain test corpus that runs to roughly 92.6 million lines. It is also a single-writer database. Its WAL design is brilliant for the embedded device that birthed it, and a hard ceiling for the embedded application that has since become a 64-thread service.

RedlineDB is a ground-up rewrite in safe Rust that keeps the SQLite API contract — the C ABI, the SQL surface, the embedded model — and replaces the storage core with **MVCC, a concurrent B-tree, real group-commit WAL, and a deterministic crash-recovery story.** It is tested end-to-end by a 1,728-child certification matrix on a 128-core host and a 24-case failpoint matrix that proves zero lost acked commits across every crash injection point we hook.

| | RedlineDB | SQLite |
|---|---|---|
| Active source LOC | **34,999 (100% Rust)** | ~250,000 (C) |
| Concurrency model | MVCC, multi-writer | Single-writer WAL |
| Test count | 243 passing | (separate test corpus) |
| Ecosystem compat | Rust-native API + `sqlite3_*` symbol shim (Rust FFI crate) | native C API |
| Memory safety | Safe Rust (unsafe limited to FFI boundary in `crates/ffi`) | Unsafe by language |

---

## Bench headlines

The headline numbers below are from the **`phase10-xbabe1-certified`** run (median of five reps per cell on **xbabe1: 128 vCPU, Linux 6.8, ext4, Docker 29.2.1, Rust 1.95.0, SQLite via rusqlite bundled**, strict durability, ~1700-child cert matrix, manifest hashed and reproducible).

| Workload | Threads | RedlineDB qps | SQLite qps | **Phase-10 Ratio** | Phase-9 |
|---|---:|---:|---:|---:|---:|
| writers-disjoint | 64 | **1,256** | 79 | **15.89×** | 8.32× |
| mixed-95/5 OLTP | 64 | **24,763** | 1,680 | **14.74×** | 7.92× |
| mixed-80/20 OLTP | 64 | **6,154** | 405 | **15.21×** | 8.01× |
| mixed-50/50 OLTP | 64 | **2,483** | 160 | **15.55×** | 7.90× |
| point-read-pk | 64 | 121,268 | 122,221 | 0.99× (parity) | 0.99× |
| point-read-pk | 32 | 32,611 | 23,371 | 1.40× | — |
| hot-row-update | 64 | 35 | 79 | 0.44× | 0.21× |
| secondary-index-range | 64 | 5,598 | 117,088 | 0.048× | 0.012× |
| secondary-index-read | 64 | 16,245 | 121,030 | 0.13× | — |

**Reading the table.** Phase 10's MVCC index format and the SQL-side index-undo removal **roughly doubled** the contended-write headlines vs phase-9: writers-disjoint goes 8.32× → **15.89×**, and the three mixed-OLTP cells move from 7.9–8.0× into the 14.7–15.6× band. The two contention-bound losses also improve (hot-row-update 0.21× → 0.44×, range scan 0.012× → 0.048×) but still trail SQLite — those are honest engineering items, not headline material. Point-read-pk at 64 threads holds parity (0.99×) where the SQLite reader path is already near-optimal.

### Throughput vs threads

<p align="center">
  <img src="assets/fig1_throughput_scaling.png" alt="Throughput vs threads" width="80%">
</p>

Five workloads across the full thread sweep, log-y. RedlineDB (solid) keeps climbing where SQLite (dashed) plateaus.

### Tail latency (p99) vs threads

<p align="center">
  <img src="assets/fig2_latency_p99.png" alt="p99 latency vs threads" width="80%">
</p>

p99 latency under load. RedlineDB's MVCC commit path keeps the 99th-percentile tail bounded as concurrency grows on read-mostly mixes; on writers-disjoint the 64-thread sweet spot is visible.

### RedlineDB / SQLite ratio per (workload, thread)

<p align="center">
  <img src="assets/fig3_ratio_bars.png" alt="RedlineDB / SQLite ratio bars" width="80%">
</p>

Each bar is `redline_qps / sqlite_qps` at one (workload, thread) cell. Bars above the gray 1.0 line are RedlineDB wins; bars below are SQLite wins. We do not airbrush the latter.

### Scaling efficiency

<p align="center">
  <img src="assets/fig4_scaling_efficiency.png" alt="Scaling efficiency" width="80%">
</p>

`qps(N) / qps(1)` per workload, with the dashed line indicating ideal linear scaling. Mixed and disjoint-writer workloads track near-linear out to 64 threads; the 128-thread regression is the cross-core scaling cliff we discuss in the paper.

### Crash and failpoint certification

<p align="center">
  <img src="assets/fig5_recovery_failpoint.png" alt="Recovery, failpoint, compat pass counts" width="60%">
</p>

- **36 / 36** recovery-matrix cases pass (3 scenarios × 2 durabilities × 6 kill windows).
- **24 / 24** failpoint-matrix cases pass with **zero lost acked commits** — a synced-ack child oracle proves recovery republishes every commit the workload acknowledged before crash.
- **40 / 40** SQL compatibility cases pass against a sqllogictest-style suite.

The cert harness is reproducible: one CLI invocation rebuilds the Docker image, runs every cell, fetches artifacts, and hashes them into a manifest carrying the git SHA, host fingerprint, image digest, and SQLite PRAGMAs.

### See also

- The full evaluation, methodology, and architecture writeup is in [paper/main.pdf](paper/main.pdf) — a 10-page IEEE conference paper.
- Per-cell numbers live in `target/bench/xbabe1/certification/{summary.csv,runs.jsonl}`.
- Reproducibility scripts: `paper/scripts/build_figs.py`, `scripts/bench/xbabe1_run.sh`.

---

## Architecture

<p align="center">
  <img src="assets/architecture.png" alt="RedlineDB architecture" width="95%">
</p>

RedlineDB is 100% Rust, top to bottom. The primary interface is the Rust facade (`redlinedb`) that owns the public types — `Database`, `Connection`, `Statement`, `Row`, `OpenOptions`. For ecosystem compatibility, `crates/ffi` is a Rust crate that exports `extern "C"` symbols (`rldb_*` and `sqlite3_*` shims) so existing SQLite-linked programs can swap in RedlineDB at link time — no C source code is involved. The SQL engine (`redlinedb-sql`) wraps the kernel via a parser-planner-executor pipeline. The kernel (`redlinedb-kernel`) holds the catalog, the B-tree index, the MVCC engine, the WAL coordinator, and the slotted-page storage layer. The entire codebase is safe Rust modulo the necessary `unsafe` at the FFI boundary in `crates/ffi` and a single audited thread-local in the kernel for failpoint thread-arming.

### Crate layout

| Crate | LOC (active) | What it owns |
|---|---:|---|
| [`crates/kernel`](crates/kernel) | 12,883 | Slotted-page heap, MVCC version chains, WAL coordinator, B-tree index, catalog snapshot, recovery |
| [`crates/sql`](crates/sql) | 11,615 | sqlparser-rs SQLite-dialect parser, cost-based planner, vectorized executor, per-tx index undo log |
| [`crates/redlinedb`](crates/redlinedb) | 2,975 | Public Rust facade — Database, Connection, Statement, Row, OpenOptions, BeginMode |
| [`crates/ffi`](crates/ffi) | 1,478 | Rust FFI crate: exports `rldb_*` and `sqlite3_*` C-callable symbols for ecosystem compatibility |
| [`crates/bench`](crates/bench) | 5,144 | Workload harness, parallel certify scheduler, recovery-matrix, failpoint-matrix, compat suite |
| [`crates/cli`](crates/cli) | — | One-shot shell for queries, stats, backups |
| [`crates/server`](crates/server) | — | Optional framed local server |

### Key engine properties

- **MVCC + concurrent B-tree.** Tuples carry version chains tagged by CSN. The index uses physical-key navigation `(logical_key, row_id)` so duplicate-key runs split correctly, range scans terminate as soon as the next leaf's first key is past the upper bound, and concurrent writers on disjoint rows do not serialize.
- **Group-commit WAL.** A `WalCoordinator` thread batches commits and surfaces real fdatasync/pwrite counters into the bench manifest. The WAL payload includes a `CatalogSnapshot` variant so DDL is durable through crash without a separate fsync dance.
- **Per-transaction index undo log.** SQL DML drives both the heap and every catalog index. On rollback **or commit failure**, the inverse log replays so the heap and indexes never diverge — even if the kernel `commit` returns an error after physical pages have been mutated.
- **Planner-executor consistency invariant.** The planner only emits an `IndexPointLookup` or `IndexRangeScan` when the index has a live handle and a populated `meta_page_id`; a debug assertion (`access_path_is_consumable_by_executor`) prevents the planner from ever proposing an access path the executor cannot honor.
- **Bin-packing parallel benchmark scheduler.** Reserves four cores for OS plus parent overhead, dispatches as many children as fit the remaining core budget. The full cert matrix on xbabe1 finishes in ~58 wall-clock minutes vs the multi-day estimate the serial harness implied.

### Insert path

<p align="center">
  <img src="assets/dataflow.png" alt="INSERT data flow" width="95%">
</p>

A single INSERT flows: `Connection.execute → Parser → Planner → Executor.execute_insert` which both writes a heap tuple and calls `BtreeIndex.insert_tx`, recording an `IndexUndoOp` for rollback. The transaction commits via `Engine.commit → WalCoordinator.append + fsync → publish CSN` — failpoints intercept every step for crash certification.

---

## Quick start

### Prerequisites

| Requirement | Version | Install |
|---|---|---|
| Rust toolchain | **1.95** | `rustup toolchain install 1.95` (or see `rust-toolchain.toml`) |
| cargo-nextest | 0.9.133 | `curl -LsSf https://get.nexte.st/0.9.133/linux \| tar zxf - -C ~/.cargo/bin` |
| just (task runner) | any | `cargo install just` |
| OS | Linux / macOS | Windows untested; `--with-strace` bench flag is Linux-only |

### Build and test

```bash
# Clone
git clone https://github.com/neverhuman/RedlineDB.git && cd RedlineDB

# Full test suite (928 passing, 22 ignored for known engine gaps)
cargo nextest run --workspace --locked

# The CI gate — fmt-check + workspace-check + nextest
just fast

# Audit score (85 / 85)
just score

# Supply-chain scan (cargo-audit + gitleaks)
just security
```

### Embedded use (Rust)

Add to `Cargo.toml`:

```toml
[dependencies]
redlinedb = { path = "crates/redlinedb" }
```

```rust
use redlinedb::{Database, DbOptions, SqlValue, Step};
use std::sync::Arc;

let db = Database::create("/tmp/demo.redline", DbOptions::default())?;
let conn: Arc<_> = db.connect();

conn.execute("CREATE TABLE kv(k INTEGER PRIMARY KEY, v TEXT)")?;
conn.execute("INSERT INTO kv VALUES (1, 'hello')")?;

let mut stmt = conn.prepare("SELECT v FROM kv WHERE k = 1")?;
while let Step::Row = stmt.step()? {
    println!("{:?}", stmt.column_value(0)?);
}
```

#### Thread model

| Type | Send | Sync | Typical use |
|---|---|---|---|
| `Database` | ✓ | ✓ | Share across threads; hand out fresh `Connection`s |
| `Connection` | ✓ | ✗ | One per thread; move between threads if needed |
| `Statement` | ✗ | ✗ | Bound to one connection borrow; do not pool |

`Database::create_in_memory()` and `Database::create_ephemeral()` create transient shared sessions that clean up when the last handle drops.

### Ecosystem compatibility (C ABI shim)

RedlineDB is 100% Rust, but the `crates/ffi` crate exports C-callable symbols so existing SQLite-linked ecosystems (rusqlite, sqlx, Python `sqlite3`, Go `mattn/go-sqlite3`) can swap in RedlineDB at link time without source changes. No C code is involved — `crates/ffi` is a Rust crate that uses `extern "C"` + `#[no_mangle]` to produce a compatible shared library.

```c
// Existing C/C++ code links against libredlinedb instead of libsqlite3.
// The sqlite3.h shim (7 lines, just a redirect) maps sqlite3_* → rldb_*.
#include "crates/ffi/include/rldb.h"

rldb *db;
rldb_open("demo.redline", &db);

rldb_stmt *stmt;
rldb_prepare_v2(db, "SELECT k FROM kv", -1, &stmt, NULL);
while (rldb_step(stmt) == RLDB_ROW) {
    int64_t k = rldb_column_int64(stmt, 0);
}
rldb_finalize(stmt);
rldb_close(db);
```

Full `sqlite3_*` symbol coverage is a planned FFI lane. See [Limitations and roadmap](#limitations-and-roadmap).

### CLI

```bash
# One-shot query
cargo run -p redlinedb-cli --release -- exec /tmp/demo.redline "SELECT count(*) FROM kv"

# Storage stats
cargo run -p redlinedb-cli --release -- stats /tmp/demo.redline --json

# Physical backup
cargo run -p redlinedb-cli --release -- backup /tmp/demo.redline /tmp/demo.bak --physical
```

---

## SQLite parity test coverage

RedlineDB ships **117 dedicated SQLite-parity tests** across five test files, plus a live differential harness (`differential_lab.rs`) that runs each query against a real `rusqlite` connection and asserts row-for-row identical results. Tests marked ⏸ are written but skipped pending engine work; the reason is recorded in the `#[ignore]` attribute on the test.

> **Summary:** 97 passing · 21 skipped (known parser/engine gaps below) · 4 differential

### Aggregate functions — `parity_agg_funcs.rs` (17 tests, all passing)

| Test | What it proves |
|---|---|
| `group_concat_basic_default_separator` | `GROUP_CONCAT(v)` default `,` separator |
| `group_concat_custom_separator` | `GROUP_CONCAT(v, ' \| ')` custom separator |
| `group_concat_skips_nulls` | NULLs omitted from concatenation |
| `group_concat_all_null_returns_null` | All-NULL group → NULL result |
| `group_concat_empty_table_returns_null` | Empty table → NULL |
| `group_concat_with_group_by` | Per-group concatenation with `GROUP BY` |
| `string_agg_alias_works` | `string_agg` is a functional alias of `group_concat` |
| `total_basic_sum` | `total()` sums real values |
| `total_all_null_returns_zero_real` | `total()` returns `0.0` for all-NULL (vs `sum()` → NULL) |
| `total_empty_table_returns_zero_real` | `total()` on empty table → `0.0` |
| `total_vs_sum_null_difference` | `total(NULL) = 0.0`, `sum(NULL) = NULL` |
| `total_skips_null_values` | NULL rows skipped in `total()` |
| `json_group_array_basic` | `json_group_array(v)` collects integers |
| `json_group_array_includes_nulls` | NULLs included in JSON array |
| `json_group_array_empty_table` | Empty table → `[]` |
| `json_group_object_basic` | `json_group_object(k, v)` builds JSON object |
| `json_group_object_skips_null_keys` | NULL keys omitted from object |

### Positive parity (constructs) — `parity_coverage.rs` (28 tests, 22 ✅ 6 ⏸)

| Test | Status | What it proves |
|---|---|---|
| `alter_table_rename_to` | ✅ | `ALTER TABLE … RENAME TO` |
| `alter_table_rename_column` | ✅ | `ALTER TABLE … RENAME COLUMN` |
| `create_and_drop_index` | ✅ | `CREATE INDEX` / `DROP INDEX` round-trip |
| `drop_index_if_exists` | ✅ | `DROP INDEX IF EXISTS` on nonexistent index |
| `returning_with_arithmetic_expression` | ✅ | `INSERT … RETURNING a + b` |
| `returning_with_function_call` | ✅ | `INSERT … RETURNING upper(name)` |
| `update_returning_with_expression` | ✅ | `UPDATE … RETURNING a * b` |
| `exists_subquery_true` | ✅ | `WHERE EXISTS (SELECT …)` — populated table |
| `exists_subquery_false` | ✅ | `WHERE EXISTS (SELECT …)` — empty table |
| `not_exists_subquery_true` | ✅ | `WHERE NOT EXISTS (SELECT …)` |
| `null_in_empty_list` | ✅ | `NULL IN (1,2,3)` → NULL |
| `value_in_list_with_null` | ✅ | `1 IN (1, NULL)` → 1 |
| `value_not_in_list_with_null` | ✅ | `2 NOT IN (1, NULL)` → NULL |
| `null_comparison_is_null` | ✅ | `NULL = NULL` → NULL |
| `null_is_null_is_true` | ✅ | `NULL IS NULL` → 1 |
| `value_is_not_null` | ✅ | `1 IS NOT NULL` → 1 |
| `pragma_integrity_check_ok` | ✅ | `PRAGMA integrity_check` returns `"ok"` |
| `nested_savepoint_basic` | ✅ | `SAVEPOINT` / `ROLLBACK TO` within `BEGIN` |
| `nested_savepoint_release` | ✅ | `SAVEPOINT` / `RELEASE` / `COMMIT` |
| `i64_max_stores_and_retrieves` | ✅ | `i64::MAX` round-trips through INTEGER column |
| `i64_min_stores_and_retrieves` | ✅ | `i64::MIN` round-trips through INTEGER column |
| `inner_join_chain` | ✅ | Three-table `JOIN … JOIN` chain |
| `pragma_auto_vacuum` | ⏸ | `PRAGMA auto_vacuum` — not yet parsed |
| `pragma_quick_check` | ⏸ | `PRAGMA quick_check` — not yet parsed |
| `pragma_wal_checkpoint_passive` | ⏸ | `PRAGMA wal_checkpoint(PASSIVE)` — mode arg not yet parsed |
| `pragma_wal_checkpoint_full` | ⏸ | `PRAGMA wal_checkpoint(FULL)` — mode arg not yet parsed |
| `pragma_wal_checkpoint_restart` | ⏸ | `PRAGMA wal_checkpoint(RESTART)` — mode arg not yet parsed |
| `pragma_wal_checkpoint_truncate` | ⏸ | `PRAGMA wal_checkpoint(TRUNCATE)` — mode arg not yet parsed |

### Negative parity (error boundaries) — `parity_negative.rs` (24 tests, 23 ✅ 1 ⏸)

Assert that unsupported SQL constructs return an error rather than silently producing wrong results.

| Test | Status | Construct rejected |
|---|---|---|
| `update_from_is_unsupported` | ✅ | `UPDATE … FROM` |
| `update_or_conflict_is_unsupported` | ✅ | `UPDATE OR IGNORE/REPLACE` |
| `delete_using_is_unsupported` | ✅ | `DELETE … USING` |
| `delete_limit_is_unsupported` | ✅ | `DELETE … LIMIT` |
| `delete_order_by_is_unsupported` | ✅ | `DELETE … ORDER BY` |
| `insert_set_syntax_is_unsupported` | ✅ | `INSERT … SET col=val` (MySQL syntax) |
| `insert_on_duplicate_key_update_is_unsupported` | ✅ | `INSERT … ON DUPLICATE KEY UPDATE` |
| `create_table_as_select_is_unsupported` | ✅ | `CREATE TABLE … AS SELECT` |
| `alter_table_only_is_unsupported` | ✅ | `ALTER TABLE ONLY` |
| `alter_table_add_column_after_is_unsupported` | ✅ | `ADD COLUMN … AFTER col` |
| `alter_table_drop_multiple_columns_is_unsupported` | ✅ | `DROP COLUMN` multiple in one statement |
| `create_index_with_include_is_unsupported` | ✅ | `CREATE INDEX … INCLUDE (col)` |
| `distinct_on_is_unsupported` | ✅ | `SELECT DISTINCT ON (…)` |
| `natural_join_is_unsupported` | ✅ | `NATURAL JOIN` |
| `group_by_all_is_unsupported` | ✅ | `GROUP BY ALL` |
| `like_any_is_unsupported` | ✅ | `LIKE ANY (…)` |
| `case_in_aggregate_is_unsupported` | ✅ | `CASE` expression inside aggregate |
| `vector_non_f32_type_is_unsupported` | ✅ | `VECTOR(64, float64)` — only f32 vectors supported |
| `cte_returns_not_implemented_error` | ✅ | `WITH … AS (…) SELECT` CTE |
| `create_view_returns_not_implemented_error` | ✅ | `CREATE VIEW` |
| `window_function_returns_not_implemented_error` | ✅ | `ROW_NUMBER() OVER (…)` window function |
| `partial_index_returns_error` | ✅ | `CREATE INDEX … WHERE` partial index |
| `unsupported_function_returns_error` | ✅ | Unknown function name |
| `in_subquery_multi_column_is_unsupported` | ⏸ | `(a,b) IN (SELECT a,b …)` — needs data-driven repro |

### Scalar functions — `parity_scalar_funcs.rs` (44 tests, 33 ✅ 11 ⏸)

| Test | Status | What it proves |
|---|---|---|
| `substr_basic_1based` | ⏸ | `substr(s, 2)` — sqlparser emits ANSI Substring AST |
| `substr_with_length` | ⏸ | `substr(s, 2, 3)` |
| `substr_negative_start` | ⏸ | `substr(s, -3)` negative-offset semantics |
| `substr_zero_start_acts_as_one` | ⏸ | `substr(s, 0, 3)` — zero treated as offset 0 |
| `substr_null_propagates` | ⏸ | `substr(NULL, 1)` / `substr(s, NULL)` → NULL |
| `substr_alias_substring` | ⏸ | `substring(s, 2, 3)` — alias |
| `substr_beyond_length_returns_empty` | ⏸ | `substr(s, 100)` → `""` |
| `instr_found` | ✅ | `instr(s, needle)` → 1-based position |
| `instr_not_found` | ✅ | `instr(s, 'xyz')` → 0 |
| `instr_null_propagates` | ✅ | `instr(NULL, …)` → NULL |
| `instr_empty_needle_returns_one` | ✅ | `instr(s, '')` → 1 |
| `trim_whitespace` | ⏸ | `trim(s)` — sqlparser emits ANSI Trim AST |
| `trim_custom_chars` | ⏸ | `trim(s, '*')` |
| `ltrim_whitespace` | ✅ | `ltrim(s)` strips leading whitespace |
| `rtrim_whitespace` | ✅ | `rtrim(s)` strips trailing whitespace |
| `trim_null_propagates` | ⏸ | `trim(NULL)` → NULL |
| `replace_basic` | ✅ | `replace(s, old, new)` |
| `replace_all_occurrences` | ✅ | All occurrences replaced in one call |
| `replace_null_propagates` | ✅ | Any NULL argument → NULL |
| `printf_string_placeholder` | ✅ | `printf('%s', …)` |
| `printf_integer_placeholder` | ✅ | `printf('%d', …)` |
| `printf_hex_placeholder` | ✅ | `printf('%x', 255)` → `"ff"` |
| `printf_percent_escape` | ✅ | `printf('100%%')` → `"100%"` |
| `format_is_alias_for_printf` | ✅ | `format(…)` == `printf(…)` |
| `printf_null_format_returns_null` | ✅ | `printf(NULL)` → NULL |
| `iif_true_branch` | ✅ | `iif(1, 'yes', 'no')` → `"yes"` |
| `iif_false_branch` | ✅ | `iif(0, 'yes', 'no')` → `"no"` |
| `iif_null_condition_returns_false_branch` | ✅ | `iif(NULL, …)` → false branch |
| `sign_positive` | ✅ | `sign(5)` → 1 |
| `sign_negative` | ✅ | `sign(-3)` → -1 |
| `sign_zero` | ✅ | `sign(0)` → 0 |
| `sign_null` | ✅ | `sign(NULL)` → NULL |
| `char_basic_ascii` | ✅ | `char(72, 105)` → `"Hi"` |
| `char_single` | ✅ | `char(65)` → `"A"` |
| `unicode_basic` | ✅ | `unicode('A')` → 65 |
| `unicode_multi_char_returns_first` | ✅ | `unicode('AB')` → codepoint of first char |
| `unicode_null_propagates` | ✅ | `unicode(NULL)` → NULL |
| `zeroblob_correct_length` | ✅ | `zeroblob(8)` → 8-byte zero blob |
| `zeroblob_zero_length` | ✅ | `zeroblob(0)` → empty blob |
| `zeroblob_null_propagates` | ✅ | `zeroblob(NULL)` → NULL |
| `randomblob_correct_length` | ✅ | `randomblob(16)` → 16-byte blob |
| `randomblob_produces_blob_of_right_size` | ✅ | length matches argument |
| `scalar_funcs_in_select_after_insert` | ⏸ | `trim()` after INSERT — ANSI Trim AST |
| `replace_in_where_clause` | ✅ | `replace()` in `WHERE` predicate |

### Differential harness — `differential_lab.rs` (4 tests, all passing)

Runs each query against both RedlineDB and a live `rusqlite` (bundled SQLite 3.x) connection and asserts row-for-row, type-for-type identical results. Queries using constructs not yet parsed (substr, trim) are skipped inline with explanatory comments.

| Test | Coverage |
|---|---|
| `diff_scalar_string_matrix` | `instr`, `replace`, `printf`, `upper`, `lower`, `length` on TEXT with NULLs |
| `diff_scalar_math_and_logic_matrix` | `iif`, `sign`, `coalesce`, `nullif` on INTEGER/REAL with NULLs |
| `diff_aggregate_matrix` | `count(*)`, `count(v)`, `sum`, `total`, `min/max` with `GROUP BY … ORDER BY` |
| `diff_join_and_subquery_matrix` | `INNER JOIN`, `IN (SELECT …)`, `NOT IN (SELECT …)` |

### Engine gap tracking

| Gap | Tests skipped | Path to fix |
|---|---|---|
| `substr()`/`substring()` — sqlparser emits ANSI `Substring` AST | 7 | Implement `Substring` eval in `crates/sql/src/exec/expr/scalar/` |
| `trim()` — sqlparser emits ANSI `Trim` AST | 4 | Implement `Trim` eval in `crates/sql/src/exec/expr/scalar/` |
| `PRAGMA wal_checkpoint(MODE)` — mode argument not parsed | 4 | Extend PRAGMA parser in `crates/sql/src/parser/` |
| `PRAGMA auto_vacuum` / `PRAGMA quick_check` | 2 | Extend PRAGMA parser |
| Multi-column `IN` subquery rejection | 1 | Data-driven repro test needed |

---

## Reproducing the benchmarks

The full xbabe1 certification at the headline thread/workload/durability sweep took **~58 minutes wall-clock on a 128-core host**. A small subset (smoke, ~1 min) reproduces locally:

```bash
# Local smoke: 4 workloads × 2 thread levels × 1 rep + 1 warmup
cargo run -p redlinedb-bench --release -- certify \
  --config crates/bench/bench/smoke.toml \
  --out-dir target/bench/certify-smoke \
  --seed 7 --repetitions 1 --warmup 1

# SQL compat (40 cases)
cargo run -p redlinedb-bench --release -- compat \
  --engine both --test-dir crates/bench/compat \
  --seed 7 --out target/bench/compat.json

# Recovery matrix (36 cases)
cargo run -p redlinedb-bench --release -- recover-matrix \
  --config crates/bench/bench/recovery-matrix.toml \
  --out target/bench/recovery.json --seed 7

# Failpoint matrix (24 cases, zero-lost-acked-commits gate)
cargo run -p redlinedb-bench --release -- failpoint-matrix \
  --config crates/bench/bench/failpoint-matrix.toml \
  --out target/bench/failpoint.json --seed 7

# Remote (xbabe1) full certification
./scripts/bench/xbabe1_sync.sh
./scripts/bench/xbabe1_run.sh cargo run -p redlinedb-bench --release -- certify \
  --config crates/bench/bench/certification.toml \
  --out-dir target/bench/xbabe1/certification \
  --seed 7 --repetitions 5 --warmup 1
./scripts/bench/xbabe1_fetch.sh certification

# Regenerate paper figures from the cert artifacts
python3 paper/scripts/build_figs.py
```

Every certification run emits a manifest with the git SHA, Docker image digest, host CPU/RAM/FS fingerprint, validated SQLite PRAGMAs, per-cell SHA-256 checksums, and `process_metrics_per_run` (RSS, fdatasync count, pwrite count, ctx switches). Two runs with the same git SHA + image digest + seed are byte-comparable.

---

## Status

RedlineDB is **Phase 9 fused and certified**. Tag history (most recent first):

| Tag | What landed |
|---|---|
| `paper-v1` | 10-page IEEE conference paper (this repo's `paper/main.pdf`) |
| `phase9-xbabe1-certified` | Full 1728-child cert on xbabe1, 0 failures |
| `wave7-fused` / `phase9-fusion-green-v3` | Reviewer pass-2: parallel scheduler, commit-failure rollback, real failpoint gating |
| `wave6-fused` / `phase9-fusion-green-v2` | Reviewer pass-1: B-tree split for duplicate keys, index undo log, ack-log fsync, BUSY/LOCKED/timeout split |
| `wave5-fused` | Failpoint hooks + matrix subcommand + child oracle |
| `wave4-fused` | SQL planner / executor index reads (IndexPointLookup, IndexRangeScan) |
| `wave3-fused` | SQL DML index maintenance with NULL parity + INSERT OR REPLACE/IGNORE |
| `wave2-fused` | BtreeIndex transactional lifecycle + Lsn sentinel cleanup |
| `wave1-fused` | Failpoint infrastructure, bench telemetry, Docker / proof-lane hygiene |
| `phase9-baseline` | Six-commit subsystem split of phase-8 WIP |

The tags are checkpoints, not promises — the only durable claim is the proof in `docs/WORKPLAN_slam.md` and the artifacts under `target/bench/`.

---

## Repository layout

```
RedlineDB/
├── crates/                  Rust workspace
│   ├── kernel/              storage, WAL, MVCC, B-tree, catalog
│   ├── sql/                 parser, planner, executor, exec/index_dml.rs
│   ├── redlinedb/           public Rust facade
│   ├── ffi/                 C ABI (rldb_*) + sqlite3.h compat header
│   ├── cli/                 one-shot shell
│   ├── server/              framed local server
│   └── bench/               workload harness + certify lane + matrices
├── agent/                   repository-level agent routing + proof metadata
├── docs/
│   ├── WORKPLAN_slam.md     proof ledger (commands + artifact hashes)
│   ├── WORKPLAN_CLAUDE.md   detailed phase plan
│   └── archive/             historical notes, reviewer tip files
├── paper/                   IEEE conference paper sources
│   ├── main.tex / main.pdf
│   ├── sections/*.tex
│   ├── figs/*.eps           camera-ready EPS (TikZ + matplotlib)
│   ├── data/*.csv           extracted bench numbers
│   └── refs/refs.bib        49-entry bibliography
├── scripts/
│   ├── bench/               xbabe1 sync + run + fetch + image build
│   └── check_file_sizes.sh  active-source LOC cap (2000 / file)
└── assets/                  README banners and rendered plots
```

---

## Limitations and roadmap

We try to publish RedlineDB's wins and its trailing edges with equal weight. As of `paper-v1`:

- **Single-row hot contention** (e.g., `hot-row-update`) is roughly 5× slower than SQLite. SQLite's WAL writer batches small commits in a way our group-commit path does not yet match. Improving this is a 2026 lane.
- **Large secondary-index range scans** are the biggest gap. The B-tree range cursor is correct but lacks prefetch and warm-leaf reuse. SQLite wins ~80× at 64 threads on `secondary-index-range`. Range-cursor prefetch is a planned kernel lane.
- **Single-thread, per-tx overhead** is higher than SQLite's. The MVCC version-chain bookkeeping and durable rowid B-tree pay off as concurrency rises but cost a constant tax at thread count = 1.
- **`sqlite3_*` ABI shim** covers the core symbol set but is not yet complete. The full symbol-level coverage (so rusqlite / Python `sqlite3` / Go drivers swap binary-compatibly without stubs) is the next FFI lane. The shim is a Rust crate (`crates/ffi`), not a separate C codebase.
- **No encryption-at-rest yet.** Pages and WAL are checksummed but not encrypted. Tracked as a Phase 10 deliverable.
- **Serializable isolation** is not yet supported; we run snapshot isolation. SSI (Cahill-style) is on the future-work list.

---

## Contributing

Bug reports and patches are welcome. The proof discipline in this repo is unusually strict for a hobby database:

- Every change runs `cargo fmt --check`, `./scripts/check_file_sizes.sh`, `cargo check --workspace --locked`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, and `cargo test --workspace --locked`.
- Active source files stay under 2,000 LOC (`agent/file-size-policy.toml`).
- New `unsafe` blocks outside `crates/ffi` go into `agent/unsafe-ledger.toml` with reviewer sign-off.
- Bench claims must come with a manifest (`target/bench/.../manifest.json`) carrying the git SHA, image digest, host fingerprint, and per-artifact SHA-256.
- The proof ledger in `docs/WORKPLAN_slam.md` is the source of truth for any performance number quoted.

See `AGENTS.md` for the agent routing protocol used during multi-stream development.

---

## Citing RedlineDB

If you use RedlineDB or its measurements in academic work, the canonical reference is the paper at [`paper/main.pdf`](paper/main.pdf):

```bibtex
@misc{redlinedb,
  title  = {{RedlineDB}: A {Rust}-Native, Concurrent-Write Embedded {SQL} Engine That Stays {SQLite}-Compatible Without Inheriting Its Concurrency Cliff},
  author = {{RedlineDB Authors}},
  year   = {2026},
  url    = {https://github.com/bentaylor/RedlineDB}
}
```

---

## License

RedlineDB is licensed under Apache-2.0. See [LICENSE](LICENSE).

The benchmark figures, recovery matrix, and failpoint matrix are reproducible from this repository at any tag from `phase9-baseline` forward; the `paper-v1` PDF embeds the `wave7-fused` certification numbers.
