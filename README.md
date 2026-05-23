<p align="center">
  <img src="assets/redlinedb-banner.png" alt="RedlineDB" width="100%">
</p>

<h1 align="center">RedlineDB</h1>

<p align="center">
  <em>Rust-native embedded SQL with SQLite-shaped compatibility, concurrent writes, and deterministic recovery.</em>
</p>

<p align="center">
  <a href="#sqlite-parity-status"><img src="https://img.shields.io/badge/full%20corpus-1127%2F1127-brightgreen" alt="full corpus parity"></a>
  <a href="#sqlite-parity-status"><img src="https://img.shields.io/badge/generated%20cases-1127-blue" alt="generated cases"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-Apache--2.0-blue" alt="license"></a>
  <a href="rust-toolchain.toml"><img src="https://img.shields.io/badge/rust-1.95-orange" alt="rust"></a>
  <img src="https://img.shields.io/badge/version-2.0.2-blue" alt="version">
  <!-- jankurai-score-badge:begin -->
  <a href=".jankurai/repo-score.md"><img src="https://img.shields.io/badge/jankurai-87%2F100%20advisory-green" alt="jankurai score: 87/100 advisory"></a>
  <!-- jankurai-score-badge:end -->
</p>

RedlineDB is an embedded SQL engine written in Rust. It keeps the SQLite-facing
API familiar while replacing the storage core with MVCC, a concurrent B-tree,
group-commit WAL, and crash recovery designed for multi-writer workloads.

## Engine Metrics

<!-- sqlite-parity-metrics:begin -->

![Beyond-SQLite feature progress chart](assets/beyond-sqlite-feature-progress.svg)

![SQLite vs RedlineDB production KSLOC chart](assets/sqlite-parity-ksloc.svg)

![RedlineDB vs SQLite Jankurai score chart](assets/sqlite-jankurai-score.svg)

![RedlineDB vs SQLite code shape score chart](assets/sqlite-code-shape.svg)

![RedlineDB vs SQLite median test performance chart](assets/sqlite-median-test-performance.svg)

<!-- sqlite-parity-metrics:end -->

## Redline Mission

RedlineDB keeps SQLite-shaped compatibility where that contract is valuable:
small embedded deployments, familiar SQL, a direct Rust API, and a SQLite-shaped
C surface for integrations that already expect it.

The engine is not a SQLite wrapper. It rebuilds the storage core in Rust so
MVCC, concurrent writes, WAL behavior, and recovery can be owned directly
instead of treated as constraints inherited from a single-writer file engine.

The codebase is also shaped for fast repair by agents and humans: smaller
modules, local invariants, routed proof lanes, generated evidence, and audit
metadata that point a fix at the narrowest lawful surface.

## At a Glance

| Area | What it is |
|---|---|
| Rust API | `redlinedb` for embedded use |
| CLI | `redlinedb-cli` for shell-style workflows |
| FFI | `crates/ffi` exports a SQLite-shaped C ABI surface |
| SQL engine | Parser, planner, executor, pragmas, and compatibility shims |
| Storage | Kernel-owned MVCC, WAL, catalog, and recovery layers |
| Default proof lane | `just fast` |
| SQLite parity gate | `just sqlite-parity-scale-ci` |

## Install

### Rust library

Pin the release in `Cargo.toml`:

```toml
[dependencies]
redlinedb = "=2.0.2"
```

For libraries, `redlinedb = "1"` is usually fine. For binaries, keep the exact
pin and commit `Cargo.lock`.

### CLI binary

Install the published shell on Linux or macOS:

```bash
curl -LsSf https://raw.githubusercontent.com/neverhuman/RedlineDB/main/scripts/install.sh | bash
```

Pin a specific release when you need reproducible installs:

```bash
curl -LsSf https://raw.githubusercontent.com/neverhuman/RedlineDB/main/scripts/install.sh | VERSION=v2.0.2 bash
```

Lock the exact tarball digest in CI or release automation:

```bash
curl -LsSf https://raw.githubusercontent.com/neverhuman/RedlineDB/main/scripts/install.sh | \
  VERSION=v2.0.2 REDLINEDB_SHA256=<sha256> bash
```

### Build from source

```bash
cargo install redlinedb-cli --version 2.0.2 --locked
```

Or install from the tagged repository release:

```bash
cargo install --git https://github.com/neverhuman/RedlineDB.git --tag v2.0.2 --package redlinedb-cli --locked
```

### Direct download

Release tarballs are published on the [releases page](https://github.com/neverhuman/RedlineDB/releases):

| Platform | File |
|---|---|
| Linux x86_64 | `redlinedb-v2.0.2-linux-x86_64.tar.gz` |
| macOS Apple Silicon | `redlinedb-v2.0.2-macos-arm64.tar.gz` |
| macOS Intel | `redlinedb-v2.0.2-macos-x86_64.tar.gz` |

Each tarball ships with a matching `.sha256` checksum and contains the CLI,
shared libraries, and public headers.

## Quick Start

### Embedded use

```rust
use redlinedb::Database;

fn main() -> redlinedb::Result<()> {
    let db = Database::create("/tmp/demo.redline")?;
    let mut conn = db.connect()?;

    conn.execute("CREATE TABLE kv(k INTEGER PRIMARY KEY, v TEXT NOT NULL)", ())?;
    conn.execute("INSERT INTO kv VALUES (1, 'hello')", ())?;

    let value: String = conn.query_row("SELECT v FROM kv WHERE k = 1", ())?;
    println!("{value}");

    Ok(())
}
```

### CLI use

```bash
rtk cargo run -p redlinedb-cli --release -- exec /tmp/demo.redline "SELECT count(*) FROM kv"
rtk cargo run -p redlinedb-cli --release -- stats /tmp/demo.redline --json
rtk cargo run -p redlinedb-cli --release -- backup /tmp/demo.redline /tmp/demo.bak --physical
```

### Default verification

```bash
rtk just fast
rtk just sqlite-parity-scale-ci
rtk just sqlite-parity-report-check
```

## SQLite Parity Status

The CI parity lane targets the full generated corpus. Missing, skipped, failed,
or unmeasured cases are hard report-check failures rather than excluded from the
denominator. The live report below is generated from
`benchmark-results/sqlite-parity/latest/` using the same full-corpus selector as
`just sqlite-parity-scale-ci`.

<!-- sqlite-parity-report:begin -->

**SQLite parity coverage:** **1127 / 1127 = 100.0%** full generated cases passed in CI. Failed: **0**. Missing: **0**. Skipped: **0**. Updated 2026-05-23.

**SQLite parity latency:** median gap **-9.79%**, worst gap **-64.19%**, faster cases **273** with a **3000000 ns** reference floor (targets: median >= -25%, worst > -120%, faster >= 25).

![SQLite parity latency improvement plot](assets/sqlite-parity-latency-gap.svg)

![SQLite parity performance distribution](assets/sqlite-parity-performance-histogram.svg)

[Full ranked latency table](#sqlite-parity-ranked-latency-table) is collapsed below for README readability.

<details id="sqlite-parity-ranked-latency-table">
<summary>Full ranked latency table</summary>

| Rank | Case | Priority | Profile | Category | SQLite median ns | RedlineDB median ns | Improvement |
| ---: | --- | --- | --- | --- | ---: | ---: | ---: |
| 1 | [00211 SQL_ATTACH_TEMPFILE_DATABASE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_211_SQL_ATTACH_TEMPFILE_DATABASE.rs) | P1 | tempfile | SQL_TEMPFILE | 1903441 | 4925668 | <span style="color:#dc2626">-64.19%</span> |
| 2 | [00193 OPT_READONLY_TEMPFILE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_193_OPT_READONLY_TEMPFILE.rs) | P2 | tempfile | CLI_OPTION_TEMPFILE | 7335567 | 11817946 | <span style="color:#dc2626">-61.10%</span> |
| 3 | [00212 SQL_VACUUM_INTO_TEMPFILE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_212_SQL_VACUUM_INTO_TEMPFILE.rs) | P2 | tempfile | SQL_TEMPFILE | 2531359 | 4750166 | <span style="color:#dc2626">-58.34%</span> |
| 4 | [00150 DOT_BACKUP_RESTORE_TEMPFILE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_150_DOT_BACKUP_RESTORE_TEMPFILE.rs) | P1 | tempfile | CLI_TEMPFILE | 2445075 | 4533967 | <span style="color:#dc2626">-51.13%</span> |
| 5 | [00151 DOT_SAVE_RESTORE_TEMPFILE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_151_DOT_SAVE_RESTORE_TEMPFILE.rs) | P1 | tempfile | CLI_TEMPFILE | 2010803 | 4520873 | <span style="color:#dc2626">-50.70%</span> |
| 6 | [00391 DML_WHERE_ORDER_LIMIT_004](crates/bench/sqlite_parity/cases/SQLITE_PARITY_391_DML_WHERE_ORDER_LIMIT_004.rs) | P1 | memory | GEN_SQL_DML | 3016236 | 4529839 | <span style="color:#dc2626">-50.18%</span> |
| 7 | [00404 DML_WHERE_ORDER_LIMIT_017](crates/bench/sqlite_parity/cases/SQLITE_PARITY_404_DML_WHERE_ORDER_LIMIT_017.rs) | P1 | memory | GEN_SQL_DML | 1655061 | 4489443 | <span style="color:#dc2626">-49.65%</span> |
| 8 | [00390 DML_WHERE_ORDER_LIMIT_003](crates/bench/sqlite_parity/cases/SQLITE_PARITY_390_DML_WHERE_ORDER_LIMIT_003.rs) | P1 | memory | GEN_SQL_DML | 1663356 | 4463554 | <span style="color:#dc2626">-48.79%</span> |
| 9 | [00678 JOIN_SUBQUERY_EXISTS_071](crates/bench/sqlite_parity/cases/SQLITE_PARITY_678_JOIN_SUBQUERY_EXISTS_071.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2187057 | 4441151 | <span style="color:#dc2626">-48.04%</span> |
| 10 | [00477 DML_WHERE_ORDER_LIMIT_090](crates/bench/sqlite_parity/cases/SQLITE_PARITY_477_DML_WHERE_ORDER_LIMIT_090.rs) | P1 | memory | GEN_SQL_DML | 1679247 | 4438948 | <span style="color:#dc2626">-47.96%</span> |
| 11 | [00684 JOIN_SUBQUERY_EXISTS_077](crates/bench/sqlite_parity/cases/SQLITE_PARITY_684_JOIN_SUBQUERY_EXISTS_077.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1704835 | 4317207 | <span style="color:#dc2626">-43.91%</span> |
| 12 | [00959 VIEW_TRIGGER_GENERATED_012](crates/bench/sqlite_parity/cases/SQLITE_PARITY_959_VIEW_TRIGGER_GENERATED_012.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1759609 | 4315634 | <span style="color:#dc2626">-43.85%</span> |
| 13 | [01001 VIEW_TRIGGER_GENERATED_054](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1001_VIEW_TRIGGER_GENERATED_054.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1946533 | 4298451 | <span style="color:#dc2626">-43.28%</span> |
| 14 | [00835 WINDOW_PARTITION_SUM_048](crates/bench/sqlite_parity/cases/SQLITE_PARITY_835_WINDOW_PARTITION_SUM_048.rs) | P2 | memory | GEN_SQL_WINDOW | 1629713 | 4296157 | <span style="color:#dc2626">-43.21%</span> |
| 15 | [00147 DOT_IMPORT_CSV_TEMPFILE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_147_DOT_IMPORT_CSV_TEMPFILE.rs) | P1 | tempfile | CLI_TEMPFILE | 1811697 | 4272622 | <span style="color:#dc2626">-42.42%</span> |
| 16 | [00939 CONSTRAINT_FK_SAVEPOINT_072](crates/bench/sqlite_parity/cases/SQLITE_PARITY_939_CONSTRAINT_FK_SAVEPOINT_072.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1694606 | 4266621 | <span style="color:#dc2626">-42.22%</span> |
| 17 | [01048 JSON_EXTRACT_SET_041](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1048_JSON_EXTRACT_SET_041.rs) | P2 | memory | GEN_SQL_JSON | 1657085 | 4252204 | <span style="color:#dc2626">-41.74%</span> |
| 18 | [00887 CONSTRAINT_FK_SAVEPOINT_020](crates/bench/sqlite_parity/cases/SQLITE_PARITY_887_CONSTRAINT_FK_SAVEPOINT_020.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1664138 | 4230763 | <span style="color:#dc2626">-41.03%</span> |
| 19 | [00522 AGG_GROUP_HAVING_015](crates/bench/sqlite_parity/cases/SQLITE_PARITY_522_AGG_GROUP_HAVING_015.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1857835 | 4223430 | <span style="color:#dc2626">-40.78%</span> |
| 20 | [00978 VIEW_TRIGGER_GENERATED_031](crates/bench/sqlite_parity/cases/SQLITE_PARITY_978_VIEW_TRIGGER_GENERATED_031.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2063173 | 4219533 | <span style="color:#dc2626">-40.65%</span> |
| 21 | [01003 VIEW_TRIGGER_GENERATED_056](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1003_VIEW_TRIGGER_GENERATED_056.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1830733 | 4216797 | <span style="color:#dc2626">-40.56%</span> |
| 22 | [00622 JOIN_SUBQUERY_EXISTS_015](crates/bench/sqlite_parity/cases/SQLITE_PARITY_622_JOIN_SUBQUERY_EXISTS_015.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1797730 | 4215976 | <span style="color:#dc2626">-40.53%</span> |
| 23 | [00152 DOT_CLONE_TEMPFILE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_152_DOT_CLONE_TEMPFILE.rs) | P2 | tempfile | CLI_TEMPFILE | 2264754 | 4205546 | <span style="color:#dc2626">-40.18%</span> |
| 24 | [00447 DML_WHERE_ORDER_LIMIT_060](crates/bench/sqlite_parity/cases/SQLITE_PARITY_447_DML_WHERE_ORDER_LIMIT_060.rs) | P1 | memory | GEN_SQL_DML | 1632008 | 4203332 | <span style="color:#dc2626">-40.11%</span> |
| 25 | [00309 SCALAR_CAST_TYPEOF_021](crates/bench/sqlite_parity/cases/SQLITE_PARITY_309_SCALAR_CAST_TYPEOF_021.rs) | P1 | memory | GEN_SQL_SCALAR | 1797200 | 4195346 | <span style="color:#dc2626">-39.84%</span> |
| 26 | [00883 CONSTRAINT_FK_SAVEPOINT_016](crates/bench/sqlite_parity/cases/SQLITE_PARITY_883_CONSTRAINT_FK_SAVEPOINT_016.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1682302 | 4189315 | <span style="color:#dc2626">-39.64%</span> |
| 27 | [01070 INDEX_SCHEMA_PRAGMA_003](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1070_INDEX_SCHEMA_PRAGMA_003.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1849258 | 4186169 | <span style="color:#dc2626">-39.54%</span> |
| 28 | [00888 CONSTRAINT_FK_SAVEPOINT_021](crates/bench/sqlite_parity/cases/SQLITE_PARITY_888_CONSTRAINT_FK_SAVEPOINT_021.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1669678 | 4172063 | <span style="color:#dc2626">-39.07%</span> |
| 29 | [00503 DML_WHERE_ORDER_LIMIT_116](crates/bench/sqlite_parity/cases/SQLITE_PARITY_503_DML_WHERE_ORDER_LIMIT_116.rs) | P1 | memory | GEN_SQL_DML | 1694275 | 4161833 | <span style="color:#dc2626">-38.73%</span> |
| 30 | [00970 VIEW_TRIGGER_GENERATED_023](crates/bench/sqlite_parity/cases/SQLITE_PARITY_970_VIEW_TRIGGER_GENERATED_023.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2123357 | 4157475 | <span style="color:#dc2626">-38.58%</span> |
| 31 | [00921 CONSTRAINT_FK_SAVEPOINT_054](crates/bench/sqlite_parity/cases/SQLITE_PARITY_921_CONSTRAINT_FK_SAVEPOINT_054.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1719933 | 4151754 | <span style="color:#dc2626">-38.39%</span> |
| 32 | [00156 DOT_RECOVER_TEMPFILE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_156_DOT_RECOVER_TEMPFILE.rs) | P3 | tempfile | CLI_TEMPFILE | 3019212 | 4149480 | <span style="color:#dc2626">-37.44%</span> |
| 33 | [01121 INDEX_SCHEMA_PRAGMA_054](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1121_INDEX_SCHEMA_PRAGMA_054.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1838048 | 4109314 | <span style="color:#dc2626">-36.98%</span> |
| 34 | [01086 INDEX_SCHEMA_PRAGMA_019](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1086_INDEX_SCHEMA_PRAGMA_019.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1788283 | 4095657 | <span style="color:#dc2626">-36.52%</span> |
| 35 | [00427 DML_WHERE_ORDER_LIMIT_040](crates/bench/sqlite_parity/cases/SQLITE_PARITY_427_DML_WHERE_ORDER_LIMIT_040.rs) | P1 | memory | GEN_SQL_DML | 1648719 | 4081020 | <span style="color:#dc2626">-36.03%</span> |
| 36 | [00433 DML_WHERE_ORDER_LIMIT_046](crates/bench/sqlite_parity/cases/SQLITE_PARITY_433_DML_WHERE_ORDER_LIMIT_046.rs) | P1 | memory | GEN_SQL_DML | 1786079 | 4081020 | <span style="color:#dc2626">-36.03%</span> |
| 37 | [00397 DML_WHERE_ORDER_LIMIT_010](crates/bench/sqlite_parity/cases/SQLITE_PARITY_397_DML_WHERE_ORDER_LIMIT_010.rs) | P1 | memory | GEN_SQL_DML | 1829922 | 4075710 | <span style="color:#dc2626">-35.86%</span> |
| 38 | [00512 AGG_GROUP_HAVING_005](crates/bench/sqlite_parity/cases/SQLITE_PARITY_512_AGG_GROUP_HAVING_005.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1794835 | 4068837 | <span style="color:#dc2626">-35.63%</span> |
| 39 | [00903 CONSTRAINT_FK_SAVEPOINT_036](crates/bench/sqlite_parity/cases/SQLITE_PARITY_903_CONSTRAINT_FK_SAVEPOINT_036.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1708993 | 4067875 | <span style="color:#dc2626">-35.60%</span> |
| 40 | [00362 SCALAR_STRING_034](crates/bench/sqlite_parity/cases/SQLITE_PARITY_362_SCALAR_STRING_034.rs) | P1 | memory | GEN_SQL_SCALAR | 1426218 | 4050844 | <span style="color:#dc2626">-35.03%</span> |
| 41 | [00511 AGG_GROUP_HAVING_004](crates/bench/sqlite_parity/cases/SQLITE_PARITY_511_AGG_GROUP_HAVING_004.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1745512 | 4047727 | <span style="color:#dc2626">-34.92%</span> |
| 42 | [00484 DML_WHERE_ORDER_LIMIT_097](crates/bench/sqlite_parity/cases/SQLITE_PARITY_484_DML_WHERE_ORDER_LIMIT_097.rs) | P1 | memory | GEN_SQL_DML | 1661153 | 4015176 | <span style="color:#dc2626">-33.84%</span> |
| 43 | [00931 CONSTRAINT_FK_SAVEPOINT_064](crates/bench/sqlite_parity/cases/SQLITE_PARITY_931_CONSTRAINT_FK_SAVEPOINT_064.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2375364 | 4005277 | <span style="color:#dc2626">-33.51%</span> |
| 44 | [00964 VIEW_TRIGGER_GENERATED_017](crates/bench/sqlite_parity/cases/SQLITE_PARITY_964_VIEW_TRIGGER_GENERATED_017.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1764488 | 4002381 | <span style="color:#dc2626">-33.41%</span> |
| 45 | [00831 WINDOW_PARTITION_SUM_044](crates/bench/sqlite_parity/cases/SQLITE_PARITY_831_WINDOW_PARTITION_SUM_044.rs) | P2 | memory | GEN_SQL_WINDOW | 1709794 | 3997512 | <span style="color:#dc2626">-33.25%</span> |
| 46 | [00919 CONSTRAINT_FK_SAVEPOINT_052](crates/bench/sqlite_parity/cases/SQLITE_PARITY_919_CONSTRAINT_FK_SAVEPOINT_052.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1700326 | 3993184 | <span style="color:#dc2626">-33.11%</span> |
| 47 | [00155 DOT_DBTOTXT_TEMPFILE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_155_DOT_DBTOTXT_TEMPFILE.rs) | P3 | tempfile | CLI_TEMPFILE | 1978873 | 3985148 | <span style="color:#dc2626">-32.84%</span> |
| 48 | [00993 VIEW_TRIGGER_GENERATED_046](crates/bench/sqlite_parity/cases/SQLITE_PARITY_993_VIEW_TRIGGER_GENERATED_046.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1759348 | 3982013 | <span style="color:#dc2626">-32.73%</span> |
| 49 | [00311 SCALAR_NULL_COALESCE_021](crates/bench/sqlite_parity/cases/SQLITE_PARITY_311_SCALAR_NULL_COALESCE_021.rs) | P1 | memory | GEN_SQL_SCALAR | 2602213 | 3977184 | <span style="color:#dc2626">-32.57%</span> |
| 50 | [00956 VIEW_TRIGGER_GENERATED_009](crates/bench/sqlite_parity/cases/SQLITE_PARITY_956_VIEW_TRIGGER_GENERATED_009.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1928809 | 3976623 | <span style="color:#dc2626">-32.55%</span> |
| 51 | [00974 VIEW_TRIGGER_GENERATED_027](crates/bench/sqlite_parity/cases/SQLITE_PARITY_974_VIEW_TRIGGER_GENERATED_027.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1673576 | 3969028 | <span style="color:#dc2626">-32.30%</span> |
| 52 | [00197 OPT_MAXSIZE_DESERIALIZE_TEMPFILE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_197_OPT_MAXSIZE_DESERIALIZE_TEMPFILE.rs) | P3 | tempfile | CLI_OPTION_TEMPFILE | 7005052 | 9248425 | <span style="color:#dc2626">-32.03%</span> |
| 53 | [00987 VIEW_TRIGGER_GENERATED_040](crates/bench/sqlite_parity/cases/SQLITE_PARITY_987_VIEW_TRIGGER_GENERATED_040.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1812709 | 3943520 | <span style="color:#dc2626">-31.45%</span> |
| 54 | [00233 SCALAR_CAST_TYPEOF_002](crates/bench/sqlite_parity/cases/SQLITE_PARITY_233_SCALAR_CAST_TYPEOF_002.rs) | P1 | memory | GEN_SQL_SCALAR | 2131903 | 3939122 | <span style="color:#dc2626">-31.30%</span> |
| 55 | [00801 WINDOW_PARTITION_SUM_014](crates/bench/sqlite_parity/cases/SQLITE_PARITY_801_WINDOW_PARTITION_SUM_014.rs) | P2 | memory | GEN_SQL_WINDOW | 1649370 | 3938961 | <span style="color:#dc2626">-31.30%</span> |
| 56 | [00392 DML_WHERE_ORDER_LIMIT_005](crates/bench/sqlite_parity/cases/SQLITE_PARITY_392_DML_WHERE_ORDER_LIMIT_005.rs) | P1 | memory | GEN_SQL_DML | 2063424 | 3937508 | <span style="color:#dc2626">-31.25%</span> |
| 57 | [00213 SQL_WAL_CHECKPOINT_TEMPFILE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_213_SQL_WAL_CHECKPOINT_TEMPFILE.rs) | P2 | tempfile | SQL_TEMPFILE | 2387397 | 3928031 | <span style="color:#dc2626">-30.93%</span> |
| 58 | [00690 JOIN_SUBQUERY_EXISTS_083](crates/bench/sqlite_parity/cases/SQLITE_PARITY_690_JOIN_SUBQUERY_EXISTS_083.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1757434 | 3923241 | <span style="color:#dc2626">-30.77%</span> |
| 59 | [00629 JOIN_SUBQUERY_EXISTS_022](crates/bench/sqlite_parity/cases/SQLITE_PARITY_629_JOIN_SUBQUERY_EXISTS_022.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1750060 | 3919505 | <span style="color:#dc2626">-30.65%</span> |
| 60 | [01091 INDEX_SCHEMA_PRAGMA_024](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1091_INDEX_SCHEMA_PRAGMA_024.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1787822 | 3918833 | <span style="color:#dc2626">-30.63%</span> |
| 61 | [00298 SCALAR_STRING_018](crates/bench/sqlite_parity/cases/SQLITE_PARITY_298_SCALAR_STRING_018.rs) | P1 | memory | GEN_SQL_SCALAR | 1513553 | 3896501 | <span style="color:#dc2626">-29.88%</span> |
| 62 | [00968 VIEW_TRIGGER_GENERATED_021](crates/bench/sqlite_parity/cases/SQLITE_PARITY_968_VIEW_TRIGGER_GENERATED_021.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1854919 | 3891402 | <span style="color:#dc2626">-29.71%</span> |
| 63 | [00973 VIEW_TRIGGER_GENERATED_026](crates/bench/sqlite_parity/cases/SQLITE_PARITY_973_VIEW_TRIGGER_GENERATED_026.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1812238 | 3891391 | <span style="color:#dc2626">-29.71%</span> |
| 64 | [00628 JOIN_SUBQUERY_EXISTS_021](crates/bench/sqlite_parity/cases/SQLITE_PARITY_628_JOIN_SUBQUERY_EXISTS_021.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1765019 | 3890309 | <span style="color:#dc2626">-29.68%</span> |
| 65 | [00982 VIEW_TRIGGER_GENERATED_035](crates/bench/sqlite_parity/cases/SQLITE_PARITY_982_VIEW_TRIGGER_GENERATED_035.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1889023 | 3882144 | <span style="color:#dc2626">-29.40%</span> |
| 66 | [00642 JOIN_SUBQUERY_EXISTS_035](crates/bench/sqlite_parity/cases/SQLITE_PARITY_642_JOIN_SUBQUERY_EXISTS_035.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1754058 | 3877786 | <span style="color:#dc2626">-29.26%</span> |
| 67 | [01056 JSON_EXTRACT_SET_049](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1056_JSON_EXTRACT_SET_049.rs) | P2 | memory | GEN_SQL_JSON | 1674298 | 3874970 | <span style="color:#dc2626">-29.17%</span> |
| 68 | [00979 VIEW_TRIGGER_GENERATED_032](crates/bench/sqlite_parity/cases/SQLITE_PARITY_979_VIEW_TRIGGER_GENERATED_032.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2206945 | 3874389 | <span style="color:#dc2626">-29.15%</span> |
| 69 | [00468 DML_WHERE_ORDER_LIMIT_081](crates/bench/sqlite_parity/cases/SQLITE_PARITY_468_DML_WHERE_ORDER_LIMIT_081.rs) | P1 | memory | GEN_SQL_DML | 1657145 | 3869440 | <span style="color:#dc2626">-28.98%</span> |
| 70 | [00598 AGG_GROUP_HAVING_091](crates/bench/sqlite_parity/cases/SQLITE_PARITY_598_AGG_GROUP_HAVING_091.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1725975 | 3863278 | <span style="color:#dc2626">-28.78%</span> |
| 71 | [00507 DML_WHERE_ORDER_LIMIT_120](crates/bench/sqlite_parity/cases/SQLITE_PARITY_507_DML_WHERE_ORDER_LIMIT_120.rs) | P1 | memory | GEN_SQL_DML | 1635414 | 3863218 | <span style="color:#dc2626">-28.77%</span> |
| 72 | [00494 DML_WHERE_ORDER_LIMIT_107](crates/bench/sqlite_parity/cases/SQLITE_PARITY_494_DML_WHERE_ORDER_LIMIT_107.rs) | P1 | memory | GEN_SQL_DML | 1636566 | 3852758 | <span style="color:#dc2626">-28.43%</span> |
| 73 | [00529 AGG_GROUP_HAVING_022](crates/bench/sqlite_parity/cases/SQLITE_PARITY_529_AGG_GROUP_HAVING_022.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1860630 | 3849803 | <span style="color:#dc2626">-28.33%</span> |
| 74 | [00673 JOIN_SUBQUERY_EXISTS_066](crates/bench/sqlite_parity/cases/SQLITE_PARITY_673_JOIN_SUBQUERY_EXISTS_066.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1896858 | 3849152 | <span style="color:#dc2626">-28.31%</span> |
| 75 | [00109 DOT_MODE_JSON](crates/bench/sqlite_parity/cases/SQLITE_PARITY_109_DOT_MODE_JSON.rs) | P0 | memory | CLI_DOT_COMMAND | 1495409 | 3845735 | <span style="color:#dc2626">-28.19%</span> |
| 76 | [00420 DML_WHERE_ORDER_LIMIT_033](crates/bench/sqlite_parity/cases/SQLITE_PARITY_420_DML_WHERE_ORDER_LIMIT_033.rs) | P1 | memory | GEN_SQL_DML | 1669899 | 3843020 | <span style="color:#dc2626">-28.10%</span> |
| 77 | [00920 CONSTRAINT_FK_SAVEPOINT_053](crates/bench/sqlite_parity/cases/SQLITE_PARITY_920_CONSTRAINT_FK_SAVEPOINT_053.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1718932 | 3836437 | <span style="color:#dc2626">-27.88%</span> |
| 78 | [00289 SCALAR_CAST_TYPEOF_016](crates/bench/sqlite_parity/cases/SQLITE_PARITY_289_SCALAR_CAST_TYPEOF_016.rs) | P1 | memory | GEN_SQL_SCALAR | 1456165 | 3831819 | <span style="color:#dc2626">-27.73%</span> |
| 79 | [00952 VIEW_TRIGGER_GENERATED_005](crates/bench/sqlite_parity/cases/SQLITE_PARITY_952_VIEW_TRIGGER_GENERATED_005.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1856653 | 3827951 | <span style="color:#dc2626">-27.60%</span> |
| 80 | [00905 CONSTRAINT_FK_SAVEPOINT_038](crates/bench/sqlite_parity/cases/SQLITE_PARITY_905_CONSTRAINT_FK_SAVEPOINT_038.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1747336 | 3827681 | <span style="color:#dc2626">-27.59%</span> |
| 81 | [00611 JOIN_SUBQUERY_EXISTS_004](crates/bench/sqlite_parity/cases/SQLITE_PARITY_611_JOIN_SUBQUERY_EXISTS_004.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1903612 | 3827270 | <span style="color:#dc2626">-27.58%</span> |
| 82 | [00646 JOIN_SUBQUERY_EXISTS_039](crates/bench/sqlite_parity/cases/SQLITE_PARITY_646_JOIN_SUBQUERY_EXISTS_039.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1747466 | 3825787 | <span style="color:#dc2626">-27.53%</span> |
| 83 | [00435 DML_WHERE_ORDER_LIMIT_048](crates/bench/sqlite_parity/cases/SQLITE_PARITY_435_DML_WHERE_ORDER_LIMIT_048.rs) | P1 | memory | GEN_SQL_DML | 1708001 | 3825287 | <span style="color:#dc2626">-27.51%</span> |
| 84 | [01089 INDEX_SCHEMA_PRAGMA_022](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1089_INDEX_SCHEMA_PRAGMA_022.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1689045 | 3822070 | <span style="color:#dc2626">-27.40%</span> |
| 85 | [00980 VIEW_TRIGGER_GENERATED_033](crates/bench/sqlite_parity/cases/SQLITE_PARITY_980_VIEW_TRIGGER_GENERATED_033.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1724793 | 3821790 | <span style="color:#dc2626">-27.39%</span> |
| 86 | [01090 INDEX_SCHEMA_PRAGMA_023](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1090_INDEX_SCHEMA_PRAGMA_023.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1740904 | 3815438 | <span style="color:#dc2626">-27.18%</span> |
| 87 | [00644 JOIN_SUBQUERY_EXISTS_037](crates/bench/sqlite_parity/cases/SQLITE_PARITY_644_JOIN_SUBQUERY_EXISTS_037.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1810014 | 3813975 | <span style="color:#dc2626">-27.13%</span> |
| 88 | [00411 DML_WHERE_ORDER_LIMIT_024](crates/bench/sqlite_parity/cases/SQLITE_PARITY_411_DML_WHERE_ORDER_LIMIT_024.rs) | P1 | memory | GEN_SQL_DML | 1690348 | 3804527 | <span style="color:#dc2626">-26.82%</span> |
| 89 | [01006 VIEW_TRIGGER_GENERATED_059](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1006_VIEW_TRIGGER_GENERATED_059.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1711999 | 3790520 | <span style="color:#dc2626">-26.35%</span> |
| 90 | [00533 AGG_GROUP_HAVING_026](crates/bench/sqlite_parity/cases/SQLITE_PARITY_533_AGG_GROUP_HAVING_026.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1646144 | 3789409 | <span style="color:#dc2626">-26.31%</span> |
| 91 | [00737 CTE_RECURSIVE_MATRIX_030](crates/bench/sqlite_parity/cases/SQLITE_PARITY_737_CTE_RECURSIVE_MATRIX_030.rs) | P1 | memory | GEN_SQL_CTE | 1682673 | 3787065 | <span style="color:#dc2626">-26.24%</span> |
| 92 | [00400 DML_WHERE_ORDER_LIMIT_013](crates/bench/sqlite_parity/cases/SQLITE_PARITY_400_DML_WHERE_ORDER_LIMIT_013.rs) | P1 | memory | GEN_SQL_DML | 2019831 | 3785271 | <span style="color:#dc2626">-26.18%</span> |
| 93 | [00911 CONSTRAINT_FK_SAVEPOINT_044](crates/bench/sqlite_parity/cases/SQLITE_PARITY_911_CONSTRAINT_FK_SAVEPOINT_044.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1749850 | 3783487 | <span style="color:#dc2626">-26.12%</span> |
| 94 | [00521 AGG_GROUP_HAVING_014](crates/bench/sqlite_parity/cases/SQLITE_PARITY_521_AGG_GROUP_HAVING_014.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1863245 | 3781383 | <span style="color:#dc2626">-26.05%</span> |
| 95 | [00616 JOIN_SUBQUERY_EXISTS_009](crates/bench/sqlite_parity/cases/SQLITE_PARITY_616_JOIN_SUBQUERY_EXISTS_009.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1973454 | 3779860 | <span style="color:#dc2626">-26.00%</span> |
| 96 | [01074 INDEX_SCHEMA_PRAGMA_007](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1074_INDEX_SCHEMA_PRAGMA_007.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1820775 | 3779380 | <span style="color:#dc2626">-25.98%</span> |
| 97 | [00845 WINDOW_PARTITION_SUM_058](crates/bench/sqlite_parity/cases/SQLITE_PARITY_845_WINDOW_PARTITION_SUM_058.rs) | P2 | memory | GEN_SQL_WINDOW | 1641806 | 3765634 | <span style="color:#dc2626">-25.52%</span> |
| 98 | [00600 AGG_GROUP_HAVING_093](crates/bench/sqlite_parity/cases/SQLITE_PARITY_600_AGG_GROUP_HAVING_093.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1718771 | 3764832 | <span style="color:#dc2626">-25.49%</span> |
| 99 | [00519 AGG_GROUP_HAVING_012](crates/bench/sqlite_parity/cases/SQLITE_PARITY_519_AGG_GROUP_HAVING_012.rs) | P1 | memory | GEN_SQL_AGGREGATE | 3039059 | 3813083 | <span style="color:#dc2626">-25.47%</span> |
| 100 | [01002 VIEW_TRIGGER_GENERATED_055](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1002_VIEW_TRIGGER_GENERATED_055.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1967772 | 3763450 | <span style="color:#dc2626">-25.45%</span> |
| 101 | [00614 JOIN_SUBQUERY_EXISTS_007](crates/bench/sqlite_parity/cases/SQLITE_PARITY_614_JOIN_SUBQUERY_EXISTS_007.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2010132 | 3760734 | <span style="color:#dc2626">-25.36%</span> |
| 102 | [00451 DML_WHERE_ORDER_LIMIT_064](crates/bench/sqlite_parity/cases/SQLITE_PARITY_451_DML_WHERE_ORDER_LIMIT_064.rs) | P1 | memory | GEN_SQL_DML | 2018568 | 3755174 | <span style="color:#dc2626">-25.17%</span> |
| 103 | [00930 CONSTRAINT_FK_SAVEPOINT_063](crates/bench/sqlite_parity/cases/SQLITE_PARITY_930_CONSTRAINT_FK_SAVEPOINT_063.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1690768 | 3754122 | <span style="color:#dc2626">-25.14%</span> |
| 104 | [00999 VIEW_TRIGGER_GENERATED_052](crates/bench/sqlite_parity/cases/SQLITE_PARITY_999_VIEW_TRIGGER_GENERATED_052.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1759708 | 3749322 | <span style="color:#dc2626">-24.98%</span> |
| 105 | [01084 INDEX_SCHEMA_PRAGMA_017](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1084_INDEX_SCHEMA_PRAGMA_017.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 2018428 | 3739504 | <span style="color:#dc2626">-24.65%</span> |
| 106 | [00702 JOIN_SUBQUERY_EXISTS_095](crates/bench/sqlite_parity/cases/SQLITE_PARITY_702_JOIN_SUBQUERY_EXISTS_095.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2003790 | 3735867 | <span style="color:#dc2626">-24.53%</span> |
| 107 | [00007 WITHOUT_ROWID_TABLE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_007_WITHOUT_ROWID_TABLE.rs) | P0 | memory | SQL_ROWID | 1548059 | 3731919 | <span style="color:#dc2626">-24.40%</span> |
| 108 | [00899 CONSTRAINT_FK_SAVEPOINT_032](crates/bench/sqlite_parity/cases/SQLITE_PARITY_899_CONSTRAINT_FK_SAVEPOINT_032.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1796168 | 3730998 | <span style="color:#dc2626">-24.37%</span> |
| 109 | [00460 DML_WHERE_ORDER_LIMIT_073](crates/bench/sqlite_parity/cases/SQLITE_PARITY_460_DML_WHERE_ORDER_LIMIT_073.rs) | P1 | memory | GEN_SQL_DML | 1897339 | 3718013 | <span style="color:#dc2626">-23.93%</span> |
| 110 | [00505 DML_WHERE_ORDER_LIMIT_118](crates/bench/sqlite_parity/cases/SQLITE_PARITY_505_DML_WHERE_ORDER_LIMIT_118.rs) | P1 | memory | GEN_SQL_DML | 1951402 | 3711891 | <span style="color:#dc2626">-23.73%</span> |
| 111 | [00079 CHECK_CONSTRAINT_FAILURE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_079_CHECK_CONSTRAINT_FAILURE.rs) | P0 | memory | SQL_CONSTRAINTS_NEGATIVE | 1586551 | 3711721 | <span style="color:#dc2626">-23.72%</span> |
| 112 | [00021 UPSERT_DO_NOTHING](crates/bench/sqlite_parity/cases/SQLITE_PARITY_021_UPSERT_DO_NOTHING.rs) | P0 | memory | SQL_UPSERT | 1587232 | 3710880 | <span style="color:#dc2626">-23.70%</span> |
| 113 | [00520 AGG_GROUP_HAVING_013](crates/bench/sqlite_parity/cases/SQLITE_PARITY_520_AGG_GROUP_HAVING_013.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2274543 | 3710519 | <span style="color:#dc2626">-23.68%</span> |
| 114 | [00912 CONSTRAINT_FK_SAVEPOINT_045](crates/bench/sqlite_parity/cases/SQLITE_PARITY_912_CONSTRAINT_FK_SAVEPOINT_045.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1650713 | 3708555 | <span style="color:#dc2626">-23.62%</span> |
| 115 | [00476 DML_WHERE_ORDER_LIMIT_089](crates/bench/sqlite_parity/cases/SQLITE_PARITY_476_DML_WHERE_ORDER_LIMIT_089.rs) | P1 | memory | GEN_SQL_DML | 1726817 | 3708175 | <span style="color:#dc2626">-23.61%</span> |
| 116 | [00885 CONSTRAINT_FK_SAVEPOINT_018](crates/bench/sqlite_parity/cases/SQLITE_PARITY_885_CONSTRAINT_FK_SAVEPOINT_018.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1737637 | 3698807 | <span style="color:#dc2626">-23.29%</span> |
| 117 | [00694 JOIN_SUBQUERY_EXISTS_087](crates/bench/sqlite_parity/cases/SQLITE_PARITY_694_JOIN_SUBQUERY_EXISTS_087.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1763166 | 3696513 | <span style="color:#dc2626">-23.22%</span> |
| 118 | [00747 CTE_RECURSIVE_MATRIX_040](crates/bench/sqlite_parity/cases/SQLITE_PARITY_747_CTE_RECURSIVE_MATRIX_040.rs) | P1 | memory | GEN_SQL_CTE | 2125631 | 3694599 | <span style="color:#dc2626">-23.15%</span> |
| 119 | [01053 JSON_EXTRACT_SET_046](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1053_JSON_EXTRACT_SET_046.rs) | P2 | memory | GEN_SQL_JSON | 1887600 | 3693036 | <span style="color:#dc2626">-23.10%</span> |
| 120 | [00452 DML_WHERE_ORDER_LIMIT_065](crates/bench/sqlite_parity/cases/SQLITE_PARITY_452_DML_WHERE_ORDER_LIMIT_065.rs) | P1 | memory | GEN_SQL_DML | 1870147 | 3692585 | <span style="color:#dc2626">-23.09%</span> |
| 121 | [00870 CONSTRAINT_FK_SAVEPOINT_003](crates/bench/sqlite_parity/cases/SQLITE_PARITY_870_CONSTRAINT_FK_SAVEPOINT_003.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1980777 | 3690201 | <span style="color:#dc2626">-23.01%</span> |
| 122 | [01067 JSON_EXTRACT_SET_060](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1067_JSON_EXTRACT_SET_060.rs) | P2 | memory | GEN_SQL_JSON | 1681060 | 3688377 | <span style="color:#dc2626">-22.95%</span> |
| 123 | [01111 INDEX_SCHEMA_PRAGMA_044](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1111_INDEX_SCHEMA_PRAGMA_044.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1822698 | 3687856 | <span style="color:#dc2626">-22.93%</span> |
| 124 | [00481 DML_WHERE_ORDER_LIMIT_094](crates/bench/sqlite_parity/cases/SQLITE_PARITY_481_DML_WHERE_ORDER_LIMIT_094.rs) | P1 | memory | GEN_SQL_DML | 1591982 | 3684710 | <span style="color:#dc2626">-22.82%</span> |
| 125 | [01096 INDEX_SCHEMA_PRAGMA_029](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1096_INDEX_SCHEMA_PRAGMA_029.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1748949 | 3682807 | <span style="color:#dc2626">-22.76%</span> |
| 126 | [00518 AGG_GROUP_HAVING_011](crates/bench/sqlite_parity/cases/SQLITE_PARITY_518_AGG_GROUP_HAVING_011.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2492835 | 3679821 | <span style="color:#dc2626">-22.66%</span> |
| 127 | [00407 DML_WHERE_ORDER_LIMIT_020](crates/bench/sqlite_parity/cases/SQLITE_PARITY_407_DML_WHERE_ORDER_LIMIT_020.rs) | P1 | memory | GEN_SQL_DML | 1922717 | 3679501 | <span style="color:#dc2626">-22.65%</span> |
| 128 | [00969 VIEW_TRIGGER_GENERATED_022](crates/bench/sqlite_parity/cases/SQLITE_PARITY_969_VIEW_TRIGGER_GENERATED_022.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1823300 | 3679360 | <span style="color:#dc2626">-22.65%</span> |
| 129 | [01058 JSON_EXTRACT_SET_051](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1058_JSON_EXTRACT_SET_051.rs) | P2 | memory | GEN_SQL_JSON | 1966400 | 3677226 | <span style="color:#dc2626">-22.57%</span> |
| 130 | [00705 JOIN_SUBQUERY_EXISTS_098](crates/bench/sqlite_parity/cases/SQLITE_PARITY_705_JOIN_SUBQUERY_EXISTS_098.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1742536 | 3673008 | <span style="color:#dc2626">-22.43%</span> |
| 131 | [00636 JOIN_SUBQUERY_EXISTS_029](crates/bench/sqlite_parity/cases/SQLITE_PARITY_636_JOIN_SUBQUERY_EXISTS_029.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1768666 | 3669843 | <span style="color:#dc2626">-22.33%</span> |
| 132 | [01077 INDEX_SCHEMA_PRAGMA_010](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1077_INDEX_SCHEMA_PRAGMA_010.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1812820 | 3667829 | <span style="color:#dc2626">-22.26%</span> |
| 133 | [00901 CONSTRAINT_FK_SAVEPOINT_034](crates/bench/sqlite_parity/cases/SQLITE_PARITY_901_CONSTRAINT_FK_SAVEPOINT_034.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1985586 | 3667679 | <span style="color:#dc2626">-22.26%</span> |
| 134 | [00351 SCALAR_NULL_COALESCE_031](crates/bench/sqlite_parity/cases/SQLITE_PARITY_351_SCALAR_NULL_COALESCE_031.rs) | P1 | memory | GEN_SQL_SCALAR | 1463037 | 3667488 | <span style="color:#dc2626">-22.25%</span> |
| 135 | [00269 SCALAR_CAST_TYPEOF_011](crates/bench/sqlite_parity/cases/SQLITE_PARITY_269_SCALAR_CAST_TYPEOF_011.rs) | P1 | memory | GEN_SQL_SCALAR | 1473789 | 3667087 | <span style="color:#dc2626">-22.24%</span> |
| 136 | [00655 JOIN_SUBQUERY_EXISTS_048](crates/bench/sqlite_parity/cases/SQLITE_PARITY_655_JOIN_SUBQUERY_EXISTS_048.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1649540 | 3666877 | <span style="color:#dc2626">-22.23%</span> |
| 137 | [00734 CTE_RECURSIVE_MATRIX_027](crates/bench/sqlite_parity/cases/SQLITE_PARITY_734_CTE_RECURSIVE_MATRIX_027.rs) | P1 | memory | GEN_SQL_CTE | 1626296 | 3666707 | <span style="color:#dc2626">-22.22%</span> |
| 138 | [00509 AGG_GROUP_HAVING_002](crates/bench/sqlite_parity/cases/SQLITE_PARITY_509_AGG_GROUP_HAVING_002.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2083692 | 3666326 | <span style="color:#dc2626">-22.21%</span> |
| 139 | [00513 AGG_GROUP_HAVING_006](crates/bench/sqlite_parity/cases/SQLITE_PARITY_513_AGG_GROUP_HAVING_006.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1749930 | 3666065 | <span style="color:#dc2626">-22.20%</span> |
| 140 | [00408 DML_WHERE_ORDER_LIMIT_021](crates/bench/sqlite_parity/cases/SQLITE_PARITY_408_DML_WHERE_ORDER_LIMIT_021.rs) | P1 | memory | GEN_SQL_DML | 1770830 | 3663230 | <span style="color:#dc2626">-22.11%</span> |
| 141 | [01114 INDEX_SCHEMA_PRAGMA_047](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1114_INDEX_SCHEMA_PRAGMA_047.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1757996 | 3661918 | <span style="color:#dc2626">-22.06%</span> |
| 142 | [00882 CONSTRAINT_FK_SAVEPOINT_015](crates/bench/sqlite_parity/cases/SQLITE_PARITY_882_CONSTRAINT_FK_SAVEPOINT_015.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1686230 | 3661646 | <span style="color:#dc2626">-22.05%</span> |
| 143 | [00510 AGG_GROUP_HAVING_003](crates/bench/sqlite_parity/cases/SQLITE_PARITY_510_AGG_GROUP_HAVING_003.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1785818 | 3661156 | <span style="color:#dc2626">-22.04%</span> |
| 144 | [01104 INDEX_SCHEMA_PRAGMA_037](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1104_INDEX_SCHEMA_PRAGMA_037.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1739872 | 3660966 | <span style="color:#dc2626">-22.03%</span> |
| 145 | [00621 JOIN_SUBQUERY_EXISTS_014](crates/bench/sqlite_parity/cases/SQLITE_PARITY_621_JOIN_SUBQUERY_EXISTS_014.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1788122 | 3660725 | <span style="color:#dc2626">-22.02%</span> |
| 146 | [00701 JOIN_SUBQUERY_EXISTS_094](crates/bench/sqlite_parity/cases/SQLITE_PARITY_701_JOIN_SUBQUERY_EXISTS_094.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1767574 | 3660345 | <span style="color:#dc2626">-22.01%</span> |
| 147 | [01093 INDEX_SCHEMA_PRAGMA_026](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1093_INDEX_SCHEMA_PRAGMA_026.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1711257 | 3658100 | <span style="color:#dc2626">-21.94%</span> |
| 148 | [00645 JOIN_SUBQUERY_EXISTS_038](crates/bench/sqlite_parity/cases/SQLITE_PARITY_645_JOIN_SUBQUERY_EXISTS_038.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1774907 | 3656597 | <span style="color:#dc2626">-21.89%</span> |
| 149 | [01124 INDEX_SCHEMA_PRAGMA_057](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1124_INDEX_SCHEMA_PRAGMA_057.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1780278 | 3655225 | <span style="color:#dc2626">-21.84%</span> |
| 150 | [01118 INDEX_SCHEMA_PRAGMA_051](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1118_INDEX_SCHEMA_PRAGMA_051.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1739010 | 3654693 | <span style="color:#dc2626">-21.82%</span> |
| 151 | [00608 JOIN_SUBQUERY_EXISTS_001](crates/bench/sqlite_parity/cases/SQLITE_PARITY_608_JOIN_SUBQUERY_EXISTS_001.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1832577 | 3654673 | <span style="color:#dc2626">-21.82%</span> |
| 152 | [01122 INDEX_SCHEMA_PRAGMA_055](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1122_INDEX_SCHEMA_PRAGMA_055.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 2007117 | 3650947 | <span style="color:#dc2626">-21.70%</span> |
| 153 | [01045 JSON_EXTRACT_SET_038](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1045_JSON_EXTRACT_SET_038.rs) | P2 | memory | GEN_SQL_JSON | 1756483 | 3647941 | <span style="color:#dc2626">-21.60%</span> |
| 154 | [00648 JOIN_SUBQUERY_EXISTS_041](crates/bench/sqlite_parity/cases/SQLITE_PARITY_648_JOIN_SUBQUERY_EXISTS_041.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1717249 | 3645826 | <span style="color:#dc2626">-21.53%</span> |
| 155 | [00626 JOIN_SUBQUERY_EXISTS_019](crates/bench/sqlite_parity/cases/SQLITE_PARITY_626_JOIN_SUBQUERY_EXISTS_019.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1757555 | 3643322 | <span style="color:#dc2626">-21.44%</span> |
| 156 | [00707 JOIN_SUBQUERY_EXISTS_100](crates/bench/sqlite_parity/cases/SQLITE_PARITY_707_JOIN_SUBQUERY_EXISTS_100.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1841985 | 3641058 | <span style="color:#dc2626">-21.37%</span> |
| 157 | [00659 JOIN_SUBQUERY_EXISTS_052](crates/bench/sqlite_parity/cases/SQLITE_PARITY_659_JOIN_SUBQUERY_EXISTS_052.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1693002 | 3640147 | <span style="color:#dc2626">-21.34%</span> |
| 158 | [00649 JOIN_SUBQUERY_EXISTS_042](crates/bench/sqlite_parity/cases/SQLITE_PARITY_649_JOIN_SUBQUERY_EXISTS_042.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1703021 | 3633083 | <span style="color:#dc2626">-21.10%</span> |
| 159 | [00462 DML_WHERE_ORDER_LIMIT_075](crates/bench/sqlite_parity/cases/SQLITE_PARITY_462_DML_WHERE_ORDER_LIMIT_075.rs) | P1 | memory | GEN_SQL_DML | 1846503 | 3632552 | <span style="color:#dc2626">-21.09%</span> |
| 160 | [00665 JOIN_SUBQUERY_EXISTS_058](crates/bench/sqlite_parity/cases/SQLITE_PARITY_665_JOIN_SUBQUERY_EXISTS_058.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1772443 | 3632031 | <span style="color:#dc2626">-21.07%</span> |
| 161 | [00299 SCALAR_NULL_COALESCE_018](crates/bench/sqlite_parity/cases/SQLITE_PARITY_299_SCALAR_NULL_COALESCE_018.rs) | P1 | memory | GEN_SQL_SCALAR | 1956350 | 3630468 | <span style="color:#dc2626">-21.02%</span> |
| 162 | [01081 INDEX_SCHEMA_PRAGMA_014](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1081_INDEX_SCHEMA_PRAGMA_014.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1752606 | 3629977 | <span style="color:#dc2626">-21.00%</span> |
| 163 | [00455 DML_WHERE_ORDER_LIMIT_068](crates/bench/sqlite_parity/cases/SQLITE_PARITY_455_DML_WHERE_ORDER_LIMIT_068.rs) | P1 | memory | GEN_SQL_DML | 2022426 | 3629596 | <span style="color:#dc2626">-20.99%</span> |
| 164 | [00688 JOIN_SUBQUERY_EXISTS_081](crates/bench/sqlite_parity/cases/SQLITE_PARITY_688_JOIN_SUBQUERY_EXISTS_081.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1762585 | 3628594 | <span style="color:#dc2626">-20.95%</span> |
| 165 | [00660 JOIN_SUBQUERY_EXISTS_053](crates/bench/sqlite_parity/cases/SQLITE_PARITY_660_JOIN_SUBQUERY_EXISTS_053.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1702390 | 3628454 | <span style="color:#dc2626">-20.95%</span> |
| 166 | [00641 JOIN_SUBQUERY_EXISTS_034](crates/bench/sqlite_parity/cases/SQLITE_PARITY_641_JOIN_SUBQUERY_EXISTS_034.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1761302 | 3627032 | <span style="color:#dc2626">-20.90%</span> |
| 167 | [00662 JOIN_SUBQUERY_EXISTS_055](crates/bench/sqlite_parity/cases/SQLITE_PARITY_662_JOIN_SUBQUERY_EXISTS_055.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1807510 | 3626851 | <span style="color:#dc2626">-20.90%</span> |
| 168 | [00612 JOIN_SUBQUERY_EXISTS_005](crates/bench/sqlite_parity/cases/SQLITE_PARITY_612_JOIN_SUBQUERY_EXISTS_005.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1740563 | 3625458 | <span style="color:#dc2626">-20.85%</span> |
| 169 | [00695 JOIN_SUBQUERY_EXISTS_088](crates/bench/sqlite_parity/cases/SQLITE_PARITY_695_JOIN_SUBQUERY_EXISTS_088.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1753808 | 3622483 | <span style="color:#dc2626">-20.75%</span> |
| 170 | [00495 DML_WHERE_ORDER_LIMIT_108](crates/bench/sqlite_parity/cases/SQLITE_PARITY_495_DML_WHERE_ORDER_LIMIT_108.rs) | P1 | memory | GEN_SQL_DML | 1633350 | 3620008 | <span style="color:#dc2626">-20.67%</span> |
| 171 | [00631 JOIN_SUBQUERY_EXISTS_024](crates/bench/sqlite_parity/cases/SQLITE_PARITY_631_JOIN_SUBQUERY_EXISTS_024.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1796298 | 3618656 | <span style="color:#dc2626">-20.62%</span> |
| 172 | [00661 JOIN_SUBQUERY_EXISTS_054](crates/bench/sqlite_parity/cases/SQLITE_PARITY_661_JOIN_SUBQUERY_EXISTS_054.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1703593 | 3618455 | <span style="color:#dc2626">-20.62%</span> |
| 173 | [00682 JOIN_SUBQUERY_EXISTS_075](crates/bench/sqlite_parity/cases/SQLITE_PARITY_682_JOIN_SUBQUERY_EXISTS_075.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2135800 | 3618405 | <span style="color:#dc2626">-20.61%</span> |
| 174 | [00698 JOIN_SUBQUERY_EXISTS_091](crates/bench/sqlite_parity/cases/SQLITE_PARITY_698_JOIN_SUBQUERY_EXISTS_091.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1718381 | 3618174 | <span style="color:#dc2626">-20.61%</span> |
| 175 | [00617 JOIN_SUBQUERY_EXISTS_010](crates/bench/sqlite_parity/cases/SQLITE_PARITY_617_JOIN_SUBQUERY_EXISTS_010.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1780228 | 3617102 | <span style="color:#dc2626">-20.57%</span> |
| 176 | [00666 JOIN_SUBQUERY_EXISTS_059](crates/bench/sqlite_parity/cases/SQLITE_PARITY_666_JOIN_SUBQUERY_EXISTS_059.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1766612 | 3616832 | <span style="color:#dc2626">-20.56%</span> |
| 177 | [00609 JOIN_SUBQUERY_EXISTS_002](crates/bench/sqlite_parity/cases/SQLITE_PARITY_609_JOIN_SUBQUERY_EXISTS_002.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1690317 | 3615740 | <span style="color:#dc2626">-20.52%</span> |
| 178 | [00514 AGG_GROUP_HAVING_007](crates/bench/sqlite_parity/cases/SQLITE_PARITY_514_AGG_GROUP_HAVING_007.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2118527 | 3615580 | <span style="color:#dc2626">-20.52%</span> |
| 179 | [00672 JOIN_SUBQUERY_EXISTS_065](crates/bench/sqlite_parity/cases/SQLITE_PARITY_672_JOIN_SUBQUERY_EXISTS_065.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1770018 | 3615319 | <span style="color:#dc2626">-20.51%</span> |
| 180 | [01126 INDEX_SCHEMA_PRAGMA_059](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1126_INDEX_SCHEMA_PRAGMA_059.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1783715 | 3615289 | <span style="color:#dc2626">-20.51%</span> |
| 181 | [01076 INDEX_SCHEMA_PRAGMA_009](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1076_INDEX_SCHEMA_PRAGMA_009.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1775008 | 3614117 | <span style="color:#dc2626">-20.47%</span> |
| 182 | [00976 VIEW_TRIGGER_GENERATED_029](crates/bench/sqlite_parity/cases/SQLITE_PARITY_976_VIEW_TRIGGER_GENERATED_029.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1809523 | 3614057 | <span style="color:#dc2626">-20.47%</span> |
| 183 | [00647 JOIN_SUBQUERY_EXISTS_040](crates/bench/sqlite_parity/cases/SQLITE_PARITY_647_JOIN_SUBQUERY_EXISTS_040.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1863836 | 3613395 | <span style="color:#dc2626">-20.45%</span> |
| 184 | [00877 CONSTRAINT_FK_SAVEPOINT_010](crates/bench/sqlite_parity/cases/SQLITE_PARITY_877_CONSTRAINT_FK_SAVEPOINT_010.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1718010 | 3611963 | <span style="color:#dc2626">-20.40%</span> |
| 185 | [00603 AGG_GROUP_HAVING_096](crates/bench/sqlite_parity/cases/SQLITE_PARITY_603_AGG_GROUP_HAVING_096.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1712530 | 3611802 | <span style="color:#dc2626">-20.39%</span> |
| 186 | [00668 JOIN_SUBQUERY_EXISTS_061](crates/bench/sqlite_parity/cases/SQLITE_PARITY_668_JOIN_SUBQUERY_EXISTS_061.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1761702 | 3611782 | <span style="color:#dc2626">-20.39%</span> |
| 187 | [00670 JOIN_SUBQUERY_EXISTS_063](crates/bench/sqlite_parity/cases/SQLITE_PARITY_670_JOIN_SUBQUERY_EXISTS_063.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1776560 | 3611241 | <span style="color:#dc2626">-20.37%</span> |
| 188 | [01112 INDEX_SCHEMA_PRAGMA_045](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1112_INDEX_SCHEMA_PRAGMA_045.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1831785 | 3610981 | <span style="color:#dc2626">-20.37%</span> |
| 189 | [00674 JOIN_SUBQUERY_EXISTS_067](crates/bench/sqlite_parity/cases/SQLITE_PARITY_674_JOIN_SUBQUERY_EXISTS_067.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1753326 | 3609638 | <span style="color:#dc2626">-20.32%</span> |
| 190 | [01107 INDEX_SCHEMA_PRAGMA_040](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1107_INDEX_SCHEMA_PRAGMA_040.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1849037 | 3604269 | <span style="color:#dc2626">-20.14%</span> |
| 191 | [00697 JOIN_SUBQUERY_EXISTS_090](crates/bench/sqlite_parity/cases/SQLITE_PARITY_697_JOIN_SUBQUERY_EXISTS_090.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1675029 | 3604007 | <span style="color:#dc2626">-20.13%</span> |
| 192 | [00681 JOIN_SUBQUERY_EXISTS_074](crates/bench/sqlite_parity/cases/SQLITE_PARITY_681_JOIN_SUBQUERY_EXISTS_074.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1751363 | 3603638 | <span style="color:#dc2626">-20.12%</span> |
| 193 | [00683 JOIN_SUBQUERY_EXISTS_076](crates/bench/sqlite_parity/cases/SQLITE_PARITY_683_JOIN_SUBQUERY_EXISTS_076.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1764778 | 3603477 | <span style="color:#dc2626">-20.12%</span> |
| 194 | [00692 JOIN_SUBQUERY_EXISTS_085](crates/bench/sqlite_parity/cases/SQLITE_PARITY_692_JOIN_SUBQUERY_EXISTS_085.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2095194 | 3603076 | <span style="color:#dc2626">-20.10%</span> |
| 195 | [00676 JOIN_SUBQUERY_EXISTS_069](crates/bench/sqlite_parity/cases/SQLITE_PARITY_676_JOIN_SUBQUERY_EXISTS_069.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1800656 | 3602545 | <span style="color:#dc2626">-20.08%</span> |
| 196 | [00457 DML_WHERE_ORDER_LIMIT_070](crates/bench/sqlite_parity/cases/SQLITE_PARITY_457_DML_WHERE_ORDER_LIMIT_070.rs) | P1 | memory | GEN_SQL_DML | 1676993 | 3601123 | <span style="color:#dc2626">-20.04%</span> |
| 197 | [00388 DML_WHERE_ORDER_LIMIT_001](crates/bench/sqlite_parity/cases/SQLITE_PARITY_388_DML_WHERE_ORDER_LIMIT_001.rs) | P1 | memory | GEN_SQL_DML | 1883523 | 3599690 | <span style="color:#dc2626">-19.99%</span> |
| 198 | [01078 INDEX_SCHEMA_PRAGMA_011](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1078_INDEX_SCHEMA_PRAGMA_011.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1713101 | 3599159 | <span style="color:#dc2626">-19.97%</span> |
| 199 | [00235 SCALAR_NULL_COALESCE_002](crates/bench/sqlite_parity/cases/SQLITE_PARITY_235_SCALAR_NULL_COALESCE_002.rs) | P1 | memory | GEN_SQL_SCALAR | 1478957 | 3598818 | <span style="color:#dc2626">-19.96%</span> |
| 200 | [00706 JOIN_SUBQUERY_EXISTS_099](crates/bench/sqlite_parity/cases/SQLITE_PARITY_706_JOIN_SUBQUERY_EXISTS_099.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1717699 | 3598697 | <span style="color:#dc2626">-19.96%</span> |
| 201 | [00632 JOIN_SUBQUERY_EXISTS_025](crates/bench/sqlite_parity/cases/SQLITE_PARITY_632_JOIN_SUBQUERY_EXISTS_025.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1724663 | 3597996 | <span style="color:#dc2626">-19.93%</span> |
| 202 | [00846 WINDOW_PARTITION_SUM_059](crates/bench/sqlite_parity/cases/SQLITE_PARITY_846_WINDOW_PARTITION_SUM_059.rs) | P2 | memory | GEN_SQL_WINDOW | 1682663 | 3597736 | <span style="color:#dc2626">-19.92%</span> |
| 203 | [00653 JOIN_SUBQUERY_EXISTS_046](crates/bench/sqlite_parity/cases/SQLITE_PARITY_653_JOIN_SUBQUERY_EXISTS_046.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1739491 | 3597435 | <span style="color:#dc2626">-19.91%</span> |
| 204 | [00619 JOIN_SUBQUERY_EXISTS_012](crates/bench/sqlite_parity/cases/SQLITE_PARITY_619_JOIN_SUBQUERY_EXISTS_012.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1780739 | 3596805 | <span style="color:#dc2626">-19.89%</span> |
| 205 | [00638 JOIN_SUBQUERY_EXISTS_031](crates/bench/sqlite_parity/cases/SQLITE_PARITY_638_JOIN_SUBQUERY_EXISTS_031.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1921995 | 3596273 | <span style="color:#dc2626">-19.88%</span> |
| 206 | [00677 JOIN_SUBQUERY_EXISTS_070](crates/bench/sqlite_parity/cases/SQLITE_PARITY_677_JOIN_SUBQUERY_EXISTS_070.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2056500 | 3596263 | <span style="color:#dc2626">-19.88%</span> |
| 207 | [00643 JOIN_SUBQUERY_EXISTS_036](crates/bench/sqlite_parity/cases/SQLITE_PARITY_643_JOIN_SUBQUERY_EXISTS_036.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1710586 | 3596193 | <span style="color:#dc2626">-19.87%</span> |
| 208 | [00483 DML_WHERE_ORDER_LIMIT_096](crates/bench/sqlite_parity/cases/SQLITE_PARITY_483_DML_WHERE_ORDER_LIMIT_096.rs) | P1 | memory | GEN_SQL_DML | 1833098 | 3595331 | <span style="color:#dc2626">-19.84%</span> |
| 209 | [00482 DML_WHERE_ORDER_LIMIT_095](crates/bench/sqlite_parity/cases/SQLITE_PARITY_482_DML_WHERE_ORDER_LIMIT_095.rs) | P1 | memory | GEN_SQL_DML | 1946242 | 3595061 | <span style="color:#dc2626">-19.84%</span> |
| 210 | [00610 JOIN_SUBQUERY_EXISTS_003](crates/bench/sqlite_parity/cases/SQLITE_PARITY_610_JOIN_SUBQUERY_EXISTS_003.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1993281 | 3593077 | <span style="color:#dc2626">-19.77%</span> |
| 211 | [01082 INDEX_SCHEMA_PRAGMA_015](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1082_INDEX_SCHEMA_PRAGMA_015.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1793883 | 3592977 | <span style="color:#dc2626">-19.77%</span> |
| 212 | [00639 JOIN_SUBQUERY_EXISTS_032](crates/bench/sqlite_parity/cases/SQLITE_PARITY_639_JOIN_SUBQUERY_EXISTS_032.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1700397 | 3592556 | <span style="color:#dc2626">-19.75%</span> |
| 213 | [00671 JOIN_SUBQUERY_EXISTS_064](crates/bench/sqlite_parity/cases/SQLITE_PARITY_671_JOIN_SUBQUERY_EXISTS_064.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1772764 | 3592546 | <span style="color:#dc2626">-19.75%</span> |
| 214 | [00623 JOIN_SUBQUERY_EXISTS_016](crates/bench/sqlite_parity/cases/SQLITE_PARITY_623_JOIN_SUBQUERY_EXISTS_016.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1706738 | 3592465 | <span style="color:#dc2626">-19.75%</span> |
| 215 | [00675 JOIN_SUBQUERY_EXISTS_068](crates/bench/sqlite_parity/cases/SQLITE_PARITY_675_JOIN_SUBQUERY_EXISTS_068.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1691800 | 3591694 | <span style="color:#dc2626">-19.72%</span> |
| 216 | [00618 JOIN_SUBQUERY_EXISTS_011](crates/bench/sqlite_parity/cases/SQLITE_PARITY_618_JOIN_SUBQUERY_EXISTS_011.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1759619 | 3590653 | <span style="color:#dc2626">-19.69%</span> |
| 217 | [01083 INDEX_SCHEMA_PRAGMA_016](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1083_INDEX_SCHEMA_PRAGMA_016.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1761062 | 3589310 | <span style="color:#dc2626">-19.64%</span> |
| 218 | [00650 JOIN_SUBQUERY_EXISTS_043](crates/bench/sqlite_parity/cases/SQLITE_PARITY_650_JOIN_SUBQUERY_EXISTS_043.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1763637 | 3588569 | <span style="color:#dc2626">-19.62%</span> |
| 219 | [00667 JOIN_SUBQUERY_EXISTS_060](crates/bench/sqlite_parity/cases/SQLITE_PARITY_667_JOIN_SUBQUERY_EXISTS_060.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1771201 | 3586926 | <span style="color:#dc2626">-19.56%</span> |
| 220 | [00450 DML_WHERE_ORDER_LIMIT_063](crates/bench/sqlite_parity/cases/SQLITE_PARITY_450_DML_WHERE_ORDER_LIMIT_063.rs) | P1 | memory | GEN_SQL_DML | 1710505 | 3586766 | <span style="color:#dc2626">-19.56%</span> |
| 221 | [00613 JOIN_SUBQUERY_EXISTS_006](crates/bench/sqlite_parity/cases/SQLITE_PARITY_613_JOIN_SUBQUERY_EXISTS_006.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1794645 | 3585774 | <span style="color:#dc2626">-19.53%</span> |
| 222 | [00669 JOIN_SUBQUERY_EXISTS_062](crates/bench/sqlite_parity/cases/SQLITE_PARITY_669_JOIN_SUBQUERY_EXISTS_062.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1759057 | 3584030 | <span style="color:#dc2626">-19.47%</span> |
| 223 | [00680 JOIN_SUBQUERY_EXISTS_073](crates/bench/sqlite_parity/cases/SQLITE_PARITY_680_JOIN_SUBQUERY_EXISTS_073.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1745442 | 3583308 | <span style="color:#dc2626">-19.44%</span> |
| 224 | [01073 INDEX_SCHEMA_PRAGMA_006](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1073_INDEX_SCHEMA_PRAGMA_006.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1769507 | 3582567 | <span style="color:#dc2626">-19.42%</span> |
| 225 | [00686 JOIN_SUBQUERY_EXISTS_079](crates/bench/sqlite_parity/cases/SQLITE_PARITY_686_JOIN_SUBQUERY_EXISTS_079.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1724883 | 3581785 | <span style="color:#dc2626">-19.39%</span> |
| 226 | [00664 JOIN_SUBQUERY_EXISTS_057](crates/bench/sqlite_parity/cases/SQLITE_PARITY_664_JOIN_SUBQUERY_EXISTS_057.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1763076 | 3581425 | <span style="color:#dc2626">-19.38%</span> |
| 227 | [00948 VIEW_TRIGGER_GENERATED_001](crates/bench/sqlite_parity/cases/SQLITE_PARITY_948_VIEW_TRIGGER_GENERATED_001.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1718251 | 3580804 | <span style="color:#dc2626">-19.36%</span> |
| 228 | [01109 INDEX_SCHEMA_PRAGMA_042](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1109_INDEX_SCHEMA_PRAGMA_042.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1833339 | 3580754 | <span style="color:#dc2626">-19.36%</span> |
| 229 | [00658 JOIN_SUBQUERY_EXISTS_051](crates/bench/sqlite_parity/cases/SQLITE_PARITY_658_JOIN_SUBQUERY_EXISTS_051.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1771141 | 3580273 | <span style="color:#dc2626">-19.34%</span> |
| 230 | [00651 JOIN_SUBQUERY_EXISTS_044](crates/bench/sqlite_parity/cases/SQLITE_PARITY_651_JOIN_SUBQUERY_EXISTS_044.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1706669 | 3579382 | <span style="color:#dc2626">-19.31%</span> |
| 231 | [00704 JOIN_SUBQUERY_EXISTS_097](crates/bench/sqlite_parity/cases/SQLITE_PARITY_704_JOIN_SUBQUERY_EXISTS_097.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1691209 | 3578379 | <span style="color:#dc2626">-19.28%</span> |
| 232 | [00872 CONSTRAINT_FK_SAVEPOINT_005](crates/bench/sqlite_parity/cases/SQLITE_PARITY_872_CONSTRAINT_FK_SAVEPOINT_005.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1669979 | 3577467 | <span style="color:#dc2626">-19.25%</span> |
| 233 | [00893 CONSTRAINT_FK_SAVEPOINT_026](crates/bench/sqlite_parity/cases/SQLITE_PARITY_893_CONSTRAINT_FK_SAVEPOINT_026.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1671272 | 3576967 | <span style="color:#dc2626">-19.23%</span> |
| 234 | [00685 JOIN_SUBQUERY_EXISTS_078](crates/bench/sqlite_parity/cases/SQLITE_PARITY_685_JOIN_SUBQUERY_EXISTS_078.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1792251 | 3576726 | <span style="color:#dc2626">-19.22%</span> |
| 235 | [00693 JOIN_SUBQUERY_EXISTS_086](crates/bench/sqlite_parity/cases/SQLITE_PARITY_693_JOIN_SUBQUERY_EXISTS_086.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2504167 | 3575675 | <span style="color:#dc2626">-19.19%</span> |
| 236 | [00663 JOIN_SUBQUERY_EXISTS_056](crates/bench/sqlite_parity/cases/SQLITE_PARITY_663_JOIN_SUBQUERY_EXISTS_056.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1681281 | 3575574 | <span style="color:#dc2626">-19.19%</span> |
| 237 | [00990 VIEW_TRIGGER_GENERATED_043](crates/bench/sqlite_parity/cases/SQLITE_PARITY_990_VIEW_TRIGGER_GENERATED_043.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1778033 | 3574482 | <span style="color:#dc2626">-19.15%</span> |
| 238 | [00634 JOIN_SUBQUERY_EXISTS_027](crates/bench/sqlite_parity/cases/SQLITE_PARITY_634_JOIN_SUBQUERY_EXISTS_027.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1713742 | 3573971 | <span style="color:#dc2626">-19.13%</span> |
| 239 | [00219 UPDATE_LIMIT_OPTIONAL](crates/bench/sqlite_parity/cases/SQLITE_PARITY_219_UPDATE_LIMIT_OPTIONAL.rs) | P3 | memory | SQL_UPDATE_OPTIONAL | 1558318 | 3573650 | <span style="color:#dc2626">-19.12%</span> |
| 240 | [00530 AGG_GROUP_HAVING_023](crates/bench/sqlite_parity/cases/SQLITE_PARITY_530_AGG_GROUP_HAVING_023.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1701188 | 3572899 | <span style="color:#dc2626">-19.10%</span> |
| 241 | [00430 DML_WHERE_ORDER_LIMIT_043](crates/bench/sqlite_parity/cases/SQLITE_PARITY_430_DML_WHERE_ORDER_LIMIT_043.rs) | P1 | memory | GEN_SQL_DML | 1666262 | 3572458 | <span style="color:#dc2626">-19.08%</span> |
| 242 | [00615 JOIN_SUBQUERY_EXISTS_008](crates/bench/sqlite_parity/cases/SQLITE_PARITY_615_JOIN_SUBQUERY_EXISTS_008.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1725594 | 3571967 | <span style="color:#dc2626">-19.07%</span> |
| 243 | [00637 JOIN_SUBQUERY_EXISTS_030](crates/bench/sqlite_parity/cases/SQLITE_PARITY_637_JOIN_SUBQUERY_EXISTS_030.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1867072 | 3570755 | <span style="color:#dc2626">-19.03%</span> |
| 244 | [00699 JOIN_SUBQUERY_EXISTS_092](crates/bench/sqlite_parity/cases/SQLITE_PARITY_699_JOIN_SUBQUERY_EXISTS_092.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1721777 | 3570535 | <span style="color:#dc2626">-19.02%</span> |
| 245 | [01106 INDEX_SCHEMA_PRAGMA_039](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1106_INDEX_SCHEMA_PRAGMA_039.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1744580 | 3569232 | <span style="color:#dc2626">-18.97%</span> |
| 246 | [01000 VIEW_TRIGGER_GENERATED_053](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1000_VIEW_TRIGGER_GENERATED_053.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1770259 | 3569222 | <span style="color:#dc2626">-18.97%</span> |
| 247 | [00260 SCALAR_ARITH_009](crates/bench/sqlite_parity/cases/SQLITE_PARITY_260_SCALAR_ARITH_009.rs) | P1 | memory | GEN_SQL_SCALAR | 1490029 | 3568841 | <span style="color:#dc2626">-18.96%</span> |
| 248 | [00981 VIEW_TRIGGER_GENERATED_034](crates/bench/sqlite_parity/cases/SQLITE_PARITY_981_VIEW_TRIGGER_GENERATED_034.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1893622 | 3568681 | <span style="color:#dc2626">-18.96%</span> |
| 249 | [00630 JOIN_SUBQUERY_EXISTS_023](crates/bench/sqlite_parity/cases/SQLITE_PARITY_630_JOIN_SUBQUERY_EXISTS_023.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1725484 | 3568350 | <span style="color:#dc2626">-18.95%</span> |
| 250 | [00409 DML_WHERE_ORDER_LIMIT_022](crates/bench/sqlite_parity/cases/SQLITE_PARITY_409_DML_WHERE_ORDER_LIMIT_022.rs) | P1 | memory | GEN_SQL_DML | 1671983 | 3567529 | <span style="color:#dc2626">-18.92%</span> |
| 251 | [00436 DML_WHERE_ORDER_LIMIT_049](crates/bench/sqlite_parity/cases/SQLITE_PARITY_436_DML_WHERE_ORDER_LIMIT_049.rs) | P1 | memory | GEN_SQL_DML | 1652767 | 3566927 | <span style="color:#dc2626">-18.90%</span> |
| 252 | [00640 JOIN_SUBQUERY_EXISTS_033](crates/bench/sqlite_parity/cases/SQLITE_PARITY_640_JOIN_SUBQUERY_EXISTS_033.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1757595 | 3566838 | <span style="color:#dc2626">-18.89%</span> |
| 253 | [00679 JOIN_SUBQUERY_EXISTS_072](crates/bench/sqlite_parity/cases/SQLITE_PARITY_679_JOIN_SUBQUERY_EXISTS_072.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2023087 | 3566597 | <span style="color:#dc2626">-18.89%</span> |
| 254 | [00687 JOIN_SUBQUERY_EXISTS_080](crates/bench/sqlite_parity/cases/SQLITE_PARITY_687_JOIN_SUBQUERY_EXISTS_080.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1764248 | 3566406 | <span style="color:#dc2626">-18.88%</span> |
| 255 | [00950 VIEW_TRIGGER_GENERATED_003](crates/bench/sqlite_parity/cases/SQLITE_PARITY_950_VIEW_TRIGGER_GENERATED_003.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1785869 | 3564863 | <span style="color:#dc2626">-18.83%</span> |
| 256 | [01102 INDEX_SCHEMA_PRAGMA_035](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1102_INDEX_SCHEMA_PRAGMA_035.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1768065 | 3564783 | <span style="color:#dc2626">-18.83%</span> |
| 257 | [00654 JOIN_SUBQUERY_EXISTS_047](crates/bench/sqlite_parity/cases/SQLITE_PARITY_654_JOIN_SUBQUERY_EXISTS_047.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1733048 | 3564623 | <span style="color:#dc2626">-18.82%</span> |
| 258 | [00657 JOIN_SUBQUERY_EXISTS_050](crates/bench/sqlite_parity/cases/SQLITE_PARITY_657_JOIN_SUBQUERY_EXISTS_050.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1730103 | 3564553 | <span style="color:#dc2626">-18.82%</span> |
| 259 | [00977 VIEW_TRIGGER_GENERATED_030](crates/bench/sqlite_parity/cases/SQLITE_PARITY_977_VIEW_TRIGGER_GENERATED_030.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1696810 | 3562880 | <span style="color:#dc2626">-18.76%</span> |
| 260 | [01085 INDEX_SCHEMA_PRAGMA_018](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1085_INDEX_SCHEMA_PRAGMA_018.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1750011 | 3562559 | <span style="color:#dc2626">-18.75%</span> |
| 261 | [00308 SCALAR_ARITH_021](crates/bench/sqlite_parity/cases/SQLITE_PARITY_308_SCALAR_ARITH_021.rs) | P1 | memory | GEN_SQL_SCALAR | 1653818 | 3562248 | <span style="color:#dc2626">-18.74%</span> |
| 262 | [01092 INDEX_SCHEMA_PRAGMA_025](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1092_INDEX_SCHEMA_PRAGMA_025.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 2106625 | 3561277 | <span style="color:#dc2626">-18.71%</span> |
| 263 | [00441 DML_WHERE_ORDER_LIMIT_054](crates/bench/sqlite_parity/cases/SQLITE_PARITY_441_DML_WHERE_ORDER_LIMIT_054.rs) | P1 | memory | GEN_SQL_DML | 1643228 | 3560726 | <span style="color:#dc2626">-18.69%</span> |
| 264 | [00393 DML_WHERE_ORDER_LIMIT_006](crates/bench/sqlite_parity/cases/SQLITE_PARITY_393_DML_WHERE_ORDER_LIMIT_006.rs) | P1 | memory | GEN_SQL_DML | 1862062 | 3560215 | <span style="color:#dc2626">-18.67%</span> |
| 265 | [00696 JOIN_SUBQUERY_EXISTS_089](crates/bench/sqlite_parity/cases/SQLITE_PARITY_696_JOIN_SUBQUERY_EXISTS_089.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1701098 | 3557400 | <span style="color:#dc2626">-18.58%</span> |
| 266 | [00491 DML_WHERE_ORDER_LIMIT_104](crates/bench/sqlite_parity/cases/SQLITE_PARITY_491_DML_WHERE_ORDER_LIMIT_104.rs) | P1 | memory | GEN_SQL_DML | 1717389 | 3556959 | <span style="color:#dc2626">-18.57%</span> |
| 267 | [00438 DML_WHERE_ORDER_LIMIT_051](crates/bench/sqlite_parity/cases/SQLITE_PARITY_438_DML_WHERE_ORDER_LIMIT_051.rs) | P1 | memory | GEN_SQL_DML | 1652807 | 3556478 | <span style="color:#dc2626">-18.55%</span> |
| 268 | [01123 INDEX_SCHEMA_PRAGMA_056](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1123_INDEX_SCHEMA_PRAGMA_056.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1681421 | 3554544 | <span style="color:#dc2626">-18.48%</span> |
| 269 | [00984 VIEW_TRIGGER_GENERATED_037](crates/bench/sqlite_parity/cases/SQLITE_PARITY_984_VIEW_TRIGGER_GENERATED_037.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1778254 | 3553011 | <span style="color:#dc2626">-18.43%</span> |
| 270 | [00627 JOIN_SUBQUERY_EXISTS_020](crates/bench/sqlite_parity/cases/SQLITE_PARITY_627_JOIN_SUBQUERY_EXISTS_020.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1740773 | 3552961 | <span style="color:#dc2626">-18.43%</span> |
| 271 | [01095 INDEX_SCHEMA_PRAGMA_028](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1095_INDEX_SCHEMA_PRAGMA_028.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1745031 | 3551979 | <span style="color:#dc2626">-18.40%</span> |
| 272 | [00315 SCALAR_NULL_COALESCE_022](crates/bench/sqlite_parity/cases/SQLITE_PARITY_315_SCALAR_NULL_COALESCE_022.rs) | P1 | memory | GEN_SQL_SCALAR | 1486892 | 3549535 | <span style="color:#dc2626">-18.32%</span> |
| 273 | [00410 DML_WHERE_ORDER_LIMIT_023](crates/bench/sqlite_parity/cases/SQLITE_PARITY_410_DML_WHERE_ORDER_LIMIT_023.rs) | P1 | memory | GEN_SQL_DML | 1698503 | 3549194 | <span style="color:#dc2626">-18.31%</span> |
| 274 | [01115 INDEX_SCHEMA_PRAGMA_048](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1115_INDEX_SCHEMA_PRAGMA_048.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1961621 | 3548994 | <span style="color:#dc2626">-18.30%</span> |
| 275 | [00937 CONSTRAINT_FK_SAVEPOINT_070](crates/bench/sqlite_parity/cases/SQLITE_PARITY_937_CONSTRAINT_FK_SAVEPOINT_070.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1692151 | 3548192 | <span style="color:#dc2626">-18.27%</span> |
| 276 | [00996 VIEW_TRIGGER_GENERATED_049](crates/bench/sqlite_parity/cases/SQLITE_PARITY_996_VIEW_TRIGGER_GENERATED_049.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1825433 | 3546599 | <span style="color:#dc2626">-18.22%</span> |
| 277 | [01094 INDEX_SCHEMA_PRAGMA_027](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1094_INDEX_SCHEMA_PRAGMA_027.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1726416 | 3545667 | <span style="color:#dc2626">-18.19%</span> |
| 278 | [00017 DELETE_BASIC](crates/bench/sqlite_parity/cases/SQLITE_PARITY_017_DELETE_BASIC.rs) | P0 | memory | SQL_DELETE | 1608452 | 3545628 | <span style="color:#dc2626">-18.19%</span> |
| 279 | [00437 DML_WHERE_ORDER_LIMIT_050](crates/bench/sqlite_parity/cases/SQLITE_PARITY_437_DML_WHERE_ORDER_LIMIT_050.rs) | P1 | memory | GEN_SQL_DML | 1717059 | 3545547 | <span style="color:#dc2626">-18.18%</span> |
| 280 | [00874 CONSTRAINT_FK_SAVEPOINT_007](crates/bench/sqlite_parity/cases/SQLITE_PARITY_874_CONSTRAINT_FK_SAVEPOINT_007.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2425238 | 3544996 | <span style="color:#dc2626">-18.17%</span> |
| 281 | [00989 VIEW_TRIGGER_GENERATED_042](crates/bench/sqlite_parity/cases/SQLITE_PARITY_989_VIEW_TRIGGER_GENERATED_042.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1767003 | 3544856 | <span style="color:#dc2626">-18.16%</span> |
| 282 | [00957 VIEW_TRIGGER_GENERATED_010](crates/bench/sqlite_parity/cases/SQLITE_PARITY_957_VIEW_TRIGGER_GENERATED_010.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1748758 | 3544535 | <span style="color:#dc2626">-18.15%</span> |
| 283 | [00434 DML_WHERE_ORDER_LIMIT_047](crates/bench/sqlite_parity/cases/SQLITE_PARITY_434_DML_WHERE_ORDER_LIMIT_047.rs) | P1 | memory | GEN_SQL_DML | 1890647 | 3544225 | <span style="color:#dc2626">-18.14%</span> |
| 284 | [00525 AGG_GROUP_HAVING_018](crates/bench/sqlite_parity/cases/SQLITE_PARITY_525_AGG_GROUP_HAVING_018.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1807570 | 3544004 | <span style="color:#dc2626">-18.13%</span> |
| 285 | [00625 JOIN_SUBQUERY_EXISTS_018](crates/bench/sqlite_parity/cases/SQLITE_PARITY_625_JOIN_SUBQUERY_EXISTS_018.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1724121 | 3542923 | <span style="color:#dc2626">-18.10%</span> |
| 286 | [00951 VIEW_TRIGGER_GENERATED_004](crates/bench/sqlite_parity/cases/SQLITE_PARITY_951_VIEW_TRIGGER_GENERATED_004.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1838248 | 3540848 | <span style="color:#dc2626">-18.03%</span> |
| 287 | [00620 JOIN_SUBQUERY_EXISTS_013](crates/bench/sqlite_parity/cases/SQLITE_PARITY_620_JOIN_SUBQUERY_EXISTS_013.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1709694 | 3540367 | <span style="color:#dc2626">-18.01%</span> |
| 288 | [01110 INDEX_SCHEMA_PRAGMA_043](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1110_INDEX_SCHEMA_PRAGMA_043.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1713190 | 3539356 | <span style="color:#dc2626">-17.98%</span> |
| 289 | [00635 JOIN_SUBQUERY_EXISTS_028](crates/bench/sqlite_parity/cases/SQLITE_PARITY_635_JOIN_SUBQUERY_EXISTS_028.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2025532 | 3537853 | <span style="color:#dc2626">-17.93%</span> |
| 290 | [01079 INDEX_SCHEMA_PRAGMA_012](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1079_INDEX_SCHEMA_PRAGMA_012.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1737006 | 3536711 | <span style="color:#dc2626">-17.89%</span> |
| 291 | [01069 INDEX_SCHEMA_PRAGMA_002](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1069_INDEX_SCHEMA_PRAGMA_002.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1846152 | 3536620 | <span style="color:#dc2626">-17.89%</span> |
| 292 | [00398 DML_WHERE_ORDER_LIMIT_011](crates/bench/sqlite_parity/cases/SQLITE_PARITY_398_DML_WHERE_ORDER_LIMIT_011.rs) | P1 | memory | GEN_SQL_DML | 1727719 | 3534607 | <span style="color:#dc2626">-17.82%</span> |
| 293 | [00944 CONSTRAINT_FK_SAVEPOINT_077](crates/bench/sqlite_parity/cases/SQLITE_PARITY_944_CONSTRAINT_FK_SAVEPOINT_077.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1728400 | 3533925 | <span style="color:#dc2626">-17.80%</span> |
| 294 | [01125 INDEX_SCHEMA_PRAGMA_058](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1125_INDEX_SCHEMA_PRAGMA_058.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1771031 | 3533425 | <span style="color:#dc2626">-17.78%</span> |
| 295 | [00490 DML_WHERE_ORDER_LIMIT_103](crates/bench/sqlite_parity/cases/SQLITE_PARITY_490_DML_WHERE_ORDER_LIMIT_103.rs) | P1 | memory | GEN_SQL_DML | 1725394 | 3532733 | <span style="color:#dc2626">-17.76%</span> |
| 296 | [00689 JOIN_SUBQUERY_EXISTS_082](crates/bench/sqlite_parity/cases/SQLITE_PARITY_689_JOIN_SUBQUERY_EXISTS_082.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1754078 | 3532132 | <span style="color:#dc2626">-17.74%</span> |
| 297 | [00423 DML_WHERE_ORDER_LIMIT_036](crates/bench/sqlite_parity/cases/SQLITE_PARITY_423_DML_WHERE_ORDER_LIMIT_036.rs) | P1 | memory | GEN_SQL_DML | 1670861 | 3531140 | <span style="color:#dc2626">-17.70%</span> |
| 298 | [00975 VIEW_TRIGGER_GENERATED_028](crates/bench/sqlite_parity/cases/SQLITE_PARITY_975_VIEW_TRIGGER_GENERATED_028.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1953766 | 3530589 | <span style="color:#dc2626">-17.69%</span> |
| 299 | [01004 VIEW_TRIGGER_GENERATED_057](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1004_VIEW_TRIGGER_GENERATED_057.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2429697 | 3528765 | <span style="color:#dc2626">-17.63%</span> |
| 300 | [00336 SCALAR_ARITH_028](crates/bench/sqlite_parity/cases/SQLITE_PARITY_336_SCALAR_ARITH_028.rs) | P1 | memory | GEN_SQL_SCALAR | 1490159 | 3528155 | <span style="color:#dc2626">-17.61%</span> |
| 301 | [00489 DML_WHERE_ORDER_LIMIT_102](crates/bench/sqlite_parity/cases/SQLITE_PARITY_489_DML_WHERE_ORDER_LIMIT_102.rs) | P1 | memory | GEN_SQL_DML | 1806157 | 3528134 | <span style="color:#dc2626">-17.60%</span> |
| 302 | [01068 INDEX_SCHEMA_PRAGMA_001](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1068_INDEX_SCHEMA_PRAGMA_001.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1789265 | 3527273 | <span style="color:#dc2626">-17.58%</span> |
| 303 | [01116 INDEX_SCHEMA_PRAGMA_049](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1116_INDEX_SCHEMA_PRAGMA_049.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1768736 | 3527002 | <span style="color:#dc2626">-17.57%</span> |
| 304 | [00445 DML_WHERE_ORDER_LIMIT_058](crates/bench/sqlite_parity/cases/SQLITE_PARITY_445_DML_WHERE_ORDER_LIMIT_058.rs) | P1 | memory | GEN_SQL_DML | 1654801 | 3526572 | <span style="color:#dc2626">-17.55%</span> |
| 305 | [01088 INDEX_SCHEMA_PRAGMA_021](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1088_INDEX_SCHEMA_PRAGMA_021.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1796378 | 3526181 | <span style="color:#dc2626">-17.54%</span> |
| 306 | [00496 DML_WHERE_ORDER_LIMIT_109](crates/bench/sqlite_parity/cases/SQLITE_PARITY_496_DML_WHERE_ORDER_LIMIT_109.rs) | P1 | memory | GEN_SQL_DML | 1708171 | 3525830 | <span style="color:#dc2626">-17.53%</span> |
| 307 | [00967 VIEW_TRIGGER_GENERATED_020](crates/bench/sqlite_parity/cases/SQLITE_PARITY_967_VIEW_TRIGGER_GENERATED_020.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1844900 | 3525369 | <span style="color:#dc2626">-17.51%</span> |
| 308 | [01027 JSON_EXTRACT_SET_020](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1027_JSON_EXTRACT_SET_020.rs) | P2 | memory | GEN_SQL_JSON | 1640374 | 3522895 | <span style="color:#dc2626">-17.43%</span> |
| 309 | [01009 JSON_EXTRACT_SET_002](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1009_JSON_EXTRACT_SET_002.rs) | P2 | memory | GEN_SQL_JSON | 2246219 | 3522764 | <span style="color:#dc2626">-17.43%</span> |
| 310 | [01071 INDEX_SCHEMA_PRAGMA_004](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1071_INDEX_SCHEMA_PRAGMA_004.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1946533 | 3522644 | <span style="color:#dc2626">-17.42%</span> |
| 311 | [00691 JOIN_SUBQUERY_EXISTS_084](crates/bench/sqlite_parity/cases/SQLITE_PARITY_691_JOIN_SUBQUERY_EXISTS_084.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1691230 | 3522624 | <span style="color:#dc2626">-17.42%</span> |
| 312 | [01100 INDEX_SCHEMA_PRAGMA_033](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1100_INDEX_SCHEMA_PRAGMA_033.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1758887 | 3521452 | <span style="color:#dc2626">-17.38%</span> |
| 313 | [00458 DML_WHERE_ORDER_LIMIT_071](crates/bench/sqlite_parity/cases/SQLITE_PARITY_458_DML_WHERE_ORDER_LIMIT_071.rs) | P1 | memory | GEN_SQL_DML | 2051911 | 3521261 | <span style="color:#dc2626">-17.38%</span> |
| 314 | [00652 JOIN_SUBQUERY_EXISTS_045](crates/bench/sqlite_parity/cases/SQLITE_PARITY_652_JOIN_SUBQUERY_EXISTS_045.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1775729 | 3521131 | <span style="color:#dc2626">-17.37%</span> |
| 315 | [00502 DML_WHERE_ORDER_LIMIT_115](crates/bench/sqlite_parity/cases/SQLITE_PARITY_502_DML_WHERE_ORDER_LIMIT_115.rs) | P1 | memory | GEN_SQL_DML | 1648258 | 3520730 | <span style="color:#dc2626">-17.36%</span> |
| 316 | [00700 JOIN_SUBQUERY_EXISTS_093](crates/bench/sqlite_parity/cases/SQLITE_PARITY_700_JOIN_SUBQUERY_EXISTS_093.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2501011 | 3520129 | <span style="color:#dc2626">-17.34%</span> |
| 317 | [00414 DML_WHERE_ORDER_LIMIT_027](crates/bench/sqlite_parity/cases/SQLITE_PARITY_414_DML_WHERE_ORDER_LIMIT_027.rs) | P1 | memory | GEN_SQL_DML | 1717298 | 3519548 | <span style="color:#dc2626">-17.32%</span> |
| 318 | [00897 CONSTRAINT_FK_SAVEPOINT_030](crates/bench/sqlite_parity/cases/SQLITE_PARITY_897_CONSTRAINT_FK_SAVEPOINT_030.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1735082 | 3518987 | <span style="color:#dc2626">-17.30%</span> |
| 319 | [00474 DML_WHERE_ORDER_LIMIT_087](crates/bench/sqlite_parity/cases/SQLITE_PARITY_474_DML_WHERE_ORDER_LIMIT_087.rs) | P1 | memory | GEN_SQL_DML | 2061620 | 3518927 | <span style="color:#dc2626">-17.30%</span> |
| 320 | [00900 CONSTRAINT_FK_SAVEPOINT_033](crates/bench/sqlite_parity/cases/SQLITE_PARITY_900_CONSTRAINT_FK_SAVEPOINT_033.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1714493 | 3518856 | <span style="color:#dc2626">-17.30%</span> |
| 321 | [00004 TABLE_CONSTRAINTS_SUCCESS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_004_TABLE_CONSTRAINTS_SUCCESS.rs) | P0 | memory | SQL_CONSTRAINTS | 2138004 | 3517724 | <span style="color:#dc2626">-17.26%</span> |
| 322 | [00998 VIEW_TRIGGER_GENERATED_051](crates/bench/sqlite_parity/cases/SQLITE_PARITY_998_VIEW_TRIGGER_GENERATED_051.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1957393 | 3516843 | <span style="color:#dc2626">-17.23%</span> |
| 323 | [00467 DML_WHERE_ORDER_LIMIT_080](crates/bench/sqlite_parity/cases/SQLITE_PARITY_467_DML_WHERE_ORDER_LIMIT_080.rs) | P1 | memory | GEN_SQL_DML | 2113437 | 3515140 | <span style="color:#dc2626">-17.17%</span> |
| 324 | [00475 DML_WHERE_ORDER_LIMIT_088](crates/bench/sqlite_parity/cases/SQLITE_PARITY_475_DML_WHERE_ORDER_LIMIT_088.rs) | P1 | memory | GEN_SQL_DML | 1656924 | 3514869 | <span style="color:#dc2626">-17.16%</span> |
| 325 | [00983 VIEW_TRIGGER_GENERATED_036](crates/bench/sqlite_parity/cases/SQLITE_PARITY_983_VIEW_TRIGGER_GENERATED_036.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1996457 | 3514258 | <span style="color:#dc2626">-17.14%</span> |
| 326 | [01120 INDEX_SCHEMA_PRAGMA_053](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1120_INDEX_SCHEMA_PRAGMA_053.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1742998 | 3514108 | <span style="color:#dc2626">-17.14%</span> |
| 327 | [00526 AGG_GROUP_HAVING_019](crates/bench/sqlite_parity/cases/SQLITE_PARITY_526_AGG_GROUP_HAVING_019.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1787411 | 3513847 | <span style="color:#dc2626">-17.13%</span> |
| 328 | [00624 JOIN_SUBQUERY_EXISTS_017](crates/bench/sqlite_parity/cases/SQLITE_PARITY_624_JOIN_SUBQUERY_EXISTS_017.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2531999 | 3511734 | <span style="color:#dc2626">-17.06%</span> |
| 329 | [00487 DML_WHERE_ORDER_LIMIT_100](crates/bench/sqlite_parity/cases/SQLITE_PARITY_487_DML_WHERE_ORDER_LIMIT_100.rs) | P1 | memory | GEN_SQL_DML | 1976789 | 3510591 | <span style="color:#dc2626">-17.02%</span> |
| 330 | [00550 AGG_GROUP_HAVING_043](crates/bench/sqlite_parity/cases/SQLITE_PARITY_550_AGG_GROUP_HAVING_043.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1717649 | 3510551 | <span style="color:#dc2626">-17.02%</span> |
| 331 | [00492 DML_WHERE_ORDER_LIMIT_105](crates/bench/sqlite_parity/cases/SQLITE_PARITY_492_DML_WHERE_ORDER_LIMIT_105.rs) | P1 | memory | GEN_SQL_DML | 1637397 | 3510190 | <span style="color:#dc2626">-17.01%</span> |
| 332 | [00443 DML_WHERE_ORDER_LIMIT_056](crates/bench/sqlite_parity/cases/SQLITE_PARITY_443_DML_WHERE_ORDER_LIMIT_056.rs) | P1 | memory | GEN_SQL_DML | 1639381 | 3509279 | <span style="color:#dc2626">-16.98%</span> |
| 333 | [00955 VIEW_TRIGGER_GENERATED_008](crates/bench/sqlite_parity/cases/SQLITE_PARITY_955_VIEW_TRIGGER_GENERATED_008.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2015993 | 3509058 | <span style="color:#dc2626">-16.97%</span> |
| 334 | [00456 DML_WHERE_ORDER_LIMIT_069](crates/bench/sqlite_parity/cases/SQLITE_PARITY_456_DML_WHERE_ORDER_LIMIT_069.rs) | P1 | memory | GEN_SQL_DML | 1682744 | 3508758 | <span style="color:#dc2626">-16.96%</span> |
| 335 | [00703 JOIN_SUBQUERY_EXISTS_096](crates/bench/sqlite_parity/cases/SQLITE_PARITY_703_JOIN_SUBQUERY_EXISTS_096.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1671272 | 3507956 | <span style="color:#dc2626">-16.93%</span> |
| 336 | [00986 VIEW_TRIGGER_GENERATED_039](crates/bench/sqlite_parity/cases/SQLITE_PARITY_986_VIEW_TRIGGER_GENERATED_039.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2108829 | 3507836 | <span style="color:#dc2626">-16.93%</span> |
| 337 | [00997 VIEW_TRIGGER_GENERATED_050](crates/bench/sqlite_parity/cases/SQLITE_PARITY_997_VIEW_TRIGGER_GENERATED_050.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1968093 | 3507325 | <span style="color:#dc2626">-16.91%</span> |
| 338 | [01010 JSON_EXTRACT_SET_003](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1010_JSON_EXTRACT_SET_003.rs) | P2 | memory | GEN_SQL_JSON | 1625916 | 3506985 | <span style="color:#dc2626">-16.90%</span> |
| 339 | [00985 VIEW_TRIGGER_GENERATED_038](crates/bench/sqlite_parity/cases/SQLITE_PARITY_985_VIEW_TRIGGER_GENERATED_038.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1808581 | 3505652 | <span style="color:#dc2626">-16.86%</span> |
| 340 | [00473 DML_WHERE_ORDER_LIMIT_086](crates/bench/sqlite_parity/cases/SQLITE_PARITY_473_DML_WHERE_ORDER_LIMIT_086.rs) | P1 | memory | GEN_SQL_DML | 1689066 | 3505331 | <span style="color:#dc2626">-16.84%</span> |
| 341 | [00633 JOIN_SUBQUERY_EXISTS_026](crates/bench/sqlite_parity/cases/SQLITE_PARITY_633_JOIN_SUBQUERY_EXISTS_026.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1711147 | 3504209 | <span style="color:#dc2626">-16.81%</span> |
| 342 | [00933 CONSTRAINT_FK_SAVEPOINT_066](crates/bench/sqlite_parity/cases/SQLITE_PARITY_933_CONSTRAINT_FK_SAVEPOINT_066.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1963775 | 3503989 | <span style="color:#dc2626">-16.80%</span> |
| 343 | [00485 DML_WHERE_ORDER_LIMIT_098](crates/bench/sqlite_parity/cases/SQLITE_PARITY_485_DML_WHERE_ORDER_LIMIT_098.rs) | P1 | memory | GEN_SQL_DML | 1731055 | 3503628 | <span style="color:#dc2626">-16.79%</span> |
| 344 | [01007 VIEW_TRIGGER_GENERATED_060](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1007_VIEW_TRIGGER_GENERATED_060.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1783454 | 3503077 | <span style="color:#dc2626">-16.77%</span> |
| 345 | [00446 DML_WHERE_ORDER_LIMIT_059](crates/bench/sqlite_parity/cases/SQLITE_PARITY_446_DML_WHERE_ORDER_LIMIT_059.rs) | P1 | memory | GEN_SQL_DML | 1684126 | 3502716 | <span style="color:#dc2626">-16.76%</span> |
| 346 | [00995 VIEW_TRIGGER_GENERATED_048](crates/bench/sqlite_parity/cases/SQLITE_PARITY_995_VIEW_TRIGGER_GENERATED_048.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2193549 | 3502536 | <span style="color:#dc2626">-16.75%</span> |
| 347 | [00424 DML_WHERE_ORDER_LIMIT_037](crates/bench/sqlite_parity/cases/SQLITE_PARITY_424_DML_WHERE_ORDER_LIMIT_037.rs) | P1 | memory | GEN_SQL_DML | 1723290 | 3502065 | <span style="color:#dc2626">-16.74%</span> |
| 348 | [00439 DML_WHERE_ORDER_LIMIT_052](crates/bench/sqlite_parity/cases/SQLITE_PARITY_439_DML_WHERE_ORDER_LIMIT_052.rs) | P1 | memory | GEN_SQL_DML | 1684015 | 3500241 | <span style="color:#dc2626">-16.67%</span> |
| 349 | [01098 INDEX_SCHEMA_PRAGMA_031](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1098_INDEX_SCHEMA_PRAGMA_031.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 2067832 | 3500232 | <span style="color:#dc2626">-16.67%</span> |
| 350 | [00988 VIEW_TRIGGER_GENERATED_041](crates/bench/sqlite_parity/cases/SQLITE_PARITY_988_VIEW_TRIGGER_GENERATED_041.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1755070 | 3500011 | <span style="color:#dc2626">-16.67%</span> |
| 351 | [00394 DML_WHERE_ORDER_LIMIT_007](crates/bench/sqlite_parity/cases/SQLITE_PARITY_394_DML_WHERE_ORDER_LIMIT_007.rs) | P1 | memory | GEN_SQL_DML | 1805556 | 3499340 | <span style="color:#dc2626">-16.64%</span> |
| 352 | [00497 DML_WHERE_ORDER_LIMIT_110](crates/bench/sqlite_parity/cases/SQLITE_PARITY_497_DML_WHERE_ORDER_LIMIT_110.rs) | P1 | memory | GEN_SQL_DML | 2293689 | 3498178 | <span style="color:#dc2626">-16.61%</span> |
| 353 | [00472 DML_WHERE_ORDER_LIMIT_085](crates/bench/sqlite_parity/cases/SQLITE_PARITY_472_DML_WHERE_ORDER_LIMIT_085.rs) | P1 | memory | GEN_SQL_DML | 1670230 | 3498067 | <span style="color:#dc2626">-16.60%</span> |
| 354 | [00470 DML_WHERE_ORDER_LIMIT_083](crates/bench/sqlite_parity/cases/SQLITE_PARITY_470_DML_WHERE_ORDER_LIMIT_083.rs) | P1 | memory | GEN_SQL_DML | 1641666 | 3497356 | <span style="color:#dc2626">-16.58%</span> |
| 355 | [00461 DML_WHERE_ORDER_LIMIT_074](crates/bench/sqlite_parity/cases/SQLITE_PARITY_461_DML_WHERE_ORDER_LIMIT_074.rs) | P1 | memory | GEN_SQL_DML | 1727167 | 3496685 | <span style="color:#dc2626">-16.56%</span> |
| 356 | [00429 DML_WHERE_ORDER_LIMIT_042](crates/bench/sqlite_parity/cases/SQLITE_PARITY_429_DML_WHERE_ORDER_LIMIT_042.rs) | P1 | memory | GEN_SQL_DML | 1633540 | 3496645 | <span style="color:#dc2626">-16.55%</span> |
| 357 | [00466 DML_WHERE_ORDER_LIMIT_079](crates/bench/sqlite_parity/cases/SQLITE_PARITY_466_DML_WHERE_ORDER_LIMIT_079.rs) | P1 | memory | GEN_SQL_DML | 1723641 | 3496484 | <span style="color:#dc2626">-16.55%</span> |
| 358 | [01127 INDEX_SCHEMA_PRAGMA_060](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1127_INDEX_SCHEMA_PRAGMA_060.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 2121744 | 3494070 | <span style="color:#dc2626">-16.47%</span> |
| 359 | [01108 INDEX_SCHEMA_PRAGMA_041](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1108_INDEX_SCHEMA_PRAGMA_041.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1760741 | 3492557 | <span style="color:#dc2626">-16.42%</span> |
| 360 | [00656 JOIN_SUBQUERY_EXISTS_049](crates/bench/sqlite_parity/cases/SQLITE_PARITY_656_JOIN_SUBQUERY_EXISTS_049.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1716618 | 3492206 | <span style="color:#dc2626">-16.41%</span> |
| 361 | [00480 DML_WHERE_ORDER_LIMIT_093](crates/bench/sqlite_parity/cases/SQLITE_PARITY_480_DML_WHERE_ORDER_LIMIT_093.rs) | P1 | memory | GEN_SQL_DML | 1555353 | 3491746 | <span style="color:#dc2626">-16.39%</span> |
| 362 | [00963 VIEW_TRIGGER_GENERATED_016](crates/bench/sqlite_parity/cases/SQLITE_PARITY_963_VIEW_TRIGGER_GENERATED_016.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1783022 | 3490884 | <span style="color:#dc2626">-16.36%</span> |
| 363 | [00994 VIEW_TRIGGER_GENERATED_047](crates/bench/sqlite_parity/cases/SQLITE_PARITY_994_VIEW_TRIGGER_GENERATED_047.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1888312 | 3490723 | <span style="color:#dc2626">-16.36%</span> |
| 364 | [00054 JOINS_INNER_LEFT_CROSS_NATURAL](crates/bench/sqlite_parity/cases/SQLITE_PARITY_054_JOINS_INNER_LEFT_CROSS_NATURAL.rs) | P0 | memory | SQL_JOIN | 1729632 | 3489311 | <span style="color:#dc2626">-16.31%</span> |
| 365 | [00953 VIEW_TRIGGER_GENERATED_006](crates/bench/sqlite_parity/cases/SQLITE_PARITY_953_VIEW_TRIGGER_GENERATED_006.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1754719 | 3489251 | <span style="color:#dc2626">-16.31%</span> |
| 366 | [00965 VIEW_TRIGGER_GENERATED_018](crates/bench/sqlite_parity/cases/SQLITE_PARITY_965_VIEW_TRIGGER_GENERATED_018.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1823680 | 3487698 | <span style="color:#dc2626">-16.26%</span> |
| 367 | [00960 VIEW_TRIGGER_GENERATED_013](crates/bench/sqlite_parity/cases/SQLITE_PARITY_960_VIEW_TRIGGER_GENERATED_013.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2010763 | 3486625 | <span style="color:#dc2626">-16.22%</span> |
| 368 | [00413 DML_WHERE_ORDER_LIMIT_026](crates/bench/sqlite_parity/cases/SQLITE_PARITY_413_DML_WHERE_ORDER_LIMIT_026.rs) | P1 | memory | GEN_SQL_DML | 2030912 | 3486606 | <span style="color:#dc2626">-16.22%</span> |
| 369 | [00949 VIEW_TRIGGER_GENERATED_002](crates/bench/sqlite_parity/cases/SQLITE_PARITY_949_VIEW_TRIGGER_GENERATED_002.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1983242 | 3486375 | <span style="color:#dc2626">-16.21%</span> |
| 370 | [00869 CONSTRAINT_FK_SAVEPOINT_002](crates/bench/sqlite_parity/cases/SQLITE_PARITY_869_CONSTRAINT_FK_SAVEPOINT_002.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1721717 | 3483911 | <span style="color:#dc2626">-16.13%</span> |
| 371 | [00402 DML_WHERE_ORDER_LIMIT_015](crates/bench/sqlite_parity/cases/SQLITE_PARITY_402_DML_WHERE_ORDER_LIMIT_015.rs) | P1 | memory | GEN_SQL_DML | 1701409 | 3483891 | <span style="color:#dc2626">-16.13%</span> |
| 372 | [00444 DML_WHERE_ORDER_LIMIT_057](crates/bench/sqlite_parity/cases/SQLITE_PARITY_444_DML_WHERE_ORDER_LIMIT_057.rs) | P1 | memory | GEN_SQL_DML | 1679938 | 3483240 | <span style="color:#dc2626">-16.11%</span> |
| 373 | [00991 VIEW_TRIGGER_GENERATED_044](crates/bench/sqlite_parity/cases/SQLITE_PARITY_991_VIEW_TRIGGER_GENERATED_044.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1770449 | 3480935 | <span style="color:#dc2626">-16.03%</span> |
| 374 | [00992 VIEW_TRIGGER_GENERATED_045](crates/bench/sqlite_parity/cases/SQLITE_PARITY_992_VIEW_TRIGGER_GENERATED_045.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1698513 | 3479442 | <span style="color:#dc2626">-15.98%</span> |
| 375 | [01080 INDEX_SCHEMA_PRAGMA_013](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1080_INDEX_SCHEMA_PRAGMA_013.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 2127414 | 3479402 | <span style="color:#dc2626">-15.98%</span> |
| 376 | [01101 INDEX_SCHEMA_PRAGMA_034](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1101_INDEX_SCHEMA_PRAGMA_034.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1804794 | 3478010 | <span style="color:#dc2626">-15.93%</span> |
| 377 | [00958 VIEW_TRIGGER_GENERATED_011](crates/bench/sqlite_parity/cases/SQLITE_PARITY_958_VIEW_TRIGGER_GENERATED_011.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2379692 | 3477298 | <span style="color:#dc2626">-15.91%</span> |
| 378 | [01103 INDEX_SCHEMA_PRAGMA_036](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1103_INDEX_SCHEMA_PRAGMA_036.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1764728 | 3476888 | <span style="color:#dc2626">-15.90%</span> |
| 379 | [00419 DML_WHERE_ORDER_LIMIT_032](crates/bench/sqlite_parity/cases/SQLITE_PARITY_419_DML_WHERE_ORDER_LIMIT_032.rs) | P1 | memory | GEN_SQL_DML | 1664358 | 3475365 | <span style="color:#dc2626">-15.85%</span> |
| 380 | [00465 DML_WHERE_ORDER_LIMIT_078](crates/bench/sqlite_parity/cases/SQLITE_PARITY_465_DML_WHERE_ORDER_LIMIT_078.rs) | P1 | memory | GEN_SQL_DML | 1819832 | 3474202 | <span style="color:#dc2626">-15.81%</span> |
| 381 | [00531 AGG_GROUP_HAVING_024](crates/bench/sqlite_parity/cases/SQLITE_PARITY_531_AGG_GROUP_HAVING_024.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1682433 | 3474052 | <span style="color:#dc2626">-15.80%</span> |
| 382 | [01005 VIEW_TRIGGER_GENERATED_058](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1005_VIEW_TRIGGER_GENERATED_058.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1759077 | 3472640 | <span style="color:#dc2626">-15.75%</span> |
| 383 | [01087 INDEX_SCHEMA_PRAGMA_020](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1087_INDEX_SCHEMA_PRAGMA_020.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1758848 | 3472429 | <span style="color:#dc2626">-15.75%</span> |
| 384 | [00422 DML_WHERE_ORDER_LIMIT_035](crates/bench/sqlite_parity/cases/SQLITE_PARITY_422_DML_WHERE_ORDER_LIMIT_035.rs) | P1 | memory | GEN_SQL_DML | 1702290 | 3472138 | <span style="color:#dc2626">-15.74%</span> |
| 385 | [00432 DML_WHERE_ORDER_LIMIT_045](crates/bench/sqlite_parity/cases/SQLITE_PARITY_432_DML_WHERE_ORDER_LIMIT_045.rs) | P1 | memory | GEN_SQL_DML | 1625866 | 3471176 | <span style="color:#dc2626">-15.71%</span> |
| 386 | [00486 DML_WHERE_ORDER_LIMIT_099](crates/bench/sqlite_parity/cases/SQLITE_PARITY_486_DML_WHERE_ORDER_LIMIT_099.rs) | P1 | memory | GEN_SQL_DML | 1689156 | 3470425 | <span style="color:#dc2626">-15.68%</span> |
| 387 | [00389 DML_WHERE_ORDER_LIMIT_002](crates/bench/sqlite_parity/cases/SQLITE_PARITY_389_DML_WHERE_ORDER_LIMIT_002.rs) | P1 | memory | GEN_SQL_DML | 1691089 | 3469743 | <span style="color:#dc2626">-15.66%</span> |
| 388 | [00001 SELECT_CORE_EXPRESSIONS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_001_SELECT_CORE_EXPRESSIONS.rs) | P0 | memory | SQL_SELECT | 1641906 | 3468822 | <span style="color:#dc2626">-15.63%</span> |
| 389 | [01113 INDEX_SCHEMA_PRAGMA_046](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1113_INDEX_SCHEMA_PRAGMA_046.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1813931 | 3468652 | <span style="color:#dc2626">-15.62%</span> |
| 390 | [00449 DML_WHERE_ORDER_LIMIT_062](crates/bench/sqlite_parity/cases/SQLITE_PARITY_449_DML_WHERE_ORDER_LIMIT_062.rs) | P1 | memory | GEN_SQL_DML | 1665220 | 3467891 | <span style="color:#dc2626">-15.60%</span> |
| 391 | [00602 AGG_GROUP_HAVING_095](crates/bench/sqlite_parity/cases/SQLITE_PARITY_602_AGG_GROUP_HAVING_095.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1767243 | 3467189 | <span style="color:#dc2626">-15.57%</span> |
| 392 | [01075 INDEX_SCHEMA_PRAGMA_008](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1075_INDEX_SCHEMA_PRAGMA_008.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1694275 | 3465656 | <span style="color:#dc2626">-15.52%</span> |
| 393 | [00418 DML_WHERE_ORDER_LIMIT_031](crates/bench/sqlite_parity/cases/SQLITE_PARITY_418_DML_WHERE_ORDER_LIMIT_031.rs) | P1 | memory | GEN_SQL_DML | 1682873 | 3464945 | <span style="color:#dc2626">-15.50%</span> |
| 394 | [00412 DML_WHERE_ORDER_LIMIT_025](crates/bench/sqlite_parity/cases/SQLITE_PARITY_412_DML_WHERE_ORDER_LIMIT_025.rs) | P1 | memory | GEN_SQL_DML | 1676131 | 3460767 | <span style="color:#dc2626">-15.36%</span> |
| 395 | [00824 WINDOW_PARTITION_SUM_037](crates/bench/sqlite_parity/cases/SQLITE_PARITY_824_WINDOW_PARTITION_SUM_037.rs) | P2 | memory | GEN_SQL_WINDOW | 1687693 | 3460005 | <span style="color:#dc2626">-15.33%</span> |
| 396 | [00879 CONSTRAINT_FK_SAVEPOINT_012](crates/bench/sqlite_parity/cases/SQLITE_PARITY_879_CONSTRAINT_FK_SAVEPOINT_012.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1679126 | 3459244 | <span style="color:#dc2626">-15.31%</span> |
| 397 | [00966 VIEW_TRIGGER_GENERATED_019](crates/bench/sqlite_parity/cases/SQLITE_PARITY_966_VIEW_TRIGGER_GENERATED_019.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1788113 | 3458883 | <span style="color:#dc2626">-15.30%</span> |
| 398 | [00501 DML_WHERE_ORDER_LIMIT_114](crates/bench/sqlite_parity/cases/SQLITE_PARITY_501_DML_WHERE_ORDER_LIMIT_114.rs) | P1 | memory | GEN_SQL_DML | 2902651 | 3457701 | <span style="color:#dc2626">-15.26%</span> |
| 399 | [01046 JSON_EXTRACT_SET_039](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1046_JSON_EXTRACT_SET_039.rs) | P2 | memory | GEN_SQL_JSON | 2011205 | 3457240 | <span style="color:#dc2626">-15.24%</span> |
| 400 | [00553 AGG_GROUP_HAVING_046](crates/bench/sqlite_parity/cases/SQLITE_PARITY_553_AGG_GROUP_HAVING_046.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1788343 | 3456779 | <span style="color:#dc2626">-15.23%</span> |
| 401 | [01105 INDEX_SCHEMA_PRAGMA_038](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1105_INDEX_SCHEMA_PRAGMA_038.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1747686 | 3456479 | <span style="color:#dc2626">-15.22%</span> |
| 402 | [00908 CONSTRAINT_FK_SAVEPOINT_041](crates/bench/sqlite_parity/cases/SQLITE_PARITY_908_CONSTRAINT_FK_SAVEPOINT_041.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1702501 | 3455407 | <span style="color:#dc2626">-15.18%</span> |
| 403 | [00524 AGG_GROUP_HAVING_017](crates/bench/sqlite_parity/cases/SQLITE_PARITY_524_AGG_GROUP_HAVING_017.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1704104 | 3454344 | <span style="color:#dc2626">-15.14%</span> |
| 404 | [00453 DML_WHERE_ORDER_LIMIT_066](crates/bench/sqlite_parity/cases/SQLITE_PARITY_453_DML_WHERE_ORDER_LIMIT_066.rs) | P1 | memory | GEN_SQL_DML | 1649440 | 3449425 | <span style="color:#dc2626">-14.98%</span> |
| 405 | [00554 AGG_GROUP_HAVING_047](crates/bench/sqlite_parity/cases/SQLITE_PARITY_554_AGG_GROUP_HAVING_047.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1693012 | 3447672 | <span style="color:#dc2626">-14.92%</span> |
| 406 | [00954 VIEW_TRIGGER_GENERATED_007](crates/bench/sqlite_parity/cases/SQLITE_PARITY_954_VIEW_TRIGGER_GENERATED_007.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1791179 | 3447471 | <span style="color:#dc2626">-14.92%</span> |
| 407 | [00607 AGG_GROUP_HAVING_100](crates/bench/sqlite_parity/cases/SQLITE_PARITY_607_AGG_GROUP_HAVING_100.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1779676 | 3446590 | <span style="color:#dc2626">-14.89%</span> |
| 408 | [00217 DETACH_DATABASE_SYNTAX](crates/bench/sqlite_parity/cases/SQLITE_PARITY_217_DETACH_DATABASE_SYNTAX.rs) | P0 | memory | SQL_ATTACH | 1697872 | 3446509 | <span style="color:#dc2626">-14.88%</span> |
| 409 | [00471 DML_WHERE_ORDER_LIMIT_084](crates/bench/sqlite_parity/cases/SQLITE_PARITY_471_DML_WHERE_ORDER_LIMIT_084.rs) | P1 | memory | GEN_SQL_DML | 2177700 | 3445998 | <span style="color:#dc2626">-14.87%</span> |
| 410 | [01119 INDEX_SCHEMA_PRAGMA_052](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1119_INDEX_SCHEMA_PRAGMA_052.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1767824 | 3444727 | <span style="color:#dc2626">-14.82%</span> |
| 411 | [00844 WINDOW_PARTITION_SUM_057](crates/bench/sqlite_parity/cases/SQLITE_PARITY_844_WINDOW_PARTITION_SUM_057.rs) | P2 | memory | GEN_SQL_WINDOW | 1801629 | 3444426 | <span style="color:#dc2626">-14.81%</span> |
| 412 | [00415 DML_WHERE_ORDER_LIMIT_028](crates/bench/sqlite_parity/cases/SQLITE_PARITY_415_DML_WHERE_ORDER_LIMIT_028.rs) | P1 | memory | GEN_SQL_DML | 2348082 | 3443865 | <span style="color:#dc2626">-14.80%</span> |
| 413 | [00962 VIEW_TRIGGER_GENERATED_015](crates/bench/sqlite_parity/cases/SQLITE_PARITY_962_VIEW_TRIGGER_GENERATED_015.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1761512 | 3443524 | <span style="color:#dc2626">-14.78%</span> |
| 414 | [01072 INDEX_SCHEMA_PRAGMA_005](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1072_INDEX_SCHEMA_PRAGMA_005.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1727207 | 3442893 | <span style="color:#dc2626">-14.76%</span> |
| 415 | [00442 DML_WHERE_ORDER_LIMIT_055](crates/bench/sqlite_parity/cases/SQLITE_PARITY_442_DML_WHERE_ORDER_LIMIT_055.rs) | P1 | memory | GEN_SQL_DML | 1689776 | 3441942 | <span style="color:#dc2626">-14.73%</span> |
| 416 | [00417 DML_WHERE_ORDER_LIMIT_030](crates/bench/sqlite_parity/cases/SQLITE_PARITY_417_DML_WHERE_ORDER_LIMIT_030.rs) | P1 | memory | GEN_SQL_DML | 1895125 | 3440208 | <span style="color:#dc2626">-14.67%</span> |
| 417 | [01097 INDEX_SCHEMA_PRAGMA_030](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1097_INDEX_SCHEMA_PRAGMA_030.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 2071819 | 3439897 | <span style="color:#dc2626">-14.66%</span> |
| 418 | [00788 WINDOW_PARTITION_SUM_001](crates/bench/sqlite_parity/cases/SQLITE_PARITY_788_WINDOW_PARTITION_SUM_001.rs) | P2 | memory | GEN_SQL_WINDOW | 1672263 | 3438515 | <span style="color:#dc2626">-14.62%</span> |
| 419 | [00946 CONSTRAINT_FK_SAVEPOINT_079](crates/bench/sqlite_parity/cases/SQLITE_PARITY_946_CONSTRAINT_FK_SAVEPOINT_079.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1754800 | 3438214 | <span style="color:#dc2626">-14.61%</span> |
| 420 | [00448 DML_WHERE_ORDER_LIMIT_061](crates/bench/sqlite_parity/cases/SQLITE_PARITY_448_DML_WHERE_ORDER_LIMIT_061.rs) | P1 | memory | GEN_SQL_DML | 2292056 | 3436521 | <span style="color:#dc2626">-14.55%</span> |
| 421 | [00508 AGG_GROUP_HAVING_001](crates/bench/sqlite_parity/cases/SQLITE_PARITY_508_AGG_GROUP_HAVING_001.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1922306 | 3436331 | <span style="color:#dc2626">-14.54%</span> |
| 422 | [00463 DML_WHERE_ORDER_LIMIT_076](crates/bench/sqlite_parity/cases/SQLITE_PARITY_463_DML_WHERE_ORDER_LIMIT_076.rs) | P1 | memory | GEN_SQL_DML | 1572725 | 3435810 | <span style="color:#dc2626">-14.53%</span> |
| 423 | [00401 DML_WHERE_ORDER_LIMIT_014](crates/bench/sqlite_parity/cases/SQLITE_PARITY_401_DML_WHERE_ORDER_LIMIT_014.rs) | P1 | memory | GEN_SQL_DML | 1622840 | 3435499 | <span style="color:#dc2626">-14.52%</span> |
| 424 | [00395 DML_WHERE_ORDER_LIMIT_008](crates/bench/sqlite_parity/cases/SQLITE_PARITY_395_DML_WHERE_ORDER_LIMIT_008.rs) | P1 | memory | GEN_SQL_DML | 1674007 | 3434087 | <span style="color:#dc2626">-14.47%</span> |
| 425 | [00606 AGG_GROUP_HAVING_099](crates/bench/sqlite_parity/cases/SQLITE_PARITY_606_AGG_GROUP_HAVING_099.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1781600 | 3433956 | <span style="color:#dc2626">-14.47%</span> |
| 426 | [00406 DML_WHERE_ORDER_LIMIT_019](crates/bench/sqlite_parity/cases/SQLITE_PARITY_406_DML_WHERE_ORDER_LIMIT_019.rs) | P1 | memory | GEN_SQL_DML | 1702371 | 3433896 | <span style="color:#dc2626">-14.46%</span> |
| 427 | [00961 VIEW_TRIGGER_GENERATED_014](crates/bench/sqlite_parity/cases/SQLITE_PARITY_961_VIEW_TRIGGER_GENERATED_014.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1771642 | 3433645 | <span style="color:#dc2626">-14.45%</span> |
| 428 | [00488 DML_WHERE_ORDER_LIMIT_101](crates/bench/sqlite_parity/cases/SQLITE_PARITY_488_DML_WHERE_ORDER_LIMIT_101.rs) | P1 | memory | GEN_SQL_DML | 1872603 | 3433456 | <span style="color:#dc2626">-14.45%</span> |
| 429 | [00500 DML_WHERE_ORDER_LIMIT_113](crates/bench/sqlite_parity/cases/SQLITE_PARITY_500_DML_WHERE_ORDER_LIMIT_113.rs) | P1 | memory | GEN_SQL_DML | 1667775 | 3432894 | <span style="color:#dc2626">-14.43%</span> |
| 430 | [00396 DML_WHERE_ORDER_LIMIT_009](crates/bench/sqlite_parity/cases/SQLITE_PARITY_396_DML_WHERE_ORDER_LIMIT_009.rs) | P1 | memory | GEN_SQL_DML | 1928067 | 3432213 | <span style="color:#dc2626">-14.41%</span> |
| 431 | [00428 DML_WHERE_ORDER_LIMIT_041](crates/bench/sqlite_parity/cases/SQLITE_PARITY_428_DML_WHERE_ORDER_LIMIT_041.rs) | P1 | memory | GEN_SQL_DML | 2062872 | 3431712 | <span style="color:#dc2626">-14.39%</span> |
| 432 | [00927 CONSTRAINT_FK_SAVEPOINT_060](crates/bench/sqlite_parity/cases/SQLITE_PARITY_927_CONSTRAINT_FK_SAVEPOINT_060.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1932917 | 3429718 | <span style="color:#dc2626">-14.32%</span> |
| 433 | [00972 VIEW_TRIGGER_GENERATED_025](crates/bench/sqlite_parity/cases/SQLITE_PARITY_972_VIEW_TRIGGER_GENERATED_025.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1720816 | 3429588 | <span style="color:#dc2626">-14.32%</span> |
| 434 | [00425 DML_WHERE_ORDER_LIMIT_038](crates/bench/sqlite_parity/cases/SQLITE_PARITY_425_DML_WHERE_ORDER_LIMIT_038.rs) | P1 | memory | GEN_SQL_DML | 1635143 | 3429267 | <span style="color:#dc2626">-14.31%</span> |
| 435 | [00205 OPT_VFS_CATALOG](crates/bench/sqlite_parity/cases/SQLITE_PARITY_205_OPT_VFS_CATALOG.rs) | P4 | catalog | CLI_OPTION_CATALOG | 1454722 | 3427704 | <span style="color:#dc2626">-14.26%</span> |
| 436 | [00914 CONSTRAINT_FK_SAVEPOINT_047](crates/bench/sqlite_parity/cases/SQLITE_PARITY_914_CONSTRAINT_FK_SAVEPOINT_047.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1783884 | 3426712 | <span style="color:#dc2626">-14.22%</span> |
| 437 | [00498 DML_WHERE_ORDER_LIMIT_111](crates/bench/sqlite_parity/cases/SQLITE_PARITY_498_DML_WHERE_ORDER_LIMIT_111.rs) | P1 | memory | GEN_SQL_DML | 1886479 | 3426121 | <span style="color:#dc2626">-14.20%</span> |
| 438 | [00493 DML_WHERE_ORDER_LIMIT_106](crates/bench/sqlite_parity/cases/SQLITE_PARITY_493_DML_WHERE_ORDER_LIMIT_106.rs) | P1 | memory | GEN_SQL_DML | 1704805 | 3425721 | <span style="color:#dc2626">-14.19%</span> |
| 439 | [00421 DML_WHERE_ORDER_LIMIT_034](crates/bench/sqlite_parity/cases/SQLITE_PARITY_421_DML_WHERE_ORDER_LIMIT_034.rs) | P1 | memory | GEN_SQL_DML | 1601079 | 3423256 | <span style="color:#dc2626">-14.11%</span> |
| 440 | [00478 DML_WHERE_ORDER_LIMIT_091](crates/bench/sqlite_parity/cases/SQLITE_PARITY_478_DML_WHERE_ORDER_LIMIT_091.rs) | P1 | memory | GEN_SQL_DML | 1714964 | 3420802 | <span style="color:#dc2626">-14.03%</span> |
| 441 | [00873 CONSTRAINT_FK_SAVEPOINT_006](crates/bench/sqlite_parity/cases/SQLITE_PARITY_873_CONSTRAINT_FK_SAVEPOINT_006.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1983732 | 3419198 | <span style="color:#dc2626">-13.97%</span> |
| 442 | [00880 CONSTRAINT_FK_SAVEPOINT_013](crates/bench/sqlite_parity/cases/SQLITE_PARITY_880_CONSTRAINT_FK_SAVEPOINT_013.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1740463 | 3418016 | <span style="color:#dc2626">-13.93%</span> |
| 443 | [00947 CONSTRAINT_FK_SAVEPOINT_080](crates/bench/sqlite_parity/cases/SQLITE_PARITY_947_CONSTRAINT_FK_SAVEPOINT_080.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1677223 | 3417545 | <span style="color:#dc2626">-13.92%</span> |
| 444 | [00431 DML_WHERE_ORDER_LIMIT_044](crates/bench/sqlite_parity/cases/SQLITE_PARITY_431_DML_WHERE_ORDER_LIMIT_044.rs) | P1 | memory | GEN_SQL_DML | 1680519 | 3416964 | <span style="color:#dc2626">-13.90%</span> |
| 445 | [00940 CONSTRAINT_FK_SAVEPOINT_073](crates/bench/sqlite_parity/cases/SQLITE_PARITY_940_CONSTRAINT_FK_SAVEPOINT_073.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1727388 | 3415171 | <span style="color:#dc2626">-13.84%</span> |
| 446 | [00459 DML_WHERE_ORDER_LIMIT_072](crates/bench/sqlite_parity/cases/SQLITE_PARITY_459_DML_WHERE_ORDER_LIMIT_072.rs) | P1 | memory | GEN_SQL_DML | 1716157 | 3414168 | <span style="color:#dc2626">-13.81%</span> |
| 447 | [00904 CONSTRAINT_FK_SAVEPOINT_037](crates/bench/sqlite_parity/cases/SQLITE_PARITY_904_CONSTRAINT_FK_SAVEPOINT_037.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1661863 | 3413527 | <span style="color:#dc2626">-13.78%</span> |
| 448 | [00416 DML_WHERE_ORDER_LIMIT_029](crates/bench/sqlite_parity/cases/SQLITE_PARITY_416_DML_WHERE_ORDER_LIMIT_029.rs) | P1 | memory | GEN_SQL_DML | 1732888 | 3412755 | <span style="color:#dc2626">-13.76%</span> |
| 449 | [00902 CONSTRAINT_FK_SAVEPOINT_035](crates/bench/sqlite_parity/cases/SQLITE_PARITY_902_CONSTRAINT_FK_SAVEPOINT_035.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1748077 | 3412455 | <span style="color:#dc2626">-13.75%</span> |
| 450 | [00342 SCALAR_STRING_029](crates/bench/sqlite_parity/cases/SQLITE_PARITY_342_SCALAR_STRING_029.rs) | P1 | memory | GEN_SQL_SCALAR | 1562676 | 3411314 | <span style="color:#dc2626">-13.71%</span> |
| 451 | [00551 AGG_GROUP_HAVING_044](crates/bench/sqlite_parity/cases/SQLITE_PARITY_551_AGG_GROUP_HAVING_044.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1900726 | 3411103 | <span style="color:#dc2626">-13.70%</span> |
| 452 | [00935 CONSTRAINT_FK_SAVEPOINT_068](crates/bench/sqlite_parity/cases/SQLITE_PARITY_935_CONSTRAINT_FK_SAVEPOINT_068.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1752014 | 3409780 | <span style="color:#dc2626">-13.66%</span> |
| 453 | [00926 CONSTRAINT_FK_SAVEPOINT_059](crates/bench/sqlite_parity/cases/SQLITE_PARITY_926_CONSTRAINT_FK_SAVEPOINT_059.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2116714 | 3408698 | <span style="color:#dc2626">-13.62%</span> |
| 454 | [00399 DML_WHERE_ORDER_LIMIT_012](crates/bench/sqlite_parity/cases/SQLITE_PARITY_399_DML_WHERE_ORDER_LIMIT_012.rs) | P1 | memory | GEN_SQL_DML | 1668877 | 3408258 | <span style="color:#dc2626">-13.61%</span> |
| 455 | [00916 CONSTRAINT_FK_SAVEPOINT_049](crates/bench/sqlite_parity/cases/SQLITE_PARITY_916_CONSTRAINT_FK_SAVEPOINT_049.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1970829 | 3404901 | <span style="color:#dc2626">-13.50%</span> |
| 456 | [00464 DML_WHERE_ORDER_LIMIT_077](crates/bench/sqlite_parity/cases/SQLITE_PARITY_464_DML_WHERE_ORDER_LIMIT_077.rs) | P1 | memory | GEN_SQL_DML | 1681361 | 3403298 | <span style="color:#dc2626">-13.44%</span> |
| 457 | [00454 DML_WHERE_ORDER_LIMIT_067](crates/bench/sqlite_parity/cases/SQLITE_PARITY_454_DML_WHERE_ORDER_LIMIT_067.rs) | P1 | memory | GEN_SQL_DML | 1691279 | 3401936 | <span style="color:#dc2626">-13.40%</span> |
| 458 | [00440 DML_WHERE_ORDER_LIMIT_053](crates/bench/sqlite_parity/cases/SQLITE_PARITY_440_DML_WHERE_ORDER_LIMIT_053.rs) | P1 | memory | GEN_SQL_DML | 1689126 | 3400403 | <span style="color:#dc2626">-13.35%</span> |
| 459 | [00426 DML_WHERE_ORDER_LIMIT_039](crates/bench/sqlite_parity/cases/SQLITE_PARITY_426_DML_WHERE_ORDER_LIMIT_039.rs) | P1 | memory | GEN_SQL_DML | 1756714 | 3398389 | <span style="color:#dc2626">-13.28%</span> |
| 460 | [00599 AGG_GROUP_HAVING_092](crates/bench/sqlite_parity/cases/SQLITE_PARITY_599_AGG_GROUP_HAVING_092.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1752255 | 3396285 | <span style="color:#dc2626">-13.21%</span> |
| 461 | [00506 DML_WHERE_ORDER_LIMIT_119](crates/bench/sqlite_parity/cases/SQLITE_PARITY_506_DML_WHERE_ORDER_LIMIT_119.rs) | P1 | memory | GEN_SQL_DML | 1876680 | 3395414 | <span style="color:#dc2626">-13.18%</span> |
| 462 | [00895 CONSTRAINT_FK_SAVEPOINT_028](crates/bench/sqlite_parity/cases/SQLITE_PARITY_895_CONSTRAINT_FK_SAVEPOINT_028.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1783504 | 3393609 | <span style="color:#dc2626">-13.12%</span> |
| 463 | [00868 CONSTRAINT_FK_SAVEPOINT_001](crates/bench/sqlite_parity/cases/SQLITE_PARITY_868_CONSTRAINT_FK_SAVEPOINT_001.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1670731 | 3392407 | <span style="color:#dc2626">-13.08%</span> |
| 464 | [00934 CONSTRAINT_FK_SAVEPOINT_067](crates/bench/sqlite_parity/cases/SQLITE_PARITY_934_CONSTRAINT_FK_SAVEPOINT_067.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1679127 | 3390805 | <span style="color:#dc2626">-13.03%</span> |
| 465 | [00517 AGG_GROUP_HAVING_010](crates/bench/sqlite_parity/cases/SQLITE_PARITY_517_AGG_GROUP_HAVING_010.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2107988 | 3390544 | <span style="color:#dc2626">-13.02%</span> |
| 466 | [00532 AGG_GROUP_HAVING_025](crates/bench/sqlite_parity/cases/SQLITE_PARITY_532_AGG_GROUP_HAVING_025.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1970478 | 3388330 | <span style="color:#dc2626">-12.94%</span> |
| 467 | [00597 AGG_GROUP_HAVING_090](crates/bench/sqlite_parity/cases/SQLITE_PARITY_597_AGG_GROUP_HAVING_090.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1780538 | 3384042 | <span style="color:#dc2626">-12.80%</span> |
| 468 | [01049 JSON_EXTRACT_SET_042](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1049_JSON_EXTRACT_SET_042.rs) | P2 | memory | GEN_SQL_JSON | 1710145 | 3383611 | <span style="color:#dc2626">-12.79%</span> |
| 469 | [00945 CONSTRAINT_FK_SAVEPOINT_078](crates/bench/sqlite_parity/cases/SQLITE_PARITY_945_CONSTRAINT_FK_SAVEPOINT_078.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1721206 | 3383390 | <span style="color:#dc2626">-12.78%</span> |
| 470 | [00922 CONSTRAINT_FK_SAVEPOINT_055](crates/bench/sqlite_parity/cases/SQLITE_PARITY_922_CONSTRAINT_FK_SAVEPOINT_055.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1724462 | 3382329 | <span style="color:#dc2626">-12.74%</span> |
| 471 | [00605 AGG_GROUP_HAVING_098](crates/bench/sqlite_parity/cases/SQLITE_PARITY_605_AGG_GROUP_HAVING_098.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1787963 | 3380645 | <span style="color:#dc2626">-12.69%</span> |
| 472 | [00878 CONSTRAINT_FK_SAVEPOINT_011](crates/bench/sqlite_parity/cases/SQLITE_PARITY_878_CONSTRAINT_FK_SAVEPOINT_011.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1779386 | 3380445 | <span style="color:#dc2626">-12.68%</span> |
| 473 | [00894 CONSTRAINT_FK_SAVEPOINT_027](crates/bench/sqlite_parity/cases/SQLITE_PARITY_894_CONSTRAINT_FK_SAVEPOINT_027.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1734100 | 3376157 | <span style="color:#dc2626">-12.54%</span> |
| 474 | [01099 INDEX_SCHEMA_PRAGMA_032](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1099_INDEX_SCHEMA_PRAGMA_032.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1778715 | 3375435 | <span style="color:#dc2626">-12.51%</span> |
| 475 | [00717 CTE_RECURSIVE_MATRIX_010](crates/bench/sqlite_parity/cases/SQLITE_PARITY_717_CTE_RECURSIVE_MATRIX_010.rs) | P1 | memory | GEN_SQL_CTE | 1778655 | 3375415 | <span style="color:#dc2626">-12.51%</span> |
| 476 | [00527 AGG_GROUP_HAVING_020](crates/bench/sqlite_parity/cases/SQLITE_PARITY_527_AGG_GROUP_HAVING_020.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1681301 | 3372008 | <span style="color:#dc2626">-12.40%</span> |
| 477 | [01059 JSON_EXTRACT_SET_052](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1059_JSON_EXTRACT_SET_052.rs) | P2 | memory | GEN_SQL_JSON | 1655251 | 3371999 | <span style="color:#dc2626">-12.40%</span> |
| 478 | [00871 CONSTRAINT_FK_SAVEPOINT_004](crates/bench/sqlite_parity/cases/SQLITE_PARITY_871_CONSTRAINT_FK_SAVEPOINT_004.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1665431 | 3370115 | <span style="color:#dc2626">-12.34%</span> |
| 479 | [00889 CONSTRAINT_FK_SAVEPOINT_022](crates/bench/sqlite_parity/cases/SQLITE_PARITY_889_CONSTRAINT_FK_SAVEPOINT_022.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2217785 | 3369755 | <span style="color:#dc2626">-12.33%</span> |
| 480 | [01015 JSON_EXTRACT_SET_008](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1015_JSON_EXTRACT_SET_008.rs) | P2 | memory | GEN_SQL_JSON | 1688584 | 3368903 | <span style="color:#dc2626">-12.30%</span> |
| 481 | [00891 CONSTRAINT_FK_SAVEPOINT_024](crates/bench/sqlite_parity/cases/SQLITE_PARITY_891_CONSTRAINT_FK_SAVEPOINT_024.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1801408 | 3367771 | <span style="color:#dc2626">-12.26%</span> |
| 482 | [00552 AGG_GROUP_HAVING_045](crates/bench/sqlite_parity/cases/SQLITE_PARITY_552_AGG_GROUP_HAVING_045.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1657836 | 3367750 | <span style="color:#dc2626">-12.26%</span> |
| 483 | [00941 CONSTRAINT_FK_SAVEPOINT_074](crates/bench/sqlite_parity/cases/SQLITE_PARITY_941_CONSTRAINT_FK_SAVEPOINT_074.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1677894 | 3367249 | <span style="color:#dc2626">-12.24%</span> |
| 484 | [00043 ATTACH_DETACH_MEMORY](crates/bench/sqlite_parity/cases/SQLITE_PARITY_043_ATTACH_DETACH_MEMORY.rs) | P0 | memory | SQL_ATTACH | 1618512 | 3365055 | <span style="color:#dc2626">-12.17%</span> |
| 485 | [00938 CONSTRAINT_FK_SAVEPOINT_071](crates/bench/sqlite_parity/cases/SQLITE_PARITY_938_CONSTRAINT_FK_SAVEPOINT_071.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2957455 | 3364935 | <span style="color:#dc2626">-12.16%</span> |
| 486 | [00504 DML_WHERE_ORDER_LIMIT_117](crates/bench/sqlite_parity/cases/SQLITE_PARITY_504_DML_WHERE_ORDER_LIMIT_117.rs) | P1 | memory | GEN_SQL_DML | 1841644 | 3364835 | <span style="color:#dc2626">-12.16%</span> |
| 487 | [00915 CONSTRAINT_FK_SAVEPOINT_048](crates/bench/sqlite_parity/cases/SQLITE_PARITY_915_CONSTRAINT_FK_SAVEPOINT_048.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1660681 | 3364024 | <span style="color:#dc2626">-12.13%</span> |
| 488 | [00499 DML_WHERE_ORDER_LIMIT_112](crates/bench/sqlite_parity/cases/SQLITE_PARITY_499_DML_WHERE_ORDER_LIMIT_112.rs) | P1 | memory | GEN_SQL_DML | 1641645 | 3363463 | <span style="color:#dc2626">-12.12%</span> |
| 489 | [00906 CONSTRAINT_FK_SAVEPOINT_039](crates/bench/sqlite_parity/cases/SQLITE_PARITY_906_CONSTRAINT_FK_SAVEPOINT_039.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1705236 | 3363002 | <span style="color:#dc2626">-12.10%</span> |
| 490 | [01065 JSON_EXTRACT_SET_058](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1065_JSON_EXTRACT_SET_058.rs) | P2 | memory | GEN_SQL_JSON | 1631567 | 3361118 | <span style="color:#dc2626">-12.04%</span> |
| 491 | [00943 CONSTRAINT_FK_SAVEPOINT_076](crates/bench/sqlite_parity/cases/SQLITE_PARITY_943_CONSTRAINT_FK_SAVEPOINT_076.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1682813 | 3361099 | <span style="color:#dc2626">-12.04%</span> |
| 492 | [00913 CONSTRAINT_FK_SAVEPOINT_046](crates/bench/sqlite_parity/cases/SQLITE_PARITY_913_CONSTRAINT_FK_SAVEPOINT_046.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1667315 | 3358313 | <span style="color:#dc2626">-11.94%</span> |
| 493 | [01052 JSON_EXTRACT_SET_045](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1052_JSON_EXTRACT_SET_045.rs) | P2 | memory | GEN_SQL_JSON | 1650933 | 3357672 | <span style="color:#dc2626">-11.92%</span> |
| 494 | [00003 CREATE_TABLE_INSERT_SELECT](crates/bench/sqlite_parity/cases/SQLITE_PARITY_003_CREATE_TABLE_INSERT_SELECT.rs) | P0 | memory | SQL_DDL_DML | 2068412 | 3356259 | <span style="color:#dc2626">-11.88%</span> |
| 495 | [00556 AGG_GROUP_HAVING_049](crates/bench/sqlite_parity/cases/SQLITE_PARITY_556_AGG_GROUP_HAVING_049.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1670309 | 3355447 | <span style="color:#dc2626">-11.85%</span> |
| 496 | [00910 CONSTRAINT_FK_SAVEPOINT_043](crates/bench/sqlite_parity/cases/SQLITE_PARITY_910_CONSTRAINT_FK_SAVEPOINT_043.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1681982 | 3354817 | <span style="color:#dc2626">-11.83%</span> |
| 497 | [00876 CONSTRAINT_FK_SAVEPOINT_009](crates/bench/sqlite_parity/cases/SQLITE_PARITY_876_CONSTRAINT_FK_SAVEPOINT_009.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1739892 | 3352092 | <span style="color:#dc2626">-11.74%</span> |
| 498 | [00881 CONSTRAINT_FK_SAVEPOINT_014](crates/bench/sqlite_parity/cases/SQLITE_PARITY_881_CONSTRAINT_FK_SAVEPOINT_014.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2006666 | 3351541 | <span style="color:#dc2626">-11.72%</span> |
| 499 | [00528 AGG_GROUP_HAVING_021](crates/bench/sqlite_parity/cases/SQLITE_PARITY_528_AGG_GROUP_HAVING_021.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1859278 | 3349697 | <span style="color:#dc2626">-11.66%</span> |
| 500 | [01043 JSON_EXTRACT_SET_036](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1043_JSON_EXTRACT_SET_036.rs) | P2 | memory | GEN_SQL_JSON | 1582844 | 3348254 | <span style="color:#dc2626">-11.61%</span> |
| 501 | [01014 JSON_EXTRACT_SET_007](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1014_JSON_EXTRACT_SET_007.rs) | P2 | memory | GEN_SQL_JSON | 1668246 | 3346881 | <span style="color:#dc2626">-11.56%</span> |
| 502 | [00310 SCALAR_STRING_021](crates/bench/sqlite_parity/cases/SQLITE_PARITY_310_SCALAR_STRING_021.rs) | P1 | memory | GEN_SQL_SCALAR | 1870188 | 3345238 | <span style="color:#dc2626">-11.51%</span> |
| 503 | [00886 CONSTRAINT_FK_SAVEPOINT_019](crates/bench/sqlite_parity/cases/SQLITE_PARITY_886_CONSTRAINT_FK_SAVEPOINT_019.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1738859 | 3343384 | <span style="color:#dc2626">-11.45%</span> |
| 504 | [00523 AGG_GROUP_HAVING_016](crates/bench/sqlite_parity/cases/SQLITE_PARITY_523_AGG_GROUP_HAVING_016.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2025181 | 3342924 | <span style="color:#dc2626">-11.43%</span> |
| 505 | [00601 AGG_GROUP_HAVING_094](crates/bench/sqlite_parity/cases/SQLITE_PARITY_601_AGG_GROUP_HAVING_094.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2047112 | 3342623 | <span style="color:#dc2626">-11.42%</span> |
| 506 | [01018 JSON_EXTRACT_SET_011](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1018_JSON_EXTRACT_SET_011.rs) | P2 | memory | GEN_SQL_JSON | 1571162 | 3341832 | <span style="color:#dc2626">-11.39%</span> |
| 507 | [00918 CONSTRAINT_FK_SAVEPOINT_051](crates/bench/sqlite_parity/cases/SQLITE_PARITY_918_CONSTRAINT_FK_SAVEPOINT_051.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1666312 | 3341301 | <span style="color:#dc2626">-11.38%</span> |
| 508 | [01026 JSON_EXTRACT_SET_019](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1026_JSON_EXTRACT_SET_019.rs) | P2 | memory | GEN_SQL_JSON | 1650352 | 3339227 | <span style="color:#dc2626">-11.31%</span> |
| 509 | [00896 CONSTRAINT_FK_SAVEPOINT_029](crates/bench/sqlite_parity/cases/SQLITE_PARITY_896_CONSTRAINT_FK_SAVEPOINT_029.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1648588 | 3339017 | <span style="color:#dc2626">-11.30%</span> |
| 510 | [01020 JSON_EXTRACT_SET_013](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1020_JSON_EXTRACT_SET_013.rs) | P2 | memory | GEN_SQL_JSON | 1606699 | 3338365 | <span style="color:#dc2626">-11.28%</span> |
| 511 | [01054 JSON_EXTRACT_SET_047](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1054_JSON_EXTRACT_SET_047.rs) | P2 | memory | GEN_SQL_JSON | 1615807 | 3337444 | <span style="color:#dc2626">-11.25%</span> |
| 512 | [00875 CONSTRAINT_FK_SAVEPOINT_008](crates/bench/sqlite_parity/cases/SQLITE_PARITY_875_CONSTRAINT_FK_SAVEPOINT_008.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1811707 | 3336902 | <span style="color:#dc2626">-11.23%</span> |
| 513 | [00898 CONSTRAINT_FK_SAVEPOINT_031](crates/bench/sqlite_parity/cases/SQLITE_PARITY_898_CONSTRAINT_FK_SAVEPOINT_031.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1743518 | 3336000 | <span style="color:#dc2626">-11.20%</span> |
| 514 | [00942 CONSTRAINT_FK_SAVEPOINT_075](crates/bench/sqlite_parity/cases/SQLITE_PARITY_942_CONSTRAINT_FK_SAVEPOINT_075.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1737217 | 3335731 | <span style="color:#dc2626">-11.19%</span> |
| 515 | [01036 JSON_EXTRACT_SET_029](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1036_JSON_EXTRACT_SET_029.rs) | P2 | memory | GEN_SQL_JSON | 1654620 | 3335489 | <span style="color:#dc2626">-11.18%</span> |
| 516 | [00053 SELECT_WHERE_ORDER_LIMIT_OFFSET](crates/bench/sqlite_parity/cases/SQLITE_PARITY_053_SELECT_WHERE_ORDER_LIMIT_OFFSET.rs) | P0 | memory | SQL_SELECT | 1555182 | 3335340 | <span style="color:#dc2626">-11.18%</span> |
| 517 | [01066 JSON_EXTRACT_SET_059](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1066_JSON_EXTRACT_SET_059.rs) | P2 | memory | GEN_SQL_JSON | 1658758 | 3333647 | <span style="color:#dc2626">-11.12%</span> |
| 518 | [00755 CTE_RECURSIVE_MATRIX_048](crates/bench/sqlite_parity/cases/SQLITE_PARITY_755_CTE_RECURSIVE_MATRIX_048.rs) | P1 | memory | GEN_SQL_CTE | 1626066 | 3332544 | <span style="color:#dc2626">-11.08%</span> |
| 519 | [01041 JSON_EXTRACT_SET_034](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1041_JSON_EXTRACT_SET_034.rs) | P2 | memory | GEN_SQL_JSON | 1679317 | 3332224 | <span style="color:#dc2626">-11.07%</span> |
| 520 | [01008 JSON_EXTRACT_SET_001](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1008_JSON_EXTRACT_SET_001.rs) | P2 | memory | GEN_SQL_JSON | 1591751 | 3332173 | <span style="color:#dc2626">-11.07%</span> |
| 521 | [01023 JSON_EXTRACT_SET_016](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1023_JSON_EXTRACT_SET_016.rs) | P2 | memory | GEN_SQL_JSON | 1682353 | 3329128 | <span style="color:#dc2626">-10.97%</span> |
| 522 | [00068 CAST_AND_TYPE_AFFINITY](crates/bench/sqlite_parity/cases/SQLITE_PARITY_068_CAST_AND_TYPE_AFFINITY.rs) | P0 | memory | SQL_EXPRESSIONS | 1506260 | 3328918 | <span style="color:#dc2626">-10.96%</span> |
| 523 | [01012 JSON_EXTRACT_SET_005](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1012_JSON_EXTRACT_SET_005.rs) | P2 | memory | GEN_SQL_JSON | 1683885 | 3328827 | <span style="color:#dc2626">-10.96%</span> |
| 524 | [01011 JSON_EXTRACT_SET_004](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1011_JSON_EXTRACT_SET_004.rs) | P2 | memory | GEN_SQL_JSON | 1650362 | 3328276 | <span style="color:#dc2626">-10.94%</span> |
| 525 | [00884 CONSTRAINT_FK_SAVEPOINT_017](crates/bench/sqlite_parity/cases/SQLITE_PARITY_884_CONSTRAINT_FK_SAVEPOINT_017.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1669638 | 3327105 | <span style="color:#dc2626">-10.90%</span> |
| 526 | [01050 JSON_EXTRACT_SET_043](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1050_JSON_EXTRACT_SET_043.rs) | P2 | memory | GEN_SQL_JSON | 1924331 | 3326062 | <span style="color:#dc2626">-10.87%</span> |
| 527 | [00971 VIEW_TRIGGER_GENERATED_024](crates/bench/sqlite_parity/cases/SQLITE_PARITY_971_VIEW_TRIGGER_GENERATED_024.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1747255 | 3325932 | <span style="color:#dc2626">-10.86%</span> |
| 528 | [01029 JSON_EXTRACT_SET_022](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1029_JSON_EXTRACT_SET_022.rs) | P2 | memory | GEN_SQL_JSON | 1644270 | 3325892 | <span style="color:#dc2626">-10.86%</span> |
| 529 | [01040 JSON_EXTRACT_SET_033](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1040_JSON_EXTRACT_SET_033.rs) | P2 | memory | GEN_SQL_JSON | 1706598 | 3325681 | <span style="color:#dc2626">-10.86%</span> |
| 530 | [01057 JSON_EXTRACT_SET_050](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1057_JSON_EXTRACT_SET_050.rs) | P2 | memory | GEN_SQL_JSON | 1851692 | 3325411 | <span style="color:#dc2626">-10.85%</span> |
| 531 | [01037 JSON_EXTRACT_SET_030](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1037_JSON_EXTRACT_SET_030.rs) | P2 | memory | GEN_SQL_JSON | 1633560 | 3321313 | <span style="color:#dc2626">-10.71%</span> |
| 532 | [01034 JSON_EXTRACT_SET_027](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1034_JSON_EXTRACT_SET_027.rs) | P2 | memory | GEN_SQL_JSON | 1560553 | 3321053 | <span style="color:#dc2626">-10.70%</span> |
| 533 | [01033 JSON_EXTRACT_SET_026](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1033_JSON_EXTRACT_SET_026.rs) | P2 | memory | GEN_SQL_JSON | 1642116 | 3317947 | <span style="color:#dc2626">-10.60%</span> |
| 534 | [01013 JSON_EXTRACT_SET_006](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1013_JSON_EXTRACT_SET_006.rs) | P2 | memory | GEN_SQL_JSON | 1604485 | 3317626 | <span style="color:#dc2626">-10.59%</span> |
| 535 | [01024 JSON_EXTRACT_SET_017](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1024_JSON_EXTRACT_SET_017.rs) | P2 | memory | GEN_SQL_JSON | 1641215 | 3316203 | <span style="color:#dc2626">-10.54%</span> |
| 536 | [01064 JSON_EXTRACT_SET_057](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1064_JSON_EXTRACT_SET_057.rs) | P2 | memory | GEN_SQL_JSON | 1836294 | 3315382 | <span style="color:#dc2626">-10.51%</span> |
| 537 | [01042 JSON_EXTRACT_SET_035](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1042_JSON_EXTRACT_SET_035.rs) | P2 | memory | GEN_SQL_JSON | 1648157 | 3315031 | <span style="color:#dc2626">-10.50%</span> |
| 538 | [00297 SCALAR_CAST_TYPEOF_018](crates/bench/sqlite_parity/cases/SQLITE_PARITY_297_SCALAR_CAST_TYPEOF_018.rs) | P1 | memory | GEN_SQL_SCALAR | 1560682 | 3311465 | <span style="color:#dc2626">-10.38%</span> |
| 539 | [00039 CREATE_TRIGGER_BEFORE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_039_CREATE_TRIGGER_BEFORE.rs) | P0 | memory | SQL_TRIGGER | 2031924 | 3311354 | <span style="color:#dc2626">-10.38%</span> |
| 540 | [00469 DML_WHERE_ORDER_LIMIT_082](crates/bench/sqlite_parity/cases/SQLITE_PARITY_469_DML_WHERE_ORDER_LIMIT_082.rs) | P1 | memory | GEN_SQL_DML | 2586603 | 3310012 | <span style="color:#dc2626">-10.33%</span> |
| 541 | [00274 SCALAR_STRING_012](crates/bench/sqlite_parity/cases/SQLITE_PARITY_274_SCALAR_STRING_012.rs) | P1 | memory | GEN_SQL_SCALAR | 1469850 | 3309610 | <span style="color:#dc2626">-10.32%</span> |
| 542 | [01016 JSON_EXTRACT_SET_009](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1016_JSON_EXTRACT_SET_009.rs) | P2 | memory | GEN_SQL_JSON | 1702410 | 3309390 | <span style="color:#dc2626">-10.31%</span> |
| 543 | [00924 CONSTRAINT_FK_SAVEPOINT_057](crates/bench/sqlite_parity/cases/SQLITE_PARITY_924_CONSTRAINT_FK_SAVEPOINT_057.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1848547 | 3309280 | <span style="color:#dc2626">-10.31%</span> |
| 544 | [00936 CONSTRAINT_FK_SAVEPOINT_069](crates/bench/sqlite_parity/cases/SQLITE_PARITY_936_CONSTRAINT_FK_SAVEPOINT_069.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2241681 | 3308809 | <span style="color:#dc2626">-10.29%</span> |
| 545 | [00516 AGG_GROUP_HAVING_009](crates/bench/sqlite_parity/cases/SQLITE_PARITY_516_AGG_GROUP_HAVING_009.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2395271 | 3308799 | <span style="color:#dc2626">-10.29%</span> |
| 546 | [00892 CONSTRAINT_FK_SAVEPOINT_025](crates/bench/sqlite_parity/cases/SQLITE_PARITY_892_CONSTRAINT_FK_SAVEPOINT_025.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1740252 | 3307386 | <span style="color:#dc2626">-10.25%</span> |
| 547 | [00804 WINDOW_PARTITION_SUM_017](crates/bench/sqlite_parity/cases/SQLITE_PARITY_804_WINDOW_PARTITION_SUM_017.rs) | P2 | memory | GEN_SQL_WINDOW | 1763477 | 3307227 | <span style="color:#dc2626">-10.24%</span> |
| 548 | [01021 JSON_EXTRACT_SET_014](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1021_JSON_EXTRACT_SET_014.rs) | P2 | memory | GEN_SQL_JSON | 1696430 | 3307226 | <span style="color:#dc2626">-10.24%</span> |
| 549 | [00056 SUBQUERIES_EXISTS_IN](crates/bench/sqlite_parity/cases/SQLITE_PARITY_056_SUBQUERIES_EXISTS_IN.rs) | P0 | memory | SQL_SELECT | 1625124 | 3306355 | <span style="color:#dc2626">-10.21%</span> |
| 550 | [00928 CONSTRAINT_FK_SAVEPOINT_061](crates/bench/sqlite_parity/cases/SQLITE_PARITY_928_CONSTRAINT_FK_SAVEPOINT_061.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1717929 | 3305593 | <span style="color:#dc2626">-10.19%</span> |
| 551 | [00749 CTE_RECURSIVE_MATRIX_042](crates/bench/sqlite_parity/cases/SQLITE_PARITY_749_CTE_RECURSIVE_MATRIX_042.rs) | P1 | memory | GEN_SQL_CTE | 1663547 | 3304902 | <span style="color:#dc2626">-10.16%</span> |
| 552 | [00154 DOT_DBINFO_TEMPFILE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_154_DOT_DBINFO_TEMPFILE.rs) | P3 | tempfile | CLI_TEMPFILE_DIAGNOSTIC | 1759579 | 3304231 | <span style="color:#dc2626">-10.14%</span> |
| 553 | [00907 CONSTRAINT_FK_SAVEPOINT_040](crates/bench/sqlite_parity/cases/SQLITE_PARITY_907_CONSTRAINT_FK_SAVEPOINT_040.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1702400 | 3303018 | <span style="color:#dc2626">-10.10%</span> |
| 554 | [01060 JSON_EXTRACT_SET_053](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1060_JSON_EXTRACT_SET_053.rs) | P2 | memory | GEN_SQL_JSON | 1650983 | 3302708 | <span style="color:#dc2626">-10.09%</span> |
| 555 | [00917 CONSTRAINT_FK_SAVEPOINT_050](crates/bench/sqlite_parity/cases/SQLITE_PARITY_917_CONSTRAINT_FK_SAVEPOINT_050.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1741515 | 3300855 | <span style="color:#dc2626">-10.03%</span> |
| 556 | [00923 CONSTRAINT_FK_SAVEPOINT_056](crates/bench/sqlite_parity/cases/SQLITE_PARITY_923_CONSTRAINT_FK_SAVEPOINT_056.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1713612 | 3300023 | <span style="color:#dc2626">-10.00%</span> |
| 557 | [01035 JSON_EXTRACT_SET_028](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1035_JSON_EXTRACT_SET_028.rs) | P2 | memory | GEN_SQL_JSON | 1655662 | 3299752 | <span style="color:#dc2626">-9.99%</span> |
| 558 | [00890 CONSTRAINT_FK_SAVEPOINT_023](crates/bench/sqlite_parity/cases/SQLITE_PARITY_890_CONSTRAINT_FK_SAVEPOINT_023.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1715836 | 3298781 | <span style="color:#dc2626">-9.96%</span> |
| 559 | [00596 AGG_GROUP_HAVING_089](crates/bench/sqlite_parity/cases/SQLITE_PARITY_596_AGG_GROUP_HAVING_089.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1638209 | 3298680 | <span style="color:#dc2626">-9.96%</span> |
| 560 | [01022 JSON_EXTRACT_SET_015](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1022_JSON_EXTRACT_SET_015.rs) | P2 | memory | GEN_SQL_JSON | 1555373 | 3298280 | <span style="color:#dc2626">-9.94%</span> |
| 561 | [01030 JSON_EXTRACT_SET_023](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1030_JSON_EXTRACT_SET_023.rs) | P2 | memory | GEN_SQL_JSON | 1613452 | 3297468 | <span style="color:#dc2626">-9.92%</span> |
| 562 | [00932 CONSTRAINT_FK_SAVEPOINT_065](crates/bench/sqlite_parity/cases/SQLITE_PARITY_932_CONSTRAINT_FK_SAVEPOINT_065.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1679697 | 3296577 | <span style="color:#dc2626">-9.89%</span> |
| 563 | [00909 CONSTRAINT_FK_SAVEPOINT_042](crates/bench/sqlite_parity/cases/SQLITE_PARITY_909_CONSTRAINT_FK_SAVEPOINT_042.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1868815 | 3295023 | <span style="color:#dc2626">-9.83%</span> |
| 564 | [00929 CONSTRAINT_FK_SAVEPOINT_062](crates/bench/sqlite_parity/cases/SQLITE_PARITY_929_CONSTRAINT_FK_SAVEPOINT_062.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1698353 | 3293730 | <span style="color:#dc2626">-9.79%</span> |
| 565 | [01038 JSON_EXTRACT_SET_031](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1038_JSON_EXTRACT_SET_031.rs) | P2 | memory | GEN_SQL_JSON | 1648638 | 3293350 | <span style="color:#dc2626">-9.78%</span> |
| 566 | [01031 JSON_EXTRACT_SET_024](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1031_JSON_EXTRACT_SET_024.rs) | P2 | memory | GEN_SQL_JSON | 1590008 | 3292428 | <span style="color:#dc2626">-9.75%</span> |
| 567 | [00378 SCALAR_STRING_038](crates/bench/sqlite_parity/cases/SQLITE_PARITY_378_SCALAR_STRING_038.rs) | P1 | memory | GEN_SQL_SCALAR | 1514124 | 3292278 | <span style="color:#dc2626">-9.74%</span> |
| 568 | [00515 AGG_GROUP_HAVING_008](crates/bench/sqlite_parity/cases/SQLITE_PARITY_515_AGG_GROUP_HAVING_008.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2259995 | 3291446 | <span style="color:#dc2626">-9.71%</span> |
| 569 | [00829 WINDOW_PARTITION_SUM_042](crates/bench/sqlite_parity/cases/SQLITE_PARITY_829_WINDOW_PARTITION_SUM_042.rs) | P2 | memory | GEN_SQL_WINDOW | 1854949 | 3290625 | <span style="color:#dc2626">-9.69%</span> |
| 570 | [00783 CTE_RECURSIVE_MATRIX_076](crates/bench/sqlite_parity/cases/SQLITE_PARITY_783_CTE_RECURSIVE_MATRIX_076.rs) | P1 | memory | GEN_SQL_CTE | 1607991 | 3289653 | <span style="color:#dc2626">-9.66%</span> |
| 571 | [01017 JSON_EXTRACT_SET_010](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1017_JSON_EXTRACT_SET_010.rs) | P2 | memory | GEN_SQL_JSON | 1575621 | 3289162 | <span style="color:#dc2626">-9.64%</span> |
| 572 | [00766 CTE_RECURSIVE_MATRIX_059](crates/bench/sqlite_parity/cases/SQLITE_PARITY_766_CTE_RECURSIVE_MATRIX_059.rs) | P1 | memory | GEN_SQL_CTE | 1570361 | 3289062 | <span style="color:#dc2626">-9.64%</span> |
| 573 | [00738 CTE_RECURSIVE_MATRIX_031](crates/bench/sqlite_parity/cases/SQLITE_PARITY_738_CTE_RECURSIVE_MATRIX_031.rs) | P1 | memory | GEN_SQL_CTE | 1704985 | 3287589 | <span style="color:#dc2626">-9.59%</span> |
| 574 | [01028 JSON_EXTRACT_SET_021](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1028_JSON_EXTRACT_SET_021.rs) | P2 | memory | GEN_SQL_JSON | 1597622 | 3284603 | <span style="color:#dc2626">-9.49%</span> |
| 575 | [00925 CONSTRAINT_FK_SAVEPOINT_058](crates/bench/sqlite_parity/cases/SQLITE_PARITY_925_CONSTRAINT_FK_SAVEPOINT_058.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1707831 | 3283803 | <span style="color:#dc2626">-9.46%</span> |
| 576 | [00579 AGG_GROUP_HAVING_072](crates/bench/sqlite_parity/cases/SQLITE_PARITY_579_AGG_GROUP_HAVING_072.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1793723 | 3281458 | <span style="color:#dc2626">-9.38%</span> |
| 577 | [00836 WINDOW_PARTITION_SUM_049](crates/bench/sqlite_parity/cases/SQLITE_PARITY_836_WINDOW_PARTITION_SUM_049.rs) | P2 | memory | GEN_SQL_WINDOW | 1601029 | 3280205 | <span style="color:#dc2626">-9.34%</span> |
| 578 | [00604 AGG_GROUP_HAVING_097](crates/bench/sqlite_parity/cases/SQLITE_PARITY_604_AGG_GROUP_HAVING_097.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2033797 | 3274966 | <span style="color:#dc2626">-9.17%</span> |
| 579 | [00739 CTE_RECURSIVE_MATRIX_032](crates/bench/sqlite_parity/cases/SQLITE_PARITY_739_CTE_RECURSIVE_MATRIX_032.rs) | P1 | memory | GEN_SQL_CTE | 1794895 | 3270778 | <span style="color:#dc2626">-9.03%</span> |
| 580 | [00713 CTE_RECURSIVE_MATRIX_006](crates/bench/sqlite_parity/cases/SQLITE_PARITY_713_CTE_RECURSIVE_MATRIX_006.rs) | P1 | memory | GEN_SQL_CTE | 1551966 | 3270457 | <span style="color:#dc2626">-9.02%</span> |
| 581 | [01051 JSON_EXTRACT_SET_044](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1051_JSON_EXTRACT_SET_044.rs) | P2 | memory | GEN_SQL_JSON | 1648279 | 3270217 | <span style="color:#dc2626">-9.01%</span> |
| 582 | [00769 CTE_RECURSIVE_MATRIX_062](crates/bench/sqlite_parity/cases/SQLITE_PARITY_769_CTE_RECURSIVE_MATRIX_062.rs) | P1 | memory | GEN_SQL_CTE | 1591430 | 3268904 | <span style="color:#dc2626">-8.96%</span> |
| 583 | [01019 JSON_EXTRACT_SET_012](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1019_JSON_EXTRACT_SET_012.rs) | P2 | memory | GEN_SQL_JSON | 1629132 | 3267491 | <span style="color:#dc2626">-8.92%</span> |
| 584 | [00785 CTE_RECURSIVE_MATRIX_078](crates/bench/sqlite_parity/cases/SQLITE_PARITY_785_CTE_RECURSIVE_MATRIX_078.rs) | P1 | memory | GEN_SQL_CTE | 1710946 | 3266509 | <span style="color:#dc2626">-8.88%</span> |
| 585 | [00800 WINDOW_PARTITION_SUM_013](crates/bench/sqlite_parity/cases/SQLITE_PARITY_800_WINDOW_PARTITION_SUM_013.rs) | P2 | memory | GEN_SQL_WINDOW | 1691670 | 3266279 | <span style="color:#dc2626">-8.88%</span> |
| 586 | [00718 CTE_RECURSIVE_MATRIX_011](crates/bench/sqlite_parity/cases/SQLITE_PARITY_718_CTE_RECURSIVE_MATRIX_011.rs) | P1 | memory | GEN_SQL_CTE | 1632829 | 3266249 | <span style="color:#dc2626">-8.87%</span> |
| 587 | [00202 OPT_APPEND_TEMPFILE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_202_OPT_APPEND_TEMPFILE.rs) | P4 | tempfile | CLI_OPTION_TEMPFILE | 1603784 | 3264365 | <span style="color:#dc2626">-8.81%</span> |
| 588 | [00711 CTE_RECURSIVE_MATRIX_004](crates/bench/sqlite_parity/cases/SQLITE_PARITY_711_CTE_RECURSIVE_MATRIX_004.rs) | P1 | memory | GEN_SQL_CTE | 1645693 | 3263544 | <span style="color:#dc2626">-8.78%</span> |
| 589 | [00015 UPDATE_BASIC](crates/bench/sqlite_parity/cases/SQLITE_PARITY_015_UPDATE_BASIC.rs) | P0 | memory | SQL_UPDATE | 1583796 | 3262772 | <span style="color:#dc2626">-8.76%</span> |
| 590 | [01061 JSON_EXTRACT_SET_054](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1061_JSON_EXTRACT_SET_054.rs) | P2 | memory | GEN_SQL_JSON | 1580640 | 3260328 | <span style="color:#dc2626">-8.68%</span> |
| 591 | [00775 CTE_RECURSIVE_MATRIX_068](crates/bench/sqlite_parity/cases/SQLITE_PARITY_775_CTE_RECURSIVE_MATRIX_068.rs) | P1 | memory | GEN_SQL_CTE | 1621488 | 3259326 | <span style="color:#dc2626">-8.64%</span> |
| 592 | [01055 JSON_EXTRACT_SET_048](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1055_JSON_EXTRACT_SET_048.rs) | P2 | memory | GEN_SQL_JSON | 1883362 | 3257652 | <span style="color:#dc2626">-8.59%</span> |
| 593 | [00746 CTE_RECURSIVE_MATRIX_039](crates/bench/sqlite_parity/cases/SQLITE_PARITY_746_CTE_RECURSIVE_MATRIX_039.rs) | P1 | memory | GEN_SQL_CTE | 1737266 | 3257292 | <span style="color:#dc2626">-8.58%</span> |
| 594 | [00330 SCALAR_STRING_026](crates/bench/sqlite_parity/cases/SQLITE_PARITY_330_SCALAR_STRING_026.rs) | P1 | memory | GEN_SQL_SCALAR | 1502452 | 3256200 | <span style="color:#dc2626">-8.54%</span> |
| 595 | [00861 WINDOW_PARTITION_SUM_074](crates/bench/sqlite_parity/cases/SQLITE_PARITY_861_WINDOW_PARTITION_SUM_074.rs) | P2 | memory | GEN_SQL_WINDOW | 1634011 | 3255919 | <span style="color:#dc2626">-8.53%</span> |
| 596 | [01062 JSON_EXTRACT_SET_055](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1062_JSON_EXTRACT_SET_055.rs) | P2 | memory | GEN_SQL_JSON | 1556725 | 3254857 | <span style="color:#dc2626">-8.50%</span> |
| 597 | [00815 WINDOW_PARTITION_SUM_028](crates/bench/sqlite_parity/cases/SQLITE_PARITY_815_WINDOW_PARTITION_SUM_028.rs) | P2 | memory | GEN_SQL_WINDOW | 1642847 | 3254777 | <span style="color:#dc2626">-8.49%</span> |
| 598 | [00741 CTE_RECURSIVE_MATRIX_034](crates/bench/sqlite_parity/cases/SQLITE_PARITY_741_CTE_RECURSIVE_MATRIX_034.rs) | P1 | memory | GEN_SQL_CTE | 1716437 | 3254707 | <span style="color:#dc2626">-8.49%</span> |
| 599 | [00095 CREATE_VIRTUAL_TABLE_RTREE_OPTIONAL](crates/bench/sqlite_parity/cases/SQLITE_PARITY_095_CREATE_VIRTUAL_TABLE_RTREE_OPTIONAL.rs) | P2 | memory | SQL_VIRTUAL_TABLE_OPTIONAL | 1745812 | 3252242 | <span style="color:#dc2626">-8.41%</span> |
| 600 | [00301 SCALAR_CAST_TYPEOF_019](crates/bench/sqlite_parity/cases/SQLITE_PARITY_301_SCALAR_CAST_TYPEOF_019.rs) | P1 | memory | GEN_SQL_SCALAR | 1537238 | 3250178 | <span style="color:#dc2626">-8.34%</span> |
| 601 | [01063 JSON_EXTRACT_SET_056](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1063_JSON_EXTRACT_SET_056.rs) | P2 | memory | GEN_SQL_JSON | 1864417 | 3248135 | <span style="color:#dc2626">-8.27%</span> |
| 602 | [00759 CTE_RECURSIVE_MATRIX_052](crates/bench/sqlite_parity/cases/SQLITE_PARITY_759_CTE_RECURSIVE_MATRIX_052.rs) | P1 | memory | GEN_SQL_CTE | 1616348 | 3247894 | <span style="color:#dc2626">-8.26%</span> |
| 603 | [00740 CTE_RECURSIVE_MATRIX_033](crates/bench/sqlite_parity/cases/SQLITE_PARITY_740_CTE_RECURSIVE_MATRIX_033.rs) | P1 | memory | GEN_SQL_CTE | 1646756 | 3246872 | <span style="color:#dc2626">-8.23%</span> |
| 604 | [00792 WINDOW_PARTITION_SUM_005](crates/bench/sqlite_parity/cases/SQLITE_PARITY_792_WINDOW_PARTITION_SUM_005.rs) | P2 | memory | GEN_SQL_WINDOW | 1651054 | 3246722 | <span style="color:#dc2626">-8.22%</span> |
| 605 | [01047 JSON_EXTRACT_SET_040](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1047_JSON_EXTRACT_SET_040.rs) | P2 | memory | GEN_SQL_JSON | 1652516 | 3246702 | <span style="color:#dc2626">-8.22%</span> |
| 606 | [00790 WINDOW_PARTITION_SUM_003](crates/bench/sqlite_parity/cases/SQLITE_PARITY_790_WINDOW_PARTITION_SUM_003.rs) | P2 | memory | GEN_SQL_WINDOW | 1679458 | 3246472 | <span style="color:#dc2626">-8.22%</span> |
| 607 | [01032 JSON_EXTRACT_SET_025](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1032_JSON_EXTRACT_SET_025.rs) | P2 | memory | GEN_SQL_JSON | 1554190 | 3245840 | <span style="color:#dc2626">-8.19%</span> |
| 608 | [00784 CTE_RECURSIVE_MATRIX_077](crates/bench/sqlite_parity/cases/SQLITE_PARITY_784_CTE_RECURSIVE_MATRIX_077.rs) | P1 | memory | GEN_SQL_CTE | 1579809 | 3245700 | <span style="color:#dc2626">-8.19%</span> |
| 609 | [00130 DOT_OPEN_MEMORY](crates/bench/sqlite_parity/cases/SQLITE_PARITY_130_DOT_OPEN_MEMORY.rs) | P0 | memory | CLI_DOT_COMMAND | 1693845 | 3245439 | <span style="color:#dc2626">-8.18%</span> |
| 610 | [00115 DOT_SCHEMA_TABLES_INDEXES](crates/bench/sqlite_parity/cases/SQLITE_PARITY_115_DOT_SCHEMA_TABLES_INDEXES.rs) | P0 | memory | CLI_DOT_COMMAND | 2088651 | 3245400 | <span style="color:#dc2626">-8.18%</span> |
| 611 | [00403 DML_WHERE_ORDER_LIMIT_016](crates/bench/sqlite_parity/cases/SQLITE_PARITY_403_DML_WHERE_ORDER_LIMIT_016.rs) | P1 | memory | GEN_SQL_DML | 1693374 | 3244748 | <span style="color:#dc2626">-8.16%</span> |
| 612 | [00712 CTE_RECURSIVE_MATRIX_005](crates/bench/sqlite_parity/cases/SQLITE_PARITY_712_CTE_RECURSIVE_MATRIX_005.rs) | P1 | memory | GEN_SQL_CTE | 1566132 | 3244117 | <span style="color:#dc2626">-8.14%</span> |
| 613 | [00776 CTE_RECURSIVE_MATRIX_069](crates/bench/sqlite_parity/cases/SQLITE_PARITY_776_CTE_RECURSIVE_MATRIX_069.rs) | P1 | memory | GEN_SQL_CTE | 1625174 | 3242765 | <span style="color:#dc2626">-8.09%</span> |
| 614 | [01117 INDEX_SCHEMA_PRAGMA_050](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1117_INDEX_SCHEMA_PRAGMA_050.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1694666 | 3242194 | <span style="color:#dc2626">-8.07%</span> |
| 615 | [00303 SCALAR_NULL_COALESCE_019](crates/bench/sqlite_parity/cases/SQLITE_PARITY_303_SCALAR_NULL_COALESCE_019.rs) | P1 | memory | GEN_SQL_SCALAR | 1494447 | 3240871 | <span style="color:#dc2626">-8.03%</span> |
| 616 | [00791 WINDOW_PARTITION_SUM_004](crates/bench/sqlite_parity/cases/SQLITE_PARITY_791_WINDOW_PARTITION_SUM_004.rs) | P2 | memory | GEN_SQL_WINDOW | 1636736 | 3240500 | <span style="color:#dc2626">-8.02%</span> |
| 617 | [01039 JSON_EXTRACT_SET_032](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1039_JSON_EXTRACT_SET_032.rs) | P2 | memory | GEN_SQL_JSON | 1612951 | 3239949 | <span style="color:#dc2626">-8.00%</span> |
| 618 | [00748 CTE_RECURSIVE_MATRIX_041](crates/bench/sqlite_parity/cases/SQLITE_PARITY_748_CTE_RECURSIVE_MATRIX_041.rs) | P1 | memory | GEN_SQL_CTE | 1565782 | 3239809 | <span style="color:#dc2626">-7.99%</span> |
| 619 | [00803 WINDOW_PARTITION_SUM_016](crates/bench/sqlite_parity/cases/SQLITE_PARITY_803_WINDOW_PARTITION_SUM_016.rs) | P2 | memory | GEN_SQL_WINDOW | 2141882 | 3238376 | <span style="color:#dc2626">-7.95%</span> |
| 620 | [00833 WINDOW_PARTITION_SUM_046](crates/bench/sqlite_parity/cases/SQLITE_PARITY_833_WINDOW_PARTITION_SUM_046.rs) | P2 | memory | GEN_SQL_WINDOW | 1681080 | 3238015 | <span style="color:#dc2626">-7.93%</span> |
| 621 | [00820 WINDOW_PARTITION_SUM_033](crates/bench/sqlite_parity/cases/SQLITE_PARITY_820_WINDOW_PARTITION_SUM_033.rs) | P2 | memory | GEN_SQL_WINDOW | 1914091 | 3237404 | <span style="color:#dc2626">-7.91%</span> |
| 622 | [00709 CTE_RECURSIVE_MATRIX_002](crates/bench/sqlite_parity/cases/SQLITE_PARITY_709_CTE_RECURSIVE_MATRIX_002.rs) | P1 | memory | GEN_SQL_CTE | 1579187 | 3235952 | <span style="color:#dc2626">-7.87%</span> |
| 623 | [00727 CTE_RECURSIVE_MATRIX_020](crates/bench/sqlite_parity/cases/SQLITE_PARITY_727_CTE_RECURSIVE_MATRIX_020.rs) | P1 | memory | GEN_SQL_CTE | 1607922 | 3233707 | <span style="color:#dc2626">-7.79%</span> |
| 624 | [00794 WINDOW_PARTITION_SUM_007](crates/bench/sqlite_parity/cases/SQLITE_PARITY_794_WINDOW_PARTITION_SUM_007.rs) | P2 | memory | GEN_SQL_WINDOW | 1693403 | 3233617 | <span style="color:#dc2626">-7.79%</span> |
| 625 | [00055 JOINS_RIGHT_FULL_OUTER](crates/bench/sqlite_parity/cases/SQLITE_PARITY_055_JOINS_RIGHT_FULL_OUTER.rs) | P0 | memory | SQL_JOIN | 1683395 | 3233427 | <span style="color:#dc2626">-7.78%</span> |
| 626 | [00809 WINDOW_PARTITION_SUM_022](crates/bench/sqlite_parity/cases/SQLITE_PARITY_809_WINDOW_PARTITION_SUM_022.rs) | P2 | memory | GEN_SQL_WINDOW | 1695888 | 3232826 | <span style="color:#dc2626">-7.76%</span> |
| 627 | [00765 CTE_RECURSIVE_MATRIX_058](crates/bench/sqlite_parity/cases/SQLITE_PARITY_765_CTE_RECURSIVE_MATRIX_058.rs) | P1 | memory | GEN_SQL_CTE | 1632989 | 3232605 | <span style="color:#dc2626">-7.75%</span> |
| 628 | [00797 WINDOW_PARTITION_SUM_010](crates/bench/sqlite_parity/cases/SQLITE_PARITY_797_WINDOW_PARTITION_SUM_010.rs) | P2 | memory | GEN_SQL_WINDOW | 1634021 | 3232144 | <span style="color:#dc2626">-7.74%</span> |
| 629 | [00864 WINDOW_PARTITION_SUM_077](crates/bench/sqlite_parity/cases/SQLITE_PARITY_864_WINDOW_PARTITION_SUM_077.rs) | P2 | memory | GEN_SQL_WINDOW | 1708122 | 3231192 | <span style="color:#dc2626">-7.71%</span> |
| 630 | [00767 CTE_RECURSIVE_MATRIX_060](crates/bench/sqlite_parity/cases/SQLITE_PARITY_767_CTE_RECURSIVE_MATRIX_060.rs) | P1 | memory | GEN_SQL_CTE | 2169955 | 3231013 | <span style="color:#dc2626">-7.70%</span> |
| 631 | [00793 WINDOW_PARTITION_SUM_006](crates/bench/sqlite_parity/cases/SQLITE_PARITY_793_WINDOW_PARTITION_SUM_006.rs) | P2 | memory | GEN_SQL_WINDOW | 1592954 | 3230512 | <span style="color:#dc2626">-7.68%</span> |
| 632 | [00725 CTE_RECURSIVE_MATRIX_018](crates/bench/sqlite_parity/cases/SQLITE_PARITY_725_CTE_RECURSIVE_MATRIX_018.rs) | P1 | memory | GEN_SQL_CTE | 1538110 | 3229749 | <span style="color:#dc2626">-7.66%</span> |
| 633 | [00863 WINDOW_PARTITION_SUM_076](crates/bench/sqlite_parity/cases/SQLITE_PARITY_863_WINDOW_PARTITION_SUM_076.rs) | P2 | memory | GEN_SQL_WINDOW | 1654701 | 3229650 | <span style="color:#dc2626">-7.66%</span> |
| 634 | [00812 WINDOW_PARTITION_SUM_025](crates/bench/sqlite_parity/cases/SQLITE_PARITY_812_WINDOW_PARTITION_SUM_025.rs) | P2 | memory | GEN_SQL_WINDOW | 1645012 | 3229019 | <span style="color:#dc2626">-7.63%</span> |
| 635 | [01025 JSON_EXTRACT_SET_018](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1025_JSON_EXTRACT_SET_018.rs) | P2 | memory | GEN_SQL_JSON | 1652546 | 3228708 | <span style="color:#dc2626">-7.62%</span> |
| 636 | [00479 DML_WHERE_ORDER_LIMIT_092](crates/bench/sqlite_parity/cases/SQLITE_PARITY_479_DML_WHERE_ORDER_LIMIT_092.rs) | P1 | memory | GEN_SQL_DML | 1574809 | 3227866 | <span style="color:#dc2626">-7.60%</span> |
| 637 | [00787 CTE_RECURSIVE_MATRIX_080](crates/bench/sqlite_parity/cases/SQLITE_PARITY_787_CTE_RECURSIVE_MATRIX_080.rs) | P1 | memory | GEN_SQL_CTE | 1702751 | 3226734 | <span style="color:#dc2626">-7.56%</span> |
| 638 | [00044 ANALYZE_SQLITE_STAT1](crates/bench/sqlite_parity/cases/SQLITE_PARITY_044_ANALYZE_SQLITE_STAT1.rs) | P0 | memory | SQL_ANALYZE | 1699224 | 3226724 | <span style="color:#dc2626">-7.56%</span> |
| 639 | [00726 CTE_RECURSIVE_MATRIX_019](crates/bench/sqlite_parity/cases/SQLITE_PARITY_726_CTE_RECURSIVE_MATRIX_019.rs) | P1 | memory | GEN_SQL_CTE | 1607822 | 3226394 | <span style="color:#dc2626">-7.55%</span> |
| 640 | [00781 CTE_RECURSIVE_MATRIX_074](crates/bench/sqlite_parity/cases/SQLITE_PARITY_781_CTE_RECURSIVE_MATRIX_074.rs) | P1 | memory | GEN_SQL_CTE | 1663016 | 3226083 | <span style="color:#dc2626">-7.54%</span> |
| 641 | [00860 WINDOW_PARTITION_SUM_073](crates/bench/sqlite_parity/cases/SQLITE_PARITY_860_WINDOW_PARTITION_SUM_073.rs) | P2 | memory | GEN_SQL_WINDOW | 1664189 | 3225853 | <span style="color:#dc2626">-7.53%</span> |
| 642 | [00774 CTE_RECURSIVE_MATRIX_067](crates/bench/sqlite_parity/cases/SQLITE_PARITY_774_CTE_RECURSIVE_MATRIX_067.rs) | P1 | memory | GEN_SQL_CTE | 1533380 | 3224199 | <span style="color:#dc2626">-7.47%</span> |
| 643 | [00743 CTE_RECURSIVE_MATRIX_036](crates/bench/sqlite_parity/cases/SQLITE_PARITY_743_CTE_RECURSIVE_MATRIX_036.rs) | P1 | memory | GEN_SQL_CTE | 1565421 | 3222707 | <span style="color:#dc2626">-7.42%</span> |
| 644 | [00780 CTE_RECURSIVE_MATRIX_073](crates/bench/sqlite_parity/cases/SQLITE_PARITY_780_CTE_RECURSIVE_MATRIX_073.rs) | P1 | memory | GEN_SQL_CTE | 1579578 | 3221384 | <span style="color:#dc2626">-7.38%</span> |
| 645 | [00828 WINDOW_PARTITION_SUM_041](crates/bench/sqlite_parity/cases/SQLITE_PARITY_828_WINDOW_PARTITION_SUM_041.rs) | P2 | memory | GEN_SQL_WINDOW | 1685288 | 3221134 | <span style="color:#dc2626">-7.37%</span> |
| 646 | [00825 WINDOW_PARTITION_SUM_038](crates/bench/sqlite_parity/cases/SQLITE_PARITY_825_WINDOW_PARTITION_SUM_038.rs) | P2 | memory | GEN_SQL_WINDOW | 1685849 | 3220041 | <span style="color:#dc2626">-7.33%</span> |
| 647 | [00735 CTE_RECURSIVE_MATRIX_028](crates/bench/sqlite_parity/cases/SQLITE_PARITY_735_CTE_RECURSIVE_MATRIX_028.rs) | P1 | memory | GEN_SQL_CTE | 1644541 | 3219099 | <span style="color:#dc2626">-7.30%</span> |
| 648 | [00838 WINDOW_PARTITION_SUM_051](crates/bench/sqlite_parity/cases/SQLITE_PARITY_838_WINDOW_PARTITION_SUM_051.rs) | P2 | memory | GEN_SQL_WINDOW | 1648248 | 3217917 | <span style="color:#dc2626">-7.26%</span> |
| 649 | [00826 WINDOW_PARTITION_SUM_039](crates/bench/sqlite_parity/cases/SQLITE_PARITY_826_WINDOW_PARTITION_SUM_039.rs) | P2 | memory | GEN_SQL_WINDOW | 1716146 | 3217257 | <span style="color:#dc2626">-7.24%</span> |
| 650 | [00040 INSTEAD_OF_TRIGGER_ON_VIEW](crates/bench/sqlite_parity/cases/SQLITE_PARITY_040_INSTEAD_OF_TRIGGER_ON_VIEW.rs) | P0 | memory | SQL_TRIGGER | 1623702 | 3216564 | <span style="color:#dc2626">-7.22%</span> |
| 651 | [00744 CTE_RECURSIVE_MATRIX_037](crates/bench/sqlite_parity/cases/SQLITE_PARITY_744_CTE_RECURSIVE_MATRIX_037.rs) | P1 | memory | GEN_SQL_CTE | 1533391 | 3215543 | <span style="color:#dc2626">-7.18%</span> |
| 652 | [00215 TRANSACTION_SYNONYMS_END_COMMIT_TRANSACTION](crates/bench/sqlite_parity/cases/SQLITE_PARITY_215_TRANSACTION_SYNONYMS_END_COMMIT_TRANSACTION.rs) | P0 | memory | SQL_TRANSACTION | 1660661 | 3215252 | <span style="color:#dc2626">-7.18%</span> |
| 653 | [00335 SCALAR_NULL_COALESCE_027](crates/bench/sqlite_parity/cases/SQLITE_PARITY_335_SCALAR_NULL_COALESCE_027.rs) | P1 | memory | GEN_SQL_SCALAR | 1485590 | 3214831 | <span style="color:#dc2626">-7.16%</span> |
| 654 | [00117 DOT_DUMP](crates/bench/sqlite_parity/cases/SQLITE_PARITY_117_DOT_DUMP.rs) | P0 | memory | CLI_DOT_COMMAND | 1682092 | 3214421 | <span style="color:#dc2626">-7.15%</span> |
| 655 | [00789 WINDOW_PARTITION_SUM_002](crates/bench/sqlite_parity/cases/SQLITE_PARITY_789_WINDOW_PARTITION_SUM_002.rs) | P2 | memory | GEN_SQL_WINDOW | 1694776 | 3214341 | <span style="color:#dc2626">-7.14%</span> |
| 656 | [00752 CTE_RECURSIVE_MATRIX_045](crates/bench/sqlite_parity/cases/SQLITE_PARITY_752_CTE_RECURSIVE_MATRIX_045.rs) | P1 | memory | GEN_SQL_CTE | 1631597 | 3214340 | <span style="color:#dc2626">-7.14%</span> |
| 657 | [00728 CTE_RECURSIVE_MATRIX_021](crates/bench/sqlite_parity/cases/SQLITE_PARITY_728_CTE_RECURSIVE_MATRIX_021.rs) | P1 | memory | GEN_SQL_CTE | 1629763 | 3213850 | <span style="color:#dc2626">-7.13%</span> |
| 658 | [00013 INSERT_SELECT](crates/bench/sqlite_parity/cases/SQLITE_PARITY_013_INSERT_SELECT.rs) | P0 | memory | SQL_INSERT | 1586030 | 3213369 | <span style="color:#dc2626">-7.11%</span> |
| 659 | [00867 WINDOW_PARTITION_SUM_080](crates/bench/sqlite_parity/cases/SQLITE_PARITY_867_WINDOW_PARTITION_SUM_080.rs) | P2 | memory | GEN_SQL_WINDOW | 1647276 | 3213269 | <span style="color:#dc2626">-7.11%</span> |
| 660 | [00719 CTE_RECURSIVE_MATRIX_012](crates/bench/sqlite_parity/cases/SQLITE_PARITY_719_CTE_RECURSIVE_MATRIX_012.rs) | P1 | memory | GEN_SQL_CTE | 1614504 | 3213088 | <span style="color:#dc2626">-7.10%</span> |
| 661 | [00834 WINDOW_PARTITION_SUM_047](crates/bench/sqlite_parity/cases/SQLITE_PARITY_834_WINDOW_PARTITION_SUM_047.rs) | P2 | memory | GEN_SQL_WINDOW | 1637097 | 3212316 | <span style="color:#dc2626">-7.08%</span> |
| 662 | [00802 WINDOW_PARTITION_SUM_015](crates/bench/sqlite_parity/cases/SQLITE_PARITY_802_WINDOW_PARTITION_SUM_015.rs) | P2 | memory | GEN_SQL_WINDOW | 1673687 | 3209572 | <span style="color:#dc2626">-6.99%</span> |
| 663 | [00810 WINDOW_PARTITION_SUM_023](crates/bench/sqlite_parity/cases/SQLITE_PARITY_810_WINDOW_PARTITION_SUM_023.rs) | P2 | memory | GEN_SQL_WINDOW | 1655652 | 3209521 | <span style="color:#dc2626">-6.98%</span> |
| 664 | [00854 WINDOW_PARTITION_SUM_067](crates/bench/sqlite_parity/cases/SQLITE_PARITY_854_WINDOW_PARTITION_SUM_067.rs) | P2 | memory | GEN_SQL_WINDOW | 1689887 | 3208951 | <span style="color:#dc2626">-6.97%</span> |
| 665 | [00808 WINDOW_PARTITION_SUM_021](crates/bench/sqlite_parity/cases/SQLITE_PARITY_808_WINDOW_PARTITION_SUM_021.rs) | P2 | memory | GEN_SQL_WINDOW | 1636566 | 3207748 | <span style="color:#dc2626">-6.92%</span> |
| 666 | [00847 WINDOW_PARTITION_SUM_060](crates/bench/sqlite_parity/cases/SQLITE_PARITY_847_WINDOW_PARTITION_SUM_060.rs) | P2 | memory | GEN_SQL_WINDOW | 2592184 | 3206566 | <span style="color:#dc2626">-6.89%</span> |
| 667 | [00729 CTE_RECURSIVE_MATRIX_022](crates/bench/sqlite_parity/cases/SQLITE_PARITY_729_CTE_RECURSIVE_MATRIX_022.rs) | P1 | memory | GEN_SQL_CTE | 1859678 | 3206345 | <span style="color:#dc2626">-6.88%</span> |
| 668 | [00593 AGG_GROUP_HAVING_086](crates/bench/sqlite_parity/cases/SQLITE_PARITY_593_AGG_GROUP_HAVING_086.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1751663 | 3205634 | <span style="color:#dc2626">-6.85%</span> |
| 669 | [00782 CTE_RECURSIVE_MATRIX_075](crates/bench/sqlite_parity/cases/SQLITE_PARITY_782_CTE_RECURSIVE_MATRIX_075.rs) | P1 | memory | GEN_SQL_CTE | 1582663 | 3205284 | <span style="color:#dc2626">-6.84%</span> |
| 670 | [00770 CTE_RECURSIVE_MATRIX_063](crates/bench/sqlite_parity/cases/SQLITE_PARITY_770_CTE_RECURSIVE_MATRIX_063.rs) | P1 | memory | GEN_SQL_CTE | 1844690 | 3204012 | <span style="color:#dc2626">-6.80%</span> |
| 671 | [00753 CTE_RECURSIVE_MATRIX_046](crates/bench/sqlite_parity/cases/SQLITE_PARITY_753_CTE_RECURSIVE_MATRIX_046.rs) | P1 | memory | GEN_SQL_CTE | 1563237 | 3203530 | <span style="color:#dc2626">-6.78%</span> |
| 672 | [00837 WINDOW_PARTITION_SUM_050](crates/bench/sqlite_parity/cases/SQLITE_PARITY_837_WINDOW_PARTITION_SUM_050.rs) | P2 | memory | GEN_SQL_WINDOW | 1701939 | 3203460 | <span style="color:#dc2626">-6.78%</span> |
| 673 | [00850 WINDOW_PARTITION_SUM_063](crates/bench/sqlite_parity/cases/SQLITE_PARITY_850_WINDOW_PARTITION_SUM_063.rs) | P2 | memory | GEN_SQL_WINDOW | 1876219 | 3203249 | <span style="color:#dc2626">-6.77%</span> |
| 674 | [00760 CTE_RECURSIVE_MATRIX_053](crates/bench/sqlite_parity/cases/SQLITE_PARITY_760_CTE_RECURSIVE_MATRIX_053.rs) | P1 | memory | GEN_SQL_CTE | 1892470 | 3202068 | <span style="color:#dc2626">-6.74%</span> |
| 675 | [00819 WINDOW_PARTITION_SUM_032](crates/bench/sqlite_parity/cases/SQLITE_PARITY_819_WINDOW_PARTITION_SUM_032.rs) | P2 | memory | GEN_SQL_WINDOW | 1781511 | 3201847 | <span style="color:#dc2626">-6.73%</span> |
| 676 | [00714 CTE_RECURSIVE_MATRIX_007](crates/bench/sqlite_parity/cases/SQLITE_PARITY_714_CTE_RECURSIVE_MATRIX_007.rs) | P1 | memory | GEN_SQL_CTE | 1571463 | 3201637 | <span style="color:#dc2626">-6.72%</span> |
| 677 | [00818 WINDOW_PARTITION_SUM_031](crates/bench/sqlite_parity/cases/SQLITE_PARITY_818_WINDOW_PARTITION_SUM_031.rs) | P2 | memory | GEN_SQL_WINDOW | 1625004 | 3201226 | <span style="color:#dc2626">-6.71%</span> |
| 678 | [00839 WINDOW_PARTITION_SUM_052](crates/bench/sqlite_parity/cases/SQLITE_PARITY_839_WINDOW_PARTITION_SUM_052.rs) | P2 | memory | GEN_SQL_WINDOW | 1638389 | 3199854 | <span style="color:#dc2626">-6.66%</span> |
| 679 | [00816 WINDOW_PARTITION_SUM_029](crates/bench/sqlite_parity/cases/SQLITE_PARITY_816_WINDOW_PARTITION_SUM_029.rs) | P2 | memory | GEN_SQL_WINDOW | 1708542 | 3199262 | <span style="color:#dc2626">-6.64%</span> |
| 680 | [00291 SCALAR_NULL_COALESCE_016](crates/bench/sqlite_parity/cases/SQLITE_PARITY_291_SCALAR_NULL_COALESCE_016.rs) | P1 | memory | GEN_SQL_SCALAR | 1893141 | 3199152 | <span style="color:#dc2626">-6.64%</span> |
| 681 | [00799 WINDOW_PARTITION_SUM_012](crates/bench/sqlite_parity/cases/SQLITE_PARITY_799_WINDOW_PARTITION_SUM_012.rs) | P2 | memory | GEN_SQL_WINDOW | 1645884 | 3198831 | <span style="color:#dc2626">-6.63%</span> |
| 682 | [00745 CTE_RECURSIVE_MATRIX_038](crates/bench/sqlite_parity/cases/SQLITE_PARITY_745_CTE_RECURSIVE_MATRIX_038.rs) | P1 | memory | GEN_SQL_CTE | 1659329 | 3198621 | <span style="color:#dc2626">-6.62%</span> |
| 683 | [00814 WINDOW_PARTITION_SUM_027](crates/bench/sqlite_parity/cases/SQLITE_PARITY_814_WINDOW_PARTITION_SUM_027.rs) | P2 | memory | GEN_SQL_WINDOW | 1628691 | 3197669 | <span style="color:#dc2626">-6.59%</span> |
| 684 | [00242 SCALAR_STRING_004](crates/bench/sqlite_parity/cases/SQLITE_PARITY_242_SCALAR_STRING_004.rs) | P1 | memory | GEN_SQL_SCALAR | 1524474 | 3197108 | <span style="color:#dc2626">-6.57%</span> |
| 685 | [00855 WINDOW_PARTITION_SUM_068](crates/bench/sqlite_parity/cases/SQLITE_PARITY_855_WINDOW_PARTITION_SUM_068.rs) | P2 | memory | GEN_SQL_WINDOW | 2441889 | 3196026 | <span style="color:#dc2626">-6.53%</span> |
| 686 | [00772 CTE_RECURSIVE_MATRIX_065](crates/bench/sqlite_parity/cases/SQLITE_PARITY_772_CTE_RECURSIVE_MATRIX_065.rs) | P1 | memory | GEN_SQL_CTE | 1627239 | 3195636 | <span style="color:#dc2626">-6.52%</span> |
| 687 | [00817 WINDOW_PARTITION_SUM_030](crates/bench/sqlite_parity/cases/SQLITE_PARITY_817_WINDOW_PARTITION_SUM_030.rs) | P2 | memory | GEN_SQL_WINDOW | 1946251 | 3195074 | <span style="color:#dc2626">-6.50%</span> |
| 688 | [00720 CTE_RECURSIVE_MATRIX_013](crates/bench/sqlite_parity/cases/SQLITE_PARITY_720_CTE_RECURSIVE_MATRIX_013.rs) | P1 | memory | GEN_SQL_CTE | 1883914 | 3194703 | <span style="color:#dc2626">-6.49%</span> |
| 689 | [00843 WINDOW_PARTITION_SUM_056](crates/bench/sqlite_parity/cases/SQLITE_PARITY_843_WINDOW_PARTITION_SUM_056.rs) | P2 | memory | GEN_SQL_WINDOW | 1620055 | 3193531 | <span style="color:#dc2626">-6.45%</span> |
| 690 | [00768 CTE_RECURSIVE_MATRIX_061](crates/bench/sqlite_parity/cases/SQLITE_PARITY_768_CTE_RECURSIVE_MATRIX_061.rs) | P1 | memory | GEN_SQL_CTE | 1648849 | 3193461 | <span style="color:#dc2626">-6.45%</span> |
| 691 | [00806 WINDOW_PARTITION_SUM_019](crates/bench/sqlite_parity/cases/SQLITE_PARITY_806_WINDOW_PARTITION_SUM_019.rs) | P2 | memory | GEN_SQL_WINDOW | 1675940 | 3193041 | <span style="color:#dc2626">-6.43%</span> |
| 692 | [00798 WINDOW_PARTITION_SUM_011](crates/bench/sqlite_parity/cases/SQLITE_PARITY_798_WINDOW_PARTITION_SUM_011.rs) | P2 | memory | GEN_SQL_WINDOW | 1574729 | 3191878 | <span style="color:#dc2626">-6.40%</span> |
| 693 | [00771 CTE_RECURSIVE_MATRIX_064](crates/bench/sqlite_parity/cases/SQLITE_PARITY_771_CTE_RECURSIVE_MATRIX_064.rs) | P1 | memory | GEN_SQL_CTE | 1602502 | 3191588 | <span style="color:#dc2626">-6.39%</span> |
| 694 | [00754 CTE_RECURSIVE_MATRIX_047](crates/bench/sqlite_parity/cases/SQLITE_PARITY_754_CTE_RECURSIVE_MATRIX_047.rs) | P1 | memory | GEN_SQL_CTE | 1594837 | 3190586 | <span style="color:#dc2626">-6.35%</span> |
| 695 | [00786 CTE_RECURSIVE_MATRIX_079](crates/bench/sqlite_parity/cases/SQLITE_PARITY_786_CTE_RECURSIVE_MATRIX_079.rs) | P1 | memory | GEN_SQL_CTE | 1651474 | 3190065 | <span style="color:#dc2626">-6.34%</span> |
| 696 | [00367 SCALAR_NULL_COALESCE_035](crates/bench/sqlite_parity/cases/SQLITE_PARITY_367_SCALAR_NULL_COALESCE_035.rs) | P1 | memory | GEN_SQL_SCALAR | 1599426 | 3189945 | <span style="color:#dc2626">-6.33%</span> |
| 697 | [00795 WINDOW_PARTITION_SUM_008](crates/bench/sqlite_parity/cases/SQLITE_PARITY_795_WINDOW_PARTITION_SUM_008.rs) | P2 | memory | GEN_SQL_WINDOW | 1670620 | 3188963 | <span style="color:#dc2626">-6.30%</span> |
| 698 | [00851 WINDOW_PARTITION_SUM_064](crates/bench/sqlite_parity/cases/SQLITE_PARITY_851_WINDOW_PARTITION_SUM_064.rs) | P2 | memory | GEN_SQL_WINDOW | 1629012 | 3188562 | <span style="color:#dc2626">-6.29%</span> |
| 699 | [00730 CTE_RECURSIVE_MATRIX_023](crates/bench/sqlite_parity/cases/SQLITE_PARITY_730_CTE_RECURSIVE_MATRIX_023.rs) | P1 | memory | GEN_SQL_CTE | 1820023 | 3188522 | <span style="color:#dc2626">-6.28%</span> |
| 700 | [00723 CTE_RECURSIVE_MATRIX_016](crates/bench/sqlite_parity/cases/SQLITE_PARITY_723_CTE_RECURSIVE_MATRIX_016.rs) | P1 | memory | GEN_SQL_CTE | 1646285 | 3187189 | <span style="color:#dc2626">-6.24%</span> |
| 701 | [00710 CTE_RECURSIVE_MATRIX_003](crates/bench/sqlite_parity/cases/SQLITE_PARITY_710_CTE_RECURSIVE_MATRIX_003.rs) | P1 | memory | GEN_SQL_CTE | 1732949 | 3185837 | <span style="color:#dc2626">-6.19%</span> |
| 702 | [00849 WINDOW_PARTITION_SUM_062](crates/bench/sqlite_parity/cases/SQLITE_PARITY_849_WINDOW_PARTITION_SUM_062.rs) | P2 | memory | GEN_SQL_WINDOW | 1951632 | 3185086 | <span style="color:#dc2626">-6.17%</span> |
| 703 | [00832 WINDOW_PARTITION_SUM_045](crates/bench/sqlite_parity/cases/SQLITE_PARITY_832_WINDOW_PARTITION_SUM_045.rs) | P2 | memory | GEN_SQL_WINDOW | 1928197 | 3184905 | <span style="color:#dc2626">-6.16%</span> |
| 704 | [00796 WINDOW_PARTITION_SUM_009](crates/bench/sqlite_parity/cases/SQLITE_PARITY_796_WINDOW_PARTITION_SUM_009.rs) | P2 | memory | GEN_SQL_WINDOW | 1688514 | 3184003 | <span style="color:#dc2626">-6.13%</span> |
| 705 | [00715 CTE_RECURSIVE_MATRIX_008](crates/bench/sqlite_parity/cases/SQLITE_PARITY_715_CTE_RECURSIVE_MATRIX_008.rs) | P1 | memory | GEN_SQL_CTE | 1570461 | 3183252 | <span style="color:#dc2626">-6.11%</span> |
| 706 | [00821 WINDOW_PARTITION_SUM_034](crates/bench/sqlite_parity/cases/SQLITE_PARITY_821_WINDOW_PARTITION_SUM_034.rs) | P2 | memory | GEN_SQL_WINDOW | 1871931 | 3182921 | <span style="color:#dc2626">-6.10%</span> |
| 707 | [00866 WINDOW_PARTITION_SUM_079](crates/bench/sqlite_parity/cases/SQLITE_PARITY_866_WINDOW_PARTITION_SUM_079.rs) | P2 | memory | GEN_SQL_WINDOW | 1802720 | 3182080 | <span style="color:#dc2626">-6.07%</span> |
| 708 | [00078 ON_CONFLICT_ALGORITHMS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_078_ON_CONFLICT_ALGORITHMS.rs) | P0 | memory | SQL_CONFLICT | 1610827 | 3181449 | <span style="color:#dc2626">-6.05%</span> |
| 709 | [00758 CTE_RECURSIVE_MATRIX_051](crates/bench/sqlite_parity/cases/SQLITE_PARITY_758_CTE_RECURSIVE_MATRIX_051.rs) | P1 | memory | GEN_SQL_CTE | 1817799 | 3180988 | <span style="color:#dc2626">-6.03%</span> |
| 710 | [00857 WINDOW_PARTITION_SUM_070](crates/bench/sqlite_parity/cases/SQLITE_PARITY_857_WINDOW_PARTITION_SUM_070.rs) | P2 | memory | GEN_SQL_WINDOW | 1660120 | 3179535 | <span style="color:#dc2626">-5.98%</span> |
| 711 | [00761 CTE_RECURSIVE_MATRIX_054](crates/bench/sqlite_parity/cases/SQLITE_PARITY_761_CTE_RECURSIVE_MATRIX_054.rs) | P1 | memory | GEN_SQL_CTE | 1585659 | 3179024 | <span style="color:#dc2626">-5.97%</span> |
| 712 | [00724 CTE_RECURSIVE_MATRIX_017](crates/bench/sqlite_parity/cases/SQLITE_PARITY_724_CTE_RECURSIVE_MATRIX_017.rs) | P1 | memory | GEN_SQL_CTE | 1541766 | 3178022 | <span style="color:#dc2626">-5.93%</span> |
| 713 | [00805 WINDOW_PARTITION_SUM_018](crates/bench/sqlite_parity/cases/SQLITE_PARITY_805_WINDOW_PARTITION_SUM_018.rs) | P2 | memory | GEN_SQL_WINDOW | 1679507 | 3176819 | <span style="color:#dc2626">-5.89%</span> |
| 714 | [00827 WINDOW_PARTITION_SUM_040](crates/bench/sqlite_parity/cases/SQLITE_PARITY_827_WINDOW_PARTITION_SUM_040.rs) | P2 | memory | GEN_SQL_WINDOW | 1670279 | 3176169 | <span style="color:#dc2626">-5.87%</span> |
| 715 | [00731 CTE_RECURSIVE_MATRIX_024](crates/bench/sqlite_parity/cases/SQLITE_PARITY_731_CTE_RECURSIVE_MATRIX_024.rs) | P1 | memory | GEN_SQL_CTE | 1561785 | 3175697 | <span style="color:#dc2626">-5.86%</span> |
| 716 | [00811 WINDOW_PARTITION_SUM_024](crates/bench/sqlite_parity/cases/SQLITE_PARITY_811_WINDOW_PARTITION_SUM_024.rs) | P2 | memory | GEN_SQL_WINDOW | 1655672 | 3174214 | <span style="color:#dc2626">-5.81%</span> |
| 717 | [00773 CTE_RECURSIVE_MATRIX_066](crates/bench/sqlite_parity/cases/SQLITE_PARITY_773_CTE_RECURSIVE_MATRIX_066.rs) | P1 | memory | GEN_SQL_CTE | 1535806 | 3172652 | <span style="color:#dc2626">-5.76%</span> |
| 718 | [00842 WINDOW_PARTITION_SUM_055](crates/bench/sqlite_parity/cases/SQLITE_PARITY_842_WINDOW_PARTITION_SUM_055.rs) | P2 | memory | GEN_SQL_WINDOW | 1892751 | 3172051 | <span style="color:#dc2626">-5.74%</span> |
| 719 | [00750 CTE_RECURSIVE_MATRIX_043](crates/bench/sqlite_parity/cases/SQLITE_PARITY_750_CTE_RECURSIVE_MATRIX_043.rs) | P1 | memory | GEN_SQL_CTE | 1572004 | 3170869 | <span style="color:#dc2626">-5.70%</span> |
| 720 | [00592 AGG_GROUP_HAVING_085](crates/bench/sqlite_parity/cases/SQLITE_PARITY_592_AGG_GROUP_HAVING_085.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1737868 | 3169166 | <span style="color:#dc2626">-5.64%</span> |
| 721 | [00778 CTE_RECURSIVE_MATRIX_071](crates/bench/sqlite_parity/cases/SQLITE_PARITY_778_CTE_RECURSIVE_MATRIX_071.rs) | P1 | memory | GEN_SQL_CTE | 1784656 | 3166290 | <span style="color:#dc2626">-5.54%</span> |
| 722 | [00019 REPLACE_INTO](crates/bench/sqlite_parity/cases/SQLITE_PARITY_019_REPLACE_INTO.rs) | P0 | memory | SQL_REPLACE | 1651314 | 3165448 | <span style="color:#dc2626">-5.51%</span> |
| 723 | [00822 WINDOW_PARTITION_SUM_035](crates/bench/sqlite_parity/cases/SQLITE_PARITY_822_WINDOW_PARTITION_SUM_035.rs) | P2 | memory | GEN_SQL_WINDOW | 1692912 | 3164807 | <span style="color:#dc2626">-5.49%</span> |
| 724 | [00807 WINDOW_PARTITION_SUM_020](crates/bench/sqlite_parity/cases/SQLITE_PARITY_807_WINDOW_PARTITION_SUM_020.rs) | P2 | memory | GEN_SQL_WINDOW | 1606349 | 3161992 | <span style="color:#dc2626">-5.40%</span> |
| 725 | [00736 CTE_RECURSIVE_MATRIX_029](crates/bench/sqlite_parity/cases/SQLITE_PARITY_736_CTE_RECURSIVE_MATRIX_029.rs) | P1 | memory | GEN_SQL_CTE | 1913790 | 3161792 | <span style="color:#dc2626">-5.39%</span> |
| 726 | [00853 WINDOW_PARTITION_SUM_066](crates/bench/sqlite_parity/cases/SQLITE_PARITY_853_WINDOW_PARTITION_SUM_066.rs) | P2 | memory | GEN_SQL_WINDOW | 1644300 | 3161371 | <span style="color:#dc2626">-5.38%</span> |
| 727 | [00045 REINDEX_COMMAND](crates/bench/sqlite_parity/cases/SQLITE_PARITY_045_REINDEX_COMMAND.rs) | P0 | memory | SQL_REINDEX | 1577875 | 3161120 | <span style="color:#dc2626">-5.37%</span> |
| 728 | [00708 CTE_RECURSIVE_MATRIX_001](crates/bench/sqlite_parity/cases/SQLITE_PARITY_708_CTE_RECURSIVE_MATRIX_001.rs) | P1 | memory | GEN_SQL_CTE | 1637828 | 3160689 | <span style="color:#dc2626">-5.36%</span> |
| 729 | [00865 WINDOW_PARTITION_SUM_078](crates/bench/sqlite_parity/cases/SQLITE_PARITY_865_WINDOW_PARTITION_SUM_078.rs) | P2 | memory | GEN_SQL_WINDOW | 1655612 | 3160489 | <span style="color:#dc2626">-5.35%</span> |
| 730 | [00381 SCALAR_CAST_TYPEOF_039](crates/bench/sqlite_parity/cases/SQLITE_PARITY_381_SCALAR_CAST_TYPEOF_039.rs) | P1 | memory | GEN_SQL_SCALAR | 1795337 | 3160118 | <span style="color:#dc2626">-5.34%</span> |
| 731 | [00270 SCALAR_STRING_011](crates/bench/sqlite_parity/cases/SQLITE_PARITY_270_SCALAR_STRING_011.rs) | P1 | memory | GEN_SQL_SCALAR | 1494747 | 3158746 | <span style="color:#dc2626">-5.29%</span> |
| 732 | [00764 CTE_RECURSIVE_MATRIX_057](crates/bench/sqlite_parity/cases/SQLITE_PARITY_764_CTE_RECURSIVE_MATRIX_057.rs) | P1 | memory | GEN_SQL_CTE | 1752555 | 3158015 | <span style="color:#dc2626">-5.27%</span> |
| 733 | [00840 WINDOW_PARTITION_SUM_053](crates/bench/sqlite_parity/cases/SQLITE_PARITY_840_WINDOW_PARTITION_SUM_053.rs) | P2 | memory | GEN_SQL_WINDOW | 2230059 | 3157312 | <span style="color:#dc2626">-5.24%</span> |
| 734 | [00756 CTE_RECURSIVE_MATRIX_049](crates/bench/sqlite_parity/cases/SQLITE_PARITY_756_CTE_RECURSIVE_MATRIX_049.rs) | P1 | memory | GEN_SQL_CTE | 1800696 | 3155900 | <span style="color:#dc2626">-5.20%</span> |
| 735 | [00733 CTE_RECURSIVE_MATRIX_026](crates/bench/sqlite_parity/cases/SQLITE_PARITY_733_CTE_RECURSIVE_MATRIX_026.rs) | P1 | memory | GEN_SQL_CTE | 1587663 | 3152985 | <span style="color:#dc2626">-5.10%</span> |
| 736 | [00010 GENERATED_COLUMNS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_010_GENERATED_COLUMNS.rs) | P0 | memory | SQL_DDL | 1603133 | 3150831 | <span style="color:#dc2626">-5.03%</span> |
| 737 | [00191 OPT_BATCH](crates/bench/sqlite_parity/cases/SQLITE_PARITY_191_OPT_BATCH.rs) | P2 | memory | CLI_OPTION | 1494848 | 3150160 | <span style="color:#dc2626">-5.01%</span> |
| 738 | [00107 DOT_HELP_PATTERN](crates/bench/sqlite_parity/cases/SQLITE_PARITY_107_DOT_HELP_PATTERN.rs) | P0 | memory | CLI_DOT_COMMAND | 1653899 | 3149388 | <span style="color:#f97316">-4.98%</span> |
| 739 | [00841 WINDOW_PARTITION_SUM_054](crates/bench/sqlite_parity/cases/SQLITE_PARITY_841_WINDOW_PARTITION_SUM_054.rs) | P2 | memory | GEN_SQL_WINDOW | 1733529 | 3148987 | <span style="color:#f97316">-4.97%</span> |
| 740 | [00852 WINDOW_PARTITION_SUM_065](crates/bench/sqlite_parity/cases/SQLITE_PARITY_852_WINDOW_PARTITION_SUM_065.rs) | P2 | memory | GEN_SQL_WINDOW | 1681822 | 3146101 | <span style="color:#f97316">-4.87%</span> |
| 741 | [00859 WINDOW_PARTITION_SUM_072](crates/bench/sqlite_parity/cases/SQLITE_PARITY_859_WINDOW_PARTITION_SUM_072.rs) | P2 | memory | GEN_SQL_WINDOW | 1703894 | 3145060 | <span style="color:#f97316">-4.84%</span> |
| 742 | [00777 CTE_RECURSIVE_MATRIX_070](crates/bench/sqlite_parity/cases/SQLITE_PARITY_777_CTE_RECURSIVE_MATRIX_070.rs) | P1 | memory | GEN_SQL_CTE | 1533351 | 3143767 | <span style="color:#f97316">-4.79%</span> |
| 743 | [00058 GROUP_BY_HAVING](crates/bench/sqlite_parity/cases/SQLITE_PARITY_058_GROUP_BY_HAVING.rs) | P0 | memory | SQL_AGGREGATE | 1601470 | 3142925 | <span style="color:#f97316">-4.76%</span> |
| 744 | [00196 OPT_MMAP](crates/bench/sqlite_parity/cases/SQLITE_PARITY_196_OPT_MMAP.rs) | P3 | memory | CLI_OPTION | 1581431 | 3142766 | <span style="color:#f97316">-4.76%</span> |
| 745 | [00578 AGG_GROUP_HAVING_071](crates/bench/sqlite_parity/cases/SQLITE_PARITY_578_AGG_GROUP_HAVING_071.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1693924 | 3142134 | <span style="color:#f97316">-4.74%</span> |
| 746 | [00716 CTE_RECURSIVE_MATRIX_009](crates/bench/sqlite_parity/cases/SQLITE_PARITY_716_CTE_RECURSIVE_MATRIX_009.rs) | P1 | memory | GEN_SQL_CTE | 1612471 | 3141282 | <span style="color:#f97316">-4.71%</span> |
| 747 | [00312 SCALAR_ARITH_022](crates/bench/sqlite_parity/cases/SQLITE_PARITY_312_SCALAR_ARITH_022.rs) | P1 | memory | GEN_SQL_SCALAR | 1560472 | 3140371 | <span style="color:#f97316">-4.68%</span> |
| 748 | [00293 SCALAR_CAST_TYPEOF_017](crates/bench/sqlite_parity/cases/SQLITE_PARITY_293_SCALAR_CAST_TYPEOF_017.rs) | P1 | memory | GEN_SQL_SCALAR | 1476744 | 3139789 | <span style="color:#f97316">-4.66%</span> |
| 749 | [00722 CTE_RECURSIVE_MATRIX_015](crates/bench/sqlite_parity/cases/SQLITE_PARITY_722_CTE_RECURSIVE_MATRIX_015.rs) | P1 | memory | GEN_SQL_CTE | 1668737 | 3138347 | <span style="color:#f97316">-4.61%</span> |
| 750 | [00757 CTE_RECURSIVE_MATRIX_050](crates/bench/sqlite_parity/cases/SQLITE_PARITY_757_CTE_RECURSIVE_MATRIX_050.rs) | P1 | memory | GEN_SQL_CTE | 1563117 | 3131534 | <span style="color:#f97316">-4.38%</span> |
| 751 | [00595 AGG_GROUP_HAVING_088](crates/bench/sqlite_parity/cases/SQLITE_PARITY_595_AGG_GROUP_HAVING_088.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1785417 | 3131013 | <span style="color:#f97316">-4.37%</span> |
| 752 | [00036 CREATE_VIEW](crates/bench/sqlite_parity/cases/SQLITE_PARITY_036_CREATE_VIEW.rs) | P0 | memory | SQL_VIEW | 1599196 | 3129661 | <span style="color:#f97316">-4.32%</span> |
| 753 | [00296 SCALAR_ARITH_018](crates/bench/sqlite_parity/cases/SQLITE_PARITY_296_SCALAR_ARITH_018.rs) | P1 | memory | GEN_SQL_SCALAR | 1555543 | 3127927 | <span style="color:#f97316">-4.26%</span> |
| 754 | [00214 DROP_TABLE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_214_DROP_TABLE.rs) | P0 | memory | SQL_DROP | 1935521 | 3127216 | <span style="color:#f97316">-4.24%</span> |
| 755 | [00026 FOREIGN_KEYS_CASCADE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_026_FOREIGN_KEYS_CASCADE.rs) | P0 | memory | SQL_FOREIGN_KEYS | 1723009 | 3126284 | <span style="color:#f97316">-4.21%</span> |
| 756 | [00148 DOT_OUTPUT_TEMPFILE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_148_DOT_OUTPUT_TEMPFILE.rs) | P1 | tempfile | CLI_TEMPFILE | 1570110 | 3125703 | <span style="color:#f97316">-4.19%</span> |
| 757 | [00122 DOT_CHANGES](crates/bench/sqlite_parity/cases/SQLITE_PARITY_122_DOT_CHANGES.rs) | P0 | memory | CLI_DOT_COMMAND | 1565591 | 3125433 | <span style="color:#f97316">-4.18%</span> |
| 758 | [00594 AGG_GROUP_HAVING_087](crates/bench/sqlite_parity/cases/SQLITE_PARITY_594_AGG_GROUP_HAVING_087.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1813390 | 3125041 | <span style="color:#f97316">-4.17%</span> |
| 759 | [00057 COMPOUND_SELECT_UNION_INTERSECT_EXCEPT](crates/bench/sqlite_parity/cases/SQLITE_PARITY_057_COMPOUND_SELECT_UNION_INTERSECT_EXCEPT.rs) | P0 | memory | SQL_SELECT | 1699926 | 3123870 | <span style="color:#f97316">-4.13%</span> |
| 760 | [00374 SCALAR_STRING_037](crates/bench/sqlite_parity/cases/SQLITE_PARITY_374_SCALAR_STRING_037.rs) | P1 | memory | GEN_SQL_SCALAR | 1476593 | 3123208 | <span style="color:#f97316">-4.11%</span> |
| 761 | [00006 ROWID_INTEGER_PRIMARY_KEY](crates/bench/sqlite_parity/cases/SQLITE_PARITY_006_ROWID_INTEGER_PRIMARY_KEY.rs) | P0 | memory | SQL_ROWID | 1612551 | 3123018 | <span style="color:#f97316">-4.10%</span> |
| 762 | [00721 CTE_RECURSIVE_MATRIX_014](crates/bench/sqlite_parity/cases/SQLITE_PARITY_721_CTE_RECURSIVE_MATRIX_014.rs) | P1 | memory | GEN_SQL_CTE | 1530175 | 3122958 | <span style="color:#f97316">-4.10%</span> |
| 763 | [00405 DML_WHERE_ORDER_LIMIT_018](crates/bench/sqlite_parity/cases/SQLITE_PARITY_405_DML_WHERE_ORDER_LIMIT_018.rs) | P1 | memory | GEN_SQL_DML | 2003931 | 3118560 | <span style="color:#f97316">-3.95%</span> |
| 764 | [00779 CTE_RECURSIVE_MATRIX_072](crates/bench/sqlite_parity/cases/SQLITE_PARITY_779_CTE_RECURSIVE_MATRIX_072.rs) | P1 | memory | GEN_SQL_CTE | 1538099 | 3115674 | <span style="color:#f97316">-3.86%</span> |
| 765 | [00742 CTE_RECURSIVE_MATRIX_035](crates/bench/sqlite_parity/cases/SQLITE_PARITY_742_CTE_RECURSIVE_MATRIX_035.rs) | P1 | memory | GEN_SQL_CTE | 1681260 | 3114983 | <span style="color:#f97316">-3.83%</span> |
| 766 | [00216 ROLLBACK_TRANSACTION_SYNTAX](crates/bench/sqlite_parity/cases/SQLITE_PARITY_216_ROLLBACK_TRANSACTION_SYNTAX.rs) | P0 | memory | SQL_TRANSACTION | 2137634 | 3114442 | <span style="color:#f97316">-3.81%</span> |
| 767 | [00146 DOT_READ_TEMPFILE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_146_DOT_READ_TEMPFILE.rs) | P1 | tempfile | CLI_TEMPFILE | 2129178 | 3112157 | <span style="color:#f97316">-3.74%</span> |
| 768 | [00546 AGG_GROUP_HAVING_039](crates/bench/sqlite_parity/cases/SQLITE_PARITY_546_AGG_GROUP_HAVING_039.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2088050 | 3111576 | <span style="color:#f97316">-3.72%</span> |
| 769 | [00823 WINDOW_PARTITION_SUM_036](crates/bench/sqlite_parity/cases/SQLITE_PARITY_823_WINDOW_PARTITION_SUM_036.rs) | P2 | memory | GEN_SQL_WINDOW | 1595899 | 3109042 | <span style="color:#f97316">-3.63%</span> |
| 770 | [00075 EXPLAIN_QUERY_PLAN](crates/bench/sqlite_parity/cases/SQLITE_PARITY_075_EXPLAIN_QUERY_PLAN.rs) | P0 | memory | SQL_EXPLAIN | 1574779 | 3107549 | <span style="color:#f97316">-3.58%</span> |
| 771 | [00547 AGG_GROUP_HAVING_040](crates/bench/sqlite_parity/cases/SQLITE_PARITY_547_AGG_GROUP_HAVING_040.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2119360 | 3106296 | <span style="color:#f97316">-3.54%</span> |
| 772 | [00061 WINDOW_ROW_NUMBER_RANK](crates/bench/sqlite_parity/cases/SQLITE_PARITY_061_WINDOW_ROW_NUMBER_RANK.rs) | P0 | memory | SQL_WINDOW | 1671602 | 3105875 | <span style="color:#f97316">-3.53%</span> |
| 773 | [00575 AGG_GROUP_HAVING_068](crates/bench/sqlite_parity/cases/SQLITE_PARITY_575_AGG_GROUP_HAVING_068.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1739049 | 3105004 | <span style="color:#f97316">-3.50%</span> |
| 774 | [00590 AGG_GROUP_HAVING_083](crates/bench/sqlite_parity/cases/SQLITE_PARITY_590_AGG_GROUP_HAVING_083.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1713401 | 3104253 | <span style="color:#f97316">-3.48%</span> |
| 775 | [00813 WINDOW_PARTITION_SUM_026](crates/bench/sqlite_parity/cases/SQLITE_PARITY_813_WINDOW_PARTITION_SUM_026.rs) | P2 | memory | GEN_SQL_WINDOW | 1715545 | 3104042 | <span style="color:#f97316">-3.47%</span> |
| 776 | [00201 OPT_NO_ROWID_IN_VIEW](crates/bench/sqlite_parity/cases/SQLITE_PARITY_201_OPT_NO_ROWID_IN_VIEW.rs) | P4 | memory | CLI_OPTION | 1560732 | 3101587 | <span style="color:#f97316">-3.39%</span> |
| 777 | [00555 AGG_GROUP_HAVING_048](crates/bench/sqlite_parity/cases/SQLITE_PARITY_555_AGG_GROUP_HAVING_048.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1783925 | 3099032 | <span style="color:#f97316">-3.30%</span> |
| 778 | [00576 AGG_GROUP_HAVING_069](crates/bench/sqlite_parity/cases/SQLITE_PARITY_576_AGG_GROUP_HAVING_069.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1673696 | 3098361 | <span style="color:#f97316">-3.28%</span> |
| 779 | [00035 DROP_INDEX](crates/bench/sqlite_parity/cases/SQLITE_PARITY_035_DROP_INDEX.rs) | P0 | memory | SQL_DROP | 1922637 | 3095245 | <span style="color:#f97316">-3.17%</span> |
| 780 | [00073 INDEXED_BY](crates/bench/sqlite_parity/cases/SQLITE_PARITY_073_INDEXED_BY.rs) | P0 | memory | SQL_INDEX | 1553760 | 3094915 | <span style="color:#f97316">-3.16%</span> |
| 781 | [00858 WINDOW_PARTITION_SUM_071](crates/bench/sqlite_parity/cases/SQLITE_PARITY_858_WINDOW_PARTITION_SUM_071.rs) | P2 | memory | GEN_SQL_WINDOW | 1823279 | 3094865 | <span style="color:#f97316">-3.16%</span> |
| 782 | [00574 AGG_GROUP_HAVING_067](crates/bench/sqlite_parity/cases/SQLITE_PARITY_574_AGG_GROUP_HAVING_067.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1688424 | 3094313 | <span style="color:#f97316">-3.14%</span> |
| 783 | [00751 CTE_RECURSIVE_MATRIX_044](crates/bench/sqlite_parity/cases/SQLITE_PARITY_751_CTE_RECURSIVE_MATRIX_044.rs) | P1 | memory | GEN_SQL_CTE | 1588625 | 3092800 | <span style="color:#f97316">-3.09%</span> |
| 784 | [00267 SCALAR_NULL_COALESCE_010](crates/bench/sqlite_parity/cases/SQLITE_PARITY_267_SCALAR_NULL_COALESCE_010.rs) | P1 | memory | GEN_SQL_SCALAR | 1555362 | 3092240 | <span style="color:#f97316">-3.07%</span> |
| 785 | [00074 NOT_INDEXED](crates/bench/sqlite_parity/cases/SQLITE_PARITY_074_NOT_INDEXED.rs) | P0 | memory | SQL_INDEX | 1572295 | 3092019 | <span style="color:#f97316">-3.07%</span> |
| 786 | [00024 SAVEPOINT_ROLLBACK_RELEASE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_024_SAVEPOINT_ROLLBACK_RELEASE.rs) | P0 | memory | SQL_SAVEPOINT | 1627979 | 3088022 | <span style="color:#f97316">-2.93%</span> |
| 787 | [00016 UPDATE_RETURNING](crates/bench/sqlite_parity/cases/SQLITE_PARITY_016_UPDATE_RETURNING.rs) | P0 | memory | SQL_UPDATE | 2200032 | 3087881 | <span style="color:#f97316">-2.93%</span> |
| 788 | [00549 AGG_GROUP_HAVING_042](crates/bench/sqlite_parity/cases/SQLITE_PARITY_549_AGG_GROUP_HAVING_042.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1781249 | 3086799 | <span style="color:#f97316">-2.89%</span> |
| 789 | [00129 DOT_CONNECTION](crates/bench/sqlite_parity/cases/SQLITE_PARITY_129_DOT_CONNECTION.rs) | P0 | memory | CLI_DOT_COMMAND | 1627459 | 3085897 | <span style="color:#f97316">-2.86%</span> |
| 790 | [00225 OPT_NONCE_SAFE_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_225_OPT_NONCE_SAFE_MODE.rs) | P2 | memory | CLI_OPTION | 1521578 | 3084645 | <span style="color:#f97316">-2.82%</span> |
| 791 | [00012 INSERT_DEFAULT_VALUES](crates/bench/sqlite_parity/cases/SQLITE_PARITY_012_INSERT_DEFAULT_VALUES.rs) | P0 | memory | SQL_INSERT | 1569228 | 3081920 | <span style="color:#f97316">-2.73%</span> |
| 792 | [00573 AGG_GROUP_HAVING_066](crates/bench/sqlite_parity/cases/SQLITE_PARITY_573_AGG_GROUP_HAVING_066.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1657576 | 3081529 | <span style="color:#f97316">-2.72%</span> |
| 793 | [00321 SCALAR_CAST_TYPEOF_024](crates/bench/sqlite_parity/cases/SQLITE_PARITY_321_SCALAR_CAST_TYPEOF_024.rs) | P1 | memory | GEN_SQL_SCALAR | 1799444 | 3078523 | <span style="color:#f97316">-2.62%</span> |
| 794 | [00548 AGG_GROUP_HAVING_041](crates/bench/sqlite_parity/cases/SQLITE_PARITY_548_AGG_GROUP_HAVING_041.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1669347 | 3078263 | <span style="color:#f97316">-2.61%</span> |
| 795 | [00862 WINDOW_PARTITION_SUM_075](crates/bench/sqlite_parity/cases/SQLITE_PARITY_862_WINDOW_PARTITION_SUM_075.rs) | P2 | memory | GEN_SQL_WINDOW | 1654039 | 3077532 | <span style="color:#f97316">-2.58%</span> |
| 796 | [00093 CREATE_VIRTUAL_TABLE_FTS5_OPTIONAL](crates/bench/sqlite_parity/cases/SQLITE_PARITY_093_CREATE_VIRTUAL_TABLE_FTS5_OPTIONAL.rs) | P1 | memory | SQL_VIRTUAL_TABLE_OPTIONAL | 1865560 | 3075508 | <span style="color:#f97316">-2.52%</span> |
| 797 | [00041 DROP_TRIGGER](crates/bench/sqlite_parity/cases/SQLITE_PARITY_041_DROP_TRIGGER.rs) | P0 | memory | SQL_DROP | 1785398 | 3075338 | <span style="color:#f97316">-2.51%</span> |
| 798 | [00023 TRANSACTION_ROLLBACK](crates/bench/sqlite_parity/cases/SQLITE_PARITY_023_TRANSACTION_ROLLBACK.rs) | P0 | memory | SQL_TRANSACTION | 1935191 | 3070659 | <span style="color:#f97316">-2.36%</span> |
| 799 | [00020 UPSERT_DO_UPDATE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_020_UPSERT_DO_UPDATE.rs) | P0 | memory | SQL_UPSERT | 2648109 | 3067242 | <span style="color:#f97316">-2.24%</span> |
| 800 | [00577 AGG_GROUP_HAVING_070](crates/bench/sqlite_parity/cases/SQLITE_PARITY_577_AGG_GROUP_HAVING_070.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1789535 | 3067122 | <span style="color:#f97316">-2.24%</span> |
| 801 | [00014 INSERT_RETURNING](crates/bench/sqlite_parity/cases/SQLITE_PARITY_014_INSERT_RETURNING.rs) | P0 | memory | SQL_INSERT | 1669869 | 3065870 | <span style="color:#f97316">-2.20%</span> |
| 802 | [00557 AGG_GROUP_HAVING_050](crates/bench/sqlite_parity/cases/SQLITE_PARITY_557_AGG_GROUP_HAVING_050.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1653368 | 3063715 | <span style="color:#f97316">-2.12%</span> |
| 803 | [00062 WINDOW_FRAMES_ROWS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_062_WINDOW_FRAMES_ROWS.rs) | P0 | memory | SQL_WINDOW | 1573106 | 3063094 | <span style="color:#f97316">-2.10%</span> |
| 804 | [00570 AGG_GROUP_HAVING_063](crates/bench/sqlite_parity/cases/SQLITE_PARITY_570_AGG_GROUP_HAVING_063.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1655482 | 3062804 | <span style="color:#f97316">-2.09%</span> |
| 805 | [00096 DBSTAT_OPTIONAL](crates/bench/sqlite_parity/cases/SQLITE_PARITY_096_DBSTAT_OPTIONAL.rs) | P3 | memory | SQL_VIRTUAL_TABLE_OPTIONAL | 1691419 | 3062183 | <span style="color:#f97316">-2.07%</span> |
| 806 | [00559 AGG_GROUP_HAVING_052](crates/bench/sqlite_parity/cases/SQLITE_PARITY_559_AGG_GROUP_HAVING_052.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1686189 | 3062053 | <span style="color:#f97316">-2.07%</span> |
| 807 | [00094 FTS5_HIGHLIGHT_OPTIONAL](crates/bench/sqlite_parity/cases/SQLITE_PARITY_094_FTS5_HIGHLIGHT_OPTIONAL.rs) | P2 | memory | SQL_VIRTUAL_TABLE_OPTIONAL | 1853787 | 3061030 | <span style="color:#f97316">-2.03%</span> |
| 808 | [00018 DELETE_RETURNING](crates/bench/sqlite_parity/cases/SQLITE_PARITY_018_DELETE_RETURNING.rs) | P0 | memory | SQL_DELETE | 1707380 | 3060430 | <span style="color:#f97316">-2.01%</span> |
| 809 | [00118 DOT_FULLSCHEMA](crates/bench/sqlite_parity/cases/SQLITE_PARITY_118_DOT_FULLSCHEMA.rs) | P0 | memory | CLI_DOT_COMMAND | 1661564 | 3059949 | <span style="color:#f97316">-2.00%</span> |
| 810 | [00589 AGG_GROUP_HAVING_082](crates/bench/sqlite_parity/cases/SQLITE_PARITY_589_AGG_GROUP_HAVING_082.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1900275 | 3058906 | <span style="color:#f97316">-1.96%</span> |
| 811 | [00591 AGG_GROUP_HAVING_084](crates/bench/sqlite_parity/cases/SQLITE_PARITY_591_AGG_GROUP_HAVING_084.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1663948 | 3056783 | <span style="color:#f97316">-1.89%</span> |
| 812 | [00108 DOT_MODE_CSV_AND_QUOTE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_108_DOT_MODE_CSV_AND_QUOTE.rs) | P0 | memory | CLI_DOT_COMMAND | 1734281 | 3056733 | <span style="color:#f97316">-1.89%</span> |
| 813 | [00030 ALTER_TABLE_ADD_DROP_COLUMN](crates/bench/sqlite_parity/cases/SQLITE_PARITY_030_ALTER_TABLE_ADD_DROP_COLUMN.rs) | P0 | memory | SQL_ALTER | 2299721 | 3052845 | <span style="color:#f97316">-1.76%</span> |
| 814 | [00542 AGG_GROUP_HAVING_035](crates/bench/sqlite_parity/cases/SQLITE_PARITY_542_AGG_GROUP_HAVING_035.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1657796 | 3052033 | <span style="color:#f97316">-1.73%</span> |
| 815 | [00571 AGG_GROUP_HAVING_064](crates/bench/sqlite_parity/cases/SQLITE_PARITY_571_AGG_GROUP_HAVING_064.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1653458 | 3051253 | <span style="color:#f97316">-1.71%</span> |
| 816 | [00350 SCALAR_STRING_031](crates/bench/sqlite_parity/cases/SQLITE_PARITY_350_SCALAR_STRING_031.rs) | P1 | memory | GEN_SQL_SCALAR | 1947253 | 3050481 | <span style="color:#f97316">-1.68%</span> |
| 817 | [00059 AGGREGATE_FUNCTIONS_CORE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_059_AGGREGATE_FUNCTIONS_CORE.rs) | P0 | memory | SQL_FUNCTIONS | 1562366 | 3048297 | <span style="color:#f97316">-1.61%</span> |
| 818 | [00543 AGG_GROUP_HAVING_036](crates/bench/sqlite_parity/cases/SQLITE_PARITY_543_AGG_GROUP_HAVING_036.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1640814 | 3047946 | <span style="color:#f97316">-1.60%</span> |
| 819 | [00008 STRICT_TABLE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_008_STRICT_TABLE.rs) | P0 | memory | SQL_DDL | 1861772 | 3047866 | <span style="color:#f97316">-1.60%</span> |
| 820 | [00537 AGG_GROUP_HAVING_030](crates/bench/sqlite_parity/cases/SQLITE_PARITY_537_AGG_GROUP_HAVING_030.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1628951 | 3047575 | <span style="color:#f97316">-1.59%</span> |
| 821 | [00222 OPT_ESCAPE_SYMBOL](crates/bench/sqlite_parity/cases/SQLITE_PARITY_222_OPT_ESCAPE_SYMBOL.rs) | P3 | memory | CLI_OPTION | 2324768 | 3047495 | <span style="color:#f97316">-1.58%</span> |
| 822 | [00569 AGG_GROUP_HAVING_062](crates/bench/sqlite_parity/cases/SQLITE_PARITY_569_AGG_GROUP_HAVING_062.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1645403 | 3046614 | <span style="color:#f97316">-1.55%</span> |
| 823 | [00317 SCALAR_CAST_TYPEOF_023](crates/bench/sqlite_parity/cases/SQLITE_PARITY_317_SCALAR_CAST_TYPEOF_023.rs) | P1 | memory | GEN_SQL_SCALAR | 1913229 | 3045682 | <span style="color:#f97316">-1.52%</span> |
| 824 | [00022 TRANSACTION_COMMIT](crates/bench/sqlite_parity/cases/SQLITE_PARITY_022_TRANSACTION_COMMIT.rs) | P0 | memory | SQL_TRANSACTION | 1752696 | 3045281 | <span style="color:#f97316">-1.51%</span> |
| 825 | [00536 AGG_GROUP_HAVING_029](crates/bench/sqlite_parity/cases/SQLITE_PARITY_536_AGG_GROUP_HAVING_029.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1634532 | 3044349 | <span style="color:#f97316">-1.48%</span> |
| 826 | [00034 EXPRESSION_INDEX](crates/bench/sqlite_parity/cases/SQLITE_PARITY_034_EXPRESSION_INDEX.rs) | P0 | memory | SQL_INDEX | 1560743 | 3043928 | <span style="color:#f97316">-1.46%</span> |
| 827 | [00237 SCALAR_CAST_TYPEOF_003](crates/bench/sqlite_parity/cases/SQLITE_PARITY_237_SCALAR_CAST_TYPEOF_003.rs) | P1 | memory | GEN_SQL_SCALAR | 1709143 | 3042656 | <span style="color:#f97316">-1.42%</span> |
| 828 | [00540 AGG_GROUP_HAVING_033](crates/bench/sqlite_parity/cases/SQLITE_PARITY_540_AGG_GROUP_HAVING_033.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1646064 | 3042215 | <span style="color:#f97316">-1.41%</span> |
| 829 | [00535 AGG_GROUP_HAVING_028](crates/bench/sqlite_parity/cases/SQLITE_PARITY_535_AGG_GROUP_HAVING_028.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1678245 | 3041985 | <span style="color:#f97316">-1.40%</span> |
| 830 | [01044 JSON_EXTRACT_SET_037](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1044_JSON_EXTRACT_SET_037.rs) | P2 | memory | GEN_SQL_JSON | 2671353 | 3041914 | <span style="color:#f97316">-1.40%</span> |
| 831 | [00572 AGG_GROUP_HAVING_065](crates/bench/sqlite_parity/cases/SQLITE_PARITY_572_AGG_GROUP_HAVING_065.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1655733 | 3038287 | <span style="color:#f97316">-1.28%</span> |
| 832 | [00033 PARTIAL_INDEX](crates/bench/sqlite_parity/cases/SQLITE_PARITY_033_PARTIAL_INDEX.rs) | P0 | memory | SQL_INDEX | 1538110 | 3038218 | <span style="color:#f97316">-1.27%</span> |
| 833 | [00534 AGG_GROUP_HAVING_027](crates/bench/sqlite_parity/cases/SQLITE_PARITY_534_AGG_GROUP_HAVING_027.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1701589 | 3037606 | <span style="color:#f97316">-1.25%</span> |
| 834 | [00560 AGG_GROUP_HAVING_053](crates/bench/sqlite_parity/cases/SQLITE_PARITY_560_AGG_GROUP_HAVING_053.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1609084 | 3036154 | <span style="color:#f97316">-1.21%</span> |
| 835 | [00065 CTE_RECURSIVE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_065_CTE_RECURSIVE.rs) | P0 | memory | SQL_CTE | 1840141 | 3034731 | <span style="color:#f97316">-1.16%</span> |
| 836 | [00319 SCALAR_NULL_COALESCE_023](crates/bench/sqlite_parity/cases/SQLITE_PARITY_319_SCALAR_NULL_COALESCE_023.rs) | P1 | memory | GEN_SQL_SCALAR | 1460192 | 3032707 | <span style="color:#f97316">-1.09%</span> |
| 837 | [00539 AGG_GROUP_HAVING_032](crates/bench/sqlite_parity/cases/SQLITE_PARITY_539_AGG_GROUP_HAVING_032.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1659759 | 3032697 | <span style="color:#f97316">-1.09%</span> |
| 838 | [00585 AGG_GROUP_HAVING_078](crates/bench/sqlite_parity/cases/SQLITE_PARITY_585_AGG_GROUP_HAVING_078.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1548479 | 3032036 | <span style="color:#f97316">-1.07%</span> |
| 839 | [00586 AGG_GROUP_HAVING_079](crates/bench/sqlite_parity/cases/SQLITE_PARITY_586_AGG_GROUP_HAVING_079.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1548719 | 3031925 | <span style="color:#f97316">-1.06%</span> |
| 840 | [00830 WINDOW_PARTITION_SUM_043](crates/bench/sqlite_parity/cases/SQLITE_PARITY_830_WINDOW_PARTITION_SUM_043.rs) | P2 | memory | GEN_SQL_WINDOW | 1682402 | 3031494 | <span style="color:#f97316">-1.05%</span> |
| 841 | [00541 AGG_GROUP_HAVING_034](crates/bench/sqlite_parity/cases/SQLITE_PARITY_541_AGG_GROUP_HAVING_034.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1644321 | 3031394 | <span style="color:#f97316">-1.05%</span> |
| 842 | [00538 AGG_GROUP_HAVING_031](crates/bench/sqlite_parity/cases/SQLITE_PARITY_538_AGG_GROUP_HAVING_031.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1640242 | 3030673 | <span style="color:#f97316">-1.02%</span> |
| 843 | [00038 CREATE_TRIGGER_AFTER](crates/bench/sqlite_parity/cases/SQLITE_PARITY_038_CREATE_TRIGGER_AFTER.rs) | P0 | memory | SQL_TRIGGER | 1555743 | 3030132 | <span style="color:#f97316">-1.00%</span> |
| 844 | [00588 AGG_GROUP_HAVING_081](crates/bench/sqlite_parity/cases/SQLITE_PARITY_588_AGG_GROUP_HAVING_081.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1554510 | 3026565 | <span style="color:#f97316">-0.89%</span> |
| 845 | [00009 STRICT_TABLE_TYPE_FAILURE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_009_STRICT_TABLE_TYPE_FAILURE.rs) | P0 | memory | SQL_DDL_NEGATIVE | 1592623 | 3024612 | <span style="color:#f97316">-0.82%</span> |
| 846 | [00060 FILTER_CLAUSE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_060_FILTER_CLAUSE.rs) | P0 | memory | SQL_AGGREGATE | 1898451 | 3021677 | <span style="color:#f97316">-0.72%</span> |
| 847 | [00031 CREATE_INDEX](crates/bench/sqlite_parity/cases/SQLITE_PARITY_031_CREATE_INDEX.rs) | P0 | memory | SQL_INDEX | 1847855 | 3019763 | <span style="color:#f97316">-0.66%</span> |
| 848 | [00558 AGG_GROUP_HAVING_051](crates/bench/sqlite_parity/cases/SQLITE_PARITY_558_AGG_GROUP_HAVING_051.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1641585 | 3013300 | <span style="color:#6b7280">-0.44%</span> |
| 849 | [00544 AGG_GROUP_HAVING_037](crates/bench/sqlite_parity/cases/SQLITE_PARITY_544_AGG_GROUP_HAVING_037.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1637057 | 3011617 | <span style="color:#6b7280">-0.39%</span> |
| 850 | [00145 DOT_SCANSTATS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_145_DOT_SCANSTATS.rs) | P3 | memory | CLI_DOT_COMMAND_DIAGNOSTIC | 2078392 | 3009072 | <span style="color:#6b7280">-0.30%</span> |
| 851 | [00341 SCALAR_CAST_TYPEOF_029](crates/bench/sqlite_parity/cases/SQLITE_PARITY_341_SCALAR_CAST_TYPEOF_029.rs) | P1 | memory | GEN_SQL_SCALAR | 1619403 | 3007480 | <span style="color:#6b7280">-0.25%</span> |
| 852 | [00101 SCHEMA_SQLITE_MASTER_ALIAS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_101_SCHEMA_SQLITE_MASTER_ALIAS.rs) | P0 | memory | SQL_SCHEMA | 1557597 | 3001408 | <span style="color:#6b7280">-0.05%</span> |
| 853 | [00119 DOT_EQP](crates/bench/sqlite_parity/cases/SQLITE_PARITY_119_DOT_EQP.rs) | P0 | memory | CLI_DOT_COMMAND | 1616458 | 3001168 | <span style="color:#6b7280">-0.04%</span> |
| 854 | [00052 PRAGMA_INTEGRITY_QUICK_CHECK](crates/bench/sqlite_parity/cases/SQLITE_PARITY_052_PRAGMA_INTEGRITY_QUICK_CHECK.rs) | P0 | memory | SQL_PRAGMA | 1651925 | 3000737 | <span style="color:#6b7280">-0.02%</span> |
| 855 | [00249 SCALAR_CAST_TYPEOF_006](crates/bench/sqlite_parity/cases/SQLITE_PARITY_249_SCALAR_CAST_TYPEOF_006.rs) | P1 | memory | GEN_SQL_SCALAR | 1529182 | 2999224 | <span style="color:#6b7280">0.03%</span> |
| 856 | [00100 SCHEMA_SQLITE_SCHEMA](crates/bench/sqlite_parity/cases/SQLITE_PARITY_100_SCHEMA_SQLITE_SCHEMA.rs) | P0 | memory | SQL_SCHEMA | 1747997 | 2997821 | <span style="color:#6b7280">0.07%</span> |
| 857 | [00325 SCALAR_CAST_TYPEOF_025](crates/bench/sqlite_parity/cases/SQLITE_PARITY_325_SCALAR_CAST_TYPEOF_025.rs) | P1 | memory | GEN_SQL_SCALAR | 1512932 | 2997731 | <span style="color:#6b7280">0.08%</span> |
| 858 | [00294 SCALAR_STRING_017](crates/bench/sqlite_parity/cases/SQLITE_PARITY_294_SCALAR_STRING_017.rs) | P1 | memory | GEN_SQL_SCALAR | 1584177 | 2996268 | <span style="color:#6b7280">0.12%</span> |
| 859 | [00762 CTE_RECURSIVE_MATRIX_055](crates/bench/sqlite_parity/cases/SQLITE_PARITY_762_CTE_RECURSIVE_MATRIX_055.rs) | P1 | memory | GEN_SQL_CTE | 1605547 | 2995607 | <span style="color:#6b7280">0.15%</span> |
| 860 | [00251 SCALAR_NULL_COALESCE_006](crates/bench/sqlite_parity/cases/SQLITE_PARITY_251_SCALAR_NULL_COALESCE_006.rs) | P1 | memory | GEN_SQL_SCALAR | 1623561 | 2995025 | <span style="color:#6b7280">0.17%</span> |
| 861 | [00255 SCALAR_NULL_COALESCE_007](crates/bench/sqlite_parity/cases/SQLITE_PARITY_255_SCALAR_NULL_COALESCE_007.rs) | P1 | memory | GEN_SQL_SCALAR | 1486101 | 2993052 | <span style="color:#6b7280">0.23%</span> |
| 862 | [00220 DELETE_LIMIT_OPTIONAL](crates/bench/sqlite_parity/cases/SQLITE_PARITY_220_DELETE_LIMIT_OPTIONAL.rs) | P3 | memory | SQL_DELETE_OPTIONAL | 1588495 | 2991689 | <span style="color:#6b7280">0.28%</span> |
| 863 | [00051 PRAGMA_INDEX_LIST_FUNCTION](crates/bench/sqlite_parity/cases/SQLITE_PARITY_051_PRAGMA_INDEX_LIST_FUNCTION.rs) | P0 | memory | SQL_PRAGMA | 1578726 | 2990047 | <span style="color:#6b7280">0.33%</span> |
| 864 | [00063 WINDOW_EXCLUDE_CURRENT_ROW](crates/bench/sqlite_parity/cases/SQLITE_PARITY_063_WINDOW_EXCLUDE_CURRENT_ROW.rs) | P0 | memory | SQL_WINDOW | 1578356 | 2985668 | <span style="color:#6b7280">0.48%</span> |
| 865 | [00355 SCALAR_NULL_COALESCE_032](crates/bench/sqlite_parity/cases/SQLITE_PARITY_355_SCALAR_NULL_COALESCE_032.rs) | P1 | memory | GEN_SQL_SCALAR | 1513042 | 2985087 | <span style="color:#6b7280">0.50%</span> |
| 866 | [00050 PRAGMA_TABLE_INFO_FUNCTION](crates/bench/sqlite_parity/cases/SQLITE_PARITY_050_PRAGMA_TABLE_INFO_FUNCTION.rs) | P0 | memory | SQL_PRAGMA | 1593054 | 2983625 | <span style="color:#16a34a">0.55%</span> |
| 867 | [00277 SCALAR_CAST_TYPEOF_013](crates/bench/sqlite_parity/cases/SQLITE_PARITY_277_SCALAR_CAST_TYPEOF_013.rs) | P1 | memory | GEN_SQL_SCALAR | 1547578 | 2981871 | <span style="color:#16a34a">0.60%</span> |
| 868 | [00210 OPT_NOUNICODE_UTF8_CATALOG](crates/bench/sqlite_parity/cases/SQLITE_PARITY_210_OPT_NOUNICODE_UTF8_CATALOG.rs) | P4 | catalog | CLI_OPTION_CATALOG | 1722749 | 2981450 | <span style="color:#16a34a">0.62%</span> |
| 869 | [00104 SELECT_DISTINCT](crates/bench/sqlite_parity/cases/SQLITE_PARITY_104_SELECT_DISTINCT.rs) | P0 | memory | SQL_SELECT | 1480631 | 2981440 | <span style="color:#16a34a">0.62%</span> |
| 870 | [00581 AGG_GROUP_HAVING_074](crates/bench/sqlite_parity/cases/SQLITE_PARITY_581_AGG_GROUP_HAVING_074.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1558869 | 2980188 | <span style="color:#16a34a">0.66%</span> |
| 871 | [00279 SCALAR_NULL_COALESCE_013](crates/bench/sqlite_parity/cases/SQLITE_PARITY_279_SCALAR_NULL_COALESCE_013.rs) | P1 | memory | GEN_SQL_SCALAR | 1638028 | 2978405 | <span style="color:#16a34a">0.72%</span> |
| 872 | [00285 SCALAR_CAST_TYPEOF_015](crates/bench/sqlite_parity/cases/SQLITE_PARITY_285_SCALAR_CAST_TYPEOF_015.rs) | P1 | memory | GEN_SQL_SCALAR | 1764047 | 2975699 | <span style="color:#16a34a">0.81%</span> |
| 873 | [00732 CTE_RECURSIVE_MATRIX_025](crates/bench/sqlite_parity/cases/SQLITE_PARITY_732_CTE_RECURSIVE_MATRIX_025.rs) | P1 | memory | GEN_SQL_CTE | 1576272 | 2975288 | <span style="color:#16a34a">0.82%</span> |
| 874 | [00313 SCALAR_CAST_TYPEOF_022](crates/bench/sqlite_parity/cases/SQLITE_PARITY_313_SCALAR_CAST_TYPEOF_022.rs) | P1 | memory | GEN_SQL_SCALAR | 1635294 | 2974066 | <span style="color:#16a34a">0.86%</span> |
| 875 | [00072 ORDER_BY_NULLS_FIRST_LAST](crates/bench/sqlite_parity/cases/SQLITE_PARITY_072_ORDER_BY_NULLS_FIRST_LAST.rs) | P0 | memory | SQL_SELECT | 1557597 | 2971802 | <span style="color:#16a34a">0.94%</span> |
| 876 | [00042 TEMP_TABLE_TEMP_SCHEMA](crates/bench/sqlite_parity/cases/SQLITE_PARITY_042_TEMP_TABLE_TEMP_SCHEMA.rs) | P0 | memory | SQL_TEMP | 1574559 | 2970569 | <span style="color:#16a34a">0.98%</span> |
| 877 | [00363 SCALAR_NULL_COALESCE_034](crates/bench/sqlite_parity/cases/SQLITE_PARITY_363_SCALAR_NULL_COALESCE_034.rs) | P1 | memory | GEN_SQL_SCALAR | 1541436 | 2969998 | <span style="color:#16a34a">1.00%</span> |
| 878 | [00318 SCALAR_STRING_023](crates/bench/sqlite_parity/cases/SQLITE_PARITY_318_SCALAR_STRING_023.rs) | P1 | memory | GEN_SQL_SCALAR | 1540173 | 2969307 | <span style="color:#16a34a">1.02%</span> |
| 879 | [00580 AGG_GROUP_HAVING_073](crates/bench/sqlite_parity/cases/SQLITE_PARITY_580_AGG_GROUP_HAVING_073.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1623091 | 2967644 | <span style="color:#16a34a">1.08%</span> |
| 880 | [00028 ALTER_TABLE_RENAME](crates/bench/sqlite_parity/cases/SQLITE_PARITY_028_ALTER_TABLE_RENAME.rs) | P0 | memory | SQL_ALTER | 1902298 | 2963947 | <span style="color:#16a34a">1.20%</span> |
| 881 | [00369 SCALAR_CAST_TYPEOF_036](crates/bench/sqlite_parity/cases/SQLITE_PARITY_369_SCALAR_CAST_TYPEOF_036.rs) | P1 | memory | GEN_SQL_SCALAR | 1518703 | 2963426 | <span style="color:#16a34a">1.22%</span> |
| 882 | [00370 SCALAR_STRING_036](crates/bench/sqlite_parity/cases/SQLITE_PARITY_370_SCALAR_STRING_036.rs) | P1 | memory | GEN_SQL_SCALAR | 1446586 | 2962394 | <span style="color:#16a34a">1.25%</span> |
| 883 | [00257 SCALAR_CAST_TYPEOF_008](crates/bench/sqlite_parity/cases/SQLITE_PARITY_257_SCALAR_CAST_TYPEOF_008.rs) | P1 | memory | GEN_SQL_SCALAR | 1451937 | 2962273 | <span style="color:#16a34a">1.26%</span> |
| 884 | [00365 SCALAR_CAST_TYPEOF_035](crates/bench/sqlite_parity/cases/SQLITE_PARITY_365_SCALAR_CAST_TYPEOF_035.rs) | P1 | memory | GEN_SQL_SCALAR | 2672757 | 2961963 | <span style="color:#16a34a">1.27%</span> |
| 885 | [00092 PERCENTILE_FUNCTIONS_OPTIONAL](crates/bench/sqlite_parity/cases/SQLITE_PARITY_092_PERCENTILE_FUNCTIONS_OPTIONAL.rs) | P3 | memory | SQL_FUNCTIONS_OPTIONAL | 1536978 | 2960841 | <span style="color:#16a34a">1.31%</span> |
| 886 | [00002 LITERALS_AND_TYPEOF](crates/bench/sqlite_parity/cases/SQLITE_PARITY_002_LITERALS_AND_TYPEOF.rs) | P0 | memory | SQL_EXPRESSIONS | 2035991 | 2960450 | <span style="color:#16a34a">1.32%</span> |
| 887 | [00037 DROP_VIEW](crates/bench/sqlite_parity/cases/SQLITE_PARITY_037_DROP_VIEW.rs) | P0 | memory | SQL_DROP | 1560752 | 2959468 | <span style="color:#16a34a">1.35%</span> |
| 888 | [00373 SCALAR_CAST_TYPEOF_037](crates/bench/sqlite_parity/cases/SQLITE_PARITY_373_SCALAR_CAST_TYPEOF_037.rs) | P1 | memory | GEN_SQL_SCALAR | 1485720 | 2958627 | <span style="color:#16a34a">1.38%</span> |
| 889 | [00275 SCALAR_NULL_COALESCE_012](crates/bench/sqlite_parity/cases/SQLITE_PARITY_275_SCALAR_NULL_COALESCE_012.rs) | P1 | memory | GEN_SQL_SCALAR | 1574559 | 2957165 | <span style="color:#16a34a">1.43%</span> |
| 890 | [00091 MATH_FUNCTIONS_OPTIONAL](crates/bench/sqlite_parity/cases/SQLITE_PARITY_091_MATH_FUNCTIONS_OPTIONAL.rs) | P2 | memory | SQL_FUNCTIONS_OPTIONAL | 1564209 | 2956913 | <span style="color:#16a34a">1.44%</span> |
| 891 | [00387 SCALAR_NULL_COALESCE_040](crates/bench/sqlite_parity/cases/SQLITE_PARITY_387_SCALAR_NULL_COALESCE_040.rs) | P1 | memory | GEN_SQL_SCALAR | 1454953 | 2955781 | <span style="color:#16a34a">1.47%</span> |
| 892 | [00282 SCALAR_STRING_014](crates/bench/sqlite_parity/cases/SQLITE_PARITY_282_SCALAR_STRING_014.rs) | P1 | memory | GEN_SQL_SCALAR | 1724332 | 2955541 | <span style="color:#16a34a">1.48%</span> |
| 893 | [00029 ALTER_TABLE_RENAME_COLUMN](crates/bench/sqlite_parity/cases/SQLITE_PARITY_029_ALTER_TABLE_RENAME_COLUMN.rs) | P0 | memory | SQL_ALTER | 1944038 | 2955020 | <span style="color:#16a34a">1.50%</span> |
| 894 | [00283 SCALAR_NULL_COALESCE_014](crates/bench/sqlite_parity/cases/SQLITE_PARITY_283_SCALAR_NULL_COALESCE_014.rs) | P1 | memory | GEN_SQL_SCALAR | 1704324 | 2954959 | <span style="color:#16a34a">1.50%</span> |
| 895 | [00339 SCALAR_NULL_COALESCE_028](crates/bench/sqlite_parity/cases/SQLITE_PARITY_339_SCALAR_NULL_COALESCE_028.rs) | P1 | memory | GEN_SQL_SCALAR | 1645933 | 2950982 | <span style="color:#16a34a">1.63%</span> |
| 896 | [00081 BLOBS_HEX_ZEROBLOB](crates/bench/sqlite_parity/cases/SQLITE_PARITY_081_BLOBS_HEX_ZEROBLOB.rs) | P0 | memory | SQL_FUNCTIONS | 1582383 | 2950401 | <span style="color:#16a34a">1.65%</span> |
| 897 | [00248 SCALAR_ARITH_006](crates/bench/sqlite_parity/cases/SQLITE_PARITY_248_SCALAR_ARITH_006.rs) | P1 | memory | GEN_SQL_SCALAR | 1513493 | 2948177 | <span style="color:#16a34a">1.73%</span> |
| 898 | [00099 CLI_UINT_COLLATION_OPTIONAL](crates/bench/sqlite_parity/cases/SQLITE_PARITY_099_CLI_UINT_COLLATION_OPTIONAL.rs) | P3 | memory | CLI_EXTENSION_OPTIONAL | 2199692 | 2948006 | <span style="color:#16a34a">1.73%</span> |
| 899 | [00354 SCALAR_STRING_032](crates/bench/sqlite_parity/cases/SQLITE_PARITY_354_SCALAR_STRING_032.rs) | P1 | memory | GEN_SQL_SCALAR | 1499607 | 2946183 | <span style="color:#16a34a">1.79%</span> |
| 900 | [00245 SCALAR_CAST_TYPEOF_005](crates/bench/sqlite_parity/cases/SQLITE_PARITY_245_SCALAR_CAST_TYPEOF_005.rs) | P1 | memory | GEN_SQL_SCALAR | 1808090 | 2945051 | <span style="color:#16a34a">1.83%</span> |
| 901 | [00258 SCALAR_STRING_008](crates/bench/sqlite_parity/cases/SQLITE_PARITY_258_SCALAR_STRING_008.rs) | P1 | memory | GEN_SQL_SCALAR | 1553288 | 2943919 | <span style="color:#16a34a">1.87%</span> |
| 902 | [00582 AGG_GROUP_HAVING_075](crates/bench/sqlite_parity/cases/SQLITE_PARITY_582_AGG_GROUP_HAVING_075.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1550503 | 2943909 | <span style="color:#16a34a">1.87%</span> |
| 903 | [00583 AGG_GROUP_HAVING_076](crates/bench/sqlite_parity/cases/SQLITE_PARITY_583_AGG_GROUP_HAVING_076.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1570811 | 2943288 | <span style="color:#16a34a">1.89%</span> |
| 904 | [00848 WINDOW_PARTITION_SUM_061](crates/bench/sqlite_parity/cases/SQLITE_PARITY_848_WINDOW_PARTITION_SUM_061.rs) | P2 | memory | GEN_SQL_WINDOW | 2372569 | 2942908 | <span style="color:#16a34a">1.90%</span> |
| 905 | [00241 SCALAR_CAST_TYPEOF_004](crates/bench/sqlite_parity/cases/SQLITE_PARITY_241_SCALAR_CAST_TYPEOF_004.rs) | P1 | memory | GEN_SQL_SCALAR | 1501390 | 2941204 | <span style="color:#16a34a">1.96%</span> |
| 906 | [00584 AGG_GROUP_HAVING_077](crates/bench/sqlite_parity/cases/SQLITE_PARITY_584_AGG_GROUP_HAVING_077.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1547638 | 2939330 | <span style="color:#16a34a">2.02%</span> |
| 907 | [00295 SCALAR_NULL_COALESCE_017](crates/bench/sqlite_parity/cases/SQLITE_PARITY_295_SCALAR_NULL_COALESCE_017.rs) | P1 | memory | GEN_SQL_SCALAR | 1517882 | 2938488 | <span style="color:#16a34a">2.05%</span> |
| 908 | [00286 SCALAR_STRING_015](crates/bench/sqlite_parity/cases/SQLITE_PARITY_286_SCALAR_STRING_015.rs) | P1 | memory | GEN_SQL_SCALAR | 1585950 | 2938439 | <span style="color:#16a34a">2.05%</span> |
| 909 | [00323 SCALAR_NULL_COALESCE_024](crates/bench/sqlite_parity/cases/SQLITE_PARITY_323_SCALAR_NULL_COALESCE_024.rs) | P1 | memory | GEN_SQL_SCALAR | 1485671 | 2937496 | <span style="color:#16a34a">2.08%</span> |
| 910 | [00046 VACUUM_MEMORY](crates/bench/sqlite_parity/cases/SQLITE_PARITY_046_VACUUM_MEMORY.rs) | P0 | memory | SQL_VACUUM | 1725304 | 2937347 | <span style="color:#16a34a">2.09%</span> |
| 911 | [00064 CTE_NON_RECURSIVE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_064_CTE_NON_RECURSIVE.rs) | P0 | memory | SQL_CTE | 1484599 | 2936495 | <span style="color:#16a34a">2.12%</span> |
| 912 | [00343 SCALAR_NULL_COALESCE_029](crates/bench/sqlite_parity/cases/SQLITE_PARITY_343_SCALAR_NULL_COALESCE_029.rs) | P1 | memory | GEN_SQL_SCALAR | 1550343 | 2935443 | <span style="color:#16a34a">2.15%</span> |
| 913 | [00348 SCALAR_ARITH_031](crates/bench/sqlite_parity/cases/SQLITE_PARITY_348_SCALAR_ARITH_031.rs) | P1 | memory | GEN_SQL_SCALAR | 1525906 | 2935022 | <span style="color:#16a34a">2.17%</span> |
| 914 | [00263 SCALAR_NULL_COALESCE_009](crates/bench/sqlite_parity/cases/SQLITE_PARITY_263_SCALAR_NULL_COALESCE_009.rs) | P1 | memory | GEN_SQL_SCALAR | 1521047 | 2934481 | <span style="color:#16a34a">2.18%</span> |
| 915 | [00856 WINDOW_PARTITION_SUM_069](crates/bench/sqlite_parity/cases/SQLITE_PARITY_856_WINDOW_PARTITION_SUM_069.rs) | P2 | memory | GEN_SQL_WINDOW | 1630043 | 2933750 | <span style="color:#16a34a">2.21%</span> |
| 916 | [00247 SCALAR_NULL_COALESCE_005](crates/bench/sqlite_parity/cases/SQLITE_PARITY_247_SCALAR_NULL_COALESCE_005.rs) | P1 | memory | GEN_SQL_SCALAR | 1524665 | 2933409 | <span style="color:#16a34a">2.22%</span> |
| 917 | [00338 SCALAR_STRING_028](crates/bench/sqlite_parity/cases/SQLITE_PARITY_338_SCALAR_STRING_028.rs) | P1 | memory | GEN_SQL_SCALAR | 1561113 | 2933399 | <span style="color:#16a34a">2.22%</span> |
| 918 | [00316 SCALAR_ARITH_023](crates/bench/sqlite_parity/cases/SQLITE_PARITY_316_SCALAR_ARITH_023.rs) | P1 | memory | GEN_SQL_SCALAR | 1608282 | 2933339 | <span style="color:#16a34a">2.22%</span> |
| 919 | [00326 SCALAR_STRING_025](crates/bench/sqlite_parity/cases/SQLITE_PARITY_326_SCALAR_STRING_025.rs) | P1 | memory | GEN_SQL_SCALAR | 1591330 | 2932758 | <span style="color:#16a34a">2.24%</span> |
| 920 | [00234 SCALAR_STRING_002](crates/bench/sqlite_parity/cases/SQLITE_PARITY_234_SCALAR_STRING_002.rs) | P1 | memory | GEN_SQL_SCALAR | 1502002 | 2932547 | <span style="color:#16a34a">2.25%</span> |
| 921 | [00090 JSON_MUTATION_FUNCTIONS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_090_JSON_MUTATION_FUNCTIONS.rs) | P0 | memory | SQL_JSON | 1485660 | 2932067 | <span style="color:#16a34a">2.26%</span> |
| 922 | [00345 SCALAR_CAST_TYPEOF_030](crates/bench/sqlite_parity/cases/SQLITE_PARITY_345_SCALAR_CAST_TYPEOF_030.rs) | P1 | memory | GEN_SQL_SCALAR | 1494587 | 2931476 | <span style="color:#16a34a">2.28%</span> |
| 923 | [00587 AGG_GROUP_HAVING_080](crates/bench/sqlite_parity/cases/SQLITE_PARITY_587_AGG_GROUP_HAVING_080.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1553128 | 2930684 | <span style="color:#16a34a">2.31%</span> |
| 924 | [00307 SCALAR_NULL_COALESCE_020](crates/bench/sqlite_parity/cases/SQLITE_PARITY_307_SCALAR_NULL_COALESCE_020.rs) | P1 | memory | GEN_SQL_SCALAR | 1486863 | 2930423 | <span style="color:#16a34a">2.32%</span> |
| 925 | [00011 CREATE_TABLE_AS_SELECT](crates/bench/sqlite_parity/cases/SQLITE_PARITY_011_CREATE_TABLE_AS_SELECT.rs) | P0 | memory | SQL_DDL | 1491391 | 2929582 | <span style="color:#16a34a">2.35%</span> |
| 926 | [00281 SCALAR_CAST_TYPEOF_014](crates/bench/sqlite_parity/cases/SQLITE_PARITY_281_SCALAR_CAST_TYPEOF_014.rs) | P1 | memory | GEN_SQL_SCALAR | 1742116 | 2928920 | <span style="color:#16a34a">2.37%</span> |
| 927 | [00337 SCALAR_CAST_TYPEOF_028](crates/bench/sqlite_parity/cases/SQLITE_PARITY_337_SCALAR_CAST_TYPEOF_028.rs) | P1 | memory | GEN_SQL_SCALAR | 1517240 | 2928109 | <span style="color:#16a34a">2.40%</span> |
| 928 | [00305 SCALAR_CAST_TYPEOF_020](crates/bench/sqlite_parity/cases/SQLITE_PARITY_305_SCALAR_CAST_TYPEOF_020.rs) | P1 | memory | GEN_SQL_SCALAR | 1490600 | 2927718 | <span style="color:#16a34a">2.41%</span> |
| 929 | [00230 SCALAR_STRING_001](crates/bench/sqlite_parity/cases/SQLITE_PARITY_230_SCALAR_STRING_001.rs) | P1 | memory | GEN_SQL_SCALAR | 1567375 | 2927608 | <span style="color:#16a34a">2.41%</span> |
| 930 | [00250 SCALAR_STRING_006](crates/bench/sqlite_parity/cases/SQLITE_PARITY_250_SCALAR_STRING_006.rs) | P1 | memory | GEN_SQL_SCALAR | 1930873 | 2926916 | <span style="color:#16a34a">2.44%</span> |
| 931 | [00371 SCALAR_NULL_COALESCE_036](crates/bench/sqlite_parity/cases/SQLITE_PARITY_371_SCALAR_NULL_COALESCE_036.rs) | P1 | memory | GEN_SQL_SCALAR | 1627589 | 2926866 | <span style="color:#16a34a">2.44%</span> |
| 932 | [00287 SCALAR_NULL_COALESCE_015](crates/bench/sqlite_parity/cases/SQLITE_PARITY_287_SCALAR_NULL_COALESCE_015.rs) | P1 | memory | GEN_SQL_SCALAR | 1550994 | 2926826 | <span style="color:#16a34a">2.44%</span> |
| 933 | [00366 SCALAR_STRING_035](crates/bench/sqlite_parity/cases/SQLITE_PARITY_366_SCALAR_STRING_035.rs) | P1 | memory | GEN_SQL_SCALAR | 1451976 | 2926135 | <span style="color:#16a34a">2.46%</span> |
| 934 | [00331 SCALAR_NULL_COALESCE_026](crates/bench/sqlite_parity/cases/SQLITE_PARITY_331_SCALAR_NULL_COALESCE_026.rs) | P1 | memory | GEN_SQL_SCALAR | 1593805 | 2924753 | <span style="color:#16a34a">2.51%</span> |
| 935 | [00567 AGG_GROUP_HAVING_060](crates/bench/sqlite_parity/cases/SQLITE_PARITY_567_AGG_GROUP_HAVING_060.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1527901 | 2924192 | <span style="color:#16a34a">2.53%</span> |
| 936 | [00290 SCALAR_STRING_016](crates/bench/sqlite_parity/cases/SQLITE_PARITY_290_SCALAR_STRING_016.rs) | P1 | memory | GEN_SQL_SCALAR | 1473127 | 2923931 | <span style="color:#16a34a">2.54%</span> |
| 937 | [00346 SCALAR_STRING_030](crates/bench/sqlite_parity/cases/SQLITE_PARITY_346_SCALAR_STRING_030.rs) | P1 | memory | GEN_SQL_SCALAR | 1565110 | 2923761 | <span style="color:#16a34a">2.54%</span> |
| 938 | [00083 CORE_NUMERIC_FUNCTIONS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_083_CORE_NUMERIC_FUNCTIONS.rs) | P0 | memory | SQL_FUNCTIONS | 1499547 | 2923290 | <span style="color:#16a34a">2.56%</span> |
| 939 | [00564 AGG_GROUP_HAVING_057](crates/bench/sqlite_parity/cases/SQLITE_PARITY_564_AGG_GROUP_HAVING_057.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1546175 | 2922669 | <span style="color:#16a34a">2.58%</span> |
| 940 | [00276 SCALAR_ARITH_013](crates/bench/sqlite_parity/cases/SQLITE_PARITY_276_SCALAR_ARITH_013.rs) | P1 | memory | GEN_SQL_SCALAR | 1431808 | 2922459 | <span style="color:#16a34a">2.58%</span> |
| 941 | [00302 SCALAR_STRING_019](crates/bench/sqlite_parity/cases/SQLITE_PARITY_302_SCALAR_STRING_019.rs) | P1 | memory | GEN_SQL_SCALAR | 1571493 | 2921627 | <span style="color:#16a34a">2.61%</span> |
| 942 | [00347 SCALAR_NULL_COALESCE_030](crates/bench/sqlite_parity/cases/SQLITE_PARITY_347_SCALAR_NULL_COALESCE_030.rs) | P1 | memory | GEN_SQL_SCALAR | 1446707 | 2921457 | <span style="color:#16a34a">2.62%</span> |
| 943 | [00084 CORE_FORMAT_QUOTE_HEX](crates/bench/sqlite_parity/cases/SQLITE_PARITY_084_CORE_FORMAT_QUOTE_HEX.rs) | P0 | memory | SQL_FUNCTIONS | 1477615 | 2921126 | <span style="color:#16a34a">2.63%</span> |
| 944 | [00333 SCALAR_CAST_TYPEOF_027](crates/bench/sqlite_parity/cases/SQLITE_PARITY_333_SCALAR_CAST_TYPEOF_027.rs) | P1 | memory | GEN_SQL_SCALAR | 1629823 | 2920435 | <span style="color:#16a34a">2.65%</span> |
| 945 | [00353 SCALAR_CAST_TYPEOF_032](crates/bench/sqlite_parity/cases/SQLITE_PARITY_353_SCALAR_CAST_TYPEOF_032.rs) | P1 | memory | GEN_SQL_SCALAR | 1508012 | 2920004 | <span style="color:#16a34a">2.67%</span> |
| 946 | [00562 AGG_GROUP_HAVING_055](crates/bench/sqlite_parity/cases/SQLITE_PARITY_562_AGG_GROUP_HAVING_055.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1545022 | 2919172 | <span style="color:#16a34a">2.69%</span> |
| 947 | [00306 SCALAR_STRING_020](crates/bench/sqlite_parity/cases/SQLITE_PARITY_306_SCALAR_STRING_020.rs) | P1 | memory | GEN_SQL_SCALAR | 1459351 | 2918962 | <span style="color:#16a34a">2.70%</span> |
| 948 | [00140 DOT_EXPERT_OPTIONAL](crates/bench/sqlite_parity/cases/SQLITE_PARITY_140_DOT_EXPERT_OPTIONAL.rs) | P3 | memory | CLI_DOT_COMMAND_OPTIONAL | 2408005 | 2918871 | <span style="color:#16a34a">2.70%</span> |
| 949 | [00349 SCALAR_CAST_TYPEOF_031](crates/bench/sqlite_parity/cases/SQLITE_PARITY_349_SCALAR_CAST_TYPEOF_031.rs) | P1 | memory | GEN_SQL_SCALAR | 1774798 | 2918301 | <span style="color:#16a34a">2.72%</span> |
| 950 | [00070 LIKE_GLOB_MATCH_ESCAPE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_070_LIKE_GLOB_MATCH_ESCAPE.rs) | P0 | memory | SQL_OPERATORS | 1462817 | 2918260 | <span style="color:#16a34a">2.72%</span> |
| 951 | [00262 SCALAR_STRING_009](crates/bench/sqlite_parity/cases/SQLITE_PARITY_262_SCALAR_STRING_009.rs) | P1 | memory | GEN_SQL_SCALAR | 1485279 | 2917248 | <span style="color:#16a34a">2.76%</span> |
| 952 | [00377 SCALAR_CAST_TYPEOF_038](crates/bench/sqlite_parity/cases/SQLITE_PARITY_377_SCALAR_CAST_TYPEOF_038.rs) | P1 | memory | GEN_SQL_SCALAR | 1580951 | 2916698 | <span style="color:#16a34a">2.78%</span> |
| 953 | [00103 WINDOW_NAMED_WINDOW](crates/bench/sqlite_parity/cases/SQLITE_PARITY_103_WINDOW_NAMED_WINDOW.rs) | P0 | memory | SQL_WINDOW | 1579648 | 2916197 | <span style="color:#16a34a">2.79%</span> |
| 954 | [00273 SCALAR_CAST_TYPEOF_012](crates/bench/sqlite_parity/cases/SQLITE_PARITY_273_SCALAR_CAST_TYPEOF_012.rs) | P1 | memory | GEN_SQL_SCALAR | 1504787 | 2916056 | <span style="color:#16a34a">2.80%</span> |
| 955 | [00243 SCALAR_NULL_COALESCE_004](crates/bench/sqlite_parity/cases/SQLITE_PARITY_243_SCALAR_NULL_COALESCE_004.rs) | P1 | memory | GEN_SQL_SCALAR | 2309329 | 2914904 | <span style="color:#16a34a">2.84%</span> |
| 956 | [00563 AGG_GROUP_HAVING_056](crates/bench/sqlite_parity/cases/SQLITE_PARITY_563_AGG_GROUP_HAVING_056.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1527780 | 2914854 | <span style="color:#16a34a">2.84%</span> |
| 957 | [00356 SCALAR_ARITH_033](crates/bench/sqlite_parity/cases/SQLITE_PARITY_356_SCALAR_ARITH_033.rs) | P1 | memory | GEN_SQL_SCALAR | 1890155 | 2914233 | <span style="color:#16a34a">2.86%</span> |
| 958 | [00149 DOT_ONCE_TEMPFILE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_149_DOT_ONCE_TEMPFILE.rs) | P1 | tempfile | CLI_TEMPFILE | 1534032 | 2913522 | <span style="color:#16a34a">2.88%</span> |
| 959 | [00278 SCALAR_STRING_013](crates/bench/sqlite_parity/cases/SQLITE_PARITY_278_SCALAR_STRING_013.rs) | P1 | memory | GEN_SQL_SCALAR | 1527209 | 2911127 | <span style="color:#16a34a">2.96%</span> |
| 960 | [00098 CLI_REGEXP_OPTIONAL](crates/bench/sqlite_parity/cases/SQLITE_PARITY_098_CLI_REGEXP_OPTIONAL.rs) | P2 | memory | CLI_EXTENSION_OPTIONAL | 1567525 | 2910677 | <span style="color:#16a34a">2.98%</span> |
| 961 | [00246 SCALAR_STRING_005](crates/bench/sqlite_parity/cases/SQLITE_PARITY_246_SCALAR_STRING_005.rs) | P1 | memory | GEN_SQL_SCALAR | 1607061 | 2908321 | <span style="color:#16a34a">3.06%</span> |
| 962 | [00271 SCALAR_NULL_COALESCE_011](crates/bench/sqlite_parity/cases/SQLITE_PARITY_271_SCALAR_NULL_COALESCE_011.rs) | P1 | memory | GEN_SQL_SCALAR | 1743197 | 2906849 | <span style="color:#16a34a">3.11%</span> |
| 963 | [00320 SCALAR_ARITH_024](crates/bench/sqlite_parity/cases/SQLITE_PARITY_320_SCALAR_ARITH_024.rs) | P1 | memory | GEN_SQL_SCALAR | 1810885 | 2906819 | <span style="color:#16a34a">3.11%</span> |
| 964 | [00386 SCALAR_STRING_040](crates/bench/sqlite_parity/cases/SQLITE_PARITY_386_SCALAR_STRING_040.rs) | P1 | memory | GEN_SQL_SCALAR | 1545123 | 2906408 | <span style="color:#16a34a">3.12%</span> |
| 965 | [00322 SCALAR_STRING_024](crates/bench/sqlite_parity/cases/SQLITE_PARITY_322_SCALAR_STRING_024.rs) | P1 | memory | GEN_SQL_SCALAR | 1641575 | 2905677 | <span style="color:#16a34a">3.14%</span> |
| 966 | [00568 AGG_GROUP_HAVING_061](crates/bench/sqlite_parity/cases/SQLITE_PARITY_568_AGG_GROUP_HAVING_061.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1541315 | 2905095 | <span style="color:#16a34a">3.16%</span> |
| 967 | [00566 AGG_GROUP_HAVING_059](crates/bench/sqlite_parity/cases/SQLITE_PARITY_566_AGG_GROUP_HAVING_059.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1537619 | 2904595 | <span style="color:#16a34a">3.18%</span> |
| 968 | [00067 CASE_COALESCE_NULLIF_IIF](crates/bench/sqlite_parity/cases/SQLITE_PARITY_067_CASE_COALESCE_NULLIF_IIF.rs) | P0 | memory | SQL_EXPRESSIONS | 1772644 | 2901780 | <span style="color:#16a34a">3.27%</span> |
| 969 | [00327 SCALAR_NULL_COALESCE_025](crates/bench/sqlite_parity/cases/SQLITE_PARITY_327_SCALAR_NULL_COALESCE_025.rs) | P1 | memory | GEN_SQL_SCALAR | 1572194 | 2899876 | <span style="color:#16a34a">3.34%</span> |
| 970 | [00383 SCALAR_NULL_COALESCE_039](crates/bench/sqlite_parity/cases/SQLITE_PARITY_383_SCALAR_NULL_COALESCE_039.rs) | P1 | memory | GEN_SQL_SCALAR | 1448480 | 2898463 | <span style="color:#16a34a">3.38%</span> |
| 971 | [00141 DOT_SHA3SUM](crates/bench/sqlite_parity/cases/SQLITE_PARITY_141_DOT_SHA3SUM.rs) | P0 | memory | CLI_DOT_COMMAND | 1826976 | 2897240 | <span style="color:#16a34a">3.43%</span> |
| 972 | [00253 SCALAR_CAST_TYPEOF_007](crates/bench/sqlite_parity/cases/SQLITE_PARITY_253_SCALAR_CAST_TYPEOF_007.rs) | P1 | memory | GEN_SQL_SCALAR | 1590559 | 2895958 | <span style="color:#16a34a">3.47%</span> |
| 973 | [00102 WITH_MATERIALIZED_HINTS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_102_WITH_MATERIALIZED_HINTS.rs) | P0 | memory | SQL_CTE | 1502933 | 2895698 | <span style="color:#16a34a">3.48%</span> |
| 974 | [00244 SCALAR_ARITH_005](crates/bench/sqlite_parity/cases/SQLITE_PARITY_244_SCALAR_ARITH_005.rs) | P1 | memory | GEN_SQL_SCALAR | 1622089 | 2894396 | <span style="color:#16a34a">3.52%</span> |
| 975 | [00071 BETWEEN_IN_ISNULL_IS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_071_BETWEEN_IN_ISNULL_IS.rs) | P0 | memory | SQL_OPERATORS | 1591281 | 2892081 | <span style="color:#16a34a">3.60%</span> |
| 976 | [00284 SCALAR_ARITH_015](crates/bench/sqlite_parity/cases/SQLITE_PARITY_284_SCALAR_ARITH_015.rs) | P1 | memory | GEN_SQL_SCALAR | 1446326 | 2891139 | <span style="color:#16a34a">3.63%</span> |
| 977 | [00266 SCALAR_STRING_010](crates/bench/sqlite_parity/cases/SQLITE_PARITY_266_SCALAR_STRING_010.rs) | P1 | memory | GEN_SQL_SCALAR | 1499236 | 2891049 | <span style="color:#16a34a">3.63%</span> |
| 978 | [00375 SCALAR_NULL_COALESCE_037](crates/bench/sqlite_parity/cases/SQLITE_PARITY_375_SCALAR_NULL_COALESCE_037.rs) | P1 | memory | GEN_SQL_SCALAR | 1605347 | 2890077 | <span style="color:#16a34a">3.66%</span> |
| 979 | [00357 SCALAR_CAST_TYPEOF_033](crates/bench/sqlite_parity/cases/SQLITE_PARITY_357_SCALAR_CAST_TYPEOF_033.rs) | P1 | memory | GEN_SQL_SCALAR | 1489297 | 2890037 | <span style="color:#16a34a">3.67%</span> |
| 980 | [00239 SCALAR_NULL_COALESCE_003](crates/bench/sqlite_parity/cases/SQLITE_PARITY_239_SCALAR_NULL_COALESCE_003.rs) | P1 | memory | GEN_SQL_SCALAR | 2258913 | 2889436 | <span style="color:#16a34a">3.69%</span> |
| 981 | [00561 AGG_GROUP_HAVING_054](crates/bench/sqlite_parity/cases/SQLITE_PARITY_561_AGG_GROUP_HAVING_054.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1503484 | 2889146 | <span style="color:#16a34a">3.70%</span> |
| 982 | [00300 SCALAR_ARITH_019](crates/bench/sqlite_parity/cases/SQLITE_PARITY_300_SCALAR_ARITH_019.rs) | P1 | memory | GEN_SQL_SCALAR | 1522630 | 2888584 | <span style="color:#16a34a">3.71%</span> |
| 983 | [00359 SCALAR_NULL_COALESCE_033](crates/bench/sqlite_parity/cases/SQLITE_PARITY_359_SCALAR_NULL_COALESCE_033.rs) | P1 | memory | GEN_SQL_SCALAR | 1531767 | 2886801 | <span style="color:#16a34a">3.77%</span> |
| 984 | [00361 SCALAR_CAST_TYPEOF_034](crates/bench/sqlite_parity/cases/SQLITE_PARITY_361_SCALAR_CAST_TYPEOF_034.rs) | P1 | memory | GEN_SQL_SCALAR | 1506389 | 2886771 | <span style="color:#16a34a">3.77%</span> |
| 985 | [00545 AGG_GROUP_HAVING_038](crates/bench/sqlite_parity/cases/SQLITE_PARITY_545_AGG_GROUP_HAVING_038.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1528281 | 2886400 | <span style="color:#16a34a">3.79%</span> |
| 986 | [00265 SCALAR_CAST_TYPEOF_010](crates/bench/sqlite_parity/cases/SQLITE_PARITY_265_SCALAR_CAST_TYPEOF_010.rs) | P1 | memory | GEN_SQL_SCALAR | 1759679 | 2884216 | <span style="color:#16a34a">3.86%</span> |
| 987 | [00238 SCALAR_STRING_003](crates/bench/sqlite_parity/cases/SQLITE_PARITY_238_SCALAR_STRING_003.rs) | P1 | memory | GEN_SQL_SCALAR | 2637179 | 2883875 | <span style="color:#16a34a">3.87%</span> |
| 988 | [00259 SCALAR_NULL_COALESCE_008](crates/bench/sqlite_parity/cases/SQLITE_PARITY_259_SCALAR_NULL_COALESCE_008.rs) | P1 | memory | GEN_SQL_SCALAR | 1500158 | 2880469 | <span style="color:#16a34a">3.98%</span> |
| 989 | [00254 SCALAR_STRING_007](crates/bench/sqlite_parity/cases/SQLITE_PARITY_254_SCALAR_STRING_007.rs) | P1 | memory | GEN_SQL_SCALAR | 1617079 | 2880319 | <span style="color:#16a34a">3.99%</span> |
| 990 | [00382 SCALAR_STRING_039](crates/bench/sqlite_parity/cases/SQLITE_PARITY_382_SCALAR_STRING_039.rs) | P1 | memory | GEN_SQL_SCALAR | 1459460 | 2879808 | <span style="color:#16a34a">4.01%</span> |
| 991 | [00565 AGG_GROUP_HAVING_058](crates/bench/sqlite_parity/cases/SQLITE_PARITY_565_AGG_GROUP_HAVING_058.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1534302 | 2878175 | <span style="color:#16a34a">4.06%</span> |
| 992 | [00027 FOREIGN_KEY_FAILURE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_027_FOREIGN_KEY_FAILURE.rs) | P0 | memory | SQL_FOREIGN_KEYS_NEGATIVE | 1847164 | 2876381 | <span style="color:#16a34a">4.12%</span> |
| 993 | [00086 DATE_TIME_FUNCTIONS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_086_DATE_TIME_FUNCTIONS.rs) | P0 | memory | SQL_FUNCTIONS | 1493876 | 2875960 | <span style="color:#16a34a">4.13%</span> |
| 994 | [00032 UNIQUE_INDEX_FAILURE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_032_UNIQUE_INDEX_FAILURE.rs) | P0 | memory | SQL_INDEX_NEGATIVE | 1793493 | 2875399 | <span style="color:#16a34a">4.15%</span> |
| 995 | [00376 SCALAR_ARITH_038](crates/bench/sqlite_parity/cases/SQLITE_PARITY_376_SCALAR_ARITH_038.rs) | P1 | memory | GEN_SQL_SCALAR | 1830413 | 2874858 | <span style="color:#16a34a">4.17%</span> |
| 996 | [00385 SCALAR_CAST_TYPEOF_040](crates/bench/sqlite_parity/cases/SQLITE_PARITY_385_SCALAR_CAST_TYPEOF_040.rs) | P1 | memory | GEN_SQL_SCALAR | 1461725 | 2874027 | <span style="color:#16a34a">4.20%</span> |
| 997 | [00186 OPT_NEWLINE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_186_OPT_NEWLINE.rs) | P2 | memory | CLI_OPTION | 1546986 | 2874007 | <span style="color:#16a34a">4.20%</span> |
| 998 | [00261 SCALAR_CAST_TYPEOF_009](crates/bench/sqlite_parity/cases/SQLITE_PARITY_261_SCALAR_CAST_TYPEOF_009.rs) | P1 | memory | GEN_SQL_SCALAR | 1508864 | 2872444 | <span style="color:#16a34a">4.25%</span> |
| 999 | [00229 SCALAR_CAST_TYPEOF_001](crates/bench/sqlite_parity/cases/SQLITE_PARITY_229_SCALAR_CAST_TYPEOF_001.rs) | P1 | memory | GEN_SQL_SCALAR | 1478407 | 2869578 | <span style="color:#16a34a">4.35%</span> |
| 1000 | [00087 DATE_TIMEDIFF_FUNCTION](crates/bench/sqlite_parity/cases/SQLITE_PARITY_087_DATE_TIMEDIFF_FUNCTION.rs) | P0 | memory | SQL_FUNCTIONS | 1604054 | 2866573 | <span style="color:#16a34a">4.45%</span> |
| 1001 | [00314 SCALAR_STRING_022](crates/bench/sqlite_parity/cases/SQLITE_PARITY_314_SCALAR_STRING_022.rs) | P1 | memory | GEN_SQL_SCALAR | 1514034 | 2862595 | <span style="color:#16a34a">4.58%</span> |
| 1002 | [00324 SCALAR_ARITH_025](crates/bench/sqlite_parity/cases/SQLITE_PARITY_324_SCALAR_ARITH_025.rs) | P1 | memory | GEN_SQL_SCALAR | 1524234 | 2861313 | <span style="color:#16a34a">4.62%</span> |
| 1003 | [00280 SCALAR_ARITH_014](crates/bench/sqlite_parity/cases/SQLITE_PARITY_280_SCALAR_ARITH_014.rs) | P1 | memory | GEN_SQL_SCALAR | 1482384 | 2860972 | <span style="color:#16a34a">4.63%</span> |
| 1004 | [00240 SCALAR_ARITH_004](crates/bench/sqlite_parity/cases/SQLITE_PARITY_240_SCALAR_ARITH_004.rs) | P1 | memory | GEN_SQL_SCALAR | 1509135 | 2860491 | <span style="color:#16a34a">4.65%</span> |
| 1005 | [00292 SCALAR_ARITH_017](crates/bench/sqlite_parity/cases/SQLITE_PARITY_292_SCALAR_ARITH_017.rs) | P1 | memory | GEN_SQL_SCALAR | 1435976 | 2855261 | <span style="color:#16a34a">4.82%</span> |
| 1006 | [00304 SCALAR_ARITH_020](crates/bench/sqlite_parity/cases/SQLITE_PARITY_304_SCALAR_ARITH_020.rs) | P1 | memory | GEN_SQL_SCALAR | 1804574 | 2855021 | <span style="color:#16a34a">4.83%</span> |
| 1007 | [00379 SCALAR_NULL_COALESCE_038](crates/bench/sqlite_parity/cases/SQLITE_PARITY_379_SCALAR_NULL_COALESCE_038.rs) | P1 | memory | GEN_SQL_SCALAR | 1479389 | 2854660 | <span style="color:#16a34a">4.84%</span> |
| 1008 | [00113 DOT_NULLVALUE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_113_DOT_NULLVALUE.rs) | P0 | memory | CLI_DOT_COMMAND | 1624433 | 2854600 | <span style="color:#16a34a">4.85%</span> |
| 1009 | [00232 SCALAR_ARITH_002](crates/bench/sqlite_parity/cases/SQLITE_PARITY_232_SCALAR_ARITH_002.rs) | P1 | memory | GEN_SQL_SCALAR | 1463839 | 2854460 | <span style="color:#16a34a">4.85%</span> |
| 1010 | [00110 DOT_MODE_LINE_COLUMN_TABLE_BOX_MARKDOWN](crates/bench/sqlite_parity/cases/SQLITE_PARITY_110_DOT_MODE_LINE_COLUMN_TABLE_BOX_MARKDOWN.rs) | P0 | memory | CLI_DOT_COMMAND | 1628922 | 2852025 | <span style="color:#16a34a">4.93%</span> |
| 1011 | [00344 SCALAR_ARITH_030](crates/bench/sqlite_parity/cases/SQLITE_PARITY_344_SCALAR_ARITH_030.rs) | P1 | memory | GEN_SQL_SCALAR | 1545033 | 2851474 | <span style="color:#16a34a">4.95%</span> |
| 1012 | [00256 SCALAR_ARITH_008](crates/bench/sqlite_parity/cases/SQLITE_PARITY_256_SCALAR_ARITH_008.rs) | P1 | memory | GEN_SQL_SCALAR | 1545393 | 2851274 | <span style="color:#16a34a">4.96%</span> |
| 1013 | [00088 JSON_SCALAR_FUNCTIONS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_088_JSON_SCALAR_FUNCTIONS.rs) | P0 | memory | SQL_JSON | 1558909 | 2850092 | <span style="color:#16a34a">5.00%</span> |
| 1014 | [00199 OPT_PAGECACHE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_199_OPT_PAGECACHE.rs) | P3 | memory | CLI_OPTION | 1492794 | 2849531 | <span style="color:#2563eb">5.02%</span> |
| 1015 | [00358 SCALAR_STRING_033](crates/bench/sqlite_parity/cases/SQLITE_PARITY_358_SCALAR_STRING_033.rs) | P1 | memory | GEN_SQL_SCALAR | 1776140 | 2849420 | <span style="color:#2563eb">5.02%</span> |
| 1016 | [00368 SCALAR_ARITH_036](crates/bench/sqlite_parity/cases/SQLITE_PARITY_368_SCALAR_ARITH_036.rs) | P1 | memory | GEN_SQL_SCALAR | 1517972 | 2848909 | <span style="color:#2563eb">5.04%</span> |
| 1017 | [00121 DOT_PARAMETER](crates/bench/sqlite_parity/cases/SQLITE_PARITY_121_DOT_PARAMETER.rs) | P0 | memory | CLI_DOT_COMMAND | 1752986 | 2848428 | <span style="color:#2563eb">5.05%</span> |
| 1018 | [00763 CTE_RECURSIVE_MATRIX_056](crates/bench/sqlite_parity/cases/SQLITE_PARITY_763_CTE_RECURSIVE_MATRIX_056.rs) | P1 | memory | GEN_SQL_CTE | 1614644 | 2847566 | <span style="color:#2563eb">5.08%</span> |
| 1019 | [00328 SCALAR_ARITH_026](crates/bench/sqlite_parity/cases/SQLITE_PARITY_328_SCALAR_ARITH_026.rs) | P1 | memory | GEN_SQL_SCALAR | 1535555 | 2847226 | <span style="color:#2563eb">5.09%</span> |
| 1020 | [00231 SCALAR_NULL_COALESCE_001](crates/bench/sqlite_parity/cases/SQLITE_PARITY_231_SCALAR_NULL_COALESCE_001.rs) | P1 | memory | GEN_SQL_SCALAR | 1450895 | 2842837 | <span style="color:#2563eb">5.24%</span> |
| 1021 | [00340 SCALAR_ARITH_029](crates/bench/sqlite_parity/cases/SQLITE_PARITY_340_SCALAR_ARITH_029.rs) | P1 | memory | GEN_SQL_SCALAR | 1553078 | 2840263 | <span style="color:#2563eb">5.32%</span> |
| 1022 | [00236 SCALAR_ARITH_003](crates/bench/sqlite_parity/cases/SQLITE_PARITY_236_SCALAR_ARITH_003.rs) | P1 | memory | GEN_SQL_SCALAR | 1448991 | 2838440 | <span style="color:#2563eb">5.39%</span> |
| 1023 | [00085 CORE_RANDOM_SHAPE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_085_CORE_RANDOM_SHAPE.rs) | P0 | memory | SQL_FUNCTIONS | 1495249 | 2836626 | <span style="color:#2563eb">5.45%</span> |
| 1024 | [00272 SCALAR_ARITH_012](crates/bench/sqlite_parity/cases/SQLITE_PARITY_272_SCALAR_ARITH_012.rs) | P1 | memory | GEN_SQL_SCALAR | 1493525 | 2835705 | <span style="color:#2563eb">5.48%</span> |
| 1025 | [00082 CORE_STRING_FUNCTIONS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_082_CORE_STRING_FUNCTIONS.rs) | P0 | memory | SQL_FUNCTIONS | 2397656 | 2833430 | <span style="color:#2563eb">5.55%</span> |
| 1026 | [00252 SCALAR_ARITH_007](crates/bench/sqlite_parity/cases/SQLITE_PARITY_252_SCALAR_ARITH_007.rs) | P1 | memory | GEN_SQL_SCALAR | 1499586 | 2832318 | <span style="color:#2563eb">5.59%</span> |
| 1027 | [00228 SCALAR_ARITH_001](crates/bench/sqlite_parity/cases/SQLITE_PARITY_228_SCALAR_ARITH_001.rs) | P1 | memory | GEN_SQL_SCALAR | 1453821 | 2832248 | <span style="color:#2563eb">5.59%</span> |
| 1028 | [00227 OPT_UNSAFE_TESTING_CATALOG](crates/bench/sqlite_parity/cases/SQLITE_PARITY_227_OPT_UNSAFE_TESTING_CATALOG.rs) | P4 | catalog | CLI_OPTION_CATALOG | 1473006 | 2831145 | <span style="color:#2563eb">5.63%</span> |
| 1029 | [00005 UNIQUE_CONSTRAINT_FAILURE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_005_UNIQUE_CONSTRAINT_FAILURE.rs) | P0 | memory | SQL_CONSTRAINTS_NEGATIVE | 1992941 | 2830745 | <span style="color:#2563eb">5.64%</span> |
| 1030 | [00268 SCALAR_ARITH_011](crates/bench/sqlite_parity/cases/SQLITE_PARITY_268_SCALAR_ARITH_011.rs) | P1 | memory | GEN_SQL_SCALAR | 1459521 | 2830564 | <span style="color:#2563eb">5.65%</span> |
| 1031 | [00198 OPT_LOOKASIDE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_198_OPT_LOOKASIDE.rs) | P3 | memory | CLI_OPTION | 1517962 | 2827880 | <span style="color:#2563eb">5.74%</span> |
| 1032 | [00208 OPT_VFSTRACE_CATALOG](crates/bench/sqlite_parity/cases/SQLITE_PARITY_208_OPT_VFSTRACE_CATALOG.rs) | P4 | catalog | CLI_OPTION_CATALOG | 1571713 | 2817460 | <span style="color:#2563eb">6.08%</span> |
| 1033 | [00329 SCALAR_CAST_TYPEOF_026](crates/bench/sqlite_parity/cases/SQLITE_PARITY_329_SCALAR_CAST_TYPEOF_026.rs) | P1 | memory | GEN_SQL_SCALAR | 1686220 | 2815717 | <span style="color:#2563eb">6.14%</span> |
| 1034 | [00080 NOT_NULL_FAILURE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_080_NOT_NULL_FAILURE.rs) | P0 | memory | SQL_CONSTRAINTS_NEGATIVE | 2174533 | 2815366 | <span style="color:#2563eb">6.15%</span> |
| 1035 | [00089 JSON_TABLE_VALUED_FUNCTIONS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_089_JSON_TABLE_VALUED_FUNCTIONS.rs) | P0 | memory | SQL_JSON | 1499908 | 2815126 | <span style="color:#2563eb">6.16%</span> |
| 1036 | [00182 OPT_COLUMN_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_182_OPT_COLUMN_MODE.rs) | P2 | memory | CLI_OPTION | 1597622 | 2812590 | <span style="color:#2563eb">6.25%</span> |
| 1037 | [00360 SCALAR_ARITH_034](crates/bench/sqlite_parity/cases/SQLITE_PARITY_360_SCALAR_ARITH_034.rs) | P1 | memory | GEN_SQL_SCALAR | 1456084 | 2811339 | <span style="color:#2563eb">6.29%</span> |
| 1038 | [00105 CASE_SENSITIVE_LIKE_PRAGMA](crates/bench/sqlite_parity/cases/SQLITE_PARITY_105_CASE_SENSITIVE_LIKE_PRAGMA.rs) | P2 | memory | SQL_PRAGMA | 1498064 | 2810247 | <span style="color:#2563eb">6.33%</span> |
| 1039 | [00025 BEGIN_MODES](crates/bench/sqlite_parity/cases/SQLITE_PARITY_025_BEGIN_MODES.rs) | P0 | memory | SQL_TRANSACTION | 1516509 | 2807461 | <span style="color:#2563eb">6.42%</span> |
| 1040 | [00380 SCALAR_ARITH_039](crates/bench/sqlite_parity/cases/SQLITE_PARITY_380_SCALAR_ARITH_039.rs) | P1 | memory | GEN_SQL_SCALAR | 1468558 | 2806589 | <span style="color:#2563eb">6.45%</span> |
| 1041 | [00224 OPT_STATS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_224_OPT_STATS.rs) | P3 | memory | CLI_OPTION_DIAGNOSTIC | 1962062 | 2804796 | <span style="color:#2563eb">6.51%</span> |
| 1042 | [00076 EXPLAIN_BYTECODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_076_EXPLAIN_BYTECODE.rs) | P0 | memory | SQL_EXPLAIN | 2228976 | 2804486 | <span style="color:#2563eb">6.52%</span> |
| 1043 | [00207 OPT_PCACHETRACE_CATALOG](crates/bench/sqlite_parity/cases/SQLITE_PARITY_207_OPT_PCACHETRACE_CATALOG.rs) | P4 | catalog | CLI_OPTION_CATALOG | 1717058 | 2802131 | <span style="color:#2563eb">6.60%</span> |
| 1044 | [00173 OPT_LIST_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_173_OPT_LIST_MODE.rs) | P1 | memory | CLI_OPTION | 1497382 | 2800618 | <span style="color:#2563eb">6.65%</span> |
| 1045 | [00097 CLI_GENERATE_SERIES_OPTIONAL](crates/bench/sqlite_parity/cases/SQLITE_PARITY_097_CLI_GENERATE_SERIES_OPTIONAL.rs) | P2 | memory | CLI_EXTENSION_OPTIONAL | 1538169 | 2796610 | <span style="color:#2563eb">6.78%</span> |
| 1046 | [00332 SCALAR_ARITH_027](crates/bench/sqlite_parity/cases/SQLITE_PARITY_332_SCALAR_ARITH_027.rs) | P1 | memory | GEN_SQL_SCALAR | 1526447 | 2795919 | <span style="color:#2563eb">6.80%</span> |
| 1047 | [00206 OPT_MEMTRACE_CATALOG](crates/bench/sqlite_parity/cases/SQLITE_PARITY_206_OPT_MEMTRACE_CATALOG.rs) | P4 | catalog | CLI_OPTION_CATALOG | 2031763 | 2791210 | <span style="color:#2563eb">6.96%</span> |
| 1048 | [00169 DOT_NONCE_SAFE_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_169_DOT_NONCE_SAFE_MODE.rs) | P2 | memory | CLI_DOT_COMMAND | 1814092 | 2786151 | <span style="color:#2563eb">7.13%</span> |
| 1049 | [00131 DOT_TIMEOUT](crates/bench/sqlite_parity/cases/SQLITE_PARITY_131_DOT_TIMEOUT.rs) | P0 | memory | CLI_DOT_COMMAND | 1615405 | 2785429 | <span style="color:#2563eb">7.15%</span> |
| 1050 | [00288 SCALAR_ARITH_016](crates/bench/sqlite_parity/cases/SQLITE_PARITY_288_SCALAR_ARITH_016.rs) | P1 | memory | GEN_SQL_SCALAR | 1488876 | 2784077 | <span style="color:#2563eb">7.20%</span> |
| 1051 | [00185 OPT_SEPARATOR](crates/bench/sqlite_parity/cases/SQLITE_PARITY_185_OPT_SEPARATOR.rs) | P1 | memory | CLI_OPTION | 1509866 | 2783786 | <span style="color:#2563eb">7.21%</span> |
| 1052 | [00372 SCALAR_ARITH_037](crates/bench/sqlite_parity/cases/SQLITE_PARITY_372_SCALAR_ARITH_037.rs) | P1 | memory | GEN_SQL_SCALAR | 1505187 | 2783205 | <span style="color:#2563eb">7.23%</span> |
| 1053 | [00069 COLLATE_NOCASE_RTRIM_BINARY](crates/bench/sqlite_parity/cases/SQLITE_PARITY_069_COLLATE_NOCASE_RTRIM_BINARY.rs) | P0 | memory | SQL_COLLATION | 1500879 | 2782824 | <span style="color:#2563eb">7.24%</span> |
| 1054 | [00126 DOT_STATS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_126_DOT_STATS.rs) | P3 | memory | CLI_DOT_COMMAND_DIAGNOSTIC | 1561885 | 2780119 | <span style="color:#2563eb">7.33%</span> |
| 1055 | [00189 OPT_ECHO](crates/bench/sqlite_parity/cases/SQLITE_PARITY_189_OPT_ECHO.rs) | P2 | memory | CLI_OPTION | 1757935 | 2777825 | <span style="color:#2563eb">7.41%</span> |
| 1056 | [00221 OPT_DOUBLE_DASH_END_OPTIONS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_221_OPT_DOUBLE_DASH_END_OPTIONS.rs) | P2 | memory | CLI_OPTION | 1674778 | 2773367 | <span style="color:#2563eb">7.55%</span> |
| 1057 | [00364 SCALAR_ARITH_035](crates/bench/sqlite_parity/cases/SQLITE_PARITY_364_SCALAR_ARITH_035.rs) | P1 | memory | GEN_SQL_SCALAR | 1905195 | 2770921 | <span style="color:#2563eb">7.64%</span> |
| 1058 | [00132 DOT_TRACE_STDOUT](crates/bench/sqlite_parity/cases/SQLITE_PARITY_132_DOT_TRACE_STDOUT.rs) | P2 | memory | CLI_DOT_COMMAND_DIAGNOSTIC | 1586010 | 2768908 | <span style="color:#2563eb">7.70%</span> |
| 1059 | [00183 OPT_TABS_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_183_OPT_TABS_MODE.rs) | P2 | memory | CLI_OPTION | 1489868 | 2765552 | <span style="color:#2563eb">7.81%</span> |
| 1060 | [00172 OPT_CMD](crates/bench/sqlite_parity/cases/SQLITE_PARITY_172_OPT_CMD.rs) | P1 | memory | CLI_OPTION | 1479609 | 2763548 | <span style="color:#2563eb">7.88%</span> |
| 1061 | [00048 PRAGMA_USER_VERSION](crates/bench/sqlite_parity/cases/SQLITE_PARITY_048_PRAGMA_USER_VERSION.rs) | P0 | memory | SQL_PRAGMA | 1615917 | 2761815 | <span style="color:#2563eb">7.94%</span> |
| 1062 | [00077 COMMENTS_AND_CLI_TERMINATORS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_077_COMMENTS_AND_CLI_TERMINATORS.rs) | P0 | memory | CLI_SQL_INPUT | 1471815 | 2757637 | <span style="color:#2563eb">8.08%</span> |
| 1063 | [00124 DOT_BAIL_OFF](crates/bench/sqlite_parity/cases/SQLITE_PARITY_124_DOT_BAIL_OFF.rs) | P0 | memory | CLI_DOT_COMMAND_NEGATIVE | 1552136 | 2755182 | <span style="color:#2563eb">8.16%</span> |
| 1064 | [00352 SCALAR_ARITH_032](crates/bench/sqlite_parity/cases/SQLITE_PARITY_352_SCALAR_ARITH_032.rs) | P1 | memory | GEN_SQL_SCALAR | 1519244 | 2754410 | <span style="color:#2563eb">8.19%</span> |
| 1065 | [00133 DOT_AUTH](crates/bench/sqlite_parity/cases/SQLITE_PARITY_133_DOT_AUTH.rs) | P3 | memory | CLI_DOT_COMMAND_DIAGNOSTIC | 1510086 | 2752587 | <span style="color:#2563eb">8.25%</span> |
| 1066 | [00384 SCALAR_ARITH_040](crates/bench/sqlite_parity/cases/SQLITE_PARITY_384_SCALAR_ARITH_040.rs) | P1 | memory | GEN_SQL_SCALAR | 1538320 | 2752296 | <span style="color:#2563eb">8.26%</span> |
| 1067 | [00175 OPT_QUOTE_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_175_OPT_QUOTE_MODE.rs) | P1 | memory | CLI_OPTION | 1477635 | 2751946 | <span style="color:#2563eb">8.27%</span> |
| 1068 | [00180 OPT_BOX_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_180_OPT_BOX_MODE.rs) | P2 | memory | CLI_OPTION | 1537729 | 2750463 | <span style="color:#2563eb">8.32%</span> |
| 1069 | [00192 OPT_INIT_TEMPFILE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_192_OPT_INIT_TEMPFILE.rs) | P2 | tempfile | CLI_OPTION_TEMPFILE | 1338241 | 2748009 | <span style="color:#2563eb">8.40%</span> |
| 1070 | [00134 DOT_CRLF](crates/bench/sqlite_parity/cases/SQLITE_PARITY_134_DOT_CRLF.rs) | P3 | memory | CLI_DOT_COMMAND_DIAGNOSTIC | 1522169 | 2747217 | <span style="color:#2563eb">8.43%</span> |
| 1071 | [00200 OPT_HEAP](crates/bench/sqlite_parity/cases/SQLITE_PARITY_200_OPT_HEAP.rs) | P4 | memory | CLI_OPTION | 1458739 | 2745934 | <span style="color:#2563eb">8.47%</span> |
| 1072 | [00177 OPT_JSON_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_177_OPT_JSON_MODE.rs) | P1 | memory | CLI_OPTION | 1425347 | 2743660 | <span style="color:#2563eb">8.54%</span> |
| 1073 | [00136 DOT_LOG](crates/bench/sqlite_parity/cases/SQLITE_PARITY_136_DOT_LOG.rs) | P0 | memory | CLI_DOT_COMMAND | 1486102 | 2741756 | <span style="color:#2563eb">8.61%</span> |
| 1074 | [00187 OPT_NULLVALUE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_187_OPT_NULLVALUE.rs) | P1 | memory | CLI_OPTION | 1431939 | 2740534 | <span style="color:#2563eb">8.65%</span> |
| 1075 | [00123 DOT_ECHO](crates/bench/sqlite_parity/cases/SQLITE_PARITY_123_DOT_ECHO.rs) | P0 | memory | CLI_DOT_COMMAND | 1501761 | 2738501 | <span style="color:#2563eb">8.72%</span> |
| 1076 | [00184 OPT_ASCII_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_184_OPT_ASCII_MODE.rs) | P2 | memory | CLI_OPTION | 1862824 | 2733992 | <span style="color:#2563eb">8.87%</span> |
| 1077 | [00188 OPT_HEADER](crates/bench/sqlite_parity/cases/SQLITE_PARITY_188_OPT_HEADER.rs) | P1 | memory | CLI_OPTION | 1486482 | 2731748 | <span style="color:#2563eb">8.94%</span> |
| 1078 | [00176 OPT_LINE_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_176_OPT_LINE_MODE.rs) | P2 | memory | CLI_OPTION | 1777612 | 2727830 | <span style="color:#2563eb">9.07%</span> |
| 1079 | [00181 OPT_TABLE_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_181_OPT_TABLE_MODE.rs) | P2 | memory | CLI_OPTION | 1456435 | 2727680 | <span style="color:#2563eb">9.08%</span> |
| 1080 | [00223 OPT_NOHEADER](crates/bench/sqlite_parity/cases/SQLITE_PARITY_223_OPT_NOHEADER.rs) | P2 | memory | CLI_OPTION | 1482384 | 2727540 | <span style="color:#2563eb">9.08%</span> |
| 1081 | [00179 OPT_MARKDOWN_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_179_OPT_MARKDOWN_MODE.rs) | P2 | memory | CLI_OPTION | 1422551 | 2717080 | <span style="color:#2563eb">9.43%</span> |
| 1082 | [00144 DOT_PROMPT](crates/bench/sqlite_parity/cases/SQLITE_PARITY_144_DOT_PROMPT.rs) | P0 | memory | CLI_DOT_COMMAND | 1483406 | 2712381 | <span style="color:#2563eb">9.59%</span> |
| 1083 | [00264 SCALAR_ARITH_010](crates/bench/sqlite_parity/cases/SQLITE_PARITY_264_SCALAR_ARITH_010.rs) | P1 | memory | GEN_SQL_SCALAR | 1471984 | 2709907 | <span style="color:#2563eb">9.67%</span> |
| 1084 | [00125 DOT_TIMER](crates/bench/sqlite_parity/cases/SQLITE_PARITY_125_DOT_TIMER.rs) | P3 | memory | CLI_DOT_COMMAND_DIAGNOSTIC | 1501851 | 2703264 | <span style="color:#2563eb">9.89%</span> |
| 1085 | [00190 OPT_BAIL](crates/bench/sqlite_parity/cases/SQLITE_PARITY_190_OPT_BAIL.rs) | P1 | memory | CLI_OPTION_NEGATIVE | 1497032 | 2702201 | <span style="color:#2563eb">9.93%</span> |
| 1086 | [00178 OPT_HTML_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_178_OPT_HTML_MODE.rs) | P2 | memory | CLI_OPTION | 1428362 | 2694387 | <span style="color:#2563eb">10.19%</span> |
| 1087 | [00174 OPT_CSV_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_174_OPT_CSV_MODE.rs) | P1 | memory | CLI_OPTION | 1437840 | 2689899 | <span style="color:#2563eb">10.34%</span> |
| 1088 | [00135 DOT_PROGRESS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_135_DOT_PROGRESS.rs) | P0 | memory | CLI_DOT_COMMAND | 1468759 | 2682274 | <span style="color:#2563eb">10.59%</span> |
| 1089 | [00111 DOT_HEADERS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_111_DOT_HEADERS.rs) | P0 | memory | CLI_DOT_COMMAND | 1556535 | 2681803 | <span style="color:#2563eb">10.61%</span> |
| 1090 | [00066 VALUES_STATEMENT](crates/bench/sqlite_parity/cases/SQLITE_PARITY_066_VALUES_STATEMENT.rs) | P0 | memory | SQL_VALUES | 1484728 | 2679128 | <span style="color:#2563eb">10.70%</span> |
| 1091 | [00334 SCALAR_STRING_027](crates/bench/sqlite_parity/cases/SQLITE_PARITY_334_SCALAR_STRING_027.rs) | P1 | memory | GEN_SQL_SCALAR | 2243594 | 2667166 | <span style="color:#2563eb">11.09%</span> |
| 1092 | [00139 DOT_LINT_FKEY_INDEXES](crates/bench/sqlite_parity/cases/SQLITE_PARITY_139_DOT_LINT_FKEY_INDEXES.rs) | P3 | memory | CLI_DOT_COMMAND_DIAGNOSTIC | 1669318 | 2662847 | <span style="color:#2563eb">11.24%</span> |
| 1093 | [00120 DOT_EXPLAIN](crates/bench/sqlite_parity/cases/SQLITE_PARITY_120_DOT_EXPLAIN.rs) | P0 | memory | CLI_DOT_COMMAND | 1469840 | 2647999 | <span style="color:#2563eb">11.73%</span> |
| 1094 | [00112 DOT_SEPARATOR](crates/bench/sqlite_parity/cases/SQLITE_PARITY_112_DOT_SEPARATOR.rs) | P0 | memory | CLI_DOT_COMMAND | 1440004 | 2634484 | <span style="color:#2563eb">12.18%</span> |
| 1095 | [00153 DOT_CD_TEMPFILE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_153_DOT_CD_TEMPFILE.rs) | P2 | tempfile | CLI_TEMPFILE | 1262197 | 2475884 | <span style="color:#2563eb">17.47%</span> |
| 1096 | [00157 DOT_ARCHIVE_TEMPFILE_OPTIONAL](crates/bench/sqlite_parity/cases/SQLITE_PARITY_157_DOT_ARCHIVE_TEMPFILE_OPTIONAL.rs) | P3 | tempfile | CLI_TEMPFILE_OPTIONAL | 2076317 | 2406823 | <span style="color:#2563eb">19.77%</span> |
| 1097 | [00218 PRAGMA_FORMS_SCHEMA_EQUALS_PARENS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_218_PRAGMA_FORMS_SCHEMA_EQUALS_PARENS.rs) | P0 | memory | SQL_PRAGMA | 1515497 | 2379361 | <span style="color:#2563eb">20.69%</span> |
| 1098 | [00049 PRAGMA_TEMP_STORE_MEMORY](crates/bench/sqlite_parity/cases/SQLITE_PARITY_049_PRAGMA_TEMP_STORE_MEMORY.rs) | P0 | memory | SQL_PRAGMA | 1484649 | 2296935 | <span style="color:#2563eb">23.44%</span> |
| 1099 | [00158 DOT_SHELL_SIDE_EFFECT](crates/bench/sqlite_parity/cases/SQLITE_PARITY_158_DOT_SHELL_SIDE_EFFECT.rs) | P4 | side_effect | CLI_SIDE_EFFECT | 2448492 | 2281356 | <span style="color:#2563eb">23.95%</span> |
| 1100 | [00047 PRAGMA_FOREIGN_KEYS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_047_PRAGMA_FOREIGN_KEYS.rs) | P0 | memory | SQL_PRAGMA | 1495369 | 2278630 | <span style="color:#2563eb">24.05%</span> |
| 1101 | [00162 DOT_LOAD_EXTENSION_NEGATIVE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_162_DOT_LOAD_EXTENSION_NEGATIVE.rs) | P4 | side_effect | CLI_SIDE_EFFECT | 1808321 | 2211243 | <span style="color:#2563eb">26.29%</span> |
| 1102 | [00137 DOT_VERSION](crates/bench/sqlite_parity/cases/SQLITE_PARITY_137_DOT_VERSION.rs) | P2 | memory | CLI_DOT_COMMAND_DIAGNOSTIC | 1439523 | 2210752 | <span style="color:#2563eb">26.31%</span> |
| 1103 | [00159 DOT_SYSTEM_SIDE_EFFECT](crates/bench/sqlite_parity/cases/SQLITE_PARITY_159_DOT_SYSTEM_SIDE_EFFECT.rs) | P4 | side_effect | CLI_SIDE_EFFECT | 2560283 | 2190724 | <span style="color:#2563eb">26.98%</span> |
| 1104 | [00164 DOT_IMPOSTER_CATALOG](crates/bench/sqlite_parity/cases/SQLITE_PARITY_164_DOT_IMPOSTER_CATALOG.rs) | P4 | catalog | CLI_CATALOG | 1402863 | 2188230 | <span style="color:#2563eb">27.06%</span> |
| 1105 | [00116 DOT_DATABASES](crates/bench/sqlite_parity/cases/SQLITE_PARITY_116_DOT_DATABASES.rs) | P0 | memory | CLI_DOT_COMMAND | 1552407 | 2175977 | <span style="color:#2563eb">27.47%</span> |
| 1106 | [00138 DOT_VFSNAME_LIST_INFO](crates/bench/sqlite_parity/cases/SQLITE_PARITY_138_DOT_VFSNAME_LIST_INFO.rs) | P3 | memory | CLI_DOT_COMMAND_DIAGNOSTIC | 1470572 | 2175646 | <span style="color:#2563eb">27.48%</span> |
| 1107 | [00106 DOT_HELP](crates/bench/sqlite_parity/cases/SQLITE_PARITY_106_DOT_HELP.rs) | P0 | memory | CLI_DOT_COMMAND | 2193710 | 2174023 | <span style="color:#2563eb">27.53%</span> |
| 1108 | [00114 DOT_PRINT](crates/bench/sqlite_parity/cases/SQLITE_PARITY_114_DOT_PRINT.rs) | P0 | memory | CLI_DOT_COMMAND | 1468768 | 2171337 | <span style="color:#2563eb">27.62%</span> |
| 1109 | [00166 DOT_SESSION_CATALOG](crates/bench/sqlite_parity/cases/SQLITE_PARITY_166_DOT_SESSION_CATALOG.rs) | P4 | catalog | CLI_CATALOG | 1791418 | 2134779 | <span style="color:#2563eb">28.84%</span> |
| 1110 | [00127 DOT_LIMIT](crates/bench/sqlite_parity/cases/SQLITE_PARITY_127_DOT_LIMIT.rs) | P0 | memory | CLI_DOT_COMMAND | 1462076 | 2131512 | <span style="color:#2563eb">28.95%</span> |
| 1111 | [00163 DOT_FILECTRL_CATALOG](crates/bench/sqlite_parity/cases/SQLITE_PARITY_163_DOT_FILECTRL_CATALOG.rs) | P4 | catalog | CLI_CATALOG | 1482305 | 2116473 | <span style="color:#2563eb">29.45%</span> |
| 1112 | [00195 OPT_SAFE_MODE_BLOCKS_SHELL](crates/bench/sqlite_parity/cases/SQLITE_PARITY_195_OPT_SAFE_MODE_BLOCKS_SHELL.rs) | P2 | memory | CLI_OPTION_NEGATIVE | 1896027 | 2114069 | <span style="color:#2563eb">29.53%</span> |
| 1113 | [00128 DOT_DBCONFIG](crates/bench/sqlite_parity/cases/SQLITE_PARITY_128_DOT_DBCONFIG.rs) | P0 | memory | CLI_DOT_COMMAND | 1422651 | 2098981 | <span style="color:#2563eb">30.03%</span> |
| 1114 | [00167 DOT_UNMODULE_CATALOG](crates/bench/sqlite_parity/cases/SQLITE_PARITY_167_DOT_UNMODULE_CATALOG.rs) | P4 | catalog | CLI_CATALOG | 1421449 | 2021695 | <span style="color:#2563eb">32.61%</span> |
| 1115 | [00168 DOT_CHECK_CATALOG](crates/bench/sqlite_parity/cases/SQLITE_PARITY_168_DOT_CHECK_CATALOG.rs) | P4 | catalog | CLI_CATALOG | 1458349 | 2010263 | <span style="color:#2563eb">32.99%</span> |
| 1116 | [00143 DOT_QUIT](crates/bench/sqlite_parity/cases/SQLITE_PARITY_143_DOT_QUIT.rs) | P0 | memory | CLI_DOT_COMMAND | 1423773 | 1881048 | <span style="color:#2563eb">37.30%</span> |
| 1117 | [00142 DOT_EXIT_CODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_142_DOT_EXIT_CODE.rs) | P0 | memory | CLI_DOT_COMMAND | 1353020 | 1879645 | <span style="color:#2563eb">37.35%</span> |
| 1118 | [00165 DOT_INTCK_CATALOG](crates/bench/sqlite_parity/cases/SQLITE_PARITY_165_DOT_INTCK_CATALOG.rs) | P4 | catalog | CLI_CATALOG | 3983506 | 2416432 | <span style="color:#2563eb">39.34%</span> |
| 1119 | [00204 OPT_ZIP_TEMPFILE_CATALOG](crates/bench/sqlite_parity/cases/SQLITE_PARITY_204_OPT_ZIP_TEMPFILE_CATALOG.rs) | P4 | catalog | CLI_OPTION_CATALOG | 1610928 | 1779526 | <span style="color:#2563eb">40.68%</span> |
| 1120 | [00194 OPT_IFEXISTS_NEGATIVE_TEMPFILE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_194_OPT_IFEXISTS_NEGATIVE_TEMPFILE.rs) | P3 | tempfile | CLI_OPTION_TEMPFILE_DIAGNOSTIC | 1348441 | 1700687 | <span style="color:#2563eb">43.31%</span> |
| 1121 | [00226 OPT_NOFOLLOW_CATALOG](crates/bench/sqlite_parity/cases/SQLITE_PARITY_226_OPT_NOFOLLOW_CATALOG.rs) | P4 | catalog | CLI_OPTION_CATALOG | 1311421 | 1607100 | <span style="color:#2563eb">46.43%</span> |
| 1122 | [00171 OPT_HELP](crates/bench/sqlite_parity/cases/SQLITE_PARITY_171_OPT_HELP.rs) | P1 | memory | CLI_OPTION | 1357287 | 1596290 | <span style="color:#2563eb">46.79%</span> |
| 1123 | [00170 OPT_VERSION](crates/bench/sqlite_parity/cases/SQLITE_PARITY_170_OPT_VERSION.rs) | P1 | memory | CLI_OPTION | 1671953 | 1595138 | <span style="color:#2563eb">46.83%</span> |
| 1124 | [00203 OPT_ARCHIVE_A_TEMPFILE_OPTIONAL](crates/bench/sqlite_parity/cases/SQLITE_PARITY_203_OPT_ARCHIVE_A_TEMPFILE_OPTIONAL.rs) | P4 | tempfile | CLI_OPTION_TEMPFILE_OPTIONAL | 1754970 | 1375773 | <span style="color:#2563eb">54.14%</span> |
| 1125 | [00161 DOT_WWW_EXTERNAL_APP](crates/bench/sqlite_parity/cases/SQLITE_PARITY_161_DOT_WWW_EXTERNAL_APP.rs) | P4 | external_app | CLI_EXTERNAL_APP | 32783467 | 2828311 | <span style="color:#2563eb">91.37%</span> |
| 1126 | [00160 DOT_EXCEL_EXTERNAL_APP](crates/bench/sqlite_parity/cases/SQLITE_PARITY_160_DOT_EXCEL_EXTERNAL_APP.rs) | P4 | external_app | CLI_EXTERNAL_APP | 33793508 | 2820796 | <span style="color:#2563eb">91.65%</span> |
| 1127 | [00209 OPT_INTERACTIVE_CATALOG](crates/bench/sqlite_parity/cases/SQLITE_PARITY_209_OPT_INTERACTIVE_CATALOG.rs) | P4 | catalog | CLI_OPTION_CATALOG | 51701404 | 2031253 | <span style="color:#2563eb">96.07%</span> |

</details>

<!-- sqlite-parity-report:end -->

## Jankurai Breakdown

<!-- sqlite-jankurai-breakdown:begin -->

![RedlineDB vs SQLite Jankurai audit breakdown](assets/sqlite-jankurai-comparison.svg)

<!-- sqlite-jankurai-breakdown:end -->

## Architecture

<p align="center">
  <img src="assets/architecture.png" alt="RedlineDB architecture" width="95%">
</p>

<p align="center">
  <img src="assets/dataflow.png" alt="INSERT data flow" width="95%">
</p>

RedlineDB is a layered Rust workspace:

- `crates/redlinedb` is the public embedded facade.
- `crates/sql` owns the parser, planner, executor, and SQLite compatibility.
- `crates/kernel` owns storage, catalog, WAL, MVCC, and recovery.
- `crates/ffi` exports the SQLite-shaped C ABI for compatibility testing.
- `crates/cli` provides the shell and administrative commands.
- `crates/bench` owns the parity corpus and benchmark evidence.

The dependency graph stays one-way: lower layers do not depend on higher layers.
That keeps the engine testable, replaceable, and easy to reason about in the
agent routing model used by this repository.

## Repository Layout

| Path | Purpose |
|---|---|
| `crates/redlinedb/` | Public Rust API |
| `crates/sql/` | SQL parser, planner, executor, and dialect support |
| `crates/kernel/` | Storage, WAL, MVCC, catalog, and recovery |
| `crates/ffi/` | SQLite-shaped C ABI shim |
| `crates/cli/` | Command-line shell |
| `crates/server/` | Optional framed server |
| `crates/bench/` | Parity corpus and bench harness |
| `benchmark-results/sqlite-parity/latest/` | Current parity report artifacts |
| `docs/` | Architecture, testing, and audit guidance |
| `paper/` | Evaluation writeup and reproducibility assets |

## Development Notes

- `just fast` is the default local proof lane for ordinary edits.
- `just sqlite-parity-report-update` refreshes the generated parity report.
- `just sqlite-parity-report-check` verifies the README report block matches the latest artifacts.
- `just sqlite-parity-scale-ci` is the reviewed CI parity gate.
- `scripts/ci-local.sh all` mirrors the broader local CI surface when you need it.

The repository keeps the parity corpus under source control and treats the full
generated corpus as the executable default parity lane. `approved-ci.txt`
remains as a local triage subset for older diagnostics only.

## Contributing

Read the root `AGENTS.md`, then follow `docs/testing.md` for proof lanes and
`docs/architecture.md` for workspace structure. Keep changes narrow, avoid
touching generated zones by hand, and prefer the smallest lawful edit that
restores the invariant.

## Citing

If you reference RedlineDB in a paper or writeup, cite the evaluation material
in `paper/main.pdf` and link back to this repository release.

## License

Apache-2.0. See [LICENSE](LICENSE).
