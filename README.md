<p align="center">
  <img src="assets/redlinedb-banner.png" alt="RedlineDB" width="100%">
</p>

<h1 align="center">RedlineDB</h1>

<p align="center">
  <em>A Rust-native, concurrent-write embedded SQL engine that stays SQLite-compatible without inheriting its concurrency cliff.</em>
</p>

<p align="center">
  <a href=".jankurai/repo-score.md"><img src="https://img.shields.io/badge/jankurai-85%2F100%20pass-brightgreen" alt="jankurai score"></a>
  <a href="#status"><img src="https://img.shields.io/badge/tests-928%20passing-brightgreen" alt="tests"></a>
  <a href="#bench-headlines"><img src="https://img.shields.io/badge/xbabe1%20cert-1728%2F1728%20%E2%9C%93-brightgreen" alt="cert"></a>
  <a href="#crash-and-failpoint-certification"><img src="https://img.shields.io/badge/recovery-36%2F36%20%E2%9C%93-brightgreen" alt="recovery"></a>
  <a href="#crash-and-failpoint-certification"><img src="https://img.shields.io/badge/failpoint-24%2F24%20%E2%9C%93-brightgreen" alt="failpoint"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-Apache--2.0-blue" alt="license"></a>
  <a href="rust-toolchain.toml"><img src="https://img.shields.io/badge/rust-1.95-orange" alt="rust"></a>
  <img src="https://img.shields.io/badge/version-1.0.1-blue" alt="version">
</p>

---

## Install

### Rust library

Add to `Cargo.toml`. Use an exact pin for production:

```toml
[dependencies]
redlinedb = "=1.0.1"   # exact pin — recommended for production
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
curl -LsSf https://raw.githubusercontent.com/neverhuman/RedlineDB/main/scripts/install.sh | VERSION=v1.0.17 bash
```

Fully lock the download by pinning both the release tag and the tarball digest:

```bash
curl -LsSf https://raw.githubusercontent.com/neverhuman/RedlineDB/main/scripts/install.sh | \
  VERSION=v1.0.17 REDLINEDB_SHA256=<sha256> bash
```

Custom install prefix:

```bash
curl -LsSf https://raw.githubusercontent.com/neverhuman/RedlineDB/main/scripts/install.sh | \
  VERSION=v1.0.17 PREFIX=~/.local bash
```

The script requires SHA-256 verification before installing. By default it
downloads the matching `.sha256` release asset; `REDLINEDB_SHA256` lets CI
pin the exact digest inline.

### cargo install (from source, version-pinned)

```bash
cargo install redlinedb-cli --version 1.0.1 --locked
# or from a specific git tag:
cargo install --git https://github.com/neverhuman/RedlineDB.git --tag v1.0.17 --package redlinedb-cli --locked
```

`--locked` enforces the committed `Cargo.lock` — ensures you get the exact dependency tree that was tested.

### Direct download

Pre-built tarballs on the [releases page](https://github.com/neverhuman/RedlineDB/releases):

| Platform | File |
|---|---|
| Linux x86_64 | `redlinedb-v1.0.17-linux-x86_64.tar.gz` |
| macOS Apple Silicon | `redlinedb-v1.0.17-macos-arm64.tar.gz` |
| macOS Intel | `redlinedb-v1.0.17-macos-x86_64.tar.gz` |

Each tarball has a matching `.sha256` checksum file and contains:
`bin/redlinedb`, `lib/libredlinedb.*`, `include/redlinedb.h`, and
`include/sqlite3.h`.

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

RedlineDB is 100% Rust, top to bottom. The primary interface is the Rust facade (`redlinedb`) that owns the public types — `Database`, `Connection`, `Statement`, `Row`, `OpenOptions`. For compatibility testing and incremental integration, `crates/ffi` exports `extern "C"` symbols (`rldb_*` plus the documented `sqlite3_*` aliases) for the covered ABI surface; no C source code is involved. The SQL engine (`redlinedb-sql`) wraps the kernel via a parser-planner-executor pipeline. The kernel (`redlinedb-kernel`) holds the catalog, the B-tree index, the MVCC engine, the WAL coordinator, and the slotted-page storage layer. The entire codebase is safe Rust modulo the necessary `unsafe` at the FFI boundary in `crates/ffi` and a single audited thread-local in the kernel for failpoint thread-arming.

### Crate layout

| Crate | LOC (active) | What it owns |
|---|---:|---|
| [`crates/kernel`](crates/kernel) | 12,883 | Slotted-page heap, MVCC version chains, WAL coordinator, B-tree index, catalog snapshot, recovery |
| [`crates/sql`](crates/sql) | 11,615 | sqlparser-rs SQLite-dialect parser, cost-based planner, vectorized executor, per-tx index undo log |
| [`crates/redlinedb`](crates/redlinedb) | 2,975 | Public Rust facade — Database, Connection, Statement, Row, OpenOptions, BeginMode |
| [`crates/ffi`](crates/ffi) | 1,478 | Rust FFI crate: exports `rldb_*` and documented `sqlite3_*` aliases for compatibility testing |
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

`Database::create_in_memory()` and `Database::create_ephemeral()` create transient shared sessions that clean up when the last handle drops. From async code, keep `Database` in shared state, open one `Connection` per blocking worker, and run synchronous SQL work inside `tokio::task::spawn_blocking` or an equivalent worker pool.

### Ecosystem compatibility (C ABI shim)

RedlineDB is 100% Rust, but the `crates/ffi` crate exports C-callable symbols so SQLite-linked ecosystems can exercise the documented compatibility surface where it matches their needs. No C code is involved — `crates/ffi` is a Rust crate that uses `extern "C"` + `#[no_mangle]` to produce a compatible shared library.

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

<!-- sqlite-parity-report:begin -->

**SQLite parity coverage:** **612 / 1127 = 54.3%** approved generated cases, with **515** remaining. Updated 2026-05-20.

![SQLite parity latency improvement plot](assets/sqlite-parity-latency-gap.svg)

<details>
<summary>Full ranked latency table</summary>

| Rank | Case | Priority | Profile | Category | SQLite median ns | RedlineDB median ns | Improvement |
| ---: | --- | --- | --- | --- | ---: | ---: | ---: |
| 1 | [00450 DML_WHERE_ORDER_LIMIT_063](crates/bench/sqlite_parity/cases/SQLITE_PARITY_450_DML_WHERE_ORDER_LIMIT_063.rs) | P1 | memory | GEN_SQL_DML | 2728567 | 2629705744 | <span style="color:#dc2626">-96276.81%</span> |
| 2 | [00802 WINDOW_PARTITION_SUM_015](crates/bench/sqlite_parity/cases/SQLITE_PARITY_802_WINDOW_PARTITION_SUM_015.rs) | P2 | memory | GEN_SQL_WINDOW | 2274468 | 706291194 | <span style="color:#dc2626">-30953.03%</span> |
| 3 | [00692 JOIN_SUBQUERY_EXISTS_085](crates/bench/sqlite_parity/cases/SQLITE_PARITY_692_JOIN_SUBQUERY_EXISTS_085.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3126060 | 864304486 | <span style="color:#dc2626">-27548.37%</span> |
| 4 | [00670 JOIN_SUBQUERY_EXISTS_063](crates/bench/sqlite_parity/cases/SQLITE_PARITY_670_JOIN_SUBQUERY_EXISTS_063.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3686381 | 1014186596 | <span style="color:#dc2626">-27411.71%</span> |
| 5 | [00985 VIEW_TRIGGER_GENERATED_038](crates/bench/sqlite_parity/cases/SQLITE_PARITY_985_VIEW_TRIGGER_GENERATED_038.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2903579 | 649847221 | <span style="color:#dc2626">-22280.90%</span> |
| 6 | [00963 VIEW_TRIGGER_GENERATED_016](crates/bench/sqlite_parity/cases/SQLITE_PARITY_963_VIEW_TRIGGER_GENERATED_016.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 3086635 | 556881442 | <span style="color:#dc2626">-17941.70%</span> |
| 7 | [00962 VIEW_TRIGGER_GENERATED_015](crates/bench/sqlite_parity/cases/SQLITE_PARITY_962_VIEW_TRIGGER_GENERATED_015.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 3016242 | 541766466 | <span style="color:#dc2626">-17861.64%</span> |
| 8 | [00986 VIEW_TRIGGER_GENERATED_039](crates/bench/sqlite_parity/cases/SQLITE_PARITY_986_VIEW_TRIGGER_GENERATED_039.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 3195713 | 567612170 | <span style="color:#dc2626">-17661.68%</span> |
| 9 | [00333 SCALAR_CAST_TYPEOF_027](crates/bench/sqlite_parity/cases/SQLITE_PARITY_333_SCALAR_CAST_TYPEOF_027.rs) | P1 | memory | GEN_SQL_SCALAR | 3259733 | 572752416 | <span style="color:#dc2626">-17470.53%</span> |
| 10 | [00697 JOIN_SUBQUERY_EXISTS_090](crates/bench/sqlite_parity/cases/SQLITE_PARITY_697_JOIN_SUBQUERY_EXISTS_090.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3505118 | 599125617 | <span style="color:#dc2626">-16992.88%</span> |
| 11 | [00987 VIEW_TRIGGER_GENERATED_040](crates/bench/sqlite_parity/cases/SQLITE_PARITY_987_VIEW_TRIGGER_GENERATED_040.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 3349644 | 565577888 | <span style="color:#dc2626">-16784.72%</span> |
| 12 | [00995 VIEW_TRIGGER_GENERATED_048](crates/bench/sqlite_parity/cases/SQLITE_PARITY_995_VIEW_TRIGGER_GENERATED_048.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 3611009 | 598907262 | <span style="color:#dc2626">-16485.59%</span> |
| 13 | [00913 CONSTRAINT_FK_SAVEPOINT_046](crates/bench/sqlite_parity/cases/SQLITE_PARITY_913_CONSTRAINT_FK_SAVEPOINT_046.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2691999 | 429684384 | <span style="color:#dc2626">-15861.54%</span> |
| 14 | [00994 VIEW_TRIGGER_GENERATED_047](crates/bench/sqlite_parity/cases/SQLITE_PARITY_994_VIEW_TRIGGER_GENERATED_047.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 3342890 | 486050616 | <span style="color:#dc2626">-14439.83%</span> |
| 15 | [00646 JOIN_SUBQUERY_EXISTS_039](crates/bench/sqlite_parity/cases/SQLITE_PARITY_646_JOIN_SUBQUERY_EXISTS_039.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2851020 | 387044035 | <span style="color:#dc2626">-13475.63%</span> |
| 16 | [00035 DROP_INDEX](crates/bench/sqlite_parity/cases/SQLITE_PARITY_035_DROP_INDEX.rs) | P0 | memory | SQL_DROP | 2621395 | 347225389 | <span style="color:#dc2626">-13145.82%</span> |
| 17 | [00645 JOIN_SUBQUERY_EXISTS_038](crates/bench/sqlite_parity/cases/SQLITE_PARITY_645_JOIN_SUBQUERY_EXISTS_038.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3812550 | 490890018 | <span style="color:#dc2626">-12775.63%</span> |
| 18 | [00332 SCALAR_ARITH_027](crates/bench/sqlite_parity/cases/SQLITE_PARITY_332_SCALAR_ARITH_027.rs) | P1 | memory | GEN_SQL_SCALAR | 3244374 | 406483475 | <span style="color:#dc2626">-12428.87%</span> |
| 19 | [00910 CONSTRAINT_FK_SAVEPOINT_043](crates/bench/sqlite_parity/cases/SQLITE_PARITY_910_CONSTRAINT_FK_SAVEPOINT_043.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 3262158 | 377299676 | <span style="color:#dc2626">-11465.95%</span> |
| 20 | [00996 VIEW_TRIGGER_GENERATED_049](crates/bench/sqlite_parity/cases/SQLITE_PARITY_996_VIEW_TRIGGER_GENERATED_049.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 3529955 | 393719594 | <span style="color:#dc2626">-11053.67%</span> |
| 21 | [00615 JOIN_SUBQUERY_EXISTS_008](crates/bench/sqlite_parity/cases/SQLITE_PARITY_615_JOIN_SUBQUERY_EXISTS_008.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3191985 | 340842795 | <span style="color:#dc2626">-10578.08%</span> |
| 22 | [00353 SCALAR_CAST_TYPEOF_032](crates/bench/sqlite_parity/cases/SQLITE_PARITY_353_SCALAR_CAST_TYPEOF_032.rs) | P1 | memory | GEN_SQL_SCALAR | 2524451 | 266044030 | <span style="color:#dc2626">-10438.69%</span> |
| 23 | [00449 DML_WHERE_ORDER_LIMIT_062](crates/bench/sqlite_parity/cases/SQLITE_PARITY_449_DML_WHERE_ORDER_LIMIT_062.rs) | P1 | memory | GEN_SQL_DML | 2750479 | 288780692 | <span style="color:#dc2626">-10399.29%</span> |
| 24 | [00032 UNIQUE_INDEX_FAILURE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_032_UNIQUE_INDEX_FAILURE.rs) | P0 | memory | SQL_INDEX_NEGATIVE | 2421205 | 239125788 | <span style="color:#dc2626">-9776.31%</span> |
| 25 | [00694 JOIN_SUBQUERY_EXISTS_087](crates/bench/sqlite_parity/cases/SQLITE_PARITY_694_JOIN_SUBQUERY_EXISTS_087.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2727094 | 261544268 | <span style="color:#dc2626">-9490.58%</span> |
| 26 | [00693 JOIN_SUBQUERY_EXISTS_086](crates/bench/sqlite_parity/cases/SQLITE_PARITY_693_JOIN_SUBQUERY_EXISTS_086.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3254994 | 311415985 | <span style="color:#dc2626">-9467.33%</span> |
| 27 | [00394 DML_WHERE_ORDER_LIMIT_007](crates/bench/sqlite_parity/cases/SQLITE_PARITY_394_DML_WHERE_ORDER_LIMIT_007.rs) | P1 | memory | GEN_SQL_DML | 2769525 | 263399634 | <span style="color:#dc2626">-9410.64%</span> |
| 28 | [00612 JOIN_SUBQUERY_EXISTS_005](crates/bench/sqlite_parity/cases/SQLITE_PARITY_612_JOIN_SUBQUERY_EXISTS_005.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2775296 | 259390460 | <span style="color:#dc2626">-9246.41%</span> |
| 29 | [00034 EXPRESSION_INDEX](crates/bench/sqlite_parity/cases/SQLITE_PARITY_034_EXPRESSION_INDEX.rs) | P0 | memory | SQL_INDEX | 3251958 | 298888332 | <span style="color:#dc2626">-9091.03%</span> |
| 30 | [00696 JOIN_SUBQUERY_EXISTS_089](crates/bench/sqlite_parity/cases/SQLITE_PARITY_696_JOIN_SUBQUERY_EXISTS_089.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3114678 | 284297158 | <span style="color:#dc2626">-9027.66%</span> |
| 31 | [00613 JOIN_SUBQUERY_EXISTS_006](crates/bench/sqlite_parity/cases/SQLITE_PARITY_613_JOIN_SUBQUERY_EXISTS_006.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3400369 | 308189165 | <span style="color:#dc2626">-8963.40%</span> |
| 32 | [00993 VIEW_TRIGGER_GENERATED_046](crates/bench/sqlite_parity/cases/SQLITE_PARITY_993_VIEW_TRIGGER_GENERATED_046.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 3229666 | 288166419 | <span style="color:#dc2626">-8822.48%</span> |
| 33 | [00611 JOIN_SUBQUERY_EXISTS_004](crates/bench/sqlite_parity/cases/SQLITE_PARITY_611_JOIN_SUBQUERY_EXISTS_004.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3405870 | 303087284 | <span style="color:#dc2626">-8798.97%</span> |
| 34 | [00695 JOIN_SUBQUERY_EXISTS_088](crates/bench/sqlite_parity/cases/SQLITE_PARITY_695_JOIN_SUBQUERY_EXISTS_088.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3174222 | 281906891 | <span style="color:#dc2626">-8781.13%</span> |
| 35 | [00033 PARTIAL_INDEX](crates/bench/sqlite_parity/cases/SQLITE_PARITY_033_PARTIAL_INDEX.rs) | P0 | memory | SQL_INDEX | 3313254 | 287130716 | <span style="color:#dc2626">-8566.12%</span> |
| 36 | [00614 JOIN_SUBQUERY_EXISTS_007](crates/bench/sqlite_parity/cases/SQLITE_PARITY_614_JOIN_SUBQUERY_EXISTS_007.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3140578 | 268386548 | <span style="color:#dc2626">-8445.77%</span> |
| 37 | [00395 DML_WHERE_ORDER_LIMIT_008](crates/bench/sqlite_parity/cases/SQLITE_PARITY_395_DML_WHERE_ORDER_LIMIT_008.rs) | P1 | memory | GEN_SQL_DML | 3269763 | 275726307 | <span style="color:#dc2626">-8332.61%</span> |
| 38 | [00673 JOIN_SUBQUERY_EXISTS_066](crates/bench/sqlite_parity/cases/SQLITE_PARITY_673_JOIN_SUBQUERY_EXISTS_066.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2731814 | 226393908 | <span style="color:#dc2626">-8187.31%</span> |
| 39 | [00672 JOIN_SUBQUERY_EXISTS_065](crates/bench/sqlite_parity/cases/SQLITE_PARITY_672_JOIN_SUBQUERY_EXISTS_065.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3446056 | 281057520 | <span style="color:#dc2626">-8055.92%</span> |
| 40 | [00467 DML_WHERE_ORDER_LIMIT_080](crates/bench/sqlite_parity/cases/SQLITE_PARITY_467_DML_WHERE_ORDER_LIMIT_080.rs) | P1 | memory | GEN_SQL_DML | 2959635 | 237189655 | <span style="color:#dc2626">-7914.15%</span> |
| 41 | [00619 JOIN_SUBQUERY_EXISTS_012](crates/bench/sqlite_parity/cases/SQLITE_PARITY_619_JOIN_SUBQUERY_EXISTS_012.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2468094 | 197493201 | <span style="color:#dc2626">-7901.85%</span> |
| 42 | [00451 DML_WHERE_ORDER_LIMIT_064](crates/bench/sqlite_parity/cases/SQLITE_PARITY_451_DML_WHERE_ORDER_LIMIT_064.rs) | P1 | memory | GEN_SQL_DML | 3267839 | 260863997 | <span style="color:#dc2626">-7882.77%</span> |
| 43 | [00997 VIEW_TRIGGER_GENERATED_050](crates/bench/sqlite_parity/cases/SQLITE_PARITY_997_VIEW_TRIGGER_GENERATED_050.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 3142271 | 250264118 | <span style="color:#dc2626">-7864.43%</span> |
| 44 | [00393 DML_WHERE_ORDER_LIMIT_006](crates/bench/sqlite_parity/cases/SQLITE_PARITY_393_DML_WHERE_ORDER_LIMIT_006.rs) | P1 | memory | GEN_SQL_DML | 3172478 | 249171528 | <span style="color:#dc2626">-7754.16%</span> |
| 45 | [00698 JOIN_SUBQUERY_EXISTS_091](crates/bench/sqlite_parity/cases/SQLITE_PARITY_698_JOIN_SUBQUERY_EXISTS_091.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3567276 | 277249201 | <span style="color:#dc2626">-7672.01%</span> |
| 46 | [00805 WINDOW_PARTITION_SUM_018](crates/bench/sqlite_parity/cases/SQLITE_PARITY_805_WINDOW_PARTITION_SUM_018.rs) | P2 | memory | GEN_SQL_WINDOW | 2467142 | 188874737 | <span style="color:#dc2626">-7555.61%</span> |
| 47 | [00392 DML_WHERE_ORDER_LIMIT_005](crates/bench/sqlite_parity/cases/SQLITE_PARITY_392_DML_WHERE_ORDER_LIMIT_005.rs) | P1 | memory | GEN_SQL_DML | 3796449 | 285580969 | <span style="color:#dc2626">-7422.32%</span> |
| 48 | [00671 JOIN_SUBQUERY_EXISTS_064](crates/bench/sqlite_parity/cases/SQLITE_PARITY_671_JOIN_SUBQUERY_EXISTS_064.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3510058 | 244561734 | <span style="color:#dc2626">-6867.46%</span> |
| 49 | [00964 VIEW_TRIGGER_GENERATED_017](crates/bench/sqlite_parity/cases/SQLITE_PARITY_964_VIEW_TRIGGER_GENERATED_017.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 3020771 | 203891150 | <span style="color:#dc2626">-6649.64%</span> |
| 50 | [00494 DML_WHERE_ORDER_LIMIT_107](crates/bench/sqlite_parity/cases/SQLITE_PARITY_494_DML_WHERE_ORDER_LIMIT_107.rs) | P1 | memory | GEN_SQL_DML | 1876894 | 126409833 | <span style="color:#dc2626">-6635.05%</span> |
| 51 | [00998 VIEW_TRIGGER_GENERATED_051](crates/bench/sqlite_parity/cases/SQLITE_PARITY_998_VIEW_TRIGGER_GENERATED_051.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 3186235 | 214423631 | <span style="color:#dc2626">-6629.69%</span> |
| 52 | [00806 WINDOW_PARTITION_SUM_019](crates/bench/sqlite_parity/cases/SQLITE_PARITY_806_WINDOW_PARTITION_SUM_019.rs) | P2 | memory | GEN_SQL_WINDOW | 2955167 | 193329462 | <span style="color:#dc2626">-6442.08%</span> |
| 53 | [00325 SCALAR_CAST_TYPEOF_025](crates/bench/sqlite_parity/cases/SQLITE_PARITY_325_SCALAR_CAST_TYPEOF_025.rs) | P1 | memory | GEN_SQL_SCALAR | 2560690 | 162592238 | <span style="color:#dc2626">-6249.55%</span> |
| 54 | [00955 VIEW_TRIGGER_GENERATED_008](crates/bench/sqlite_parity/cases/SQLITE_PARITY_955_VIEW_TRIGGER_GENERATED_008.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2339951 | 145719851 | <span style="color:#dc2626">-6127.47%</span> |
| 55 | [00452 DML_WHERE_ORDER_LIMIT_065](crates/bench/sqlite_parity/cases/SQLITE_PARITY_452_DML_WHERE_ORDER_LIMIT_065.rs) | P1 | memory | GEN_SQL_DML | 2748636 | 169251524 | <span style="color:#dc2626">-6057.66%</span> |
| 56 | [00959 VIEW_TRIGGER_GENERATED_012](crates/bench/sqlite_parity/cases/SQLITE_PARITY_959_VIEW_TRIGGER_GENERATED_012.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2667272 | 145373225 | <span style="color:#dc2626">-5350.26%</span> |
| 57 | [00650 JOIN_SUBQUERY_EXISTS_043](crates/bench/sqlite_parity/cases/SQLITE_PARITY_650_JOIN_SUBQUERY_EXISTS_043.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2629420 | 139899743 | <span style="color:#dc2626">-5220.56%</span> |
| 58 | [01006 VIEW_TRIGGER_GENERATED_059](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1006_VIEW_TRIGGER_GENERATED_059.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2652944 | 140884889 | <span style="color:#dc2626">-5210.51%</span> |
| 59 | [00991 VIEW_TRIGGER_GENERATED_044](crates/bench/sqlite_parity/cases/SQLITE_PARITY_991_VIEW_TRIGGER_GENERATED_044.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2797388 | 145434253 | <span style="color:#dc2626">-5098.93%</span> |
| 60 | [00970 VIEW_TRIGGER_GENERATED_023](crates/bench/sqlite_parity/cases/SQLITE_PARITY_970_VIEW_TRIGGER_GENERATED_023.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2706837 | 138631709 | <span style="color:#dc2626">-5021.54%</span> |
| 61 | [00895 CONSTRAINT_FK_SAVEPOINT_028](crates/bench/sqlite_parity/cases/SQLITE_PARITY_895_CONSTRAINT_FK_SAVEPOINT_028.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2718618 | 138694164 | <span style="color:#dc2626">-5001.64%</span> |
| 62 | [00975 VIEW_TRIGGER_GENERATED_028](crates/bench/sqlite_parity/cases/SQLITE_PARITY_975_VIEW_TRIGGER_GENERATED_028.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2663865 | 134999079 | <span style="color:#dc2626">-4967.79%</span> |
| 63 | [00960 VIEW_TRIGGER_GENERATED_013](crates/bench/sqlite_parity/cases/SQLITE_PARITY_960_VIEW_TRIGGER_GENERATED_013.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 3023617 | 151687383 | <span style="color:#dc2626">-4916.75%</span> |
| 64 | [00957 VIEW_TRIGGER_GENERATED_010](crates/bench/sqlite_parity/cases/SQLITE_PARITY_957_VIEW_TRIGGER_GENERATED_010.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2834528 | 140908349 | <span style="color:#dc2626">-4871.14%</span> |
| 65 | [00651 JOIN_SUBQUERY_EXISTS_044](crates/bench/sqlite_parity/cases/SQLITE_PARITY_651_JOIN_SUBQUERY_EXISTS_044.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2728458 | 135590322 | <span style="color:#dc2626">-4869.49%</span> |
| 66 | [00967 VIEW_TRIGGER_GENERATED_020](crates/bench/sqlite_parity/cases/SQLITE_PARITY_967_VIEW_TRIGGER_GENERATED_020.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2718278 | 134576989 | <span style="color:#dc2626">-4850.82%</span> |
| 67 | [00966 VIEW_TRIGGER_GENERATED_019](crates/bench/sqlite_parity/cases/SQLITE_PARITY_966_VIEW_TRIGGER_GENERATED_019.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2709551 | 134116828 | <span style="color:#dc2626">-4849.78%</span> |
| 68 | [00951 VIEW_TRIGGER_GENERATED_004](crates/bench/sqlite_parity/cases/SQLITE_PARITY_951_VIEW_TRIGGER_GENERATED_004.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2793501 | 138182016 | <span style="color:#dc2626">-4846.55%</span> |
| 69 | [00978 VIEW_TRIGGER_GENERATED_031](crates/bench/sqlite_parity/cases/SQLITE_PARITY_978_VIEW_TRIGGER_GENERATED_031.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 3124167 | 153909994 | <span style="color:#dc2626">-4826.43%</span> |
| 70 | [00647 JOIN_SUBQUERY_EXISTS_040](crates/bench/sqlite_parity/cases/SQLITE_PARITY_647_JOIN_SUBQUERY_EXISTS_040.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3260665 | 160182414 | <span style="color:#dc2626">-4812.57%</span> |
| 71 | [00950 VIEW_TRIGGER_GENERATED_003](crates/bench/sqlite_parity/cases/SQLITE_PARITY_950_VIEW_TRIGGER_GENERATED_003.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2915471 | 143086735 | <span style="color:#dc2626">-4807.84%</span> |
| 72 | [01005 VIEW_TRIGGER_GENERATED_058](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1005_VIEW_TRIGGER_GENERATED_058.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2894913 | 140487465 | <span style="color:#dc2626">-4752.91%</span> |
| 73 | [00803 WINDOW_PARTITION_SUM_016](crates/bench/sqlite_parity/cases/SQLITE_PARITY_803_WINDOW_PARTITION_SUM_016.rs) | P2 | memory | GEN_SQL_WINDOW | 3293667 | 159743547 | <span style="color:#dc2626">-4750.02%</span> |
| 74 | [00953 VIEW_TRIGGER_GENERATED_006](crates/bench/sqlite_parity/cases/SQLITE_PARITY_953_VIEW_TRIGGER_GENERATED_006.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2865116 | 138768988 | <span style="color:#dc2626">-4743.40%</span> |
| 75 | [00954 VIEW_TRIGGER_GENERATED_007](crates/bench/sqlite_parity/cases/SQLITE_PARITY_954_VIEW_TRIGGER_GENERATED_007.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2881096 | 139394482 | <span style="color:#dc2626">-4738.24%</span> |
| 76 | [01003 VIEW_TRIGGER_GENERATED_056](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1003_VIEW_TRIGGER_GENERATED_056.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 3048383 | 147393274 | <span style="color:#dc2626">-4735.13%</span> |
| 77 | [00961 VIEW_TRIGGER_GENERATED_014](crates/bench/sqlite_parity/cases/SQLITE_PARITY_961_VIEW_TRIGGER_GENERATED_014.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 3227913 | 154413567 | <span style="color:#dc2626">-4683.70%</span> |
| 78 | [00981 VIEW_TRIGGER_GENERATED_034](crates/bench/sqlite_parity/cases/SQLITE_PARITY_981_VIEW_TRIGGER_GENERATED_034.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 3080494 | 146676996 | <span style="color:#dc2626">-4661.48%</span> |
| 79 | [00983 VIEW_TRIGGER_GENERATED_036](crates/bench/sqlite_parity/cases/SQLITE_PARITY_983_VIEW_TRIGGER_GENERATED_036.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 3101694 | 147379997 | <span style="color:#dc2626">-4651.60%</span> |
| 80 | [01002 VIEW_TRIGGER_GENERATED_055](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1002_VIEW_TRIGGER_GENERATED_055.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 3203967 | 151415181 | <span style="color:#dc2626">-4625.87%</span> |
| 81 | [00976 VIEW_TRIGGER_GENERATED_029](crates/bench/sqlite_parity/cases/SQLITE_PARITY_976_VIEW_TRIGGER_GENERATED_029.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2919098 | 135889104 | <span style="color:#dc2626">-4555.17%</span> |
| 82 | [00640 JOIN_SUBQUERY_EXISTS_033](crates/bench/sqlite_parity/cases/SQLITE_PARITY_640_JOIN_SUBQUERY_EXISTS_033.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2884653 | 132541136 | <span style="color:#dc2626">-4494.70%</span> |
| 83 | [00958 VIEW_TRIGGER_GENERATED_011](crates/bench/sqlite_parity/cases/SQLITE_PARITY_958_VIEW_TRIGGER_GENERATED_011.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 3114138 | 142683161 | <span style="color:#dc2626">-4481.79%</span> |
| 84 | [00949 VIEW_TRIGGER_GENERATED_002](crates/bench/sqlite_parity/cases/SQLITE_PARITY_949_VIEW_TRIGGER_GENERATED_002.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2923617 | 133477436 | <span style="color:#dc2626">-4465.49%</span> |
| 85 | [00965 VIEW_TRIGGER_GENERATED_018](crates/bench/sqlite_parity/cases/SQLITE_PARITY_965_VIEW_TRIGGER_GENERATED_018.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2963673 | 134906353 | <span style="color:#dc2626">-4452.00%</span> |
| 86 | [00952 VIEW_TRIGGER_GENERATED_005](crates/bench/sqlite_parity/cases/SQLITE_PARITY_952_VIEW_TRIGGER_GENERATED_005.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 3330187 | 151326459 | <span style="color:#dc2626">-4444.08%</span> |
| 87 | [01004 VIEW_TRIGGER_GENERATED_057](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1004_VIEW_TRIGGER_GENERATED_057.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 3040899 | 138117727 | <span style="color:#dc2626">-4442.00%</span> |
| 88 | [00979 VIEW_TRIGGER_GENERATED_032](crates/bench/sqlite_parity/cases/SQLITE_PARITY_979_VIEW_TRIGGER_GENERATED_032.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2968613 | 134775266 | <span style="color:#dc2626">-4440.01%</span> |
| 89 | [00462 DML_WHERE_ORDER_LIMIT_075](crates/bench/sqlite_parity/cases/SQLITE_PARITY_462_DML_WHERE_ORDER_LIMIT_075.rs) | P1 | memory | GEN_SQL_DML | 3601781 | 163351832 | <span style="color:#dc2626">-4435.31%</span> |
| 90 | [00973 VIEW_TRIGGER_GENERATED_026](crates/bench/sqlite_parity/cases/SQLITE_PARITY_973_VIEW_TRIGGER_GENERATED_026.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2965446 | 134286951 | <span style="color:#dc2626">-4428.39%</span> |
| 91 | [00982 VIEW_TRIGGER_GENERATED_035](crates/bench/sqlite_parity/cases/SQLITE_PARITY_982_VIEW_TRIGGER_GENERATED_035.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 3281945 | 148321260 | <span style="color:#dc2626">-4419.31%</span> |
| 92 | [00984 VIEW_TRIGGER_GENERATED_037](crates/bench/sqlite_parity/cases/SQLITE_PARITY_984_VIEW_TRIGGER_GENERATED_037.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 3239886 | 146358302 | <span style="color:#dc2626">-4417.39%</span> |
| 93 | [00804 WINDOW_PARTITION_SUM_017](crates/bench/sqlite_parity/cases/SQLITE_PARITY_804_WINDOW_PARTITION_SUM_017.rs) | P2 | memory | GEN_SQL_WINDOW | 2550190 | 114759966 | <span style="color:#dc2626">-4400.06%</span> |
| 94 | [00977 VIEW_TRIGGER_GENERATED_030](crates/bench/sqlite_parity/cases/SQLITE_PARITY_977_VIEW_TRIGGER_GENERATED_030.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 3013497 | 134297962 | <span style="color:#dc2626">-4356.55%</span> |
| 95 | [00969 VIEW_TRIGGER_GENERATED_022](crates/bench/sqlite_parity/cases/SQLITE_PARITY_969_VIEW_TRIGGER_GENERATED_022.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 3001194 | 132950409 | <span style="color:#dc2626">-4329.92%</span> |
| 96 | [00629 JOIN_SUBQUERY_EXISTS_022](crates/bench/sqlite_parity/cases/SQLITE_PARITY_629_JOIN_SUBQUERY_EXISTS_022.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3448972 | 152588282 | <span style="color:#dc2626">-4324.17%</span> |
| 97 | [01007 VIEW_TRIGGER_GENERATED_060](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1007_VIEW_TRIGGER_GENERATED_060.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 3138364 | 138483660 | <span style="color:#dc2626">-4312.61%</span> |
| 98 | [00328 SCALAR_ARITH_026](crates/bench/sqlite_parity/cases/SQLITE_PARITY_328_SCALAR_ARITH_026.rs) | P1 | memory | GEN_SQL_SCALAR | 2684704 | 117811645 | <span style="color:#dc2626">-4288.25%</span> |
| 99 | [00468 DML_WHERE_ORDER_LIMIT_081](crates/bench/sqlite_parity/cases/SQLITE_PARITY_468_DML_WHERE_ORDER_LIMIT_081.rs) | P1 | memory | GEN_SQL_DML | 2480056 | 108429141 | <span style="color:#dc2626">-4272.04%</span> |
| 100 | [00974 VIEW_TRIGGER_GENERATED_027](crates/bench/sqlite_parity/cases/SQLITE_PARITY_974_VIEW_TRIGGER_GENERATED_027.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 3091194 | 134381730 | <span style="color:#dc2626">-4247.24%</span> |
| 101 | [00329 SCALAR_CAST_TYPEOF_026](crates/bench/sqlite_parity/cases/SQLITE_PARITY_329_SCALAR_CAST_TYPEOF_026.rs) | P1 | memory | GEN_SQL_SCALAR | 1830286 | 79227897 | <span style="color:#dc2626">-4228.72%</span> |
| 102 | [00989 VIEW_TRIGGER_GENERATED_042](crates/bench/sqlite_parity/cases/SQLITE_PARITY_989_VIEW_TRIGGER_GENERATED_042.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 3112405 | 134292532 | <span style="color:#dc2626">-4214.75%</span> |
| 103 | [00956 VIEW_TRIGGER_GENERATED_009](crates/bench/sqlite_parity/cases/SQLITE_PARITY_956_VIEW_TRIGGER_GENERATED_009.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 3320629 | 142654115 | <span style="color:#dc2626">-4196.00%</span> |
| 104 | [00980 VIEW_TRIGGER_GENERATED_033](crates/bench/sqlite_parity/cases/SQLITE_PARITY_980_VIEW_TRIGGER_GENERATED_033.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 3507212 | 150185751 | <span style="color:#dc2626">-4182.20%</span> |
| 105 | [00638 JOIN_SUBQUERY_EXISTS_031](crates/bench/sqlite_parity/cases/SQLITE_PARITY_638_JOIN_SUBQUERY_EXISTS_031.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3323474 | 142022973 | <span style="color:#dc2626">-4173.33%</span> |
| 106 | [00026 FOREIGN_KEYS_CASCADE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_026_FOREIGN_KEYS_CASCADE.rs) | P0 | memory | SQL_FOREIGN_KEYS | 1940736 | 82917052 | <span style="color:#dc2626">-4172.45%</span> |
| 107 | [00988 VIEW_TRIGGER_GENERATED_041](crates/bench/sqlite_parity/cases/SQLITE_PARITY_988_VIEW_TRIGGER_GENERATED_041.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 3142422 | 132998842 | <span style="color:#dc2626">-4132.37%</span> |
| 108 | [00972 VIEW_TRIGGER_GENERATED_025](crates/bench/sqlite_parity/cases/SQLITE_PARITY_972_VIEW_TRIGGER_GENERATED_025.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 3153612 | 133416401 | <span style="color:#dc2626">-4130.59%</span> |
| 109 | [00948 VIEW_TRIGGER_GENERATED_001](crates/bench/sqlite_parity/cases/SQLITE_PARITY_948_VIEW_TRIGGER_GENERATED_001.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 3147150 | 132385287 | <span style="color:#dc2626">-4106.51%</span> |
| 110 | [01001 VIEW_TRIGGER_GENERATED_054](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1001_VIEW_TRIGGER_GENERATED_054.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 3431018 | 144080049 | <span style="color:#dc2626">-4099.34%</span> |
| 111 | [01000 VIEW_TRIGGER_GENERATED_053](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1000_VIEW_TRIGGER_GENERATED_053.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 3218535 | 134227810 | <span style="color:#dc2626">-4070.46%</span> |
| 112 | [00641 JOIN_SUBQUERY_EXISTS_034](crates/bench/sqlite_parity/cases/SQLITE_PARITY_641_JOIN_SUBQUERY_EXISTS_034.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3248311 | 134461653 | <span style="color:#dc2626">-4039.43%</span> |
| 113 | [00999 VIEW_TRIGGER_GENERATED_052](crates/bench/sqlite_parity/cases/SQLITE_PARITY_999_VIEW_TRIGGER_GENERATED_052.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 3200352 | 132207814 | <span style="color:#dc2626">-4031.04%</span> |
| 114 | [00649 JOIN_SUBQUERY_EXISTS_042](crates/bench/sqlite_parity/cases/SQLITE_PARITY_649_JOIN_SUBQUERY_EXISTS_042.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3024227 | 122935335 | <span style="color:#dc2626">-3965.02%</span> |
| 115 | [00630 JOIN_SUBQUERY_EXISTS_023](crates/bench/sqlite_parity/cases/SQLITE_PARITY_630_JOIN_SUBQUERY_EXISTS_023.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3129476 | 126233220 | <span style="color:#dc2626">-3933.69%</span> |
| 116 | [00992 VIEW_TRIGGER_GENERATED_045](crates/bench/sqlite_parity/cases/SQLITE_PARITY_992_VIEW_TRIGGER_GENERATED_045.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 3360574 | 135470142 | <span style="color:#dc2626">-3931.16%</span> |
| 117 | [00971 VIEW_TRIGGER_GENERATED_024](crates/bench/sqlite_parity/cases/SQLITE_PARITY_971_VIEW_TRIGGER_GENERATED_024.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 3421900 | 135274511 | <span style="color:#dc2626">-3853.20%</span> |
| 118 | [00652 JOIN_SUBQUERY_EXISTS_045](crates/bench/sqlite_parity/cases/SQLITE_PARITY_652_JOIN_SUBQUERY_EXISTS_045.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3756193 | 144302210 | <span style="color:#dc2626">-3741.71%</span> |
| 119 | [00922 CONSTRAINT_FK_SAVEPOINT_055](crates/bench/sqlite_parity/cases/SQLITE_PARITY_922_CONSTRAINT_FK_SAVEPOINT_055.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2486790 | 95483260 | <span style="color:#dc2626">-3739.62%</span> |
| 120 | [00968 VIEW_TRIGGER_GENERATED_021](crates/bench/sqlite_parity/cases/SQLITE_PARITY_968_VIEW_TRIGGER_GENERATED_021.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 3494418 | 132664267 | <span style="color:#dc2626">-3696.46%</span> |
| 121 | [00326 SCALAR_STRING_025](crates/bench/sqlite_parity/cases/SQLITE_PARITY_326_SCALAR_STRING_025.rs) | P1 | memory | GEN_SQL_SCALAR | 2003754 | 75714113 | <span style="color:#dc2626">-3678.61%</span> |
| 122 | [00676 JOIN_SUBQUERY_EXISTS_069](crates/bench/sqlite_parity/cases/SQLITE_PARITY_676_JOIN_SUBQUERY_EXISTS_069.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2376701 | 89706027 | <span style="color:#dc2626">-3674.39%</span> |
| 123 | [00990 VIEW_TRIGGER_GENERATED_043](crates/bench/sqlite_parity/cases/SQLITE_PARITY_990_VIEW_TRIGGER_GENERATED_043.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 3568508 | 134513882 | <span style="color:#dc2626">-3669.47%</span> |
| 124 | [00919 CONSTRAINT_FK_SAVEPOINT_052](crates/bench/sqlite_parity/cases/SQLITE_PARITY_919_CONSTRAINT_FK_SAVEPOINT_052.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2679504 | 100949372 | <span style="color:#dc2626">-3667.46%</span> |
| 125 | [00946 CONSTRAINT_FK_SAVEPOINT_079](crates/bench/sqlite_parity/cases/SQLITE_PARITY_946_CONSTRAINT_FK_SAVEPOINT_079.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2494734 | 90894079 | <span style="color:#dc2626">-3543.44%</span> |
| 126 | [00648 JOIN_SUBQUERY_EXISTS_041](crates/bench/sqlite_parity/cases/SQLITE_PARITY_648_JOIN_SUBQUERY_EXISTS_041.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2924548 | 106414559 | <span style="color:#dc2626">-3538.67%</span> |
| 127 | [00406 DML_WHERE_ORDER_LIMIT_019](crates/bench/sqlite_parity/cases/SQLITE_PARITY_406_DML_WHERE_ORDER_LIMIT_019.rs) | P1 | memory | GEN_SQL_DML | 2660889 | 96797310 | <span style="color:#dc2626">-3537.78%</span> |
| 128 | [00931 CONSTRAINT_FK_SAVEPOINT_064](crates/bench/sqlite_parity/cases/SQLITE_PARITY_931_CONSTRAINT_FK_SAVEPOINT_064.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2554919 | 91114106 | <span style="color:#dc2626">-3466.22%</span> |
| 129 | [00907 CONSTRAINT_FK_SAVEPOINT_040](crates/bench/sqlite_parity/cases/SQLITE_PARITY_907_CONSTRAINT_FK_SAVEPOINT_040.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 3124237 | 109777231 | <span style="color:#dc2626">-3413.73%</span> |
| 130 | [00892 CONSTRAINT_FK_SAVEPOINT_025](crates/bench/sqlite_parity/cases/SQLITE_PARITY_892_CONSTRAINT_FK_SAVEPOINT_025.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2720722 | 95038758 | <span style="color:#dc2626">-3393.14%</span> |
| 131 | [00635 JOIN_SUBQUERY_EXISTS_028](crates/bench/sqlite_parity/cases/SQLITE_PARITY_635_JOIN_SUBQUERY_EXISTS_028.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3167018 | 110067826 | <span style="color:#dc2626">-3375.44%</span> |
| 132 | [00904 CONSTRAINT_FK_SAVEPOINT_037](crates/bench/sqlite_parity/cases/SQLITE_PARITY_904_CONSTRAINT_FK_SAVEPOINT_037.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 3022364 | 104067638 | <span style="color:#dc2626">-3343.25%</span> |
| 133 | [00868 CONSTRAINT_FK_SAVEPOINT_001](crates/bench/sqlite_parity/cases/SQLITE_PARITY_868_CONSTRAINT_FK_SAVEPOINT_001.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2744638 | 94378628 | <span style="color:#dc2626">-3338.65%</span> |
| 134 | [00499 DML_WHERE_ORDER_LIMIT_112](crates/bench/sqlite_parity/cases/SQLITE_PARITY_499_DML_WHERE_ORDER_LIMIT_112.rs) | P1 | memory | GEN_SQL_DML | 2833646 | 97421465 | <span style="color:#dc2626">-3338.03%</span> |
| 135 | [00633 JOIN_SUBQUERY_EXISTS_026](crates/bench/sqlite_parity/cases/SQLITE_PARITY_633_JOIN_SUBQUERY_EXISTS_026.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3238063 | 109665824 | <span style="color:#dc2626">-3286.77%</span> |
| 136 | [00639 JOIN_SUBQUERY_EXISTS_032](crates/bench/sqlite_parity/cases/SQLITE_PARITY_639_JOIN_SUBQUERY_EXISTS_032.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3397304 | 114728673 | <span style="color:#dc2626">-3277.05%</span> |
| 137 | [00925 CONSTRAINT_FK_SAVEPOINT_058](crates/bench/sqlite_parity/cases/SQLITE_PARITY_925_CONSTRAINT_FK_SAVEPOINT_058.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2741362 | 92119630 | <span style="color:#dc2626">-3260.36%</span> |
| 138 | [00928 CONSTRAINT_FK_SAVEPOINT_061](crates/bench/sqlite_parity/cases/SQLITE_PARITY_928_CONSTRAINT_FK_SAVEPOINT_061.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2740621 | 91889655 | <span style="color:#dc2626">-3252.88%</span> |
| 139 | [00812 WINDOW_PARTITION_SUM_025](crates/bench/sqlite_parity/cases/SQLITE_PARITY_812_WINDOW_PARTITION_SUM_025.rs) | P2 | memory | GEN_SQL_WINDOW | 2602298 | 87242282 | <span style="color:#dc2626">-3252.51%</span> |
| 140 | [00482 DML_WHERE_ORDER_LIMIT_095](crates/bench/sqlite_parity/cases/SQLITE_PARITY_482_DML_WHERE_ORDER_LIMIT_095.rs) | P1 | memory | GEN_SQL_DML | 1828593 | 61090051 | <span style="color:#dc2626">-3240.82%</span> |
| 141 | [00642 JOIN_SUBQUERY_EXISTS_035](crates/bench/sqlite_parity/cases/SQLITE_PARITY_642_JOIN_SUBQUERY_EXISTS_035.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2714581 | 90092167 | <span style="color:#dc2626">-3218.82%</span> |
| 142 | [00699 JOIN_SUBQUERY_EXISTS_092](crates/bench/sqlite_parity/cases/SQLITE_PARITY_699_JOIN_SUBQUERY_EXISTS_092.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2924299 | 96812966 | <span style="color:#dc2626">-3210.64%</span> |
| 143 | [00883 CONSTRAINT_FK_SAVEPOINT_016](crates/bench/sqlite_parity/cases/SQLITE_PARITY_883_CONSTRAINT_FK_SAVEPOINT_016.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2804201 | 92716189 | <span style="color:#dc2626">-3206.33%</span> |
| 144 | [00475 DML_WHERE_ORDER_LIMIT_088](crates/bench/sqlite_parity/cases/SQLITE_PARITY_475_DML_WHERE_ORDER_LIMIT_088.rs) | P1 | memory | GEN_SQL_DML | 3131560 | 103281633 | <span style="color:#dc2626">-3198.09%</span> |
| 145 | [00880 CONSTRAINT_FK_SAVEPOINT_013](crates/bench/sqlite_parity/cases/SQLITE_PARITY_880_CONSTRAINT_FK_SAVEPOINT_013.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2807597 | 92231091 | <span style="color:#dc2626">-3185.05%</span> |
| 146 | [00627 JOIN_SUBQUERY_EXISTS_020](crates/bench/sqlite_parity/cases/SQLITE_PARITY_627_JOIN_SUBQUERY_EXISTS_020.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2173686 | 71206851 | <span style="color:#dc2626">-3175.86%</span> |
| 147 | [00898 CONSTRAINT_FK_SAVEPOINT_031](crates/bench/sqlite_parity/cases/SQLITE_PARITY_898_CONSTRAINT_FK_SAVEPOINT_031.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 3017134 | 98826602 | <span style="color:#dc2626">-3175.51%</span> |
| 148 | [00505 DML_WHERE_ORDER_LIMIT_118](crates/bench/sqlite_parity/cases/SQLITE_PARITY_505_DML_WHERE_ORDER_LIMIT_118.rs) | P1 | memory | GEN_SQL_DML | 2480367 | 81184366 | <span style="color:#dc2626">-3173.08%</span> |
| 149 | [00617 JOIN_SUBQUERY_EXISTS_010](crates/bench/sqlite_parity/cases/SQLITE_PARITY_617_JOIN_SUBQUERY_EXISTS_010.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2221888 | 72351900 | <span style="color:#dc2626">-3156.33%</span> |
| 150 | [00814 WINDOW_PARTITION_SUM_027](crates/bench/sqlite_parity/cases/SQLITE_PARITY_814_WINDOW_PARTITION_SUM_027.rs) | P2 | memory | GEN_SQL_WINDOW | 2012501 | 65068288 | <span style="color:#dc2626">-3133.21%</span> |
| 151 | [00038 CREATE_TRIGGER_AFTER](crates/bench/sqlite_parity/cases/SQLITE_PARITY_038_CREATE_TRIGGER_AFTER.rs) | P0 | memory | SQL_TRIGGER | 2488232 | 80156905 | <span style="color:#dc2626">-3121.44%</span> |
| 152 | [00496 DML_WHERE_ORDER_LIMIT_109](crates/bench/sqlite_parity/cases/SQLITE_PARITY_496_DML_WHERE_ORDER_LIMIT_109.rs) | P1 | memory | GEN_SQL_DML | 2035034 | 65467853 | <span style="color:#dc2626">-3117.04%</span> |
| 153 | [00493 DML_WHERE_ORDER_LIMIT_106](crates/bench/sqlite_parity/cases/SQLITE_PARITY_493_DML_WHERE_ORDER_LIMIT_106.rs) | P1 | memory | GEN_SQL_DML | 1867126 | 59374211 | <span style="color:#dc2626">-3079.98%</span> |
| 154 | [00634 JOIN_SUBQUERY_EXISTS_027](crates/bench/sqlite_parity/cases/SQLITE_PARITY_634_JOIN_SUBQUERY_EXISTS_027.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3540354 | 112458083 | <span style="color:#dc2626">-3076.46%</span> |
| 155 | [00463 DML_WHERE_ORDER_LIMIT_076](crates/bench/sqlite_parity/cases/SQLITE_PARITY_463_DML_WHERE_ORDER_LIMIT_076.rs) | P1 | memory | GEN_SQL_DML | 3165786 | 100523018 | <span style="color:#dc2626">-3075.29%</span> |
| 156 | [00934 CONSTRAINT_FK_SAVEPOINT_067](crates/bench/sqlite_parity/cases/SQLITE_PARITY_934_CONSTRAINT_FK_SAVEPOINT_067.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2894532 | 91779597 | <span style="color:#dc2626">-3070.79%</span> |
| 157 | [00943 CONSTRAINT_FK_SAVEPOINT_076](crates/bench/sqlite_parity/cases/SQLITE_PARITY_943_CONSTRAINT_FK_SAVEPOINT_076.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2859094 | 90265228 | <span style="color:#dc2626">-3057.13%</span> |
| 158 | [00644 JOIN_SUBQUERY_EXISTS_037](crates/bench/sqlite_parity/cases/SQLITE_PARITY_644_JOIN_SUBQUERY_EXISTS_037.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3414417 | 107620762 | <span style="color:#dc2626">-3051.95%</span> |
| 159 | [00616 JOIN_SUBQUERY_EXISTS_009](crates/bench/sqlite_parity/cases/SQLITE_PARITY_616_JOIN_SUBQUERY_EXISTS_009.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2301899 | 72426510 | <span style="color:#dc2626">-3046.38%</span> |
| 160 | [00700 JOIN_SUBQUERY_EXISTS_093](crates/bench/sqlite_parity/cases/SQLITE_PARITY_700_JOIN_SUBQUERY_EXISTS_093.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2725883 | 85092119 | <span style="color:#dc2626">-3021.64%</span> |
| 161 | [00498 DML_WHERE_ORDER_LIMIT_111](crates/bench/sqlite_parity/cases/SQLITE_PARITY_498_DML_WHERE_ORDER_LIMIT_111.rs) | P1 | memory | GEN_SQL_DML | 2566290 | 79954887 | <span style="color:#dc2626">-3015.58%</span> |
| 162 | [00871 CONSTRAINT_FK_SAVEPOINT_004](crates/bench/sqlite_parity/cases/SQLITE_PARITY_871_CONSTRAINT_FK_SAVEPOINT_004.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 3108537 | 96747865 | <span style="color:#dc2626">-3012.33%</span> |
| 163 | [00626 JOIN_SUBQUERY_EXISTS_019](crates/bench/sqlite_parity/cases/SQLITE_PARITY_626_JOIN_SUBQUERY_EXISTS_019.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2798249 | 87074361 | <span style="color:#dc2626">-3011.74%</span> |
| 164 | [00637 JOIN_SUBQUERY_EXISTS_030](crates/bench/sqlite_parity/cases/SQLITE_PARITY_637_JOIN_SUBQUERY_EXISTS_030.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3194740 | 98974808 | <span style="color:#dc2626">-2998.06%</span> |
| 165 | [00618 JOIN_SUBQUERY_EXISTS_011](crates/bench/sqlite_parity/cases/SQLITE_PARITY_618_JOIN_SUBQUERY_EXISTS_011.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2333409 | 72273771 | <span style="color:#dc2626">-2997.35%</span> |
| 166 | [00631 JOIN_SUBQUERY_EXISTS_024](crates/bench/sqlite_parity/cases/SQLITE_PARITY_631_JOIN_SUBQUERY_EXISTS_024.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3393997 | 104989218 | <span style="color:#dc2626">-2993.38%</span> |
| 167 | [00398 DML_WHERE_ORDER_LIMIT_011](crates/bench/sqlite_parity/cases/SQLITE_PARITY_398_DML_WHERE_ORDER_LIMIT_011.rs) | P1 | memory | GEN_SQL_DML | 1908074 | 59004270 | <span style="color:#dc2626">-2992.35%</span> |
| 168 | [00643 JOIN_SUBQUERY_EXISTS_036](crates/bench/sqlite_parity/cases/SQLITE_PARITY_643_JOIN_SUBQUERY_EXISTS_036.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3453911 | 106718453 | <span style="color:#dc2626">-2989.79%</span> |
| 169 | [00492 DML_WHERE_ORDER_LIMIT_105](crates/bench/sqlite_parity/cases/SQLITE_PARITY_492_DML_WHERE_ORDER_LIMIT_105.rs) | P1 | memory | GEN_SQL_DML | 2846410 | 87881378 | <span style="color:#dc2626">-2987.45%</span> |
| 170 | [00704 JOIN_SUBQUERY_EXISTS_097](crates/bench/sqlite_parity/cases/SQLITE_PARITY_704_JOIN_SUBQUERY_EXISTS_097.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2756230 | 84606249 | <span style="color:#dc2626">-2969.64%</span> |
| 171 | [00401 DML_WHERE_ORDER_LIMIT_014](crates/bench/sqlite_parity/cases/SQLITE_PARITY_401_DML_WHERE_ORDER_LIMIT_014.rs) | P1 | memory | GEN_SQL_DML | 1872356 | 57334487 | <span style="color:#dc2626">-2962.16%</span> |
| 172 | [00665 JOIN_SUBQUERY_EXISTS_058](crates/bench/sqlite_parity/cases/SQLITE_PARITY_665_JOIN_SUBQUERY_EXISTS_058.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2792959 | 85454705 | <span style="color:#dc2626">-2959.65%</span> |
| 173 | [00489 DML_WHERE_ORDER_LIMIT_102](crates/bench/sqlite_parity/cases/SQLITE_PARITY_489_DML_WHERE_ORDER_LIMIT_102.rs) | P1 | memory | GEN_SQL_DML | 2027309 | 61960741 | <span style="color:#dc2626">-2956.30%</span> |
| 174 | [00669 JOIN_SUBQUERY_EXISTS_062](crates/bench/sqlite_parity/cases/SQLITE_PARITY_669_JOIN_SUBQUERY_EXISTS_062.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2317739 | 70388262 | <span style="color:#dc2626">-2936.94%</span> |
| 175 | [00448 DML_WHERE_ORDER_LIMIT_061](crates/bench/sqlite_parity/cases/SQLITE_PARITY_448_DML_WHERE_ORDER_LIMIT_061.rs) | P1 | memory | GEN_SQL_DML | 2485658 | 75392755 | <span style="color:#dc2626">-2933.11%</span> |
| 176 | [00400 DML_WHERE_ORDER_LIMIT_013](crates/bench/sqlite_parity/cases/SQLITE_PARITY_400_DML_WHERE_ORDER_LIMIT_013.rs) | P1 | memory | GEN_SQL_DML | 1934043 | 58481961 | <span style="color:#dc2626">-2923.82%</span> |
| 177 | [00874 CONSTRAINT_FK_SAVEPOINT_007](crates/bench/sqlite_parity/cases/SQLITE_PARITY_874_CONSTRAINT_FK_SAVEPOINT_007.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 3085353 | 93291839 | <span style="color:#dc2626">-2923.70%</span> |
| 178 | [00683 JOIN_SUBQUERY_EXISTS_076](crates/bench/sqlite_parity/cases/SQLITE_PARITY_683_JOIN_SUBQUERY_EXISTS_076.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2557003 | 76863986 | <span style="color:#dc2626">-2906.02%</span> |
| 179 | [00453 DML_WHERE_ORDER_LIMIT_066](crates/bench/sqlite_parity/cases/SQLITE_PARITY_453_DML_WHERE_ORDER_LIMIT_066.rs) | P1 | memory | GEN_SQL_DML | 2515975 | 75621491 | <span style="color:#dc2626">-2905.65%</span> |
| 180 | [00388 DML_WHERE_ORDER_LIMIT_001](crates/bench/sqlite_parity/cases/SQLITE_PARITY_388_DML_WHERE_ORDER_LIMIT_001.rs) | P1 | memory | GEN_SQL_DML | 2326495 | 69284427 | <span style="color:#dc2626">-2878.06%</span> |
| 181 | [00886 CONSTRAINT_FK_SAVEPOINT_019](crates/bench/sqlite_parity/cases/SQLITE_PARITY_886_CONSTRAINT_FK_SAVEPOINT_019.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 3303276 | 97805849 | <span style="color:#dc2626">-2860.87%</span> |
| 182 | [00079 CHECK_CONSTRAINT_FAILURE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_079_CHECK_CONSTRAINT_FAILURE.rs) | P0 | memory | SQL_CONSTRAINTS_NEGATIVE | 1862467 | 55086038 | <span style="color:#dc2626">-2857.69%</span> |
| 183 | [00877 CONSTRAINT_FK_SAVEPOINT_010](crates/bench/sqlite_parity/cases/SQLITE_PARITY_877_CONSTRAINT_FK_SAVEPOINT_010.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 3161717 | 93472132 | <span style="color:#dc2626">-2856.37%</span> |
| 184 | [00485 DML_WHERE_ORDER_LIMIT_098](crates/bench/sqlite_parity/cases/SQLITE_PARITY_485_DML_WHERE_ORDER_LIMIT_098.rs) | P1 | memory | GEN_SQL_DML | 2024855 | 59844312 | <span style="color:#dc2626">-2855.49%</span> |
| 185 | [00455 DML_WHERE_ORDER_LIMIT_068](crates/bench/sqlite_parity/cases/SQLITE_PARITY_455_DML_WHERE_ORDER_LIMIT_068.rs) | P1 | memory | GEN_SQL_DML | 2038982 | 59994516 | <span style="color:#dc2626">-2842.38%</span> |
| 186 | [00901 CONSTRAINT_FK_SAVEPOINT_034](crates/bench/sqlite_parity/cases/SQLITE_PARITY_901_CONSTRAINT_FK_SAVEPOINT_034.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 3312985 | 97050428 | <span style="color:#dc2626">-2829.40%</span> |
| 187 | [00214 DROP_TABLE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_214_DROP_TABLE.rs) | P0 | memory | SQL_DROP | 1851777 | 53811655 | <span style="color:#dc2626">-2805.95%</span> |
| 188 | [00500 DML_WHERE_ORDER_LIMIT_113](crates/bench/sqlite_parity/cases/SQLITE_PARITY_500_DML_WHERE_ORDER_LIMIT_113.rs) | P1 | memory | GEN_SQL_DML | 2802858 | 81414551 | <span style="color:#dc2626">-2804.70%</span> |
| 189 | [00041 DROP_TRIGGER](crates/bench/sqlite_parity/cases/SQLITE_PARITY_041_DROP_TRIGGER.rs) | P0 | memory | SQL_DROP | 2599593 | 74473252 | <span style="color:#dc2626">-2764.80%</span> |
| 190 | [00940 CONSTRAINT_FK_SAVEPOINT_073](crates/bench/sqlite_parity/cases/SQLITE_PARITY_940_CONSTRAINT_FK_SAVEPOINT_073.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 3126421 | 89432241 | <span style="color:#dc2626">-2760.53%</span> |
| 191 | [00675 JOIN_SUBQUERY_EXISTS_068](crates/bench/sqlite_parity/cases/SQLITE_PARITY_675_JOIN_SUBQUERY_EXISTS_068.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2505165 | 71623041 | <span style="color:#dc2626">-2759.01%</span> |
| 192 | [00691 JOIN_SUBQUERY_EXISTS_084](crates/bench/sqlite_parity/cases/SQLITE_PARITY_691_JOIN_SUBQUERY_EXISTS_084.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2473444 | 70678542 | <span style="color:#dc2626">-2757.50%</span> |
| 193 | [00916 CONSTRAINT_FK_SAVEPOINT_049](crates/bench/sqlite_parity/cases/SQLITE_PARITY_916_CONSTRAINT_FK_SAVEPOINT_049.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 3219397 | 91990536 | <span style="color:#dc2626">-2757.38%</span> |
| 194 | [00667 JOIN_SUBQUERY_EXISTS_060](crates/bench/sqlite_parity/cases/SQLITE_PARITY_667_JOIN_SUBQUERY_EXISTS_060.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2432987 | 69446238 | <span style="color:#dc2626">-2754.36%</span> |
| 195 | [00632 JOIN_SUBQUERY_EXISTS_025](crates/bench/sqlite_parity/cases/SQLITE_PARITY_632_JOIN_SUBQUERY_EXISTS_025.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3432400 | 97769556 | <span style="color:#dc2626">-2748.43%</span> |
| 196 | [00461 DML_WHERE_ORDER_LIMIT_074](crates/bench/sqlite_parity/cases/SQLITE_PARITY_461_DML_WHERE_ORDER_LIMIT_074.rs) | P1 | memory | GEN_SQL_DML | 2243449 | 63587842 | <span style="color:#dc2626">-2734.38%</span> |
| 197 | [00017 DELETE_BASIC](crates/bench/sqlite_parity/cases/SQLITE_PARITY_017_DELETE_BASIC.rs) | P0 | memory | SQL_DELETE | 2316587 | 65630927 | <span style="color:#dc2626">-2733.09%</span> |
| 198 | [00620 JOIN_SUBQUERY_EXISTS_013](crates/bench/sqlite_parity/cases/SQLITE_PARITY_620_JOIN_SUBQUERY_EXISTS_013.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2511717 | 71010610 | <span style="color:#dc2626">-2727.17%</span> |
| 199 | [00889 CONSTRAINT_FK_SAVEPOINT_022](crates/bench/sqlite_parity/cases/SQLITE_PARITY_889_CONSTRAINT_FK_SAVEPOINT_022.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 3361256 | 95023980 | <span style="color:#dc2626">-2727.04%</span> |
| 200 | [00464 DML_WHERE_ORDER_LIMIT_077](crates/bench/sqlite_parity/cases/SQLITE_PARITY_464_DML_WHERE_ORDER_LIMIT_077.rs) | P1 | memory | GEN_SQL_DML | 2222779 | 62698527 | <span style="color:#dc2626">-2720.73%</span> |
| 201 | [00610 JOIN_SUBQUERY_EXISTS_003](crates/bench/sqlite_parity/cases/SQLITE_PARITY_610_JOIN_SUBQUERY_EXISTS_003.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2714280 | 76359398 | <span style="color:#dc2626">-2713.25%</span> |
| 202 | [00472 DML_WHERE_ORDER_LIMIT_085](crates/bench/sqlite_parity/cases/SQLITE_PARITY_472_DML_WHERE_ORDER_LIMIT_085.rs) | P1 | memory | GEN_SQL_DML | 2132509 | 59595421 | <span style="color:#dc2626">-2694.62%</span> |
| 203 | [00465 DML_WHERE_ORDER_LIMIT_078](crates/bench/sqlite_parity/cases/SQLITE_PARITY_465_DML_WHERE_ORDER_LIMIT_078.rs) | P1 | memory | GEN_SQL_DML | 2436354 | 67609467 | <span style="color:#dc2626">-2675.03%</span> |
| 204 | [00608 JOIN_SUBQUERY_EXISTS_001](crates/bench/sqlite_parity/cases/SQLITE_PARITY_608_JOIN_SUBQUERY_EXISTS_001.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2749998 | 76029785 | <span style="color:#dc2626">-2664.72%</span> |
| 205 | [00024 SAVEPOINT_ROLLBACK_RELEASE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_024_SAVEPOINT_ROLLBACK_RELEASE.rs) | P0 | memory | SQL_SAVEPOINT | 1989317 | 54899304 | <span style="color:#dc2626">-2659.71%</span> |
| 206 | [00410 DML_WHERE_ORDER_LIMIT_023](crates/bench/sqlite_parity/cases/SQLITE_PARITY_410_DML_WHERE_ORDER_LIMIT_023.rs) | P1 | memory | GEN_SQL_DML | 2471410 | 67849741 | <span style="color:#dc2626">-2645.39%</span> |
| 207 | [00502 DML_WHERE_ORDER_LIMIT_115](crates/bench/sqlite_parity/cases/SQLITE_PARITY_502_DML_WHERE_ORDER_LIMIT_115.rs) | P1 | memory | GEN_SQL_DML | 2317439 | 63284838 | <span style="color:#dc2626">-2630.81%</span> |
| 208 | [00477 DML_WHERE_ORDER_LIMIT_090](crates/bench/sqlite_parity/cases/SQLITE_PARITY_477_DML_WHERE_ORDER_LIMIT_090.rs) | P1 | memory | GEN_SQL_DML | 2211258 | 60321596 | <span style="color:#dc2626">-2627.93%</span> |
| 209 | [00435 DML_WHERE_ORDER_LIMIT_048](crates/bench/sqlite_parity/cases/SQLITE_PARITY_435_DML_WHERE_ORDER_LIMIT_048.rs) | P1 | memory | GEN_SQL_DML | 2489956 | 67745283 | <span style="color:#dc2626">-2620.74%</span> |
| 210 | [00682 JOIN_SUBQUERY_EXISTS_075](crates/bench/sqlite_parity/cases/SQLITE_PARITY_682_JOIN_SUBQUERY_EXISTS_075.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3094811 | 84178428 | <span style="color:#dc2626">-2619.99%</span> |
| 211 | [00622 JOIN_SUBQUERY_EXISTS_015](crates/bench/sqlite_parity/cases/SQLITE_PARITY_622_JOIN_SUBQUERY_EXISTS_015.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2535422 | 68766861 | <span style="color:#dc2626">-2612.25%</span> |
| 212 | [00458 DML_WHERE_ORDER_LIMIT_071](crates/bench/sqlite_parity/cases/SQLITE_PARITY_458_DML_WHERE_ORDER_LIMIT_071.rs) | P1 | memory | GEN_SQL_DML | 2207600 | 59812612 | <span style="color:#dc2626">-2609.40%</span> |
| 213 | [00937 CONSTRAINT_FK_SAVEPOINT_070](crates/bench/sqlite_parity/cases/SQLITE_PARITY_937_CONSTRAINT_FK_SAVEPOINT_070.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 3468419 | 93227899 | <span style="color:#dc2626">-2587.91%</span> |
| 214 | [00479 DML_WHERE_ORDER_LIMIT_092](crates/bench/sqlite_parity/cases/SQLITE_PARITY_479_DML_WHERE_ORDER_LIMIT_092.rs) | P1 | memory | GEN_SQL_DML | 2444530 | 65160309 | <span style="color:#dc2626">-2565.56%</span> |
| 215 | [00788 WINDOW_PARTITION_SUM_001](crates/bench/sqlite_parity/cases/SQLITE_PARITY_788_WINDOW_PARTITION_SUM_001.rs) | P2 | memory | GEN_SQL_WINDOW | 2636383 | 70196670 | <span style="color:#dc2626">-2562.61%</span> |
| 216 | [00407 DML_WHERE_ORDER_LIMIT_020](crates/bench/sqlite_parity/cases/SQLITE_PARITY_407_DML_WHERE_ORDER_LIMIT_020.rs) | P1 | memory | GEN_SQL_DML | 2512498 | 66722376 | <span style="color:#dc2626">-2555.62%</span> |
| 217 | [00658 JOIN_SUBQUERY_EXISTS_051](crates/bench/sqlite_parity/cases/SQLITE_PARITY_658_JOIN_SUBQUERY_EXISTS_051.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2952762 | 77765903 | <span style="color:#dc2626">-2533.67%</span> |
| 218 | [00011 CREATE_TABLE_AS_SELECT](crates/bench/sqlite_parity/cases/SQLITE_PARITY_011_CREATE_TABLE_AS_SELECT.rs) | P0 | memory | SQL_DDL | 1788938 | 47104914 | <span style="color:#dc2626">-2533.12%</span> |
| 219 | [00408 DML_WHERE_ORDER_LIMIT_021](crates/bench/sqlite_parity/cases/SQLITE_PARITY_408_DML_WHERE_ORDER_LIMIT_021.rs) | P1 | memory | GEN_SQL_DML | 2349991 | 61822086 | <span style="color:#dc2626">-2530.74%</span> |
| 220 | [00843 WINDOW_PARTITION_SUM_056](crates/bench/sqlite_parity/cases/SQLITE_PARITY_843_WINDOW_PARTITION_SUM_056.rs) | P2 | memory | GEN_SQL_WINDOW | 2323160 | 60758046 | <span style="color:#dc2626">-2515.32%</span> |
| 221 | [00488 DML_WHERE_ORDER_LIMIT_101](crates/bench/sqlite_parity/cases/SQLITE_PARITY_488_DML_WHERE_ORDER_LIMIT_101.rs) | P1 | memory | GEN_SQL_DML | 2967509 | 77600829 | <span style="color:#dc2626">-2515.02%</span> |
| 222 | [00039 CREATE_TRIGGER_BEFORE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_039_CREATE_TRIGGER_BEFORE.rs) | P0 | memory | SQL_TRIGGER | 2397500 | 62662235 | <span style="color:#dc2626">-2513.65%</span> |
| 223 | [00390 DML_WHERE_ORDER_LIMIT_003](crates/bench/sqlite_parity/cases/SQLITE_PARITY_390_DML_WHERE_ORDER_LIMIT_003.rs) | P1 | memory | GEN_SQL_DML | 2838816 | 74135474 | <span style="color:#dc2626">-2511.49%</span> |
| 224 | [00417 DML_WHERE_ORDER_LIMIT_030](crates/bench/sqlite_parity/cases/SQLITE_PARITY_417_DML_WHERE_ORDER_LIMIT_030.rs) | P1 | memory | GEN_SQL_DML | 2702518 | 70556296 | <span style="color:#dc2626">-2510.76%</span> |
| 225 | [00503 DML_WHERE_ORDER_LIMIT_116](crates/bench/sqlite_parity/cases/SQLITE_PARITY_503_DML_WHERE_ORDER_LIMIT_116.rs) | P1 | memory | GEN_SQL_DML | 2333510 | 60902045 | <span style="color:#dc2626">-2509.89%</span> |
| 226 | [00473 DML_WHERE_ORDER_LIMIT_086](crates/bench/sqlite_parity/cases/SQLITE_PARITY_473_DML_WHERE_ORDER_LIMIT_086.rs) | P1 | memory | GEN_SQL_DML | 2948474 | 76803909 | <span style="color:#dc2626">-2504.87%</span> |
| 227 | [00666 JOIN_SUBQUERY_EXISTS_059](crates/bench/sqlite_parity/cases/SQLITE_PARITY_666_JOIN_SUBQUERY_EXISTS_059.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2589083 | 67310223 | <span style="color:#dc2626">-2499.77%</span> |
| 228 | [00399 DML_WHERE_ORDER_LIMIT_012](crates/bench/sqlite_parity/cases/SQLITE_PARITY_399_DML_WHERE_ORDER_LIMIT_012.rs) | P1 | memory | GEN_SQL_DML | 2594584 | 66725972 | <span style="color:#dc2626">-2471.74%</span> |
| 229 | [00625 JOIN_SUBQUERY_EXISTS_018](crates/bench/sqlite_parity/cases/SQLITE_PARITY_625_JOIN_SUBQUERY_EXISTS_018.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3026101 | 77385443 | <span style="color:#dc2626">-2457.27%</span> |
| 230 | [00684 JOIN_SUBQUERY_EXISTS_077](crates/bench/sqlite_parity/cases/SQLITE_PARITY_684_JOIN_SUBQUERY_EXISTS_077.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3209167 | 82059606 | <span style="color:#dc2626">-2457.04%</span> |
| 231 | [00403 DML_WHERE_ORDER_LIMIT_016](crates/bench/sqlite_parity/cases/SQLITE_PARITY_403_DML_WHERE_ORDER_LIMIT_016.rs) | P1 | memory | GEN_SQL_DML | 2349189 | 60015424 | <span style="color:#dc2626">-2454.73%</span> |
| 232 | [00687 JOIN_SUBQUERY_EXISTS_080](crates/bench/sqlite_parity/cases/SQLITE_PARITY_687_JOIN_SUBQUERY_EXISTS_080.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3010070 | 76807149 | <span style="color:#dc2626">-2451.67%</span> |
| 233 | [00660 JOIN_SUBQUERY_EXISTS_053](crates/bench/sqlite_parity/cases/SQLITE_PARITY_660_JOIN_SUBQUERY_EXISTS_053.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2681768 | 68309345 | <span style="color:#dc2626">-2447.18%</span> |
| 234 | [00474 DML_WHERE_ORDER_LIMIT_087](crates/bench/sqlite_parity/cases/SQLITE_PARITY_474_DML_WHERE_ORDER_LIMIT_087.rs) | P1 | memory | GEN_SQL_DML | 2290988 | 58295899 | <span style="color:#dc2626">-2444.57%</span> |
| 235 | [00624 JOIN_SUBQUERY_EXISTS_017](crates/bench/sqlite_parity/cases/SQLITE_PARITY_624_JOIN_SUBQUERY_EXISTS_017.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2875365 | 72803926 | <span style="color:#dc2626">-2431.99%</span> |
| 236 | [00412 DML_WHERE_ORDER_LIMIT_025](crates/bench/sqlite_parity/cases/SQLITE_PARITY_412_DML_WHERE_ORDER_LIMIT_025.rs) | P1 | memory | GEN_SQL_DML | 2322368 | 58749087 | <span style="color:#dc2626">-2429.71%</span> |
| 237 | [00036 CREATE_VIEW](crates/bench/sqlite_parity/cases/SQLITE_PARITY_036_CREATE_VIEW.rs) | P0 | memory | SQL_VIEW | 2781988 | 70370592 | <span style="color:#dc2626">-2429.51%</span> |
| 238 | [00030 ALTER_TABLE_ADD_DROP_COLUMN](crates/bench/sqlite_parity/cases/SQLITE_PARITY_030_ALTER_TABLE_ADD_DROP_COLUMN.rs) | P0 | memory | SQL_ALTER | 3228925 | 81437680 | <span style="color:#dc2626">-2422.13%</span> |
| 239 | [00636 JOIN_SUBQUERY_EXISTS_029](crates/bench/sqlite_parity/cases/SQLITE_PARITY_636_JOIN_SUBQUERY_EXISTS_029.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3883034 | 97332799 | <span style="color:#dc2626">-2406.62%</span> |
| 240 | [00628 JOIN_SUBQUERY_EXISTS_021](crates/bench/sqlite_parity/cases/SQLITE_PARITY_628_JOIN_SUBQUERY_EXISTS_021.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2809972 | 70396577 | <span style="color:#dc2626">-2405.24%</span> |
| 241 | [00456 DML_WHERE_ORDER_LIMIT_069](crates/bench/sqlite_parity/cases/SQLITE_PARITY_456_DML_WHERE_ORDER_LIMIT_069.rs) | P1 | memory | GEN_SQL_DML | 2399995 | 60042096 | <span style="color:#dc2626">-2401.76%</span> |
| 242 | [00623 JOIN_SUBQUERY_EXISTS_016](crates/bench/sqlite_parity/cases/SQLITE_PARITY_623_JOIN_SUBQUERY_EXISTS_016.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3090132 | 76896706 | <span style="color:#dc2626">-2388.46%</span> |
| 243 | [00101 SCHEMA_SQLITE_MASTER_ALIAS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_101_SCHEMA_SQLITE_MASTER_ALIAS.rs) | P0 | memory | SQL_SCHEMA | 1795120 | 44634396 | <span style="color:#dc2626">-2386.43%</span> |
| 244 | [00705 JOIN_SUBQUERY_EXISTS_098](crates/bench/sqlite_parity/cases/SQLITE_PARITY_705_JOIN_SUBQUERY_EXISTS_098.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2891546 | 71812571 | <span style="color:#dc2626">-2383.54%</span> |
| 245 | [00405 DML_WHERE_ORDER_LIMIT_018](crates/bench/sqlite_parity/cases/SQLITE_PARITY_405_DML_WHERE_ORDER_LIMIT_018.rs) | P1 | memory | GEN_SQL_DML | 2361051 | 58506497 | <span style="color:#dc2626">-2377.99%</span> |
| 246 | [00668 JOIN_SUBQUERY_EXISTS_061](crates/bench/sqlite_parity/cases/SQLITE_PARITY_668_JOIN_SUBQUERY_EXISTS_061.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2812747 | 69636479 | <span style="color:#dc2626">-2375.75%</span> |
| 247 | [00686 JOIN_SUBQUERY_EXISTS_079](crates/bench/sqlite_parity/cases/SQLITE_PARITY_686_JOIN_SUBQUERY_EXISTS_079.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3049124 | 75153467 | <span style="color:#dc2626">-2364.76%</span> |
| 248 | [00330 SCALAR_STRING_026](crates/bench/sqlite_parity/cases/SQLITE_PARITY_330_SCALAR_STRING_026.rs) | P1 | memory | GEN_SQL_SCALAR | 2758384 | 67833018 | <span style="color:#dc2626">-2359.16%</span> |
| 249 | [00469 DML_WHERE_ORDER_LIMIT_082](crates/bench/sqlite_parity/cases/SQLITE_PARITY_469_DML_WHERE_ORDER_LIMIT_082.rs) | P1 | memory | GEN_SQL_DML | 2337136 | 57208581 | <span style="color:#dc2626">-2347.81%</span> |
| 250 | [00444 DML_WHERE_ORDER_LIMIT_057](crates/bench/sqlite_parity/cases/SQLITE_PARITY_444_DML_WHERE_ORDER_LIMIT_057.rs) | P1 | memory | GEN_SQL_DML | 2860968 | 69873724 | <span style="color:#dc2626">-2342.31%</span> |
| 251 | [00790 WINDOW_PARTITION_SUM_003](crates/bench/sqlite_parity/cases/SQLITE_PARITY_790_WINDOW_PARTITION_SUM_003.rs) | P2 | memory | GEN_SQL_WINDOW | 1887524 | 46056683 | <span style="color:#dc2626">-2340.06%</span> |
| 252 | [00037 DROP_VIEW](crates/bench/sqlite_parity/cases/SQLITE_PARITY_037_DROP_VIEW.rs) | P0 | memory | SQL_DROP | 2597199 | 63307207 | <span style="color:#dc2626">-2337.52%</span> |
| 253 | [00425 DML_WHERE_ORDER_LIMIT_038](crates/bench/sqlite_parity/cases/SQLITE_PARITY_425_DML_WHERE_ORDER_LIMIT_038.rs) | P1 | memory | GEN_SQL_DML | 2450641 | 59691491 | <span style="color:#dc2626">-2335.75%</span> |
| 254 | [00685 JOIN_SUBQUERY_EXISTS_078](crates/bench/sqlite_parity/cases/SQLITE_PARITY_685_JOIN_SUBQUERY_EXISTS_078.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3195482 | 77791613 | <span style="color:#dc2626">-2334.43%</span> |
| 255 | [00006 ROWID_INTEGER_PRIMARY_KEY](crates/bench/sqlite_parity/cases/SQLITE_PARITY_006_ROWID_INTEGER_PRIMARY_KEY.rs) | P0 | memory | SQL_ROWID | 1795170 | 43554880 | <span style="color:#dc2626">-2326.23%</span> |
| 256 | [00688 JOIN_SUBQUERY_EXISTS_081](crates/bench/sqlite_parity/cases/SQLITE_PARITY_688_JOIN_SUBQUERY_EXISTS_081.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3199710 | 77528435 | <span style="color:#dc2626">-2322.98%</span> |
| 257 | [00396 DML_WHERE_ORDER_LIMIT_009](crates/bench/sqlite_parity/cases/SQLITE_PARITY_396_DML_WHERE_ORDER_LIMIT_009.rs) | P1 | memory | GEN_SQL_DML | 2423118 | 58698700 | <span style="color:#dc2626">-2322.44%</span> |
| 258 | [00013 INSERT_SELECT](crates/bench/sqlite_parity/cases/SQLITE_PARITY_013_INSERT_SELECT.rs) | P0 | memory | SQL_INSERT | 1872436 | 45306548 | <span style="color:#dc2626">-2319.66%</span> |
| 259 | [00466 DML_WHERE_ORDER_LIMIT_079](crates/bench/sqlite_parity/cases/SQLITE_PARITY_466_DML_WHERE_ORDER_LIMIT_079.rs) | P1 | memory | GEN_SQL_DML | 2909691 | 70320171 | <span style="color:#dc2626">-2316.76%</span> |
| 260 | [00397 DML_WHERE_ORDER_LIMIT_010](crates/bench/sqlite_parity/cases/SQLITE_PARITY_397_DML_WHERE_ORDER_LIMIT_010.rs) | P1 | memory | GEN_SQL_DML | 2488612 | 59833750 | <span style="color:#dc2626">-2304.30%</span> |
| 261 | [00433 DML_WHERE_ORDER_LIMIT_046](crates/bench/sqlite_parity/cases/SQLITE_PARITY_433_DML_WHERE_ORDER_LIMIT_046.rs) | P1 | memory | GEN_SQL_DML | 2759175 | 66204325 | <span style="color:#dc2626">-2299.42%</span> |
| 262 | [00680 JOIN_SUBQUERY_EXISTS_073](crates/bench/sqlite_parity/cases/SQLITE_PARITY_680_JOIN_SUBQUERY_EXISTS_073.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2977258 | 71427099 | <span style="color:#dc2626">-2299.09%</span> |
| 263 | [00411 DML_WHERE_ORDER_LIMIT_024](crates/bench/sqlite_parity/cases/SQLITE_PARITY_411_DML_WHERE_ORDER_LIMIT_024.rs) | P1 | memory | GEN_SQL_DML | 2327167 | 55822674 | <span style="color:#dc2626">-2298.74%</span> |
| 264 | [00487 DML_WHERE_ORDER_LIMIT_100](crates/bench/sqlite_parity/cases/SQLITE_PARITY_487_DML_WHERE_ORDER_LIMIT_100.rs) | P1 | memory | GEN_SQL_DML | 2602499 | 62406514 | <span style="color:#dc2626">-2297.95%</span> |
| 265 | [00428 DML_WHERE_ORDER_LIMIT_041](crates/bench/sqlite_parity/cases/SQLITE_PARITY_428_DML_WHERE_ORDER_LIMIT_041.rs) | P1 | memory | GEN_SQL_DML | 3345485 | 80200630 | <span style="color:#dc2626">-2297.28%</span> |
| 266 | [00681 JOIN_SUBQUERY_EXISTS_074](crates/bench/sqlite_parity/cases/SQLITE_PARITY_681_JOIN_SUBQUERY_EXISTS_074.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3392214 | 81131138 | <span style="color:#dc2626">-2291.69%</span> |
| 267 | [00689 JOIN_SUBQUERY_EXISTS_082](crates/bench/sqlite_parity/cases/SQLITE_PARITY_689_JOIN_SUBQUERY_EXISTS_082.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2952832 | 70621815 | <span style="color:#dc2626">-2291.66%</span> |
| 268 | [00080 NOT_NULL_FAILURE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_080_NOT_NULL_FAILURE.rs) | P0 | memory | SQL_CONSTRAINTS_NEGATIVE | 2694232 | 64373366 | <span style="color:#dc2626">-2289.30%</span> |
| 269 | [00661 JOIN_SUBQUERY_EXISTS_054](crates/bench/sqlite_parity/cases/SQLITE_PARITY_661_JOIN_SUBQUERY_EXISTS_054.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2964364 | 70771658 | <span style="color:#dc2626">-2287.41%</span> |
| 270 | [00823 WINDOW_PARTITION_SUM_036](crates/bench/sqlite_parity/cases/SQLITE_PARITY_823_WINDOW_PARTITION_SUM_036.rs) | P2 | memory | GEN_SQL_WINDOW | 1946085 | 46431283 | <span style="color:#dc2626">-2285.88%</span> |
| 271 | [00655 JOIN_SUBQUERY_EXISTS_048](crates/bench/sqlite_parity/cases/SQLITE_PARITY_655_JOIN_SUBQUERY_EXISTS_048.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2888330 | 68874215 | <span style="color:#dc2626">-2284.57%</span> |
| 272 | [00706 JOIN_SUBQUERY_EXISTS_099](crates/bench/sqlite_parity/cases/SQLITE_PARITY_706_JOIN_SUBQUERY_EXISTS_099.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2994711 | 71292466 | <span style="color:#dc2626">-2280.61%</span> |
| 273 | [00621 JOIN_SUBQUERY_EXISTS_014](crates/bench/sqlite_parity/cases/SQLITE_PARITY_621_JOIN_SUBQUERY_EXISTS_014.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3042462 | 72322805 | <span style="color:#dc2626">-2277.11%</span> |
| 274 | [00506 DML_WHERE_ORDER_LIMIT_119](crates/bench/sqlite_parity/cases/SQLITE_PARITY_506_DML_WHERE_ORDER_LIMIT_119.rs) | P1 | memory | GEN_SQL_DML | 3103548 | 73662591 | <span style="color:#dc2626">-2273.50%</span> |
| 275 | [00679 JOIN_SUBQUERY_EXISTS_072](crates/bench/sqlite_parity/cases/SQLITE_PARITY_679_JOIN_SUBQUERY_EXISTS_072.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3337110 | 78502609 | <span style="color:#dc2626">-2252.41%</span> |
| 276 | [00483 DML_WHERE_ORDER_LIMIT_096](crates/bench/sqlite_parity/cases/SQLITE_PARITY_483_DML_WHERE_ORDER_LIMIT_096.rs) | P1 | memory | GEN_SQL_DML | 2959124 | 69604425 | <span style="color:#dc2626">-2252.20%</span> |
| 277 | [00424 DML_WHERE_ORDER_LIMIT_037](crates/bench/sqlite_parity/cases/SQLITE_PARITY_424_DML_WHERE_ORDER_LIMIT_037.rs) | P1 | memory | GEN_SQL_DML | 3252529 | 76359777 | <span style="color:#dc2626">-2247.70%</span> |
| 278 | [00384 SCALAR_ARITH_040](crates/bench/sqlite_parity/cases/SQLITE_PARITY_384_SCALAR_ARITH_040.rs) | P1 | memory | GEN_SQL_SCALAR | 1864792 | 43581472 | <span style="color:#dc2626">-2237.07%</span> |
| 279 | [00663 JOIN_SUBQUERY_EXISTS_056](crates/bench/sqlite_parity/cases/SQLITE_PARITY_663_JOIN_SUBQUERY_EXISTS_056.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2878491 | 67069517 | <span style="color:#dc2626">-2230.02%</span> |
| 280 | [00436 DML_WHERE_ORDER_LIMIT_049](crates/bench/sqlite_parity/cases/SQLITE_PARITY_436_DML_WHERE_ORDER_LIMIT_049.rs) | P1 | memory | GEN_SQL_DML | 2606907 | 60663092 | <span style="color:#dc2626">-2227.01%</span> |
| 281 | [00845 WINDOW_PARTITION_SUM_058](crates/bench/sqlite_parity/cases/SQLITE_PARITY_845_WINDOW_PARTITION_SUM_058.rs) | P2 | memory | GEN_SQL_WINDOW | 1984949 | 46177362 | <span style="color:#dc2626">-2226.38%</span> |
| 282 | [00657 JOIN_SUBQUERY_EXISTS_050](crates/bench/sqlite_parity/cases/SQLITE_PARITY_657_JOIN_SUBQUERY_EXISTS_050.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2943926 | 68116159 | <span style="color:#dc2626">-2213.79%</span> |
| 283 | [00389 DML_WHERE_ORDER_LIMIT_002](crates/bench/sqlite_parity/cases/SQLITE_PARITY_389_DML_WHERE_ORDER_LIMIT_002.rs) | P1 | memory | GEN_SQL_DML | 2777930 | 64155354 | <span style="color:#dc2626">-2209.47%</span> |
| 284 | [00495 DML_WHERE_ORDER_LIMIT_108](crates/bench/sqlite_parity/cases/SQLITE_PARITY_495_DML_WHERE_ORDER_LIMIT_108.rs) | P1 | memory | GEN_SQL_DML | 2668414 | 61587934 | <span style="color:#dc2626">-2208.04%</span> |
| 285 | [00432 DML_WHERE_ORDER_LIMIT_045](crates/bench/sqlite_parity/cases/SQLITE_PARITY_432_DML_WHERE_ORDER_LIMIT_045.rs) | P1 | memory | GEN_SQL_DML | 3055697 | 70231131 | <span style="color:#dc2626">-2198.37%</span> |
| 286 | [00656 JOIN_SUBQUERY_EXISTS_049](crates/bench/sqlite_parity/cases/SQLITE_PARITY_656_JOIN_SUBQUERY_EXISTS_049.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2966187 | 68129985 | <span style="color:#dc2626">-2196.89%</span> |
| 287 | [00659 JOIN_SUBQUERY_EXISTS_052](crates/bench/sqlite_parity/cases/SQLITE_PARITY_659_JOIN_SUBQUERY_EXISTS_052.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2941030 | 67484433 | <span style="color:#dc2626">-2194.58%</span> |
| 288 | [00507 DML_WHERE_ORDER_LIMIT_120](crates/bench/sqlite_parity/cases/SQLITE_PARITY_507_DML_WHERE_ORDER_LIMIT_120.rs) | P1 | memory | GEN_SQL_DML | 2676489 | 61326590 | <span style="color:#dc2626">-2191.31%</span> |
| 289 | [00807 WINDOW_PARTITION_SUM_020](crates/bench/sqlite_parity/cases/SQLITE_PARITY_807_WINDOW_PARTITION_SUM_020.rs) | P2 | memory | GEN_SQL_WINDOW | 2854516 | 65277905 | <span style="color:#dc2626">-2186.83%</span> |
| 290 | [00678 JOIN_SUBQUERY_EXISTS_071](crates/bench/sqlite_parity/cases/SQLITE_PARITY_678_JOIN_SUBQUERY_EXISTS_071.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3210109 | 73408993 | <span style="color:#dc2626">-2186.81%</span> |
| 291 | [00440 DML_WHERE_ORDER_LIMIT_053](crates/bench/sqlite_parity/cases/SQLITE_PARITY_440_DML_WHERE_ORDER_LIMIT_053.rs) | P1 | memory | GEN_SQL_DML | 2443728 | 55852561 | <span style="color:#dc2626">-2185.55%</span> |
| 292 | [00854 WINDOW_PARTITION_SUM_067](crates/bench/sqlite_parity/cases/SQLITE_PARITY_854_WINDOW_PARTITION_SUM_067.rs) | P2 | memory | GEN_SQL_WINDOW | 2031327 | 46419912 | <span style="color:#dc2626">-2185.20%</span> |
| 293 | [00431 DML_WHERE_ORDER_LIMIT_044](crates/bench/sqlite_parity/cases/SQLITE_PARITY_431_DML_WHERE_ORDER_LIMIT_044.rs) | P1 | memory | GEN_SQL_DML | 2846471 | 65046853 | <span style="color:#dc2626">-2185.18%</span> |
| 294 | [00797 WINDOW_PARTITION_SUM_010](crates/bench/sqlite_parity/cases/SQLITE_PARITY_797_WINDOW_PARTITION_SUM_010.rs) | P2 | memory | GEN_SQL_WINDOW | 2047047 | 46775695 | <span style="color:#dc2626">-2185.03%</span> |
| 295 | [00481 DML_WHERE_ORDER_LIMIT_094](crates/bench/sqlite_parity/cases/SQLITE_PARITY_481_DML_WHERE_ORDER_LIMIT_094.rs) | P1 | memory | GEN_SQL_DML | 2641463 | 60262003 | <span style="color:#dc2626">-2181.39%</span> |
| 296 | [00426 DML_WHERE_ORDER_LIMIT_039](crates/bench/sqlite_parity/cases/SQLITE_PARITY_426_DML_WHERE_ORDER_LIMIT_039.rs) | P1 | memory | GEN_SQL_DML | 2795013 | 63753895 | <span style="color:#dc2626">-2180.99%</span> |
| 297 | [00609 JOIN_SUBQUERY_EXISTS_002](crates/bench/sqlite_parity/cases/SQLITE_PARITY_609_JOIN_SUBQUERY_EXISTS_002.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3166988 | 71931774 | <span style="color:#dc2626">-2171.30%</span> |
| 298 | [00429 DML_WHERE_ORDER_LIMIT_042](crates/bench/sqlite_parity/cases/SQLITE_PARITY_429_DML_WHERE_ORDER_LIMIT_042.rs) | P1 | memory | GEN_SQL_DML | 2868413 | 65019361 | <span style="color:#dc2626">-2166.74%</span> |
| 299 | [00690 JOIN_SUBQUERY_EXISTS_083](crates/bench/sqlite_parity/cases/SQLITE_PARITY_690_JOIN_SUBQUERY_EXISTS_083.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3074623 | 69634906 | <span style="color:#dc2626">-2164.83%</span> |
| 300 | [00826 WINDOW_PARTITION_SUM_039](crates/bench/sqlite_parity/cases/SQLITE_PARITY_826_WINDOW_PARTITION_SUM_039.rs) | P2 | memory | GEN_SQL_WINDOW | 2257506 | 50922178 | <span style="color:#dc2626">-2155.68%</span> |
| 301 | [00674 JOIN_SUBQUERY_EXISTS_067](crates/bench/sqlite_parity/cases/SQLITE_PARITY_674_JOIN_SUBQUERY_EXISTS_067.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3233053 | 72847199 | <span style="color:#dc2626">-2153.20%</span> |
| 302 | [00423 DML_WHERE_ORDER_LIMIT_036](crates/bench/sqlite_parity/cases/SQLITE_PARITY_423_DML_WHERE_ORDER_LIMIT_036.rs) | P1 | memory | GEN_SQL_DML | 2673413 | 60137926 | <span style="color:#dc2626">-2149.48%</span> |
| 303 | [00791 WINDOW_PARTITION_SUM_004](crates/bench/sqlite_parity/cases/SQLITE_PARITY_791_WINDOW_PARTITION_SUM_004.rs) | P2 | memory | GEN_SQL_WINDOW | 2038200 | 45845373 | <span style="color:#dc2626">-2149.31%</span> |
| 304 | [00501 DML_WHERE_ORDER_LIMIT_114](crates/bench/sqlite_parity/cases/SQLITE_PARITY_501_DML_WHERE_ORDER_LIMIT_114.rs) | P1 | memory | GEN_SQL_DML | 2971256 | 66537638 | <span style="color:#dc2626">-2139.38%</span> |
| 305 | [00852 WINDOW_PARTITION_SUM_065](crates/bench/sqlite_parity/cases/SQLITE_PARITY_852_WINDOW_PARTITION_SUM_065.rs) | P2 | memory | GEN_SQL_WINDOW | 2307781 | 51188052 | <span style="color:#dc2626">-2118.06%</span> |
| 306 | [00409 DML_WHERE_ORDER_LIMIT_022](crates/bench/sqlite_parity/cases/SQLITE_PARITY_409_DML_WHERE_ORDER_LIMIT_022.rs) | P1 | memory | GEN_SQL_DML | 2728557 | 60501124 | <span style="color:#dc2626">-2117.33%</span> |
| 307 | [00471 DML_WHERE_ORDER_LIMIT_084](crates/bench/sqlite_parity/cases/SQLITE_PARITY_471_DML_WHERE_ORDER_LIMIT_084.rs) | P1 | memory | GEN_SQL_DML | 2683852 | 59416943 | <span style="color:#dc2626">-2113.87%</span> |
| 308 | [00848 WINDOW_PARTITION_SUM_061](crates/bench/sqlite_parity/cases/SQLITE_PARITY_848_WINDOW_PARTITION_SUM_061.rs) | P2 | memory | GEN_SQL_WINDOW | 1968788 | 43535750 | <span style="color:#dc2626">-2111.30%</span> |
| 309 | [00459 DML_WHERE_ORDER_LIMIT_072](crates/bench/sqlite_parity/cases/SQLITE_PARITY_459_DML_WHERE_ORDER_LIMIT_072.rs) | P1 | memory | GEN_SQL_DML | 2888200 | 63859656 | <span style="color:#dc2626">-2111.05%</span> |
| 310 | [00460 DML_WHERE_ORDER_LIMIT_073](crates/bench/sqlite_parity/cases/SQLITE_PARITY_460_DML_WHERE_ORDER_LIMIT_073.rs) | P1 | memory | GEN_SQL_DML | 2976266 | 65781014 | <span style="color:#dc2626">-2110.19%</span> |
| 311 | [00434 DML_WHERE_ORDER_LIMIT_047](crates/bench/sqlite_parity/cases/SQLITE_PARITY_434_DML_WHERE_ORDER_LIMIT_047.rs) | P1 | memory | GEN_SQL_DML | 2989240 | 65973618 | <span style="color:#dc2626">-2107.04%</span> |
| 312 | [00427 DML_WHERE_ORDER_LIMIT_040](crates/bench/sqlite_parity/cases/SQLITE_PARITY_427_DML_WHERE_ORDER_LIMIT_040.rs) | P1 | memory | GEN_SQL_DML | 2762962 | 60922212 | <span style="color:#dc2626">-2104.96%</span> |
| 313 | [00067 CASE_COALESCE_NULLIF_IIF](crates/bench/sqlite_parity/cases/SQLITE_PARITY_067_CASE_COALESCE_NULLIF_IIF.rs) | P0 | memory | SQL_EXPRESSIONS | 1751817 | 38599146 | <span style="color:#dc2626">-2103.38%</span> |
| 314 | [00075 EXPLAIN_QUERY_PLAN](crates/bench/sqlite_parity/cases/SQLITE_PARITY_075_EXPLAIN_QUERY_PLAN.rs) | P0 | memory | SQL_EXPLAIN | 2923035 | 64237298 | <span style="color:#dc2626">-2097.62%</span> |
| 315 | [00662 JOIN_SUBQUERY_EXISTS_055](crates/bench/sqlite_parity/cases/SQLITE_PARITY_662_JOIN_SUBQUERY_EXISTS_055.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3471545 | 76058510 | <span style="color:#dc2626">-2090.91%</span> |
| 316 | [00811 WINDOW_PARTITION_SUM_024](crates/bench/sqlite_parity/cases/SQLITE_PARITY_811_WINDOW_PARTITION_SUM_024.rs) | P2 | memory | GEN_SQL_WINDOW | 2366873 | 51758903 | <span style="color:#dc2626">-2086.81%</span> |
| 317 | [00414 DML_WHERE_ORDER_LIMIT_027](crates/bench/sqlite_parity/cases/SQLITE_PARITY_414_DML_WHERE_ORDER_LIMIT_027.rs) | P1 | memory | GEN_SQL_DML | 2572902 | 56223303 | <span style="color:#dc2626">-2085.21%</span> |
| 318 | [00108 DOT_MODE_CSV_AND_QUOTE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_108_DOT_MODE_CSV_AND_QUOTE.rs) | P0 | memory | CLI_DOT_COMMAND | 2318270 | 50446983 | <span style="color:#dc2626">-2076.06%</span> |
| 319 | [00430 DML_WHERE_ORDER_LIMIT_043](crates/bench/sqlite_parity/cases/SQLITE_PARITY_430_DML_WHERE_ORDER_LIMIT_043.rs) | P1 | memory | GEN_SQL_DML | 3052180 | 66411407 | <span style="color:#dc2626">-2075.87%</span> |
| 320 | [00504 DML_WHERE_ORDER_LIMIT_117](crates/bench/sqlite_parity/cases/SQLITE_PARITY_504_DML_WHERE_ORDER_LIMIT_117.rs) | P1 | memory | GEN_SQL_DML | 2775747 | 60340993 | <span style="color:#dc2626">-2073.87%</span> |
| 321 | [00703 JOIN_SUBQUERY_EXISTS_096](crates/bench/sqlite_parity/cases/SQLITE_PARITY_703_JOIN_SUBQUERY_EXISTS_096.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3280603 | 71299950 | <span style="color:#dc2626">-2073.38%</span> |
| 322 | [00277 SCALAR_CAST_TYPEOF_013](crates/bench/sqlite_parity/cases/SQLITE_PARITY_277_SCALAR_CAST_TYPEOF_013.rs) | P1 | memory | GEN_SQL_SCALAR | 1891872 | 40842364 | <span style="color:#dc2626">-2058.83%</span> |
| 323 | [00654 JOIN_SUBQUERY_EXISTS_047](crates/bench/sqlite_parity/cases/SQLITE_PARITY_654_JOIN_SUBQUERY_EXISTS_047.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3282637 | 70631803 | <span style="color:#dc2626">-2051.68%</span> |
| 324 | [00078 ON_CONFLICT_ALGORITHMS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_078_ON_CONFLICT_ALGORITHMS.rs) | P0 | memory | SQL_CONFLICT | 2690595 | 57886382 | <span style="color:#dc2626">-2051.43%</span> |
| 325 | [00707 JOIN_SUBQUERY_EXISTS_100](crates/bench/sqlite_parity/cases/SQLITE_PARITY_707_JOIN_SUBQUERY_EXISTS_100.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3264162 | 69972346 | <span style="color:#dc2626">-2043.65%</span> |
| 326 | [00115 DOT_SCHEMA_TABLES_INDEXES](crates/bench/sqlite_parity/cases/SQLITE_PARITY_115_DOT_SCHEMA_TABLES_INDEXES.rs) | P0 | memory | CLI_DOT_COMMAND | 2961849 | 63417546 | <span style="color:#dc2626">-2041.15%</span> |
| 327 | [00246 SCALAR_STRING_005](crates/bench/sqlite_parity/cases/SQLITE_PARITY_246_SCALAR_STRING_005.rs) | P1 | memory | GEN_SQL_SCALAR | 1948259 | 41678829 | <span style="color:#dc2626">-2039.29%</span> |
| 328 | [00418 DML_WHERE_ORDER_LIMIT_031](crates/bench/sqlite_parity/cases/SQLITE_PARITY_418_DML_WHERE_ORDER_LIMIT_031.rs) | P1 | memory | GEN_SQL_DML | 2754736 | 58907536 | <span style="color:#dc2626">-2038.41%</span> |
| 329 | [00837 WINDOW_PARTITION_SUM_050](crates/bench/sqlite_parity/cases/SQLITE_PARITY_837_WINDOW_PARTITION_SUM_050.rs) | P2 | memory | GEN_SQL_WINDOW | 2344710 | 50106503 | <span style="color:#dc2626">-2037.00%</span> |
| 330 | [00838 WINDOW_PARTITION_SUM_051](crates/bench/sqlite_parity/cases/SQLITE_PARITY_838_WINDOW_PARTITION_SUM_051.rs) | P2 | memory | GEN_SQL_WINDOW | 3160165 | 67389866 | <span style="color:#dc2626">-2032.48%</span> |
| 331 | [00439 DML_WHERE_ORDER_LIMIT_052](crates/bench/sqlite_parity/cases/SQLITE_PARITY_439_DML_WHERE_ORDER_LIMIT_052.rs) | P1 | memory | GEN_SQL_DML | 3059804 | 65135040 | <span style="color:#dc2626">-2028.73%</span> |
| 332 | [00446 DML_WHERE_ORDER_LIMIT_059](crates/bench/sqlite_parity/cases/SQLITE_PARITY_446_DML_WHERE_ORDER_LIMIT_059.rs) | P1 | memory | GEN_SQL_DML | 2631564 | 55976165 | <span style="color:#dc2626">-2027.11%</span> |
| 333 | [00404 DML_WHERE_ORDER_LIMIT_017](crates/bench/sqlite_parity/cases/SQLITE_PARITY_404_DML_WHERE_ORDER_LIMIT_017.rs) | P1 | memory | GEN_SQL_DML | 3310970 | 70277798 | <span style="color:#dc2626">-2022.57%</span> |
| 334 | [00653 JOIN_SUBQUERY_EXISTS_046](crates/bench/sqlite_parity/cases/SQLITE_PARITY_653_JOIN_SUBQUERY_EXISTS_046.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3251928 | 68821615 | <span style="color:#dc2626">-2016.33%</span> |
| 335 | [00789 WINDOW_PARTITION_SUM_002](crates/bench/sqlite_parity/cases/SQLITE_PARITY_789_WINDOW_PARTITION_SUM_002.rs) | P2 | memory | GEN_SQL_WINDOW | 2976818 | 62902836 | <span style="color:#dc2626">-2013.09%</span> |
| 336 | [00497 DML_WHERE_ORDER_LIMIT_110](crates/bench/sqlite_parity/cases/SQLITE_PARITY_497_DML_WHERE_ORDER_LIMIT_110.rs) | P1 | memory | GEN_SQL_DML | 2832464 | 59494450 | <span style="color:#dc2626">-2000.45%</span> |
| 337 | [00438 DML_WHERE_ORDER_LIMIT_051](crates/bench/sqlite_parity/cases/SQLITE_PARITY_438_DML_WHERE_ORDER_LIMIT_051.rs) | P1 | memory | GEN_SQL_DML | 2742875 | 57491355 | <span style="color:#dc2626">-1996.03%</span> |
| 338 | [00031 CREATE_INDEX](crates/bench/sqlite_parity/cases/SQLITE_PARITY_031_CREATE_INDEX.rs) | P0 | memory | SQL_INDEX | 2686417 | 56297933 | <span style="color:#dc2626">-1995.65%</span> |
| 339 | [00454 DML_WHERE_ORDER_LIMIT_067](crates/bench/sqlite_parity/cases/SQLITE_PARITY_454_DML_WHERE_ORDER_LIMIT_067.rs) | P1 | memory | GEN_SQL_DML | 2748205 | 57357833 | <span style="color:#dc2626">-1987.10%</span> |
| 340 | [00830 WINDOW_PARTITION_SUM_043](crates/bench/sqlite_parity/cases/SQLITE_PARITY_830_WINDOW_PARTITION_SUM_043.rs) | P2 | memory | GEN_SQL_WINDOW | 2462423 | 51362603 | <span style="color:#dc2626">-1985.86%</span> |
| 341 | [00012 INSERT_DEFAULT_VALUES](crates/bench/sqlite_parity/cases/SQLITE_PARITY_012_INSERT_DEFAULT_VALUES.rs) | P0 | memory | SQL_INSERT | 2201469 | 45522707 | <span style="color:#dc2626">-1967.83%</span> |
| 342 | [00081 BLOBS_HEX_ZEROBLOB](crates/bench/sqlite_parity/cases/SQLITE_PARITY_081_BLOBS_HEX_ZEROBLOB.rs) | P0 | memory | SQL_FUNCTIONS | 1707533 | 35236498 | <span style="color:#dc2626">-1963.59%</span> |
| 343 | [00402 DML_WHERE_ORDER_LIMIT_015](crates/bench/sqlite_parity/cases/SQLITE_PARITY_402_DML_WHERE_ORDER_LIMIT_015.rs) | P1 | memory | GEN_SQL_DML | 2824499 | 58275680 | <span style="color:#dc2626">-1963.22%</span> |
| 344 | [00420 DML_WHERE_ORDER_LIMIT_033](crates/bench/sqlite_parity/cases/SQLITE_PARITY_420_DML_WHERE_ORDER_LIMIT_033.rs) | P1 | memory | GEN_SQL_DML | 3160987 | 65153705 | <span style="color:#dc2626">-1961.18%</span> |
| 345 | [00702 JOIN_SUBQUERY_EXISTS_095](crates/bench/sqlite_parity/cases/SQLITE_PARITY_702_JOIN_SUBQUERY_EXISTS_095.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3436267 | 70752934 | <span style="color:#dc2626">-1959.01%</span> |
| 346 | [00677 JOIN_SUBQUERY_EXISTS_070](crates/bench/sqlite_parity/cases/SQLITE_PARITY_677_JOIN_SUBQUERY_EXISTS_070.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3583887 | 73560079 | <span style="color:#dc2626">-1952.52%</span> |
| 347 | [00833 WINDOW_PARTITION_SUM_046](crates/bench/sqlite_parity/cases/SQLITE_PARITY_833_WINDOW_PARTITION_SUM_046.rs) | P2 | memory | GEN_SQL_WINDOW | 2512418 | 51490584 | <span style="color:#dc2626">-1949.44%</span> |
| 348 | [00184 OPT_ASCII_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_184_OPT_ASCII_MODE.rs) | P2 | memory | CLI_OPTION | 1699188 | 34644487 | <span style="color:#dc2626">-1938.88%</span> |
| 349 | [00290 SCALAR_STRING_016](crates/bench/sqlite_parity/cases/SQLITE_PARITY_290_SCALAR_STRING_016.rs) | P1 | memory | GEN_SQL_SCALAR | 1832461 | 37261123 | <span style="color:#dc2626">-1933.39%</span> |
| 350 | [00486 DML_WHERE_ORDER_LIMIT_099](crates/bench/sqlite_parity/cases/SQLITE_PARITY_486_DML_WHERE_ORDER_LIMIT_099.rs) | P1 | memory | GEN_SQL_DML | 2972460 | 60429029 | <span style="color:#dc2626">-1932.96%</span> |
| 351 | [00391 DML_WHERE_ORDER_LIMIT_004](crates/bench/sqlite_parity/cases/SQLITE_PARITY_391_DML_WHERE_ORDER_LIMIT_004.rs) | P1 | memory | GEN_SQL_DML | 3406521 | 69011630 | <span style="color:#dc2626">-1925.87%</span> |
| 352 | [00835 WINDOW_PARTITION_SUM_048](crates/bench/sqlite_parity/cases/SQLITE_PARITY_835_WINDOW_PARTITION_SUM_048.rs) | P2 | memory | GEN_SQL_WINDOW | 2523499 | 50790769 | <span style="color:#dc2626">-1912.71%</span> |
| 353 | [00850 WINDOW_PARTITION_SUM_063](crates/bench/sqlite_parity/cases/SQLITE_PARITY_850_WINDOW_PARTITION_SUM_063.rs) | P2 | memory | GEN_SQL_WINDOW | 2355061 | 47095271 | <span style="color:#dc2626">-1899.75%</span> |
| 354 | [00360 SCALAR_ARITH_034](crates/bench/sqlite_parity/cases/SQLITE_PARITY_360_SCALAR_ARITH_034.rs) | P1 | memory | GEN_SQL_SCALAR | 2804882 | 56029706 | <span style="color:#dc2626">-1897.58%</span> |
| 355 | [00413 DML_WHERE_ORDER_LIMIT_026](crates/bench/sqlite_parity/cases/SQLITE_PARITY_413_DML_WHERE_ORDER_LIMIT_026.rs) | P1 | memory | GEN_SQL_DML | 2973411 | 59185362 | <span style="color:#dc2626">-1890.49%</span> |
| 356 | [00664 JOIN_SUBQUERY_EXISTS_057](crates/bench/sqlite_parity/cases/SQLITE_PARITY_664_JOIN_SUBQUERY_EXISTS_057.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3483908 | 69117686 | <span style="color:#dc2626">-1883.91%</span> |
| 357 | [00421 DML_WHERE_ORDER_LIMIT_034](crates/bench/sqlite_parity/cases/SQLITE_PARITY_421_DML_WHERE_ORDER_LIMIT_034.rs) | P1 | memory | GEN_SQL_DML | 3046399 | 60192190 | <span style="color:#dc2626">-1875.85%</span> |
| 358 | [00058 GROUP_BY_HAVING](crates/bench/sqlite_parity/cases/SQLITE_PARITY_058_GROUP_BY_HAVING.rs) | P0 | memory | SQL_AGGREGATE | 2574666 | 50498440 | <span style="color:#dc2626">-1861.36%</span> |
| 359 | [00842 WINDOW_PARTITION_SUM_055](crates/bench/sqlite_parity/cases/SQLITE_PARITY_842_WINDOW_PARTITION_SUM_055.rs) | P2 | memory | GEN_SQL_WINDOW | 2337026 | 45578719 | <span style="color:#dc2626">-1850.29%</span> |
| 360 | [00019 REPLACE_INTO](crates/bench/sqlite_parity/cases/SQLITE_PARITY_019_REPLACE_INTO.rs) | P0 | memory | SQL_REPLACE | 2737464 | 53340362 | <span style="color:#dc2626">-1848.53%</span> |
| 361 | [00701 JOIN_SUBQUERY_EXISTS_094](crates/bench/sqlite_parity/cases/SQLITE_PARITY_701_JOIN_SUBQUERY_EXISTS_094.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3493556 | 67988659 | <span style="color:#dc2626">-1846.12%</span> |
| 362 | [00809 WINDOW_PARTITION_SUM_022](crates/bench/sqlite_parity/cases/SQLITE_PARITY_809_WINDOW_PARTITION_SUM_022.rs) | P2 | memory | GEN_SQL_WINDOW | 2546392 | 49541964 | <span style="color:#dc2626">-1845.57%</span> |
| 363 | [00003 CREATE_TABLE_INSERT_SELECT](crates/bench/sqlite_parity/cases/SQLITE_PARITY_003_CREATE_TABLE_INSERT_SELECT.rs) | P0 | memory | SQL_DDL_DML | 2597429 | 50376268 | <span style="color:#dc2626">-1839.47%</span> |
| 364 | [00484 DML_WHERE_ORDER_LIMIT_097](crates/bench/sqlite_parity/cases/SQLITE_PARITY_484_DML_WHERE_ORDER_LIMIT_097.rs) | P1 | memory | GEN_SQL_DML | 3235287 | 62509670 | <span style="color:#dc2626">-1832.12%</span> |
| 365 | [00817 WINDOW_PARTITION_SUM_030](crates/bench/sqlite_parity/cases/SQLITE_PARITY_817_WINDOW_PARTITION_SUM_030.rs) | P2 | memory | GEN_SQL_WINDOW | 2366011 | 45582956 | <span style="color:#dc2626">-1826.57%</span> |
| 366 | [00415 DML_WHERE_ORDER_LIMIT_028](crates/bench/sqlite_parity/cases/SQLITE_PARITY_415_DML_WHERE_ORDER_LIMIT_028.rs) | P1 | memory | GEN_SQL_DML | 3208596 | 61773895 | <span style="color:#dc2626">-1825.26%</span> |
| 367 | [00175 OPT_QUOTE_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_175_OPT_QUOTE_MODE.rs) | P1 | memory | CLI_OPTION | 1856686 | 35712660 | <span style="color:#dc2626">-1823.46%</span> |
| 368 | [00818 WINDOW_PARTITION_SUM_031](crates/bench/sqlite_parity/cases/SQLITE_PARITY_818_WINDOW_PARTITION_SUM_031.rs) | P2 | memory | GEN_SQL_WINDOW | 2402771 | 46191589 | <span style="color:#dc2626">-1822.43%</span> |
| 369 | [00437 DML_WHERE_ORDER_LIMIT_050](crates/bench/sqlite_parity/cases/SQLITE_PARITY_437_DML_WHERE_ORDER_LIMIT_050.rs) | P1 | memory | GEN_SQL_DML | 3111181 | 59292236 | <span style="color:#dc2626">-1805.78%</span> |
| 370 | [00416 DML_WHERE_ORDER_LIMIT_029](crates/bench/sqlite_parity/cases/SQLITE_PARITY_416_DML_WHERE_ORDER_LIMIT_029.rs) | P1 | memory | GEN_SQL_DML | 3081015 | 58713670 | <span style="color:#dc2626">-1805.66%</span> |
| 371 | [00832 WINDOW_PARTITION_SUM_045](crates/bench/sqlite_parity/cases/SQLITE_PARITY_832_WINDOW_PARTITION_SUM_045.rs) | P2 | memory | GEN_SQL_WINDOW | 2706476 | 51507797 | <span style="color:#dc2626">-1803.13%</span> |
| 372 | [00831 WINDOW_PARTITION_SUM_044](crates/bench/sqlite_parity/cases/SQLITE_PARITY_831_WINDOW_PARTITION_SUM_044.rs) | P2 | memory | GEN_SQL_WINDOW | 2731814 | 51976465 | <span style="color:#dc2626">-1802.64%</span> |
| 373 | [00828 WINDOW_PARTITION_SUM_041](crates/bench/sqlite_parity/cases/SQLITE_PARITY_828_WINDOW_PARTITION_SUM_041.rs) | P2 | memory | GEN_SQL_WINDOW | 2587601 | 49204566 | <span style="color:#dc2626">-1801.55%</span> |
| 374 | [00419 DML_WHERE_ORDER_LIMIT_032](crates/bench/sqlite_parity/cases/SQLITE_PARITY_419_DML_WHERE_ORDER_LIMIT_032.rs) | P1 | memory | GEN_SQL_DML | 3049024 | 57885611 | <span style="color:#dc2626">-1798.50%</span> |
| 375 | [00447 DML_WHERE_ORDER_LIMIT_060](crates/bench/sqlite_parity/cases/SQLITE_PARITY_447_DML_WHERE_ORDER_LIMIT_060.rs) | P1 | memory | GEN_SQL_DML | 3091064 | 58667532 | <span style="color:#dc2626">-1797.97%</span> |
| 376 | [00490 DML_WHERE_ORDER_LIMIT_103](crates/bench/sqlite_parity/cases/SQLITE_PARITY_490_DML_WHERE_ORDER_LIMIT_103.rs) | P1 | memory | GEN_SQL_DML | 3135548 | 59505931 | <span style="color:#dc2626">-1797.78%</span> |
| 377 | [00441 DML_WHERE_ORDER_LIMIT_054](crates/bench/sqlite_parity/cases/SQLITE_PARITY_441_DML_WHERE_ORDER_LIMIT_054.rs) | P1 | memory | GEN_SQL_DML | 3044816 | 57717914 | <span style="color:#dc2626">-1795.61%</span> |
| 378 | [00478 DML_WHERE_ORDER_LIMIT_091](crates/bench/sqlite_parity/cases/SQLITE_PARITY_478_DML_WHERE_ORDER_LIMIT_091.rs) | P1 | memory | GEN_SQL_DML | 3104900 | 58784645 | <span style="color:#dc2626">-1793.29%</span> |
| 379 | [00350 SCALAR_STRING_031](crates/bench/sqlite_parity/cases/SQLITE_PARITY_350_SCALAR_STRING_031.rs) | P1 | memory | GEN_SQL_SCALAR | 1759823 | 33181688 | <span style="color:#dc2626">-1785.51%</span> |
| 380 | [00422 DML_WHERE_ORDER_LIMIT_035](crates/bench/sqlite_parity/cases/SQLITE_PARITY_422_DML_WHERE_ORDER_LIMIT_035.rs) | P1 | memory | GEN_SQL_DML | 3214257 | 60457192 | <span style="color:#dc2626">-1780.91%</span> |
| 381 | [00189 OPT_ECHO](crates/bench/sqlite_parity/cases/SQLITE_PARITY_189_OPT_ECHO.rs) | P2 | memory | CLI_OPTION | 1941667 | 36460507 | <span style="color:#dc2626">-1777.79%</span> |
| 382 | [00068 CAST_AND_TYPE_AFFINITY](crates/bench/sqlite_parity/cases/SQLITE_PARITY_068_CAST_AND_TYPE_AFFINITY.rs) | P0 | memory | SQL_EXPRESSIONS | 2636493 | 49360014 | <span style="color:#dc2626">-1772.18%</span> |
| 383 | [00442 DML_WHERE_ORDER_LIMIT_055](crates/bench/sqlite_parity/cases/SQLITE_PARITY_442_DML_WHERE_ORDER_LIMIT_055.rs) | P1 | memory | GEN_SQL_DML | 3021752 | 56562806 | <span style="color:#dc2626">-1771.85%</span> |
| 384 | [00021 UPSERT_DO_NOTHING](crates/bench/sqlite_parity/cases/SQLITE_PARITY_021_UPSERT_DO_NOTHING.rs) | P0 | memory | SQL_UPSERT | 2773713 | 51652936 | <span style="color:#dc2626">-1762.23%</span> |
| 385 | [00819 WINDOW_PARTITION_SUM_032](crates/bench/sqlite_parity/cases/SQLITE_PARITY_819_WINDOW_PARTITION_SUM_032.rs) | P2 | memory | GEN_SQL_WINDOW | 2599002 | 48384602 | <span style="color:#dc2626">-1761.66%</span> |
| 386 | [00015 UPDATE_BASIC](crates/bench/sqlite_parity/cases/SQLITE_PARITY_015_UPDATE_BASIC.rs) | P0 | memory | SQL_UPDATE | 2748615 | 51146227 | <span style="color:#dc2626">-1760.80%</span> |
| 387 | [00282 SCALAR_STRING_014](crates/bench/sqlite_parity/cases/SQLITE_PARITY_282_SCALAR_STRING_014.rs) | P1 | memory | GEN_SQL_SCALAR | 1878557 | 34930549 | <span style="color:#dc2626">-1759.44%</span> |
| 388 | [00821 WINDOW_PARTITION_SUM_034](crates/bench/sqlite_parity/cases/SQLITE_PARITY_821_WINDOW_PARTITION_SUM_034.rs) | P2 | memory | GEN_SQL_WINDOW | 2466071 | 45809585 | <span style="color:#dc2626">-1757.59%</span> |
| 389 | [00381 SCALAR_CAST_TYPEOF_039](crates/bench/sqlite_parity/cases/SQLITE_PARITY_381_SCALAR_CAST_TYPEOF_039.rs) | P1 | memory | GEN_SQL_SCALAR | 1894608 | 35155877 | <span style="color:#dc2626">-1755.58%</span> |
| 390 | [00008 STRICT_TABLE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_008_STRICT_TABLE.rs) | P0 | memory | SQL_DDL | 2418260 | 44649653 | <span style="color:#dc2626">-1746.35%</span> |
| 391 | [00834 WINDOW_PARTITION_SUM_047](crates/bench/sqlite_parity/cases/SQLITE_PARITY_834_WINDOW_PARTITION_SUM_047.rs) | P2 | memory | GEN_SQL_WINDOW | 2711695 | 50057621 | <span style="color:#dc2626">-1745.99%</span> |
| 392 | [00470 DML_WHERE_ORDER_LIMIT_083](crates/bench/sqlite_parity/cases/SQLITE_PARITY_470_DML_WHERE_ORDER_LIMIT_083.rs) | P1 | memory | GEN_SQL_DML | 3198497 | 58697580 | <span style="color:#dc2626">-1735.16%</span> |
| 393 | [00457 DML_WHERE_ORDER_LIMIT_070](crates/bench/sqlite_parity/cases/SQLITE_PARITY_457_DML_WHERE_ORDER_LIMIT_070.rs) | P1 | memory | GEN_SQL_DML | 3270244 | 59924804 | <span style="color:#dc2626">-1732.43%</span> |
| 394 | [00824 WINDOW_PARTITION_SUM_037](crates/bench/sqlite_parity/cases/SQLITE_PARITY_824_WINDOW_PARTITION_SUM_037.rs) | P2 | memory | GEN_SQL_WINDOW | 2845629 | 51976605 | <span style="color:#dc2626">-1726.54%</span> |
| 395 | [00491 DML_WHERE_ORDER_LIMIT_104](crates/bench/sqlite_parity/cases/SQLITE_PARITY_491_DML_WHERE_ORDER_LIMIT_104.rs) | P1 | memory | GEN_SQL_DML | 3239405 | 58808911 | <span style="color:#dc2626">-1715.42%</span> |
| 396 | [00855 WINDOW_PARTITION_SUM_068](crates/bench/sqlite_parity/cases/SQLITE_PARITY_855_WINDOW_PARTITION_SUM_068.rs) | P2 | memory | GEN_SQL_WINDOW | 2510935 | 45582396 | <span style="color:#dc2626">-1715.36%</span> |
| 397 | [00825 WINDOW_PARTITION_SUM_038](crates/bench/sqlite_parity/cases/SQLITE_PARITY_825_WINDOW_PARTITION_SUM_038.rs) | P2 | memory | GEN_SQL_WINDOW | 2586127 | 46483091 | <span style="color:#dc2626">-1697.40%</span> |
| 398 | [00051 PRAGMA_INDEX_LIST_FUNCTION](crates/bench/sqlite_parity/cases/SQLITE_PARITY_051_PRAGMA_INDEX_LIST_FUNCTION.rs) | P0 | memory | SQL_PRAGMA | 3328032 | 59766652 | <span style="color:#dc2626">-1695.86%</span> |
| 399 | [00840 WINDOW_PARTITION_SUM_053](crates/bench/sqlite_parity/cases/SQLITE_PARITY_840_WINDOW_PARTITION_SUM_053.rs) | P2 | memory | GEN_SQL_WINDOW | 2607718 | 46527235 | <span style="color:#dc2626">-1684.21%</span> |
| 400 | [00864 WINDOW_PARTITION_SUM_077](crates/bench/sqlite_parity/cases/SQLITE_PARITY_864_WINDOW_PARTITION_SUM_077.rs) | P2 | memory | GEN_SQL_WINDOW | 2788671 | 49514352 | <span style="color:#dc2626">-1675.55%</span> |
| 401 | [00443 DML_WHERE_ORDER_LIMIT_056](crates/bench/sqlite_parity/cases/SQLITE_PARITY_443_DML_WHERE_ORDER_LIMIT_056.rs) | P1 | memory | GEN_SQL_DML | 3164954 | 55965726 | <span style="color:#dc2626">-1668.30%</span> |
| 402 | [00820 WINDOW_PARTITION_SUM_033](crates/bench/sqlite_parity/cases/SQLITE_PARITY_820_WINDOW_PARTITION_SUM_033.rs) | P2 | memory | GEN_SQL_WINDOW | 2612949 | 46079256 | <span style="color:#dc2626">-1663.50%</span> |
| 403 | [00480 DML_WHERE_ORDER_LIMIT_093](crates/bench/sqlite_parity/cases/SQLITE_PARITY_480_DML_WHERE_ORDER_LIMIT_093.rs) | P1 | memory | GEN_SQL_DML | 3253581 | 57369024 | <span style="color:#dc2626">-1663.26%</span> |
| 404 | [00839 WINDOW_PARTITION_SUM_052](crates/bench/sqlite_parity/cases/SQLITE_PARITY_839_WINDOW_PARTITION_SUM_052.rs) | P2 | memory | GEN_SQL_WINDOW | 2534941 | 44672412 | <span style="color:#dc2626">-1662.27%</span> |
| 405 | [00016 UPDATE_RETURNING](crates/bench/sqlite_parity/cases/SQLITE_PARITY_016_UPDATE_RETURNING.rs) | P0 | memory | SQL_UPDATE | 2905402 | 51131809 | <span style="color:#dc2626">-1659.89%</span> |
| 406 | [00851 WINDOW_PARTITION_SUM_064](crates/bench/sqlite_parity/cases/SQLITE_PARITY_851_WINDOW_PARTITION_SUM_064.rs) | P2 | memory | GEN_SQL_WINDOW | 2730791 | 47920643 | <span style="color:#dc2626">-1654.83%</span> |
| 407 | [00813 WINDOW_PARTITION_SUM_026](crates/bench/sqlite_parity/cases/SQLITE_PARITY_813_WINDOW_PARTITION_SUM_026.rs) | P2 | memory | GEN_SQL_WINDOW | 2820983 | 49461392 | <span style="color:#dc2626">-1653.34%</span> |
| 408 | [00476 DML_WHERE_ORDER_LIMIT_089](crates/bench/sqlite_parity/cases/SQLITE_PARITY_476_DML_WHERE_ORDER_LIMIT_089.rs) | P1 | memory | GEN_SQL_DML | 3377095 | 59190304 | <span style="color:#dc2626">-1652.70%</span> |
| 409 | [00793 WINDOW_PARTITION_SUM_006](crates/bench/sqlite_parity/cases/SQLITE_PARITY_793_WINDOW_PARTITION_SUM_006.rs) | P2 | memory | GEN_SQL_WINDOW | 2479526 | 43259737 | <span style="color:#dc2626">-1644.68%</span> |
| 410 | [00018 DELETE_RETURNING](crates/bench/sqlite_parity/cases/SQLITE_PARITY_018_DELETE_RETURNING.rs) | P0 | memory | SQL_DELETE | 2834437 | 49213336 | <span style="color:#dc2626">-1636.26%</span> |
| 411 | [00009 STRICT_TABLE_TYPE_FAILURE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_009_STRICT_TABLE_TYPE_FAILURE.rs) | P0 | memory | SQL_DDL_NEGATIVE | 2169789 | 37456482 | <span style="color:#dc2626">-1626.27%</span> |
| 412 | [00844 WINDOW_PARTITION_SUM_057](crates/bench/sqlite_parity/cases/SQLITE_PARITY_844_WINDOW_PARTITION_SUM_057.rs) | P2 | memory | GEN_SQL_WINDOW | 2649277 | 45672597 | <span style="color:#dc2626">-1623.96%</span> |
| 413 | [00796 WINDOW_PARTITION_SUM_009](crates/bench/sqlite_parity/cases/SQLITE_PARITY_796_WINDOW_PARTITION_SUM_009.rs) | P2 | memory | GEN_SQL_WINDOW | 2899982 | 49788591 | <span style="color:#dc2626">-1616.86%</span> |
| 414 | [00027 FOREIGN_KEY_FAILURE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_027_FOREIGN_KEY_FAILURE.rs) | P0 | memory | SQL_FOREIGN_KEYS_NEGATIVE | 3236529 | 55528275 | <span style="color:#dc2626">-1615.67%</span> |
| 415 | [00808 WINDOW_PARTITION_SUM_021](crates/bench/sqlite_parity/cases/SQLITE_PARITY_808_WINDOW_PARTITION_SUM_021.rs) | P2 | memory | GEN_SQL_WINDOW | 2875807 | 49188895 | <span style="color:#dc2626">-1610.44%</span> |
| 416 | [00028 ALTER_TABLE_RENAME](crates/bench/sqlite_parity/cases/SQLITE_PARITY_028_ALTER_TABLE_RENAME.rs) | P0 | memory | SQL_ALTER | 3218105 | 54888023 | <span style="color:#dc2626">-1605.60%</span> |
| 417 | [00014 INSERT_RETURNING](crates/bench/sqlite_parity/cases/SQLITE_PARITY_014_INSERT_RETURNING.rs) | P0 | memory | SQL_INSERT | 2674104 | 45556011 | <span style="color:#dc2626">-1603.60%</span> |
| 418 | [00007 WITHOUT_ROWID_TABLE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_007_WITHOUT_ROWID_TABLE.rs) | P0 | memory | SQL_ROWID | 2574094 | 43612931 | <span style="color:#dc2626">-1594.30%</span> |
| 419 | [00862 WINDOW_PARTITION_SUM_075](crates/bench/sqlite_parity/cases/SQLITE_PARITY_862_WINDOW_PARTITION_SUM_075.rs) | P2 | memory | GEN_SQL_WINDOW | 2995664 | 50440937 | <span style="color:#dc2626">-1583.80%</span> |
| 420 | [00249 SCALAR_CAST_TYPEOF_006](crates/bench/sqlite_parity/cases/SQLITE_PARITY_249_SCALAR_CAST_TYPEOF_006.rs) | P1 | memory | GEN_SQL_SCALAR | 1925215 | 32282004 | <span style="color:#dc2626">-1576.80%</span> |
| 421 | [00798 WINDOW_PARTITION_SUM_011](crates/bench/sqlite_parity/cases/SQLITE_PARITY_798_WINDOW_PARTITION_SUM_011.rs) | P2 | memory | GEN_SQL_WINDOW | 3085513 | 51719207 | <span style="color:#dc2626">-1576.19%</span> |
| 422 | [00865 WINDOW_PARTITION_SUM_078](crates/bench/sqlite_parity/cases/SQLITE_PARITY_865_WINDOW_PARTITION_SUM_078.rs) | P2 | memory | GEN_SQL_WINDOW | 2756831 | 46165861 | <span style="color:#dc2626">-1574.60%</span> |
| 423 | [00867 WINDOW_PARTITION_SUM_080](crates/bench/sqlite_parity/cases/SQLITE_PARITY_867_WINDOW_PARTITION_SUM_080.rs) | P2 | memory | GEN_SQL_WINDOW | 2803158 | 46906173 | <span style="color:#dc2626">-1573.33%</span> |
| 424 | [00052 PRAGMA_INTEGRITY_QUICK_CHECK](crates/bench/sqlite_parity/cases/SQLITE_PARITY_052_PRAGMA_INTEGRITY_QUICK_CHECK.rs) | P0 | memory | SQL_PRAGMA | 3022214 | 50494032 | <span style="color:#dc2626">-1570.76%</span> |
| 425 | [00822 WINDOW_PARTITION_SUM_035](crates/bench/sqlite_parity/cases/SQLITE_PARITY_822_WINDOW_PARTITION_SUM_035.rs) | P2 | memory | GEN_SQL_WINDOW | 2867922 | 47480269 | <span style="color:#dc2626">-1555.56%</span> |
| 426 | [00445 DML_WHERE_ORDER_LIMIT_058](crates/bench/sqlite_parity/cases/SQLITE_PARITY_445_DML_WHERE_ORDER_LIMIT_058.rs) | P1 | memory | GEN_SQL_DML | 3329635 | 54659251 | <span style="color:#dc2626">-1541.60%</span> |
| 427 | [00228 SCALAR_ARITH_001](crates/bench/sqlite_parity/cases/SQLITE_PARITY_228_SCALAR_ARITH_001.rs) | P1 | memory | GEN_SQL_SCALAR | 2061244 | 33828652 | <span style="color:#dc2626">-1541.18%</span> |
| 428 | [00083 CORE_NUMERIC_FUNCTIONS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_083_CORE_NUMERIC_FUNCTIONS.rs) | P0 | memory | SQL_FUNCTIONS | 1705530 | 27934891 | <span style="color:#dc2626">-1537.90%</span> |
| 429 | [00020 UPSERT_DO_UPDATE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_020_UPSERT_DO_UPDATE.rs) | P0 | memory | SQL_UPSERT | 3213044 | 52522232 | <span style="color:#dc2626">-1534.66%</span> |
| 430 | [00841 WINDOW_PARTITION_SUM_054](crates/bench/sqlite_parity/cases/SQLITE_PARITY_841_WINDOW_PARTITION_SUM_054.rs) | P2 | memory | GEN_SQL_WINDOW | 2853104 | 46254558 | <span style="color:#dc2626">-1521.20%</span> |
| 431 | [00256 SCALAR_ARITH_008](crates/bench/sqlite_parity/cases/SQLITE_PARITY_256_SCALAR_ARITH_008.rs) | P1 | memory | GEN_SQL_SCALAR | 1653081 | 26620892 | <span style="color:#dc2626">-1510.38%</span> |
| 432 | [00836 WINDOW_PARTITION_SUM_049](crates/bench/sqlite_parity/cases/SQLITE_PARITY_836_WINDOW_PARTITION_SUM_049.rs) | P2 | memory | GEN_SQL_WINDOW | 3174122 | 51096809 | <span style="color:#dc2626">-1509.79%</span> |
| 433 | [00859 WINDOW_PARTITION_SUM_072](crates/bench/sqlite_parity/cases/SQLITE_PARITY_859_WINDOW_PARTITION_SUM_072.rs) | P2 | memory | GEN_SQL_WINDOW | 2746321 | 44000450 | <span style="color:#dc2626">-1502.16%</span> |
| 434 | [00025 BEGIN_MODES](crates/bench/sqlite_parity/cases/SQLITE_PARITY_025_BEGIN_MODES.rs) | P0 | memory | SQL_TRANSACTION | 2479266 | 39486265 | <span style="color:#dc2626">-1492.66%</span> |
| 435 | [00849 WINDOW_PARTITION_SUM_062](crates/bench/sqlite_parity/cases/SQLITE_PARITY_849_WINDOW_PARTITION_SUM_062.rs) | P2 | memory | GEN_SQL_WINDOW | 2768092 | 43912403 | <span style="color:#dc2626">-1486.38%</span> |
| 436 | [00100 SCHEMA_SQLITE_SCHEMA](crates/bench/sqlite_parity/cases/SQLITE_PARITY_100_SCHEMA_SQLITE_SCHEMA.rs) | P0 | memory | SQL_SCHEMA | 2882739 | 45515654 | <span style="color:#dc2626">-1478.90%</span> |
| 437 | [00801 WINDOW_PARTITION_SUM_014](crates/bench/sqlite_parity/cases/SQLITE_PARITY_801_WINDOW_PARTITION_SUM_014.rs) | P2 | memory | GEN_SQL_WINDOW | 2884603 | 45543872 | <span style="color:#dc2626">-1478.86%</span> |
| 438 | [00860 WINDOW_PARTITION_SUM_073](crates/bench/sqlite_parity/cases/SQLITE_PARITY_860_WINDOW_PARTITION_SUM_073.rs) | P2 | memory | GEN_SQL_WINDOW | 2750859 | 43384743 | <span style="color:#dc2626">-1477.13%</span> |
| 439 | [00846 WINDOW_PARTITION_SUM_059](crates/bench/sqlite_parity/cases/SQLITE_PARITY_846_WINDOW_PARTITION_SUM_059.rs) | P2 | memory | GEN_SQL_WINDOW | 2976346 | 46679493 | <span style="color:#dc2626">-1468.35%</span> |
| 440 | [00858 WINDOW_PARTITION_SUM_071](crates/bench/sqlite_parity/cases/SQLITE_PARITY_858_WINDOW_PARTITION_SUM_071.rs) | P2 | memory | GEN_SQL_WINDOW | 2981697 | 46761018 | <span style="color:#dc2626">-1468.27%</span> |
| 441 | [00010 GENERATED_COLUMNS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_010_GENERATED_COLUMNS.rs) | P0 | memory | SQL_DDL | 2869303 | 44842189 | <span style="color:#dc2626">-1462.83%</span> |
| 442 | [00863 WINDOW_PARTITION_SUM_076](crates/bench/sqlite_parity/cases/SQLITE_PARITY_863_WINDOW_PARTITION_SUM_076.rs) | P2 | memory | GEN_SQL_WINDOW | 2939657 | 45829944 | <span style="color:#dc2626">-1459.02%</span> |
| 443 | [00337 SCALAR_CAST_TYPEOF_028](crates/bench/sqlite_parity/cases/SQLITE_PARITY_337_SCALAR_CAST_TYPEOF_028.rs) | P1 | memory | GEN_SQL_SCALAR | 2773493 | 43160365 | <span style="color:#dc2626">-1456.17%</span> |
| 444 | [00827 WINDOW_PARTITION_SUM_040](crates/bench/sqlite_parity/cases/SQLITE_PARITY_827_WINDOW_PARTITION_SUM_040.rs) | P2 | memory | GEN_SQL_WINDOW | 3249684 | 50536678 | <span style="color:#dc2626">-1455.13%</span> |
| 445 | [00084 CORE_FORMAT_QUOTE_HEX](crates/bench/sqlite_parity/cases/SQLITE_PARITY_084_CORE_FORMAT_QUOTE_HEX.rs) | P0 | memory | SQL_FUNCTIONS | 1697214 | 26366601 | <span style="color:#dc2626">-1453.52%</span> |
| 446 | [00799 WINDOW_PARTITION_SUM_012](crates/bench/sqlite_parity/cases/SQLITE_PARITY_799_WINDOW_PARTITION_SUM_012.rs) | P2 | memory | GEN_SQL_WINDOW | 3193297 | 49319372 | <span style="color:#dc2626">-1444.47%</span> |
| 447 | [00792 WINDOW_PARTITION_SUM_005](crates/bench/sqlite_parity/cases/SQLITE_PARITY_792_WINDOW_PARTITION_SUM_005.rs) | P2 | memory | GEN_SQL_WINDOW | 2909790 | 44936602 | <span style="color:#dc2626">-1444.32%</span> |
| 448 | [00114 DOT_PRINT](crates/bench/sqlite_parity/cases/SQLITE_PARITY_114_DOT_PRINT.rs) | P0 | memory | CLI_DOT_COMMAND | 2785925 | 42914689 | <span style="color:#dc2626">-1440.41%</span> |
| 449 | [00385 SCALAR_CAST_TYPEOF_040](crates/bench/sqlite_parity/cases/SQLITE_PARITY_385_SCALAR_CAST_TYPEOF_040.rs) | P1 | memory | GEN_SQL_SCALAR | 1848030 | 28347363 | <span style="color:#dc2626">-1433.92%</span> |
| 450 | [00273 SCALAR_CAST_TYPEOF_012](crates/bench/sqlite_parity/cases/SQLITE_PARITY_273_SCALAR_CAST_TYPEOF_012.rs) | P1 | memory | GEN_SQL_SCALAR | 1712613 | 26162475 | <span style="color:#dc2626">-1427.63%</span> |
| 451 | [00795 WINDOW_PARTITION_SUM_008](crates/bench/sqlite_parity/cases/SQLITE_PARITY_795_WINDOW_PARTITION_SUM_008.rs) | P2 | memory | GEN_SQL_WINDOW | 3110782 | 47501559 | <span style="color:#dc2626">-1427.00%</span> |
| 452 | [00800 WINDOW_PARTITION_SUM_013](crates/bench/sqlite_parity/cases/SQLITE_PARITY_800_WINDOW_PARTITION_SUM_013.rs) | P2 | memory | GEN_SQL_WINDOW | 3178920 | 48469844 | <span style="color:#dc2626">-1424.73%</span> |
| 453 | [00810 WINDOW_PARTITION_SUM_023](crates/bench/sqlite_parity/cases/SQLITE_PARITY_810_WINDOW_PARTITION_SUM_023.rs) | P2 | memory | GEN_SQL_WINDOW | 3130668 | 47450353 | <span style="color:#dc2626">-1415.66%</span> |
| 454 | [00829 WINDOW_PARTITION_SUM_042](crates/bench/sqlite_parity/cases/SQLITE_PARITY_829_WINDOW_PARTITION_SUM_042.rs) | P2 | memory | GEN_SQL_WINDOW | 3184872 | 48242744 | <span style="color:#dc2626">-1414.75%</span> |
| 455 | [00029 ALTER_TABLE_RENAME_COLUMN](crates/bench/sqlite_parity/cases/SQLITE_PARITY_029_ALTER_TABLE_RENAME_COLUMN.rs) | P0 | memory | SQL_ALTER | 3700207 | 56028522 | <span style="color:#dc2626">-1414.20%</span> |
| 456 | [00305 SCALAR_CAST_TYPEOF_020](crates/bench/sqlite_parity/cases/SQLITE_PARITY_305_SCALAR_CAST_TYPEOF_020.rs) | P1 | memory | GEN_SQL_SCALAR | 1730297 | 26166121 | <span style="color:#dc2626">-1412.23%</span> |
| 457 | [00248 SCALAR_ARITH_006](crates/bench/sqlite_parity/cases/SQLITE_PARITY_248_SCALAR_ARITH_006.rs) | P1 | memory | GEN_SQL_SCALAR | 1821599 | 27498164 | <span style="color:#dc2626">-1409.56%</span> |
| 458 | [00314 SCALAR_STRING_022](crates/bench/sqlite_parity/cases/SQLITE_PARITY_314_SCALAR_STRING_022.rs) | P1 | memory | GEN_SQL_SCALAR | 2208763 | 33281366 | <span style="color:#dc2626">-1406.79%</span> |
| 459 | [00815 WINDOW_PARTITION_SUM_028](crates/bench/sqlite_parity/cases/SQLITE_PARITY_815_WINDOW_PARTITION_SUM_028.rs) | P2 | memory | GEN_SQL_WINDOW | 3110911 | 46719919 | <span style="color:#dc2626">-1401.81%</span> |
| 460 | [00288 SCALAR_ARITH_016](crates/bench/sqlite_parity/cases/SQLITE_PARITY_288_SCALAR_ARITH_016.rs) | P1 | memory | GEN_SQL_SCALAR | 2091410 | 31390184 | <span style="color:#dc2626">-1400.91%</span> |
| 461 | [00056 SUBQUERIES_EXISTS_IN](crates/bench/sqlite_parity/cases/SQLITE_PARITY_056_SUBQUERIES_EXISTS_IN.rs) | P0 | memory | SQL_SELECT | 2923947 | 43841464 | <span style="color:#dc2626">-1399.39%</span> |
| 462 | [00301 SCALAR_CAST_TYPEOF_019](crates/bench/sqlite_parity/cases/SQLITE_PARITY_301_SCALAR_CAST_TYPEOF_019.rs) | P1 | memory | GEN_SQL_SCALAR | 1800740 | 26917955 | <span style="color:#dc2626">-1394.83%</span> |
| 463 | [00286 SCALAR_STRING_015](crates/bench/sqlite_parity/cases/SQLITE_PARITY_286_SCALAR_STRING_015.rs) | P1 | memory | GEN_SQL_SCALAR | 2044943 | 30546327 | <span style="color:#dc2626">-1393.75%</span> |
| 464 | [00113 DOT_NULLVALUE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_113_DOT_NULLVALUE.rs) | P0 | memory | CLI_DOT_COMMAND | 2557373 | 38168521 | <span style="color:#dc2626">-1392.49%</span> |
| 465 | [00866 WINDOW_PARTITION_SUM_079](crates/bench/sqlite_parity/cases/SQLITE_PARITY_866_WINDOW_PARTITION_SUM_079.rs) | P2 | memory | GEN_SQL_WINDOW | 3052541 | 45549784 | <span style="color:#dc2626">-1392.19%</span> |
| 466 | [00365 SCALAR_CAST_TYPEOF_035](crates/bench/sqlite_parity/cases/SQLITE_PARITY_365_SCALAR_CAST_TYPEOF_035.rs) | P1 | memory | GEN_SQL_SCALAR | 1975841 | 29282323 | <span style="color:#dc2626">-1382.02%</span> |
| 467 | [00794 WINDOW_PARTITION_SUM_007](crates/bench/sqlite_parity/cases/SQLITE_PARITY_794_WINDOW_PARTITION_SUM_007.rs) | P2 | memory | GEN_SQL_WINDOW | 3071256 | 45467899 | <span style="color:#dc2626">-1380.43%</span> |
| 468 | [00853 WINDOW_PARTITION_SUM_066](crates/bench/sqlite_parity/cases/SQLITE_PARITY_853_WINDOW_PARTITION_SUM_066.rs) | P2 | memory | GEN_SQL_WINDOW | 3121832 | 46046043 | <span style="color:#dc2626">-1374.97%</span> |
| 469 | [00082 CORE_STRING_FUNCTIONS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_082_CORE_STRING_FUNCTIONS.rs) | P0 | memory | SQL_FUNCTIONS | 2119674 | 31251201 | <span style="color:#dc2626">-1374.34%</span> |
| 470 | [00001 SELECT_CORE_EXPRESSIONS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_001_SELECT_CORE_EXPRESSIONS.rs) | P0 | memory | SQL_SELECT | 2047187 | 29950959 | <span style="color:#dc2626">-1363.03%</span> |
| 471 | [00857 WINDOW_PARTITION_SUM_070](crates/bench/sqlite_parity/cases/SQLITE_PARITY_857_WINDOW_PARTITION_SUM_070.rs) | P2 | memory | GEN_SQL_WINDOW | 2990032 | 43692536 | <span style="color:#dc2626">-1361.27%</span> |
| 472 | [00322 SCALAR_STRING_024](crates/bench/sqlite_parity/cases/SQLITE_PARITY_322_SCALAR_STRING_024.rs) | P1 | memory | GEN_SQL_SCALAR | 1734565 | 25190845 | <span style="color:#dc2626">-1352.29%</span> |
| 473 | [00109 DOT_MODE_JSON](crates/bench/sqlite_parity/cases/SQLITE_PARITY_109_DOT_MODE_JSON.rs) | P0 | memory | CLI_DOT_COMMAND | 1733993 | 25132614 | <span style="color:#dc2626">-1349.41%</span> |
| 474 | [00004 TABLE_CONSTRAINTS_SUCCESS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_004_TABLE_CONSTRAINTS_SUCCESS.rs) | P0 | memory | SQL_CONSTRAINTS | 3224286 | 46572906 | <span style="color:#dc2626">-1344.44%</span> |
| 475 | [00294 SCALAR_STRING_017](crates/bench/sqlite_parity/cases/SQLITE_PARITY_294_SCALAR_STRING_017.rs) | P1 | memory | GEN_SQL_SCALAR | 1658120 | 23905791 | <span style="color:#dc2626">-1341.74%</span> |
| 476 | [00023 TRANSACTION_ROLLBACK](crates/bench/sqlite_parity/cases/SQLITE_PARITY_023_TRANSACTION_ROLLBACK.rs) | P0 | memory | SQL_TRANSACTION | 2773171 | 39888428 | <span style="color:#dc2626">-1338.37%</span> |
| 477 | [00118 DOT_FULLSCHEMA](crates/bench/sqlite_parity/cases/SQLITE_PARITY_118_DOT_FULLSCHEMA.rs) | P0 | memory | CLI_DOT_COMMAND | 2681037 | 38455945 | <span style="color:#dc2626">-1334.37%</span> |
| 478 | [00022 TRANSACTION_COMMIT](crates/bench/sqlite_parity/cases/SQLITE_PARITY_022_TRANSACTION_COMMIT.rs) | P0 | memory | SQL_TRANSACTION | 2593442 | 37148860 | <span style="color:#dc2626">-1332.42%</span> |
| 479 | [00298 SCALAR_STRING_018](crates/bench/sqlite_parity/cases/SQLITE_PARITY_298_SCALAR_STRING_018.rs) | P1 | memory | GEN_SQL_SCALAR | 1844823 | 26161352 | <span style="color:#dc2626">-1318.10%</span> |
| 480 | [00285 SCALAR_CAST_TYPEOF_015](crates/bench/sqlite_parity/cases/SQLITE_PARITY_285_SCALAR_CAST_TYPEOF_015.rs) | P1 | memory | GEN_SQL_SCALAR | 2422288 | 34327127 | <span style="color:#dc2626">-1317.14%</span> |
| 481 | [00380 SCALAR_ARITH_039](crates/bench/sqlite_parity/cases/SQLITE_PARITY_380_SCALAR_ARITH_039.rs) | P1 | memory | GEN_SQL_SCALAR | 1727251 | 24384197 | <span style="color:#dc2626">-1311.73%</span> |
| 482 | [00816 WINDOW_PARTITION_SUM_029](crates/bench/sqlite_parity/cases/SQLITE_PARITY_816_WINDOW_PARTITION_SUM_029.rs) | P2 | memory | GEN_SQL_WINDOW | 3172458 | 44733949 | <span style="color:#dc2626">-1310.07%</span> |
| 483 | [00102 WITH_MATERIALIZED_HINTS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_102_WITH_MATERIALIZED_HINTS.rs) | P0 | memory | SQL_CTE | 1818093 | 25536920 | <span style="color:#dc2626">-1304.60%</span> |
| 484 | [00861 WINDOW_PARTITION_SUM_074](crates/bench/sqlite_parity/cases/SQLITE_PARITY_861_WINDOW_PARTITION_SUM_074.rs) | P2 | memory | GEN_SQL_WINDOW | 3465433 | 48522504 | <span style="color:#dc2626">-1300.19%</span> |
| 485 | [00266 SCALAR_STRING_010](crates/bench/sqlite_parity/cases/SQLITE_PARITY_266_SCALAR_STRING_010.rs) | P1 | memory | GEN_SQL_SCALAR | 1803275 | 25131252 | <span style="color:#dc2626">-1293.65%</span> |
| 486 | [00005 UNIQUE_CONSTRAINT_FAILURE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_005_UNIQUE_CONSTRAINT_FAILURE.rs) | P0 | memory | SQL_CONSTRAINTS_NEGATIVE | 3111913 | 43199337 | <span style="color:#dc2626">-1288.19%</span> |
| 487 | [00050 PRAGMA_TABLE_INFO_FUNCTION](crates/bench/sqlite_parity/cases/SQLITE_PARITY_050_PRAGMA_TABLE_INFO_FUNCTION.rs) | P0 | memory | SQL_PRAGMA | 2852432 | 39162093 | <span style="color:#dc2626">-1272.94%</span> |
| 488 | [00340 SCALAR_ARITH_029](crates/bench/sqlite_parity/cases/SQLITE_PARITY_340_SCALAR_ARITH_029.rs) | P1 | memory | GEN_SQL_SCALAR | 1762788 | 24193656 | <span style="color:#dc2626">-1272.47%</span> |
| 489 | [00176 OPT_LINE_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_176_OPT_LINE_MODE.rs) | P2 | memory | CLI_OPTION | 1683609 | 22926096 | <span style="color:#dc2626">-1261.72%</span> |
| 490 | [00069 COLLATE_NOCASE_RTRIM_BINARY](crates/bench/sqlite_parity/cases/SQLITE_PARITY_069_COLLATE_NOCASE_RTRIM_BINARY.rs) | P0 | memory | SQL_COLLATION | 1675803 | 22755322 | <span style="color:#dc2626">-1257.88%</span> |
| 491 | [00847 WINDOW_PARTITION_SUM_060](crates/bench/sqlite_parity/cases/SQLITE_PARITY_847_WINDOW_PARTITION_SUM_060.rs) | P2 | memory | GEN_SQL_WINDOW | 3227723 | 43790111 | <span style="color:#dc2626">-1256.69%</span> |
| 492 | [00341 SCALAR_CAST_TYPEOF_029](crates/bench/sqlite_parity/cases/SQLITE_PARITY_341_SCALAR_CAST_TYPEOF_029.rs) | P1 | memory | GEN_SQL_SCALAR | 1736920 | 23368785 | <span style="color:#dc2626">-1245.42%</span> |
| 493 | [00272 SCALAR_ARITH_012](crates/bench/sqlite_parity/cases/SQLITE_PARITY_272_SCALAR_ARITH_012.rs) | P1 | memory | GEN_SQL_SCALAR | 1669091 | 22187257 | <span style="color:#dc2626">-1229.30%</span> |
| 494 | [00349 SCALAR_CAST_TYPEOF_031](crates/bench/sqlite_parity/cases/SQLITE_PARITY_349_SCALAR_CAST_TYPEOF_031.rs) | P1 | memory | GEN_SQL_SCALAR | 3048663 | 40461063 | <span style="color:#dc2626">-1227.17%</span> |
| 495 | [00223 OPT_NOHEADER](crates/bench/sqlite_parity/cases/SQLITE_PARITY_223_OPT_NOHEADER.rs) | P2 | memory | CLI_OPTION | 1751487 | 22919393 | <span style="color:#dc2626">-1208.57%</span> |
| 496 | [00110 DOT_MODE_LINE_COLUMN_TABLE_BOX_MARKDOWN](crates/bench/sqlite_parity/cases/SQLITE_PARITY_110_DOT_MODE_LINE_COLUMN_TABLE_BOX_MARKDOWN.rs) | P0 | memory | CLI_DOT_COMMAND | 2494063 | 32568485 | <span style="color:#dc2626">-1205.84%</span> |
| 497 | [00357 SCALAR_CAST_TYPEOF_033](crates/bench/sqlite_parity/cases/SQLITE_PARITY_357_SCALAR_CAST_TYPEOF_033.rs) | P1 | memory | GEN_SQL_SCALAR | 1682055 | 21914570 | <span style="color:#dc2626">-1202.85%</span> |
| 498 | [00316 SCALAR_ARITH_023](crates/bench/sqlite_parity/cases/SQLITE_PARITY_316_SCALAR_ARITH_023.rs) | P1 | memory | GEN_SQL_SCALAR | 1780131 | 23136635 | <span style="color:#dc2626">-1199.72%</span> |
| 499 | [00241 SCALAR_CAST_TYPEOF_004](crates/bench/sqlite_parity/cases/SQLITE_PARITY_241_SCALAR_CAST_TYPEOF_004.rs) | P1 | memory | GEN_SQL_SCALAR | 1703116 | 22102717 | <span style="color:#dc2626">-1197.78%</span> |
| 500 | [00276 SCALAR_ARITH_013](crates/bench/sqlite_parity/cases/SQLITE_PARITY_276_SCALAR_ARITH_013.rs) | P1 | memory | GEN_SQL_SCALAR | 2019895 | 26204365 | <span style="color:#dc2626">-1197.31%</span> |
| 501 | [00856 WINDOW_PARTITION_SUM_069](crates/bench/sqlite_parity/cases/SQLITE_PARITY_856_WINDOW_PARTITION_SUM_069.rs) | P2 | memory | GEN_SQL_WINDOW | 3551295 | 46028550 | <span style="color:#dc2626">-1196.11%</span> |
| 502 | [00112 DOT_SEPARATOR](crates/bench/sqlite_parity/cases/SQLITE_PARITY_112_DOT_SEPARATOR.rs) | P0 | memory | CLI_DOT_COMMAND | 2603690 | 33717111 | <span style="color:#dc2626">-1194.97%</span> |
| 503 | [00173 OPT_LIST_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_173_OPT_LIST_MODE.rs) | P1 | memory | CLI_OPTION | 1694780 | 21920502 | <span style="color:#dc2626">-1193.41%</span> |
| 504 | [00320 SCALAR_ARITH_024](crates/bench/sqlite_parity/cases/SQLITE_PARITY_320_SCALAR_ARITH_024.rs) | P1 | memory | GEN_SQL_SCALAR | 1754042 | 22669400 | <span style="color:#dc2626">-1192.41%</span> |
| 505 | [00238 SCALAR_STRING_003](crates/bench/sqlite_parity/cases/SQLITE_PARITY_238_SCALAR_STRING_003.rs) | P1 | memory | GEN_SQL_SCALAR | 1691784 | 21726714 | <span style="color:#dc2626">-1184.25%</span> |
| 506 | [00178 OPT_HTML_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_178_OPT_HTML_MODE.rs) | P2 | memory | CLI_OPTION | 2796025 | 35415397 | <span style="color:#dc2626">-1166.63%</span> |
| 507 | [00318 SCALAR_STRING_023](crates/bench/sqlite_parity/cases/SQLITE_PARITY_318_SCALAR_STRING_023.rs) | P1 | memory | GEN_SQL_SCALAR | 1823052 | 23008131 | <span style="color:#dc2626">-1162.07%</span> |
| 508 | [00264 SCALAR_ARITH_010](crates/bench/sqlite_parity/cases/SQLITE_PARITY_264_SCALAR_ARITH_010.rs) | P1 | memory | GEN_SQL_SCALAR | 1780522 | 22449183 | <span style="color:#dc2626">-1160.82%</span> |
| 509 | [00324 SCALAR_ARITH_025](crates/bench/sqlite_parity/cases/SQLITE_PARITY_324_SCALAR_ARITH_025.rs) | P1 | memory | GEN_SQL_SCALAR | 1786984 | 22525196 | <span style="color:#dc2626">-1160.51%</span> |
| 510 | [00048 PRAGMA_USER_VERSION](crates/bench/sqlite_parity/cases/SQLITE_PARITY_048_PRAGMA_USER_VERSION.rs) | P0 | memory | SQL_PRAGMA | 2401388 | 30217254 | <span style="color:#dc2626">-1158.32%</span> |
| 511 | [00306 SCALAR_STRING_020](crates/bench/sqlite_parity/cases/SQLITE_PARITY_306_SCALAR_STRING_020.rs) | P1 | memory | GEN_SQL_SCALAR | 1855404 | 23211496 | <span style="color:#dc2626">-1151.02%</span> |
| 512 | [00274 SCALAR_STRING_012](crates/bench/sqlite_parity/cases/SQLITE_PARITY_274_SCALAR_STRING_012.rs) | P1 | memory | GEN_SQL_SCALAR | 1747580 | 21816756 | <span style="color:#dc2626">-1148.40%</span> |
| 513 | [00346 SCALAR_STRING_030](crates/bench/sqlite_parity/cases/SQLITE_PARITY_346_SCALAR_STRING_030.rs) | P1 | memory | GEN_SQL_SCALAR | 1723174 | 21278295 | <span style="color:#dc2626">-1134.83%</span> |
| 514 | [00261 SCALAR_CAST_TYPEOF_009](crates/bench/sqlite_parity/cases/SQLITE_PARITY_261_SCALAR_CAST_TYPEOF_009.rs) | P1 | memory | GEN_SQL_SCALAR | 1792184 | 21995835 | <span style="color:#dc2626">-1127.32%</span> |
| 515 | [00179 OPT_MARKDOWN_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_179_OPT_MARKDOWN_MODE.rs) | P2 | memory | CLI_OPTION | 2521545 | 30920806 | <span style="color:#dc2626">-1126.26%</span> |
| 516 | [00373 SCALAR_CAST_TYPEOF_037](crates/bench/sqlite_parity/cases/SQLITE_PARITY_373_SCALAR_CAST_TYPEOF_037.rs) | P1 | memory | GEN_SQL_SCALAR | 1977234 | 24226919 | <span style="color:#dc2626">-1125.29%</span> |
| 517 | [00364 SCALAR_ARITH_035](crates/bench/sqlite_parity/cases/SQLITE_PARITY_364_SCALAR_ARITH_035.rs) | P1 | memory | GEN_SQL_SCALAR | 2632405 | 32236087 | <span style="color:#dc2626">-1124.59%</span> |
| 518 | [00268 SCALAR_ARITH_011](crates/bench/sqlite_parity/cases/SQLITE_PARITY_268_SCALAR_ARITH_011.rs) | P1 | memory | GEN_SQL_SCALAR | 1747410 | 21186561 | <span style="color:#dc2626">-1112.46%</span> |
| 519 | [00338 SCALAR_STRING_028](crates/bench/sqlite_parity/cases/SQLITE_PARITY_338_SCALAR_STRING_028.rs) | P1 | memory | GEN_SQL_SCALAR | 2572011 | 31100056 | <span style="color:#dc2626">-1109.17%</span> |
| 520 | [00378 SCALAR_STRING_038](crates/bench/sqlite_parity/cases/SQLITE_PARITY_378_SCALAR_STRING_038.rs) | P1 | memory | GEN_SQL_SCALAR | 2119644 | 25608196 | <span style="color:#dc2626">-1108.14%</span> |
| 521 | [00370 SCALAR_STRING_036](crates/bench/sqlite_parity/cases/SQLITE_PARITY_370_SCALAR_STRING_036.rs) | P1 | memory | GEN_SQL_SCALAR | 1829014 | 21951701 | <span style="color:#dc2626">-1100.19%</span> |
| 522 | [00250 SCALAR_STRING_006](crates/bench/sqlite_parity/cases/SQLITE_PARITY_250_SCALAR_STRING_006.rs) | P1 | memory | GEN_SQL_SCALAR | 1822962 | 21857853 | <span style="color:#dc2626">-1099.03%</span> |
| 523 | [00296 SCALAR_ARITH_018](crates/bench/sqlite_parity/cases/SQLITE_PARITY_296_SCALAR_ARITH_018.rs) | P1 | memory | GEN_SQL_SCALAR | 1787655 | 21432959 | <span style="color:#dc2626">-1098.94%</span> |
| 524 | [00002 LITERALS_AND_TYPEOF](crates/bench/sqlite_parity/cases/SQLITE_PARITY_002_LITERALS_AND_TYPEOF.rs) | P0 | memory | SQL_EXPRESSIONS | 1855894 | 22190412 | <span style="color:#dc2626">-1095.67%</span> |
| 525 | [00111 DOT_HEADERS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_111_DOT_HEADERS.rs) | P0 | memory | CLI_DOT_COMMAND | 2052306 | 24446815 | <span style="color:#dc2626">-1091.19%</span> |
| 526 | [00191 OPT_BATCH](crates/bench/sqlite_parity/cases/SQLITE_PARITY_191_OPT_BATCH.rs) | P2 | memory | CLI_OPTION | 1677517 | 19920855 | <span style="color:#dc2626">-1087.52%</span> |
| 527 | [00233 SCALAR_CAST_TYPEOF_002](crates/bench/sqlite_parity/cases/SQLITE_PARITY_233_SCALAR_CAST_TYPEOF_002.rs) | P1 | memory | GEN_SQL_SCALAR | 1698016 | 20110213 | <span style="color:#dc2626">-1084.34%</span> |
| 528 | [00253 SCALAR_CAST_TYPEOF_007](crates/bench/sqlite_parity/cases/SQLITE_PARITY_253_SCALAR_CAST_TYPEOF_007.rs) | P1 | memory | GEN_SQL_SCALAR | 1724025 | 20410552 | <span style="color:#dc2626">-1083.89%</span> |
| 529 | [00230 SCALAR_STRING_001](crates/bench/sqlite_parity/cases/SQLITE_PARITY_230_SCALAR_STRING_001.rs) | P1 | memory | GEN_SQL_SCALAR | 1836017 | 21636684 | <span style="color:#dc2626">-1078.46%</span> |
| 530 | [00242 SCALAR_STRING_004](crates/bench/sqlite_parity/cases/SQLITE_PARITY_242_SCALAR_STRING_004.rs) | P1 | memory | GEN_SQL_SCALAR | 1745215 | 20479072 | <span style="color:#dc2626">-1073.44%</span> |
| 531 | [00289 SCALAR_CAST_TYPEOF_016](crates/bench/sqlite_parity/cases/SQLITE_PARITY_289_SCALAR_CAST_TYPEOF_016.rs) | P1 | memory | GEN_SQL_SCALAR | 1949162 | 22828311 | <span style="color:#dc2626">-1071.19%</span> |
| 532 | [00374 SCALAR_STRING_037](crates/bench/sqlite_parity/cases/SQLITE_PARITY_374_SCALAR_STRING_037.rs) | P1 | memory | GEN_SQL_SCALAR | 2696817 | 31363064 | <span style="color:#dc2626">-1062.97%</span> |
| 533 | [00302 SCALAR_STRING_019](crates/bench/sqlite_parity/cases/SQLITE_PARITY_302_SCALAR_STRING_019.rs) | P1 | memory | GEN_SQL_SCALAR | 2124643 | 24698693 | <span style="color:#dc2626">-1062.49%</span> |
| 534 | [00244 SCALAR_ARITH_005](crates/bench/sqlite_parity/cases/SQLITE_PARITY_244_SCALAR_ARITH_005.rs) | P1 | memory | GEN_SQL_SCALAR | 1711551 | 19882853 | <span style="color:#dc2626">-1061.69%</span> |
| 535 | [00366 SCALAR_STRING_035](crates/bench/sqlite_parity/cases/SQLITE_PARITY_366_SCALAR_STRING_035.rs) | P1 | memory | GEN_SQL_SCALAR | 2333208 | 27075794 | <span style="color:#dc2626">-1060.45%</span> |
| 536 | [00386 SCALAR_STRING_040](crates/bench/sqlite_parity/cases/SQLITE_PARITY_386_SCALAR_STRING_040.rs) | P1 | memory | GEN_SQL_SCALAR | 2166272 | 25079174 | <span style="color:#dc2626">-1057.71%</span> |
| 537 | [00369 SCALAR_CAST_TYPEOF_036](crates/bench/sqlite_parity/cases/SQLITE_PARITY_369_SCALAR_CAST_TYPEOF_036.rs) | P1 | memory | GEN_SQL_SCALAR | 1923984 | 22223295 | <span style="color:#dc2626">-1055.07%</span> |
| 538 | [00270 SCALAR_STRING_011](crates/bench/sqlite_parity/cases/SQLITE_PARITY_270_SCALAR_STRING_011.rs) | P1 | memory | GEN_SQL_SCALAR | 1694859 | 19489558 | <span style="color:#dc2626">-1049.92%</span> |
| 539 | [00183 OPT_TABS_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_183_OPT_TABS_MODE.rs) | P2 | memory | CLI_OPTION | 1979378 | 22729945 | <span style="color:#dc2626">-1048.34%</span> |
| 540 | [00372 SCALAR_ARITH_037](crates/bench/sqlite_parity/cases/SQLITE_PARITY_372_SCALAR_ARITH_037.rs) | P1 | memory | GEN_SQL_SCALAR | 2146545 | 24626306 | <span style="color:#dc2626">-1047.25%</span> |
| 541 | [00064 CTE_NON_RECURSIVE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_064_CTE_NON_RECURSIVE.rs) | P0 | memory | SQL_CTE | 1993426 | 22842868 | <span style="color:#dc2626">-1045.91%</span> |
| 542 | [00232 SCALAR_ARITH_002](crates/bench/sqlite_parity/cases/SQLITE_PARITY_232_SCALAR_ARITH_002.rs) | P1 | memory | GEN_SQL_SCALAR | 1930536 | 22052050 | <span style="color:#dc2626">-1042.28%</span> |
| 543 | [00229 SCALAR_CAST_TYPEOF_001](crates/bench/sqlite_parity/cases/SQLITE_PARITY_229_SCALAR_CAST_TYPEOF_001.rs) | P1 | memory | GEN_SQL_SCALAR | 2529360 | 28653372 | <span style="color:#dc2626">-1032.83%</span> |
| 544 | [00127 DOT_LIMIT](crates/bench/sqlite_parity/cases/SQLITE_PARITY_127_DOT_LIMIT.rs) | P0 | memory | CLI_DOT_COMMAND | 1740666 | 19561514 | <span style="color:#dc2626">-1023.79%</span> |
| 545 | [00182 OPT_COLUMN_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_182_OPT_COLUMN_MODE.rs) | P2 | memory | CLI_OPTION | 2029844 | 22773367 | <span style="color:#dc2626">-1021.93%</span> |
| 546 | [00321 SCALAR_CAST_TYPEOF_024](crates/bench/sqlite_parity/cases/SQLITE_PARITY_321_SCALAR_CAST_TYPEOF_024.rs) | P1 | memory | GEN_SQL_SCALAR | 2182724 | 24474358 | <span style="color:#dc2626">-1021.28%</span> |
| 547 | [00098 CLI_REGEXP_OPTIONAL](crates/bench/sqlite_parity/cases/SQLITE_PARITY_098_CLI_REGEXP_OPTIONAL.rs) | P2 | memory | CLI_EXTENSION_OPTIONAL | 2281911 | 25540287 | <span style="color:#dc2626">-1019.25%</span> |
| 548 | [00107 DOT_HELP_PATTERN](crates/bench/sqlite_parity/cases/SQLITE_PARITY_107_DOT_HELP_PATTERN.rs) | P0 | memory | CLI_DOT_COMMAND | 3018155 | 33464633 | <span style="color:#dc2626">-1008.78%</span> |
| 549 | [00352 SCALAR_ARITH_032](crates/bench/sqlite_parity/cases/SQLITE_PARITY_352_SCALAR_ARITH_032.rs) | P1 | memory | GEN_SQL_SCALAR | 2127799 | 23296107 | <span style="color:#dc2626">-994.85%</span> |
| 550 | [00281 SCALAR_CAST_TYPEOF_014](crates/bench/sqlite_parity/cases/SQLITE_PARITY_281_SCALAR_CAST_TYPEOF_014.rs) | P1 | memory | GEN_SQL_SCALAR | 2925029 | 31945486 | <span style="color:#dc2626">-992.14%</span> |
| 551 | [00317 SCALAR_CAST_TYPEOF_023](crates/bench/sqlite_parity/cases/SQLITE_PARITY_317_SCALAR_CAST_TYPEOF_023.rs) | P1 | memory | GEN_SQL_SCALAR | 2127529 | 23142335 | <span style="color:#dc2626">-987.76%</span> |
| 552 | [00260 SCALAR_ARITH_009](crates/bench/sqlite_parity/cases/SQLITE_PARITY_260_SCALAR_ARITH_009.rs) | P1 | memory | GEN_SQL_SCALAR | 2144091 | 23235031 | <span style="color:#dc2626">-983.68%</span> |
| 553 | [00047 PRAGMA_FOREIGN_KEYS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_047_PRAGMA_FOREIGN_KEYS.rs) | P0 | memory | SQL_PRAGMA | 2125254 | 22997952 | <span style="color:#dc2626">-982.13%</span> |
| 554 | [00382 SCALAR_STRING_039](crates/bench/sqlite_parity/cases/SQLITE_PARITY_382_SCALAR_STRING_039.rs) | P1 | memory | GEN_SQL_SCALAR | 2674314 | 28597717 | <span style="color:#dc2626">-969.35%</span> |
| 555 | [00304 SCALAR_ARITH_020](crates/bench/sqlite_parity/cases/SQLITE_PARITY_304_SCALAR_ARITH_020.rs) | P1 | memory | GEN_SQL_SCALAR | 2208712 | 23574804 | <span style="color:#dc2626">-967.36%</span> |
| 556 | [00086 DATE_TIME_FUNCTIONS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_086_DATE_TIME_FUNCTIONS.rs) | P0 | memory | SQL_FUNCTIONS | 2091451 | 22233865 | <span style="color:#dc2626">-963.08%</span> |
| 557 | [00049 PRAGMA_TEMP_STORE_MEMORY](crates/bench/sqlite_parity/cases/SQLITE_PARITY_049_PRAGMA_TEMP_STORE_MEMORY.rs) | P0 | memory | SQL_PRAGMA | 1987945 | 20964932 | <span style="color:#dc2626">-954.60%</span> |
| 558 | [00106 DOT_HELP](crates/bench/sqlite_parity/cases/SQLITE_PARITY_106_DOT_HELP.rs) | P0 | memory | CLI_DOT_COMMAND | 2077845 | 21858444 | <span style="color:#dc2626">-951.98%</span> |
| 559 | [00308 SCALAR_ARITH_021](crates/bench/sqlite_parity/cases/SQLITE_PARITY_308_SCALAR_ARITH_021.rs) | P1 | memory | GEN_SQL_SCALAR | 2072274 | 21534732 | <span style="color:#dc2626">-939.18%</span> |
| 560 | [00123 DOT_ECHO](crates/bench/sqlite_parity/cases/SQLITE_PARITY_123_DOT_ECHO.rs) | P0 | memory | CLI_DOT_COMMAND | 2041707 | 20939674 | <span style="color:#dc2626">-925.60%</span> |
| 561 | [00348 SCALAR_ARITH_031](crates/bench/sqlite_parity/cases/SQLITE_PARITY_348_SCALAR_ARITH_031.rs) | P1 | memory | GEN_SQL_SCALAR | 2323961 | 23647342 | <span style="color:#dc2626">-917.54%</span> |
| 562 | [00245 SCALAR_CAST_TYPEOF_005](crates/bench/sqlite_parity/cases/SQLITE_PARITY_245_SCALAR_CAST_TYPEOF_005.rs) | P1 | memory | GEN_SQL_SCALAR | 1994227 | 20134890 | <span style="color:#dc2626">-909.66%</span> |
| 563 | [00240 SCALAR_ARITH_004](crates/bench/sqlite_parity/cases/SQLITE_PARITY_240_SCALAR_ARITH_004.rs) | P1 | memory | GEN_SQL_SCALAR | 2157546 | 21634890 | <span style="color:#dc2626">-902.75%</span> |
| 564 | [00116 DOT_DATABASES](crates/bench/sqlite_parity/cases/SQLITE_PARITY_116_DOT_DATABASES.rs) | P0 | memory | CLI_DOT_COMMAND | 2796696 | 27586822 | <span style="color:#dc2626">-886.41%</span> |
| 565 | [00336 SCALAR_ARITH_028](crates/bench/sqlite_parity/cases/SQLITE_PARITY_336_SCALAR_ARITH_028.rs) | P1 | memory | GEN_SQL_SCALAR | 2398762 | 23565877 | <span style="color:#dc2626">-882.42%</span> |
| 566 | [00181 OPT_TABLE_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_181_OPT_TABLE_MODE.rs) | P2 | memory | CLI_OPTION | 2298723 | 22499889 | <span style="color:#dc2626">-878.80%</span> |
| 567 | [00221 OPT_DOUBLE_DASH_END_OPTIONS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_221_OPT_DOUBLE_DASH_END_OPTIONS.rs) | P2 | memory | CLI_OPTION | 2207661 | 21526526 | <span style="color:#dc2626">-875.08%</span> |
| 568 | [00258 SCALAR_STRING_008](crates/bench/sqlite_parity/cases/SQLITE_PARITY_258_SCALAR_STRING_008.rs) | P1 | memory | GEN_SQL_SCALAR | 2255401 | 21955027 | <span style="color:#dc2626">-873.44%</span> |
| 569 | [00300 SCALAR_ARITH_019](crates/bench/sqlite_parity/cases/SQLITE_PARITY_300_SCALAR_ARITH_019.rs) | P1 | memory | GEN_SQL_SCALAR | 2599964 | 25030190 | <span style="color:#dc2626">-862.71%</span> |
| 570 | [00088 JSON_SCALAR_FUNCTIONS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_088_JSON_SCALAR_FUNCTIONS.rs) | P0 | memory | SQL_JSON | 2374337 | 22815676 | <span style="color:#dc2626">-860.93%</span> |
| 571 | [00237 SCALAR_CAST_TYPEOF_003](crates/bench/sqlite_parity/cases/SQLITE_PARITY_237_SCALAR_CAST_TYPEOF_003.rs) | P1 | memory | GEN_SQL_SCALAR | 2327187 | 22325067 | <span style="color:#dc2626">-859.32%</span> |
| 572 | [00376 SCALAR_ARITH_038](crates/bench/sqlite_parity/cases/SQLITE_PARITY_376_SCALAR_ARITH_038.rs) | P1 | memory | GEN_SQL_SCALAR | 2702387 | 25868228 | <span style="color:#dc2626">-857.24%</span> |
| 573 | [00293 SCALAR_CAST_TYPEOF_017](crates/bench/sqlite_parity/cases/SQLITE_PARITY_293_SCALAR_CAST_TYPEOF_017.rs) | P1 | memory | GEN_SQL_SCALAR | 2609622 | 24750220 | <span style="color:#dc2626">-848.42%</span> |
| 574 | [00361 SCALAR_CAST_TYPEOF_034](crates/bench/sqlite_parity/cases/SQLITE_PARITY_361_SCALAR_CAST_TYPEOF_034.rs) | P1 | memory | GEN_SQL_SCALAR | 2292191 | 21654849 | <span style="color:#dc2626">-844.72%</span> |
| 575 | [00313 SCALAR_CAST_TYPEOF_022](crates/bench/sqlite_parity/cases/SQLITE_PARITY_313_SCALAR_CAST_TYPEOF_022.rs) | P1 | memory | GEN_SQL_SCALAR | 2436333 | 23006619 | <span style="color:#dc2626">-844.31%</span> |
| 576 | [00377 SCALAR_CAST_TYPEOF_038](crates/bench/sqlite_parity/cases/SQLITE_PARITY_377_SCALAR_CAST_TYPEOF_038.rs) | P1 | memory | GEN_SQL_SCALAR | 3022504 | 28389703 | <span style="color:#dc2626">-839.28%</span> |
| 577 | [00342 SCALAR_STRING_029](crates/bench/sqlite_parity/cases/SQLITE_PARITY_342_SCALAR_STRING_029.rs) | P1 | memory | GEN_SQL_SCALAR | 2525443 | 23401066 | <span style="color:#dc2626">-826.61%</span> |
| 578 | [00358 SCALAR_STRING_033](crates/bench/sqlite_parity/cases/SQLITE_PARITY_358_SCALAR_STRING_033.rs) | P1 | memory | GEN_SQL_SCALAR | 2239311 | 20717464 | <span style="color:#dc2626">-825.17%</span> |
| 579 | [00180 OPT_BOX_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_180_OPT_BOX_MODE.rs) | P2 | memory | CLI_OPTION | 2125135 | 19656824 | <span style="color:#dc2626">-824.97%</span> |
| 580 | [00356 SCALAR_ARITH_033](crates/bench/sqlite_parity/cases/SQLITE_PARITY_356_SCALAR_ARITH_033.rs) | P1 | memory | GEN_SQL_SCALAR | 2540822 | 23481227 | <span style="color:#dc2626">-824.16%</span> |
| 581 | [00345 SCALAR_CAST_TYPEOF_030](crates/bench/sqlite_parity/cases/SQLITE_PARITY_345_SCALAR_CAST_TYPEOF_030.rs) | P1 | memory | GEN_SQL_SCALAR | 2497880 | 22925164 | <span style="color:#dc2626">-817.78%</span> |
| 582 | [00278 SCALAR_STRING_013](crates/bench/sqlite_parity/cases/SQLITE_PARITY_278_SCALAR_STRING_013.rs) | P1 | memory | GEN_SQL_SCALAR | 2582440 | 23625320 | <span style="color:#dc2626">-814.84%</span> |
| 583 | [00269 SCALAR_CAST_TYPEOF_011](crates/bench/sqlite_parity/cases/SQLITE_PARITY_269_SCALAR_CAST_TYPEOF_011.rs) | P1 | memory | GEN_SQL_SCALAR | 2149921 | 19659500 | <span style="color:#dc2626">-814.43%</span> |
| 584 | [00368 SCALAR_ARITH_036](crates/bench/sqlite_parity/cases/SQLITE_PARITY_368_SCALAR_ARITH_036.rs) | P1 | memory | GEN_SQL_SCALAR | 2953173 | 26969303 | <span style="color:#dc2626">-813.23%</span> |
| 585 | [00334 SCALAR_STRING_027](crates/bench/sqlite_parity/cases/SQLITE_PARITY_334_SCALAR_STRING_027.rs) | P1 | memory | GEN_SQL_SCALAR | 2618318 | 23547863 | <span style="color:#dc2626">-799.35%</span> |
| 586 | [00262 SCALAR_STRING_009](crates/bench/sqlite_parity/cases/SQLITE_PARITY_262_SCALAR_STRING_009.rs) | P1 | memory | GEN_SQL_SCALAR | 2202131 | 19726807 | <span style="color:#dc2626">-795.81%</span> |
| 587 | [00254 SCALAR_STRING_007](crates/bench/sqlite_parity/cases/SQLITE_PARITY_254_SCALAR_STRING_007.rs) | P1 | memory | GEN_SQL_SCALAR | 2594573 | 23177121 | <span style="color:#dc2626">-793.29%</span> |
| 588 | [00090 JSON_MUTATION_FUNCTIONS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_090_JSON_MUTATION_FUNCTIONS.rs) | P0 | memory | SQL_JSON | 2522417 | 22492885 | <span style="color:#dc2626">-791.72%</span> |
| 589 | [00344 SCALAR_ARITH_030](crates/bench/sqlite_parity/cases/SQLITE_PARITY_344_SCALAR_ARITH_030.rs) | P1 | memory | GEN_SQL_SCALAR | 2607047 | 23088393 | <span style="color:#dc2626">-785.61%</span> |
| 590 | [00280 SCALAR_ARITH_014](crates/bench/sqlite_parity/cases/SQLITE_PARITY_280_SCALAR_ARITH_014.rs) | P1 | memory | GEN_SQL_SCALAR | 2586929 | 22761094 | <span style="color:#dc2626">-779.85%</span> |
| 591 | [00172 OPT_CMD](crates/bench/sqlite_parity/cases/SQLITE_PARITY_172_OPT_CMD.rs) | P1 | memory | CLI_OPTION | 2595595 | 22765341 | <span style="color:#dc2626">-777.08%</span> |
| 592 | [00362 SCALAR_STRING_034](crates/bench/sqlite_parity/cases/SQLITE_PARITY_362_SCALAR_STRING_034.rs) | P1 | memory | GEN_SQL_SCALAR | 2503882 | 21694313 | <span style="color:#dc2626">-766.43%</span> |
| 593 | [00177 OPT_JSON_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_177_OPT_JSON_MODE.rs) | P1 | memory | CLI_OPTION | 2591537 | 22240147 | <span style="color:#dc2626">-758.18%</span> |
| 594 | [00089 JSON_TABLE_VALUED_FUNCTIONS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_089_JSON_TABLE_VALUED_FUNCTIONS.rs) | P0 | memory | SQL_JSON | 2926842 | 24903049 | <span style="color:#dc2626">-750.85%</span> |
| 595 | [00309 SCALAR_CAST_TYPEOF_021](crates/bench/sqlite_parity/cases/SQLITE_PARITY_309_SCALAR_CAST_TYPEOF_021.rs) | P1 | memory | GEN_SQL_SCALAR | 2433078 | 20661858 | <span style="color:#dc2626">-749.21%</span> |
| 596 | [00236 SCALAR_ARITH_003](crates/bench/sqlite_parity/cases/SQLITE_PARITY_236_SCALAR_ARITH_003.rs) | P1 | memory | GEN_SQL_SCALAR | 2419943 | 20478040 | <span style="color:#dc2626">-746.22%</span> |
| 597 | [00297 SCALAR_CAST_TYPEOF_018](crates/bench/sqlite_parity/cases/SQLITE_PARITY_297_SCALAR_CAST_TYPEOF_018.rs) | P1 | memory | GEN_SQL_SCALAR | 2788210 | 23519149 | <span style="color:#dc2626">-743.52%</span> |
| 598 | [00085 CORE_RANDOM_SHAPE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_085_CORE_RANDOM_SHAPE.rs) | P0 | memory | SQL_FUNCTIONS | 2785355 | 23280417 | <span style="color:#dc2626">-735.82%</span> |
| 599 | [00252 SCALAR_ARITH_007](crates/bench/sqlite_parity/cases/SQLITE_PARITY_252_SCALAR_ARITH_007.rs) | P1 | memory | GEN_SQL_SCALAR | 2487150 | 20435569 | <span style="color:#dc2626">-721.65%</span> |
| 600 | [00234 SCALAR_STRING_002](crates/bench/sqlite_parity/cases/SQLITE_PARITY_234_SCALAR_STRING_002.rs) | P1 | memory | GEN_SQL_SCALAR | 2645129 | 21526525 | <span style="color:#dc2626">-713.82%</span> |
| 601 | [00174 OPT_CSV_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_174_OPT_CSV_MODE.rs) | P1 | memory | CLI_OPTION | 2505475 | 20250680 | <span style="color:#dc2626">-708.26%</span> |
| 602 | [00310 SCALAR_STRING_021](crates/bench/sqlite_parity/cases/SQLITE_PARITY_310_SCALAR_STRING_021.rs) | P1 | memory | GEN_SQL_SCALAR | 2879984 | 23059930 | <span style="color:#dc2626">-700.70%</span> |
| 603 | [00185 OPT_SEPARATOR](crates/bench/sqlite_parity/cases/SQLITE_PARITY_185_OPT_SEPARATOR.rs) | P1 | memory | CLI_OPTION | 2904911 | 22932267 | <span style="color:#dc2626">-689.43%</span> |
| 604 | [00284 SCALAR_ARITH_015](crates/bench/sqlite_parity/cases/SQLITE_PARITY_284_SCALAR_ARITH_015.rs) | P1 | memory | GEN_SQL_SCALAR | 2563264 | 20084755 | <span style="color:#dc2626">-683.56%</span> |
| 605 | [00265 SCALAR_CAST_TYPEOF_010](crates/bench/sqlite_parity/cases/SQLITE_PARITY_265_SCALAR_CAST_TYPEOF_010.rs) | P1 | memory | GEN_SQL_SCALAR | 2556531 | 19987471 | <span style="color:#dc2626">-681.82%</span> |
| 606 | [00312 SCALAR_ARITH_022](crates/bench/sqlite_parity/cases/SQLITE_PARITY_312_SCALAR_ARITH_022.rs) | P1 | memory | GEN_SQL_SCALAR | 2970886 | 22755022 | <span style="color:#dc2626">-665.93%</span> |
| 607 | [00218 PRAGMA_FORMS_SCHEMA_EQUALS_PARENS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_218_PRAGMA_FORMS_SCHEMA_EQUALS_PARENS.rs) | P0 | memory | SQL_PRAGMA | 2649487 | 20118409 | <span style="color:#dc2626">-659.33%</span> |
| 608 | [00354 SCALAR_STRING_032](crates/bench/sqlite_parity/cases/SQLITE_PARITY_354_SCALAR_STRING_032.rs) | P1 | memory | GEN_SQL_SCALAR | 3222733 | 24215909 | <span style="color:#dc2626">-651.41%</span> |
| 609 | [00143 DOT_QUIT](crates/bench/sqlite_parity/cases/SQLITE_PARITY_143_DOT_QUIT.rs) | P0 | memory | CLI_DOT_COMMAND | 2529100 | 18716213 | <span style="color:#dc2626">-640.03%</span> |
| 610 | [00292 SCALAR_ARITH_017](crates/bench/sqlite_parity/cases/SQLITE_PARITY_292_SCALAR_ARITH_017.rs) | P1 | memory | GEN_SQL_SCALAR | 3043854 | 22439775 | <span style="color:#dc2626">-637.22%</span> |
| 611 | [00257 SCALAR_CAST_TYPEOF_008](crates/bench/sqlite_parity/cases/SQLITE_PARITY_257_SCALAR_CAST_TYPEOF_008.rs) | P1 | memory | GEN_SQL_SCALAR | 2912545 | 20063806 | <span style="color:#dc2626">-588.88%</span> |
| 612 | [00170 OPT_VERSION](crates/bench/sqlite_parity/cases/SQLITE_PARITY_170_OPT_VERSION.rs) | P1 | memory | CLI_OPTION | 2237457 | 2415134 | <span style="color:#dc2626">-7.94%</span> |

</details>

<!-- sqlite-parity-report:end -->


The default SQLite-visible contract is documented in [docs/sqlite-parity.md](docs/sqlite-parity.md).

RedlineDB currently ships **300+ SQL-focused SQLite parity tests** across
dedicated oracle, surface, and negative-boundary suites, plus the
`parity_oracle` corpus under `crates/sql/tests/parity_corpus/`. The high-level
status is:

| Suite | Tests | Role |
|---|---:|---|
| `crates/sql/tests/parity_agg_funcs.rs` | 17 | SQLite aggregate and JSON aggregate behavior |
| `crates/sql/tests/parity_coverage.rs` | 29 | Positive SQL constructs: ALTER, DROP INDEX, RETURNING, subqueries, NULL behavior, PRAGMAs, savepoints, joins |
| `crates/sql/tests/parity_negative.rs` | 24 | Explicit error boundaries for unsupported SQL so gaps do not silently mis-execute |
| `crates/sql/tests/parity_scalar_funcs.rs` | 44 | SQLite scalar functions including `substr`, `trim`, `instr`, `replace`, `printf`, `iif`, `char`, `unicode`, blobs |
| `crates/sql/tests/parity_{cte,compound_select,window,view,trigger,attach,fk_enforce,json1,json_table,partial_index,expr_index,generated_col,operators}.rs` | 140+ | Focused rusqlite-oracle parity for implemented SQLite SQL surfaces |
| `crates/sql/tests/differential_lab.rs` | 4 | Live row-for-row differential matrices against bundled `rusqlite` |
| `crates/sql/tests/sqlite_full_parity.rs` | 3 | Reference-build metadata, representative differential coverage, and full-parity sentinels |
| `crates/sql/tests/parity_oracle.rs` | 58 | Full corpus gate across 55 SQL files plus harness self-tests |

> **Current local proof:** `rtk cargo test -p redlinedb-sql --test parity_agg_funcs --test parity_coverage --test parity_negative --test parity_scalar_funcs --test differential_lab --test sqlite_full_parity --quiet --locked`

The parity suites have **0 ignored tests**. RedlineDB is still **not full
SQLite**: native SQLite file-format compatibility, rollback-journal/WAL byte
compatibility, full reference-build PRAGMA coverage, collation completeness,
natural joins, cross-database writes, and some ALTER/UPSERT/view/trigger/CLI
edge cases remain tracked as `partial`, `fail`, `not-started`, or
`rejects-by-design` in [docs/sqlite-parity.md](docs/sqlite-parity.md). CTEs,
compound SELECT, window functions, views, triggers, generated columns,
partial/expression indexes, foreign keys, ATTACH/DETACH read paths, JSON1, and
the covered `sqlite3_*` ABI surface are no longer listed as unstarted gaps.

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
├── .jankurai/                   repository-level agent routing + proof metadata
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
- **`sqlite3_*` ABI shim** now covers the documented symbol set enforced by
  `crates/ffi/tests/symbol_diff.rs`; any deliberately excluded symbols stay in
  the allowlist instead of being counted as SQL parity passes. The shim is a
  Rust crate (`crates/ffi`), not a separate C codebase.
- **No encryption-at-rest yet.** Pages and WAL are checksummed but not encrypted. Tracked as a Phase 10 deliverable.
- **Serializable isolation** is not yet supported; we run snapshot isolation. SSI (Cahill-style) is on the future-work list.

---

## Contributing

Bug reports and patches are welcome. The proof discipline in this repo is unusually strict for a hobby database:

- Every change runs `cargo fmt --check`, `./scripts/check_file_sizes.sh`, `cargo check --workspace --locked`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, and `cargo test --workspace --locked`.
- Active source files stay under 2,000 LOC (`.jankurai/file-size-policy.toml`).
- New `unsafe` blocks outside `crates/ffi` go into `.jankurai/unsafe-ledger.toml` with reviewer sign-off.
- Bench claims must come with a manifest (`target/bench/.../manifest.json`) carrying the git SHA, image digest, host fingerprint, and per-artifact SHA-256.
- The proof ledger in `docs/WORKPLAN_slam.md` is the source of truth for any performance number quoted.
- Enable the repo-managed Git hooks with `git config core.hooksPath tools/jankurai-hooks` so the tracked pre-commit hook can block stale branches before commit.

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
