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
  <img src="https://img.shields.io/badge/version-1.0.22-blue" alt="version">
</p>

---

## Install

### Rust library

Add to `Cargo.toml`. Use an exact pin for production:

```toml
[dependencies]
redlinedb = "=1.0.22"  # exact pin — recommended for production
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
curl -LsSf https://raw.githubusercontent.com/neverhuman/RedlineDB/main/scripts/install.sh | VERSION=v1.0.22 bash
```

Fully lock the download by pinning both the release tag and the tarball digest:

```bash
curl -LsSf https://raw.githubusercontent.com/neverhuman/RedlineDB/main/scripts/install.sh | \
  VERSION=v1.0.22 REDLINEDB_SHA256=<sha256> bash
```

Custom install prefix:

```bash
curl -LsSf https://raw.githubusercontent.com/neverhuman/RedlineDB/main/scripts/install.sh | \
  VERSION=v1.0.22 PREFIX=~/.local bash
```

The script requires SHA-256 verification before installing. By default it
downloads the matching `.sha256` release asset; `REDLINEDB_SHA256` lets CI
pin the exact digest inline.

### cargo install (from source, version-pinned)

```bash
cargo install redlinedb-cli --version 1.0.22 --locked
# or from a specific git tag:
cargo install --git https://github.com/neverhuman/RedlineDB.git --tag v1.0.22 --package redlinedb-cli --locked
```

`--locked` enforces the committed `Cargo.lock` — ensures you get the exact dependency tree that was tested.

### Direct download

Pre-built tarballs on the [releases page](https://github.com/neverhuman/RedlineDB/releases):

| Platform | File |
|---|---|
| Linux x86_64 | `redlinedb-v1.0.22-linux-x86_64.tar.gz` |
| macOS Apple Silicon | `redlinedb-v1.0.22-macos-arm64.tar.gz` |
| macOS Intel | `redlinedb-v1.0.22-macos-x86_64.tar.gz` |

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

**SQLite parity coverage:** **1049 / 1127 = 93.1%** approved generated cases, with **78** remaining. Updated 2026-05-21.

![SQLite parity latency improvement plot](assets/sqlite-parity-latency-gap.svg)

<details>
<summary>Full ranked latency table</summary>

| Rank | Case | Priority | Profile | Category | SQLite median ns | RedlineDB median ns | Improvement |
| ---: | --- | --- | --- | --- | ---: | ---: | ---: |
| 1 | [00905 CONSTRAINT_FK_SAVEPOINT_038](crates/bench/sqlite_parity/cases/SQLITE_PARITY_905_CONSTRAINT_FK_SAVEPOINT_038.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1992890 | 5234563 | <span style="color:#dc2626">-162.66%</span> |
| 2 | [00777 CTE_RECURSIVE_MATRIX_070](crates/bench/sqlite_parity/cases/SQLITE_PARITY_777_CTE_RECURSIVE_MATRIX_070.rs) | P1 | memory | GEN_SQL_CTE | 1820224 | 4554285 | <span style="color:#dc2626">-150.20%</span> |
| 3 | [00636 JOIN_SUBQUERY_EXISTS_029](crates/bench/sqlite_parity/cases/SQLITE_PARITY_636_JOIN_SUBQUERY_EXISTS_029.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1943066 | 4817584 | <span style="color:#dc2626">-147.94%</span> |
| 4 | [00662 JOIN_SUBQUERY_EXISTS_055](crates/bench/sqlite_parity/cases/SQLITE_PARITY_662_JOIN_SUBQUERY_EXISTS_055.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2080806 | 5133662 | <span style="color:#dc2626">-146.72%</span> |
| 5 | [00813 WINDOW_PARTITION_SUM_026](crates/bench/sqlite_parity/cases/SQLITE_PARITY_813_WINDOW_PARTITION_SUM_026.rs) | P2 | memory | GEN_SQL_WINDOW | 1894845 | 4621843 | <span style="color:#dc2626">-143.92%</span> |
| 6 | [00597 AGG_GROUP_HAVING_090](crates/bench/sqlite_parity/cases/SQLITE_PARITY_597_AGG_GROUP_HAVING_090.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1906396 | 4634237 | <span style="color:#dc2626">-143.09%</span> |
| 7 | [00730 CTE_RECURSIVE_MATRIX_023](crates/bench/sqlite_parity/cases/SQLITE_PARITY_730_CTE_RECURSIVE_MATRIX_023.rs) | P1 | memory | GEN_SQL_CTE | 1827327 | 4346753 | <span style="color:#dc2626">-137.87%</span> |
| 8 | [00755 CTE_RECURSIVE_MATRIX_048](crates/bench/sqlite_parity/cases/SQLITE_PARITY_755_CTE_RECURSIVE_MATRIX_048.rs) | P1 | memory | GEN_SQL_CTE | 1829211 | 4345902 | <span style="color:#dc2626">-137.58%</span> |
| 9 | [00460 DML_WHERE_ORDER_LIMIT_073](crates/bench/sqlite_parity/cases/SQLITE_PARITY_460_DML_WHERE_ORDER_LIMIT_073.rs) | P1 | memory | GEN_SQL_DML | 1966811 | 4654996 | <span style="color:#dc2626">-136.68%</span> |
| 10 | [01010 JSON_EXTRACT_SET_003](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1010_JSON_EXTRACT_SET_003.rs) | P2 | memory | GEN_SQL_JSON | 1839200 | 4316035 | <span style="color:#dc2626">-134.67%</span> |
| 11 | [00825 WINDOW_PARTITION_SUM_038](crates/bench/sqlite_parity/cases/SQLITE_PARITY_825_WINDOW_PARTITION_SUM_038.rs) | P2 | memory | GEN_SQL_WINDOW | 1944138 | 4551070 | <span style="color:#dc2626">-134.09%</span> |
| 12 | [01024 JSON_EXTRACT_SET_017](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1024_JSON_EXTRACT_SET_017.rs) | P2 | memory | GEN_SQL_JSON | 1838929 | 4290567 | <span style="color:#dc2626">-133.32%</span> |
| 13 | [00827 WINDOW_PARTITION_SUM_040](crates/bench/sqlite_parity/cases/SQLITE_PARITY_827_WINDOW_PARTITION_SUM_040.rs) | P2 | memory | GEN_SQL_WINDOW | 1989253 | 4617776 | <span style="color:#dc2626">-132.14%</span> |
| 14 | [00840 WINDOW_PARTITION_SUM_053](crates/bench/sqlite_parity/cases/SQLITE_PARITY_840_WINDOW_PARTITION_SUM_053.rs) | P2 | memory | GEN_SQL_WINDOW | 1988572 | 4603168 | <span style="color:#dc2626">-131.48%</span> |
| 15 | [00759 CTE_RECURSIVE_MATRIX_052](crates/bench/sqlite_parity/cases/SQLITE_PARITY_759_CTE_RECURSIVE_MATRIX_052.rs) | P1 | memory | GEN_SQL_CTE | 1849208 | 4264719 | <span style="color:#dc2626">-130.62%</span> |
| 16 | [00379 SCALAR_NULL_COALESCE_038](crates/bench/sqlite_parity/cases/SQLITE_PARITY_379_SCALAR_NULL_COALESCE_038.rs) | P1 | memory | GEN_SQL_SCALAR | 1750211 | 4025746 | <span style="color:#dc2626">-130.01%</span> |
| 17 | [00697 JOIN_SUBQUERY_EXISTS_090](crates/bench/sqlite_parity/cases/SQLITE_PARITY_697_JOIN_SUBQUERY_EXISTS_090.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2076549 | 4758412 | <span style="color:#dc2626">-129.15%</span> |
| 18 | [00616 JOIN_SUBQUERY_EXISTS_009](crates/bench/sqlite_parity/cases/SQLITE_PARITY_616_JOIN_SUBQUERY_EXISTS_009.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2092298 | 4773130 | <span style="color:#dc2626">-128.13%</span> |
| 19 | [00022 TRANSACTION_COMMIT](crates/bench/sqlite_parity/cases/SQLITE_PARITY_022_TRANSACTION_COMMIT.rs) | P0 | memory | SQL_TRANSACTION | 1781610 | 4047647 | <span style="color:#dc2626">-127.19%</span> |
| 20 | [00939 CONSTRAINT_FK_SAVEPOINT_072](crates/bench/sqlite_parity/cases/SQLITE_PARITY_939_CONSTRAINT_FK_SAVEPOINT_072.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1963174 | 4431293 | <span style="color:#dc2626">-125.72%</span> |
| 21 | [00086 DATE_TIME_FUNCTIONS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_086_DATE_TIME_FUNCTIONS.rs) | P0 | memory | SQL_FUNCTIONS | 1712129 | 3851136 | <span style="color:#dc2626">-124.93%</span> |
| 22 | [00879 CONSTRAINT_FK_SAVEPOINT_012](crates/bench/sqlite_parity/cases/SQLITE_PARITY_879_CONSTRAINT_FK_SAVEPOINT_012.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1965428 | 4416836 | <span style="color:#dc2626">-124.73%</span> |
| 23 | [01046 JSON_EXTRACT_SET_039](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1046_JSON_EXTRACT_SET_039.rs) | P2 | memory | GEN_SQL_JSON | 1903962 | 4277953 | <span style="color:#dc2626">-124.69%</span> |
| 24 | [00773 CTE_RECURSIVE_MATRIX_066](crates/bench/sqlite_parity/cases/SQLITE_PARITY_773_CTE_RECURSIVE_MATRIX_066.rs) | P1 | memory | GEN_SQL_CTE | 1869096 | 4179406 | <span style="color:#dc2626">-123.61%</span> |
| 25 | [01093 INDEX_SCHEMA_PRAGMA_026](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1093_INDEX_SCHEMA_PRAGMA_026.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1962502 | 4380417 | <span style="color:#dc2626">-123.21%</span> |
| 26 | [01073 INDEX_SCHEMA_PRAGMA_006](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1073_INDEX_SCHEMA_PRAGMA_006.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 2024480 | 4518649 | <span style="color:#dc2626">-123.20%</span> |
| 27 | [00797 WINDOW_PARTITION_SUM_010](crates/bench/sqlite_parity/cases/SQLITE_PARITY_797_WINDOW_PARTITION_SUM_010.rs) | P2 | memory | GEN_SQL_WINDOW | 1991116 | 4415974 | <span style="color:#dc2626">-121.78%</span> |
| 28 | [00801 WINDOW_PARTITION_SUM_014](crates/bench/sqlite_parity/cases/SQLITE_PARITY_801_WINDOW_PARTITION_SUM_014.rs) | P2 | memory | GEN_SQL_WINDOW | 1940150 | 4295276 | <span style="color:#dc2626">-121.39%</span> |
| 29 | [00792 WINDOW_PARTITION_SUM_005](crates/bench/sqlite_parity/cases/SQLITE_PARITY_792_WINDOW_PARTITION_SUM_005.rs) | P2 | memory | GEN_SQL_WINDOW | 1933237 | 4276300 | <span style="color:#dc2626">-121.20%</span> |
| 30 | [00389 DML_WHERE_ORDER_LIMIT_002](crates/bench/sqlite_parity/cases/SQLITE_PARITY_389_DML_WHERE_ORDER_LIMIT_002.rs) | P1 | memory | GEN_SQL_DML | 1913029 | 4220976 | <span style="color:#dc2626">-120.64%</span> |
| 31 | [01118 INDEX_SCHEMA_PRAGMA_051](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1118_INDEX_SCHEMA_PRAGMA_051.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 2088040 | 4607086 | <span style="color:#dc2626">-120.64%</span> |
| 32 | [00768 CTE_RECURSIVE_MATRIX_061](crates/bench/sqlite_parity/cases/SQLITE_PARITY_768_CTE_RECURSIVE_MATRIX_061.rs) | P1 | memory | GEN_SQL_CTE | 1806487 | 3970101 | <span style="color:#dc2626">-119.77%</span> |
| 33 | [00938 CONSTRAINT_FK_SAVEPOINT_071](crates/bench/sqlite_parity/cases/SQLITE_PARITY_938_CONSTRAINT_FK_SAVEPOINT_071.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1965187 | 4308711 | <span style="color:#dc2626">-119.25%</span> |
| 34 | [00058 GROUP_BY_HAVING](crates/bench/sqlite_parity/cases/SQLITE_PARITY_058_GROUP_BY_HAVING.rs) | P0 | memory | SQL_AGGREGATE | 1915884 | 4198924 | <span style="color:#dc2626">-119.16%</span> |
| 35 | [00574 AGG_GROUP_HAVING_067](crates/bench/sqlite_parity/cases/SQLITE_PARITY_574_AGG_GROUP_HAVING_067.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1988582 | 4358214 | <span style="color:#dc2626">-119.16%</span> |
| 36 | [01050 JSON_EXTRACT_SET_043](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1050_JSON_EXTRACT_SET_043.rs) | P2 | memory | GEN_SQL_JSON | 1864086 | 4077143 | <span style="color:#dc2626">-118.72%</span> |
| 37 | [00401 DML_WHERE_ORDER_LIMIT_014](crates/bench/sqlite_parity/cases/SQLITE_PARITY_401_DML_WHERE_ORDER_LIMIT_014.rs) | P1 | memory | GEN_SQL_DML | 1861812 | 4063638 | <span style="color:#dc2626">-118.26%</span> |
| 38 | [01037 JSON_EXTRACT_SET_030](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1037_JSON_EXTRACT_SET_030.rs) | P2 | memory | GEN_SQL_JSON | 1803381 | 3934964 | <span style="color:#dc2626">-118.20%</span> |
| 39 | [00832 WINDOW_PARTITION_SUM_045](crates/bench/sqlite_parity/cases/SQLITE_PARITY_832_WINDOW_PARTITION_SUM_045.rs) | P2 | memory | GEN_SQL_WINDOW | 1975087 | 4306376 | <span style="color:#dc2626">-118.03%</span> |
| 40 | [00620 JOIN_SUBQUERY_EXISTS_013](crates/bench/sqlite_parity/cases/SQLITE_PARITY_620_JOIN_SUBQUERY_EXISTS_013.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1999152 | 4338037 | <span style="color:#dc2626">-116.99%</span> |
| 41 | [00739 CTE_RECURSIVE_MATRIX_032](crates/bench/sqlite_parity/cases/SQLITE_PARITY_739_CTE_RECURSIVE_MATRIX_032.rs) | P1 | memory | GEN_SQL_CTE | 1868905 | 4053759 | <span style="color:#dc2626">-116.91%</span> |
| 42 | [00618 JOIN_SUBQUERY_EXISTS_011](crates/bench/sqlite_parity/cases/SQLITE_PARITY_618_JOIN_SUBQUERY_EXISTS_011.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2075907 | 4498440 | <span style="color:#dc2626">-116.70%</span> |
| 43 | [00973 VIEW_TRIGGER_GENERATED_026](crates/bench/sqlite_parity/cases/SQLITE_PARITY_973_VIEW_TRIGGER_GENERATED_026.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2068904 | 4471569 | <span style="color:#dc2626">-116.13%</span> |
| 44 | [00357 SCALAR_CAST_TYPEOF_033](crates/bench/sqlite_parity/cases/SQLITE_PARITY_357_SCALAR_CAST_TYPEOF_033.rs) | P1 | memory | GEN_SQL_SCALAR | 1787933 | 3860974 | <span style="color:#dc2626">-115.95%</span> |
| 45 | [00640 JOIN_SUBQUERY_EXISTS_033](crates/bench/sqlite_parity/cases/SQLITE_PARITY_640_JOIN_SUBQUERY_EXISTS_033.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2105603 | 4541692 | <span style="color:#dc2626">-115.70%</span> |
| 46 | [00400 DML_WHERE_ORDER_LIMIT_013](crates/bench/sqlite_parity/cases/SQLITE_PARITY_400_DML_WHERE_ORDER_LIMIT_013.rs) | P1 | memory | GEN_SQL_DML | 1891568 | 4075840 | <span style="color:#dc2626">-115.47%</span> |
| 47 | [00780 CTE_RECURSIVE_MATRIX_073](crates/bench/sqlite_parity/cases/SQLITE_PARITY_780_CTE_RECURSIVE_MATRIX_073.rs) | P1 | memory | GEN_SQL_CTE | 1974395 | 4252355 | <span style="color:#dc2626">-115.38%</span> |
| 48 | [00453 DML_WHERE_ORDER_LIMIT_066](crates/bench/sqlite_parity/cases/SQLITE_PARITY_453_DML_WHERE_ORDER_LIMIT_066.rs) | P1 | memory | GEN_SQL_DML | 1950419 | 4191660 | <span style="color:#dc2626">-114.91%</span> |
| 49 | [00501 DML_WHERE_ORDER_LIMIT_114](crates/bench/sqlite_parity/cases/SQLITE_PARITY_501_DML_WHERE_ORDER_LIMIT_114.rs) | P1 | memory | GEN_SQL_DML | 1944579 | 4172353 | <span style="color:#dc2626">-114.56%</span> |
| 50 | [00740 CTE_RECURSIVE_MATRIX_033](crates/bench/sqlite_parity/cases/SQLITE_PARITY_740_CTE_RECURSIVE_MATRIX_033.rs) | P1 | memory | GEN_SQL_CTE | 1869878 | 4005457 | <span style="color:#dc2626">-114.21%</span> |
| 51 | [00750 CTE_RECURSIVE_MATRIX_043](crates/bench/sqlite_parity/cases/SQLITE_PARITY_750_CTE_RECURSIVE_MATRIX_043.rs) | P1 | memory | GEN_SQL_CTE | 1842095 | 3943470 | <span style="color:#dc2626">-114.08%</span> |
| 52 | [00393 DML_WHERE_ORDER_LIMIT_006](crates/bench/sqlite_parity/cases/SQLITE_PARITY_393_DML_WHERE_ORDER_LIMIT_006.rs) | P1 | memory | GEN_SQL_DML | 1922767 | 4114213 | <span style="color:#dc2626">-113.97%</span> |
| 53 | [01094 INDEX_SCHEMA_PRAGMA_027](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1094_INDEX_SCHEMA_PRAGMA_027.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1974966 | 4225895 | <span style="color:#dc2626">-113.97%</span> |
| 54 | [00363 SCALAR_NULL_COALESCE_034](crates/bench/sqlite_parity/cases/SQLITE_PARITY_363_SCALAR_NULL_COALESCE_034.rs) | P1 | memory | GEN_SQL_SCALAR | 1736836 | 3715048 | <span style="color:#dc2626">-113.90%</span> |
| 55 | [00977 VIEW_TRIGGER_GENERATED_030](crates/bench/sqlite_parity/cases/SQLITE_PARITY_977_VIEW_TRIGGER_GENERATED_030.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2147432 | 4591005 | <span style="color:#dc2626">-113.79%</span> |
| 56 | [00691 JOIN_SUBQUERY_EXISTS_084](crates/bench/sqlite_parity/cases/SQLITE_PARITY_691_JOIN_SUBQUERY_EXISTS_084.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2042113 | 4354268 | <span style="color:#dc2626">-113.22%</span> |
| 57 | [00625 JOIN_SUBQUERY_EXISTS_018](crates/bench/sqlite_parity/cases/SQLITE_PARITY_625_JOIN_SUBQUERY_EXISTS_018.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2083992 | 4442154 | <span style="color:#dc2626">-113.16%</span> |
| 58 | [01055 JSON_EXTRACT_SET_048](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1055_JSON_EXTRACT_SET_048.rs) | P2 | memory | GEN_SQL_JSON | 1858365 | 3957927 | <span style="color:#dc2626">-112.98%</span> |
| 59 | [00500 DML_WHERE_ORDER_LIMIT_113](crates/bench/sqlite_parity/cases/SQLITE_PARITY_500_DML_WHERE_ORDER_LIMIT_113.rs) | P1 | memory | GEN_SQL_DML | 1915704 | 4078666 | <span style="color:#dc2626">-112.91%</span> |
| 60 | [00690 JOIN_SUBQUERY_EXISTS_083](crates/bench/sqlite_parity/cases/SQLITE_PARITY_690_JOIN_SUBQUERY_EXISTS_083.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2048475 | 4360018 | <span style="color:#dc2626">-112.84%</span> |
| 61 | [00737 CTE_RECURSIVE_MATRIX_030](crates/bench/sqlite_parity/cases/SQLITE_PARITY_737_CTE_RECURSIVE_MATRIX_030.rs) | P1 | memory | GEN_SQL_CTE | 1899624 | 4032188 | <span style="color:#dc2626">-112.26%</span> |
| 62 | [00994 VIEW_TRIGGER_GENERATED_047](crates/bench/sqlite_parity/cases/SQLITE_PARITY_994_VIEW_TRIGGER_GENERATED_047.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1988812 | 4207470 | <span style="color:#dc2626">-111.56%</span> |
| 63 | [00824 WINDOW_PARTITION_SUM_037](crates/bench/sqlite_parity/cases/SQLITE_PARITY_824_WINDOW_PARTITION_SUM_037.rs) | P2 | memory | GEN_SQL_WINDOW | 1863937 | 3938872 | <span style="color:#dc2626">-111.32%</span> |
| 64 | [00946 CONSTRAINT_FK_SAVEPOINT_079](crates/bench/sqlite_parity/cases/SQLITE_PARITY_946_CONSTRAINT_FK_SAVEPOINT_079.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2012728 | 4242166 | <span style="color:#dc2626">-110.77%</span> |
| 65 | [00771 CTE_RECURSIVE_MATRIX_064](crates/bench/sqlite_parity/cases/SQLITE_PARITY_771_CTE_RECURSIVE_MATRIX_064.rs) | P1 | memory | GEN_SQL_CTE | 1851011 | 3892444 | <span style="color:#dc2626">-110.29%</span> |
| 66 | [00369 SCALAR_CAST_TYPEOF_036](crates/bench/sqlite_parity/cases/SQLITE_PARITY_369_SCALAR_CAST_TYPEOF_036.rs) | P1 | memory | GEN_SQL_SCALAR | 1693063 | 3558452 | <span style="color:#dc2626">-110.18%</span> |
| 67 | [01076 INDEX_SCHEMA_PRAGMA_009](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1076_INDEX_SCHEMA_PRAGMA_009.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 2008510 | 4218761 | <span style="color:#dc2626">-110.04%</span> |
| 68 | [01007 VIEW_TRIGGER_GENERATED_060](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1007_VIEW_TRIGGER_GENERATED_060.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1988101 | 4163306 | <span style="color:#dc2626">-109.41%</span> |
| 69 | [00932 CONSTRAINT_FK_SAVEPOINT_065](crates/bench/sqlite_parity/cases/SQLITE_PARITY_932_CONSTRAINT_FK_SAVEPOINT_065.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2001236 | 4186540 | <span style="color:#dc2626">-109.20%</span> |
| 70 | [00358 SCALAR_STRING_033](crates/bench/sqlite_parity/cases/SQLITE_PARITY_358_SCALAR_STRING_033.rs) | P1 | memory | GEN_SQL_SCALAR | 1773335 | 3706902 | <span style="color:#dc2626">-109.04%</span> |
| 71 | [00463 DML_WHERE_ORDER_LIMIT_076](crates/bench/sqlite_parity/cases/SQLITE_PARITY_463_DML_WHERE_ORDER_LIMIT_076.rs) | P1 | memory | GEN_SQL_DML | 1939409 | 4051555 | <span style="color:#dc2626">-108.91%</span> |
| 72 | [00878 CONSTRAINT_FK_SAVEPOINT_011](crates/bench/sqlite_parity/cases/SQLITE_PARITY_878_CONSTRAINT_FK_SAVEPOINT_011.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2039989 | 4256482 | <span style="color:#dc2626">-108.65%</span> |
| 73 | [00726 CTE_RECURSIVE_MATRIX_019](crates/bench/sqlite_parity/cases/SQLITE_PARITY_726_CTE_RECURSIVE_MATRIX_019.rs) | P1 | memory | GEN_SQL_CTE | 1882792 | 3921659 | <span style="color:#dc2626">-108.29%</span> |
| 74 | [00708 CTE_RECURSIVE_MATRIX_001](crates/bench/sqlite_parity/cases/SQLITE_PARITY_708_CTE_RECURSIVE_MATRIX_001.rs) | P1 | memory | GEN_SQL_CTE | 1843157 | 3838782 | <span style="color:#dc2626">-108.27%</span> |
| 75 | [00473 DML_WHERE_ORDER_LIMIT_086](crates/bench/sqlite_parity/cases/SQLITE_PARITY_473_DML_WHERE_ORDER_LIMIT_086.rs) | P1 | memory | GEN_SQL_DML | 1958314 | 4075590 | <span style="color:#dc2626">-108.12%</span> |
| 76 | [00889 CONSTRAINT_FK_SAVEPOINT_022](crates/bench/sqlite_parity/cases/SQLITE_PARITY_889_CONSTRAINT_FK_SAVEPOINT_022.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2008139 | 4179146 | <span style="color:#dc2626">-108.11%</span> |
| 77 | [01016 JSON_EXTRACT_SET_009](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1016_JSON_EXTRACT_SET_009.rs) | P2 | memory | GEN_SQL_JSON | 1917318 | 3974008 | <span style="color:#dc2626">-107.27%</span> |
| 78 | [00355 SCALAR_NULL_COALESCE_032](crates/bench/sqlite_parity/cases/SQLITE_PARITY_355_SCALAR_NULL_COALESCE_032.rs) | P1 | memory | GEN_SQL_SCALAR | 1715104 | 3552260 | <span style="color:#dc2626">-107.12%</span> |
| 79 | [01080 INDEX_SCHEMA_PRAGMA_013](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1080_INDEX_SCHEMA_PRAGMA_013.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 2013399 | 4169058 | <span style="color:#dc2626">-107.07%</span> |
| 80 | [00853 WINDOW_PARTITION_SUM_066](crates/bench/sqlite_parity/cases/SQLITE_PARITY_853_WINDOW_PARTITION_SUM_066.rs) | P2 | memory | GEN_SQL_WINDOW | 1903761 | 3937720 | <span style="color:#dc2626">-106.84%</span> |
| 81 | [00602 AGG_GROUP_HAVING_095](crates/bench/sqlite_parity/cases/SQLITE_PARITY_602_AGG_GROUP_HAVING_095.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2021835 | 4176350 | <span style="color:#dc2626">-106.56%</span> |
| 82 | [00018 DELETE_RETURNING](crates/bench/sqlite_parity/cases/SQLITE_PARITY_018_DELETE_RETURNING.rs) | P0 | memory | SQL_DELETE | 1974235 | 4077665 | <span style="color:#dc2626">-106.54%</span> |
| 83 | [00891 CONSTRAINT_FK_SAVEPOINT_024](crates/bench/sqlite_parity/cases/SQLITE_PARITY_891_CONSTRAINT_FK_SAVEPOINT_024.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2111775 | 4360459 | <span style="color:#dc2626">-106.48%</span> |
| 84 | [01095 INDEX_SCHEMA_PRAGMA_028](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1095_INDEX_SCHEMA_PRAGMA_028.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1965027 | 4050393 | <span style="color:#dc2626">-106.12%</span> |
| 85 | [00397 DML_WHERE_ORDER_LIMIT_010](crates/bench/sqlite_parity/cases/SQLITE_PARITY_397_DML_WHERE_ORDER_LIMIT_010.rs) | P1 | memory | GEN_SQL_DML | 2080336 | 4278444 | <span style="color:#dc2626">-105.66%</span> |
| 86 | [00972 VIEW_TRIGGER_GENERATED_025](crates/bench/sqlite_parity/cases/SQLITE_PARITY_972_VIEW_TRIGGER_GENERATED_025.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2048686 | 4211848 | <span style="color:#dc2626">-105.59%</span> |
| 87 | [00629 JOIN_SUBQUERY_EXISTS_022](crates/bench/sqlite_parity/cases/SQLITE_PARITY_629_JOIN_SUBQUERY_EXISTS_022.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2018368 | 4148619 | <span style="color:#dc2626">-105.54%</span> |
| 88 | [00359 SCALAR_NULL_COALESCE_033](crates/bench/sqlite_parity/cases/SQLITE_PARITY_359_SCALAR_NULL_COALESCE_033.rs) | P1 | memory | GEN_SQL_SCALAR | 1679647 | 3451119 | <span style="color:#dc2626">-105.47%</span> |
| 89 | [00954 VIEW_TRIGGER_GENERATED_007](crates/bench/sqlite_parity/cases/SQLITE_PARITY_954_VIEW_TRIGGER_GENERATED_007.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2029539 | 4163777 | <span style="color:#dc2626">-105.16%</span> |
| 90 | [00002 LITERALS_AND_TYPEOF](crates/bench/sqlite_parity/cases/SQLITE_PARITY_002_LITERALS_AND_TYPEOF.rs) | P0 | memory | SQL_EXPRESSIONS | 1804254 | 3690452 | <span style="color:#dc2626">-104.54%</span> |
| 91 | [00663 JOIN_SUBQUERY_EXISTS_056](crates/bench/sqlite_parity/cases/SQLITE_PARITY_663_JOIN_SUBQUERY_EXISTS_056.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2100704 | 4285918 | <span style="color:#dc2626">-104.02%</span> |
| 92 | [00772 CTE_RECURSIVE_MATRIX_065](crates/bench/sqlite_parity/cases/SQLITE_PARITY_772_CTE_RECURSIVE_MATRIX_065.rs) | P1 | memory | GEN_SQL_CTE | 1879466 | 3830918 | <span style="color:#dc2626">-103.83%</span> |
| 93 | [00341 SCALAR_CAST_TYPEOF_029](crates/bench/sqlite_parity/cases/SQLITE_PARITY_341_SCALAR_CAST_TYPEOF_029.rs) | P1 | memory | GEN_SQL_SCALAR | 1707280 | 3473221 | <span style="color:#dc2626">-103.44%</span> |
| 94 | [00766 CTE_RECURSIVE_MATRIX_059](crates/bench/sqlite_parity/cases/SQLITE_PARITY_766_CTE_RECURSIVE_MATRIX_059.rs) | P1 | memory | GEN_SQL_CTE | 1902700 | 3866495 | <span style="color:#dc2626">-103.21%</span> |
| 95 | [01081 INDEX_SCHEMA_PRAGMA_014](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1081_INDEX_SCHEMA_PRAGMA_014.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 2080566 | 4225784 | <span style="color:#dc2626">-103.11%</span> |
| 96 | [00593 AGG_GROUP_HAVING_086](crates/bench/sqlite_parity/cases/SQLITE_PARITY_593_AGG_GROUP_HAVING_086.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1974305 | 4007342 | <span style="color:#dc2626">-102.97%</span> |
| 97 | [00466 DML_WHERE_ORDER_LIMIT_079](crates/bench/sqlite_parity/cases/SQLITE_PARITY_466_DML_WHERE_ORDER_LIMIT_079.rs) | P1 | memory | GEN_SQL_DML | 1837366 | 3728794 | <span style="color:#dc2626">-102.94%</span> |
| 98 | [00108 DOT_MODE_CSV_AND_QUOTE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_108_DOT_MODE_CSV_AND_QUOTE.rs) | P0 | memory | CLI_DOT_COMMAND | 1822547 | 3697415 | <span style="color:#dc2626">-102.87%</span> |
| 99 | [01008 JSON_EXTRACT_SET_001](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1008_JSON_EXTRACT_SET_001.rs) | P2 | memory | GEN_SQL_JSON | 1858106 | 3763770 | <span style="color:#dc2626">-102.56%</span> |
| 100 | [00725 CTE_RECURSIVE_MATRIX_018](crates/bench/sqlite_parity/cases/SQLITE_PARITY_725_CTE_RECURSIVE_MATRIX_018.rs) | P1 | memory | GEN_SQL_CTE | 1916075 | 3873799 | <span style="color:#dc2626">-102.17%</span> |
| 101 | [00561 AGG_GROUP_HAVING_054](crates/bench/sqlite_parity/cases/SQLITE_PARITY_561_AGG_GROUP_HAVING_054.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1989463 | 4021669 | <span style="color:#dc2626">-102.15%</span> |
| 102 | [00438 DML_WHERE_ORDER_LIMIT_051](crates/bench/sqlite_parity/cases/SQLITE_PARITY_438_DML_WHERE_ORDER_LIMIT_051.rs) | P1 | memory | GEN_SQL_DML | 1947745 | 3937128 | <span style="color:#dc2626">-102.14%</span> |
| 103 | [00985 VIEW_TRIGGER_GENERATED_038](crates/bench/sqlite_parity/cases/SQLITE_PARITY_985_VIEW_TRIGGER_GENERATED_038.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2097127 | 4238819 | <span style="color:#dc2626">-102.13%</span> |
| 104 | [00643 JOIN_SUBQUERY_EXISTS_036](crates/bench/sqlite_parity/cases/SQLITE_PARITY_643_JOIN_SUBQUERY_EXISTS_036.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2098480 | 4232777 | <span style="color:#dc2626">-101.71%</span> |
| 105 | [00786 CTE_RECURSIVE_MATRIX_079](crates/bench/sqlite_parity/cases/SQLITE_PARITY_786_CTE_RECURSIVE_MATRIX_079.rs) | P1 | memory | GEN_SQL_CTE | 1847024 | 3724536 | <span style="color:#dc2626">-101.65%</span> |
| 106 | [00778 CTE_RECURSIVE_MATRIX_071](crates/bench/sqlite_parity/cases/SQLITE_PARITY_778_CTE_RECURSIVE_MATRIX_071.rs) | P1 | memory | GEN_SQL_CTE | 2211103 | 4456161 | <span style="color:#dc2626">-101.54%</span> |
| 107 | [01082 INDEX_SCHEMA_PRAGMA_015](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1082_INDEX_SCHEMA_PRAGMA_015.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 2041842 | 4113953 | <span style="color:#dc2626">-101.48%</span> |
| 108 | [00431 DML_WHERE_ORDER_LIMIT_044](crates/bench/sqlite_parity/cases/SQLITE_PARITY_431_DML_WHERE_ORDER_LIMIT_044.rs) | P1 | memory | GEN_SQL_DML | 1941212 | 3904957 | <span style="color:#dc2626">-101.16%</span> |
| 109 | [00794 WINDOW_PARTITION_SUM_007](crates/bench/sqlite_parity/cases/SQLITE_PARITY_794_WINDOW_PARTITION_SUM_007.rs) | P2 | memory | GEN_SQL_WINDOW | 1984514 | 3989227 | <span style="color:#dc2626">-101.02%</span> |
| 110 | [00981 VIEW_TRIGGER_GENERATED_034](crates/bench/sqlite_parity/cases/SQLITE_PARITY_981_VIEW_TRIGGER_GENERATED_034.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2187549 | 4392840 | <span style="color:#dc2626">-100.81%</span> |
| 111 | [00919 CONSTRAINT_FK_SAVEPOINT_052](crates/bench/sqlite_parity/cases/SQLITE_PARITY_919_CONSTRAINT_FK_SAVEPOINT_052.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2133386 | 4282151 | <span style="color:#dc2626">-100.72%</span> |
| 112 | [00995 VIEW_TRIGGER_GENERATED_048](crates/bench/sqlite_parity/cases/SQLITE_PARITY_995_VIEW_TRIGGER_GENERATED_048.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2025893 | 4062255 | <span style="color:#dc2626">-100.52%</span> |
| 113 | [00368 SCALAR_ARITH_036](crates/bench/sqlite_parity/cases/SQLITE_PARITY_368_SCALAR_ARITH_036.rs) | P1 | memory | GEN_SQL_SCALAR | 1696409 | 3400503 | <span style="color:#dc2626">-100.45%</span> |
| 114 | [01034 JSON_EXTRACT_SET_027](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1034_JSON_EXTRACT_SET_027.rs) | P2 | memory | GEN_SQL_JSON | 1979845 | 3960853 | <span style="color:#dc2626">-100.06%</span> |
| 115 | [00430 DML_WHERE_ORDER_LIMIT_043](crates/bench/sqlite_parity/cases/SQLITE_PARITY_430_DML_WHERE_ORDER_LIMIT_043.rs) | P1 | memory | GEN_SQL_DML | 1947744 | 3889167 | <span style="color:#dc2626">-99.68%</span> |
| 116 | [00917 CONSTRAINT_FK_SAVEPOINT_050](crates/bench/sqlite_parity/cases/SQLITE_PARITY_917_CONSTRAINT_FK_SAVEPOINT_050.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1987069 | 3956014 | <span style="color:#dc2626">-99.09%</span> |
| 117 | [00447 DML_WHERE_ORDER_LIMIT_060](crates/bench/sqlite_parity/cases/SQLITE_PARITY_447_DML_WHERE_ORDER_LIMIT_060.rs) | P1 | memory | GEN_SQL_DML | 1977191 | 3929434 | <span style="color:#dc2626">-98.74%</span> |
| 118 | [00032 UNIQUE_INDEX_FAILURE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_032_UNIQUE_INDEX_FAILURE.rs) | P0 | memory | SQL_INDEX_NEGATIVE | 1820143 | 3614889 | <span style="color:#dc2626">-98.60%</span> |
| 119 | [01071 INDEX_SCHEMA_PRAGMA_004](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1071_INDEX_SCHEMA_PRAGMA_004.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 2065637 | 4102351 | <span style="color:#dc2626">-98.60%</span> |
| 120 | [00989 VIEW_TRIGGER_GENERATED_042](crates/bench/sqlite_parity/cases/SQLITE_PARITY_989_VIEW_TRIGGER_GENERATED_042.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2075647 | 4119764 | <span style="color:#dc2626">-98.48%</span> |
| 121 | [00147 DOT_IMPORT_CSV_TEMPFILE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_147_DOT_IMPORT_CSV_TEMPFILE.rs) | P1 | tempfile | CLI_TEMPFILE | 2174414 | 4315594 | <span style="color:#dc2626">-98.47%</span> |
| 122 | [01072 INDEX_SCHEMA_PRAGMA_005](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1072_INDEX_SCHEMA_PRAGMA_005.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1991678 | 3952187 | <span style="color:#dc2626">-98.44%</span> |
| 123 | [00920 CONSTRAINT_FK_SAVEPOINT_053](crates/bench/sqlite_parity/cases/SQLITE_PARITY_920_CONSTRAINT_FK_SAVEPOINT_053.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1999773 | 3966053 | <span style="color:#dc2626">-98.33%</span> |
| 124 | [00603 AGG_GROUP_HAVING_096](crates/bench/sqlite_parity/cases/SQLITE_PARITY_603_AGG_GROUP_HAVING_096.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2030230 | 4021569 | <span style="color:#dc2626">-98.08%</span> |
| 125 | [00673 JOIN_SUBQUERY_EXISTS_066](crates/bench/sqlite_parity/cases/SQLITE_PARITY_673_JOIN_SUBQUERY_EXISTS_066.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2200012 | 4357674 | <span style="color:#dc2626">-98.08%</span> |
| 126 | [00811 WINDOW_PARTITION_SUM_024](crates/bench/sqlite_parity/cases/SQLITE_PARITY_811_WINDOW_PARTITION_SUM_024.rs) | P2 | memory | GEN_SQL_WINDOW | 2018879 | 3995027 | <span style="color:#dc2626">-97.88%</span> |
| 127 | [00021 UPSERT_DO_NOTHING](crates/bench/sqlite_parity/cases/SQLITE_PARITY_021_UPSERT_DO_NOTHING.rs) | P0 | memory | SQL_UPSERT | 1837226 | 3634346 | <span style="color:#dc2626">-97.82%</span> |
| 128 | [00575 AGG_GROUP_HAVING_068](crates/bench/sqlite_parity/cases/SQLITE_PARITY_575_AGG_GROUP_HAVING_068.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1994763 | 3945434 | <span style="color:#dc2626">-97.79%</span> |
| 129 | [00668 JOIN_SUBQUERY_EXISTS_061](crates/bench/sqlite_parity/cases/SQLITE_PARITY_668_JOIN_SUBQUERY_EXISTS_061.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2093300 | 4139932 | <span style="color:#dc2626">-97.77%</span> |
| 130 | [01120 INDEX_SCHEMA_PRAGMA_053](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1120_INDEX_SCHEMA_PRAGMA_053.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 2027115 | 3992323 | <span style="color:#dc2626">-96.95%</span> |
| 131 | [00053 SELECT_WHERE_ORDER_LIMIT_OFFSET](crates/bench/sqlite_parity/cases/SQLITE_PARITY_053_SELECT_WHERE_ORDER_LIMIT_OFFSET.rs) | P0 | memory | SQL_SELECT | 1844329 | 3630007 | <span style="color:#dc2626">-96.82%</span> |
| 132 | [00959 VIEW_TRIGGER_GENERATED_012](crates/bench/sqlite_parity/cases/SQLITE_PARITY_959_VIEW_TRIGGER_GENERATED_012.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2070306 | 4071833 | <span style="color:#dc2626">-96.68%</span> |
| 133 | [00382 SCALAR_STRING_039](crates/bench/sqlite_parity/cases/SQLITE_PARITY_382_SCALAR_STRING_039.rs) | P1 | memory | GEN_SQL_SCALAR | 1800597 | 3540909 | <span style="color:#dc2626">-96.65%</span> |
| 134 | [00516 AGG_GROUP_HAVING_009](crates/bench/sqlite_parity/cases/SQLITE_PARITY_516_AGG_GROUP_HAVING_009.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2001226 | 3933071 | <span style="color:#dc2626">-96.53%</span> |
| 135 | [00767 CTE_RECURSIVE_MATRIX_060](crates/bench/sqlite_parity/cases/SQLITE_PARITY_767_CTE_RECURSIVE_MATRIX_060.rs) | P1 | memory | GEN_SQL_CTE | 1840191 | 3615791 | <span style="color:#dc2626">-96.49%</span> |
| 136 | [01030 JSON_EXTRACT_SET_023](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1030_JSON_EXTRACT_SET_023.rs) | P2 | memory | GEN_SQL_JSON | 1921405 | 3764311 | <span style="color:#dc2626">-95.91%</span> |
| 137 | [00647 JOIN_SUBQUERY_EXISTS_040](crates/bench/sqlite_parity/cases/SQLITE_PARITY_647_JOIN_SUBQUERY_EXISTS_040.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2005745 | 3929464 | <span style="color:#dc2626">-95.91%</span> |
| 138 | [00790 WINDOW_PARTITION_SUM_003](crates/bench/sqlite_parity/cases/SQLITE_PARITY_790_WINDOW_PARTITION_SUM_003.rs) | P2 | memory | GEN_SQL_WINDOW | 1990195 | 3895169 | <span style="color:#dc2626">-95.72%</span> |
| 139 | [00467 DML_WHERE_ORDER_LIMIT_080](crates/bench/sqlite_parity/cases/SQLITE_PARITY_467_DML_WHERE_ORDER_LIMIT_080.rs) | P1 | memory | GEN_SQL_DML | 2010965 | 3932871 | <span style="color:#dc2626">-95.57%</span> |
| 140 | [00650 JOIN_SUBQUERY_EXISTS_043](crates/bench/sqlite_parity/cases/SQLITE_PARITY_650_JOIN_SUBQUERY_EXISTS_043.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1952984 | 3815178 | <span style="color:#dc2626">-95.35%</span> |
| 141 | [00340 SCALAR_ARITH_029](crates/bench/sqlite_parity/cases/SQLITE_PARITY_340_SCALAR_ARITH_029.rs) | P1 | memory | GEN_SQL_SCALAR | 1708071 | 3331533 | <span style="color:#dc2626">-95.05%</span> |
| 142 | [00371 SCALAR_NULL_COALESCE_036](crates/bench/sqlite_parity/cases/SQLITE_PARITY_371_SCALAR_NULL_COALESCE_036.rs) | P1 | memory | GEN_SQL_SCALAR | 1796068 | 3500061 | <span style="color:#dc2626">-94.87%</span> |
| 143 | [00390 DML_WHERE_ORDER_LIMIT_003](crates/bench/sqlite_parity/cases/SQLITE_PARITY_390_DML_WHERE_ORDER_LIMIT_003.rs) | P1 | memory | GEN_SQL_DML | 1908640 | 3714918 | <span style="color:#dc2626">-94.64%</span> |
| 144 | [00556 AGG_GROUP_HAVING_049](crates/bench/sqlite_parity/cases/SQLITE_PARITY_556_AGG_GROUP_HAVING_049.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2126513 | 4138769 | <span style="color:#dc2626">-94.63%</span> |
| 145 | [00670 JOIN_SUBQUERY_EXISTS_063](crates/bench/sqlite_parity/cases/SQLITE_PARITY_670_JOIN_SUBQUERY_EXISTS_063.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2349946 | 4570517 | <span style="color:#dc2626">-94.49%</span> |
| 146 | [00701 JOIN_SUBQUERY_EXISTS_094](crates/bench/sqlite_parity/cases/SQLITE_PARITY_701_JOIN_SUBQUERY_EXISTS_094.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2054767 | 3990148 | <span style="color:#dc2626">-94.19%</span> |
| 147 | [00144 DOT_PROMPT](crates/bench/sqlite_parity/cases/SQLITE_PARITY_144_DOT_PROMPT.rs) | P0 | memory | CLI_DOT_COMMAND | 1750913 | 3399310 | <span style="color:#dc2626">-94.14%</span> |
| 148 | [01049 JSON_EXTRACT_SET_042](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1049_JSON_EXTRACT_SET_042.rs) | P2 | memory | GEN_SQL_JSON | 1864798 | 3620319 | <span style="color:#dc2626">-94.14%</span> |
| 149 | [01087 INDEX_SCHEMA_PRAGMA_020](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1087_INDEX_SCHEMA_PRAGMA_020.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 2034599 | 3944863 | <span style="color:#dc2626">-93.89%</span> |
| 150 | [00676 JOIN_SUBQUERY_EXISTS_069](crates/bench/sqlite_parity/cases/SQLITE_PARITY_676_JOIN_SUBQUERY_EXISTS_069.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2038135 | 3950644 | <span style="color:#dc2626">-93.84%</span> |
| 151 | [00775 CTE_RECURSIVE_MATRIX_068](crates/bench/sqlite_parity/cases/SQLITE_PARITY_775_CTE_RECURSIVE_MATRIX_068.rs) | P1 | memory | GEN_SQL_CTE | 2077861 | 4017290 | <span style="color:#dc2626">-93.34%</span> |
| 152 | [01100 INDEX_SCHEMA_PRAGMA_033](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1100_INDEX_SCHEMA_PRAGMA_033.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 2224528 | 4300395 | <span style="color:#dc2626">-93.32%</span> |
| 153 | [00313 SCALAR_CAST_TYPEOF_022](crates/bench/sqlite_parity/cases/SQLITE_PARITY_313_SCALAR_CAST_TYPEOF_022.rs) | P1 | memory | GEN_SQL_SCALAR | 1761382 | 3403318 | <span style="color:#dc2626">-93.22%</span> |
| 154 | [00033 PARTIAL_INDEX](crates/bench/sqlite_parity/cases/SQLITE_PARITY_033_PARTIAL_INDEX.rs) | P0 | memory | SQL_INDEX | 1789335 | 3456499 | <span style="color:#dc2626">-93.17%</span> |
| 155 | [01110 INDEX_SCHEMA_PRAGMA_043](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1110_INDEX_SCHEMA_PRAGMA_043.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 2069665 | 3992704 | <span style="color:#dc2626">-92.92%</span> |
| 156 | [00571 AGG_GROUP_HAVING_064](crates/bench/sqlite_parity/cases/SQLITE_PARITY_571_AGG_GROUP_HAVING_064.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2061670 | 3973386 | <span style="color:#dc2626">-92.73%</span> |
| 157 | [01017 JSON_EXTRACT_SET_010](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1017_JSON_EXTRACT_SET_010.rs) | P2 | memory | GEN_SQL_JSON | 1832287 | 3528466 | <span style="color:#dc2626">-92.57%</span> |
| 158 | [00601 AGG_GROUP_HAVING_094](crates/bench/sqlite_parity/cases/SQLITE_PARITY_601_AGG_GROUP_HAVING_094.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2534935 | 4881294 | <span style="color:#dc2626">-92.56%</span> |
| 159 | [00533 AGG_GROUP_HAVING_026](crates/bench/sqlite_parity/cases/SQLITE_PARITY_533_AGG_GROUP_HAVING_026.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2004763 | 3859171 | <span style="color:#dc2626">-92.50%</span> |
| 160 | [00823 WINDOW_PARTITION_SUM_036](crates/bench/sqlite_parity/cases/SQLITE_PARITY_823_WINDOW_PARTITION_SUM_036.rs) | P2 | memory | GEN_SQL_WINDOW | 2038526 | 3920146 | <span style="color:#dc2626">-92.30%</span> |
| 161 | [00645 JOIN_SUBQUERY_EXISTS_038](crates/bench/sqlite_parity/cases/SQLITE_PARITY_645_JOIN_SUBQUERY_EXISTS_038.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2005915 | 3855564 | <span style="color:#dc2626">-92.21%</span> |
| 162 | [00084 CORE_FORMAT_QUOTE_HEX](crates/bench/sqlite_parity/cases/SQLITE_PARITY_084_CORE_FORMAT_QUOTE_HEX.rs) | P0 | memory | SQL_FUNCTIONS | 1733439 | 3330400 | <span style="color:#dc2626">-92.13%</span> |
| 163 | [00523 AGG_GROUP_HAVING_016](crates/bench/sqlite_parity/cases/SQLITE_PARITY_523_AGG_GROUP_HAVING_016.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2109030 | 4050873 | <span style="color:#dc2626">-92.07%</span> |
| 164 | [00372 SCALAR_ARITH_037](crates/bench/sqlite_parity/cases/SQLITE_PARITY_372_SCALAR_ARITH_037.rs) | P1 | memory | GEN_SQL_SCALAR | 1741514 | 3343104 | <span style="color:#dc2626">-91.97%</span> |
| 165 | [00257 SCALAR_CAST_TYPEOF_008](crates/bench/sqlite_parity/cases/SQLITE_PARITY_257_SCALAR_CAST_TYPEOF_008.rs) | P1 | memory | GEN_SQL_SCALAR | 1856021 | 3562600 | <span style="color:#dc2626">-91.95%</span> |
| 166 | [00112 DOT_SEPARATOR](crates/bench/sqlite_parity/cases/SQLITE_PARITY_112_DOT_SEPARATOR.rs) | P0 | memory | CLI_DOT_COMMAND | 1655041 | 3176048 | <span style="color:#dc2626">-91.90%</span> |
| 167 | [00462 DML_WHERE_ORDER_LIMIT_075](crates/bench/sqlite_parity/cases/SQLITE_PARITY_462_DML_WHERE_ORDER_LIMIT_075.rs) | P1 | memory | GEN_SQL_DML | 1939119 | 3712583 | <span style="color:#dc2626">-91.46%</span> |
| 168 | [01031 JSON_EXTRACT_SET_024](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1031_JSON_EXTRACT_SET_024.rs) | P2 | memory | GEN_SQL_JSON | 1917708 | 3670444 | <span style="color:#dc2626">-91.40%</span> |
| 169 | [00630 JOIN_SUBQUERY_EXISTS_023](crates/bench/sqlite_parity/cases/SQLITE_PARITY_630_JOIN_SUBQUERY_EXISTS_023.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2120081 | 4053018 | <span style="color:#dc2626">-91.17%</span> |
| 170 | [00774 CTE_RECURSIVE_MATRIX_067](crates/bench/sqlite_parity/cases/SQLITE_PARITY_774_CTE_RECURSIVE_MATRIX_067.rs) | P1 | memory | GEN_SQL_CTE | 2100583 | 4006029 | <span style="color:#dc2626">-90.71%</span> |
| 171 | [00684 JOIN_SUBQUERY_EXISTS_077](crates/bench/sqlite_parity/cases/SQLITE_PARITY_684_JOIN_SUBQUERY_EXISTS_077.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1986699 | 3787585 | <span style="color:#dc2626">-90.65%</span> |
| 172 | [00652 JOIN_SUBQUERY_EXISTS_045](crates/bench/sqlite_parity/cases/SQLITE_PARITY_652_JOIN_SUBQUERY_EXISTS_045.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2016675 | 3843251 | <span style="color:#dc2626">-90.57%</span> |
| 173 | [00111 DOT_HEADERS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_111_DOT_HEADERS.rs) | P0 | memory | CLI_DOT_COMMAND | 1749590 | 3328036 | <span style="color:#dc2626">-90.22%</span> |
| 174 | [01053 JSON_EXTRACT_SET_046](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1053_JSON_EXTRACT_SET_046.rs) | P2 | memory | GEN_SQL_JSON | 1860650 | 3535939 | <span style="color:#dc2626">-90.04%</span> |
| 175 | [00421 DML_WHERE_ORDER_LIMIT_034](crates/bench/sqlite_parity/cases/SQLITE_PARITY_421_DML_WHERE_ORDER_LIMIT_034.rs) | P1 | memory | GEN_SQL_DML | 2094772 | 3980110 | <span style="color:#dc2626">-90.00%</span> |
| 176 | [00386 SCALAR_STRING_040](crates/bench/sqlite_parity/cases/SQLITE_PARITY_386_SCALAR_STRING_040.rs) | P1 | memory | GEN_SQL_SCALAR | 1801418 | 3415140 | <span style="color:#dc2626">-89.58%</span> |
| 177 | [00979 VIEW_TRIGGER_GENERATED_032](crates/bench/sqlite_parity/cases/SQLITE_PARITY_979_VIEW_TRIGGER_GENERATED_032.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2051311 | 3885661 | <span style="color:#dc2626">-89.42%</span> |
| 178 | [00258 SCALAR_STRING_008](crates/bench/sqlite_parity/cases/SQLITE_PARITY_258_SCALAR_STRING_008.rs) | P1 | memory | GEN_SQL_SCALAR | 1781170 | 3371327 | <span style="color:#dc2626">-89.28%</span> |
| 179 | [00048 PRAGMA_USER_VERSION](crates/bench/sqlite_parity/cases/SQLITE_PARITY_048_PRAGMA_USER_VERSION.rs) | P0 | memory | SQL_PRAGMA | 1753938 | 3318859 | <span style="color:#dc2626">-89.22%</span> |
| 180 | [00810 WINDOW_PARTITION_SUM_023](crates/bench/sqlite_parity/cases/SQLITE_PARITY_810_WINDOW_PARTITION_SUM_023.rs) | P2 | memory | GEN_SQL_WINDOW | 2346028 | 4437284 | <span style="color:#dc2626">-89.14%</span> |
| 181 | [00481 DML_WHERE_ORDER_LIMIT_094](crates/bench/sqlite_parity/cases/SQLITE_PARITY_481_DML_WHERE_ORDER_LIMIT_094.rs) | P1 | memory | GEN_SQL_DML | 1919160 | 3626601 | <span style="color:#dc2626">-88.97%</span> |
| 182 | [00255 SCALAR_NULL_COALESCE_007](crates/bench/sqlite_parity/cases/SQLITE_PARITY_255_SCALAR_NULL_COALESCE_007.rs) | P1 | memory | GEN_SQL_SCALAR | 1790397 | 3382730 | <span style="color:#dc2626">-88.94%</span> |
| 183 | [00744 CTE_RECURSIVE_MATRIX_037](crates/bench/sqlite_parity/cases/SQLITE_PARITY_744_CTE_RECURSIVE_MATRIX_037.rs) | P1 | memory | GEN_SQL_CTE | 1883774 | 3557981 | <span style="color:#dc2626">-88.88%</span> |
| 184 | [00321 SCALAR_CAST_TYPEOF_024](crates/bench/sqlite_parity/cases/SQLITE_PARITY_321_SCALAR_CAST_TYPEOF_024.rs) | P1 | memory | GEN_SQL_SCALAR | 1798252 | 3395994 | <span style="color:#dc2626">-88.85%</span> |
| 185 | [00415 DML_WHERE_ORDER_LIMIT_028](crates/bench/sqlite_parity/cases/SQLITE_PARITY_415_DML_WHERE_ORDER_LIMIT_028.rs) | P1 | memory | GEN_SQL_DML | 2212996 | 4173385 | <span style="color:#dc2626">-88.59%</span> |
| 186 | [00803 WINDOW_PARTITION_SUM_016](crates/bench/sqlite_parity/cases/SQLITE_PARITY_803_WINDOW_PARTITION_SUM_016.rs) | P2 | memory | GEN_SQL_WINDOW | 1891678 | 3566928 | <span style="color:#dc2626">-88.56%</span> |
| 187 | [01096 INDEX_SCHEMA_PRAGMA_029](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1096_INDEX_SCHEMA_PRAGMA_029.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 2019190 | 3806512 | <span style="color:#dc2626">-88.52%</span> |
| 188 | [00376 SCALAR_ARITH_038](crates/bench/sqlite_parity/cases/SQLITE_PARITY_376_SCALAR_ARITH_038.rs) | P1 | memory | GEN_SQL_SCALAR | 1818551 | 3422705 | <span style="color:#dc2626">-88.21%</span> |
| 189 | [01029 JSON_EXTRACT_SET_022](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1029_JSON_EXTRACT_SET_022.rs) | P2 | memory | GEN_SQL_JSON | 1854208 | 3481837 | <span style="color:#dc2626">-87.78%</span> |
| 190 | [00425 DML_WHERE_ORDER_LIMIT_038](crates/bench/sqlite_parity/cases/SQLITE_PARITY_425_DML_WHERE_ORDER_LIMIT_038.rs) | P1 | memory | GEN_SQL_DML | 1937024 | 3635147 | <span style="color:#dc2626">-87.67%</span> |
| 191 | [00377 SCALAR_CAST_TYPEOF_038](crates/bench/sqlite_parity/cases/SQLITE_PARITY_377_SCALAR_CAST_TYPEOF_038.rs) | P1 | memory | GEN_SQL_SCALAR | 1760170 | 3301405 | <span style="color:#dc2626">-87.56%</span> |
| 192 | [01074 INDEX_SCHEMA_PRAGMA_007](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1074_INDEX_SCHEMA_PRAGMA_007.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 2040500 | 3814176 | <span style="color:#dc2626">-86.92%</span> |
| 193 | [01000 VIEW_TRIGGER_GENERATED_053](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1000_VIEW_TRIGGER_GENERATED_053.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2283390 | 4267653 | <span style="color:#dc2626">-86.90%</span> |
| 194 | [00485 DML_WHERE_ORDER_LIMIT_098](crates/bench/sqlite_parity/cases/SQLITE_PARITY_485_DML_WHERE_ORDER_LIMIT_098.rs) | P1 | memory | GEN_SQL_DML | 1960589 | 3663851 | <span style="color:#dc2626">-86.88%</span> |
| 195 | [00942 CONSTRAINT_FK_SAVEPOINT_075](crates/bench/sqlite_parity/cases/SQLITE_PARITY_942_CONSTRAINT_FK_SAVEPOINT_075.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2019190 | 3770663 | <span style="color:#dc2626">-86.74%</span> |
| 196 | [00553 AGG_GROUP_HAVING_046](crates/bench/sqlite_parity/cases/SQLITE_PARITY_553_AGG_GROUP_HAVING_046.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2214319 | 4131827 | <span style="color:#dc2626">-86.60%</span> |
| 197 | [00074 NOT_INDEXED](crates/bench/sqlite_parity/cases/SQLITE_PARITY_074_NOT_INDEXED.rs) | P0 | memory | SQL_INDEX | 1828569 | 3409139 | <span style="color:#dc2626">-86.44%</span> |
| 198 | [00424 DML_WHERE_ORDER_LIMIT_037](crates/bench/sqlite_parity/cases/SQLITE_PARITY_424_DML_WHERE_ORDER_LIMIT_037.rs) | P1 | memory | GEN_SQL_DML | 1910073 | 3560626 | <span style="color:#dc2626">-86.41%</span> |
| 199 | [00351 SCALAR_NULL_COALESCE_031](crates/bench/sqlite_parity/cases/SQLITE_PARITY_351_SCALAR_NULL_COALESCE_031.rs) | P1 | memory | GEN_SQL_SCALAR | 1736415 | 3234700 | <span style="color:#dc2626">-86.29%</span> |
| 200 | [00907 CONSTRAINT_FK_SAVEPOINT_040](crates/bench/sqlite_parity/cases/SQLITE_PARITY_907_CONSTRAINT_FK_SAVEPOINT_040.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1959738 | 3645226 | <span style="color:#dc2626">-86.01%</span> |
| 201 | [01036 JSON_EXTRACT_SET_029](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1036_JSON_EXTRACT_SET_029.rs) | P2 | memory | GEN_SQL_JSON | 1891248 | 3516432 | <span style="color:#dc2626">-85.93%</span> |
| 202 | [00952 VIEW_TRIGGER_GENERATED_005](crates/bench/sqlite_parity/cases/SQLITE_PARITY_952_VIEW_TRIGGER_GENERATED_005.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2013018 | 3739434 | <span style="color:#dc2626">-85.76%</span> |
| 203 | [00456 DML_WHERE_ORDER_LIMIT_069](crates/bench/sqlite_parity/cases/SQLITE_PARITY_456_DML_WHERE_ORDER_LIMIT_069.rs) | P1 | memory | GEN_SQL_DML | 1904173 | 3536310 | <span style="color:#dc2626">-85.71%</span> |
| 204 | [00657 JOIN_SUBQUERY_EXISTS_050](crates/bench/sqlite_parity/cases/SQLITE_PARITY_657_JOIN_SUBQUERY_EXISTS_050.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2591883 | 4813106 | <span style="color:#dc2626">-85.70%</span> |
| 205 | [00702 JOIN_SUBQUERY_EXISTS_095](crates/bench/sqlite_parity/cases/SQLITE_PARITY_702_JOIN_SUBQUERY_EXISTS_095.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1952684 | 3622674 | <span style="color:#dc2626">-85.52%</span> |
| 206 | [00019 REPLACE_INTO](crates/bench/sqlite_parity/cases/SQLITE_PARITY_019_REPLACE_INTO.rs) | P0 | memory | SQL_REPLACE | 1892290 | 3509519 | <span style="color:#dc2626">-85.46%</span> |
| 207 | [01021 JSON_EXTRACT_SET_014](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1021_JSON_EXTRACT_SET_014.rs) | P2 | memory | GEN_SQL_JSON | 1837346 | 3404280 | <span style="color:#dc2626">-85.28%</span> |
| 208 | [01126 INDEX_SCHEMA_PRAGMA_059](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1126_INDEX_SCHEMA_PRAGMA_059.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 2015943 | 3732682 | <span style="color:#dc2626">-85.16%</span> |
| 209 | [00362 SCALAR_STRING_034](crates/bench/sqlite_parity/cases/SQLITE_PARITY_362_SCALAR_STRING_034.rs) | P1 | memory | GEN_SQL_SCALAR | 1796118 | 3323618 | <span style="color:#dc2626">-85.04%</span> |
| 210 | [00842 WINDOW_PARTITION_SUM_055](crates/bench/sqlite_parity/cases/SQLITE_PARITY_842_WINDOW_PARTITION_SUM_055.rs) | P2 | memory | GEN_SQL_WINDOW | 1995455 | 3688669 | <span style="color:#dc2626">-84.85%</span> |
| 211 | [00508 AGG_GROUP_HAVING_001](crates/bench/sqlite_parity/cases/SQLITE_PARITY_508_AGG_GROUP_HAVING_001.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2085666 | 3852298 | <span style="color:#dc2626">-84.70%</span> |
| 212 | [00528 AGG_GROUP_HAVING_021](crates/bench/sqlite_parity/cases/SQLITE_PARITY_528_AGG_GROUP_HAVING_021.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2033297 | 3749363 | <span style="color:#dc2626">-84.40%</span> |
| 213 | [00680 JOIN_SUBQUERY_EXISTS_073](crates/bench/sqlite_parity/cases/SQLITE_PARITY_680_JOIN_SUBQUERY_EXISTS_073.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1966601 | 3625419 | <span style="color:#dc2626">-84.35%</span> |
| 214 | [00614 JOIN_SUBQUERY_EXISTS_007](crates/bench/sqlite_parity/cases/SQLITE_PARITY_614_JOIN_SUBQUERY_EXISTS_007.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2116093 | 3897663 | <span style="color:#dc2626">-84.19%</span> |
| 215 | [00936 CONSTRAINT_FK_SAVEPOINT_069](crates/bench/sqlite_parity/cases/SQLITE_PARITY_936_CONSTRAINT_FK_SAVEPOINT_069.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2100574 | 3867196 | <span style="color:#dc2626">-84.10%</span> |
| 216 | [00745 CTE_RECURSIVE_MATRIX_038](crates/bench/sqlite_parity/cases/SQLITE_PARITY_745_CTE_RECURSIVE_MATRIX_038.rs) | P1 | memory | GEN_SQL_CTE | 1870639 | 3443635 | <span style="color:#dc2626">-84.09%</span> |
| 217 | [00378 SCALAR_STRING_038](crates/bench/sqlite_parity/cases/SQLITE_PARITY_378_SCALAR_STRING_038.rs) | P1 | memory | GEN_SQL_SCALAR | 1974265 | 3633734 | <span style="color:#dc2626">-84.06%</span> |
| 218 | [00374 SCALAR_STRING_037](crates/bench/sqlite_parity/cases/SQLITE_PARITY_374_SCALAR_STRING_037.rs) | P1 | memory | GEN_SQL_SCALAR | 1832657 | 3371538 | <span style="color:#dc2626">-83.97%</span> |
| 219 | [00754 CTE_RECURSIVE_MATRIX_047](crates/bench/sqlite_parity/cases/SQLITE_PARITY_754_CTE_RECURSIVE_MATRIX_047.rs) | P1 | memory | GEN_SQL_CTE | 2003691 | 3676755 | <span style="color:#dc2626">-83.50%</span> |
| 220 | [01062 JSON_EXTRACT_SET_055](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1062_JSON_EXTRACT_SET_055.rs) | P2 | memory | GEN_SQL_JSON | 1856052 | 3404921 | <span style="color:#dc2626">-83.45%</span> |
| 221 | [00115 DOT_SCHEMA_TABLES_INDEXES](crates/bench/sqlite_parity/cases/SQLITE_PARITY_115_DOT_SCHEMA_TABLES_INDEXES.rs) | P0 | memory | CLI_DOT_COMMAND | 2193119 | 4016538 | <span style="color:#dc2626">-83.14%</span> |
| 222 | [00826 WINDOW_PARTITION_SUM_039](crates/bench/sqlite_parity/cases/SQLITE_PARITY_826_WINDOW_PARTITION_SUM_039.rs) | P2 | memory | GEN_SQL_WINDOW | 2039809 | 3735527 | <span style="color:#dc2626">-83.13%</span> |
| 223 | [01026 JSON_EXTRACT_SET_019](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1026_JSON_EXTRACT_SET_019.rs) | P2 | memory | GEN_SQL_JSON | 1824621 | 3341030 | <span style="color:#dc2626">-83.11%</span> |
| 224 | [00254 SCALAR_STRING_007](crates/bench/sqlite_parity/cases/SQLITE_PARITY_254_SCALAR_STRING_007.rs) | P1 | memory | GEN_SQL_SCALAR | 1777372 | 3254427 | <span style="color:#dc2626">-83.10%</span> |
| 225 | [00555 AGG_GROUP_HAVING_048](crates/bench/sqlite_parity/cases/SQLITE_PARITY_555_AGG_GROUP_HAVING_048.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2207335 | 4039041 | <span style="color:#dc2626">-82.98%</span> |
| 226 | [00796 WINDOW_PARTITION_SUM_009](crates/bench/sqlite_parity/cases/SQLITE_PARITY_796_WINDOW_PARTITION_SUM_009.rs) | P2 | memory | GEN_SQL_WINDOW | 2005314 | 3669231 | <span style="color:#dc2626">-82.98%</span> |
| 227 | [00558 AGG_GROUP_HAVING_051](crates/bench/sqlite_parity/cases/SQLITE_PARITY_558_AGG_GROUP_HAVING_051.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2105734 | 3851276 | <span style="color:#dc2626">-82.89%</span> |
| 228 | [00414 DML_WHERE_ORDER_LIMIT_027](crates/bench/sqlite_parity/cases/SQLITE_PARITY_414_DML_WHERE_ORDER_LIMIT_027.rs) | P1 | memory | GEN_SQL_DML | 2131592 | 3894468 | <span style="color:#dc2626">-82.70%</span> |
| 229 | [00050 PRAGMA_TABLE_INFO_FUNCTION](crates/bench/sqlite_parity/cases/SQLITE_PARITY_050_PRAGMA_TABLE_INFO_FUNCTION.rs) | P0 | memory | SQL_PRAGMA | 2012588 | 3666266 | <span style="color:#dc2626">-82.17%</span> |
| 230 | [00705 JOIN_SUBQUERY_EXISTS_098](crates/bench/sqlite_parity/cases/SQLITE_PARITY_705_JOIN_SUBQUERY_EXISTS_098.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2052202 | 3733664 | <span style="color:#dc2626">-81.93%</span> |
| 231 | [00300 SCALAR_ARITH_019](crates/bench/sqlite_parity/cases/SQLITE_PARITY_300_SCALAR_ARITH_019.rs) | P1 | memory | GEN_SQL_SCALAR | 1883624 | 3426833 | <span style="color:#dc2626">-81.93%</span> |
| 232 | [00128 DOT_DBCONFIG](crates/bench/sqlite_parity/cases/SQLITE_PARITY_128_DOT_DBCONFIG.rs) | P0 | memory | CLI_DOT_COMMAND | 1752625 | 3184655 | <span style="color:#dc2626">-81.71%</span> |
| 233 | [00548 AGG_GROUP_HAVING_041](crates/bench/sqlite_parity/cases/SQLITE_PARITY_548_AGG_GROUP_HAVING_041.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2079755 | 3777246 | <span style="color:#dc2626">-81.62%</span> |
| 234 | [00746 CTE_RECURSIVE_MATRIX_039](crates/bench/sqlite_parity/cases/SQLITE_PARITY_746_CTE_RECURSIVE_MATRIX_039.rs) | P1 | memory | GEN_SQL_CTE | 1823439 | 3309471 | <span style="color:#dc2626">-81.50%</span> |
| 235 | [01019 JSON_EXTRACT_SET_012](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1019_JSON_EXTRACT_SET_012.rs) | P2 | memory | GEN_SQL_JSON | 1895275 | 3439286 | <span style="color:#dc2626">-81.47%</span> |
| 236 | [01044 JSON_EXTRACT_SET_037](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1044_JSON_EXTRACT_SET_037.rs) | P2 | memory | GEN_SQL_JSON | 1888833 | 3425360 | <span style="color:#dc2626">-81.35%</span> |
| 237 | [00027 FOREIGN_KEY_FAILURE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_027_FOREIGN_KEY_FAILURE.rs) | P0 | memory | SQL_FOREIGN_KEYS_NEGATIVE | 1869867 | 3388220 | <span style="color:#dc2626">-81.20%</span> |
| 238 | [00685 JOIN_SUBQUERY_EXISTS_078](crates/bench/sqlite_parity/cases/SQLITE_PARITY_685_JOIN_SUBQUERY_EXISTS_078.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1961190 | 3552982 | <span style="color:#dc2626">-81.16%</span> |
| 239 | [00732 CTE_RECURSIVE_MATRIX_025](crates/bench/sqlite_parity/cases/SQLITE_PARITY_732_CTE_RECURSIVE_MATRIX_025.rs) | P1 | memory | GEN_SQL_CTE | 1790247 | 3240681 | <span style="color:#dc2626">-81.02%</span> |
| 240 | [00751 CTE_RECURSIVE_MATRIX_044](crates/bench/sqlite_parity/cases/SQLITE_PARITY_751_CTE_RECURSIVE_MATRIX_044.rs) | P1 | memory | GEN_SQL_CTE | 2046311 | 3703687 | <span style="color:#dc2626">-80.99%</span> |
| 241 | [00406 DML_WHERE_ORDER_LIMIT_019](crates/bench/sqlite_parity/cases/SQLITE_PARITY_406_DML_WHERE_ORDER_LIMIT_019.rs) | P1 | memory | GEN_SQL_DML | 1928759 | 3482829 | <span style="color:#dc2626">-80.57%</span> |
| 242 | [00339 SCALAR_NULL_COALESCE_028](crates/bench/sqlite_parity/cases/SQLITE_PARITY_339_SCALAR_NULL_COALESCE_028.rs) | P1 | memory | GEN_SQL_SCALAR | 1696820 | 3062083 | <span style="color:#dc2626">-80.46%</span> |
| 243 | [00875 CONSTRAINT_FK_SAVEPOINT_008](crates/bench/sqlite_parity/cases/SQLITE_PARITY_875_CONSTRAINT_FK_SAVEPOINT_008.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2048796 | 3693538 | <span style="color:#dc2626">-80.28%</span> |
| 244 | [00404 DML_WHERE_ORDER_LIMIT_017](crates/bench/sqlite_parity/cases/SQLITE_PARITY_404_DML_WHERE_ORDER_LIMIT_017.rs) | P1 | memory | GEN_SQL_DML | 1908460 | 3440319 | <span style="color:#dc2626">-80.27%</span> |
| 245 | [00784 CTE_RECURSIVE_MATRIX_077](crates/bench/sqlite_parity/cases/SQLITE_PARITY_784_CTE_RECURSIVE_MATRIX_077.rs) | P1 | memory | GEN_SQL_CTE | 1825193 | 3289012 | <span style="color:#dc2626">-80.20%</span> |
| 246 | [00310 SCALAR_STRING_021](crates/bench/sqlite_parity/cases/SQLITE_PARITY_310_SCALAR_STRING_021.rs) | P1 | memory | GEN_SQL_SCALAR | 1779496 | 3206125 | <span style="color:#dc2626">-80.17%</span> |
| 247 | [00982 VIEW_TRIGGER_GENERATED_035](crates/bench/sqlite_parity/cases/SQLITE_PARITY_982_VIEW_TRIGGER_GENERATED_035.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1966480 | 3542411 | <span style="color:#dc2626">-80.14%</span> |
| 248 | [00661 JOIN_SUBQUERY_EXISTS_054](crates/bench/sqlite_parity/cases/SQLITE_PARITY_661_JOIN_SUBQUERY_EXISTS_054.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2039278 | 3670403 | <span style="color:#dc2626">-79.99%</span> |
| 249 | [00280 SCALAR_ARITH_014](crates/bench/sqlite_parity/cases/SQLITE_PARITY_280_SCALAR_ARITH_014.rs) | P1 | memory | GEN_SQL_SCALAR | 1951372 | 3511784 | <span style="color:#dc2626">-79.96%</span> |
| 250 | [00497 DML_WHERE_ORDER_LIMIT_110](crates/bench/sqlite_parity/cases/SQLITE_PARITY_497_DML_WHERE_ORDER_LIMIT_110.rs) | P1 | memory | GEN_SQL_DML | 2137233 | 3839744 | <span style="color:#dc2626">-79.66%</span> |
| 251 | [00578 AGG_GROUP_HAVING_071](crates/bench/sqlite_parity/cases/SQLITE_PARITY_578_AGG_GROUP_HAVING_071.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2108379 | 3783468 | <span style="color:#dc2626">-79.45%</span> |
| 252 | [01004 VIEW_TRIGGER_GENERATED_057](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1004_VIEW_TRIGGER_GENERATED_057.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2367529 | 4242566 | <span style="color:#dc2626">-79.20%</span> |
| 253 | [01127 INDEX_SCHEMA_PRAGMA_060](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1127_INDEX_SCHEMA_PRAGMA_060.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 2024581 | 3626811 | <span style="color:#dc2626">-79.14%</span> |
| 254 | [00741 CTE_RECURSIVE_MATRIX_034](crates/bench/sqlite_parity/cases/SQLITE_PARITY_741_CTE_RECURSIVE_MATRIX_034.rs) | P1 | memory | GEN_SQL_CTE | 1770890 | 3169516 | <span style="color:#dc2626">-78.98%</span> |
| 255 | [00687 JOIN_SUBQUERY_EXISTS_080](crates/bench/sqlite_parity/cases/SQLITE_PARITY_687_JOIN_SUBQUERY_EXISTS_080.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2136762 | 3822502 | <span style="color:#dc2626">-78.89%</span> |
| 256 | [00235 SCALAR_NULL_COALESCE_002](crates/bench/sqlite_parity/cases/SQLITE_PARITY_235_SCALAR_NULL_COALESCE_002.rs) | P1 | memory | GEN_SQL_SCALAR | 1788834 | 3197539 | <span style="color:#dc2626">-78.75%</span> |
| 257 | [00692 JOIN_SUBQUERY_EXISTS_085](crates/bench/sqlite_parity/cases/SQLITE_PARITY_692_JOIN_SUBQUERY_EXISTS_085.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2027285 | 3621721 | <span style="color:#dc2626">-78.65%</span> |
| 258 | [00060 FILTER_CLAUSE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_060_FILTER_CLAUSE.rs) | P0 | memory | SQL_AGGREGATE | 1706127 | 3045231 | <span style="color:#dc2626">-78.49%</span> |
| 259 | [00405 DML_WHERE_ORDER_LIMIT_018](crates/bench/sqlite_parity/cases/SQLITE_PARITY_405_DML_WHERE_ORDER_LIMIT_018.rs) | P1 | memory | GEN_SQL_DML | 1944830 | 3470726 | <span style="color:#dc2626">-78.46%</span> |
| 260 | [00865 WINDOW_PARTITION_SUM_078](crates/bench/sqlite_parity/cases/SQLITE_PARITY_865_WINDOW_PARTITION_SUM_078.rs) | P2 | memory | GEN_SQL_WINDOW | 1959717 | 3496736 | <span style="color:#dc2626">-78.43%</span> |
| 261 | [00461 DML_WHERE_ORDER_LIMIT_074](crates/bench/sqlite_parity/cases/SQLITE_PARITY_461_DML_WHERE_ORDER_LIMIT_074.rs) | P1 | memory | GEN_SQL_DML | 2682574 | 4782628 | <span style="color:#dc2626">-78.29%</span> |
| 262 | [00479 DML_WHERE_ORDER_LIMIT_092](crates/bench/sqlite_parity/cases/SQLITE_PARITY_479_DML_WHERE_ORDER_LIMIT_092.rs) | P1 | memory | GEN_SQL_DML | 1883664 | 3356701 | <span style="color:#dc2626">-78.20%</span> |
| 263 | [00872 CONSTRAINT_FK_SAVEPOINT_005](crates/bench/sqlite_parity/cases/SQLITE_PARITY_872_CONSTRAINT_FK_SAVEPOINT_005.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2070437 | 3688598 | <span style="color:#dc2626">-78.16%</span> |
| 264 | [00678 JOIN_SUBQUERY_EXISTS_071](crates/bench/sqlite_parity/cases/SQLITE_PARITY_678_JOIN_SUBQUERY_EXISTS_071.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2005704 | 3573030 | <span style="color:#dc2626">-78.14%</span> |
| 265 | [00275 SCALAR_NULL_COALESCE_012](crates/bench/sqlite_parity/cases/SQLITE_PARITY_275_SCALAR_NULL_COALESCE_012.rs) | P1 | memory | GEN_SQL_SCALAR | 1772994 | 3156531 | <span style="color:#dc2626">-78.03%</span> |
| 266 | [01068 INDEX_SCHEMA_PRAGMA_001](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1068_INDEX_SCHEMA_PRAGMA_001.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 2178251 | 3875882 | <span style="color:#dc2626">-77.94%</span> |
| 267 | [00935 CONSTRAINT_FK_SAVEPOINT_068](crates/bench/sqlite_parity/cases/SQLITE_PARITY_935_CONSTRAINT_FK_SAVEPOINT_068.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2142814 | 3810148 | <span style="color:#dc2626">-77.81%</span> |
| 268 | [00846 WINDOW_PARTITION_SUM_059](crates/bench/sqlite_parity/cases/SQLITE_PARITY_846_WINDOW_PARTITION_SUM_059.rs) | P2 | memory | GEN_SQL_WINDOW | 2092819 | 3719426 | <span style="color:#dc2626">-77.72%</span> |
| 269 | [01067 JSON_EXTRACT_SET_060](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1067_JSON_EXTRACT_SET_060.rs) | P2 | memory | GEN_SQL_JSON | 1819402 | 3232195 | <span style="color:#dc2626">-77.65%</span> |
| 270 | [00760 CTE_RECURSIVE_MATRIX_053](crates/bench/sqlite_parity/cases/SQLITE_PARITY_760_CTE_RECURSIVE_MATRIX_053.rs) | P1 | memory | GEN_SQL_CTE | 2226352 | 3952778 | <span style="color:#dc2626">-77.55%</span> |
| 271 | [00474 DML_WHERE_ORDER_LIMIT_087](crates/bench/sqlite_parity/cases/SQLITE_PARITY_474_DML_WHERE_ORDER_LIMIT_087.rs) | P1 | memory | GEN_SQL_DML | 1927377 | 3421272 | <span style="color:#dc2626">-77.51%</span> |
| 272 | [00683 JOIN_SUBQUERY_EXISTS_076](crates/bench/sqlite_parity/cases/SQLITE_PARITY_683_JOIN_SUBQUERY_EXISTS_076.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2016415 | 3576997 | <span style="color:#dc2626">-77.39%</span> |
| 273 | [00075 EXPLAIN_QUERY_PLAN](crates/bench/sqlite_parity/cases/SQLITE_PARITY_075_EXPLAIN_QUERY_PLAN.rs) | P0 | memory | SQL_EXPLAIN | 1842736 | 3267351 | <span style="color:#dc2626">-77.31%</span> |
| 274 | [00677 JOIN_SUBQUERY_EXISTS_070](crates/bench/sqlite_parity/cases/SQLITE_PARITY_677_JOIN_SUBQUERY_EXISTS_070.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2000034 | 3545067 | <span style="color:#dc2626">-77.25%</span> |
| 275 | [00679 JOIN_SUBQUERY_EXISTS_072](crates/bench/sqlite_parity/cases/SQLITE_PARITY_679_JOIN_SUBQUERY_EXISTS_072.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2023348 | 3585833 | <span style="color:#dc2626">-77.22%</span> |
| 276 | [00607 AGG_GROUP_HAVING_100](crates/bench/sqlite_parity/cases/SQLITE_PARITY_607_AGG_GROUP_HAVING_100.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2018649 | 3577087 | <span style="color:#dc2626">-77.20%</span> |
| 277 | [00681 JOIN_SUBQUERY_EXISTS_074](crates/bench/sqlite_parity/cases/SQLITE_PARITY_681_JOIN_SUBQUERY_EXISTS_074.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2013820 | 3563131 | <span style="color:#dc2626">-76.93%</span> |
| 278 | [00492 DML_WHERE_ORDER_LIMIT_105](crates/bench/sqlite_parity/cases/SQLITE_PARITY_492_DML_WHERE_ORDER_LIMIT_105.rs) | P1 | memory | GEN_SQL_DML | 1932807 | 3419780 | <span style="color:#dc2626">-76.93%</span> |
| 279 | [00809 WINDOW_PARTITION_SUM_022](crates/bench/sqlite_parity/cases/SQLITE_PARITY_809_WINDOW_PARTITION_SUM_022.rs) | P2 | memory | GEN_SQL_WINDOW | 1968835 | 3483440 | <span style="color:#dc2626">-76.93%</span> |
| 280 | [00787 CTE_RECURSIVE_MATRIX_080](crates/bench/sqlite_parity/cases/SQLITE_PARITY_787_CTE_RECURSIVE_MATRIX_080.rs) | P1 | memory | GEN_SQL_CTE | 1809874 | 3200826 | <span style="color:#dc2626">-76.85%</span> |
| 281 | [00874 CONSTRAINT_FK_SAVEPOINT_007](crates/bench/sqlite_parity/cases/SQLITE_PARITY_874_CONSTRAINT_FK_SAVEPOINT_007.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1973012 | 3489081 | <span style="color:#dc2626">-76.84%</span> |
| 282 | [00495 DML_WHERE_ORDER_LIMIT_108](crates/bench/sqlite_parity/cases/SQLITE_PARITY_495_DML_WHERE_ORDER_LIMIT_108.rs) | P1 | memory | GEN_SQL_DML | 1913630 | 3382499 | <span style="color:#dc2626">-76.76%</span> |
| 283 | [00734 CTE_RECURSIVE_MATRIX_027](crates/bench/sqlite_parity/cases/SQLITE_PARITY_734_CTE_RECURSIVE_MATRIX_027.rs) | P1 | memory | GEN_SQL_CTE | 1868575 | 3301917 | <span style="color:#dc2626">-76.71%</span> |
| 284 | [00451 DML_WHERE_ORDER_LIMIT_064](crates/bench/sqlite_parity/cases/SQLITE_PARITY_451_DML_WHERE_ORDER_LIMIT_064.rs) | P1 | memory | GEN_SQL_DML | 1940902 | 3424779 | <span style="color:#dc2626">-76.45%</span> |
| 285 | [01045 JSON_EXTRACT_SET_038](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1045_JSON_EXTRACT_SET_038.rs) | P2 | memory | GEN_SQL_JSON | 1882451 | 3319630 | <span style="color:#dc2626">-76.35%</span> |
| 286 | [00664 JOIN_SUBQUERY_EXISTS_057](crates/bench/sqlite_parity/cases/SQLITE_PARITY_664_JOIN_SUBQUERY_EXISTS_057.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2524386 | 4450990 | <span style="color:#dc2626">-76.32%</span> |
| 287 | [00764 CTE_RECURSIVE_MATRIX_057](crates/bench/sqlite_parity/cases/SQLITE_PARITY_764_CTE_RECURSIVE_MATRIX_057.rs) | P1 | memory | GEN_SQL_CTE | 1844770 | 3250049 | <span style="color:#dc2626">-76.18%</span> |
| 288 | [01078 INDEX_SCHEMA_PRAGMA_011](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1078_INDEX_SCHEMA_PRAGMA_011.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 2051031 | 3613126 | <span style="color:#dc2626">-76.16%</span> |
| 289 | [00644 JOIN_SUBQUERY_EXISTS_037](crates/bench/sqlite_parity/cases/SQLITE_PARITY_644_JOIN_SUBQUERY_EXISTS_037.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2094833 | 3689770 | <span style="color:#dc2626">-76.14%</span> |
| 290 | [00542 AGG_GROUP_HAVING_035](crates/bench/sqlite_parity/cases/SQLITE_PARITY_542_AGG_GROUP_HAVING_035.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1950961 | 3434628 | <span style="color:#dc2626">-76.05%</span> |
| 291 | [00557 AGG_GROUP_HAVING_050](crates/bench/sqlite_parity/cases/SQLITE_PARITY_557_AGG_GROUP_HAVING_050.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1999412 | 3518446 | <span style="color:#dc2626">-75.97%</span> |
| 292 | [00698 JOIN_SUBQUERY_EXISTS_091](crates/bench/sqlite_parity/cases/SQLITE_PARITY_698_JOIN_SUBQUERY_EXISTS_091.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1996257 | 3512876 | <span style="color:#dc2626">-75.97%</span> |
| 293 | [00665 JOIN_SUBQUERY_EXISTS_058](crates/bench/sqlite_parity/cases/SQLITE_PARITY_665_JOIN_SUBQUERY_EXISTS_058.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2230810 | 3923683 | <span style="color:#dc2626">-75.89%</span> |
| 294 | [00441 DML_WHERE_ORDER_LIMIT_054](crates/bench/sqlite_parity/cases/SQLITE_PARITY_441_DML_WHERE_ORDER_LIMIT_054.rs) | P1 | memory | GEN_SQL_DML | 1952664 | 3433615 | <span style="color:#dc2626">-75.84%</span> |
| 295 | [00486 DML_WHERE_ORDER_LIMIT_099](crates/bench/sqlite_parity/cases/SQLITE_PARITY_486_DML_WHERE_ORDER_LIMIT_099.rs) | P1 | memory | GEN_SQL_DML | 1918369 | 3372811 | <span style="color:#dc2626">-75.82%</span> |
| 296 | [00884 CONSTRAINT_FK_SAVEPOINT_017](crates/bench/sqlite_parity/cases/SQLITE_PARITY_884_CONSTRAINT_FK_SAVEPOINT_017.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2067761 | 3635278 | <span style="color:#dc2626">-75.81%</span> |
| 297 | [00515 AGG_GROUP_HAVING_008](crates/bench/sqlite_parity/cases/SQLITE_PARITY_515_AGG_GROUP_HAVING_008.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2072561 | 3643452 | <span style="color:#dc2626">-75.79%</span> |
| 298 | [00841 WINDOW_PARTITION_SUM_054](crates/bench/sqlite_parity/cases/SQLITE_PARITY_841_WINDOW_PARTITION_SUM_054.rs) | P2 | memory | GEN_SQL_WINDOW | 2394880 | 4207610 | <span style="color:#dc2626">-75.69%</span> |
| 299 | [00658 JOIN_SUBQUERY_EXISTS_051](crates/bench/sqlite_parity/cases/SQLITE_PARITY_658_JOIN_SUBQUERY_EXISTS_051.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2111875 | 3709838 | <span style="color:#dc2626">-75.67%</span> |
| 300 | [00427 DML_WHERE_ORDER_LIMIT_040](crates/bench/sqlite_parity/cases/SQLITE_PARITY_427_DML_WHERE_ORDER_LIMIT_040.rs) | P1 | memory | GEN_SQL_DML | 1913690 | 3360117 | <span style="color:#dc2626">-75.58%</span> |
| 301 | [00503 DML_WHERE_ORDER_LIMIT_116](crates/bench/sqlite_parity/cases/SQLITE_PARITY_503_DML_WHERE_ORDER_LIMIT_116.rs) | P1 | memory | GEN_SQL_DML | 1907188 | 3348314 | <span style="color:#dc2626">-75.56%</span> |
| 302 | [00436 DML_WHERE_ORDER_LIMIT_049](crates/bench/sqlite_parity/cases/SQLITE_PARITY_436_DML_WHERE_ORDER_LIMIT_049.rs) | P1 | memory | GEN_SQL_DML | 1935291 | 3395634 | <span style="color:#dc2626">-75.46%</span> |
| 303 | [01066 JSON_EXTRACT_SET_059](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1066_JSON_EXTRACT_SET_059.rs) | P2 | memory | GEN_SQL_JSON | 1865028 | 3270908 | <span style="color:#dc2626">-75.38%</span> |
| 304 | [00412 DML_WHERE_ORDER_LIMIT_025](crates/bench/sqlite_parity/cases/SQLITE_PARITY_412_DML_WHERE_ORDER_LIMIT_025.rs) | P1 | memory | GEN_SQL_DML | 1946333 | 3413007 | <span style="color:#dc2626">-75.36%</span> |
| 305 | [00216 ROLLBACK_TRANSACTION_SYNTAX](crates/bench/sqlite_parity/cases/SQLITE_PARITY_216_ROLLBACK_TRANSACTION_SYNTAX.rs) | P0 | memory | SQL_TRANSACTION | 1917358 | 3360227 | <span style="color:#dc2626">-75.25%</span> |
| 306 | [00815 WINDOW_PARTITION_SUM_028](crates/bench/sqlite_parity/cases/SQLITE_PARITY_815_WINDOW_PARTITION_SUM_028.rs) | P2 | memory | GEN_SQL_WINDOW | 2001878 | 3508207 | <span style="color:#dc2626">-75.25%</span> |
| 307 | [00829 WINDOW_PARTITION_SUM_042](crates/bench/sqlite_parity/cases/SQLITE_PARITY_829_WINDOW_PARTITION_SUM_042.rs) | P2 | memory | GEN_SQL_WINDOW | 1970508 | 3450829 | <span style="color:#dc2626">-75.12%</span> |
| 308 | [00277 SCALAR_CAST_TYPEOF_013](crates/bench/sqlite_parity/cases/SQLITE_PARITY_277_SCALAR_CAST_TYPEOF_013.rs) | P1 | memory | GEN_SQL_SCALAR | 1892941 | 3310963 | <span style="color:#dc2626">-74.91%</span> |
| 309 | [01092 INDEX_SCHEMA_PRAGMA_025](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1092_INDEX_SCHEMA_PRAGMA_025.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1962062 | 3429889 | <span style="color:#dc2626">-74.81%</span> |
| 310 | [00264 SCALAR_ARITH_010](crates/bench/sqlite_parity/cases/SQLITE_PARITY_264_SCALAR_ARITH_010.rs) | P1 | memory | GEN_SQL_SCALAR | 1720805 | 3007459 | <span style="color:#dc2626">-74.77%</span> |
| 311 | [01057 JSON_EXTRACT_SET_050](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1057_JSON_EXTRACT_SET_050.rs) | P2 | memory | GEN_SQL_JSON | 1873214 | 3273422 | <span style="color:#dc2626">-74.75%</span> |
| 312 | [00559 AGG_GROUP_HAVING_052](crates/bench/sqlite_parity/cases/SQLITE_PARITY_559_AGG_GROUP_HAVING_052.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1990686 | 3478110 | <span style="color:#dc2626">-74.72%</span> |
| 313 | [00449 DML_WHERE_ORDER_LIMIT_062](crates/bench/sqlite_parity/cases/SQLITE_PARITY_449_DML_WHERE_ORDER_LIMIT_062.rs) | P1 | memory | GEN_SQL_DML | 1929390 | 3370356 | <span style="color:#dc2626">-74.69%</span> |
| 314 | [00586 AGG_GROUP_HAVING_079](crates/bench/sqlite_parity/cases/SQLITE_PARITY_586_AGG_GROUP_HAVING_079.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1925954 | 3364294 | <span style="color:#dc2626">-74.68%</span> |
| 315 | [00279 SCALAR_NULL_COALESCE_013](crates/bench/sqlite_parity/cases/SQLITE_PARITY_279_SCALAR_NULL_COALESCE_013.rs) | P1 | memory | GEN_SQL_SCALAR | 1704886 | 2977784 | <span style="color:#dc2626">-74.66%</span> |
| 316 | [00444 DML_WHERE_ORDER_LIMIT_057](crates/bench/sqlite_parity/cases/SQLITE_PARITY_444_DML_WHERE_ORDER_LIMIT_057.rs) | P1 | memory | GEN_SQL_DML | 1955219 | 3414059 | <span style="color:#dc2626">-74.61%</span> |
| 317 | [00003 CREATE_TABLE_INSERT_SELECT](crates/bench/sqlite_parity/cases/SQLITE_PARITY_003_CREATE_TABLE_INSERT_SELECT.rs) | P0 | memory | SQL_DDL_DML | 2052824 | 3583920 | <span style="color:#dc2626">-74.58%</span> |
| 318 | [00445 DML_WHERE_ORDER_LIMIT_058](crates/bench/sqlite_parity/cases/SQLITE_PARITY_445_DML_WHERE_ORDER_LIMIT_058.rs) | P1 | memory | GEN_SQL_DML | 1898251 | 3311956 | <span style="color:#dc2626">-74.47%</span> |
| 319 | [00505 DML_WHERE_ORDER_LIMIT_118](crates/bench/sqlite_parity/cases/SQLITE_PARITY_505_DML_WHERE_ORDER_LIMIT_118.rs) | P1 | memory | GEN_SQL_DML | 1922908 | 3353324 | <span style="color:#dc2626">-74.39%</span> |
| 320 | [00765 CTE_RECURSIVE_MATRIX_058](crates/bench/sqlite_parity/cases/SQLITE_PARITY_765_CTE_RECURSIVE_MATRIX_058.rs) | P1 | memory | GEN_SQL_CTE | 1849879 | 3225211 | <span style="color:#dc2626">-74.35%</span> |
| 321 | [00136 DOT_LOG](crates/bench/sqlite_parity/cases/SQLITE_PARITY_136_DOT_LOG.rs) | P0 | memory | CLI_DOT_COMMAND | 1743879 | 3040331 | <span style="color:#dc2626">-74.34%</span> |
| 322 | [00863 WINDOW_PARTITION_SUM_076](crates/bench/sqlite_parity/cases/SQLITE_PARITY_863_WINDOW_PARTITION_SUM_076.rs) | P2 | memory | GEN_SQL_WINDOW | 1988602 | 3466348 | <span style="color:#dc2626">-74.31%</span> |
| 323 | [00013 INSERT_SELECT](crates/bench/sqlite_parity/cases/SQLITE_PARITY_013_INSERT_SELECT.rs) | P0 | memory | SQL_INSERT | 1833568 | 3195185 | <span style="color:#dc2626">-74.26%</span> |
| 324 | [00269 SCALAR_CAST_TYPEOF_011](crates/bench/sqlite_parity/cases/SQLITE_PARITY_269_SCALAR_CAST_TYPEOF_011.rs) | P1 | memory | GEN_SQL_SCALAR | 1683384 | 2933108 | <span style="color:#dc2626">-74.24%</span> |
| 325 | [01091 INDEX_SCHEMA_PRAGMA_024](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1091_INDEX_SCHEMA_PRAGMA_024.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1982751 | 3451299 | <span style="color:#dc2626">-74.07%</span> |
| 326 | [01075 INDEX_SCHEMA_PRAGMA_008](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1075_INDEX_SCHEMA_PRAGMA_008.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1997248 | 3475705 | <span style="color:#dc2626">-74.02%</span> |
| 327 | [01058 JSON_EXTRACT_SET_051](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1058_JSON_EXTRACT_SET_051.rs) | P2 | memory | GEN_SQL_JSON | 1918219 | 3337153 | <span style="color:#dc2626">-73.97%</span> |
| 328 | [00579 AGG_GROUP_HAVING_072](crates/bench/sqlite_parity/cases/SQLITE_PARITY_579_AGG_GROUP_HAVING_072.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2013268 | 3500933 | <span style="color:#dc2626">-73.89%</span> |
| 329 | [01109 INDEX_SCHEMA_PRAGMA_042](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1109_INDEX_SCHEMA_PRAGMA_042.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1999503 | 3476176 | <span style="color:#dc2626">-73.85%</span> |
| 330 | [00493 DML_WHERE_ORDER_LIMIT_106](crates/bench/sqlite_parity/cases/SQLITE_PARITY_493_DML_WHERE_ORDER_LIMIT_106.rs) | P1 | memory | GEN_SQL_DML | 1923439 | 3341973 | <span style="color:#dc2626">-73.75%</span> |
| 331 | [01047 JSON_EXTRACT_SET_040](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1047_JSON_EXTRACT_SET_040.rs) | P2 | memory | GEN_SQL_JSON | 1919512 | 3334549 | <span style="color:#dc2626">-73.72%</span> |
| 332 | [00635 JOIN_SUBQUERY_EXISTS_028](crates/bench/sqlite_parity/cases/SQLITE_PARITY_635_JOIN_SUBQUERY_EXISTS_028.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2020833 | 3510050 | <span style="color:#dc2626">-73.69%</span> |
| 333 | [00407 DML_WHERE_ORDER_LIMIT_020](crates/bench/sqlite_parity/cases/SQLITE_PARITY_407_DML_WHERE_ORDER_LIMIT_020.rs) | P1 | memory | GEN_SQL_DML | 1958214 | 3400924 | <span style="color:#dc2626">-73.67%</span> |
| 334 | [00718 CTE_RECURSIVE_MATRIX_011](crates/bench/sqlite_parity/cases/SQLITE_PARITY_718_CTE_RECURSIVE_MATRIX_011.rs) | P1 | memory | GEN_SQL_CTE | 1864597 | 3237585 | <span style="color:#dc2626">-73.63%</span> |
| 335 | [00504 DML_WHERE_ORDER_LIMIT_117](crates/bench/sqlite_parity/cases/SQLITE_PARITY_504_DML_WHERE_ORDER_LIMIT_117.rs) | P1 | memory | GEN_SQL_DML | 1914592 | 3324068 | <span style="color:#dc2626">-73.62%</span> |
| 336 | [00259 SCALAR_NULL_COALESCE_008](crates/bench/sqlite_parity/cases/SQLITE_PARITY_259_SCALAR_NULL_COALESCE_008.rs) | P1 | memory | GEN_SQL_SCALAR | 1694516 | 2941565 | <span style="color:#dc2626">-73.59%</span> |
| 337 | [00215 TRANSACTION_SYNONYMS_END_COMMIT_TRANSACTION](crates/bench/sqlite_parity/cases/SQLITE_PARITY_215_TRANSACTION_SYNONYMS_END_COMMIT_TRANSACTION.rs) | P0 | memory | SQL_TRANSACTION | 1943036 | 3371428 | <span style="color:#dc2626">-73.51%</span> |
| 338 | [00459 DML_WHERE_ORDER_LIMIT_072](crates/bench/sqlite_parity/cases/SQLITE_PARITY_459_DML_WHERE_ORDER_LIMIT_072.rs) | P1 | memory | GEN_SQL_DML | 1930863 | 3349517 | <span style="color:#dc2626">-73.47%</span> |
| 339 | [00494 DML_WHERE_ORDER_LIMIT_107](crates/bench/sqlite_parity/cases/SQLITE_PARITY_494_DML_WHERE_ORDER_LIMIT_107.rs) | P1 | memory | GEN_SQL_DML | 1923940 | 3337453 | <span style="color:#dc2626">-73.47%</span> |
| 340 | [00468 DML_WHERE_ORDER_LIMIT_081](crates/bench/sqlite_parity/cases/SQLITE_PARITY_468_DML_WHERE_ORDER_LIMIT_081.rs) | P1 | memory | GEN_SQL_DML | 1917407 | 3326042 | <span style="color:#dc2626">-73.47%</span> |
| 341 | [00612 JOIN_SUBQUERY_EXISTS_005](crates/bench/sqlite_parity/cases/SQLITE_PARITY_612_JOIN_SUBQUERY_EXISTS_005.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2030141 | 3519428 | <span style="color:#dc2626">-73.36%</span> |
| 342 | [00067 CASE_COALESCE_NULLIF_IIF](crates/bench/sqlite_parity/cases/SQLITE_PARITY_067_CASE_COALESCE_NULLIF_IIF.rs) | P0 | memory | SQL_EXPRESSIONS | 1682583 | 2916818 | <span style="color:#dc2626">-73.35%</span> |
| 343 | [00457 DML_WHERE_ORDER_LIMIT_070](crates/bench/sqlite_parity/cases/SQLITE_PARITY_457_DML_WHERE_ORDER_LIMIT_070.rs) | P1 | memory | GEN_SQL_DML | 2048766 | 3551289 | <span style="color:#dc2626">-73.34%</span> |
| 344 | [00895 CONSTRAINT_FK_SAVEPOINT_028](crates/bench/sqlite_parity/cases/SQLITE_PARITY_895_CONSTRAINT_FK_SAVEPOINT_028.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2030061 | 3518787 | <span style="color:#dc2626">-73.33%</span> |
| 345 | [00609 JOIN_SUBQUERY_EXISTS_002](crates/bench/sqlite_parity/cases/SQLITE_PARITY_609_JOIN_SUBQUERY_EXISTS_002.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1988922 | 3445759 | <span style="color:#dc2626">-73.25%</span> |
| 346 | [00422 DML_WHERE_ORDER_LIMIT_035](crates/bench/sqlite_parity/cases/SQLITE_PARITY_422_DML_WHERE_ORDER_LIMIT_035.rs) | P1 | memory | GEN_SQL_DML | 1983041 | 3434618 | <span style="color:#dc2626">-73.20%</span> |
| 347 | [00805 WINDOW_PARTITION_SUM_018](crates/bench/sqlite_parity/cases/SQLITE_PARITY_805_WINDOW_PARTITION_SUM_018.rs) | P2 | memory | GEN_SQL_WINDOW | 2289421 | 3964461 | <span style="color:#dc2626">-73.16%</span> |
| 348 | [00716 CTE_RECURSIVE_MATRIX_009](crates/bench/sqlite_parity/cases/SQLITE_PARITY_716_CTE_RECURSIVE_MATRIX_009.rs) | P1 | memory | GEN_SQL_CTE | 1804124 | 3123339 | <span style="color:#dc2626">-73.12%</span> |
| 349 | [00192 OPT_INIT_TEMPFILE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_192_OPT_INIT_TEMPFILE.rs) | P2 | tempfile | CLI_OPTION_TEMPFILE | 1584006 | 2742238 | <span style="color:#dc2626">-73.12%</span> |
| 350 | [00409 DML_WHERE_ORDER_LIMIT_022](crates/bench/sqlite_parity/cases/SQLITE_PARITY_409_DML_WHERE_ORDER_LIMIT_022.rs) | P1 | memory | GEN_SQL_DML | 1931524 | 3342834 | <span style="color:#dc2626">-73.07%</span> |
| 351 | [00653 JOIN_SUBQUERY_EXISTS_046](crates/bench/sqlite_parity/cases/SQLITE_PARITY_653_JOIN_SUBQUERY_EXISTS_046.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2002508 | 3464955 | <span style="color:#dc2626">-73.03%</span> |
| 352 | [00793 WINDOW_PARTITION_SUM_006](crates/bench/sqlite_parity/cases/SQLITE_PARITY_793_WINDOW_PARTITION_SUM_006.rs) | P2 | memory | GEN_SQL_WINDOW | 2023378 | 3500422 | <span style="color:#dc2626">-73.00%</span> |
| 353 | [00749 CTE_RECURSIVE_MATRIX_042](crates/bench/sqlite_parity/cases/SQLITE_PARITY_749_CTE_RECURSIVE_MATRIX_042.rs) | P1 | memory | GEN_SQL_CTE | 1942334 | 3359535 | <span style="color:#dc2626">-72.96%</span> |
| 354 | [01090 INDEX_SCHEMA_PRAGMA_023](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1090_INDEX_SCHEMA_PRAGMA_023.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 2017357 | 3489000 | <span style="color:#dc2626">-72.95%</span> |
| 355 | [00694 JOIN_SUBQUERY_EXISTS_087](crates/bench/sqlite_parity/cases/SQLITE_PARITY_694_JOIN_SUBQUERY_EXISTS_087.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2422803 | 4190036 | <span style="color:#dc2626">-72.94%</span> |
| 356 | [00560 AGG_GROUP_HAVING_053](crates/bench/sqlite_parity/cases/SQLITE_PARITY_560_AGG_GROUP_HAVING_053.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1957453 | 3384072 | <span style="color:#dc2626">-72.88%</span> |
| 357 | [00712 CTE_RECURSIVE_MATRIX_005](crates/bench/sqlite_parity/cases/SQLITE_PARITY_712_CTE_RECURSIVE_MATRIX_005.rs) | P1 | memory | GEN_SQL_CTE | 1837717 | 3176540 | <span style="color:#dc2626">-72.85%</span> |
| 358 | [00471 DML_WHERE_ORDER_LIMIT_084](crates/bench/sqlite_parity/cases/SQLITE_PARITY_471_DML_WHERE_ORDER_LIMIT_084.rs) | P1 | memory | GEN_SQL_DML | 1937876 | 3343956 | <span style="color:#dc2626">-72.56%</span> |
| 359 | [00733 CTE_RECURSIVE_MATRIX_026](crates/bench/sqlite_parity/cases/SQLITE_PARITY_733_CTE_RECURSIVE_MATRIX_026.rs) | P1 | memory | GEN_SQL_CTE | 1834851 | 3165829 | <span style="color:#dc2626">-72.54%</span> |
| 360 | [00472 DML_WHERE_ORDER_LIMIT_085](crates/bench/sqlite_parity/cases/SQLITE_PARITY_472_DML_WHERE_ORDER_LIMIT_085.rs) | P1 | memory | GEN_SQL_DML | 1981299 | 3417846 | <span style="color:#dc2626">-72.51%</span> |
| 361 | [00888 CONSTRAINT_FK_SAVEPOINT_021](crates/bench/sqlite_parity/cases/SQLITE_PARITY_888_CONSTRAINT_FK_SAVEPOINT_021.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2046812 | 3529878 | <span style="color:#dc2626">-72.46%</span> |
| 362 | [00482 DML_WHERE_ORDER_LIMIT_095](crates/bench/sqlite_parity/cases/SQLITE_PARITY_482_DML_WHERE_ORDER_LIMIT_095.rs) | P1 | memory | GEN_SQL_DML | 1977290 | 3408478 | <span style="color:#dc2626">-72.38%</span> |
| 363 | [00547 AGG_GROUP_HAVING_040](crates/bench/sqlite_parity/cases/SQLITE_PARITY_547_AGG_GROUP_HAVING_040.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2475573 | 4265089 | <span style="color:#dc2626">-72.29%</span> |
| 364 | [00391 DML_WHERE_ORDER_LIMIT_004](crates/bench/sqlite_parity/cases/SQLITE_PARITY_391_DML_WHERE_ORDER_LIMIT_004.rs) | P1 | memory | GEN_SQL_DML | 1910324 | 3290195 | <span style="color:#dc2626">-72.23%</span> |
| 365 | [00498 DML_WHERE_ORDER_LIMIT_111](crates/bench/sqlite_parity/cases/SQLITE_PARITY_498_DML_WHERE_ORDER_LIMIT_111.rs) | P1 | memory | GEN_SQL_DML | 1954307 | 3364996 | <span style="color:#dc2626">-72.18%</span> |
| 366 | [00433 DML_WHERE_ORDER_LIMIT_046](crates/bench/sqlite_parity/cases/SQLITE_PARITY_433_DML_WHERE_ORDER_LIMIT_046.rs) | P1 | memory | GEN_SQL_DML | 1984144 | 3413738 | <span style="color:#dc2626">-72.05%</span> |
| 367 | [00887 CONSTRAINT_FK_SAVEPOINT_020](crates/bench/sqlite_parity/cases/SQLITE_PARITY_887_CONSTRAINT_FK_SAVEPOINT_020.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2092398 | 3599629 | <span style="color:#dc2626">-72.03%</span> |
| 368 | [00756 CTE_RECURSIVE_MATRIX_049](crates/bench/sqlite_parity/cases/SQLITE_PARITY_756_CTE_RECURSIVE_MATRIX_049.rs) | P1 | memory | GEN_SQL_CTE | 1867914 | 3213179 | <span style="color:#dc2626">-72.02%</span> |
| 369 | [01125 INDEX_SCHEMA_PRAGMA_058](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1125_INDEX_SCHEMA_PRAGMA_058.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 2210772 | 3802643 | <span style="color:#dc2626">-72.01%</span> |
| 370 | [00502 DML_WHERE_ORDER_LIMIT_115](crates/bench/sqlite_parity/cases/SQLITE_PARITY_502_DML_WHERE_ORDER_LIMIT_115.rs) | P1 | memory | GEN_SQL_DML | 1980957 | 3406775 | <span style="color:#dc2626">-71.98%</span> |
| 371 | [00758 CTE_RECURSIVE_MATRIX_051](crates/bench/sqlite_parity/cases/SQLITE_PARITY_758_CTE_RECURSIVE_MATRIX_051.rs) | P1 | memory | GEN_SQL_CTE | 1869036 | 3213670 | <span style="color:#dc2626">-71.94%</span> |
| 372 | [00267 SCALAR_NULL_COALESCE_010](crates/bench/sqlite_parity/cases/SQLITE_PARITY_267_SCALAR_NULL_COALESCE_010.rs) | P1 | memory | GEN_SQL_SCALAR | 1826295 | 3140150 | <span style="color:#dc2626">-71.94%</span> |
| 373 | [00830 WINDOW_PARTITION_SUM_043](crates/bench/sqlite_parity/cases/SQLITE_PARITY_830_WINDOW_PARTITION_SUM_043.rs) | P2 | memory | GEN_SQL_WINDOW | 2214329 | 3805820 | <span style="color:#dc2626">-71.87%</span> |
| 374 | [00651 JOIN_SUBQUERY_EXISTS_044](crates/bench/sqlite_parity/cases/SQLITE_PARITY_651_JOIN_SUBQUERY_EXISTS_044.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2072270 | 3561527 | <span style="color:#dc2626">-71.87%</span> |
| 375 | [01054 JSON_EXTRACT_SET_047](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1054_JSON_EXTRACT_SET_047.rs) | P2 | memory | GEN_SQL_JSON | 1822378 | 3130833 | <span style="color:#dc2626">-71.80%</span> |
| 376 | [01063 JSON_EXTRACT_SET_056](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1063_JSON_EXTRACT_SET_056.rs) | P2 | memory | GEN_SQL_JSON | 1842426 | 3164998 | <span style="color:#dc2626">-71.78%</span> |
| 377 | [00613 JOIN_SUBQUERY_EXISTS_006](crates/bench/sqlite_parity/cases/SQLITE_PARITY_613_JOIN_SUBQUERY_EXISTS_006.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2019951 | 3469694 | <span style="color:#dc2626">-71.77%</span> |
| 378 | [00483 DML_WHERE_ORDER_LIMIT_096](crates/bench/sqlite_parity/cases/SQLITE_PARITY_483_DML_WHERE_ORDER_LIMIT_096.rs) | P1 | memory | GEN_SQL_DML | 1994373 | 3423967 | <span style="color:#dc2626">-71.68%</span> |
| 379 | [00496 DML_WHERE_ORDER_LIMIT_109](crates/bench/sqlite_parity/cases/SQLITE_PARITY_496_DML_WHERE_ORDER_LIMIT_109.rs) | P1 | memory | GEN_SQL_DML | 1934940 | 3319360 | <span style="color:#dc2626">-71.55%</span> |
| 380 | [00912 CONSTRAINT_FK_SAVEPOINT_045](crates/bench/sqlite_parity/cases/SQLITE_PARITY_912_CONSTRAINT_FK_SAVEPOINT_045.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1944107 | 3334949 | <span style="color:#dc2626">-71.54%</span> |
| 381 | [01033 JSON_EXTRACT_SET_026](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1033_JSON_EXTRACT_SET_026.rs) | P2 | memory | GEN_SQL_JSON | 1853747 | 3179896 | <span style="color:#dc2626">-71.54%</span> |
| 382 | [01117 INDEX_SCHEMA_PRAGMA_050](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1117_INDEX_SCHEMA_PRAGMA_050.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 2028477 | 3478420 | <span style="color:#dc2626">-71.48%</span> |
| 383 | [00974 VIEW_TRIGGER_GENERATED_027](crates/bench/sqlite_parity/cases/SQLITE_PARITY_974_VIEW_TRIGGER_GENERATED_027.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2368661 | 4061754 | <span style="color:#dc2626">-71.48%</span> |
| 384 | [01085 INDEX_SCHEMA_PRAGMA_018](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1085_INDEX_SCHEMA_PRAGMA_018.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 2049597 | 3514399 | <span style="color:#dc2626">-71.47%</span> |
| 385 | [01069 INDEX_SCHEMA_PRAGMA_002](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1069_INDEX_SCHEMA_PRAGMA_002.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 2001767 | 3430721 | <span style="color:#dc2626">-71.38%</span> |
| 386 | [00465 DML_WHERE_ORDER_LIMIT_078](crates/bench/sqlite_parity/cases/SQLITE_PARITY_465_DML_WHERE_ORDER_LIMIT_078.rs) | P1 | memory | GEN_SQL_DML | 1965879 | 3368262 | <span style="color:#dc2626">-71.34%</span> |
| 387 | [00410 DML_WHERE_ORDER_LIMIT_023](crates/bench/sqlite_parity/cases/SQLITE_PARITY_410_DML_WHERE_ORDER_LIMIT_023.rs) | P1 | memory | GEN_SQL_DML | 1963585 | 3364234 | <span style="color:#dc2626">-71.33%</span> |
| 388 | [00779 CTE_RECURSIVE_MATRIX_072](crates/bench/sqlite_parity/cases/SQLITE_PARITY_779_CTE_RECURSIVE_MATRIX_072.rs) | P1 | memory | GEN_SQL_CTE | 1860340 | 3187160 | <span style="color:#dc2626">-71.32%</span> |
| 389 | [00990 VIEW_TRIGGER_GENERATED_043](crates/bench/sqlite_parity/cases/SQLITE_PARITY_990_VIEW_TRIGGER_GENERATED_043.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2088572 | 3577077 | <span style="color:#dc2626">-71.27%</span> |
| 390 | [01105 INDEX_SCHEMA_PRAGMA_038](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1105_INDEX_SCHEMA_PRAGMA_038.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 2034950 | 3483480 | <span style="color:#dc2626">-71.18%</span> |
| 391 | [00452 DML_WHERE_ORDER_LIMIT_065](crates/bench/sqlite_parity/cases/SQLITE_PARITY_452_DML_WHERE_ORDER_LIMIT_065.rs) | P1 | memory | GEN_SQL_DML | 1988782 | 3403669 | <span style="color:#dc2626">-71.14%</span> |
| 392 | [00322 SCALAR_STRING_024](crates/bench/sqlite_parity/cases/SQLITE_PARITY_322_SCALAR_STRING_024.rs) | P1 | memory | GEN_SQL_SCALAR | 1667875 | 2854020 | <span style="color:#dc2626">-71.12%</span> |
| 393 | [00910 CONSTRAINT_FK_SAVEPOINT_043](crates/bench/sqlite_parity/cases/SQLITE_PARITY_910_CONSTRAINT_FK_SAVEPOINT_043.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1959367 | 3350849 | <span style="color:#dc2626">-71.02%</span> |
| 394 | [01052 JSON_EXTRACT_SET_045](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1052_JSON_EXTRACT_SET_045.rs) | P2 | memory | GEN_SQL_JSON | 1929901 | 3299752 | <span style="color:#dc2626">-70.98%</span> |
| 395 | [00266 SCALAR_STRING_010](crates/bench/sqlite_parity/cases/SQLITE_PARITY_266_SCALAR_STRING_010.rs) | P1 | memory | GEN_SQL_SCALAR | 1732137 | 2959769 | <span style="color:#dc2626">-70.87%</span> |
| 396 | [01089 INDEX_SCHEMA_PRAGMA_022](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1089_INDEX_SCHEMA_PRAGMA_022.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 2043837 | 3492147 | <span style="color:#dc2626">-70.86%</span> |
| 397 | [00512 AGG_GROUP_HAVING_005](crates/bench/sqlite_parity/cases/SQLITE_PARITY_512_AGG_GROUP_HAVING_005.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1924190 | 3287659 | <span style="color:#dc2626">-70.86%</span> |
| 398 | [00420 DML_WHERE_ORDER_LIMIT_033](crates/bench/sqlite_parity/cases/SQLITE_PARITY_420_DML_WHERE_ORDER_LIMIT_033.rs) | P1 | memory | GEN_SQL_DML | 1968634 | 3363132 | <span style="color:#dc2626">-70.84%</span> |
| 399 | [00927 CONSTRAINT_FK_SAVEPOINT_060](crates/bench/sqlite_parity/cases/SQLITE_PARITY_927_CONSTRAINT_FK_SAVEPOINT_060.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1973975 | 3371899 | <span style="color:#dc2626">-70.82%</span> |
| 400 | [00854 WINDOW_PARTITION_SUM_067](crates/bench/sqlite_parity/cases/SQLITE_PARITY_854_WINDOW_PARTITION_SUM_067.rs) | P2 | memory | GEN_SQL_WINDOW | 1953796 | 3334127 | <span style="color:#dc2626">-70.65%</span> |
| 401 | [00064 CTE_NON_RECURSIVE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_064_CTE_NON_RECURSIVE.rs) | P0 | memory | SQL_CTE | 1725034 | 2943669 | <span style="color:#dc2626">-70.64%</span> |
| 402 | [00747 CTE_RECURSIVE_MATRIX_040](crates/bench/sqlite_parity/cases/SQLITE_PARITY_747_CTE_RECURSIVE_MATRIX_040.rs) | P1 | memory | GEN_SQL_CTE | 2383048 | 4064730 | <span style="color:#dc2626">-70.57%</span> |
| 403 | [00036 CREATE_VIEW](crates/bench/sqlite_parity/cases/SQLITE_PARITY_036_CREATE_VIEW.rs) | P0 | memory | SQL_VIEW | 1814743 | 3094734 | <span style="color:#dc2626">-70.53%</span> |
| 404 | [00411 DML_WHERE_ORDER_LIMIT_024](crates/bench/sqlite_parity/cases/SQLITE_PARITY_411_DML_WHERE_ORDER_LIMIT_024.rs) | P1 | memory | GEN_SQL_DML | 1952403 | 3328838 | <span style="color:#dc2626">-70.50%</span> |
| 405 | [01039 JSON_EXTRACT_SET_032](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1039_JSON_EXTRACT_SET_032.rs) | P2 | memory | GEN_SQL_JSON | 1805365 | 3078063 | <span style="color:#dc2626">-70.50%</span> |
| 406 | [00008 STRICT_TABLE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_008_STRICT_TABLE.rs) | P0 | memory | SQL_DDL | 2222995 | 3789769 | <span style="color:#dc2626">-70.48%</span> |
| 407 | [00966 VIEW_TRIGGER_GENERATED_019](crates/bench/sqlite_parity/cases/SQLITE_PARITY_966_VIEW_TRIGGER_GENERATED_019.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2018178 | 3439277 | <span style="color:#dc2626">-70.41%</span> |
| 408 | [00894 CONSTRAINT_FK_SAVEPOINT_027](crates/bench/sqlite_parity/cases/SQLITE_PARITY_894_CONSTRAINT_FK_SAVEPOINT_027.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1983623 | 3377349 | <span style="color:#dc2626">-70.26%</span> |
| 409 | [00686 JOIN_SUBQUERY_EXISTS_079](crates/bench/sqlite_parity/cases/SQLITE_PARITY_686_JOIN_SUBQUERY_EXISTS_079.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2002568 | 3407066 | <span style="color:#dc2626">-70.13%</span> |
| 410 | [01023 JSON_EXTRACT_SET_016](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1023_JSON_EXTRACT_SET_016.rs) | P2 | memory | GEN_SQL_JSON | 1877512 | 3191298 | <span style="color:#dc2626">-69.97%</span> |
| 411 | [00965 VIEW_TRIGGER_GENERATED_018](crates/bench/sqlite_parity/cases/SQLITE_PARITY_965_VIEW_TRIGGER_GENERATED_018.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2010573 | 3415292 | <span style="color:#dc2626">-69.87%</span> |
| 412 | [00491 DML_WHERE_ORDER_LIMIT_104](crates/bench/sqlite_parity/cases/SQLITE_PARITY_491_DML_WHERE_ORDER_LIMIT_104.rs) | P1 | memory | GEN_SQL_DML | 1986017 | 3373071 | <span style="color:#dc2626">-69.84%</span> |
| 413 | [00948 VIEW_TRIGGER_GENERATED_001](crates/bench/sqlite_parity/cases/SQLITE_PARITY_948_VIEW_TRIGGER_GENERATED_001.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2012998 | 3418797 | <span style="color:#dc2626">-69.84%</span> |
| 414 | [00675 JOIN_SUBQUERY_EXISTS_068](crates/bench/sqlite_parity/cases/SQLITE_PARITY_675_JOIN_SUBQUERY_EXISTS_068.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2052854 | 3485715 | <span style="color:#dc2626">-69.80%</span> |
| 415 | [00098 CLI_REGEXP_OPTIONAL](crates/bench/sqlite_parity/cases/SQLITE_PARITY_098_CLI_REGEXP_OPTIONAL.rs) | P2 | memory | CLI_EXTENSION_OPTIONAL | 1717950 | 2917038 | <span style="color:#dc2626">-69.80%</span> |
| 416 | [00478 DML_WHERE_ORDER_LIMIT_091](crates/bench/sqlite_parity/cases/SQLITE_PARITY_478_DML_WHERE_ORDER_LIMIT_091.rs) | P1 | memory | GEN_SQL_DML | 2034950 | 3453203 | <span style="color:#dc2626">-69.69%</span> |
| 417 | [00253 SCALAR_CAST_TYPEOF_007](crates/bench/sqlite_parity/cases/SQLITE_PARITY_253_SCALAR_CAST_TYPEOF_007.rs) | P1 | memory | GEN_SQL_SCALAR | 1702952 | 2888775 | <span style="color:#dc2626">-69.63%</span> |
| 418 | [00428 DML_WHERE_ORDER_LIMIT_041](crates/bench/sqlite_parity/cases/SQLITE_PARITY_428_DML_WHERE_ORDER_LIMIT_041.rs) | P1 | memory | GEN_SQL_DML | 1973042 | 3346802 | <span style="color:#dc2626">-69.63%</span> |
| 419 | [01116 INDEX_SCHEMA_PRAGMA_049](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1116_INDEX_SCHEMA_PRAGMA_049.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 2017416 | 3421723 | <span style="color:#dc2626">-69.61%</span> |
| 420 | [00728 CTE_RECURSIVE_MATRIX_021](crates/bench/sqlite_parity/cases/SQLITE_PARITY_728_CTE_RECURSIVE_MATRIX_021.rs) | P1 | memory | GEN_SQL_CTE | 1856863 | 3149147 | <span style="color:#dc2626">-69.60%</span> |
| 421 | [00610 JOIN_SUBQUERY_EXISTS_003](crates/bench/sqlite_parity/cases/SQLITE_PARITY_610_JOIN_SUBQUERY_EXISTS_003.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2025742 | 3433225 | <span style="color:#dc2626">-69.48%</span> |
| 422 | [00429 DML_WHERE_ORDER_LIMIT_042](crates/bench/sqlite_parity/cases/SQLITE_PARITY_429_DML_WHERE_ORDER_LIMIT_042.rs) | P1 | memory | GEN_SQL_DML | 1987400 | 3367802 | <span style="color:#dc2626">-69.46%</span> |
| 423 | [01115 INDEX_SCHEMA_PRAGMA_048](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1115_INDEX_SCHEMA_PRAGMA_048.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 2021755 | 3425731 | <span style="color:#dc2626">-69.44%</span> |
| 424 | [00608 JOIN_SUBQUERY_EXISTS_001](crates/bench/sqlite_parity/cases/SQLITE_PARITY_608_JOIN_SUBQUERY_EXISTS_001.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2063434 | 3493760 | <span style="color:#dc2626">-69.32%</span> |
| 425 | [00150 DOT_BACKUP_RESTORE_TEMPFILE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_150_DOT_BACKUP_RESTORE_TEMPFILE.rs) | P1 | tempfile | CLI_TEMPFILE | 2644483 | 4473232 | <span style="color:#dc2626">-69.15%</span> |
| 426 | [00028 ALTER_TABLE_RENAME](crates/bench/sqlite_parity/cases/SQLITE_PARITY_028_ALTER_TABLE_RENAME.rs) | P0 | memory | SQL_ALTER | 2138756 | 3617534 | <span style="color:#dc2626">-69.14%</span> |
| 427 | [00976 VIEW_TRIGGER_GENERATED_029](crates/bench/sqlite_parity/cases/SQLITE_PARITY_976_VIEW_TRIGGER_GENERATED_029.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2028147 | 3430109 | <span style="color:#dc2626">-69.13%</span> |
| 428 | [01003 VIEW_TRIGGER_GENERATED_056](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1003_VIEW_TRIGGER_GENERATED_056.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2749962 | 4650037 | <span style="color:#dc2626">-69.09%</span> |
| 429 | [01048 JSON_EXTRACT_SET_041](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1048_JSON_EXTRACT_SET_041.rs) | P2 | memory | GEN_SQL_JSON | 2293128 | 3875061 | <span style="color:#dc2626">-68.99%</span> |
| 430 | [00870 CONSTRAINT_FK_SAVEPOINT_003](crates/bench/sqlite_parity/cases/SQLITE_PARITY_870_CONSTRAINT_FK_SAVEPOINT_003.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2041452 | 3449436 | <span style="color:#dc2626">-68.97%</span> |
| 431 | [00131 DOT_TIMEOUT](crates/bench/sqlite_parity/cases/SQLITE_PARITY_131_DOT_TIMEOUT.rs) | P0 | memory | CLI_DOT_COMMAND | 1750812 | 2958317 | <span style="color:#dc2626">-68.97%</span> |
| 432 | [00700 JOIN_SUBQUERY_EXISTS_093](crates/bench/sqlite_parity/cases/SQLITE_PARITY_700_JOIN_SUBQUERY_EXISTS_093.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2034428 | 3437272 | <span style="color:#dc2626">-68.96%</span> |
| 433 | [00949 VIEW_TRIGGER_GENERATED_002](crates/bench/sqlite_parity/cases/SQLITE_PARITY_949_VIEW_TRIGGER_GENERATED_002.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2030902 | 3431081 | <span style="color:#dc2626">-68.94%</span> |
| 434 | [00283 SCALAR_NULL_COALESCE_014](crates/bench/sqlite_parity/cases/SQLITE_PARITY_283_SCALAR_NULL_COALESCE_014.rs) | P1 | memory | GEN_SQL_SCALAR | 1690848 | 2856173 | <span style="color:#dc2626">-68.92%</span> |
| 435 | [00432 DML_WHERE_ORDER_LIMIT_045](crates/bench/sqlite_parity/cases/SQLITE_PARITY_432_DML_WHERE_ORDER_LIMIT_045.rs) | P1 | memory | GEN_SQL_DML | 2009291 | 3393881 | <span style="color:#dc2626">-68.91%</span> |
| 436 | [00186 OPT_NEWLINE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_186_OPT_NEWLINE.rs) | P2 | memory | CLI_OPTION | 1725744 | 2914494 | <span style="color:#dc2626">-68.88%</span> |
| 437 | [00240 SCALAR_ARITH_004](crates/bench/sqlite_parity/cases/SQLITE_PARITY_240_SCALAR_ARITH_004.rs) | P1 | memory | GEN_SQL_SCALAR | 1668527 | 2817700 | <span style="color:#dc2626">-68.87%</span> |
| 438 | [01098 INDEX_SCHEMA_PRAGMA_031](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1098_INDEX_SCHEMA_PRAGMA_031.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 2046261 | 3455507 | <span style="color:#dc2626">-68.87%</span> |
| 439 | [00655 JOIN_SUBQUERY_EXISTS_048](crates/bench/sqlite_parity/cases/SQLITE_PARITY_655_JOIN_SUBQUERY_EXISTS_048.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2500640 | 4222097 | <span style="color:#dc2626">-68.84%</span> |
| 440 | [00519 AGG_GROUP_HAVING_012](crates/bench/sqlite_parity/cases/SQLITE_PARITY_519_AGG_GROUP_HAVING_012.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1983372 | 3347543 | <span style="color:#dc2626">-68.78%</span> |
| 441 | [00564 AGG_GROUP_HAVING_057](crates/bench/sqlite_parity/cases/SQLITE_PARITY_564_AGG_GROUP_HAVING_057.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1987600 | 3354386 | <span style="color:#dc2626">-68.77%</span> |
| 442 | [00527 AGG_GROUP_HAVING_020](crates/bench/sqlite_parity/cases/SQLITE_PARITY_527_AGG_GROUP_HAVING_020.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2044517 | 3450267 | <span style="color:#dc2626">-68.76%</span> |
| 443 | [00398 DML_WHERE_ORDER_LIMIT_011](crates/bench/sqlite_parity/cases/SQLITE_PARITY_398_DML_WHERE_ORDER_LIMIT_011.rs) | P1 | memory | GEN_SQL_DML | 2047874 | 3454665 | <span style="color:#dc2626">-68.70%</span> |
| 444 | [00908 CONSTRAINT_FK_SAVEPOINT_041](crates/bench/sqlite_parity/cases/SQLITE_PARITY_908_CONSTRAINT_FK_SAVEPOINT_041.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2011034 | 3392398 | <span style="color:#dc2626">-68.69%</span> |
| 445 | [00638 JOIN_SUBQUERY_EXISTS_031](crates/bench/sqlite_parity/cases/SQLITE_PARITY_638_JOIN_SUBQUERY_EXISTS_031.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2080486 | 3509018 | <span style="color:#dc2626">-68.66%</span> |
| 446 | [00148 DOT_OUTPUT_TEMPFILE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_148_DOT_OUTPUT_TEMPFILE.rs) | P1 | tempfile | CLI_TEMPFILE | 1853607 | 3125603 | <span style="color:#dc2626">-68.62%</span> |
| 447 | [00807 WINDOW_PARTITION_SUM_020](crates/bench/sqlite_parity/cases/SQLITE_PARITY_807_WINDOW_PARTITION_SUM_020.rs) | P2 | memory | GEN_SQL_WINDOW | 1911196 | 3221585 | <span style="color:#dc2626">-68.56%</span> |
| 448 | [00987 VIEW_TRIGGER_GENERATED_040](crates/bench/sqlite_parity/cases/SQLITE_PARITY_987_VIEW_TRIGGER_GENERATED_040.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2709045 | 4565828 | <span style="color:#dc2626">-68.54%</span> |
| 449 | [01099 INDEX_SCHEMA_PRAGMA_032](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1099_INDEX_SCHEMA_PRAGMA_032.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 2040200 | 3438344 | <span style="color:#dc2626">-68.53%</span> |
| 450 | [01032 JSON_EXTRACT_SET_025](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1032_JSON_EXTRACT_SET_025.rs) | P2 | memory | GEN_SQL_JSON | 1940562 | 3269185 | <span style="color:#dc2626">-68.47%</span> |
| 451 | [00127 DOT_LIMIT](crates/bench/sqlite_parity/cases/SQLITE_PARITY_127_DOT_LIMIT.rs) | P0 | memory | CLI_DOT_COMMAND | 1783133 | 3002851 | <span style="color:#dc2626">-68.40%</span> |
| 452 | [00649 JOIN_SUBQUERY_EXISTS_042](crates/bench/sqlite_parity/cases/SQLITE_PARITY_649_JOIN_SUBQUERY_EXISTS_042.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2091597 | 3522203 | <span style="color:#dc2626">-68.40%</span> |
| 453 | [00563 AGG_GROUP_HAVING_056](crates/bench/sqlite_parity/cases/SQLITE_PARITY_563_AGG_GROUP_HAVING_056.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2007237 | 3379904 | <span style="color:#dc2626">-68.39%</span> |
| 454 | [00576 AGG_GROUP_HAVING_069](crates/bench/sqlite_parity/cases/SQLITE_PARITY_576_AGG_GROUP_HAVING_069.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2040119 | 3433796 | <span style="color:#dc2626">-68.31%</span> |
| 455 | [01056 JSON_EXTRACT_SET_049](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1056_JSON_EXTRACT_SET_049.rs) | P2 | memory | GEN_SQL_JSON | 2143675 | 3605471 | <span style="color:#dc2626">-68.19%</span> |
| 456 | [01027 JSON_EXTRACT_SET_020](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1027_JSON_EXTRACT_SET_020.rs) | P2 | memory | GEN_SQL_JSON | 1901056 | 3194052 | <span style="color:#dc2626">-68.01%</span> |
| 457 | [00969 VIEW_TRIGGER_GENERATED_022](crates/bench/sqlite_parity/cases/SQLITE_PARITY_969_VIEW_TRIGGER_GENERATED_022.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2012768 | 3380916 | <span style="color:#dc2626">-67.97%</span> |
| 458 | [00308 SCALAR_ARITH_021](crates/bench/sqlite_parity/cases/SQLITE_PARITY_308_SCALAR_ARITH_021.rs) | P1 | memory | GEN_SQL_SCALAR | 1703362 | 2861072 | <span style="color:#dc2626">-67.97%</span> |
| 459 | [00864 WINDOW_PARTITION_SUM_077](crates/bench/sqlite_parity/cases/SQLITE_PARITY_864_WINDOW_PARTITION_SUM_077.rs) | P2 | memory | GEN_SQL_WINDOW | 2332794 | 3918193 | <span style="color:#dc2626">-67.96%</span> |
| 460 | [01122 INDEX_SCHEMA_PRAGMA_055](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1122_INDEX_SCHEMA_PRAGMA_055.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 2068724 | 3474093 | <span style="color:#dc2626">-67.93%</span> |
| 461 | [01059 JSON_EXTRACT_SET_052](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1059_JSON_EXTRACT_SET_052.rs) | P2 | memory | GEN_SQL_JSON | 1943517 | 3263213 | <span style="color:#dc2626">-67.90%</span> |
| 462 | [00331 SCALAR_NULL_COALESCE_026](crates/bench/sqlite_parity/cases/SQLITE_PARITY_331_SCALAR_NULL_COALESCE_026.rs) | P1 | memory | GEN_SQL_SCALAR | 1712931 | 2875971 | <span style="color:#dc2626">-67.90%</span> |
| 463 | [00301 SCALAR_CAST_TYPEOF_019](crates/bench/sqlite_parity/cases/SQLITE_PARITY_301_SCALAR_CAST_TYPEOF_019.rs) | P1 | memory | GEN_SQL_SCALAR | 1744871 | 2928410 | <span style="color:#dc2626">-67.83%</span> |
| 464 | [00278 SCALAR_STRING_013](crates/bench/sqlite_parity/cases/SQLITE_PARITY_278_SCALAR_STRING_013.rs) | P1 | memory | GEN_SQL_SCALAR | 1911686 | 3207558 | <span style="color:#dc2626">-67.79%</span> |
| 465 | [00233 SCALAR_CAST_TYPEOF_002](crates/bench/sqlite_parity/cases/SQLITE_PARITY_233_SCALAR_CAST_TYPEOF_002.rs) | P1 | memory | GEN_SQL_SCALAR | 1723531 | 2890739 | <span style="color:#dc2626">-67.72%</span> |
| 466 | [00443 DML_WHERE_ORDER_LIMIT_056](crates/bench/sqlite_parity/cases/SQLITE_PARITY_443_DML_WHERE_ORDER_LIMIT_056.rs) | P1 | memory | GEN_SQL_DML | 1961631 | 3288942 | <span style="color:#dc2626">-67.66%</span> |
| 467 | [00271 SCALAR_NULL_COALESCE_011](crates/bench/sqlite_parity/cases/SQLITE_PARITY_271_SCALAR_NULL_COALESCE_011.rs) | P1 | memory | GEN_SQL_SCALAR | 1714604 | 2874087 | <span style="color:#dc2626">-67.62%</span> |
| 468 | [01113 INDEX_SCHEMA_PRAGMA_046](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1113_INDEX_SCHEMA_PRAGMA_046.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 2436029 | 4083094 | <span style="color:#dc2626">-67.61%</span> |
| 469 | [00770 CTE_RECURSIVE_MATRIX_063](crates/bench/sqlite_parity/cases/SQLITE_PARITY_770_CTE_RECURSIVE_MATRIX_063.rs) | P1 | memory | GEN_SQL_CTE | 2300272 | 3853079 | <span style="color:#dc2626">-67.51%</span> |
| 470 | [00930 CONSTRAINT_FK_SAVEPOINT_063](crates/bench/sqlite_parity/cases/SQLITE_PARITY_930_CONSTRAINT_FK_SAVEPOINT_063.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1996727 | 3344447 | <span style="color:#dc2626">-67.50%</span> |
| 471 | [00484 DML_WHERE_ORDER_LIMIT_097](crates/bench/sqlite_parity/cases/SQLITE_PARITY_484_DML_WHERE_ORDER_LIMIT_097.rs) | P1 | memory | GEN_SQL_DML | 2025141 | 3391526 | <span style="color:#dc2626">-67.47%</span> |
| 472 | [00270 SCALAR_STRING_011](crates/bench/sqlite_parity/cases/SQLITE_PARITY_270_SCALAR_STRING_011.rs) | P1 | memory | GEN_SQL_SCALAR | 1711477 | 2865951 | <span style="color:#dc2626">-67.45%</span> |
| 473 | [00146 DOT_READ_TEMPFILE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_146_DOT_READ_TEMPFILE.rs) | P1 | tempfile | CLI_TEMPFILE | 1780288 | 2980729 | <span style="color:#dc2626">-67.43%</span> |
| 474 | [00869 CONSTRAINT_FK_SAVEPOINT_002](crates/bench/sqlite_parity/cases/SQLITE_PARITY_869_CONSTRAINT_FK_SAVEPOINT_002.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2028608 | 3396015 | <span style="color:#dc2626">-67.41%</span> |
| 475 | [00499 DML_WHERE_ORDER_LIMIT_112](crates/bench/sqlite_parity/cases/SQLITE_PARITY_499_DML_WHERE_ORDER_LIMIT_112.rs) | P1 | memory | GEN_SQL_DML | 2026925 | 3392698 | <span style="color:#dc2626">-67.38%</span> |
| 476 | [00585 AGG_GROUP_HAVING_078](crates/bench/sqlite_parity/cases/SQLITE_PARITY_585_AGG_GROUP_HAVING_078.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2493718 | 4173926 | <span style="color:#dc2626">-67.38%</span> |
| 477 | [00487 DML_WHERE_ORDER_LIMIT_100](crates/bench/sqlite_parity/cases/SQLITE_PARITY_487_DML_WHERE_ORDER_LIMIT_100.rs) | P1 | memory | GEN_SQL_DML | 2011164 | 3365026 | <span style="color:#dc2626">-67.32%</span> |
| 478 | [00992 VIEW_TRIGGER_GENERATED_045](crates/bench/sqlite_parity/cases/SQLITE_PARITY_992_VIEW_TRIGGER_GENERATED_045.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2079634 | 3477359 | <span style="color:#dc2626">-67.21%</span> |
| 479 | [00945 CONSTRAINT_FK_SAVEPOINT_078](crates/bench/sqlite_parity/cases/SQLITE_PARITY_945_CONSTRAINT_FK_SAVEPOINT_078.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2036192 | 3404360 | <span style="color:#dc2626">-67.19%</span> |
| 480 | [00588 AGG_GROUP_HAVING_081](crates/bench/sqlite_parity/cases/SQLITE_PARITY_588_AGG_GROUP_HAVING_081.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2034449 | 3400253 | <span style="color:#dc2626">-67.13%</span> |
| 481 | [00957 VIEW_TRIGGER_GENERATED_010](crates/bench/sqlite_parity/cases/SQLITE_PARITY_957_VIEW_TRIGGER_GENERATED_010.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2095164 | 3501344 | <span style="color:#dc2626">-67.12%</span> |
| 482 | [00245 SCALAR_CAST_TYPEOF_005](crates/bench/sqlite_parity/cases/SQLITE_PARITY_245_SCALAR_CAST_TYPEOF_005.rs) | P1 | memory | GEN_SQL_SCALAR | 1775749 | 2967364 | <span style="color:#dc2626">-67.10%</span> |
| 483 | [00682 JOIN_SUBQUERY_EXISTS_075](crates/bench/sqlite_parity/cases/SQLITE_PARITY_682_JOIN_SUBQUERY_EXISTS_075.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2276677 | 3804247 | <span style="color:#dc2626">-67.10%</span> |
| 484 | [00868 CONSTRAINT_FK_SAVEPOINT_001](crates/bench/sqlite_parity/cases/SQLITE_PARITY_868_CONSTRAINT_FK_SAVEPOINT_001.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2002037 | 3345219 | <span style="color:#dc2626">-67.09%</span> |
| 485 | [00286 SCALAR_STRING_015](crates/bench/sqlite_parity/cases/SQLITE_PARITY_286_SCALAR_STRING_015.rs) | P1 | memory | GEN_SQL_SCALAR | 1696800 | 2834492 | <span style="color:#dc2626">-67.05%</span> |
| 486 | [00554 AGG_GROUP_HAVING_047](crates/bench/sqlite_parity/cases/SQLITE_PARITY_554_AGG_GROUP_HAVING_047.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2047043 | 3419379 | <span style="color:#dc2626">-67.04%</span> |
| 487 | [00238 SCALAR_STRING_003](crates/bench/sqlite_parity/cases/SQLITE_PARITY_238_SCALAR_STRING_003.rs) | P1 | memory | GEN_SQL_SCALAR | 1728910 | 2886591 | <span style="color:#dc2626">-66.96%</span> |
| 488 | [00250 SCALAR_STRING_006](crates/bench/sqlite_parity/cases/SQLITE_PARITY_250_SCALAR_STRING_006.rs) | P1 | memory | GEN_SQL_SCALAR | 2056200 | 3430670 | <span style="color:#dc2626">-66.85%</span> |
| 489 | [00918 CONSTRAINT_FK_SAVEPOINT_051](crates/bench/sqlite_parity/cases/SQLITE_PARITY_918_CONSTRAINT_FK_SAVEPOINT_051.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1965308 | 3278693 | <span style="color:#dc2626">-66.83%</span> |
| 490 | [00149 DOT_ONCE_TEMPFILE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_149_DOT_ONCE_TEMPFILE.rs) | P1 | tempfile | CLI_TEMPFILE | 1869626 | 3118760 | <span style="color:#dc2626">-66.81%</span> |
| 491 | [00947 CONSTRAINT_FK_SAVEPOINT_080](crates/bench/sqlite_parity/cases/SQLITE_PARITY_947_CONSTRAINT_FK_SAVEPOINT_080.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2079564 | 3468381 | <span style="color:#dc2626">-66.78%</span> |
| 492 | [00970 VIEW_TRIGGER_GENERATED_023](crates/bench/sqlite_parity/cases/SQLITE_PARITY_970_VIEW_TRIGGER_GENERATED_023.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2138646 | 3565395 | <span style="color:#dc2626">-66.71%</span> |
| 493 | [00338 SCALAR_STRING_028](crates/bench/sqlite_parity/cases/SQLITE_PARITY_338_SCALAR_STRING_028.rs) | P1 | memory | GEN_SQL_SCALAR | 1678345 | 2797583 | <span style="color:#dc2626">-66.69%</span> |
| 494 | [00288 SCALAR_ARITH_016](crates/bench/sqlite_parity/cases/SQLITE_PARITY_288_SCALAR_ARITH_016.rs) | P1 | memory | GEN_SQL_SCALAR | 1726847 | 2876161 | <span style="color:#dc2626">-66.56%</span> |
| 495 | [00394 DML_WHERE_ORDER_LIMIT_007](crates/bench/sqlite_parity/cases/SQLITE_PARITY_394_DML_WHERE_ORDER_LIMIT_007.rs) | P1 | memory | GEN_SQL_DML | 2018318 | 3361379 | <span style="color:#dc2626">-66.54%</span> |
| 496 | [00243 SCALAR_NULL_COALESCE_004](crates/bench/sqlite_parity/cases/SQLITE_PARITY_243_SCALAR_NULL_COALESCE_004.rs) | P1 | memory | GEN_SQL_SCALAR | 1683645 | 2803815 | <span style="color:#dc2626">-66.53%</span> |
| 497 | [00900 CONSTRAINT_FK_SAVEPOINT_033](crates/bench/sqlite_parity/cases/SQLITE_PARITY_900_CONSTRAINT_FK_SAVEPOINT_033.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2069355 | 3445057 | <span style="color:#dc2626">-66.48%</span> |
| 498 | [00304 SCALAR_ARITH_020](crates/bench/sqlite_parity/cases/SQLITE_PARITY_304_SCALAR_ARITH_020.rs) | P1 | memory | GEN_SQL_SCALAR | 1686390 | 2807070 | <span style="color:#dc2626">-66.45%</span> |
| 499 | [00898 CONSTRAINT_FK_SAVEPOINT_031](crates/bench/sqlite_parity/cases/SQLITE_PARITY_898_CONSTRAINT_FK_SAVEPOINT_031.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1993532 | 3316344 | <span style="color:#dc2626">-66.36%</span> |
| 500 | [00518 AGG_GROUP_HAVING_011](crates/bench/sqlite_parity/cases/SQLITE_PARITY_518_AGG_GROUP_HAVING_011.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1935782 | 3219812 | <span style="color:#dc2626">-66.33%</span> |
| 501 | [00783 CTE_RECURSIVE_MATRIX_076](crates/bench/sqlite_parity/cases/SQLITE_PARITY_783_CTE_RECURSIVE_MATRIX_076.rs) | P1 | memory | GEN_SQL_CTE | 1958766 | 3255960 | <span style="color:#dc2626">-66.23%</span> |
| 502 | [01083 INDEX_SCHEMA_PRAGMA_016](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1083_INDEX_SCHEMA_PRAGMA_016.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 2088060 | 3470816 | <span style="color:#dc2626">-66.22%</span> |
| 503 | [00582 AGG_GROUP_HAVING_075](crates/bench/sqlite_parity/cases/SQLITE_PARITY_582_AGG_GROUP_HAVING_075.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2011756 | 3342243 | <span style="color:#dc2626">-66.14%</span> |
| 504 | [00550 AGG_GROUP_HAVING_043](crates/bench/sqlite_parity/cases/SQLITE_PARITY_550_AGG_GROUP_HAVING_043.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2030361 | 3372951 | <span style="color:#dc2626">-66.13%</span> |
| 505 | [00023 TRANSACTION_ROLLBACK](crates/bench/sqlite_parity/cases/SQLITE_PARITY_023_TRANSACTION_ROLLBACK.rs) | P0 | memory | SQL_TRANSACTION | 1806147 | 2999895 | <span style="color:#dc2626">-66.09%</span> |
| 506 | [00968 VIEW_TRIGGER_GENERATED_021](crates/bench/sqlite_parity/cases/SQLITE_PARITY_968_VIEW_TRIGGER_GENERATED_021.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2021284 | 3357191 | <span style="color:#dc2626">-66.09%</span> |
| 507 | [00914 CONSTRAINT_FK_SAVEPOINT_047](crates/bench/sqlite_parity/cases/SQLITE_PARITY_914_CONSTRAINT_FK_SAVEPOINT_047.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1971259 | 3271740 | <span style="color:#dc2626">-65.97%</span> |
| 508 | [00524 AGG_GROUP_HAVING_017](crates/bench/sqlite_parity/cases/SQLITE_PARITY_524_AGG_GROUP_HAVING_017.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2022005 | 3355788 | <span style="color:#dc2626">-65.96%</span> |
| 509 | [00311 SCALAR_NULL_COALESCE_021](crates/bench/sqlite_parity/cases/SQLITE_PARITY_311_SCALAR_NULL_COALESCE_021.rs) | P1 | memory | GEN_SQL_SCALAR | 1726276 | 2864820 | <span style="color:#dc2626">-65.95%</span> |
| 510 | [00130 DOT_OPEN_MEMORY](crates/bench/sqlite_parity/cases/SQLITE_PARITY_130_DOT_OPEN_MEMORY.rs) | P0 | memory | CLI_DOT_COMMAND | 1911466 | 3171710 | <span style="color:#dc2626">-65.93%</span> |
| 511 | [00606 AGG_GROUP_HAVING_099](crates/bench/sqlite_parity/cases/SQLITE_PARITY_606_AGG_GROUP_HAVING_099.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2025051 | 3359676 | <span style="color:#dc2626">-65.91%</span> |
| 512 | [00446 DML_WHERE_ORDER_LIMIT_059](crates/bench/sqlite_parity/cases/SQLITE_PARITY_446_DML_WHERE_ORDER_LIMIT_059.rs) | P1 | memory | GEN_SQL_DML | 1983503 | 3290565 | <span style="color:#dc2626">-65.90%</span> |
| 513 | [00817 WINDOW_PARTITION_SUM_030](crates/bench/sqlite_parity/cases/SQLITE_PARITY_817_WINDOW_PARTITION_SUM_030.rs) | P2 | memory | GEN_SQL_WINDOW | 2060918 | 3418788 | <span style="color:#dc2626">-65.89%</span> |
| 514 | [00538 AGG_GROUP_HAVING_031](crates/bench/sqlite_parity/cases/SQLITE_PARITY_538_AGG_GROUP_HAVING_031.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1990115 | 3300594 | <span style="color:#dc2626">-65.85%</span> |
| 515 | [00955 VIEW_TRIGGER_GENERATED_008](crates/bench/sqlite_parity/cases/SQLITE_PARITY_955_VIEW_TRIGGER_GENERATED_008.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2105583 | 3490484 | <span style="color:#dc2626">-65.77%</span> |
| 516 | [00964 VIEW_TRIGGER_GENERATED_017](crates/bench/sqlite_parity/cases/SQLITE_PARITY_964_VIEW_TRIGGER_GENERATED_017.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2034940 | 3371889 | <span style="color:#dc2626">-65.70%</span> |
| 517 | [00812 WINDOW_PARTITION_SUM_025](crates/bench/sqlite_parity/cases/SQLITE_PARITY_812_WINDOW_PARTITION_SUM_025.rs) | P2 | memory | GEN_SQL_WINDOW | 1947615 | 3226955 | <span style="color:#dc2626">-65.69%</span> |
| 518 | [01002 VIEW_TRIGGER_GENERATED_055](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1002_VIEW_TRIGGER_GENERATED_055.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2026774 | 3358082 | <span style="color:#dc2626">-65.69%</span> |
| 519 | [00265 SCALAR_CAST_TYPEOF_010](crates/bench/sqlite_parity/cases/SQLITE_PARITY_265_SCALAR_CAST_TYPEOF_010.rs) | P1 | memory | GEN_SQL_SCALAR | 1708391 | 2830204 | <span style="color:#dc2626">-65.66%</span> |
| 520 | [00962 VIEW_TRIGGER_GENERATED_015](crates/bench/sqlite_parity/cases/SQLITE_PARITY_962_VIEW_TRIGGER_GENERATED_015.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2090395 | 3462069 | <span style="color:#dc2626">-65.62%</span> |
| 521 | [00513 AGG_GROUP_HAVING_006](crates/bench/sqlite_parity/cases/SQLITE_PARITY_513_AGG_GROUP_HAVING_006.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2062932 | 3416273 | <span style="color:#dc2626">-65.60%</span> |
| 522 | [00237 SCALAR_CAST_TYPEOF_003](crates/bench/sqlite_parity/cases/SQLITE_PARITY_237_SCALAR_CAST_TYPEOF_003.rs) | P1 | memory | GEN_SQL_SCALAR | 1706849 | 2825636 | <span style="color:#dc2626">-65.55%</span> |
| 523 | [00738 CTE_RECURSIVE_MATRIX_031](crates/bench/sqlite_parity/cases/SQLITE_PARITY_738_CTE_RECURSIVE_MATRIX_031.rs) | P1 | memory | GEN_SQL_CTE | 2315911 | 3833412 | <span style="color:#dc2626">-65.53%</span> |
| 524 | [00437 DML_WHERE_ORDER_LIMIT_050](crates/bench/sqlite_parity/cases/SQLITE_PARITY_437_DML_WHERE_ORDER_LIMIT_050.rs) | P1 | memory | GEN_SQL_DML | 2121834 | 3510912 | <span style="color:#dc2626">-65.47%</span> |
| 525 | [01070 INDEX_SCHEMA_PRAGMA_003](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1070_INDEX_SCHEMA_PRAGMA_003.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 2071459 | 3427314 | <span style="color:#dc2626">-65.45%</span> |
| 526 | [00873 CONSTRAINT_FK_SAVEPOINT_006](crates/bench/sqlite_parity/cases/SQLITE_PARITY_873_CONSTRAINT_FK_SAVEPOINT_006.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2008109 | 3321724 | <span style="color:#dc2626">-65.42%</span> |
| 527 | [00117 DOT_DUMP](crates/bench/sqlite_parity/cases/SQLITE_PARITY_117_DOT_DUMP.rs) | P0 | memory | CLI_DOT_COMMAND | 1895516 | 3134991 | <span style="color:#dc2626">-65.39%</span> |
| 528 | [00861 WINDOW_PARTITION_SUM_074](crates/bench/sqlite_parity/cases/SQLITE_PARITY_861_WINDOW_PARTITION_SUM_074.rs) | P2 | memory | GEN_SQL_WINDOW | 1977891 | 3271148 | <span style="color:#dc2626">-65.39%</span> |
| 529 | [00883 CONSTRAINT_FK_SAVEPOINT_016](crates/bench/sqlite_parity/cases/SQLITE_PARITY_883_CONSTRAINT_FK_SAVEPOINT_016.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2049126 | 3388951 | <span style="color:#dc2626">-65.39%</span> |
| 530 | [01107 INDEX_SCHEMA_PRAGMA_040](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1107_INDEX_SCHEMA_PRAGMA_040.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 2054236 | 3397237 | <span style="color:#dc2626">-65.38%</span> |
| 531 | [00729 CTE_RECURSIVE_MATRIX_022](crates/bench/sqlite_parity/cases/SQLITE_PARITY_729_CTE_RECURSIVE_MATRIX_022.rs) | P1 | memory | GEN_SQL_CTE | 1873304 | 3096768 | <span style="color:#dc2626">-65.31%</span> |
| 532 | [00834 WINDOW_PARTITION_SUM_047](crates/bench/sqlite_parity/cases/SQLITE_PARITY_834_WINDOW_PARTITION_SUM_047.rs) | P2 | memory | GEN_SQL_WINDOW | 2018799 | 3337083 | <span style="color:#dc2626">-65.30%</span> |
| 533 | [00223 OPT_NOHEADER](crates/bench/sqlite_parity/cases/SQLITE_PARITY_223_OPT_NOHEADER.rs) | P2 | memory | CLI_OPTION | 1689937 | 2793235 | <span style="color:#dc2626">-65.29%</span> |
| 534 | [00488 DML_WHERE_ORDER_LIMIT_101](crates/bench/sqlite_parity/cases/SQLITE_PARITY_488_DML_WHERE_ORDER_LIMIT_101.rs) | P1 | memory | GEN_SQL_DML | 1997669 | 3301365 | <span style="color:#dc2626">-65.26%</span> |
| 535 | [00953 VIEW_TRIGGER_GENERATED_006](crates/bench/sqlite_parity/cases/SQLITE_PARITY_953_VIEW_TRIGGER_GENERATED_006.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2200513 | 3636299 | <span style="color:#dc2626">-65.25%</span> |
| 536 | [00113 DOT_NULLVALUE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_113_DOT_NULLVALUE.rs) | P0 | memory | CLI_DOT_COMMAND | 1704575 | 2816368 | <span style="color:#dc2626">-65.22%</span> |
| 537 | [00928 CONSTRAINT_FK_SAVEPOINT_061](crates/bench/sqlite_parity/cases/SQLITE_PARITY_928_CONSTRAINT_FK_SAVEPOINT_061.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1938417 | 3202318 | <span style="color:#dc2626">-65.20%</span> |
| 538 | [00715 CTE_RECURSIVE_MATRIX_008](crates/bench/sqlite_parity/cases/SQLITE_PARITY_715_CTE_RECURSIVE_MATRIX_008.rs) | P1 | memory | GEN_SQL_CTE | 1884074 | 3112227 | <span style="color:#dc2626">-65.19%</span> |
| 539 | [00320 SCALAR_ARITH_024](crates/bench/sqlite_parity/cases/SQLITE_PARITY_320_SCALAR_ARITH_024.rs) | P1 | memory | GEN_SQL_SCALAR | 1692702 | 2795549 | <span style="color:#dc2626">-65.15%</span> |
| 540 | [00897 CONSTRAINT_FK_SAVEPOINT_030](crates/bench/sqlite_parity/cases/SQLITE_PARITY_897_CONSTRAINT_FK_SAVEPOINT_030.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2110593 | 3485414 | <span style="color:#dc2626">-65.14%</span> |
| 541 | [00634 JOIN_SUBQUERY_EXISTS_027](crates/bench/sqlite_parity/cases/SQLITE_PARITY_634_JOIN_SUBQUERY_EXISTS_027.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2069355 | 3417215 | <span style="color:#dc2626">-65.13%</span> |
| 542 | [00798 WINDOW_PARTITION_SUM_011](crates/bench/sqlite_parity/cases/SQLITE_PARITY_798_WINDOW_PARTITION_SUM_011.rs) | P2 | memory | GEN_SQL_WINDOW | 2530748 | 4177623 | <span style="color:#dc2626">-65.07%</span> |
| 543 | [01061 JSON_EXTRACT_SET_054](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1061_JSON_EXTRACT_SET_054.rs) | P2 | memory | GEN_SQL_JSON | 1978212 | 3264756 | <span style="color:#dc2626">-65.04%</span> |
| 544 | [00305 SCALAR_CAST_TYPEOF_020](crates/bench/sqlite_parity/cases/SQLITE_PARITY_305_SCALAR_CAST_TYPEOF_020.rs) | P1 | memory | GEN_SQL_SCALAR | 1755561 | 2897161 | <span style="color:#dc2626">-65.03%</span> |
| 545 | [00314 SCALAR_STRING_022](crates/bench/sqlite_parity/cases/SQLITE_PARITY_314_SCALAR_STRING_022.rs) | P1 | memory | GEN_SQL_SCALAR | 1772793 | 2925424 | <span style="color:#dc2626">-65.02%</span> |
| 546 | [00229 SCALAR_CAST_TYPEOF_001](crates/bench/sqlite_parity/cases/SQLITE_PARITY_229_SCALAR_CAST_TYPEOF_001.rs) | P1 | memory | GEN_SQL_SCALAR | 1785828 | 2946684 | <span style="color:#dc2626">-65.00%</span> |
| 547 | [00532 AGG_GROUP_HAVING_025](crates/bench/sqlite_parity/cases/SQLITE_PARITY_532_AGG_GROUP_HAVING_025.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1977621 | 3262882 | <span style="color:#dc2626">-64.99%</span> |
| 548 | [00521 AGG_GROUP_HAVING_014](crates/bench/sqlite_parity/cases/SQLITE_PARITY_521_AGG_GROUP_HAVING_014.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2039338 | 3364575 | <span style="color:#dc2626">-64.98%</span> |
| 549 | [01028 JSON_EXTRACT_SET_021](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1028_JSON_EXTRACT_SET_021.rs) | P2 | memory | GEN_SQL_JSON | 1867854 | 3081630 | <span style="color:#dc2626">-64.98%</span> |
| 550 | [00545 AGG_GROUP_HAVING_038](crates/bench/sqlite_parity/cases/SQLITE_PARITY_545_AGG_GROUP_HAVING_038.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2126142 | 3507336 | <span style="color:#dc2626">-64.96%</span> |
| 551 | [00867 WINDOW_PARTITION_SUM_080](crates/bench/sqlite_parity/cases/SQLITE_PARITY_867_WINDOW_PARTITION_SUM_080.rs) | P2 | memory | GEN_SQL_WINDOW | 1928739 | 3179094 | <span style="color:#dc2626">-64.83%</span> |
| 552 | [00514 AGG_GROUP_HAVING_007](crates/bench/sqlite_parity/cases/SQLITE_PARITY_514_AGG_GROUP_HAVING_007.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1968684 | 3241302 | <span style="color:#dc2626">-64.64%</span> |
| 553 | [00001 SELECT_CORE_EXPRESSIONS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_001_SELECT_CORE_EXPRESSIONS.rs) | P0 | memory | SQL_SELECT | 1754248 | 2888083 | <span style="color:#dc2626">-64.63%</span> |
| 554 | [00913 CONSTRAINT_FK_SAVEPOINT_046](crates/bench/sqlite_parity/cases/SQLITE_PARITY_913_CONSTRAINT_FK_SAVEPOINT_046.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1947364 | 3204873 | <span style="color:#dc2626">-64.57%</span> |
| 555 | [00180 OPT_BOX_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_180_OPT_BOX_MODE.rs) | P2 | memory | CLI_OPTION | 1699235 | 2796030 | <span style="color:#dc2626">-64.55%</span> |
| 556 | [00072 ORDER_BY_NULLS_FIRST_LAST](crates/bench/sqlite_parity/cases/SQLITE_PARITY_072_ORDER_BY_NULLS_FIRST_LAST.rs) | P0 | memory | SQL_SELECT | 1795677 | 2954429 | <span style="color:#dc2626">-64.53%</span> |
| 557 | [00073 INDEXED_BY](crates/bench/sqlite_parity/cases/SQLITE_PARITY_073_INDEXED_BY.rs) | P0 | memory | SQL_INDEX | 1892630 | 3111637 | <span style="color:#dc2626">-64.41%</span> |
| 558 | [00961 VIEW_TRIGGER_GENERATED_014](crates/bench/sqlite_parity/cases/SQLITE_PARITY_961_VIEW_TRIGGER_GENERATED_014.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2040490 | 3354557 | <span style="color:#dc2626">-64.40%</span> |
| 559 | [00335 SCALAR_NULL_COALESCE_027](crates/bench/sqlite_parity/cases/SQLITE_PARITY_335_SCALAR_NULL_COALESCE_027.rs) | P1 | memory | GEN_SQL_SCALAR | 1719192 | 2824573 | <span style="color:#dc2626">-64.30%</span> |
| 560 | [00568 AGG_GROUP_HAVING_061](crates/bench/sqlite_parity/cases/SQLITE_PARITY_568_AGG_GROUP_HAVING_061.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1962192 | 3223418 | <span style="color:#dc2626">-64.28%</span> |
| 561 | [00517 AGG_GROUP_HAVING_010](crates/bench/sqlite_parity/cases/SQLITE_PARITY_517_AGG_GROUP_HAVING_010.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2000334 | 3285886 | <span style="color:#dc2626">-64.27%</span> |
| 562 | [01009 JSON_EXTRACT_SET_002](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1009_JSON_EXTRACT_SET_002.rs) | P2 | memory | GEN_SQL_JSON | 1920754 | 3152784 | <span style="color:#dc2626">-64.14%</span> |
| 563 | [00247 SCALAR_NULL_COALESCE_005](crates/bench/sqlite_parity/cases/SQLITE_PARITY_247_SCALAR_NULL_COALESCE_005.rs) | P1 | memory | GEN_SQL_SCALAR | 1699826 | 2789998 | <span style="color:#dc2626">-64.13%</span> |
| 564 | [00381 SCALAR_CAST_TYPEOF_039](crates/bench/sqlite_parity/cases/SQLITE_PARITY_381_SCALAR_CAST_TYPEOF_039.rs) | P1 | memory | GEN_SQL_SCALAR | 1802510 | 2958446 | <span style="color:#dc2626">-64.13%</span> |
| 565 | [00958 VIEW_TRIGGER_GENERATED_011](crates/bench/sqlite_parity/cases/SQLITE_PARITY_958_VIEW_TRIGGER_GENERATED_011.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2123748 | 3485404 | <span style="color:#dc2626">-64.12%</span> |
| 566 | [00988 VIEW_TRIGGER_GENERATED_041](crates/bench/sqlite_parity/cases/SQLITE_PARITY_988_VIEW_TRIGGER_GENERATED_041.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2149957 | 3527884 | <span style="color:#dc2626">-64.09%</span> |
| 567 | [00506 DML_WHERE_ORDER_LIMIT_119](crates/bench/sqlite_parity/cases/SQLITE_PARITY_506_DML_WHERE_ORDER_LIMIT_119.rs) | P1 | memory | GEN_SQL_DML | 2066860 | 3387809 | <span style="color:#dc2626">-63.91%</span> |
| 568 | [00082 CORE_STRING_FUNCTIONS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_082_CORE_STRING_FUNCTIONS.rs) | P0 | memory | SQL_FUNCTIONS | 1759229 | 2882654 | <span style="color:#dc2626">-63.86%</span> |
| 569 | [00709 CTE_RECURSIVE_MATRIX_002](crates/bench/sqlite_parity/cases/SQLITE_PARITY_709_CTE_RECURSIVE_MATRIX_002.rs) | P1 | memory | GEN_SQL_CTE | 2501833 | 4098794 | <span style="color:#dc2626">-63.83%</span> |
| 570 | [00295 SCALAR_NULL_COALESCE_017](crates/bench/sqlite_parity/cases/SQLITE_PARITY_295_SCALAR_NULL_COALESCE_017.rs) | P1 | memory | GEN_SQL_SCALAR | 1747947 | 2863687 | <span style="color:#dc2626">-63.83%</span> |
| 571 | [00276 SCALAR_ARITH_013](crates/bench/sqlite_parity/cases/SQLITE_PARITY_276_SCALAR_ARITH_013.rs) | P1 | memory | GEN_SQL_SCALAR | 1699024 | 2783185 | <span style="color:#dc2626">-63.81%</span> |
| 572 | [00866 WINDOW_PARTITION_SUM_079](crates/bench/sqlite_parity/cases/SQLITE_PARITY_866_WINDOW_PARTITION_SUM_079.rs) | P2 | memory | GEN_SQL_WINDOW | 2294781 | 3758901 | <span style="color:#dc2626">-63.80%</span> |
| 573 | [01114 INDEX_SCHEMA_PRAGMA_047](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1114_INDEX_SCHEMA_PRAGMA_047.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 2162481 | 3541931 | <span style="color:#dc2626">-63.79%</span> |
| 574 | [00017 DELETE_BASIC](crates/bench/sqlite_parity/cases/SQLITE_PARITY_017_DELETE_BASIC.rs) | P0 | memory | SQL_DELETE | 1839099 | 3011116 | <span style="color:#dc2626">-63.73%</span> |
| 575 | [00367 SCALAR_NULL_COALESCE_035](crates/bench/sqlite_parity/cases/SQLITE_PARITY_367_SCALAR_NULL_COALESCE_035.rs) | P1 | memory | GEN_SQL_SCALAR | 1770569 | 2898253 | <span style="color:#dc2626">-63.69%</span> |
| 576 | [00442 DML_WHERE_ORDER_LIMIT_055](crates/bench/sqlite_parity/cases/SQLITE_PARITY_442_DML_WHERE_ORDER_LIMIT_055.rs) | P1 | memory | GEN_SQL_DML | 2015042 | 3298380 | <span style="color:#dc2626">-63.69%</span> |
| 577 | [00020 UPSERT_DO_UPDATE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_020_UPSERT_DO_UPDATE.rs) | P0 | memory | SQL_UPSERT | 1920013 | 3142415 | <span style="color:#dc2626">-63.67%</span> |
| 578 | [00544 AGG_GROUP_HAVING_037](crates/bench/sqlite_parity/cases/SQLITE_PARITY_544_AGG_GROUP_HAVING_037.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1957103 | 3202739 | <span style="color:#dc2626">-63.65%</span> |
| 579 | [00580 AGG_GROUP_HAVING_073](crates/bench/sqlite_parity/cases/SQLITE_PARITY_580_AGG_GROUP_HAVING_073.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2021023 | 3307136 | <span style="color:#dc2626">-63.64%</span> |
| 580 | [00015 UPDATE_BASIC](crates/bench/sqlite_parity/cases/SQLITE_PARITY_015_UPDATE_BASIC.rs) | P0 | memory | SQL_UPDATE | 1855620 | 3035512 | <span style="color:#dc2626">-63.58%</span> |
| 581 | [00963 VIEW_TRIGGER_GENERATED_016](crates/bench/sqlite_parity/cases/SQLITE_PARITY_963_VIEW_TRIGGER_GENERATED_016.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2095224 | 3426623 | <span style="color:#dc2626">-63.54%</span> |
| 582 | [00004 TABLE_CONSTRAINTS_SUCCESS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_004_TABLE_CONSTRAINTS_SUCCESS.rs) | P0 | memory | SQL_CONSTRAINTS | 1882120 | 3078093 | <span style="color:#dc2626">-63.54%</span> |
| 583 | [00526 AGG_GROUP_HAVING_019](crates/bench/sqlite_parity/cases/SQLITE_PARITY_526_AGG_GROUP_HAVING_019.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2050148 | 3352843 | <span style="color:#dc2626">-63.54%</span> |
| 584 | [00384 SCALAR_ARITH_040](crates/bench/sqlite_parity/cases/SQLITE_PARITY_384_SCALAR_ARITH_040.rs) | P1 | memory | GEN_SQL_SCALAR | 1776611 | 2904625 | <span style="color:#dc2626">-63.49%</span> |
| 585 | [00978 VIEW_TRIGGER_GENERATED_031](crates/bench/sqlite_parity/cases/SQLITE_PARITY_978_VIEW_TRIGGER_GENERATED_031.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2076879 | 3395234 | <span style="color:#dc2626">-63.48%</span> |
| 586 | [00934 CONSTRAINT_FK_SAVEPOINT_067](crates/bench/sqlite_parity/cases/SQLITE_PARITY_934_CONSTRAINT_FK_SAVEPOINT_067.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1996107 | 3261610 | <span style="color:#dc2626">-63.40%</span> |
| 587 | [01022 JSON_EXTRACT_SET_015](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1022_JSON_EXTRACT_SET_015.rs) | P2 | memory | GEN_SQL_JSON | 1878144 | 3068525 | <span style="color:#dc2626">-63.38%</span> |
| 588 | [00710 CTE_RECURSIVE_MATRIX_003](crates/bench/sqlite_parity/cases/SQLITE_PARITY_710_CTE_RECURSIVE_MATRIX_003.rs) | P1 | memory | GEN_SQL_CTE | 1996367 | 3261390 | <span style="color:#dc2626">-63.37%</span> |
| 589 | [00349 SCALAR_CAST_TYPEOF_031](crates/bench/sqlite_parity/cases/SQLITE_PARITY_349_SCALAR_CAST_TYPEOF_031.rs) | P1 | memory | GEN_SQL_SCALAR | 1725354 | 2816388 | <span style="color:#dc2626">-63.24%</span> |
| 590 | [00068 CAST_AND_TYPE_AFFINITY](crates/bench/sqlite_parity/cases/SQLITE_PARITY_068_CAST_AND_TYPE_AFFINITY.rs) | P0 | memory | SQL_EXPRESSIONS | 1870288 | 3052585 | <span style="color:#dc2626">-63.21%</span> |
| 591 | [00848 WINDOW_PARTITION_SUM_061](crates/bench/sqlite_parity/cases/SQLITE_PARITY_848_WINDOW_PARTITION_SUM_061.rs) | P2 | memory | GEN_SQL_WINDOW | 2305753 | 3761937 | <span style="color:#dc2626">-63.15%</span> |
| 592 | [00915 CONSTRAINT_FK_SAVEPOINT_048](crates/bench/sqlite_parity/cases/SQLITE_PARITY_915_CONSTRAINT_FK_SAVEPOINT_048.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2025040 | 3303690 | <span style="color:#dc2626">-63.14%</span> |
| 593 | [00903 CONSTRAINT_FK_SAVEPOINT_036](crates/bench/sqlite_parity/cases/SQLITE_PARITY_903_CONSTRAINT_FK_SAVEPOINT_036.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2047122 | 3339167 | <span style="color:#dc2626">-63.12%</span> |
| 594 | [01018 JSON_EXTRACT_SET_011](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1018_JSON_EXTRACT_SET_011.rs) | P2 | memory | GEN_SQL_JSON | 1977431 | 3223719 | <span style="color:#dc2626">-63.03%</span> |
| 595 | [00886 CONSTRAINT_FK_SAVEPOINT_019](crates/bench/sqlite_parity/cases/SQLITE_PARITY_886_CONSTRAINT_FK_SAVEPOINT_019.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2045049 | 3332534 | <span style="color:#dc2626">-62.96%</span> |
| 596 | [00230 SCALAR_STRING_001](crates/bench/sqlite_parity/cases/SQLITE_PARITY_230_SCALAR_STRING_001.rs) | P1 | memory | GEN_SQL_SCALAR | 1720104 | 2802001 | <span style="color:#dc2626">-62.90%</span> |
| 597 | [00838 WINDOW_PARTITION_SUM_051](crates/bench/sqlite_parity/cases/SQLITE_PARITY_838_WINDOW_PARTITION_SUM_051.rs) | P2 | memory | GEN_SQL_WINDOW | 2045760 | 3331974 | <span style="color:#dc2626">-62.87%</span> |
| 598 | [00901 CONSTRAINT_FK_SAVEPOINT_034](crates/bench/sqlite_parity/cases/SQLITE_PARITY_901_CONSTRAINT_FK_SAVEPOINT_034.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2013168 | 3277871 | <span style="color:#dc2626">-62.82%</span> |
| 599 | [00342 SCALAR_STRING_029](crates/bench/sqlite_parity/cases/SQLITE_PARITY_342_SCALAR_STRING_029.rs) | P1 | memory | GEN_SQL_SCALAR | 1751204 | 2850833 | <span style="color:#dc2626">-62.79%</span> |
| 600 | [00262 SCALAR_STRING_009](crates/bench/sqlite_parity/cases/SQLITE_PARITY_262_SCALAR_STRING_009.rs) | P1 | memory | GEN_SQL_SCALAR | 1738760 | 2830435 | <span style="color:#dc2626">-62.78%</span> |
| 601 | [00929 CONSTRAINT_FK_SAVEPOINT_062](crates/bench/sqlite_parity/cases/SQLITE_PARITY_929_CONSTRAINT_FK_SAVEPOINT_062.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2280505 | 3711211 | <span style="color:#dc2626">-62.74%</span> |
| 602 | [00464 DML_WHERE_ORDER_LIMIT_077](crates/bench/sqlite_parity/cases/SQLITE_PARITY_464_DML_WHERE_ORDER_LIMIT_077.rs) | P1 | memory | GEN_SQL_DML | 2327163 | 3786293 | <span style="color:#dc2626">-62.70%</span> |
| 603 | [00285 SCALAR_CAST_TYPEOF_015](crates/bench/sqlite_parity/cases/SQLITE_PARITY_285_SCALAR_CAST_TYPEOF_015.rs) | P1 | memory | GEN_SQL_SCALAR | 1762525 | 2867524 | <span style="color:#dc2626">-62.69%</span> |
| 604 | [00375 SCALAR_NULL_COALESCE_037](crates/bench/sqlite_parity/cases/SQLITE_PARITY_375_SCALAR_NULL_COALESCE_037.rs) | P1 | memory | GEN_SQL_SCALAR | 1779436 | 2894776 | <span style="color:#dc2626">-62.68%</span> |
| 605 | [00476 DML_WHERE_ORDER_LIMIT_089](crates/bench/sqlite_parity/cases/SQLITE_PARITY_476_DML_WHERE_ORDER_LIMIT_089.rs) | P1 | memory | GEN_SQL_DML | 2999735 | 4879551 | <span style="color:#dc2626">-62.67%</span> |
| 606 | [00104 SELECT_DISTINCT](crates/bench/sqlite_parity/cases/SQLITE_PARITY_104_SELECT_DISTINCT.rs) | P0 | memory | SQL_SELECT | 1738078 | 2826557 | <span style="color:#dc2626">-62.63%</span> |
| 607 | [00294 SCALAR_STRING_017](crates/bench/sqlite_parity/cases/SQLITE_PARITY_294_SCALAR_STRING_017.rs) | P1 | memory | GEN_SQL_SCALAR | 1788413 | 2907651 | <span style="color:#dc2626">-62.58%</span> |
| 608 | [00408 DML_WHERE_ORDER_LIMIT_021](crates/bench/sqlite_parity/cases/SQLITE_PARITY_408_DML_WHERE_ORDER_LIMIT_021.rs) | P1 | memory | GEN_SQL_DML | 2079343 | 3380426 | <span style="color:#dc2626">-62.57%</span> |
| 609 | [00921 CONSTRAINT_FK_SAVEPOINT_054](crates/bench/sqlite_parity/cases/SQLITE_PARITY_921_CONSTRAINT_FK_SAVEPOINT_054.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2190915 | 3561258 | <span style="color:#dc2626">-62.55%</span> |
| 610 | [00822 WINDOW_PARTITION_SUM_035](crates/bench/sqlite_parity/cases/SQLITE_PARITY_822_WINDOW_PARTITION_SUM_035.rs) | P2 | memory | GEN_SQL_WINDOW | 2003039 | 3254858 | <span style="color:#dc2626">-62.50%</span> |
| 611 | [00877 CONSTRAINT_FK_SAVEPOINT_010](crates/bench/sqlite_parity/cases/SQLITE_PARITY_877_CONSTRAINT_FK_SAVEPOINT_010.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2038266 | 3310283 | <span style="color:#dc2626">-62.41%</span> |
| 612 | [00272 SCALAR_ARITH_012](crates/bench/sqlite_parity/cases/SQLITE_PARITY_272_SCALAR_ARITH_012.rs) | P1 | memory | GEN_SQL_SCALAR | 1681641 | 2730205 | <span style="color:#dc2626">-62.35%</span> |
| 613 | [00551 AGG_GROUP_HAVING_044](crates/bench/sqlite_parity/cases/SQLITE_PARITY_551_AGG_GROUP_HAVING_044.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2164465 | 3514058 | <span style="color:#dc2626">-62.35%</span> |
| 614 | [00010 GENERATED_COLUMNS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_010_GENERATED_COLUMNS.rs) | P0 | memory | SQL_DDL | 1877612 | 3045372 | <span style="color:#dc2626">-62.19%</span> |
| 615 | [00566 AGG_GROUP_HAVING_059](crates/bench/sqlite_parity/cases/SQLITE_PARITY_566_AGG_GROUP_HAVING_059.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2006566 | 3253915 | <span style="color:#dc2626">-62.16%</span> |
| 616 | [00246 SCALAR_STRING_005](crates/bench/sqlite_parity/cases/SQLITE_PARITY_246_SCALAR_STRING_005.rs) | P1 | memory | GEN_SQL_SCALAR | 1791900 | 2905156 | <span style="color:#dc2626">-62.13%</span> |
| 617 | [00605 AGG_GROUP_HAVING_098](crates/bench/sqlite_parity/cases/SQLITE_PARITY_605_AGG_GROUP_HAVING_098.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2133496 | 3458974 | <span style="color:#dc2626">-62.13%</span> |
| 618 | [00302 SCALAR_STRING_019](crates/bench/sqlite_parity/cases/SQLITE_PARITY_302_SCALAR_STRING_019.rs) | P1 | memory | GEN_SQL_SCALAR | 1747186 | 2832539 | <span style="color:#dc2626">-62.12%</span> |
| 619 | [00509 AGG_GROUP_HAVING_002](crates/bench/sqlite_parity/cases/SQLITE_PARITY_509_AGG_GROUP_HAVING_002.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2060839 | 3340510 | <span style="color:#dc2626">-62.09%</span> |
| 620 | [00232 SCALAR_ARITH_002](crates/bench/sqlite_parity/cases/SQLITE_PARITY_232_SCALAR_ARITH_002.rs) | P1 | memory | GEN_SQL_SCALAR | 1714553 | 2778907 | <span style="color:#dc2626">-62.08%</span> |
| 621 | [00584 AGG_GROUP_HAVING_077](crates/bench/sqlite_parity/cases/SQLITE_PARITY_584_AGG_GROUP_HAVING_077.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2054356 | 3327345 | <span style="color:#dc2626">-61.97%</span> |
| 622 | [01079 INDEX_SCHEMA_PRAGMA_012](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1079_INDEX_SCHEMA_PRAGMA_012.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 2382608 | 3858940 | <span style="color:#dc2626">-61.96%</span> |
| 623 | [00845 WINDOW_PARTITION_SUM_058](crates/bench/sqlite_parity/cases/SQLITE_PARITY_845_WINDOW_PARTITION_SUM_058.rs) | P2 | memory | GEN_SQL_WINDOW | 1940231 | 3142235 | <span style="color:#dc2626">-61.95%</span> |
| 624 | [00315 SCALAR_NULL_COALESCE_022](crates/bench/sqlite_parity/cases/SQLITE_PARITY_315_SCALAR_NULL_COALESCE_022.rs) | P1 | memory | GEN_SQL_SCALAR | 1743388 | 2823131 | <span style="color:#dc2626">-61.93%</span> |
| 625 | [00896 CONSTRAINT_FK_SAVEPOINT_029](crates/bench/sqlite_parity/cases/SQLITE_PARITY_896_CONSTRAINT_FK_SAVEPOINT_029.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2005494 | 3246822 | <span style="color:#dc2626">-61.90%</span> |
| 626 | [00573 AGG_GROUP_HAVING_066](crates/bench/sqlite_parity/cases/SQLITE_PARITY_573_AGG_GROUP_HAVING_066.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2044528 | 3309891 | <span style="color:#dc2626">-61.89%</span> |
| 627 | [00293 SCALAR_CAST_TYPEOF_017](crates/bench/sqlite_parity/cases/SQLITE_PARITY_293_SCALAR_CAST_TYPEOF_017.rs) | P1 | memory | GEN_SQL_SCALAR | 1751403 | 2835334 | <span style="color:#dc2626">-61.89%</span> |
| 628 | [00078 ON_CONFLICT_ALGORITHMS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_078_ON_CONFLICT_ALGORITHMS.rs) | P0 | memory | SQL_CONFLICT | 1929501 | 3122107 | <span style="color:#dc2626">-61.81%</span> |
| 629 | [00011 CREATE_TABLE_AS_SELECT](crates/bench/sqlite_parity/cases/SQLITE_PARITY_011_CREATE_TABLE_AS_SELECT.rs) | P0 | memory | SQL_DDL | 1891047 | 3057544 | <span style="color:#dc2626">-61.69%</span> |
| 630 | [00510 AGG_GROUP_HAVING_003](crates/bench/sqlite_parity/cases/SQLITE_PARITY_510_AGG_GROUP_HAVING_003.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2043095 | 3302959 | <span style="color:#dc2626">-61.66%</span> |
| 631 | [00572 AGG_GROUP_HAVING_065](crates/bench/sqlite_parity/cases/SQLITE_PARITY_572_AGG_GROUP_HAVING_065.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2046572 | 3308398 | <span style="color:#dc2626">-61.66%</span> |
| 632 | [00534 AGG_GROUP_HAVING_027](crates/bench/sqlite_parity/cases/SQLITE_PARITY_534_AGG_GROUP_HAVING_027.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1959317 | 3164887 | <span style="color:#dc2626">-61.53%</span> |
| 633 | [00722 CTE_RECURSIVE_MATRIX_015](crates/bench/sqlite_parity/cases/SQLITE_PARITY_722_CTE_RECURSIVE_MATRIX_015.rs) | P1 | memory | GEN_SQL_CTE | 2011285 | 3247323 | <span style="color:#dc2626">-61.46%</span> |
| 634 | [00041 DROP_TRIGGER](crates/bench/sqlite_parity/cases/SQLITE_PARITY_041_DROP_TRIGGER.rs) | P0 | memory | SQL_DROP | 1984434 | 3200735 | <span style="color:#dc2626">-61.29%</span> |
| 635 | [00346 SCALAR_STRING_030](crates/bench/sqlite_parity/cases/SQLITE_PARITY_346_SCALAR_STRING_030.rs) | P1 | memory | GEN_SQL_SCALAR | 1758787 | 2836646 | <span style="color:#dc2626">-61.28%</span> |
| 636 | [00454 DML_WHERE_ORDER_LIMIT_067](crates/bench/sqlite_parity/cases/SQLITE_PARITY_454_DML_WHERE_ORDER_LIMIT_067.rs) | P1 | memory | GEN_SQL_DML | 2085606 | 3363574 | <span style="color:#dc2626">-61.28%</span> |
| 637 | [00327 SCALAR_NULL_COALESCE_025](crates/bench/sqlite_parity/cases/SQLITE_PARITY_327_SCALAR_NULL_COALESCE_025.rs) | P1 | memory | GEN_SQL_SCALAR | 1786239 | 2879087 | <span style="color:#dc2626">-61.18%</span> |
| 638 | [00999 VIEW_TRIGGER_GENERATED_052](crates/bench/sqlite_parity/cases/SQLITE_PARITY_999_VIEW_TRIGGER_GENERATED_052.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2053214 | 3309110 | <span style="color:#dc2626">-61.17%</span> |
| 639 | [00993 VIEW_TRIGGER_GENERATED_046](crates/bench/sqlite_parity/cases/SQLITE_PARITY_993_VIEW_TRIGGER_GENERATED_046.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2066409 | 3329900 | <span style="color:#dc2626">-61.14%</span> |
| 640 | [00520 AGG_GROUP_HAVING_013](crates/bench/sqlite_parity/cases/SQLITE_PARITY_520_AGG_GROUP_HAVING_013.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2098871 | 3381397 | <span style="color:#dc2626">-61.11%</span> |
| 641 | [00007 WITHOUT_ROWID_TABLE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_007_WITHOUT_ROWID_TABLE.rs) | P0 | memory | SQL_ROWID | 2342231 | 3773369 | <span style="color:#dc2626">-61.10%</span> |
| 642 | [00800 WINDOW_PARTITION_SUM_013](crates/bench/sqlite_parity/cases/SQLITE_PARITY_800_WINDOW_PARTITION_SUM_013.rs) | P2 | memory | GEN_SQL_WINDOW | 1938497 | 3122848 | <span style="color:#dc2626">-61.10%</span> |
| 643 | [00541 AGG_GROUP_HAVING_034](crates/bench/sqlite_parity/cases/SQLITE_PARITY_541_AGG_GROUP_HAVING_034.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2043096 | 3291277 | <span style="color:#dc2626">-61.09%</span> |
| 644 | [00944 CONSTRAINT_FK_SAVEPOINT_077](crates/bench/sqlite_parity/cases/SQLITE_PARITY_944_CONSTRAINT_FK_SAVEPOINT_077.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2023708 | 3259446 | <span style="color:#dc2626">-61.06%</span> |
| 645 | [00066 VALUES_STATEMENT](crates/bench/sqlite_parity/cases/SQLITE_PARITY_066_VALUES_STATEMENT.rs) | P0 | memory | SQL_VALUES | 1695318 | 2730145 | <span style="color:#dc2626">-61.04%</span> |
| 646 | [00291 SCALAR_NULL_COALESCE_016](crates/bench/sqlite_parity/cases/SQLITE_PARITY_291_SCALAR_NULL_COALESCE_016.rs) | P1 | memory | GEN_SQL_SCALAR | 1758396 | 2830975 | <span style="color:#dc2626">-61.00%</span> |
| 647 | [00325 SCALAR_CAST_TYPEOF_025](crates/bench/sqlite_parity/cases/SQLITE_PARITY_325_SCALAR_CAST_TYPEOF_025.rs) | P1 | memory | GEN_SQL_SCALAR | 1786199 | 2875390 | <span style="color:#dc2626">-60.98%</span> |
| 648 | [00837 WINDOW_PARTITION_SUM_050](crates/bench/sqlite_parity/cases/SQLITE_PARITY_837_WINDOW_PARTITION_SUM_050.rs) | P2 | memory | GEN_SQL_WINDOW | 1961370 | 3156492 | <span style="color:#dc2626">-60.93%</span> |
| 649 | [00536 AGG_GROUP_HAVING_029](crates/bench/sqlite_parity/cases/SQLITE_PARITY_536_AGG_GROUP_HAVING_029.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1999663 | 3217507 | <span style="color:#dc2626">-60.90%</span> |
| 650 | [00290 SCALAR_STRING_016](crates/bench/sqlite_parity/cases/SQLITE_PARITY_290_SCALAR_STRING_016.rs) | P1 | memory | GEN_SQL_SCALAR | 1716757 | 2761905 | <span style="color:#dc2626">-60.88%</span> |
| 651 | [00261 SCALAR_CAST_TYPEOF_009](crates/bench/sqlite_parity/cases/SQLITE_PARITY_261_SCALAR_CAST_TYPEOF_009.rs) | P1 | memory | GEN_SQL_SCALAR | 1729332 | 2781973 | <span style="color:#dc2626">-60.87%</span> |
| 652 | [00909 CONSTRAINT_FK_SAVEPOINT_042](crates/bench/sqlite_parity/cases/SQLITE_PARITY_909_CONSTRAINT_FK_SAVEPOINT_042.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2051532 | 3299492 | <span style="color:#dc2626">-60.83%</span> |
| 653 | [00328 SCALAR_ARITH_026](crates/bench/sqlite_parity/cases/SQLITE_PARITY_328_SCALAR_ARITH_026.rs) | P1 | memory | GEN_SQL_SCALAR | 1737918 | 2794536 | <span style="color:#dc2626">-60.80%</span> |
| 654 | [00833 WINDOW_PARTITION_SUM_046](crates/bench/sqlite_parity/cases/SQLITE_PARITY_833_WINDOW_PARTITION_SUM_046.rs) | P2 | memory | GEN_SQL_WINDOW | 1965058 | 3159477 | <span style="color:#dc2626">-60.78%</span> |
| 655 | [00480 DML_WHERE_ORDER_LIMIT_093](crates/bench/sqlite_parity/cases/SQLITE_PARITY_480_DML_WHERE_ORDER_LIMIT_093.rs) | P1 | memory | GEN_SQL_DML | 2097749 | 3372390 | <span style="color:#dc2626">-60.76%</span> |
| 656 | [00026 FOREIGN_KEYS_CASCADE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_026_FOREIGN_KEYS_CASCADE.rs) | P0 | memory | SQL_FOREIGN_KEYS | 1932145 | 3104884 | <span style="color:#dc2626">-60.70%</span> |
| 657 | [00858 WINDOW_PARTITION_SUM_071](crates/bench/sqlite_parity/cases/SQLITE_PARITY_858_WINDOW_PARTITION_SUM_071.rs) | P2 | memory | GEN_SQL_WINDOW | 1960950 | 3149849 | <span style="color:#dc2626">-60.63%</span> |
| 658 | [00329 SCALAR_CAST_TYPEOF_026](crates/bench/sqlite_parity/cases/SQLITE_PARITY_329_SCALAR_CAST_TYPEOF_026.rs) | P1 | memory | GEN_SQL_SCALAR | 1814092 | 2913582 | <span style="color:#dc2626">-60.61%</span> |
| 659 | [00735 CTE_RECURSIVE_MATRIX_028](crates/bench/sqlite_parity/cases/SQLITE_PARITY_735_CTE_RECURSIVE_MATRIX_028.rs) | P1 | memory | GEN_SQL_CTE | 1882220 | 3022589 | <span style="color:#dc2626">-60.59%</span> |
| 660 | [00859 WINDOW_PARTITION_SUM_072](crates/bench/sqlite_parity/cases/SQLITE_PARITY_859_WINDOW_PARTITION_SUM_072.rs) | P2 | memory | GEN_SQL_WINDOW | 2074765 | 3330361 | <span style="color:#dc2626">-60.52%</span> |
| 661 | [00986 VIEW_TRIGGER_GENERATED_039](crates/bench/sqlite_parity/cases/SQLITE_PARITY_986_VIEW_TRIGGER_GENERATED_039.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2180134 | 3498348 | <span style="color:#dc2626">-60.46%</span> |
| 662 | [00083 CORE_NUMERIC_FUNCTIONS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_083_CORE_NUMERIC_FUNCTIONS.rs) | P0 | memory | SQL_FUNCTIONS | 1770439 | 2839572 | <span style="color:#dc2626">-60.39%</span> |
| 663 | [00343 SCALAR_NULL_COALESCE_029](crates/bench/sqlite_parity/cases/SQLITE_PARITY_343_SCALAR_NULL_COALESCE_029.rs) | P1 | memory | GEN_SQL_SCALAR | 1771942 | 2841866 | <span style="color:#dc2626">-60.38%</span> |
| 664 | [01112 INDEX_SCHEMA_PRAGMA_045](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1112_INDEX_SCHEMA_PRAGMA_045.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 2373139 | 3805730 | <span style="color:#dc2626">-60.37%</span> |
| 665 | [00941 CONSTRAINT_FK_SAVEPOINT_074](crates/bench/sqlite_parity/cases/SQLITE_PARITY_941_CONSTRAINT_FK_SAVEPOINT_074.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2031463 | 3257613 | <span style="color:#dc2626">-60.36%</span> |
| 666 | [00370 SCALAR_STRING_036](crates/bench/sqlite_parity/cases/SQLITE_PARITY_370_SCALAR_STRING_036.rs) | P1 | memory | GEN_SQL_SCALAR | 1822908 | 2922779 | <span style="color:#dc2626">-60.34%</span> |
| 667 | [00124 DOT_BAIL_OFF](crates/bench/sqlite_parity/cases/SQLITE_PARITY_124_DOT_BAIL_OFF.rs) | P0 | memory | CLI_DOT_COMMAND_NEGATIVE | 1768346 | 2834292 | <span style="color:#dc2626">-60.28%</span> |
| 668 | [00892 CONSTRAINT_FK_SAVEPOINT_025](crates/bench/sqlite_parity/cases/SQLITE_PARITY_892_CONSTRAINT_FK_SAVEPOINT_025.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2077089 | 3329118 | <span style="color:#dc2626">-60.28%</span> |
| 669 | [00567 AGG_GROUP_HAVING_060](crates/bench/sqlite_parity/cases/SQLITE_PARITY_567_AGG_GROUP_HAVING_060.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2034579 | 3260629 | <span style="color:#dc2626">-60.26%</span> |
| 670 | [00318 SCALAR_STRING_023](crates/bench/sqlite_parity/cases/SQLITE_PARITY_318_SCALAR_STRING_023.rs) | P1 | memory | GEN_SQL_SCALAR | 1736425 | 2782184 | <span style="color:#dc2626">-60.22%</span> |
| 671 | [00263 SCALAR_NULL_COALESCE_009](crates/bench/sqlite_parity/cases/SQLITE_PARITY_263_SCALAR_NULL_COALESCE_009.rs) | P1 | memory | GEN_SQL_SCALAR | 1762124 | 2822380 | <span style="color:#dc2626">-60.17%</span> |
| 672 | [00081 BLOBS_HEX_ZEROBLOB](crates/bench/sqlite_parity/cases/SQLITE_PARITY_081_BLOBS_HEX_ZEROBLOB.rs) | P0 | memory | SQL_FUNCTIONS | 1745332 | 2795328 | <span style="color:#dc2626">-60.16%</span> |
| 673 | [00273 SCALAR_CAST_TYPEOF_012](crates/bench/sqlite_parity/cases/SQLITE_PARITY_273_SCALAR_CAST_TYPEOF_012.rs) | P1 | memory | GEN_SQL_SCALAR | 1779085 | 2846475 | <span style="color:#dc2626">-60.00%</span> |
| 674 | [00591 AGG_GROUP_HAVING_084](crates/bench/sqlite_parity/cases/SQLITE_PARITY_591_AGG_GROUP_HAVING_084.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2256589 | 3609438 | <span style="color:#dc2626">-59.95%</span> |
| 675 | [00132 DOT_TRACE_STDOUT](crates/bench/sqlite_parity/cases/SQLITE_PARITY_132_DOT_TRACE_STDOUT.rs) | P2 | memory | CLI_DOT_COMMAND_DIAGNOSTIC | 1667555 | 2666444 | <span style="color:#dc2626">-59.90%</span> |
| 676 | [00844 WINDOW_PARTITION_SUM_057](crates/bench/sqlite_parity/cases/SQLITE_PARITY_844_WINDOW_PARTITION_SUM_057.rs) | P2 | memory | GEN_SQL_WINDOW | 1950510 | 3117809 | <span style="color:#dc2626">-59.85%</span> |
| 677 | [00570 AGG_GROUP_HAVING_063](crates/bench/sqlite_parity/cases/SQLITE_PARITY_570_AGG_GROUP_HAVING_063.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2166980 | 3463442 | <span style="color:#dc2626">-59.83%</span> |
| 678 | [00299 SCALAR_NULL_COALESCE_018](crates/bench/sqlite_parity/cases/SQLITE_PARITY_299_SCALAR_NULL_COALESCE_018.rs) | P1 | memory | GEN_SQL_SCALAR | 1796058 | 2870390 | <span style="color:#dc2626">-59.82%</span> |
| 679 | [00847 WINDOW_PARTITION_SUM_060](crates/bench/sqlite_parity/cases/SQLITE_PARITY_847_WINDOW_PARTITION_SUM_060.rs) | P2 | memory | GEN_SQL_WINDOW | 2037244 | 3254547 | <span style="color:#dc2626">-59.75%</span> |
| 680 | [00881 CONSTRAINT_FK_SAVEPOINT_014](crates/bench/sqlite_parity/cases/SQLITE_PARITY_881_CONSTRAINT_FK_SAVEPOINT_014.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2134548 | 3403468 | <span style="color:#dc2626">-59.45%</span> |
| 681 | [00931 CONSTRAINT_FK_SAVEPOINT_064](crates/bench/sqlite_parity/cases/SQLITE_PARITY_931_CONSTRAINT_FK_SAVEPOINT_064.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2043846 | 3258144 | <span style="color:#dc2626">-59.41%</span> |
| 682 | [00069 COLLATE_NOCASE_RTRIM_BINARY](crates/bench/sqlite_parity/cases/SQLITE_PARITY_069_COLLATE_NOCASE_RTRIM_BINARY.rs) | P0 | memory | SQL_COLLATION | 1797230 | 2864740 | <span style="color:#dc2626">-59.40%</span> |
| 683 | [00038 CREATE_TRIGGER_AFTER](crates/bench/sqlite_parity/cases/SQLITE_PARITY_038_CREATE_TRIGGER_AFTER.rs) | P0 | memory | SQL_TRIGGER | 1944699 | 3098232 | <span style="color:#dc2626">-59.32%</span> |
| 684 | [00871 CONSTRAINT_FK_SAVEPOINT_004](crates/bench/sqlite_parity/cases/SQLITE_PARITY_871_CONSTRAINT_FK_SAVEPOINT_004.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2096226 | 3339007 | <span style="color:#dc2626">-59.29%</span> |
| 685 | [00242 SCALAR_STRING_004](crates/bench/sqlite_parity/cases/SQLITE_PARITY_242_SCALAR_STRING_004.rs) | P1 | memory | GEN_SQL_SCALAR | 1738208 | 2768317 | <span style="color:#dc2626">-59.26%</span> |
| 686 | [00297 SCALAR_CAST_TYPEOF_018](crates/bench/sqlite_parity/cases/SQLITE_PARITY_297_SCALAR_CAST_TYPEOF_018.rs) | P1 | memory | GEN_SQL_SCALAR | 1781670 | 2836296 | <span style="color:#dc2626">-59.19%</span> |
| 687 | [00353 SCALAR_CAST_TYPEOF_032](crates/bench/sqlite_parity/cases/SQLITE_PARITY_353_SCALAR_CAST_TYPEOF_032.rs) | P1 | memory | GEN_SQL_SCALAR | 1781761 | 2834051 | <span style="color:#dc2626">-59.06%</span> |
| 688 | [00819 WINDOW_PARTITION_SUM_032](crates/bench/sqlite_parity/cases/SQLITE_PARITY_819_WINDOW_PARTITION_SUM_032.rs) | P2 | memory | GEN_SQL_WINDOW | 2014892 | 3203320 | <span style="color:#dc2626">-58.98%</span> |
| 689 | [00804 WINDOW_PARTITION_SUM_017](crates/bench/sqlite_parity/cases/SQLITE_PARITY_804_WINDOW_PARTITION_SUM_017.rs) | P2 | memory | GEN_SQL_WINDOW | 2301774 | 3658622 | <span style="color:#dc2626">-58.95%</span> |
| 690 | [00248 SCALAR_ARITH_006](crates/bench/sqlite_parity/cases/SQLITE_PARITY_248_SCALAR_ARITH_006.rs) | P1 | memory | GEN_SQL_SCALAR | 1745762 | 2774428 | <span style="color:#dc2626">-58.92%</span> |
| 691 | [00856 WINDOW_PARTITION_SUM_069](crates/bench/sqlite_parity/cases/SQLITE_PARITY_856_WINDOW_PARTITION_SUM_069.rs) | P2 | memory | GEN_SQL_WINDOW | 2011405 | 3195455 | <span style="color:#dc2626">-58.87%</span> |
| 692 | [00695 JOIN_SUBQUERY_EXISTS_088](crates/bench/sqlite_parity/cases/SQLITE_PARITY_695_JOIN_SUBQUERY_EXISTS_088.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2783015 | 4418068 | <span style="color:#dc2626">-58.75%</span> |
| 693 | [00152 DOT_CLONE_TEMPFILE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_152_DOT_CLONE_TEMPFILE.rs) | P2 | tempfile | CLI_TEMPFILE | 2522533 | 4003474 | <span style="color:#dc2626">-58.71%</span> |
| 694 | [00543 AGG_GROUP_HAVING_036](crates/bench/sqlite_parity/cases/SQLITE_PARITY_543_AGG_GROUP_HAVING_036.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2076879 | 3295775 | <span style="color:#dc2626">-58.69%</span> |
| 695 | [00906 CONSTRAINT_FK_SAVEPOINT_039](crates/bench/sqlite_parity/cases/SQLITE_PARITY_906_CONSTRAINT_FK_SAVEPOINT_039.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2084563 | 3307958 | <span style="color:#dc2626">-58.69%</span> |
| 696 | [00252 SCALAR_ARITH_007](crates/bench/sqlite_parity/cases/SQLITE_PARITY_252_SCALAR_ARITH_007.rs) | P1 | memory | GEN_SQL_SCALAR | 1788113 | 2834212 | <span style="color:#dc2626">-58.50%</span> |
| 697 | [00352 SCALAR_ARITH_032](crates/bench/sqlite_parity/cases/SQLITE_PARITY_352_SCALAR_ARITH_032.rs) | P1 | memory | GEN_SQL_SCALAR | 1728601 | 2739864 | <span style="color:#dc2626">-58.50%</span> |
| 698 | [00323 SCALAR_NULL_COALESCE_024](crates/bench/sqlite_parity/cases/SQLITE_PARITY_323_SCALAR_NULL_COALESCE_024.rs) | P1 | memory | GEN_SQL_SCALAR | 1771551 | 2807772 | <span style="color:#dc2626">-58.49%</span> |
| 699 | [00251 SCALAR_NULL_COALESCE_006](crates/bench/sqlite_parity/cases/SQLITE_PARITY_251_SCALAR_NULL_COALESCE_006.rs) | P1 | memory | GEN_SQL_SCALAR | 1828640 | 2896690 | <span style="color:#dc2626">-58.41%</span> |
| 700 | [00174 OPT_CSV_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_174_OPT_CSV_MODE.rs) | P1 | memory | CLI_OPTION | 1721868 | 2727189 | <span style="color:#dc2626">-58.39%</span> |
| 701 | [00426 DML_WHERE_ORDER_LIMIT_039](crates/bench/sqlite_parity/cases/SQLITE_PARITY_426_DML_WHERE_ORDER_LIMIT_039.rs) | P1 | memory | GEN_SQL_DML | 2141381 | 3391596 | <span style="color:#dc2626">-58.38%</span> |
| 702 | [01121 INDEX_SCHEMA_PRAGMA_054](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1121_INDEX_SCHEMA_PRAGMA_054.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 2255847 | 3572769 | <span style="color:#dc2626">-58.38%</span> |
| 703 | [00274 SCALAR_STRING_012](crates/bench/sqlite_parity/cases/SQLITE_PARITY_274_SCALAR_STRING_012.rs) | P1 | memory | GEN_SQL_SCALAR | 1784136 | 2825224 | <span style="color:#dc2626">-58.35%</span> |
| 704 | [00175 OPT_QUOTE_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_175_OPT_QUOTE_MODE.rs) | P1 | memory | CLI_OPTION | 1672093 | 2647599 | <span style="color:#dc2626">-58.34%</span> |
| 705 | [00671 JOIN_SUBQUERY_EXISTS_064](crates/bench/sqlite_parity/cases/SQLITE_PARITY_671_JOIN_SUBQUERY_EXISTS_064.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2751476 | 4355690 | <span style="color:#dc2626">-58.30%</span> |
| 706 | [00950 VIEW_TRIGGER_GENERATED_003](crates/bench/sqlite_parity/cases/SQLITE_PARITY_950_VIEW_TRIGGER_GENERATED_003.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1987620 | 3144299 | <span style="color:#dc2626">-58.19%</span> |
| 707 | [00998 VIEW_TRIGGER_GENERATED_051](crates/bench/sqlite_parity/cases/SQLITE_PARITY_998_VIEW_TRIGGER_GENERATED_051.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2062341 | 3261981 | <span style="color:#dc2626">-58.17%</span> |
| 708 | [00882 CONSTRAINT_FK_SAVEPOINT_015](crates/bench/sqlite_parity/cases/SQLITE_PARITY_882_CONSTRAINT_FK_SAVEPOINT_015.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2036973 | 3220983 | <span style="color:#dc2626">-58.13%</span> |
| 709 | [00660 JOIN_SUBQUERY_EXISTS_053](crates/bench/sqlite_parity/cases/SQLITE_PARITY_660_JOIN_SUBQUERY_EXISTS_053.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2156239 | 3409420 | <span style="color:#dc2626">-58.12%</span> |
| 710 | [00583 AGG_GROUP_HAVING_076](crates/bench/sqlite_parity/cases/SQLITE_PARITY_583_AGG_GROUP_HAVING_076.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2076890 | 3283242 | <span style="color:#dc2626">-58.08%</span> |
| 711 | [00899 CONSTRAINT_FK_SAVEPOINT_032](crates/bench/sqlite_parity/cases/SQLITE_PARITY_899_CONSTRAINT_FK_SAVEPOINT_032.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2087730 | 3300254 | <span style="color:#dc2626">-58.08%</span> |
| 712 | [00589 AGG_GROUP_HAVING_082](crates/bench/sqlite_parity/cases/SQLITE_PARITY_589_AGG_GROUP_HAVING_082.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2085415 | 3294272 | <span style="color:#dc2626">-57.97%</span> |
| 713 | [00324 SCALAR_ARITH_025](crates/bench/sqlite_parity/cases/SQLITE_PARITY_324_SCALAR_ARITH_025.rs) | P1 | memory | GEN_SQL_SCALAR | 1787652 | 2823101 | <span style="color:#dc2626">-57.92%</span> |
| 714 | [00100 SCHEMA_SQLITE_SCHEMA](crates/bench/sqlite_parity/cases/SQLITE_PARITY_100_SCHEMA_SQLITE_SCHEMA.rs) | P0 | memory | SQL_SCHEMA | 1832096 | 2893053 | <span style="color:#dc2626">-57.91%</span> |
| 715 | [00195 OPT_SAFE_MODE_BLOCKS_SHELL](crates/bench/sqlite_parity/cases/SQLITE_PARITY_195_OPT_SAFE_MODE_BLOCKS_SHELL.rs) | P2 | memory | CLI_OPTION_NEGATIVE | 1761973 | 2781401 | <span style="color:#dc2626">-57.86%</span> |
| 716 | [00337 SCALAR_CAST_TYPEOF_028](crates/bench/sqlite_parity/cases/SQLITE_PARITY_337_SCALAR_CAST_TYPEOF_028.rs) | P1 | memory | GEN_SQL_SCALAR | 1771210 | 2795719 | <span style="color:#dc2626">-57.84%</span> |
| 717 | [00604 AGG_GROUP_HAVING_097](crates/bench/sqlite_parity/cases/SQLITE_PARITY_604_AGG_GROUP_HAVING_097.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2161249 | 3409701 | <span style="color:#dc2626">-57.77%</span> |
| 718 | [00366 SCALAR_STRING_035](crates/bench/sqlite_parity/cases/SQLITE_PARITY_366_SCALAR_STRING_035.rs) | P1 | memory | GEN_SQL_SCALAR | 1792501 | 2824323 | <span style="color:#dc2626">-57.56%</span> |
| 719 | [00289 SCALAR_CAST_TYPEOF_016](crates/bench/sqlite_parity/cases/SQLITE_PARITY_289_SCALAR_CAST_TYPEOF_016.rs) | P1 | memory | GEN_SQL_SCALAR | 1781941 | 2805718 | <span style="color:#dc2626">-57.45%</span> |
| 720 | [00239 SCALAR_NULL_COALESCE_003](crates/bench/sqlite_parity/cases/SQLITE_PARITY_239_SCALAR_NULL_COALESCE_003.rs) | P1 | memory | GEN_SQL_SCALAR | 1733960 | 2729213 | <span style="color:#dc2626">-57.40%</span> |
| 721 | [00470 DML_WHERE_ORDER_LIMIT_083](crates/bench/sqlite_parity/cases/SQLITE_PARITY_470_DML_WHERE_ORDER_LIMIT_083.rs) | P1 | memory | GEN_SQL_DML | 2404519 | 3784570 | <span style="color:#dc2626">-57.39%</span> |
| 722 | [00012 INSERT_DEFAULT_VALUES](crates/bench/sqlite_parity/cases/SQLITE_PARITY_012_INSERT_DEFAULT_VALUES.rs) | P0 | memory | SQL_INSERT | 1913670 | 3011197 | <span style="color:#dc2626">-57.35%</span> |
| 723 | [00281 SCALAR_CAST_TYPEOF_014](crates/bench/sqlite_parity/cases/SQLITE_PARITY_281_SCALAR_CAST_TYPEOF_014.rs) | P1 | memory | GEN_SQL_SCALAR | 1788173 | 2811970 | <span style="color:#dc2626">-57.25%</span> |
| 724 | [00173 OPT_LIST_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_173_OPT_LIST_MODE.rs) | P1 | memory | CLI_OPTION | 1745091 | 2743219 | <span style="color:#dc2626">-57.20%</span> |
| 725 | [00176 OPT_LINE_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_176_OPT_LINE_MODE.rs) | P2 | memory | CLI_OPTION | 1749900 | 2750603 | <span style="color:#dc2626">-57.19%</span> |
| 726 | [00418 DML_WHERE_ORDER_LIMIT_031](crates/bench/sqlite_parity/cases/SQLITE_PARITY_418_DML_WHERE_ORDER_LIMIT_031.rs) | P1 | memory | GEN_SQL_DML | 2151861 | 3381718 | <span style="color:#dc2626">-57.15%</span> |
| 727 | [00707 JOIN_SUBQUERY_EXISTS_100](crates/bench/sqlite_parity/cases/SQLITE_PARITY_707_JOIN_SUBQUERY_EXISTS_100.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2664611 | 4184386 | <span style="color:#dc2626">-57.04%</span> |
| 728 | [00642 JOIN_SUBQUERY_EXISTS_035](crates/bench/sqlite_parity/cases/SQLITE_PARITY_642_JOIN_SUBQUERY_EXISTS_035.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2571776 | 4038189 | <span style="color:#dc2626">-57.02%</span> |
| 729 | [00088 JSON_SCALAR_FUNCTIONS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_088_JSON_SCALAR_FUNCTIONS.rs) | P0 | memory | SQL_JSON | 1793974 | 2816197 | <span style="color:#dc2626">-56.98%</span> |
| 730 | [00354 SCALAR_STRING_032](crates/bench/sqlite_parity/cases/SQLITE_PARITY_354_SCALAR_STRING_032.rs) | P1 | memory | GEN_SQL_SCALAR | 1800376 | 2826197 | <span style="color:#dc2626">-56.98%</span> |
| 731 | [00850 WINDOW_PARTITION_SUM_063](crates/bench/sqlite_parity/cases/SQLITE_PARITY_850_WINDOW_PARTITION_SUM_063.rs) | P2 | memory | GEN_SQL_WINDOW | 2044718 | 3208129 | <span style="color:#dc2626">-56.90%</span> |
| 732 | [00911 CONSTRAINT_FK_SAVEPOINT_044](crates/bench/sqlite_parity/cases/SQLITE_PARITY_911_CONSTRAINT_FK_SAVEPOINT_044.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2093962 | 3283832 | <span style="color:#dc2626">-56.82%</span> |
| 733 | [00356 SCALAR_ARITH_033](crates/bench/sqlite_parity/cases/SQLITE_PARITY_356_SCALAR_ARITH_033.rs) | P1 | memory | GEN_SQL_SCALAR | 1750531 | 2744201 | <span style="color:#dc2626">-56.76%</span> |
| 734 | [00312 SCALAR_ARITH_022](crates/bench/sqlite_parity/cases/SQLITE_PARITY_312_SCALAR_ARITH_022.rs) | P1 | memory | GEN_SQL_SCALAR | 1771882 | 2777494 | <span style="color:#dc2626">-56.75%</span> |
| 735 | [00925 CONSTRAINT_FK_SAVEPOINT_058](crates/bench/sqlite_parity/cases/SQLITE_PARITY_925_CONSTRAINT_FK_SAVEPOINT_058.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1934770 | 3031374 | <span style="color:#dc2626">-56.68%</span> |
| 736 | [00249 SCALAR_CAST_TYPEOF_006](crates/bench/sqlite_parity/cases/SQLITE_PARITY_249_SCALAR_CAST_TYPEOF_006.rs) | P1 | memory | GEN_SQL_SCALAR | 1841484 | 2885038 | <span style="color:#dc2626">-56.67%</span> |
| 737 | [00287 SCALAR_NULL_COALESCE_015](crates/bench/sqlite_parity/cases/SQLITE_PARITY_287_SCALAR_NULL_COALESCE_015.rs) | P1 | memory | GEN_SQL_SCALAR | 1766241 | 2766303 | <span style="color:#dc2626">-56.62%</span> |
| 738 | [00876 CONSTRAINT_FK_SAVEPOINT_009](crates/bench/sqlite_parity/cases/SQLITE_PARITY_876_CONSTRAINT_FK_SAVEPOINT_009.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2134779 | 3343475 | <span style="color:#dc2626">-56.62%</span> |
| 739 | [01005 VIEW_TRIGGER_GENERATED_058](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1005_VIEW_TRIGGER_GENERATED_058.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2211173 | 3457762 | <span style="color:#dc2626">-56.38%</span> |
| 740 | [00282 SCALAR_STRING_014](crates/bench/sqlite_parity/cases/SQLITE_PARITY_282_SCALAR_STRING_014.rs) | P1 | memory | GEN_SQL_SCALAR | 1763346 | 2757377 | <span style="color:#dc2626">-56.37%</span> |
| 741 | [00102 WITH_MATERIALIZED_HINTS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_102_WITH_MATERIALIZED_HINTS.rs) | P0 | memory | SQL_CTE | 1885447 | 2947876 | <span style="color:#dc2626">-56.35%</span> |
| 742 | [00596 AGG_GROUP_HAVING_089](crates/bench/sqlite_parity/cases/SQLITE_PARITY_596_AGG_GROUP_HAVING_089.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1936915 | 3024612 | <span style="color:#dc2626">-56.16%</span> |
| 743 | [00241 SCALAR_CAST_TYPEOF_004](crates/bench/sqlite_parity/cases/SQLITE_PARITY_241_SCALAR_CAST_TYPEOF_004.rs) | P1 | memory | GEN_SQL_SCALAR | 1766452 | 2756976 | <span style="color:#dc2626">-56.07%</span> |
| 744 | [00178 OPT_HTML_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_178_OPT_HTML_MODE.rs) | P2 | memory | CLI_OPTION | 1699445 | 2650524 | <span style="color:#dc2626">-55.96%</span> |
| 745 | [00785 CTE_RECURSIVE_MATRIX_078](crates/bench/sqlite_parity/cases/SQLITE_PARITY_785_CTE_RECURSIVE_MATRIX_078.rs) | P1 | memory | GEN_SQL_CTE | 2090615 | 3260549 | <span style="color:#dc2626">-55.96%</span> |
| 746 | [00626 JOIN_SUBQUERY_EXISTS_019](crates/bench/sqlite_parity/cases/SQLITE_PARITY_626_JOIN_SUBQUERY_EXISTS_019.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2716409 | 4235903 | <span style="color:#dc2626">-55.94%</span> |
| 747 | [00236 SCALAR_ARITH_003](crates/bench/sqlite_parity/cases/SQLITE_PARITY_236_SCALAR_ARITH_003.rs) | P1 | memory | GEN_SQL_SCALAR | 1794906 | 2798133 | <span style="color:#dc2626">-55.89%</span> |
| 748 | [00996 VIEW_TRIGGER_GENERATED_049](crates/bench/sqlite_parity/cases/SQLITE_PARITY_996_VIEW_TRIGGER_GENERATED_049.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2583608 | 4027339 | <span style="color:#dc2626">-55.88%</span> |
| 749 | [00511 AGG_GROUP_HAVING_004](crates/bench/sqlite_parity/cases/SQLITE_PARITY_511_AGG_GROUP_HAVING_004.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2114210 | 3290014 | <span style="color:#dc2626">-55.61%</span> |
| 750 | [00118 DOT_FULLSCHEMA](crates/bench/sqlite_parity/cases/SQLITE_PARITY_118_DOT_FULLSCHEMA.rs) | P0 | memory | CLI_DOT_COMMAND | 1979064 | 3079245 | <span style="color:#dc2626">-55.59%</span> |
| 751 | [00169 DOT_NONCE_SAFE_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_169_DOT_NONCE_SAFE_MODE.rs) | P2 | memory | CLI_DOT_COMMAND | 1770650 | 2754642 | <span style="color:#dc2626">-55.57%</span> |
| 752 | [00802 WINDOW_PARTITION_SUM_015](crates/bench/sqlite_parity/cases/SQLITE_PARITY_802_WINDOW_PARTITION_SUM_015.rs) | P2 | memory | GEN_SQL_WINDOW | 1920434 | 2986059 | <span style="color:#dc2626">-55.49%</span> |
| 753 | [00902 CONSTRAINT_FK_SAVEPOINT_035](crates/bench/sqlite_parity/cases/SQLITE_PARITY_902_CONSTRAINT_FK_SAVEPOINT_035.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2038667 | 3169797 | <span style="color:#dc2626">-55.48%</span> |
| 754 | [00025 BEGIN_MODES](crates/bench/sqlite_parity/cases/SQLITE_PARITY_025_BEGIN_MODES.rs) | P0 | memory | SQL_TRANSACTION | 1688043 | 2624044 | <span style="color:#dc2626">-55.45%</span> |
| 755 | [00188 OPT_HEADER](crates/bench/sqlite_parity/cases/SQLITE_PARITY_188_OPT_HEADER.rs) | P1 | memory | CLI_OPTION | 1751723 | 2722029 | <span style="color:#dc2626">-55.39%</span> |
| 756 | [00577 AGG_GROUP_HAVING_070](crates/bench/sqlite_parity/cases/SQLITE_PARITY_577_AGG_GROUP_HAVING_070.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2146340 | 3335110 | <span style="color:#dc2626">-55.39%</span> |
| 757 | [00714 CTE_RECURSIVE_MATRIX_007](crates/bench/sqlite_parity/cases/SQLITE_PARITY_714_CTE_RECURSIVE_MATRIX_007.rs) | P1 | memory | GEN_SQL_CTE | 2004692 | 3114922 | <span style="color:#dc2626">-55.38%</span> |
| 758 | [00530 AGG_GROUP_HAVING_023](crates/bench/sqlite_parity/cases/SQLITE_PARITY_530_AGG_GROUP_HAVING_023.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2162110 | 3357181 | <span style="color:#dc2626">-55.27%</span> |
| 759 | [00980 VIEW_TRIGGER_GENERATED_033](crates/bench/sqlite_parity/cases/SQLITE_PARITY_980_VIEW_TRIGGER_GENERATED_033.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2183330 | 3390033 | <span style="color:#dc2626">-55.27%</span> |
| 760 | [00172 OPT_CMD](crates/bench/sqlite_parity/cases/SQLITE_PARITY_172_OPT_CMD.rs) | P1 | memory | CLI_OPTION | 1750682 | 2716379 | <span style="color:#dc2626">-55.16%</span> |
| 761 | [00960 VIEW_TRIGGER_GENERATED_013](crates/bench/sqlite_parity/cases/SQLITE_PARITY_960_VIEW_TRIGGER_GENERATED_013.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2310110 | 3583820 | <span style="color:#dc2626">-55.14%</span> |
| 762 | [00373 SCALAR_CAST_TYPEOF_037](crates/bench/sqlite_parity/cases/SQLITE_PARITY_373_SCALAR_CAST_TYPEOF_037.rs) | P1 | memory | GEN_SQL_SCALAR | 1853166 | 2874879 | <span style="color:#dc2626">-55.13%</span> |
| 763 | [00590 AGG_GROUP_HAVING_083](crates/bench/sqlite_parity/cases/SQLITE_PARITY_590_AGG_GROUP_HAVING_083.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2094262 | 3243326 | <span style="color:#dc2626">-54.87%</span> |
| 764 | [01111 INDEX_SCHEMA_PRAGMA_044](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1111_INDEX_SCHEMA_PRAGMA_044.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 2316162 | 3586465 | <span style="color:#dc2626">-54.85%</span> |
| 765 | [00303 SCALAR_NULL_COALESCE_019](crates/bench/sqlite_parity/cases/SQLITE_PARITY_303_SCALAR_NULL_COALESCE_019.rs) | P1 | memory | GEN_SQL_SCALAR | 1826145 | 2827058 | <span style="color:#dc2626">-54.81%</span> |
| 766 | [00539 AGG_GROUP_HAVING_032](crates/bench/sqlite_parity/cases/SQLITE_PARITY_539_AGG_GROUP_HAVING_032.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2157752 | 3339107 | <span style="color:#dc2626">-54.75%</span> |
| 767 | [00639 JOIN_SUBQUERY_EXISTS_032](crates/bench/sqlite_parity/cases/SQLITE_PARITY_639_JOIN_SUBQUERY_EXISTS_032.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2230940 | 3446651 | <span style="color:#dc2626">-54.49%</span> |
| 768 | [00631 JOIN_SUBQUERY_EXISTS_024](crates/bench/sqlite_parity/cases/SQLITE_PARITY_631_JOIN_SUBQUERY_EXISTS_024.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2332903 | 3600240 | <span style="color:#dc2626">-54.32%</span> |
| 769 | [00009 STRICT_TABLE_TYPE_FAILURE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_009_STRICT_TABLE_TYPE_FAILURE.rs) | P0 | memory | SQL_DDL_NEGATIVE | 2415189 | 3726189 | <span style="color:#dc2626">-54.28%</span> |
| 770 | [00789 WINDOW_PARTITION_SUM_002](crates/bench/sqlite_parity/cases/SQLITE_PARITY_789_WINDOW_PARTITION_SUM_002.rs) | P2 | memory | GEN_SQL_WINDOW | 2060408 | 3178202 | <span style="color:#dc2626">-54.25%</span> |
| 771 | [00885 CONSTRAINT_FK_SAVEPOINT_018](crates/bench/sqlite_parity/cases/SQLITE_PARITY_885_CONSTRAINT_FK_SAVEPOINT_018.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2495681 | 3849122 | <span style="color:#dc2626">-54.23%</span> |
| 772 | [00713 CTE_RECURSIVE_MATRIX_006](crates/bench/sqlite_parity/cases/SQLITE_PARITY_713_CTE_RECURSIVE_MATRIX_006.rs) | P1 | memory | GEN_SQL_CTE | 2069055 | 3190876 | <span style="color:#dc2626">-54.22%</span> |
| 773 | [01038 JSON_EXTRACT_SET_031](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1038_JSON_EXTRACT_SET_031.rs) | P2 | memory | GEN_SQL_JSON | 2013809 | 3105595 | <span style="color:#dc2626">-54.21%</span> |
| 774 | [00231 SCALAR_NULL_COALESCE_001](crates/bench/sqlite_parity/cases/SQLITE_PARITY_231_SCALAR_NULL_COALESCE_001.rs) | P1 | memory | GEN_SQL_SCALAR | 1793002 | 2763047 | <span style="color:#dc2626">-54.10%</span> |
| 775 | [00565 AGG_GROUP_HAVING_058](crates/bench/sqlite_parity/cases/SQLITE_PARITY_565_AGG_GROUP_HAVING_058.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2258372 | 3479323 | <span style="color:#dc2626">-54.06%</span> |
| 776 | [00179 OPT_MARKDOWN_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_179_OPT_MARKDOWN_MODE.rs) | P2 | memory | CLI_OPTION | 1723350 | 2654642 | <span style="color:#dc2626">-54.04%</span> |
| 777 | [01012 JSON_EXTRACT_SET_005](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1012_JSON_EXTRACT_SET_005.rs) | P2 | memory | GEN_SQL_JSON | 2121644 | 3268002 | <span style="color:#dc2626">-54.03%</span> |
| 778 | [00316 SCALAR_ARITH_023](crates/bench/sqlite_parity/cases/SQLITE_PARITY_316_SCALAR_ARITH_023.rs) | P1 | memory | GEN_SQL_SCALAR | 1808271 | 2784518 | <span style="color:#dc2626">-53.99%</span> |
| 779 | [00967 VIEW_TRIGGER_GENERATED_020](crates/bench/sqlite_parity/cases/SQLITE_PARITY_967_VIEW_TRIGGER_GENERATED_020.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2308457 | 3553974 | <span style="color:#dc2626">-53.95%</span> |
| 780 | [00569 AGG_GROUP_HAVING_062](crates/bench/sqlite_parity/cases/SQLITE_PARITY_569_AGG_GROUP_HAVING_062.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2287397 | 3519428 | <span style="color:#dc2626">-53.86%</span> |
| 781 | [00317 SCALAR_CAST_TYPEOF_023](crates/bench/sqlite_parity/cases/SQLITE_PARITY_317_SCALAR_CAST_TYPEOF_023.rs) | P1 | memory | GEN_SQL_SCALAR | 1976500 | 3039931 | <span style="color:#dc2626">-53.80%</span> |
| 782 | [00292 SCALAR_ARITH_017](crates/bench/sqlite_parity/cases/SQLITE_PARITY_292_SCALAR_ARITH_017.rs) | P1 | memory | GEN_SQL_SCALAR | 1771602 | 2723923 | <span style="color:#dc2626">-53.75%</span> |
| 783 | [00079 CHECK_CONSTRAINT_FAILURE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_079_CHECK_CONSTRAINT_FAILURE.rs) | P0 | memory | SQL_CONSTRAINTS_NEGATIVE | 1873905 | 2880980 | <span style="color:#dc2626">-53.74%</span> |
| 784 | [00347 SCALAR_NULL_COALESCE_030](crates/bench/sqlite_parity/cases/SQLITE_PARITY_347_SCALAR_NULL_COALESCE_030.rs) | P1 | memory | GEN_SQL_SCALAR | 1840482 | 2828952 | <span style="color:#dc2626">-53.71%</span> |
| 785 | [00177 OPT_JSON_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_177_OPT_JSON_MODE.rs) | P1 | memory | CLI_OPTION | 1787041 | 2746545 | <span style="color:#dc2626">-53.69%</span> |
| 786 | [00924 CONSTRAINT_FK_SAVEPOINT_057](crates/bench/sqlite_parity/cases/SQLITE_PARITY_924_CONSTRAINT_FK_SAVEPOINT_057.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1979926 | 3039640 | <span style="color:#dc2626">-53.52%</span> |
| 787 | [00849 WINDOW_PARTITION_SUM_062](crates/bench/sqlite_parity/cases/SQLITE_PARITY_849_WINDOW_PARTITION_SUM_062.rs) | P2 | memory | GEN_SQL_WINDOW | 1978533 | 3035933 | <span style="color:#dc2626">-53.44%</span> |
| 788 | [00923 CONSTRAINT_FK_SAVEPOINT_056](crates/bench/sqlite_parity/cases/SQLITE_PARITY_923_CONSTRAINT_FK_SAVEPOINT_056.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1993010 | 3057324 | <span style="color:#dc2626">-53.40%</span> |
| 789 | [00089 JSON_TABLE_VALUED_FUNCTIONS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_089_JSON_TABLE_VALUED_FUNCTIONS.rs) | P0 | memory | SQL_JSON | 1826365 | 2801420 | <span style="color:#dc2626">-53.39%</span> |
| 790 | [00348 SCALAR_ARITH_031](crates/bench/sqlite_parity/cases/SQLITE_PARITY_348_SCALAR_ARITH_031.rs) | P1 | memory | GEN_SQL_SCALAR | 1777893 | 2726829 | <span style="color:#dc2626">-53.37%</span> |
| 791 | [00214 DROP_TABLE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_214_DROP_TABLE.rs) | P0 | memory | SQL_DROP | 1947875 | 2984045 | <span style="color:#dc2626">-53.19%</span> |
| 792 | [00183 OPT_TABS_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_183_OPT_TABS_MODE.rs) | P2 | memory | CLI_OPTION | 1729682 | 2649012 | <span style="color:#dc2626">-53.15%</span> |
| 793 | [00836 WINDOW_PARTITION_SUM_049](crates/bench/sqlite_parity/cases/SQLITE_PARITY_836_WINDOW_PARTITION_SUM_049.rs) | P2 | memory | GEN_SQL_WINDOW | 2185154 | 3345639 | <span style="color:#dc2626">-53.11%</span> |
| 794 | [00669 JOIN_SUBQUERY_EXISTS_062](crates/bench/sqlite_parity/cases/SQLITE_PARITY_669_JOIN_SUBQUERY_EXISTS_062.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2567657 | 3929183 | <span style="color:#dc2626">-53.03%</span> |
| 795 | [00762 CTE_RECURSIVE_MATRIX_055](crates/bench/sqlite_parity/cases/SQLITE_PARITY_762_CTE_RECURSIVE_MATRIX_055.rs) | P1 | memory | GEN_SQL_CTE | 2561515 | 3917602 | <span style="color:#dc2626">-52.94%</span> |
| 796 | [00622 JOIN_SUBQUERY_EXISTS_015](crates/bench/sqlite_parity/cases/SQLITE_PARITY_622_JOIN_SUBQUERY_EXISTS_015.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2183781 | 3338375 | <span style="color:#dc2626">-52.87%</span> |
| 797 | [00926 CONSTRAINT_FK_SAVEPOINT_059](crates/bench/sqlite_parity/cases/SQLITE_PARITY_926_CONSTRAINT_FK_SAVEPOINT_059.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2002458 | 3059668 | <span style="color:#dc2626">-52.80%</span> |
| 798 | [00123 DOT_ECHO](crates/bench/sqlite_parity/cases/SQLITE_PARITY_123_DOT_ECHO.rs) | P0 | memory | CLI_DOT_COMMAND | 1771922 | 2706249 | <span style="color:#dc2626">-52.73%</span> |
| 799 | [00916 CONSTRAINT_FK_SAVEPOINT_049](crates/bench/sqlite_parity/cases/SQLITE_PARITY_916_CONSTRAINT_FK_SAVEPOINT_049.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2278931 | 3477579 | <span style="color:#dc2626">-52.60%</span> |
| 800 | [00983 VIEW_TRIGGER_GENERATED_036](crates/bench/sqlite_parity/cases/SQLITE_PARITY_983_VIEW_TRIGGER_GENERATED_036.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2203849 | 3362301 | <span style="color:#dc2626">-52.56%</span> |
| 801 | [00181 OPT_TABLE_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_181_OPT_TABLE_MODE.rs) | P2 | memory | CLI_OPTION | 1730483 | 2639503 | <span style="color:#dc2626">-52.53%</span> |
| 802 | [00448 DML_WHERE_ORDER_LIMIT_061](crates/bench/sqlite_parity/cases/SQLITE_PARITY_448_DML_WHERE_ORDER_LIMIT_061.rs) | P1 | memory | GEN_SQL_DML | 2240378 | 3416804 | <span style="color:#dc2626">-52.51%</span> |
| 803 | [00385 SCALAR_CAST_TYPEOF_040](crates/bench/sqlite_parity/cases/SQLITE_PARITY_385_SCALAR_CAST_TYPEOF_040.rs) | P1 | memory | GEN_SQL_SCALAR | 2180195 | 3324920 | <span style="color:#dc2626">-52.51%</span> |
| 804 | [00666 JOIN_SUBQUERY_EXISTS_059](crates/bench/sqlite_parity/cases/SQLITE_PARITY_666_JOIN_SUBQUERY_EXISTS_059.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2450987 | 3735366 | <span style="color:#dc2626">-52.40%</span> |
| 805 | [00769 CTE_RECURSIVE_MATRIX_062](crates/bench/sqlite_parity/cases/SQLITE_PARITY_769_CTE_RECURSIVE_MATRIX_062.rs) | P1 | memory | GEN_SQL_CTE | 2257350 | 3438565 | <span style="color:#dc2626">-52.33%</span> |
| 806 | [00234 SCALAR_STRING_002](crates/bench/sqlite_parity/cases/SQLITE_PARITY_234_SCALAR_STRING_002.rs) | P1 | memory | GEN_SQL_SCALAR | 1821005 | 2773888 | <span style="color:#dc2626">-52.33%</span> |
| 807 | [00820 WINDOW_PARTITION_SUM_033](crates/bench/sqlite_parity/cases/SQLITE_PARITY_820_WINDOW_PARTITION_SUM_033.rs) | P2 | memory | GEN_SQL_WINDOW | 2128236 | 3241072 | <span style="color:#dc2626">-52.29%</span> |
| 808 | [00828 WINDOW_PARTITION_SUM_041](crates/bench/sqlite_parity/cases/SQLITE_PARITY_828_WINDOW_PARTITION_SUM_041.rs) | P2 | memory | GEN_SQL_WINDOW | 2194813 | 3341351 | <span style="color:#dc2626">-52.24%</span> |
| 809 | [00396 DML_WHERE_ORDER_LIMIT_009](crates/bench/sqlite_parity/cases/SQLITE_PARITY_396_DML_WHERE_ORDER_LIMIT_009.rs) | P1 | memory | GEN_SQL_DML | 2169945 | 3300625 | <span style="color:#dc2626">-52.11%</span> |
| 810 | [00646 JOIN_SUBQUERY_EXISTS_039](crates/bench/sqlite_parity/cases/SQLITE_PARITY_646_JOIN_SUBQUERY_EXISTS_039.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2173091 | 3303349 | <span style="color:#dc2626">-52.01%</span> |
| 811 | [00743 CTE_RECURSIVE_MATRIX_036](crates/bench/sqlite_parity/cases/SQLITE_PARITY_743_CTE_RECURSIVE_MATRIX_036.rs) | P1 | memory | GEN_SQL_CTE | 2462659 | 3742891 | <span style="color:#dc2626">-51.99%</span> |
| 812 | [00029 ALTER_TABLE_RENAME_COLUMN](crates/bench/sqlite_parity/cases/SQLITE_PARITY_029_ALTER_TABLE_RENAME_COLUMN.rs) | P0 | memory | SQL_ALTER | 2510018 | 3814406 | <span style="color:#dc2626">-51.97%</span> |
| 813 | [00736 CTE_RECURSIVE_MATRIX_029](crates/bench/sqlite_parity/cases/SQLITE_PARITY_736_CTE_RECURSIVE_MATRIX_029.rs) | P1 | memory | GEN_SQL_CTE | 2157291 | 3274375 | <span style="color:#dc2626">-51.78%</span> |
| 814 | [00193 OPT_READONLY_TEMPFILE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_193_OPT_READONLY_TEMPFILE.rs) | P2 | tempfile | CLI_OPTION_TEMPFILE | 7816998 | 11830421 | <span style="color:#dc2626">-51.34%</span> |
| 815 | [00034 EXPRESSION_INDEX](crates/bench/sqlite_parity/cases/SQLITE_PARITY_034_EXPRESSION_INDEX.rs) | P0 | memory | SQL_INDEX | 2206514 | 3337815 | <span style="color:#dc2626">-51.27%</span> |
| 816 | [00190 OPT_BAIL](crates/bench/sqlite_parity/cases/SQLITE_PARITY_190_OPT_BAIL.rs) | P1 | memory | CLI_OPTION_NEGATIVE | 1746043 | 2641107 | <span style="color:#dc2626">-51.26%</span> |
| 817 | [00956 VIEW_TRIGGER_GENERATED_009](crates/bench/sqlite_parity/cases/SQLITE_PARITY_956_VIEW_TRIGGER_GENERATED_009.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2354234 | 3558913 | <span style="color:#dc2626">-51.17%</span> |
| 818 | [00016 UPDATE_RETURNING](crates/bench/sqlite_parity/cases/SQLITE_PARITY_016_UPDATE_RETURNING.rs) | P0 | memory | SQL_UPDATE | 1959186 | 2959318 | <span style="color:#dc2626">-51.05%</span> |
| 819 | [00298 SCALAR_STRING_018](crates/bench/sqlite_parity/cases/SQLITE_PARITY_298_SCALAR_STRING_018.rs) | P1 | memory | GEN_SQL_SCALAR | 1880668 | 2837679 | <span style="color:#dc2626">-50.89%</span> |
| 820 | [00039 CREATE_TRIGGER_BEFORE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_039_CREATE_TRIGGER_BEFORE.rs) | P0 | memory | SQL_TRIGGER | 2059486 | 3105535 | <span style="color:#dc2626">-50.79%</span> |
| 821 | [00619 JOIN_SUBQUERY_EXISTS_012](crates/bench/sqlite_parity/cases/SQLITE_PARITY_619_JOIN_SUBQUERY_EXISTS_012.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2332623 | 3516383 | <span style="color:#dc2626">-50.75%</span> |
| 822 | [00922 CONSTRAINT_FK_SAVEPOINT_055](crates/bench/sqlite_parity/cases/SQLITE_PARITY_922_CONSTRAINT_FK_SAVEPOINT_055.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2044026 | 3078524 | <span style="color:#dc2626">-50.61%</span> |
| 823 | [00080 NOT_NULL_FAILURE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_080_NOT_NULL_FAILURE.rs) | P0 | memory | SQL_CONSTRAINTS_NEGATIVE | 1881399 | 2830415 | <span style="color:#dc2626">-50.44%</span> |
| 824 | [00332 SCALAR_ARITH_027](crates/bench/sqlite_parity/cases/SQLITE_PARITY_332_SCALAR_ARITH_027.rs) | P1 | memory | GEN_SQL_SCALAR | 1823721 | 2742207 | <span style="color:#dc2626">-50.36%</span> |
| 825 | [00256 SCALAR_ARITH_008](crates/bench/sqlite_parity/cases/SQLITE_PARITY_256_SCALAR_ARITH_008.rs) | P1 | memory | GEN_SQL_SCALAR | 1746875 | 2622962 | <span style="color:#dc2626">-50.15%</span> |
| 826 | [00417 DML_WHERE_ORDER_LIMIT_030](crates/bench/sqlite_parity/cases/SQLITE_PARITY_417_DML_WHERE_ORDER_LIMIT_030.rs) | P1 | memory | GEN_SQL_DML | 2345517 | 3515130 | <span style="color:#dc2626">-49.87%</span> |
| 827 | [01014 JSON_EXTRACT_SET_007](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1014_JSON_EXTRACT_SET_007.rs) | P2 | memory | GEN_SQL_JSON | 2399910 | 3596063 | <span style="color:#dc2626">-49.84%</span> |
| 828 | [00711 CTE_RECURSIVE_MATRIX_004](crates/bench/sqlite_parity/cases/SQLITE_PARITY_711_CTE_RECURSIVE_MATRIX_004.rs) | P1 | memory | GEN_SQL_CTE | 2143215 | 3210814 | <span style="color:#dc2626">-49.81%</span> |
| 829 | [00893 CONSTRAINT_FK_SAVEPOINT_026](crates/bench/sqlite_parity/cases/SQLITE_PARITY_893_CONSTRAINT_FK_SAVEPOINT_026.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2186026 | 3273032 | <span style="color:#dc2626">-49.73%</span> |
| 830 | [00062 WINDOW_FRAMES_ROWS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_062_WINDOW_FRAMES_ROWS.rs) | P0 | memory | SQL_WINDOW | 1979324 | 2962775 | <span style="color:#dc2626">-49.69%</span> |
| 831 | [00129 DOT_CONNECTION](crates/bench/sqlite_parity/cases/SQLITE_PARITY_129_DOT_CONNECTION.rs) | P0 | memory | CLI_DOT_COMMAND | 1979354 | 2961723 | <span style="color:#dc2626">-49.63%</span> |
| 832 | [00182 OPT_COLUMN_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_182_OPT_COLUMN_MODE.rs) | P2 | memory | CLI_OPTION | 1823018 | 2726919 | <span style="color:#dc2626">-49.58%</span> |
| 833 | [00507 DML_WHERE_ORDER_LIMIT_120](crates/bench/sqlite_parity/cases/SQLITE_PARITY_507_DML_WHERE_ORDER_LIMIT_120.rs) | P1 | memory | GEN_SQL_DML | 2240879 | 3349236 | <span style="color:#dc2626">-49.46%</span> |
| 834 | [00937 CONSTRAINT_FK_SAVEPOINT_070](crates/bench/sqlite_parity/cases/SQLITE_PARITY_937_CONSTRAINT_FK_SAVEPOINT_070.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2007007 | 2999044 | <span style="color:#dc2626">-49.43%</span> |
| 835 | [01097 INDEX_SCHEMA_PRAGMA_030](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1097_INDEX_SCHEMA_PRAGMA_030.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 2335688 | 3490153 | <span style="color:#dc2626">-49.43%</span> |
| 836 | [00014 INSERT_RETURNING](crates/bench/sqlite_parity/cases/SQLITE_PARITY_014_INSERT_RETURNING.rs) | P0 | memory | SQL_INSERT | 1903011 | 2842588 | <span style="color:#dc2626">-49.37%</span> |
| 837 | [00549 AGG_GROUP_HAVING_042](crates/bench/sqlite_parity/cases/SQLITE_PARITY_549_AGG_GROUP_HAVING_042.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2272680 | 3382419 | <span style="color:#dc2626">-48.83%</span> |
| 838 | [00090 JSON_MUTATION_FUNCTIONS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_090_JSON_MUTATION_FUNCTIONS.rs) | P0 | memory | SQL_JSON | 1924500 | 2861383 | <span style="color:#dc2626">-48.68%</span> |
| 839 | [00107 DOT_HELP_PATTERN](crates/bench/sqlite_parity/cases/SQLITE_PARITY_107_DOT_HELP_PATTERN.rs) | P0 | memory | CLI_DOT_COMMAND | 1783845 | 2651987 | <span style="color:#dc2626">-48.67%</span> |
| 840 | [00228 SCALAR_ARITH_001](crates/bench/sqlite_parity/cases/SQLITE_PARITY_228_SCALAR_ARITH_001.rs) | P1 | memory | GEN_SQL_SCALAR | 1749499 | 2600230 | <span style="color:#dc2626">-48.63%</span> |
| 841 | [01086 INDEX_SCHEMA_PRAGMA_019](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1086_INDEX_SCHEMA_PRAGMA_019.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 2337322 | 3470225 | <span style="color:#dc2626">-48.47%</span> |
| 842 | [00689 JOIN_SUBQUERY_EXISTS_082](crates/bench/sqlite_parity/cases/SQLITE_PARITY_689_JOIN_SUBQUERY_EXISTS_082.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2513034 | 3729836 | <span style="color:#dc2626">-48.42%</span> |
| 843 | [01084 INDEX_SCHEMA_PRAGMA_017](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1084_INDEX_SCHEMA_PRAGMA_017.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 2383108 | 3533304 | <span style="color:#dc2626">-48.26%</span> |
| 844 | [00816 WINDOW_PARTITION_SUM_029](crates/bench/sqlite_parity/cases/SQLITE_PARITY_816_WINDOW_PARTITION_SUM_029.rs) | P2 | memory | GEN_SQL_WINDOW | 2111725 | 3128920 | <span style="color:#dc2626">-48.17%</span> |
| 845 | [00546 AGG_GROUP_HAVING_039](crates/bench/sqlite_parity/cases/SQLITE_PARITY_546_AGG_GROUP_HAVING_039.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2298649 | 3404410 | <span style="color:#dc2626">-48.10%</span> |
| 846 | [00696 JOIN_SUBQUERY_EXISTS_089](crates/bench/sqlite_parity/cases/SQLITE_PARITY_696_JOIN_SUBQUERY_EXISTS_089.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2480282 | 3657158 | <span style="color:#dc2626">-47.45%</span> |
| 847 | [01035 JSON_EXTRACT_SET_028](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1035_JSON_EXTRACT_SET_028.rs) | P2 | memory | GEN_SQL_JSON | 2150118 | 3169647 | <span style="color:#dc2626">-47.42%</span> |
| 848 | [01119 INDEX_SCHEMA_PRAGMA_052](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1119_INDEX_SCHEMA_PRAGMA_052.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 2315992 | 3412426 | <span style="color:#dc2626">-47.34%</span> |
| 849 | [01077 INDEX_SCHEMA_PRAGMA_010](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1077_INDEX_SCHEMA_PRAGMA_010.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 2329357 | 3431512 | <span style="color:#dc2626">-47.32%</span> |
| 850 | [00185 OPT_SEPARATOR](crates/bench/sqlite_parity/cases/SQLITE_PARITY_185_OPT_SEPARATOR.rs) | P1 | memory | CLI_OPTION | 1802670 | 2653720 | <span style="color:#dc2626">-47.21%</span> |
| 851 | [00667 JOIN_SUBQUERY_EXISTS_060](crates/bench/sqlite_parity/cases/SQLITE_PARITY_667_JOIN_SUBQUERY_EXISTS_060.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2639814 | 3871354 | <span style="color:#dc2626">-46.65%</span> |
| 852 | [00419 DML_WHERE_ORDER_LIMIT_032](crates/bench/sqlite_parity/cases/SQLITE_PARITY_419_DML_WHERE_ORDER_LIMIT_032.rs) | P1 | memory | GEN_SQL_DML | 2308948 | 3381136 | <span style="color:#dc2626">-46.44%</span> |
| 853 | [00416 DML_WHERE_ORDER_LIMIT_029](crates/bench/sqlite_parity/cases/SQLITE_PARITY_416_DML_WHERE_ORDER_LIMIT_029.rs) | P1 | memory | GEN_SQL_DML | 2684298 | 3929413 | <span style="color:#dc2626">-46.39%</span> |
| 854 | [00037 DROP_VIEW](crates/bench/sqlite_parity/cases/SQLITE_PARITY_037_DROP_VIEW.rs) | P0 | memory | SQL_DROP | 2015583 | 2950361 | <span style="color:#dc2626">-46.38%</span> |
| 855 | [00260 SCALAR_ARITH_009](crates/bench/sqlite_parity/cases/SQLITE_PARITY_260_SCALAR_ARITH_009.rs) | P1 | memory | GEN_SQL_SCALAR | 1842286 | 2696071 | <span style="color:#dc2626">-46.34%</span> |
| 856 | [00862 WINDOW_PARTITION_SUM_075](crates/bench/sqlite_parity/cases/SQLITE_PARITY_862_WINDOW_PARTITION_SUM_075.rs) | P2 | memory | GEN_SQL_WINDOW | 2194462 | 3211265 | <span style="color:#dc2626">-46.33%</span> |
| 857 | [00818 WINDOW_PARTITION_SUM_031](crates/bench/sqlite_parity/cases/SQLITE_PARITY_818_WINDOW_PARTITION_SUM_031.rs) | P2 | memory | GEN_SQL_WINDOW | 2198810 | 3215212 | <span style="color:#dc2626">-46.23%</span> |
| 858 | [00306 SCALAR_STRING_020](crates/bench/sqlite_parity/cases/SQLITE_PARITY_306_SCALAR_STRING_020.rs) | P1 | memory | GEN_SQL_SCALAR | 1972131 | 2883555 | <span style="color:#dc2626">-46.22%</span> |
| 859 | [00540 AGG_GROUP_HAVING_033](crates/bench/sqlite_parity/cases/SQLITE_PARITY_540_AGG_GROUP_HAVING_033.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2297827 | 3357101 | <span style="color:#dc2626">-46.10%</span> |
| 860 | [00387 SCALAR_NULL_COALESCE_040](crates/bench/sqlite_parity/cases/SQLITE_PARITY_387_SCALAR_NULL_COALESCE_040.rs) | P1 | memory | GEN_SQL_SCALAR | 2518164 | 3674742 | <span style="color:#dc2626">-45.93%</span> |
| 861 | [00627 JOIN_SUBQUERY_EXISTS_020](crates/bench/sqlite_parity/cases/SQLITE_PARITY_627_JOIN_SUBQUERY_EXISTS_020.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2503105 | 3645607 | <span style="color:#dc2626">-45.64%</span> |
| 862 | [00795 WINDOW_PARTITION_SUM_008](crates/bench/sqlite_parity/cases/SQLITE_PARITY_795_WINDOW_PARTITION_SUM_008.rs) | P2 | memory | GEN_SQL_WINDOW | 2132053 | 3103200 | <span style="color:#dc2626">-45.55%</span> |
| 863 | [01006 VIEW_TRIGGER_GENERATED_059](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1006_VIEW_TRIGGER_GENERATED_059.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2395862 | 3475665 | <span style="color:#dc2626">-45.07%</span> |
| 864 | [00450 DML_WHERE_ORDER_LIMIT_063](crates/bench/sqlite_parity/cases/SQLITE_PARITY_450_DML_WHERE_ORDER_LIMIT_063.rs) | P1 | memory | GEN_SQL_DML | 2698144 | 3913674 | <span style="color:#dc2626">-45.05%</span> |
| 865 | [00244 SCALAR_ARITH_005](crates/bench/sqlite_parity/cases/SQLITE_PARITY_244_SCALAR_ARITH_005.rs) | P1 | memory | GEN_SQL_SCALAR | 1786109 | 2586854 | <span style="color:#dc2626">-44.83%</span> |
| 866 | [00633 JOIN_SUBQUERY_EXISTS_026](crates/bench/sqlite_parity/cases/SQLITE_PARITY_633_JOIN_SUBQUERY_EXISTS_026.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2323936 | 3363012 | <span style="color:#dc2626">-44.71%</span> |
| 867 | [01088 INDEX_SCHEMA_PRAGMA_021](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1088_INDEX_SCHEMA_PRAGMA_021.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 2506952 | 3623735 | <span style="color:#dc2626">-44.55%</span> |
| 868 | [00808 WINDOW_PARTITION_SUM_021](crates/bench/sqlite_parity/cases/SQLITE_PARITY_808_WINDOW_PARTITION_SUM_021.rs) | P2 | memory | GEN_SQL_WINDOW | 2634053 | 3804097 | <span style="color:#dc2626">-44.42%</span> |
| 869 | [00890 CONSTRAINT_FK_SAVEPOINT_023](crates/bench/sqlite_parity/cases/SQLITE_PARITY_890_CONSTRAINT_FK_SAVEPOINT_023.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2907501 | 4194035 | <span style="color:#dc2626">-44.25%</span> |
| 870 | [00656 JOIN_SUBQUERY_EXISTS_049](crates/bench/sqlite_parity/cases/SQLITE_PARITY_656_JOIN_SUBQUERY_EXISTS_049.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2876081 | 4141084 | <span style="color:#dc2626">-43.98%</span> |
| 871 | [00753 CTE_RECURSIVE_MATRIX_046](crates/bench/sqlite_parity/cases/SQLITE_PARITY_753_CTE_RECURSIVE_MATRIX_046.rs) | P1 | memory | GEN_SQL_CTE | 2859139 | 4114825 | <span style="color:#dc2626">-43.92%</span> |
| 872 | [00600 AGG_GROUP_HAVING_093](crates/bench/sqlite_parity/cases/SQLITE_PARITY_600_AGG_GROUP_HAVING_093.rs) | P1 | memory | GEN_SQL_AGGREGATE | 3003893 | 4321957 | <span style="color:#dc2626">-43.88%</span> |
| 873 | [00221 OPT_DOUBLE_DASH_END_OPTIONS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_221_OPT_DOUBLE_DASH_END_OPTIONS.rs) | P2 | memory | CLI_OPTION | 1877271 | 2699917 | <span style="color:#dc2626">-43.82%</span> |
| 874 | [00489 DML_WHERE_ORDER_LIMIT_102](crates/bench/sqlite_parity/cases/SQLITE_PARITY_489_DML_WHERE_ORDER_LIMIT_102.rs) | P1 | memory | GEN_SQL_DML | 2568469 | 3686895 | <span style="color:#dc2626">-43.54%</span> |
| 875 | [00855 WINDOW_PARTITION_SUM_068](crates/bench/sqlite_parity/cases/SQLITE_PARITY_855_WINDOW_PARTITION_SUM_068.rs) | P2 | memory | GEN_SQL_WINDOW | 2091196 | 2991409 | <span style="color:#dc2626">-43.05%</span> |
| 876 | [00731 CTE_RECURSIVE_MATRIX_024](crates/bench/sqlite_parity/cases/SQLITE_PARITY_731_CTE_RECURSIVE_MATRIX_024.rs) | P1 | memory | GEN_SQL_CTE | 2371486 | 3392157 | <span style="color:#dc2626">-43.04%</span> |
| 877 | [00814 WINDOW_PARTITION_SUM_027](crates/bench/sqlite_parity/cases/SQLITE_PARITY_814_WINDOW_PARTITION_SUM_027.rs) | P2 | memory | GEN_SQL_WINDOW | 2386915 | 3413408 | <span style="color:#dc2626">-43.01%</span> |
| 878 | [00031 CREATE_INDEX](crates/bench/sqlite_parity/cases/SQLITE_PARITY_031_CREATE_INDEX.rs) | P0 | memory | SQL_INDEX | 2817340 | 4019725 | <span style="color:#dc2626">-42.68%</span> |
| 879 | [00703 JOIN_SUBQUERY_EXISTS_096](crates/bench/sqlite_parity/cases/SQLITE_PARITY_703_JOIN_SUBQUERY_EXISTS_096.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2925926 | 4169287 | <span style="color:#dc2626">-42.49%</span> |
| 880 | [00581 AGG_GROUP_HAVING_074](crates/bench/sqlite_parity/cases/SQLITE_PARITY_581_AGG_GROUP_HAVING_074.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2274984 | 3239198 | <span style="color:#dc2626">-42.38%</span> |
| 881 | [00151 DOT_SAVE_RESTORE_TEMPFILE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_151_DOT_SAVE_RESTORE_TEMPFILE.rs) | P1 | tempfile | CLI_TEMPFILE | 3131154 | 4455018 | <span style="color:#dc2626">-42.28%</span> |
| 882 | [00059 AGGREGATE_FUNCTIONS_CORE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_059_AGGREGATE_FUNCTIONS_CORE.rs) | P0 | memory | SQL_FUNCTIONS | 2123948 | 3015084 | <span style="color:#dc2626">-41.96%</span> |
| 883 | [00399 DML_WHERE_ORDER_LIMIT_012](crates/bench/sqlite_parity/cases/SQLITE_PARITY_399_DML_WHERE_ORDER_LIMIT_012.rs) | P1 | memory | GEN_SQL_DML | 3261961 | 4615833 | <span style="color:#dc2626">-41.50%</span> |
| 884 | [00531 AGG_GROUP_HAVING_024](crates/bench/sqlite_parity/cases/SQLITE_PARITY_531_AGG_GROUP_HAVING_024.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2289441 | 3234810 | <span style="color:#dc2626">-41.29%</span> |
| 885 | [00599 AGG_GROUP_HAVING_092](crates/bench/sqlite_parity/cases/SQLITE_PARITY_599_AGG_GROUP_HAVING_092.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2642338 | 3730767 | <span style="color:#dc2626">-41.19%</span> |
| 886 | [00469 DML_WHERE_ORDER_LIMIT_082](crates/bench/sqlite_parity/cases/SQLITE_PARITY_469_DML_WHERE_ORDER_LIMIT_082.rs) | P1 | memory | GEN_SQL_DML | 2565213 | 3619939 | <span style="color:#dc2626">-41.12%</span> |
| 887 | [00904 CONSTRAINT_FK_SAVEPOINT_037](crates/bench/sqlite_parity/cases/SQLITE_PARITY_904_CONSTRAINT_FK_SAVEPOINT_037.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2171990 | 3061812 | <span style="color:#dc2626">-40.97%</span> |
| 888 | [00402 DML_WHERE_ORDER_LIMIT_015](crates/bench/sqlite_parity/cases/SQLITE_PARITY_402_DML_WHERE_ORDER_LIMIT_015.rs) | P1 | memory | GEN_SQL_DML | 2689879 | 3791362 | <span style="color:#dc2626">-40.95%</span> |
| 889 | [00361 SCALAR_CAST_TYPEOF_034](crates/bench/sqlite_parity/cases/SQLITE_PARITY_361_SCALAR_CAST_TYPEOF_034.rs) | P1 | memory | GEN_SQL_SCALAR | 2147522 | 3022898 | <span style="color:#dc2626">-40.76%</span> |
| 890 | [00334 SCALAR_STRING_027](crates/bench/sqlite_parity/cases/SQLITE_PARITY_334_SCALAR_STRING_027.rs) | P1 | memory | GEN_SQL_SCALAR | 1988732 | 2797833 | <span style="color:#dc2626">-40.68%</span> |
| 891 | [01001 VIEW_TRIGGER_GENERATED_054](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1001_VIEW_TRIGGER_GENERATED_054.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 3129821 | 4397860 | <span style="color:#dc2626">-40.51%</span> |
| 892 | [00598 AGG_GROUP_HAVING_091](crates/bench/sqlite_parity/cases/SQLITE_PARITY_598_AGG_GROUP_HAVING_091.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2679298 | 3758400 | <span style="color:#dc2626">-40.28%</span> |
| 893 | [00595 AGG_GROUP_HAVING_088](crates/bench/sqlite_parity/cases/SQLITE_PARITY_595_AGG_GROUP_HAVING_088.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2786131 | 3907242 | <span style="color:#dc2626">-40.24%</span> |
| 894 | [01102 INDEX_SCHEMA_PRAGMA_035](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1102_INDEX_SCHEMA_PRAGMA_035.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 3198811 | 4475147 | <span style="color:#dc2626">-39.90%</span> |
| 895 | [00724 CTE_RECURSIVE_MATRIX_017](crates/bench/sqlite_parity/cases/SQLITE_PARITY_724_CTE_RECURSIVE_MATRIX_017.rs) | P1 | memory | GEN_SQL_CTE | 2478629 | 3465306 | <span style="color:#dc2626">-39.81%</span> |
| 896 | [00360 SCALAR_ARITH_034](crates/bench/sqlite_parity/cases/SQLITE_PARITY_360_SCALAR_ARITH_034.rs) | P1 | memory | GEN_SQL_SCALAR | 2777425 | 3881042 | <span style="color:#dc2626">-39.74%</span> |
| 897 | [00380 SCALAR_ARITH_039](crates/bench/sqlite_parity/cases/SQLITE_PARITY_380_SCALAR_ARITH_039.rs) | P1 | memory | GEN_SQL_SCALAR | 2820766 | 3934984 | <span style="color:#dc2626">-39.50%</span> |
| 898 | [00971 VIEW_TRIGGER_GENERATED_024](crates/bench/sqlite_parity/cases/SQLITE_PARITY_971_VIEW_TRIGGER_GENERATED_024.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2310190 | 3215834 | <span style="color:#dc2626">-39.20%</span> |
| 899 | [00333 SCALAR_CAST_TYPEOF_027](crates/bench/sqlite_parity/cases/SQLITE_PARITY_333_SCALAR_CAST_TYPEOF_027.rs) | P1 | memory | GEN_SQL_SCALAR | 2050379 | 2853618 | <span style="color:#dc2626">-39.18%</span> |
| 900 | [00184 OPT_ASCII_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_184_OPT_ASCII_MODE.rs) | P2 | memory | CLI_OPTION | 1877201 | 2610669 | <span style="color:#dc2626">-39.07%</span> |
| 901 | [00704 JOIN_SUBQUERY_EXISTS_097](crates/bench/sqlite_parity/cases/SQLITE_PARITY_704_JOIN_SUBQUERY_EXISTS_097.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2518364 | 3498158 | <span style="color:#dc2626">-38.91%</span> |
| 902 | [00552 AGG_GROUP_HAVING_045](crates/bench/sqlite_parity/cases/SQLITE_PARITY_552_AGG_GROUP_HAVING_045.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2282107 | 3169746 | <span style="color:#dc2626">-38.90%</span> |
| 903 | [01020 JSON_EXTRACT_SET_013](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1020_JSON_EXTRACT_SET_013.rs) | P2 | memory | GEN_SQL_JSON | 2306834 | 3202749 | <span style="color:#dc2626">-38.84%</span> |
| 904 | [00628 JOIN_SUBQUERY_EXISTS_021](crates/bench/sqlite_parity/cases/SQLITE_PARITY_628_JOIN_SUBQUERY_EXISTS_021.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2507093 | 3476096 | <span style="color:#dc2626">-38.65%</span> |
| 905 | [00857 WINDOW_PARTITION_SUM_070](crates/bench/sqlite_parity/cases/SQLITE_PARITY_857_WINDOW_PARTITION_SUM_070.rs) | P2 | memory | GEN_SQL_WINDOW | 2290022 | 3174907 | <span style="color:#dc2626">-38.64%</span> |
| 906 | [00525 AGG_GROUP_HAVING_018](crates/bench/sqlite_parity/cases/SQLITE_PARITY_525_AGG_GROUP_HAVING_018.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2381746 | 3300985 | <span style="color:#dc2626">-38.60%</span> |
| 907 | [00135 DOT_PROGRESS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_135_DOT_PROGRESS.rs) | P0 | memory | CLI_DOT_COMMAND | 1918841 | 2654913 | <span style="color:#dc2626">-38.36%</span> |
| 908 | [00782 CTE_RECURSIVE_MATRIX_075](crates/bench/sqlite_parity/cases/SQLITE_PARITY_782_CTE_RECURSIVE_MATRIX_075.rs) | P1 | memory | GEN_SQL_CTE | 2615678 | 3615530 | <span style="color:#dc2626">-38.23%</span> |
| 909 | [00688 JOIN_SUBQUERY_EXISTS_081](crates/bench/sqlite_parity/cases/SQLITE_PARITY_688_JOIN_SUBQUERY_EXISTS_081.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3014923 | 4166612 | <span style="color:#dc2626">-38.20%</span> |
| 910 | [00121 DOT_PARAMETER](crates/bench/sqlite_parity/cases/SQLITE_PARITY_121_DOT_PARAMETER.rs) | P0 | memory | CLI_DOT_COMMAND | 2146951 | 2966812 | <span style="color:#dc2626">-38.19%</span> |
| 911 | [00951 VIEW_TRIGGER_GENERATED_004](crates/bench/sqlite_parity/cases/SQLITE_PARITY_951_VIEW_TRIGGER_GENERATED_004.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2715597 | 3741048 | <span style="color:#dc2626">-37.76%</span> |
| 912 | [01042 JSON_EXTRACT_SET_035](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1042_JSON_EXTRACT_SET_035.rs) | P2 | memory | GEN_SQL_JSON | 2988213 | 4112741 | <span style="color:#dc2626">-37.63%</span> |
| 913 | [00475 DML_WHERE_ORDER_LIMIT_088](crates/bench/sqlite_parity/cases/SQLITE_PARITY_475_DML_WHERE_ORDER_LIMIT_088.rs) | P1 | memory | GEN_SQL_DML | 2483648 | 3416333 | <span style="color:#dc2626">-37.55%</span> |
| 914 | [00319 SCALAR_NULL_COALESCE_023](crates/bench/sqlite_parity/cases/SQLITE_PARITY_319_SCALAR_NULL_COALESCE_023.rs) | P1 | memory | GEN_SQL_SCALAR | 2100123 | 2888774 | <span style="color:#dc2626">-37.55%</span> |
| 915 | [00388 DML_WHERE_ORDER_LIMIT_001](crates/bench/sqlite_parity/cases/SQLITE_PARITY_388_DML_WHERE_ORDER_LIMIT_001.rs) | P1 | memory | GEN_SQL_DML | 2588076 | 3557600 | <span style="color:#dc2626">-37.46%</span> |
| 916 | [00776 CTE_RECURSIVE_MATRIX_069](crates/bench/sqlite_parity/cases/SQLITE_PARITY_776_CTE_RECURSIVE_MATRIX_069.rs) | P1 | memory | GEN_SQL_CTE | 2612201 | 3587747 | <span style="color:#dc2626">-37.35%</span> |
| 917 | [00851 WINDOW_PARTITION_SUM_064](crates/bench/sqlite_parity/cases/SQLITE_PARITY_851_WINDOW_PARTITION_SUM_064.rs) | P2 | memory | GEN_SQL_WINDOW | 2357029 | 3232636 | <span style="color:#dc2626">-37.15%</span> |
| 918 | [00122 DOT_CHANGES](crates/bench/sqlite_parity/cases/SQLITE_PARITY_122_DOT_CHANGES.rs) | P0 | memory | CLI_DOT_COMMAND | 2237633 | 3065941 | <span style="color:#dc2626">-37.02%</span> |
| 919 | [01051 JSON_EXTRACT_SET_044](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1051_JSON_EXTRACT_SET_044.rs) | P2 | memory | GEN_SQL_JSON | 2483268 | 3392107 | <span style="color:#dc2626">-36.60%</span> |
| 920 | [00458 DML_WHERE_ORDER_LIMIT_071](crates/bench/sqlite_parity/cases/SQLITE_PARITY_458_DML_WHERE_ORDER_LIMIT_071.rs) | P1 | memory | GEN_SQL_DML | 2661044 | 3633514 | <span style="color:#dc2626">-36.54%</span> |
| 921 | [00056 SUBQUERIES_EXISTS_IN](crates/bench/sqlite_parity/cases/SQLITE_PARITY_056_SUBQUERIES_EXISTS_IN.rs) | P0 | memory | SQL_SELECT | 2964067 | 4046726 | <span style="color:#dc2626">-36.53%</span> |
| 922 | [00403 DML_WHERE_ORDER_LIMIT_016](crates/bench/sqlite_parity/cases/SQLITE_PARITY_403_DML_WHERE_ORDER_LIMIT_016.rs) | P1 | memory | GEN_SQL_DML | 2557799 | 3489742 | <span style="color:#dc2626">-36.44%</span> |
| 923 | [00006 ROWID_INTEGER_PRIMARY_KEY](crates/bench/sqlite_parity/cases/SQLITE_PARITY_006_ROWID_INTEGER_PRIMARY_KEY.rs) | P0 | memory | SQL_ROWID | 3040532 | 4145031 | <span style="color:#dc2626">-36.33%</span> |
| 924 | [00191 OPT_BATCH](crates/bench/sqlite_parity/cases/SQLITE_PARITY_191_OPT_BATCH.rs) | P2 | memory | CLI_OPTION | 1948035 | 2653350 | <span style="color:#dc2626">-36.21%</span> |
| 925 | [00641 JOIN_SUBQUERY_EXISTS_034](crates/bench/sqlite_parity/cases/SQLITE_PARITY_641_JOIN_SUBQUERY_EXISTS_034.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2557408 | 3479062 | <span style="color:#dc2626">-36.04%</span> |
| 926 | [00413 DML_WHERE_ORDER_LIMIT_026](crates/bench/sqlite_parity/cases/SQLITE_PARITY_413_DML_WHERE_ORDER_LIMIT_026.rs) | P1 | memory | GEN_SQL_DML | 2548672 | 3466177 | <span style="color:#dc2626">-36.00%</span> |
| 927 | [00991 VIEW_TRIGGER_GENERATED_044](crates/bench/sqlite_parity/cases/SQLITE_PARITY_991_VIEW_TRIGGER_GENERATED_044.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2550795 | 3464144 | <span style="color:#dc2626">-35.81%</span> |
| 928 | [00720 CTE_RECURSIVE_MATRIX_013](crates/bench/sqlite_parity/cases/SQLITE_PARITY_720_CTE_RECURSIVE_MATRIX_013.rs) | P1 | memory | GEN_SQL_CTE | 2385783 | 3236723 | <span style="color:#dc2626">-35.67%</span> |
| 929 | [00307 SCALAR_NULL_COALESCE_020](crates/bench/sqlite_parity/cases/SQLITE_PARITY_307_SCALAR_NULL_COALESCE_020.rs) | P1 | memory | GEN_SQL_SCALAR | 2156360 | 2921066 | <span style="color:#dc2626">-35.46%</span> |
| 930 | [00706 JOIN_SUBQUERY_EXISTS_099](crates/bench/sqlite_parity/cases/SQLITE_PARITY_706_JOIN_SUBQUERY_EXISTS_099.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2634173 | 3567118 | <span style="color:#dc2626">-35.42%</span> |
| 931 | [00788 WINDOW_PARTITION_SUM_001](crates/bench/sqlite_parity/cases/SQLITE_PARITY_788_WINDOW_PARTITION_SUM_001.rs) | P2 | memory | GEN_SQL_WINDOW | 2370374 | 3208770 | <span style="color:#dc2626">-35.37%</span> |
| 932 | [00350 SCALAR_STRING_031](crates/bench/sqlite_parity/cases/SQLITE_PARITY_350_SCALAR_STRING_031.rs) | P1 | memory | GEN_SQL_SCALAR | 2067191 | 2797793 | <span style="color:#dc2626">-35.34%</span> |
| 933 | [00383 SCALAR_NULL_COALESCE_039](crates/bench/sqlite_parity/cases/SQLITE_PARITY_383_SCALAR_NULL_COALESCE_039.rs) | P1 | memory | GEN_SQL_SCALAR | 2350146 | 3179976 | <span style="color:#dc2626">-35.31%</span> |
| 934 | [00326 SCALAR_STRING_025](crates/bench/sqlite_parity/cases/SQLITE_PARITY_326_SCALAR_STRING_025.rs) | P1 | memory | GEN_SQL_SCALAR | 2135790 | 2877033 | <span style="color:#dc2626">-34.71%</span> |
| 935 | [00799 WINDOW_PARTITION_SUM_012](crates/bench/sqlite_parity/cases/SQLITE_PARITY_799_WINDOW_PARTITION_SUM_012.rs) | P2 | memory | GEN_SQL_WINDOW | 2216563 | 2985819 | <span style="color:#dc2626">-34.70%</span> |
| 936 | [00839 WINDOW_PARTITION_SUM_052](crates/bench/sqlite_parity/cases/SQLITE_PARITY_839_WINDOW_PARTITION_SUM_052.rs) | P2 | memory | GEN_SQL_WINDOW | 2453632 | 3304040 | <span style="color:#dc2626">-34.66%</span> |
| 937 | [00742 CTE_RECURSIVE_MATRIX_035](crates/bench/sqlite_parity/cases/SQLITE_PARITY_742_CTE_RECURSIVE_MATRIX_035.rs) | P1 | memory | GEN_SQL_CTE | 3290274 | 4423478 | <span style="color:#dc2626">-34.44%</span> |
| 938 | [01064 JSON_EXTRACT_SET_057](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1064_JSON_EXTRACT_SET_057.rs) | P2 | memory | GEN_SQL_JSON | 2524816 | 3388390 | <span style="color:#dc2626">-34.20%</span> |
| 939 | [00674 JOIN_SUBQUERY_EXISTS_067](crates/bench/sqlite_parity/cases/SQLITE_PARITY_674_JOIN_SUBQUERY_EXISTS_067.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2695649 | 3612854 | <span style="color:#dc2626">-34.03%</span> |
| 940 | [00727 CTE_RECURSIVE_MATRIX_020](crates/bench/sqlite_parity/cases/SQLITE_PARITY_727_CTE_RECURSIVE_MATRIX_020.rs) | P1 | memory | GEN_SQL_CTE | 2347100 | 3141343 | <span style="color:#dc2626">-33.84%</span> |
| 941 | [00065 CTE_RECURSIVE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_065_CTE_RECURSIVE.rs) | P0 | memory | SQL_CTE | 2429045 | 3247183 | <span style="color:#dc2626">-33.68%</span> |
| 942 | [00284 SCALAR_ARITH_015](crates/bench/sqlite_parity/cases/SQLITE_PARITY_284_SCALAR_ARITH_015.rs) | P1 | memory | GEN_SQL_SCALAR | 2068262 | 2759290 | <span style="color:#dc2626">-33.41%</span> |
| 943 | [01101 INDEX_SCHEMA_PRAGMA_034](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1101_INDEX_SCHEMA_PRAGMA_034.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 3101838 | 4135533 | <span style="color:#dc2626">-33.33%</span> |
| 944 | [00035 DROP_INDEX](crates/bench/sqlite_parity/cases/SQLITE_PARITY_035_DROP_INDEX.rs) | P0 | memory | SQL_DROP | 2332954 | 3104914 | <span style="color:#dc2626">-33.09%</span> |
| 945 | [00455 DML_WHERE_ORDER_LIMIT_068](crates/bench/sqlite_parity/cases/SQLITE_PARITY_455_DML_WHERE_ORDER_LIMIT_068.rs) | P1 | memory | GEN_SQL_DML | 2563610 | 3407256 | <span style="color:#dc2626">-32.91%</span> |
| 946 | [01106 INDEX_SCHEMA_PRAGMA_039](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1106_INDEX_SCHEMA_PRAGMA_039.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 2586584 | 3437202 | <span style="color:#dc2626">-32.89%</span> |
| 947 | [00757 CTE_RECURSIVE_MATRIX_050](crates/bench/sqlite_parity/cases/SQLITE_PARITY_757_CTE_RECURSIVE_MATRIX_050.rs) | P1 | memory | GEN_SQL_CTE | 2393448 | 3179425 | <span style="color:#dc2626">-32.84%</span> |
| 948 | [00672 JOIN_SUBQUERY_EXISTS_065](crates/bench/sqlite_parity/cases/SQLITE_PARITY_672_JOIN_SUBQUERY_EXISTS_065.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2455746 | 3256009 | <span style="color:#dc2626">-32.59%</span> |
| 949 | [00434 DML_WHERE_ORDER_LIMIT_047](crates/bench/sqlite_parity/cases/SQLITE_PARITY_434_DML_WHERE_ORDER_LIMIT_047.rs) | P1 | memory | GEN_SQL_DML | 2567787 | 3404531 | <span style="color:#dc2626">-32.59%</span> |
| 950 | [00880 CONSTRAINT_FK_SAVEPOINT_013](crates/bench/sqlite_parity/cases/SQLITE_PARITY_880_CONSTRAINT_FK_SAVEPOINT_013.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2623052 | 3449005 | <span style="color:#dc2626">-31.49%</span> |
| 951 | [00806 WINDOW_PARTITION_SUM_019](crates/bench/sqlite_parity/cases/SQLITE_PARITY_806_WINDOW_PARTITION_SUM_019.rs) | P2 | memory | GEN_SQL_WINDOW | 2445256 | 3214000 | <span style="color:#dc2626">-31.44%</span> |
| 952 | [00624 JOIN_SUBQUERY_EXISTS_017](crates/bench/sqlite_parity/cases/SQLITE_PARITY_624_JOIN_SUBQUERY_EXISTS_017.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2432101 | 3196627 | <span style="color:#dc2626">-31.43%</span> |
| 953 | [00723 CTE_RECURSIVE_MATRIX_016](crates/bench/sqlite_parity/cases/SQLITE_PARITY_723_CTE_RECURSIVE_MATRIX_016.rs) | P1 | memory | GEN_SQL_CTE | 2438313 | 3190245 | <span style="color:#dc2626">-30.84%</span> |
| 954 | [00592 AGG_GROUP_HAVING_085](crates/bench/sqlite_parity/cases/SQLITE_PARITY_592_AGG_GROUP_HAVING_085.rs) | P1 | memory | GEN_SQL_AGGREGATE | 3076290 | 4020917 | <span style="color:#dc2626">-30.71%</span> |
| 955 | [00562 AGG_GROUP_HAVING_055](crates/bench/sqlite_parity/cases/SQLITE_PARITY_562_AGG_GROUP_HAVING_055.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2370986 | 3097319 | <span style="color:#dc2626">-30.63%</span> |
| 956 | [00423 DML_WHERE_ORDER_LIMIT_036](crates/bench/sqlite_parity/cases/SQLITE_PARITY_423_DML_WHERE_ORDER_LIMIT_036.rs) | P1 | memory | GEN_SQL_DML | 2641136 | 3443684 | <span style="color:#dc2626">-30.39%</span> |
| 957 | [00268 SCALAR_ARITH_011](crates/bench/sqlite_parity/cases/SQLITE_PARITY_268_SCALAR_ARITH_011.rs) | P1 | memory | GEN_SQL_SCALAR | 2141301 | 2784477 | <span style="color:#dc2626">-30.04%</span> |
| 958 | [00077 COMMENTS_AND_CLI_TERMINATORS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_077_COMMENTS_AND_CLI_TERMINATORS.rs) | P0 | memory | CLI_SQL_INPUT | 2099531 | 2723953 | <span style="color:#dc2626">-29.74%</span> |
| 959 | [01104 INDEX_SCHEMA_PRAGMA_037](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1104_INDEX_SCHEMA_PRAGMA_037.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 3074556 | 3985149 | <span style="color:#dc2626">-29.62%</span> |
| 960 | [00617 JOIN_SUBQUERY_EXISTS_010](crates/bench/sqlite_parity/cases/SQLITE_PARITY_617_JOIN_SUBQUERY_EXISTS_010.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2685049 | 3478881 | <span style="color:#dc2626">-29.56%</span> |
| 961 | [00101 SCHEMA_SQLITE_MASTER_ALIAS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_101_SCHEMA_SQLITE_MASTER_ALIAS.rs) | P0 | memory | SQL_SCHEMA | 2428674 | 3138387 | <span style="color:#dc2626">-29.22%</span> |
| 962 | [00719 CTE_RECURSIVE_MATRIX_012](crates/bench/sqlite_parity/cases/SQLITE_PARITY_719_CTE_RECURSIVE_MATRIX_012.rs) | P1 | memory | GEN_SQL_CTE | 2860421 | 3681394 | <span style="color:#dc2626">-28.70%</span> |
| 963 | [00218 PRAGMA_FORMS_SCHEMA_EQUALS_PARENS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_218_PRAGMA_FORMS_SCHEMA_EQUALS_PARENS.rs) | P0 | memory | SQL_PRAGMA | 1817268 | 2333725 | <span style="color:#dc2626">-28.42%</span> |
| 964 | [00594 AGG_GROUP_HAVING_087](crates/bench/sqlite_parity/cases/SQLITE_PARITY_594_AGG_GROUP_HAVING_087.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2843670 | 3647451 | <span style="color:#dc2626">-28.27%</span> |
| 965 | [00440 DML_WHERE_ORDER_LIMIT_053](crates/bench/sqlite_parity/cases/SQLITE_PARITY_440_DML_WHERE_ORDER_LIMIT_053.rs) | P1 | memory | GEN_SQL_DML | 2628563 | 3371378 | <span style="color:#dc2626">-28.26%</span> |
| 966 | [00535 AGG_GROUP_HAVING_028](crates/bench/sqlite_parity/cases/SQLITE_PARITY_535_AGG_GROUP_HAVING_028.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2523063 | 3213880 | <span style="color:#dc2626">-27.38%</span> |
| 967 | [00648 JOIN_SUBQUERY_EXISTS_041](crates/bench/sqlite_parity/cases/SQLITE_PARITY_648_JOIN_SUBQUERY_EXISTS_041.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2855592 | 3629096 | <span style="color:#dc2626">-27.09%</span> |
| 968 | [00137 DOT_VERSION](crates/bench/sqlite_parity/cases/SQLITE_PARITY_137_DOT_VERSION.rs) | P2 | memory | CLI_DOT_COMMAND_DIAGNOSTIC | 1721657 | 2168453 | <span style="color:#dc2626">-25.95%</span> |
| 969 | [00395 DML_WHERE_ORDER_LIMIT_008](crates/bench/sqlite_parity/cases/SQLITE_PARITY_395_DML_WHERE_ORDER_LIMIT_008.rs) | P1 | memory | GEN_SQL_DML | 2617501 | 3295394 | <span style="color:#dc2626">-25.90%</span> |
| 970 | [00654 JOIN_SUBQUERY_EXISTS_047](crates/bench/sqlite_parity/cases/SQLITE_PARITY_654_JOIN_SUBQUERY_EXISTS_047.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2696241 | 3363994 | <span style="color:#dc2626">-24.77%</span> |
| 971 | [00781 CTE_RECURSIVE_MATRIX_074](crates/bench/sqlite_parity/cases/SQLITE_PARITY_781_CTE_RECURSIVE_MATRIX_074.rs) | P1 | memory | GEN_SQL_CTE | 2602103 | 3246111 | <span style="color:#dc2626">-24.75%</span> |
| 972 | [00791 WINDOW_PARTITION_SUM_004](crates/bench/sqlite_parity/cases/SQLITE_PARITY_791_WINDOW_PARTITION_SUM_004.rs) | P2 | memory | GEN_SQL_WINDOW | 2776652 | 3462790 | <span style="color:#dc2626">-24.71%</span> |
| 973 | [00611 JOIN_SUBQUERY_EXISTS_004](crates/bench/sqlite_parity/cases/SQLITE_PARITY_611_JOIN_SUBQUERY_EXISTS_004.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2765892 | 3447963 | <span style="color:#dc2626">-24.66%</span> |
| 974 | [00114 DOT_PRINT](crates/bench/sqlite_parity/cases/SQLITE_PARITY_114_DOT_PRINT.rs) | P0 | memory | CLI_DOT_COMMAND | 1760641 | 2188941 | <span style="color:#dc2626">-24.33%</span> |
| 975 | [00537 AGG_GROUP_HAVING_030](crates/bench/sqlite_parity/cases/SQLITE_PARITY_537_AGG_GROUP_HAVING_030.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2611010 | 3244929 | <span style="color:#dc2626">-24.28%</span> |
| 976 | [01124 INDEX_SCHEMA_PRAGMA_057](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1124_INDEX_SCHEMA_PRAGMA_057.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 2774639 | 3446119 | <span style="color:#dc2626">-24.20%</span> |
| 977 | [00997 VIEW_TRIGGER_GENERATED_050](crates/bench/sqlite_parity/cases/SQLITE_PARITY_997_VIEW_TRIGGER_GENERATED_050.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2667296 | 3308950 | <span style="color:#dc2626">-24.06%</span> |
| 978 | [00344 SCALAR_ARITH_030](crates/bench/sqlite_parity/cases/SQLITE_PARITY_344_SCALAR_ARITH_030.rs) | P1 | memory | GEN_SQL_SCALAR | 2299030 | 2852066 | <span style="color:#dc2626">-24.06%</span> |
| 979 | [00336 SCALAR_ARITH_028](crates/bench/sqlite_parity/cases/SQLITE_PARITY_336_SCALAR_ARITH_028.rs) | P1 | memory | GEN_SQL_SCALAR | 2218988 | 2750092 | <span style="color:#dc2626">-23.93%</span> |
| 980 | [00748 CTE_RECURSIVE_MATRIX_041](crates/bench/sqlite_parity/cases/SQLITE_PARITY_748_CTE_RECURSIVE_MATRIX_041.rs) | P1 | memory | GEN_SQL_CTE | 2658359 | 3292859 | <span style="color:#dc2626">-23.87%</span> |
| 981 | [00821 WINDOW_PARTITION_SUM_034](crates/bench/sqlite_parity/cases/SQLITE_PARITY_821_WINDOW_PARTITION_SUM_034.rs) | P2 | memory | GEN_SQL_WINDOW | 2360826 | 2922168 | <span style="color:#dc2626">-23.78%</span> |
| 982 | [00763 CTE_RECURSIVE_MATRIX_056](crates/bench/sqlite_parity/cases/SQLITE_PARITY_763_CTE_RECURSIVE_MATRIX_056.rs) | P1 | memory | GEN_SQL_CTE | 2690941 | 3325662 | <span style="color:#dc2626">-23.59%</span> |
| 983 | [00439 DML_WHERE_ORDER_LIMIT_052](crates/bench/sqlite_parity/cases/SQLITE_PARITY_439_DML_WHERE_ORDER_LIMIT_052.rs) | P1 | memory | GEN_SQL_DML | 2752578 | 3387298 | <span style="color:#dc2626">-23.06%</span> |
| 984 | [00392 DML_WHERE_ORDER_LIMIT_005](crates/bench/sqlite_parity/cases/SQLITE_PARITY_392_DML_WHERE_ORDER_LIMIT_005.rs) | P1 | memory | GEN_SQL_DML | 2937548 | 3614328 | <span style="color:#dc2626">-23.04%</span> |
| 985 | [00752 CTE_RECURSIVE_MATRIX_045](crates/bench/sqlite_parity/cases/SQLITE_PARITY_752_CTE_RECURSIVE_MATRIX_045.rs) | P1 | memory | GEN_SQL_CTE | 3163014 | 3867487 | <span style="color:#dc2626">-22.27%</span> |
| 986 | [00623 JOIN_SUBQUERY_EXISTS_016](crates/bench/sqlite_parity/cases/SQLITE_PARITY_623_JOIN_SUBQUERY_EXISTS_016.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2708755 | 3308358 | <span style="color:#dc2626">-22.14%</span> |
| 987 | [00835 WINDOW_PARTITION_SUM_048](crates/bench/sqlite_parity/cases/SQLITE_PARITY_835_WINDOW_PARTITION_SUM_048.rs) | P2 | memory | GEN_SQL_WINDOW | 2729394 | 3329910 | <span style="color:#dc2626">-22.00%</span> |
| 988 | [00529 AGG_GROUP_HAVING_022](crates/bench/sqlite_parity/cases/SQLITE_PARITY_529_AGG_GROUP_HAVING_022.rs) | P1 | memory | GEN_SQL_AGGREGATE | 3034741 | 3696513 | <span style="color:#dc2626">-21.81%</span> |
| 989 | [00309 SCALAR_CAST_TYPEOF_021](crates/bench/sqlite_parity/cases/SQLITE_PARITY_309_SCALAR_CAST_TYPEOF_021.rs) | P1 | memory | GEN_SQL_SCALAR | 2542820 | 3092771 | <span style="color:#dc2626">-21.63%</span> |
| 990 | [00225 OPT_NONCE_SAFE_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_225_OPT_NONCE_SAFE_MODE.rs) | P2 | memory | CLI_OPTION | 2180455 | 2643250 | <span style="color:#dc2626">-21.22%</span> |
| 991 | [00109 DOT_MODE_JSON](crates/bench/sqlite_parity/cases/SQLITE_PARITY_109_DOT_MODE_JSON.rs) | P0 | memory | CLI_DOT_COMMAND | 2201936 | 2661114 | <span style="color:#dc2626">-20.85%</span> |
| 992 | [00365 SCALAR_CAST_TYPEOF_035](crates/bench/sqlite_parity/cases/SQLITE_PARITY_365_SCALAR_CAST_TYPEOF_035.rs) | P1 | memory | GEN_SQL_SCALAR | 2398448 | 2889847 | <span style="color:#dc2626">-20.49%</span> |
| 993 | [01065 JSON_EXTRACT_SET_058](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1065_JSON_EXTRACT_SET_058.rs) | P2 | memory | GEN_SQL_JSON | 2661465 | 3203791 | <span style="color:#dc2626">-20.38%</span> |
| 994 | [00693 JOIN_SUBQUERY_EXISTS_086](crates/bench/sqlite_parity/cases/SQLITE_PARITY_693_JOIN_SUBQUERY_EXISTS_086.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3347774 | 4027799 | <span style="color:#dc2626">-20.31%</span> |
| 995 | [00975 VIEW_TRIGGER_GENERATED_028](crates/bench/sqlite_parity/cases/SQLITE_PARITY_975_VIEW_TRIGGER_GENERATED_028.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2901559 | 3477699 | <span style="color:#dc2626">-19.86%</span> |
| 996 | [01041 JSON_EXTRACT_SET_034](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1041_JSON_EXTRACT_SET_034.rs) | P2 | memory | GEN_SQL_JSON | 2707151 | 3235481 | <span style="color:#dc2626">-19.52%</span> |
| 997 | [01025 JSON_EXTRACT_SET_018](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1025_JSON_EXTRACT_SET_018.rs) | P2 | memory | GEN_SQL_JSON | 2607803 | 3112619 | <span style="color:#dc2626">-19.36%</span> |
| 998 | [00364 SCALAR_ARITH_035](crates/bench/sqlite_parity/cases/SQLITE_PARITY_364_SCALAR_ARITH_035.rs) | P1 | memory | GEN_SQL_SCALAR | 2284613 | 2725886 | <span style="color:#dc2626">-19.31%</span> |
| 999 | [00490 DML_WHERE_ORDER_LIMIT_103](crates/bench/sqlite_parity/cases/SQLITE_PARITY_490_DML_WHERE_ORDER_LIMIT_103.rs) | P1 | memory | GEN_SQL_DML | 2871572 | 3425220 | <span style="color:#dc2626">-19.28%</span> |
| 1000 | [01015 JSON_EXTRACT_SET_008](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1015_JSON_EXTRACT_SET_008.rs) | P2 | memory | GEN_SQL_JSON | 2716489 | 3237675 | <span style="color:#dc2626">-19.19%</span> |
| 1001 | [00477 DML_WHERE_ORDER_LIMIT_090](crates/bench/sqlite_parity/cases/SQLITE_PARITY_477_DML_WHERE_ORDER_LIMIT_090.rs) | P1 | memory | GEN_SQL_DML | 2629404 | 3130012 | <span style="color:#dc2626">-19.04%</span> |
| 1002 | [00052 PRAGMA_INTEGRITY_QUICK_CHECK](crates/bench/sqlite_parity/cases/SQLITE_PARITY_052_PRAGMA_INTEGRITY_QUICK_CHECK.rs) | P0 | memory | SQL_PRAGMA | 2845924 | 3379062 | <span style="color:#dc2626">-18.73%</span> |
| 1003 | [00699 JOIN_SUBQUERY_EXISTS_092](crates/bench/sqlite_parity/cases/SQLITE_PARITY_699_JOIN_SUBQUERY_EXISTS_092.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3056382 | 3613165 | <span style="color:#dc2626">-18.22%</span> |
| 1004 | [00142 DOT_EXIT_CODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_142_DOT_EXIT_CODE.rs) | P0 | memory | CLI_DOT_COMMAND | 1605296 | 1894654 | <span style="color:#dc2626">-18.03%</span> |
| 1005 | [00116 DOT_DATABASES](crates/bench/sqlite_parity/cases/SQLITE_PARITY_116_DOT_DATABASES.rs) | P0 | memory | CLI_DOT_COMMAND | 1830453 | 2156199 | <span style="color:#dc2626">-17.80%</span> |
| 1006 | [00860 WINDOW_PARTITION_SUM_073](crates/bench/sqlite_parity/cases/SQLITE_PARITY_860_WINDOW_PARTITION_SUM_073.rs) | P2 | memory | GEN_SQL_WINDOW | 2757817 | 3245741 | <span style="color:#dc2626">-17.69%</span> |
| 1007 | [00761 CTE_RECURSIVE_MATRIX_054](crates/bench/sqlite_parity/cases/SQLITE_PARITY_761_CTE_RECURSIVE_MATRIX_054.rs) | P1 | memory | GEN_SQL_CTE | 2619996 | 3082271 | <span style="color:#dc2626">-17.64%</span> |
| 1008 | [00940 CONSTRAINT_FK_SAVEPOINT_073](crates/bench/sqlite_parity/cases/SQLITE_PARITY_940_CONSTRAINT_FK_SAVEPOINT_073.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2518414 | 2961252 | <span style="color:#dc2626">-17.58%</span> |
| 1009 | [00632 JOIN_SUBQUERY_EXISTS_025](crates/bench/sqlite_parity/cases/SQLITE_PARITY_632_JOIN_SUBQUERY_EXISTS_025.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2958436 | 3472730 | <span style="color:#dc2626">-17.38%</span> |
| 1010 | [00721 CTE_RECURSIVE_MATRIX_014](crates/bench/sqlite_parity/cases/SQLITE_PARITY_721_CTE_RECURSIVE_MATRIX_014.rs) | P1 | memory | GEN_SQL_CTE | 2770491 | 3238416 | <span style="color:#dc2626">-16.89%</span> |
| 1011 | [00085 CORE_RANDOM_SHAPE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_085_CORE_RANDOM_SHAPE.rs) | P0 | memory | SQL_FUNCTIONS | 2630687 | 3074756 | <span style="color:#dc2626">-16.88%</span> |
| 1012 | [00153 DOT_CD_TEMPFILE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_153_DOT_CD_TEMPFILE.rs) | P2 | tempfile | CLI_TEMPFILE | 2127444 | 2480413 | <span style="color:#dc2626">-16.59%</span> |
| 1013 | [01108 INDEX_SCHEMA_PRAGMA_041](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1108_INDEX_SCHEMA_PRAGMA_041.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 2965690 | 3456038 | <span style="color:#dc2626">-16.53%</span> |
| 1014 | [00143 DOT_QUIT](crates/bench/sqlite_parity/cases/SQLITE_PARITY_143_DOT_QUIT.rs) | P0 | memory | CLI_DOT_COMMAND | 1619343 | 1884245 | <span style="color:#dc2626">-16.36%</span> |
| 1015 | [00843 WINDOW_PARTITION_SUM_056](crates/bench/sqlite_parity/cases/SQLITE_PARITY_843_WINDOW_PARTITION_SUM_056.rs) | P2 | memory | GEN_SQL_WINDOW | 2744823 | 3184815 | <span style="color:#dc2626">-16.03%</span> |
| 1016 | [00615 JOIN_SUBQUERY_EXISTS_008](crates/bench/sqlite_parity/cases/SQLITE_PARITY_615_JOIN_SUBQUERY_EXISTS_008.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2943178 | 3412015 | <span style="color:#dc2626">-15.93%</span> |
| 1017 | [00435 DML_WHERE_ORDER_LIMIT_048](crates/bench/sqlite_parity/cases/SQLITE_PARITY_435_DML_WHERE_ORDER_LIMIT_048.rs) | P1 | memory | GEN_SQL_DML | 2918832 | 3378822 | <span style="color:#dc2626">-15.76%</span> |
| 1018 | [00587 AGG_GROUP_HAVING_080](crates/bench/sqlite_parity/cases/SQLITE_PARITY_587_AGG_GROUP_HAVING_080.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2866984 | 3318679 | <span style="color:#dc2626">-15.76%</span> |
| 1019 | [01040 JSON_EXTRACT_SET_033](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1040_JSON_EXTRACT_SET_033.rs) | P2 | memory | GEN_SQL_JSON | 2797573 | 3231563 | <span style="color:#dc2626">-15.51%</span> |
| 1020 | [00852 WINDOW_PARTITION_SUM_065](crates/bench/sqlite_parity/cases/SQLITE_PARITY_852_WINDOW_PARTITION_SUM_065.rs) | P2 | memory | GEN_SQL_WINDOW | 2836577 | 3273423 | <span style="color:#dc2626">-15.40%</span> |
| 1021 | [01013 JSON_EXTRACT_SET_006](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1013_JSON_EXTRACT_SET_006.rs) | P2 | memory | GEN_SQL_JSON | 2589569 | 2970058 | <span style="color:#dc2626">-14.69%</span> |
| 1022 | [01123 INDEX_SCHEMA_PRAGMA_056](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1123_INDEX_SCHEMA_PRAGMA_056.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 3012879 | 3450748 | <span style="color:#dc2626">-14.53%</span> |
| 1023 | [00831 WINDOW_PARTITION_SUM_044](crates/bench/sqlite_parity/cases/SQLITE_PARITY_831_WINDOW_PARTITION_SUM_044.rs) | P2 | memory | GEN_SQL_WINDOW | 2804555 | 3203420 | <span style="color:#dc2626">-14.22%</span> |
| 1024 | [01043 JSON_EXTRACT_SET_036](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1043_JSON_EXTRACT_SET_036.rs) | P2 | memory | GEN_SQL_JSON | 3224420 | 3651247 | <span style="color:#dc2626">-13.24%</span> |
| 1025 | [00933 CONSTRAINT_FK_SAVEPOINT_066](crates/bench/sqlite_parity/cases/SQLITE_PARITY_933_CONSTRAINT_FK_SAVEPOINT_066.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2940913 | 3323447 | <span style="color:#dc2626">-13.01%</span> |
| 1026 | [00110 DOT_MODE_LINE_COLUMN_TABLE_BOX_MARKDOWN](crates/bench/sqlite_parity/cases/SQLITE_PARITY_110_DOT_MODE_LINE_COLUMN_TABLE_BOX_MARKDOWN.rs) | P0 | memory | CLI_DOT_COMMAND | 2493166 | 2781943 | <span style="color:#dc2626">-11.58%</span> |
| 1027 | [01060 JSON_EXTRACT_SET_053](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1060_JSON_EXTRACT_SET_053.rs) | P2 | memory | GEN_SQL_JSON | 2735254 | 3051894 | <span style="color:#dc2626">-11.58%</span> |
| 1028 | [01103 INDEX_SCHEMA_PRAGMA_036](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1103_INDEX_SCHEMA_PRAGMA_036.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 3123910 | 3477199 | <span style="color:#dc2626">-11.31%</span> |
| 1029 | [00187 OPT_NULLVALUE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_187_OPT_NULLVALUE.rs) | P1 | memory | CLI_OPTION | 2503346 | 2774680 | <span style="color:#dc2626">-10.84%</span> |
| 1030 | [00637 JOIN_SUBQUERY_EXISTS_030](crates/bench/sqlite_parity/cases/SQLITE_PARITY_637_JOIN_SUBQUERY_EXISTS_030.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3035573 | 3340489 | <span style="color:#dc2626">-10.04%</span> |
| 1031 | [00345 SCALAR_CAST_TYPEOF_030](crates/bench/sqlite_parity/cases/SQLITE_PARITY_345_SCALAR_CAST_TYPEOF_030.rs) | P1 | memory | GEN_SQL_SCALAR | 2643501 | 2849741 | <span style="color:#dc2626">-7.80%</span> |
| 1032 | [00984 VIEW_TRIGGER_GENERATED_037](crates/bench/sqlite_parity/cases/SQLITE_PARITY_984_VIEW_TRIGGER_GENERATED_037.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 3065129 | 3299812 | <span style="color:#dc2626">-7.66%</span> |
| 1033 | [00330 SCALAR_STRING_026](crates/bench/sqlite_parity/cases/SQLITE_PARITY_330_SCALAR_STRING_026.rs) | P1 | memory | GEN_SQL_SCALAR | 2705799 | 2901128 | <span style="color:#dc2626">-7.22%</span> |
| 1034 | [00047 PRAGMA_FOREIGN_KEYS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_047_PRAGMA_FOREIGN_KEYS.rs) | P0 | memory | SQL_PRAGMA | 2493768 | 2652628 | <span style="color:#dc2626">-6.37%</span> |
| 1035 | [00943 CONSTRAINT_FK_SAVEPOINT_076](crates/bench/sqlite_parity/cases/SQLITE_PARITY_943_CONSTRAINT_FK_SAVEPOINT_076.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 3016937 | 3196788 | <span style="color:#dc2626">-5.96%</span> |
| 1036 | [00005 UNIQUE_CONSTRAINT_FAILURE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_005_UNIQUE_CONSTRAINT_FAILURE.rs) | P0 | memory | SQL_CONSTRAINTS_NEGATIVE | 3085667 | 3230752 | <span style="color:#f97316">-4.70%</span> |
| 1037 | [00659 JOIN_SUBQUERY_EXISTS_052](crates/bench/sqlite_parity/cases/SQLITE_PARITY_659_JOIN_SUBQUERY_EXISTS_052.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3263093 | 3349897 | <span style="color:#f97316">-2.66%</span> |
| 1038 | [00051 PRAGMA_INDEX_LIST_FUNCTION](crates/bench/sqlite_parity/cases/SQLITE_PARITY_051_PRAGMA_INDEX_LIST_FUNCTION.rs) | P0 | memory | SQL_PRAGMA | 2932428 | 3005486 | <span style="color:#f97316">-2.49%</span> |
| 1039 | [00024 SAVEPOINT_ROLLBACK_RELEASE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_024_SAVEPOINT_ROLLBACK_RELEASE.rs) | P0 | memory | SQL_SAVEPOINT | 2879196 | 2893975 | <span style="color:#f97316">-0.51%</span> |
| 1040 | [00621 JOIN_SUBQUERY_EXISTS_014](crates/bench/sqlite_parity/cases/SQLITE_PARITY_621_JOIN_SUBQUERY_EXISTS_014.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3365217 | 3361109 | <span style="color:#6b7280">0.12%</span> |
| 1041 | [00296 SCALAR_ARITH_018](crates/bench/sqlite_parity/cases/SQLITE_PARITY_296_SCALAR_ARITH_018.rs) | P1 | memory | GEN_SQL_SCALAR | 2678607 | 2665343 | <span style="color:#6b7280">0.50%</span> |
| 1042 | [00717 CTE_RECURSIVE_MATRIX_010](crates/bench/sqlite_parity/cases/SQLITE_PARITY_717_CTE_RECURSIVE_MATRIX_010.rs) | P1 | memory | GEN_SQL_CTE | 3239509 | 3200384 | <span style="color:#16a34a">1.21%</span> |
| 1043 | [01011 JSON_EXTRACT_SET_004](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1011_JSON_EXTRACT_SET_004.rs) | P2 | memory | GEN_SQL_JSON | 3222396 | 3113270 | <span style="color:#16a34a">3.39%</span> |
| 1044 | [00170 OPT_VERSION](crates/bench/sqlite_parity/cases/SQLITE_PARITY_170_OPT_VERSION.rs) | P1 | memory | CLI_OPTION | 1696139 | 1611609 | <span style="color:#16a34a">4.98%</span> |
| 1045 | [00522 AGG_GROUP_HAVING_015](crates/bench/sqlite_parity/cases/SQLITE_PARITY_522_AGG_GROUP_HAVING_015.rs) | P1 | memory | GEN_SQL_AGGREGATE | 3342092 | 3085968 | <span style="color:#2563eb">7.66%</span> |
| 1046 | [00049 PRAGMA_TEMP_STORE_MEMORY](crates/bench/sqlite_parity/cases/SQLITE_PARITY_049_PRAGMA_TEMP_STORE_MEMORY.rs) | P0 | memory | SQL_PRAGMA | 2682846 | 2460205 | <span style="color:#2563eb">8.30%</span> |
| 1047 | [00189 OPT_ECHO](crates/bench/sqlite_parity/cases/SQLITE_PARITY_189_OPT_ECHO.rs) | P2 | memory | CLI_OPTION | 2968516 | 2620949 | <span style="color:#2563eb">11.71%</span> |
| 1048 | [00030 ALTER_TABLE_ADD_DROP_COLUMN](crates/bench/sqlite_parity/cases/SQLITE_PARITY_030_ALTER_TABLE_ADD_DROP_COLUMN.rs) | P0 | memory | SQL_ALTER | 3178914 | 2693385 | <span style="color:#2563eb">15.27%</span> |
| 1049 | [00106 DOT_HELP](crates/bench/sqlite_parity/cases/SQLITE_PARITY_106_DOT_HELP.rs) | P0 | memory | CLI_DOT_COMMAND | 2800578 | 2223366 | <span style="color:#2563eb">20.61%</span> |

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
