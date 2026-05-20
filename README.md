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
  <img src="https://img.shields.io/badge/version-1.0.18-blue" alt="version">
</p>

---

## Install

### Rust library

Add to `Cargo.toml`. Use an exact pin for production:

```toml
[dependencies]
redlinedb = "=1.0.18"  # exact pin — recommended for production
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
curl -LsSf https://raw.githubusercontent.com/neverhuman/RedlineDB/main/scripts/install.sh | VERSION=v1.0.18 bash
```

Fully lock the download by pinning both the release tag and the tarball digest:

```bash
curl -LsSf https://raw.githubusercontent.com/neverhuman/RedlineDB/main/scripts/install.sh | \
  VERSION=v1.0.18 REDLINEDB_SHA256=<sha256> bash
```

Custom install prefix:

```bash
curl -LsSf https://raw.githubusercontent.com/neverhuman/RedlineDB/main/scripts/install.sh | \
  VERSION=v1.0.18 PREFIX=~/.local bash
```

The script requires SHA-256 verification before installing. By default it
downloads the matching `.sha256` release asset; `REDLINEDB_SHA256` lets CI
pin the exact digest inline.

### cargo install (from source, version-pinned)

```bash
cargo install redlinedb-cli --version 1.0.18 --locked
# or from a specific git tag:
cargo install --git https://github.com/neverhuman/RedlineDB.git --tag v1.0.18 --package redlinedb-cli --locked
```

`--locked` enforces the committed `Cargo.lock` — ensures you get the exact dependency tree that was tested.

### Direct download

Pre-built tarballs on the [releases page](https://github.com/neverhuman/RedlineDB/releases):

| Platform | File |
|---|---|
| Linux x86_64 | `redlinedb-v1.0.18-linux-x86_64.tar.gz` |
| macOS Apple Silicon | `redlinedb-v1.0.18-macos-arm64.tar.gz` |
| macOS Intel | `redlinedb-v1.0.18-macos-x86_64.tar.gz` |

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
| 1 | [00968 VIEW_TRIGGER_GENERATED_021](crates/bench/sqlite_parity/cases/SQLITE_PARITY_968_VIEW_TRIGGER_GENERATED_021.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2010459 | 5086383 | <span style="color:#dc2626">-153.00%</span> |
| 2 | [00280 SCALAR_ARITH_014](crates/bench/sqlite_parity/cases/SQLITE_PARITY_280_SCALAR_ARITH_014.rs) | P1 | memory | GEN_SQL_SCALAR | 1672571 | 4211349 | <span style="color:#dc2626">-151.79%</span> |
| 3 | [00394 DML_WHERE_ORDER_LIMIT_007](crates/bench/sqlite_parity/cases/SQLITE_PARITY_394_DML_WHERE_ORDER_LIMIT_007.rs) | P1 | memory | GEN_SQL_DML | 1884241 | 4671779 | <span style="color:#dc2626">-147.94%</span> |
| 4 | [00423 DML_WHERE_ORDER_LIMIT_036](crates/bench/sqlite_parity/cases/SQLITE_PARITY_423_DML_WHERE_ORDER_LIMIT_036.rs) | P1 | memory | GEN_SQL_DML | 1890973 | 4679924 | <span style="color:#dc2626">-147.49%</span> |
| 5 | [01003 VIEW_TRIGGER_GENERATED_056](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1003_VIEW_TRIGGER_GENERATED_056.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1995802 | 4782549 | <span style="color:#dc2626">-139.63%</span> |
| 6 | [01007 VIEW_TRIGGER_GENERATED_060](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1007_VIEW_TRIGGER_GENERATED_060.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2020288 | 4783360 | <span style="color:#dc2626">-136.77%</span> |
| 7 | [00183 OPT_TABS_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_183_OPT_TABS_MODE.rs) | P2 | memory | CLI_OPTION | 1746601 | 4021751 | <span style="color:#dc2626">-130.26%</span> |
| 8 | [00418 DML_WHERE_ORDER_LIMIT_031](crates/bench/sqlite_parity/cases/SQLITE_PARITY_418_DML_WHERE_ORDER_LIMIT_031.rs) | P1 | memory | GEN_SQL_DML | 1827834 | 4204706 | <span style="color:#dc2626">-130.04%</span> |
| 9 | [00050 PRAGMA_TABLE_INFO_FUNCTION](crates/bench/sqlite_parity/cases/SQLITE_PARITY_050_PRAGMA_TABLE_INFO_FUNCTION.rs) | P0 | memory | SQL_PRAGMA | 1833064 | 4189247 | <span style="color:#dc2626">-128.54%</span> |
| 10 | [00829 WINDOW_PARTITION_SUM_042](crates/bench/sqlite_parity/cases/SQLITE_PARITY_829_WINDOW_PARTITION_SUM_042.rs) | P2 | memory | GEN_SQL_WINDOW | 1944715 | 4435522 | <span style="color:#dc2626">-128.08%</span> |
| 11 | [00017 DELETE_BASIC](crates/bench/sqlite_parity/cases/SQLITE_PARITY_017_DELETE_BASIC.rs) | P0 | memory | SQL_DELETE | 1812936 | 4122892 | <span style="color:#dc2626">-127.42%</span> |
| 12 | [00404 DML_WHERE_ORDER_LIMIT_017](crates/bench/sqlite_parity/cases/SQLITE_PARITY_404_DML_WHERE_ORDER_LIMIT_017.rs) | P1 | memory | GEN_SQL_DML | 1903177 | 4236797 | <span style="color:#dc2626">-122.62%</span> |
| 13 | [00419 DML_WHERE_ORDER_LIMIT_032](crates/bench/sqlite_parity/cases/SQLITE_PARITY_419_DML_WHERE_ORDER_LIMIT_032.rs) | P1 | memory | GEN_SQL_DML | 1910391 | 4187083 | <span style="color:#dc2626">-119.17%</span> |
| 14 | [00400 DML_WHERE_ORDER_LIMIT_013](crates/bench/sqlite_parity/cases/SQLITE_PARITY_400_DML_WHERE_ORDER_LIMIT_013.rs) | P1 | memory | GEN_SQL_DML | 1895102 | 4151386 | <span style="color:#dc2626">-119.06%</span> |
| 15 | [00276 SCALAR_ARITH_013](crates/bench/sqlite_parity/cases/SQLITE_PARITY_276_SCALAR_ARITH_013.rs) | P1 | memory | GEN_SQL_SCALAR | 1741842 | 3794110 | <span style="color:#dc2626">-117.82%</span> |
| 16 | [00686 JOIN_SUBQUERY_EXISTS_079](crates/bench/sqlite_parity/cases/SQLITE_PARITY_686_JOIN_SUBQUERY_EXISTS_079.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2003807 | 4345433 | <span style="color:#dc2626">-116.86%</span> |
| 17 | [00282 SCALAR_STRING_014](crates/bench/sqlite_parity/cases/SQLITE_PARITY_282_SCALAR_STRING_014.rs) | P1 | memory | GEN_SQL_SCALAR | 1782770 | 3865836 | <span style="color:#dc2626">-116.84%</span> |
| 18 | [00448 DML_WHERE_ORDER_LIMIT_061](crates/bench/sqlite_parity/cases/SQLITE_PARITY_448_DML_WHERE_ORDER_LIMIT_061.rs) | P1 | memory | GEN_SQL_DML | 1776708 | 3826672 | <span style="color:#dc2626">-115.38%</span> |
| 19 | [00274 SCALAR_STRING_012](crates/bench/sqlite_parity/cases/SQLITE_PARITY_274_SCALAR_STRING_012.rs) | P1 | memory | GEN_SQL_SCALAR | 2099909 | 4510905 | <span style="color:#dc2626">-114.81%</span> |
| 20 | [00415 DML_WHERE_ORDER_LIMIT_028](crates/bench/sqlite_parity/cases/SQLITE_PARITY_415_DML_WHERE_ORDER_LIMIT_028.rs) | P1 | memory | GEN_SQL_DML | 1816233 | 3839737 | <span style="color:#dc2626">-111.41%</span> |
| 21 | [00635 JOIN_SUBQUERY_EXISTS_028](crates/bench/sqlite_parity/cases/SQLITE_PARITY_635_JOIN_SUBQUERY_EXISTS_028.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2050475 | 4310917 | <span style="color:#dc2626">-110.24%</span> |
| 22 | [00241 SCALAR_CAST_TYPEOF_004](crates/bench/sqlite_parity/cases/SQLITE_PARITY_241_SCALAR_CAST_TYPEOF_004.rs) | P1 | memory | GEN_SQL_SCALAR | 1711955 | 3598821 | <span style="color:#dc2626">-110.22%</span> |
| 23 | [00395 DML_WHERE_ORDER_LIMIT_008](crates/bench/sqlite_parity/cases/SQLITE_PARITY_395_DML_WHERE_ORDER_LIMIT_008.rs) | P1 | memory | GEN_SQL_DML | 2039244 | 4279738 | <span style="color:#dc2626">-109.87%</span> |
| 24 | [00835 WINDOW_PARTITION_SUM_048](crates/bench/sqlite_parity/cases/SQLITE_PARITY_835_WINDOW_PARTITION_SUM_048.rs) | P2 | memory | GEN_SQL_WINDOW | 1986384 | 4159311 | <span style="color:#dc2626">-109.39%</span> |
| 25 | [00501 DML_WHERE_ORDER_LIMIT_114](crates/bench/sqlite_parity/cases/SQLITE_PARITY_501_DML_WHERE_ORDER_LIMIT_114.rs) | P1 | memory | GEN_SQL_DML | 1808899 | 3787558 | <span style="color:#dc2626">-109.38%</span> |
| 26 | [00495 DML_WHERE_ORDER_LIMIT_108](crates/bench/sqlite_parity/cases/SQLITE_PARITY_495_DML_WHERE_ORDER_LIMIT_108.rs) | P1 | memory | GEN_SQL_DML | 1857461 | 3873290 | <span style="color:#dc2626">-108.53%</span> |
| 27 | [00281 SCALAR_CAST_TYPEOF_014](crates/bench/sqlite_parity/cases/SQLITE_PARITY_281_SCALAR_CAST_TYPEOF_014.rs) | P1 | memory | GEN_SQL_SCALAR | 1675637 | 3483634 | <span style="color:#dc2626">-107.90%</span> |
| 28 | [00016 UPDATE_RETURNING](crates/bench/sqlite_parity/cases/SQLITE_PARITY_016_UPDATE_RETURNING.rs) | P0 | memory | SQL_UPDATE | 1863923 | 3873210 | <span style="color:#dc2626">-107.80%</span> |
| 29 | [00469 DML_WHERE_ORDER_LIMIT_082](crates/bench/sqlite_parity/cases/SQLITE_PARITY_469_DML_WHERE_ORDER_LIMIT_082.rs) | P1 | memory | GEN_SQL_DML | 1846740 | 3830008 | <span style="color:#dc2626">-107.39%</span> |
| 30 | [00465 DML_WHERE_ORDER_LIMIT_078](crates/bench/sqlite_parity/cases/SQLITE_PARITY_465_DML_WHERE_ORDER_LIMIT_078.rs) | P1 | memory | GEN_SQL_DML | 1868070 | 3872628 | <span style="color:#dc2626">-107.31%</span> |
| 31 | [00417 DML_WHERE_ORDER_LIMIT_030](crates/bench/sqlite_parity/cases/SQLITE_PARITY_417_DML_WHERE_ORDER_LIMIT_030.rs) | P1 | memory | GEN_SQL_DML | 1876566 | 3888869 | <span style="color:#dc2626">-107.23%</span> |
| 32 | [00467 DML_WHERE_ORDER_LIMIT_080](crates/bench/sqlite_parity/cases/SQLITE_PARITY_467_DML_WHERE_ORDER_LIMIT_080.rs) | P1 | memory | GEN_SQL_DML | 1836681 | 3801655 | <span style="color:#dc2626">-106.99%</span> |
| 33 | [00408 DML_WHERE_ORDER_LIMIT_021](crates/bench/sqlite_parity/cases/SQLITE_PARITY_408_DML_WHERE_ORDER_LIMIT_021.rs) | P1 | memory | GEN_SQL_DML | 1918886 | 3967287 | <span style="color:#dc2626">-106.75%</span> |
| 34 | [00452 DML_WHERE_ORDER_LIMIT_065](crates/bench/sqlite_parity/cases/SQLITE_PARITY_452_DML_WHERE_ORDER_LIMIT_065.rs) | P1 | memory | GEN_SQL_DML | 1837272 | 3795403 | <span style="color:#dc2626">-106.58%</span> |
| 35 | [00477 DML_WHERE_ORDER_LIMIT_090](crates/bench/sqlite_parity/cases/SQLITE_PARITY_477_DML_WHERE_ORDER_LIMIT_090.rs) | P1 | memory | GEN_SQL_DML | 1853833 | 3828605 | <span style="color:#dc2626">-106.52%</span> |
| 36 | [00425 DML_WHERE_ORDER_LIMIT_038](crates/bench/sqlite_parity/cases/SQLITE_PARITY_425_DML_WHERE_ORDER_LIMIT_038.rs) | P1 | memory | GEN_SQL_DML | 1871517 | 3855507 | <span style="color:#dc2626">-106.01%</span> |
| 37 | [00393 DML_WHERE_ORDER_LIMIT_006](crates/bench/sqlite_parity/cases/SQLITE_PARITY_393_DML_WHERE_ORDER_LIMIT_006.rs) | P1 | memory | GEN_SQL_DML | 1952310 | 4018995 | <span style="color:#dc2626">-105.86%</span> |
| 38 | [00443 DML_WHERE_ORDER_LIMIT_056](crates/bench/sqlite_parity/cases/SQLITE_PARITY_443_DML_WHERE_ORDER_LIMIT_056.rs) | P1 | memory | GEN_SQL_DML | 1809119 | 3723737 | <span style="color:#dc2626">-105.83%</span> |
| 39 | [00817 WINDOW_PARTITION_SUM_030](crates/bench/sqlite_parity/cases/SQLITE_PARITY_817_WINDOW_PARTITION_SUM_030.rs) | P2 | memory | GEN_SQL_WINDOW | 1967298 | 4048451 | <span style="color:#dc2626">-105.79%</span> |
| 40 | [00420 DML_WHERE_ORDER_LIMIT_033](crates/bench/sqlite_parity/cases/SQLITE_PARITY_420_DML_WHERE_ORDER_LIMIT_033.rs) | P1 | memory | GEN_SQL_DML | 1904058 | 3916462 | <span style="color:#dc2626">-105.69%</span> |
| 41 | [00411 DML_WHERE_ORDER_LIMIT_024](crates/bench/sqlite_parity/cases/SQLITE_PARITY_411_DML_WHERE_ORDER_LIMIT_024.rs) | P1 | memory | GEN_SQL_DML | 1849976 | 3802646 | <span style="color:#dc2626">-105.55%</span> |
| 42 | [00470 DML_WHERE_ORDER_LIMIT_083](crates/bench/sqlite_parity/cases/SQLITE_PARITY_470_DML_WHERE_ORDER_LIMIT_083.rs) | P1 | memory | GEN_SQL_DML | 1867679 | 3838224 | <span style="color:#dc2626">-105.51%</span> |
| 43 | [00445 DML_WHERE_ORDER_LIMIT_058](crates/bench/sqlite_parity/cases/SQLITE_PARITY_445_DML_WHERE_ORDER_LIMIT_058.rs) | P1 | memory | GEN_SQL_DML | 1870465 | 3843714 | <span style="color:#dc2626">-105.50%</span> |
| 44 | [00406 DML_WHERE_ORDER_LIMIT_019](crates/bench/sqlite_parity/cases/SQLITE_PARITY_406_DML_WHERE_ORDER_LIMIT_019.rs) | P1 | memory | GEN_SQL_DML | 1866968 | 3834246 | <span style="color:#dc2626">-105.37%</span> |
| 45 | [00494 DML_WHERE_ORDER_LIMIT_107](crates/bench/sqlite_parity/cases/SQLITE_PARITY_494_DML_WHERE_ORDER_LIMIT_107.rs) | P1 | memory | GEN_SQL_DML | 1887076 | 3873110 | <span style="color:#dc2626">-105.24%</span> |
| 46 | [00427 DML_WHERE_ORDER_LIMIT_040](crates/bench/sqlite_parity/cases/SQLITE_PARITY_427_DML_WHERE_ORDER_LIMIT_040.rs) | P1 | memory | GEN_SQL_DML | 1816432 | 3727865 | <span style="color:#dc2626">-105.23%</span> |
| 47 | [00001 SELECT_CORE_EXPRESSIONS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_001_SELECT_CORE_EXPRESSIONS.rs) | P0 | memory | SQL_SELECT | 1839727 | 3774613 | <span style="color:#dc2626">-105.17%</span> |
| 48 | [00438 DML_WHERE_ORDER_LIMIT_051](crates/bench/sqlite_parity/cases/SQLITE_PARITY_438_DML_WHERE_ORDER_LIMIT_051.rs) | P1 | memory | GEN_SQL_DML | 1832303 | 3758814 | <span style="color:#dc2626">-105.14%</span> |
| 49 | [00388 DML_WHERE_ORDER_LIMIT_001](crates/bench/sqlite_parity/cases/SQLITE_PARITY_388_DML_WHERE_ORDER_LIMIT_001.rs) | P1 | memory | GEN_SQL_DML | 1901163 | 3895441 | <span style="color:#dc2626">-104.90%</span> |
| 50 | [00506 DML_WHERE_ORDER_LIMIT_119](crates/bench/sqlite_parity/cases/SQLITE_PARITY_506_DML_WHERE_ORDER_LIMIT_119.rs) | P1 | memory | GEN_SQL_DML | 1833575 | 3756560 | <span style="color:#dc2626">-104.88%</span> |
| 51 | [00392 DML_WHERE_ORDER_LIMIT_005](crates/bench/sqlite_parity/cases/SQLITE_PARITY_392_DML_WHERE_ORDER_LIMIT_005.rs) | P1 | memory | GEN_SQL_DML | 1866748 | 3823145 | <span style="color:#dc2626">-104.80%</span> |
| 52 | [00481 DML_WHERE_ORDER_LIMIT_094](crates/bench/sqlite_parity/cases/SQLITE_PARITY_481_DML_WHERE_ORDER_LIMIT_094.rs) | P1 | memory | GEN_SQL_DML | 1856548 | 3800743 | <span style="color:#dc2626">-104.72%</span> |
| 53 | [00956 VIEW_TRIGGER_GENERATED_009](crates/bench/sqlite_parity/cases/SQLITE_PARITY_956_VIEW_TRIGGER_GENERATED_009.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1987056 | 4067808 | <span style="color:#dc2626">-104.72%</span> |
| 54 | [00478 DML_WHERE_ORDER_LIMIT_091](crates/bench/sqlite_parity/cases/SQLITE_PARITY_478_DML_WHERE_ORDER_LIMIT_091.rs) | P1 | memory | GEN_SQL_DML | 1868642 | 3824838 | <span style="color:#dc2626">-104.69%</span> |
| 55 | [00431 DML_WHERE_ORDER_LIMIT_044](crates/bench/sqlite_parity/cases/SQLITE_PARITY_431_DML_WHERE_ORDER_LIMIT_044.rs) | P1 | memory | GEN_SQL_DML | 1857500 | 3799230 | <span style="color:#dc2626">-104.53%</span> |
| 56 | [00831 WINDOW_PARTITION_SUM_044](crates/bench/sqlite_parity/cases/SQLITE_PARITY_831_WINDOW_PARTITION_SUM_044.rs) | P2 | memory | GEN_SQL_WINDOW | 1867569 | 3818947 | <span style="color:#dc2626">-104.49%</span> |
| 57 | [00389 DML_WHERE_ORDER_LIMIT_002](crates/bench/sqlite_parity/cases/SQLITE_PARITY_389_DML_WHERE_ORDER_LIMIT_002.rs) | P1 | memory | GEN_SQL_DML | 1864814 | 3812244 | <span style="color:#dc2626">-104.43%</span> |
| 58 | [00492 DML_WHERE_ORDER_LIMIT_105](crates/bench/sqlite_parity/cases/SQLITE_PARITY_492_DML_WHERE_ORDER_LIMIT_105.rs) | P1 | memory | GEN_SQL_DML | 1896725 | 3875644 | <span style="color:#dc2626">-104.33%</span> |
| 59 | [00412 DML_WHERE_ORDER_LIMIT_025](crates/bench/sqlite_parity/cases/SQLITE_PARITY_412_DML_WHERE_ORDER_LIMIT_025.rs) | P1 | memory | GEN_SQL_DML | 1894089 | 3867599 | <span style="color:#dc2626">-104.19%</span> |
| 60 | [00424 DML_WHERE_ORDER_LIMIT_037](crates/bench/sqlite_parity/cases/SQLITE_PARITY_424_DML_WHERE_ORDER_LIMIT_037.rs) | P1 | memory | GEN_SQL_DML | 1932291 | 3943432 | <span style="color:#dc2626">-104.08%</span> |
| 61 | [00474 DML_WHERE_ORDER_LIMIT_087](crates/bench/sqlite_parity/cases/SQLITE_PARITY_474_DML_WHERE_ORDER_LIMIT_087.rs) | P1 | memory | GEN_SQL_DML | 1837633 | 3748053 | <span style="color:#dc2626">-103.96%</span> |
| 62 | [00390 DML_WHERE_ORDER_LIMIT_003](crates/bench/sqlite_parity/cases/SQLITE_PARITY_390_DML_WHERE_ORDER_LIMIT_003.rs) | P1 | memory | GEN_SQL_DML | 1834738 | 3741441 | <span style="color:#dc2626">-103.92%</span> |
| 63 | [00503 DML_WHERE_ORDER_LIMIT_116](crates/bench/sqlite_parity/cases/SQLITE_PARITY_503_DML_WHERE_ORDER_LIMIT_116.rs) | P1 | memory | GEN_SQL_DML | 1886345 | 3846299 | <span style="color:#dc2626">-103.90%</span> |
| 64 | [00678 JOIN_SUBQUERY_EXISTS_071](crates/bench/sqlite_parity/cases/SQLITE_PARITY_678_JOIN_SUBQUERY_EXISTS_071.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1910861 | 3893779 | <span style="color:#dc2626">-103.77%</span> |
| 65 | [00409 DML_WHERE_ORDER_LIMIT_022](crates/bench/sqlite_parity/cases/SQLITE_PARITY_409_DML_WHERE_ORDER_LIMIT_022.rs) | P1 | memory | GEN_SQL_DML | 1898498 | 3858973 | <span style="color:#dc2626">-103.26%</span> |
| 66 | [00500 DML_WHERE_ORDER_LIMIT_113](crates/bench/sqlite_parity/cases/SQLITE_PARITY_500_DML_WHERE_ORDER_LIMIT_113.rs) | P1 | memory | GEN_SQL_DML | 1889901 | 3840959 | <span style="color:#dc2626">-103.24%</span> |
| 67 | [00433 DML_WHERE_ORDER_LIMIT_046](crates/bench/sqlite_parity/cases/SQLITE_PARITY_433_DML_WHERE_ORDER_LIMIT_046.rs) | P1 | memory | GEN_SQL_DML | 1837082 | 3731683 | <span style="color:#dc2626">-103.13%</span> |
| 68 | [00414 DML_WHERE_ORDER_LIMIT_027](crates/bench/sqlite_parity/cases/SQLITE_PARITY_414_DML_WHERE_ORDER_LIMIT_027.rs) | P1 | memory | GEN_SQL_DML | 1822855 | 3699181 | <span style="color:#dc2626">-102.93%</span> |
| 69 | [00455 DML_WHERE_ORDER_LIMIT_068](crates/bench/sqlite_parity/cases/SQLITE_PARITY_455_DML_WHERE_ORDER_LIMIT_068.rs) | P1 | memory | GEN_SQL_DML | 1953482 | 3964212 | <span style="color:#dc2626">-102.93%</span> |
| 70 | [00954 VIEW_TRIGGER_GENERATED_007](crates/bench/sqlite_parity/cases/SQLITE_PARITY_954_VIEW_TRIGGER_GENERATED_007.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1967198 | 3991634 | <span style="color:#dc2626">-102.91%</span> |
| 71 | [00969 VIEW_TRIGGER_GENERATED_022](crates/bench/sqlite_parity/cases/SQLITE_PARITY_969_VIEW_TRIGGER_GENERATED_022.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1995832 | 4045095 | <span style="color:#dc2626">-102.68%</span> |
| 72 | [00483 DML_WHERE_ORDER_LIMIT_096](crates/bench/sqlite_parity/cases/SQLITE_PARITY_483_DML_WHERE_ORDER_LIMIT_096.rs) | P1 | memory | GEN_SQL_DML | 1859224 | 3767349 | <span style="color:#dc2626">-102.63%</span> |
| 73 | [00444 DML_WHERE_ORDER_LIMIT_057](crates/bench/sqlite_parity/cases/SQLITE_PARITY_444_DML_WHERE_ORDER_LIMIT_057.rs) | P1 | memory | GEN_SQL_DML | 1842903 | 3731562 | <span style="color:#dc2626">-102.48%</span> |
| 74 | [00975 VIEW_TRIGGER_GENERATED_028](crates/bench/sqlite_parity/cases/SQLITE_PARITY_975_VIEW_TRIGGER_GENERATED_028.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1945257 | 3936018 | <span style="color:#dc2626">-102.34%</span> |
| 75 | [00459 DML_WHERE_ORDER_LIMIT_072](crates/bench/sqlite_parity/cases/SQLITE_PARITY_459_DML_WHERE_ORDER_LIMIT_072.rs) | P1 | memory | GEN_SQL_DML | 1959724 | 3964422 | <span style="color:#dc2626">-102.29%</span> |
| 76 | [00496 DML_WHERE_ORDER_LIMIT_109](crates/bench/sqlite_parity/cases/SQLITE_PARITY_496_DML_WHERE_ORDER_LIMIT_109.rs) | P1 | memory | GEN_SQL_DML | 1876887 | 3795363 | <span style="color:#dc2626">-102.22%</span> |
| 77 | [00111 DOT_HEADERS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_111_DOT_HEADERS.rs) | P0 | memory | CLI_DOT_COMMAND | 1820841 | 3681588 | <span style="color:#dc2626">-102.19%</span> |
| 78 | [00487 DML_WHERE_ORDER_LIMIT_100](crates/bench/sqlite_parity/cases/SQLITE_PARITY_487_DML_WHERE_ORDER_LIMIT_100.rs) | P1 | memory | GEN_SQL_DML | 1866287 | 3772089 | <span style="color:#dc2626">-102.12%</span> |
| 79 | [00631 JOIN_SUBQUERY_EXISTS_024](crates/bench/sqlite_parity/cases/SQLITE_PARITY_631_JOIN_SUBQUERY_EXISTS_024.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1941620 | 3924136 | <span style="color:#dc2626">-102.11%</span> |
| 80 | [00948 VIEW_TRIGGER_GENERATED_001](crates/bench/sqlite_parity/cases/SQLITE_PARITY_948_VIEW_TRIGGER_GENERATED_001.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1937642 | 3914639 | <span style="color:#dc2626">-102.03%</span> |
| 81 | [00442 DML_WHERE_ORDER_LIMIT_055](crates/bench/sqlite_parity/cases/SQLITE_PARITY_442_DML_WHERE_ORDER_LIMIT_055.rs) | P1 | memory | GEN_SQL_DML | 1839296 | 3715642 | <span style="color:#dc2626">-102.01%</span> |
| 82 | [00462 DML_WHERE_ORDER_LIMIT_075](crates/bench/sqlite_parity/cases/SQLITE_PARITY_462_DML_WHERE_ORDER_LIMIT_075.rs) | P1 | memory | GEN_SQL_DML | 1953802 | 3946849 | <span style="color:#dc2626">-102.01%</span> |
| 83 | [00435 DML_WHERE_ORDER_LIMIT_048](crates/bench/sqlite_parity/cases/SQLITE_PARITY_435_DML_WHERE_ORDER_LIMIT_048.rs) | P1 | memory | GEN_SQL_DML | 1887798 | 3810341 | <span style="color:#dc2626">-101.84%</span> |
| 84 | [00485 DML_WHERE_ORDER_LIMIT_098](crates/bench/sqlite_parity/cases/SQLITE_PARITY_485_DML_WHERE_ORDER_LIMIT_098.rs) | P1 | memory | GEN_SQL_DML | 1908016 | 3846359 | <span style="color:#dc2626">-101.59%</span> |
| 85 | [00457 DML_WHERE_ORDER_LIMIT_070](crates/bench/sqlite_parity/cases/SQLITE_PARITY_457_DML_WHERE_ORDER_LIMIT_070.rs) | P1 | memory | GEN_SQL_DML | 1903437 | 3835879 | <span style="color:#dc2626">-101.52%</span> |
| 86 | [00614 JOIN_SUBQUERY_EXISTS_007](crates/bench/sqlite_parity/cases/SQLITE_PARITY_614_JOIN_SUBQUERY_EXISTS_007.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1911433 | 3848984 | <span style="color:#dc2626">-101.37%</span> |
| 87 | [00961 VIEW_TRIGGER_GENERATED_014](crates/bench/sqlite_parity/cases/SQLITE_PARITY_961_VIEW_TRIGGER_GENERATED_014.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1930369 | 3886666 | <span style="color:#dc2626">-101.34%</span> |
| 88 | [00480 DML_WHERE_ORDER_LIMIT_093](crates/bench/sqlite_parity/cases/SQLITE_PARITY_480_DML_WHERE_ORDER_LIMIT_093.rs) | P1 | memory | GEN_SQL_DML | 1878220 | 3781487 | <span style="color:#dc2626">-101.33%</span> |
| 89 | [00983 VIEW_TRIGGER_GENERATED_036](crates/bench/sqlite_parity/cases/SQLITE_PARITY_983_VIEW_TRIGGER_GENERATED_036.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1982557 | 3984139 | <span style="color:#dc2626">-100.96%</span> |
| 90 | [00984 VIEW_TRIGGER_GENERATED_037](crates/bench/sqlite_parity/cases/SQLITE_PARITY_984_VIEW_TRIGGER_GENERATED_037.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1995171 | 4008516 | <span style="color:#dc2626">-100.91%</span> |
| 91 | [00977 VIEW_TRIGGER_GENERATED_030](crates/bench/sqlite_parity/cases/SQLITE_PARITY_977_VIEW_TRIGGER_GENERATED_030.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1935248 | 3887447 | <span style="color:#dc2626">-100.88%</span> |
| 92 | [00699 JOIN_SUBQUERY_EXISTS_092](crates/bench/sqlite_parity/cases/SQLITE_PARITY_699_JOIN_SUBQUERY_EXISTS_092.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1939857 | 3896173 | <span style="color:#dc2626">-100.85%</span> |
| 93 | [00432 DML_WHERE_ORDER_LIMIT_045](crates/bench/sqlite_parity/cases/SQLITE_PARITY_432_DML_WHERE_ORDER_LIMIT_045.rs) | P1 | memory | GEN_SQL_DML | 1852190 | 3718477 | <span style="color:#dc2626">-100.76%</span> |
| 94 | [00398 DML_WHERE_ORDER_LIMIT_011](crates/bench/sqlite_parity/cases/SQLITE_PARITY_398_DML_WHERE_ORDER_LIMIT_011.rs) | P1 | memory | GEN_SQL_DML | 1941439 | 3897456 | <span style="color:#dc2626">-100.75%</span> |
| 95 | [00428 DML_WHERE_ORDER_LIMIT_041](crates/bench/sqlite_parity/cases/SQLITE_PARITY_428_DML_WHERE_ORDER_LIMIT_041.rs) | P1 | memory | GEN_SQL_DML | 1895362 | 3802617 | <span style="color:#dc2626">-100.63%</span> |
| 96 | [00004 TABLE_CONSTRAINTS_SUCCESS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_004_TABLE_CONSTRAINTS_SUCCESS.rs) | P0 | memory | SQL_CONSTRAINTS | 1798148 | 3603570 | <span style="color:#dc2626">-100.40%</span> |
| 97 | [00974 VIEW_TRIGGER_GENERATED_027](crates/bench/sqlite_parity/cases/SQLITE_PARITY_974_VIEW_TRIGGER_GENERATED_027.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1944725 | 3895612 | <span style="color:#dc2626">-100.32%</span> |
| 98 | [00634 JOIN_SUBQUERY_EXISTS_027](crates/bench/sqlite_parity/cases/SQLITE_PARITY_634_JOIN_SUBQUERY_EXISTS_027.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1902285 | 3808578 | <span style="color:#dc2626">-100.21%</span> |
| 99 | [00454 DML_WHERE_ORDER_LIMIT_067](crates/bench/sqlite_parity/cases/SQLITE_PARITY_454_DML_WHERE_ORDER_LIMIT_067.rs) | P1 | memory | GEN_SQL_DML | 1862991 | 3729859 | <span style="color:#dc2626">-100.21%</span> |
| 100 | [00364 SCALAR_ARITH_035](crates/bench/sqlite_parity/cases/SQLITE_PARITY_364_SCALAR_ARITH_035.rs) | P1 | memory | GEN_SQL_SCALAR | 1704612 | 3409012 | <span style="color:#dc2626">-99.99%</span> |
| 101 | [00964 VIEW_TRIGGER_GENERATED_017](crates/bench/sqlite_parity/cases/SQLITE_PARITY_964_VIEW_TRIGGER_GENERATED_017.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1948412 | 3895862 | <span style="color:#dc2626">-99.95%</span> |
| 102 | [00675 JOIN_SUBQUERY_EXISTS_068](crates/bench/sqlite_parity/cases/SQLITE_PARITY_675_JOIN_SUBQUERY_EXISTS_068.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1930499 | 3859414 | <span style="color:#dc2626">-99.92%</span> |
| 103 | [00654 JOIN_SUBQUERY_EXISTS_047](crates/bench/sqlite_parity/cases/SQLITE_PARITY_654_JOIN_SUBQUERY_EXISTS_047.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1955666 | 3907865 | <span style="color:#dc2626">-99.82%</span> |
| 104 | [00489 DML_WHERE_ORDER_LIMIT_102](crates/bench/sqlite_parity/cases/SQLITE_PARITY_489_DML_WHERE_ORDER_LIMIT_102.rs) | P1 | memory | GEN_SQL_DML | 1896875 | 3790093 | <span style="color:#dc2626">-99.81%</span> |
| 105 | [00685 JOIN_SUBQUERY_EXISTS_078](crates/bench/sqlite_parity/cases/SQLITE_PARITY_685_JOIN_SUBQUERY_EXISTS_078.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1950516 | 3894140 | <span style="color:#dc2626">-99.65%</span> |
| 106 | [00625 JOIN_SUBQUERY_EXISTS_018](crates/bench/sqlite_parity/cases/SQLITE_PARITY_625_JOIN_SUBQUERY_EXISTS_018.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1890513 | 3774113 | <span style="color:#dc2626">-99.63%</span> |
| 107 | [00952 VIEW_TRIGGER_GENERATED_005](crates/bench/sqlite_parity/cases/SQLITE_PARITY_952_VIEW_TRIGGER_GENERATED_005.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2021590 | 4032421 | <span style="color:#dc2626">-99.47%</span> |
| 108 | [00857 WINDOW_PARTITION_SUM_070](crates/bench/sqlite_parity/cases/SQLITE_PARITY_857_WINDOW_PARTITION_SUM_070.rs) | P2 | memory | GEN_SQL_WINDOW | 1895973 | 3781216 | <span style="color:#dc2626">-99.43%</span> |
| 109 | [00493 DML_WHERE_ORDER_LIMIT_106](crates/bench/sqlite_parity/cases/SQLITE_PARITY_493_DML_WHERE_ORDER_LIMIT_106.rs) | P1 | memory | GEN_SQL_DML | 1936369 | 3854074 | <span style="color:#dc2626">-99.04%</span> |
| 110 | [00497 DML_WHERE_ORDER_LIMIT_110](crates/bench/sqlite_parity/cases/SQLITE_PARITY_497_DML_WHERE_ORDER_LIMIT_110.rs) | P1 | memory | GEN_SQL_DML | 1910661 | 3802236 | <span style="color:#dc2626">-99.00%</span> |
| 111 | [00620 JOIN_SUBQUERY_EXISTS_013](crates/bench/sqlite_parity/cases/SQLITE_PARITY_620_JOIN_SUBQUERY_EXISTS_013.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1922363 | 3825389 | <span style="color:#dc2626">-98.99%</span> |
| 112 | [00698 JOIN_SUBQUERY_EXISTS_091](crates/bench/sqlite_parity/cases/SQLITE_PARITY_698_JOIN_SUBQUERY_EXISTS_091.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1943573 | 3866908 | <span style="color:#dc2626">-98.96%</span> |
| 113 | [00985 VIEW_TRIGGER_GENERATED_038](crates/bench/sqlite_parity/cases/SQLITE_PARITY_985_VIEW_TRIGGER_GENERATED_038.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1958602 | 3896374 | <span style="color:#dc2626">-98.94%</span> |
| 114 | [00608 JOIN_SUBQUERY_EXISTS_001](crates/bench/sqlite_parity/cases/SQLITE_PARITY_608_JOIN_SUBQUERY_EXISTS_001.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1912775 | 3802927 | <span style="color:#dc2626">-98.82%</span> |
| 115 | [00621 JOIN_SUBQUERY_EXISTS_014](crates/bench/sqlite_parity/cases/SQLITE_PARITY_621_JOIN_SUBQUERY_EXISTS_014.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1888780 | 3754967 | <span style="color:#dc2626">-98.80%</span> |
| 116 | [00430 DML_WHERE_ORDER_LIMIT_043](crates/bench/sqlite_parity/cases/SQLITE_PARITY_430_DML_WHERE_ORDER_LIMIT_043.rs) | P1 | memory | GEN_SQL_DML | 1887167 | 3748765 | <span style="color:#dc2626">-98.65%</span> |
| 117 | [00988 VIEW_TRIGGER_GENERATED_041](crates/bench/sqlite_parity/cases/SQLITE_PARITY_988_VIEW_TRIGGER_GENERATED_041.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1988618 | 3950015 | <span style="color:#dc2626">-98.63%</span> |
| 118 | [00616 JOIN_SUBQUERY_EXISTS_009](crates/bench/sqlite_parity/cases/SQLITE_PARITY_616_JOIN_SUBQUERY_EXISTS_009.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1915580 | 3803949 | <span style="color:#dc2626">-98.58%</span> |
| 119 | [00696 JOIN_SUBQUERY_EXISTS_089](crates/bench/sqlite_parity/cases/SQLITE_PARITY_696_JOIN_SUBQUERY_EXISTS_089.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1916963 | 3806163 | <span style="color:#dc2626">-98.55%</span> |
| 120 | [00946 CONSTRAINT_FK_SAVEPOINT_079](crates/bench/sqlite_parity/cases/SQLITE_PARITY_946_CONSTRAINT_FK_SAVEPOINT_079.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1977557 | 3926050 | <span style="color:#dc2626">-98.53%</span> |
| 121 | [00650 JOIN_SUBQUERY_EXISTS_043](crates/bench/sqlite_parity/cases/SQLITE_PARITY_650_JOIN_SUBQUERY_EXISTS_043.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1908728 | 3787839 | <span style="color:#dc2626">-98.45%</span> |
| 122 | [00636 JOIN_SUBQUERY_EXISTS_029](crates/bench/sqlite_parity/cases/SQLITE_PARITY_636_JOIN_SUBQUERY_EXISTS_029.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1913637 | 3797457 | <span style="color:#dc2626">-98.44%</span> |
| 123 | [00707 JOIN_SUBQUERY_EXISTS_100](crates/bench/sqlite_parity/cases/SQLITE_PARITY_707_JOIN_SUBQUERY_EXISTS_100.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1937541 | 3844035 | <span style="color:#dc2626">-98.40%</span> |
| 124 | [00655 JOIN_SUBQUERY_EXISTS_048](crates/bench/sqlite_parity/cases/SQLITE_PARITY_655_JOIN_SUBQUERY_EXISTS_048.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1943443 | 3855126 | <span style="color:#dc2626">-98.37%</span> |
| 125 | [00995 VIEW_TRIGGER_GENERATED_048](crates/bench/sqlite_parity/cases/SQLITE_PARITY_995_VIEW_TRIGGER_GENERATED_048.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1997225 | 3961767 | <span style="color:#dc2626">-98.36%</span> |
| 126 | [00453 DML_WHERE_ORDER_LIMIT_066](crates/bench/sqlite_parity/cases/SQLITE_PARITY_453_DML_WHERE_ORDER_LIMIT_066.rs) | P1 | memory | GEN_SQL_DML | 1911322 | 3790945 | <span style="color:#dc2626">-98.34%</span> |
| 127 | [00639 JOIN_SUBQUERY_EXISTS_032](crates/bench/sqlite_parity/cases/SQLITE_PARITY_639_JOIN_SUBQUERY_EXISTS_032.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1925759 | 3817445 | <span style="color:#dc2626">-98.23%</span> |
| 128 | [00704 JOIN_SUBQUERY_EXISTS_097](crates/bench/sqlite_parity/cases/SQLITE_PARITY_704_JOIN_SUBQUERY_EXISTS_097.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1946308 | 3857691 | <span style="color:#dc2626">-98.21%</span> |
| 129 | [00472 DML_WHERE_ORDER_LIMIT_085](crates/bench/sqlite_parity/cases/SQLITE_PARITY_472_DML_WHERE_ORDER_LIMIT_085.rs) | P1 | memory | GEN_SQL_DML | 1888740 | 3741842 | <span style="color:#dc2626">-98.11%</span> |
| 130 | [00475 DML_WHERE_ORDER_LIMIT_088](crates/bench/sqlite_parity/cases/SQLITE_PARITY_475_DML_WHERE_ORDER_LIMIT_088.rs) | P1 | memory | GEN_SQL_DML | 1885964 | 3735580 | <span style="color:#dc2626">-98.07%</span> |
| 131 | [00987 VIEW_TRIGGER_GENERATED_040](crates/bench/sqlite_parity/cases/SQLITE_PARITY_987_VIEW_TRIGGER_GENERATED_040.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1972328 | 3905491 | <span style="color:#dc2626">-98.01%</span> |
| 132 | [00507 DML_WHERE_ORDER_LIMIT_120](crates/bench/sqlite_parity/cases/SQLITE_PARITY_507_DML_WHERE_ORDER_LIMIT_120.rs) | P1 | memory | GEN_SQL_DML | 1929717 | 3820921 | <span style="color:#dc2626">-98.00%</span> |
| 133 | [00450 DML_WHERE_ORDER_LIMIT_063](crates/bench/sqlite_parity/cases/SQLITE_PARITY_450_DML_WHERE_ORDER_LIMIT_063.rs) | P1 | memory | GEN_SQL_DML | 1902976 | 3765797 | <span style="color:#dc2626">-97.89%</span> |
| 134 | [00456 DML_WHERE_ORDER_LIMIT_069](crates/bench/sqlite_parity/cases/SQLITE_PARITY_456_DML_WHERE_ORDER_LIMIT_069.rs) | P1 | memory | GEN_SQL_DML | 1966767 | 3889992 | <span style="color:#dc2626">-97.79%</span> |
| 135 | [00436 DML_WHERE_ORDER_LIMIT_049](crates/bench/sqlite_parity/cases/SQLITE_PARITY_436_DML_WHERE_ORDER_LIMIT_049.rs) | P1 | memory | GEN_SQL_DML | 1885574 | 3729048 | <span style="color:#dc2626">-97.77%</span> |
| 136 | [00965 VIEW_TRIGGER_GENERATED_018](crates/bench/sqlite_parity/cases/SQLITE_PARITY_965_VIEW_TRIGGER_GENERATED_018.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1989950 | 3934446 | <span style="color:#dc2626">-97.72%</span> |
| 137 | [00437 DML_WHERE_ORDER_LIMIT_050](crates/bench/sqlite_parity/cases/SQLITE_PARITY_437_DML_WHERE_ORDER_LIMIT_050.rs) | P1 | memory | GEN_SQL_DML | 1877799 | 3712516 | <span style="color:#dc2626">-97.71%</span> |
| 138 | [00683 JOIN_SUBQUERY_EXISTS_076](crates/bench/sqlite_parity/cases/SQLITE_PARITY_683_JOIN_SUBQUERY_EXISTS_076.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1930558 | 3816323 | <span style="color:#dc2626">-97.68%</span> |
| 139 | [00846 WINDOW_PARTITION_SUM_059](crates/bench/sqlite_parity/cases/SQLITE_PARITY_846_WINDOW_PARTITION_SUM_059.rs) | P2 | memory | GEN_SQL_WINDOW | 1952831 | 3860346 | <span style="color:#dc2626">-97.68%</span> |
| 140 | [00992 VIEW_TRIGGER_GENERATED_045](crates/bench/sqlite_parity/cases/SQLITE_PARITY_992_VIEW_TRIGGER_GENERATED_045.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2004018 | 3961107 | <span style="color:#dc2626">-97.66%</span> |
| 141 | [00611 JOIN_SUBQUERY_EXISTS_004](crates/bench/sqlite_parity/cases/SQLITE_PARITY_611_JOIN_SUBQUERY_EXISTS_004.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1982667 | 3918676 | <span style="color:#dc2626">-97.65%</span> |
| 142 | [00963 VIEW_TRIGGER_GENERATED_016](crates/bench/sqlite_parity/cases/SQLITE_PARITY_963_VIEW_TRIGGER_GENERATED_016.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2071315 | 4091783 | <span style="color:#dc2626">-97.55%</span> |
| 143 | [00505 DML_WHERE_ORDER_LIMIT_118](crates/bench/sqlite_parity/cases/SQLITE_PARITY_505_DML_WHERE_ORDER_LIMIT_118.rs) | P1 | memory | GEN_SQL_DML | 1929697 | 3811474 | <span style="color:#dc2626">-97.52%</span> |
| 144 | [00458 DML_WHERE_ORDER_LIMIT_071](crates/bench/sqlite_parity/cases/SQLITE_PARITY_458_DML_WHERE_ORDER_LIMIT_071.rs) | P1 | memory | GEN_SQL_DML | 1917835 | 3787979 | <span style="color:#dc2626">-97.51%</span> |
| 145 | [00657 JOIN_SUBQUERY_EXISTS_050](crates/bench/sqlite_parity/cases/SQLITE_PARITY_657_JOIN_SUBQUERY_EXISTS_050.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1942942 | 3836902 | <span style="color:#dc2626">-97.48%</span> |
| 146 | [00441 DML_WHERE_ORDER_LIMIT_054](crates/bench/sqlite_parity/cases/SQLITE_PARITY_441_DML_WHERE_ORDER_LIMIT_054.rs) | P1 | memory | GEN_SQL_DML | 1906323 | 3764134 | <span style="color:#dc2626">-97.46%</span> |
| 147 | [00041 DROP_TRIGGER](crates/bench/sqlite_parity/cases/SQLITE_PARITY_041_DROP_TRIGGER.rs) | P0 | memory | SQL_DROP | 1798870 | 3550039 | <span style="color:#dc2626">-97.35%</span> |
| 148 | [00962 VIEW_TRIGGER_GENERATED_015](crates/bench/sqlite_parity/cases/SQLITE_PARITY_962_VIEW_TRIGGER_GENERATED_015.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1954423 | 3856338 | <span style="color:#dc2626">-97.31%</span> |
| 149 | [00661 JOIN_SUBQUERY_EXISTS_054](crates/bench/sqlite_parity/cases/SQLITE_PARITY_661_JOIN_SUBQUERY_EXISTS_054.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1927723 | 3803408 | <span style="color:#dc2626">-97.30%</span> |
| 150 | [00660 JOIN_SUBQUERY_EXISTS_053](crates/bench/sqlite_parity/cases/SQLITE_PARITY_660_JOIN_SUBQUERY_EXISTS_053.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1928374 | 3802497 | <span style="color:#dc2626">-97.19%</span> |
| 151 | [00491 DML_WHERE_ORDER_LIMIT_104](crates/bench/sqlite_parity/cases/SQLITE_PARITY_491_DML_WHERE_ORDER_LIMIT_104.rs) | P1 | memory | GEN_SQL_DML | 1946730 | 3838404 | <span style="color:#dc2626">-97.17%</span> |
| 152 | [00502 DML_WHERE_ORDER_LIMIT_115](crates/bench/sqlite_parity/cases/SQLITE_PARITY_502_DML_WHERE_ORDER_LIMIT_115.rs) | P1 | memory | GEN_SQL_DML | 1941991 | 3828425 | <span style="color:#dc2626">-97.14%</span> |
| 153 | [00626 JOIN_SUBQUERY_EXISTS_019](crates/bench/sqlite_parity/cases/SQLITE_PARITY_626_JOIN_SUBQUERY_EXISTS_019.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1941990 | 3828175 | <span style="color:#dc2626">-97.13%</span> |
| 154 | [00407 DML_WHERE_ORDER_LIMIT_020](crates/bench/sqlite_parity/cases/SQLITE_PARITY_407_DML_WHERE_ORDER_LIMIT_020.rs) | P1 | memory | GEN_SQL_DML | 1973359 | 3889511 | <span style="color:#dc2626">-97.10%</span> |
| 155 | [00391 DML_WHERE_ORDER_LIMIT_004](crates/bench/sqlite_parity/cases/SQLITE_PARITY_391_DML_WHERE_ORDER_LIMIT_004.rs) | P1 | memory | GEN_SQL_DML | 1997044 | 3933954 | <span style="color:#dc2626">-96.99%</span> |
| 156 | [00695 JOIN_SUBQUERY_EXISTS_088](crates/bench/sqlite_parity/cases/SQLITE_PARITY_695_JOIN_SUBQUERY_EXISTS_088.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1931971 | 3805592 | <span style="color:#dc2626">-96.98%</span> |
| 157 | [00998 VIEW_TRIGGER_GENERATED_051](crates/bench/sqlite_parity/cases/SQLITE_PARITY_998_VIEW_TRIGGER_GENERATED_051.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2045115 | 4027662 | <span style="color:#dc2626">-96.94%</span> |
| 158 | [00479 DML_WHERE_ORDER_LIMIT_092](crates/bench/sqlite_parity/cases/SQLITE_PARITY_479_DML_WHERE_ORDER_LIMIT_092.rs) | P1 | memory | GEN_SQL_DML | 1928554 | 3796766 | <span style="color:#dc2626">-96.87%</span> |
| 159 | [00619 JOIN_SUBQUERY_EXISTS_012](crates/bench/sqlite_parity/cases/SQLITE_PARITY_619_JOIN_SUBQUERY_EXISTS_012.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1964563 | 3862569 | <span style="color:#dc2626">-96.61%</span> |
| 160 | [00669 JOIN_SUBQUERY_EXISTS_062](crates/bench/sqlite_parity/cases/SQLITE_PARITY_669_JOIN_SUBQUERY_EXISTS_062.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1964663 | 3862761 | <span style="color:#dc2626">-96.61%</span> |
| 161 | [00643 JOIN_SUBQUERY_EXISTS_036](crates/bench/sqlite_parity/cases/SQLITE_PARITY_643_JOIN_SUBQUERY_EXISTS_036.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1958792 | 3850688 | <span style="color:#dc2626">-96.58%</span> |
| 162 | [00440 DML_WHERE_ORDER_LIMIT_053](crates/bench/sqlite_parity/cases/SQLITE_PARITY_440_DML_WHERE_ORDER_LIMIT_053.rs) | P1 | memory | GEN_SQL_DML | 1887067 | 3709030 | <span style="color:#dc2626">-96.55%</span> |
| 163 | [00504 DML_WHERE_ORDER_LIMIT_117](crates/bench/sqlite_parity/cases/SQLITE_PARITY_504_DML_WHERE_ORDER_LIMIT_117.rs) | P1 | memory | GEN_SQL_DML | 1939014 | 3810281 | <span style="color:#dc2626">-96.51%</span> |
| 164 | [00671 JOIN_SUBQUERY_EXISTS_064](crates/bench/sqlite_parity/cases/SQLITE_PARITY_671_JOIN_SUBQUERY_EXISTS_064.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1954805 | 3840488 | <span style="color:#dc2626">-96.46%</span> |
| 165 | [00996 VIEW_TRIGGER_GENERATED_049](crates/bench/sqlite_parity/cases/SQLITE_PARITY_996_VIEW_TRIGGER_GENERATED_049.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1991454 | 3912324 | <span style="color:#dc2626">-96.46%</span> |
| 166 | [00473 DML_WHERE_ORDER_LIMIT_086](crates/bench/sqlite_parity/cases/SQLITE_PARITY_473_DML_WHERE_ORDER_LIMIT_086.rs) | P1 | memory | GEN_SQL_DML | 1899279 | 3730881 | <span style="color:#dc2626">-96.44%</span> |
| 167 | [00993 VIEW_TRIGGER_GENERATED_046](crates/bench/sqlite_parity/cases/SQLITE_PARITY_993_VIEW_TRIGGER_GENERATED_046.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1958702 | 3846249 | <span style="color:#dc2626">-96.37%</span> |
| 168 | [01000 VIEW_TRIGGER_GENERATED_053](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1000_VIEW_TRIGGER_GENERATED_053.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2032972 | 3991905 | <span style="color:#dc2626">-96.36%</span> |
| 169 | [00706 JOIN_SUBQUERY_EXISTS_099](crates/bench/sqlite_parity/cases/SQLITE_PARITY_706_JOIN_SUBQUERY_EXISTS_099.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1966086 | 3859154 | <span style="color:#dc2626">-96.29%</span> |
| 170 | [00937 CONSTRAINT_FK_SAVEPOINT_070](crates/bench/sqlite_parity/cases/SQLITE_PARITY_937_CONSTRAINT_FK_SAVEPOINT_070.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1883440 | 3695845 | <span style="color:#dc2626">-96.23%</span> |
| 171 | [00498 DML_WHERE_ORDER_LIMIT_111](crates/bench/sqlite_parity/cases/SQLITE_PARITY_498_DML_WHERE_ORDER_LIMIT_111.rs) | P1 | memory | GEN_SQL_DML | 1912294 | 3752031 | <span style="color:#dc2626">-96.21%</span> |
| 172 | [00953 VIEW_TRIGGER_GENERATED_006](crates/bench/sqlite_parity/cases/SQLITE_PARITY_953_VIEW_TRIGGER_GENERATED_006.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1981265 | 3886885 | <span style="color:#dc2626">-96.18%</span> |
| 173 | [00682 JOIN_SUBQUERY_EXISTS_075](crates/bench/sqlite_parity/cases/SQLITE_PARITY_682_JOIN_SUBQUERY_EXISTS_075.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1936099 | 3797887 | <span style="color:#dc2626">-96.16%</span> |
| 174 | [00632 JOIN_SUBQUERY_EXISTS_025](crates/bench/sqlite_parity/cases/SQLITE_PARITY_632_JOIN_SUBQUERY_EXISTS_025.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1960686 | 3845758 | <span style="color:#dc2626">-96.14%</span> |
| 175 | [00971 VIEW_TRIGGER_GENERATED_024](crates/bench/sqlite_parity/cases/SQLITE_PARITY_971_VIEW_TRIGGER_GENERATED_024.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2033393 | 3984932 | <span style="color:#dc2626">-95.97%</span> |
| 176 | [00486 DML_WHERE_ORDER_LIMIT_099](crates/bench/sqlite_parity/cases/SQLITE_PARITY_486_DML_WHERE_ORDER_LIMIT_099.rs) | P1 | memory | GEN_SQL_DML | 1907665 | 3738436 | <span style="color:#dc2626">-95.97%</span> |
| 177 | [00970 VIEW_TRIGGER_GENERATED_023](crates/bench/sqlite_parity/cases/SQLITE_PARITY_970_VIEW_TRIGGER_GENERATED_023.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2237679 | 4382633 | <span style="color:#dc2626">-95.86%</span> |
| 178 | [01006 VIEW_TRIGGER_GENERATED_059](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1006_VIEW_TRIGGER_GENERATED_059.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1972859 | 3862590 | <span style="color:#dc2626">-95.79%</span> |
| 179 | [00622 JOIN_SUBQUERY_EXISTS_015](crates/bench/sqlite_parity/cases/SQLITE_PARITY_622_JOIN_SUBQUERY_EXISTS_015.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2008646 | 3929146 | <span style="color:#dc2626">-95.61%</span> |
| 180 | [00800 WINDOW_PARTITION_SUM_013](crates/bench/sqlite_parity/cases/SQLITE_PARITY_800_WINDOW_PARTITION_SUM_013.rs) | P2 | memory | GEN_SQL_WINDOW | 1849676 | 3618048 | <span style="color:#dc2626">-95.60%</span> |
| 181 | [00973 VIEW_TRIGGER_GENERATED_026](crates/bench/sqlite_parity/cases/SQLITE_PARITY_973_VIEW_TRIGGER_GENERATED_026.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1980964 | 3874362 | <span style="color:#dc2626">-95.58%</span> |
| 182 | [00476 DML_WHERE_ORDER_LIMIT_089](crates/bench/sqlite_parity/cases/SQLITE_PARITY_476_DML_WHERE_ORDER_LIMIT_089.rs) | P1 | memory | GEN_SQL_DML | 1954684 | 3822143 | <span style="color:#dc2626">-95.54%</span> |
| 183 | [00672 JOIN_SUBQUERY_EXISTS_065](crates/bench/sqlite_parity/cases/SQLITE_PARITY_672_JOIN_SUBQUERY_EXISTS_065.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1908557 | 3731422 | <span style="color:#dc2626">-95.51%</span> |
| 184 | [00451 DML_WHERE_ORDER_LIMIT_064](crates/bench/sqlite_parity/cases/SQLITE_PARITY_451_DML_WHERE_ORDER_LIMIT_064.rs) | P1 | memory | GEN_SQL_DML | 1938684 | 3789683 | <span style="color:#dc2626">-95.48%</span> |
| 185 | [00688 JOIN_SUBQUERY_EXISTS_081](crates/bench/sqlite_parity/cases/SQLITE_PARITY_688_JOIN_SUBQUERY_EXISTS_081.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1975794 | 3858201 | <span style="color:#dc2626">-95.27%</span> |
| 186 | [00402 DML_WHERE_ORDER_LIMIT_015](crates/bench/sqlite_parity/cases/SQLITE_PARITY_402_DML_WHERE_ORDER_LIMIT_015.rs) | P1 | memory | GEN_SQL_DML | 1911512 | 3731723 | <span style="color:#dc2626">-95.22%</span> |
| 187 | [00997 VIEW_TRIGGER_GENERATED_050](crates/bench/sqlite_parity/cases/SQLITE_PARITY_997_VIEW_TRIGGER_GENERATED_050.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2038242 | 3978078 | <span style="color:#dc2626">-95.17%</span> |
| 188 | [00630 JOIN_SUBQUERY_EXISTS_023](crates/bench/sqlite_parity/cases/SQLITE_PARITY_630_JOIN_SUBQUERY_EXISTS_023.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1925168 | 3755788 | <span style="color:#dc2626">-95.09%</span> |
| 189 | [00637 JOIN_SUBQUERY_EXISTS_030](crates/bench/sqlite_parity/cases/SQLITE_PARITY_637_JOIN_SUBQUERY_EXISTS_030.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1934917 | 3773472 | <span style="color:#dc2626">-95.02%</span> |
| 190 | [00640 JOIN_SUBQUERY_EXISTS_033](crates/bench/sqlite_parity/cases/SQLITE_PARITY_640_JOIN_SUBQUERY_EXISTS_033.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1924617 | 3753203 | <span style="color:#dc2626">-95.01%</span> |
| 191 | [01002 VIEW_TRIGGER_GENERATED_055](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1002_VIEW_TRIGGER_GENERATED_055.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2085752 | 4065424 | <span style="color:#dc2626">-94.91%</span> |
| 192 | [00499 DML_WHERE_ORDER_LIMIT_112](crates/bench/sqlite_parity/cases/SQLITE_PARITY_499_DML_WHERE_ORDER_LIMIT_112.rs) | P1 | memory | GEN_SQL_DML | 2003617 | 3905120 | <span style="color:#dc2626">-94.90%</span> |
| 193 | [00863 WINDOW_PARTITION_SUM_076](crates/bench/sqlite_parity/cases/SQLITE_PARITY_863_WINDOW_PARTITION_SUM_076.rs) | P2 | memory | GEN_SQL_WINDOW | 2138201 | 4166785 | <span style="color:#dc2626">-94.87%</span> |
| 194 | [00644 JOIN_SUBQUERY_EXISTS_037](crates/bench/sqlite_parity/cases/SQLITE_PARITY_644_JOIN_SUBQUERY_EXISTS_037.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1958822 | 3816563 | <span style="color:#dc2626">-94.84%</span> |
| 195 | [00647 JOIN_SUBQUERY_EXISTS_040](crates/bench/sqlite_parity/cases/SQLITE_PARITY_647_JOIN_SUBQUERY_EXISTS_040.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1946038 | 3790874 | <span style="color:#dc2626">-94.80%</span> |
| 196 | [00979 VIEW_TRIGGER_GENERATED_032](crates/bench/sqlite_parity/cases/SQLITE_PARITY_979_VIEW_TRIGGER_GENERATED_032.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1991534 | 3879272 | <span style="color:#dc2626">-94.79%</span> |
| 197 | [00665 JOIN_SUBQUERY_EXISTS_058](crates/bench/sqlite_parity/cases/SQLITE_PARITY_665_JOIN_SUBQUERY_EXISTS_058.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1940497 | 3776236 | <span style="color:#dc2626">-94.60%</span> |
| 198 | [00666 JOIN_SUBQUERY_EXISTS_059](crates/bench/sqlite_parity/cases/SQLITE_PARITY_666_JOIN_SUBQUERY_EXISTS_059.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1927723 | 3749326 | <span style="color:#dc2626">-94.50%</span> |
| 199 | [00959 VIEW_TRIGGER_GENERATED_012](crates/bench/sqlite_parity/cases/SQLITE_PARITY_959_VIEW_TRIGGER_GENERATED_012.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2018144 | 3924688 | <span style="color:#dc2626">-94.47%</span> |
| 200 | [00416 DML_WHERE_ORDER_LIMIT_029](crates/bench/sqlite_parity/cases/SQLITE_PARITY_416_DML_WHERE_ORDER_LIMIT_029.rs) | P1 | memory | GEN_SQL_DML | 1957249 | 3806003 | <span style="color:#dc2626">-94.46%</span> |
| 201 | [00221 OPT_DOUBLE_DASH_END_OPTIONS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_221_OPT_DOUBLE_DASH_END_OPTIONS.rs) | P2 | memory | CLI_OPTION | 1655619 | 3217490 | <span style="color:#dc2626">-94.34%</span> |
| 202 | [00357 SCALAR_CAST_TYPEOF_033](crates/bench/sqlite_parity/cases/SQLITE_PARITY_357_SCALAR_CAST_TYPEOF_033.rs) | P1 | memory | GEN_SQL_SCALAR | 1697278 | 3297602 | <span style="color:#dc2626">-94.29%</span> |
| 203 | [00674 JOIN_SUBQUERY_EXISTS_067](crates/bench/sqlite_parity/cases/SQLITE_PARITY_674_JOIN_SUBQUERY_EXISTS_067.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1975083 | 3836722 | <span style="color:#dc2626">-94.26%</span> |
| 204 | [00681 JOIN_SUBQUERY_EXISTS_074](crates/bench/sqlite_parity/cases/SQLITE_PARITY_681_JOIN_SUBQUERY_EXISTS_074.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1989460 | 3864544 | <span style="color:#dc2626">-94.25%</span> |
| 205 | [00815 WINDOW_PARTITION_SUM_028](crates/bench/sqlite_parity/cases/SQLITE_PARITY_815_WINDOW_PARTITION_SUM_028.rs) | P2 | memory | GEN_SQL_WINDOW | 1899079 | 3688601 | <span style="color:#dc2626">-94.23%</span> |
| 206 | [00624 JOIN_SUBQUERY_EXISTS_017](crates/bench/sqlite_parity/cases/SQLITE_PARITY_624_JOIN_SUBQUERY_EXISTS_017.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1962339 | 3809710 | <span style="color:#dc2626">-94.14%</span> |
| 207 | [00641 JOIN_SUBQUERY_EXISTS_034](crates/bench/sqlite_parity/cases/SQLITE_PARITY_641_JOIN_SUBQUERY_EXISTS_034.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1932752 | 3751290 | <span style="color:#dc2626">-94.09%</span> |
| 208 | [00471 DML_WHERE_ORDER_LIMIT_084](crates/bench/sqlite_parity/cases/SQLITE_PARITY_471_DML_WHERE_ORDER_LIMIT_084.rs) | P1 | memory | GEN_SQL_DML | 1960395 | 3801134 | <span style="color:#dc2626">-93.90%</span> |
| 209 | [00694 JOIN_SUBQUERY_EXISTS_087](crates/bench/sqlite_parity/cases/SQLITE_PARITY_694_JOIN_SUBQUERY_EXISTS_087.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1976475 | 3830069 | <span style="color:#dc2626">-93.78%</span> |
| 210 | [00991 VIEW_TRIGGER_GENERATED_044](crates/bench/sqlite_parity/cases/SQLITE_PARITY_991_VIEW_TRIGGER_GENERATED_044.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1976336 | 3828626 | <span style="color:#dc2626">-93.72%</span> |
| 211 | [00700 JOIN_SUBQUERY_EXISTS_093](crates/bench/sqlite_parity/cases/SQLITE_PARITY_700_JOIN_SUBQUERY_EXISTS_093.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1971877 | 3819158 | <span style="color:#dc2626">-93.68%</span> |
| 212 | [00663 JOIN_SUBQUERY_EXISTS_056](crates/bench/sqlite_parity/cases/SQLITE_PARITY_663_JOIN_SUBQUERY_EXISTS_056.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1936319 | 3749476 | <span style="color:#dc2626">-93.64%</span> |
| 213 | [00960 VIEW_TRIGGER_GENERATED_013](crates/bench/sqlite_parity/cases/SQLITE_PARITY_960_VIEW_TRIGGER_GENERATED_013.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2016641 | 3903568 | <span style="color:#dc2626">-93.57%</span> |
| 214 | [00434 DML_WHERE_ORDER_LIMIT_047](crates/bench/sqlite_parity/cases/SQLITE_PARITY_434_DML_WHERE_ORDER_LIMIT_047.rs) | P1 | memory | GEN_SQL_DML | 1941669 | 3757742 | <span style="color:#dc2626">-93.53%</span> |
| 215 | [00628 JOIN_SUBQUERY_EXISTS_021](crates/bench/sqlite_parity/cases/SQLITE_PARITY_628_JOIN_SUBQUERY_EXISTS_021.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1974502 | 3820631 | <span style="color:#dc2626">-93.50%</span> |
| 216 | [00990 VIEW_TRIGGER_GENERATED_043](crates/bench/sqlite_parity/cases/SQLITE_PARITY_990_VIEW_TRIGGER_GENERATED_043.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2004108 | 3875665 | <span style="color:#dc2626">-93.39%</span> |
| 217 | [00439 DML_WHERE_ORDER_LIMIT_052](crates/bench/sqlite_parity/cases/SQLITE_PARITY_439_DML_WHERE_ORDER_LIMIT_052.rs) | P1 | memory | GEN_SQL_DML | 1955245 | 3780705 | <span style="color:#dc2626">-93.36%</span> |
| 218 | [00667 JOIN_SUBQUERY_EXISTS_060](crates/bench/sqlite_parity/cases/SQLITE_PARITY_667_JOIN_SUBQUERY_EXISTS_060.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1947531 | 3764705 | <span style="color:#dc2626">-93.31%</span> |
| 219 | [00680 JOIN_SUBQUERY_EXISTS_073](crates/bench/sqlite_parity/cases/SQLITE_PARITY_680_JOIN_SUBQUERY_EXISTS_073.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1942822 | 3754686 | <span style="color:#dc2626">-93.26%</span> |
| 220 | [00623 JOIN_SUBQUERY_EXISTS_016](crates/bench/sqlite_parity/cases/SQLITE_PARITY_623_JOIN_SUBQUERY_EXISTS_016.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1961266 | 3789933 | <span style="color:#dc2626">-93.24%</span> |
| 221 | [00656 JOIN_SUBQUERY_EXISTS_049](crates/bench/sqlite_parity/cases/SQLITE_PARITY_656_JOIN_SUBQUERY_EXISTS_049.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1975664 | 3815671 | <span style="color:#dc2626">-93.13%</span> |
| 222 | [00013 INSERT_SELECT](crates/bench/sqlite_parity/cases/SQLITE_PARITY_013_INSERT_SELECT.rs) | P0 | memory | SQL_INSERT | 1785815 | 3447776 | <span style="color:#dc2626">-93.06%</span> |
| 223 | [00690 JOIN_SUBQUERY_EXISTS_083](crates/bench/sqlite_parity/cases/SQLITE_PARITY_690_JOIN_SUBQUERY_EXISTS_083.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2022873 | 3903337 | <span style="color:#dc2626">-92.96%</span> |
| 224 | [01004 VIEW_TRIGGER_GENERATED_057](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1004_VIEW_TRIGGER_GENERATED_057.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2005189 | 3867440 | <span style="color:#dc2626">-92.87%</span> |
| 225 | [00075 EXPLAIN_QUERY_PLAN](crates/bench/sqlite_parity/cases/SQLITE_PARITY_075_EXPLAIN_QUERY_PLAN.rs) | P0 | memory | SQL_EXPLAIN | 1804730 | 3478023 | <span style="color:#dc2626">-92.72%</span> |
| 226 | [00697 JOIN_SUBQUERY_EXISTS_090](crates/bench/sqlite_parity/cases/SQLITE_PARITY_697_JOIN_SUBQUERY_EXISTS_090.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1977287 | 3806524 | <span style="color:#dc2626">-92.51%</span> |
| 227 | [00801 WINDOW_PARTITION_SUM_014](crates/bench/sqlite_parity/cases/SQLITE_PARITY_801_WINDOW_PARTITION_SUM_014.rs) | P2 | memory | GEN_SQL_WINDOW | 1973370 | 3794451 | <span style="color:#dc2626">-92.28%</span> |
| 228 | [00693 JOIN_SUBQUERY_EXISTS_086](crates/bench/sqlite_parity/cases/SQLITE_PARITY_693_JOIN_SUBQUERY_EXISTS_086.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1964934 | 3776588 | <span style="color:#dc2626">-92.20%</span> |
| 229 | [00994 VIEW_TRIGGER_GENERATED_047](crates/bench/sqlite_parity/cases/SQLITE_PARITY_994_VIEW_TRIGGER_GENERATED_047.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2043412 | 3927042 | <span style="color:#dc2626">-92.18%</span> |
| 230 | [00972 VIEW_TRIGGER_GENERATED_025](crates/bench/sqlite_parity/cases/SQLITE_PARITY_972_VIEW_TRIGGER_GENERATED_025.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1992385 | 3826852 | <span style="color:#dc2626">-92.07%</span> |
| 231 | [00653 JOIN_SUBQUERY_EXISTS_046](crates/bench/sqlite_parity/cases/SQLITE_PARITY_653_JOIN_SUBQUERY_EXISTS_046.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2028344 | 3894090 | <span style="color:#dc2626">-91.98%</span> |
| 232 | [00378 SCALAR_STRING_038](crates/bench/sqlite_parity/cases/SQLITE_PARITY_378_SCALAR_STRING_038.rs) | P1 | memory | GEN_SQL_SCALAR | 1666980 | 3200268 | <span style="color:#dc2626">-91.98%</span> |
| 233 | [00447 DML_WHERE_ORDER_LIMIT_060](crates/bench/sqlite_parity/cases/SQLITE_PARITY_447_DML_WHERE_ORDER_LIMIT_060.rs) | P1 | memory | GEN_SQL_DML | 1908297 | 3663373 | <span style="color:#dc2626">-91.97%</span> |
| 234 | [00967 VIEW_TRIGGER_GENERATED_020](crates/bench/sqlite_parity/cases/SQLITE_PARITY_967_VIEW_TRIGGER_GENERATED_020.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2036910 | 3907945 | <span style="color:#dc2626">-91.86%</span> |
| 235 | [00638 JOIN_SUBQUERY_EXISTS_031](crates/bench/sqlite_parity/cases/SQLITE_PARITY_638_JOIN_SUBQUERY_EXISTS_031.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1966045 | 3768652 | <span style="color:#dc2626">-91.69%</span> |
| 236 | [00413 DML_WHERE_ORDER_LIMIT_026](crates/bench/sqlite_parity/cases/SQLITE_PARITY_413_DML_WHERE_ORDER_LIMIT_026.rs) | P1 | memory | GEN_SQL_DML | 1964853 | 3766348 | <span style="color:#dc2626">-91.69%</span> |
| 237 | [00349 SCALAR_CAST_TYPEOF_031](crates/bench/sqlite_parity/cases/SQLITE_PARITY_349_SCALAR_CAST_TYPEOF_031.rs) | P1 | memory | GEN_SQL_SCALAR | 1648085 | 3158640 | <span style="color:#dc2626">-91.66%</span> |
| 238 | [00664 JOIN_SUBQUERY_EXISTS_057](crates/bench/sqlite_parity/cases/SQLITE_PARITY_664_JOIN_SUBQUERY_EXISTS_057.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1911583 | 3662923 | <span style="color:#dc2626">-91.62%</span> |
| 239 | [00646 JOIN_SUBQUERY_EXISTS_039](crates/bench/sqlite_parity/cases/SQLITE_PARITY_646_JOIN_SUBQUERY_EXISTS_039.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1982767 | 3799210 | <span style="color:#dc2626">-91.61%</span> |
| 240 | [00819 WINDOW_PARTITION_SUM_032](crates/bench/sqlite_parity/cases/SQLITE_PARITY_819_WINDOW_PARTITION_SUM_032.rs) | P2 | memory | GEN_SQL_WINDOW | 1945557 | 3727896 | <span style="color:#dc2626">-91.61%</span> |
| 241 | [00056 SUBQUERIES_EXISTS_IN](crates/bench/sqlite_parity/cases/SQLITE_PARITY_056_SUBQUERIES_EXISTS_IN.rs) | P0 | memory | SQL_SELECT | 1873752 | 3588502 | <span style="color:#dc2626">-91.51%</span> |
| 242 | [00648 JOIN_SUBQUERY_EXISTS_041](crates/bench/sqlite_parity/cases/SQLITE_PARITY_648_JOIN_SUBQUERY_EXISTS_041.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1970213 | 3772390 | <span style="color:#dc2626">-91.47%</span> |
| 243 | [00676 JOIN_SUBQUERY_EXISTS_069](crates/bench/sqlite_parity/cases/SQLITE_PARITY_676_JOIN_SUBQUERY_EXISTS_069.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1985773 | 3801595 | <span style="color:#dc2626">-91.44%</span> |
| 244 | [00691 JOIN_SUBQUERY_EXISTS_084](crates/bench/sqlite_parity/cases/SQLITE_PARITY_691_JOIN_SUBQUERY_EXISTS_084.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2025728 | 3878019 | <span style="color:#dc2626">-91.44%</span> |
| 245 | [00950 VIEW_TRIGGER_GENERATED_003](crates/bench/sqlite_parity/cases/SQLITE_PARITY_950_VIEW_TRIGGER_GENERATED_003.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2095400 | 4007654 | <span style="color:#dc2626">-91.26%</span> |
| 246 | [00951 VIEW_TRIGGER_GENERATED_004](crates/bench/sqlite_parity/cases/SQLITE_PARITY_951_VIEW_TRIGGER_GENERATED_004.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2013536 | 3850467 | <span style="color:#dc2626">-91.23%</span> |
| 247 | [00820 WINDOW_PARTITION_SUM_033](crates/bench/sqlite_parity/cases/SQLITE_PARITY_820_WINDOW_PARTITION_SUM_033.rs) | P2 | memory | GEN_SQL_WINDOW | 1845117 | 3526414 | <span style="color:#dc2626">-91.12%</span> |
| 248 | [00617 JOIN_SUBQUERY_EXISTS_010](crates/bench/sqlite_parity/cases/SQLITE_PARITY_617_JOIN_SUBQUERY_EXISTS_010.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2011362 | 3843875 | <span style="color:#dc2626">-91.11%</span> |
| 249 | [00833 WINDOW_PARTITION_SUM_046](crates/bench/sqlite_parity/cases/SQLITE_PARITY_833_WINDOW_PARTITION_SUM_046.rs) | P2 | memory | GEN_SQL_WINDOW | 1862100 | 3555680 | <span style="color:#dc2626">-90.95%</span> |
| 250 | [00668 JOIN_SUBQUERY_EXISTS_061](crates/bench/sqlite_parity/cases/SQLITE_PARITY_668_JOIN_SUBQUERY_EXISTS_061.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1975543 | 3770005 | <span style="color:#dc2626">-90.83%</span> |
| 251 | [00679 JOIN_SUBQUERY_EXISTS_072](crates/bench/sqlite_parity/cases/SQLITE_PARITY_679_JOIN_SUBQUERY_EXISTS_072.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1958060 | 3734849 | <span style="color:#dc2626">-90.74%</span> |
| 252 | [00658 JOIN_SUBQUERY_EXISTS_051](crates/bench/sqlite_parity/cases/SQLITE_PARITY_658_JOIN_SUBQUERY_EXISTS_051.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1980393 | 3774163 | <span style="color:#dc2626">-90.58%</span> |
| 253 | [00610 JOIN_SUBQUERY_EXISTS_003](crates/bench/sqlite_parity/cases/SQLITE_PARITY_610_JOIN_SUBQUERY_EXISTS_003.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1979131 | 3771167 | <span style="color:#dc2626">-90.55%</span> |
| 254 | [00098 CLI_REGEXP_OPTIONAL](crates/bench/sqlite_parity/cases/SQLITE_PARITY_098_CLI_REGEXP_OPTIONAL.rs) | P2 | memory | CLI_EXTENSION_OPTIONAL | 1660809 | 3164070 | <span style="color:#dc2626">-90.51%</span> |
| 255 | [00652 JOIN_SUBQUERY_EXISTS_045](crates/bench/sqlite_parity/cases/SQLITE_PARITY_652_JOIN_SUBQUERY_EXISTS_045.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1966476 | 3745909 | <span style="color:#dc2626">-90.49%</span> |
| 256 | [00250 SCALAR_STRING_006](crates/bench/sqlite_parity/cases/SQLITE_PARITY_250_SCALAR_STRING_006.rs) | P1 | memory | GEN_SQL_SCALAR | 1670567 | 3181583 | <span style="color:#dc2626">-90.45%</span> |
| 257 | [00813 WINDOW_PARTITION_SUM_026](crates/bench/sqlite_parity/cases/SQLITE_PARITY_813_WINDOW_PARTITION_SUM_026.rs) | P2 | memory | GEN_SQL_WINDOW | 1852351 | 3527737 | <span style="color:#dc2626">-90.45%</span> |
| 258 | [00825 WINDOW_PARTITION_SUM_038](crates/bench/sqlite_parity/cases/SQLITE_PARITY_825_WINDOW_PARTITION_SUM_038.rs) | P2 | memory | GEN_SQL_WINDOW | 1835859 | 3495977 | <span style="color:#dc2626">-90.43%</span> |
| 259 | [00260 SCALAR_ARITH_009](crates/bench/sqlite_parity/cases/SQLITE_PARITY_260_SCALAR_ARITH_009.rs) | P1 | memory | GEN_SQL_SCALAR | 1724339 | 3279157 | <span style="color:#dc2626">-90.17%</span> |
| 260 | [00867 WINDOW_PARTITION_SUM_080](crates/bench/sqlite_parity/cases/SQLITE_PARITY_867_WINDOW_PARTITION_SUM_080.rs) | P2 | memory | GEN_SQL_WINDOW | 1868250 | 3551432 | <span style="color:#dc2626">-90.09%</span> |
| 261 | [00702 JOIN_SUBQUERY_EXISTS_095](crates/bench/sqlite_parity/cases/SQLITE_PARITY_702_JOIN_SUBQUERY_EXISTS_095.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2040046 | 3877699 | <span style="color:#dc2626">-90.08%</span> |
| 262 | [01005 VIEW_TRIGGER_GENERATED_058](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1005_VIEW_TRIGGER_GENERATED_058.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2046828 | 3889491 | <span style="color:#dc2626">-90.03%</span> |
| 263 | [00618 JOIN_SUBQUERY_EXISTS_011](crates/bench/sqlite_parity/cases/SQLITE_PARITY_618_JOIN_SUBQUERY_EXISTS_011.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2008826 | 3817094 | <span style="color:#dc2626">-90.02%</span> |
| 264 | [00320 SCALAR_ARITH_024](crates/bench/sqlite_parity/cases/SQLITE_PARITY_320_SCALAR_ARITH_024.rs) | P1 | memory | GEN_SQL_SCALAR | 1714480 | 3257486 | <span style="color:#dc2626">-90.00%</span> |
| 265 | [00847 WINDOW_PARTITION_SUM_060](crates/bench/sqlite_parity/cases/SQLITE_PARITY_847_WINDOW_PARTITION_SUM_060.rs) | P2 | memory | GEN_SQL_WINDOW | 1811453 | 3441564 | <span style="color:#dc2626">-89.99%</span> |
| 266 | [00488 DML_WHERE_ORDER_LIMIT_101](crates/bench/sqlite_parity/cases/SQLITE_PARITY_488_DML_WHERE_ORDER_LIMIT_101.rs) | P1 | memory | GEN_SQL_DML | 2001472 | 3800814 | <span style="color:#dc2626">-89.90%</span> |
| 267 | [00871 CONSTRAINT_FK_SAVEPOINT_004](crates/bench/sqlite_parity/cases/SQLITE_PARITY_871_CONSTRAINT_FK_SAVEPOINT_004.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1915070 | 3636663 | <span style="color:#dc2626">-89.90%</span> |
| 268 | [00612 JOIN_SUBQUERY_EXISTS_005](crates/bench/sqlite_parity/cases/SQLITE_PARITY_612_JOIN_SUBQUERY_EXISTS_005.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2000661 | 3798439 | <span style="color:#dc2626">-89.86%</span> |
| 269 | [00689 JOIN_SUBQUERY_EXISTS_082](crates/bench/sqlite_parity/cases/SQLITE_PARITY_689_JOIN_SUBQUERY_EXISTS_082.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2040155 | 3872058 | <span style="color:#dc2626">-89.79%</span> |
| 270 | [00862 WINDOW_PARTITION_SUM_075](crates/bench/sqlite_parity/cases/SQLITE_PARITY_862_WINDOW_PARTITION_SUM_075.rs) | P2 | memory | GEN_SQL_WINDOW | 1925499 | 3654156 | <span style="color:#dc2626">-89.78%</span> |
| 271 | [00649 JOIN_SUBQUERY_EXISTS_042](crates/bench/sqlite_parity/cases/SQLITE_PARITY_649_JOIN_SUBQUERY_EXISTS_042.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2008416 | 3808657 | <span style="color:#dc2626">-89.63%</span> |
| 272 | [00670 JOIN_SUBQUERY_EXISTS_063](crates/bench/sqlite_parity/cases/SQLITE_PARITY_670_JOIN_SUBQUERY_EXISTS_063.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1954183 | 3705684 | <span style="color:#dc2626">-89.63%</span> |
| 273 | [00904 CONSTRAINT_FK_SAVEPOINT_037](crates/bench/sqlite_parity/cases/SQLITE_PARITY_904_CONSTRAINT_FK_SAVEPOINT_037.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1896074 | 3590235 | <span style="color:#dc2626">-89.35%</span> |
| 274 | [00981 VIEW_TRIGGER_GENERATED_034](crates/bench/sqlite_parity/cases/SQLITE_PARITY_981_VIEW_TRIGGER_GENERATED_034.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2064482 | 3908056 | <span style="color:#dc2626">-89.30%</span> |
| 275 | [00809 WINDOW_PARTITION_SUM_022](crates/bench/sqlite_parity/cases/SQLITE_PARITY_809_WINDOW_PARTITION_SUM_022.rs) | P2 | memory | GEN_SQL_WINDOW | 1860105 | 3521095 | <span style="color:#dc2626">-89.30%</span> |
| 276 | [00026 FOREIGN_KEYS_CASCADE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_026_FOREIGN_KEYS_CASCADE.rs) | P0 | memory | SQL_FOREIGN_KEYS | 1904068 | 3603581 | <span style="color:#dc2626">-89.26%</span> |
| 277 | [00613 JOIN_SUBQUERY_EXISTS_006](crates/bench/sqlite_parity/cases/SQLITE_PARITY_613_JOIN_SUBQUERY_EXISTS_006.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2029115 | 3839937 | <span style="color:#dc2626">-89.24%</span> |
| 278 | [00426 DML_WHERE_ORDER_LIMIT_039](crates/bench/sqlite_parity/cases/SQLITE_PARITY_426_DML_WHERE_ORDER_LIMIT_039.rs) | P1 | memory | GEN_SQL_DML | 1973680 | 3734969 | <span style="color:#dc2626">-89.24%</span> |
| 279 | [00677 JOIN_SUBQUERY_EXISTS_070](crates/bench/sqlite_parity/cases/SQLITE_PARITY_677_JOIN_SUBQUERY_EXISTS_070.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2006702 | 3791295 | <span style="color:#dc2626">-88.93%</span> |
| 280 | [00796 WINDOW_PARTITION_SUM_009](crates/bench/sqlite_parity/cases/SQLITE_PARITY_796_WINDOW_PARTITION_SUM_009.rs) | P2 | memory | GEN_SQL_WINDOW | 1820160 | 3436184 | <span style="color:#dc2626">-88.78%</span> |
| 281 | [00645 JOIN_SUBQUERY_EXISTS_038](crates/bench/sqlite_parity/cases/SQLITE_PARITY_645_JOIN_SUBQUERY_EXISTS_038.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2026660 | 3825079 | <span style="color:#dc2626">-88.74%</span> |
| 282 | [00264 SCALAR_ARITH_010](crates/bench/sqlite_parity/cases/SQLITE_PARITY_264_SCALAR_ARITH_010.rs) | P1 | memory | GEN_SQL_SCALAR | 1721333 | 3247007 | <span style="color:#dc2626">-88.63%</span> |
| 283 | [00008 STRICT_TABLE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_008_STRICT_TABLE.rs) | P0 | memory | SQL_DDL | 1756931 | 3314103 | <span style="color:#dc2626">-88.63%</span> |
| 284 | [00609 JOIN_SUBQUERY_EXISTS_002](crates/bench/sqlite_parity/cases/SQLITE_PARITY_609_JOIN_SUBQUERY_EXISTS_002.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2017362 | 3805152 | <span style="color:#dc2626">-88.62%</span> |
| 285 | [00326 SCALAR_STRING_025](crates/bench/sqlite_parity/cases/SQLITE_PARITY_326_SCALAR_STRING_025.rs) | P1 | memory | GEN_SQL_SCALAR | 1626304 | 3066765 | <span style="color:#dc2626">-88.57%</span> |
| 286 | [00982 VIEW_TRIGGER_GENERATED_035](crates/bench/sqlite_parity/cases/SQLITE_PARITY_982_VIEW_TRIGGER_GENERATED_035.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2063089 | 3889561 | <span style="color:#dc2626">-88.53%</span> |
| 287 | [00794 WINDOW_PARTITION_SUM_007](crates/bench/sqlite_parity/cases/SQLITE_PARITY_794_WINDOW_PARTITION_SUM_007.rs) | P2 | memory | GEN_SQL_WINDOW | 1843444 | 3475127 | <span style="color:#dc2626">-88.51%</span> |
| 288 | [00958 VIEW_TRIGGER_GENERATED_011](crates/bench/sqlite_parity/cases/SQLITE_PARITY_958_VIEW_TRIGGER_GENERATED_011.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2112543 | 3981515 | <span style="color:#dc2626">-88.47%</span> |
| 289 | [00849 WINDOW_PARTITION_SUM_062](crates/bench/sqlite_parity/cases/SQLITE_PARITY_849_WINDOW_PARTITION_SUM_062.rs) | P2 | memory | GEN_SQL_WINDOW | 1894530 | 3569696 | <span style="color:#dc2626">-88.42%</span> |
| 290 | [00976 VIEW_TRIGGER_GENERATED_029](crates/bench/sqlite_parity/cases/SQLITE_PARITY_976_VIEW_TRIGGER_GENERATED_029.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1950156 | 3670036 | <span style="color:#dc2626">-88.19%</span> |
| 291 | [00898 CONSTRAINT_FK_SAVEPOINT_031](crates/bench/sqlite_parity/cases/SQLITE_PARITY_898_CONSTRAINT_FK_SAVEPOINT_031.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1918275 | 3608059 | <span style="color:#dc2626">-88.09%</span> |
| 292 | [00814 WINDOW_PARTITION_SUM_027](crates/bench/sqlite_parity/cases/SQLITE_PARITY_814_WINDOW_PARTITION_SUM_027.rs) | P2 | memory | GEN_SQL_WINDOW | 1852100 | 3478784 | <span style="color:#dc2626">-87.83%</span> |
| 293 | [00662 JOIN_SUBQUERY_EXISTS_055](crates/bench/sqlite_parity/cases/SQLITE_PARITY_662_JOIN_SUBQUERY_EXISTS_055.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2033884 | 3816062 | <span style="color:#dc2626">-87.62%</span> |
| 294 | [00490 DML_WHERE_ORDER_LIMIT_103](crates/bench/sqlite_parity/cases/SQLITE_PARITY_490_DML_WHERE_ORDER_LIMIT_103.rs) | P1 | memory | GEN_SQL_DML | 1995642 | 3741441 | <span style="color:#dc2626">-87.48%</span> |
| 295 | [00839 WINDOW_PARTITION_SUM_052](crates/bench/sqlite_parity/cases/SQLITE_PARITY_839_WINDOW_PARTITION_SUM_052.rs) | P2 | memory | GEN_SQL_WINDOW | 1846690 | 3461462 | <span style="color:#dc2626">-87.44%</span> |
| 296 | [00955 VIEW_TRIGGER_GENERATED_008](crates/bench/sqlite_parity/cases/SQLITE_PARITY_955_VIEW_TRIGGER_GENERATED_008.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2072517 | 3883680 | <span style="color:#dc2626">-87.39%</span> |
| 297 | [00108 DOT_MODE_CSV_AND_QUOTE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_108_DOT_MODE_CSV_AND_QUOTE.rs) | P0 | memory | CLI_DOT_COMMAND | 1751310 | 3277744 | <span style="color:#dc2626">-87.16%</span> |
| 298 | [00841 WINDOW_PARTITION_SUM_054](crates/bench/sqlite_parity/cases/SQLITE_PARITY_841_WINDOW_PARTITION_SUM_054.rs) | P2 | memory | GEN_SQL_WINDOW | 2012473 | 3765106 | <span style="color:#dc2626">-87.09%</span> |
| 299 | [00901 CONSTRAINT_FK_SAVEPOINT_034](crates/bench/sqlite_parity/cases/SQLITE_PARITY_901_CONSTRAINT_FK_SAVEPOINT_034.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1941109 | 3628718 | <span style="color:#dc2626">-86.94%</span> |
| 300 | [00854 WINDOW_PARTITION_SUM_067](crates/bench/sqlite_parity/cases/SQLITE_PARITY_854_WINDOW_PARTITION_SUM_067.rs) | P2 | memory | GEN_SQL_WINDOW | 1844657 | 3447896 | <span style="color:#dc2626">-86.91%</span> |
| 301 | [00812 WINDOW_PARTITION_SUM_025](crates/bench/sqlite_parity/cases/SQLITE_PARITY_812_WINDOW_PARTITION_SUM_025.rs) | P2 | memory | GEN_SQL_WINDOW | 1828356 | 3416647 | <span style="color:#dc2626">-86.87%</span> |
| 302 | [00877 CONSTRAINT_FK_SAVEPOINT_010](crates/bench/sqlite_parity/cases/SQLITE_PARITY_877_CONSTRAINT_FK_SAVEPOINT_010.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1925289 | 3597550 | <span style="color:#dc2626">-86.86%</span> |
| 303 | [00633 JOIN_SUBQUERY_EXISTS_026](crates/bench/sqlite_parity/cases/SQLITE_PARITY_633_JOIN_SUBQUERY_EXISTS_026.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2010329 | 3756249 | <span style="color:#dc2626">-86.85%</span> |
| 304 | [00916 CONSTRAINT_FK_SAVEPOINT_049](crates/bench/sqlite_parity/cases/SQLITE_PARITY_916_CONSTRAINT_FK_SAVEPOINT_049.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1903457 | 3552925 | <span style="color:#dc2626">-86.66%</span> |
| 305 | [00067 CASE_COALESCE_NULLIF_IIF](crates/bench/sqlite_parity/cases/SQLITE_PARITY_067_CASE_COALESCE_NULLIF_IIF.rs) | P0 | memory | SQL_EXPRESSIONS | 1702588 | 3176183 | <span style="color:#dc2626">-86.55%</span> |
| 306 | [00386 SCALAR_STRING_040](crates/bench/sqlite_parity/cases/SQLITE_PARITY_386_SCALAR_STRING_040.rs) | P1 | memory | GEN_SQL_SCALAR | 1636864 | 3049954 | <span style="color:#dc2626">-86.33%</span> |
| 307 | [00310 SCALAR_STRING_021](crates/bench/sqlite_parity/cases/SQLITE_PARITY_310_SCALAR_STRING_021.rs) | P1 | memory | GEN_SQL_SCALAR | 1687609 | 3143842 | <span style="color:#dc2626">-86.29%</span> |
| 308 | [00880 CONSTRAINT_FK_SAVEPOINT_013](crates/bench/sqlite_parity/cases/SQLITE_PARITY_880_CONSTRAINT_FK_SAVEPOINT_013.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1966206 | 3662262 | <span style="color:#dc2626">-86.26%</span> |
| 309 | [00399 DML_WHERE_ORDER_LIMIT_012](crates/bench/sqlite_parity/cases/SQLITE_PARITY_399_DML_WHERE_ORDER_LIMIT_012.rs) | P1 | memory | GEN_SQL_DML | 2034986 | 3784052 | <span style="color:#dc2626">-85.95%</span> |
| 310 | [00803 WINDOW_PARTITION_SUM_016](crates/bench/sqlite_parity/cases/SQLITE_PARITY_803_WINDOW_PARTITION_SUM_016.rs) | P2 | memory | GEN_SQL_WINDOW | 1847301 | 3432567 | <span style="color:#dc2626">-85.82%</span> |
| 311 | [00036 CREATE_VIEW](crates/bench/sqlite_parity/cases/SQLITE_PARITY_036_CREATE_VIEW.rs) | P0 | memory | SQL_VIEW | 1822645 | 3384476 | <span style="color:#dc2626">-85.69%</span> |
| 312 | [00298 SCALAR_STRING_018](crates/bench/sqlite_parity/cases/SQLITE_PARITY_298_SCALAR_STRING_018.rs) | P1 | memory | GEN_SQL_SCALAR | 1674684 | 3109627 | <span style="color:#dc2626">-85.68%</span> |
| 313 | [00362 SCALAR_STRING_034](crates/bench/sqlite_parity/cases/SQLITE_PARITY_362_SCALAR_STRING_034.rs) | P1 | memory | GEN_SQL_SCALAR | 1671960 | 3104567 | <span style="color:#dc2626">-85.68%</span> |
| 314 | [00034 EXPRESSION_INDEX](crates/bench/sqlite_parity/cases/SQLITE_PARITY_034_EXPRESSION_INDEX.rs) | P0 | memory | SQL_INDEX | 1833866 | 3404804 | <span style="color:#dc2626">-85.66%</span> |
| 315 | [00844 WINDOW_PARTITION_SUM_057](crates/bench/sqlite_parity/cases/SQLITE_PARITY_844_WINDOW_PARTITION_SUM_057.rs) | P2 | memory | GEN_SQL_WINDOW | 1886796 | 3502219 | <span style="color:#dc2626">-85.62%</span> |
| 316 | [00336 SCALAR_ARITH_028](crates/bench/sqlite_parity/cases/SQLITE_PARITY_336_SCALAR_ARITH_028.rs) | P1 | memory | GEN_SQL_SCALAR | 1647123 | 3053661 | <span style="color:#dc2626">-85.39%</span> |
| 317 | [00037 DROP_VIEW](crates/bench/sqlite_parity/cases/SQLITE_PARITY_037_DROP_VIEW.rs) | P0 | memory | SQL_DROP | 1776727 | 3293654 | <span style="color:#dc2626">-85.38%</span> |
| 318 | [00892 CONSTRAINT_FK_SAVEPOINT_025](crates/bench/sqlite_parity/cases/SQLITE_PARITY_892_CONSTRAINT_FK_SAVEPOINT_025.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1926722 | 3566811 | <span style="color:#dc2626">-85.12%</span> |
| 319 | [00799 WINDOW_PARTITION_SUM_012](crates/bench/sqlite_parity/cases/SQLITE_PARITY_799_WINDOW_PARTITION_SUM_012.rs) | P2 | memory | GEN_SQL_WINDOW | 1904068 | 3523229 | <span style="color:#dc2626">-85.04%</span> |
| 320 | [00463 DML_WHERE_ORDER_LIMIT_076](crates/bench/sqlite_parity/cases/SQLITE_PARITY_463_DML_WHERE_ORDER_LIMIT_076.rs) | P1 | memory | GEN_SQL_DML | 2135326 | 3946869 | <span style="color:#dc2626">-84.84%</span> |
| 321 | [00886 CONSTRAINT_FK_SAVEPOINT_019](crates/bench/sqlite_parity/cases/SQLITE_PARITY_886_CONSTRAINT_FK_SAVEPOINT_019.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1971315 | 3640631 | <span style="color:#dc2626">-84.68%</span> |
| 322 | [00253 SCALAR_CAST_TYPEOF_007](crates/bench/sqlite_parity/cases/SQLITE_PARITY_253_SCALAR_CAST_TYPEOF_007.rs) | P1 | memory | GEN_SQL_SCALAR | 1708209 | 3153590 | <span style="color:#dc2626">-84.61%</span> |
| 323 | [00701 JOIN_SUBQUERY_EXISTS_094](crates/bench/sqlite_parity/cases/SQLITE_PARITY_701_JOIN_SUBQUERY_EXISTS_094.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2109256 | 3893909 | <span style="color:#dc2626">-84.61%</span> |
| 324 | [00928 CONSTRAINT_FK_SAVEPOINT_061](crates/bench/sqlite_parity/cases/SQLITE_PARITY_928_CONSTRAINT_FK_SAVEPOINT_061.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1960696 | 3619211 | <span style="color:#dc2626">-84.59%</span> |
| 325 | [00659 JOIN_SUBQUERY_EXISTS_052](crates/bench/sqlite_parity/cases/SQLITE_PARITY_659_JOIN_SUBQUERY_EXISTS_052.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2089148 | 3855556 | <span style="color:#dc2626">-84.55%</span> |
| 326 | [00853 WINDOW_PARTITION_SUM_066](crates/bench/sqlite_parity/cases/SQLITE_PARITY_853_WINDOW_PARTITION_SUM_066.rs) | P2 | memory | GEN_SQL_WINDOW | 1921091 | 3543327 | <span style="color:#dc2626">-84.44%</span> |
| 327 | [00922 CONSTRAINT_FK_SAVEPOINT_055](crates/bench/sqlite_parity/cases/SQLITE_PARITY_922_CONSTRAINT_FK_SAVEPOINT_055.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1934686 | 3568004 | <span style="color:#dc2626">-84.42%</span> |
| 328 | [00842 WINDOW_PARTITION_SUM_055](crates/bench/sqlite_parity/cases/SQLITE_PARITY_842_WINDOW_PARTITION_SUM_055.rs) | P2 | memory | GEN_SQL_WINDOW | 1846630 | 3405456 | <span style="color:#dc2626">-84.41%</span> |
| 329 | [00230 SCALAR_STRING_001](crates/bench/sqlite_parity/cases/SQLITE_PARITY_230_SCALAR_STRING_001.rs) | P1 | memory | GEN_SQL_SCALAR | 1735880 | 3201100 | <span style="color:#dc2626">-84.41%</span> |
| 330 | [00793 WINDOW_PARTITION_SUM_006](crates/bench/sqlite_parity/cases/SQLITE_PARITY_793_WINDOW_PARTITION_SUM_006.rs) | P2 | memory | GEN_SQL_WINDOW | 1847912 | 3406268 | <span style="color:#dc2626">-84.33%</span> |
| 331 | [00081 BLOBS_HEX_ZEROBLOB](crates/bench/sqlite_parity/cases/SQLITE_PARITY_081_BLOBS_HEX_ZEROBLOB.rs) | P0 | memory | SQL_FUNCTIONS | 1685165 | 3105018 | <span style="color:#dc2626">-84.26%</span> |
| 332 | [00083 CORE_NUMERIC_FUNCTIONS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_083_CORE_NUMERIC_FUNCTIONS.rs) | P0 | memory | SQL_FUNCTIONS | 1706786 | 3144152 | <span style="color:#dc2626">-84.21%</span> |
| 333 | [00006 ROWID_INTEGER_PRIMARY_KEY](crates/bench/sqlite_parity/cases/SQLITE_PARITY_006_ROWID_INTEGER_PRIMARY_KEY.rs) | P0 | memory | SQL_ROWID | 1810111 | 3333721 | <span style="color:#dc2626">-84.17%</span> |
| 334 | [00931 CONSTRAINT_FK_SAVEPOINT_064](crates/bench/sqlite_parity/cases/SQLITE_PARITY_931_CONSTRAINT_FK_SAVEPOINT_064.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1938644 | 3569426 | <span style="color:#dc2626">-84.12%</span> |
| 335 | [00889 CONSTRAINT_FK_SAVEPOINT_022](crates/bench/sqlite_parity/cases/SQLITE_PARITY_889_CONSTRAINT_FK_SAVEPOINT_022.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1974522 | 3633497 | <span style="color:#dc2626">-84.02%</span> |
| 336 | [00031 CREATE_INDEX](crates/bench/sqlite_parity/cases/SQLITE_PARITY_031_CREATE_INDEX.rs) | P0 | memory | SQL_INDEX | 1799862 | 3310817 | <span style="color:#dc2626">-83.95%</span> |
| 337 | [00687 JOIN_SUBQUERY_EXISTS_080](crates/bench/sqlite_parity/cases/SQLITE_PARITY_687_JOIN_SUBQUERY_EXISTS_080.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2051718 | 3773702 | <span style="color:#dc2626">-83.93%</span> |
| 338 | [00864 WINDOW_PARTITION_SUM_077](crates/bench/sqlite_parity/cases/SQLITE_PARITY_864_WINDOW_PARTITION_SUM_077.rs) | P2 | memory | GEN_SQL_WINDOW | 1941920 | 3568855 | <span style="color:#dc2626">-83.78%</span> |
| 339 | [00989 VIEW_TRIGGER_GENERATED_042](crates/bench/sqlite_parity/cases/SQLITE_PARITY_989_VIEW_TRIGGER_GENERATED_042.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2101902 | 3861818 | <span style="color:#dc2626">-83.73%</span> |
| 340 | [00069 COLLATE_NOCASE_RTRIM_BINARY](crates/bench/sqlite_parity/cases/SQLITE_PARITY_069_COLLATE_NOCASE_RTRIM_BINARY.rs) | P0 | memory | SQL_COLLATION | 1682599 | 3089268 | <span style="color:#dc2626">-83.60%</span> |
| 341 | [00830 WINDOW_PARTITION_SUM_043](crates/bench/sqlite_parity/cases/SQLITE_PARITY_830_WINDOW_PARTITION_SUM_043.rs) | P2 | memory | GEN_SQL_WINDOW | 1878771 | 3448187 | <span style="color:#dc2626">-83.53%</span> |
| 342 | [00058 GROUP_BY_HAVING](crates/bench/sqlite_parity/cases/SQLITE_PARITY_058_GROUP_BY_HAVING.rs) | P0 | memory | SQL_AGGREGATE | 1783451 | 3271012 | <span style="color:#dc2626">-83.41%</span> |
| 343 | [00421 DML_WHERE_ORDER_LIMIT_034](crates/bench/sqlite_parity/cases/SQLITE_PARITY_421_DML_WHERE_ORDER_LIMIT_034.rs) | P1 | memory | GEN_SQL_DML | 2277083 | 4175822 | <span style="color:#dc2626">-83.38%</span> |
| 344 | [00792 WINDOW_PARTITION_SUM_005](crates/bench/sqlite_parity/cases/SQLITE_PARITY_792_WINDOW_PARTITION_SUM_005.rs) | P2 | memory | GEN_SQL_WINDOW | 1924517 | 3526655 | <span style="color:#dc2626">-83.25%</span> |
| 345 | [00913 CONSTRAINT_FK_SAVEPOINT_046](crates/bench/sqlite_parity/cases/SQLITE_PARITY_913_CONSTRAINT_FK_SAVEPOINT_046.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1999479 | 3663745 | <span style="color:#dc2626">-83.23%</span> |
| 346 | [00082 CORE_STRING_FUNCTIONS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_082_CORE_STRING_FUNCTIONS.rs) | P0 | memory | SQL_FUNCTIONS | 1697819 | 3110728 | <span style="color:#dc2626">-83.22%</span> |
| 347 | [01001 VIEW_TRIGGER_GENERATED_054](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1001_VIEW_TRIGGER_GENERATED_054.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2857381 | 5234243 | <span style="color:#dc2626">-83.18%</span> |
| 348 | [00826 WINDOW_PARTITION_SUM_039](crates/bench/sqlite_parity/cases/SQLITE_PARITY_826_WINDOW_PARTITION_SUM_039.rs) | P2 | memory | GEN_SQL_WINDOW | 1907424 | 3493092 | <span style="color:#dc2626">-83.13%</span> |
| 349 | [00306 SCALAR_STRING_020](crates/bench/sqlite_parity/cases/SQLITE_PARITY_306_SCALAR_STRING_020.rs) | P1 | memory | GEN_SQL_SCALAR | 1679785 | 3076083 | <span style="color:#dc2626">-83.12%</span> |
| 350 | [00808 WINDOW_PARTITION_SUM_021](crates/bench/sqlite_parity/cases/SQLITE_PARITY_808_WINDOW_PARTITION_SUM_021.rs) | P2 | memory | GEN_SQL_WINDOW | 1869062 | 3420284 | <span style="color:#dc2626">-82.99%</span> |
| 351 | [00446 DML_WHERE_ORDER_LIMIT_059](crates/bench/sqlite_parity/cases/SQLITE_PARITY_446_DML_WHERE_ORDER_LIMIT_059.rs) | P1 | memory | GEN_SQL_DML | 2062568 | 3768161 | <span style="color:#dc2626">-82.69%</span> |
| 352 | [00286 SCALAR_STRING_015](crates/bench/sqlite_parity/cases/SQLITE_PARITY_286_SCALAR_STRING_015.rs) | P1 | memory | GEN_SQL_SCALAR | 1746210 | 3187654 | <span style="color:#dc2626">-82.55%</span> |
| 353 | [00373 SCALAR_CAST_TYPEOF_037](crates/bench/sqlite_parity/cases/SQLITE_PARITY_373_SCALAR_CAST_TYPEOF_037.rs) | P1 | memory | GEN_SQL_SCALAR | 1681228 | 3067777 | <span style="color:#dc2626">-82.47%</span> |
| 354 | [00866 WINDOW_PARTITION_SUM_079](crates/bench/sqlite_parity/cases/SQLITE_PARITY_866_WINDOW_PARTITION_SUM_079.rs) | P2 | memory | GEN_SQL_WINDOW | 1935738 | 3532176 | <span style="color:#dc2626">-82.47%</span> |
| 355 | [00895 CONSTRAINT_FK_SAVEPOINT_028](crates/bench/sqlite_parity/cases/SQLITE_PARITY_895_CONSTRAINT_FK_SAVEPOINT_028.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2090290 | 3812705 | <span style="color:#dc2626">-82.40%</span> |
| 356 | [00052 PRAGMA_INTEGRITY_QUICK_CHECK](crates/bench/sqlite_parity/cases/SQLITE_PARITY_052_PRAGMA_INTEGRITY_QUICK_CHECK.rs) | P0 | memory | SQL_PRAGMA | 1872709 | 3415325 | <span style="color:#dc2626">-82.37%</span> |
| 357 | [00468 DML_WHERE_ORDER_LIMIT_081](crates/bench/sqlite_parity/cases/SQLITE_PARITY_468_DML_WHERE_ORDER_LIMIT_081.rs) | P1 | memory | GEN_SQL_DML | 2045987 | 3730661 | <span style="color:#dc2626">-82.34%</span> |
| 358 | [00828 WINDOW_PARTITION_SUM_041](crates/bench/sqlite_parity/cases/SQLITE_PARITY_828_WINDOW_PARTITION_SUM_041.rs) | P2 | memory | GEN_SQL_WINDOW | 1914598 | 3490436 | <span style="color:#dc2626">-82.31%</span> |
| 359 | [00910 CONSTRAINT_FK_SAVEPOINT_043](crates/bench/sqlite_parity/cases/SQLITE_PARITY_910_CONSTRAINT_FK_SAVEPOINT_043.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1994219 | 3634028 | <span style="color:#dc2626">-82.23%</span> |
| 360 | [00022 TRANSACTION_COMMIT](crates/bench/sqlite_parity/cases/SQLITE_PARITY_022_TRANSACTION_COMMIT.rs) | P0 | memory | SQL_TRANSACTION | 1795063 | 3270882 | <span style="color:#dc2626">-82.22%</span> |
| 361 | [00403 DML_WHERE_ORDER_LIMIT_016](crates/bench/sqlite_parity/cases/SQLITE_PARITY_403_DML_WHERE_ORDER_LIMIT_016.rs) | P1 | memory | GEN_SQL_DML | 2508731 | 4569105 | <span style="color:#dc2626">-82.13%</span> |
| 362 | [00797 WINDOW_PARTITION_SUM_010](crates/bench/sqlite_parity/cases/SQLITE_PARITY_797_WINDOW_PARTITION_SUM_010.rs) | P2 | memory | GEN_SQL_WINDOW | 1863953 | 3394656 | <span style="color:#dc2626">-82.12%</span> |
| 363 | [00078 ON_CONFLICT_ALGORITHMS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_078_ON_CONFLICT_ALGORITHMS.rs) | P0 | memory | SQL_CONFLICT | 1882407 | 3427959 | <span style="color:#dc2626">-82.11%</span> |
| 364 | [00358 SCALAR_STRING_033](crates/bench/sqlite_parity/cases/SQLITE_PARITY_358_SCALAR_STRING_033.rs) | P1 | memory | GEN_SQL_SCALAR | 1689102 | 3075232 | <span style="color:#dc2626">-82.06%</span> |
| 365 | [00346 SCALAR_STRING_030](crates/bench/sqlite_parity/cases/SQLITE_PARITY_346_SCALAR_STRING_030.rs) | P1 | memory | GEN_SQL_SCALAR | 1905811 | 3468315 | <span style="color:#dc2626">-81.99%</span> |
| 366 | [00925 CONSTRAINT_FK_SAVEPOINT_058](crates/bench/sqlite_parity/cases/SQLITE_PARITY_925_CONSTRAINT_FK_SAVEPOINT_058.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1961848 | 3569566 | <span style="color:#dc2626">-81.95%</span> |
| 367 | [00822 WINDOW_PARTITION_SUM_035](crates/bench/sqlite_parity/cases/SQLITE_PARITY_822_WINDOW_PARTITION_SUM_035.rs) | P2 | memory | GEN_SQL_WINDOW | 1893058 | 3443708 | <span style="color:#dc2626">-81.91%</span> |
| 368 | [00254 SCALAR_STRING_007](crates/bench/sqlite_parity/cases/SQLITE_PARITY_254_SCALAR_STRING_007.rs) | P1 | memory | GEN_SQL_SCALAR | 1676308 | 3048631 | <span style="color:#dc2626">-81.87%</span> |
| 369 | [00348 SCALAR_ARITH_031](crates/bench/sqlite_parity/cases/SQLITE_PARITY_348_SCALAR_ARITH_031.rs) | P1 | memory | GEN_SQL_SCALAR | 1686938 | 3067648 | <span style="color:#dc2626">-81.85%</span> |
| 370 | [00806 WINDOW_PARTITION_SUM_019](crates/bench/sqlite_parity/cases/SQLITE_PARITY_806_WINDOW_PARTITION_SUM_019.rs) | P2 | memory | GEN_SQL_WINDOW | 1952841 | 3549879 | <span style="color:#dc2626">-81.78%</span> |
| 371 | [00874 CONSTRAINT_FK_SAVEPOINT_007](crates/bench/sqlite_parity/cases/SQLITE_PARITY_874_CONSTRAINT_FK_SAVEPOINT_007.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1994550 | 3623178 | <span style="color:#dc2626">-81.65%</span> |
| 372 | [00021 UPSERT_DO_NOTHING](crates/bench/sqlite_parity/cases/SQLITE_PARITY_021_UPSERT_DO_NOTHING.rs) | P0 | memory | SQL_UPSERT | 1827844 | 3319974 | <span style="color:#dc2626">-81.63%</span> |
| 373 | [00342 SCALAR_STRING_029](crates/bench/sqlite_parity/cases/SQLITE_PARITY_342_SCALAR_STRING_029.rs) | P1 | memory | GEN_SQL_SCALAR | 1742463 | 3162336 | <span style="color:#dc2626">-81.49%</span> |
| 374 | [00836 WINDOW_PARTITION_SUM_049](crates/bench/sqlite_parity/cases/SQLITE_PARITY_836_WINDOW_PARTITION_SUM_049.rs) | P2 | memory | GEN_SQL_WINDOW | 1897286 | 3442947 | <span style="color:#dc2626">-81.47%</span> |
| 375 | [00832 WINDOW_PARTITION_SUM_045](crates/bench/sqlite_parity/cases/SQLITE_PARITY_832_WINDOW_PARTITION_SUM_045.rs) | P2 | memory | GEN_SQL_WINDOW | 1898848 | 3444811 | <span style="color:#dc2626">-81.42%</span> |
| 376 | [00032 UNIQUE_INDEX_FAILURE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_032_UNIQUE_INDEX_FAILURE.rs) | P0 | memory | SQL_INDEX_NEGATIVE | 1759966 | 3188185 | <span style="color:#dc2626">-81.15%</span> |
| 377 | [00382 SCALAR_STRING_039](crates/bench/sqlite_parity/cases/SQLITE_PARITY_382_SCALAR_STRING_039.rs) | P1 | memory | GEN_SQL_SCALAR | 1749807 | 3169711 | <span style="color:#dc2626">-81.15%</span> |
| 378 | [00789 WINDOW_PARTITION_SUM_002](crates/bench/sqlite_parity/cases/SQLITE_PARITY_789_WINDOW_PARTITION_SUM_002.rs) | P2 | memory | GEN_SQL_WINDOW | 1949464 | 3530051 | <span style="color:#dc2626">-81.08%</span> |
| 379 | [00088 JSON_SCALAR_FUNCTIONS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_088_JSON_SCALAR_FUNCTIONS.rs) | P0 | memory | SQL_JSON | 1711204 | 3098425 | <span style="color:#dc2626">-81.07%</span> |
| 380 | [00033 PARTIAL_INDEX](crates/bench/sqlite_parity/cases/SQLITE_PARITY_033_PARTIAL_INDEX.rs) | P0 | memory | SQL_INDEX | 1839056 | 3328932 | <span style="color:#dc2626">-81.01%</span> |
| 381 | [00856 WINDOW_PARTITION_SUM_069](crates/bench/sqlite_parity/cases/SQLITE_PARITY_856_WINDOW_PARTITION_SUM_069.rs) | P2 | memory | GEN_SQL_WINDOW | 1960215 | 3546863 | <span style="color:#dc2626">-80.94%</span> |
| 382 | [00883 CONSTRAINT_FK_SAVEPOINT_016](crates/bench/sqlite_parity/cases/SQLITE_PARITY_883_CONSTRAINT_FK_SAVEPOINT_016.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2017143 | 3647854 | <span style="color:#dc2626">-80.84%</span> |
| 383 | [00068 CAST_AND_TYPE_AFFINITY](crates/bench/sqlite_parity/cases/SQLITE_PARITY_068_CAST_AND_TYPE_AFFINITY.rs) | P0 | memory | SQL_EXPRESSIONS | 1842261 | 3331045 | <span style="color:#dc2626">-80.81%</span> |
| 384 | [00461 DML_WHERE_ORDER_LIMIT_074](crates/bench/sqlite_parity/cases/SQLITE_PARITY_461_DML_WHERE_ORDER_LIMIT_074.rs) | P1 | memory | GEN_SQL_DML | 2172856 | 3926722 | <span style="color:#dc2626">-80.72%</span> |
| 385 | [00018 DELETE_RETURNING](crates/bench/sqlite_parity/cases/SQLITE_PARITY_018_DELETE_RETURNING.rs) | P0 | memory | SQL_DELETE | 1875564 | 3388143 | <span style="color:#dc2626">-80.65%</span> |
| 386 | [00321 SCALAR_CAST_TYPEOF_024](crates/bench/sqlite_parity/cases/SQLITE_PARITY_321_SCALAR_CAST_TYPEOF_024.rs) | P1 | memory | GEN_SQL_SCALAR | 1685014 | 3043692 | <span style="color:#dc2626">-80.63%</span> |
| 387 | [00300 SCALAR_ARITH_019](crates/bench/sqlite_parity/cases/SQLITE_PARITY_300_SCALAR_ARITH_019.rs) | P1 | memory | GEN_SQL_SCALAR | 1679915 | 3034324 | <span style="color:#dc2626">-80.62%</span> |
| 388 | [00804 WINDOW_PARTITION_SUM_017](crates/bench/sqlite_parity/cases/SQLITE_PARITY_804_WINDOW_PARTITION_SUM_017.rs) | P2 | memory | GEN_SQL_WINDOW | 1884692 | 3403502 | <span style="color:#dc2626">-80.59%</span> |
| 389 | [00396 DML_WHERE_ORDER_LIMIT_009](crates/bench/sqlite_parity/cases/SQLITE_PARITY_396_DML_WHERE_ORDER_LIMIT_009.rs) | P1 | memory | GEN_SQL_DML | 2328932 | 4202111 | <span style="color:#dc2626">-80.43%</span> |
| 390 | [00090 JSON_MUTATION_FUNCTIONS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_090_JSON_MUTATION_FUNCTIONS.rs) | P0 | memory | SQL_JSON | 1695043 | 3056988 | <span style="color:#dc2626">-80.35%</span> |
| 391 | [00940 CONSTRAINT_FK_SAVEPOINT_073](crates/bench/sqlite_parity/cases/SQLITE_PARITY_940_CONSTRAINT_FK_SAVEPOINT_073.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2038392 | 3672471 | <span style="color:#dc2626">-80.17%</span> |
| 392 | [00314 SCALAR_STRING_022](crates/bench/sqlite_parity/cases/SQLITE_PARITY_314_SCALAR_STRING_022.rs) | P1 | memory | GEN_SQL_SCALAR | 1709170 | 3078948 | <span style="color:#dc2626">-80.14%</span> |
| 393 | [00949 VIEW_TRIGGER_GENERATED_002](crates/bench/sqlite_parity/cases/SQLITE_PARITY_949_VIEW_TRIGGER_GENERATED_002.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2166464 | 3899189 | <span style="color:#dc2626">-79.98%</span> |
| 394 | [00039 CREATE_TRIGGER_BEFORE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_039_CREATE_TRIGGER_BEFORE.rs) | P0 | memory | SQL_TRIGGER | 1955656 | 3519201 | <span style="color:#dc2626">-79.95%</span> |
| 395 | [00354 SCALAR_STRING_032](crates/bench/sqlite_parity/cases/SQLITE_PARITY_354_SCALAR_STRING_032.rs) | P1 | memory | GEN_SQL_SCALAR | 1818948 | 3271323 | <span style="color:#dc2626">-79.85%</span> |
| 396 | [00838 WINDOW_PARTITION_SUM_051](crates/bench/sqlite_parity/cases/SQLITE_PARITY_838_WINDOW_PARTITION_SUM_051.rs) | P2 | memory | GEN_SQL_WINDOW | 1913797 | 3439440 | <span style="color:#dc2626">-79.72%</span> |
| 397 | [00848 WINDOW_PARTITION_SUM_061](crates/bench/sqlite_parity/cases/SQLITE_PARITY_848_WINDOW_PARTITION_SUM_061.rs) | P2 | memory | GEN_SQL_WINDOW | 1932503 | 3471160 | <span style="color:#dc2626">-79.62%</span> |
| 398 | [00369 SCALAR_CAST_TYPEOF_036](crates/bench/sqlite_parity/cases/SQLITE_PARITY_369_SCALAR_CAST_TYPEOF_036.rs) | P1 | memory | GEN_SQL_SCALAR | 1698440 | 3050635 | <span style="color:#dc2626">-79.61%</span> |
| 399 | [00011 CREATE_TABLE_AS_SELECT](crates/bench/sqlite_parity/cases/SQLITE_PARITY_011_CREATE_TABLE_AS_SELECT.rs) | P0 | memory | SQL_DDL | 1777869 | 3191672 | <span style="color:#dc2626">-79.52%</span> |
| 400 | [00615 JOIN_SUBQUERY_EXISTS_008](crates/bench/sqlite_parity/cases/SQLITE_PARITY_615_JOIN_SUBQUERY_EXISTS_008.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2138732 | 3839266 | <span style="color:#dc2626">-79.51%</span> |
| 401 | [00943 CONSTRAINT_FK_SAVEPOINT_076](crates/bench/sqlite_parity/cases/SQLITE_PARITY_943_CONSTRAINT_FK_SAVEPOINT_076.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2024546 | 3633366 | <span style="color:#dc2626">-79.47%</span> |
| 402 | [00858 WINDOW_PARTITION_SUM_071](crates/bench/sqlite_parity/cases/SQLITE_PARITY_858_WINDOW_PARTITION_SUM_071.rs) | P2 | memory | GEN_SQL_WINDOW | 1941590 | 3484345 | <span style="color:#dc2626">-79.46%</span> |
| 403 | [00248 SCALAR_ARITH_006](crates/bench/sqlite_parity/cases/SQLITE_PARITY_248_SCALAR_ARITH_006.rs) | P1 | memory | GEN_SQL_SCALAR | 1667772 | 2990211 | <span style="color:#dc2626">-79.29%</span> |
| 404 | [00237 SCALAR_CAST_TYPEOF_003](crates/bench/sqlite_parity/cases/SQLITE_PARITY_237_SCALAR_CAST_TYPEOF_003.rs) | P1 | memory | GEN_SQL_SCALAR | 1760046 | 3154963 | <span style="color:#dc2626">-79.25%</span> |
| 405 | [00827 WINDOW_PARTITION_SUM_040](crates/bench/sqlite_parity/cases/SQLITE_PARITY_827_WINDOW_PARTITION_SUM_040.rs) | P2 | memory | GEN_SQL_WINDOW | 1945637 | 3486419 | <span style="color:#dc2626">-79.19%</span> |
| 406 | [00277 SCALAR_CAST_TYPEOF_013](crates/bench/sqlite_parity/cases/SQLITE_PARITY_277_SCALAR_CAST_TYPEOF_013.rs) | P1 | memory | GEN_SQL_SCALAR | 1762791 | 3156606 | <span style="color:#dc2626">-79.07%</span> |
| 407 | [00266 SCALAR_STRING_010](crates/bench/sqlite_parity/cases/SQLITE_PARITY_266_SCALAR_STRING_010.rs) | P1 | memory | GEN_SQL_SCALAR | 1870856 | 3343809 | <span style="color:#dc2626">-78.73%</span> |
| 408 | [00705 JOIN_SUBQUERY_EXISTS_098](crates/bench/sqlite_parity/cases/SQLITE_PARITY_705_JOIN_SUBQUERY_EXISTS_098.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2163599 | 3866337 | <span style="color:#dc2626">-78.70%</span> |
| 409 | [00333 SCALAR_CAST_TYPEOF_027](crates/bench/sqlite_parity/cases/SQLITE_PARITY_333_SCALAR_CAST_TYPEOF_027.rs) | P1 | memory | GEN_SQL_SCALAR | 1724189 | 3080752 | <span style="color:#dc2626">-78.68%</span> |
| 410 | [00978 VIEW_TRIGGER_GENERATED_031](crates/bench/sqlite_parity/cases/SQLITE_PARITY_978_VIEW_TRIGGER_GENERATED_031.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2253128 | 4025789 | <span style="color:#dc2626">-78.68%</span> |
| 411 | [00859 WINDOW_PARTITION_SUM_072](crates/bench/sqlite_parity/cases/SQLITE_PARITY_859_WINDOW_PARTITION_SUM_072.rs) | P2 | memory | GEN_SQL_WINDOW | 1941589 | 3469096 | <span style="color:#dc2626">-78.67%</span> |
| 412 | [00802 WINDOW_PARTITION_SUM_015](crates/bench/sqlite_parity/cases/SQLITE_PARITY_802_WINDOW_PARTITION_SUM_015.rs) | P2 | memory | GEN_SQL_WINDOW | 1918886 | 3428269 | <span style="color:#dc2626">-78.66%</span> |
| 413 | [00019 REPLACE_INTO](crates/bench/sqlite_parity/cases/SQLITE_PARITY_019_REPLACE_INTO.rs) | P0 | memory | SQL_REPLACE | 1905982 | 3404574 | <span style="color:#dc2626">-78.63%</span> |
| 414 | [00256 SCALAR_ARITH_008](crates/bench/sqlite_parity/cases/SQLITE_PARITY_256_SCALAR_ARITH_008.rs) | P1 | memory | GEN_SQL_SCALAR | 1713949 | 3061456 | <span style="color:#dc2626">-78.62%</span> |
| 415 | [00324 SCALAR_ARITH_025](crates/bench/sqlite_parity/cases/SQLITE_PARITY_324_SCALAR_ARITH_025.rs) | P1 | memory | GEN_SQL_SCALAR | 1667040 | 2976595 | <span style="color:#dc2626">-78.56%</span> |
| 416 | [00249 SCALAR_CAST_TYPEOF_006](crates/bench/sqlite_parity/cases/SQLITE_PARITY_249_SCALAR_CAST_TYPEOF_006.rs) | P1 | memory | GEN_SQL_SCALAR | 1787438 | 3189458 | <span style="color:#dc2626">-78.44%</span> |
| 417 | [00855 WINDOW_PARTITION_SUM_068](crates/bench/sqlite_parity/cases/SQLITE_PARITY_855_WINDOW_PARTITION_SUM_068.rs) | P2 | memory | GEN_SQL_WINDOW | 1952941 | 3484255 | <span style="color:#dc2626">-78.41%</span> |
| 418 | [00015 UPDATE_BASIC](crates/bench/sqlite_parity/cases/SQLITE_PARITY_015_UPDATE_BASIC.rs) | P0 | memory | SQL_UPDATE | 1899961 | 3387081 | <span style="color:#dc2626">-78.27%</span> |
| 419 | [00245 SCALAR_CAST_TYPEOF_005](crates/bench/sqlite_parity/cases/SQLITE_PARITY_245_SCALAR_CAST_TYPEOF_005.rs) | P1 | memory | GEN_SQL_SCALAR | 1736953 | 3095670 | <span style="color:#dc2626">-78.22%</span> |
| 420 | [00381 SCALAR_CAST_TYPEOF_039](crates/bench/sqlite_parity/cases/SQLITE_PARITY_381_SCALAR_CAST_TYPEOF_039.rs) | P1 | memory | GEN_SQL_SCALAR | 1725982 | 3074781 | <span style="color:#dc2626">-78.15%</span> |
| 421 | [00313 SCALAR_CAST_TYPEOF_022](crates/bench/sqlite_parity/cases/SQLITE_PARITY_313_SCALAR_CAST_TYPEOF_022.rs) | P1 | memory | GEN_SQL_SCALAR | 1728216 | 3078067 | <span style="color:#dc2626">-78.11%</span> |
| 422 | [00805 WINDOW_PARTITION_SUM_018](crates/bench/sqlite_parity/cases/SQLITE_PARITY_805_WINDOW_PARTITION_SUM_018.rs) | P2 | memory | GEN_SQL_WINDOW | 1940758 | 3454589 | <span style="color:#dc2626">-78.00%</span> |
| 423 | [00084 CORE_FORMAT_QUOTE_HEX](crates/bench/sqlite_parity/cases/SQLITE_PARITY_084_CORE_FORMAT_QUOTE_HEX.rs) | P0 | memory | SQL_FUNCTIONS | 1703640 | 3031840 | <span style="color:#dc2626">-77.96%</span> |
| 424 | [00907 CONSTRAINT_FK_SAVEPOINT_040](crates/bench/sqlite_parity/cases/SQLITE_PARITY_907_CONSTRAINT_FK_SAVEPOINT_040.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1986725 | 3532756 | <span style="color:#dc2626">-77.82%</span> |
| 425 | [00289 SCALAR_CAST_TYPEOF_016](crates/bench/sqlite_parity/cases/SQLITE_PARITY_289_SCALAR_CAST_TYPEOF_016.rs) | P1 | memory | GEN_SQL_SCALAR | 1793870 | 3189217 | <span style="color:#dc2626">-77.78%</span> |
| 426 | [00304 SCALAR_ARITH_020](crates/bench/sqlite_parity/cases/SQLITE_PARITY_304_SCALAR_ARITH_020.rs) | P1 | memory | GEN_SQL_SCALAR | 1652663 | 2936760 | <span style="color:#dc2626">-77.70%</span> |
| 427 | [00845 WINDOW_PARTITION_SUM_058](crates/bench/sqlite_parity/cases/SQLITE_PARITY_845_WINDOW_PARTITION_SUM_058.rs) | P2 | memory | GEN_SQL_WINDOW | 1934897 | 3435192 | <span style="color:#dc2626">-77.54%</span> |
| 428 | [00262 SCALAR_STRING_009](crates/bench/sqlite_parity/cases/SQLITE_PARITY_262_SCALAR_STRING_009.rs) | P1 | memory | GEN_SQL_SCALAR | 1760907 | 3126169 | <span style="color:#dc2626">-77.53%</span> |
| 429 | [00795 WINDOW_PARTITION_SUM_008](crates/bench/sqlite_parity/cases/SQLITE_PARITY_795_WINDOW_PARTITION_SUM_008.rs) | P2 | memory | GEN_SQL_WINDOW | 1978880 | 3510775 | <span style="color:#dc2626">-77.41%</span> |
| 430 | [00791 WINDOW_PARTITION_SUM_004](crates/bench/sqlite_parity/cases/SQLITE_PARITY_791_WINDOW_PARTITION_SUM_004.rs) | P2 | memory | GEN_SQL_WINDOW | 1937311 | 3436124 | <span style="color:#dc2626">-77.37%</span> |
| 431 | [00173 OPT_LIST_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_173_OPT_LIST_MODE.rs) | P1 | memory | CLI_OPTION | 1685676 | 2989380 | <span style="color:#dc2626">-77.34%</span> |
| 432 | [00330 SCALAR_STRING_026](crates/bench/sqlite_parity/cases/SQLITE_PARITY_330_SCALAR_STRING_026.rs) | P1 | memory | GEN_SQL_SCALAR | 1714581 | 3038041 | <span style="color:#dc2626">-77.19%</span> |
| 433 | [00823 WINDOW_PARTITION_SUM_036](crates/bench/sqlite_parity/cases/SQLITE_PARITY_823_WINDOW_PARTITION_SUM_036.rs) | P2 | memory | GEN_SQL_WINDOW | 1933544 | 3423591 | <span style="color:#dc2626">-77.06%</span> |
| 434 | [00012 INSERT_DEFAULT_VALUES](crates/bench/sqlite_parity/cases/SQLITE_PARITY_012_INSERT_DEFAULT_VALUES.rs) | P0 | memory | SQL_INSERT | 1802116 | 3190560 | <span style="color:#dc2626">-77.05%</span> |
| 435 | [00651 JOIN_SUBQUERY_EXISTS_044](crates/bench/sqlite_parity/cases/SQLITE_PARITY_651_JOIN_SUBQUERY_EXISTS_044.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2155354 | 3813477 | <span style="color:#dc2626">-76.93%</span> |
| 436 | [00368 SCALAR_ARITH_036](crates/bench/sqlite_parity/cases/SQLITE_PARITY_368_SCALAR_ARITH_036.rs) | P1 | memory | GEN_SQL_SCALAR | 1677009 | 2965906 | <span style="color:#dc2626">-76.86%</span> |
| 437 | [00246 SCALAR_STRING_005](crates/bench/sqlite_parity/cases/SQLITE_PARITY_246_SCALAR_STRING_005.rs) | P1 | memory | GEN_SQL_SCALAR | 1752482 | 3096883 | <span style="color:#dc2626">-76.71%</span> |
| 438 | [00999 VIEW_TRIGGER_GENERATED_052](crates/bench/sqlite_parity/cases/SQLITE_PARITY_999_VIEW_TRIGGER_GENERATED_052.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2274679 | 4017493 | <span style="color:#dc2626">-76.62%</span> |
| 439 | [00361 SCALAR_CAST_TYPEOF_034](crates/bench/sqlite_parity/cases/SQLITE_PARITY_361_SCALAR_CAST_TYPEOF_034.rs) | P1 | memory | GEN_SQL_SCALAR | 1744027 | 3079901 | <span style="color:#dc2626">-76.60%</span> |
| 440 | [00309 SCALAR_CAST_TYPEOF_021](crates/bench/sqlite_parity/cases/SQLITE_PARITY_309_SCALAR_CAST_TYPEOF_021.rs) | P1 | memory | GEN_SQL_SCALAR | 1748394 | 3085261 | <span style="color:#dc2626">-76.46%</span> |
| 441 | [00257 SCALAR_CAST_TYPEOF_008](crates/bench/sqlite_parity/cases/SQLITE_PARITY_257_SCALAR_CAST_TYPEOF_008.rs) | P1 | memory | GEN_SQL_SCALAR | 1780725 | 3142028 | <span style="color:#dc2626">-76.45%</span> |
| 442 | [00261 SCALAR_CAST_TYPEOF_009](crates/bench/sqlite_parity/cases/SQLITE_PARITY_261_SCALAR_CAST_TYPEOF_009.rs) | P1 | memory | GEN_SQL_SCALAR | 1756579 | 3097815 | <span style="color:#dc2626">-76.36%</span> |
| 443 | [00085 CORE_RANDOM_SHAPE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_085_CORE_RANDOM_SHAPE.rs) | P0 | memory | SQL_FUNCTIONS | 1739417 | 3063700 | <span style="color:#dc2626">-76.13%</span> |
| 444 | [00020 UPSERT_DO_UPDATE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_020_UPSERT_DO_UPDATE.rs) | P0 | memory | SQL_UPSERT | 1898518 | 3343920 | <span style="color:#dc2626">-76.13%</span> |
| 445 | [00385 SCALAR_CAST_TYPEOF_040](crates/bench/sqlite_parity/cases/SQLITE_PARITY_385_SCALAR_CAST_TYPEOF_040.rs) | P1 | memory | GEN_SQL_SCALAR | 1717546 | 3024686 | <span style="color:#dc2626">-76.11%</span> |
| 446 | [00317 SCALAR_CAST_TYPEOF_023](crates/bench/sqlite_parity/cases/SQLITE_PARITY_317_SCALAR_CAST_TYPEOF_023.rs) | P1 | memory | GEN_SQL_SCALAR | 1780124 | 3134243 | <span style="color:#dc2626">-76.07%</span> |
| 447 | [00228 SCALAR_ARITH_001](crates/bench/sqlite_parity/cases/SQLITE_PARITY_228_SCALAR_ARITH_001.rs) | P1 | memory | GEN_SQL_SCALAR | 1715271 | 3016942 | <span style="color:#dc2626">-75.89%</span> |
| 448 | [00234 SCALAR_STRING_002](crates/bench/sqlite_parity/cases/SQLITE_PARITY_234_SCALAR_STRING_002.rs) | P1 | memory | GEN_SQL_SCALAR | 1740870 | 3059182 | <span style="color:#dc2626">-75.73%</span> |
| 449 | [00110 DOT_MODE_LINE_COLUMN_TABLE_BOX_MARKDOWN](crates/bench/sqlite_parity/cases/SQLITE_PARITY_110_DOT_MODE_LINE_COLUMN_TABLE_BOX_MARKDOWN.rs) | P0 | memory | CLI_DOT_COMMAND | 1748023 | 3067447 | <span style="color:#dc2626">-75.48%</span> |
| 450 | [00868 CONSTRAINT_FK_SAVEPOINT_001](crates/bench/sqlite_parity/cases/SQLITE_PARITY_868_CONSTRAINT_FK_SAVEPOINT_001.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2082747 | 3651681 | <span style="color:#dc2626">-75.33%</span> |
| 451 | [00181 OPT_TABLE_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_181_OPT_TABLE_MODE.rs) | P2 | memory | CLI_OPTION | 1692298 | 2966787 | <span style="color:#dc2626">-75.31%</span> |
| 452 | [00372 SCALAR_ARITH_037](crates/bench/sqlite_parity/cases/SQLITE_PARITY_372_SCALAR_ARITH_037.rs) | P1 | memory | GEN_SQL_SCALAR | 1718658 | 3010089 | <span style="color:#dc2626">-75.14%</span> |
| 453 | [00297 SCALAR_CAST_TYPEOF_018](crates/bench/sqlite_parity/cases/SQLITE_PARITY_297_SCALAR_CAST_TYPEOF_018.rs) | P1 | memory | GEN_SQL_SCALAR | 1769354 | 3095971 | <span style="color:#dc2626">-74.98%</span> |
| 454 | [00380 SCALAR_ARITH_039](crates/bench/sqlite_parity/cases/SQLITE_PARITY_380_SCALAR_ARITH_039.rs) | P1 | memory | GEN_SQL_SCALAR | 1745368 | 3053640 | <span style="color:#dc2626">-74.96%</span> |
| 455 | [00184 OPT_ASCII_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_184_OPT_ASCII_MODE.rs) | P2 | memory | CLI_OPTION | 1684243 | 2946508 | <span style="color:#dc2626">-74.95%</span> |
| 456 | [00038 CREATE_TRIGGER_AFTER](crates/bench/sqlite_parity/cases/SQLITE_PARITY_038_CREATE_TRIGGER_AFTER.rs) | P0 | memory | SQL_TRIGGER | 2011481 | 3518149 | <span style="color:#dc2626">-74.90%</span> |
| 457 | [00798 WINDOW_PARTITION_SUM_011](crates/bench/sqlite_parity/cases/SQLITE_PARITY_798_WINDOW_PARTITION_SUM_011.rs) | P2 | memory | GEN_SQL_WINDOW | 1975664 | 3454509 | <span style="color:#dc2626">-74.85%</span> |
| 458 | [00338 SCALAR_STRING_028](crates/bench/sqlite_parity/cases/SQLITE_PARITY_338_SCALAR_STRING_028.rs) | P1 | memory | GEN_SQL_SCALAR | 1743095 | 3047840 | <span style="color:#dc2626">-74.85%</span> |
| 459 | [00422 DML_WHERE_ORDER_LIMIT_035](crates/bench/sqlite_parity/cases/SQLITE_PARITY_422_DML_WHERE_ORDER_LIMIT_035.rs) | P1 | memory | GEN_SQL_DML | 2236777 | 3906133 | <span style="color:#dc2626">-74.63%</span> |
| 460 | [00344 SCALAR_ARITH_030](crates/bench/sqlite_parity/cases/SQLITE_PARITY_344_SCALAR_ARITH_030.rs) | P1 | memory | GEN_SQL_SCALAR | 1722075 | 3004738 | <span style="color:#dc2626">-74.48%</span> |
| 461 | [00405 DML_WHERE_ORDER_LIMIT_018](crates/bench/sqlite_parity/cases/SQLITE_PARITY_405_DML_WHERE_ORDER_LIMIT_018.rs) | P1 | memory | GEN_SQL_DML | 2286270 | 3988457 | <span style="color:#dc2626">-74.45%</span> |
| 462 | [00834 WINDOW_PARTITION_SUM_047](crates/bench/sqlite_parity/cases/SQLITE_PARITY_834_WINDOW_PARTITION_SUM_047.rs) | P2 | memory | GEN_SQL_WINDOW | 1966396 | 3429201 | <span style="color:#dc2626">-74.39%</span> |
| 463 | [00177 OPT_JSON_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_177_OPT_JSON_MODE.rs) | P1 | memory | CLI_OPTION | 1720372 | 2999799 | <span style="color:#dc2626">-74.37%</span> |
| 464 | [00258 SCALAR_STRING_008](crates/bench/sqlite_parity/cases/SQLITE_PARITY_258_SCALAR_STRING_008.rs) | P1 | memory | GEN_SQL_SCALAR | 1700103 | 2963310 | <span style="color:#dc2626">-74.30%</span> |
| 465 | [00337 SCALAR_CAST_TYPEOF_028](crates/bench/sqlite_parity/cases/SQLITE_PARITY_337_SCALAR_CAST_TYPEOF_028.rs) | P1 | memory | GEN_SQL_SCALAR | 1729759 | 3014658 | <span style="color:#dc2626">-74.28%</span> |
| 466 | [00051 PRAGMA_INDEX_LIST_FUNCTION](crates/bench/sqlite_parity/cases/SQLITE_PARITY_051_PRAGMA_INDEX_LIST_FUNCTION.rs) | P0 | memory | SQL_PRAGMA | 1930548 | 3364158 | <span style="color:#dc2626">-74.26%</span> |
| 467 | [00123 DOT_ECHO](crates/bench/sqlite_parity/cases/SQLITE_PARITY_123_DOT_ECHO.rs) | P0 | memory | CLI_DOT_COMMAND | 1665698 | 2901924 | <span style="color:#dc2626">-74.22%</span> |
| 468 | [00229 SCALAR_CAST_TYPEOF_001](crates/bench/sqlite_parity/cases/SQLITE_PARITY_229_SCALAR_CAST_TYPEOF_001.rs) | P1 | memory | GEN_SQL_SCALAR | 1811383 | 3154382 | <span style="color:#dc2626">-74.14%</span> |
| 469 | [00308 SCALAR_ARITH_021](crates/bench/sqlite_parity/cases/SQLITE_PARITY_308_SCALAR_ARITH_021.rs) | P1 | memory | GEN_SQL_SCALAR | 1712978 | 2982647 | <span style="color:#dc2626">-74.12%</span> |
| 470 | [00374 SCALAR_STRING_037](crates/bench/sqlite_parity/cases/SQLITE_PARITY_374_SCALAR_STRING_037.rs) | P1 | memory | GEN_SQL_SCALAR | 1749296 | 3045866 | <span style="color:#dc2626">-74.12%</span> |
| 471 | [00238 SCALAR_STRING_003](crates/bench/sqlite_parity/cases/SQLITE_PARITY_238_SCALAR_STRING_003.rs) | P1 | memory | GEN_SQL_SCALAR | 1817755 | 3163799 | <span style="color:#dc2626">-74.05%</span> |
| 472 | [00064 CTE_NON_RECURSIVE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_064_CTE_NON_RECURSIVE.rs) | P0 | memory | SQL_CTE | 1725771 | 3003436 | <span style="color:#dc2626">-74.03%</span> |
| 473 | [00265 SCALAR_CAST_TYPEOF_010](crates/bench/sqlite_parity/cases/SQLITE_PARITY_265_SCALAR_CAST_TYPEOF_010.rs) | P1 | memory | GEN_SQL_SCALAR | 1802808 | 3137299 | <span style="color:#dc2626">-74.02%</span> |
| 474 | [00035 DROP_INDEX](crates/bench/sqlite_parity/cases/SQLITE_PARITY_035_DROP_INDEX.rs) | P0 | memory | SQL_DROP | 1927192 | 3352846 | <span style="color:#dc2626">-73.98%</span> |
| 475 | [00302 SCALAR_STRING_019](crates/bench/sqlite_parity/cases/SQLITE_PARITY_302_SCALAR_STRING_019.rs) | P1 | memory | GEN_SQL_SCALAR | 1726704 | 3003336 | <span style="color:#dc2626">-73.93%</span> |
| 476 | [00340 SCALAR_ARITH_029](crates/bench/sqlite_parity/cases/SQLITE_PARITY_340_SCALAR_ARITH_029.rs) | P1 | memory | GEN_SQL_SCALAR | 1723127 | 2996332 | <span style="color:#dc2626">-73.89%</span> |
| 477 | [00334 SCALAR_STRING_027](crates/bench/sqlite_parity/cases/SQLITE_PARITY_334_SCALAR_STRING_027.rs) | P1 | memory | GEN_SQL_SCALAR | 1759866 | 3059622 | <span style="color:#dc2626">-73.86%</span> |
| 478 | [00384 SCALAR_ARITH_040](crates/bench/sqlite_parity/cases/SQLITE_PARITY_384_SCALAR_ARITH_040.rs) | P1 | memory | GEN_SQL_SCALAR | 1704010 | 2959814 | <span style="color:#dc2626">-73.70%</span> |
| 479 | [00089 JSON_TABLE_VALUED_FUNCTIONS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_089_JSON_TABLE_VALUED_FUNCTIONS.rs) | P0 | memory | SQL_JSON | 1726433 | 2997415 | <span style="color:#dc2626">-73.62%</span> |
| 480 | [00345 SCALAR_CAST_TYPEOF_030](crates/bench/sqlite_parity/cases/SQLITE_PARITY_345_SCALAR_CAST_TYPEOF_030.rs) | P1 | memory | GEN_SQL_SCALAR | 1697959 | 2943583 | <span style="color:#dc2626">-73.36%</span> |
| 481 | [00240 SCALAR_ARITH_004](crates/bench/sqlite_parity/cases/SQLITE_PARITY_240_SCALAR_ARITH_004.rs) | P1 | memory | GEN_SQL_SCALAR | 1729058 | 2996954 | <span style="color:#dc2626">-73.33%</span> |
| 482 | [00086 DATE_TIME_FUNCTIONS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_086_DATE_TIME_FUNCTIONS.rs) | P0 | memory | SQL_FUNCTIONS | 1755127 | 3039364 | <span style="color:#dc2626">-73.17%</span> |
| 483 | [00329 SCALAR_CAST_TYPEOF_026](crates/bench/sqlite_parity/cases/SQLITE_PARITY_329_SCALAR_CAST_TYPEOF_026.rs) | P1 | memory | GEN_SQL_SCALAR | 1768612 | 3060524 | <span style="color:#dc2626">-73.05%</span> |
| 484 | [00242 SCALAR_STRING_004](crates/bench/sqlite_parity/cases/SQLITE_PARITY_242_SCALAR_STRING_004.rs) | P1 | memory | GEN_SQL_SCALAR | 1763463 | 3049483 | <span style="color:#dc2626">-72.93%</span> |
| 485 | [00966 VIEW_TRIGGER_GENERATED_019](crates/bench/sqlite_parity/cases/SQLITE_PARITY_966_VIEW_TRIGGER_GENERATED_019.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2337328 | 4040806 | <span style="color:#dc2626">-72.88%</span> |
| 486 | [00322 SCALAR_STRING_024](crates/bench/sqlite_parity/cases/SQLITE_PARITY_322_SCALAR_STRING_024.rs) | P1 | memory | GEN_SQL_SCALAR | 1789382 | 3093026 | <span style="color:#dc2626">-72.85%</span> |
| 487 | [00821 WINDOW_PARTITION_SUM_034](crates/bench/sqlite_parity/cases/SQLITE_PARITY_821_WINDOW_PARTITION_SUM_034.rs) | P2 | memory | GEN_SQL_WINDOW | 2022372 | 3494775 | <span style="color:#dc2626">-72.81%</span> |
| 488 | [00232 SCALAR_ARITH_002](crates/bench/sqlite_parity/cases/SQLITE_PARITY_232_SCALAR_ARITH_002.rs) | P1 | memory | GEN_SQL_SCALAR | 1726402 | 2982908 | <span style="color:#dc2626">-72.78%</span> |
| 489 | [00353 SCALAR_CAST_TYPEOF_032](crates/bench/sqlite_parity/cases/SQLITE_PARITY_353_SCALAR_CAST_TYPEOF_032.rs) | P1 | memory | GEN_SQL_SCALAR | 1781527 | 3075292 | <span style="color:#dc2626">-72.62%</span> |
| 490 | [00305 SCALAR_CAST_TYPEOF_020](crates/bench/sqlite_parity/cases/SQLITE_PARITY_305_SCALAR_CAST_TYPEOF_020.rs) | P1 | memory | GEN_SQL_SCALAR | 1781697 | 3075051 | <span style="color:#dc2626">-72.59%</span> |
| 491 | [00185 OPT_SEPARATOR](crates/bench/sqlite_parity/cases/SQLITE_PARITY_185_OPT_SEPARATOR.rs) | P1 | memory | CLI_OPTION | 1696877 | 2928454 | <span style="color:#dc2626">-72.58%</span> |
| 492 | [00332 SCALAR_ARITH_027](crates/bench/sqlite_parity/cases/SQLITE_PARITY_332_SCALAR_ARITH_027.rs) | P1 | memory | GEN_SQL_SCALAR | 1699592 | 2932662 | <span style="color:#dc2626">-72.55%</span> |
| 493 | [00818 WINDOW_PARTITION_SUM_031](crates/bench/sqlite_parity/cases/SQLITE_PARITY_818_WINDOW_PARTITION_SUM_031.rs) | P2 | memory | GEN_SQL_WINDOW | 2067317 | 3566160 | <span style="color:#dc2626">-72.50%</span> |
| 494 | [00236 SCALAR_ARITH_003](crates/bench/sqlite_parity/cases/SQLITE_PARITY_236_SCALAR_ARITH_003.rs) | P1 | memory | GEN_SQL_SCALAR | 1712917 | 2954183 | <span style="color:#dc2626">-72.47%</span> |
| 495 | [00180 OPT_BOX_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_180_OPT_BOX_MODE.rs) | P2 | memory | CLI_OPTION | 1668754 | 2877729 | <span style="color:#dc2626">-72.45%</span> |
| 496 | [00860 WINDOW_PARTITION_SUM_073](crates/bench/sqlite_parity/cases/SQLITE_PARITY_860_WINDOW_PARTITION_SUM_073.rs) | P2 | memory | GEN_SQL_WINDOW | 2011942 | 3469086 | <span style="color:#dc2626">-72.42%</span> |
| 497 | [00328 SCALAR_ARITH_026](crates/bench/sqlite_parity/cases/SQLITE_PARITY_328_SCALAR_ARITH_026.rs) | P1 | memory | GEN_SQL_SCALAR | 1699713 | 2929727 | <span style="color:#dc2626">-72.37%</span> |
| 498 | [00233 SCALAR_CAST_TYPEOF_002](crates/bench/sqlite_parity/cases/SQLITE_PARITY_233_SCALAR_CAST_TYPEOF_002.rs) | P1 | memory | GEN_SQL_SCALAR | 1786216 | 3078668 | <span style="color:#dc2626">-72.36%</span> |
| 499 | [00312 SCALAR_ARITH_022](crates/bench/sqlite_parity/cases/SQLITE_PARITY_312_SCALAR_ARITH_022.rs) | P1 | memory | GEN_SQL_SCALAR | 1699592 | 2929336 | <span style="color:#dc2626">-72.36%</span> |
| 500 | [00178 OPT_HTML_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_178_OPT_HTML_MODE.rs) | P2 | memory | CLI_OPTION | 1686166 | 2902656 | <span style="color:#dc2626">-72.15%</span> |
| 501 | [00113 DOT_NULLVALUE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_113_DOT_NULLVALUE.rs) | P0 | memory | CLI_DOT_COMMAND | 1747973 | 3001803 | <span style="color:#dc2626">-71.73%</span> |
| 502 | [00175 OPT_QUOTE_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_175_OPT_QUOTE_MODE.rs) | P1 | memory | CLI_OPTION | 1816703 | 3108435 | <span style="color:#dc2626">-71.10%</span> |
| 503 | [00182 OPT_COLUMN_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_182_OPT_COLUMN_MODE.rs) | P2 | memory | CLI_OPTION | 1729549 | 2956236 | <span style="color:#dc2626">-70.93%</span> |
| 504 | [00692 JOIN_SUBQUERY_EXISTS_085](crates/bench/sqlite_parity/cases/SQLITE_PARITY_692_JOIN_SUBQUERY_EXISTS_085.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2240354 | 3828486 | <span style="color:#dc2626">-70.89%</span> |
| 505 | [00325 SCALAR_CAST_TYPEOF_025](crates/bench/sqlite_parity/cases/SQLITE_PARITY_325_SCALAR_CAST_TYPEOF_025.rs) | P1 | memory | GEN_SQL_SCALAR | 1802597 | 3078728 | <span style="color:#dc2626">-70.79%</span> |
| 506 | [00301 SCALAR_CAST_TYPEOF_019](crates/bench/sqlite_parity/cases/SQLITE_PARITY_301_SCALAR_CAST_TYPEOF_019.rs) | P1 | memory | GEN_SQL_SCALAR | 1797667 | 3063580 | <span style="color:#dc2626">-70.42%</span> |
| 507 | [00174 OPT_CSV_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_174_OPT_CSV_MODE.rs) | P1 | memory | CLI_OPTION | 1713578 | 2915159 | <span style="color:#dc2626">-70.12%</span> |
| 508 | [00223 OPT_NOHEADER](crates/bench/sqlite_parity/cases/SQLITE_PARITY_223_OPT_NOHEADER.rs) | P2 | memory | CLI_OPTION | 1733756 | 2948763 | <span style="color:#dc2626">-70.08%</span> |
| 509 | [00811 WINDOW_PARTITION_SUM_024](crates/bench/sqlite_parity/cases/SQLITE_PARITY_811_WINDOW_PARTITION_SUM_024.rs) | P2 | memory | GEN_SQL_WINDOW | 2041288 | 3466391 | <span style="color:#dc2626">-69.81%</span> |
| 510 | [00366 SCALAR_STRING_035](crates/bench/sqlite_parity/cases/SQLITE_PARITY_366_SCALAR_STRING_035.rs) | P1 | memory | GEN_SQL_SCALAR | 1822575 | 3092234 | <span style="color:#dc2626">-69.66%</span> |
| 511 | [00112 DOT_SEPARATOR](crates/bench/sqlite_parity/cases/SQLITE_PARITY_112_DOT_SEPARATOR.rs) | P0 | memory | CLI_DOT_COMMAND | 1756820 | 2975633 | <span style="color:#dc2626">-69.38%</span> |
| 512 | [00429 DML_WHERE_ORDER_LIMIT_042](crates/bench/sqlite_parity/cases/SQLITE_PARITY_429_DML_WHERE_ORDER_LIMIT_042.rs) | P1 | memory | GEN_SQL_DML | 2243039 | 3798830 | <span style="color:#dc2626">-69.36%</span> |
| 513 | [00360 SCALAR_ARITH_034](crates/bench/sqlite_parity/cases/SQLITE_PARITY_360_SCALAR_ARITH_034.rs) | P1 | memory | GEN_SQL_SCALAR | 1783721 | 3015298 | <span style="color:#dc2626">-69.05%</span> |
| 514 | [00102 WITH_MATERIALIZED_HINTS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_102_WITH_MATERIALIZED_HINTS.rs) | P0 | memory | SQL_CTE | 1885043 | 3186472 | <span style="color:#dc2626">-69.04%</span> |
| 515 | [00027 FOREIGN_KEY_FAILURE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_027_FOREIGN_KEY_FAILURE.rs) | P0 | memory | SQL_FOREIGN_KEYS_NEGATIVE | 1854083 | 3124034 | <span style="color:#dc2626">-68.49%</span> |
| 516 | [00377 SCALAR_CAST_TYPEOF_038](crates/bench/sqlite_parity/cases/SQLITE_PARITY_377_SCALAR_CAST_TYPEOF_038.rs) | P1 | memory | GEN_SQL_SCALAR | 1868491 | 3146577 | <span style="color:#dc2626">-68.40%</span> |
| 517 | [00252 SCALAR_ARITH_007](crates/bench/sqlite_parity/cases/SQLITE_PARITY_252_SCALAR_ARITH_007.rs) | P1 | memory | GEN_SQL_SCALAR | 1796555 | 3014848 | <span style="color:#dc2626">-67.81%</span> |
| 518 | [00270 SCALAR_STRING_011](crates/bench/sqlite_parity/cases/SQLITE_PARITY_270_SCALAR_STRING_011.rs) | P1 | memory | GEN_SQL_SCALAR | 1811334 | 3038081 | <span style="color:#dc2626">-67.73%</span> |
| 519 | [00296 SCALAR_ARITH_018](crates/bench/sqlite_parity/cases/SQLITE_PARITY_296_SCALAR_ARITH_018.rs) | P1 | memory | GEN_SQL_SCALAR | 1772580 | 2970915 | <span style="color:#dc2626">-67.60%</span> |
| 520 | [00861 WINDOW_PARTITION_SUM_074](crates/bench/sqlite_parity/cases/SQLITE_PARITY_861_WINDOW_PARTITION_SUM_074.rs) | P2 | memory | GEN_SQL_WINDOW | 2367805 | 3965314 | <span style="color:#dc2626">-67.47%</span> |
| 521 | [00293 SCALAR_CAST_TYPEOF_017](crates/bench/sqlite_parity/cases/SQLITE_PARITY_293_SCALAR_CAST_TYPEOF_017.rs) | P1 | memory | GEN_SQL_SCALAR | 2033022 | 3403743 | <span style="color:#dc2626">-67.42%</span> |
| 522 | [00642 JOIN_SUBQUERY_EXISTS_035](crates/bench/sqlite_parity/cases/SQLITE_PARITY_642_JOIN_SUBQUERY_EXISTS_035.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2284688 | 3824548 | <span style="color:#dc2626">-67.40%</span> |
| 523 | [00482 DML_WHERE_ORDER_LIMIT_095](crates/bench/sqlite_parity/cases/SQLITE_PARITY_482_DML_WHERE_ORDER_LIMIT_095.rs) | P1 | memory | GEN_SQL_DML | 2272515 | 3802235 | <span style="color:#dc2626">-67.31%</span> |
| 524 | [00278 SCALAR_STRING_013](crates/bench/sqlite_parity/cases/SQLITE_PARITY_278_SCALAR_STRING_013.rs) | P1 | memory | GEN_SQL_SCALAR | 1723407 | 2878981 | <span style="color:#dc2626">-67.05%</span> |
| 525 | [00980 VIEW_TRIGGER_GENERATED_033](crates/bench/sqlite_parity/cases/SQLITE_PARITY_980_VIEW_TRIGGER_GENERATED_033.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2376041 | 3956347 | <span style="color:#dc2626">-66.51%</span> |
| 526 | [00080 NOT_NULL_FAILURE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_080_NOT_NULL_FAILURE.rs) | P0 | memory | SQL_CONSTRAINTS_NEGATIVE | 1798198 | 2989640 | <span style="color:#dc2626">-66.26%</span> |
| 527 | [00484 DML_WHERE_ORDER_LIMIT_097](crates/bench/sqlite_parity/cases/SQLITE_PARITY_484_DML_WHERE_ORDER_LIMIT_097.rs) | P1 | memory | GEN_SQL_DML | 2290489 | 3803037 | <span style="color:#dc2626">-66.04%</span> |
| 528 | [00288 SCALAR_ARITH_016](crates/bench/sqlite_parity/cases/SQLITE_PARITY_288_SCALAR_ARITH_016.rs) | P1 | memory | GEN_SQL_SCALAR | 1889571 | 3136708 | <span style="color:#dc2626">-66.00%</span> |
| 529 | [00007 WITHOUT_ROWID_TABLE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_007_WITHOUT_ROWID_TABLE.rs) | P0 | memory | SQL_ROWID | 1961777 | 3255923 | <span style="color:#dc2626">-65.97%</span> |
| 530 | [00118 DOT_FULLSCHEMA](crates/bench/sqlite_parity/cases/SQLITE_PARITY_118_DOT_FULLSCHEMA.rs) | P0 | memory | CLI_DOT_COMMAND | 1919538 | 3184508 | <span style="color:#dc2626">-65.90%</span> |
| 531 | [00843 WINDOW_PARTITION_SUM_056](crates/bench/sqlite_parity/cases/SQLITE_PARITY_843_WINDOW_PARTITION_SUM_056.rs) | P2 | memory | GEN_SQL_WINDOW | 2068699 | 3428831 | <span style="color:#dc2626">-65.75%</span> |
| 532 | [00810 WINDOW_PARTITION_SUM_023](crates/bench/sqlite_parity/cases/SQLITE_PARITY_810_WINDOW_PARTITION_SUM_023.rs) | P2 | memory | GEN_SQL_WINDOW | 2171464 | 3592109 | <span style="color:#dc2626">-65.42%</span> |
| 533 | [00268 SCALAR_ARITH_011](crates/bench/sqlite_parity/cases/SQLITE_PARITY_268_SCALAR_ARITH_011.rs) | P1 | memory | GEN_SQL_SCALAR | 1753564 | 2897456 | <span style="color:#dc2626">-65.23%</span> |
| 534 | [00014 INSERT_RETURNING](crates/bench/sqlite_parity/cases/SQLITE_PARITY_014_INSERT_RETURNING.rs) | P0 | memory | SQL_INSERT | 2126468 | 3511586 | <span style="color:#dc2626">-65.14%</span> |
| 535 | [00629 JOIN_SUBQUERY_EXISTS_022](crates/bench/sqlite_parity/cases/SQLITE_PARITY_629_JOIN_SUBQUERY_EXISTS_022.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2394766 | 3951969 | <span style="color:#dc2626">-65.03%</span> |
| 536 | [00290 SCALAR_STRING_016](crates/bench/sqlite_parity/cases/SQLITE_PARITY_290_SCALAR_STRING_016.rs) | P1 | memory | GEN_SQL_SCALAR | 1797527 | 2959152 | <span style="color:#dc2626">-64.62%</span> |
| 537 | [00807 WINDOW_PARTITION_SUM_020](crates/bench/sqlite_parity/cases/SQLITE_PARITY_807_WINDOW_PARTITION_SUM_020.rs) | P2 | memory | GEN_SQL_WINDOW | 2011041 | 3310276 | <span style="color:#dc2626">-64.61%</span> |
| 538 | [00100 SCHEMA_SQLITE_SCHEMA](crates/bench/sqlite_parity/cases/SQLITE_PARITY_100_SCHEMA_SQLITE_SCHEMA.rs) | P0 | memory | SQL_SCHEMA | 2543016 | 4176724 | <span style="color:#dc2626">-64.24%</span> |
| 539 | [00172 OPT_CMD](crates/bench/sqlite_parity/cases/SQLITE_PARITY_172_OPT_CMD.rs) | P1 | memory | CLI_OPTION | 1778742 | 2913026 | <span style="color:#dc2626">-63.77%</span> |
| 540 | [00273 SCALAR_CAST_TYPEOF_012](crates/bench/sqlite_parity/cases/SQLITE_PARITY_273_SCALAR_CAST_TYPEOF_012.rs) | P1 | memory | GEN_SQL_SCALAR | 1745078 | 2848544 | <span style="color:#dc2626">-63.23%</span> |
| 541 | [00109 DOT_MODE_JSON](crates/bench/sqlite_parity/cases/SQLITE_PARITY_109_DOT_MODE_JSON.rs) | P0 | memory | CLI_DOT_COMMAND | 1861398 | 3037490 | <span style="color:#dc2626">-63.18%</span> |
| 542 | [00285 SCALAR_CAST_TYPEOF_015](crates/bench/sqlite_parity/cases/SQLITE_PARITY_285_SCALAR_CAST_TYPEOF_015.rs) | P1 | memory | GEN_SQL_SCALAR | 1805091 | 2942151 | <span style="color:#dc2626">-62.99%</span> |
| 543 | [00397 DML_WHERE_ORDER_LIMIT_010](crates/bench/sqlite_parity/cases/SQLITE_PARITY_397_DML_WHERE_ORDER_LIMIT_010.rs) | P1 | memory | GEN_SQL_DML | 2505695 | 4082305 | <span style="color:#dc2626">-62.92%</span> |
| 544 | [00115 DOT_SCHEMA_TABLES_INDEXES](crates/bench/sqlite_parity/cases/SQLITE_PARITY_115_DOT_SCHEMA_TABLES_INDEXES.rs) | P0 | memory | CLI_DOT_COMMAND | 2155183 | 3507438 | <span style="color:#dc2626">-62.74%</span> |
| 545 | [00214 DROP_TABLE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_214_DROP_TABLE.rs) | P0 | memory | SQL_DROP | 2036058 | 3305417 | <span style="color:#dc2626">-62.34%</span> |
| 546 | [00466 DML_WHERE_ORDER_LIMIT_079](crates/bench/sqlite_parity/cases/SQLITE_PARITY_466_DML_WHERE_ORDER_LIMIT_079.rs) | P1 | memory | GEN_SQL_DML | 2289196 | 3697227 | <span style="color:#dc2626">-61.51%</span> |
| 547 | [00464 DML_WHERE_ORDER_LIMIT_077](crates/bench/sqlite_parity/cases/SQLITE_PARITY_464_DML_WHERE_ORDER_LIMIT_077.rs) | P1 | memory | GEN_SQL_DML | 2348659 | 3785474 | <span style="color:#dc2626">-61.18%</span> |
| 548 | [00009 STRICT_TABLE_TYPE_FAILURE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_009_STRICT_TABLE_TYPE_FAILURE.rs) | P0 | memory | SQL_DDL_NEGATIVE | 1821723 | 2928374 | <span style="color:#dc2626">-60.75%</span> |
| 549 | [00627 JOIN_SUBQUERY_EXISTS_020](crates/bench/sqlite_parity/cases/SQLITE_PARITY_627_JOIN_SUBQUERY_EXISTS_020.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2310637 | 3709510 | <span style="color:#dc2626">-60.54%</span> |
| 550 | [00003 CREATE_TABLE_INSERT_SELECT](crates/bench/sqlite_parity/cases/SQLITE_PARITY_003_CREATE_TABLE_INSERT_SELECT.rs) | P0 | memory | SQL_DDL_DML | 2143191 | 3425915 | <span style="color:#dc2626">-59.85%</span> |
| 551 | [00370 SCALAR_STRING_036](crates/bench/sqlite_parity/cases/SQLITE_PARITY_370_SCALAR_STRING_036.rs) | P1 | memory | GEN_SQL_SCALAR | 1967919 | 3139032 | <span style="color:#dc2626">-59.51%</span> |
| 552 | [00179 OPT_MARKDOWN_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_179_OPT_MARKDOWN_MODE.rs) | P2 | memory | CLI_OPTION | 1845198 | 2913927 | <span style="color:#dc2626">-57.92%</span> |
| 553 | [00005 UNIQUE_CONSTRAINT_FAILURE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_005_UNIQUE_CONSTRAINT_FAILURE.rs) | P0 | memory | SQL_CONSTRAINTS_NEGATIVE | 1848974 | 2918255 | <span style="color:#dc2626">-57.83%</span> |
| 554 | [00460 DML_WHERE_ORDER_LIMIT_073](crates/bench/sqlite_parity/cases/SQLITE_PARITY_460_DML_WHERE_ORDER_LIMIT_073.rs) | P1 | memory | GEN_SQL_DML | 2436555 | 3844286 | <span style="color:#dc2626">-57.78%</span> |
| 555 | [00934 CONSTRAINT_FK_SAVEPOINT_067](crates/bench/sqlite_parity/cases/SQLITE_PARITY_934_CONSTRAINT_FK_SAVEPOINT_067.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2274729 | 3575888 | <span style="color:#dc2626">-57.20%</span> |
| 556 | [00673 JOIN_SUBQUERY_EXISTS_066](crates/bench/sqlite_parity/cases/SQLITE_PARITY_673_JOIN_SUBQUERY_EXISTS_066.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2413461 | 3765416 | <span style="color:#dc2626">-56.02%</span> |
| 557 | [00341 SCALAR_CAST_TYPEOF_029](crates/bench/sqlite_parity/cases/SQLITE_PARITY_341_SCALAR_CAST_TYPEOF_029.rs) | P1 | memory | GEN_SQL_SCALAR | 1958551 | 3037190 | <span style="color:#dc2626">-55.07%</span> |
| 558 | [00449 DML_WHERE_ORDER_LIMIT_062](crates/bench/sqlite_parity/cases/SQLITE_PARITY_449_DML_WHERE_ORDER_LIMIT_062.rs) | P1 | memory | GEN_SQL_DML | 2447877 | 3794471 | <span style="color:#dc2626">-55.01%</span> |
| 559 | [00191 OPT_BATCH](crates/bench/sqlite_parity/cases/SQLITE_PARITY_191_OPT_BATCH.rs) | P2 | memory | CLI_OPTION | 1900301 | 2945025 | <span style="color:#dc2626">-54.98%</span> |
| 560 | [00986 VIEW_TRIGGER_GENERATED_039](crates/bench/sqlite_parity/cases/SQLITE_PARITY_986_VIEW_TRIGGER_GENERATED_039.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2612768 | 4043953 | <span style="color:#dc2626">-54.78%</span> |
| 561 | [00410 DML_WHERE_ORDER_LIMIT_023](crates/bench/sqlite_parity/cases/SQLITE_PARITY_410_DML_WHERE_ORDER_LIMIT_023.rs) | P1 | memory | GEN_SQL_DML | 2466261 | 3814749 | <span style="color:#dc2626">-54.68%</span> |
| 562 | [00010 GENERATED_COLUMNS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_010_GENERATED_COLUMNS.rs) | P0 | memory | SQL_DDL | 2078307 | 3208694 | <span style="color:#dc2626">-54.39%</span> |
| 563 | [00919 CONSTRAINT_FK_SAVEPOINT_052](crates/bench/sqlite_parity/cases/SQLITE_PARITY_919_CONSTRAINT_FK_SAVEPOINT_052.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2420544 | 3718287 | <span style="color:#dc2626">-53.61%</span> |
| 564 | [00852 WINDOW_PARTITION_SUM_065](crates/bench/sqlite_parity/cases/SQLITE_PARITY_852_WINDOW_PARTITION_SUM_065.rs) | P2 | memory | GEN_SQL_WINDOW | 2688801 | 4073729 | <span style="color:#dc2626">-51.51%</span> |
| 565 | [00837 WINDOW_PARTITION_SUM_050](crates/bench/sqlite_parity/cases/SQLITE_PARITY_837_WINDOW_PARTITION_SUM_050.rs) | P2 | memory | GEN_SQL_WINDOW | 2274569 | 3431596 | <span style="color:#dc2626">-50.87%</span> |
| 566 | [00824 WINDOW_PARTITION_SUM_037](crates/bench/sqlite_parity/cases/SQLITE_PARITY_824_WINDOW_PARTITION_SUM_037.rs) | P2 | memory | GEN_SQL_WINDOW | 2284477 | 3418651 | <span style="color:#dc2626">-49.65%</span> |
| 567 | [00840 WINDOW_PARTITION_SUM_053](crates/bench/sqlite_parity/cases/SQLITE_PARITY_840_WINDOW_PARTITION_SUM_053.rs) | P2 | memory | GEN_SQL_WINDOW | 3007895 | 4465118 | <span style="color:#dc2626">-48.45%</span> |
| 568 | [00318 SCALAR_STRING_023](crates/bench/sqlite_parity/cases/SQLITE_PARITY_318_SCALAR_STRING_023.rs) | P1 | memory | GEN_SQL_SCALAR | 2140696 | 3126148 | <span style="color:#dc2626">-46.03%</span> |
| 569 | [00048 PRAGMA_USER_VERSION](crates/bench/sqlite_parity/cases/SQLITE_PARITY_048_PRAGMA_USER_VERSION.rs) | P0 | memory | SQL_PRAGMA | 1798559 | 2618007 | <span style="color:#dc2626">-45.56%</span> |
| 570 | [00376 SCALAR_ARITH_038](crates/bench/sqlite_parity/cases/SQLITE_PARITY_376_SCALAR_ARITH_038.rs) | P1 | memory | GEN_SQL_SCALAR | 2083868 | 3015359 | <span style="color:#dc2626">-44.70%</span> |
| 571 | [00025 BEGIN_MODES](crates/bench/sqlite_parity/cases/SQLITE_PARITY_025_BEGIN_MODES.rs) | P0 | memory | SQL_TRANSACTION | 2104588 | 3038583 | <span style="color:#dc2626">-44.38%</span> |
| 572 | [00002 LITERALS_AND_TYPEOF](crates/bench/sqlite_parity/cases/SQLITE_PARITY_002_LITERALS_AND_TYPEOF.rs) | P0 | memory | SQL_EXPRESSIONS | 2255132 | 3241306 | <span style="color:#dc2626">-43.73%</span> |
| 573 | [00356 SCALAR_ARITH_033](crates/bench/sqlite_parity/cases/SQLITE_PARITY_356_SCALAR_ARITH_033.rs) | P1 | memory | GEN_SQL_SCALAR | 1997215 | 2866077 | <span style="color:#dc2626">-43.50%</span> |
| 574 | [00023 TRANSACTION_ROLLBACK](crates/bench/sqlite_parity/cases/SQLITE_PARITY_023_TRANSACTION_ROLLBACK.rs) | P0 | memory | SQL_TRANSACTION | 2299345 | 3299626 | <span style="color:#dc2626">-43.50%</span> |
| 575 | [00865 WINDOW_PARTITION_SUM_078](crates/bench/sqlite_parity/cases/SQLITE_PARITY_865_WINDOW_PARTITION_SUM_078.rs) | P2 | memory | GEN_SQL_WINDOW | 2453367 | 3472442 | <span style="color:#dc2626">-41.54%</span> |
| 576 | [00030 ALTER_TABLE_ADD_DROP_COLUMN](crates/bench/sqlite_parity/cases/SQLITE_PARITY_030_ALTER_TABLE_ADD_DROP_COLUMN.rs) | P0 | memory | SQL_ALTER | 2231607 | 3154131 | <span style="color:#dc2626">-41.34%</span> |
| 577 | [00189 OPT_ECHO](crates/bench/sqlite_parity/cases/SQLITE_PARITY_189_OPT_ECHO.rs) | P2 | memory | CLI_OPTION | 2068870 | 2922042 | <span style="color:#dc2626">-41.24%</span> |
| 578 | [00218 PRAGMA_FORMS_SCHEMA_EQUALS_PARENS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_218_PRAGMA_FORMS_SCHEMA_EQUALS_PARENS.rs) | P0 | memory | SQL_PRAGMA | 1762691 | 2486790 | <span style="color:#dc2626">-41.08%</span> |
| 579 | [00684 JOIN_SUBQUERY_EXISTS_077](crates/bench/sqlite_parity/cases/SQLITE_PARITY_684_JOIN_SUBQUERY_EXISTS_077.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2803990 | 3914939 | <span style="color:#dc2626">-39.62%</span> |
| 580 | [00127 DOT_LIMIT](crates/bench/sqlite_parity/cases/SQLITE_PARITY_127_DOT_LIMIT.rs) | P0 | memory | CLI_DOT_COMMAND | 1769284 | 2463445 | <span style="color:#dc2626">-39.23%</span> |
| 581 | [00114 DOT_PRINT](crates/bench/sqlite_parity/cases/SQLITE_PARITY_114_DOT_PRINT.rs) | P0 | memory | CLI_DOT_COMMAND | 1759485 | 2448537 | <span style="color:#dc2626">-39.16%</span> |
| 582 | [00143 DOT_QUIT](crates/bench/sqlite_parity/cases/SQLITE_PARITY_143_DOT_QUIT.rs) | P0 | memory | CLI_DOT_COMMAND | 1581649 | 2198375 | <span style="color:#dc2626">-38.99%</span> |
| 583 | [00244 SCALAR_ARITH_005](crates/bench/sqlite_parity/cases/SQLITE_PARITY_244_SCALAR_ARITH_005.rs) | P1 | memory | GEN_SQL_SCALAR | 2181853 | 3025428 | <span style="color:#dc2626">-38.66%</span> |
| 584 | [00116 DOT_DATABASES](crates/bench/sqlite_parity/cases/SQLITE_PARITY_116_DOT_DATABASES.rs) | P0 | memory | CLI_DOT_COMMAND | 1744937 | 2418150 | <span style="color:#dc2626">-38.58%</span> |
| 585 | [00352 SCALAR_ARITH_032](crates/bench/sqlite_parity/cases/SQLITE_PARITY_352_SCALAR_ARITH_032.rs) | P1 | memory | GEN_SQL_SCALAR | 2219194 | 3073258 | <span style="color:#dc2626">-38.49%</span> |
| 586 | [00790 WINDOW_PARTITION_SUM_003](crates/bench/sqlite_parity/cases/SQLITE_PARITY_790_WINDOW_PARTITION_SUM_003.rs) | P2 | memory | GEN_SQL_WINDOW | 2500115 | 3462013 | <span style="color:#dc2626">-38.47%</span> |
| 587 | [00269 SCALAR_CAST_TYPEOF_011](crates/bench/sqlite_parity/cases/SQLITE_PARITY_269_SCALAR_CAST_TYPEOF_011.rs) | P1 | memory | GEN_SQL_SCALAR | 2395337 | 3248109 | <span style="color:#dc2626">-35.60%</span> |
| 588 | [00024 SAVEPOINT_ROLLBACK_RELEASE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_024_SAVEPOINT_ROLLBACK_RELEASE.rs) | P0 | memory | SQL_SAVEPOINT | 2553486 | 3449158 | <span style="color:#dc2626">-35.08%</span> |
| 589 | [00107 DOT_HELP_PATTERN](crates/bench/sqlite_parity/cases/SQLITE_PARITY_107_DOT_HELP_PATTERN.rs) | P0 | memory | CLI_DOT_COMMAND | 1766198 | 2380118 | <span style="color:#dc2626">-34.76%</span> |
| 590 | [00816 WINDOW_PARTITION_SUM_029](crates/bench/sqlite_parity/cases/SQLITE_PARITY_816_WINDOW_PARTITION_SUM_029.rs) | P2 | memory | GEN_SQL_WINDOW | 2440271 | 3285940 | <span style="color:#dc2626">-34.65%</span> |
| 591 | [00788 WINDOW_PARTITION_SUM_001](crates/bench/sqlite_parity/cases/SQLITE_PARITY_788_WINDOW_PARTITION_SUM_001.rs) | P2 | memory | GEN_SQL_WINDOW | 2685446 | 3569877 | <span style="color:#dc2626">-32.93%</span> |
| 592 | [00401 DML_WHERE_ORDER_LIMIT_014](crates/bench/sqlite_parity/cases/SQLITE_PARITY_401_DML_WHERE_ORDER_LIMIT_014.rs) | P1 | memory | GEN_SQL_DML | 2890012 | 3826241 | <span style="color:#dc2626">-32.40%</span> |
| 593 | [00292 SCALAR_ARITH_017](crates/bench/sqlite_parity/cases/SQLITE_PARITY_292_SCALAR_ARITH_017.rs) | P1 | memory | GEN_SQL_SCALAR | 2199287 | 2883480 | <span style="color:#dc2626">-31.11%</span> |
| 594 | [00047 PRAGMA_FOREIGN_KEYS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_047_PRAGMA_FOREIGN_KEYS.rs) | P0 | memory | SQL_PRAGMA | 1956497 | 2543867 | <span style="color:#dc2626">-30.02%</span> |
| 595 | [00703 JOIN_SUBQUERY_EXISTS_096](crates/bench/sqlite_parity/cases/SQLITE_PARITY_703_JOIN_SUBQUERY_EXISTS_096.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2978259 | 3868351 | <span style="color:#dc2626">-29.89%</span> |
| 596 | [00284 SCALAR_ARITH_015](crates/bench/sqlite_parity/cases/SQLITE_PARITY_284_SCALAR_ARITH_015.rs) | P1 | memory | GEN_SQL_SCALAR | 2329473 | 3010449 | <span style="color:#dc2626">-29.23%</span> |
| 597 | [00957 VIEW_TRIGGER_GENERATED_010](crates/bench/sqlite_parity/cases/SQLITE_PARITY_957_VIEW_TRIGGER_GENERATED_010.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 3026250 | 3904589 | <span style="color:#dc2626">-29.02%</span> |
| 598 | [00079 CHECK_CONSTRAINT_FAILURE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_079_CHECK_CONSTRAINT_FAILURE.rs) | P0 | memory | SQL_CONSTRAINTS_NEGATIVE | 2361623 | 3042099 | <span style="color:#dc2626">-28.81%</span> |
| 599 | [00294 SCALAR_STRING_017](crates/bench/sqlite_parity/cases/SQLITE_PARITY_294_SCALAR_STRING_017.rs) | P1 | memory | GEN_SQL_SCALAR | 2406989 | 3055965 | <span style="color:#dc2626">-26.96%</span> |
| 600 | [00851 WINDOW_PARTITION_SUM_064](crates/bench/sqlite_parity/cases/SQLITE_PARITY_851_WINDOW_PARTITION_SUM_064.rs) | P2 | memory | GEN_SQL_WINDOW | 2800032 | 3487611 | <span style="color:#dc2626">-24.56%</span> |
| 601 | [00176 OPT_LINE_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_176_OPT_LINE_MODE.rs) | P2 | memory | CLI_OPTION | 2348398 | 2916892 | <span style="color:#dc2626">-24.21%</span> |
| 602 | [00101 SCHEMA_SQLITE_MASTER_ALIAS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_101_SCHEMA_SQLITE_MASTER_ALIAS.rs) | P0 | memory | SQL_SCHEMA | 2420785 | 2994509 | <span style="color:#dc2626">-23.70%</span> |
| 603 | [00850 WINDOW_PARTITION_SUM_063](crates/bench/sqlite_parity/cases/SQLITE_PARITY_850_WINDOW_PARTITION_SUM_063.rs) | P2 | memory | GEN_SQL_WINDOW | 2784493 | 3435643 | <span style="color:#dc2626">-23.38%</span> |
| 604 | [00049 PRAGMA_TEMP_STORE_MEMORY](crates/bench/sqlite_parity/cases/SQLITE_PARITY_049_PRAGMA_TEMP_STORE_MEMORY.rs) | P0 | memory | SQL_PRAGMA | 2113635 | 2545661 | <span style="color:#dc2626">-20.44%</span> |
| 605 | [00316 SCALAR_ARITH_023](crates/bench/sqlite_parity/cases/SQLITE_PARITY_316_SCALAR_ARITH_023.rs) | P1 | memory | GEN_SQL_SCALAR | 2507188 | 3006422 | <span style="color:#dc2626">-19.91%</span> |
| 606 | [00028 ALTER_TABLE_RENAME](crates/bench/sqlite_parity/cases/SQLITE_PARITY_028_ALTER_TABLE_RENAME.rs) | P0 | memory | SQL_ALTER | 2950616 | 3407970 | <span style="color:#dc2626">-15.50%</span> |
| 607 | [00029 ALTER_TABLE_RENAME_COLUMN](crates/bench/sqlite_parity/cases/SQLITE_PARITY_029_ALTER_TABLE_RENAME_COLUMN.rs) | P0 | memory | SQL_ALTER | 2847652 | 3232519 | <span style="color:#dc2626">-13.52%</span> |
| 608 | [00106 DOT_HELP](crates/bench/sqlite_parity/cases/SQLITE_PARITY_106_DOT_HELP.rs) | P0 | memory | CLI_DOT_COMMAND | 2078859 | 2353818 | <span style="color:#dc2626">-13.23%</span> |
| 609 | [00350 SCALAR_STRING_031](crates/bench/sqlite_parity/cases/SQLITE_PARITY_350_SCALAR_STRING_031.rs) | P1 | memory | GEN_SQL_SCALAR | 2809129 | 3125627 | <span style="color:#dc2626">-11.27%</span> |
| 610 | [00365 SCALAR_CAST_TYPEOF_035](crates/bench/sqlite_parity/cases/SQLITE_PARITY_365_SCALAR_CAST_TYPEOF_035.rs) | P1 | memory | GEN_SQL_SCALAR | 3008005 | 3096763 | <span style="color:#f97316">-2.95%</span> |
| 611 | [00170 OPT_VERSION](crates/bench/sqlite_parity/cases/SQLITE_PARITY_170_OPT_VERSION.rs) | P1 | memory | CLI_OPTION | 1690495 | 1543417 | <span style="color:#2563eb">8.70%</span> |
| 612 | [00272 SCALAR_ARITH_012](crates/bench/sqlite_parity/cases/SQLITE_PARITY_272_SCALAR_ARITH_012.rs) | P1 | memory | GEN_SQL_SCALAR | 3146557 | 2872078 | <span style="color:#2563eb">8.72%</span> |

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
