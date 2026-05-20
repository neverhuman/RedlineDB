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
| 1 | [00949 VIEW_TRIGGER_GENERATED_002](crates/bench/sqlite_parity/cases/SQLITE_PARITY_949_VIEW_TRIGGER_GENERATED_002.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2956518 | 2805727770 | <span style="color:#dc2626">-94799.74%</span> |
| 2 | [00807 WINDOW_PARTITION_SUM_020](crates/bench/sqlite_parity/cases/SQLITE_PARITY_807_WINDOW_PARTITION_SUM_020.rs) | P2 | memory | GEN_SQL_WINDOW | 3270913 | 1691192145 | <span style="color:#dc2626">-51603.98%</span> |
| 3 | [00274 SCALAR_STRING_012](crates/bench/sqlite_parity/cases/SQLITE_PARITY_274_SCALAR_STRING_012.rs) | P1 | memory | GEN_SQL_SCALAR | 3271714 | 1132155570 | <span style="color:#dc2626">-34504.36%</span> |
| 4 | [00808 WINDOW_PARTITION_SUM_021](crates/bench/sqlite_parity/cases/SQLITE_PARITY_808_WINDOW_PARTITION_SUM_021.rs) | P2 | memory | GEN_SQL_WINDOW | 2900101 | 989563493 | <span style="color:#dc2626">-34021.69%</span> |
| 5 | [00273 SCALAR_CAST_TYPEOF_012](crates/bench/sqlite_parity/cases/SQLITE_PARITY_273_SCALAR_CAST_TYPEOF_012.rs) | P1 | memory | GEN_SQL_SCALAR | 3377093 | 1107636407 | <span style="color:#dc2626">-32698.52%</span> |
| 6 | [00455 DML_WHERE_ORDER_LIMIT_068](crates/bench/sqlite_parity/cases/SQLITE_PARITY_455_DML_WHERE_ORDER_LIMIT_068.rs) | P1 | memory | GEN_SQL_DML | 3520946 | 1007280867 | <span style="color:#dc2626">-28508.25%</span> |
| 7 | [00641 JOIN_SUBQUERY_EXISTS_034](crates/bench/sqlite_parity/cases/SQLITE_PARITY_641_JOIN_SUBQUERY_EXISTS_034.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3405668 | 965000465 | <span style="color:#dc2626">-28235.13%</span> |
| 8 | [00642 JOIN_SUBQUERY_EXISTS_035](crates/bench/sqlite_parity/cases/SQLITE_PARITY_642_JOIN_SUBQUERY_EXISTS_035.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3194057 | 865919100 | <span style="color:#dc2626">-27010.32%</span> |
| 9 | [00281 SCALAR_CAST_TYPEOF_014](crates/bench/sqlite_parity/cases/SQLITE_PARITY_281_SCALAR_CAST_TYPEOF_014.rs) | P1 | memory | GEN_SQL_SCALAR | 2816762 | 674669123 | <span style="color:#dc2626">-23851.94%</span> |
| 10 | [00956 VIEW_TRIGGER_GENERATED_009](crates/bench/sqlite_parity/cases/SQLITE_PARITY_956_VIEW_TRIGGER_GENERATED_009.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2803317 | 644539376 | <span style="color:#dc2626">-22892.03%</span> |
| 11 | [00954 VIEW_TRIGGER_GENERATED_007](crates/bench/sqlite_parity/cases/SQLITE_PARITY_954_VIEW_TRIGGER_GENERATED_007.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2859885 | 610869147 | <span style="color:#dc2626">-21259.92%</span> |
| 12 | [00276 SCALAR_ARITH_013](crates/bench/sqlite_parity/cases/SQLITE_PARITY_276_SCALAR_ARITH_013.rs) | P1 | memory | GEN_SQL_SCALAR | 2555668 | 527317422 | <span style="color:#dc2626">-20533.25%</span> |
| 13 | [00955 VIEW_TRIGGER_GENERATED_008](crates/bench/sqlite_parity/cases/SQLITE_PARITY_955_VIEW_TRIGGER_GENERATED_008.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2765235 | 569940320 | <span style="color:#dc2626">-20510.92%</span> |
| 14 | [00278 SCALAR_STRING_013](crates/bench/sqlite_parity/cases/SQLITE_PARITY_278_SCALAR_STRING_013.rs) | P1 | memory | GEN_SQL_SCALAR | 2696435 | 527681534 | <span style="color:#dc2626">-19469.60%</span> |
| 15 | [00454 DML_WHERE_ORDER_LIMIT_067](crates/bench/sqlite_parity/cases/SQLITE_PARITY_454_DML_WHERE_ORDER_LIMIT_067.rs) | P1 | memory | GEN_SQL_DML | 3111821 | 603643052 | <span style="color:#dc2626">-19298.39%</span> |
| 16 | [00640 JOIN_SUBQUERY_EXISTS_033](crates/bench/sqlite_parity/cases/SQLITE_PARITY_640_JOIN_SUBQUERY_EXISTS_033.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3104247 | 598036914 | <span style="color:#dc2626">-19165.12%</span> |
| 17 | [00809 WINDOW_PARTITION_SUM_022](crates/bench/sqlite_parity/cases/SQLITE_PARITY_809_WINDOW_PARTITION_SUM_022.rs) | P2 | memory | GEN_SQL_WINDOW | 3055305 | 583954223 | <span style="color:#dc2626">-19012.80%</span> |
| 18 | [00988 VIEW_TRIGGER_GENERATED_041](crates/bench/sqlite_parity/cases/SQLITE_PARITY_988_VIEW_TRIGGER_GENERATED_041.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 3186063 | 603143086 | <span style="color:#dc2626">-18830.67%</span> |
| 19 | [00950 VIEW_TRIGGER_GENERATED_003](crates/bench/sqlite_parity/cases/SQLITE_PARITY_950_VIEW_TRIGGER_GENERATED_003.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 3228602 | 530264181 | <span style="color:#dc2626">-16323.96%</span> |
| 20 | [00280 SCALAR_ARITH_014](crates/bench/sqlite_parity/cases/SQLITE_PARITY_280_SCALAR_ARITH_014.rs) | P1 | memory | GEN_SQL_SCALAR | 2602186 | 369939358 | <span style="color:#dc2626">-14116.48%</span> |
| 21 | [00868 CONSTRAINT_FK_SAVEPOINT_001](crates/bench/sqlite_parity/cases/SQLITE_PARITY_868_CONSTRAINT_FK_SAVEPOINT_001.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 3245474 | 434413311 | <span style="color:#dc2626">-13285.20%</span> |
| 22 | [00987 VIEW_TRIGGER_GENERATED_040](crates/bench/sqlite_parity/cases/SQLITE_PARITY_987_VIEW_TRIGGER_GENERATED_040.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2895442 | 341037266 | <span style="color:#dc2626">-11678.42%</span> |
| 23 | [00646 JOIN_SUBQUERY_EXISTS_039](crates/bench/sqlite_parity/cases/SQLITE_PARITY_646_JOIN_SUBQUERY_EXISTS_039.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2915369 | 325512899 | <span style="color:#dc2626">-11065.41%</span> |
| 24 | [00462 DML_WHERE_ORDER_LIMIT_075](crates/bench/sqlite_parity/cases/SQLITE_PARITY_462_DML_WHERE_ORDER_LIMIT_075.rs) | P1 | memory | GEN_SQL_DML | 2745578 | 303836369 | <span style="color:#dc2626">-10966.39%</span> |
| 25 | [00452 DML_WHERE_ORDER_LIMIT_065](crates/bench/sqlite_parity/cases/SQLITE_PARITY_452_DML_WHERE_ORDER_LIMIT_065.rs) | P1 | memory | GEN_SQL_DML | 2746159 | 296962541 | <span style="color:#dc2626">-10713.74%</span> |
| 26 | [00460 DML_WHERE_ORDER_LIMIT_073](crates/bench/sqlite_parity/cases/SQLITE_PARITY_460_DML_WHERE_ORDER_LIMIT_073.rs) | P1 | memory | GEN_SQL_DML | 2472431 | 252636501 | <span style="color:#dc2626">-10118.14%</span> |
| 27 | [00458 DML_WHERE_ORDER_LIMIT_071](crates/bench/sqlite_parity/cases/SQLITE_PARITY_458_DML_WHERE_ORDER_LIMIT_071.rs) | P1 | memory | GEN_SQL_DML | 2336263 | 236837271 | <span style="color:#dc2626">-10037.44%</span> |
| 28 | [00459 DML_WHERE_ORDER_LIMIT_072](crates/bench/sqlite_parity/cases/SQLITE_PARITY_459_DML_WHERE_ORDER_LIMIT_072.rs) | P1 | memory | GEN_SQL_DML | 2724718 | 255361339 | <span style="color:#dc2626">-9272.03%</span> |
| 29 | [00277 SCALAR_CAST_TYPEOF_013](crates/bench/sqlite_parity/cases/SQLITE_PARITY_277_SCALAR_CAST_TYPEOF_013.rs) | P1 | memory | GEN_SQL_SCALAR | 2661829 | 242770800 | <span style="color:#dc2626">-9020.45%</span> |
| 30 | [00647 JOIN_SUBQUERY_EXISTS_040](crates/bench/sqlite_parity/cases/SQLITE_PARITY_647_JOIN_SUBQUERY_EXISTS_040.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3281573 | 297974405 | <span style="color:#dc2626">-8980.23%</span> |
| 31 | [00645 JOIN_SUBQUERY_EXISTS_038](crates/bench/sqlite_parity/cases/SQLITE_PARITY_645_JOIN_SUBQUERY_EXISTS_038.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2918946 | 261016096 | <span style="color:#dc2626">-8842.14%</span> |
| 32 | [00648 JOIN_SUBQUERY_EXISTS_041](crates/bench/sqlite_parity/cases/SQLITE_PARITY_648_JOIN_SUBQUERY_EXISTS_041.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2808618 | 245917853 | <span style="color:#dc2626">-8655.83%</span> |
| 33 | [00003 CREATE_TABLE_INSERT_SELECT](crates/bench/sqlite_parity/cases/SQLITE_PARITY_003_CREATE_TABLE_INSERT_SELECT.rs) | P0 | memory | SQL_DDL_DML | 2980452 | 260886169 | <span style="color:#dc2626">-8653.24%</span> |
| 34 | [00643 JOIN_SUBQUERY_EXISTS_036](crates/bench/sqlite_parity/cases/SQLITE_PARITY_643_JOIN_SUBQUERY_EXISTS_036.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3199948 | 266190200 | <span style="color:#dc2626">-8218.58%</span> |
| 35 | [00457 DML_WHERE_ORDER_LIMIT_070](crates/bench/sqlite_parity/cases/SQLITE_PARITY_457_DML_WHERE_ORDER_LIMIT_070.rs) | P1 | memory | GEN_SQL_DML | 2597858 | 212004910 | <span style="color:#dc2626">-8060.76%</span> |
| 36 | [00456 DML_WHERE_ORDER_LIMIT_069](crates/bench/sqlite_parity/cases/SQLITE_PARITY_456_DML_WHERE_ORDER_LIMIT_069.rs) | P1 | memory | GEN_SQL_DML | 2830779 | 228987660 | <span style="color:#dc2626">-7989.21%</span> |
| 37 | [00285 SCALAR_CAST_TYPEOF_015](crates/bench/sqlite_parity/cases/SQLITE_PARITY_285_SCALAR_CAST_TYPEOF_015.rs) | P1 | memory | GEN_SQL_SCALAR | 1716449 | 137625965 | <span style="color:#dc2626">-7918.06%</span> |
| 38 | [00863 WINDOW_PARTITION_SUM_076](crates/bench/sqlite_parity/cases/SQLITE_PARITY_863_WINDOW_PARTITION_SUM_076.rs) | P2 | memory | GEN_SQL_WINDOW | 3083447 | 243487275 | <span style="color:#dc2626">-7796.59%</span> |
| 39 | [00108 DOT_MODE_CSV_AND_QUOTE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_108_DOT_MODE_CSV_AND_QUOTE.rs) | P0 | memory | CLI_DOT_COMMAND | 1970461 | 153784769 | <span style="color:#dc2626">-7704.51%</span> |
| 40 | [00866 WINDOW_PARTITION_SUM_079](crates/bench/sqlite_parity/cases/SQLITE_PARITY_866_WINDOW_PARTITION_SUM_079.rs) | P2 | memory | GEN_SQL_WINDOW | 2410113 | 186118874 | <span style="color:#dc2626">-7622.41%</span> |
| 41 | [00001 SELECT_CORE_EXPRESSIONS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_001_SELECT_CORE_EXPRESSIONS.rs) | P0 | memory | SQL_SELECT | 2480475 | 191067221 | <span style="color:#dc2626">-7602.85%</span> |
| 42 | [00453 DML_WHERE_ORDER_LIMIT_066](crates/bench/sqlite_parity/cases/SQLITE_PARITY_453_DML_WHERE_ORDER_LIMIT_066.rs) | P1 | memory | GEN_SQL_DML | 2879040 | 219955258 | <span style="color:#dc2626">-7539.88%</span> |
| 43 | [00966 VIEW_TRIGGER_GENERATED_019](crates/bench/sqlite_parity/cases/SQLITE_PARITY_966_VIEW_TRIGGER_GENERATED_019.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 3005079 | 228385794 | <span style="color:#dc2626">-7499.99%</span> |
| 44 | [00461 DML_WHERE_ORDER_LIMIT_074](crates/bench/sqlite_parity/cases/SQLITE_PARITY_461_DML_WHERE_ORDER_LIMIT_074.rs) | P1 | memory | GEN_SQL_DML | 2687758 | 203680191 | <span style="color:#dc2626">-7478.07%</span> |
| 45 | [00644 JOIN_SUBQUERY_EXISTS_037](crates/bench/sqlite_parity/cases/SQLITE_PARITY_644_JOIN_SUBQUERY_EXISTS_037.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3309385 | 247697711 | <span style="color:#dc2626">-7384.71%</span> |
| 46 | [00006 ROWID_INTEGER_PRIMARY_KEY](crates/bench/sqlite_parity/cases/SQLITE_PARITY_006_ROWID_INTEGER_PRIMARY_KEY.rs) | P0 | memory | SQL_ROWID | 3217100 | 240372347 | <span style="color:#dc2626">-7371.71%</span> |
| 47 | [00996 VIEW_TRIGGER_GENERATED_049](crates/bench/sqlite_parity/cases/SQLITE_PARITY_996_VIEW_TRIGGER_GENERATED_049.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2544668 | 185775585 | <span style="color:#dc2626">-7200.58%</span> |
| 48 | [00002 LITERALS_AND_TYPEOF](crates/bench/sqlite_parity/cases/SQLITE_PARITY_002_LITERALS_AND_TYPEOF.rs) | P0 | memory | SQL_EXPRESSIONS | 2073795 | 150257608 | <span style="color:#dc2626">-7145.54%</span> |
| 49 | [00290 SCALAR_STRING_016](crates/bench/sqlite_parity/cases/SQLITE_PARITY_290_SCALAR_STRING_016.rs) | P1 | memory | GEN_SQL_SCALAR | 2298712 | 165335011 | <span style="color:#dc2626">-7092.51%</span> |
| 50 | [00967 VIEW_TRIGGER_GENERATED_020](crates/bench/sqlite_parity/cases/SQLITE_PARITY_967_VIEW_TRIGGER_GENERATED_020.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 3457786 | 242915543 | <span style="color:#dc2626">-6925.18%</span> |
| 51 | [00118 DOT_FULLSCHEMA](crates/bench/sqlite_parity/cases/SQLITE_PARITY_118_DOT_FULLSCHEMA.rs) | P0 | memory | CLI_DOT_COMMAND | 2350449 | 164132057 | <span style="color:#dc2626">-6883.01%</span> |
| 52 | [00865 WINDOW_PARTITION_SUM_078](crates/bench/sqlite_parity/cases/SQLITE_PARITY_865_WINDOW_PARTITION_SUM_078.rs) | P2 | memory | GEN_SQL_WINDOW | 2742672 | 189090320 | <span style="color:#dc2626">-6794.38%</span> |
| 53 | [00005 UNIQUE_CONSTRAINT_FAILURE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_005_UNIQUE_CONSTRAINT_FAILURE.rs) | P0 | memory | SQL_CONSTRAINTS_NEGATIVE | 2387099 | 162311587 | <span style="color:#dc2626">-6699.53%</span> |
| 54 | [00110 DOT_MODE_LINE_COLUMN_TABLE_BOX_MARKDOWN](crates/bench/sqlite_parity/cases/SQLITE_PARITY_110_DOT_MODE_LINE_COLUMN_TABLE_BOX_MARKDOWN.rs) | P0 | memory | CLI_DOT_COMMAND | 1836125 | 120982945 | <span style="color:#dc2626">-6489.04%</span> |
| 55 | [00972 VIEW_TRIGGER_GENERATED_025](crates/bench/sqlite_parity/cases/SQLITE_PARITY_972_VIEW_TRIGGER_GENERATED_025.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 3231018 | 206091184 | <span style="color:#dc2626">-6278.52%</span> |
| 56 | [00007 WITHOUT_ROWID_TABLE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_007_WITHOUT_ROWID_TABLE.rs) | P0 | memory | SQL_ROWID | 2796423 | 177829432 | <span style="color:#dc2626">-6259.17%</span> |
| 57 | [00463 DML_WHERE_ORDER_LIMIT_076](crates/bench/sqlite_parity/cases/SQLITE_PARITY_463_DML_WHERE_ORDER_LIMIT_076.rs) | P1 | memory | GEN_SQL_DML | 3443720 | 218959087 | <span style="color:#dc2626">-6258.21%</span> |
| 58 | [00981 VIEW_TRIGGER_GENERATED_034](crates/bench/sqlite_parity/cases/SQLITE_PARITY_981_VIEW_TRIGGER_GENERATED_034.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2981975 | 189467563 | <span style="color:#dc2626">-6253.76%</span> |
| 59 | [00806 WINDOW_PARTITION_SUM_019](crates/bench/sqlite_parity/cases/SQLITE_PARITY_806_WINDOW_PARTITION_SUM_019.rs) | P2 | memory | GEN_SQL_WINDOW | 2898918 | 181456151 | <span style="color:#dc2626">-6159.44%</span> |
| 60 | [00864 WINDOW_PARTITION_SUM_077](crates/bench/sqlite_parity/cases/SQLITE_PARITY_864_WINDOW_PARTITION_SUM_077.rs) | P2 | memory | GEN_SQL_WINDOW | 2967188 | 185108120 | <span style="color:#dc2626">-6138.50%</span> |
| 61 | [00812 WINDOW_PARTITION_SUM_025](crates/bench/sqlite_parity/cases/SQLITE_PARITY_812_WINDOW_PARTITION_SUM_025.rs) | P2 | memory | GEN_SQL_WINDOW | 3021661 | 185764541 | <span style="color:#dc2626">-6047.76%</span> |
| 62 | [00995 VIEW_TRIGGER_GENERATED_048](crates/bench/sqlite_parity/cases/SQLITE_PARITY_995_VIEW_TRIGGER_GENERATED_048.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2510654 | 152107181 | <span style="color:#dc2626">-5958.47%</span> |
| 63 | [00004 TABLE_CONSTRAINTS_SUCCESS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_004_TABLE_CONSTRAINTS_SUCCESS.rs) | P0 | memory | SQL_CONSTRAINTS | 3066064 | 184323857 | <span style="color:#dc2626">-5911.74%</span> |
| 64 | [00805 WINDOW_PARTITION_SUM_018](crates/bench/sqlite_parity/cases/SQLITE_PARITY_805_WINDOW_PARTITION_SUM_018.rs) | P2 | memory | GEN_SQL_WINDOW | 3371743 | 201825544 | <span style="color:#dc2626">-5885.79%</span> |
| 65 | [00980 VIEW_TRIGGER_GENERATED_033](crates/bench/sqlite_parity/cases/SQLITE_PARITY_980_VIEW_TRIGGER_GENERATED_033.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 3188487 | 187500018 | <span style="color:#dc2626">-5780.53%</span> |
| 66 | [00813 WINDOW_PARTITION_SUM_026](crates/bench/sqlite_parity/cases/SQLITE_PARITY_813_WINDOW_PARTITION_SUM_026.rs) | P2 | memory | GEN_SQL_WINDOW | 2753723 | 161055501 | <span style="color:#dc2626">-5748.65%</span> |
| 67 | [00979 VIEW_TRIGGER_GENERATED_032](crates/bench/sqlite_parity/cases/SQLITE_PARITY_979_VIEW_TRIGGER_GENERATED_032.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2866537 | 162470510 | <span style="color:#dc2626">-5567.83%</span> |
| 68 | [00948 VIEW_TRIGGER_GENERATED_001](crates/bench/sqlite_parity/cases/SQLITE_PARITY_948_VIEW_TRIGGER_GENERATED_001.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2692127 | 152280803 | <span style="color:#dc2626">-5556.52%</span> |
| 69 | [00282 SCALAR_STRING_014](crates/bench/sqlite_parity/cases/SQLITE_PARITY_282_SCALAR_STRING_014.rs) | P1 | memory | GEN_SQL_SCALAR | 2878259 | 162383744 | <span style="color:#dc2626">-5541.73%</span> |
| 70 | [00115 DOT_SCHEMA_TABLES_INDEXES](crates/bench/sqlite_parity/cases/SQLITE_PARITY_115_DOT_SCHEMA_TABLES_INDEXES.rs) | P0 | memory | CLI_DOT_COMMAND | 3573394 | 199335178 | <span style="color:#dc2626">-5478.32%</span> |
| 71 | [00974 VIEW_TRIGGER_GENERATED_027](crates/bench/sqlite_parity/cases/SQLITE_PARITY_974_VIEW_TRIGGER_GENERATED_027.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2853473 | 157960741 | <span style="color:#dc2626">-5435.74%</span> |
| 72 | [00810 WINDOW_PARTITION_SUM_023](crates/bench/sqlite_parity/cases/SQLITE_PARITY_810_WINDOW_PARTITION_SUM_023.rs) | P2 | memory | GEN_SQL_WINDOW | 2924937 | 161781345 | <span style="color:#dc2626">-5431.11%</span> |
| 73 | [00969 VIEW_TRIGGER_GENERATED_022](crates/bench/sqlite_parity/cases/SQLITE_PARITY_969_VIEW_TRIGGER_GENERATED_022.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2462612 | 135467265 | <span style="color:#dc2626">-5400.96%</span> |
| 74 | [01006 VIEW_TRIGGER_GENERATED_059](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1006_VIEW_TRIGGER_GENERATED_059.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2762500 | 151708357 | <span style="color:#dc2626">-5391.71%</span> |
| 75 | [00953 VIEW_TRIGGER_GENERATED_006](crates/bench/sqlite_parity/cases/SQLITE_PARITY_953_VIEW_TRIGGER_GENERATED_006.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2729427 | 149615640 | <span style="color:#dc2626">-5381.58%</span> |
| 76 | [01005 VIEW_TRIGGER_GENERATED_058](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1005_VIEW_TRIGGER_GENERATED_058.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2473864 | 135235579 | <span style="color:#dc2626">-5366.57%</span> |
| 77 | [00993 VIEW_TRIGGER_GENERATED_046](crates/bench/sqlite_parity/cases/SQLITE_PARITY_993_VIEW_TRIGGER_GENERATED_046.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2642183 | 143672111 | <span style="color:#dc2626">-5337.63%</span> |
| 78 | [00973 VIEW_TRIGGER_GENERATED_026](crates/bench/sqlite_parity/cases/SQLITE_PARITY_973_VIEW_TRIGGER_GENERATED_026.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2703509 | 146900752 | <span style="color:#dc2626">-5333.71%</span> |
| 79 | [00968 VIEW_TRIGGER_GENERATED_021](crates/bench/sqlite_parity/cases/SQLITE_PARITY_968_VIEW_TRIGGER_GENERATED_021.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 3375672 | 180480487 | <span style="color:#dc2626">-5246.51%</span> |
| 80 | [00994 VIEW_TRIGGER_GENERATED_047](crates/bench/sqlite_parity/cases/SQLITE_PARITY_994_VIEW_TRIGGER_GENERATED_047.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2903477 | 152077766 | <span style="color:#dc2626">-5137.78%</span> |
| 81 | [00867 WINDOW_PARTITION_SUM_080](crates/bench/sqlite_parity/cases/SQLITE_PARITY_867_WINDOW_PARTITION_SUM_080.rs) | P2 | memory | GEN_SQL_WINDOW | 3174149 | 164029444 | <span style="color:#dc2626">-5067.67%</span> |
| 82 | [00951 VIEW_TRIGGER_GENERATED_004](crates/bench/sqlite_parity/cases/SQLITE_PARITY_951_VIEW_TRIGGER_GENERATED_004.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2807966 | 144979022 | <span style="color:#dc2626">-5063.13%</span> |
| 83 | [00804 WINDOW_PARTITION_SUM_017](crates/bench/sqlite_parity/cases/SQLITE_PARITY_804_WINDOW_PARTITION_SUM_017.rs) | P2 | memory | GEN_SQL_WINDOW | 2616874 | 134291579 | <span style="color:#dc2626">-5031.76%</span> |
| 84 | [00116 DOT_DATABASES](crates/bench/sqlite_parity/cases/SQLITE_PARITY_116_DOT_DATABASES.rs) | P0 | memory | CLI_DOT_COMMAND | 2096589 | 105876458 | <span style="color:#dc2626">-4949.94%</span> |
| 85 | [00991 VIEW_TRIGGER_GENERATED_044](crates/bench/sqlite_parity/cases/SQLITE_PARITY_991_VIEW_TRIGGER_GENERATED_044.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2757851 | 138591271 | <span style="color:#dc2626">-4925.34%</span> |
| 86 | [00997 VIEW_TRIGGER_GENERATED_050](crates/bench/sqlite_parity/cases/SQLITE_PARITY_997_VIEW_TRIGGER_GENERATED_050.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2935307 | 147149595 | <span style="color:#dc2626">-4913.09%</span> |
| 87 | [00998 VIEW_TRIGGER_GENERATED_051](crates/bench/sqlite_parity/cases/SQLITE_PARITY_998_VIEW_TRIGGER_GENERATED_051.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2654426 | 131792280 | <span style="color:#dc2626">-4865.00%</span> |
| 88 | [00975 VIEW_TRIGGER_GENERATED_028](crates/bench/sqlite_parity/cases/SQLITE_PARITY_975_VIEW_TRIGGER_GENERATED_028.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 3364570 | 163673447 | <span style="color:#dc2626">-4764.62%</span> |
| 89 | [00986 VIEW_TRIGGER_GENERATED_039](crates/bench/sqlite_parity/cases/SQLITE_PARITY_986_VIEW_TRIGGER_GENERATED_039.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 3103516 | 150743859 | <span style="color:#dc2626">-4757.20%</span> |
| 90 | [00976 VIEW_TRIGGER_GENERATED_029](crates/bench/sqlite_parity/cases/SQLITE_PARITY_976_VIEW_TRIGGER_GENERATED_029.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2989459 | 143290366 | <span style="color:#dc2626">-4693.19%</span> |
| 91 | [00109 DOT_MODE_JSON](crates/bench/sqlite_parity/cases/SQLITE_PARITY_109_DOT_MODE_JSON.rs) | P0 | memory | CLI_DOT_COMMAND | 2199073 | 105234261 | <span style="color:#dc2626">-4685.39%</span> |
| 92 | [00628 JOIN_SUBQUERY_EXISTS_021](crates/bench/sqlite_parity/cases/SQLITE_PARITY_628_JOIN_SUBQUERY_EXISTS_021.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2329319 | 111463516 | <span style="color:#dc2626">-4685.24%</span> |
| 93 | [00978 VIEW_TRIGGER_GENERATED_031](crates/bench/sqlite_parity/cases/SQLITE_PARITY_978_VIEW_TRIGGER_GENERATED_031.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 3035247 | 144479769 | <span style="color:#dc2626">-4660.07%</span> |
| 94 | [00964 VIEW_TRIGGER_GENERATED_017](crates/bench/sqlite_parity/cases/SQLITE_PARITY_964_VIEW_TRIGGER_GENERATED_017.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2911142 | 138443549 | <span style="color:#dc2626">-4655.64%</span> |
| 95 | [01000 VIEW_TRIGGER_GENERATED_053](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1000_VIEW_TRIGGER_GENERATED_053.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2771768 | 131469409 | <span style="color:#dc2626">-4643.16%</span> |
| 96 | [00284 SCALAR_ARITH_015](crates/bench/sqlite_parity/cases/SQLITE_PARITY_284_SCALAR_ARITH_015.rs) | P1 | memory | GEN_SQL_SCALAR | 2080689 | 98623652 | <span style="color:#dc2626">-4639.95%</span> |
| 97 | [01001 VIEW_TRIGGER_GENERATED_054](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1001_VIEW_TRIGGER_GENERATED_054.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2817665 | 132719425 | <span style="color:#dc2626">-4610.26%</span> |
| 98 | [00960 VIEW_TRIGGER_GENERATED_013](crates/bench/sqlite_parity/cases/SQLITE_PARITY_960_VIEW_TRIGGER_GENERATED_013.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2937882 | 137224901 | <span style="color:#dc2626">-4570.88%</span> |
| 99 | [01003 VIEW_TRIGGER_GENERATED_056](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1003_VIEW_TRIGGER_GENERATED_056.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2898087 | 135339916 | <span style="color:#dc2626">-4569.97%</span> |
| 100 | [00114 DOT_PRINT](crates/bench/sqlite_parity/cases/SQLITE_PARITY_114_DOT_PRINT.rs) | P0 | memory | CLI_DOT_COMMAND | 1847477 | 85806694 | <span style="color:#dc2626">-4544.53%</span> |
| 101 | [00984 VIEW_TRIGGER_GENERATED_037](crates/bench/sqlite_parity/cases/SQLITE_PARITY_984_VIEW_TRIGGER_GENERATED_037.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 3095010 | 143236195 | <span style="color:#dc2626">-4527.97%</span> |
| 102 | [00892 CONSTRAINT_FK_SAVEPOINT_025](crates/bench/sqlite_parity/cases/SQLITE_PARITY_892_CONSTRAINT_FK_SAVEPOINT_025.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1987312 | 91089713 | <span style="color:#dc2626">-4483.56%</span> |
| 103 | [00965 VIEW_TRIGGER_GENERATED_018](crates/bench/sqlite_parity/cases/SQLITE_PARITY_965_VIEW_TRIGGER_GENERATED_018.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 3008366 | 137776286 | <span style="color:#dc2626">-4479.77%</span> |
| 104 | [00990 VIEW_TRIGGER_GENERATED_043](crates/bench/sqlite_parity/cases/SQLITE_PARITY_990_VIEW_TRIGGER_GENERATED_043.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 3011802 | 136923474 | <span style="color:#dc2626">-4446.23%</span> |
| 105 | [00961 VIEW_TRIGGER_GENERATED_014](crates/bench/sqlite_parity/cases/SQLITE_PARITY_961_VIEW_TRIGGER_GENERATED_014.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 3266976 | 147452004 | <span style="color:#dc2626">-4413.41%</span> |
| 106 | [00957 VIEW_TRIGGER_GENERATED_010](crates/bench/sqlite_parity/cases/SQLITE_PARITY_957_VIEW_TRIGGER_GENERATED_010.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 3031108 | 136169383 | <span style="color:#dc2626">-4392.40%</span> |
| 107 | [00963 VIEW_TRIGGER_GENERATED_016](crates/bench/sqlite_parity/cases/SQLITE_PARITY_963_VIEW_TRIGGER_GENERATED_016.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 3049523 | 136703425 | <span style="color:#dc2626">-4382.78%</span> |
| 108 | [00992 VIEW_TRIGGER_GENERATED_045](crates/bench/sqlite_parity/cases/SQLITE_PARITY_992_VIEW_TRIGGER_GENERATED_045.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 3111471 | 138820326 | <span style="color:#dc2626">-4361.57%</span> |
| 109 | [00172 OPT_CMD](crates/bench/sqlite_parity/cases/SQLITE_PARITY_172_OPT_CMD.rs) | P1 | memory | CLI_OPTION | 2513368 | 112002826 | <span style="color:#dc2626">-4356.28%</span> |
| 110 | [00999 VIEW_TRIGGER_GENERATED_052](crates/bench/sqlite_parity/cases/SQLITE_PARITY_999_VIEW_TRIGGER_GENERATED_052.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 3021281 | 134123604 | <span style="color:#dc2626">-4339.30%</span> |
| 111 | [00989 VIEW_TRIGGER_GENERATED_042](crates/bench/sqlite_parity/cases/SQLITE_PARITY_989_VIEW_TRIGGER_GENERATED_042.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 3183808 | 141227402 | <span style="color:#dc2626">-4335.80%</span> |
| 112 | [00030 ALTER_TABLE_ADD_DROP_COLUMN](crates/bench/sqlite_parity/cases/SQLITE_PARITY_030_ALTER_TABLE_ADD_DROP_COLUMN.rs) | P0 | memory | SQL_ALTER | 2889210 | 127522493 | <span style="color:#dc2626">-4313.75%</span> |
| 113 | [01007 VIEW_TRIGGER_GENERATED_060](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1007_VIEW_TRIGGER_GENERATED_060.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 3093317 | 135669521 | <span style="color:#dc2626">-4285.89%</span> |
| 114 | [00985 VIEW_TRIGGER_GENERATED_038](crates/bench/sqlite_parity/cases/SQLITE_PARITY_985_VIEW_TRIGGER_GENERATED_038.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2983900 | 130685603 | <span style="color:#dc2626">-4279.69%</span> |
| 115 | [00811 WINDOW_PARTITION_SUM_024](crates/bench/sqlite_parity/cases/SQLITE_PARITY_811_WINDOW_PARTITION_SUM_024.rs) | P2 | memory | GEN_SQL_WINDOW | 3620254 | 158206727 | <span style="color:#dc2626">-4270.04%</span> |
| 116 | [00958 VIEW_TRIGGER_GENERATED_011](crates/bench/sqlite_parity/cases/SQLITE_PARITY_958_VIEW_TRIGGER_GENERATED_011.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 3113534 | 135983732 | <span style="color:#dc2626">-4267.50%</span> |
| 117 | [00983 VIEW_TRIGGER_GENERATED_036](crates/bench/sqlite_parity/cases/SQLITE_PARITY_983_VIEW_TRIGGER_GENERATED_036.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 3075032 | 132934440 | <span style="color:#dc2626">-4223.03%</span> |
| 118 | [00970 VIEW_TRIGGER_GENERATED_023](crates/bench/sqlite_parity/cases/SQLITE_PARITY_970_VIEW_TRIGGER_GENERATED_023.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 3225648 | 137151744 | <span style="color:#dc2626">-4151.91%</span> |
| 119 | [00977 VIEW_TRIGGER_GENERATED_030](crates/bench/sqlite_parity/cases/SQLITE_PARITY_977_VIEW_TRIGGER_GENERATED_030.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 3089299 | 130997644 | <span style="color:#dc2626">-4140.37%</span> |
| 120 | [01004 VIEW_TRIGGER_GENERATED_057](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1004_VIEW_TRIGGER_GENERATED_057.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 3214345 | 135805648 | <span style="color:#dc2626">-4124.99%</span> |
| 121 | [01002 VIEW_TRIGGER_GENERATED_055](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1002_VIEW_TRIGGER_GENERATED_055.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 3227069 | 133685104 | <span style="color:#dc2626">-4042.62%</span> |
| 122 | [00982 VIEW_TRIGGER_GENERATED_035](crates/bench/sqlite_parity/cases/SQLITE_PARITY_982_VIEW_TRIGGER_GENERATED_035.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 3571652 | 147706488 | <span style="color:#dc2626">-4035.52%</span> |
| 123 | [00937 CONSTRAINT_FK_SAVEPOINT_070](crates/bench/sqlite_parity/cases/SQLITE_PARITY_937_CONSTRAINT_FK_SAVEPOINT_070.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2452343 | 100382076 | <span style="color:#dc2626">-3993.31%</span> |
| 124 | [00962 VIEW_TRIGGER_GENERATED_015](crates/bench/sqlite_parity/cases/SQLITE_PARITY_962_VIEW_TRIGGER_GENERATED_015.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 3394887 | 137180368 | <span style="color:#dc2626">-3940.79%</span> |
| 125 | [00925 CONSTRAINT_FK_SAVEPOINT_058](crates/bench/sqlite_parity/cases/SQLITE_PARITY_925_CONSTRAINT_FK_SAVEPOINT_058.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2186289 | 87993973 | <span style="color:#dc2626">-3924.81%</span> |
| 126 | [00959 VIEW_TRIGGER_GENERATED_012](crates/bench/sqlite_parity/cases/SQLITE_PARITY_959_VIEW_TRIGGER_GENERATED_012.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 3365612 | 134318118 | <span style="color:#dc2626">-3890.90%</span> |
| 127 | [00952 VIEW_TRIGGER_GENERATED_005](crates/bench/sqlite_parity/cases/SQLITE_PARITY_952_VIEW_TRIGGER_GENERATED_005.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 3602150 | 141298162 | <span style="color:#dc2626">-3822.61%</span> |
| 128 | [00653 JOIN_SUBQUERY_EXISTS_046](crates/bench/sqlite_parity/cases/SQLITE_PARITY_653_JOIN_SUBQUERY_EXISTS_046.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2655888 | 103854314 | <span style="color:#dc2626">-3810.34%</span> |
| 129 | [00943 CONSTRAINT_FK_SAVEPOINT_076](crates/bench/sqlite_parity/cases/SQLITE_PARITY_943_CONSTRAINT_FK_SAVEPOINT_076.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2500794 | 97417685 | <span style="color:#dc2626">-3795.47%</span> |
| 130 | [00940 CONSTRAINT_FK_SAVEPOINT_073](crates/bench/sqlite_parity/cases/SQLITE_PARITY_940_CONSTRAINT_FK_SAVEPOINT_073.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2562591 | 98900532 | <span style="color:#dc2626">-3759.40%</span> |
| 131 | [00289 SCALAR_CAST_TYPEOF_016](crates/bench/sqlite_parity/cases/SQLITE_PARITY_289_SCALAR_CAST_TYPEOF_016.rs) | P1 | memory | GEN_SQL_SCALAR | 2176179 | 82691320 | <span style="color:#dc2626">-3699.84%</span> |
| 132 | [00639 JOIN_SUBQUERY_EXISTS_032](crates/bench/sqlite_parity/cases/SQLITE_PARITY_639_JOIN_SUBQUERY_EXISTS_032.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3044964 | 115679308 | <span style="color:#dc2626">-3699.04%</span> |
| 133 | [00482 DML_WHERE_ORDER_LIMIT_095](crates/bench/sqlite_parity/cases/SQLITE_PARITY_482_DML_WHERE_ORDER_LIMIT_095.rs) | P1 | memory | GEN_SQL_DML | 2864372 | 107379093 | <span style="color:#dc2626">-3648.78%</span> |
| 134 | [00934 CONSTRAINT_FK_SAVEPOINT_067](crates/bench/sqlite_parity/cases/SQLITE_PARITY_934_CONSTRAINT_FK_SAVEPOINT_067.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2704029 | 100420729 | <span style="color:#dc2626">-3613.74%</span> |
| 135 | [00451 DML_WHERE_ORDER_LIMIT_064](crates/bench/sqlite_parity/cases/SQLITE_PARITY_451_DML_WHERE_ORDER_LIMIT_064.rs) | P1 | memory | GEN_SQL_DML | 2620161 | 96498030 | <span style="color:#dc2626">-3582.90%</span> |
| 136 | [00971 VIEW_TRIGGER_GENERATED_024](crates/bench/sqlite_parity/cases/SQLITE_PARITY_971_VIEW_TRIGGER_GENERATED_024.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 3628960 | 133227364 | <span style="color:#dc2626">-3571.23%</span> |
| 137 | [00676 JOIN_SUBQUERY_EXISTS_069](crates/bench/sqlite_parity/cases/SQLITE_PARITY_676_JOIN_SUBQUERY_EXISTS_069.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2108391 | 76306762 | <span style="color:#dc2626">-3519.19%</span> |
| 138 | [00656 JOIN_SUBQUERY_EXISTS_049](crates/bench/sqlite_parity/cases/SQLITE_PARITY_656_JOIN_SUBQUERY_EXISTS_049.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2073225 | 74988917 | <span style="color:#dc2626">-3517.02%</span> |
| 139 | [00629 JOIN_SUBQUERY_EXISTS_022](crates/bench/sqlite_parity/cases/SQLITE_PARITY_629_JOIN_SUBQUERY_EXISTS_022.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2491226 | 89543058 | <span style="color:#dc2626">-3494.34%</span> |
| 140 | [00293 SCALAR_CAST_TYPEOF_017](crates/bench/sqlite_parity/cases/SQLITE_PARITY_293_SCALAR_CAST_TYPEOF_017.rs) | P1 | memory | GEN_SQL_SCALAR | 2178144 | 77269697 | <span style="color:#dc2626">-3447.50%</span> |
| 141 | [00700 JOIN_SUBQUERY_EXISTS_093](crates/bench/sqlite_parity/cases/SQLITE_PARITY_700_JOIN_SUBQUERY_EXISTS_093.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2082352 | 73649542 | <span style="color:#dc2626">-3436.84%</span> |
| 142 | [00886 CONSTRAINT_FK_SAVEPOINT_019](crates/bench/sqlite_parity/cases/SQLITE_PARITY_886_CONSTRAINT_FK_SAVEPOINT_019.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 3023203 | 106474781 | <span style="color:#dc2626">-3421.92%</span> |
| 143 | [00608 JOIN_SUBQUERY_EXISTS_001](crates/bench/sqlite_parity/cases/SQLITE_PARITY_608_JOIN_SUBQUERY_EXISTS_001.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1986110 | 69813887 | <span style="color:#dc2626">-3415.11%</span> |
| 144 | [00654 JOIN_SUBQUERY_EXISTS_047](crates/bench/sqlite_parity/cases/SQLITE_PARITY_654_JOIN_SUBQUERY_EXISTS_047.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2079798 | 73058361 | <span style="color:#dc2626">-3412.76%</span> |
| 145 | [00483 DML_WHERE_ORDER_LIMIT_096](crates/bench/sqlite_parity/cases/SQLITE_PARITY_483_DML_WHERE_ORDER_LIMIT_096.rs) | P1 | memory | GEN_SQL_DML | 2304703 | 80945981 | <span style="color:#dc2626">-3412.21%</span> |
| 146 | [00946 CONSTRAINT_FK_SAVEPOINT_079](crates/bench/sqlite_parity/cases/SQLITE_PARITY_946_CONSTRAINT_FK_SAVEPOINT_079.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2576228 | 90192406 | <span style="color:#dc2626">-3400.95%</span> |
| 147 | [00907 CONSTRAINT_FK_SAVEPOINT_040](crates/bench/sqlite_parity/cases/SQLITE_PARITY_907_CONSTRAINT_FK_SAVEPOINT_040.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2682559 | 92966937 | <span style="color:#dc2626">-3365.61%</span> |
| 148 | [00895 CONSTRAINT_FK_SAVEPOINT_028](crates/bench/sqlite_parity/cases/SQLITE_PARITY_895_CONSTRAINT_FK_SAVEPOINT_028.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2679443 | 92400215 | <span style="color:#dc2626">-3348.49%</span> |
| 149 | [00901 CONSTRAINT_FK_SAVEPOINT_034](crates/bench/sqlite_parity/cases/SQLITE_PARITY_901_CONSTRAINT_FK_SAVEPOINT_034.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2888118 | 99157067 | <span style="color:#dc2626">-3333.28%</span> |
| 150 | [00678 JOIN_SUBQUERY_EXISTS_071](crates/bench/sqlite_parity/cases/SQLITE_PARITY_678_JOIN_SUBQUERY_EXISTS_071.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2133199 | 73216041 | <span style="color:#dc2626">-3332.22%</span> |
| 151 | [00931 CONSTRAINT_FK_SAVEPOINT_064](crates/bench/sqlite_parity/cases/SQLITE_PARITY_931_CONSTRAINT_FK_SAVEPOINT_064.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2932962 | 100093049 | <span style="color:#dc2626">-3312.70%</span> |
| 152 | [00657 JOIN_SUBQUERY_EXISTS_050](crates/bench/sqlite_parity/cases/SQLITE_PARITY_657_JOIN_SUBQUERY_EXISTS_050.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2319170 | 78838615 | <span style="color:#dc2626">-3299.43%</span> |
| 153 | [00913 CONSTRAINT_FK_SAVEPOINT_046](crates/bench/sqlite_parity/cases/SQLITE_PARITY_913_CONSTRAINT_FK_SAVEPOINT_046.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2767279 | 94040630 | <span style="color:#dc2626">-3298.31%</span> |
| 154 | [00041 DROP_TRIGGER](crates/bench/sqlite_parity/cases/SQLITE_PARITY_041_DROP_TRIGGER.rs) | P0 | memory | SQL_DROP | 2273043 | 77020722 | <span style="color:#dc2626">-3288.44%</span> |
| 155 | [00889 CONSTRAINT_FK_SAVEPOINT_022](crates/bench/sqlite_parity/cases/SQLITE_PARITY_889_CONSTRAINT_FK_SAVEPOINT_022.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2672129 | 90314987 | <span style="color:#dc2626">-3279.89%</span> |
| 156 | [00292 SCALAR_ARITH_017](crates/bench/sqlite_parity/cases/SQLITE_PARITY_292_SCALAR_ARITH_017.rs) | P1 | memory | GEN_SQL_SCALAR | 2167262 | 72957233 | <span style="color:#dc2626">-3266.33%</span> |
| 157 | [00928 CONSTRAINT_FK_SAVEPOINT_061](crates/bench/sqlite_parity/cases/SQLITE_PARITY_928_CONSTRAINT_FK_SAVEPOINT_061.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2679674 | 90126210 | <span style="color:#dc2626">-3263.33%</span> |
| 158 | [00874 CONSTRAINT_FK_SAVEPOINT_007](crates/bench/sqlite_parity/cases/SQLITE_PARITY_874_CONSTRAINT_FK_SAVEPOINT_007.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2652362 | 88754822 | <span style="color:#dc2626">-3246.26%</span> |
| 159 | [00898 CONSTRAINT_FK_SAVEPOINT_031](crates/bench/sqlite_parity/cases/SQLITE_PARITY_898_CONSTRAINT_FK_SAVEPOINT_031.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2729819 | 91140529 | <span style="color:#dc2626">-3238.70%</span> |
| 160 | [00500 DML_WHERE_ORDER_LIMIT_113](crates/bench/sqlite_parity/cases/SQLITE_PARITY_500_DML_WHERE_ORDER_LIMIT_113.rs) | P1 | memory | GEN_SQL_DML | 2563683 | 85033640 | <span style="color:#dc2626">-3216.85%</span> |
| 161 | [00614 JOIN_SUBQUERY_EXISTS_007](crates/bench/sqlite_parity/cases/SQLITE_PARITY_614_JOIN_SUBQUERY_EXISTS_007.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2050873 | 67953555 | <span style="color:#dc2626">-3213.40%</span> |
| 162 | [00668 JOIN_SUBQUERY_EXISTS_061](crates/bench/sqlite_parity/cases/SQLITE_PARITY_668_JOIN_SUBQUERY_EXISTS_061.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2117769 | 69307201 | <span style="color:#dc2626">-3172.65%</span> |
| 163 | [00922 CONSTRAINT_FK_SAVEPOINT_055](crates/bench/sqlite_parity/cases/SQLITE_PARITY_922_CONSTRAINT_FK_SAVEPOINT_055.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2748674 | 89930299 | <span style="color:#dc2626">-3171.77%</span> |
| 164 | [00916 CONSTRAINT_FK_SAVEPOINT_049](crates/bench/sqlite_parity/cases/SQLITE_PARITY_916_CONSTRAINT_FK_SAVEPOINT_049.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2828585 | 91992242 | <span style="color:#dc2626">-3152.24%</span> |
| 165 | [00880 CONSTRAINT_FK_SAVEPOINT_013](crates/bench/sqlite_parity/cases/SQLITE_PARITY_880_CONSTRAINT_FK_SAVEPOINT_013.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2812875 | 90478165 | <span style="color:#dc2626">-3116.57%</span> |
| 166 | [00682 JOIN_SUBQUERY_EXISTS_075](crates/bench/sqlite_parity/cases/SQLITE_PARITY_682_JOIN_SUBQUERY_EXISTS_075.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2409541 | 77205885 | <span style="color:#dc2626">-3104.17%</span> |
| 167 | [00670 JOIN_SUBQUERY_EXISTS_063](crates/bench/sqlite_parity/cases/SQLITE_PARITY_670_JOIN_SUBQUERY_EXISTS_063.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2224872 | 71230782 | <span style="color:#dc2626">-3101.57%</span> |
| 168 | [00704 JOIN_SUBQUERY_EXISTS_097](crates/bench/sqlite_parity/cases/SQLITE_PARITY_704_JOIN_SUBQUERY_EXISTS_097.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2254468 | 72121930 | <span style="color:#dc2626">-3099.07%</span> |
| 169 | [00673 JOIN_SUBQUERY_EXISTS_066](crates/bench/sqlite_parity/cases/SQLITE_PARITY_673_JOIN_SUBQUERY_EXISTS_066.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2672429 | 85018875 | <span style="color:#dc2626">-3081.33%</span> |
| 170 | [00464 DML_WHERE_ORDER_LIMIT_077](crates/bench/sqlite_parity/cases/SQLITE_PARITY_464_DML_WHERE_ORDER_LIMIT_077.rs) | P1 | memory | GEN_SQL_DML | 2436362 | 77144153 | <span style="color:#dc2626">-3066.37%</span> |
| 171 | [00288 SCALAR_ARITH_016](crates/bench/sqlite_parity/cases/SQLITE_PARITY_288_SCALAR_ARITH_016.rs) | P1 | memory | GEN_SQL_SCALAR | 2534848 | 79325860 | <span style="color:#dc2626">-3029.41%</span> |
| 172 | [00877 CONSTRAINT_FK_SAVEPOINT_010](crates/bench/sqlite_parity/cases/SQLITE_PARITY_877_CONSTRAINT_FK_SAVEPOINT_010.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 3008355 | 93487082 | <span style="color:#dc2626">-3007.58%</span> |
| 173 | [00919 CONSTRAINT_FK_SAVEPOINT_052](crates/bench/sqlite_parity/cases/SQLITE_PARITY_919_CONSTRAINT_FK_SAVEPOINT_052.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 3026690 | 93914402 | <span style="color:#dc2626">-3002.87%</span> |
| 174 | [00035 DROP_INDEX](crates/bench/sqlite_parity/cases/SQLITE_PARITY_035_DROP_INDEX.rs) | P0 | memory | SQL_DROP | 2432885 | 75403320 | <span style="color:#dc2626">-2999.34%</span> |
| 175 | [00613 JOIN_SUBQUERY_EXISTS_006](crates/bench/sqlite_parity/cases/SQLITE_PARITY_613_JOIN_SUBQUERY_EXISTS_006.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2110045 | 64581442 | <span style="color:#dc2626">-2960.67%</span> |
| 176 | [00816 WINDOW_PARTITION_SUM_029](crates/bench/sqlite_parity/cases/SQLITE_PARITY_816_WINDOW_PARTITION_SUM_029.rs) | P2 | memory | GEN_SQL_WINDOW | 2059600 | 62925261 | <span style="color:#dc2626">-2955.22%</span> |
| 177 | [00024 SAVEPOINT_ROLLBACK_RELEASE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_024_SAVEPOINT_ROLLBACK_RELEASE.rs) | P0 | memory | SQL_SAVEPOINT | 2249889 | 68498017 | <span style="color:#dc2626">-2944.51%</span> |
| 178 | [00444 DML_WHERE_ORDER_LIMIT_057](crates/bench/sqlite_parity/cases/SQLITE_PARITY_444_DML_WHERE_ORDER_LIMIT_057.rs) | P1 | memory | GEN_SQL_DML | 2275267 | 69022126 | <span style="color:#dc2626">-2933.58%</span> |
| 179 | [00611 JOIN_SUBQUERY_EXISTS_004](crates/bench/sqlite_parity/cases/SQLITE_PARITY_611_JOIN_SUBQUERY_EXISTS_004.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2214041 | 66975874 | <span style="color:#dc2626">-2925.05%</span> |
| 180 | [00048 PRAGMA_USER_VERSION](crates/bench/sqlite_parity/cases/SQLITE_PARITY_048_PRAGMA_USER_VERSION.rs) | P0 | memory | SQL_PRAGMA | 1766073 | 53366725 | <span style="color:#dc2626">-2921.77%</span> |
| 181 | [00662 JOIN_SUBQUERY_EXISTS_055](crates/bench/sqlite_parity/cases/SQLITE_PARITY_662_JOIN_SUBQUERY_EXISTS_055.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3283396 | 98046639 | <span style="color:#dc2626">-2886.14%</span> |
| 182 | [00699 JOIN_SUBQUERY_EXISTS_092](crates/bench/sqlite_parity/cases/SQLITE_PARITY_699_JOIN_SUBQUERY_EXISTS_092.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2410974 | 71960364 | <span style="color:#dc2626">-2884.70%</span> |
| 183 | [00498 DML_WHERE_ORDER_LIMIT_111](crates/bench/sqlite_parity/cases/SQLITE_PARITY_498_DML_WHERE_ORDER_LIMIT_111.rs) | P1 | memory | GEN_SQL_DML | 2152144 | 63970956 | <span style="color:#dc2626">-2872.43%</span> |
| 184 | [00904 CONSTRAINT_FK_SAVEPOINT_037](crates/bench/sqlite_parity/cases/SQLITE_PARITY_904_CONSTRAINT_FK_SAVEPOINT_037.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 3124215 | 92682490 | <span style="color:#dc2626">-2866.58%</span> |
| 185 | [00910 CONSTRAINT_FK_SAVEPOINT_043](crates/bench/sqlite_parity/cases/SQLITE_PARITY_910_CONSTRAINT_FK_SAVEPOINT_043.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 3311540 | 98121587 | <span style="color:#dc2626">-2863.02%</span> |
| 186 | [00630 JOIN_SUBQUERY_EXISTS_023](crates/bench/sqlite_parity/cases/SQLITE_PARITY_630_JOIN_SUBQUERY_EXISTS_023.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2506355 | 73839358 | <span style="color:#dc2626">-2846.09%</span> |
| 187 | [00478 DML_WHERE_ORDER_LIMIT_091](crates/bench/sqlite_parity/cases/SQLITE_PARITY_478_DML_WHERE_ORDER_LIMIT_091.rs) | P1 | memory | GEN_SQL_DML | 1945924 | 57226327 | <span style="color:#dc2626">-2840.83%</span> |
| 188 | [00665 JOIN_SUBQUERY_EXISTS_058](crates/bench/sqlite_parity/cases/SQLITE_PARITY_665_JOIN_SUBQUERY_EXISTS_058.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2366941 | 69170231 | <span style="color:#dc2626">-2822.35%</span> |
| 189 | [00038 CREATE_TRIGGER_AFTER](crates/bench/sqlite_parity/cases/SQLITE_PARITY_038_CREATE_TRIGGER_AFTER.rs) | P0 | memory | SQL_TRIGGER | 2832281 | 82653935 | <span style="color:#dc2626">-2818.28%</span> |
| 190 | [00702 JOIN_SUBQUERY_EXISTS_095](crates/bench/sqlite_parity/cases/SQLITE_PARITY_702_JOIN_SUBQUERY_EXISTS_095.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2397038 | 69951511 | <span style="color:#dc2626">-2818.25%</span> |
| 191 | [00286 SCALAR_STRING_015](crates/bench/sqlite_parity/cases/SQLITE_PARITY_286_SCALAR_STRING_015.rs) | P1 | memory | GEN_SQL_SCALAR | 2636421 | 76702563 | <span style="color:#dc2626">-2809.34%</span> |
| 192 | [00650 JOIN_SUBQUERY_EXISTS_043](crates/bench/sqlite_parity/cases/SQLITE_PARITY_650_JOIN_SUBQUERY_EXISTS_043.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2555208 | 73957374 | <span style="color:#dc2626">-2794.38%</span> |
| 193 | [00051 PRAGMA_INDEX_LIST_FUNCTION](crates/bench/sqlite_parity/cases/SQLITE_PARITY_051_PRAGMA_INDEX_LIST_FUNCTION.rs) | P0 | memory | SQL_PRAGMA | 2037958 | 58897865 | <span style="color:#dc2626">-2790.04%</span> |
| 194 | [00883 CONSTRAINT_FK_SAVEPOINT_016](crates/bench/sqlite_parity/cases/SQLITE_PARITY_883_CONSTRAINT_FK_SAVEPOINT_016.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 3231278 | 93369419 | <span style="color:#dc2626">-2789.55%</span> |
| 195 | [00683 JOIN_SUBQUERY_EXISTS_076](crates/bench/sqlite_parity/cases/SQLITE_PARITY_683_JOIN_SUBQUERY_EXISTS_076.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2612696 | 75464508 | <span style="color:#dc2626">-2788.38%</span> |
| 196 | [00638 JOIN_SUBQUERY_EXISTS_031](crates/bench/sqlite_parity/cases/SQLITE_PARITY_638_JOIN_SUBQUERY_EXISTS_031.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2649526 | 76425966 | <span style="color:#dc2626">-2784.51%</span> |
| 197 | [00666 JOIN_SUBQUERY_EXISTS_059](crates/bench/sqlite_parity/cases/SQLITE_PARITY_666_JOIN_SUBQUERY_EXISTS_059.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2474785 | 71342594 | <span style="color:#dc2626">-2782.78%</span> |
| 198 | [00112 DOT_SEPARATOR](crates/bench/sqlite_parity/cases/SQLITE_PARITY_112_DOT_SEPARATOR.rs) | P0 | memory | CLI_DOT_COMMAND | 2648634 | 75856369 | <span style="color:#dc2626">-2763.98%</span> |
| 199 | [00123 DOT_ECHO](crates/bench/sqlite_parity/cases/SQLITE_PARITY_123_DOT_ECHO.rs) | P0 | memory | CLI_DOT_COMMAND | 2928604 | 83816608 | <span style="color:#dc2626">-2762.00%</span> |
| 200 | [00143 DOT_QUIT](crates/bench/sqlite_parity/cases/SQLITE_PARITY_143_DOT_QUIT.rs) | P0 | memory | CLI_DOT_COMMAND | 2393771 | 68059970 | <span style="color:#dc2626">-2743.21%</span> |
| 201 | [00619 JOIN_SUBQUERY_EXISTS_012](crates/bench/sqlite_parity/cases/SQLITE_PARITY_619_JOIN_SUBQUERY_EXISTS_012.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2331815 | 66265380 | <span style="color:#dc2626">-2741.79%</span> |
| 202 | [00610 JOIN_SUBQUERY_EXISTS_003](crates/bench/sqlite_parity/cases/SQLITE_PARITY_610_JOIN_SUBQUERY_EXISTS_003.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2535099 | 71452810 | <span style="color:#dc2626">-2718.54%</span> |
| 203 | [00340 SCALAR_ARITH_029](crates/bench/sqlite_parity/cases/SQLITE_PARITY_340_SCALAR_ARITH_029.rs) | P1 | memory | GEN_SQL_SCALAR | 2704570 | 75754419 | <span style="color:#dc2626">-2700.98%</span> |
| 204 | [00627 JOIN_SUBQUERY_EXISTS_020](crates/bench/sqlite_parity/cases/SQLITE_PARITY_627_JOIN_SUBQUERY_EXISTS_020.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2540459 | 71155838 | <span style="color:#dc2626">-2700.90%</span> |
| 205 | [00636 JOIN_SUBQUERY_EXISTS_029](crates/bench/sqlite_parity/cases/SQLITE_PARITY_636_JOIN_SUBQUERY_EXISTS_029.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2636261 | 73727968 | <span style="color:#dc2626">-2696.69%</span> |
| 206 | [00499 DML_WHERE_ORDER_LIMIT_112](crates/bench/sqlite_parity/cases/SQLITE_PARITY_499_DML_WHERE_ORDER_LIMIT_112.rs) | P1 | memory | GEN_SQL_DML | 2312368 | 64664599 | <span style="color:#dc2626">-2696.47%</span> |
| 207 | [00687 JOIN_SUBQUERY_EXISTS_080](crates/bench/sqlite_parity/cases/SQLITE_PARITY_687_JOIN_SUBQUERY_EXISTS_080.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2719458 | 75551252 | <span style="color:#dc2626">-2678.17%</span> |
| 208 | [00008 STRICT_TABLE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_008_STRICT_TABLE.rs) | P0 | memory | SQL_DDL | 1798966 | 49956839 | <span style="color:#dc2626">-2676.98%</span> |
| 209 | [00681 JOIN_SUBQUERY_EXISTS_074](crates/bench/sqlite_parity/cases/SQLITE_PARITY_681_JOIN_SUBQUERY_EXISTS_074.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2567541 | 71245461 | <span style="color:#dc2626">-2674.85%</span> |
| 210 | [00468 DML_WHERE_ORDER_LIMIT_081](crates/bench/sqlite_parity/cases/SQLITE_PARITY_468_DML_WHERE_ORDER_LIMIT_081.rs) | P1 | memory | GEN_SQL_DML | 2069849 | 57415986 | <span style="color:#dc2626">-2673.92%</span> |
| 211 | [00021 UPSERT_DO_NOTHING](crates/bench/sqlite_parity/cases/SQLITE_PARITY_021_UPSERT_DO_NOTHING.rs) | P0 | memory | SQL_UPSERT | 1875971 | 51997181 | <span style="color:#dc2626">-2671.75%</span> |
| 212 | [00680 JOIN_SUBQUERY_EXISTS_073](crates/bench/sqlite_parity/cases/SQLITE_PARITY_680_JOIN_SUBQUERY_EXISTS_073.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2552853 | 70657297 | <span style="color:#dc2626">-2667.78%</span> |
| 213 | [00862 WINDOW_PARTITION_SUM_075](crates/bench/sqlite_parity/cases/SQLITE_PARITY_862_WINDOW_PARTITION_SUM_075.rs) | P2 | memory | GEN_SQL_WINDOW | 3107202 | 85363923 | <span style="color:#dc2626">-2647.29%</span> |
| 214 | [00475 DML_WHERE_ORDER_LIMIT_088](crates/bench/sqlite_parity/cases/SQLITE_PARITY_475_DML_WHERE_ORDER_LIMIT_088.rs) | P1 | memory | GEN_SQL_DML | 2157224 | 59196366 | <span style="color:#dc2626">-2644.10%</span> |
| 215 | [00490 DML_WHERE_ORDER_LIMIT_103](crates/bench/sqlite_parity/cases/SQLITE_PARITY_490_DML_WHERE_ORDER_LIMIT_103.rs) | P1 | memory | GEN_SQL_DML | 2101308 | 57519381 | <span style="color:#dc2626">-2637.31%</span> |
| 216 | [00436 DML_WHERE_ORDER_LIMIT_049](crates/bench/sqlite_parity/cases/SQLITE_PARITY_436_DML_WHERE_ORDER_LIMIT_049.rs) | P1 | memory | GEN_SQL_DML | 2246523 | 61298263 | <span style="color:#dc2626">-2628.58%</span> |
| 217 | [00637 JOIN_SUBQUERY_EXISTS_030](crates/bench/sqlite_parity/cases/SQLITE_PARITY_637_JOIN_SUBQUERY_EXISTS_030.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2875744 | 78445619 | <span style="color:#dc2626">-2627.84%</span> |
| 218 | [00861 WINDOW_PARTITION_SUM_074](crates/bench/sqlite_parity/cases/SQLITE_PARITY_861_WINDOW_PARTITION_SUM_074.rs) | P2 | memory | GEN_SQL_WINDOW | 1932459 | 52685525 | <span style="color:#dc2626">-2626.35%</span> |
| 219 | [00033 PARTIAL_INDEX](crates/bench/sqlite_parity/cases/SQLITE_PARITY_033_PARTIAL_INDEX.rs) | P0 | memory | SQL_INDEX | 2118130 | 57729433 | <span style="color:#dc2626">-2625.49%</span> |
| 220 | [00434 DML_WHERE_ORDER_LIMIT_047](crates/bench/sqlite_parity/cases/SQLITE_PARITY_434_DML_WHERE_ORDER_LIMIT_047.rs) | P1 | memory | GEN_SQL_DML | 2148948 | 58453227 | <span style="color:#dc2626">-2620.09%</span> |
| 221 | [00853 WINDOW_PARTITION_SUM_066](crates/bench/sqlite_parity/cases/SQLITE_PARITY_853_WINDOW_PARTITION_SUM_066.rs) | P2 | memory | GEN_SQL_WINDOW | 2600874 | 70603798 | <span style="color:#dc2626">-2614.62%</span> |
| 222 | [00659 JOIN_SUBQUERY_EXISTS_052](crates/bench/sqlite_parity/cases/SQLITE_PARITY_659_JOIN_SUBQUERY_EXISTS_052.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3109317 | 84261672 | <span style="color:#dc2626">-2609.97%</span> |
| 223 | [00422 DML_WHERE_ORDER_LIMIT_035](crates/bench/sqlite_parity/cases/SQLITE_PARITY_422_DML_WHERE_ORDER_LIMIT_035.rs) | P1 | memory | GEN_SQL_DML | 2093834 | 56728914 | <span style="color:#dc2626">-2609.33%</span> |
| 224 | [00432 DML_WHERE_ORDER_LIMIT_045](crates/bench/sqlite_parity/cases/SQLITE_PARITY_432_DML_WHERE_ORDER_LIMIT_045.rs) | P1 | memory | GEN_SQL_DML | 2116627 | 57301918 | <span style="color:#dc2626">-2607.23%</span> |
| 225 | [00675 JOIN_SUBQUERY_EXISTS_068](crates/bench/sqlite_parity/cases/SQLITE_PARITY_675_JOIN_SUBQUERY_EXISTS_068.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2660557 | 71800491 | <span style="color:#dc2626">-2598.70%</span> |
| 226 | [00706 JOIN_SUBQUERY_EXISTS_099](crates/bench/sqlite_parity/cases/SQLITE_PARITY_706_JOIN_SUBQUERY_EXISTS_099.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2627665 | 70811359 | <span style="color:#dc2626">-2594.84%</span> |
| 227 | [00449 DML_WHERE_ORDER_LIMIT_062](crates/bench/sqlite_parity/cases/SQLITE_PARITY_449_DML_WHERE_ORDER_LIMIT_062.rs) | P1 | memory | GEN_SQL_DML | 2425793 | 65287225 | <span style="color:#dc2626">-2591.38%</span> |
| 228 | [00026 FOREIGN_KEYS_CASCADE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_026_FOREIGN_KEYS_CASCADE.rs) | P0 | memory | SQL_FOREIGN_KEYS | 3076113 | 82413720 | <span style="color:#dc2626">-2579.15%</span> |
| 229 | [00692 JOIN_SUBQUERY_EXISTS_085](crates/bench/sqlite_parity/cases/SQLITE_PARITY_692_JOIN_SUBQUERY_EXISTS_085.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2715822 | 72759146 | <span style="color:#dc2626">-2579.08%</span> |
| 230 | [00658 JOIN_SUBQUERY_EXISTS_051](crates/bench/sqlite_parity/cases/SQLITE_PARITY_658_JOIN_SUBQUERY_EXISTS_051.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2809398 | 74920056 | <span style="color:#dc2626">-2566.77%</span> |
| 231 | [00690 JOIN_SUBQUERY_EXISTS_083](crates/bench/sqlite_parity/cases/SQLITE_PARITY_690_JOIN_SUBQUERY_EXISTS_083.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2617496 | 69763254 | <span style="color:#dc2626">-2565.27%</span> |
| 232 | [00344 SCALAR_ARITH_030](crates/bench/sqlite_parity/cases/SQLITE_PARITY_344_SCALAR_ARITH_030.rs) | P1 | memory | GEN_SQL_SCALAR | 1884637 | 49804533 | <span style="color:#dc2626">-2542.66%</span> |
| 233 | [00663 JOIN_SUBQUERY_EXISTS_056](crates/bench/sqlite_parity/cases/SQLITE_PARITY_663_JOIN_SUBQUERY_EXISTS_056.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2643415 | 69722707 | <span style="color:#dc2626">-2537.60%</span> |
| 234 | [00631 JOIN_SUBQUERY_EXISTS_024](crates/bench/sqlite_parity/cases/SQLITE_PARITY_631_JOIN_SUBQUERY_EXISTS_024.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2813897 | 74006004 | <span style="color:#dc2626">-2530.02%</span> |
| 235 | [00397 DML_WHERE_ORDER_LIMIT_010](crates/bench/sqlite_parity/cases/SQLITE_PARITY_397_DML_WHERE_ORDER_LIMIT_010.rs) | P1 | memory | GEN_SQL_DML | 2408830 | 63337994 | <span style="color:#dc2626">-2529.41%</span> |
| 236 | [00437 DML_WHERE_ORDER_LIMIT_050](crates/bench/sqlite_parity/cases/SQLITE_PARITY_437_DML_WHERE_ORDER_LIMIT_050.rs) | P1 | memory | GEN_SQL_DML | 2605352 | 68010611 | <span style="color:#dc2626">-2510.42%</span> |
| 237 | [00620 JOIN_SUBQUERY_EXISTS_013](crates/bench/sqlite_parity/cases/SQLITE_PARITY_620_JOIN_SUBQUERY_EXISTS_013.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3184369 | 82639178 | <span style="color:#dc2626">-2495.15%</span> |
| 238 | [00616 JOIN_SUBQUERY_EXISTS_009](crates/bench/sqlite_parity/cases/SQLITE_PARITY_616_JOIN_SUBQUERY_EXISTS_009.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2623246 | 67963685 | <span style="color:#dc2626">-2490.82%</span> |
| 239 | [00660 JOIN_SUBQUERY_EXISTS_053](crates/bench/sqlite_parity/cases/SQLITE_PARITY_660_JOIN_SUBQUERY_EXISTS_053.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3283086 | 84971435 | <span style="color:#dc2626">-2488.16%</span> |
| 240 | [00100 SCHEMA_SQLITE_SCHEMA](crates/bench/sqlite_parity/cases/SQLITE_PARITY_100_SCHEMA_SQLITE_SCHEMA.rs) | P0 | memory | SQL_SCHEMA | 2254357 | 58343977 | <span style="color:#dc2626">-2488.05%</span> |
| 241 | [00695 JOIN_SUBQUERY_EXISTS_088](crates/bench/sqlite_parity/cases/SQLITE_PARITY_695_JOIN_SUBQUERY_EXISTS_088.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2725219 | 70361978 | <span style="color:#dc2626">-2481.88%</span> |
| 242 | [00032 UNIQUE_INDEX_FAILURE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_032_UNIQUE_INDEX_FAILURE.rs) | P0 | memory | SQL_INDEX_NEGATIVE | 2185918 | 56413652 | <span style="color:#dc2626">-2480.78%</span> |
| 243 | [00438 DML_WHERE_ORDER_LIMIT_051](crates/bench/sqlite_parity/cases/SQLITE_PARITY_438_DML_WHERE_ORDER_LIMIT_051.rs) | P1 | memory | GEN_SQL_DML | 2410343 | 62072520 | <span style="color:#dc2626">-2475.26%</span> |
| 244 | [00489 DML_WHERE_ORDER_LIMIT_102](crates/bench/sqlite_parity/cases/SQLITE_PARITY_489_DML_WHERE_ORDER_LIMIT_102.rs) | P1 | memory | GEN_SQL_DML | 2244609 | 57712768 | <span style="color:#dc2626">-2471.17%</span> |
| 245 | [00496 DML_WHERE_ORDER_LIMIT_109](crates/bench/sqlite_parity/cases/SQLITE_PARITY_496_DML_WHERE_ORDER_LIMIT_109.rs) | P1 | memory | GEN_SQL_DML | 2513187 | 64591410 | <span style="color:#dc2626">-2470.10%</span> |
| 246 | [00037 DROP_VIEW](crates/bench/sqlite_parity/cases/SQLITE_PARITY_037_DROP_VIEW.rs) | P0 | memory | SQL_DROP | 2249058 | 57759750 | <span style="color:#dc2626">-2468.18%</span> |
| 247 | [00470 DML_WHERE_ORDER_LIMIT_083](crates/bench/sqlite_parity/cases/SQLITE_PARITY_470_DML_WHERE_ORDER_LIMIT_083.rs) | P1 | memory | GEN_SQL_DML | 2217108 | 56862378 | <span style="color:#dc2626">-2464.71%</span> |
| 248 | [00446 DML_WHERE_ORDER_LIMIT_059](crates/bench/sqlite_parity/cases/SQLITE_PARITY_446_DML_WHERE_ORDER_LIMIT_059.rs) | P1 | memory | GEN_SQL_DML | 2386248 | 61195088 | <span style="color:#dc2626">-2464.49%</span> |
| 249 | [00686 JOIN_SUBQUERY_EXISTS_079](crates/bench/sqlite_parity/cases/SQLITE_PARITY_686_JOIN_SUBQUERY_EXISTS_079.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2737372 | 70085845 | <span style="color:#dc2626">-2460.33%</span> |
| 250 | [00111 DOT_HEADERS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_111_DOT_HEADERS.rs) | P0 | memory | CLI_DOT_COMMAND | 3072647 | 78488471 | <span style="color:#dc2626">-2454.43%</span> |
| 251 | [00127 DOT_LIMIT](crates/bench/sqlite_parity/cases/SQLITE_PARITY_127_DOT_LIMIT.rs) | P0 | memory | CLI_DOT_COMMAND | 2814437 | 71890301 | <span style="color:#dc2626">-2454.34%</span> |
| 252 | [00792 WINDOW_PARTITION_SUM_005](crates/bench/sqlite_parity/cases/SQLITE_PARITY_792_WINDOW_PARTITION_SUM_005.rs) | P2 | memory | GEN_SQL_WINDOW | 1995157 | 50928788 | <span style="color:#dc2626">-2452.62%</span> |
| 253 | [00409 DML_WHERE_ORDER_LIMIT_022](crates/bench/sqlite_parity/cases/SQLITE_PARITY_409_DML_WHERE_ORDER_LIMIT_022.rs) | P1 | memory | GEN_SQL_DML | 2210114 | 56397065 | <span style="color:#dc2626">-2451.77%</span> |
| 254 | [00412 DML_WHERE_ORDER_LIMIT_025](crates/bench/sqlite_parity/cases/SQLITE_PARITY_412_DML_WHERE_ORDER_LIMIT_025.rs) | P1 | memory | GEN_SQL_DML | 2311446 | 58845911 | <span style="color:#dc2626">-2445.85%</span> |
| 255 | [00391 DML_WHERE_ORDER_LIMIT_004](crates/bench/sqlite_parity/cases/SQLITE_PARITY_391_DML_WHERE_ORDER_LIMIT_004.rs) | P1 | memory | GEN_SQL_DML | 2262023 | 57429278 | <span style="color:#dc2626">-2438.85%</span> |
| 256 | [00214 DROP_TABLE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_214_DROP_TABLE.rs) | P0 | memory | SQL_DROP | 2254337 | 57205543 | <span style="color:#dc2626">-2437.58%</span> |
| 257 | [00476 DML_WHERE_ORDER_LIMIT_089](crates/bench/sqlite_parity/cases/SQLITE_PARITY_476_DML_WHERE_ORDER_LIMIT_089.rs) | P1 | memory | GEN_SQL_DML | 2315523 | 58680800 | <span style="color:#dc2626">-2434.24%</span> |
| 258 | [00501 DML_WHERE_ORDER_LIMIT_114](crates/bench/sqlite_parity/cases/SQLITE_PARITY_501_DML_WHERE_ORDER_LIMIT_114.rs) | P1 | memory | GEN_SQL_DML | 2218930 | 56212007 | <span style="color:#dc2626">-2433.29%</span> |
| 259 | [00672 JOIN_SUBQUERY_EXISTS_065](crates/bench/sqlite_parity/cases/SQLITE_PARITY_672_JOIN_SUBQUERY_EXISTS_065.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2710471 | 68637373 | <span style="color:#dc2626">-2432.30%</span> |
| 260 | [00626 JOIN_SUBQUERY_EXISTS_019](crates/bench/sqlite_parity/cases/SQLITE_PARITY_626_JOIN_SUBQUERY_EXISTS_019.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2754164 | 69626522 | <span style="color:#dc2626">-2428.05%</span> |
| 261 | [00871 CONSTRAINT_FK_SAVEPOINT_004](crates/bench/sqlite_parity/cases/SQLITE_PARITY_871_CONSTRAINT_FK_SAVEPOINT_004.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 3588654 | 90408754 | <span style="color:#dc2626">-2419.29%</span> |
| 262 | [00703 JOIN_SUBQUERY_EXISTS_096](crates/bench/sqlite_parity/cases/SQLITE_PARITY_703_JOIN_SUBQUERY_EXISTS_096.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2943091 | 74036175 | <span style="color:#dc2626">-2415.59%</span> |
| 263 | [00688 JOIN_SUBQUERY_EXISTS_081](crates/bench/sqlite_parity/cases/SQLITE_PARITY_688_JOIN_SUBQUERY_EXISTS_081.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2734737 | 68759374 | <span style="color:#dc2626">-2414.30%</span> |
| 264 | [00844 WINDOW_PARTITION_SUM_057](crates/bench/sqlite_parity/cases/SQLITE_PARITY_844_WINDOW_PARTITION_SUM_057.rs) | P2 | memory | GEN_SQL_WINDOW | 2048899 | 51466647 | <span style="color:#dc2626">-2411.92%</span> |
| 265 | [00502 DML_WHERE_ORDER_LIMIT_115](crates/bench/sqlite_parity/cases/SQLITE_PARITY_502_DML_WHERE_ORDER_LIMIT_115.rs) | P1 | memory | GEN_SQL_DML | 2321545 | 58035449 | <span style="color:#dc2626">-2399.86%</span> |
| 266 | [00609 JOIN_SUBQUERY_EXISTS_002](crates/bench/sqlite_parity/cases/SQLITE_PARITY_609_JOIN_SUBQUERY_EXISTS_002.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2803297 | 69898287 | <span style="color:#dc2626">-2393.43%</span> |
| 267 | [00671 JOIN_SUBQUERY_EXISTS_064](crates/bench/sqlite_parity/cases/SQLITE_PARITY_671_JOIN_SUBQUERY_EXISTS_064.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2705772 | 67249114 | <span style="color:#dc2626">-2385.39%</span> |
| 268 | [00632 JOIN_SUBQUERY_EXISTS_025](crates/bench/sqlite_parity/cases/SQLITE_PARITY_632_JOIN_SUBQUERY_EXISTS_025.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2969181 | 73734791 | <span style="color:#dc2626">-2383.34%</span> |
| 269 | [00480 DML_WHERE_ORDER_LIMIT_093](crates/bench/sqlite_parity/cases/SQLITE_PARITY_480_DML_WHERE_ORDER_LIMIT_093.rs) | P1 | memory | GEN_SQL_DML | 2367982 | 58774998 | <span style="color:#dc2626">-2382.07%</span> |
| 270 | [00697 JOIN_SUBQUERY_EXISTS_090](crates/bench/sqlite_parity/cases/SQLITE_PARITY_697_JOIN_SUBQUERY_EXISTS_090.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2862590 | 70929943 | <span style="color:#dc2626">-2377.82%</span> |
| 271 | [00635 JOIN_SUBQUERY_EXISTS_028](crates/bench/sqlite_parity/cases/SQLITE_PARITY_635_JOIN_SUBQUERY_EXISTS_028.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2920419 | 72312829 | <span style="color:#dc2626">-2376.11%</span> |
| 272 | [00433 DML_WHERE_ORDER_LIMIT_046](crates/bench/sqlite_parity/cases/SQLITE_PARITY_433_DML_WHERE_ORDER_LIMIT_046.rs) | P1 | memory | GEN_SQL_DML | 2368884 | 58580268 | <span style="color:#dc2626">-2372.91%</span> |
| 273 | [00440 DML_WHERE_ORDER_LIMIT_053](crates/bench/sqlite_parity/cases/SQLITE_PARITY_440_DML_WHERE_ORDER_LIMIT_053.rs) | P1 | memory | GEN_SQL_DML | 2428717 | 60051003 | <span style="color:#dc2626">-2372.54%</span> |
| 274 | [00506 DML_WHERE_ORDER_LIMIT_119](crates/bench/sqlite_parity/cases/SQLITE_PARITY_506_DML_WHERE_ORDER_LIMIT_119.rs) | P1 | memory | GEN_SQL_DML | 2309543 | 57071234 | <span style="color:#dc2626">-2371.11%</span> |
| 275 | [00831 WINDOW_PARTITION_SUM_044](crates/bench/sqlite_parity/cases/SQLITE_PARITY_831_WINDOW_PARTITION_SUM_044.rs) | P2 | memory | GEN_SQL_WINDOW | 2017760 | 49845498 | <span style="color:#dc2626">-2370.34%</span> |
| 276 | [00652 JOIN_SUBQUERY_EXISTS_045](crates/bench/sqlite_parity/cases/SQLITE_PARITY_652_JOIN_SUBQUERY_EXISTS_045.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2787978 | 68846818 | <span style="color:#dc2626">-2369.42%</span> |
| 277 | [00018 DELETE_RETURNING](crates/bench/sqlite_parity/cases/SQLITE_PARITY_018_DELETE_RETURNING.rs) | P0 | memory | SQL_DELETE | 2056453 | 50694916 | <span style="color:#dc2626">-2365.16%</span> |
| 278 | [00450 DML_WHERE_ORDER_LIMIT_063](crates/bench/sqlite_parity/cases/SQLITE_PARITY_450_DML_WHERE_ORDER_LIMIT_063.rs) | P1 | memory | GEN_SQL_DML | 2633866 | 64849918 | <span style="color:#dc2626">-2362.16%</span> |
| 279 | [00664 JOIN_SUBQUERY_EXISTS_057](crates/bench/sqlite_parity/cases/SQLITE_PARITY_664_JOIN_SUBQUERY_EXISTS_057.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2829737 | 69532557 | <span style="color:#dc2626">-2357.21%</span> |
| 280 | [00472 DML_WHERE_ORDER_LIMIT_085](crates/bench/sqlite_parity/cases/SQLITE_PARITY_472_DML_WHERE_ORDER_LIMIT_085.rs) | P1 | memory | GEN_SQL_DML | 2412547 | 59243956 | <span style="color:#dc2626">-2355.66%</span> |
| 281 | [00701 JOIN_SUBQUERY_EXISTS_094](crates/bench/sqlite_parity/cases/SQLITE_PARITY_701_JOIN_SUBQUERY_EXISTS_094.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2890081 | 70857716 | <span style="color:#dc2626">-2351.76%</span> |
| 282 | [00845 WINDOW_PARTITION_SUM_058](crates/bench/sqlite_parity/cases/SQLITE_PARITY_845_WINDOW_PARTITION_SUM_058.rs) | P2 | memory | GEN_SQL_WINDOW | 2080629 | 50924130 | <span style="color:#dc2626">-2347.54%</span> |
| 283 | [00791 WINDOW_PARTITION_SUM_004](crates/bench/sqlite_parity/cases/SQLITE_PARITY_791_WINDOW_PARTITION_SUM_004.rs) | P2 | memory | GEN_SQL_WINDOW | 1957185 | 47848516 | <span style="color:#dc2626">-2344.76%</span> |
| 284 | [00471 DML_WHERE_ORDER_LIMIT_084](crates/bench/sqlite_parity/cases/SQLITE_PARITY_471_DML_WHERE_ORDER_LIMIT_084.rs) | P1 | memory | GEN_SQL_DML | 2461680 | 60088555 | <span style="color:#dc2626">-2340.96%</span> |
| 285 | [00031 CREATE_INDEX](crates/bench/sqlite_parity/cases/SQLITE_PARITY_031_CREATE_INDEX.rs) | P0 | memory | SQL_INDEX | 2376428 | 57994705 | <span style="color:#dc2626">-2340.41%</span> |
| 286 | [00617 JOIN_SUBQUERY_EXISTS_010](crates/bench/sqlite_parity/cases/SQLITE_PARITY_617_JOIN_SUBQUERY_EXISTS_010.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2775384 | 67706869 | <span style="color:#dc2626">-2339.55%</span> |
| 287 | [00661 JOIN_SUBQUERY_EXISTS_054](crates/bench/sqlite_parity/cases/SQLITE_PARITY_661_JOIN_SUBQUERY_EXISTS_054.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3033453 | 73990607 | <span style="color:#dc2626">-2339.15%</span> |
| 288 | [00669 JOIN_SUBQUERY_EXISTS_062](crates/bench/sqlite_parity/cases/SQLITE_PARITY_669_JOIN_SUBQUERY_EXISTS_062.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3075122 | 75003375 | <span style="color:#dc2626">-2339.04%</span> |
| 289 | [00698 JOIN_SUBQUERY_EXISTS_091](crates/bench/sqlite_parity/cases/SQLITE_PARITY_698_JOIN_SUBQUERY_EXISTS_091.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2927993 | 71352934 | <span style="color:#dc2626">-2336.92%</span> |
| 290 | [00674 JOIN_SUBQUERY_EXISTS_067](crates/bench/sqlite_parity/cases/SQLITE_PARITY_674_JOIN_SUBQUERY_EXISTS_067.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2951177 | 71532704 | <span style="color:#dc2626">-2323.87%</span> |
| 291 | [00078 ON_CONFLICT_ALGORITHMS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_078_ON_CONFLICT_ALGORITHMS.rs) | P0 | memory | SQL_CONFLICT | 2447864 | 59205467 | <span style="color:#dc2626">-2318.66%</span> |
| 292 | [00401 DML_WHERE_ORDER_LIMIT_014](crates/bench/sqlite_parity/cases/SQLITE_PARITY_401_DML_WHERE_ORDER_LIMIT_014.rs) | P1 | memory | GEN_SQL_DML | 2542794 | 61380409 | <span style="color:#dc2626">-2313.90%</span> |
| 293 | [00411 DML_WHERE_ORDER_LIMIT_024](crates/bench/sqlite_parity/cases/SQLITE_PARITY_411_DML_WHERE_ORDER_LIMIT_024.rs) | P1 | memory | GEN_SQL_DML | 2290807 | 55294257 | <span style="color:#dc2626">-2313.75%</span> |
| 294 | [00633 JOIN_SUBQUERY_EXISTS_026](crates/bench/sqlite_parity/cases/SQLITE_PARITY_633_JOIN_SUBQUERY_EXISTS_026.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2952560 | 70855139 | <span style="color:#dc2626">-2299.79%</span> |
| 295 | [00058 GROUP_BY_HAVING](crates/bench/sqlite_parity/cases/SQLITE_PARITY_058_GROUP_BY_HAVING.rs) | P0 | memory | SQL_AGGREGATE | 1967705 | 47181275 | <span style="color:#dc2626">-2297.78%</span> |
| 296 | [00649 JOIN_SUBQUERY_EXISTS_042](crates/bench/sqlite_parity/cases/SQLITE_PARITY_649_JOIN_SUBQUERY_EXISTS_042.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3004558 | 71936528 | <span style="color:#dc2626">-2294.25%</span> |
| 297 | [00623 JOIN_SUBQUERY_EXISTS_016](crates/bench/sqlite_parity/cases/SQLITE_PARITY_623_JOIN_SUBQUERY_EXISTS_016.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2938493 | 70269049 | <span style="color:#dc2626">-2291.33%</span> |
| 298 | [00625 JOIN_SUBQUERY_EXISTS_018](crates/bench/sqlite_parity/cases/SQLITE_PARITY_625_JOIN_SUBQUERY_EXISTS_018.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3718439 | 88904800 | <span style="color:#dc2626">-2290.92%</span> |
| 299 | [00028 ALTER_TABLE_RENAME](crates/bench/sqlite_parity/cases/SQLITE_PARITY_028_ALTER_TABLE_RENAME.rs) | P0 | memory | SQL_ALTER | 2541912 | 60730113 | <span style="color:#dc2626">-2289.15%</span> |
| 300 | [00404 DML_WHERE_ORDER_LIMIT_017](crates/bench/sqlite_parity/cases/SQLITE_PARITY_404_DML_WHERE_ORDER_LIMIT_017.rs) | P1 | memory | GEN_SQL_DML | 3039824 | 72446708 | <span style="color:#dc2626">-2283.25%</span> |
| 301 | [00492 DML_WHERE_ORDER_LIMIT_105](crates/bench/sqlite_parity/cases/SQLITE_PARITY_492_DML_WHERE_ORDER_LIMIT_105.rs) | P1 | memory | GEN_SQL_DML | 2632474 | 62686313 | <span style="color:#dc2626">-2281.27%</span> |
| 302 | [00473 DML_WHERE_ORDER_LIMIT_086](crates/bench/sqlite_parity/cases/SQLITE_PARITY_473_DML_WHERE_ORDER_LIMIT_086.rs) | P1 | memory | GEN_SQL_DML | 2488340 | 59249738 | <span style="color:#dc2626">-2281.09%</span> |
| 303 | [00494 DML_WHERE_ORDER_LIMIT_107](crates/bench/sqlite_parity/cases/SQLITE_PARITY_494_DML_WHERE_ORDER_LIMIT_107.rs) | P1 | memory | GEN_SQL_DML | 2625871 | 62523204 | <span style="color:#dc2626">-2281.05%</span> |
| 304 | [00696 JOIN_SUBQUERY_EXISTS_089](crates/bench/sqlite_parity/cases/SQLITE_PARITY_696_JOIN_SUBQUERY_EXISTS_089.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2946338 | 70142582 | <span style="color:#dc2626">-2280.67%</span> |
| 305 | [00417 DML_WHERE_ORDER_LIMIT_030](crates/bench/sqlite_parity/cases/SQLITE_PARITY_417_DML_WHERE_ORDER_LIMIT_030.rs) | P1 | memory | GEN_SQL_DML | 2404022 | 57174607 | <span style="color:#dc2626">-2278.29%</span> |
| 306 | [00402 DML_WHERE_ORDER_LIMIT_015](crates/bench/sqlite_parity/cases/SQLITE_PARITY_402_DML_WHERE_ORDER_LIMIT_015.rs) | P1 | memory | GEN_SQL_DML | 2641050 | 62694967 | <span style="color:#dc2626">-2273.87%</span> |
| 307 | [00854 WINDOW_PARTITION_SUM_067](crates/bench/sqlite_parity/cases/SQLITE_PARITY_854_WINDOW_PARTITION_SUM_067.rs) | P2 | memory | GEN_SQL_WINDOW | 2084536 | 49397189 | <span style="color:#dc2626">-2269.70%</span> |
| 308 | [00493 DML_WHERE_ORDER_LIMIT_106](crates/bench/sqlite_parity/cases/SQLITE_PARITY_493_DML_WHERE_ORDER_LIMIT_106.rs) | P1 | memory | GEN_SQL_DML | 2691475 | 63307088 | <span style="color:#dc2626">-2252.13%</span> |
| 309 | [00685 JOIN_SUBQUERY_EXISTS_078](crates/bench/sqlite_parity/cases/SQLITE_PARITY_685_JOIN_SUBQUERY_EXISTS_078.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3031329 | 71161883 | <span style="color:#dc2626">-2247.55%</span> |
| 310 | [00504 DML_WHERE_ORDER_LIMIT_117](crates/bench/sqlite_parity/cases/SQLITE_PARITY_504_DML_WHERE_ORDER_LIMIT_117.rs) | P1 | memory | GEN_SQL_DML | 2411275 | 56603017 | <span style="color:#dc2626">-2247.43%</span> |
| 311 | [00474 DML_WHERE_ORDER_LIMIT_087](crates/bench/sqlite_parity/cases/SQLITE_PARITY_474_DML_WHERE_ORDER_LIMIT_087.rs) | P1 | memory | GEN_SQL_DML | 2633526 | 61581672 | <span style="color:#dc2626">-2238.37%</span> |
| 312 | [00394 DML_WHERE_ORDER_LIMIT_007](crates/bench/sqlite_parity/cases/SQLITE_PARITY_394_DML_WHERE_ORDER_LIMIT_007.rs) | P1 | memory | GEN_SQL_DML | 3196642 | 74396369 | <span style="color:#dc2626">-2227.33%</span> |
| 313 | [00469 DML_WHERE_ORDER_LIMIT_082](crates/bench/sqlite_parity/cases/SQLITE_PARITY_469_DML_WHERE_ORDER_LIMIT_082.rs) | P1 | memory | GEN_SQL_DML | 2639296 | 61258961 | <span style="color:#dc2626">-2221.03%</span> |
| 314 | [00634 JOIN_SUBQUERY_EXISTS_027](crates/bench/sqlite_parity/cases/SQLITE_PARITY_634_JOIN_SUBQUERY_EXISTS_027.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3093597 | 71582606 | <span style="color:#dc2626">-2213.90%</span> |
| 315 | [00850 WINDOW_PARTITION_SUM_063](crates/bench/sqlite_parity/cases/SQLITE_PARITY_850_WINDOW_PARTITION_SUM_063.rs) | P2 | memory | GEN_SQL_WINDOW | 2225183 | 51384823 | <span style="color:#dc2626">-2209.24%</span> |
| 316 | [00413 DML_WHERE_ORDER_LIMIT_026](crates/bench/sqlite_parity/cases/SQLITE_PARITY_413_DML_WHERE_ORDER_LIMIT_026.rs) | P1 | memory | GEN_SQL_DML | 2420762 | 55705596 | <span style="color:#dc2626">-2201.16%</span> |
| 317 | [00798 WINDOW_PARTITION_SUM_011](crates/bench/sqlite_parity/cases/SQLITE_PARITY_798_WINDOW_PARTITION_SUM_011.rs) | P2 | memory | GEN_SQL_WINDOW | 1974177 | 45418296 | <span style="color:#dc2626">-2200.62%</span> |
| 318 | [00075 EXPLAIN_QUERY_PLAN](crates/bench/sqlite_parity/cases/SQLITE_PARITY_075_EXPLAIN_QUERY_PLAN.rs) | P0 | memory | SQL_EXPLAIN | 2991914 | 68703067 | <span style="color:#dc2626">-2196.29%</span> |
| 319 | [00442 DML_WHERE_ORDER_LIMIT_055](crates/bench/sqlite_parity/cases/SQLITE_PARITY_442_DML_WHERE_ORDER_LIMIT_055.rs) | P1 | memory | GEN_SQL_DML | 2807034 | 64419723 | <span style="color:#dc2626">-2194.94%</span> |
| 320 | [00707 JOIN_SUBQUERY_EXISTS_100](crates/bench/sqlite_parity/cases/SQLITE_PARITY_707_JOIN_SUBQUERY_EXISTS_100.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3243401 | 74369796 | <span style="color:#dc2626">-2192.96%</span> |
| 321 | [00015 UPDATE_BASIC](crates/bench/sqlite_parity/cases/SQLITE_PARITY_015_UPDATE_BASIC.rs) | P0 | memory | SQL_UPDATE | 2190066 | 50174100 | <span style="color:#dc2626">-2190.99%</span> |
| 322 | [00399 DML_WHERE_ORDER_LIMIT_012](crates/bench/sqlite_parity/cases/SQLITE_PARITY_399_DML_WHERE_ORDER_LIMIT_012.rs) | P1 | memory | GEN_SQL_DML | 2688118 | 61483664 | <span style="color:#dc2626">-2187.24%</span> |
| 323 | [00050 PRAGMA_TABLE_INFO_FUNCTION](crates/bench/sqlite_parity/cases/SQLITE_PARITY_050_PRAGMA_TABLE_INFO_FUNCTION.rs) | P0 | memory | SQL_PRAGMA | 1933681 | 43985616 | <span style="color:#dc2626">-2174.71%</span> |
| 324 | [00445 DML_WHERE_ORDER_LIMIT_058](crates/bench/sqlite_parity/cases/SQLITE_PARITY_445_DML_WHERE_ORDER_LIMIT_058.rs) | P1 | memory | GEN_SQL_DML | 2944895 | 66792575 | <span style="color:#dc2626">-2168.08%</span> |
| 325 | [00447 DML_WHERE_ORDER_LIMIT_060](crates/bench/sqlite_parity/cases/SQLITE_PARITY_447_DML_WHERE_ORDER_LIMIT_060.rs) | P1 | memory | GEN_SQL_DML | 2673832 | 60621613 | <span style="color:#dc2626">-2167.22%</span> |
| 326 | [00618 JOIN_SUBQUERY_EXISTS_011](crates/bench/sqlite_parity/cases/SQLITE_PARITY_618_JOIN_SUBQUERY_EXISTS_011.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3019296 | 68390423 | <span style="color:#dc2626">-2165.11%</span> |
| 327 | [00694 JOIN_SUBQUERY_EXISTS_087](crates/bench/sqlite_parity/cases/SQLITE_PARITY_694_JOIN_SUBQUERY_EXISTS_087.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3109296 | 70368540 | <span style="color:#dc2626">-2163.17%</span> |
| 328 | [00023 TRANSACTION_ROLLBACK](crates/bench/sqlite_parity/cases/SQLITE_PARITY_023_TRANSACTION_ROLLBACK.rs) | P0 | memory | SQL_TRANSACTION | 1743019 | 39345913 | <span style="color:#dc2626">-2157.34%</span> |
| 329 | [00705 JOIN_SUBQUERY_EXISTS_098](crates/bench/sqlite_parity/cases/SQLITE_PARITY_705_JOIN_SUBQUERY_EXISTS_098.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3158690 | 71110756 | <span style="color:#dc2626">-2151.27%</span> |
| 330 | [00677 JOIN_SUBQUERY_EXISTS_070](crates/bench/sqlite_parity/cases/SQLITE_PARITY_677_JOIN_SUBQUERY_EXISTS_070.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3059562 | 68680615 | <span style="color:#dc2626">-2144.79%</span> |
| 331 | [00491 DML_WHERE_ORDER_LIMIT_104](crates/bench/sqlite_parity/cases/SQLITE_PARITY_491_DML_WHERE_ORDER_LIMIT_104.rs) | P1 | memory | GEN_SQL_DML | 2844255 | 63775385 | <span style="color:#dc2626">-2142.25%</span> |
| 332 | [00398 DML_WHERE_ORDER_LIMIT_011](crates/bench/sqlite_parity/cases/SQLITE_PARITY_398_DML_WHERE_ORDER_LIMIT_011.rs) | P1 | memory | GEN_SQL_DML | 2864312 | 64138410 | <span style="color:#dc2626">-2139.23%</span> |
| 333 | [00505 DML_WHERE_ORDER_LIMIT_118](crates/bench/sqlite_parity/cases/SQLITE_PARITY_505_DML_WHERE_ORDER_LIMIT_118.rs) | P1 | memory | GEN_SQL_DML | 2572420 | 57470650 | <span style="color:#dc2626">-2134.11%</span> |
| 334 | [00036 CREATE_VIEW](crates/bench/sqlite_parity/cases/SQLITE_PARITY_036_CREATE_VIEW.rs) | P0 | memory | SQL_VIEW | 2983197 | 66424723 | <span style="color:#dc2626">-2126.63%</span> |
| 335 | [00410 DML_WHERE_ORDER_LIMIT_023](crates/bench/sqlite_parity/cases/SQLITE_PARITY_410_DML_WHERE_ORDER_LIMIT_023.rs) | P1 | memory | GEN_SQL_DML | 2508448 | 55745281 | <span style="color:#dc2626">-2122.30%</span> |
| 336 | [00684 JOIN_SUBQUERY_EXISTS_077](crates/bench/sqlite_parity/cases/SQLITE_PARITY_684_JOIN_SUBQUERY_EXISTS_077.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3106792 | 68740368 | <span style="color:#dc2626">-2112.58%</span> |
| 337 | [00218 PRAGMA_FORMS_SCHEMA_EQUALS_PARENS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_218_PRAGMA_FORMS_SCHEMA_EQUALS_PARENS.rs) | P0 | memory | SQL_PRAGMA | 1709546 | 37641266 | <span style="color:#dc2626">-2101.83%</span> |
| 338 | [00466 DML_WHERE_ORDER_LIMIT_079](crates/bench/sqlite_parity/cases/SQLITE_PARITY_466_DML_WHERE_ORDER_LIMIT_079.rs) | P1 | memory | GEN_SQL_DML | 2795583 | 61463288 | <span style="color:#dc2626">-2098.59%</span> |
| 339 | [00651 JOIN_SUBQUERY_EXISTS_044](crates/bench/sqlite_parity/cases/SQLITE_PARITY_651_JOIN_SUBQUERY_EXISTS_044.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3204727 | 70289620 | <span style="color:#dc2626">-2093.31%</span> |
| 340 | [00507 DML_WHERE_ORDER_LIMIT_120](crates/bench/sqlite_parity/cases/SQLITE_PARITY_507_DML_WHERE_ORDER_LIMIT_120.rs) | P1 | memory | GEN_SQL_DML | 2634367 | 57761260 | <span style="color:#dc2626">-2092.60%</span> |
| 341 | [00428 DML_WHERE_ORDER_LIMIT_041](crates/bench/sqlite_parity/cases/SQLITE_PARITY_428_DML_WHERE_ORDER_LIMIT_041.rs) | P1 | memory | GEN_SQL_DML | 2715341 | 59456056 | <span style="color:#dc2626">-2089.63%</span> |
| 342 | [00017 DELETE_BASIC](crates/bench/sqlite_parity/cases/SQLITE_PARITY_017_DELETE_BASIC.rs) | P0 | memory | SQL_DELETE | 2364977 | 51775341 | <span style="color:#dc2626">-2089.25%</span> |
| 343 | [00056 SUBQUERIES_EXISTS_IN](crates/bench/sqlite_parity/cases/SQLITE_PARITY_056_SUBQUERIES_EXISTS_IN.rs) | P0 | memory | SQL_SELECT | 2346632 | 51217987 | <span style="color:#dc2626">-2082.62%</span> |
| 344 | [00407 DML_WHERE_ORDER_LIMIT_020](crates/bench/sqlite_parity/cases/SQLITE_PARITY_407_DML_WHERE_ORDER_LIMIT_020.rs) | P1 | memory | GEN_SQL_DML | 2635950 | 57348946 | <span style="color:#dc2626">-2075.65%</span> |
| 345 | [00423 DML_WHERE_ORDER_LIMIT_036](crates/bench/sqlite_parity/cases/SQLITE_PARITY_423_DML_WHERE_ORDER_LIMIT_036.rs) | P1 | memory | GEN_SQL_DML | 2732473 | 59385172 | <span style="color:#dc2626">-2073.31%</span> |
| 346 | [00014 INSERT_RETURNING](crates/bench/sqlite_parity/cases/SQLITE_PARITY_014_INSERT_RETURNING.rs) | P0 | memory | SQL_INSERT | 1975139 | 42920278 | <span style="color:#dc2626">-2073.03%</span> |
| 347 | [00020 UPSERT_DO_UPDATE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_020_UPSERT_DO_UPDATE.rs) | P0 | memory | SQL_UPSERT | 2373013 | 51487286 | <span style="color:#dc2626">-2069.70%</span> |
| 348 | [00612 JOIN_SUBQUERY_EXISTS_005](crates/bench/sqlite_parity/cases/SQLITE_PARITY_612_JOIN_SUBQUERY_EXISTS_005.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3059241 | 66136616 | <span style="color:#dc2626">-2061.86%</span> |
| 349 | [00817 WINDOW_PARTITION_SUM_030](crates/bench/sqlite_parity/cases/SQLITE_PARITY_817_WINDOW_PARTITION_SUM_030.rs) | P2 | memory | GEN_SQL_WINDOW | 2241383 | 48412555 | <span style="color:#dc2626">-2059.94%</span> |
| 350 | [00022 TRANSACTION_COMMIT](crates/bench/sqlite_parity/cases/SQLITE_PARITY_022_TRANSACTION_COMMIT.rs) | P0 | memory | SQL_TRANSACTION | 1779669 | 38352953 | <span style="color:#dc2626">-2055.06%</span> |
| 351 | [00481 DML_WHERE_ORDER_LIMIT_094](crates/bench/sqlite_parity/cases/SQLITE_PARITY_481_DML_WHERE_ORDER_LIMIT_094.rs) | P1 | memory | GEN_SQL_DML | 2737443 | 58980919 | <span style="color:#dc2626">-2054.60%</span> |
| 352 | [00655 JOIN_SUBQUERY_EXISTS_048](crates/bench/sqlite_parity/cases/SQLITE_PARITY_655_JOIN_SUBQUERY_EXISTS_048.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3243751 | 69712848 | <span style="color:#dc2626">-2049.14%</span> |
| 353 | [00395 DML_WHERE_ORDER_LIMIT_008](crates/bench/sqlite_parity/cases/SQLITE_PARITY_395_DML_WHERE_ORDER_LIMIT_008.rs) | P1 | memory | GEN_SQL_DML | 2970053 | 63662508 | <span style="color:#dc2626">-2043.48%</span> |
| 354 | [00679 JOIN_SUBQUERY_EXISTS_072](crates/bench/sqlite_parity/cases/SQLITE_PARITY_679_JOIN_SUBQUERY_EXISTS_072.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3519463 | 75376641 | <span style="color:#dc2626">-2041.71%</span> |
| 355 | [00396 DML_WHERE_ORDER_LIMIT_009](crates/bench/sqlite_parity/cases/SQLITE_PARITY_396_DML_WHERE_ORDER_LIMIT_009.rs) | P1 | memory | GEN_SQL_DML | 3103455 | 66300612 | <span style="color:#dc2626">-2036.35%</span> |
| 356 | [00503 DML_WHERE_ORDER_LIMIT_116](crates/bench/sqlite_parity/cases/SQLITE_PARITY_503_DML_WHERE_ORDER_LIMIT_116.rs) | P1 | memory | GEN_SQL_DML | 2625641 | 55993042 | <span style="color:#dc2626">-2032.55%</span> |
| 357 | [00392 DML_WHERE_ORDER_LIMIT_005](crates/bench/sqlite_parity/cases/SQLITE_PARITY_392_DML_WHERE_ORDER_LIMIT_005.rs) | P1 | memory | GEN_SQL_DML | 2690955 | 57318428 | <span style="color:#dc2626">-2030.04%</span> |
| 358 | [00624 JOIN_SUBQUERY_EXISTS_017](crates/bench/sqlite_parity/cases/SQLITE_PARITY_624_JOIN_SUBQUERY_EXISTS_017.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3327209 | 70789695 | <span style="color:#dc2626">-2027.60%</span> |
| 359 | [00400 DML_WHERE_ORDER_LIMIT_013](crates/bench/sqlite_parity/cases/SQLITE_PARITY_400_DML_WHERE_ORDER_LIMIT_013.rs) | P1 | memory | GEN_SQL_DML | 2975553 | 63301766 | <span style="color:#dc2626">-2027.40%</span> |
| 360 | [00848 WINDOW_PARTITION_SUM_061](crates/bench/sqlite_parity/cases/SQLITE_PARITY_848_WINDOW_PARTITION_SUM_061.rs) | P2 | memory | GEN_SQL_WINDOW | 2289114 | 48675002 | <span style="color:#dc2626">-2026.37%</span> |
| 361 | [00799 WINDOW_PARTITION_SUM_012](crates/bench/sqlite_parity/cases/SQLITE_PARITY_799_WINDOW_PARTITION_SUM_012.rs) | P2 | memory | GEN_SQL_WINDOW | 2129712 | 45276567 | <span style="color:#dc2626">-2025.95%</span> |
| 362 | [00424 DML_WHERE_ORDER_LIMIT_037](crates/bench/sqlite_parity/cases/SQLITE_PARITY_424_DML_WHERE_ORDER_LIMIT_037.rs) | P1 | memory | GEN_SQL_DML | 2661959 | 56399740 | <span style="color:#dc2626">-2018.73%</span> |
| 363 | [00849 WINDOW_PARTITION_SUM_062](crates/bench/sqlite_parity/cases/SQLITE_PARITY_849_WINDOW_PARTITION_SUM_062.rs) | P2 | memory | GEN_SQL_WINDOW | 2298451 | 48686834 | <span style="color:#dc2626">-2018.25%</span> |
| 364 | [00416 DML_WHERE_ORDER_LIMIT_029](crates/bench/sqlite_parity/cases/SQLITE_PARITY_416_DML_WHERE_ORDER_LIMIT_029.rs) | P1 | memory | GEN_SQL_DML | 2651560 | 56163443 | <span style="color:#dc2626">-2018.13%</span> |
| 365 | [00039 CREATE_TRIGGER_BEFORE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_039_CREATE_TRIGGER_BEFORE.rs) | P0 | memory | SQL_TRIGGER | 3113654 | 65782027 | <span style="color:#dc2626">-2012.70%</span> |
| 366 | [00621 JOIN_SUBQUERY_EXISTS_014](crates/bench/sqlite_parity/cases/SQLITE_PARITY_621_JOIN_SUBQUERY_EXISTS_014.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3417029 | 72180829 | <span style="color:#dc2626">-2012.39%</span> |
| 367 | [00691 JOIN_SUBQUERY_EXISTS_084](crates/bench/sqlite_parity/cases/SQLITE_PARITY_691_JOIN_SUBQUERY_EXISTS_084.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3299657 | 69624662 | <span style="color:#dc2626">-2010.06%</span> |
| 368 | [00488 DML_WHERE_ORDER_LIMIT_101](crates/bench/sqlite_parity/cases/SQLITE_PARITY_488_DML_WHERE_ORDER_LIMIT_101.rs) | P1 | memory | GEN_SQL_DML | 2753463 | 58011343 | <span style="color:#dc2626">-2006.85%</span> |
| 369 | [00443 DML_WHERE_ORDER_LIMIT_056](crates/bench/sqlite_parity/cases/SQLITE_PARITY_443_DML_WHERE_ORDER_LIMIT_056.rs) | P1 | memory | GEN_SQL_DML | 2918595 | 61480709 | <span style="color:#dc2626">-2006.52%</span> |
| 370 | [00477 DML_WHERE_ORDER_LIMIT_090](crates/bench/sqlite_parity/cases/SQLITE_PARITY_477_DML_WHERE_ORDER_LIMIT_090.rs) | P1 | memory | GEN_SQL_DML | 2765826 | 58185493 | <span style="color:#dc2626">-2003.73%</span> |
| 371 | [00485 DML_WHERE_ORDER_LIMIT_098](crates/bench/sqlite_parity/cases/SQLITE_PARITY_485_DML_WHERE_ORDER_LIMIT_098.rs) | P1 | memory | GEN_SQL_DML | 2712105 | 57006571 | <span style="color:#dc2626">-2001.93%</span> |
| 372 | [00113 DOT_NULLVALUE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_113_DOT_NULLVALUE.rs) | P0 | memory | CLI_DOT_COMMAND | 2964321 | 62057436 | <span style="color:#dc2626">-1993.48%</span> |
| 373 | [00832 WINDOW_PARTITION_SUM_045](crates/bench/sqlite_parity/cases/SQLITE_PARITY_832_WINDOW_PARTITION_SUM_045.rs) | P2 | memory | GEN_SQL_WINDOW | 2278954 | 47617851 | <span style="color:#dc2626">-1989.46%</span> |
| 374 | [00426 DML_WHERE_ORDER_LIMIT_039](crates/bench/sqlite_parity/cases/SQLITE_PARITY_426_DML_WHERE_ORDER_LIMIT_039.rs) | P1 | memory | GEN_SQL_DML | 2772769 | 57935377 | <span style="color:#dc2626">-1989.44%</span> |
| 375 | [00435 DML_WHERE_ORDER_LIMIT_048](crates/bench/sqlite_parity/cases/SQLITE_PARITY_435_DML_WHERE_ORDER_LIMIT_048.rs) | P1 | memory | GEN_SQL_DML | 2803768 | 58443088 | <span style="color:#dc2626">-1984.45%</span> |
| 376 | [00833 WINDOW_PARTITION_SUM_046](crates/bench/sqlite_parity/cases/SQLITE_PARITY_833_WINDOW_PARTITION_SUM_046.rs) | P2 | memory | GEN_SQL_WINDOW | 2274336 | 47357268 | <span style="color:#dc2626">-1982.25%</span> |
| 377 | [00486 DML_WHERE_ORDER_LIMIT_099](crates/bench/sqlite_parity/cases/SQLITE_PARITY_486_DML_WHERE_ORDER_LIMIT_099.rs) | P1 | memory | GEN_SQL_DML | 2765496 | 57502700 | <span style="color:#dc2626">-1979.29%</span> |
| 378 | [00448 DML_WHERE_ORDER_LIMIT_061](crates/bench/sqlite_parity/cases/SQLITE_PARITY_448_DML_WHERE_ORDER_LIMIT_061.rs) | P1 | memory | GEN_SQL_DML | 2862419 | 59376095 | <span style="color:#dc2626">-1974.33%</span> |
| 379 | [00802 WINDOW_PARTITION_SUM_015](crates/bench/sqlite_parity/cases/SQLITE_PARITY_802_WINDOW_PARTITION_SUM_015.rs) | P2 | memory | GEN_SQL_WINDOW | 2514290 | 52123179 | <span style="color:#dc2626">-1973.08%</span> |
| 380 | [00408 DML_WHERE_ORDER_LIMIT_021](crates/bench/sqlite_parity/cases/SQLITE_PARITY_408_DML_WHERE_ORDER_LIMIT_021.rs) | P1 | memory | GEN_SQL_DML | 2710912 | 56154626 | <span style="color:#dc2626">-1971.43%</span> |
| 381 | [00796 WINDOW_PARTITION_SUM_009](crates/bench/sqlite_parity/cases/SQLITE_PARITY_796_WINDOW_PARTITION_SUM_009.rs) | P2 | memory | GEN_SQL_WINDOW | 2356432 | 48745415 | <span style="color:#dc2626">-1968.61%</span> |
| 382 | [00838 WINDOW_PARTITION_SUM_051](crates/bench/sqlite_parity/cases/SQLITE_PARITY_838_WINDOW_PARTITION_SUM_051.rs) | P2 | memory | GEN_SQL_WINDOW | 2399072 | 49609001 | <span style="color:#dc2626">-1967.84%</span> |
| 383 | [00495 DML_WHERE_ORDER_LIMIT_108](crates/bench/sqlite_parity/cases/SQLITE_PARITY_495_DML_WHERE_ORDER_LIMIT_108.rs) | P1 | memory | GEN_SQL_DML | 3063410 | 63208542 | <span style="color:#dc2626">-1963.34%</span> |
| 384 | [00790 WINDOW_PARTITION_SUM_003](crates/bench/sqlite_parity/cases/SQLITE_PARITY_790_WINDOW_PARTITION_SUM_003.rs) | P2 | memory | GEN_SQL_WINDOW | 2329580 | 48056790 | <span style="color:#dc2626">-1962.90%</span> |
| 385 | [00857 WINDOW_PARTITION_SUM_070](crates/bench/sqlite_parity/cases/SQLITE_PARITY_857_WINDOW_PARTITION_SUM_070.rs) | P2 | memory | GEN_SQL_WINDOW | 2421304 | 49888429 | <span style="color:#dc2626">-1960.40%</span> |
| 386 | [00029 ALTER_TABLE_RENAME_COLUMN](crates/bench/sqlite_parity/cases/SQLITE_PARITY_029_ALTER_TABLE_RENAME_COLUMN.rs) | P0 | memory | SQL_ALTER | 2870134 | 59102112 | <span style="color:#dc2626">-1959.21%</span> |
| 387 | [00427 DML_WHERE_ORDER_LIMIT_040](crates/bench/sqlite_parity/cases/SQLITE_PARITY_427_DML_WHERE_ORDER_LIMIT_040.rs) | P1 | memory | GEN_SQL_DML | 2839425 | 58447647 | <span style="color:#dc2626">-1958.43%</span> |
| 388 | [00406 DML_WHERE_ORDER_LIMIT_019](crates/bench/sqlite_parity/cases/SQLITE_PARITY_406_DML_WHERE_ORDER_LIMIT_019.rs) | P1 | memory | GEN_SQL_DML | 2820630 | 57961316 | <span style="color:#dc2626">-1954.91%</span> |
| 389 | [00390 DML_WHERE_ORDER_LIMIT_003](crates/bench/sqlite_parity/cases/SQLITE_PARITY_390_DML_WHERE_ORDER_LIMIT_003.rs) | P1 | memory | GEN_SQL_DML | 2691225 | 55277695 | <span style="color:#dc2626">-1954.00%</span> |
| 390 | [00622 JOIN_SUBQUERY_EXISTS_015](crates/bench/sqlite_parity/cases/SQLITE_PARITY_622_JOIN_SUBQUERY_EXISTS_015.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3246567 | 66611946 | <span style="color:#dc2626">-1951.77%</span> |
| 391 | [00405 DML_WHERE_ORDER_LIMIT_018](crates/bench/sqlite_parity/cases/SQLITE_PARITY_405_DML_WHERE_ORDER_LIMIT_018.rs) | P1 | memory | GEN_SQL_DML | 2836540 | 58125857 | <span style="color:#dc2626">-1949.18%</span> |
| 392 | [00012 INSERT_DEFAULT_VALUES](crates/bench/sqlite_parity/cases/SQLITE_PARITY_012_INSERT_DEFAULT_VALUES.rs) | P0 | memory | SQL_INSERT | 2198642 | 44930774 | <span style="color:#dc2626">-1943.57%</span> |
| 393 | [00797 WINDOW_PARTITION_SUM_010](crates/bench/sqlite_parity/cases/SQLITE_PARITY_797_WINDOW_PARTITION_SUM_010.rs) | P2 | memory | GEN_SQL_WINDOW | 2309162 | 47141357 | <span style="color:#dc2626">-1941.49%</span> |
| 394 | [00788 WINDOW_PARTITION_SUM_001](crates/bench/sqlite_parity/cases/SQLITE_PARITY_788_WINDOW_PARTITION_SUM_001.rs) | P2 | memory | GEN_SQL_WINDOW | 2389935 | 48765172 | <span style="color:#dc2626">-1940.44%</span> |
| 395 | [00393 DML_WHERE_ORDER_LIMIT_006](crates/bench/sqlite_parity/cases/SQLITE_PARITY_393_DML_WHERE_ORDER_LIMIT_006.rs) | P1 | memory | GEN_SQL_DML | 2772699 | 56551807 | <span style="color:#dc2626">-1939.59%</span> |
| 396 | [00841 WINDOW_PARTITION_SUM_054](crates/bench/sqlite_parity/cases/SQLITE_PARITY_841_WINDOW_PARTITION_SUM_054.rs) | P2 | memory | GEN_SQL_WINDOW | 2306777 | 47031119 | <span style="color:#dc2626">-1938.82%</span> |
| 397 | [00815 WINDOW_PARTITION_SUM_028](crates/bench/sqlite_parity/cases/SQLITE_PARITY_815_WINDOW_PARTITION_SUM_028.rs) | P2 | memory | GEN_SQL_WINDOW | 2226826 | 45350759 | <span style="color:#dc2626">-1936.57%</span> |
| 398 | [00425 DML_WHERE_ORDER_LIMIT_038](crates/bench/sqlite_parity/cases/SQLITE_PARITY_425_DML_WHERE_ORDER_LIMIT_038.rs) | P1 | memory | GEN_SQL_DML | 2900191 | 58886087 | <span style="color:#dc2626">-1930.42%</span> |
| 399 | [00801 WINDOW_PARTITION_SUM_014](crates/bench/sqlite_parity/cases/SQLITE_PARITY_801_WINDOW_PARTITION_SUM_014.rs) | P2 | memory | GEN_SQL_WINDOW | 2594291 | 52101688 | <span style="color:#dc2626">-1908.32%</span> |
| 400 | [00615 JOIN_SUBQUERY_EXISTS_008](crates/bench/sqlite_parity/cases/SQLITE_PARITY_615_JOIN_SUBQUERY_EXISTS_008.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3319064 | 66656800 | <span style="color:#dc2626">-1908.30%</span> |
| 401 | [00487 DML_WHERE_ORDER_LIMIT_100](crates/bench/sqlite_parity/cases/SQLITE_PARITY_487_DML_WHERE_ORDER_LIMIT_100.rs) | P1 | memory | GEN_SQL_DML | 2893418 | 58106763 | <span style="color:#dc2626">-1908.24%</span> |
| 402 | [00314 SCALAR_STRING_022](crates/bench/sqlite_parity/cases/SQLITE_PARITY_314_SCALAR_STRING_022.rs) | P1 | memory | GEN_SQL_SCALAR | 1787153 | 35826783 | <span style="color:#dc2626">-1904.68%</span> |
| 403 | [00828 WINDOW_PARTITION_SUM_041](crates/bench/sqlite_parity/cases/SQLITE_PARITY_828_WINDOW_PARTITION_SUM_041.rs) | P2 | memory | GEN_SQL_WINDOW | 2367082 | 47338763 | <span style="color:#dc2626">-1899.88%</span> |
| 404 | [00430 DML_WHERE_ORDER_LIMIT_043](crates/bench/sqlite_parity/cases/SQLITE_PARITY_430_DML_WHERE_ORDER_LIMIT_043.rs) | P1 | memory | GEN_SQL_DML | 2925318 | 58479167 | <span style="color:#dc2626">-1899.07%</span> |
| 405 | [00803 WINDOW_PARTITION_SUM_016](crates/bench/sqlite_parity/cases/SQLITE_PARITY_803_WINDOW_PARTITION_SUM_016.rs) | P2 | memory | GEN_SQL_WINDOW | 2467412 | 49040062 | <span style="color:#dc2626">-1887.51%</span> |
| 406 | [00389 DML_WHERE_ORDER_LIMIT_002](crates/bench/sqlite_parity/cases/SQLITE_PARITY_389_DML_WHERE_ORDER_LIMIT_002.rs) | P1 | memory | GEN_SQL_DML | 2939605 | 58321858 | <span style="color:#dc2626">-1884.00%</span> |
| 407 | [00420 DML_WHERE_ORDER_LIMIT_033](crates/bench/sqlite_parity/cases/SQLITE_PARITY_420_DML_WHERE_ORDER_LIMIT_033.rs) | P1 | memory | GEN_SQL_DML | 2877918 | 57076260 | <span style="color:#dc2626">-1883.25%</span> |
| 408 | [00419 DML_WHERE_ORDER_LIMIT_032](crates/bench/sqlite_parity/cases/SQLITE_PARITY_419_DML_WHERE_ORDER_LIMIT_032.rs) | P1 | memory | GEN_SQL_DML | 2824206 | 56009792 | <span style="color:#dc2626">-1883.20%</span> |
| 409 | [00667 JOIN_SUBQUERY_EXISTS_060](crates/bench/sqlite_parity/cases/SQLITE_PARITY_667_JOIN_SUBQUERY_EXISTS_060.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3472644 | 68717314 | <span style="color:#dc2626">-1878.82%</span> |
| 410 | [00693 JOIN_SUBQUERY_EXISTS_086](crates/bench/sqlite_parity/cases/SQLITE_PARITY_693_JOIN_SUBQUERY_EXISTS_086.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3613651 | 71485745 | <span style="color:#dc2626">-1878.21%</span> |
| 411 | [00010 GENERATED_COLUMNS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_010_GENERATED_COLUMNS.rs) | P0 | memory | SQL_DDL | 2215503 | 43675348 | <span style="color:#dc2626">-1871.35%</span> |
| 412 | [00820 WINDOW_PARTITION_SUM_033](crates/bench/sqlite_parity/cases/SQLITE_PARITY_820_WINDOW_PARTITION_SUM_033.rs) | P2 | memory | GEN_SQL_WINDOW | 2189155 | 43103163 | <span style="color:#dc2626">-1868.94%</span> |
| 413 | [00181 OPT_TABLE_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_181_OPT_TABLE_MODE.rs) | P2 | memory | CLI_OPTION | 1698085 | 33342779 | <span style="color:#dc2626">-1863.55%</span> |
| 414 | [00403 DML_WHERE_ORDER_LIMIT_016](crates/bench/sqlite_parity/cases/SQLITE_PARITY_403_DML_WHERE_ORDER_LIMIT_016.rs) | P1 | memory | GEN_SQL_DML | 3210808 | 63030543 | <span style="color:#dc2626">-1863.07%</span> |
| 415 | [00034 EXPRESSION_INDEX](crates/bench/sqlite_parity/cases/SQLITE_PARITY_034_EXPRESSION_INDEX.rs) | P0 | memory | SQL_INDEX | 2887005 | 56667323 | <span style="color:#dc2626">-1862.84%</span> |
| 416 | [00822 WINDOW_PARTITION_SUM_035](crates/bench/sqlite_parity/cases/SQLITE_PARITY_822_WINDOW_PARTITION_SUM_035.rs) | P2 | memory | GEN_SQL_WINDOW | 2369977 | 46496808 | <span style="color:#dc2626">-1861.91%</span> |
| 417 | [00016 UPDATE_RETURNING](crates/bench/sqlite_parity/cases/SQLITE_PARITY_016_UPDATE_RETURNING.rs) | P0 | memory | SQL_UPDATE | 3335845 | 65113140 | <span style="color:#dc2626">-1851.92%</span> |
| 418 | [00421 DML_WHERE_ORDER_LIMIT_034](crates/bench/sqlite_parity/cases/SQLITE_PARITY_421_DML_WHERE_ORDER_LIMIT_034.rs) | P1 | memory | GEN_SQL_DML | 2740067 | 53370846 | <span style="color:#dc2626">-1847.79%</span> |
| 419 | [00846 WINDOW_PARTITION_SUM_059](crates/bench/sqlite_parity/cases/SQLITE_PARITY_846_WINDOW_PARTITION_SUM_059.rs) | P2 | memory | GEN_SQL_WINDOW | 2594331 | 50462927 | <span style="color:#dc2626">-1845.12%</span> |
| 420 | [00388 DML_WHERE_ORDER_LIMIT_001](crates/bench/sqlite_parity/cases/SQLITE_PARITY_388_DML_WHERE_ORDER_LIMIT_001.rs) | P1 | memory | GEN_SQL_DML | 3051947 | 59342671 | <span style="color:#dc2626">-1844.42%</span> |
| 421 | [00414 DML_WHERE_ORDER_LIMIT_027](crates/bench/sqlite_parity/cases/SQLITE_PARITY_414_DML_WHERE_ORDER_LIMIT_027.rs) | P1 | memory | GEN_SQL_DML | 2814158 | 54603419 | <span style="color:#dc2626">-1840.31%</span> |
| 422 | [00847 WINDOW_PARTITION_SUM_060](crates/bench/sqlite_parity/cases/SQLITE_PARITY_847_WINDOW_PARTITION_SUM_060.rs) | P2 | memory | GEN_SQL_WINDOW | 2470798 | 47838739 | <span style="color:#dc2626">-1836.17%</span> |
| 423 | [00027 FOREIGN_KEY_FAILURE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_027_FOREIGN_KEY_FAILURE.rs) | P0 | memory | SQL_FOREIGN_KEYS_NEGATIVE | 2889901 | 55758411 | <span style="color:#dc2626">-1829.42%</span> |
| 424 | [00860 WINDOW_PARTITION_SUM_073](crates/bench/sqlite_parity/cases/SQLITE_PARITY_860_WINDOW_PARTITION_SUM_073.rs) | P2 | memory | GEN_SQL_WINDOW | 2951097 | 56872211 | <span style="color:#dc2626">-1827.15%</span> |
| 425 | [00301 SCALAR_CAST_TYPEOF_019](crates/bench/sqlite_parity/cases/SQLITE_PARITY_301_SCALAR_CAST_TYPEOF_019.rs) | P1 | memory | GEN_SQL_SCALAR | 2233317 | 42962030 | <span style="color:#dc2626">-1823.69%</span> |
| 426 | [00851 WINDOW_PARTITION_SUM_064](crates/bench/sqlite_parity/cases/SQLITE_PARITY_851_WINDOW_PARTITION_SUM_064.rs) | P2 | memory | GEN_SQL_WINDOW | 2619089 | 50366855 | <span style="color:#dc2626">-1823.07%</span> |
| 427 | [00800 WINDOW_PARTITION_SUM_013](crates/bench/sqlite_parity/cases/SQLITE_PARITY_800_WINDOW_PARTITION_SUM_013.rs) | P2 | memory | GEN_SQL_WINDOW | 2759424 | 52867719 | <span style="color:#dc2626">-1815.90%</span> |
| 428 | [00479 DML_WHERE_ORDER_LIMIT_092](crates/bench/sqlite_parity/cases/SQLITE_PARITY_479_DML_WHERE_ORDER_LIMIT_092.rs) | P1 | memory | GEN_SQL_DML | 3155273 | 60372172 | <span style="color:#dc2626">-1813.37%</span> |
| 429 | [00834 WINDOW_PARTITION_SUM_047](crates/bench/sqlite_parity/cases/SQLITE_PARITY_834_WINDOW_PARTITION_SUM_047.rs) | P2 | memory | GEN_SQL_WINDOW | 2483371 | 47428662 | <span style="color:#dc2626">-1809.85%</span> |
| 430 | [00689 JOIN_SUBQUERY_EXISTS_082](crates/bench/sqlite_parity/cases/SQLITE_PARITY_689_JOIN_SUBQUERY_EXISTS_082.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3649209 | 69621386 | <span style="color:#dc2626">-1807.85%</span> |
| 431 | [00294 SCALAR_STRING_017](crates/bench/sqlite_parity/cases/SQLITE_PARITY_294_SCALAR_STRING_017.rs) | P1 | memory | GEN_SQL_SCALAR | 1819304 | 34678760 | <span style="color:#dc2626">-1806.16%</span> |
| 432 | [00441 DML_WHERE_ORDER_LIMIT_054](crates/bench/sqlite_parity/cases/SQLITE_PARITY_441_DML_WHERE_ORDER_LIMIT_054.rs) | P1 | memory | GEN_SQL_DML | 3284738 | 62464451 | <span style="color:#dc2626">-1801.66%</span> |
| 433 | [00415 DML_WHERE_ORDER_LIMIT_028](crates/bench/sqlite_parity/cases/SQLITE_PARITY_415_DML_WHERE_ORDER_LIMIT_028.rs) | P1 | memory | GEN_SQL_DML | 2870865 | 54299675 | <span style="color:#dc2626">-1791.40%</span> |
| 434 | [00418 DML_WHERE_ORDER_LIMIT_031](crates/bench/sqlite_parity/cases/SQLITE_PARITY_418_DML_WHERE_ORDER_LIMIT_031.rs) | P1 | memory | GEN_SQL_DML | 2932061 | 55079560 | <span style="color:#dc2626">-1778.53%</span> |
| 435 | [00484 DML_WHERE_ORDER_LIMIT_097](crates/bench/sqlite_parity/cases/SQLITE_PARITY_484_DML_WHERE_ORDER_LIMIT_097.rs) | P1 | memory | GEN_SQL_DML | 3105018 | 58010601 | <span style="color:#dc2626">-1768.29%</span> |
| 436 | [00795 WINDOW_PARTITION_SUM_008](crates/bench/sqlite_parity/cases/SQLITE_PARITY_795_WINDOW_PARTITION_SUM_008.rs) | P2 | memory | GEN_SQL_WINDOW | 2574204 | 47926875 | <span style="color:#dc2626">-1761.81%</span> |
| 437 | [00827 WINDOW_PARTITION_SUM_040](crates/bench/sqlite_parity/cases/SQLITE_PARITY_827_WINDOW_PARTITION_SUM_040.rs) | P2 | memory | GEN_SQL_WINDOW | 2743575 | 50848246 | <span style="color:#dc2626">-1753.36%</span> |
| 438 | [00011 CREATE_TABLE_AS_SELECT](crates/bench/sqlite_parity/cases/SQLITE_PARITY_011_CREATE_TABLE_AS_SELECT.rs) | P0 | memory | SQL_DDL | 1984386 | 36536184 | <span style="color:#dc2626">-1741.18%</span> |
| 439 | [00317 SCALAR_CAST_TYPEOF_023](crates/bench/sqlite_parity/cases/SQLITE_PARITY_317_SCALAR_CAST_TYPEOF_023.rs) | P1 | memory | GEN_SQL_SCALAR | 1873376 | 34419940 | <span style="color:#dc2626">-1737.32%</span> |
| 440 | [00431 DML_WHERE_ORDER_LIMIT_044](crates/bench/sqlite_parity/cases/SQLITE_PARITY_431_DML_WHERE_ORDER_LIMIT_044.rs) | P1 | memory | GEN_SQL_DML | 3095790 | 56012377 | <span style="color:#dc2626">-1709.31%</span> |
| 441 | [00497 DML_WHERE_ORDER_LIMIT_110](crates/bench/sqlite_parity/cases/SQLITE_PARITY_497_DML_WHERE_ORDER_LIMIT_110.rs) | P1 | memory | GEN_SQL_DML | 3601318 | 64767814 | <span style="color:#dc2626">-1698.45%</span> |
| 442 | [00019 REPLACE_INTO](crates/bench/sqlite_parity/cases/SQLITE_PARITY_019_REPLACE_INTO.rs) | P0 | memory | SQL_REPLACE | 2937271 | 52542513 | <span style="color:#dc2626">-1688.82%</span> |
| 443 | [00818 WINDOW_PARTITION_SUM_031](crates/bench/sqlite_parity/cases/SQLITE_PARITY_818_WINDOW_PARTITION_SUM_031.rs) | P2 | memory | GEN_SQL_WINDOW | 2462993 | 44023045 | <span style="color:#dc2626">-1687.38%</span> |
| 444 | [00836 WINDOW_PARTITION_SUM_049](crates/bench/sqlite_parity/cases/SQLITE_PARITY_836_WINDOW_PARTITION_SUM_049.rs) | P2 | memory | GEN_SQL_WINDOW | 2673762 | 47455544 | <span style="color:#dc2626">-1674.86%</span> |
| 445 | [00842 WINDOW_PARTITION_SUM_055](crates/bench/sqlite_parity/cases/SQLITE_PARITY_842_WINDOW_PARTITION_SUM_055.rs) | P2 | memory | GEN_SQL_WINDOW | 2797557 | 49367343 | <span style="color:#dc2626">-1664.66%</span> |
| 446 | [00429 DML_WHERE_ORDER_LIMIT_042](crates/bench/sqlite_parity/cases/SQLITE_PARITY_429_DML_WHERE_ORDER_LIMIT_042.rs) | P1 | memory | GEN_SQL_DML | 3265362 | 57197510 | <span style="color:#dc2626">-1651.64%</span> |
| 447 | [00837 WINDOW_PARTITION_SUM_050](crates/bench/sqlite_parity/cases/SQLITE_PARITY_837_WINDOW_PARTITION_SUM_050.rs) | P2 | memory | GEN_SQL_WINDOW | 2733234 | 47844760 | <span style="color:#dc2626">-1650.48%</span> |
| 448 | [00824 WINDOW_PARTITION_SUM_037](crates/bench/sqlite_parity/cases/SQLITE_PARITY_824_WINDOW_PARTITION_SUM_037.rs) | P2 | memory | GEN_SQL_WINDOW | 2513609 | 43951790 | <span style="color:#dc2626">-1648.55%</span> |
| 449 | [00794 WINDOW_PARTITION_SUM_007](crates/bench/sqlite_parity/cases/SQLITE_PARITY_794_WINDOW_PARTITION_SUM_007.rs) | P2 | memory | GEN_SQL_WINDOW | 2716883 | 47457847 | <span style="color:#dc2626">-1646.78%</span> |
| 450 | [00083 CORE_NUMERIC_FUNCTIONS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_083_CORE_NUMERIC_FUNCTIONS.rs) | P0 | memory | SQL_FUNCTIONS | 2169136 | 37581513 | <span style="color:#dc2626">-1632.56%</span> |
| 451 | [00840 WINDOW_PARTITION_SUM_053](crates/bench/sqlite_parity/cases/SQLITE_PARITY_840_WINDOW_PARTITION_SUM_053.rs) | P2 | memory | GEN_SQL_WINDOW | 2817033 | 48594079 | <span style="color:#dc2626">-1625.01%</span> |
| 452 | [00047 PRAGMA_FOREIGN_KEYS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_047_PRAGMA_FOREIGN_KEYS.rs) | P0 | memory | SQL_PRAGMA | 2114132 | 36421918 | <span style="color:#dc2626">-1622.78%</span> |
| 453 | [00101 SCHEMA_SQLITE_MASTER_ALIAS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_101_SCHEMA_SQLITE_MASTER_ALIAS.rs) | P0 | memory | SQL_SCHEMA | 2554175 | 43906386 | <span style="color:#dc2626">-1619.00%</span> |
| 454 | [00013 INSERT_SELECT](crates/bench/sqlite_parity/cases/SQLITE_PARITY_013_INSERT_SELECT.rs) | P0 | memory | SQL_INSERT | 2686776 | 46156926 | <span style="color:#dc2626">-1617.93%</span> |
| 455 | [00368 SCALAR_ARITH_036](crates/bench/sqlite_parity/cases/SQLITE_PARITY_368_SCALAR_ARITH_036.rs) | P1 | memory | GEN_SQL_SCALAR | 1818523 | 31128939 | <span style="color:#dc2626">-1611.77%</span> |
| 456 | [00852 WINDOW_PARTITION_SUM_065](crates/bench/sqlite_parity/cases/SQLITE_PARITY_852_WINDOW_PARTITION_SUM_065.rs) | P2 | memory | GEN_SQL_WINDOW | 3083348 | 52756319 | <span style="color:#dc2626">-1611.01%</span> |
| 457 | [00826 WINDOW_PARTITION_SUM_039](crates/bench/sqlite_parity/cases/SQLITE_PARITY_826_WINDOW_PARTITION_SUM_039.rs) | P2 | memory | GEN_SQL_WINDOW | 2986504 | 51007348 | <span style="color:#dc2626">-1607.93%</span> |
| 458 | [00465 DML_WHERE_ORDER_LIMIT_078](crates/bench/sqlite_parity/cases/SQLITE_PARITY_465_DML_WHERE_ORDER_LIMIT_078.rs) | P1 | memory | GEN_SQL_DML | 3419354 | 58305089 | <span style="color:#dc2626">-1605.15%</span> |
| 459 | [00439 DML_WHERE_ORDER_LIMIT_052](crates/bench/sqlite_parity/cases/SQLITE_PARITY_439_DML_WHERE_ORDER_LIMIT_052.rs) | P1 | memory | GEN_SQL_DML | 3551474 | 60233739 | <span style="color:#dc2626">-1596.02%</span> |
| 460 | [00819 WINDOW_PARTITION_SUM_032](crates/bench/sqlite_parity/cases/SQLITE_PARITY_819_WINDOW_PARTITION_SUM_032.rs) | P2 | memory | GEN_SQL_WINDOW | 2656750 | 44946644 | <span style="color:#dc2626">-1591.79%</span> |
| 461 | [00839 WINDOW_PARTITION_SUM_052](crates/bench/sqlite_parity/cases/SQLITE_PARITY_839_WINDOW_PARTITION_SUM_052.rs) | P2 | memory | GEN_SQL_WINDOW | 2722515 | 45866816 | <span style="color:#dc2626">-1584.72%</span> |
| 462 | [00793 WINDOW_PARTITION_SUM_006](crates/bench/sqlite_parity/cases/SQLITE_PARITY_793_WINDOW_PARTITION_SUM_006.rs) | P2 | memory | GEN_SQL_WINDOW | 2863982 | 48026844 | <span style="color:#dc2626">-1576.93%</span> |
| 463 | [00338 SCALAR_STRING_028](crates/bench/sqlite_parity/cases/SQLITE_PARITY_338_SCALAR_STRING_028.rs) | P1 | memory | GEN_SQL_SCALAR | 3446675 | 57039791 | <span style="color:#dc2626">-1554.92%</span> |
| 464 | [00814 WINDOW_PARTITION_SUM_027](crates/bench/sqlite_parity/cases/SQLITE_PARITY_814_WINDOW_PARTITION_SUM_027.rs) | P2 | memory | GEN_SQL_WINDOW | 2822443 | 46540661 | <span style="color:#dc2626">-1548.95%</span> |
| 465 | [00858 WINDOW_PARTITION_SUM_071](crates/bench/sqlite_parity/cases/SQLITE_PARITY_858_WINDOW_PARTITION_SUM_071.rs) | P2 | memory | GEN_SQL_WINDOW | 3024166 | 49363446 | <span style="color:#dc2626">-1532.30%</span> |
| 466 | [00234 SCALAR_STRING_002](crates/bench/sqlite_parity/cases/SQLITE_PARITY_234_SCALAR_STRING_002.rs) | P1 | memory | GEN_SQL_SCALAR | 2418097 | 39361984 | <span style="color:#dc2626">-1527.81%</span> |
| 467 | [00467 DML_WHERE_ORDER_LIMIT_080](crates/bench/sqlite_parity/cases/SQLITE_PARITY_467_DML_WHERE_ORDER_LIMIT_080.rs) | P1 | memory | GEN_SQL_DML | 3462465 | 56183092 | <span style="color:#dc2626">-1522.63%</span> |
| 468 | [00378 SCALAR_STRING_038](crates/bench/sqlite_parity/cases/SQLITE_PARITY_378_SCALAR_STRING_038.rs) | P1 | memory | GEN_SQL_SCALAR | 2085217 | 33698904 | <span style="color:#dc2626">-1516.09%</span> |
| 469 | [00843 WINDOW_PARTITION_SUM_056](crates/bench/sqlite_parity/cases/SQLITE_PARITY_843_WINDOW_PARTITION_SUM_056.rs) | P2 | memory | GEN_SQL_WINDOW | 3029385 | 48686564 | <span style="color:#dc2626">-1507.14%</span> |
| 470 | [00835 WINDOW_PARTITION_SUM_048](crates/bench/sqlite_parity/cases/SQLITE_PARITY_835_WINDOW_PARTITION_SUM_048.rs) | P2 | memory | GEN_SQL_WINDOW | 3180501 | 51087219 | <span style="color:#dc2626">-1506.26%</span> |
| 471 | [00856 WINDOW_PARTITION_SUM_069](crates/bench/sqlite_parity/cases/SQLITE_PARITY_856_WINDOW_PARTITION_SUM_069.rs) | P2 | memory | GEN_SQL_WINDOW | 2841049 | 45546911 | <span style="color:#dc2626">-1503.17%</span> |
| 472 | [00789 WINDOW_PARTITION_SUM_002](crates/bench/sqlite_parity/cases/SQLITE_PARITY_789_WINDOW_PARTITION_SUM_002.rs) | P2 | memory | GEN_SQL_WINDOW | 2960164 | 46904519 | <span style="color:#dc2626">-1484.52%</span> |
| 473 | [00830 WINDOW_PARTITION_SUM_043](crates/bench/sqlite_parity/cases/SQLITE_PARITY_830_WINDOW_PARTITION_SUM_043.rs) | P2 | memory | GEN_SQL_WINDOW | 2891334 | 45254156 | <span style="color:#dc2626">-1465.17%</span> |
| 474 | [00821 WINDOW_PARTITION_SUM_034](crates/bench/sqlite_parity/cases/SQLITE_PARITY_821_WINDOW_PARTITION_SUM_034.rs) | P2 | memory | GEN_SQL_WINDOW | 2866147 | 44832788 | <span style="color:#dc2626">-1464.22%</span> |
| 475 | [00049 PRAGMA_TEMP_STORE_MEMORY](crates/bench/sqlite_parity/cases/SQLITE_PARITY_049_PRAGMA_TEMP_STORE_MEMORY.rs) | P0 | memory | SQL_PRAGMA | 1702132 | 26453006 | <span style="color:#dc2626">-1454.11%</span> |
| 476 | [00052 PRAGMA_INTEGRITY_QUICK_CHECK](crates/bench/sqlite_parity/cases/SQLITE_PARITY_052_PRAGMA_INTEGRITY_QUICK_CHECK.rs) | P0 | memory | SQL_PRAGMA | 3089268 | 47994405 | <span style="color:#dc2626">-1453.59%</span> |
| 477 | [00068 CAST_AND_TYPE_AFFINITY](crates/bench/sqlite_parity/cases/SQLITE_PARITY_068_CAST_AND_TYPE_AFFINITY.rs) | P0 | memory | SQL_EXPRESSIONS | 3123983 | 48421603 | <span style="color:#dc2626">-1450.00%</span> |
| 478 | [00823 WINDOW_PARTITION_SUM_036](crates/bench/sqlite_parity/cases/SQLITE_PARITY_823_WINDOW_PARTITION_SUM_036.rs) | P2 | memory | GEN_SQL_WINDOW | 2984300 | 45931739 | <span style="color:#dc2626">-1439.11%</span> |
| 479 | [00080 NOT_NULL_FAILURE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_080_NOT_NULL_FAILURE.rs) | P0 | memory | SQL_CONSTRAINTS_NEGATIVE | 2535700 | 38905138 | <span style="color:#dc2626">-1434.30%</span> |
| 480 | [00334 SCALAR_STRING_027](crates/bench/sqlite_parity/cases/SQLITE_PARITY_334_SCALAR_STRING_027.rs) | P1 | memory | GEN_SQL_SCALAR | 2443455 | 37389751 | <span style="color:#dc2626">-1430.20%</span> |
| 481 | [00859 WINDOW_PARTITION_SUM_072](crates/bench/sqlite_parity/cases/SQLITE_PARITY_859_WINDOW_PARTITION_SUM_072.rs) | P2 | memory | GEN_SQL_WINDOW | 3087005 | 47025690 | <span style="color:#dc2626">-1423.34%</span> |
| 482 | [00236 SCALAR_ARITH_003](crates/bench/sqlite_parity/cases/SQLITE_PARITY_236_SCALAR_ARITH_003.rs) | P1 | memory | GEN_SQL_SCALAR | 1871643 | 28419319 | <span style="color:#dc2626">-1418.42%</span> |
| 483 | [00342 SCALAR_STRING_029](crates/bench/sqlite_parity/cases/SQLITE_PARITY_342_SCALAR_STRING_029.rs) | P1 | memory | GEN_SQL_SCALAR | 1751767 | 26549520 | <span style="color:#dc2626">-1415.59%</span> |
| 484 | [00079 CHECK_CONSTRAINT_FAILURE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_079_CHECK_CONSTRAINT_FAILURE.rs) | P0 | memory | SQL_CONSTRAINTS_NEGATIVE | 2602717 | 39236406 | <span style="color:#dc2626">-1407.52%</span> |
| 485 | [00179 OPT_MARKDOWN_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_179_OPT_MARKDOWN_MODE.rs) | P2 | memory | CLI_OPTION | 2506514 | 37686221 | <span style="color:#dc2626">-1403.53%</span> |
| 486 | [00309 SCALAR_CAST_TYPEOF_021](crates/bench/sqlite_parity/cases/SQLITE_PARITY_309_SCALAR_CAST_TYPEOF_021.rs) | P1 | memory | GEN_SQL_SCALAR | 1703245 | 24904807 | <span style="color:#dc2626">-1362.20%</span> |
| 487 | [00305 SCALAR_CAST_TYPEOF_020](crates/bench/sqlite_parity/cases/SQLITE_PARITY_305_SCALAR_CAST_TYPEOF_020.rs) | P1 | memory | GEN_SQL_SCALAR | 1754952 | 25579413 | <span style="color:#dc2626">-1357.56%</span> |
| 488 | [00341 SCALAR_CAST_TYPEOF_029](crates/bench/sqlite_parity/cases/SQLITE_PARITY_341_SCALAR_CAST_TYPEOF_029.rs) | P1 | memory | GEN_SQL_SCALAR | 2656709 | 38686677 | <span style="color:#dc2626">-1356.19%</span> |
| 489 | [00825 WINDOW_PARTITION_SUM_038](crates/bench/sqlite_parity/cases/SQLITE_PARITY_825_WINDOW_PARTITION_SUM_038.rs) | P2 | memory | GEN_SQL_WINDOW | 3075693 | 44205861 | <span style="color:#dc2626">-1337.27%</span> |
| 490 | [00025 BEGIN_MODES](crates/bench/sqlite_parity/cases/SQLITE_PARITY_025_BEGIN_MODES.rs) | P0 | memory | SQL_TRANSACTION | 2944494 | 42184375 | <span style="color:#dc2626">-1332.65%</span> |
| 491 | [00262 SCALAR_STRING_009](crates/bench/sqlite_parity/cases/SQLITE_PARITY_262_SCALAR_STRING_009.rs) | P1 | memory | GEN_SQL_SCALAR | 1946555 | 27698445 | <span style="color:#dc2626">-1322.95%</span> |
| 492 | [00829 WINDOW_PARTITION_SUM_042](crates/bench/sqlite_parity/cases/SQLITE_PARITY_829_WINDOW_PARTITION_SUM_042.rs) | P2 | memory | GEN_SQL_WINDOW | 3311149 | 46879463 | <span style="color:#dc2626">-1315.81%</span> |
| 493 | [00855 WINDOW_PARTITION_SUM_068](crates/bench/sqlite_parity/cases/SQLITE_PARITY_855_WINDOW_PARTITION_SUM_068.rs) | P2 | memory | GEN_SQL_WINDOW | 3294939 | 46432546 | <span style="color:#dc2626">-1309.21%</span> |
| 494 | [00241 SCALAR_CAST_TYPEOF_004](crates/bench/sqlite_parity/cases/SQLITE_PARITY_241_SCALAR_CAST_TYPEOF_004.rs) | P1 | memory | GEN_SQL_SCALAR | 1655514 | 23298495 | <span style="color:#dc2626">-1307.33%</span> |
| 495 | [00098 CLI_REGEXP_OPTIONAL](crates/bench/sqlite_parity/cases/SQLITE_PARITY_098_CLI_REGEXP_OPTIONAL.rs) | P2 | memory | CLI_EXTENSION_OPTIONAL | 1716499 | 23875808 | <span style="color:#dc2626">-1290.96%</span> |
| 496 | [00269 SCALAR_CAST_TYPEOF_011](crates/bench/sqlite_parity/cases/SQLITE_PARITY_269_SCALAR_CAST_TYPEOF_011.rs) | P1 | memory | GEN_SQL_SCALAR | 1710127 | 23671120 | <span style="color:#dc2626">-1284.17%</span> |
| 497 | [00082 CORE_STRING_FUNCTIONS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_082_CORE_STRING_FUNCTIONS.rs) | P0 | memory | SQL_FUNCTIONS | 1736377 | 23925512 | <span style="color:#dc2626">-1277.90%</span> |
| 498 | [00085 CORE_RANDOM_SHAPE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_085_CORE_RANDOM_SHAPE.rs) | P0 | memory | SQL_FUNCTIONS | 1783466 | 24383378 | <span style="color:#dc2626">-1267.19%</span> |
| 499 | [00333 SCALAR_CAST_TYPEOF_027](crates/bench/sqlite_parity/cases/SQLITE_PARITY_333_SCALAR_CAST_TYPEOF_027.rs) | P1 | memory | GEN_SQL_SCALAR | 1683427 | 22798970 | <span style="color:#dc2626">-1254.32%</span> |
| 500 | [00069 COLLATE_NOCASE_RTRIM_BINARY](crates/bench/sqlite_parity/cases/SQLITE_PARITY_069_COLLATE_NOCASE_RTRIM_BINARY.rs) | P0 | memory | SQL_COLLATION | 1834863 | 24803524 | <span style="color:#dc2626">-1251.79%</span> |
| 501 | [00180 OPT_BOX_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_180_OPT_BOX_MODE.rs) | P2 | memory | CLI_OPTION | 1741336 | 23215909 | <span style="color:#dc2626">-1233.22%</span> |
| 502 | [00373 SCALAR_CAST_TYPEOF_037](crates/bench/sqlite_parity/cases/SQLITE_PARITY_373_SCALAR_CAST_TYPEOF_037.rs) | P1 | memory | GEN_SQL_SCALAR | 1694488 | 22565318 | <span style="color:#dc2626">-1231.69%</span> |
| 503 | [00086 DATE_TIME_FUNCTIONS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_086_DATE_TIME_FUNCTIONS.rs) | P0 | memory | SQL_FUNCTIONS | 1699827 | 22625150 | <span style="color:#dc2626">-1231.03%</span> |
| 504 | [00257 SCALAR_CAST_TYPEOF_008](crates/bench/sqlite_parity/cases/SQLITE_PARITY_257_SCALAR_CAST_TYPEOF_008.rs) | P1 | memory | GEN_SQL_SCALAR | 1738170 | 23088407 | <span style="color:#dc2626">-1228.32%</span> |
| 505 | [00107 DOT_HELP_PATTERN](crates/bench/sqlite_parity/cases/SQLITE_PARITY_107_DOT_HELP_PATTERN.rs) | P0 | memory | CLI_DOT_COMMAND | 1978315 | 26184588 | <span style="color:#dc2626">-1223.58%</span> |
| 506 | [00009 STRICT_TABLE_TYPE_FAILURE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_009_STRICT_TABLE_TYPE_FAILURE.rs) | P0 | memory | SQL_DDL_NEGATIVE | 2796073 | 37006183 | <span style="color:#dc2626">-1223.51%</span> |
| 507 | [00358 SCALAR_STRING_033](crates/bench/sqlite_parity/cases/SQLITE_PARITY_358_SCALAR_STRING_033.rs) | P1 | memory | GEN_SQL_SCALAR | 1747699 | 23071767 | <span style="color:#dc2626">-1220.12%</span> |
| 508 | [00300 SCALAR_ARITH_019](crates/bench/sqlite_parity/cases/SQLITE_PARITY_300_SCALAR_ARITH_019.rs) | P1 | memory | GEN_SQL_SCALAR | 2338928 | 30664350 | <span style="color:#dc2626">-1211.04%</span> |
| 509 | [00296 SCALAR_ARITH_018](crates/bench/sqlite_parity/cases/SQLITE_PARITY_296_SCALAR_ARITH_018.rs) | P1 | memory | GEN_SQL_SCALAR | 1664401 | 21810740 | <span style="color:#dc2626">-1210.43%</span> |
| 510 | [00304 SCALAR_ARITH_020](crates/bench/sqlite_parity/cases/SQLITE_PARITY_304_SCALAR_ARITH_020.rs) | P1 | memory | GEN_SQL_SCALAR | 1903845 | 24744874 | <span style="color:#dc2626">-1199.73%</span> |
| 511 | [00175 OPT_QUOTE_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_175_OPT_QUOTE_MODE.rs) | P1 | memory | CLI_OPTION | 1765001 | 22897065 | <span style="color:#dc2626">-1197.28%</span> |
| 512 | [00385 SCALAR_CAST_TYPEOF_040](crates/bench/sqlite_parity/cases/SQLITE_PARITY_385_SCALAR_CAST_TYPEOF_040.rs) | P1 | memory | GEN_SQL_SCALAR | 1840895 | 23842406 | <span style="color:#dc2626">-1195.15%</span> |
| 513 | [00249 SCALAR_CAST_TYPEOF_006](crates/bench/sqlite_parity/cases/SQLITE_PARITY_249_SCALAR_CAST_TYPEOF_006.rs) | P1 | memory | GEN_SQL_SCALAR | 1807411 | 23393215 | <span style="color:#dc2626">-1194.29%</span> |
| 514 | [00352 SCALAR_ARITH_032](crates/bench/sqlite_parity/cases/SQLITE_PARITY_352_SCALAR_ARITH_032.rs) | P1 | memory | GEN_SQL_SCALAR | 1799056 | 22995973 | <span style="color:#dc2626">-1178.22%</span> |
| 515 | [00240 SCALAR_ARITH_004](crates/bench/sqlite_parity/cases/SQLITE_PARITY_240_SCALAR_ARITH_004.rs) | P1 | memory | GEN_SQL_SCALAR | 1780440 | 22737263 | <span style="color:#dc2626">-1177.06%</span> |
| 516 | [00302 SCALAR_STRING_019](crates/bench/sqlite_parity/cases/SQLITE_PARITY_302_SCALAR_STRING_019.rs) | P1 | memory | GEN_SQL_SCALAR | 2189064 | 27902843 | <span style="color:#dc2626">-1174.65%</span> |
| 517 | [00102 WITH_MATERIALIZED_HINTS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_102_WITH_MATERIALIZED_HINTS.rs) | P0 | memory | SQL_CTE | 1773848 | 22552442 | <span style="color:#dc2626">-1171.39%</span> |
| 518 | [00324 SCALAR_ARITH_025](crates/bench/sqlite_parity/cases/SQLITE_PARITY_324_SCALAR_ARITH_025.rs) | P1 | memory | GEN_SQL_SCALAR | 1668810 | 21180275 | <span style="color:#dc2626">-1169.18%</span> |
| 519 | [00089 JSON_TABLE_VALUED_FUNCTIONS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_089_JSON_TABLE_VALUED_FUNCTIONS.rs) | P0 | memory | SQL_JSON | 1713374 | 21672847 | <span style="color:#dc2626">-1164.92%</span> |
| 520 | [00228 SCALAR_ARITH_001](crates/bench/sqlite_parity/cases/SQLITE_PARITY_228_SCALAR_ARITH_001.rs) | P1 | memory | GEN_SQL_SCALAR | 1710248 | 21624556 | <span style="color:#dc2626">-1164.41%</span> |
| 521 | [00362 SCALAR_STRING_034](crates/bench/sqlite_parity/cases/SQLITE_PARITY_362_SCALAR_STRING_034.rs) | P1 | memory | GEN_SQL_SCALAR | 1719064 | 21718455 | <span style="color:#dc2626">-1163.39%</span> |
| 522 | [00377 SCALAR_CAST_TYPEOF_038](crates/bench/sqlite_parity/cases/SQLITE_PARITY_377_SCALAR_CAST_TYPEOF_038.rs) | P1 | memory | GEN_SQL_SCALAR | 1811470 | 22822866 | <span style="color:#dc2626">-1159.91%</span> |
| 523 | [00260 SCALAR_ARITH_009](crates/bench/sqlite_parity/cases/SQLITE_PARITY_260_SCALAR_ARITH_009.rs) | P1 | memory | GEN_SQL_SCALAR | 1849962 | 23177256 | <span style="color:#dc2626">-1152.85%</span> |
| 524 | [00381 SCALAR_CAST_TYPEOF_039](crates/bench/sqlite_parity/cases/SQLITE_PARITY_381_SCALAR_CAST_TYPEOF_039.rs) | P1 | memory | GEN_SQL_SCALAR | 1827279 | 22759255 | <span style="color:#dc2626">-1145.53%</span> |
| 525 | [00380 SCALAR_ARITH_039](crates/bench/sqlite_parity/cases/SQLITE_PARITY_380_SCALAR_ARITH_039.rs) | P1 | memory | GEN_SQL_SCALAR | 1855943 | 23090843 | <span style="color:#dc2626">-1144.16%</span> |
| 526 | [00106 DOT_HELP](crates/bench/sqlite_parity/cases/SQLITE_PARITY_106_DOT_HELP.rs) | P0 | memory | CLI_DOT_COMMAND | 2267503 | 28189724 | <span style="color:#dc2626">-1143.21%</span> |
| 527 | [00372 SCALAR_ARITH_037](crates/bench/sqlite_parity/cases/SQLITE_PARITY_372_SCALAR_ARITH_037.rs) | P1 | memory | GEN_SQL_SCALAR | 1835114 | 22684383 | <span style="color:#dc2626">-1136.13%</span> |
| 528 | [00374 SCALAR_STRING_037](crates/bench/sqlite_parity/cases/SQLITE_PARITY_374_SCALAR_STRING_037.rs) | P1 | memory | GEN_SQL_SCALAR | 1878426 | 23171666 | <span style="color:#dc2626">-1133.57%</span> |
| 529 | [00081 BLOBS_HEX_ZEROBLOB](crates/bench/sqlite_parity/cases/SQLITE_PARITY_081_BLOBS_HEX_ZEROBLOB.rs) | P0 | memory | SQL_FUNCTIONS | 1726128 | 21278230 | <span style="color:#dc2626">-1132.71%</span> |
| 530 | [00325 SCALAR_CAST_TYPEOF_025](crates/bench/sqlite_parity/cases/SQLITE_PARITY_325_SCALAR_CAST_TYPEOF_025.rs) | P1 | memory | GEN_SQL_SCALAR | 1892543 | 22956258 | <span style="color:#dc2626">-1112.98%</span> |
| 531 | [00316 SCALAR_ARITH_023](crates/bench/sqlite_parity/cases/SQLITE_PARITY_316_SCALAR_ARITH_023.rs) | P1 | memory | GEN_SQL_SCALAR | 1880509 | 22768342 | <span style="color:#dc2626">-1110.75%</span> |
| 532 | [00312 SCALAR_ARITH_022](crates/bench/sqlite_parity/cases/SQLITE_PARITY_312_SCALAR_ARITH_022.rs) | P1 | memory | GEN_SQL_SCALAR | 1867485 | 22604792 | <span style="color:#dc2626">-1110.44%</span> |
| 533 | [00256 SCALAR_ARITH_008](crates/bench/sqlite_parity/cases/SQLITE_PARITY_256_SCALAR_ARITH_008.rs) | P1 | memory | GEN_SQL_SCALAR | 1776914 | 21477928 | <span style="color:#dc2626">-1108.72%</span> |
| 534 | [00258 SCALAR_STRING_008](crates/bench/sqlite_parity/cases/SQLITE_PARITY_258_SCALAR_STRING_008.rs) | P1 | memory | GEN_SQL_SCALAR | 1751376 | 21009522 | <span style="color:#dc2626">-1099.60%</span> |
| 535 | [00326 SCALAR_STRING_025](crates/bench/sqlite_parity/cases/SQLITE_PARITY_326_SCALAR_STRING_025.rs) | P1 | memory | GEN_SQL_SCALAR | 1901300 | 22598861 | <span style="color:#dc2626">-1088.60%</span> |
| 536 | [00386 SCALAR_STRING_040](crates/bench/sqlite_parity/cases/SQLITE_PARITY_386_SCALAR_STRING_040.rs) | P1 | memory | GEN_SQL_SCALAR | 1753519 | 20790828 | <span style="color:#dc2626">-1085.66%</span> |
| 537 | [00318 SCALAR_STRING_023](crates/bench/sqlite_parity/cases/SQLITE_PARITY_318_SCALAR_STRING_023.rs) | P1 | memory | GEN_SQL_SCALAR | 2230172 | 26374379 | <span style="color:#dc2626">-1082.62%</span> |
| 538 | [00360 SCALAR_ARITH_034](crates/bench/sqlite_parity/cases/SQLITE_PARITY_360_SCALAR_ARITH_034.rs) | P1 | memory | GEN_SQL_SCALAR | 1714075 | 20232792 | <span style="color:#dc2626">-1080.39%</span> |
| 539 | [00184 OPT_ASCII_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_184_OPT_ASCII_MODE.rs) | P2 | memory | CLI_OPTION | 1943859 | 22938273 | <span style="color:#dc2626">-1080.04%</span> |
| 540 | [00264 SCALAR_ARITH_010](crates/bench/sqlite_parity/cases/SQLITE_PARITY_264_SCALAR_ARITH_010.rs) | P1 | memory | GEN_SQL_SCALAR | 1784609 | 21006216 | <span style="color:#dc2626">-1077.08%</span> |
| 541 | [00261 SCALAR_CAST_TYPEOF_009](crates/bench/sqlite_parity/cases/SQLITE_PARITY_261_SCALAR_CAST_TYPEOF_009.rs) | P1 | memory | GEN_SQL_SCALAR | 1750183 | 20582413 | <span style="color:#dc2626">-1076.01%</span> |
| 542 | [00356 SCALAR_ARITH_033](crates/bench/sqlite_parity/cases/SQLITE_PARITY_356_SCALAR_ARITH_033.rs) | P1 | memory | GEN_SQL_SCALAR | 1717672 | 20081907 | <span style="color:#dc2626">-1069.14%</span> |
| 543 | [00238 SCALAR_STRING_003](crates/bench/sqlite_parity/cases/SQLITE_PARITY_238_SCALAR_STRING_003.rs) | P1 | memory | GEN_SQL_SCALAR | 1773327 | 20705125 | <span style="color:#dc2626">-1067.59%</span> |
| 544 | [00321 SCALAR_CAST_TYPEOF_024](crates/bench/sqlite_parity/cases/SQLITE_PARITY_321_SCALAR_CAST_TYPEOF_024.rs) | P1 | memory | GEN_SQL_SCALAR | 2039341 | 23784757 | <span style="color:#dc2626">-1066.30%</span> |
| 545 | [00245 SCALAR_CAST_TYPEOF_005](crates/bench/sqlite_parity/cases/SQLITE_PARITY_245_SCALAR_CAST_TYPEOF_005.rs) | P1 | memory | GEN_SQL_SCALAR | 1862536 | 21661416 | <span style="color:#dc2626">-1063.01%</span> |
| 546 | [00332 SCALAR_ARITH_027](crates/bench/sqlite_parity/cases/SQLITE_PARITY_332_SCALAR_ARITH_027.rs) | P1 | memory | GEN_SQL_SCALAR | 1773117 | 20551826 | <span style="color:#dc2626">-1059.08%</span> |
| 547 | [00365 SCALAR_CAST_TYPEOF_035](crates/bench/sqlite_parity/cases/SQLITE_PARITY_365_SCALAR_CAST_TYPEOF_035.rs) | P1 | memory | GEN_SQL_SCALAR | 1751616 | 20299488 | <span style="color:#dc2626">-1058.90%</span> |
| 548 | [00230 SCALAR_STRING_001](crates/bench/sqlite_parity/cases/SQLITE_PARITY_230_SCALAR_STRING_001.rs) | P1 | memory | GEN_SQL_SCALAR | 2052255 | 23732086 | <span style="color:#dc2626">-1056.39%</span> |
| 549 | [00361 SCALAR_CAST_TYPEOF_034](crates/bench/sqlite_parity/cases/SQLITE_PARITY_361_SCALAR_CAST_TYPEOF_034.rs) | P1 | memory | GEN_SQL_SCALAR | 1977233 | 22839197 | <span style="color:#dc2626">-1055.11%</span> |
| 550 | [00328 SCALAR_ARITH_026](crates/bench/sqlite_parity/cases/SQLITE_PARITY_328_SCALAR_ARITH_026.rs) | P1 | memory | GEN_SQL_SCALAR | 1817010 | 20949409 | <span style="color:#dc2626">-1052.96%</span> |
| 551 | [00223 OPT_NOHEADER](crates/bench/sqlite_parity/cases/SQLITE_PARITY_223_OPT_NOHEADER.rs) | P2 | memory | CLI_OPTION | 1733571 | 19923506 | <span style="color:#dc2626">-1049.28%</span> |
| 552 | [00346 SCALAR_STRING_030](crates/bench/sqlite_parity/cases/SQLITE_PARITY_346_SCALAR_STRING_030.rs) | P1 | memory | GEN_SQL_SCALAR | 1870932 | 21426883 | <span style="color:#dc2626">-1045.25%</span> |
| 553 | [00233 SCALAR_CAST_TYPEOF_002](crates/bench/sqlite_parity/cases/SQLITE_PARITY_233_SCALAR_CAST_TYPEOF_002.rs) | P1 | memory | GEN_SQL_SCALAR | 1833551 | 20988572 | <span style="color:#dc2626">-1044.70%</span> |
| 554 | [00354 SCALAR_STRING_032](crates/bench/sqlite_parity/cases/SQLITE_PARITY_354_SCALAR_STRING_032.rs) | P1 | memory | GEN_SQL_SCALAR | 1775210 | 20316711 | <span style="color:#dc2626">-1044.47%</span> |
| 555 | [00322 SCALAR_STRING_024](crates/bench/sqlite_parity/cases/SQLITE_PARITY_322_SCALAR_STRING_024.rs) | P1 | memory | GEN_SQL_SCALAR | 1812010 | 20424855 | <span style="color:#dc2626">-1027.19%</span> |
| 556 | [00382 SCALAR_STRING_039](crates/bench/sqlite_parity/cases/SQLITE_PARITY_382_SCALAR_STRING_039.rs) | P1 | memory | GEN_SQL_SCALAR | 1829734 | 20606620 | <span style="color:#dc2626">-1026.21%</span> |
| 557 | [00237 SCALAR_CAST_TYPEOF_003](crates/bench/sqlite_parity/cases/SQLITE_PARITY_237_SCALAR_CAST_TYPEOF_003.rs) | P1 | memory | GEN_SQL_SCALAR | 2101188 | 23361164 | <span style="color:#dc2626">-1011.81%</span> |
| 558 | [00306 SCALAR_STRING_020](crates/bench/sqlite_parity/cases/SQLITE_PARITY_306_SCALAR_STRING_020.rs) | P1 | memory | GEN_SQL_SCALAR | 1812110 | 20139355 | <span style="color:#dc2626">-1011.38%</span> |
| 559 | [00353 SCALAR_CAST_TYPEOF_032](crates/bench/sqlite_parity/cases/SQLITE_PARITY_353_SCALAR_CAST_TYPEOF_032.rs) | P1 | memory | GEN_SQL_SCALAR | 2061313 | 22824508 | <span style="color:#dc2626">-1007.28%</span> |
| 560 | [00313 SCALAR_CAST_TYPEOF_022](crates/bench/sqlite_parity/cases/SQLITE_PARITY_313_SCALAR_CAST_TYPEOF_022.rs) | P1 | memory | GEN_SQL_SCALAR | 1825636 | 20209207 | <span style="color:#dc2626">-1006.97%</span> |
| 561 | [00268 SCALAR_ARITH_011](crates/bench/sqlite_parity/cases/SQLITE_PARITY_268_SCALAR_ARITH_011.rs) | P1 | memory | GEN_SQL_SCALAR | 2118790 | 23439572 | <span style="color:#dc2626">-1006.27%</span> |
| 562 | [00067 CASE_COALESCE_NULLIF_IIF](crates/bench/sqlite_parity/cases/SQLITE_PARITY_067_CASE_COALESCE_NULLIF_IIF.rs) | P0 | memory | SQL_EXPRESSIONS | 1947326 | 21471135 | <span style="color:#dc2626">-1002.60%</span> |
| 563 | [00248 SCALAR_ARITH_006](crates/bench/sqlite_parity/cases/SQLITE_PARITY_248_SCALAR_ARITH_006.rs) | P1 | memory | GEN_SQL_SCALAR | 1912531 | 20967462 | <span style="color:#dc2626">-996.32%</span> |
| 564 | [00329 SCALAR_CAST_TYPEOF_026](crates/bench/sqlite_parity/cases/SQLITE_PARITY_329_SCALAR_CAST_TYPEOF_026.rs) | P1 | memory | GEN_SQL_SCALAR | 2909188 | 31879831 | <span style="color:#dc2626">-995.83%</span> |
| 565 | [00308 SCALAR_ARITH_021](crates/bench/sqlite_parity/cases/SQLITE_PARITY_308_SCALAR_ARITH_021.rs) | P1 | memory | GEN_SQL_SCALAR | 1824695 | 19981987 | <span style="color:#dc2626">-995.09%</span> |
| 566 | [00090 JSON_MUTATION_FUNCTIONS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_090_JSON_MUTATION_FUNCTIONS.rs) | P0 | memory | SQL_JSON | 2016668 | 22047226 | <span style="color:#dc2626">-993.25%</span> |
| 567 | [00229 SCALAR_CAST_TYPEOF_001](crates/bench/sqlite_parity/cases/SQLITE_PARITY_229_SCALAR_CAST_TYPEOF_001.rs) | P1 | memory | GEN_SQL_SCALAR | 1875360 | 20326338 | <span style="color:#dc2626">-983.86%</span> |
| 568 | [00177 OPT_JSON_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_177_OPT_JSON_MODE.rs) | P1 | memory | CLI_OPTION | 2566739 | 27692924 | <span style="color:#dc2626">-978.91%</span> |
| 569 | [00366 SCALAR_STRING_035](crates/bench/sqlite_parity/cases/SQLITE_PARITY_366_SCALAR_STRING_035.rs) | P1 | memory | GEN_SQL_SCALAR | 2123930 | 22872951 | <span style="color:#dc2626">-976.92%</span> |
| 570 | [00337 SCALAR_CAST_TYPEOF_028](crates/bench/sqlite_parity/cases/SQLITE_PARITY_337_SCALAR_CAST_TYPEOF_028.rs) | P1 | memory | GEN_SQL_SCALAR | 2480817 | 26101062 | <span style="color:#dc2626">-952.12%</span> |
| 571 | [00250 SCALAR_STRING_006](crates/bench/sqlite_parity/cases/SQLITE_PARITY_250_SCALAR_STRING_006.rs) | P1 | memory | GEN_SQL_SCALAR | 2172733 | 22847542 | <span style="color:#dc2626">-951.56%</span> |
| 572 | [00297 SCALAR_CAST_TYPEOF_018](crates/bench/sqlite_parity/cases/SQLITE_PARITY_297_SCALAR_CAST_TYPEOF_018.rs) | P1 | memory | GEN_SQL_SCALAR | 2137867 | 22457304 | <span style="color:#dc2626">-950.45%</span> |
| 573 | [00084 CORE_FORMAT_QUOTE_HEX](crates/bench/sqlite_parity/cases/SQLITE_PARITY_084_CORE_FORMAT_QUOTE_HEX.rs) | P0 | memory | SQL_FUNCTIONS | 2126666 | 22280428 | <span style="color:#dc2626">-947.67%</span> |
| 574 | [00310 SCALAR_STRING_021](crates/bench/sqlite_parity/cases/SQLITE_PARITY_310_SCALAR_STRING_021.rs) | P1 | memory | GEN_SQL_SCALAR | 2225383 | 23290341 | <span style="color:#dc2626">-946.58%</span> |
| 575 | [00254 SCALAR_STRING_007](crates/bench/sqlite_parity/cases/SQLITE_PARITY_254_SCALAR_STRING_007.rs) | P1 | memory | GEN_SQL_SCALAR | 2023962 | 21167822 | <span style="color:#dc2626">-945.86%</span> |
| 576 | [00176 OPT_LINE_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_176_OPT_LINE_MODE.rs) | P2 | memory | CLI_OPTION | 1941155 | 20293857 | <span style="color:#dc2626">-945.45%</span> |
| 577 | [00369 SCALAR_CAST_TYPEOF_036](crates/bench/sqlite_parity/cases/SQLITE_PARITY_369_SCALAR_CAST_TYPEOF_036.rs) | P1 | memory | GEN_SQL_SCALAR | 2038169 | 21268513 | <span style="color:#dc2626">-943.51%</span> |
| 578 | [00178 OPT_HTML_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_178_OPT_HTML_MODE.rs) | P2 | memory | CLI_OPTION | 1923250 | 19908818 | <span style="color:#dc2626">-935.17%</span> |
| 579 | [00246 SCALAR_STRING_005](crates/bench/sqlite_parity/cases/SQLITE_PARITY_246_SCALAR_STRING_005.rs) | P1 | memory | GEN_SQL_SCALAR | 1996559 | 20431167 | <span style="color:#dc2626">-923.32%</span> |
| 580 | [00189 OPT_ECHO](crates/bench/sqlite_parity/cases/SQLITE_PARITY_189_OPT_ECHO.rs) | P2 | memory | CLI_OPTION | 2257433 | 23070484 | <span style="color:#dc2626">-921.98%</span> |
| 581 | [00348 SCALAR_ARITH_031](crates/bench/sqlite_parity/cases/SQLITE_PARITY_348_SCALAR_ARITH_031.rs) | P1 | memory | GEN_SQL_SCALAR | 2255580 | 22755408 | <span style="color:#dc2626">-908.85%</span> |
| 582 | [00364 SCALAR_ARITH_035](crates/bench/sqlite_parity/cases/SQLITE_PARITY_364_SCALAR_ARITH_035.rs) | P1 | memory | GEN_SQL_SCALAR | 2075128 | 20847236 | <span style="color:#dc2626">-904.62%</span> |
| 583 | [00185 OPT_SEPARATOR](crates/bench/sqlite_parity/cases/SQLITE_PARITY_185_OPT_SEPARATOR.rs) | P1 | memory | CLI_OPTION | 2010396 | 20137822 | <span style="color:#dc2626">-901.68%</span> |
| 584 | [00330 SCALAR_STRING_026](crates/bench/sqlite_parity/cases/SQLITE_PARITY_330_SCALAR_STRING_026.rs) | P1 | memory | GEN_SQL_SCALAR | 2303961 | 22653465 | <span style="color:#dc2626">-883.24%</span> |
| 585 | [00272 SCALAR_ARITH_012](crates/bench/sqlite_parity/cases/SQLITE_PARITY_272_SCALAR_ARITH_012.rs) | P1 | memory | GEN_SQL_SCALAR | 2173605 | 21289402 | <span style="color:#dc2626">-879.45%</span> |
| 586 | [00088 JSON_SCALAR_FUNCTIONS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_088_JSON_SCALAR_FUNCTIONS.rs) | P0 | memory | SQL_JSON | 2178875 | 21288740 | <span style="color:#dc2626">-877.05%</span> |
| 587 | [00182 OPT_COLUMN_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_182_OPT_COLUMN_MODE.rs) | P2 | memory | CLI_OPTION | 2158957 | 21005715 | <span style="color:#dc2626">-872.96%</span> |
| 588 | [00253 SCALAR_CAST_TYPEOF_007](crates/bench/sqlite_parity/cases/SQLITE_PARITY_253_SCALAR_CAST_TYPEOF_007.rs) | P1 | memory | GEN_SQL_SCALAR | 2528586 | 24352761 | <span style="color:#dc2626">-863.10%</span> |
| 589 | [00265 SCALAR_CAST_TYPEOF_010](crates/bench/sqlite_parity/cases/SQLITE_PARITY_265_SCALAR_CAST_TYPEOF_010.rs) | P1 | memory | GEN_SQL_SCALAR | 2165860 | 20520866 | <span style="color:#dc2626">-847.47%</span> |
| 590 | [00345 SCALAR_CAST_TYPEOF_030](crates/bench/sqlite_parity/cases/SQLITE_PARITY_345_SCALAR_CAST_TYPEOF_030.rs) | P1 | memory | GEN_SQL_SCALAR | 2230242 | 21123970 | <span style="color:#dc2626">-847.16%</span> |
| 591 | [00350 SCALAR_STRING_031](crates/bench/sqlite_parity/cases/SQLITE_PARITY_350_SCALAR_STRING_031.rs) | P1 | memory | GEN_SQL_SCALAR | 2476899 | 23247619 | <span style="color:#dc2626">-838.58%</span> |
| 592 | [00252 SCALAR_ARITH_007](crates/bench/sqlite_parity/cases/SQLITE_PARITY_252_SCALAR_ARITH_007.rs) | P1 | memory | GEN_SQL_SCALAR | 2209673 | 20641585 | <span style="color:#dc2626">-834.15%</span> |
| 593 | [00270 SCALAR_STRING_011](crates/bench/sqlite_parity/cases/SQLITE_PARITY_270_SCALAR_STRING_011.rs) | P1 | memory | GEN_SQL_SCALAR | 2225333 | 20236990 | <span style="color:#dc2626">-809.39%</span> |
| 594 | [00384 SCALAR_ARITH_040](crates/bench/sqlite_parity/cases/SQLITE_PARITY_384_SCALAR_ARITH_040.rs) | P1 | memory | GEN_SQL_SCALAR | 2339950 | 21235721 | <span style="color:#dc2626">-807.53%</span> |
| 595 | [00336 SCALAR_ARITH_028](crates/bench/sqlite_parity/cases/SQLITE_PARITY_336_SCALAR_ARITH_028.rs) | P1 | memory | GEN_SQL_SCALAR | 2734517 | 24040090 | <span style="color:#dc2626">-779.13%</span> |
| 596 | [00173 OPT_LIST_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_173_OPT_LIST_MODE.rs) | P1 | memory | CLI_OPTION | 2319241 | 20078519 | <span style="color:#dc2626">-765.74%</span> |
| 597 | [00242 SCALAR_STRING_004](crates/bench/sqlite_parity/cases/SQLITE_PARITY_242_SCALAR_STRING_004.rs) | P1 | memory | GEN_SQL_SCALAR | 2621372 | 22612606 | <span style="color:#dc2626">-762.62%</span> |
| 598 | [00266 SCALAR_STRING_010](crates/bench/sqlite_parity/cases/SQLITE_PARITY_266_SCALAR_STRING_010.rs) | P1 | memory | GEN_SQL_SCALAR | 2849204 | 24417784 | <span style="color:#dc2626">-757.00%</span> |
| 599 | [00244 SCALAR_ARITH_005](crates/bench/sqlite_parity/cases/SQLITE_PARITY_244_SCALAR_ARITH_005.rs) | P1 | memory | GEN_SQL_SCALAR | 2375908 | 20358600 | <span style="color:#dc2626">-756.88%</span> |
| 600 | [00320 SCALAR_ARITH_024](crates/bench/sqlite_parity/cases/SQLITE_PARITY_320_SCALAR_ARITH_024.rs) | P1 | memory | GEN_SQL_SCALAR | 2338326 | 19975375 | <span style="color:#dc2626">-754.26%</span> |
| 601 | [00232 SCALAR_ARITH_002](crates/bench/sqlite_parity/cases/SQLITE_PARITY_232_SCALAR_ARITH_002.rs) | P1 | memory | GEN_SQL_SCALAR | 2752611 | 23504766 | <span style="color:#dc2626">-753.91%</span> |
| 602 | [00357 SCALAR_CAST_TYPEOF_033](crates/bench/sqlite_parity/cases/SQLITE_PARITY_357_SCALAR_CAST_TYPEOF_033.rs) | P1 | memory | GEN_SQL_SCALAR | 2502297 | 21336922 | <span style="color:#dc2626">-752.69%</span> |
| 603 | [00064 CTE_NON_RECURSIVE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_064_CTE_NON_RECURSIVE.rs) | P0 | memory | SQL_CTE | 2566639 | 20960850 | <span style="color:#dc2626">-716.67%</span> |
| 604 | [00376 SCALAR_ARITH_038](crates/bench/sqlite_parity/cases/SQLITE_PARITY_376_SCALAR_ARITH_038.rs) | P1 | memory | GEN_SQL_SCALAR | 2480466 | 20255947 | <span style="color:#dc2626">-716.62%</span> |
| 605 | [00174 OPT_CSV_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_174_OPT_CSV_MODE.rs) | P1 | memory | CLI_OPTION | 2539517 | 20586612 | <span style="color:#dc2626">-710.65%</span> |
| 606 | [00370 SCALAR_STRING_036](crates/bench/sqlite_parity/cases/SQLITE_PARITY_370_SCALAR_STRING_036.rs) | P1 | memory | GEN_SQL_SCALAR | 2505744 | 20282797 | <span style="color:#dc2626">-709.45%</span> |
| 607 | [00298 SCALAR_STRING_018](crates/bench/sqlite_parity/cases/SQLITE_PARITY_298_SCALAR_STRING_018.rs) | P1 | memory | GEN_SQL_SCALAR | 2784881 | 21914806 | <span style="color:#dc2626">-686.92%</span> |
| 608 | [00191 OPT_BATCH](crates/bench/sqlite_parity/cases/SQLITE_PARITY_191_OPT_BATCH.rs) | P2 | memory | CLI_OPTION | 2873209 | 20116201 | <span style="color:#dc2626">-600.13%</span> |
| 609 | [00221 OPT_DOUBLE_DASH_END_OPTIONS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_221_OPT_DOUBLE_DASH_END_OPTIONS.rs) | P2 | memory | CLI_OPTION | 2948061 | 20459510 | <span style="color:#dc2626">-594.00%</span> |
| 610 | [00349 SCALAR_CAST_TYPEOF_031](crates/bench/sqlite_parity/cases/SQLITE_PARITY_349_SCALAR_CAST_TYPEOF_031.rs) | P1 | memory | GEN_SQL_SCALAR | 2920468 | 20202625 | <span style="color:#dc2626">-591.76%</span> |
| 611 | [00183 OPT_TABS_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_183_OPT_TABS_MODE.rs) | P2 | memory | CLI_OPTION | 2988768 | 20416769 | <span style="color:#dc2626">-583.12%</span> |
| 612 | [00170 OPT_VERSION](crates/bench/sqlite_parity/cases/SQLITE_PARITY_170_OPT_VERSION.rs) | P1 | memory | CLI_OPTION | 2998035 | 1634855 | <span style="color:#2563eb">45.47%</span> |

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
