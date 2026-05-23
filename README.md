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

**SQLite parity latency:** median gap **-6.86%**, worst gap **-64.07%**, faster cases **315** with a **3000000 ns** reference floor (targets: median >= -25%, worst > -85%, faster >= 25).

![SQLite parity latency improvement plot](assets/sqlite-parity-latency-gap.svg)

![SQLite parity performance distribution](assets/sqlite-parity-performance-histogram.svg)

[Full ranked latency table](#sqlite-parity-ranked-latency-table) is collapsed below for README readability.

<details id="sqlite-parity-ranked-latency-table">
<summary>Full ranked latency table</summary>

| Rank | Case | Priority | Profile | Category | SQLite median ns | RedlineDB median ns | Improvement |
| ---: | --- | --- | --- | --- | ---: | ---: | ---: |
| 1 | [00197 OPT_MAXSIZE_DESERIALIZE_TEMPFILE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_197_OPT_MAXSIZE_DESERIALIZE_TEMPFILE.rs) | P3 | tempfile | CLI_OPTION_TEMPFILE | 4996545 | 8197969 | <span style="color:#dc2626">-64.07%</span> |
| 2 | [00152 DOT_CLONE_TEMPFILE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_152_DOT_CLONE_TEMPFILE.rs) | P2 | tempfile | CLI_TEMPFILE | 2073417 | 4806876 | <span style="color:#dc2626">-60.23%</span> |
| 3 | [00970 VIEW_TRIGGER_GENERATED_023](crates/bench/sqlite_parity/cases/SQLITE_PARITY_970_VIEW_TRIGGER_GENERATED_023.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2514633 | 4716874 | <span style="color:#dc2626">-57.23%</span> |
| 4 | [00212 SQL_VACUUM_INTO_TEMPFILE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_212_SQL_VACUUM_INTO_TEMPFILE.rs) | P2 | tempfile | SQL_TEMPFILE | 2014977 | 4658044 | <span style="color:#dc2626">-55.27%</span> |
| 5 | [01083 INDEX_SCHEMA_PRAGMA_016](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1083_INDEX_SCHEMA_PRAGMA_016.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1995811 | 4587760 | <span style="color:#dc2626">-52.93%</span> |
| 6 | [00193 OPT_READONLY_TEMPFILE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_193_OPT_READONLY_TEMPFILE.rs) | P2 | tempfile | CLI_OPTION_TEMPFILE | 6403339 | 9724571 | <span style="color:#dc2626">-51.87%</span> |
| 7 | [00619 JOIN_SUBQUERY_EXISTS_012](crates/bench/sqlite_parity/cases/SQLITE_PARITY_619_JOIN_SUBQUERY_EXISTS_012.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1657149 | 4486488 | <span style="color:#dc2626">-49.55%</span> |
| 8 | [00969 VIEW_TRIGGER_GENERATED_022](crates/bench/sqlite_parity/cases/SQLITE_PARITY_969_VIEW_TRIGGER_GENERATED_022.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2485829 | 4469476 | <span style="color:#dc2626">-48.98%</span> |
| 9 | [00992 VIEW_TRIGGER_GENERATED_045](crates/bench/sqlite_parity/cases/SQLITE_PARITY_992_VIEW_TRIGGER_GENERATED_045.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1676566 | 4444198 | <span style="color:#dc2626">-48.14%</span> |
| 10 | [01103 INDEX_SCHEMA_PRAGMA_036](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1103_INDEX_SCHEMA_PRAGMA_036.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 2602420 | 4443838 | <span style="color:#dc2626">-48.13%</span> |
| 11 | [00502 DML_WHERE_ORDER_LIMIT_115](crates/bench/sqlite_parity/cases/SQLITE_PARITY_502_DML_WHERE_ORDER_LIMIT_115.rs) | P1 | memory | GEN_SQL_DML | 2975215 | 4439760 | <span style="color:#dc2626">-47.99%</span> |
| 12 | [00645 JOIN_SUBQUERY_EXISTS_038](crates/bench/sqlite_parity/cases/SQLITE_PARITY_645_JOIN_SUBQUERY_EXISTS_038.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1747811 | 4439068 | <span style="color:#dc2626">-47.97%</span> |
| 13 | [00115 DOT_SCHEMA_TABLES_INDEXES](crates/bench/sqlite_parity/cases/SQLITE_PARITY_115_DOT_SCHEMA_TABLES_INDEXES.rs) | P0 | memory | CLI_DOT_COMMAND | 2071834 | 4410024 | <span style="color:#dc2626">-47.00%</span> |
| 14 | [00967 VIEW_TRIGGER_GENERATED_020](crates/bench/sqlite_parity/cases/SQLITE_PARITY_967_VIEW_TRIGGER_GENERATED_020.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2003805 | 4405785 | <span style="color:#dc2626">-46.86%</span> |
| 15 | [01002 VIEW_TRIGGER_GENERATED_055](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1002_VIEW_TRIGGER_GENERATED_055.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1613025 | 4377532 | <span style="color:#dc2626">-45.92%</span> |
| 16 | [00211 SQL_ATTACH_TEMPFILE_DATABASE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_211_SQL_ATTACH_TEMPFILE_DATABASE.rs) | P1 | tempfile | SQL_TEMPFILE | 1728925 | 4370268 | <span style="color:#dc2626">-45.68%</span> |
| 17 | [00575 AGG_GROUP_HAVING_068](crates/bench/sqlite_parity/cases/SQLITE_PARITY_575_AGG_GROUP_HAVING_068.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1659243 | 4346974 | <span style="color:#dc2626">-44.90%</span> |
| 18 | [01126 INDEX_SCHEMA_PRAGMA_059](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1126_INDEX_SCHEMA_PRAGMA_059.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1845286 | 4339450 | <span style="color:#dc2626">-44.65%</span> |
| 19 | [00639 JOIN_SUBQUERY_EXISTS_032](crates/bench/sqlite_parity/cases/SQLITE_PARITY_639_JOIN_SUBQUERY_EXISTS_032.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1672267 | 4332457 | <span style="color:#dc2626">-44.42%</span> |
| 20 | [01123 INDEX_SCHEMA_PRAGMA_056](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1123_INDEX_SCHEMA_PRAGMA_056.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1670314 | 4282542 | <span style="color:#dc2626">-42.75%</span> |
| 21 | [00396 DML_WHERE_ORDER_LIMIT_009](crates/bench/sqlite_parity/cases/SQLITE_PARITY_396_DML_WHERE_ORDER_LIMIT_009.rs) | P1 | memory | GEN_SQL_DML | 1575585 | 4282282 | <span style="color:#dc2626">-42.74%</span> |
| 22 | [00516 AGG_GROUP_HAVING_009](crates/bench/sqlite_parity/cases/SQLITE_PARITY_516_AGG_GROUP_HAVING_009.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1795031 | 4280227 | <span style="color:#dc2626">-42.67%</span> |
| 23 | [00147 DOT_IMPORT_CSV_TEMPFILE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_147_DOT_IMPORT_CSV_TEMPFILE.rs) | P1 | tempfile | CLI_TEMPFILE | 1786013 | 4267173 | <span style="color:#dc2626">-42.24%</span> |
| 24 | [00150 DOT_BACKUP_RESTORE_TEMPFILE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_150_DOT_BACKUP_RESTORE_TEMPFILE.rs) | P1 | tempfile | CLI_TEMPFILE | 2042749 | 4260751 | <span style="color:#dc2626">-42.03%</span> |
| 25 | [00710 CTE_RECURSIVE_MATRIX_003](crates/bench/sqlite_parity/cases/SQLITE_PARITY_710_CTE_RECURSIVE_MATRIX_003.rs) | P1 | memory | GEN_SQL_CTE | 1575004 | 4223179 | <span style="color:#dc2626">-40.77%</span> |
| 26 | [00586 AGG_GROUP_HAVING_079](crates/bench/sqlite_parity/cases/SQLITE_PARITY_586_AGG_GROUP_HAVING_079.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1680694 | 4213892 | <span style="color:#dc2626">-40.46%</span> |
| 27 | [01065 JSON_EXTRACT_SET_058](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1065_JSON_EXTRACT_SET_058.rs) | P2 | memory | GEN_SQL_JSON | 1533745 | 4212890 | <span style="color:#dc2626">-40.43%</span> |
| 28 | [00459 DML_WHERE_ORDER_LIMIT_072](crates/bench/sqlite_parity/cases/SQLITE_PARITY_459_DML_WHERE_ORDER_LIMIT_072.rs) | P1 | memory | GEN_SQL_DML | 1604589 | 4211708 | <span style="color:#dc2626">-40.39%</span> |
| 29 | [01081 INDEX_SCHEMA_PRAGMA_014](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1081_INDEX_SCHEMA_PRAGMA_014.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1847099 | 4209173 | <span style="color:#dc2626">-40.31%</span> |
| 30 | [00657 JOIN_SUBQUERY_EXISTS_050](crates/bench/sqlite_parity/cases/SQLITE_PARITY_657_JOIN_SUBQUERY_EXISTS_050.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1621381 | 4180209 | <span style="color:#dc2626">-39.34%</span> |
| 31 | [00156 DOT_RECOVER_TEMPFILE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_156_DOT_RECOVER_TEMPFILE.rs) | P3 | tempfile | CLI_TEMPFILE | 2924149 | 4175319 | <span style="color:#dc2626">-39.18%</span> |
| 32 | [00872 CONSTRAINT_FK_SAVEPOINT_005](crates/bench/sqlite_parity/cases/SQLITE_PARITY_872_CONSTRAINT_FK_SAVEPOINT_005.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2483845 | 4169217 | <span style="color:#dc2626">-38.97%</span> |
| 33 | [01124 INDEX_SCHEMA_PRAGMA_057](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1124_INDEX_SCHEMA_PRAGMA_057.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1724156 | 4160411 | <span style="color:#dc2626">-38.68%</span> |
| 34 | [01003 VIEW_TRIGGER_GENERATED_056](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1003_VIEW_TRIGGER_GENERATED_056.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1611743 | 4152536 | <span style="color:#dc2626">-38.42%</span> |
| 35 | [00751 CTE_RECURSIVE_MATRIX_044](crates/bench/sqlite_parity/cases/SQLITE_PARITY_751_CTE_RECURSIVE_MATRIX_044.rs) | P1 | memory | GEN_SQL_CTE | 1581936 | 4150963 | <span style="color:#dc2626">-38.37%</span> |
| 36 | [00151 DOT_SAVE_RESTORE_TEMPFILE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_151_DOT_SAVE_RESTORE_TEMPFILE.rs) | P1 | tempfile | CLI_TEMPFILE | 1952919 | 4137157 | <span style="color:#dc2626">-37.91%</span> |
| 37 | [00391 DML_WHERE_ORDER_LIMIT_004](crates/bench/sqlite_parity/cases/SQLITE_PARITY_391_DML_WHERE_ORDER_LIMIT_004.rs) | P1 | memory | GEN_SQL_DML | 1535699 | 4136336 | <span style="color:#dc2626">-37.88%</span> |
| 38 | [00675 JOIN_SUBQUERY_EXISTS_068](crates/bench/sqlite_parity/cases/SQLITE_PARITY_675_JOIN_SUBQUERY_EXISTS_068.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1679441 | 4135784 | <span style="color:#dc2626">-37.86%</span> |
| 39 | [00504 DML_WHERE_ORDER_LIMIT_117](crates/bench/sqlite_parity/cases/SQLITE_PARITY_504_DML_WHERE_ORDER_LIMIT_117.rs) | P1 | memory | GEN_SQL_DML | 1556508 | 4133369 | <span style="color:#dc2626">-37.78%</span> |
| 40 | [01064 JSON_EXTRACT_SET_057](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1064_JSON_EXTRACT_SET_057.rs) | P2 | memory | GEN_SQL_JSON | 1807203 | 4127859 | <span style="color:#dc2626">-37.60%</span> |
| 41 | [00775 CTE_RECURSIVE_MATRIX_068](crates/bench/sqlite_parity/cases/SQLITE_PARITY_775_CTE_RECURSIVE_MATRIX_068.rs) | P1 | memory | GEN_SQL_CTE | 2332167 | 4127709 | <span style="color:#dc2626">-37.59%</span> |
| 42 | [00669 JOIN_SUBQUERY_EXISTS_062](crates/bench/sqlite_parity/cases/SQLITE_PARITY_669_JOIN_SUBQUERY_EXISTS_062.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1722843 | 4121717 | <span style="color:#dc2626">-37.39%</span> |
| 43 | [00517 AGG_GROUP_HAVING_010](crates/bench/sqlite_parity/cases/SQLITE_PARITY_517_AGG_GROUP_HAVING_010.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1798967 | 4119763 | <span style="color:#dc2626">-37.33%</span> |
| 44 | [00902 CONSTRAINT_FK_SAVEPOINT_035](crates/bench/sqlite_parity/cases/SQLITE_PARITY_902_CONSTRAINT_FK_SAVEPOINT_035.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1659964 | 4105878 | <span style="color:#dc2626">-36.86%</span> |
| 45 | [00509 AGG_GROUP_HAVING_002](crates/bench/sqlite_parity/cases/SQLITE_PARITY_509_AGG_GROUP_HAVING_002.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1846528 | 4104825 | <span style="color:#dc2626">-36.83%</span> |
| 46 | [01110 INDEX_SCHEMA_PRAGMA_043](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1110_INDEX_SCHEMA_PRAGMA_043.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1911049 | 4104495 | <span style="color:#dc2626">-36.82%</span> |
| 47 | [00024 SAVEPOINT_ROLLBACK_RELEASE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_024_SAVEPOINT_ROLLBACK_RELEASE.rs) | P0 | memory | SQL_SAVEPOINT | 2670178 | 4085559 | <span style="color:#dc2626">-36.19%</span> |
| 48 | [00673 JOIN_SUBQUERY_EXISTS_066](crates/bench/sqlite_parity/cases/SQLITE_PARITY_673_JOIN_SUBQUERY_EXISTS_066.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1616782 | 4080299 | <span style="color:#dc2626">-36.01%</span> |
| 49 | [00895 CONSTRAINT_FK_SAVEPOINT_028](crates/bench/sqlite_parity/cases/SQLITE_PARITY_895_CONSTRAINT_FK_SAVEPOINT_028.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2241075 | 4075370 | <span style="color:#dc2626">-35.85%</span> |
| 50 | [00704 JOIN_SUBQUERY_EXISTS_097](crates/bench/sqlite_parity/cases/SQLITE_PARITY_704_JOIN_SUBQUERY_EXISTS_097.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1936328 | 4074969 | <span style="color:#dc2626">-35.83%</span> |
| 51 | [00596 AGG_GROUP_HAVING_089](crates/bench/sqlite_parity/cases/SQLITE_PARITY_596_AGG_GROUP_HAVING_089.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1716682 | 4067254 | <span style="color:#dc2626">-35.58%</span> |
| 52 | [00736 CTE_RECURSIVE_MATRIX_029](crates/bench/sqlite_parity/cases/SQLITE_PARITY_736_CTE_RECURSIVE_MATRIX_029.rs) | P1 | memory | GEN_SQL_CTE | 1508247 | 4065240 | <span style="color:#dc2626">-35.51%</span> |
| 53 | [01075 INDEX_SCHEMA_PRAGMA_008](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1075_INDEX_SCHEMA_PRAGMA_008.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1712494 | 4064139 | <span style="color:#dc2626">-35.47%</span> |
| 54 | [00878 CONSTRAINT_FK_SAVEPOINT_011](crates/bench/sqlite_parity/cases/SQLITE_PARITY_878_CONSTRAINT_FK_SAVEPOINT_011.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2410506 | 4060412 | <span style="color:#dc2626">-35.35%</span> |
| 55 | [00708 CTE_RECURSIVE_MATRIX_001](crates/bench/sqlite_parity/cases/SQLITE_PARITY_708_CTE_RECURSIVE_MATRIX_001.rs) | P1 | memory | GEN_SQL_CTE | 1489832 | 4060241 | <span style="color:#dc2626">-35.34%</span> |
| 56 | [00011 CREATE_TABLE_AS_SELECT](crates/bench/sqlite_parity/cases/SQLITE_PARITY_011_CREATE_TABLE_AS_SELECT.rs) | P0 | memory | SQL_DDL | 1524057 | 4050413 | <span style="color:#dc2626">-35.01%</span> |
| 57 | [00625 JOIN_SUBQUERY_EXISTS_018](crates/bench/sqlite_parity/cases/SQLITE_PARITY_625_JOIN_SUBQUERY_EXISTS_018.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1658602 | 4049822 | <span style="color:#dc2626">-34.99%</span> |
| 58 | [00975 VIEW_TRIGGER_GENERATED_028](crates/bench/sqlite_parity/cases/SQLITE_PARITY_975_VIEW_TRIGGER_GENERATED_028.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1687746 | 4032568 | <span style="color:#dc2626">-34.42%</span> |
| 59 | [00538 AGG_GROUP_HAVING_031](crates/bench/sqlite_parity/cases/SQLITE_PARITY_538_AGG_GROUP_HAVING_031.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1693779 | 4029302 | <span style="color:#dc2626">-34.31%</span> |
| 60 | [00789 WINDOW_PARTITION_SUM_002](crates/bench/sqlite_parity/cases/SQLITE_PARITY_789_WINDOW_PARTITION_SUM_002.rs) | P2 | memory | GEN_SQL_WINDOW | 1596223 | 4010367 | <span style="color:#dc2626">-33.68%</span> |
| 61 | [00405 DML_WHERE_ORDER_LIMIT_018](crates/bench/sqlite_parity/cases/SQLITE_PARITY_405_DML_WHERE_ORDER_LIMIT_018.rs) | P1 | memory | GEN_SQL_DML | 1626481 | 3996390 | <span style="color:#dc2626">-33.21%</span> |
| 62 | [00747 CTE_RECURSIVE_MATRIX_040](crates/bench/sqlite_parity/cases/SQLITE_PARITY_747_CTE_RECURSIVE_MATRIX_040.rs) | P1 | memory | GEN_SQL_CTE | 1600221 | 3986601 | <span style="color:#dc2626">-32.89%</span> |
| 63 | [00885 CONSTRAINT_FK_SAVEPOINT_018](crates/bench/sqlite_parity/cases/SQLITE_PARITY_885_CONSTRAINT_FK_SAVEPOINT_018.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1632752 | 3981803 | <span style="color:#dc2626">-32.73%</span> |
| 64 | [00800 WINDOW_PARTITION_SUM_013](crates/bench/sqlite_parity/cases/SQLITE_PARITY_800_WINDOW_PARTITION_SUM_013.rs) | P2 | memory | GEN_SQL_WINDOW | 1520901 | 3976232 | <span style="color:#dc2626">-32.54%</span> |
| 65 | [00794 WINDOW_PARTITION_SUM_007](crates/bench/sqlite_parity/cases/SQLITE_PARITY_794_WINDOW_PARTITION_SUM_007.rs) | P2 | memory | GEN_SQL_WINDOW | 1591054 | 3966774 | <span style="color:#dc2626">-32.23%</span> |
| 66 | [00437 DML_WHERE_ORDER_LIMIT_050](crates/bench/sqlite_parity/cases/SQLITE_PARITY_437_DML_WHERE_ORDER_LIMIT_050.rs) | P1 | memory | GEN_SQL_DML | 1574653 | 3959110 | <span style="color:#dc2626">-31.97%</span> |
| 67 | [00941 CONSTRAINT_FK_SAVEPOINT_074](crates/bench/sqlite_parity/cases/SQLITE_PARITY_941_CONSTRAINT_FK_SAVEPOINT_074.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1628364 | 3952928 | <span style="color:#dc2626">-31.76%</span> |
| 68 | [00505 DML_WHERE_ORDER_LIMIT_118](crates/bench/sqlite_parity/cases/SQLITE_PARITY_505_DML_WHERE_ORDER_LIMIT_118.rs) | P1 | memory | GEN_SQL_DML | 2090891 | 3950934 | <span style="color:#dc2626">-31.70%</span> |
| 69 | [00953 VIEW_TRIGGER_GENERATED_006](crates/bench/sqlite_parity/cases/SQLITE_PARITY_953_VIEW_TRIGGER_GENERATED_006.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1622524 | 3947277 | <span style="color:#dc2626">-31.58%</span> |
| 70 | [01100 INDEX_SCHEMA_PRAGMA_033](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1100_INDEX_SCHEMA_PRAGMA_033.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1668601 | 3932538 | <span style="color:#dc2626">-31.08%</span> |
| 71 | [00694 JOIN_SUBQUERY_EXISTS_087](crates/bench/sqlite_parity/cases/SQLITE_PARITY_694_JOIN_SUBQUERY_EXISTS_087.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2011359 | 3930455 | <span style="color:#dc2626">-31.02%</span> |
| 72 | [00293 SCALAR_CAST_TYPEOF_017](crates/bench/sqlite_parity/cases/SQLITE_PARITY_293_SCALAR_CAST_TYPEOF_017.rs) | P1 | memory | GEN_SQL_SCALAR | 1532232 | 3911389 | <span style="color:#dc2626">-30.38%</span> |
| 73 | [00754 CTE_RECURSIVE_MATRIX_047](crates/bench/sqlite_parity/cases/SQLITE_PARITY_754_CTE_RECURSIVE_MATRIX_047.rs) | P1 | memory | GEN_SQL_CTE | 1562780 | 3899476 | <span style="color:#dc2626">-29.98%</span> |
| 74 | [01057 JSON_EXTRACT_SET_050](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1057_JSON_EXTRACT_SET_050.rs) | P2 | memory | GEN_SQL_JSON | 1496725 | 3896070 | <span style="color:#dc2626">-29.87%</span> |
| 75 | [00572 AGG_GROUP_HAVING_065](crates/bench/sqlite_parity/cases/SQLITE_PARITY_572_AGG_GROUP_HAVING_065.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1695351 | 3877034 | <span style="color:#dc2626">-29.23%</span> |
| 76 | [00757 CTE_RECURSIVE_MATRIX_050](crates/bench/sqlite_parity/cases/SQLITE_PARITY_757_CTE_RECURSIVE_MATRIX_050.rs) | P1 | memory | GEN_SQL_CTE | 1530609 | 3873267 | <span style="color:#dc2626">-29.11%</span> |
| 77 | [00140 DOT_EXPERT_OPTIONAL](crates/bench/sqlite_parity/cases/SQLITE_PARITY_140_DOT_EXPERT_OPTIONAL.rs) | P3 | memory | CLI_DOT_COMMAND_OPTIONAL | 2344040 | 3860934 | <span style="color:#dc2626">-28.70%</span> |
| 78 | [00646 JOIN_SUBQUERY_EXISTS_039](crates/bench/sqlite_parity/cases/SQLITE_PARITY_646_JOIN_SUBQUERY_EXISTS_039.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1665495 | 3856866 | <span style="color:#dc2626">-28.56%</span> |
| 79 | [00897 CONSTRAINT_FK_SAVEPOINT_030](crates/bench/sqlite_parity/cases/SQLITE_PARITY_897_CONSTRAINT_FK_SAVEPOINT_030.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1678920 | 3854050 | <span style="color:#dc2626">-28.47%</span> |
| 80 | [00007 WITHOUT_ROWID_TABLE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_007_WITHOUT_ROWID_TABLE.rs) | P0 | memory | SQL_ROWID | 1581616 | 3843761 | <span style="color:#dc2626">-28.13%</span> |
| 81 | [00381 SCALAR_CAST_TYPEOF_039](crates/bench/sqlite_parity/cases/SQLITE_PARITY_381_SCALAR_CAST_TYPEOF_039.rs) | P1 | memory | GEN_SQL_SCALAR | 1641099 | 3842979 | <span style="color:#dc2626">-28.10%</span> |
| 82 | [00155 DOT_DBTOTXT_TEMPFILE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_155_DOT_DBTOTXT_TEMPFILE.rs) | P3 | tempfile | CLI_TEMPFILE | 1797454 | 3831167 | <span style="color:#dc2626">-27.71%</span> |
| 83 | [00471 DML_WHERE_ORDER_LIMIT_084](crates/bench/sqlite_parity/cases/SQLITE_PARITY_471_DML_WHERE_ORDER_LIMIT_084.rs) | P1 | memory | GEN_SQL_DML | 1617484 | 3831127 | <span style="color:#dc2626">-27.70%</span> |
| 84 | [00599 AGG_GROUP_HAVING_092](crates/bench/sqlite_parity/cases/SQLITE_PARITY_599_AGG_GROUP_HAVING_092.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1661818 | 3820547 | <span style="color:#dc2626">-27.35%</span> |
| 85 | [01019 JSON_EXTRACT_SET_012](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1019_JSON_EXTRACT_SET_012.rs) | P2 | memory | GEN_SQL_JSON | 1474313 | 3812772 | <span style="color:#dc2626">-27.09%</span> |
| 86 | [00039 CREATE_TRIGGER_BEFORE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_039_CREATE_TRIGGER_BEFORE.rs) | P0 | memory | SQL_TRIGGER | 1623355 | 3811109 | <span style="color:#dc2626">-27.04%</span> |
| 87 | [01043 JSON_EXTRACT_SET_036](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1043_JSON_EXTRACT_SET_036.rs) | P2 | memory | GEN_SQL_JSON | 1952669 | 3809766 | <span style="color:#dc2626">-26.99%</span> |
| 88 | [00795 WINDOW_PARTITION_SUM_008](crates/bench/sqlite_parity/cases/SQLITE_PARITY_795_WINDOW_PARTITION_SUM_008.rs) | P2 | memory | GEN_SQL_WINDOW | 1889449 | 3808293 | <span style="color:#dc2626">-26.94%</span> |
| 89 | [00478 DML_WHERE_ORDER_LIMIT_091](crates/bench/sqlite_parity/cases/SQLITE_PARITY_478_DML_WHERE_ORDER_LIMIT_091.rs) | P1 | memory | GEN_SQL_DML | 1886964 | 3774109 | <span style="color:#dc2626">-25.80%</span> |
| 90 | [00213 SQL_WAL_CHECKPOINT_TEMPFILE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_213_SQL_WAL_CHECKPOINT_TEMPFILE.rs) | P2 | tempfile | SQL_TEMPFILE | 1775944 | 3773297 | <span style="color:#dc2626">-25.78%</span> |
| 91 | [00401 DML_WHERE_ORDER_LIMIT_014](crates/bench/sqlite_parity/cases/SQLITE_PARITY_401_DML_WHERE_ORDER_LIMIT_014.rs) | P1 | memory | GEN_SQL_DML | 1591685 | 3770071 | <span style="color:#dc2626">-25.67%</span> |
| 92 | [00968 VIEW_TRIGGER_GENERATED_021](crates/bench/sqlite_parity/cases/SQLITE_PARITY_968_VIEW_TRIGGER_GENERATED_021.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2234002 | 3765773 | <span style="color:#dc2626">-25.53%</span> |
| 93 | [00584 AGG_GROUP_HAVING_077](crates/bench/sqlite_parity/cases/SQLITE_PARITY_584_AGG_GROUP_HAVING_077.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1641900 | 3759021 | <span style="color:#dc2626">-25.30%</span> |
| 94 | [00409 DML_WHERE_ORDER_LIMIT_022](crates/bench/sqlite_parity/cases/SQLITE_PARITY_409_DML_WHERE_ORDER_LIMIT_022.rs) | P1 | memory | GEN_SQL_DML | 1642952 | 3758079 | <span style="color:#dc2626">-25.27%</span> |
| 95 | [01044 JSON_EXTRACT_SET_037](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1044_JSON_EXTRACT_SET_037.rs) | P2 | memory | GEN_SQL_JSON | 1766746 | 3753359 | <span style="color:#dc2626">-25.11%</span> |
| 96 | [00955 VIEW_TRIGGER_GENERATED_008](crates/bench/sqlite_parity/cases/SQLITE_PARITY_955_VIEW_TRIGGER_GENERATED_008.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1649344 | 3752368 | <span style="color:#dc2626">-25.08%</span> |
| 97 | [00422 DML_WHERE_ORDER_LIMIT_035](crates/bench/sqlite_parity/cases/SQLITE_PARITY_422_DML_WHERE_ORDER_LIMIT_035.rs) | P1 | memory | GEN_SQL_DML | 1581326 | 3752188 | <span style="color:#dc2626">-25.07%</span> |
| 98 | [00835 WINDOW_PARTITION_SUM_048](crates/bench/sqlite_parity/cases/SQLITE_PARITY_835_WINDOW_PARTITION_SUM_048.rs) | P2 | memory | GEN_SQL_WINDOW | 1610361 | 3749863 | <span style="color:#dc2626">-25.00%</span> |
| 99 | [01078 INDEX_SCHEMA_PRAGMA_011](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1078_INDEX_SCHEMA_PRAGMA_011.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1654033 | 3749042 | <span style="color:#dc2626">-24.97%</span> |
| 100 | [00335 SCALAR_NULL_COALESCE_027](crates/bench/sqlite_parity/cases/SQLITE_PARITY_335_SCALAR_NULL_COALESCE_027.rs) | P1 | memory | GEN_SQL_SCALAR | 1477529 | 3748390 | <span style="color:#dc2626">-24.95%</span> |
| 101 | [00791 WINDOW_PARTITION_SUM_004](crates/bench/sqlite_parity/cases/SQLITE_PARITY_791_WINDOW_PARTITION_SUM_004.rs) | P2 | memory | GEN_SQL_WINDOW | 1598207 | 3742379 | <span style="color:#dc2626">-24.75%</span> |
| 102 | [00530 AGG_GROUP_HAVING_023](crates/bench/sqlite_parity/cases/SQLITE_PARITY_530_AGG_GROUP_HAVING_023.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1692536 | 3738562 | <span style="color:#dc2626">-24.62%</span> |
| 103 | [00043 ATTACH_DETACH_MEMORY](crates/bench/sqlite_parity/cases/SQLITE_PARITY_043_ATTACH_DETACH_MEMORY.rs) | P0 | memory | SQL_ATTACH | 1637442 | 3717903 | <span style="color:#dc2626">-23.93%</span> |
| 104 | [01111 INDEX_SCHEMA_PRAGMA_044](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1111_INDEX_SCHEMA_PRAGMA_044.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1694259 | 3709947 | <span style="color:#dc2626">-23.66%</span> |
| 105 | [01099 INDEX_SCHEMA_PRAGMA_032](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1099_INDEX_SCHEMA_PRAGMA_032.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 2124534 | 3701592 | <span style="color:#dc2626">-23.39%</span> |
| 106 | [00779 CTE_RECURSIVE_MATRIX_072](crates/bench/sqlite_parity/cases/SQLITE_PARITY_779_CTE_RECURSIVE_MATRIX_072.rs) | P1 | memory | GEN_SQL_CTE | 1454546 | 3700750 | <span style="color:#dc2626">-23.36%</span> |
| 107 | [00444 DML_WHERE_ORDER_LIMIT_057](crates/bench/sqlite_parity/cases/SQLITE_PARITY_444_DML_WHERE_ORDER_LIMIT_057.rs) | P1 | memory | GEN_SQL_DML | 1550046 | 3699949 | <span style="color:#dc2626">-23.33%</span> |
| 108 | [00771 CTE_RECURSIVE_MATRIX_064](crates/bench/sqlite_parity/cases/SQLITE_PARITY_771_CTE_RECURSIVE_MATRIX_064.rs) | P1 | memory | GEN_SQL_CTE | 1475775 | 3697234 | <span style="color:#dc2626">-23.24%</span> |
| 109 | [00506 DML_WHERE_ORDER_LIMIT_119](crates/bench/sqlite_parity/cases/SQLITE_PARITY_506_DML_WHERE_ORDER_LIMIT_119.rs) | P1 | memory | GEN_SQL_DML | 1768250 | 3695480 | <span style="color:#dc2626">-23.18%</span> |
| 110 | [00508 AGG_GROUP_HAVING_001](crates/bench/sqlite_parity/cases/SQLITE_PARITY_508_AGG_GROUP_HAVING_001.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1663020 | 3694468 | <span style="color:#dc2626">-23.15%</span> |
| 111 | [00607 AGG_GROUP_HAVING_100](crates/bench/sqlite_parity/cases/SQLITE_PARITY_607_AGG_GROUP_HAVING_100.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1647641 | 3693616 | <span style="color:#dc2626">-23.12%</span> |
| 112 | [00075 EXPLAIN_QUERY_PLAN](crates/bench/sqlite_parity/cases/SQLITE_PARITY_075_EXPLAIN_QUERY_PLAN.rs) | P0 | memory | SQL_EXPLAIN | 1501584 | 3692404 | <span style="color:#dc2626">-23.08%</span> |
| 113 | [01052 JSON_EXTRACT_SET_045](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1052_JSON_EXTRACT_SET_045.rs) | P2 | memory | GEN_SQL_JSON | 2272655 | 3690530 | <span style="color:#dc2626">-23.02%</span> |
| 114 | [00377 SCALAR_CAST_TYPEOF_038](crates/bench/sqlite_parity/cases/SQLITE_PARITY_377_SCALAR_CAST_TYPEOF_038.rs) | P1 | memory | GEN_SQL_SCALAR | 1391897 | 3690230 | <span style="color:#dc2626">-23.01%</span> |
| 115 | [00242 SCALAR_STRING_004](crates/bench/sqlite_parity/cases/SQLITE_PARITY_242_SCALAR_STRING_004.rs) | P1 | memory | GEN_SQL_SCALAR | 1405633 | 3687555 | <span style="color:#dc2626">-22.92%</span> |
| 116 | [00332 SCALAR_ARITH_027](crates/bench/sqlite_parity/cases/SQLITE_PARITY_332_SCALAR_ARITH_027.rs) | P1 | memory | GEN_SQL_SCALAR | 1591314 | 3684960 | <span style="color:#dc2626">-22.83%</span> |
| 117 | [00896 CONSTRAINT_FK_SAVEPOINT_029](crates/bench/sqlite_parity/cases/SQLITE_PARITY_896_CONSTRAINT_FK_SAVEPOINT_029.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2216258 | 3683918 | <span style="color:#dc2626">-22.80%</span> |
| 118 | [00952 VIEW_TRIGGER_GENERATED_005](crates/bench/sqlite_parity/cases/SQLITE_PARITY_952_VIEW_TRIGGER_GENERATED_005.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1649314 | 3675371 | <span style="color:#dc2626">-22.51%</span> |
| 119 | [01054 JSON_EXTRACT_SET_047](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1054_JSON_EXTRACT_SET_047.rs) | P2 | memory | GEN_SQL_JSON | 2128913 | 3673989 | <span style="color:#dc2626">-22.47%</span> |
| 120 | [00799 WINDOW_PARTITION_SUM_012](crates/bench/sqlite_parity/cases/SQLITE_PARITY_799_WINDOW_PARTITION_SUM_012.rs) | P2 | memory | GEN_SQL_WINDOW | 1554244 | 3671685 | <span style="color:#dc2626">-22.39%</span> |
| 121 | [00788 WINDOW_PARTITION_SUM_001](crates/bench/sqlite_parity/cases/SQLITE_PARITY_788_WINDOW_PARTITION_SUM_001.rs) | P2 | memory | GEN_SQL_WINDOW | 1539927 | 3666244 | <span style="color:#dc2626">-22.21%</span> |
| 122 | [01127 INDEX_SCHEMA_PRAGMA_060](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1127_INDEX_SCHEMA_PRAGMA_060.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1687857 | 3663690 | <span style="color:#dc2626">-22.12%</span> |
| 123 | [00154 DOT_DBINFO_TEMPFILE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_154_DOT_DBINFO_TEMPFILE.rs) | P3 | tempfile | CLI_TEMPFILE_DIAGNOSTIC | 1745667 | 3656937 | <span style="color:#dc2626">-21.90%</span> |
| 124 | [00977 VIEW_TRIGGER_GENERATED_030](crates/bench/sqlite_parity/cases/SQLITE_PARITY_977_VIEW_TRIGGER_GENERATED_030.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1988156 | 3655794 | <span style="color:#dc2626">-21.86%</span> |
| 125 | [00727 CTE_RECURSIVE_MATRIX_020](crates/bench/sqlite_parity/cases/SQLITE_PARITY_727_CTE_RECURSIVE_MATRIX_020.rs) | P1 | memory | GEN_SQL_CTE | 1574934 | 3654442 | <span style="color:#dc2626">-21.81%</span> |
| 126 | [00748 CTE_RECURSIVE_MATRIX_041](crates/bench/sqlite_parity/cases/SQLITE_PARITY_748_CTE_RECURSIVE_MATRIX_041.rs) | P1 | memory | GEN_SQL_CTE | 1524447 | 3649754 | <span style="color:#dc2626">-21.66%</span> |
| 127 | [00905 CONSTRAINT_FK_SAVEPOINT_038](crates/bench/sqlite_parity/cases/SQLITE_PARITY_905_CONSTRAINT_FK_SAVEPOINT_038.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1840736 | 3643922 | <span style="color:#dc2626">-21.46%</span> |
| 128 | [00773 CTE_RECURSIVE_MATRIX_066](crates/bench/sqlite_parity/cases/SQLITE_PARITY_773_CTE_RECURSIVE_MATRIX_066.rs) | P1 | memory | GEN_SQL_CTE | 1516893 | 3641477 | <span style="color:#dc2626">-21.38%</span> |
| 129 | [00804 WINDOW_PARTITION_SUM_017](crates/bench/sqlite_parity/cases/SQLITE_PARITY_804_WINDOW_PARTITION_SUM_017.rs) | P2 | memory | GEN_SQL_WINDOW | 1574532 | 3641397 | <span style="color:#dc2626">-21.38%</span> |
| 130 | [00759 CTE_RECURSIVE_MATRIX_052](crates/bench/sqlite_parity/cases/SQLITE_PARITY_759_CTE_RECURSIVE_MATRIX_052.rs) | P1 | memory | GEN_SQL_CTE | 1596233 | 3641267 | <span style="color:#dc2626">-21.38%</span> |
| 131 | [00618 JOIN_SUBQUERY_EXISTS_011](crates/bench/sqlite_parity/cases/SQLITE_PARITY_618_JOIN_SUBQUERY_EXISTS_011.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1792425 | 3640906 | <span style="color:#dc2626">-21.36%</span> |
| 132 | [00919 CONSTRAINT_FK_SAVEPOINT_052](crates/bench/sqlite_parity/cases/SQLITE_PARITY_919_CONSTRAINT_FK_SAVEPOINT_052.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1700341 | 3637330 | <span style="color:#dc2626">-21.24%</span> |
| 133 | [00768 CTE_RECURSIVE_MATRIX_061](crates/bench/sqlite_parity/cases/SQLITE_PARITY_768_CTE_RECURSIVE_MATRIX_061.rs) | P1 | memory | GEN_SQL_CTE | 1532954 | 3624205 | <span style="color:#dc2626">-20.81%</span> |
| 134 | [00427 DML_WHERE_ORDER_LIMIT_040](crates/bench/sqlite_parity/cases/SQLITE_PARITY_427_DML_WHERE_ORDER_LIMIT_040.rs) | P1 | memory | GEN_SQL_DML | 1533856 | 3619546 | <span style="color:#dc2626">-20.65%</span> |
| 135 | [00392 DML_WHERE_ORDER_LIMIT_005](crates/bench/sqlite_parity/cases/SQLITE_PARITY_392_DML_WHERE_ORDER_LIMIT_005.rs) | P1 | memory | GEN_SQL_DML | 1637813 | 3615699 | <span style="color:#dc2626">-20.52%</span> |
| 136 | [01071 INDEX_SCHEMA_PRAGMA_004](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1071_INDEX_SCHEMA_PRAGMA_004.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1689250 | 3611571 | <span style="color:#dc2626">-20.39%</span> |
| 137 | [00971 VIEW_TRIGGER_GENERATED_024](crates/bench/sqlite_parity/cases/SQLITE_PARITY_971_VIEW_TRIGGER_GENERATED_024.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1944674 | 3610779 | <span style="color:#dc2626">-20.36%</span> |
| 138 | [00659 JOIN_SUBQUERY_EXISTS_052](crates/bench/sqlite_parity/cases/SQLITE_PARITY_659_JOIN_SUBQUERY_EXISTS_052.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1837230 | 3609507 | <span style="color:#dc2626">-20.32%</span> |
| 139 | [00413 DML_WHERE_ORDER_LIMIT_026](crates/bench/sqlite_parity/cases/SQLITE_PARITY_413_DML_WHERE_ORDER_LIMIT_026.rs) | P1 | memory | GEN_SQL_DML | 1731329 | 3603375 | <span style="color:#dc2626">-20.11%</span> |
| 140 | [01076 INDEX_SCHEMA_PRAGMA_009](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1076_INDEX_SCHEMA_PRAGMA_009.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 2803821 | 3594018 | <span style="color:#dc2626">-19.80%</span> |
| 141 | [00282 SCALAR_STRING_014](crates/bench/sqlite_parity/cases/SQLITE_PARITY_282_SCALAR_STRING_014.rs) | P1 | memory | GEN_SQL_SCALAR | 1503969 | 3587676 | <span style="color:#dc2626">-19.59%</span> |
| 142 | [00417 DML_WHERE_ORDER_LIMIT_030](crates/bench/sqlite_parity/cases/SQLITE_PARITY_417_DML_WHERE_ORDER_LIMIT_030.rs) | P1 | memory | GEN_SQL_DML | 1882666 | 3587466 | <span style="color:#dc2626">-19.58%</span> |
| 143 | [00783 CTE_RECURSIVE_MATRIX_076](crates/bench/sqlite_parity/cases/SQLITE_PARITY_783_CTE_RECURSIVE_MATRIX_076.rs) | P1 | memory | GEN_SQL_CTE | 1527153 | 3582095 | <span style="color:#dc2626">-19.40%</span> |
| 144 | [00880 CONSTRAINT_FK_SAVEPOINT_013](crates/bench/sqlite_parity/cases/SQLITE_PARITY_880_CONSTRAINT_FK_SAVEPOINT_013.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2038581 | 3580683 | <span style="color:#dc2626">-19.36%</span> |
| 145 | [00634 JOIN_SUBQUERY_EXISTS_027](crates/bench/sqlite_parity/cases/SQLITE_PARITY_634_JOIN_SUBQUERY_EXISTS_027.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1722573 | 3579721 | <span style="color:#dc2626">-19.32%</span> |
| 146 | [00640 JOIN_SUBQUERY_EXISTS_033](crates/bench/sqlite_parity/cases/SQLITE_PARITY_640_JOIN_SUBQUERY_EXISTS_033.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1619769 | 3576354 | <span style="color:#dc2626">-19.21%</span> |
| 147 | [00464 DML_WHERE_ORDER_LIMIT_077](crates/bench/sqlite_parity/cases/SQLITE_PARITY_464_DML_WHERE_ORDER_LIMIT_077.rs) | P1 | memory | GEN_SQL_DML | 1588298 | 3573699 | <span style="color:#dc2626">-19.12%</span> |
| 148 | [00656 JOIN_SUBQUERY_EXISTS_049](crates/bench/sqlite_parity/cases/SQLITE_PARITY_656_JOIN_SUBQUERY_EXISTS_049.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1643263 | 3572107 | <span style="color:#dc2626">-19.07%</span> |
| 149 | [00261 SCALAR_CAST_TYPEOF_009](crates/bench/sqlite_parity/cases/SQLITE_PARITY_261_SCALAR_CAST_TYPEOF_009.rs) | P1 | memory | GEN_SQL_SCALAR | 1461378 | 3570423 | <span style="color:#dc2626">-19.01%</span> |
| 150 | [00760 CTE_RECURSIVE_MATRIX_053](crates/bench/sqlite_parity/cases/SQLITE_PARITY_760_CTE_RECURSIVE_MATRIX_053.rs) | P1 | memory | GEN_SQL_CTE | 1609539 | 3569522 | <span style="color:#dc2626">-18.98%</span> |
| 151 | [00899 CONSTRAINT_FK_SAVEPOINT_032](crates/bench/sqlite_parity/cases/SQLITE_PARITY_899_CONSTRAINT_FK_SAVEPOINT_032.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1672989 | 3567928 | <span style="color:#dc2626">-18.93%</span> |
| 152 | [00699 JOIN_SUBQUERY_EXISTS_092](crates/bench/sqlite_parity/cases/SQLITE_PARITY_699_JOIN_SUBQUERY_EXISTS_092.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1713295 | 3565023 | <span style="color:#dc2626">-18.83%</span> |
| 153 | [00660 JOIN_SUBQUERY_EXISTS_053](crates/bench/sqlite_parity/cases/SQLITE_PARITY_660_JOIN_SUBQUERY_EXISTS_053.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1606483 | 3557899 | <span style="color:#dc2626">-18.60%</span> |
| 154 | [01098 INDEX_SCHEMA_PRAGMA_031](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1098_INDEX_SCHEMA_PRAGMA_031.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1740567 | 3555024 | <span style="color:#dc2626">-18.50%</span> |
| 155 | [00865 WINDOW_PARTITION_SUM_078](crates/bench/sqlite_parity/cases/SQLITE_PARITY_865_WINDOW_PARTITION_SUM_078.rs) | P2 | memory | GEN_SQL_WINDOW | 1558672 | 3553581 | <span style="color:#dc2626">-18.45%</span> |
| 156 | [00327 SCALAR_NULL_COALESCE_025](crates/bench/sqlite_parity/cases/SQLITE_PARITY_327_SCALAR_NULL_COALESCE_025.rs) | P1 | memory | GEN_SQL_SCALAR | 1596233 | 3551979 | <span style="color:#dc2626">-18.40%</span> |
| 157 | [01074 INDEX_SCHEMA_PRAGMA_007](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1074_INDEX_SCHEMA_PRAGMA_007.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 2061264 | 3549634 | <span style="color:#dc2626">-18.32%</span> |
| 158 | [00663 JOIN_SUBQUERY_EXISTS_056](crates/bench/sqlite_parity/cases/SQLITE_PARITY_663_JOIN_SUBQUERY_EXISTS_056.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1671626 | 3547881 | <span style="color:#dc2626">-18.26%</span> |
| 159 | [00956 VIEW_TRIGGER_GENERATED_009](crates/bench/sqlite_parity/cases/SQLITE_PARITY_956_VIEW_TRIGGER_GENERATED_009.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1641981 | 3546668 | <span style="color:#dc2626">-18.22%</span> |
| 160 | [01090 INDEX_SCHEMA_PRAGMA_023](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1090_INDEX_SCHEMA_PRAGMA_023.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1987164 | 3545877 | <span style="color:#dc2626">-18.20%</span> |
| 161 | [00246 SCALAR_STRING_005](crates/bench/sqlite_parity/cases/SQLITE_PARITY_246_SCALAR_STRING_005.rs) | P1 | memory | GEN_SQL_SCALAR | 1420881 | 3537821 | <span style="color:#dc2626">-17.93%</span> |
| 162 | [00486 DML_WHERE_ORDER_LIMIT_099](crates/bench/sqlite_parity/cases/SQLITE_PARITY_486_DML_WHERE_ORDER_LIMIT_099.rs) | P1 | memory | GEN_SQL_DML | 1553011 | 3533934 | <span style="color:#dc2626">-17.80%</span> |
| 163 | [01120 INDEX_SCHEMA_PRAGMA_053](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1120_INDEX_SCHEMA_PRAGMA_053.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1876143 | 3533313 | <span style="color:#dc2626">-17.78%</span> |
| 164 | [00681 JOIN_SUBQUERY_EXISTS_074](crates/bench/sqlite_parity/cases/SQLITE_PARITY_681_JOIN_SUBQUERY_EXISTS_074.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1743973 | 3532982 | <span style="color:#dc2626">-17.77%</span> |
| 165 | [00617 JOIN_SUBQUERY_EXISTS_010](crates/bench/sqlite_parity/cases/SQLITE_PARITY_617_JOIN_SUBQUERY_EXISTS_010.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1610451 | 3532411 | <span style="color:#dc2626">-17.75%</span> |
| 166 | [00519 AGG_GROUP_HAVING_012](crates/bench/sqlite_parity/cases/SQLITE_PARITY_519_AGG_GROUP_HAVING_012.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1717784 | 3527502 | <span style="color:#dc2626">-17.58%</span> |
| 167 | [00753 CTE_RECURSIVE_MATRIX_046](crates/bench/sqlite_parity/cases/SQLITE_PARITY_753_CTE_RECURSIVE_MATRIX_046.rs) | P1 | memory | GEN_SQL_CTE | 1827141 | 3526851 | <span style="color:#dc2626">-17.56%</span> |
| 168 | [01042 JSON_EXTRACT_SET_035](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1042_JSON_EXTRACT_SET_035.rs) | P2 | memory | GEN_SQL_JSON | 1892895 | 3526119 | <span style="color:#dc2626">-17.54%</span> |
| 169 | [00684 JOIN_SUBQUERY_EXISTS_077](crates/bench/sqlite_parity/cases/SQLITE_PARITY_684_JOIN_SUBQUERY_EXISTS_077.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1728154 | 3522413 | <span style="color:#dc2626">-17.41%</span> |
| 170 | [00608 JOIN_SUBQUERY_EXISTS_001](crates/bench/sqlite_parity/cases/SQLITE_PARITY_608_JOIN_SUBQUERY_EXISTS_001.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1734115 | 3520219 | <span style="color:#dc2626">-17.34%</span> |
| 171 | [00665 JOIN_SUBQUERY_EXISTS_058](crates/bench/sqlite_parity/cases/SQLITE_PARITY_665_JOIN_SUBQUERY_EXISTS_058.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1642071 | 3519797 | <span style="color:#dc2626">-17.33%</span> |
| 172 | [00680 JOIN_SUBQUERY_EXISTS_073](crates/bench/sqlite_parity/cases/SQLITE_PARITY_680_JOIN_SUBQUERY_EXISTS_073.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1650847 | 3518214 | <span style="color:#dc2626">-17.27%</span> |
| 173 | [00698 JOIN_SUBQUERY_EXISTS_091](crates/bench/sqlite_parity/cases/SQLITE_PARITY_698_JOIN_SUBQUERY_EXISTS_091.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2624832 | 3517162 | <span style="color:#dc2626">-17.24%</span> |
| 174 | [00787 CTE_RECURSIVE_MATRIX_080](crates/bench/sqlite_parity/cases/SQLITE_PARITY_787_CTE_RECURSIVE_MATRIX_080.rs) | P1 | memory | GEN_SQL_CTE | 1535629 | 3516621 | <span style="color:#dc2626">-17.22%</span> |
| 175 | [00641 JOIN_SUBQUERY_EXISTS_034](crates/bench/sqlite_parity/cases/SQLITE_PARITY_641_JOIN_SUBQUERY_EXISTS_034.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1607175 | 3515379 | <span style="color:#dc2626">-17.18%</span> |
| 176 | [01082 INDEX_SCHEMA_PRAGMA_015](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1082_INDEX_SCHEMA_PRAGMA_015.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1632522 | 3514888 | <span style="color:#dc2626">-17.16%</span> |
| 177 | [00633 JOIN_SUBQUERY_EXISTS_026](crates/bench/sqlite_parity/cases/SQLITE_PARITY_633_JOIN_SUBQUERY_EXISTS_026.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1912042 | 3514176 | <span style="color:#dc2626">-17.14%</span> |
| 178 | [00602 AGG_GROUP_HAVING_095](crates/bench/sqlite_parity/cases/SQLITE_PARITY_602_AGG_GROUP_HAVING_095.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1833513 | 3513716 | <span style="color:#dc2626">-17.12%</span> |
| 179 | [00648 JOIN_SUBQUERY_EXISTS_041](crates/bench/sqlite_parity/cases/SQLITE_PARITY_648_JOIN_SUBQUERY_EXISTS_041.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1681325 | 3513656 | <span style="color:#dc2626">-17.12%</span> |
| 180 | [00615 JOIN_SUBQUERY_EXISTS_008](crates/bench/sqlite_parity/cases/SQLITE_PARITY_615_JOIN_SUBQUERY_EXISTS_008.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1697164 | 3511071 | <span style="color:#dc2626">-17.04%</span> |
| 181 | [00612 JOIN_SUBQUERY_EXISTS_005](crates/bench/sqlite_parity/cases/SQLITE_PARITY_612_JOIN_SUBQUERY_EXISTS_005.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1677327 | 3510059 | <span style="color:#dc2626">-17.00%</span> |
| 182 | [00624 JOIN_SUBQUERY_EXISTS_017](crates/bench/sqlite_parity/cases/SQLITE_PARITY_624_JOIN_SUBQUERY_EXISTS_017.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1714949 | 3505820 | <span style="color:#dc2626">-16.86%</span> |
| 183 | [00705 JOIN_SUBQUERY_EXISTS_098](crates/bench/sqlite_parity/cases/SQLITE_PARITY_705_JOIN_SUBQUERY_EXISTS_098.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1693227 | 3505490 | <span style="color:#dc2626">-16.85%</span> |
| 184 | [00623 JOIN_SUBQUERY_EXISTS_016](crates/bench/sqlite_parity/cases/SQLITE_PARITY_623_JOIN_SUBQUERY_EXISTS_016.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1600461 | 3504568 | <span style="color:#dc2626">-16.82%</span> |
| 185 | [00393 DML_WHERE_ORDER_LIMIT_006](crates/bench/sqlite_parity/cases/SQLITE_PARITY_393_DML_WHERE_ORDER_LIMIT_006.rs) | P1 | memory | GEN_SQL_DML | 1566287 | 3504027 | <span style="color:#dc2626">-16.80%</span> |
| 186 | [00676 JOIN_SUBQUERY_EXISTS_069](crates/bench/sqlite_parity/cases/SQLITE_PARITY_676_JOIN_SUBQUERY_EXISTS_069.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1712935 | 3499919 | <span style="color:#dc2626">-16.66%</span> |
| 187 | [00277 SCALAR_CAST_TYPEOF_013](crates/bench/sqlite_parity/cases/SQLITE_PARITY_277_SCALAR_CAST_TYPEOF_013.rs) | P1 | memory | GEN_SQL_SCALAR | 1717313 | 3497795 | <span style="color:#dc2626">-16.59%</span> |
| 188 | [00666 JOIN_SUBQUERY_EXISTS_059](crates/bench/sqlite_parity/cases/SQLITE_PARITY_666_JOIN_SUBQUERY_EXISTS_059.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1770754 | 3494770 | <span style="color:#dc2626">-16.49%</span> |
| 189 | [00683 JOIN_SUBQUERY_EXISTS_076](crates/bench/sqlite_parity/cases/SQLITE_PARITY_683_JOIN_SUBQUERY_EXISTS_076.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1691274 | 3493607 | <span style="color:#dc2626">-16.45%</span> |
| 190 | [00682 JOIN_SUBQUERY_EXISTS_075](crates/bench/sqlite_parity/cases/SQLITE_PARITY_682_JOIN_SUBQUERY_EXISTS_075.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1724988 | 3493297 | <span style="color:#dc2626">-16.44%</span> |
| 191 | [01068 INDEX_SCHEMA_PRAGMA_001](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1068_INDEX_SCHEMA_PRAGMA_001.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1853240 | 3493207 | <span style="color:#dc2626">-16.44%</span> |
| 192 | [00697 JOIN_SUBQUERY_EXISTS_090](crates/bench/sqlite_parity/cases/SQLITE_PARITY_697_JOIN_SUBQUERY_EXISTS_090.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1684350 | 3490431 | <span style="color:#dc2626">-16.35%</span> |
| 193 | [00395 DML_WHERE_ORDER_LIMIT_008](crates/bench/sqlite_parity/cases/SQLITE_PARITY_395_DML_WHERE_ORDER_LIMIT_008.rs) | P1 | memory | GEN_SQL_DML | 1990059 | 3490262 | <span style="color:#dc2626">-16.34%</span> |
| 194 | [01004 VIEW_TRIGGER_GENERATED_057](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1004_VIEW_TRIGGER_GENERATED_057.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1712033 | 3487476 | <span style="color:#dc2626">-16.25%</span> |
| 195 | [00399 DML_WHERE_ORDER_LIMIT_012](crates/bench/sqlite_parity/cases/SQLITE_PARITY_399_DML_WHERE_ORDER_LIMIT_012.rs) | P1 | memory | GEN_SQL_DML | 1584722 | 3484250 | <span style="color:#dc2626">-16.14%</span> |
| 196 | [00614 JOIN_SUBQUERY_EXISTS_007](crates/bench/sqlite_parity/cases/SQLITE_PARITY_614_JOIN_SUBQUERY_EXISTS_007.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1738202 | 3483929 | <span style="color:#dc2626">-16.13%</span> |
| 197 | [00996 VIEW_TRIGGER_GENERATED_049](crates/bench/sqlite_parity/cases/SQLITE_PARITY_996_VIEW_TRIGGER_GENERATED_049.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1675093 | 3483469 | <span style="color:#dc2626">-16.12%</span> |
| 198 | [01112 INDEX_SCHEMA_PRAGMA_045](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1112_INDEX_SCHEMA_PRAGMA_045.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 2002373 | 3482155 | <span style="color:#dc2626">-16.07%</span> |
| 199 | [00620 JOIN_SUBQUERY_EXISTS_013](crates/bench/sqlite_parity/cases/SQLITE_PARITY_620_JOIN_SUBQUERY_EXISTS_013.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1689621 | 3481034 | <span style="color:#dc2626">-16.03%</span> |
| 200 | [00642 JOIN_SUBQUERY_EXISTS_035](crates/bench/sqlite_parity/cases/SQLITE_PARITY_642_JOIN_SUBQUERY_EXISTS_035.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1810980 | 3478890 | <span style="color:#dc2626">-15.96%</span> |
| 201 | [00610 JOIN_SUBQUERY_EXISTS_003](crates/bench/sqlite_parity/cases/SQLITE_PARITY_610_JOIN_SUBQUERY_EXISTS_003.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1635579 | 3478779 | <span style="color:#dc2626">-15.96%</span> |
| 202 | [00679 JOIN_SUBQUERY_EXISTS_072](crates/bench/sqlite_parity/cases/SQLITE_PARITY_679_JOIN_SUBQUERY_EXISTS_072.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1654794 | 3477798 | <span style="color:#dc2626">-15.93%</span> |
| 203 | [00475 DML_WHERE_ORDER_LIMIT_088](crates/bench/sqlite_parity/cases/SQLITE_PARITY_475_DML_WHERE_ORDER_LIMIT_088.rs) | P1 | memory | GEN_SQL_DML | 1672638 | 3477728 | <span style="color:#dc2626">-15.92%</span> |
| 204 | [00690 JOIN_SUBQUERY_EXISTS_083](crates/bench/sqlite_parity/cases/SQLITE_PARITY_690_JOIN_SUBQUERY_EXISTS_083.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1602035 | 3477467 | <span style="color:#dc2626">-15.92%</span> |
| 205 | [00628 JOIN_SUBQUERY_EXISTS_021](crates/bench/sqlite_parity/cases/SQLITE_PARITY_628_JOIN_SUBQUERY_EXISTS_021.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1685433 | 3477197 | <span style="color:#dc2626">-15.91%</span> |
| 206 | [00520 AGG_GROUP_HAVING_013](crates/bench/sqlite_parity/cases/SQLITE_PARITY_520_AGG_GROUP_HAVING_013.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1680614 | 3477117 | <span style="color:#dc2626">-15.90%</span> |
| 207 | [00658 JOIN_SUBQUERY_EXISTS_051](crates/bench/sqlite_parity/cases/SQLITE_PARITY_658_JOIN_SUBQUERY_EXISTS_051.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1636249 | 3476665 | <span style="color:#dc2626">-15.89%</span> |
| 208 | [00503 DML_WHERE_ORDER_LIMIT_116](crates/bench/sqlite_parity/cases/SQLITE_PARITY_503_DML_WHERE_ORDER_LIMIT_116.rs) | P1 | memory | GEN_SQL_DML | 1580163 | 3475514 | <span style="color:#dc2626">-15.85%</span> |
| 209 | [00661 JOIN_SUBQUERY_EXISTS_054](crates/bench/sqlite_parity/cases/SQLITE_PARITY_661_JOIN_SUBQUERY_EXISTS_054.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2289387 | 3472808 | <span style="color:#dc2626">-15.76%</span> |
| 210 | [00398 DML_WHERE_ORDER_LIMIT_011](crates/bench/sqlite_parity/cases/SQLITE_PARITY_398_DML_WHERE_ORDER_LIMIT_011.rs) | P1 | memory | GEN_SQL_DML | 1693118 | 3469622 | <span style="color:#dc2626">-15.65%</span> |
| 211 | [00652 JOIN_SUBQUERY_EXISTS_045](crates/bench/sqlite_parity/cases/SQLITE_PARITY_652_JOIN_SUBQUERY_EXISTS_045.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1800761 | 3468420 | <span style="color:#dc2626">-15.61%</span> |
| 212 | [00982 VIEW_TRIGGER_GENERATED_035](crates/bench/sqlite_parity/cases/SQLITE_PARITY_982_VIEW_TRIGGER_GENERATED_035.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1774491 | 3465684 | <span style="color:#dc2626">-15.52%</span> |
| 213 | [00609 JOIN_SUBQUERY_EXISTS_002](crates/bench/sqlite_parity/cases/SQLITE_PARITY_609_JOIN_SUBQUERY_EXISTS_002.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1710881 | 3464071 | <span style="color:#dc2626">-15.47%</span> |
| 214 | [00662 JOIN_SUBQUERY_EXISTS_055](crates/bench/sqlite_parity/cases/SQLITE_PARITY_662_JOIN_SUBQUERY_EXISTS_055.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1660966 | 3461837 | <span style="color:#dc2626">-15.39%</span> |
| 215 | [01105 INDEX_SCHEMA_PRAGMA_038](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1105_INDEX_SCHEMA_PRAGMA_038.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1667709 | 3461657 | <span style="color:#dc2626">-15.39%</span> |
| 216 | [00678 JOIN_SUBQUERY_EXISTS_071](crates/bench/sqlite_parity/cases/SQLITE_PARITY_678_JOIN_SUBQUERY_EXISTS_071.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1666967 | 3460646 | <span style="color:#dc2626">-15.35%</span> |
| 217 | [00104 SELECT_DISTINCT](crates/bench/sqlite_parity/cases/SQLITE_PARITY_104_SELECT_DISTINCT.rs) | P0 | memory | SQL_SELECT | 1443164 | 3460625 | <span style="color:#dc2626">-15.35%</span> |
| 218 | [00972 VIEW_TRIGGER_GENERATED_025](crates/bench/sqlite_parity/cases/SQLITE_PARITY_972_VIEW_TRIGGER_GENERATED_025.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1794709 | 3460334 | <span style="color:#dc2626">-15.34%</span> |
| 219 | [00664 JOIN_SUBQUERY_EXISTS_057](crates/bench/sqlite_parity/cases/SQLITE_PARITY_664_JOIN_SUBQUERY_EXISTS_057.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1700772 | 3460104 | <span style="color:#dc2626">-15.34%</span> |
| 220 | [01073 INDEX_SCHEMA_PRAGMA_006](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1073_INDEX_SCHEMA_PRAGMA_006.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1823314 | 3458621 | <span style="color:#dc2626">-15.29%</span> |
| 221 | [00010 GENERATED_COLUMNS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_010_GENERATED_COLUMNS.rs) | P0 | memory | SQL_DDL | 1552611 | 3458000 | <span style="color:#dc2626">-15.27%</span> |
| 222 | [00611 JOIN_SUBQUERY_EXISTS_004](crates/bench/sqlite_parity/cases/SQLITE_PARITY_611_JOIN_SUBQUERY_EXISTS_004.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2204245 | 3457479 | <span style="color:#dc2626">-15.25%</span> |
| 223 | [00627 JOIN_SUBQUERY_EXISTS_020](crates/bench/sqlite_parity/cases/SQLITE_PARITY_627_JOIN_SUBQUERY_EXISTS_020.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1671967 | 3457108 | <span style="color:#dc2626">-15.24%</span> |
| 224 | [00632 JOIN_SUBQUERY_EXISTS_025](crates/bench/sqlite_parity/cases/SQLITE_PARITY_632_JOIN_SUBQUERY_EXISTS_025.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1768039 | 3455756 | <span style="color:#dc2626">-15.19%</span> |
| 225 | [01102 INDEX_SCHEMA_PRAGMA_035](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1102_INDEX_SCHEMA_PRAGMA_035.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1722463 | 3455315 | <span style="color:#dc2626">-15.18%</span> |
| 226 | [01024 JSON_EXTRACT_SET_017](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1024_JSON_EXTRACT_SET_017.rs) | P2 | memory | GEN_SQL_JSON | 1490023 | 3454744 | <span style="color:#dc2626">-15.16%</span> |
| 227 | [00635 JOIN_SUBQUERY_EXISTS_028](crates/bench/sqlite_parity/cases/SQLITE_PARITY_635_JOIN_SUBQUERY_EXISTS_028.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1718205 | 3454493 | <span style="color:#dc2626">-15.15%</span> |
| 228 | [00650 JOIN_SUBQUERY_EXISTS_043](crates/bench/sqlite_parity/cases/SQLITE_PARITY_650_JOIN_SUBQUERY_EXISTS_043.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1622884 | 3452420 | <span style="color:#dc2626">-15.08%</span> |
| 229 | [01125 INDEX_SCHEMA_PRAGMA_058](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1125_INDEX_SCHEMA_PRAGMA_058.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1706412 | 3451277 | <span style="color:#dc2626">-15.04%</span> |
| 230 | [01021 JSON_EXTRACT_SET_014](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1021_JSON_EXTRACT_SET_014.rs) | P2 | memory | GEN_SQL_JSON | 1599189 | 3450446 | <span style="color:#dc2626">-15.01%</span> |
| 231 | [00029 ALTER_TABLE_RENAME_COLUMN](crates/bench/sqlite_parity/cases/SQLITE_PARITY_029_ALTER_TABLE_RENAME_COLUMN.rs) | P0 | memory | SQL_ALTER | 2064370 | 3450366 | <span style="color:#dc2626">-15.01%</span> |
| 232 | [00696 JOIN_SUBQUERY_EXISTS_089](crates/bench/sqlite_parity/cases/SQLITE_PARITY_696_JOIN_SUBQUERY_EXISTS_089.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1988186 | 3449384 | <span style="color:#dc2626">-14.98%</span> |
| 233 | [00637 JOIN_SUBQUERY_EXISTS_030](crates/bench/sqlite_parity/cases/SQLITE_PARITY_637_JOIN_SUBQUERY_EXISTS_030.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1592647 | 3449274 | <span style="color:#dc2626">-14.98%</span> |
| 234 | [00649 JOIN_SUBQUERY_EXISTS_042](crates/bench/sqlite_parity/cases/SQLITE_PARITY_649_JOIN_SUBQUERY_EXISTS_042.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1977316 | 3448241 | <span style="color:#dc2626">-14.94%</span> |
| 235 | [00677 JOIN_SUBQUERY_EXISTS_070](crates/bench/sqlite_parity/cases/SQLITE_PARITY_677_JOIN_SUBQUERY_EXISTS_070.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1615179 | 3446879 | <span style="color:#dc2626">-14.90%</span> |
| 236 | [00670 JOIN_SUBQUERY_EXISTS_063](crates/bench/sqlite_parity/cases/SQLITE_PARITY_670_JOIN_SUBQUERY_EXISTS_063.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1716872 | 3443803 | <span style="color:#dc2626">-14.79%</span> |
| 237 | [00644 JOIN_SUBQUERY_EXISTS_037](crates/bench/sqlite_parity/cases/SQLITE_PARITY_644_JOIN_SUBQUERY_EXISTS_037.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1682236 | 3443082 | <span style="color:#dc2626">-14.77%</span> |
| 238 | [01116 INDEX_SCHEMA_PRAGMA_049](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1116_INDEX_SCHEMA_PRAGMA_049.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1644425 | 3442932 | <span style="color:#dc2626">-14.76%</span> |
| 239 | [00782 CTE_RECURSIVE_MATRIX_075](crates/bench/sqlite_parity/cases/SQLITE_PARITY_782_CTE_RECURSIVE_MATRIX_075.rs) | P1 | memory | GEN_SQL_CTE | 1521452 | 3442191 | <span style="color:#dc2626">-14.74%</span> |
| 240 | [00957 VIEW_TRIGGER_GENERATED_010](crates/bench/sqlite_parity/cases/SQLITE_PARITY_957_VIEW_TRIGGER_GENERATED_010.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2451253 | 3441769 | <span style="color:#dc2626">-14.73%</span> |
| 241 | [01072 INDEX_SCHEMA_PRAGMA_005](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1072_INDEX_SCHEMA_PRAGMA_005.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 2320927 | 3441670 | <span style="color:#dc2626">-14.72%</span> |
| 242 | [00433 DML_WHERE_ORDER_LIMIT_046](crates/bench/sqlite_parity/cases/SQLITE_PARITY_433_DML_WHERE_ORDER_LIMIT_046.rs) | P1 | memory | GEN_SQL_DML | 1561027 | 3441019 | <span style="color:#dc2626">-14.70%</span> |
| 243 | [00668 JOIN_SUBQUERY_EXISTS_061](crates/bench/sqlite_parity/cases/SQLITE_PARITY_668_JOIN_SUBQUERY_EXISTS_061.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1764202 | 3438583 | <span style="color:#dc2626">-14.62%</span> |
| 244 | [00419 DML_WHERE_ORDER_LIMIT_032](crates/bench/sqlite_parity/cases/SQLITE_PARITY_419_DML_WHERE_ORDER_LIMIT_032.rs) | P1 | memory | GEN_SQL_DML | 1615700 | 3437221 | <span style="color:#dc2626">-14.57%</span> |
| 245 | [00693 JOIN_SUBQUERY_EXISTS_086](crates/bench/sqlite_parity/cases/SQLITE_PARITY_693_JOIN_SUBQUERY_EXISTS_086.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1662409 | 3435307 | <span style="color:#dc2626">-14.51%</span> |
| 246 | [01108 INDEX_SCHEMA_PRAGMA_041](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1108_INDEX_SCHEMA_PRAGMA_041.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1662479 | 3434526 | <span style="color:#dc2626">-14.48%</span> |
| 247 | [00655 JOIN_SUBQUERY_EXISTS_048](crates/bench/sqlite_parity/cases/SQLITE_PARITY_655_JOIN_SUBQUERY_EXISTS_048.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1753201 | 3432712 | <span style="color:#dc2626">-14.42%</span> |
| 248 | [00004 TABLE_CONSTRAINTS_SUCCESS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_004_TABLE_CONSTRAINTS_SUCCESS.rs) | P0 | memory | SQL_CONSTRAINTS | 1588119 | 3432122 | <span style="color:#dc2626">-14.40%</span> |
| 249 | [00960 VIEW_TRIGGER_GENERATED_013](crates/bench/sqlite_parity/cases/SQLITE_PARITY_960_VIEW_TRIGGER_GENERATED_013.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1745326 | 3432081 | <span style="color:#dc2626">-14.40%</span> |
| 250 | [01005 VIEW_TRIGGER_GENERATED_058](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1005_VIEW_TRIGGER_GENERATED_058.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1793176 | 3430939 | <span style="color:#dc2626">-14.36%</span> |
| 251 | [00700 JOIN_SUBQUERY_EXISTS_093](crates/bench/sqlite_parity/cases/SQLITE_PARITY_700_JOIN_SUBQUERY_EXISTS_093.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1965143 | 3430388 | <span style="color:#dc2626">-14.35%</span> |
| 252 | [00959 VIEW_TRIGGER_GENERATED_012](crates/bench/sqlite_parity/cases/SQLITE_PARITY_959_VIEW_TRIGGER_GENERATED_012.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2374267 | 3430178 | <span style="color:#dc2626">-14.34%</span> |
| 253 | [00701 JOIN_SUBQUERY_EXISTS_094](crates/bench/sqlite_parity/cases/SQLITE_PARITY_701_JOIN_SUBQUERY_EXISTS_094.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1706703 | 3429166 | <span style="color:#dc2626">-14.31%</span> |
| 254 | [01085 INDEX_SCHEMA_PRAGMA_018](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1085_INDEX_SCHEMA_PRAGMA_018.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 2717337 | 3428915 | <span style="color:#dc2626">-14.30%</span> |
| 255 | [01053 JSON_EXTRACT_SET_046](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1053_JSON_EXTRACT_SET_046.rs) | P2 | memory | GEN_SQL_JSON | 1869261 | 3428374 | <span style="color:#dc2626">-14.28%</span> |
| 256 | [00654 JOIN_SUBQUERY_EXISTS_047](crates/bench/sqlite_parity/cases/SQLITE_PARITY_654_JOIN_SUBQUERY_EXISTS_047.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1660906 | 3427021 | <span style="color:#dc2626">-14.23%</span> |
| 257 | [00647 JOIN_SUBQUERY_EXISTS_040](crates/bench/sqlite_parity/cases/SQLITE_PARITY_647_JOIN_SUBQUERY_EXISTS_040.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1652451 | 3425008 | <span style="color:#dc2626">-14.17%</span> |
| 258 | [00457 DML_WHERE_ORDER_LIMIT_070](crates/bench/sqlite_parity/cases/SQLITE_PARITY_457_DML_WHERE_ORDER_LIMIT_070.rs) | P1 | memory | GEN_SQL_DML | 1640007 | 3422062 | <span style="color:#dc2626">-14.07%</span> |
| 259 | [00643 JOIN_SUBQUERY_EXISTS_036](crates/bench/sqlite_parity/cases/SQLITE_PARITY_643_JOIN_SUBQUERY_EXISTS_036.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1887656 | 3422062 | <span style="color:#dc2626">-14.07%</span> |
| 260 | [00397 DML_WHERE_ORDER_LIMIT_010](crates/bench/sqlite_parity/cases/SQLITE_PARITY_397_DML_WHERE_ORDER_LIMIT_010.rs) | P1 | memory | GEN_SQL_DML | 1744374 | 3421751 | <span style="color:#dc2626">-14.06%</span> |
| 261 | [00394 DML_WHERE_ORDER_LIMIT_007](crates/bench/sqlite_parity/cases/SQLITE_PARITY_394_DML_WHERE_ORDER_LIMIT_007.rs) | P1 | memory | GEN_SQL_DML | 1602266 | 3421702 | <span style="color:#dc2626">-14.06%</span> |
| 262 | [00651 JOIN_SUBQUERY_EXISTS_044](crates/bench/sqlite_parity/cases/SQLITE_PARITY_651_JOIN_SUBQUERY_EXISTS_044.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1607855 | 3419326 | <span style="color:#dc2626">-13.98%</span> |
| 263 | [01095 INDEX_SCHEMA_PRAGMA_028](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1095_INDEX_SCHEMA_PRAGMA_028.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1913044 | 3419307 | <span style="color:#dc2626">-13.98%</span> |
| 264 | [00420 DML_WHERE_ORDER_LIMIT_033](crates/bench/sqlite_parity/cases/SQLITE_PARITY_420_DML_WHERE_ORDER_LIMIT_033.rs) | P1 | memory | GEN_SQL_DML | 1568962 | 3419046 | <span style="color:#dc2626">-13.97%</span> |
| 265 | [01035 JSON_EXTRACT_SET_028](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1035_JSON_EXTRACT_SET_028.rs) | P2 | memory | GEN_SQL_JSON | 1598849 | 3417744 | <span style="color:#dc2626">-13.92%</span> |
| 266 | [00707 JOIN_SUBQUERY_EXISTS_100](crates/bench/sqlite_parity/cases/SQLITE_PARITY_707_JOIN_SUBQUERY_EXISTS_100.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1634906 | 3416833 | <span style="color:#dc2626">-13.89%</span> |
| 267 | [00687 JOIN_SUBQUERY_EXISTS_080](crates/bench/sqlite_parity/cases/SQLITE_PARITY_687_JOIN_SUBQUERY_EXISTS_080.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1621742 | 3414648 | <span style="color:#dc2626">-13.82%</span> |
| 268 | [01109 INDEX_SCHEMA_PRAGMA_042](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1109_INDEX_SCHEMA_PRAGMA_042.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 2268597 | 3413276 | <span style="color:#dc2626">-13.78%</span> |
| 269 | [00616 JOIN_SUBQUERY_EXISTS_009](crates/bench/sqlite_parity/cases/SQLITE_PARITY_616_JOIN_SUBQUERY_EXISTS_009.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1621441 | 3410510 | <span style="color:#dc2626">-13.68%</span> |
| 270 | [01079 INDEX_SCHEMA_PRAGMA_012](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1079_INDEX_SCHEMA_PRAGMA_012.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1634817 | 3409929 | <span style="color:#dc2626">-13.66%</span> |
| 271 | [00388 DML_WHERE_ORDER_LIMIT_001](crates/bench/sqlite_parity/cases/SQLITE_PARITY_388_DML_WHERE_ORDER_LIMIT_001.rs) | P1 | memory | GEN_SQL_DML | 1575805 | 3408447 | <span style="color:#dc2626">-13.61%</span> |
| 272 | [00626 JOIN_SUBQUERY_EXISTS_019](crates/bench/sqlite_parity/cases/SQLITE_PARITY_626_JOIN_SUBQUERY_EXISTS_019.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1653172 | 3407786 | <span style="color:#dc2626">-13.59%</span> |
| 273 | [01119 INDEX_SCHEMA_PRAGMA_052](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1119_INDEX_SCHEMA_PRAGMA_052.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1773169 | 3406172 | <span style="color:#dc2626">-13.54%</span> |
| 274 | [01104 INDEX_SCHEMA_PRAGMA_037](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1104_INDEX_SCHEMA_PRAGMA_037.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1717042 | 3405410 | <span style="color:#dc2626">-13.51%</span> |
| 275 | [00404 DML_WHERE_ORDER_LIMIT_017](crates/bench/sqlite_parity/cases/SQLITE_PARITY_404_DML_WHERE_ORDER_LIMIT_017.rs) | P1 | memory | GEN_SQL_DML | 1652039 | 3404529 | <span style="color:#dc2626">-13.48%</span> |
| 276 | [00674 JOIN_SUBQUERY_EXISTS_067](crates/bench/sqlite_parity/cases/SQLITE_PARITY_674_JOIN_SUBQUERY_EXISTS_067.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1605280 | 3404098 | <span style="color:#dc2626">-13.47%</span> |
| 277 | [00958 VIEW_TRIGGER_GENERATED_011](crates/bench/sqlite_parity/cases/SQLITE_PARITY_958_VIEW_TRIGGER_GENERATED_011.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1753020 | 3402976 | <span style="color:#dc2626">-13.43%</span> |
| 278 | [00445 DML_WHERE_ORDER_LIMIT_058](crates/bench/sqlite_parity/cases/SQLITE_PARITY_445_DML_WHERE_ORDER_LIMIT_058.rs) | P1 | memory | GEN_SQL_DML | 1574532 | 3402795 | <span style="color:#dc2626">-13.43%</span> |
| 279 | [00653 JOIN_SUBQUERY_EXISTS_046](crates/bench/sqlite_parity/cases/SQLITE_PARITY_653_JOIN_SUBQUERY_EXISTS_046.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1733484 | 3402775 | <span style="color:#dc2626">-13.43%</span> |
| 280 | [00671 JOIN_SUBQUERY_EXISTS_064](crates/bench/sqlite_parity/cases/SQLITE_PARITY_671_JOIN_SUBQUERY_EXISTS_064.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1682648 | 3402515 | <span style="color:#dc2626">-13.42%</span> |
| 281 | [00554 AGG_GROUP_HAVING_047](crates/bench/sqlite_parity/cases/SQLITE_PARITY_554_AGG_GROUP_HAVING_047.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1697114 | 3401864 | <span style="color:#dc2626">-13.40%</span> |
| 282 | [00630 JOIN_SUBQUERY_EXISTS_023](crates/bench/sqlite_parity/cases/SQLITE_PARITY_630_JOIN_SUBQUERY_EXISTS_023.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1731109 | 3400591 | <span style="color:#dc2626">-13.35%</span> |
| 283 | [00432 DML_WHERE_ORDER_LIMIT_045](crates/bench/sqlite_parity/cases/SQLITE_PARITY_432_DML_WHERE_ORDER_LIMIT_045.rs) | P1 | memory | GEN_SQL_DML | 1642361 | 3400261 | <span style="color:#dc2626">-13.34%</span> |
| 284 | [00494 DML_WHERE_ORDER_LIMIT_107](crates/bench/sqlite_parity/cases/SQLITE_PARITY_494_DML_WHERE_ORDER_LIMIT_107.rs) | P1 | memory | GEN_SQL_DML | 1543072 | 3399740 | <span style="color:#dc2626">-13.32%</span> |
| 285 | [00973 VIEW_TRIGGER_GENERATED_026](crates/bench/sqlite_parity/cases/SQLITE_PARITY_973_VIEW_TRIGGER_GENERATED_026.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1734325 | 3399539 | <span style="color:#dc2626">-13.32%</span> |
| 286 | [01062 JSON_EXTRACT_SET_055](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1062_JSON_EXTRACT_SET_055.rs) | P2 | memory | GEN_SQL_JSON | 1884439 | 3398988 | <span style="color:#dc2626">-13.30%</span> |
| 287 | [00470 DML_WHERE_ORDER_LIMIT_083](crates/bench/sqlite_parity/cases/SQLITE_PARITY_470_DML_WHERE_ORDER_LIMIT_083.rs) | P1 | memory | GEN_SQL_DML | 1605441 | 3398548 | <span style="color:#dc2626">-13.28%</span> |
| 288 | [00691 JOIN_SUBQUERY_EXISTS_084](crates/bench/sqlite_parity/cases/SQLITE_PARITY_691_JOIN_SUBQUERY_EXISTS_084.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1696604 | 3398478 | <span style="color:#dc2626">-13.28%</span> |
| 289 | [00629 JOIN_SUBQUERY_EXISTS_022](crates/bench/sqlite_parity/cases/SQLITE_PARITY_629_JOIN_SUBQUERY_EXISTS_022.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1727712 | 3398317 | <span style="color:#dc2626">-13.28%</span> |
| 290 | [00978 VIEW_TRIGGER_GENERATED_031](crates/bench/sqlite_parity/cases/SQLITE_PARITY_978_VIEW_TRIGGER_GENERATED_031.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2057828 | 3398067 | <span style="color:#dc2626">-13.27%</span> |
| 291 | [00949 VIEW_TRIGGER_GENERATED_002](crates/bench/sqlite_parity/cases/SQLITE_PARITY_949_VIEW_TRIGGER_GENERATED_002.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1631460 | 3396854 | <span style="color:#dc2626">-13.23%</span> |
| 292 | [00991 VIEW_TRIGGER_GENERATED_044](crates/bench/sqlite_parity/cases/SQLITE_PARITY_991_VIEW_TRIGGER_GENERATED_044.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1711852 | 3396814 | <span style="color:#dc2626">-13.23%</span> |
| 293 | [01094 INDEX_SCHEMA_PRAGMA_027](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1094_INDEX_SCHEMA_PRAGMA_027.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1667779 | 3396794 | <span style="color:#dc2626">-13.23%</span> |
| 294 | [00415 DML_WHERE_ORDER_LIMIT_028](crates/bench/sqlite_parity/cases/SQLITE_PARITY_415_DML_WHERE_ORDER_LIMIT_028.rs) | P1 | memory | GEN_SQL_DML | 1585313 | 3396224 | <span style="color:#dc2626">-13.21%</span> |
| 295 | [00692 JOIN_SUBQUERY_EXISTS_085](crates/bench/sqlite_parity/cases/SQLITE_PARITY_692_JOIN_SUBQUERY_EXISTS_085.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1653753 | 3395552 | <span style="color:#dc2626">-13.19%</span> |
| 296 | [00695 JOIN_SUBQUERY_EXISTS_088](crates/bench/sqlite_parity/cases/SQLITE_PARITY_695_JOIN_SUBQUERY_EXISTS_088.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1698127 | 3394841 | <span style="color:#dc2626">-13.16%</span> |
| 297 | [00986 VIEW_TRIGGER_GENERATED_039](crates/bench/sqlite_parity/cases/SQLITE_PARITY_986_VIEW_TRIGGER_GENERATED_039.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1757779 | 3394409 | <span style="color:#dc2626">-13.15%</span> |
| 298 | [00976 VIEW_TRIGGER_GENERATED_029](crates/bench/sqlite_parity/cases/SQLITE_PARITY_976_VIEW_TRIGGER_GENERATED_029.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1702735 | 3393609 | <span style="color:#dc2626">-13.12%</span> |
| 299 | [00638 JOIN_SUBQUERY_EXISTS_031](crates/bench/sqlite_parity/cases/SQLITE_PARITY_638_JOIN_SUBQUERY_EXISTS_031.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1592597 | 3393207 | <span style="color:#dc2626">-13.11%</span> |
| 300 | [00755 CTE_RECURSIVE_MATRIX_048](crates/bench/sqlite_parity/cases/SQLITE_PARITY_755_CTE_RECURSIVE_MATRIX_048.rs) | P1 | memory | GEN_SQL_CTE | 1541179 | 3393067 | <span style="color:#dc2626">-13.10%</span> |
| 301 | [01093 INDEX_SCHEMA_PRAGMA_026](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1093_INDEX_SCHEMA_PRAGMA_026.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1714026 | 3392636 | <span style="color:#dc2626">-13.09%</span> |
| 302 | [00497 DML_WHERE_ORDER_LIMIT_110](crates/bench/sqlite_parity/cases/SQLITE_PARITY_497_DML_WHERE_ORDER_LIMIT_110.rs) | P1 | memory | GEN_SQL_DML | 1598007 | 3391645 | <span style="color:#dc2626">-13.05%</span> |
| 303 | [00480 DML_WHERE_ORDER_LIMIT_093](crates/bench/sqlite_parity/cases/SQLITE_PARITY_480_DML_WHERE_ORDER_LIMIT_093.rs) | P1 | memory | GEN_SQL_DML | 1593018 | 3391515 | <span style="color:#dc2626">-13.05%</span> |
| 304 | [00492 DML_WHERE_ORDER_LIMIT_105](crates/bench/sqlite_parity/cases/SQLITE_PARITY_492_DML_WHERE_ORDER_LIMIT_105.rs) | P1 | memory | GEN_SQL_DML | 1658862 | 3391163 | <span style="color:#dc2626">-13.04%</span> |
| 305 | [00858 WINDOW_PARTITION_SUM_071](crates/bench/sqlite_parity/cases/SQLITE_PARITY_858_WINDOW_PARTITION_SUM_071.rs) | P2 | memory | GEN_SQL_WINDOW | 2403462 | 3388218 | <span style="color:#dc2626">-12.94%</span> |
| 306 | [01113 INDEX_SCHEMA_PRAGMA_046](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1113_INDEX_SCHEMA_PRAGMA_046.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 2028172 | 3387717 | <span style="color:#dc2626">-12.92%</span> |
| 307 | [00990 VIEW_TRIGGER_GENERATED_043](crates/bench/sqlite_parity/cases/SQLITE_PARITY_990_VIEW_TRIGGER_GENERATED_043.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1720239 | 3387617 | <span style="color:#dc2626">-12.92%</span> |
| 308 | [00917 CONSTRAINT_FK_SAVEPOINT_050](crates/bench/sqlite_parity/cases/SQLITE_PARITY_917_CONSTRAINT_FK_SAVEPOINT_050.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1811061 | 3386625 | <span style="color:#dc2626">-12.89%</span> |
| 309 | [00965 VIEW_TRIGGER_GENERATED_018](crates/bench/sqlite_parity/cases/SQLITE_PARITY_965_VIEW_TRIGGER_GENERATED_018.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1731299 | 3386174 | <span style="color:#dc2626">-12.87%</span> |
| 310 | [00390 DML_WHERE_ORDER_LIMIT_003](crates/bench/sqlite_parity/cases/SQLITE_PARITY_390_DML_WHERE_ORDER_LIMIT_003.rs) | P1 | memory | GEN_SQL_DML | 1609418 | 3385462 | <span style="color:#dc2626">-12.85%</span> |
| 311 | [00988 VIEW_TRIGGER_GENERATED_041](crates/bench/sqlite_parity/cases/SQLITE_PARITY_988_VIEW_TRIGGER_GENERATED_041.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1783017 | 3385202 | <span style="color:#dc2626">-12.84%</span> |
| 312 | [00414 DML_WHERE_ORDER_LIMIT_027](crates/bench/sqlite_parity/cases/SQLITE_PARITY_414_DML_WHERE_ORDER_LIMIT_027.rs) | P1 | memory | GEN_SQL_DML | 1598828 | 3384612 | <span style="color:#dc2626">-12.82%</span> |
| 313 | [00622 JOIN_SUBQUERY_EXISTS_015](crates/bench/sqlite_parity/cases/SQLITE_PARITY_622_JOIN_SUBQUERY_EXISTS_015.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1657720 | 3384271 | <span style="color:#dc2626">-12.81%</span> |
| 314 | [00434 DML_WHERE_ORDER_LIMIT_047](crates/bench/sqlite_parity/cases/SQLITE_PARITY_434_DML_WHERE_ORDER_LIMIT_047.rs) | P1 | memory | GEN_SQL_DML | 1645267 | 3382727 | <span style="color:#dc2626">-12.76%</span> |
| 315 | [00948 VIEW_TRIGGER_GENERATED_001](crates/bench/sqlite_parity/cases/SQLITE_PARITY_948_VIEW_TRIGGER_GENERATED_001.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1652560 | 3382457 | <span style="color:#dc2626">-12.75%</span> |
| 316 | [01086 INDEX_SCHEMA_PRAGMA_019](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1086_INDEX_SCHEMA_PRAGMA_019.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 2596839 | 3382357 | <span style="color:#dc2626">-12.75%</span> |
| 317 | [00621 JOIN_SUBQUERY_EXISTS_014](crates/bench/sqlite_parity/cases/SQLITE_PARITY_621_JOIN_SUBQUERY_EXISTS_014.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1862498 | 3382227 | <span style="color:#dc2626">-12.74%</span> |
| 318 | [00429 DML_WHERE_ORDER_LIMIT_042](crates/bench/sqlite_parity/cases/SQLITE_PARITY_429_DML_WHERE_ORDER_LIMIT_042.rs) | P1 | memory | GEN_SQL_DML | 1633194 | 3381957 | <span style="color:#dc2626">-12.73%</span> |
| 319 | [01122 INDEX_SCHEMA_PRAGMA_055](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1122_INDEX_SCHEMA_PRAGMA_055.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1627032 | 3380974 | <span style="color:#dc2626">-12.70%</span> |
| 320 | [00477 DML_WHERE_ORDER_LIMIT_090](crates/bench/sqlite_parity/cases/SQLITE_PARITY_477_DML_WHERE_ORDER_LIMIT_090.rs) | P1 | memory | GEN_SQL_DML | 1610150 | 3380764 | <span style="color:#dc2626">-12.69%</span> |
| 321 | [00472 DML_WHERE_ORDER_LIMIT_085](crates/bench/sqlite_parity/cases/SQLITE_PARITY_472_DML_WHERE_ORDER_LIMIT_085.rs) | P1 | memory | GEN_SQL_DML | 1618325 | 3380614 | <span style="color:#dc2626">-12.69%</span> |
| 322 | [00507 DML_WHERE_ORDER_LIMIT_120](crates/bench/sqlite_parity/cases/SQLITE_PARITY_507_DML_WHERE_ORDER_LIMIT_120.rs) | P1 | memory | GEN_SQL_DML | 1918955 | 3380504 | <span style="color:#dc2626">-12.68%</span> |
| 323 | [00989 VIEW_TRIGGER_GENERATED_042](crates/bench/sqlite_parity/cases/SQLITE_PARITY_989_VIEW_TRIGGER_GENERATED_042.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1698488 | 3379812 | <span style="color:#dc2626">-12.66%</span> |
| 324 | [00416 DML_WHERE_ORDER_LIMIT_029](crates/bench/sqlite_parity/cases/SQLITE_PARITY_416_DML_WHERE_ORDER_LIMIT_029.rs) | P1 | memory | GEN_SQL_DML | 1634766 | 3379782 | <span style="color:#dc2626">-12.66%</span> |
| 325 | [00461 DML_WHERE_ORDER_LIMIT_074](crates/bench/sqlite_parity/cases/SQLITE_PARITY_461_DML_WHERE_ORDER_LIMIT_074.rs) | P1 | memory | GEN_SQL_DML | 1581666 | 3379652 | <span style="color:#dc2626">-12.66%</span> |
| 326 | [00476 DML_WHERE_ORDER_LIMIT_089](crates/bench/sqlite_parity/cases/SQLITE_PARITY_476_DML_WHERE_ORDER_LIMIT_089.rs) | P1 | memory | GEN_SQL_DML | 1547061 | 3379151 | <span style="color:#dc2626">-12.64%</span> |
| 327 | [00961 VIEW_TRIGGER_GENERATED_014](crates/bench/sqlite_parity/cases/SQLITE_PARITY_961_VIEW_TRIGGER_GENERATED_014.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1727943 | 3378750 | <span style="color:#dc2626">-12.62%</span> |
| 328 | [00436 DML_WHERE_ORDER_LIMIT_049](crates/bench/sqlite_parity/cases/SQLITE_PARITY_436_DML_WHERE_ORDER_LIMIT_049.rs) | P1 | memory | GEN_SQL_DML | 1620500 | 3378650 | <span style="color:#dc2626">-12.62%</span> |
| 329 | [00479 DML_WHERE_ORDER_LIMIT_092](crates/bench/sqlite_parity/cases/SQLITE_PARITY_479_DML_WHERE_ORDER_LIMIT_092.rs) | P1 | memory | GEN_SQL_DML | 1538124 | 3378420 | <span style="color:#dc2626">-12.61%</span> |
| 330 | [01032 JSON_EXTRACT_SET_025](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1032_JSON_EXTRACT_SET_025.rs) | P2 | memory | GEN_SQL_JSON | 1530419 | 3377618 | <span style="color:#dc2626">-12.59%</span> |
| 331 | [00702 JOIN_SUBQUERY_EXISTS_095](crates/bench/sqlite_parity/cases/SQLITE_PARITY_702_JOIN_SUBQUERY_EXISTS_095.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2546393 | 3377267 | <span style="color:#dc2626">-12.58%</span> |
| 332 | [00983 VIEW_TRIGGER_GENERATED_036](crates/bench/sqlite_parity/cases/SQLITE_PARITY_983_VIEW_TRIGGER_GENERATED_036.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1720189 | 3376967 | <span style="color:#dc2626">-12.57%</span> |
| 333 | [01070 INDEX_SCHEMA_PRAGMA_003](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1070_INDEX_SCHEMA_PRAGMA_003.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 2512769 | 3376877 | <span style="color:#dc2626">-12.56%</span> |
| 334 | [00688 JOIN_SUBQUERY_EXISTS_081](crates/bench/sqlite_parity/cases/SQLITE_PARITY_688_JOIN_SUBQUERY_EXISTS_081.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1689991 | 3376465 | <span style="color:#dc2626">-12.55%</span> |
| 335 | [00689 JOIN_SUBQUERY_EXISTS_082](crates/bench/sqlite_parity/cases/SQLITE_PARITY_689_JOIN_SUBQUERY_EXISTS_082.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1657280 | 3376375 | <span style="color:#dc2626">-12.55%</span> |
| 336 | [01092 INDEX_SCHEMA_PRAGMA_025](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1092_INDEX_SCHEMA_PRAGMA_025.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1706092 | 3376175 | <span style="color:#dc2626">-12.54%</span> |
| 337 | [00597 AGG_GROUP_HAVING_090](crates/bench/sqlite_parity/cases/SQLITE_PARITY_597_AGG_GROUP_HAVING_090.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1642641 | 3375664 | <span style="color:#dc2626">-12.52%</span> |
| 338 | [00685 JOIN_SUBQUERY_EXISTS_078](crates/bench/sqlite_parity/cases/SQLITE_PARITY_685_JOIN_SUBQUERY_EXISTS_078.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1644074 | 3375554 | <span style="color:#dc2626">-12.52%</span> |
| 339 | [00576 AGG_GROUP_HAVING_069](crates/bench/sqlite_parity/cases/SQLITE_PARITY_576_AGG_GROUP_HAVING_069.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1608066 | 3374111 | <span style="color:#dc2626">-12.47%</span> |
| 340 | [00515 AGG_GROUP_HAVING_008](crates/bench/sqlite_parity/cases/SQLITE_PARITY_515_AGG_GROUP_HAVING_008.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2136727 | 3372308 | <span style="color:#dc2626">-12.41%</span> |
| 341 | [00585 AGG_GROUP_HAVING_078](crates/bench/sqlite_parity/cases/SQLITE_PARITY_585_AGG_GROUP_HAVING_078.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1783829 | 3372017 | <span style="color:#dc2626">-12.40%</span> |
| 342 | [01084 INDEX_SCHEMA_PRAGMA_017](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1084_INDEX_SCHEMA_PRAGMA_017.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1951406 | 3371917 | <span style="color:#dc2626">-12.40%</span> |
| 343 | [01096 INDEX_SCHEMA_PRAGMA_029](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1096_INDEX_SCHEMA_PRAGMA_029.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1638253 | 3368210 | <span style="color:#dc2626">-12.27%</span> |
| 344 | [00460 DML_WHERE_ORDER_LIMIT_073](crates/bench/sqlite_parity/cases/SQLITE_PARITY_460_DML_WHERE_ORDER_LIMIT_073.rs) | P1 | memory | GEN_SQL_DML | 1725679 | 3367849 | <span style="color:#dc2626">-12.26%</span> |
| 345 | [01088 INDEX_SCHEMA_PRAGMA_021](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1088_INDEX_SCHEMA_PRAGMA_021.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1629877 | 3367579 | <span style="color:#dc2626">-12.25%</span> |
| 346 | [00980 VIEW_TRIGGER_GENERATED_033](crates/bench/sqlite_parity/cases/SQLITE_PARITY_980_VIEW_TRIGGER_GENERATED_033.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1709798 | 3365315 | <span style="color:#dc2626">-12.18%</span> |
| 347 | [00962 VIEW_TRIGGER_GENERATED_015](crates/bench/sqlite_parity/cases/SQLITE_PARITY_962_VIEW_TRIGGER_GENERATED_015.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1719176 | 3364863 | <span style="color:#dc2626">-12.16%</span> |
| 348 | [01006 VIEW_TRIGGER_GENERATED_059](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1006_VIEW_TRIGGER_GENERATED_059.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1741719 | 3363892 | <span style="color:#dc2626">-12.13%</span> |
| 349 | [01089 INDEX_SCHEMA_PRAGMA_022](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1089_INDEX_SCHEMA_PRAGMA_022.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 2846202 | 3363691 | <span style="color:#dc2626">-12.12%</span> |
| 350 | [00817 WINDOW_PARTITION_SUM_030](crates/bench/sqlite_parity/cases/SQLITE_PARITY_817_WINDOW_PARTITION_SUM_030.rs) | P2 | memory | GEN_SQL_WINDOW | 1584441 | 3362349 | <span style="color:#dc2626">-12.08%</span> |
| 351 | [00825 WINDOW_PARTITION_SUM_038](crates/bench/sqlite_parity/cases/SQLITE_PARITY_825_WINDOW_PARTITION_SUM_038.rs) | P2 | memory | GEN_SQL_WINDOW | 1597627 | 3360505 | <span style="color:#dc2626">-12.02%</span> |
| 352 | [00994 VIEW_TRIGGER_GENERATED_047](crates/bench/sqlite_parity/cases/SQLITE_PARITY_994_VIEW_TRIGGER_GENERATED_047.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1768069 | 3358723 | <span style="color:#dc2626">-11.96%</span> |
| 353 | [01001 VIEW_TRIGGER_GENERATED_054](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1001_VIEW_TRIGGER_GENERATED_054.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1653702 | 3358141 | <span style="color:#dc2626">-11.94%</span> |
| 354 | [00703 JOIN_SUBQUERY_EXISTS_096](crates/bench/sqlite_parity/cases/SQLITE_PARITY_703_JOIN_SUBQUERY_EXISTS_096.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1659584 | 3356808 | <span style="color:#dc2626">-11.89%</span> |
| 355 | [00469 DML_WHERE_ORDER_LIMIT_082](crates/bench/sqlite_parity/cases/SQLITE_PARITY_469_DML_WHERE_ORDER_LIMIT_082.rs) | P1 | memory | GEN_SQL_DML | 1587387 | 3355987 | <span style="color:#dc2626">-11.87%</span> |
| 356 | [00403 DML_WHERE_ORDER_LIMIT_016](crates/bench/sqlite_parity/cases/SQLITE_PARITY_403_DML_WHERE_ORDER_LIMIT_016.rs) | P1 | memory | GEN_SQL_DML | 1560726 | 3355406 | <span style="color:#dc2626">-11.85%</span> |
| 357 | [00410 DML_WHERE_ORDER_LIMIT_023](crates/bench/sqlite_parity/cases/SQLITE_PARITY_410_DML_WHERE_ORDER_LIMIT_023.rs) | P1 | memory | GEN_SQL_DML | 1582157 | 3355225 | <span style="color:#dc2626">-11.84%</span> |
| 358 | [00054 JOINS_INNER_LEFT_CROSS_NATURAL](crates/bench/sqlite_parity/cases/SQLITE_PARITY_054_JOINS_INNER_LEFT_CROSS_NATURAL.rs) | P0 | memory | SQL_JOIN | 1721411 | 3354865 | <span style="color:#dc2626">-11.83%</span> |
| 359 | [00525 AGG_GROUP_HAVING_018](crates/bench/sqlite_parity/cases/SQLITE_PARITY_525_AGG_GROUP_HAVING_018.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1719466 | 3354364 | <span style="color:#dc2626">-11.81%</span> |
| 360 | [00613 JOIN_SUBQUERY_EXISTS_006](crates/bench/sqlite_parity/cases/SQLITE_PARITY_613_JOIN_SUBQUERY_EXISTS_006.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1609248 | 3354094 | <span style="color:#dc2626">-11.80%</span> |
| 361 | [00447 DML_WHERE_ORDER_LIMIT_060](crates/bench/sqlite_parity/cases/SQLITE_PARITY_447_DML_WHERE_ORDER_LIMIT_060.rs) | P1 | memory | GEN_SQL_DML | 1756848 | 3352550 | <span style="color:#dc2626">-11.75%</span> |
| 362 | [00449 DML_WHERE_ORDER_LIMIT_062](crates/bench/sqlite_parity/cases/SQLITE_PARITY_449_DML_WHERE_ORDER_LIMIT_062.rs) | P1 | memory | GEN_SQL_DML | 1852399 | 3351008 | <span style="color:#dc2626">-11.70%</span> |
| 363 | [00974 VIEW_TRIGGER_GENERATED_027](crates/bench/sqlite_parity/cases/SQLITE_PARITY_974_VIEW_TRIGGER_GENERATED_027.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1685273 | 3348543 | <span style="color:#dc2626">-11.62%</span> |
| 364 | [00465 DML_WHERE_ORDER_LIMIT_078](crates/bench/sqlite_parity/cases/SQLITE_PARITY_465_DML_WHERE_ORDER_LIMIT_078.rs) | P1 | memory | GEN_SQL_DML | 1557961 | 3347852 | <span style="color:#dc2626">-11.60%</span> |
| 365 | [00686 JOIN_SUBQUERY_EXISTS_079](crates/bench/sqlite_parity/cases/SQLITE_PARITY_686_JOIN_SUBQUERY_EXISTS_079.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1703958 | 3344775 | <span style="color:#dc2626">-11.49%</span> |
| 366 | [00524 AGG_GROUP_HAVING_017](crates/bench/sqlite_parity/cases/SQLITE_PARITY_524_AGG_GROUP_HAVING_017.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1654093 | 3344545 | <span style="color:#dc2626">-11.48%</span> |
| 367 | [00439 DML_WHERE_ORDER_LIMIT_052](crates/bench/sqlite_parity/cases/SQLITE_PARITY_439_DML_WHERE_ORDER_LIMIT_052.rs) | P1 | memory | GEN_SQL_DML | 1747580 | 3344476 | <span style="color:#dc2626">-11.48%</span> |
| 368 | [00448 DML_WHERE_ORDER_LIMIT_061](crates/bench/sqlite_parity/cases/SQLITE_PARITY_448_DML_WHERE_ORDER_LIMIT_061.rs) | P1 | memory | GEN_SQL_DML | 1781765 | 3344225 | <span style="color:#dc2626">-11.47%</span> |
| 369 | [00493 DML_WHERE_ORDER_LIMIT_106](crates/bench/sqlite_parity/cases/SQLITE_PARITY_493_DML_WHERE_ORDER_LIMIT_106.rs) | P1 | memory | GEN_SQL_DML | 1622543 | 3343764 | <span style="color:#dc2626">-11.46%</span> |
| 370 | [00481 DML_WHERE_ORDER_LIMIT_094](crates/bench/sqlite_parity/cases/SQLITE_PARITY_481_DML_WHERE_ORDER_LIMIT_094.rs) | P1 | memory | GEN_SQL_DML | 1576036 | 3343454 | <span style="color:#dc2626">-11.45%</span> |
| 371 | [00389 DML_WHERE_ORDER_LIMIT_002](crates/bench/sqlite_parity/cases/SQLITE_PARITY_389_DML_WHERE_ORDER_LIMIT_002.rs) | P1 | memory | GEN_SQL_DML | 1585042 | 3341239 | <span style="color:#dc2626">-11.37%</span> |
| 372 | [01097 INDEX_SCHEMA_PRAGMA_030](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1097_INDEX_SCHEMA_PRAGMA_030.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1690261 | 3340958 | <span style="color:#dc2626">-11.37%</span> |
| 373 | [00987 VIEW_TRIGGER_GENERATED_040](crates/bench/sqlite_parity/cases/SQLITE_PARITY_987_VIEW_TRIGGER_GENERATED_040.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1657430 | 3340878 | <span style="color:#dc2626">-11.36%</span> |
| 374 | [00418 DML_WHERE_ORDER_LIMIT_031](crates/bench/sqlite_parity/cases/SQLITE_PARITY_418_DML_WHERE_ORDER_LIMIT_031.rs) | P1 | memory | GEN_SQL_DML | 1603487 | 3340237 | <span style="color:#dc2626">-11.34%</span> |
| 375 | [00995 VIEW_TRIGGER_GENERATED_048](crates/bench/sqlite_parity/cases/SQLITE_PARITY_995_VIEW_TRIGGER_GENERATED_048.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1953280 | 3339696 | <span style="color:#dc2626">-11.32%</span> |
| 376 | [01087 INDEX_SCHEMA_PRAGMA_020](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1087_INDEX_SCHEMA_PRAGMA_020.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 2051155 | 3339696 | <span style="color:#dc2626">-11.32%</span> |
| 377 | [00408 DML_WHERE_ORDER_LIMIT_021](crates/bench/sqlite_parity/cases/SQLITE_PARITY_408_DML_WHERE_ORDER_LIMIT_021.rs) | P1 | memory | GEN_SQL_DML | 1582838 | 3338594 | <span style="color:#dc2626">-11.29%</span> |
| 378 | [00636 JOIN_SUBQUERY_EXISTS_029](crates/bench/sqlite_parity/cases/SQLITE_PARITY_636_JOIN_SUBQUERY_EXISTS_029.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1669452 | 3338073 | <span style="color:#dc2626">-11.27%</span> |
| 379 | [00985 VIEW_TRIGGER_GENERATED_038](crates/bench/sqlite_parity/cases/SQLITE_PARITY_985_VIEW_TRIGGER_GENERATED_038.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1720629 | 3337873 | <span style="color:#dc2626">-11.26%</span> |
| 380 | [00984 VIEW_TRIGGER_GENERATED_037](crates/bench/sqlite_parity/cases/SQLITE_PARITY_984_VIEW_TRIGGER_GENERATED_037.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1712203 | 3337823 | <span style="color:#dc2626">-11.26%</span> |
| 381 | [00407 DML_WHERE_ORDER_LIMIT_020](crates/bench/sqlite_parity/cases/SQLITE_PARITY_407_DML_WHERE_ORDER_LIMIT_020.rs) | P1 | memory | GEN_SQL_DML | 1676807 | 3335859 | <span style="color:#dc2626">-11.20%</span> |
| 382 | [00421 DML_WHERE_ORDER_LIMIT_034](crates/bench/sqlite_parity/cases/SQLITE_PARITY_421_DML_WHERE_ORDER_LIMIT_034.rs) | P1 | memory | GEN_SQL_DML | 1631410 | 3334667 | <span style="color:#dc2626">-11.16%</span> |
| 383 | [00446 DML_WHERE_ORDER_LIMIT_059](crates/bench/sqlite_parity/cases/SQLITE_PARITY_446_DML_WHERE_ORDER_LIMIT_059.rs) | P1 | memory | GEN_SQL_DML | 1637832 | 3334346 | <span style="color:#dc2626">-11.14%</span> |
| 384 | [01106 INDEX_SCHEMA_PRAGMA_039](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1106_INDEX_SCHEMA_PRAGMA_039.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1988216 | 3331110 | <span style="color:#dc2626">-11.04%</span> |
| 385 | [00452 DML_WHERE_ORDER_LIMIT_065](crates/bench/sqlite_parity/cases/SQLITE_PARITY_452_DML_WHERE_ORDER_LIMIT_065.rs) | P1 | memory | GEN_SQL_DML | 1639575 | 3330789 | <span style="color:#dc2626">-11.03%</span> |
| 386 | [01007 VIEW_TRIGGER_GENERATED_060](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1007_VIEW_TRIGGER_GENERATED_060.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2027290 | 3330639 | <span style="color:#dc2626">-11.02%</span> |
| 387 | [00998 VIEW_TRIGGER_GENERATED_051](crates/bench/sqlite_parity/cases/SQLITE_PARITY_998_VIEW_TRIGGER_GENERATED_051.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1665094 | 3330368 | <span style="color:#dc2626">-11.01%</span> |
| 388 | [01115 INDEX_SCHEMA_PRAGMA_048](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1115_INDEX_SCHEMA_PRAGMA_048.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1651368 | 3329998 | <span style="color:#dc2626">-11.00%</span> |
| 389 | [00498 DML_WHERE_ORDER_LIMIT_111](crates/bench/sqlite_parity/cases/SQLITE_PARITY_498_DML_WHERE_ORDER_LIMIT_111.rs) | P1 | memory | GEN_SQL_DML | 1578169 | 3329897 | <span style="color:#dc2626">-11.00%</span> |
| 390 | [00453 DML_WHERE_ORDER_LIMIT_066](crates/bench/sqlite_parity/cases/SQLITE_PARITY_453_DML_WHERE_ORDER_LIMIT_066.rs) | P1 | memory | GEN_SQL_DML | 1775814 | 3328595 | <span style="color:#dc2626">-10.95%</span> |
| 391 | [00999 VIEW_TRIGGER_GENERATED_052](crates/bench/sqlite_parity/cases/SQLITE_PARITY_999_VIEW_TRIGGER_GENERATED_052.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1697646 | 3328585 | <span style="color:#dc2626">-10.95%</span> |
| 392 | [00568 AGG_GROUP_HAVING_061](crates/bench/sqlite_parity/cases/SQLITE_PARITY_568_AGG_GROUP_HAVING_061.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1697856 | 3328325 | <span style="color:#dc2626">-10.94%</span> |
| 393 | [00966 VIEW_TRIGGER_GENERATED_019](crates/bench/sqlite_parity/cases/SQLITE_PARITY_966_VIEW_TRIGGER_GENERATED_019.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1713696 | 3327603 | <span style="color:#dc2626">-10.92%</span> |
| 394 | [00466 DML_WHERE_ORDER_LIMIT_079](crates/bench/sqlite_parity/cases/SQLITE_PARITY_466_DML_WHERE_ORDER_LIMIT_079.rs) | P1 | memory | GEN_SQL_DML | 1569513 | 3327353 | <span style="color:#dc2626">-10.91%</span> |
| 395 | [00400 DML_WHERE_ORDER_LIMIT_013](crates/bench/sqlite_parity/cases/SQLITE_PARITY_400_DML_WHERE_ORDER_LIMIT_013.rs) | P1 | memory | GEN_SQL_DML | 1648483 | 3326732 | <span style="color:#dc2626">-10.89%</span> |
| 396 | [00490 DML_WHERE_ORDER_LIMIT_103](crates/bench/sqlite_parity/cases/SQLITE_PARITY_490_DML_WHERE_ORDER_LIMIT_103.rs) | P1 | memory | GEN_SQL_DML | 1569844 | 3326551 | <span style="color:#dc2626">-10.89%</span> |
| 397 | [01121 INDEX_SCHEMA_PRAGMA_054](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1121_INDEX_SCHEMA_PRAGMA_054.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1653332 | 3325169 | <span style="color:#dc2626">-10.84%</span> |
| 398 | [00451 DML_WHERE_ORDER_LIMIT_064](crates/bench/sqlite_parity/cases/SQLITE_PARITY_451_DML_WHERE_ORDER_LIMIT_064.rs) | P1 | memory | GEN_SQL_DML | 1936358 | 3324036 | <span style="color:#dc2626">-10.80%</span> |
| 399 | [00545 AGG_GROUP_HAVING_038](crates/bench/sqlite_parity/cases/SQLITE_PARITY_545_AGG_GROUP_HAVING_038.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1849243 | 3323726 | <span style="color:#dc2626">-10.79%</span> |
| 400 | [00210 OPT_NOUNICODE_UTF8_CATALOG](crates/bench/sqlite_parity/cases/SQLITE_PARITY_210_OPT_NOUNICODE_UTF8_CATALOG.rs) | P4 | catalog | CLI_OPTION_CATALOG | 2017973 | 3323605 | <span style="color:#dc2626">-10.79%</span> |
| 401 | [01080 INDEX_SCHEMA_PRAGMA_013](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1080_INDEX_SCHEMA_PRAGMA_013.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1842901 | 3322233 | <span style="color:#dc2626">-10.74%</span> |
| 402 | [00463 DML_WHERE_ORDER_LIMIT_076](crates/bench/sqlite_parity/cases/SQLITE_PARITY_463_DML_WHERE_ORDER_LIMIT_076.rs) | P1 | memory | GEN_SQL_DML | 1582898 | 3321972 | <span style="color:#dc2626">-10.73%</span> |
| 403 | [00631 JOIN_SUBQUERY_EXISTS_024](crates/bench/sqlite_parity/cases/SQLITE_PARITY_631_JOIN_SUBQUERY_EXISTS_024.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1631791 | 3320530 | <span style="color:#dc2626">-10.68%</span> |
| 404 | [01069 INDEX_SCHEMA_PRAGMA_002](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1069_INDEX_SCHEMA_PRAGMA_002.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1913805 | 3319377 | <span style="color:#dc2626">-10.65%</span> |
| 405 | [00997 VIEW_TRIGGER_GENERATED_050](crates/bench/sqlite_parity/cases/SQLITE_PARITY_997_VIEW_TRIGGER_GENERATED_050.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1728584 | 3317795 | <span style="color:#dc2626">-10.59%</span> |
| 406 | [00529 AGG_GROUP_HAVING_022](crates/bench/sqlite_parity/cases/SQLITE_PARITY_529_AGG_GROUP_HAVING_022.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1747871 | 3314017 | <span style="color:#dc2626">-10.47%</span> |
| 407 | [00306 SCALAR_STRING_020](crates/bench/sqlite_parity/cases/SQLITE_PARITY_306_SCALAR_STRING_020.rs) | P1 | memory | GEN_SQL_SCALAR | 1504139 | 3313988 | <span style="color:#dc2626">-10.47%</span> |
| 408 | [00950 VIEW_TRIGGER_GENERATED_003](crates/bench/sqlite_parity/cases/SQLITE_PARITY_950_VIEW_TRIGGER_GENERATED_003.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1709158 | 3313646 | <span style="color:#dc2626">-10.45%</span> |
| 409 | [00425 DML_WHERE_ORDER_LIMIT_038](crates/bench/sqlite_parity/cases/SQLITE_PARITY_425_DML_WHERE_ORDER_LIMIT_038.rs) | P1 | memory | GEN_SQL_DML | 1576496 | 3313627 | <span style="color:#dc2626">-10.45%</span> |
| 410 | [00898 CONSTRAINT_FK_SAVEPOINT_031](crates/bench/sqlite_parity/cases/SQLITE_PARITY_898_CONSTRAINT_FK_SAVEPOINT_031.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1634285 | 3312976 | <span style="color:#dc2626">-10.43%</span> |
| 411 | [00435 DML_WHERE_ORDER_LIMIT_048](crates/bench/sqlite_parity/cases/SQLITE_PARITY_435_DML_WHERE_ORDER_LIMIT_048.rs) | P1 | memory | GEN_SQL_DML | 1651127 | 3310892 | <span style="color:#dc2626">-10.36%</span> |
| 412 | [01077 INDEX_SCHEMA_PRAGMA_010](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1077_INDEX_SCHEMA_PRAGMA_010.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1682377 | 3310080 | <span style="color:#dc2626">-10.34%</span> |
| 413 | [00606 AGG_GROUP_HAVING_099](crates/bench/sqlite_parity/cases/SQLITE_PARITY_606_AGG_GROUP_HAVING_099.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1639365 | 3309419 | <span style="color:#dc2626">-10.31%</span> |
| 414 | [00458 DML_WHERE_ORDER_LIMIT_071](crates/bench/sqlite_parity/cases/SQLITE_PARITY_458_DML_WHERE_ORDER_LIMIT_071.rs) | P1 | memory | GEN_SQL_DML | 1687055 | 3309158 | <span style="color:#dc2626">-10.31%</span> |
| 415 | [00943 CONSTRAINT_FK_SAVEPOINT_076](crates/bench/sqlite_parity/cases/SQLITE_PARITY_943_CONSTRAINT_FK_SAVEPOINT_076.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1636870 | 3308276 | <span style="color:#dc2626">-10.28%</span> |
| 416 | [00604 AGG_GROUP_HAVING_097](crates/bench/sqlite_parity/cases/SQLITE_PARITY_604_AGG_GROUP_HAVING_097.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1671066 | 3308146 | <span style="color:#dc2626">-10.27%</span> |
| 417 | [01018 JSON_EXTRACT_SET_011](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1018_JSON_EXTRACT_SET_011.rs) | P2 | memory | GEN_SQL_JSON | 1554254 | 3307736 | <span style="color:#dc2626">-10.26%</span> |
| 418 | [00582 AGG_GROUP_HAVING_075](crates/bench/sqlite_parity/cases/SQLITE_PARITY_582_AGG_GROUP_HAVING_075.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1842841 | 3306614 | <span style="color:#dc2626">-10.22%</span> |
| 419 | [00406 DML_WHERE_ORDER_LIMIT_019](crates/bench/sqlite_parity/cases/SQLITE_PARITY_406_DML_WHERE_ORDER_LIMIT_019.rs) | P1 | memory | GEN_SQL_DML | 2371291 | 3306574 | <span style="color:#dc2626">-10.22%</span> |
| 420 | [01059 JSON_EXTRACT_SET_052](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1059_JSON_EXTRACT_SET_052.rs) | P2 | memory | GEN_SQL_JSON | 1465125 | 3306343 | <span style="color:#dc2626">-10.21%</span> |
| 421 | [00423 DML_WHERE_ORDER_LIMIT_036](crates/bench/sqlite_parity/cases/SQLITE_PARITY_423_DML_WHERE_ORDER_LIMIT_036.rs) | P1 | memory | GEN_SQL_DML | 1524688 | 3305682 | <span style="color:#dc2626">-10.19%</span> |
| 422 | [00468 DML_WHERE_ORDER_LIMIT_081](crates/bench/sqlite_parity/cases/SQLITE_PARITY_468_DML_WHERE_ORDER_LIMIT_081.rs) | P1 | memory | GEN_SQL_DML | 1513657 | 3305171 | <span style="color:#dc2626">-10.17%</span> |
| 423 | [00574 AGG_GROUP_HAVING_067](crates/bench/sqlite_parity/cases/SQLITE_PARITY_574_AGG_GROUP_HAVING_067.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1848342 | 3303037 | <span style="color:#dc2626">-10.10%</span> |
| 424 | [00454 DML_WHERE_ORDER_LIMIT_067](crates/bench/sqlite_parity/cases/SQLITE_PARITY_454_DML_WHERE_ORDER_LIMIT_067.rs) | P1 | memory | GEN_SQL_DML | 1573410 | 3302996 | <span style="color:#dc2626">-10.10%</span> |
| 425 | [00571 AGG_GROUP_HAVING_064](crates/bench/sqlite_parity/cases/SQLITE_PARITY_571_AGG_GROUP_HAVING_064.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1826410 | 3302957 | <span style="color:#dc2626">-10.10%</span> |
| 426 | [00672 JOIN_SUBQUERY_EXISTS_065](crates/bench/sqlite_parity/cases/SQLITE_PARITY_672_JOIN_SUBQUERY_EXISTS_065.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1660165 | 3302767 | <span style="color:#dc2626">-10.09%</span> |
| 427 | [00485 DML_WHERE_ORDER_LIMIT_098](crates/bench/sqlite_parity/cases/SQLITE_PARITY_485_DML_WHERE_ORDER_LIMIT_098.rs) | P1 | memory | GEN_SQL_DML | 1540268 | 3301704 | <span style="color:#dc2626">-10.06%</span> |
| 428 | [01114 INDEX_SCHEMA_PRAGMA_047](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1114_INDEX_SCHEMA_PRAGMA_047.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1654794 | 3301584 | <span style="color:#dc2626">-10.05%</span> |
| 429 | [00495 DML_WHERE_ORDER_LIMIT_108](crates/bench/sqlite_parity/cases/SQLITE_PARITY_495_DML_WHERE_ORDER_LIMIT_108.rs) | P1 | memory | GEN_SQL_DML | 1792666 | 3300602 | <span style="color:#dc2626">-10.02%</span> |
| 430 | [00438 DML_WHERE_ORDER_LIMIT_051](crates/bench/sqlite_parity/cases/SQLITE_PARITY_438_DML_WHERE_ORDER_LIMIT_051.rs) | P1 | memory | GEN_SQL_DML | 1543694 | 3300301 | <span style="color:#dc2626">-10.01%</span> |
| 431 | [00521 AGG_GROUP_HAVING_014](crates/bench/sqlite_parity/cases/SQLITE_PARITY_521_AGG_GROUP_HAVING_014.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2311037 | 3300192 | <span style="color:#dc2626">-10.01%</span> |
| 432 | [00928 CONSTRAINT_FK_SAVEPOINT_061](crates/bench/sqlite_parity/cases/SQLITE_PARITY_928_CONSTRAINT_FK_SAVEPOINT_061.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1675003 | 3300141 | <span style="color:#dc2626">-10.00%</span> |
| 433 | [00565 AGG_GROUP_HAVING_058](crates/bench/sqlite_parity/cases/SQLITE_PARITY_565_AGG_GROUP_HAVING_058.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1619928 | 3299130 | <span style="color:#dc2626">-9.97%</span> |
| 434 | [00603 AGG_GROUP_HAVING_096](crates/bench/sqlite_parity/cases/SQLITE_PARITY_603_AGG_GROUP_HAVING_096.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1682087 | 3297907 | <span style="color:#dc2626">-9.93%</span> |
| 435 | [00954 VIEW_TRIGGER_GENERATED_007](crates/bench/sqlite_parity/cases/SQLITE_PARITY_954_VIEW_TRIGGER_GENERATED_007.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1677838 | 3297366 | <span style="color:#dc2626">-9.91%</span> |
| 436 | [00964 VIEW_TRIGGER_GENERATED_017](crates/bench/sqlite_parity/cases/SQLITE_PARITY_964_VIEW_TRIGGER_GENERATED_017.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1671136 | 3296334 | <span style="color:#dc2626">-9.88%</span> |
| 437 | [00484 DML_WHERE_ORDER_LIMIT_097](crates/bench/sqlite_parity/cases/SQLITE_PARITY_484_DML_WHERE_ORDER_LIMIT_097.rs) | P1 | memory | GEN_SQL_DML | 1580905 | 3296204 | <span style="color:#dc2626">-9.87%</span> |
| 438 | [00595 AGG_GROUP_HAVING_088](crates/bench/sqlite_parity/cases/SQLITE_PARITY_595_AGG_GROUP_HAVING_088.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1793227 | 3295933 | <span style="color:#dc2626">-9.86%</span> |
| 439 | [00981 VIEW_TRIGGER_GENERATED_034](crates/bench/sqlite_parity/cases/SQLITE_PARITY_981_VIEW_TRIGGER_GENERATED_034.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1665334 | 3294881 | <span style="color:#dc2626">-9.83%</span> |
| 440 | [00904 CONSTRAINT_FK_SAVEPOINT_037](crates/bench/sqlite_parity/cases/SQLITE_PARITY_904_CONSTRAINT_FK_SAVEPOINT_037.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1701523 | 3294210 | <span style="color:#dc2626">-9.81%</span> |
| 441 | [00483 DML_WHERE_ORDER_LIMIT_096](crates/bench/sqlite_parity/cases/SQLITE_PARITY_483_DML_WHERE_ORDER_LIMIT_096.rs) | P1 | memory | GEN_SQL_DML | 1607766 | 3294080 | <span style="color:#dc2626">-9.80%</span> |
| 442 | [00774 CTE_RECURSIVE_MATRIX_067](crates/bench/sqlite_parity/cases/SQLITE_PARITY_774_CTE_RECURSIVE_MATRIX_067.rs) | P1 | memory | GEN_SQL_CTE | 1468733 | 3293940 | <span style="color:#dc2626">-9.80%</span> |
| 443 | [00579 AGG_GROUP_HAVING_072](crates/bench/sqlite_parity/cases/SQLITE_PARITY_579_AGG_GROUP_HAVING_072.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1736690 | 3293368 | <span style="color:#dc2626">-9.78%</span> |
| 444 | [01000 VIEW_TRIGGER_GENERATED_053](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1000_VIEW_TRIGGER_GENERATED_053.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2028072 | 3293248 | <span style="color:#dc2626">-9.77%</span> |
| 445 | [00563 AGG_GROUP_HAVING_056](crates/bench/sqlite_parity/cases/SQLITE_PARITY_563_AGG_GROUP_HAVING_056.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1630399 | 3292937 | <span style="color:#dc2626">-9.76%</span> |
| 446 | [00963 VIEW_TRIGGER_GENERATED_016](crates/bench/sqlite_parity/cases/SQLITE_PARITY_963_VIEW_TRIGGER_GENERATED_016.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1695782 | 3292417 | <span style="color:#dc2626">-9.75%</span> |
| 447 | [00500 DML_WHERE_ORDER_LIMIT_113](crates/bench/sqlite_parity/cases/SQLITE_PARITY_500_DML_WHERE_ORDER_LIMIT_113.rs) | P1 | memory | GEN_SQL_DML | 1644856 | 3292296 | <span style="color:#dc2626">-9.74%</span> |
| 448 | [00467 DML_WHERE_ORDER_LIMIT_080](crates/bench/sqlite_parity/cases/SQLITE_PARITY_467_DML_WHERE_ORDER_LIMIT_080.rs) | P1 | memory | GEN_SQL_DML | 1597186 | 3292056 | <span style="color:#dc2626">-9.74%</span> |
| 449 | [00462 DML_WHERE_ORDER_LIMIT_075](crates/bench/sqlite_parity/cases/SQLITE_PARITY_462_DML_WHERE_ORDER_LIMIT_075.rs) | P1 | memory | GEN_SQL_DML | 1611552 | 3291675 | <span style="color:#dc2626">-9.72%</span> |
| 450 | [00412 DML_WHERE_ORDER_LIMIT_025](crates/bench/sqlite_parity/cases/SQLITE_PARITY_412_DML_WHERE_ORDER_LIMIT_025.rs) | P1 | memory | GEN_SQL_DML | 1567669 | 3291505 | <span style="color:#dc2626">-9.72%</span> |
| 451 | [00871 CONSTRAINT_FK_SAVEPOINT_004](crates/bench/sqlite_parity/cases/SQLITE_PARITY_871_CONSTRAINT_FK_SAVEPOINT_004.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1695231 | 3291474 | <span style="color:#dc2626">-9.72%</span> |
| 452 | [00402 DML_WHERE_ORDER_LIMIT_015](crates/bench/sqlite_parity/cases/SQLITE_PARITY_402_DML_WHERE_ORDER_LIMIT_015.rs) | P1 | memory | GEN_SQL_DML | 1740397 | 3290664 | <span style="color:#dc2626">-9.69%</span> |
| 453 | [00489 DML_WHERE_ORDER_LIMIT_102](crates/bench/sqlite_parity/cases/SQLITE_PARITY_489_DML_WHERE_ORDER_LIMIT_102.rs) | P1 | memory | GEN_SQL_DML | 1550587 | 3290293 | <span style="color:#dc2626">-9.68%</span> |
| 454 | [00487 DML_WHERE_ORDER_LIMIT_100](crates/bench/sqlite_parity/cases/SQLITE_PARITY_487_DML_WHERE_ORDER_LIMIT_100.rs) | P1 | memory | GEN_SQL_DML | 1917192 | 3289811 | <span style="color:#dc2626">-9.66%</span> |
| 455 | [00527 AGG_GROUP_HAVING_020](crates/bench/sqlite_parity/cases/SQLITE_PARITY_527_AGG_GROUP_HAVING_020.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1682487 | 3289592 | <span style="color:#dc2626">-9.65%</span> |
| 456 | [00544 AGG_GROUP_HAVING_037](crates/bench/sqlite_parity/cases/SQLITE_PARITY_544_AGG_GROUP_HAVING_037.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1660475 | 3289391 | <span style="color:#dc2626">-9.65%</span> |
| 457 | [00598 AGG_GROUP_HAVING_091](crates/bench/sqlite_parity/cases/SQLITE_PARITY_598_AGG_GROUP_HAVING_091.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1597846 | 3289030 | <span style="color:#dc2626">-9.63%</span> |
| 458 | [00930 CONSTRAINT_FK_SAVEPOINT_063](crates/bench/sqlite_parity/cases/SQLITE_PARITY_930_CONSTRAINT_FK_SAVEPOINT_063.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1694760 | 3288500 | <span style="color:#dc2626">-9.62%</span> |
| 459 | [00906 CONSTRAINT_FK_SAVEPOINT_039](crates/bench/sqlite_parity/cases/SQLITE_PARITY_906_CONSTRAINT_FK_SAVEPOINT_039.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1978338 | 3287617 | <span style="color:#dc2626">-9.59%</span> |
| 460 | [00430 DML_WHERE_ORDER_LIMIT_043](crates/bench/sqlite_parity/cases/SQLITE_PARITY_430_DML_WHERE_ORDER_LIMIT_043.rs) | P1 | memory | GEN_SQL_DML | 1556980 | 3287437 | <span style="color:#dc2626">-9.58%</span> |
| 461 | [00915 CONSTRAINT_FK_SAVEPOINT_048](crates/bench/sqlite_parity/cases/SQLITE_PARITY_915_CONSTRAINT_FK_SAVEPOINT_048.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1686064 | 3287006 | <span style="color:#dc2626">-9.57%</span> |
| 462 | [00473 DML_WHERE_ORDER_LIMIT_086](crates/bench/sqlite_parity/cases/SQLITE_PARITY_473_DML_WHERE_ORDER_LIMIT_086.rs) | P1 | memory | GEN_SQL_DML | 1863450 | 3285103 | <span style="color:#dc2626">-9.50%</span> |
| 463 | [00979 VIEW_TRIGGER_GENERATED_032](crates/bench/sqlite_parity/cases/SQLITE_PARITY_979_VIEW_TRIGGER_GENERATED_032.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1727642 | 3284492 | <span style="color:#dc2626">-9.48%</span> |
| 464 | [00561 AGG_GROUP_HAVING_054](crates/bench/sqlite_parity/cases/SQLITE_PARITY_561_AGG_GROUP_HAVING_054.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1650787 | 3283260 | <span style="color:#dc2626">-9.44%</span> |
| 465 | [00501 DML_WHERE_ORDER_LIMIT_114](crates/bench/sqlite_parity/cases/SQLITE_PARITY_501_DML_WHERE_ORDER_LIMIT_114.rs) | P1 | memory | GEN_SQL_DML | 1701694 | 3281576 | <span style="color:#dc2626">-9.39%</span> |
| 466 | [00942 CONSTRAINT_FK_SAVEPOINT_075](crates/bench/sqlite_parity/cases/SQLITE_PARITY_942_CONSTRAINT_FK_SAVEPOINT_075.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1646559 | 3281115 | <span style="color:#dc2626">-9.37%</span> |
| 467 | [00901 CONSTRAINT_FK_SAVEPOINT_034](crates/bench/sqlite_parity/cases/SQLITE_PARITY_901_CONSTRAINT_FK_SAVEPOINT_034.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1674161 | 3279131 | <span style="color:#dc2626">-9.30%</span> |
| 468 | [00951 VIEW_TRIGGER_GENERATED_004](crates/bench/sqlite_parity/cases/SQLITE_PARITY_951_VIEW_TRIGGER_GENERATED_004.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1721251 | 3279051 | <span style="color:#dc2626">-9.30%</span> |
| 469 | [00426 DML_WHERE_ORDER_LIMIT_039](crates/bench/sqlite_parity/cases/SQLITE_PARITY_426_DML_WHERE_ORDER_LIMIT_039.rs) | P1 | memory | GEN_SQL_DML | 1695512 | 3278971 | <span style="color:#dc2626">-9.30%</span> |
| 470 | [00566 AGG_GROUP_HAVING_059](crates/bench/sqlite_parity/cases/SQLITE_PARITY_566_AGG_GROUP_HAVING_059.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1568982 | 3278490 | <span style="color:#dc2626">-9.28%</span> |
| 471 | [00534 AGG_GROUP_HAVING_027](crates/bench/sqlite_parity/cases/SQLITE_PARITY_534_AGG_GROUP_HAVING_027.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1751798 | 3276927 | <span style="color:#dc2626">-9.23%</span> |
| 472 | [00556 AGG_GROUP_HAVING_049](crates/bench/sqlite_parity/cases/SQLITE_PARITY_556_AGG_GROUP_HAVING_049.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1636921 | 3275033 | <span style="color:#dc2626">-9.17%</span> |
| 473 | [00455 DML_WHERE_ORDER_LIMIT_068](crates/bench/sqlite_parity/cases/SQLITE_PARITY_455_DML_WHERE_ORDER_LIMIT_068.rs) | P1 | memory | GEN_SQL_DML | 1525971 | 3273621 | <span style="color:#dc2626">-9.12%</span> |
| 474 | [00875 CONSTRAINT_FK_SAVEPOINT_008](crates/bench/sqlite_parity/cases/SQLITE_PARITY_875_CONSTRAINT_FK_SAVEPOINT_008.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1638193 | 3272339 | <span style="color:#dc2626">-9.08%</span> |
| 475 | [00587 AGG_GROUP_HAVING_080](crates/bench/sqlite_parity/cases/SQLITE_PARITY_587_AGG_GROUP_HAVING_080.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2103775 | 3271407 | <span style="color:#dc2626">-9.05%</span> |
| 476 | [00547 AGG_GROUP_HAVING_040](crates/bench/sqlite_parity/cases/SQLITE_PARITY_547_AGG_GROUP_HAVING_040.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1678750 | 3271086 | <span style="color:#dc2626">-9.04%</span> |
| 477 | [00526 AGG_GROUP_HAVING_019](crates/bench/sqlite_parity/cases/SQLITE_PARITY_526_AGG_GROUP_HAVING_019.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1705020 | 3269713 | <span style="color:#dc2626">-8.99%</span> |
| 478 | [00591 AGG_GROUP_HAVING_084](crates/bench/sqlite_parity/cases/SQLITE_PARITY_591_AGG_GROUP_HAVING_084.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1637973 | 3269052 | <span style="color:#dc2626">-8.97%</span> |
| 479 | [00891 CONSTRAINT_FK_SAVEPOINT_024](crates/bench/sqlite_parity/cases/SQLITE_PARITY_891_CONSTRAINT_FK_SAVEPOINT_024.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1697255 | 3267819 | <span style="color:#dc2626">-8.93%</span> |
| 480 | [00874 CONSTRAINT_FK_SAVEPOINT_007](crates/bench/sqlite_parity/cases/SQLITE_PARITY_874_CONSTRAINT_FK_SAVEPOINT_007.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1797275 | 3267309 | <span style="color:#dc2626">-8.91%</span> |
| 481 | [00559 AGG_GROUP_HAVING_052](crates/bench/sqlite_parity/cases/SQLITE_PARITY_559_AGG_GROUP_HAVING_052.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1666156 | 3266718 | <span style="color:#dc2626">-8.89%</span> |
| 482 | [00540 AGG_GROUP_HAVING_033](crates/bench/sqlite_parity/cases/SQLITE_PARITY_540_AGG_GROUP_HAVING_033.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1659504 | 3265886 | <span style="color:#dc2626">-8.86%</span> |
| 483 | [00720 CTE_RECURSIVE_MATRIX_013](crates/bench/sqlite_parity/cases/SQLITE_PARITY_720_CTE_RECURSIVE_MATRIX_013.rs) | P1 | memory | GEN_SQL_CTE | 1558792 | 3265316 | <span style="color:#dc2626">-8.84%</span> |
| 484 | [00474 DML_WHERE_ORDER_LIMIT_087](crates/bench/sqlite_parity/cases/SQLITE_PARITY_474_DML_WHERE_ORDER_LIMIT_087.rs) | P1 | memory | GEN_SQL_DML | 1676346 | 3264484 | <span style="color:#dc2626">-8.82%</span> |
| 485 | [00583 AGG_GROUP_HAVING_076](crates/bench/sqlite_parity/cases/SQLITE_PARITY_583_AGG_GROUP_HAVING_076.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1668490 | 3264414 | <span style="color:#dc2626">-8.81%</span> |
| 486 | [01066 JSON_EXTRACT_SET_059](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1066_JSON_EXTRACT_SET_059.rs) | P2 | memory | GEN_SQL_JSON | 1671095 | 3263882 | <span style="color:#dc2626">-8.80%</span> |
| 487 | [01117 INDEX_SCHEMA_PRAGMA_050](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1117_INDEX_SCHEMA_PRAGMA_050.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1652289 | 3263322 | <span style="color:#dc2626">-8.78%</span> |
| 488 | [00947 CONSTRAINT_FK_SAVEPOINT_080](crates/bench/sqlite_parity/cases/SQLITE_PARITY_947_CONSTRAINT_FK_SAVEPOINT_080.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1666827 | 3263291 | <span style="color:#dc2626">-8.78%</span> |
| 489 | [00411 DML_WHERE_ORDER_LIMIT_024](crates/bench/sqlite_parity/cases/SQLITE_PARITY_411_DML_WHERE_ORDER_LIMIT_024.rs) | P1 | memory | GEN_SQL_DML | 1591474 | 3263061 | <span style="color:#dc2626">-8.77%</span> |
| 490 | [00222 OPT_ESCAPE_SYMBOL](crates/bench/sqlite_parity/cases/SQLITE_PARITY_222_OPT_ESCAPE_SYMBOL.rs) | P3 | memory | CLI_OPTION | 1460326 | 3260917 | <span style="color:#dc2626">-8.70%</span> |
| 491 | [01017 JSON_EXTRACT_SET_010](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1017_JSON_EXTRACT_SET_010.rs) | P2 | memory | GEN_SQL_JSON | 1523576 | 3260015 | <span style="color:#dc2626">-8.67%</span> |
| 492 | [00922 CONSTRAINT_FK_SAVEPOINT_055](crates/bench/sqlite_parity/cases/SQLITE_PARITY_922_CONSTRAINT_FK_SAVEPOINT_055.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1630168 | 3259004 | <span style="color:#dc2626">-8.63%</span> |
| 493 | [00440 DML_WHERE_ORDER_LIMIT_053](crates/bench/sqlite_parity/cases/SQLITE_PARITY_440_DML_WHERE_ORDER_LIMIT_053.rs) | P1 | memory | GEN_SQL_DML | 1502857 | 3258923 | <span style="color:#dc2626">-8.63%</span> |
| 494 | [00006 ROWID_INTEGER_PRIMARY_KEY](crates/bench/sqlite_parity/cases/SQLITE_PARITY_006_ROWID_INTEGER_PRIMARY_KEY.rs) | P0 | memory | SQL_ROWID | 1556309 | 3256519 | <span style="color:#dc2626">-8.55%</span> |
| 495 | [00884 CONSTRAINT_FK_SAVEPOINT_017](crates/bench/sqlite_parity/cases/SQLITE_PARITY_884_CONSTRAINT_FK_SAVEPOINT_017.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1817784 | 3256388 | <span style="color:#dc2626">-8.55%</span> |
| 496 | [00940 CONSTRAINT_FK_SAVEPOINT_073](crates/bench/sqlite_parity/cases/SQLITE_PARITY_940_CONSTRAINT_FK_SAVEPOINT_073.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1643713 | 3255056 | <span style="color:#dc2626">-8.50%</span> |
| 497 | [00382 SCALAR_STRING_039](crates/bench/sqlite_parity/cases/SQLITE_PARITY_382_SCALAR_STRING_039.rs) | P1 | memory | GEN_SQL_SCALAR | 1465366 | 3254474 | <span style="color:#dc2626">-8.48%</span> |
| 498 | [00937 CONSTRAINT_FK_SAVEPOINT_070](crates/bench/sqlite_parity/cases/SQLITE_PARITY_937_CONSTRAINT_FK_SAVEPOINT_070.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1666867 | 3254294 | <span style="color:#dc2626">-8.48%</span> |
| 499 | [00217 DETACH_DATABASE_SYNTAX](crates/bench/sqlite_parity/cases/SQLITE_PARITY_217_DETACH_DATABASE_SYNTAX.rs) | P0 | memory | SQL_ATTACH | 1532062 | 3252792 | <span style="color:#dc2626">-8.43%</span> |
| 500 | [00877 CONSTRAINT_FK_SAVEPOINT_010](crates/bench/sqlite_parity/cases/SQLITE_PARITY_877_CONSTRAINT_FK_SAVEPOINT_010.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1652450 | 3251208 | <span style="color:#dc2626">-8.37%</span> |
| 501 | [00746 CTE_RECURSIVE_MATRIX_039](crates/bench/sqlite_parity/cases/SQLITE_PARITY_746_CTE_RECURSIVE_MATRIX_039.rs) | P1 | memory | GEN_SQL_CTE | 1524087 | 3250337 | <span style="color:#dc2626">-8.34%</span> |
| 502 | [00456 DML_WHERE_ORDER_LIMIT_069](crates/bench/sqlite_parity/cases/SQLITE_PARITY_456_DML_WHERE_ORDER_LIMIT_069.rs) | P1 | memory | GEN_SQL_DML | 1502075 | 3250147 | <span style="color:#dc2626">-8.34%</span> |
| 503 | [00532 AGG_GROUP_HAVING_025](crates/bench/sqlite_parity/cases/SQLITE_PARITY_532_AGG_GROUP_HAVING_025.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1719538 | 3249405 | <span style="color:#dc2626">-8.31%</span> |
| 504 | [00929 CONSTRAINT_FK_SAVEPOINT_062](crates/bench/sqlite_parity/cases/SQLITE_PARITY_929_CONSTRAINT_FK_SAVEPOINT_062.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1657781 | 3249185 | <span style="color:#dc2626">-8.31%</span> |
| 505 | [00592 AGG_GROUP_HAVING_085](crates/bench/sqlite_parity/cases/SQLITE_PARITY_592_AGG_GROUP_HAVING_085.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1617022 | 3249115 | <span style="color:#dc2626">-8.30%</span> |
| 506 | [00918 CONSTRAINT_FK_SAVEPOINT_051](crates/bench/sqlite_parity/cases/SQLITE_PARITY_918_CONSTRAINT_FK_SAVEPOINT_051.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1643323 | 3248834 | <span style="color:#dc2626">-8.29%</span> |
| 507 | [00605 AGG_GROUP_HAVING_098](crates/bench/sqlite_parity/cases/SQLITE_PARITY_605_AGG_GROUP_HAVING_098.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1605020 | 3248543 | <span style="color:#dc2626">-8.28%</span> |
| 508 | [00594 AGG_GROUP_HAVING_087](crates/bench/sqlite_parity/cases/SQLITE_PARITY_594_AGG_GROUP_HAVING_087.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1928583 | 3247982 | <span style="color:#dc2626">-8.27%</span> |
| 509 | [00931 CONSTRAINT_FK_SAVEPOINT_064](crates/bench/sqlite_parity/cases/SQLITE_PARITY_931_CONSTRAINT_FK_SAVEPOINT_064.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1816470 | 3247842 | <span style="color:#dc2626">-8.26%</span> |
| 510 | [00450 DML_WHERE_ORDER_LIMIT_063](crates/bench/sqlite_parity/cases/SQLITE_PARITY_450_DML_WHERE_ORDER_LIMIT_063.rs) | P1 | memory | GEN_SQL_DML | 1562049 | 3247772 | <span style="color:#dc2626">-8.26%</span> |
| 511 | [00523 AGG_GROUP_HAVING_016](crates/bench/sqlite_parity/cases/SQLITE_PARITY_523_AGG_GROUP_HAVING_016.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1629687 | 3247722 | <span style="color:#dc2626">-8.26%</span> |
| 512 | [00924 CONSTRAINT_FK_SAVEPOINT_057](crates/bench/sqlite_parity/cases/SQLITE_PARITY_924_CONSTRAINT_FK_SAVEPOINT_057.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1650998 | 3247471 | <span style="color:#dc2626">-8.25%</span> |
| 513 | [00424 DML_WHERE_ORDER_LIMIT_037](crates/bench/sqlite_parity/cases/SQLITE_PARITY_424_DML_WHERE_ORDER_LIMIT_037.rs) | P1 | memory | GEN_SQL_DML | 1730548 | 3247432 | <span style="color:#dc2626">-8.25%</span> |
| 514 | [00314 SCALAR_STRING_022](crates/bench/sqlite_parity/cases/SQLITE_PARITY_314_SCALAR_STRING_022.rs) | P1 | memory | GEN_SQL_SCALAR | 1394251 | 3246540 | <span style="color:#dc2626">-8.22%</span> |
| 515 | [00518 AGG_GROUP_HAVING_011](crates/bench/sqlite_parity/cases/SQLITE_PARITY_518_AGG_GROUP_HAVING_011.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1761436 | 3246380 | <span style="color:#dc2626">-8.21%</span> |
| 516 | [00443 DML_WHERE_ORDER_LIMIT_056](crates/bench/sqlite_parity/cases/SQLITE_PARITY_443_DML_WHERE_ORDER_LIMIT_056.rs) | P1 | memory | GEN_SQL_DML | 1496344 | 3246068 | <span style="color:#dc2626">-8.20%</span> |
| 517 | [00555 AGG_GROUP_HAVING_048](crates/bench/sqlite_parity/cases/SQLITE_PARITY_555_AGG_GROUP_HAVING_048.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1638835 | 3245838 | <span style="color:#dc2626">-8.19%</span> |
| 518 | [01028 JSON_EXTRACT_SET_021](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1028_JSON_EXTRACT_SET_021.rs) | P2 | memory | GEN_SQL_JSON | 1945134 | 3245117 | <span style="color:#dc2626">-8.17%</span> |
| 519 | [00903 CONSTRAINT_FK_SAVEPOINT_036](crates/bench/sqlite_parity/cases/SQLITE_PARITY_903_CONSTRAINT_FK_SAVEPOINT_036.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1648463 | 3245097 | <span style="color:#dc2626">-8.17%</span> |
| 520 | [00938 CONSTRAINT_FK_SAVEPOINT_071](crates/bench/sqlite_parity/cases/SQLITE_PARITY_938_CONSTRAINT_FK_SAVEPOINT_071.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1763991 | 3243413 | <span style="color:#dc2626">-8.11%</span> |
| 521 | [00441 DML_WHERE_ORDER_LIMIT_054](crates/bench/sqlite_parity/cases/SQLITE_PARITY_441_DML_WHERE_ORDER_LIMIT_054.rs) | P1 | memory | GEN_SQL_DML | 1499901 | 3241741 | <span style="color:#dc2626">-8.06%</span> |
| 522 | [00570 AGG_GROUP_HAVING_063](crates/bench/sqlite_parity/cases/SQLITE_PARITY_570_AGG_GROUP_HAVING_063.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1913525 | 3240037 | <span style="color:#dc2626">-8.00%</span> |
| 523 | [00546 AGG_GROUP_HAVING_039](crates/bench/sqlite_parity/cases/SQLITE_PARITY_546_AGG_GROUP_HAVING_039.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1637071 | 3237493 | <span style="color:#dc2626">-7.92%</span> |
| 524 | [01011 JSON_EXTRACT_SET_004](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1011_JSON_EXTRACT_SET_004.rs) | P2 | memory | GEN_SQL_JSON | 1569603 | 3237302 | <span style="color:#dc2626">-7.91%</span> |
| 525 | [00887 CONSTRAINT_FK_SAVEPOINT_020](crates/bench/sqlite_parity/cases/SQLITE_PARITY_887_CONSTRAINT_FK_SAVEPOINT_020.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1674913 | 3236650 | <span style="color:#dc2626">-7.89%</span> |
| 526 | [00055 JOINS_RIGHT_FULL_OUTER](crates/bench/sqlite_parity/cases/SQLITE_PARITY_055_JOINS_RIGHT_FULL_OUTER.rs) | P0 | memory | SQL_JOIN | 2323812 | 3233405 | <span style="color:#dc2626">-7.78%</span> |
| 527 | [00535 AGG_GROUP_HAVING_028](crates/bench/sqlite_parity/cases/SQLITE_PARITY_535_AGG_GROUP_HAVING_028.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1728855 | 3233314 | <span style="color:#dc2626">-7.78%</span> |
| 528 | [00913 CONSTRAINT_FK_SAVEPOINT_046](crates/bench/sqlite_parity/cases/SQLITE_PARITY_913_CONSTRAINT_FK_SAVEPOINT_046.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2085340 | 3233044 | <span style="color:#dc2626">-7.77%</span> |
| 529 | [00528 AGG_GROUP_HAVING_021](crates/bench/sqlite_parity/cases/SQLITE_PARITY_528_AGG_GROUP_HAVING_021.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1861316 | 3232012 | <span style="color:#dc2626">-7.73%</span> |
| 530 | [00581 AGG_GROUP_HAVING_074](crates/bench/sqlite_parity/cases/SQLITE_PARITY_581_AGG_GROUP_HAVING_074.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1722683 | 3231712 | <span style="color:#dc2626">-7.72%</span> |
| 531 | [00550 AGG_GROUP_HAVING_043](crates/bench/sqlite_parity/cases/SQLITE_PARITY_550_AGG_GROUP_HAVING_043.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1655616 | 3230810 | <span style="color:#dc2626">-7.69%</span> |
| 532 | [00548 AGG_GROUP_HAVING_041](crates/bench/sqlite_parity/cases/SQLITE_PARITY_548_AGG_GROUP_HAVING_041.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1593548 | 3230609 | <span style="color:#dc2626">-7.69%</span> |
| 533 | [00510 AGG_GROUP_HAVING_003](crates/bench/sqlite_parity/cases/SQLITE_PARITY_510_AGG_GROUP_HAVING_003.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1700210 | 3230550 | <span style="color:#dc2626">-7.69%</span> |
| 534 | [00588 AGG_GROUP_HAVING_081](crates/bench/sqlite_parity/cases/SQLITE_PARITY_588_AGG_GROUP_HAVING_081.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1630639 | 3230409 | <span style="color:#dc2626">-7.68%</span> |
| 535 | [00533 AGG_GROUP_HAVING_026](crates/bench/sqlite_parity/cases/SQLITE_PARITY_533_AGG_GROUP_HAVING_026.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1684070 | 3230178 | <span style="color:#dc2626">-7.67%</span> |
| 536 | [00589 AGG_GROUP_HAVING_082](crates/bench/sqlite_parity/cases/SQLITE_PARITY_589_AGG_GROUP_HAVING_082.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1663121 | 3230089 | <span style="color:#dc2626">-7.67%</span> |
| 537 | [00537 AGG_GROUP_HAVING_030](crates/bench/sqlite_parity/cases/SQLITE_PARITY_537_AGG_GROUP_HAVING_030.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1676465 | 3229998 | <span style="color:#dc2626">-7.67%</span> |
| 538 | [00499 DML_WHERE_ORDER_LIMIT_112](crates/bench/sqlite_parity/cases/SQLITE_PARITY_499_DML_WHERE_ORDER_LIMIT_112.rs) | P1 | memory | GEN_SQL_DML | 1757329 | 3229407 | <span style="color:#dc2626">-7.65%</span> |
| 539 | [01031 JSON_EXTRACT_SET_024](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1031_JSON_EXTRACT_SET_024.rs) | P2 | memory | GEN_SQL_JSON | 1571747 | 3228745 | <span style="color:#dc2626">-7.62%</span> |
| 540 | [00836 WINDOW_PARTITION_SUM_049](crates/bench/sqlite_parity/cases/SQLITE_PARITY_836_WINDOW_PARTITION_SUM_049.rs) | P2 | memory | GEN_SQL_WINDOW | 1580484 | 3228225 | <span style="color:#dc2626">-7.61%</span> |
| 541 | [00927 CONSTRAINT_FK_SAVEPOINT_060](crates/bench/sqlite_parity/cases/SQLITE_PARITY_927_CONSTRAINT_FK_SAVEPOINT_060.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1660195 | 3227564 | <span style="color:#dc2626">-7.59%</span> |
| 542 | [00900 CONSTRAINT_FK_SAVEPOINT_033](crates/bench/sqlite_parity/cases/SQLITE_PARITY_900_CONSTRAINT_FK_SAVEPOINT_033.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1823515 | 3227163 | <span style="color:#dc2626">-7.57%</span> |
| 543 | [00580 AGG_GROUP_HAVING_073](crates/bench/sqlite_parity/cases/SQLITE_PARITY_580_AGG_GROUP_HAVING_073.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1608607 | 3225319 | <span style="color:#dc2626">-7.51%</span> |
| 544 | [00734 CTE_RECURSIVE_MATRIX_027](crates/bench/sqlite_parity/cases/SQLITE_PARITY_734_CTE_RECURSIVE_MATRIX_027.rs) | P1 | memory | GEN_SQL_CTE | 1586916 | 3223696 | <span style="color:#dc2626">-7.46%</span> |
| 545 | [00882 CONSTRAINT_FK_SAVEPOINT_015](crates/bench/sqlite_parity/cases/SQLITE_PARITY_882_CONSTRAINT_FK_SAVEPOINT_015.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1664313 | 3221492 | <span style="color:#dc2626">-7.38%</span> |
| 546 | [00920 CONSTRAINT_FK_SAVEPOINT_053](crates/bench/sqlite_parity/cases/SQLITE_PARITY_920_CONSTRAINT_FK_SAVEPOINT_053.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1640848 | 3221382 | <span style="color:#dc2626">-7.38%</span> |
| 547 | [00496 DML_WHERE_ORDER_LIMIT_109](crates/bench/sqlite_parity/cases/SQLITE_PARITY_496_DML_WHERE_ORDER_LIMIT_109.rs) | P1 | memory | GEN_SQL_DML | 1601564 | 3219158 | <span style="color:#dc2626">-7.31%</span> |
| 548 | [00522 AGG_GROUP_HAVING_015](crates/bench/sqlite_parity/cases/SQLITE_PARITY_522_AGG_GROUP_HAVING_015.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1676335 | 3219078 | <span style="color:#dc2626">-7.30%</span> |
| 549 | [00934 CONSTRAINT_FK_SAVEPOINT_067](crates/bench/sqlite_parity/cases/SQLITE_PARITY_934_CONSTRAINT_FK_SAVEPOINT_067.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1618555 | 3218687 | <span style="color:#dc2626">-7.29%</span> |
| 550 | [00925 CONSTRAINT_FK_SAVEPOINT_058](crates/bench/sqlite_parity/cases/SQLITE_PARITY_925_CONSTRAINT_FK_SAVEPOINT_058.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1635669 | 3214919 | <span style="color:#dc2626">-7.16%</span> |
| 551 | [00564 AGG_GROUP_HAVING_057](crates/bench/sqlite_parity/cases/SQLITE_PARITY_564_AGG_GROUP_HAVING_057.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1589020 | 3213938 | <span style="color:#dc2626">-7.13%</span> |
| 552 | [00868 CONSTRAINT_FK_SAVEPOINT_001](crates/bench/sqlite_parity/cases/SQLITE_PARITY_868_CONSTRAINT_FK_SAVEPOINT_001.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1592697 | 3213748 | <span style="color:#dc2626">-7.12%</span> |
| 553 | [00873 CONSTRAINT_FK_SAVEPOINT_006](crates/bench/sqlite_parity/cases/SQLITE_PARITY_873_CONSTRAINT_FK_SAVEPOINT_006.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2518230 | 3213497 | <span style="color:#dc2626">-7.12%</span> |
| 554 | [01025 JSON_EXTRACT_SET_018](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1025_JSON_EXTRACT_SET_018.rs) | P2 | memory | GEN_SQL_JSON | 1488810 | 3213177 | <span style="color:#dc2626">-7.11%</span> |
| 555 | [00552 AGG_GROUP_HAVING_045](crates/bench/sqlite_parity/cases/SQLITE_PARITY_552_AGG_GROUP_HAVING_045.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1647921 | 3212926 | <span style="color:#dc2626">-7.10%</span> |
| 556 | [00512 AGG_GROUP_HAVING_005](crates/bench/sqlite_parity/cases/SQLITE_PARITY_512_AGG_GROUP_HAVING_005.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1655115 | 3210772 | <span style="color:#dc2626">-7.03%</span> |
| 557 | [00511 AGG_GROUP_HAVING_004](crates/bench/sqlite_parity/cases/SQLITE_PARITY_511_AGG_GROUP_HAVING_004.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1673771 | 3210511 | <span style="color:#dc2626">-7.02%</span> |
| 558 | [00738 CTE_RECURSIVE_MATRIX_031](crates/bench/sqlite_parity/cases/SQLITE_PARITY_738_CTE_RECURSIVE_MATRIX_031.rs) | P1 | memory | GEN_SQL_CTE | 1520941 | 3210221 | <span style="color:#dc2626">-7.01%</span> |
| 559 | [00514 AGG_GROUP_HAVING_007](crates/bench/sqlite_parity/cases/SQLITE_PARITY_514_AGG_GROUP_HAVING_007.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1639325 | 3208438 | <span style="color:#dc2626">-6.95%</span> |
| 560 | [00553 AGG_GROUP_HAVING_046](crates/bench/sqlite_parity/cases/SQLITE_PARITY_553_AGG_GROUP_HAVING_046.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1652831 | 3207635 | <span style="color:#dc2626">-6.92%</span> |
| 561 | [00706 JOIN_SUBQUERY_EXISTS_099](crates/bench/sqlite_parity/cases/SQLITE_PARITY_706_JOIN_SUBQUERY_EXISTS_099.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1677117 | 3206684 | <span style="color:#dc2626">-6.89%</span> |
| 562 | [00886 CONSTRAINT_FK_SAVEPOINT_019](crates/bench/sqlite_parity/cases/SQLITE_PARITY_886_CONSTRAINT_FK_SAVEPOINT_019.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1878728 | 3206403 | <span style="color:#dc2626">-6.88%</span> |
| 563 | [00914 CONSTRAINT_FK_SAVEPOINT_047](crates/bench/sqlite_parity/cases/SQLITE_PARITY_914_CONSTRAINT_FK_SAVEPOINT_047.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1776344 | 3206273 | <span style="color:#dc2626">-6.88%</span> |
| 564 | [00911 CONSTRAINT_FK_SAVEPOINT_044](crates/bench/sqlite_parity/cases/SQLITE_PARITY_911_CONSTRAINT_FK_SAVEPOINT_044.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1621522 | 3205933 | <span style="color:#dc2626">-6.86%</span> |
| 565 | [01039 JSON_EXTRACT_SET_032](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1039_JSON_EXTRACT_SET_032.rs) | P2 | memory | GEN_SQL_JSON | 1554204 | 3205442 | <span style="color:#dc2626">-6.85%</span> |
| 566 | [00531 AGG_GROUP_HAVING_024](crates/bench/sqlite_parity/cases/SQLITE_PARITY_531_AGG_GROUP_HAVING_024.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2024886 | 3205382 | <span style="color:#dc2626">-6.85%</span> |
| 567 | [00883 CONSTRAINT_FK_SAVEPOINT_016](crates/bench/sqlite_parity/cases/SQLITE_PARITY_883_CONSTRAINT_FK_SAVEPOINT_016.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1580293 | 3204460 | <span style="color:#dc2626">-6.82%</span> |
| 568 | [00577 AGG_GROUP_HAVING_070](crates/bench/sqlite_parity/cases/SQLITE_PARITY_577_AGG_GROUP_HAVING_070.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1691744 | 3204420 | <span style="color:#dc2626">-6.81%</span> |
| 569 | [00536 AGG_GROUP_HAVING_029](crates/bench/sqlite_parity/cases/SQLITE_PARITY_536_AGG_GROUP_HAVING_029.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1783498 | 3203668 | <span style="color:#dc2626">-6.79%</span> |
| 570 | [00737 CTE_RECURSIVE_MATRIX_030](crates/bench/sqlite_parity/cases/SQLITE_PARITY_737_CTE_RECURSIVE_MATRIX_030.rs) | P1 | memory | GEN_SQL_CTE | 1513727 | 3203598 | <span style="color:#dc2626">-6.79%</span> |
| 571 | [00876 CONSTRAINT_FK_SAVEPOINT_009](crates/bench/sqlite_parity/cases/SQLITE_PARITY_876_CONSTRAINT_FK_SAVEPOINT_009.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1685603 | 3203388 | <span style="color:#dc2626">-6.78%</span> |
| 572 | [01091 INDEX_SCHEMA_PRAGMA_024](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1091_INDEX_SCHEMA_PRAGMA_024.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 2009747 | 3201865 | <span style="color:#dc2626">-6.73%</span> |
| 573 | [00542 AGG_GROUP_HAVING_035](crates/bench/sqlite_parity/cases/SQLITE_PARITY_542_AGG_GROUP_HAVING_035.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1708607 | 3201144 | <span style="color:#dc2626">-6.70%</span> |
| 574 | [00869 CONSTRAINT_FK_SAVEPOINT_002](crates/bench/sqlite_parity/cases/SQLITE_PARITY_869_CONSTRAINT_FK_SAVEPOINT_002.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1668270 | 3200352 | <span style="color:#dc2626">-6.68%</span> |
| 575 | [00278 SCALAR_STRING_013](crates/bench/sqlite_parity/cases/SQLITE_PARITY_278_SCALAR_STRING_013.rs) | P1 | memory | GEN_SQL_SCALAR | 1616953 | 3199851 | <span style="color:#dc2626">-6.66%</span> |
| 576 | [00567 AGG_GROUP_HAVING_060](crates/bench/sqlite_parity/cases/SQLITE_PARITY_567_AGG_GROUP_HAVING_060.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1764271 | 3197697 | <span style="color:#dc2626">-6.59%</span> |
| 577 | [00936 CONSTRAINT_FK_SAVEPOINT_069](crates/bench/sqlite_parity/cases/SQLITE_PARITY_936_CONSTRAINT_FK_SAVEPOINT_069.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1699179 | 3196334 | <span style="color:#dc2626">-6.54%</span> |
| 578 | [00539 AGG_GROUP_HAVING_032](crates/bench/sqlite_parity/cases/SQLITE_PARITY_539_AGG_GROUP_HAVING_032.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1785051 | 3192888 | <span style="color:#dc2626">-6.43%</span> |
| 579 | [00758 CTE_RECURSIVE_MATRIX_051](crates/bench/sqlite_parity/cases/SQLITE_PARITY_758_CTE_RECURSIVE_MATRIX_051.rs) | P1 | memory | GEN_SQL_CTE | 1490944 | 3192478 | <span style="color:#dc2626">-6.42%</span> |
| 580 | [00578 AGG_GROUP_HAVING_071](crates/bench/sqlite_parity/cases/SQLITE_PARITY_578_AGG_GROUP_HAVING_071.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1568131 | 3192467 | <span style="color:#dc2626">-6.42%</span> |
| 581 | [00289 SCALAR_CAST_TYPEOF_016](crates/bench/sqlite_parity/cases/SQLITE_PARITY_289_SCALAR_CAST_TYPEOF_016.rs) | P1 | memory | GEN_SQL_SCALAR | 1922271 | 3190063 | <span style="color:#dc2626">-6.34%</span> |
| 582 | [00921 CONSTRAINT_FK_SAVEPOINT_054](crates/bench/sqlite_parity/cases/SQLITE_PARITY_921_CONSTRAINT_FK_SAVEPOINT_054.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1676656 | 3189742 | <span style="color:#dc2626">-6.32%</span> |
| 583 | [00551 AGG_GROUP_HAVING_044](crates/bench/sqlite_parity/cases/SQLITE_PARITY_551_AGG_GROUP_HAVING_044.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1655796 | 3189732 | <span style="color:#dc2626">-6.32%</span> |
| 584 | [00442 DML_WHERE_ORDER_LIMIT_055](crates/bench/sqlite_parity/cases/SQLITE_PARITY_442_DML_WHERE_ORDER_LIMIT_055.rs) | P1 | memory | GEN_SQL_DML | 1509469 | 3188249 | <span style="color:#dc2626">-6.27%</span> |
| 585 | [00893 CONSTRAINT_FK_SAVEPOINT_026](crates/bench/sqlite_parity/cases/SQLITE_PARITY_893_CONSTRAINT_FK_SAVEPOINT_026.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1613096 | 3187357 | <span style="color:#dc2626">-6.25%</span> |
| 586 | [00749 CTE_RECURSIVE_MATRIX_042](crates/bench/sqlite_parity/cases/SQLITE_PARITY_749_CTE_RECURSIVE_MATRIX_042.rs) | P1 | memory | GEN_SQL_CTE | 1455818 | 3187188 | <span style="color:#dc2626">-6.24%</span> |
| 587 | [01022 JSON_EXTRACT_SET_015](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1022_JSON_EXTRACT_SET_015.rs) | P2 | memory | GEN_SQL_JSON | 1565365 | 3186756 | <span style="color:#dc2626">-6.23%</span> |
| 588 | [00933 CONSTRAINT_FK_SAVEPOINT_066](crates/bench/sqlite_parity/cases/SQLITE_PARITY_933_CONSTRAINT_FK_SAVEPOINT_066.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1626450 | 3184382 | <span style="color:#dc2626">-6.15%</span> |
| 589 | [00944 CONSTRAINT_FK_SAVEPOINT_077](crates/bench/sqlite_parity/cases/SQLITE_PARITY_944_CONSTRAINT_FK_SAVEPOINT_077.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1702905 | 3183811 | <span style="color:#dc2626">-6.13%</span> |
| 590 | [00012 INSERT_DEFAULT_VALUES](crates/bench/sqlite_parity/cases/SQLITE_PARITY_012_INSERT_DEFAULT_VALUES.rs) | P0 | memory | SQL_INSERT | 1978658 | 3183110 | <span style="color:#dc2626">-6.10%</span> |
| 591 | [00889 CONSTRAINT_FK_SAVEPOINT_022](crates/bench/sqlite_parity/cases/SQLITE_PARITY_889_CONSTRAINT_FK_SAVEPOINT_022.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1699830 | 3182308 | <span style="color:#dc2626">-6.08%</span> |
| 592 | [01009 JSON_EXTRACT_SET_002](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1009_JSON_EXTRACT_SET_002.rs) | P2 | memory | GEN_SQL_JSON | 1552241 | 3181306 | <span style="color:#dc2626">-6.04%</span> |
| 593 | [00945 CONSTRAINT_FK_SAVEPOINT_078](crates/bench/sqlite_parity/cases/SQLITE_PARITY_945_CONSTRAINT_FK_SAVEPOINT_078.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1638894 | 3178831 | <span style="color:#dc2626">-5.96%</span> |
| 594 | [00805 WINDOW_PARTITION_SUM_018](crates/bench/sqlite_parity/cases/SQLITE_PARITY_805_WINDOW_PARTITION_SUM_018.rs) | P2 | memory | GEN_SQL_WINDOW | 1531511 | 3177710 | <span style="color:#dc2626">-5.92%</span> |
| 595 | [00815 WINDOW_PARTITION_SUM_028](crates/bench/sqlite_parity/cases/SQLITE_PARITY_815_WINDOW_PARTITION_SUM_028.rs) | P2 | memory | GEN_SQL_WINDOW | 1939715 | 3177068 | <span style="color:#dc2626">-5.90%</span> |
| 596 | [00939 CONSTRAINT_FK_SAVEPOINT_072](crates/bench/sqlite_parity/cases/SQLITE_PARITY_939_CONSTRAINT_FK_SAVEPOINT_072.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1654755 | 3176427 | <span style="color:#dc2626">-5.88%</span> |
| 597 | [00935 CONSTRAINT_FK_SAVEPOINT_068](crates/bench/sqlite_parity/cases/SQLITE_PARITY_935_CONSTRAINT_FK_SAVEPOINT_068.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1659263 | 3174634 | <span style="color:#dc2626">-5.82%</span> |
| 598 | [00543 AGG_GROUP_HAVING_036](crates/bench/sqlite_parity/cases/SQLITE_PARITY_543_AGG_GROUP_HAVING_036.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1582438 | 3173801 | <span style="color:#dc2626">-5.79%</span> |
| 599 | [00562 AGG_GROUP_HAVING_055](crates/bench/sqlite_parity/cases/SQLITE_PARITY_562_AGG_GROUP_HAVING_055.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1900260 | 3173672 | <span style="color:#dc2626">-5.79%</span> |
| 600 | [00888 CONSTRAINT_FK_SAVEPOINT_021](crates/bench/sqlite_parity/cases/SQLITE_PARITY_888_CONSTRAINT_FK_SAVEPOINT_021.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1886403 | 3173601 | <span style="color:#dc2626">-5.79%</span> |
| 601 | [01012 JSON_EXTRACT_SET_005](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1012_JSON_EXTRACT_SET_005.rs) | P2 | memory | GEN_SQL_JSON | 1657640 | 3172309 | <span style="color:#dc2626">-5.74%</span> |
| 602 | [00513 AGG_GROUP_HAVING_006](crates/bench/sqlite_parity/cases/SQLITE_PARITY_513_AGG_GROUP_HAVING_006.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1658913 | 3170937 | <span style="color:#dc2626">-5.70%</span> |
| 603 | [01040 JSON_EXTRACT_SET_033](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1040_JSON_EXTRACT_SET_033.rs) | P2 | memory | GEN_SQL_JSON | 1531952 | 3170876 | <span style="color:#dc2626">-5.70%</span> |
| 604 | [00811 WINDOW_PARTITION_SUM_024](crates/bench/sqlite_parity/cases/SQLITE_PARITY_811_WINDOW_PARTITION_SUM_024.rs) | P2 | memory | GEN_SQL_WINDOW | 1590142 | 3170666 | <span style="color:#dc2626">-5.69%</span> |
| 605 | [00667 JOIN_SUBQUERY_EXISTS_060](crates/bench/sqlite_parity/cases/SQLITE_PARITY_667_JOIN_SUBQUERY_EXISTS_060.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1619348 | 3169634 | <span style="color:#dc2626">-5.65%</span> |
| 606 | [00549 AGG_GROUP_HAVING_042](crates/bench/sqlite_parity/cases/SQLITE_PARITY_549_AGG_GROUP_HAVING_042.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1669723 | 3169533 | <span style="color:#dc2626">-5.65%</span> |
| 607 | [01030 JSON_EXTRACT_SET_023](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1030_JSON_EXTRACT_SET_023.rs) | P2 | memory | GEN_SQL_JSON | 1538835 | 3167560 | <span style="color:#dc2626">-5.59%</span> |
| 608 | [01008 JSON_EXTRACT_SET_001](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1008_JSON_EXTRACT_SET_001.rs) | P2 | memory | GEN_SQL_JSON | 1541499 | 3167270 | <span style="color:#dc2626">-5.58%</span> |
| 609 | [01055 JSON_EXTRACT_SET_048](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1055_JSON_EXTRACT_SET_048.rs) | P2 | memory | GEN_SQL_JSON | 1510101 | 3166868 | <span style="color:#dc2626">-5.56%</span> |
| 610 | [00716 CTE_RECURSIVE_MATRIX_009](crates/bench/sqlite_parity/cases/SQLITE_PARITY_716_CTE_RECURSIVE_MATRIX_009.rs) | P1 | memory | GEN_SQL_CTE | 2433249 | 3166017 | <span style="color:#dc2626">-5.53%</span> |
| 611 | [00569 AGG_GROUP_HAVING_062](crates/bench/sqlite_parity/cases/SQLITE_PARITY_569_AGG_GROUP_HAVING_062.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1684781 | 3165977 | <span style="color:#dc2626">-5.53%</span> |
| 612 | [01027 JSON_EXTRACT_SET_020](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1027_JSON_EXTRACT_SET_020.rs) | P2 | memory | GEN_SQL_JSON | 1559504 | 3165957 | <span style="color:#dc2626">-5.53%</span> |
| 613 | [00932 CONSTRAINT_FK_SAVEPOINT_065](crates/bench/sqlite_parity/cases/SQLITE_PARITY_932_CONSTRAINT_FK_SAVEPOINT_065.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1658331 | 3165346 | <span style="color:#dc2626">-5.51%</span> |
| 614 | [00910 CONSTRAINT_FK_SAVEPOINT_043](crates/bench/sqlite_parity/cases/SQLITE_PARITY_910_CONSTRAINT_FK_SAVEPOINT_043.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1654644 | 3163712 | <span style="color:#dc2626">-5.46%</span> |
| 615 | [00769 CTE_RECURSIVE_MATRIX_062](crates/bench/sqlite_parity/cases/SQLITE_PARITY_769_CTE_RECURSIVE_MATRIX_062.rs) | P1 | memory | GEN_SQL_CTE | 1607294 | 3163623 | <span style="color:#dc2626">-5.45%</span> |
| 616 | [01014 JSON_EXTRACT_SET_007](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1014_JSON_EXTRACT_SET_007.rs) | P2 | memory | GEN_SQL_JSON | 1563001 | 3162701 | <span style="color:#dc2626">-5.42%</span> |
| 617 | [00790 WINDOW_PARTITION_SUM_003](crates/bench/sqlite_parity/cases/SQLITE_PARITY_790_WINDOW_PARTITION_SUM_003.rs) | P2 | memory | GEN_SQL_WINDOW | 1523787 | 3162660 | <span style="color:#dc2626">-5.42%</span> |
| 618 | [00890 CONSTRAINT_FK_SAVEPOINT_023](crates/bench/sqlite_parity/cases/SQLITE_PARITY_890_CONSTRAINT_FK_SAVEPOINT_023.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1614338 | 3161599 | <span style="color:#dc2626">-5.39%</span> |
| 619 | [00275 SCALAR_NULL_COALESCE_012](crates/bench/sqlite_parity/cases/SQLITE_PARITY_275_SCALAR_NULL_COALESCE_012.rs) | P1 | memory | GEN_SQL_SCALAR | 2952443 | 3160387 | <span style="color:#dc2626">-5.35%</span> |
| 620 | [00892 CONSTRAINT_FK_SAVEPOINT_025](crates/bench/sqlite_parity/cases/SQLITE_PARITY_892_CONSTRAINT_FK_SAVEPOINT_025.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1704188 | 3158974 | <span style="color:#dc2626">-5.30%</span> |
| 621 | [00740 CTE_RECURSIVE_MATRIX_033](crates/bench/sqlite_parity/cases/SQLITE_PARITY_740_CTE_RECURSIVE_MATRIX_033.rs) | P1 | memory | GEN_SQL_CTE | 2100609 | 3158643 | <span style="color:#dc2626">-5.29%</span> |
| 622 | [00923 CONSTRAINT_FK_SAVEPOINT_056](crates/bench/sqlite_parity/cases/SQLITE_PARITY_923_CONSTRAINT_FK_SAVEPOINT_056.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1625809 | 3156138 | <span style="color:#dc2626">-5.20%</span> |
| 623 | [00810 WINDOW_PARTITION_SUM_023](crates/bench/sqlite_parity/cases/SQLITE_PARITY_810_WINDOW_PARTITION_SUM_023.rs) | P2 | memory | GEN_SQL_WINDOW | 1929415 | 3155697 | <span style="color:#dc2626">-5.19%</span> |
| 624 | [00907 CONSTRAINT_FK_SAVEPOINT_040](crates/bench/sqlite_parity/cases/SQLITE_PARITY_907_CONSTRAINT_FK_SAVEPOINT_040.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1639837 | 3154776 | <span style="color:#dc2626">-5.16%</span> |
| 625 | [00743 CTE_RECURSIVE_MATRIX_036](crates/bench/sqlite_parity/cases/SQLITE_PARITY_743_CTE_RECURSIVE_MATRIX_036.rs) | P1 | memory | GEN_SQL_CTE | 1601132 | 3153182 | <span style="color:#dc2626">-5.11%</span> |
| 626 | [01038 JSON_EXTRACT_SET_031](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1038_JSON_EXTRACT_SET_031.rs) | P2 | memory | GEN_SQL_JSON | 1551799 | 3152572 | <span style="color:#dc2626">-5.09%</span> |
| 627 | [00367 SCALAR_NULL_COALESCE_035](crates/bench/sqlite_parity/cases/SQLITE_PARITY_367_SCALAR_NULL_COALESCE_035.rs) | P1 | memory | GEN_SQL_SCALAR | 1439106 | 3151610 | <span style="color:#dc2626">-5.05%</span> |
| 628 | [01023 JSON_EXTRACT_SET_016](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1023_JSON_EXTRACT_SET_016.rs) | P2 | memory | GEN_SQL_JSON | 1552591 | 3151189 | <span style="color:#dc2626">-5.04%</span> |
| 629 | [00601 AGG_GROUP_HAVING_094](crates/bench/sqlite_parity/cases/SQLITE_PARITY_601_AGG_GROUP_HAVING_094.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1640227 | 3150909 | <span style="color:#dc2626">-5.03%</span> |
| 630 | [00317 SCALAR_CAST_TYPEOF_023](crates/bench/sqlite_parity/cases/SQLITE_PARITY_317_SCALAR_CAST_TYPEOF_023.rs) | P1 | memory | GEN_SQL_SCALAR | 1426482 | 3150769 | <span style="color:#dc2626">-5.03%</span> |
| 631 | [00385 SCALAR_CAST_TYPEOF_040](crates/bench/sqlite_parity/cases/SQLITE_PARITY_385_SCALAR_CAST_TYPEOF_040.rs) | P1 | memory | GEN_SQL_SCALAR | 1677358 | 3149816 | <span style="color:#f97316">-4.99%</span> |
| 632 | [01063 JSON_EXTRACT_SET_056](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1063_JSON_EXTRACT_SET_056.rs) | P2 | memory | GEN_SQL_JSON | 1572809 | 3149636 | <span style="color:#f97316">-4.99%</span> |
| 633 | [00909 CONSTRAINT_FK_SAVEPOINT_042](crates/bench/sqlite_parity/cases/SQLITE_PARITY_909_CONSTRAINT_FK_SAVEPOINT_042.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1703607 | 3149366 | <span style="color:#f97316">-4.98%</span> |
| 634 | [01029 JSON_EXTRACT_SET_022](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1029_JSON_EXTRACT_SET_022.rs) | P2 | memory | GEN_SQL_JSON | 1951977 | 3148964 | <span style="color:#f97316">-4.97%</span> |
| 635 | [00818 WINDOW_PARTITION_SUM_031](crates/bench/sqlite_parity/cases/SQLITE_PARITY_818_WINDOW_PARTITION_SUM_031.rs) | P2 | memory | GEN_SQL_WINDOW | 1834835 | 3148363 | <span style="color:#f97316">-4.95%</span> |
| 636 | [01026 JSON_EXTRACT_SET_019](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1026_JSON_EXTRACT_SET_019.rs) | P2 | memory | GEN_SQL_JSON | 1560376 | 3147321 | <span style="color:#f97316">-4.91%</span> |
| 637 | [00095 CREATE_VIRTUAL_TABLE_RTREE_OPTIONAL](crates/bench/sqlite_parity/cases/SQLITE_PARITY_095_CREATE_VIRTUAL_TABLE_RTREE_OPTIONAL.rs) | P2 | memory | SQL_VIRTUAL_TABLE_OPTIONAL | 1775814 | 3146430 | <span style="color:#f97316">-4.88%</span> |
| 638 | [01037 JSON_EXTRACT_SET_030](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1037_JSON_EXTRACT_SET_030.rs) | P2 | memory | GEN_SQL_JSON | 1573450 | 3143734 | <span style="color:#f97316">-4.79%</span> |
| 639 | [00044 ANALYZE_SQLITE_STAT1](crates/bench/sqlite_parity/cases/SQLITE_PARITY_044_ANALYZE_SQLITE_STAT1.rs) | P0 | memory | SQL_ANALYZE | 1843092 | 3140289 | <span style="color:#f97316">-4.68%</span> |
| 640 | [00019 REPLACE_INTO](crates/bench/sqlite_parity/cases/SQLITE_PARITY_019_REPLACE_INTO.rs) | P0 | memory | SQL_REPLACE | 1575855 | 3137332 | <span style="color:#f97316">-4.58%</span> |
| 641 | [00284 SCALAR_ARITH_015](crates/bench/sqlite_parity/cases/SQLITE_PARITY_284_SCALAR_ARITH_015.rs) | P1 | memory | GEN_SQL_SCALAR | 1407265 | 3137313 | <span style="color:#f97316">-4.58%</span> |
| 642 | [00719 CTE_RECURSIVE_MATRIX_012](crates/bench/sqlite_parity/cases/SQLITE_PARITY_719_CTE_RECURSIVE_MATRIX_012.rs) | P1 | memory | GEN_SQL_CTE | 1547371 | 3136742 | <span style="color:#f97316">-4.56%</span> |
| 643 | [00252 SCALAR_ARITH_007](crates/bench/sqlite_parity/cases/SQLITE_PARITY_252_SCALAR_ARITH_007.rs) | P1 | memory | GEN_SQL_SCALAR | 1441411 | 3134217 | <span style="color:#f97316">-4.47%</span> |
| 644 | [00028 ALTER_TABLE_RENAME](crates/bench/sqlite_parity/cases/SQLITE_PARITY_028_ALTER_TABLE_RENAME.rs) | P0 | memory | SQL_ALTER | 1903406 | 3133877 | <span style="color:#f97316">-4.46%</span> |
| 645 | [00912 CONSTRAINT_FK_SAVEPOINT_045](crates/bench/sqlite_parity/cases/SQLITE_PARITY_912_CONSTRAINT_FK_SAVEPOINT_045.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1663862 | 3132013 | <span style="color:#f97316">-4.40%</span> |
| 646 | [00593 AGG_GROUP_HAVING_086](crates/bench/sqlite_parity/cases/SQLITE_PARITY_593_AGG_GROUP_HAVING_086.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1591886 | 3131983 | <span style="color:#f97316">-4.40%</span> |
| 647 | [01013 JSON_EXTRACT_SET_006](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1013_JSON_EXTRACT_SET_006.rs) | P2 | memory | GEN_SQL_JSON | 1620419 | 3130450 | <span style="color:#f97316">-4.35%</span> |
| 648 | [00362 SCALAR_STRING_034](crates/bench/sqlite_parity/cases/SQLITE_PARITY_362_SCALAR_STRING_034.rs) | P1 | memory | GEN_SQL_SCALAR | 1392257 | 3129709 | <span style="color:#f97316">-4.32%</span> |
| 649 | [00894 CONSTRAINT_FK_SAVEPOINT_027](crates/bench/sqlite_parity/cases/SQLITE_PARITY_894_CONSTRAINT_FK_SAVEPOINT_027.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1742891 | 3129649 | <span style="color:#f97316">-4.32%</span> |
| 650 | [01061 JSON_EXTRACT_SET_054](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1061_JSON_EXTRACT_SET_054.rs) | P2 | memory | GEN_SQL_JSON | 1560316 | 3129247 | <span style="color:#f97316">-4.31%</span> |
| 651 | [00852 WINDOW_PARTITION_SUM_065](crates/bench/sqlite_parity/cases/SQLITE_PARITY_852_WINDOW_PARTITION_SUM_065.rs) | P2 | memory | GEN_SQL_WINDOW | 1657810 | 3128827 | <span style="color:#f97316">-4.29%</span> |
| 652 | [00726 CTE_RECURSIVE_MATRIX_019](crates/bench/sqlite_parity/cases/SQLITE_PARITY_726_CTE_RECURSIVE_MATRIX_019.rs) | P1 | memory | GEN_SQL_CTE | 1557160 | 3126833 | <span style="color:#f97316">-4.23%</span> |
| 653 | [00741 CTE_RECURSIVE_MATRIX_034](crates/bench/sqlite_parity/cases/SQLITE_PARITY_741_CTE_RECURSIVE_MATRIX_034.rs) | P1 | memory | GEN_SQL_CTE | 1645597 | 3124368 | <span style="color:#f97316">-4.15%</span> |
| 654 | [00003 CREATE_TABLE_INSERT_SELECT](crates/bench/sqlite_parity/cases/SQLITE_PARITY_003_CREATE_TABLE_INSERT_SELECT.rs) | P0 | memory | SQL_DDL_DML | 1653212 | 3123347 | <span style="color:#f97316">-4.11%</span> |
| 655 | [00094 FTS5_HIGHLIGHT_OPTIONAL](crates/bench/sqlite_parity/cases/SQLITE_PARITY_094_FTS5_HIGHLIGHT_OPTIONAL.rs) | P2 | memory | SQL_VIRTUAL_TABLE_OPTIONAL | 1840857 | 3122945 | <span style="color:#f97316">-4.10%</span> |
| 656 | [00729 CTE_RECURSIVE_MATRIX_022](crates/bench/sqlite_parity/cases/SQLITE_PARITY_729_CTE_RECURSIVE_MATRIX_022.rs) | P1 | memory | GEN_SQL_CTE | 1570054 | 3122514 | <span style="color:#f97316">-4.08%</span> |
| 657 | [00993 VIEW_TRIGGER_GENERATED_046](crates/bench/sqlite_parity/cases/SQLITE_PARITY_993_VIEW_TRIGGER_GENERATED_046.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1641559 | 3120431 | <span style="color:#f97316">-4.01%</span> |
| 658 | [01016 JSON_EXTRACT_SET_009](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1016_JSON_EXTRACT_SET_009.rs) | P2 | memory | GEN_SQL_JSON | 1584622 | 3120130 | <span style="color:#f97316">-4.00%</span> |
| 659 | [00870 CONSTRAINT_FK_SAVEPOINT_003](crates/bench/sqlite_parity/cases/SQLITE_PARITY_870_CONSTRAINT_FK_SAVEPOINT_003.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1593007 | 3119930 | <span style="color:#f97316">-4.00%</span> |
| 660 | [00908 CONSTRAINT_FK_SAVEPOINT_041](crates/bench/sqlite_parity/cases/SQLITE_PARITY_908_CONSTRAINT_FK_SAVEPOINT_041.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1749344 | 3115942 | <span style="color:#f97316">-3.86%</span> |
| 661 | [00752 CTE_RECURSIVE_MATRIX_045](crates/bench/sqlite_parity/cases/SQLITE_PARITY_752_CTE_RECURSIVE_MATRIX_045.rs) | P1 | memory | GEN_SQL_CTE | 1752509 | 3115843 | <span style="color:#f97316">-3.86%</span> |
| 662 | [00056 SUBQUERIES_EXISTS_IN](crates/bench/sqlite_parity/cases/SQLITE_PARITY_056_SUBQUERIES_EXISTS_IN.rs) | P0 | memory | SQL_SELECT | 1847018 | 3114820 | <span style="color:#f97316">-3.83%</span> |
| 663 | [01060 JSON_EXTRACT_SET_053](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1060_JSON_EXTRACT_SET_053.rs) | P2 | memory | GEN_SQL_JSON | 1536340 | 3113678 | <span style="color:#f97316">-3.79%</span> |
| 664 | [00809 WINDOW_PARTITION_SUM_022](crates/bench/sqlite_parity/cases/SQLITE_PARITY_809_WINDOW_PARTITION_SUM_022.rs) | P2 | memory | GEN_SQL_WINDOW | 1666487 | 3112556 | <span style="color:#f97316">-3.75%</span> |
| 665 | [00078 ON_CONFLICT_ALGORITHMS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_078_ON_CONFLICT_ALGORITHMS.rs) | P0 | memory | SQL_CONFLICT | 1606824 | 3112466 | <span style="color:#f97316">-3.75%</span> |
| 666 | [00590 AGG_GROUP_HAVING_083](crates/bench/sqlite_parity/cases/SQLITE_PARITY_590_AGG_GROUP_HAVING_083.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1763490 | 3111935 | <span style="color:#f97316">-3.73%</span> |
| 667 | [00017 DELETE_BASIC](crates/bench/sqlite_parity/cases/SQLITE_PARITY_017_DELETE_BASIC.rs) | P0 | memory | SQL_DELETE | 1539436 | 3111854 | <span style="color:#f97316">-3.73%</span> |
| 668 | [01033 JSON_EXTRACT_SET_026](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1033_JSON_EXTRACT_SET_026.rs) | P2 | memory | GEN_SQL_JSON | 1588659 | 3108959 | <span style="color:#f97316">-3.63%</span> |
| 669 | [00853 WINDOW_PARTITION_SUM_066](crates/bench/sqlite_parity/cases/SQLITE_PARITY_853_WINDOW_PARTITION_SUM_066.rs) | P2 | memory | GEN_SQL_WINDOW | 1572719 | 3107567 | <span style="color:#f97316">-3.59%</span> |
| 670 | [00711 CTE_RECURSIVE_MATRIX_004](crates/bench/sqlite_parity/cases/SQLITE_PARITY_711_CTE_RECURSIVE_MATRIX_004.rs) | P1 | memory | GEN_SQL_CTE | 1523015 | 3105894 | <span style="color:#f97316">-3.53%</span> |
| 671 | [00428 DML_WHERE_ORDER_LIMIT_041](crates/bench/sqlite_parity/cases/SQLITE_PARITY_428_DML_WHERE_ORDER_LIMIT_041.rs) | P1 | memory | GEN_SQL_DML | 1844283 | 3105373 | <span style="color:#f97316">-3.51%</span> |
| 672 | [01010 JSON_EXTRACT_SET_003](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1010_JSON_EXTRACT_SET_003.rs) | P2 | memory | GEN_SQL_JSON | 1708847 | 3103299 | <span style="color:#f97316">-3.44%</span> |
| 673 | [00380 SCALAR_ARITH_039](crates/bench/sqlite_parity/cases/SQLITE_PARITY_380_SCALAR_ARITH_039.rs) | P1 | memory | GEN_SQL_SCALAR | 2621887 | 3103249 | <span style="color:#f97316">-3.44%</span> |
| 674 | [00718 CTE_RECURSIVE_MATRIX_011](crates/bench/sqlite_parity/cases/SQLITE_PARITY_718_CTE_RECURSIVE_MATRIX_011.rs) | P1 | memory | GEN_SQL_CTE | 1576245 | 3100553 | <span style="color:#f97316">-3.35%</span> |
| 675 | [01036 JSON_EXTRACT_SET_029](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1036_JSON_EXTRACT_SET_029.rs) | P2 | memory | GEN_SQL_JSON | 1562149 | 3099702 | <span style="color:#f97316">-3.32%</span> |
| 676 | [00040 INSTEAD_OF_TRIGGER_ON_VIEW](crates/bench/sqlite_parity/cases/SQLITE_PARITY_040_INSTEAD_OF_TRIGGER_ON_VIEW.rs) | P0 | memory | SQL_TRIGGER | 1570885 | 3099491 | <span style="color:#f97316">-3.32%</span> |
| 677 | [00013 INSERT_SELECT](crates/bench/sqlite_parity/cases/SQLITE_PARITY_013_INSERT_SELECT.rs) | P0 | memory | SQL_INSERT | 1531992 | 3099271 | <span style="color:#f97316">-3.31%</span> |
| 678 | [01067 JSON_EXTRACT_SET_060](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1067_JSON_EXTRACT_SET_060.rs) | P2 | memory | GEN_SQL_JSON | 1515431 | 3098279 | <span style="color:#f97316">-3.28%</span> |
| 679 | [00849 WINDOW_PARTITION_SUM_062](crates/bench/sqlite_parity/cases/SQLITE_PARITY_849_WINDOW_PARTITION_SUM_062.rs) | P2 | memory | GEN_SQL_WINDOW | 1540648 | 3097968 | <span style="color:#f97316">-3.27%</span> |
| 680 | [01101 INDEX_SCHEMA_PRAGMA_034](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1101_INDEX_SCHEMA_PRAGMA_034.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 2617558 | 3096987 | <span style="color:#f97316">-3.23%</span> |
| 681 | [00713 CTE_RECURSIVE_MATRIX_006](crates/bench/sqlite_parity/cases/SQLITE_PARITY_713_CTE_RECURSIVE_MATRIX_006.rs) | P1 | memory | GEN_SQL_CTE | 1524658 | 3096776 | <span style="color:#f97316">-3.23%</span> |
| 682 | [01056 JSON_EXTRACT_SET_049](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1056_JSON_EXTRACT_SET_049.rs) | P2 | memory | GEN_SQL_JSON | 1594921 | 3096495 | <span style="color:#f97316">-3.22%</span> |
| 683 | [00732 CTE_RECURSIVE_MATRIX_025](crates/bench/sqlite_parity/cases/SQLITE_PARITY_732_CTE_RECURSIVE_MATRIX_025.rs) | P1 | memory | GEN_SQL_CTE | 1496845 | 3096214 | <span style="color:#f97316">-3.21%</span> |
| 684 | [00355 SCALAR_NULL_COALESCE_032](crates/bench/sqlite_parity/cases/SQLITE_PARITY_355_SCALAR_NULL_COALESCE_032.rs) | P1 | memory | GEN_SQL_SCALAR | 1389402 | 3095594 | <span style="color:#f97316">-3.19%</span> |
| 685 | [01034 JSON_EXTRACT_SET_027](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1034_JSON_EXTRACT_SET_027.rs) | P2 | memory | GEN_SQL_JSON | 1542021 | 3094862 | <span style="color:#f97316">-3.16%</span> |
| 686 | [00296 SCALAR_ARITH_018](crates/bench/sqlite_parity/cases/SQLITE_PARITY_296_SCALAR_ARITH_018.rs) | P1 | memory | GEN_SQL_SCALAR | 1414420 | 3094061 | <span style="color:#f97316">-3.14%</span> |
| 687 | [00786 CTE_RECURSIVE_MATRIX_079](crates/bench/sqlite_parity/cases/SQLITE_PARITY_786_CTE_RECURSIVE_MATRIX_079.rs) | P1 | memory | GEN_SQL_CTE | 1476006 | 3094000 | <span style="color:#f97316">-3.13%</span> |
| 688 | [00730 CTE_RECURSIVE_MATRIX_023](crates/bench/sqlite_parity/cases/SQLITE_PARITY_730_CTE_RECURSIVE_MATRIX_023.rs) | P1 | memory | GEN_SQL_CTE | 1542372 | 3093800 | <span style="color:#f97316">-3.13%</span> |
| 689 | [00061 WINDOW_ROW_NUMBER_RANK](crates/bench/sqlite_parity/cases/SQLITE_PARITY_061_WINDOW_ROW_NUMBER_RANK.rs) | P0 | memory | SQL_WINDOW | 1658061 | 3093169 | <span style="color:#f97316">-3.11%</span> |
| 690 | [00827 WINDOW_PARTITION_SUM_040](crates/bench/sqlite_parity/cases/SQLITE_PARITY_827_WINDOW_PARTITION_SUM_040.rs) | P2 | memory | GEN_SQL_WINDOW | 1887555 | 3091486 | <span style="color:#f97316">-3.05%</span> |
| 691 | [00946 CONSTRAINT_FK_SAVEPOINT_079](crates/bench/sqlite_parity/cases/SQLITE_PARITY_946_CONSTRAINT_FK_SAVEPOINT_079.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1711853 | 3091326 | <span style="color:#f97316">-3.04%</span> |
| 692 | [00866 WINDOW_PARTITION_SUM_079](crates/bench/sqlite_parity/cases/SQLITE_PARITY_866_WINDOW_PARTITION_SUM_079.rs) | P2 | memory | GEN_SQL_WINDOW | 1686424 | 3089712 | <span style="color:#f97316">-2.99%</span> |
| 693 | [00816 WINDOW_PARTITION_SUM_029](crates/bench/sqlite_parity/cases/SQLITE_PARITY_816_WINDOW_PARTITION_SUM_029.rs) | P2 | memory | GEN_SQL_WINDOW | 1551640 | 3086376 | <span style="color:#f97316">-2.88%</span> |
| 694 | [00780 CTE_RECURSIVE_MATRIX_073](crates/bench/sqlite_parity/cases/SQLITE_PARITY_780_CTE_RECURSIVE_MATRIX_073.rs) | P1 | memory | GEN_SQL_CTE | 1556288 | 3086256 | <span style="color:#f97316">-2.88%</span> |
| 695 | [00723 CTE_RECURSIVE_MATRIX_016](crates/bench/sqlite_parity/cases/SQLITE_PARITY_723_CTE_RECURSIVE_MATRIX_016.rs) | P1 | memory | GEN_SQL_CTE | 2326126 | 3086035 | <span style="color:#f97316">-2.87%</span> |
| 696 | [00801 WINDOW_PARTITION_SUM_014](crates/bench/sqlite_parity/cases/SQLITE_PARITY_801_WINDOW_PARTITION_SUM_014.rs) | P2 | memory | GEN_SQL_WINDOW | 1656258 | 3085715 | <span style="color:#f97316">-2.86%</span> |
| 697 | [00820 WINDOW_PARTITION_SUM_033](crates/bench/sqlite_parity/cases/SQLITE_PARITY_820_WINDOW_PARTITION_SUM_033.rs) | P2 | memory | GEN_SQL_WINDOW | 1613296 | 3085584 | <span style="color:#f97316">-2.85%</span> |
| 698 | [00712 CTE_RECURSIVE_MATRIX_005](crates/bench/sqlite_parity/cases/SQLITE_PARITY_712_CTE_RECURSIVE_MATRIX_005.rs) | P1 | memory | GEN_SQL_CTE | 1551148 | 3085294 | <span style="color:#f97316">-2.84%</span> |
| 699 | [00796 WINDOW_PARTITION_SUM_009](crates/bench/sqlite_parity/cases/SQLITE_PARITY_796_WINDOW_PARTITION_SUM_009.rs) | P2 | memory | GEN_SQL_WINDOW | 1619618 | 3084914 | <span style="color:#f97316">-2.83%</span> |
| 700 | [00269 SCALAR_CAST_TYPEOF_011](crates/bench/sqlite_parity/cases/SQLITE_PARITY_269_SCALAR_CAST_TYPEOF_011.rs) | P1 | memory | GEN_SQL_SCALAR | 1511312 | 3084343 | <span style="color:#f97316">-2.81%</span> |
| 701 | [00840 WINDOW_PARTITION_SUM_053](crates/bench/sqlite_parity/cases/SQLITE_PARITY_840_WINDOW_PARTITION_SUM_053.rs) | P2 | memory | GEN_SQL_WINDOW | 1581005 | 3084152 | <span style="color:#f97316">-2.81%</span> |
| 702 | [00857 WINDOW_PARTITION_SUM_070](crates/bench/sqlite_parity/cases/SQLITE_PARITY_857_WINDOW_PARTITION_SUM_070.rs) | P2 | memory | GEN_SQL_WINDOW | 1571456 | 3080516 | <span style="color:#f97316">-2.68%</span> |
| 703 | [00776 CTE_RECURSIVE_MATRIX_069](crates/bench/sqlite_parity/cases/SQLITE_PARITY_776_CTE_RECURSIVE_MATRIX_069.rs) | P1 | memory | GEN_SQL_CTE | 1515080 | 3079654 | <span style="color:#f97316">-2.66%</span> |
| 704 | [00767 CTE_RECURSIVE_MATRIX_060](crates/bench/sqlite_parity/cases/SQLITE_PARITY_767_CTE_RECURSIVE_MATRIX_060.rs) | P1 | memory | GEN_SQL_CTE | 1605732 | 3079093 | <span style="color:#f97316">-2.64%</span> |
| 705 | [01107 INDEX_SCHEMA_PRAGMA_040](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1107_INDEX_SCHEMA_PRAGMA_040.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 2811566 | 3077128 | <span style="color:#f97316">-2.57%</span> |
| 706 | [00387 SCALAR_NULL_COALESCE_040](crates/bench/sqlite_parity/cases/SQLITE_PARITY_387_SCALAR_NULL_COALESCE_040.rs) | P1 | memory | GEN_SQL_SCALAR | 1447392 | 3074594 | <span style="color:#f97316">-2.49%</span> |
| 707 | [00288 SCALAR_ARITH_016](crates/bench/sqlite_parity/cases/SQLITE_PARITY_288_SCALAR_ARITH_016.rs) | P1 | memory | GEN_SQL_SCALAR | 1454735 | 3074043 | <span style="color:#f97316">-2.47%</span> |
| 708 | [00761 CTE_RECURSIVE_MATRIX_054](crates/bench/sqlite_parity/cases/SQLITE_PARITY_761_CTE_RECURSIVE_MATRIX_054.rs) | P1 | memory | GEN_SQL_CTE | 1512485 | 3073592 | <span style="color:#f97316">-2.45%</span> |
| 709 | [00832 WINDOW_PARTITION_SUM_045](crates/bench/sqlite_parity/cases/SQLITE_PARITY_832_WINDOW_PARTITION_SUM_045.rs) | P2 | memory | GEN_SQL_WINDOW | 1812023 | 3073292 | <span style="color:#f97316">-2.44%</span> |
| 710 | [01118 INDEX_SCHEMA_PRAGMA_051](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1118_INDEX_SCHEMA_PRAGMA_051.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 2425715 | 3072910 | <span style="color:#f97316">-2.43%</span> |
| 711 | [00130 DOT_OPEN_MEMORY](crates/bench/sqlite_parity/cases/SQLITE_PARITY_130_DOT_OPEN_MEMORY.rs) | P0 | memory | CLI_DOT_COMMAND | 1967978 | 3072070 | <span style="color:#f97316">-2.40%</span> |
| 712 | [00709 CTE_RECURSIVE_MATRIX_002](crates/bench/sqlite_parity/cases/SQLITE_PARITY_709_CTE_RECURSIVE_MATRIX_002.rs) | P1 | memory | GEN_SQL_CTE | 1564854 | 3071288 | <span style="color:#f97316">-2.38%</span> |
| 713 | [00803 WINDOW_PARTITION_SUM_016](crates/bench/sqlite_parity/cases/SQLITE_PARITY_803_WINDOW_PARTITION_SUM_016.rs) | P2 | memory | GEN_SQL_WINDOW | 1533234 | 3071127 | <span style="color:#f97316">-2.37%</span> |
| 714 | [01015 JSON_EXTRACT_SET_008](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1015_JSON_EXTRACT_SET_008.rs) | P2 | memory | GEN_SQL_JSON | 1580774 | 3070816 | <span style="color:#f97316">-2.36%</span> |
| 715 | [00744 CTE_RECURSIVE_MATRIX_037](crates/bench/sqlite_parity/cases/SQLITE_PARITY_744_CTE_RECURSIVE_MATRIX_037.rs) | P1 | memory | GEN_SQL_CTE | 1785983 | 3069745 | <span style="color:#f97316">-2.32%</span> |
| 716 | [00274 SCALAR_STRING_012](crates/bench/sqlite_parity/cases/SQLITE_PARITY_274_SCALAR_STRING_012.rs) | P1 | memory | GEN_SQL_SCALAR | 1527243 | 3068181 | <span style="color:#f97316">-2.27%</span> |
| 717 | [00861 WINDOW_PARTITION_SUM_074](crates/bench/sqlite_parity/cases/SQLITE_PARITY_861_WINDOW_PARTITION_SUM_074.rs) | P2 | memory | GEN_SQL_WINDOW | 1609158 | 3066930 | <span style="color:#f97316">-2.23%</span> |
| 718 | [00843 WINDOW_PARTITION_SUM_056](crates/bench/sqlite_parity/cases/SQLITE_PARITY_843_WINDOW_PARTITION_SUM_056.rs) | P2 | memory | GEN_SQL_WINDOW | 1665134 | 3066148 | <span style="color:#f97316">-2.20%</span> |
| 719 | [00793 WINDOW_PARTITION_SUM_006](crates/bench/sqlite_parity/cases/SQLITE_PARITY_793_WINDOW_PARTITION_SUM_006.rs) | P2 | memory | GEN_SQL_WINDOW | 1595362 | 3065366 | <span style="color:#f97316">-2.18%</span> |
| 720 | [00839 WINDOW_PARTITION_SUM_052](crates/bench/sqlite_parity/cases/SQLITE_PARITY_839_WINDOW_PARTITION_SUM_052.rs) | P2 | memory | GEN_SQL_WINDOW | 1800981 | 3065286 | <span style="color:#f97316">-2.18%</span> |
| 721 | [00862 WINDOW_PARTITION_SUM_075](crates/bench/sqlite_parity/cases/SQLITE_PARITY_862_WINDOW_PARTITION_SUM_075.rs) | P2 | memory | GEN_SQL_WINDOW | 1580253 | 3063543 | <span style="color:#f97316">-2.12%</span> |
| 722 | [00488 DML_WHERE_ORDER_LIMIT_101](crates/bench/sqlite_parity/cases/SQLITE_PARITY_488_DML_WHERE_ORDER_LIMIT_101.rs) | P1 | memory | GEN_SQL_DML | 1534647 | 3063252 | <span style="color:#f97316">-2.11%</span> |
| 723 | [00714 CTE_RECURSIVE_MATRIX_007](crates/bench/sqlite_parity/cases/SQLITE_PARITY_714_CTE_RECURSIVE_MATRIX_007.rs) | P1 | memory | GEN_SQL_CTE | 1894970 | 3061800 | <span style="color:#f97316">-2.06%</span> |
| 724 | [00867 WINDOW_PARTITION_SUM_080](crates/bench/sqlite_parity/cases/SQLITE_PARITY_867_WINDOW_PARTITION_SUM_080.rs) | P2 | memory | GEN_SQL_WINDOW | 1547972 | 3061439 | <span style="color:#f97316">-2.05%</span> |
| 725 | [00735 CTE_RECURSIVE_MATRIX_028](crates/bench/sqlite_parity/cases/SQLITE_PARITY_735_CTE_RECURSIVE_MATRIX_028.rs) | P1 | memory | GEN_SQL_CTE | 2314475 | 3060848 | <span style="color:#f97316">-2.03%</span> |
| 726 | [00807 WINDOW_PARTITION_SUM_020](crates/bench/sqlite_parity/cases/SQLITE_PARITY_807_WINDOW_PARTITION_SUM_020.rs) | P2 | memory | GEN_SQL_WINDOW | 1640878 | 3060767 | <span style="color:#f97316">-2.03%</span> |
| 727 | [00247 SCALAR_NULL_COALESCE_005](crates/bench/sqlite_parity/cases/SQLITE_PARITY_247_SCALAR_NULL_COALESCE_005.rs) | P1 | memory | GEN_SQL_SCALAR | 1383791 | 3060197 | <span style="color:#f97316">-2.01%</span> |
| 728 | [00838 WINDOW_PARTITION_SUM_051](crates/bench/sqlite_parity/cases/SQLITE_PARITY_838_WINDOW_PARTITION_SUM_051.rs) | P2 | memory | GEN_SQL_WINDOW | 1825157 | 3059786 | <span style="color:#f97316">-1.99%</span> |
| 729 | [01041 JSON_EXTRACT_SET_034](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1041_JSON_EXTRACT_SET_034.rs) | P2 | memory | GEN_SQL_JSON | 1506483 | 3059745 | <span style="color:#f97316">-1.99%</span> |
| 730 | [00851 WINDOW_PARTITION_SUM_064](crates/bench/sqlite_parity/cases/SQLITE_PARITY_851_WINDOW_PARTITION_SUM_064.rs) | P2 | memory | GEN_SQL_WINDOW | 1814016 | 3059596 | <span style="color:#f97316">-1.99%</span> |
| 731 | [00045 REINDEX_COMMAND](crates/bench/sqlite_parity/cases/SQLITE_PARITY_045_REINDEX_COMMAND.rs) | P0 | memory | SQL_REINDEX | 1567008 | 3057912 | <span style="color:#f97316">-1.93%</span> |
| 732 | [00491 DML_WHERE_ORDER_LIMIT_104](crates/bench/sqlite_parity/cases/SQLITE_PARITY_491_DML_WHERE_ORDER_LIMIT_104.rs) | P1 | memory | GEN_SQL_DML | 1564534 | 3057762 | <span style="color:#f97316">-1.93%</span> |
| 733 | [00431 DML_WHERE_ORDER_LIMIT_044](crates/bench/sqlite_parity/cases/SQLITE_PARITY_431_DML_WHERE_ORDER_LIMIT_044.rs) | P1 | memory | GEN_SQL_DML | 1528114 | 3057691 | <span style="color:#f97316">-1.92%</span> |
| 734 | [00824 WINDOW_PARTITION_SUM_037](crates/bench/sqlite_parity/cases/SQLITE_PARITY_824_WINDOW_PARTITION_SUM_037.rs) | P2 | memory | GEN_SQL_WINDOW | 1560296 | 3057221 | <span style="color:#f97316">-1.91%</span> |
| 735 | [00096 DBSTAT_OPTIONAL](crates/bench/sqlite_parity/cases/SQLITE_PARITY_096_DBSTAT_OPTIONAL.rs) | P3 | memory | SQL_VIRTUAL_TABLE_OPTIONAL | 1571958 | 3056439 | <span style="color:#f97316">-1.88%</span> |
| 736 | [00822 WINDOW_PARTITION_SUM_035](crates/bench/sqlite_parity/cases/SQLITE_PARITY_822_WINDOW_PARTITION_SUM_035.rs) | P2 | memory | GEN_SQL_WINDOW | 1593549 | 3056129 | <span style="color:#f97316">-1.87%</span> |
| 737 | [00766 CTE_RECURSIVE_MATRIX_059](crates/bench/sqlite_parity/cases/SQLITE_PARITY_766_CTE_RECURSIVE_MATRIX_059.rs) | P1 | memory | GEN_SQL_CTE | 1569553 | 3056058 | <span style="color:#f97316">-1.87%</span> |
| 738 | [00845 WINDOW_PARTITION_SUM_058](crates/bench/sqlite_parity/cases/SQLITE_PARITY_845_WINDOW_PARTITION_SUM_058.rs) | P2 | memory | GEN_SQL_WINDOW | 1613105 | 3055828 | <span style="color:#f97316">-1.86%</span> |
| 739 | [00728 CTE_RECURSIVE_MATRIX_021](crates/bench/sqlite_parity/cases/SQLITE_PARITY_728_CTE_RECURSIVE_MATRIX_021.rs) | P1 | memory | GEN_SQL_CTE | 1502105 | 3054656 | <span style="color:#f97316">-1.82%</span> |
| 740 | [00772 CTE_RECURSIVE_MATRIX_065](crates/bench/sqlite_parity/cases/SQLITE_PARITY_772_CTE_RECURSIVE_MATRIX_065.rs) | P1 | memory | GEN_SQL_CTE | 2125446 | 3053154 | <span style="color:#f97316">-1.77%</span> |
| 741 | [01020 JSON_EXTRACT_SET_013](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1020_JSON_EXTRACT_SET_013.rs) | P2 | memory | GEN_SQL_JSON | 1774241 | 3052783 | <span style="color:#f97316">-1.76%</span> |
| 742 | [00821 WINDOW_PARTITION_SUM_034](crates/bench/sqlite_parity/cases/SQLITE_PARITY_821_WINDOW_PARTITION_SUM_034.rs) | P2 | memory | GEN_SQL_WINDOW | 1650757 | 3051590 | <span style="color:#f97316">-1.72%</span> |
| 743 | [00812 WINDOW_PARTITION_SUM_025](crates/bench/sqlite_parity/cases/SQLITE_PARITY_812_WINDOW_PARTITION_SUM_025.rs) | P2 | memory | GEN_SQL_WINDOW | 1613987 | 3050569 | <span style="color:#f97316">-1.69%</span> |
| 744 | [00073 INDEXED_BY](crates/bench/sqlite_parity/cases/SQLITE_PARITY_073_INDEXED_BY.rs) | P0 | memory | SQL_INDEX | 1563161 | 3050368 | <span style="color:#f97316">-1.68%</span> |
| 745 | [00762 CTE_RECURSIVE_MATRIX_055](crates/bench/sqlite_parity/cases/SQLITE_PARITY_762_CTE_RECURSIVE_MATRIX_055.rs) | P1 | memory | GEN_SQL_CTE | 1581365 | 3049957 | <span style="color:#f97316">-1.67%</span> |
| 746 | [00117 DOT_DUMP](crates/bench/sqlite_parity/cases/SQLITE_PARITY_117_DOT_DUMP.rs) | P0 | memory | CLI_DOT_COMMAND | 1612374 | 3049356 | <span style="color:#f97316">-1.65%</span> |
| 747 | [00715 CTE_RECURSIVE_MATRIX_008](crates/bench/sqlite_parity/cases/SQLITE_PARITY_715_CTE_RECURSIVE_MATRIX_008.rs) | P1 | memory | GEN_SQL_CTE | 2342066 | 3049026 | <span style="color:#f97316">-1.63%</span> |
| 748 | [00831 WINDOW_PARTITION_SUM_044](crates/bench/sqlite_parity/cases/SQLITE_PARITY_831_WINDOW_PARTITION_SUM_044.rs) | P2 | memory | GEN_SQL_WINDOW | 1503618 | 3048204 | <span style="color:#f97316">-1.61%</span> |
| 749 | [00482 DML_WHERE_ORDER_LIMIT_095](crates/bench/sqlite_parity/cases/SQLITE_PARITY_482_DML_WHERE_ORDER_LIMIT_095.rs) | P1 | memory | GEN_SQL_DML | 1513106 | 3047853 | <span style="color:#f97316">-1.60%</span> |
| 750 | [00008 STRICT_TABLE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_008_STRICT_TABLE.rs) | P0 | memory | SQL_DDL | 1510511 | 3046451 | <span style="color:#f97316">-1.55%</span> |
| 751 | [00842 WINDOW_PARTITION_SUM_055](crates/bench/sqlite_parity/cases/SQLITE_PARITY_842_WINDOW_PARTITION_SUM_055.rs) | P2 | memory | GEN_SQL_WINDOW | 1562630 | 3046411 | <span style="color:#f97316">-1.55%</span> |
| 752 | [00093 CREATE_VIRTUAL_TABLE_FTS5_OPTIONAL](crates/bench/sqlite_parity/cases/SQLITE_PARITY_093_CREATE_VIRTUAL_TABLE_FTS5_OPTIONAL.rs) | P1 | memory | SQL_VIRTUAL_TABLE_OPTIONAL | 1799028 | 3046020 | <span style="color:#f97316">-1.53%</span> |
| 753 | [00724 CTE_RECURSIVE_MATRIX_017](crates/bench/sqlite_parity/cases/SQLITE_PARITY_724_CTE_RECURSIVE_MATRIX_017.rs) | P1 | memory | GEN_SQL_CTE | 1615751 | 3044767 | <span style="color:#f97316">-1.49%</span> |
| 754 | [00110 DOT_MODE_LINE_COLUMN_TABLE_BOX_MARKDOWN](crates/bench/sqlite_parity/cases/SQLITE_PARITY_110_DOT_MODE_LINE_COLUMN_TABLE_BOX_MARKDOWN.rs) | P0 | memory | CLI_DOT_COMMAND | 1405803 | 3044317 | <span style="color:#f97316">-1.48%</span> |
| 755 | [00864 WINDOW_PARTITION_SUM_077](crates/bench/sqlite_parity/cases/SQLITE_PARITY_864_WINDOW_PARTITION_SUM_077.rs) | P2 | memory | GEN_SQL_WINDOW | 1584260 | 3043535 | <span style="color:#f97316">-1.45%</span> |
| 756 | [00733 CTE_RECURSIVE_MATRIX_026](crates/bench/sqlite_parity/cases/SQLITE_PARITY_733_CTE_RECURSIVE_MATRIX_026.rs) | P1 | memory | GEN_SQL_CTE | 1473260 | 3043425 | <span style="color:#f97316">-1.45%</span> |
| 757 | [00879 CONSTRAINT_FK_SAVEPOINT_012](crates/bench/sqlite_parity/cases/SQLITE_PARITY_879_CONSTRAINT_FK_SAVEPOINT_012.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2734359 | 3042503 | <span style="color:#f97316">-1.42%</span> |
| 758 | [00717 CTE_RECURSIVE_MATRIX_010](crates/bench/sqlite_parity/cases/SQLITE_PARITY_717_CTE_RECURSIVE_MATRIX_010.rs) | P1 | memory | GEN_SQL_CTE | 1687737 | 3042232 | <span style="color:#f97316">-1.41%</span> |
| 759 | [00806 WINDOW_PARTITION_SUM_019](crates/bench/sqlite_parity/cases/SQLITE_PARITY_806_WINDOW_PARTITION_SUM_019.rs) | P2 | memory | GEN_SQL_WINDOW | 1639165 | 3041040 | <span style="color:#f97316">-1.37%</span> |
| 760 | [00854 WINDOW_PARTITION_SUM_067](crates/bench/sqlite_parity/cases/SQLITE_PARITY_854_WINDOW_PARTITION_SUM_067.rs) | P2 | memory | GEN_SQL_WINDOW | 1594901 | 3040980 | <span style="color:#f97316">-1.37%</span> |
| 761 | [00765 CTE_RECURSIVE_MATRIX_058](crates/bench/sqlite_parity/cases/SQLITE_PARITY_765_CTE_RECURSIVE_MATRIX_058.rs) | P1 | memory | GEN_SQL_CTE | 1552040 | 3040419 | <span style="color:#f97316">-1.35%</span> |
| 762 | [00777 CTE_RECURSIVE_MATRIX_070](crates/bench/sqlite_parity/cases/SQLITE_PARITY_777_CTE_RECURSIVE_MATRIX_070.rs) | P1 | memory | GEN_SQL_CTE | 1487417 | 3036982 | <span style="color:#f97316">-1.23%</span> |
| 763 | [00837 WINDOW_PARTITION_SUM_050](crates/bench/sqlite_parity/cases/SQLITE_PARITY_837_WINDOW_PARTITION_SUM_050.rs) | P2 | memory | GEN_SQL_WINDOW | 1847359 | 3035911 | <span style="color:#f97316">-1.20%</span> |
| 764 | [00860 WINDOW_PARTITION_SUM_073](crates/bench/sqlite_parity/cases/SQLITE_PARITY_860_WINDOW_PARTITION_SUM_073.rs) | P2 | memory | GEN_SQL_WINDOW | 1577127 | 3035720 | <span style="color:#f97316">-1.19%</span> |
| 765 | [00189 OPT_ECHO](crates/bench/sqlite_parity/cases/SQLITE_PARITY_189_OPT_ECHO.rs) | P2 | memory | CLI_OPTION | 1436762 | 3035139 | <span style="color:#f97316">-1.17%</span> |
| 766 | [00031 CREATE_INDEX](crates/bench/sqlite_parity/cases/SQLITE_PARITY_031_CREATE_INDEX.rs) | P0 | memory | SQL_INDEX | 1521041 | 3034899 | <span style="color:#f97316">-1.16%</span> |
| 767 | [00792 WINDOW_PARTITION_SUM_005](crates/bench/sqlite_parity/cases/SQLITE_PARITY_792_WINDOW_PARTITION_SUM_005.rs) | P2 | memory | GEN_SQL_WINDOW | 1979209 | 3034759 | <span style="color:#f97316">-1.16%</span> |
| 768 | [00823 WINDOW_PARTITION_SUM_036](crates/bench/sqlite_parity/cases/SQLITE_PARITY_823_WINDOW_PARTITION_SUM_036.rs) | P2 | memory | GEN_SQL_WINDOW | 1620370 | 3034308 | <span style="color:#f97316">-1.14%</span> |
| 769 | [00856 WINDOW_PARTITION_SUM_069](crates/bench/sqlite_parity/cases/SQLITE_PARITY_856_WINDOW_PARTITION_SUM_069.rs) | P2 | memory | GEN_SQL_WINDOW | 1621151 | 3034217 | <span style="color:#f97316">-1.14%</span> |
| 770 | [00721 CTE_RECURSIVE_MATRIX_014](crates/bench/sqlite_parity/cases/SQLITE_PARITY_721_CTE_RECURSIVE_MATRIX_014.rs) | P1 | memory | GEN_SQL_CTE | 1466839 | 3033506 | <span style="color:#f97316">-1.12%</span> |
| 771 | [00074 NOT_INDEXED](crates/bench/sqlite_parity/cases/SQLITE_PARITY_074_NOT_INDEXED.rs) | P0 | memory | SQL_INDEX | 1536341 | 3032956 | <span style="color:#f97316">-1.10%</span> |
| 772 | [00781 CTE_RECURSIVE_MATRIX_074](crates/bench/sqlite_parity/cases/SQLITE_PARITY_781_CTE_RECURSIVE_MATRIX_074.rs) | P1 | memory | GEN_SQL_CTE | 1776165 | 3032033 | <span style="color:#f97316">-1.07%</span> |
| 773 | [00742 CTE_RECURSIVE_MATRIX_035](crates/bench/sqlite_parity/cases/SQLITE_PARITY_742_CTE_RECURSIVE_MATRIX_035.rs) | P1 | memory | GEN_SQL_CTE | 1533475 | 3031843 | <span style="color:#f97316">-1.06%</span> |
| 774 | [00750 CTE_RECURSIVE_MATRIX_043](crates/bench/sqlite_parity/cases/SQLITE_PARITY_750_CTE_RECURSIVE_MATRIX_043.rs) | P1 | memory | GEN_SQL_CTE | 1488610 | 3029719 | <span style="color:#f97316">-0.99%</span> |
| 775 | [00829 WINDOW_PARTITION_SUM_042](crates/bench/sqlite_parity/cases/SQLITE_PARITY_829_WINDOW_PARTITION_SUM_042.rs) | P2 | memory | GEN_SQL_WINDOW | 1576886 | 3028767 | <span style="color:#f97316">-0.96%</span> |
| 776 | [00848 WINDOW_PARTITION_SUM_061](crates/bench/sqlite_parity/cases/SQLITE_PARITY_848_WINDOW_PARTITION_SUM_061.rs) | P2 | memory | GEN_SQL_WINDOW | 1566547 | 3028657 | <span style="color:#f97316">-0.96%</span> |
| 777 | [00846 WINDOW_PARTITION_SUM_059](crates/bench/sqlite_parity/cases/SQLITE_PARITY_846_WINDOW_PARTITION_SUM_059.rs) | P2 | memory | GEN_SQL_WINDOW | 1611462 | 3026673 | <span style="color:#f97316">-0.89%</span> |
| 778 | [00722 CTE_RECURSIVE_MATRIX_015](crates/bench/sqlite_parity/cases/SQLITE_PARITY_722_CTE_RECURSIVE_MATRIX_015.rs) | P1 | memory | GEN_SQL_CTE | 1545558 | 3026263 | <span style="color:#f97316">-0.88%</span> |
| 779 | [00026 FOREIGN_KEYS_CASCADE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_026_FOREIGN_KEYS_CASCADE.rs) | P0 | memory | SQL_FOREIGN_KEYS | 1699079 | 3026122 | <span style="color:#f97316">-0.87%</span> |
| 780 | [00015 UPDATE_BASIC](crates/bench/sqlite_parity/cases/SQLITE_PARITY_015_UPDATE_BASIC.rs) | P0 | memory | SQL_UPDATE | 1871686 | 3025501 | <span style="color:#f97316">-0.85%</span> |
| 781 | [00859 WINDOW_PARTITION_SUM_072](crates/bench/sqlite_parity/cases/SQLITE_PARITY_859_WINDOW_PARTITION_SUM_072.rs) | P2 | memory | GEN_SQL_WINDOW | 1864251 | 3025140 | <span style="color:#f97316">-0.84%</span> |
| 782 | [00844 WINDOW_PARTITION_SUM_057](crates/bench/sqlite_parity/cases/SQLITE_PARITY_844_WINDOW_PARTITION_SUM_057.rs) | P2 | memory | GEN_SQL_WINDOW | 1582627 | 3024910 | <span style="color:#f97316">-0.83%</span> |
| 783 | [00311 SCALAR_NULL_COALESCE_021](crates/bench/sqlite_parity/cases/SQLITE_PARITY_311_SCALAR_NULL_COALESCE_021.rs) | P1 | memory | GEN_SQL_SCALAR | 1935857 | 3024790 | <span style="color:#f97316">-0.83%</span> |
| 784 | [00725 CTE_RECURSIVE_MATRIX_018](crates/bench/sqlite_parity/cases/SQLITE_PARITY_725_CTE_RECURSIVE_MATRIX_018.rs) | P1 | memory | GEN_SQL_CTE | 1700882 | 3024268 | <span style="color:#f97316">-0.81%</span> |
| 785 | [00797 WINDOW_PARTITION_SUM_010](crates/bench/sqlite_parity/cases/SQLITE_PARITY_797_WINDOW_PARTITION_SUM_010.rs) | P2 | memory | GEN_SQL_WINDOW | 1534026 | 3022846 | <span style="color:#f97316">-0.76%</span> |
| 786 | [00731 CTE_RECURSIVE_MATRIX_024](crates/bench/sqlite_parity/cases/SQLITE_PARITY_731_CTE_RECURSIVE_MATRIX_024.rs) | P1 | memory | GEN_SQL_CTE | 1743392 | 3022074 | <span style="color:#f97316">-0.74%</span> |
| 787 | [00600 AGG_GROUP_HAVING_093](crates/bench/sqlite_parity/cases/SQLITE_PARITY_600_AGG_GROUP_HAVING_093.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1618506 | 3021673 | <span style="color:#f97316">-0.72%</span> |
| 788 | [00020 UPSERT_DO_UPDATE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_020_UPSERT_DO_UPDATE.rs) | P0 | memory | SQL_UPSERT | 2164280 | 3019500 | <span style="color:#f97316">-0.65%</span> |
| 789 | [00030 ALTER_TABLE_ADD_DROP_COLUMN](crates/bench/sqlite_parity/cases/SQLITE_PARITY_030_ALTER_TABLE_ADD_DROP_COLUMN.rs) | P0 | memory | SQL_ALTER | 2072165 | 3019349 | <span style="color:#f97316">-0.64%</span> |
| 790 | [00573 AGG_GROUP_HAVING_066](crates/bench/sqlite_parity/cases/SQLITE_PARITY_573_AGG_GROUP_HAVING_066.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1702875 | 3018188 | <span style="color:#f97316">-0.61%</span> |
| 791 | [00798 WINDOW_PARTITION_SUM_011](crates/bench/sqlite_parity/cases/SQLITE_PARITY_798_WINDOW_PARTITION_SUM_011.rs) | P2 | memory | GEN_SQL_WINDOW | 1567449 | 3017285 | <span style="color:#f97316">-0.58%</span> |
| 792 | [00059 AGGREGATE_FUNCTIONS_CORE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_059_AGGREGATE_FUNCTIONS_CORE.rs) | P0 | memory | SQL_FUNCTIONS | 1751177 | 3016684 | <span style="color:#f97316">-0.56%</span> |
| 793 | [00001 SELECT_CORE_EXPRESSIONS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_001_SELECT_CORE_EXPRESSIONS.rs) | P0 | memory | SQL_SELECT | 1546560 | 3016204 | <span style="color:#f97316">-0.54%</span> |
| 794 | [00813 WINDOW_PARTITION_SUM_026](crates/bench/sqlite_parity/cases/SQLITE_PARITY_813_WINDOW_PARTITION_SUM_026.rs) | P2 | memory | GEN_SQL_WINDOW | 1898546 | 3015062 | <span style="color:#f97316">-0.50%</span> |
| 795 | [00802 WINDOW_PARTITION_SUM_015](crates/bench/sqlite_parity/cases/SQLITE_PARITY_802_WINDOW_PARTITION_SUM_015.rs) | P2 | memory | GEN_SQL_WINDOW | 1627243 | 3013999 | <span style="color:#6b7280">-0.47%</span> |
| 796 | [00035 DROP_INDEX](crates/bench/sqlite_parity/cases/SQLITE_PARITY_035_DROP_INDEX.rs) | P0 | memory | SQL_DROP | 1645898 | 3013899 | <span style="color:#6b7280">-0.46%</span> |
| 797 | [00819 WINDOW_PARTITION_SUM_032](crates/bench/sqlite_parity/cases/SQLITE_PARITY_819_WINDOW_PARTITION_SUM_032.rs) | P2 | memory | GEN_SQL_WINDOW | 1643293 | 3012957 | <span style="color:#6b7280">-0.43%</span> |
| 798 | [00841 WINDOW_PARTITION_SUM_054](crates/bench/sqlite_parity/cases/SQLITE_PARITY_841_WINDOW_PARTITION_SUM_054.rs) | P2 | memory | GEN_SQL_WINDOW | 1762027 | 3012426 | <span style="color:#6b7280">-0.41%</span> |
| 799 | [00847 WINDOW_PARTITION_SUM_060](crates/bench/sqlite_parity/cases/SQLITE_PARITY_847_WINDOW_PARTITION_SUM_060.rs) | P2 | memory | GEN_SQL_WINDOW | 1557730 | 3009951 | <span style="color:#6b7280">-0.33%</span> |
| 800 | [00850 WINDOW_PARTITION_SUM_063](crates/bench/sqlite_parity/cases/SQLITE_PARITY_850_WINDOW_PARTITION_SUM_063.rs) | P2 | memory | GEN_SQL_WINDOW | 1545528 | 3009942 | <span style="color:#6b7280">-0.33%</span> |
| 801 | [00745 CTE_RECURSIVE_MATRIX_038](crates/bench/sqlite_parity/cases/SQLITE_PARITY_745_CTE_RECURSIVE_MATRIX_038.rs) | P1 | memory | GEN_SQL_CTE | 1579171 | 3008939 | <span style="color:#6b7280">-0.30%</span> |
| 802 | [00558 AGG_GROUP_HAVING_051](crates/bench/sqlite_parity/cases/SQLITE_PARITY_558_AGG_GROUP_HAVING_051.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1777647 | 3007447 | <span style="color:#6b7280">-0.25%</span> |
| 803 | [00215 TRANSACTION_SYNONYMS_END_COMMIT_TRANSACTION](crates/bench/sqlite_parity/cases/SQLITE_PARITY_215_TRANSACTION_SYNONYMS_END_COMMIT_TRANSACTION.rs) | P0 | memory | SQL_TRANSACTION | 1807204 | 3005984 | <span style="color:#6b7280">-0.20%</span> |
| 804 | [00826 WINDOW_PARTITION_SUM_039](crates/bench/sqlite_parity/cases/SQLITE_PARITY_826_WINDOW_PARTITION_SUM_039.rs) | P2 | memory | GEN_SQL_WINDOW | 1865513 | 3005503 | <span style="color:#6b7280">-0.18%</span> |
| 805 | [00833 WINDOW_PARTITION_SUM_046](crates/bench/sqlite_parity/cases/SQLITE_PARITY_833_WINDOW_PARTITION_SUM_046.rs) | P2 | memory | GEN_SQL_WINDOW | 1833283 | 3004581 | <span style="color:#6b7280">-0.15%</span> |
| 806 | [00784 CTE_RECURSIVE_MATRIX_077](crates/bench/sqlite_parity/cases/SQLITE_PARITY_784_CTE_RECURSIVE_MATRIX_077.rs) | P1 | memory | GEN_SQL_CTE | 2697399 | 3004020 | <span style="color:#6b7280">-0.13%</span> |
| 807 | [00881 CONSTRAINT_FK_SAVEPOINT_014](crates/bench/sqlite_parity/cases/SQLITE_PARITY_881_CONSTRAINT_FK_SAVEPOINT_014.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1706633 | 3003749 | <span style="color:#6b7280">-0.12%</span> |
| 808 | [00225 OPT_NONCE_SAFE_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_225_OPT_NONCE_SAFE_MODE.rs) | P2 | memory | CLI_OPTION | 1423847 | 3003389 | <span style="color:#6b7280">-0.11%</span> |
| 809 | [00033 PARTIAL_INDEX](crates/bench/sqlite_parity/cases/SQLITE_PARITY_033_PARTIAL_INDEX.rs) | P0 | memory | SQL_INDEX | 1496545 | 3003199 | <span style="color:#6b7280">-0.11%</span> |
| 810 | [00343 SCALAR_NULL_COALESCE_029](crates/bench/sqlite_parity/cases/SQLITE_PARITY_343_SCALAR_NULL_COALESCE_029.rs) | P1 | memory | GEN_SQL_SCALAR | 1443705 | 3002998 | <span style="color:#6b7280">-0.10%</span> |
| 811 | [00834 WINDOW_PARTITION_SUM_047](crates/bench/sqlite_parity/cases/SQLITE_PARITY_834_WINDOW_PARTITION_SUM_047.rs) | P2 | memory | GEN_SQL_WINDOW | 1586195 | 3001245 | <span style="color:#6b7280">-0.04%</span> |
| 812 | [00002 LITERALS_AND_TYPEOF](crates/bench/sqlite_parity/cases/SQLITE_PARITY_002_LITERALS_AND_TYPEOF.rs) | P0 | memory | SQL_EXPRESSIONS | 1547822 | 3000834 | <span style="color:#6b7280">-0.03%</span> |
| 813 | [00214 DROP_TABLE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_214_DROP_TABLE.rs) | P0 | memory | SQL_DROP | 1676405 | 2999201 | <span style="color:#6b7280">0.03%</span> |
| 814 | [00814 WINDOW_PARTITION_SUM_027](crates/bench/sqlite_parity/cases/SQLITE_PARITY_814_WINDOW_PARTITION_SUM_027.rs) | P2 | memory | GEN_SQL_WINDOW | 1581595 | 2998109 | <span style="color:#6b7280">0.06%</span> |
| 815 | [01049 JSON_EXTRACT_SET_042](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1049_JSON_EXTRACT_SET_042.rs) | P2 | memory | GEN_SQL_JSON | 1541150 | 2996957 | <span style="color:#6b7280">0.10%</span> |
| 816 | [00541 AGG_GROUP_HAVING_034](crates/bench/sqlite_parity/cases/SQLITE_PARITY_541_AGG_GROUP_HAVING_034.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1681115 | 2995975 | <span style="color:#6b7280">0.13%</span> |
| 817 | [00763 CTE_RECURSIVE_MATRIX_056](crates/bench/sqlite_parity/cases/SQLITE_PARITY_763_CTE_RECURSIVE_MATRIX_056.rs) | P1 | memory | GEN_SQL_CTE | 1543794 | 2993851 | <span style="color:#6b7280">0.20%</span> |
| 818 | [00375 SCALAR_NULL_COALESCE_037](crates/bench/sqlite_parity/cases/SQLITE_PARITY_375_SCALAR_NULL_COALESCE_037.rs) | P1 | memory | GEN_SQL_SCALAR | 1676977 | 2990865 | <span style="color:#6b7280">0.30%</span> |
| 819 | [00785 CTE_RECURSIVE_MATRIX_078](crates/bench/sqlite_parity/cases/SQLITE_PARITY_785_CTE_RECURSIVE_MATRIX_078.rs) | P1 | memory | GEN_SQL_CTE | 1486005 | 2990474 | <span style="color:#6b7280">0.32%</span> |
| 820 | [00041 DROP_TRIGGER](crates/bench/sqlite_parity/cases/SQLITE_PARITY_041_DROP_TRIGGER.rs) | P0 | memory | SQL_DROP | 1532393 | 2989102 | <span style="color:#6b7280">0.36%</span> |
| 821 | [00926 CONSTRAINT_FK_SAVEPOINT_059](crates/bench/sqlite_parity/cases/SQLITE_PARITY_926_CONSTRAINT_FK_SAVEPOINT_059.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1646168 | 2988841 | <span style="color:#6b7280">0.37%</span> |
| 822 | [00065 CTE_RECURSIVE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_065_CTE_RECURSIVE.rs) | P0 | memory | SQL_CTE | 1925157 | 2987759 | <span style="color:#6b7280">0.41%</span> |
| 823 | [00051 PRAGMA_INDEX_LIST_FUNCTION](crates/bench/sqlite_parity/cases/SQLITE_PARITY_051_PRAGMA_INDEX_LIST_FUNCTION.rs) | P0 | memory | SQL_PRAGMA | 1532022 | 2986818 | <span style="color:#6b7280">0.44%</span> |
| 824 | [00068 CAST_AND_TYPE_AFFINITY](crates/bench/sqlite_parity/cases/SQLITE_PARITY_068_CAST_AND_TYPE_AFFINITY.rs) | P0 | memory | SQL_EXPRESSIONS | 1476476 | 2986507 | <span style="color:#6b7280">0.45%</span> |
| 825 | [00244 SCALAR_ARITH_005](crates/bench/sqlite_parity/cases/SQLITE_PARITY_244_SCALAR_ARITH_005.rs) | P1 | memory | GEN_SQL_SCALAR | 1468702 | 2983041 | <span style="color:#16a34a">0.57%</span> |
| 826 | [00770 CTE_RECURSIVE_MATRIX_063](crates/bench/sqlite_parity/cases/SQLITE_PARITY_770_CTE_RECURSIVE_MATRIX_063.rs) | P1 | memory | GEN_SQL_CTE | 1514729 | 2983010 | <span style="color:#16a34a">0.57%</span> |
| 827 | [00756 CTE_RECURSIVE_MATRIX_049](crates/bench/sqlite_parity/cases/SQLITE_PARITY_756_CTE_RECURSIVE_MATRIX_049.rs) | P1 | memory | GEN_SQL_CTE | 1535979 | 2982920 | <span style="color:#16a34a">0.57%</span> |
| 828 | [00916 CONSTRAINT_FK_SAVEPOINT_049](crates/bench/sqlite_parity/cases/SQLITE_PARITY_916_CONSTRAINT_FK_SAVEPOINT_049.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1638013 | 2982219 | <span style="color:#16a34a">0.59%</span> |
| 829 | [00316 SCALAR_ARITH_023](crates/bench/sqlite_parity/cases/SQLITE_PARITY_316_SCALAR_ARITH_023.rs) | P1 | memory | GEN_SQL_SCALAR | 1449275 | 2982189 | <span style="color:#16a34a">0.59%</span> |
| 830 | [00764 CTE_RECURSIVE_MATRIX_057](crates/bench/sqlite_parity/cases/SQLITE_PARITY_764_CTE_RECURSIVE_MATRIX_057.rs) | P1 | memory | GEN_SQL_CTE | 1503639 | 2980676 | <span style="color:#16a34a">0.64%</span> |
| 831 | [00863 WINDOW_PARTITION_SUM_076](crates/bench/sqlite_parity/cases/SQLITE_PARITY_863_WINDOW_PARTITION_SUM_076.rs) | P2 | memory | GEN_SQL_WINDOW | 1545427 | 2976298 | <span style="color:#16a34a">0.79%</span> |
| 832 | [00560 AGG_GROUP_HAVING_053](crates/bench/sqlite_parity/cases/SQLITE_PARITY_560_AGG_GROUP_HAVING_053.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1599089 | 2975536 | <span style="color:#16a34a">0.82%</span> |
| 833 | [00018 DELETE_RETURNING](crates/bench/sqlite_parity/cases/SQLITE_PARITY_018_DELETE_RETURNING.rs) | P0 | memory | SQL_DELETE | 1660295 | 2970617 | <span style="color:#16a34a">0.98%</span> |
| 834 | [00119 DOT_EQP](crates/bench/sqlite_parity/cases/SQLITE_PARITY_119_DOT_EQP.rs) | P0 | memory | CLI_DOT_COMMAND | 1465697 | 2969655 | <span style="color:#16a34a">1.01%</span> |
| 835 | [00149 DOT_ONCE_TEMPFILE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_149_DOT_ONCE_TEMPFILE.rs) | P1 | tempfile | CLI_TEMPFILE | 1463422 | 2968103 | <span style="color:#16a34a">1.06%</span> |
| 836 | [00058 GROUP_BY_HAVING](crates/bench/sqlite_parity/cases/SQLITE_PARITY_058_GROUP_BY_HAVING.rs) | P0 | memory | SQL_AGGREGATE | 1507145 | 2967872 | <span style="color:#16a34a">1.07%</span> |
| 837 | [00216 ROLLBACK_TRANSACTION_SYNTAX](crates/bench/sqlite_parity/cases/SQLITE_PARITY_216_ROLLBACK_TRANSACTION_SYNTAX.rs) | P0 | memory | SQL_TRANSACTION | 1580855 | 2965547 | <span style="color:#16a34a">1.15%</span> |
| 838 | [00808 WINDOW_PARTITION_SUM_021](crates/bench/sqlite_parity/cases/SQLITE_PARITY_808_WINDOW_PARTITION_SUM_021.rs) | P2 | memory | GEN_SQL_WINDOW | 1545999 | 2962442 | <span style="color:#16a34a">1.25%</span> |
| 839 | [00101 SCHEMA_SQLITE_MASTER_ALIAS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_101_SCHEMA_SQLITE_MASTER_ALIAS.rs) | P0 | memory | SQL_SCHEMA | 1550326 | 2960879 | <span style="color:#16a34a">1.30%</span> |
| 840 | [00057 COMPOUND_SELECT_UNION_INTERSECT_EXCEPT](crates/bench/sqlite_parity/cases/SQLITE_PARITY_057_COMPOUND_SELECT_UNION_INTERSECT_EXCEPT.rs) | P0 | memory | SQL_SELECT | 1540227 | 2958984 | <span style="color:#16a34a">1.37%</span> |
| 841 | [00146 DOT_READ_TEMPFILE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_146_DOT_READ_TEMPFILE.rs) | P1 | tempfile | CLI_TEMPFILE | 1547201 | 2956991 | <span style="color:#16a34a">1.43%</span> |
| 842 | [00042 TEMP_TABLE_TEMP_SCHEMA](crates/bench/sqlite_parity/cases/SQLITE_PARITY_042_TEMP_TABLE_TEMP_SCHEMA.rs) | P0 | memory | SQL_TEMP | 1589370 | 2955859 | <span style="color:#16a34a">1.47%</span> |
| 843 | [00034 EXPRESSION_INDEX](crates/bench/sqlite_parity/cases/SQLITE_PARITY_034_EXPRESSION_INDEX.rs) | P0 | memory | SQL_INDEX | 1538044 | 2954757 | <span style="color:#16a34a">1.51%</span> |
| 844 | [00037 DROP_VIEW](crates/bench/sqlite_parity/cases/SQLITE_PARITY_037_DROP_VIEW.rs) | P0 | memory | SQL_DROP | 1467339 | 2953124 | <span style="color:#16a34a">1.56%</span> |
| 845 | [00778 CTE_RECURSIVE_MATRIX_071](crates/bench/sqlite_parity/cases/SQLITE_PARITY_778_CTE_RECURSIVE_MATRIX_071.rs) | P1 | memory | GEN_SQL_CTE | 1643924 | 2952974 | <span style="color:#16a34a">1.57%</span> |
| 846 | [00122 DOT_CHANGES](crates/bench/sqlite_parity/cases/SQLITE_PARITY_122_DOT_CHANGES.rs) | P0 | memory | CLI_DOT_COMMAND | 1422665 | 2952302 | <span style="color:#16a34a">1.59%</span> |
| 847 | [00341 SCALAR_CAST_TYPEOF_029](crates/bench/sqlite_parity/cases/SQLITE_PARITY_341_SCALAR_CAST_TYPEOF_029.rs) | P1 | memory | GEN_SQL_SCALAR | 1563281 | 2942484 | <span style="color:#16a34a">1.92%</span> |
| 848 | [00108 DOT_MODE_CSV_AND_QUOTE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_108_DOT_MODE_CSV_AND_QUOTE.rs) | P0 | memory | CLI_DOT_COMMAND | 1522584 | 2942464 | <span style="color:#16a34a">1.92%</span> |
| 849 | [00828 WINDOW_PARTITION_SUM_041](crates/bench/sqlite_parity/cases/SQLITE_PARITY_828_WINDOW_PARTITION_SUM_041.rs) | P2 | memory | GEN_SQL_WINDOW | 1530008 | 2941823 | <span style="color:#16a34a">1.94%</span> |
| 850 | [00557 AGG_GROUP_HAVING_050](crates/bench/sqlite_parity/cases/SQLITE_PARITY_557_AGG_GROUP_HAVING_050.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1597186 | 2940641 | <span style="color:#16a34a">1.98%</span> |
| 851 | [00103 WINDOW_NAMED_WINDOW](crates/bench/sqlite_parity/cases/SQLITE_PARITY_103_WINDOW_NAMED_WINDOW.rs) | P0 | memory | SQL_WINDOW | 1620770 | 2939117 | <span style="color:#16a34a">2.03%</span> |
| 852 | [00118 DOT_FULLSCHEMA](crates/bench/sqlite_parity/cases/SQLITE_PARITY_118_DOT_FULLSCHEMA.rs) | P0 | memory | CLI_DOT_COMMAND | 1717844 | 2937344 | <span style="color:#16a34a">2.09%</span> |
| 853 | [00855 WINDOW_PARTITION_SUM_068](crates/bench/sqlite_parity/cases/SQLITE_PARITY_855_WINDOW_PARTITION_SUM_068.rs) | P2 | memory | GEN_SQL_WINDOW | 1586815 | 2936763 | <span style="color:#16a34a">2.11%</span> |
| 854 | [01058 JSON_EXTRACT_SET_051](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1058_JSON_EXTRACT_SET_051.rs) | P2 | memory | GEN_SQL_JSON | 1642852 | 2934368 | <span style="color:#16a34a">2.19%</span> |
| 855 | [00273 SCALAR_CAST_TYPEOF_012](crates/bench/sqlite_parity/cases/SQLITE_PARITY_273_SCALAR_CAST_TYPEOF_012.rs) | P1 | memory | GEN_SQL_SCALAR | 2173216 | 2932415 | <span style="color:#16a34a">2.25%</span> |
| 856 | [00016 UPDATE_RETURNING](crates/bench/sqlite_parity/cases/SQLITE_PARITY_016_UPDATE_RETURNING.rs) | P0 | memory | SQL_UPDATE | 1534667 | 2929930 | <span style="color:#16a34a">2.34%</span> |
| 857 | [00063 WINDOW_EXCLUDE_CURRENT_ROW](crates/bench/sqlite_parity/cases/SQLITE_PARITY_063_WINDOW_EXCLUDE_CURRENT_ROW.rs) | P0 | memory | SQL_WINDOW | 1527073 | 2923357 | <span style="color:#16a34a">2.55%</span> |
| 858 | [00023 TRANSACTION_ROLLBACK](crates/bench/sqlite_parity/cases/SQLITE_PARITY_023_TRANSACTION_ROLLBACK.rs) | P0 | memory | SQL_TRANSACTION | 1490534 | 2923057 | <span style="color:#16a34a">2.56%</span> |
| 859 | [00072 ORDER_BY_NULLS_FIRST_LAST](crates/bench/sqlite_parity/cases/SQLITE_PARITY_072_ORDER_BY_NULLS_FIRST_LAST.rs) | P0 | memory | SQL_SELECT | 1453504 | 2917887 | <span style="color:#16a34a">2.74%</span> |
| 860 | [00062 WINDOW_FRAMES_ROWS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_062_WINDOW_FRAMES_ROWS.rs) | P0 | memory | SQL_WINDOW | 1569874 | 2914951 | <span style="color:#16a34a">2.83%</span> |
| 861 | [00021 UPSERT_DO_NOTHING](crates/bench/sqlite_parity/cases/SQLITE_PARITY_021_UPSERT_DO_NOTHING.rs) | P0 | memory | SQL_UPSERT | 1564103 | 2907677 | <span style="color:#16a34a">3.08%</span> |
| 862 | [00014 INSERT_RETURNING](crates/bench/sqlite_parity/cases/SQLITE_PARITY_014_INSERT_RETURNING.rs) | P0 | memory | SQL_INSERT | 1599910 | 2903680 | <span style="color:#16a34a">3.21%</span> |
| 863 | [00353 SCALAR_CAST_TYPEOF_032](crates/bench/sqlite_parity/cases/SQLITE_PARITY_353_SCALAR_CAST_TYPEOF_032.rs) | P1 | memory | GEN_SQL_SCALAR | 1428636 | 2901706 | <span style="color:#16a34a">3.28%</span> |
| 864 | [00022 TRANSACTION_COMMIT](crates/bench/sqlite_parity/cases/SQLITE_PARITY_022_TRANSACTION_COMMIT.rs) | P0 | memory | SQL_TRANSACTION | 1491636 | 2901506 | <span style="color:#16a34a">3.28%</span> |
| 865 | [00219 UPDATE_LIMIT_OPTIONAL](crates/bench/sqlite_parity/cases/SQLITE_PARITY_219_UPDATE_LIMIT_OPTIONAL.rs) | P3 | memory | SQL_UPDATE_OPTIONAL | 1568741 | 2898290 | <span style="color:#16a34a">3.39%</span> |
| 866 | [00830 WINDOW_PARTITION_SUM_043](crates/bench/sqlite_parity/cases/SQLITE_PARITY_830_WINDOW_PARTITION_SUM_043.rs) | P2 | memory | GEN_SQL_WINDOW | 1572890 | 2893511 | <span style="color:#16a34a">3.55%</span> |
| 867 | [00329 SCALAR_CAST_TYPEOF_026](crates/bench/sqlite_parity/cases/SQLITE_PARITY_329_SCALAR_CAST_TYPEOF_026.rs) | P1 | memory | GEN_SQL_SCALAR | 1378351 | 2892359 | <span style="color:#16a34a">3.59%</span> |
| 868 | [00239 SCALAR_NULL_COALESCE_003](crates/bench/sqlite_parity/cases/SQLITE_PARITY_239_SCALAR_NULL_COALESCE_003.rs) | P1 | memory | GEN_SQL_SCALAR | 1489512 | 2889824 | <span style="color:#16a34a">3.67%</span> |
| 869 | [00279 SCALAR_NULL_COALESCE_013](crates/bench/sqlite_parity/cases/SQLITE_PARITY_279_SCALAR_NULL_COALESCE_013.rs) | P1 | memory | GEN_SQL_SCALAR | 1513707 | 2885917 | <span style="color:#16a34a">3.80%</span> |
| 870 | [00357 SCALAR_CAST_TYPEOF_033](crates/bench/sqlite_parity/cases/SQLITE_PARITY_357_SCALAR_CAST_TYPEOF_033.rs) | P1 | memory | GEN_SQL_SCALAR | 1509329 | 2885886 | <span style="color:#16a34a">3.80%</span> |
| 871 | [00241 SCALAR_CAST_TYPEOF_004](crates/bench/sqlite_parity/cases/SQLITE_PARITY_241_SCALAR_CAST_TYPEOF_004.rs) | P1 | memory | GEN_SQL_SCALAR | 1410361 | 2884825 | <span style="color:#16a34a">3.84%</span> |
| 872 | [00345 SCALAR_CAST_TYPEOF_030](crates/bench/sqlite_parity/cases/SQLITE_PARITY_345_SCALAR_CAST_TYPEOF_030.rs) | P1 | memory | GEN_SQL_SCALAR | 1404270 | 2882580 | <span style="color:#16a34a">3.91%</span> |
| 873 | [00266 SCALAR_STRING_010](crates/bench/sqlite_parity/cases/SQLITE_PARITY_266_SCALAR_STRING_010.rs) | P1 | memory | GEN_SQL_SCALAR | 1482237 | 2880116 | <span style="color:#16a34a">4.00%</span> |
| 874 | [00257 SCALAR_CAST_TYPEOF_008](crates/bench/sqlite_parity/cases/SQLITE_PARITY_257_SCALAR_CAST_TYPEOF_008.rs) | P1 | memory | GEN_SQL_SCALAR | 1452751 | 2879464 | <span style="color:#16a34a">4.02%</span> |
| 875 | [00060 FILTER_CLAUSE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_060_FILTER_CLAUSE.rs) | P0 | memory | SQL_AGGREGATE | 1799769 | 2877491 | <span style="color:#16a34a">4.08%</span> |
| 876 | [00145 DOT_SCANSTATS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_145_DOT_SCANSTATS.rs) | P3 | memory | CLI_DOT_COMMAND_DIAGNOSTIC | 1550777 | 2874886 | <span style="color:#16a34a">4.17%</span> |
| 877 | [00369 SCALAR_CAST_TYPEOF_036](crates/bench/sqlite_parity/cases/SQLITE_PARITY_369_SCALAR_CAST_TYPEOF_036.rs) | P1 | memory | GEN_SQL_SCALAR | 1469313 | 2874264 | <span style="color:#16a34a">4.19%</span> |
| 878 | [00270 SCALAR_STRING_011](crates/bench/sqlite_parity/cases/SQLITE_PARITY_270_SCALAR_STRING_011.rs) | P1 | memory | GEN_SQL_SCALAR | 1424328 | 2872581 | <span style="color:#16a34a">4.25%</span> |
| 879 | [00092 PERCENTILE_FUNCTIONS_OPTIONAL](crates/bench/sqlite_parity/cases/SQLITE_PARITY_092_PERCENTILE_FUNCTIONS_OPTIONAL.rs) | P3 | memory | SQL_FUNCTIONS_OPTIONAL | 1438606 | 2872021 | <span style="color:#16a34a">4.27%</span> |
| 880 | [00098 CLI_REGEXP_OPTIONAL](crates/bench/sqlite_parity/cases/SQLITE_PARITY_098_CLI_REGEXP_OPTIONAL.rs) | P2 | memory | CLI_EXTENSION_OPTIONAL | 1387548 | 2870527 | <span style="color:#16a34a">4.32%</span> |
| 881 | [01050 JSON_EXTRACT_SET_043](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1050_JSON_EXTRACT_SET_043.rs) | P2 | memory | GEN_SQL_JSON | 2062046 | 2866289 | <span style="color:#16a34a">4.46%</span> |
| 882 | [00243 SCALAR_NULL_COALESCE_004](crates/bench/sqlite_parity/cases/SQLITE_PARITY_243_SCALAR_NULL_COALESCE_004.rs) | P1 | memory | GEN_SQL_SCALAR | 1394522 | 2864586 | <span style="color:#16a34a">4.51%</span> |
| 883 | [00229 SCALAR_CAST_TYPEOF_001](crates/bench/sqlite_parity/cases/SQLITE_PARITY_229_SCALAR_CAST_TYPEOF_001.rs) | P1 | memory | GEN_SQL_SCALAR | 1479242 | 2860999 | <span style="color:#16a34a">4.63%</span> |
| 884 | [00050 PRAGMA_TABLE_INFO_FUNCTION](crates/bench/sqlite_parity/cases/SQLITE_PARITY_050_PRAGMA_TABLE_INFO_FUNCTION.rs) | P0 | memory | SQL_PRAGMA | 1512075 | 2857784 | <span style="color:#16a34a">4.74%</span> |
| 885 | [00230 SCALAR_STRING_001](crates/bench/sqlite_parity/cases/SQLITE_PARITY_230_SCALAR_STRING_001.rs) | P1 | memory | GEN_SQL_SCALAR | 1811571 | 2857543 | <span style="color:#16a34a">4.75%</span> |
| 886 | [00305 SCALAR_CAST_TYPEOF_020](crates/bench/sqlite_parity/cases/SQLITE_PARITY_305_SCALAR_CAST_TYPEOF_020.rs) | P1 | memory | GEN_SQL_SCALAR | 1439356 | 2857232 | <span style="color:#16a34a">4.76%</span> |
| 887 | [00265 SCALAR_CAST_TYPEOF_010](crates/bench/sqlite_parity/cases/SQLITE_PARITY_265_SCALAR_CAST_TYPEOF_010.rs) | P1 | memory | GEN_SQL_SCALAR | 1576556 | 2856410 | <span style="color:#16a34a">4.79%</span> |
| 888 | [00102 WITH_MATERIALIZED_HINTS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_102_WITH_MATERIALIZED_HINTS.rs) | P0 | memory | SQL_CTE | 1895931 | 2855379 | <span style="color:#16a34a">4.82%</span> |
| 889 | [00346 SCALAR_STRING_030](crates/bench/sqlite_parity/cases/SQLITE_PARITY_346_SCALAR_STRING_030.rs) | P1 | memory | GEN_SQL_SCALAR | 1481566 | 2852113 | <span style="color:#16a34a">4.93%</span> |
| 890 | [00262 SCALAR_STRING_009](crates/bench/sqlite_parity/cases/SQLITE_PARITY_262_SCALAR_STRING_009.rs) | P1 | memory | GEN_SQL_SCALAR | 1548013 | 2851762 | <span style="color:#16a34a">4.94%</span> |
| 891 | [00347 SCALAR_NULL_COALESCE_030](crates/bench/sqlite_parity/cases/SQLITE_PARITY_347_SCALAR_NULL_COALESCE_030.rs) | P1 | memory | GEN_SQL_SCALAR | 1488920 | 2850279 | <span style="color:#16a34a">4.99%</span> |
| 892 | [00220 DELETE_LIMIT_OPTIONAL](crates/bench/sqlite_parity/cases/SQLITE_PARITY_220_DELETE_LIMIT_OPTIONAL.rs) | P3 | memory | SQL_DELETE_OPTIONAL | 1590553 | 2848556 | <span style="color:#2563eb">5.05%</span> |
| 893 | [00379 SCALAR_NULL_COALESCE_038](crates/bench/sqlite_parity/cases/SQLITE_PARITY_379_SCALAR_NULL_COALESCE_038.rs) | P1 | memory | GEN_SQL_SCALAR | 1444536 | 2847844 | <span style="color:#2563eb">5.07%</span> |
| 894 | [00350 SCALAR_STRING_031](crates/bench/sqlite_parity/cases/SQLITE_PARITY_350_SCALAR_STRING_031.rs) | P1 | memory | GEN_SQL_SCALAR | 1450988 | 2847614 | <span style="color:#2563eb">5.08%</span> |
| 895 | [00337 SCALAR_CAST_TYPEOF_028](crates/bench/sqlite_parity/cases/SQLITE_PARITY_337_SCALAR_CAST_TYPEOF_028.rs) | P1 | memory | GEN_SQL_SCALAR | 1412396 | 2847153 | <span style="color:#2563eb">5.09%</span> |
| 896 | [00253 SCALAR_CAST_TYPEOF_007](crates/bench/sqlite_parity/cases/SQLITE_PARITY_253_SCALAR_CAST_TYPEOF_007.rs) | P1 | memory | GEN_SQL_SCALAR | 1932862 | 2845840 | <span style="color:#2563eb">5.14%</span> |
| 897 | [00321 SCALAR_CAST_TYPEOF_024](crates/bench/sqlite_parity/cases/SQLITE_PARITY_321_SCALAR_CAST_TYPEOF_024.rs) | P1 | memory | GEN_SQL_SCALAR | 1510020 | 2844358 | <span style="color:#2563eb">5.19%</span> |
| 898 | [00294 SCALAR_STRING_017](crates/bench/sqlite_parity/cases/SQLITE_PARITY_294_SCALAR_STRING_017.rs) | P1 | memory | GEN_SQL_SCALAR | 1413527 | 2843226 | <span style="color:#2563eb">5.23%</span> |
| 899 | [01051 JSON_EXTRACT_SET_044](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1051_JSON_EXTRACT_SET_044.rs) | P2 | memory | GEN_SQL_JSON | 1530489 | 2843145 | <span style="color:#2563eb">5.23%</span> |
| 900 | [00373 SCALAR_CAST_TYPEOF_037](crates/bench/sqlite_parity/cases/SQLITE_PARITY_373_SCALAR_CAST_TYPEOF_037.rs) | P1 | memory | GEN_SQL_SCALAR | 1407356 | 2843116 | <span style="color:#2563eb">5.23%</span> |
| 901 | [00281 SCALAR_CAST_TYPEOF_014](crates/bench/sqlite_parity/cases/SQLITE_PARITY_281_SCALAR_CAST_TYPEOF_014.rs) | P1 | memory | GEN_SQL_SCALAR | 1466838 | 2842845 | <span style="color:#2563eb">5.24%</span> |
| 902 | [00383 SCALAR_NULL_COALESCE_039](crates/bench/sqlite_parity/cases/SQLITE_PARITY_383_SCALAR_NULL_COALESCE_039.rs) | P1 | memory | GEN_SQL_SCALAR | 1476957 | 2842404 | <span style="color:#2563eb">5.25%</span> |
| 903 | [00290 SCALAR_STRING_016](crates/bench/sqlite_parity/cases/SQLITE_PARITY_290_SCALAR_STRING_016.rs) | P1 | memory | GEN_SQL_SCALAR | 1434417 | 2841402 | <span style="color:#2563eb">5.29%</span> |
| 904 | [00371 SCALAR_NULL_COALESCE_036](crates/bench/sqlite_parity/cases/SQLITE_PARITY_371_SCALAR_NULL_COALESCE_036.rs) | P1 | memory | GEN_SQL_SCALAR | 1429427 | 2841022 | <span style="color:#2563eb">5.30%</span> |
| 905 | [00325 SCALAR_CAST_TYPEOF_025](crates/bench/sqlite_parity/cases/SQLITE_PARITY_325_SCALAR_CAST_TYPEOF_025.rs) | P1 | memory | GEN_SQL_SCALAR | 1436441 | 2837465 | <span style="color:#2563eb">5.42%</span> |
| 906 | [00088 JSON_SCALAR_FUNCTIONS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_088_JSON_SCALAR_FUNCTIONS.rs) | P0 | memory | SQL_JSON | 1435298 | 2835581 | <span style="color:#2563eb">5.48%</span> |
| 907 | [00245 SCALAR_CAST_TYPEOF_005](crates/bench/sqlite_parity/cases/SQLITE_PARITY_245_SCALAR_CAST_TYPEOF_005.rs) | P1 | memory | GEN_SQL_SCALAR | 1628114 | 2833658 | <span style="color:#2563eb">5.54%</span> |
| 908 | [00133 DOT_AUTH](crates/bench/sqlite_parity/cases/SQLITE_PARITY_133_DOT_AUTH.rs) | P3 | memory | CLI_DOT_COMMAND_DIAGNOSTIC | 1444897 | 2833437 | <span style="color:#2563eb">5.55%</span> |
| 909 | [00315 SCALAR_NULL_COALESCE_022](crates/bench/sqlite_parity/cases/SQLITE_PARITY_315_SCALAR_NULL_COALESCE_022.rs) | P1 | memory | GEN_SQL_SCALAR | 1550898 | 2831062 | <span style="color:#2563eb">5.63%</span> |
| 910 | [00331 SCALAR_NULL_COALESCE_026](crates/bench/sqlite_parity/cases/SQLITE_PARITY_331_SCALAR_NULL_COALESCE_026.rs) | P1 | memory | GEN_SQL_SCALAR | 1456860 | 2830031 | <span style="color:#2563eb">5.67%</span> |
| 911 | [00129 DOT_CONNECTION](crates/bench/sqlite_parity/cases/SQLITE_PARITY_129_DOT_CONNECTION.rs) | P0 | memory | CLI_DOT_COMMAND | 1948451 | 2829901 | <span style="color:#2563eb">5.67%</span> |
| 912 | [00334 SCALAR_STRING_027](crates/bench/sqlite_parity/cases/SQLITE_PARITY_334_SCALAR_STRING_027.rs) | P1 | memory | GEN_SQL_SCALAR | 1403328 | 2829089 | <span style="color:#2563eb">5.70%</span> |
| 913 | [00342 SCALAR_STRING_029](crates/bench/sqlite_parity/cases/SQLITE_PARITY_342_SCALAR_STRING_029.rs) | P1 | memory | GEN_SQL_SCALAR | 1794670 | 2828367 | <span style="color:#2563eb">5.72%</span> |
| 914 | [00064 CTE_NON_RECURSIVE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_064_CTE_NON_RECURSIVE.rs) | P0 | memory | SQL_CTE | 1475555 | 2827817 | <span style="color:#2563eb">5.74%</span> |
| 915 | [00323 SCALAR_NULL_COALESCE_024](crates/bench/sqlite_parity/cases/SQLITE_PARITY_323_SCALAR_NULL_COALESCE_024.rs) | P1 | memory | GEN_SQL_SCALAR | 2442797 | 2827797 | <span style="color:#2563eb">5.74%</span> |
| 916 | [00285 SCALAR_CAST_TYPEOF_015](crates/bench/sqlite_parity/cases/SQLITE_PARITY_285_SCALAR_CAST_TYPEOF_015.rs) | P1 | memory | GEN_SQL_SCALAR | 1623285 | 2826384 | <span style="color:#2563eb">5.79%</span> |
| 917 | [00238 SCALAR_STRING_003](crates/bench/sqlite_parity/cases/SQLITE_PARITY_238_SCALAR_STRING_003.rs) | P1 | memory | GEN_SQL_SCALAR | 1461959 | 2826113 | <span style="color:#2563eb">5.80%</span> |
| 918 | [00339 SCALAR_NULL_COALESCE_028](crates/bench/sqlite_parity/cases/SQLITE_PARITY_339_SCALAR_NULL_COALESCE_028.rs) | P1 | memory | GEN_SQL_SCALAR | 1394101 | 2825853 | <span style="color:#2563eb">5.80%</span> |
| 919 | [00313 SCALAR_CAST_TYPEOF_022](crates/bench/sqlite_parity/cases/SQLITE_PARITY_313_SCALAR_CAST_TYPEOF_022.rs) | P1 | memory | GEN_SQL_SCALAR | 1430991 | 2825632 | <span style="color:#2563eb">5.81%</span> |
| 920 | [00100 SCHEMA_SQLITE_SCHEMA](crates/bench/sqlite_parity/cases/SQLITE_PARITY_100_SCHEMA_SQLITE_SCHEMA.rs) | P0 | memory | SQL_SCHEMA | 1474893 | 2825132 | <span style="color:#2563eb">5.83%</span> |
| 921 | [00046 VACUUM_MEMORY](crates/bench/sqlite_parity/cases/SQLITE_PARITY_046_VACUUM_MEMORY.rs) | P0 | memory | SQL_VACUUM | 1733875 | 2824720 | <span style="color:#2563eb">5.84%</span> |
| 922 | [00237 SCALAR_CAST_TYPEOF_003](crates/bench/sqlite_parity/cases/SQLITE_PARITY_237_SCALAR_CAST_TYPEOF_003.rs) | P1 | memory | GEN_SQL_SCALAR | 1470074 | 2824490 | <span style="color:#2563eb">5.85%</span> |
| 923 | [00254 SCALAR_STRING_007](crates/bench/sqlite_parity/cases/SQLITE_PARITY_254_SCALAR_STRING_007.rs) | P1 | memory | GEN_SQL_SCALAR | 1498989 | 2824420 | <span style="color:#2563eb">5.85%</span> |
| 924 | [00082 CORE_STRING_FUNCTIONS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_082_CORE_STRING_FUNCTIONS.rs) | P0 | memory | SQL_FUNCTIONS | 1473682 | 2822757 | <span style="color:#2563eb">5.91%</span> |
| 925 | [00333 SCALAR_CAST_TYPEOF_027](crates/bench/sqlite_parity/cases/SQLITE_PARITY_333_SCALAR_CAST_TYPEOF_027.rs) | P1 | memory | GEN_SQL_SCALAR | 1463943 | 2821825 | <span style="color:#2563eb">5.94%</span> |
| 926 | [00255 SCALAR_NULL_COALESCE_007](crates/bench/sqlite_parity/cases/SQLITE_PARITY_255_SCALAR_NULL_COALESCE_007.rs) | P1 | memory | GEN_SQL_SCALAR | 1471488 | 2817116 | <span style="color:#2563eb">6.10%</span> |
| 927 | [00378 SCALAR_STRING_038](crates/bench/sqlite_parity/cases/SQLITE_PARITY_378_SCALAR_STRING_038.rs) | P1 | memory | GEN_SQL_SCALAR | 1489912 | 2816956 | <span style="color:#2563eb">6.10%</span> |
| 928 | [00739 CTE_RECURSIVE_MATRIX_032](crates/bench/sqlite_parity/cases/SQLITE_PARITY_739_CTE_RECURSIVE_MATRIX_032.rs) | P1 | memory | GEN_SQL_CTE | 1511373 | 2814992 | <span style="color:#2563eb">6.17%</span> |
| 929 | [00259 SCALAR_NULL_COALESCE_008](crates/bench/sqlite_parity/cases/SQLITE_PARITY_259_SCALAR_NULL_COALESCE_008.rs) | P1 | memory | GEN_SQL_SCALAR | 1453102 | 2813229 | <span style="color:#2563eb">6.23%</span> |
| 930 | [00286 SCALAR_STRING_015](crates/bench/sqlite_parity/cases/SQLITE_PARITY_286_SCALAR_STRING_015.rs) | P1 | memory | GEN_SQL_SCALAR | 1421102 | 2812698 | <span style="color:#2563eb">6.24%</span> |
| 931 | [00291 SCALAR_NULL_COALESCE_016](crates/bench/sqlite_parity/cases/SQLITE_PARITY_291_SCALAR_NULL_COALESCE_016.rs) | P1 | memory | GEN_SQL_SCALAR | 1718134 | 2812457 | <span style="color:#2563eb">6.25%</span> |
| 932 | [00338 SCALAR_STRING_028](crates/bench/sqlite_parity/cases/SQLITE_PARITY_338_SCALAR_STRING_028.rs) | P1 | memory | GEN_SQL_SCALAR | 1410582 | 2811917 | <span style="color:#2563eb">6.27%</span> |
| 933 | [00354 SCALAR_STRING_032](crates/bench/sqlite_parity/cases/SQLITE_PARITY_354_SCALAR_STRING_032.rs) | P1 | memory | GEN_SQL_SCALAR | 1406864 | 2811416 | <span style="color:#2563eb">6.29%</span> |
| 934 | [00302 SCALAR_STRING_019](crates/bench/sqlite_parity/cases/SQLITE_PARITY_302_SCALAR_STRING_019.rs) | P1 | memory | GEN_SQL_SCALAR | 1480254 | 2809632 | <span style="color:#2563eb">6.35%</span> |
| 935 | [00099 CLI_UINT_COLLATION_OPTIONAL](crates/bench/sqlite_parity/cases/SQLITE_PARITY_099_CLI_UINT_COLLATION_OPTIONAL.rs) | P3 | memory | CLI_EXTENSION_OPTIONAL | 1443484 | 2807799 | <span style="color:#2563eb">6.41%</span> |
| 936 | [00090 JSON_MUTATION_FUNCTIONS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_090_JSON_MUTATION_FUNCTIONS.rs) | P0 | memory | SQL_JSON | 1416473 | 2807228 | <span style="color:#2563eb">6.43%</span> |
| 937 | [00297 SCALAR_CAST_TYPEOF_018](crates/bench/sqlite_parity/cases/SQLITE_PARITY_297_SCALAR_CAST_TYPEOF_018.rs) | P1 | memory | GEN_SQL_SCALAR | 1832952 | 2806847 | <span style="color:#2563eb">6.44%</span> |
| 938 | [00087 DATE_TIMEDIFF_FUNCTION](crates/bench/sqlite_parity/cases/SQLITE_PARITY_087_DATE_TIMEDIFF_FUNCTION.rs) | P0 | memory | SQL_FUNCTIONS | 1448604 | 2805784 | <span style="color:#2563eb">6.47%</span> |
| 939 | [00340 SCALAR_ARITH_029](crates/bench/sqlite_parity/cases/SQLITE_PARITY_340_SCALAR_ARITH_029.rs) | P1 | memory | GEN_SQL_SCALAR | 1539065 | 2804963 | <span style="color:#2563eb">6.50%</span> |
| 940 | [00091 MATH_FUNCTIONS_OPTIONAL](crates/bench/sqlite_parity/cases/SQLITE_PARITY_091_MATH_FUNCTIONS_OPTIONAL.rs) | P2 | memory | SQL_FUNCTIONS_OPTIONAL | 1469344 | 2803861 | <span style="color:#2563eb">6.54%</span> |
| 941 | [00299 SCALAR_NULL_COALESCE_018](crates/bench/sqlite_parity/cases/SQLITE_PARITY_299_SCALAR_NULL_COALESCE_018.rs) | P1 | memory | GEN_SQL_SCALAR | 1448684 | 2802339 | <span style="color:#2563eb">6.59%</span> |
| 942 | [00386 SCALAR_STRING_040](crates/bench/sqlite_parity/cases/SQLITE_PARITY_386_SCALAR_STRING_040.rs) | P1 | memory | GEN_SQL_SCALAR | 1525981 | 2801146 | <span style="color:#2563eb">6.63%</span> |
| 943 | [00258 SCALAR_STRING_008](crates/bench/sqlite_parity/cases/SQLITE_PARITY_258_SCALAR_STRING_008.rs) | P1 | memory | GEN_SQL_SCALAR | 1463552 | 2800846 | <span style="color:#2563eb">6.64%</span> |
| 944 | [00121 DOT_PARAMETER](crates/bench/sqlite_parity/cases/SQLITE_PARITY_121_DOT_PARAMETER.rs) | P0 | memory | CLI_DOT_COMMAND | 1662860 | 2799713 | <span style="color:#2563eb">6.68%</span> |
| 945 | [00228 SCALAR_ARITH_001](crates/bench/sqlite_parity/cases/SQLITE_PARITY_228_SCALAR_ARITH_001.rs) | P1 | memory | GEN_SQL_SCALAR | 1426963 | 2799032 | <span style="color:#2563eb">6.70%</span> |
| 946 | [00053 SELECT_WHERE_ORDER_LIMIT_OFFSET](crates/bench/sqlite_parity/cases/SQLITE_PARITY_053_SELECT_WHERE_ORDER_LIMIT_OFFSET.rs) | P0 | memory | SQL_SELECT | 1744584 | 2797148 | <span style="color:#2563eb">6.76%</span> |
| 947 | [00322 SCALAR_STRING_024](crates/bench/sqlite_parity/cases/SQLITE_PARITY_322_SCALAR_STRING_024.rs) | P1 | memory | GEN_SQL_SCALAR | 1426262 | 2796016 | <span style="color:#2563eb">6.80%</span> |
| 948 | [00249 SCALAR_CAST_TYPEOF_006](crates/bench/sqlite_parity/cases/SQLITE_PARITY_249_SCALAR_CAST_TYPEOF_006.rs) | P1 | memory | GEN_SQL_SCALAR | 1402266 | 2796006 | <span style="color:#2563eb">6.80%</span> |
| 949 | [00083 CORE_NUMERIC_FUNCTIONS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_083_CORE_NUMERIC_FUNCTIONS.rs) | P0 | memory | SQL_FUNCTIONS | 1474633 | 2795676 | <span style="color:#2563eb">6.81%</span> |
| 950 | [00349 SCALAR_CAST_TYPEOF_031](crates/bench/sqlite_parity/cases/SQLITE_PARITY_349_SCALAR_CAST_TYPEOF_031.rs) | P1 | memory | GEN_SQL_SCALAR | 1450989 | 2794924 | <span style="color:#2563eb">6.84%</span> |
| 951 | [00298 SCALAR_STRING_018](crates/bench/sqlite_parity/cases/SQLITE_PARITY_298_SCALAR_STRING_018.rs) | P1 | memory | GEN_SQL_SCALAR | 1439697 | 2794714 | <span style="color:#2563eb">6.84%</span> |
| 952 | [00370 SCALAR_STRING_036](crates/bench/sqlite_parity/cases/SQLITE_PARITY_370_SCALAR_STRING_036.rs) | P1 | memory | GEN_SQL_SCALAR | 1541430 | 2792950 | <span style="color:#2563eb">6.90%</span> |
| 953 | [00330 SCALAR_STRING_026](crates/bench/sqlite_parity/cases/SQLITE_PARITY_330_SCALAR_STRING_026.rs) | P1 | memory | GEN_SQL_SCALAR | 1460226 | 2791929 | <span style="color:#2563eb">6.94%</span> |
| 954 | [00086 DATE_TIME_FUNCTIONS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_086_DATE_TIME_FUNCTIONS.rs) | P0 | memory | SQL_FUNCTIONS | 1480804 | 2791738 | <span style="color:#2563eb">6.94%</span> |
| 955 | [00236 SCALAR_ARITH_003](crates/bench/sqlite_parity/cases/SQLITE_PARITY_236_SCALAR_ARITH_003.rs) | P1 | memory | GEN_SQL_SCALAR | 1472760 | 2789103 | <span style="color:#2563eb">7.03%</span> |
| 956 | [00231 SCALAR_NULL_COALESCE_001](crates/bench/sqlite_parity/cases/SQLITE_PARITY_231_SCALAR_NULL_COALESCE_001.rs) | P1 | memory | GEN_SQL_SCALAR | 1466879 | 2786869 | <span style="color:#2563eb">7.10%</span> |
| 957 | [00263 SCALAR_NULL_COALESCE_009](crates/bench/sqlite_parity/cases/SQLITE_PARITY_263_SCALAR_NULL_COALESCE_009.rs) | P1 | memory | GEN_SQL_SCALAR | 1465305 | 2785947 | <span style="color:#2563eb">7.14%</span> |
| 958 | [00052 PRAGMA_INTEGRITY_QUICK_CHECK](crates/bench/sqlite_parity/cases/SQLITE_PARITY_052_PRAGMA_INTEGRITY_QUICK_CHECK.rs) | P0 | memory | SQL_PRAGMA | 1535248 | 2785476 | <span style="color:#2563eb">7.15%</span> |
| 959 | [00235 SCALAR_NULL_COALESCE_002](crates/bench/sqlite_parity/cases/SQLITE_PARITY_235_SCALAR_NULL_COALESCE_002.rs) | P1 | memory | GEN_SQL_SCALAR | 1401735 | 2783502 | <span style="color:#2563eb">7.22%</span> |
| 960 | [00361 SCALAR_CAST_TYPEOF_034](crates/bench/sqlite_parity/cases/SQLITE_PARITY_361_SCALAR_CAST_TYPEOF_034.rs) | P1 | memory | GEN_SQL_SCALAR | 1444265 | 2782641 | <span style="color:#2563eb">7.25%</span> |
| 961 | [00036 CREATE_VIEW](crates/bench/sqlite_parity/cases/SQLITE_PARITY_036_CREATE_VIEW.rs) | P0 | memory | SQL_VIEW | 1508006 | 2781099 | <span style="color:#2563eb">7.30%</span> |
| 962 | [00081 BLOBS_HEX_ZEROBLOB](crates/bench/sqlite_parity/cases/SQLITE_PARITY_081_BLOBS_HEX_ZEROBLOB.rs) | P0 | memory | SQL_FUNCTIONS | 1440018 | 2779926 | <span style="color:#2563eb">7.34%</span> |
| 963 | [00374 SCALAR_STRING_037](crates/bench/sqlite_parity/cases/SQLITE_PARITY_374_SCALAR_STRING_037.rs) | P1 | memory | GEN_SQL_SCALAR | 1432534 | 2779856 | <span style="color:#2563eb">7.34%</span> |
| 964 | [00085 CORE_RANDOM_SHAPE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_085_CORE_RANDOM_SHAPE.rs) | P0 | memory | SQL_FUNCTIONS | 1387808 | 2777501 | <span style="color:#2563eb">7.42%</span> |
| 965 | [00300 SCALAR_ARITH_019](crates/bench/sqlite_parity/cases/SQLITE_PARITY_300_SCALAR_ARITH_019.rs) | P1 | memory | GEN_SQL_SCALAR | 1439246 | 2777471 | <span style="color:#2563eb">7.42%</span> |
| 966 | [00283 SCALAR_NULL_COALESCE_014](crates/bench/sqlite_parity/cases/SQLITE_PARITY_283_SCALAR_NULL_COALESCE_014.rs) | P1 | memory | GEN_SQL_SCALAR | 1396826 | 2776880 | <span style="color:#2563eb">7.44%</span> |
| 967 | [00310 SCALAR_STRING_021](crates/bench/sqlite_parity/cases/SQLITE_PARITY_310_SCALAR_STRING_021.rs) | P1 | memory | GEN_SQL_SCALAR | 1432414 | 2776379 | <span style="color:#2563eb">7.45%</span> |
| 968 | [00336 SCALAR_ARITH_028](crates/bench/sqlite_parity/cases/SQLITE_PARITY_336_SCALAR_ARITH_028.rs) | P1 | memory | GEN_SQL_SCALAR | 1824496 | 2775738 | <span style="color:#2563eb">7.48%</span> |
| 969 | [00080 NOT_NULL_FAILURE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_080_NOT_NULL_FAILURE.rs) | P0 | memory | SQL_CONSTRAINTS_NEGATIVE | 1614077 | 2774686 | <span style="color:#2563eb">7.51%</span> |
| 970 | [00360 SCALAR_ARITH_034](crates/bench/sqlite_parity/cases/SQLITE_PARITY_360_SCALAR_ARITH_034.rs) | P1 | memory | GEN_SQL_SCALAR | 1416023 | 2771269 | <span style="color:#2563eb">7.62%</span> |
| 971 | [00234 SCALAR_STRING_002](crates/bench/sqlite_parity/cases/SQLITE_PARITY_234_SCALAR_STRING_002.rs) | P1 | memory | GEN_SQL_SCALAR | 1787476 | 2771190 | <span style="color:#2563eb">7.63%</span> |
| 972 | [00233 SCALAR_CAST_TYPEOF_002](crates/bench/sqlite_parity/cases/SQLITE_PARITY_233_SCALAR_CAST_TYPEOF_002.rs) | P1 | memory | GEN_SQL_SCALAR | 1798717 | 2771139 | <span style="color:#2563eb">7.63%</span> |
| 973 | [00141 DOT_SHA3SUM](crates/bench/sqlite_parity/cases/SQLITE_PARITY_141_DOT_SHA3SUM.rs) | P0 | memory | CLI_DOT_COMMAND | 1710199 | 2770198 | <span style="color:#2563eb">7.66%</span> |
| 974 | [00027 FOREIGN_KEY_FAILURE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_027_FOREIGN_KEY_FAILURE.rs) | P0 | memory | SQL_FOREIGN_KEYS_NEGATIVE | 1486235 | 2766952 | <span style="color:#2563eb">7.77%</span> |
| 975 | [00251 SCALAR_NULL_COALESCE_006](crates/bench/sqlite_parity/cases/SQLITE_PARITY_251_SCALAR_NULL_COALESCE_006.rs) | P1 | memory | GEN_SQL_SCALAR | 1426212 | 2766621 | <span style="color:#2563eb">7.78%</span> |
| 976 | [00307 SCALAR_NULL_COALESCE_020](crates/bench/sqlite_parity/cases/SQLITE_PARITY_307_SCALAR_NULL_COALESCE_020.rs) | P1 | memory | GEN_SQL_SCALAR | 2498513 | 2765479 | <span style="color:#2563eb">7.82%</span> |
| 977 | [00271 SCALAR_NULL_COALESCE_011](crates/bench/sqlite_parity/cases/SQLITE_PARITY_271_SCALAR_NULL_COALESCE_011.rs) | P1 | memory | GEN_SQL_SCALAR | 1456209 | 2764066 | <span style="color:#2563eb">7.86%</span> |
| 978 | [00344 SCALAR_ARITH_030](crates/bench/sqlite_parity/cases/SQLITE_PARITY_344_SCALAR_ARITH_030.rs) | P1 | memory | GEN_SQL_SCALAR | 1478090 | 2762873 | <span style="color:#2563eb">7.90%</span> |
| 979 | [00358 SCALAR_STRING_033](crates/bench/sqlite_parity/cases/SQLITE_PARITY_358_SCALAR_STRING_033.rs) | P1 | memory | GEN_SQL_SCALAR | 1469594 | 2761861 | <span style="color:#2563eb">7.94%</span> |
| 980 | [00319 SCALAR_NULL_COALESCE_023](crates/bench/sqlite_parity/cases/SQLITE_PARITY_319_SCALAR_NULL_COALESCE_023.rs) | P1 | memory | GEN_SQL_SCALAR | 1396315 | 2760319 | <span style="color:#2563eb">7.99%</span> |
| 981 | [00301 SCALAR_CAST_TYPEOF_019](crates/bench/sqlite_parity/cases/SQLITE_PARITY_301_SCALAR_CAST_TYPEOF_019.rs) | P1 | memory | GEN_SQL_SCALAR | 1408568 | 2759668 | <span style="color:#2563eb">8.01%</span> |
| 982 | [00025 BEGIN_MODES](crates/bench/sqlite_parity/cases/SQLITE_PARITY_025_BEGIN_MODES.rs) | P0 | memory | SQL_TRANSACTION | 1377059 | 2759588 | <span style="color:#2563eb">8.01%</span> |
| 983 | [00067 CASE_COALESCE_NULLIF_IIF](crates/bench/sqlite_parity/cases/SQLITE_PARITY_067_CASE_COALESCE_NULLIF_IIF.rs) | P0 | memory | SQL_EXPRESSIONS | 1667980 | 2758746 | <span style="color:#2563eb">8.04%</span> |
| 984 | [00227 OPT_UNSAFE_TESTING_CATALOG](crates/bench/sqlite_parity/cases/SQLITE_PARITY_227_OPT_UNSAFE_TESTING_CATALOG.rs) | P4 | catalog | CLI_OPTION_CATALOG | 1424098 | 2757914 | <span style="color:#2563eb">8.07%</span> |
| 985 | [00324 SCALAR_ARITH_025](crates/bench/sqlite_parity/cases/SQLITE_PARITY_324_SCALAR_ARITH_025.rs) | P1 | memory | GEN_SQL_SCALAR | 1390003 | 2757653 | <span style="color:#2563eb">8.08%</span> |
| 986 | [00292 SCALAR_ARITH_017](crates/bench/sqlite_parity/cases/SQLITE_PARITY_292_SCALAR_ARITH_017.rs) | P1 | memory | GEN_SQL_SCALAR | 1377489 | 2755900 | <span style="color:#2563eb">8.14%</span> |
| 987 | [00070 LIKE_GLOB_MATCH_ESCAPE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_070_LIKE_GLOB_MATCH_ESCAPE.rs) | P0 | memory | SQL_OPERATORS | 1458323 | 2755750 | <span style="color:#2563eb">8.14%</span> |
| 988 | [00363 SCALAR_NULL_COALESCE_034](crates/bench/sqlite_parity/cases/SQLITE_PARITY_363_SCALAR_NULL_COALESCE_034.rs) | P1 | memory | GEN_SQL_SCALAR | 1501914 | 2755700 | <span style="color:#2563eb">8.14%</span> |
| 989 | [00005 UNIQUE_CONSTRAINT_FAILURE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_005_UNIQUE_CONSTRAINT_FAILURE.rs) | P0 | memory | SQL_CONSTRAINTS_NEGATIVE | 1639766 | 2754668 | <span style="color:#2563eb">8.18%</span> |
| 990 | [00351 SCALAR_NULL_COALESCE_031](crates/bench/sqlite_parity/cases/SQLITE_PARITY_351_SCALAR_NULL_COALESCE_031.rs) | P1 | memory | GEN_SQL_SCALAR | 1395634 | 2753836 | <span style="color:#2563eb">8.21%</span> |
| 991 | [00071 BETWEEN_IN_ISNULL_IS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_071_BETWEEN_IN_ISNULL_IS.rs) | P0 | memory | SQL_OPERATORS | 1501044 | 2753326 | <span style="color:#2563eb">8.22%</span> |
| 992 | [00272 SCALAR_ARITH_012](crates/bench/sqlite_parity/cases/SQLITE_PARITY_272_SCALAR_ARITH_012.rs) | P1 | memory | GEN_SQL_SCALAR | 1440148 | 2752304 | <span style="color:#2563eb">8.26%</span> |
| 993 | [00256 SCALAR_ARITH_008](crates/bench/sqlite_parity/cases/SQLITE_PARITY_256_SCALAR_ARITH_008.rs) | P1 | memory | GEN_SQL_SCALAR | 1393580 | 2749779 | <span style="color:#2563eb">8.34%</span> |
| 994 | [00169 DOT_NONCE_SAFE_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_169_DOT_NONCE_SAFE_MODE.rs) | P2 | memory | CLI_DOT_COMMAND | 1725288 | 2749197 | <span style="color:#2563eb">8.36%</span> |
| 995 | [01047 JSON_EXTRACT_SET_040](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1047_JSON_EXTRACT_SET_040.rs) | P2 | memory | GEN_SQL_JSON | 1415061 | 2748977 | <span style="color:#2563eb">8.37%</span> |
| 996 | [00148 DOT_OUTPUT_TEMPFILE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_148_DOT_OUTPUT_TEMPFILE.rs) | P1 | tempfile | CLI_TEMPFILE | 1537483 | 2747584 | <span style="color:#2563eb">8.41%</span> |
| 997 | [00248 SCALAR_ARITH_006](crates/bench/sqlite_parity/cases/SQLITE_PARITY_248_SCALAR_ARITH_006.rs) | P1 | memory | GEN_SQL_SCALAR | 1472148 | 2746924 | <span style="color:#2563eb">8.44%</span> |
| 998 | [00038 CREATE_TRIGGER_AFTER](crates/bench/sqlite_parity/cases/SQLITE_PARITY_038_CREATE_TRIGGER_AFTER.rs) | P0 | memory | SQL_TRIGGER | 1544446 | 2745751 | <span style="color:#2563eb">8.47%</span> |
| 999 | [00309 SCALAR_CAST_TYPEOF_021](crates/bench/sqlite_parity/cases/SQLITE_PARITY_309_SCALAR_CAST_TYPEOF_021.rs) | P1 | memory | GEN_SQL_SCALAR | 1577899 | 2745170 | <span style="color:#2563eb">8.49%</span> |
| 1000 | [00359 SCALAR_NULL_COALESCE_033](crates/bench/sqlite_parity/cases/SQLITE_PARITY_359_SCALAR_NULL_COALESCE_033.rs) | P1 | memory | GEN_SQL_SCALAR | 1413668 | 2744659 | <span style="color:#2563eb">8.51%</span> |
| 1001 | [00089 JSON_TABLE_VALUED_FUNCTIONS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_089_JSON_TABLE_VALUED_FUNCTIONS.rs) | P0 | memory | SQL_JSON | 1490693 | 2744489 | <span style="color:#2563eb">8.52%</span> |
| 1002 | [00318 SCALAR_STRING_023](crates/bench/sqlite_parity/cases/SQLITE_PARITY_318_SCALAR_STRING_023.rs) | P1 | memory | GEN_SQL_SCALAR | 1496314 | 2744268 | <span style="color:#2563eb">8.52%</span> |
| 1003 | [00032 UNIQUE_INDEX_FAILURE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_032_UNIQUE_INDEX_FAILURE.rs) | P0 | memory | SQL_INDEX_NEGATIVE | 1485103 | 2744208 | <span style="color:#2563eb">8.53%</span> |
| 1004 | [00384 SCALAR_ARITH_040](crates/bench/sqlite_parity/cases/SQLITE_PARITY_384_SCALAR_ARITH_040.rs) | P1 | memory | GEN_SQL_SCALAR | 1425199 | 2742044 | <span style="color:#2563eb">8.60%</span> |
| 1005 | [00185 OPT_SEPARATOR](crates/bench/sqlite_parity/cases/SQLITE_PARITY_185_OPT_SEPARATOR.rs) | P1 | memory | CLI_OPTION | 1284974 | 2741924 | <span style="color:#2563eb">8.60%</span> |
| 1006 | [00326 SCALAR_STRING_025](crates/bench/sqlite_parity/cases/SQLITE_PARITY_326_SCALAR_STRING_025.rs) | P1 | memory | GEN_SQL_SCALAR | 1410131 | 2738278 | <span style="color:#2563eb">8.72%</span> |
| 1007 | [00084 CORE_FORMAT_QUOTE_HEX](crates/bench/sqlite_parity/cases/SQLITE_PARITY_084_CORE_FORMAT_QUOTE_HEX.rs) | P0 | memory | SQL_FUNCTIONS | 1457531 | 2737456 | <span style="color:#2563eb">8.75%</span> |
| 1008 | [00287 SCALAR_NULL_COALESCE_015](crates/bench/sqlite_parity/cases/SQLITE_PARITY_287_SCALAR_NULL_COALESCE_015.rs) | P1 | memory | GEN_SQL_SCALAR | 1430880 | 2736353 | <span style="color:#2563eb">8.79%</span> |
| 1009 | [01045 JSON_EXTRACT_SET_038](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1045_JSON_EXTRACT_SET_038.rs) | P2 | memory | GEN_SQL_JSON | 1405092 | 2734700 | <span style="color:#2563eb">8.84%</span> |
| 1010 | [00240 SCALAR_ARITH_004](crates/bench/sqlite_parity/cases/SQLITE_PARITY_240_SCALAR_ARITH_004.rs) | P1 | memory | GEN_SQL_SCALAR | 1479172 | 2731294 | <span style="color:#2563eb">8.96%</span> |
| 1011 | [00134 DOT_CRLF](crates/bench/sqlite_parity/cases/SQLITE_PARITY_134_DOT_CRLF.rs) | P3 | memory | CLI_DOT_COMMAND_DIAGNOSTIC | 1390625 | 2727236 | <span style="color:#2563eb">9.09%</span> |
| 1012 | [00365 SCALAR_CAST_TYPEOF_035](crates/bench/sqlite_parity/cases/SQLITE_PARITY_365_SCALAR_CAST_TYPEOF_035.rs) | P1 | memory | GEN_SQL_SCALAR | 1473060 | 2726365 | <span style="color:#2563eb">9.12%</span> |
| 1013 | [01048 JSON_EXTRACT_SET_041](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1048_JSON_EXTRACT_SET_041.rs) | P2 | memory | GEN_SQL_JSON | 1489492 | 2725312 | <span style="color:#2563eb">9.16%</span> |
| 1014 | [00356 SCALAR_ARITH_033](crates/bench/sqlite_parity/cases/SQLITE_PARITY_356_SCALAR_ARITH_033.rs) | P1 | memory | GEN_SQL_SCALAR | 1446991 | 2724911 | <span style="color:#2563eb">9.17%</span> |
| 1015 | [00172 OPT_CMD](crates/bench/sqlite_parity/cases/SQLITE_PARITY_172_OPT_CMD.rs) | P1 | memory | CLI_OPTION | 1455487 | 2724892 | <span style="color:#2563eb">9.17%</span> |
| 1016 | [00312 SCALAR_ARITH_022](crates/bench/sqlite_parity/cases/SQLITE_PARITY_312_SCALAR_ARITH_022.rs) | P1 | memory | GEN_SQL_SCALAR | 1384132 | 2723499 | <span style="color:#2563eb">9.22%</span> |
| 1017 | [00076 EXPLAIN_BYTECODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_076_EXPLAIN_BYTECODE.rs) | P0 | memory | SQL_EXPLAIN | 1610551 | 2723199 | <span style="color:#2563eb">9.23%</span> |
| 1018 | [00280 SCALAR_ARITH_014](crates/bench/sqlite_parity/cases/SQLITE_PARITY_280_SCALAR_ARITH_014.rs) | P1 | memory | GEN_SQL_SCALAR | 1475715 | 2722477 | <span style="color:#2563eb">9.25%</span> |
| 1019 | [00376 SCALAR_ARITH_038](crates/bench/sqlite_parity/cases/SQLITE_PARITY_376_SCALAR_ARITH_038.rs) | P1 | memory | GEN_SQL_SCALAR | 1486616 | 2722317 | <span style="color:#2563eb">9.26%</span> |
| 1020 | [00352 SCALAR_ARITH_032](crates/bench/sqlite_parity/cases/SQLITE_PARITY_352_SCALAR_ARITH_032.rs) | P1 | memory | GEN_SQL_SCALAR | 1411343 | 2721756 | <span style="color:#2563eb">9.27%</span> |
| 1021 | [00267 SCALAR_NULL_COALESCE_010](crates/bench/sqlite_parity/cases/SQLITE_PARITY_267_SCALAR_NULL_COALESCE_010.rs) | P1 | memory | GEN_SQL_SCALAR | 1489872 | 2721185 | <span style="color:#2563eb">9.29%</span> |
| 1022 | [01046 JSON_EXTRACT_SET_039](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1046_JSON_EXTRACT_SET_039.rs) | P2 | memory | GEN_SQL_JSON | 1451960 | 2721004 | <span style="color:#2563eb">9.30%</span> |
| 1023 | [00079 CHECK_CONSTRAINT_FAILURE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_079_CHECK_CONSTRAINT_FAILURE.rs) | P0 | memory | SQL_CONSTRAINTS_NEGATIVE | 1675965 | 2720584 | <span style="color:#2563eb">9.31%</span> |
| 1024 | [00268 SCALAR_ARITH_011](crates/bench/sqlite_parity/cases/SQLITE_PARITY_268_SCALAR_ARITH_011.rs) | P1 | memory | GEN_SQL_SCALAR | 1457561 | 2718078 | <span style="color:#2563eb">9.40%</span> |
| 1025 | [00224 OPT_STATS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_224_OPT_STATS.rs) | P3 | memory | CLI_OPTION_DIAGNOSTIC | 1489432 | 2717427 | <span style="color:#2563eb">9.42%</span> |
| 1026 | [00232 SCALAR_ARITH_002](crates/bench/sqlite_parity/cases/SQLITE_PARITY_232_SCALAR_ARITH_002.rs) | P1 | memory | GEN_SQL_SCALAR | 1448294 | 2717378 | <span style="color:#2563eb">9.42%</span> |
| 1027 | [00366 SCALAR_STRING_035](crates/bench/sqlite_parity/cases/SQLITE_PARITY_366_SCALAR_STRING_035.rs) | P1 | memory | GEN_SQL_SCALAR | 1457231 | 2716024 | <span style="color:#2563eb">9.47%</span> |
| 1028 | [00295 SCALAR_NULL_COALESCE_017](crates/bench/sqlite_parity/cases/SQLITE_PARITY_295_SCALAR_NULL_COALESCE_017.rs) | P1 | memory | GEN_SQL_SCALAR | 1451068 | 2715494 | <span style="color:#2563eb">9.48%</span> |
| 1029 | [00264 SCALAR_ARITH_010](crates/bench/sqlite_parity/cases/SQLITE_PARITY_264_SCALAR_ARITH_010.rs) | P1 | memory | GEN_SQL_SCALAR | 1431101 | 2715174 | <span style="color:#2563eb">9.49%</span> |
| 1030 | [00114 DOT_PRINT](crates/bench/sqlite_parity/cases/SQLITE_PARITY_114_DOT_PRINT.rs) | P0 | memory | CLI_DOT_COMMAND | 1429397 | 2712479 | <span style="color:#2563eb">9.58%</span> |
| 1031 | [00077 COMMENTS_AND_CLI_TERMINATORS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_077_COMMENTS_AND_CLI_TERMINATORS.rs) | P0 | memory | CLI_SQL_INPUT | 1483721 | 2707429 | <span style="color:#2563eb">9.75%</span> |
| 1032 | [00069 COLLATE_NOCASE_RTRIM_BINARY](crates/bench/sqlite_parity/cases/SQLITE_PARITY_069_COLLATE_NOCASE_RTRIM_BINARY.rs) | P0 | memory | SQL_COLLATION | 1759553 | 2704613 | <span style="color:#2563eb">9.85%</span> |
| 1033 | [00308 SCALAR_ARITH_021](crates/bench/sqlite_parity/cases/SQLITE_PARITY_308_SCALAR_ARITH_021.rs) | P1 | memory | GEN_SQL_SCALAR | 2315817 | 2703952 | <span style="color:#2563eb">9.87%</span> |
| 1034 | [00131 DOT_TIMEOUT](crates/bench/sqlite_parity/cases/SQLITE_PARITY_131_DOT_TIMEOUT.rs) | P0 | memory | CLI_DOT_COMMAND | 1748131 | 2702680 | <span style="color:#2563eb">9.91%</span> |
| 1035 | [00348 SCALAR_ARITH_031](crates/bench/sqlite_parity/cases/SQLITE_PARITY_348_SCALAR_ARITH_031.rs) | P1 | memory | GEN_SQL_SCALAR | 1446540 | 2702359 | <span style="color:#2563eb">9.92%</span> |
| 1036 | [00105 CASE_SENSITIVE_LIKE_PRAGMA](crates/bench/sqlite_parity/cases/SQLITE_PARITY_105_CASE_SENSITIVE_LIKE_PRAGMA.rs) | P2 | memory | SQL_PRAGMA | 1495262 | 2701457 | <span style="color:#2563eb">9.95%</span> |
| 1037 | [00320 SCALAR_ARITH_024](crates/bench/sqlite_parity/cases/SQLITE_PARITY_320_SCALAR_ARITH_024.rs) | P1 | memory | GEN_SQL_SCALAR | 1467741 | 2698752 | <span style="color:#2563eb">10.04%</span> |
| 1038 | [00303 SCALAR_NULL_COALESCE_019](crates/bench/sqlite_parity/cases/SQLITE_PARITY_303_SCALAR_NULL_COALESCE_019.rs) | P1 | memory | GEN_SQL_SCALAR | 1449456 | 2695696 | <span style="color:#2563eb">10.14%</span> |
| 1039 | [00132 DOT_TRACE_STDOUT](crates/bench/sqlite_parity/cases/SQLITE_PARITY_132_DOT_TRACE_STDOUT.rs) | P2 | memory | CLI_DOT_COMMAND_DIAGNOSTIC | 1484622 | 2689134 | <span style="color:#2563eb">10.36%</span> |
| 1040 | [00372 SCALAR_ARITH_037](crates/bench/sqlite_parity/cases/SQLITE_PARITY_372_SCALAR_ARITH_037.rs) | P1 | memory | GEN_SQL_SCALAR | 1393931 | 2688012 | <span style="color:#2563eb">10.40%</span> |
| 1041 | [00328 SCALAR_ARITH_026](crates/bench/sqlite_parity/cases/SQLITE_PARITY_328_SCALAR_ARITH_026.rs) | P1 | memory | GEN_SQL_SCALAR | 1529458 | 2686949 | <span style="color:#2563eb">10.44%</span> |
| 1042 | [00175 OPT_QUOTE_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_175_OPT_QUOTE_MODE.rs) | P1 | memory | CLI_OPTION | 1735478 | 2686468 | <span style="color:#2563eb">10.45%</span> |
| 1043 | [00097 CLI_GENERATE_SERIES_OPTIONAL](crates/bench/sqlite_parity/cases/SQLITE_PARITY_097_CLI_GENERATE_SERIES_OPTIONAL.rs) | P2 | memory | CLI_EXTENSION_OPTIONAL | 1674922 | 2685878 | <span style="color:#2563eb">10.47%</span> |
| 1044 | [00135 DOT_PROGRESS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_135_DOT_PROGRESS.rs) | P0 | memory | CLI_DOT_COMMAND | 1488329 | 2684915 | <span style="color:#2563eb">10.50%</span> |
| 1045 | [00009 STRICT_TABLE_TYPE_FAILURE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_009_STRICT_TABLE_TYPE_FAILURE.rs) | P0 | memory | SQL_DDL_NEGATIVE | 1417205 | 2682472 | <span style="color:#2563eb">10.58%</span> |
| 1046 | [00260 SCALAR_ARITH_009](crates/bench/sqlite_parity/cases/SQLITE_PARITY_260_SCALAR_ARITH_009.rs) | P1 | memory | GEN_SQL_SCALAR | 1372129 | 2681449 | <span style="color:#2563eb">10.62%</span> |
| 1047 | [00364 SCALAR_ARITH_035](crates/bench/sqlite_parity/cases/SQLITE_PARITY_364_SCALAR_ARITH_035.rs) | P1 | memory | GEN_SQL_SCALAR | 1382919 | 2680728 | <span style="color:#2563eb">10.64%</span> |
| 1048 | [00173 OPT_LIST_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_173_OPT_LIST_MODE.rs) | P1 | memory | CLI_OPTION | 1528355 | 2675538 | <span style="color:#2563eb">10.82%</span> |
| 1049 | [00202 OPT_APPEND_TEMPFILE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_202_OPT_APPEND_TEMPFILE.rs) | P4 | tempfile | CLI_OPTION_TEMPFILE | 1297138 | 2671520 | <span style="color:#2563eb">10.95%</span> |
| 1050 | [00221 OPT_DOUBLE_DASH_END_OPTIONS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_221_OPT_DOUBLE_DASH_END_OPTIONS.rs) | P2 | memory | CLI_OPTION | 1393169 | 2669858 | <span style="color:#2563eb">11.00%</span> |
| 1051 | [00304 SCALAR_ARITH_020](crates/bench/sqlite_parity/cases/SQLITE_PARITY_304_SCALAR_ARITH_020.rs) | P1 | memory | GEN_SQL_SCALAR | 1727101 | 2663155 | <span style="color:#2563eb">11.23%</span> |
| 1052 | [00112 DOT_SEPARATOR](crates/bench/sqlite_parity/cases/SQLITE_PARITY_112_DOT_SEPARATOR.rs) | P0 | memory | CLI_DOT_COMMAND | 1465676 | 2662694 | <span style="color:#2563eb">11.24%</span> |
| 1053 | [00144 DOT_PROMPT](crates/bench/sqlite_parity/cases/SQLITE_PARITY_144_DOT_PROMPT.rs) | P0 | memory | CLI_DOT_COMMAND | 1424218 | 2655590 | <span style="color:#2563eb">11.48%</span> |
| 1054 | [00066 VALUES_STATEMENT](crates/bench/sqlite_parity/cases/SQLITE_PARITY_066_VALUES_STATEMENT.rs) | P0 | memory | SQL_VALUES | 1407596 | 2653847 | <span style="color:#2563eb">11.54%</span> |
| 1055 | [00136 DOT_LOG](crates/bench/sqlite_parity/cases/SQLITE_PARITY_136_DOT_LOG.rs) | P0 | memory | CLI_DOT_COMMAND | 1459876 | 2650551 | <span style="color:#2563eb">11.65%</span> |
| 1056 | [00109 DOT_MODE_JSON](crates/bench/sqlite_parity/cases/SQLITE_PARITY_109_DOT_MODE_JSON.rs) | P0 | memory | CLI_DOT_COMMAND | 1397637 | 2644779 | <span style="color:#2563eb">11.84%</span> |
| 1057 | [00111 DOT_HEADERS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_111_DOT_HEADERS.rs) | P0 | memory | CLI_DOT_COMMAND | 1464313 | 2644299 | <span style="color:#2563eb">11.86%</span> |
| 1058 | [00123 DOT_ECHO](crates/bench/sqlite_parity/cases/SQLITE_PARITY_123_DOT_ECHO.rs) | P0 | memory | CLI_DOT_COMMAND | 1366308 | 2641594 | <span style="color:#2563eb">11.95%</span> |
| 1059 | [00368 SCALAR_ARITH_036](crates/bench/sqlite_parity/cases/SQLITE_PARITY_368_SCALAR_ARITH_036.rs) | P1 | memory | GEN_SQL_SCALAR | 1368522 | 2630473 | <span style="color:#2563eb">12.32%</span> |
| 1060 | [00223 OPT_NOHEADER](crates/bench/sqlite_parity/cases/SQLITE_PARITY_223_OPT_NOHEADER.rs) | P2 | memory | CLI_OPTION | 1424037 | 2629771 | <span style="color:#2563eb">12.34%</span> |
| 1061 | [00125 DOT_TIMER](crates/bench/sqlite_parity/cases/SQLITE_PARITY_125_DOT_TIMER.rs) | P3 | memory | CLI_DOT_COMMAND_DIAGNOSTIC | 1442011 | 2625032 | <span style="color:#2563eb">12.50%</span> |
| 1062 | [00126 DOT_STATS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_126_DOT_STATS.rs) | P3 | memory | CLI_DOT_COMMAND_DIAGNOSTIC | 1422876 | 2623249 | <span style="color:#2563eb">12.56%</span> |
| 1063 | [00174 OPT_CSV_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_174_OPT_CSV_MODE.rs) | P1 | memory | CLI_OPTION | 1336491 | 2621095 | <span style="color:#2563eb">12.63%</span> |
| 1064 | [00113 DOT_NULLVALUE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_113_DOT_NULLVALUE.rs) | P0 | memory | CLI_DOT_COMMAND | 1371017 | 2619712 | <span style="color:#2563eb">12.68%</span> |
| 1065 | [00139 DOT_LINT_FKEY_INDEXES](crates/bench/sqlite_parity/cases/SQLITE_PARITY_139_DOT_LINT_FKEY_INDEXES.rs) | P3 | memory | CLI_DOT_COMMAND_DIAGNOSTIC | 1587016 | 2618871 | <span style="color:#2563eb">12.70%</span> |
| 1066 | [00120 DOT_EXPLAIN](crates/bench/sqlite_parity/cases/SQLITE_PARITY_120_DOT_EXPLAIN.rs) | P0 | memory | CLI_DOT_COMMAND | 1464985 | 2606938 | <span style="color:#2563eb">13.10%</span> |
| 1067 | [00124 DOT_BAIL_OFF](crates/bench/sqlite_parity/cases/SQLITE_PARITY_124_DOT_BAIL_OFF.rs) | P0 | memory | CLI_DOT_COMMAND_NEGATIVE | 1645156 | 2567844 | <span style="color:#2563eb">14.41%</span> |
| 1068 | [00192 OPT_INIT_TEMPFILE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_192_OPT_INIT_TEMPFILE.rs) | P2 | tempfile | CLI_OPTION_TEMPFILE | 1244256 | 2564367 | <span style="color:#2563eb">14.52%</span> |
| 1069 | [00250 SCALAR_STRING_006](crates/bench/sqlite_parity/cases/SQLITE_PARITY_250_SCALAR_STRING_006.rs) | P1 | memory | GEN_SQL_SCALAR | 1454485 | 2514103 | <span style="color:#2563eb">16.20%</span> |
| 1070 | [00201 OPT_NO_ROWID_IN_VIEW](crates/bench/sqlite_parity/cases/SQLITE_PARITY_201_OPT_NO_ROWID_IN_VIEW.rs) | P4 | memory | CLI_OPTION | 1323457 | 2512830 | <span style="color:#2563eb">16.24%</span> |
| 1071 | [00276 SCALAR_ARITH_013](crates/bench/sqlite_parity/cases/SQLITE_PARITY_276_SCALAR_ARITH_013.rs) | P1 | memory | GEN_SQL_SCALAR | 2389156 | 2508462 | <span style="color:#2563eb">16.38%</span> |
| 1072 | [00164 DOT_IMPOSTER_CATALOG](crates/bench/sqlite_parity/cases/SQLITE_PARITY_164_DOT_IMPOSTER_CATALOG.rs) | P4 | catalog | CLI_CATALOG | 1377740 | 2506377 | <span style="color:#2563eb">16.45%</span> |
| 1073 | [00181 OPT_TABLE_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_181_OPT_TABLE_MODE.rs) | P2 | memory | CLI_OPTION | 1316554 | 2499134 | <span style="color:#2563eb">16.70%</span> |
| 1074 | [00186 OPT_NEWLINE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_186_OPT_NEWLINE.rs) | P2 | memory | CLI_OPTION | 1638514 | 2477633 | <span style="color:#2563eb">17.41%</span> |
| 1075 | [00188 OPT_HEADER](crates/bench/sqlite_parity/cases/SQLITE_PARITY_188_OPT_HEADER.rs) | P1 | memory | CLI_OPTION | 1613546 | 2447736 | <span style="color:#2563eb">18.41%</span> |
| 1076 | [00176 OPT_LINE_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_176_OPT_LINE_MODE.rs) | P2 | memory | CLI_OPTION | 1337253 | 2422418 | <span style="color:#2563eb">19.25%</span> |
| 1077 | [00198 OPT_LOOKASIDE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_198_OPT_LOOKASIDE.rs) | P3 | memory | CLI_OPTION | 1382609 | 2419213 | <span style="color:#2563eb">19.36%</span> |
| 1078 | [00199 OPT_PAGECACHE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_199_OPT_PAGECACHE.rs) | P3 | memory | CLI_OPTION | 1386617 | 2417609 | <span style="color:#2563eb">19.41%</span> |
| 1079 | [00196 OPT_MMAP](crates/bench/sqlite_parity/cases/SQLITE_PARITY_196_OPT_MMAP.rs) | P3 | memory | CLI_OPTION | 1370717 | 2414423 | <span style="color:#2563eb">19.52%</span> |
| 1080 | [00187 OPT_NULLVALUE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_187_OPT_NULLVALUE.rs) | P1 | memory | CLI_OPTION | 1681486 | 2404134 | <span style="color:#2563eb">19.86%</span> |
| 1081 | [00179 OPT_MARKDOWN_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_179_OPT_MARKDOWN_MODE.rs) | P2 | memory | CLI_OPTION | 1334949 | 2385769 | <span style="color:#2563eb">20.47%</span> |
| 1082 | [00048 PRAGMA_USER_VERSION](crates/bench/sqlite_parity/cases/SQLITE_PARITY_048_PRAGMA_USER_VERSION.rs) | P0 | memory | SQL_PRAGMA | 1469534 | 2366783 | <span style="color:#2563eb">21.11%</span> |
| 1083 | [00191 OPT_BATCH](crates/bench/sqlite_parity/cases/SQLITE_PARITY_191_OPT_BATCH.rs) | P2 | memory | CLI_OPTION | 1346290 | 2362274 | <span style="color:#2563eb">21.26%</span> |
| 1084 | [00177 OPT_JSON_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_177_OPT_JSON_MODE.rs) | P1 | memory | CLI_OPTION | 1358654 | 2353588 | <span style="color:#2563eb">21.55%</span> |
| 1085 | [00153 DOT_CD_TEMPFILE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_153_DOT_CD_TEMPFILE.rs) | P2 | tempfile | CLI_TEMPFILE | 1264195 | 2352837 | <span style="color:#2563eb">21.57%</span> |
| 1086 | [00178 OPT_HTML_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_178_OPT_HTML_MODE.rs) | P2 | memory | CLI_OPTION | 1358925 | 2350292 | <span style="color:#2563eb">21.66%</span> |
| 1087 | [00180 OPT_BOX_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_180_OPT_BOX_MODE.rs) | P2 | memory | CLI_OPTION | 1360297 | 2341796 | <span style="color:#2563eb">21.94%</span> |
| 1088 | [00200 OPT_HEAP](crates/bench/sqlite_parity/cases/SQLITE_PARITY_200_OPT_HEAP.rs) | P4 | memory | CLI_OPTION | 1280325 | 2321828 | <span style="color:#2563eb">22.61%</span> |
| 1089 | [00182 OPT_COLUMN_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_182_OPT_COLUMN_MODE.rs) | P2 | memory | CLI_OPTION | 1269595 | 2313121 | <span style="color:#2563eb">22.90%</span> |
| 1090 | [00157 DOT_ARCHIVE_TEMPFILE_OPTIONAL](crates/bench/sqlite_parity/cases/SQLITE_PARITY_157_DOT_ARCHIVE_TEMPFILE_OPTIONAL.rs) | P3 | tempfile | CLI_TEMPFILE_OPTIONAL | 1935596 | 2307211 | <span style="color:#2563eb">23.09%</span> |
| 1091 | [00206 OPT_MEMTRACE_CATALOG](crates/bench/sqlite_parity/cases/SQLITE_PARITY_206_OPT_MEMTRACE_CATALOG.rs) | P4 | catalog | CLI_OPTION_CATALOG | 1948441 | 2289817 | <span style="color:#2563eb">23.67%</span> |
| 1092 | [00207 OPT_PCACHETRACE_CATALOG](crates/bench/sqlite_parity/cases/SQLITE_PARITY_207_OPT_PCACHETRACE_CATALOG.rs) | P4 | catalog | CLI_OPTION_CATALOG | 1311644 | 2288505 | <span style="color:#2563eb">23.72%</span> |
| 1093 | [00205 OPT_VFS_CATALOG](crates/bench/sqlite_parity/cases/SQLITE_PARITY_205_OPT_VFS_CATALOG.rs) | P4 | catalog | CLI_OPTION_CATALOG | 1276538 | 2285629 | <span style="color:#2563eb">23.81%</span> |
| 1094 | [00208 OPT_VFSTRACE_CATALOG](crates/bench/sqlite_parity/cases/SQLITE_PARITY_208_OPT_VFSTRACE_CATALOG.rs) | P4 | catalog | CLI_OPTION_CATALOG | 1292378 | 2276152 | <span style="color:#2563eb">24.13%</span> |
| 1095 | [00183 OPT_TABS_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_183_OPT_TABS_MODE.rs) | P2 | memory | CLI_OPTION | 1258243 | 2268096 | <span style="color:#2563eb">24.40%</span> |
| 1096 | [00184 OPT_ASCII_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_184_OPT_ASCII_MODE.rs) | P2 | memory | CLI_OPTION | 1254747 | 2265352 | <span style="color:#2563eb">24.49%</span> |
| 1097 | [00047 PRAGMA_FOREIGN_KEYS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_047_PRAGMA_FOREIGN_KEYS.rs) | P0 | memory | SQL_PRAGMA | 1569523 | 2259450 | <span style="color:#2563eb">24.69%</span> |
| 1098 | [00166 DOT_SESSION_CATALOG](crates/bench/sqlite_parity/cases/SQLITE_PARITY_166_DOT_SESSION_CATALOG.rs) | P4 | catalog | CLI_CATALOG | 1530288 | 2251605 | <span style="color:#2563eb">24.95%</span> |
| 1099 | [00218 PRAGMA_FORMS_SCHEMA_EQUALS_PARENS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_218_PRAGMA_FORMS_SCHEMA_EQUALS_PARENS.rs) | P0 | memory | SQL_PRAGMA | 1428265 | 2248439 | <span style="color:#2563eb">25.05%</span> |
| 1100 | [00190 OPT_BAIL](crates/bench/sqlite_parity/cases/SQLITE_PARITY_190_OPT_BAIL.rs) | P1 | memory | CLI_OPTION_NEGATIVE | 1274324 | 2244281 | <span style="color:#2563eb">25.19%</span> |
| 1101 | [00107 DOT_HELP_PATTERN](crates/bench/sqlite_parity/cases/SQLITE_PARITY_107_DOT_HELP_PATTERN.rs) | P0 | memory | CLI_DOT_COMMAND | 1356259 | 2136748 | <span style="color:#2563eb">28.78%</span> |
| 1102 | [00049 PRAGMA_TEMP_STORE_MEMORY](crates/bench/sqlite_parity/cases/SQLITE_PARITY_049_PRAGMA_TEMP_STORE_MEMORY.rs) | P0 | memory | SQL_PRAGMA | 1430750 | 2133882 | <span style="color:#2563eb">28.87%</span> |
| 1103 | [00116 DOT_DATABASES](crates/bench/sqlite_parity/cases/SQLITE_PARITY_116_DOT_DATABASES.rs) | P0 | memory | CLI_DOT_COMMAND | 1442783 | 2109656 | <span style="color:#2563eb">29.68%</span> |
| 1104 | [00106 DOT_HELP](crates/bench/sqlite_parity/cases/SQLITE_PARITY_106_DOT_HELP.rs) | P0 | memory | CLI_DOT_COMMAND | 1416673 | 2077746 | <span style="color:#2563eb">30.74%</span> |
| 1105 | [00128 DOT_DBCONFIG](crates/bench/sqlite_parity/cases/SQLITE_PARITY_128_DOT_DBCONFIG.rs) | P0 | memory | CLI_DOT_COMMAND | 1469593 | 2061425 | <span style="color:#2563eb">31.29%</span> |
| 1106 | [00168 DOT_CHECK_CATALOG](crates/bench/sqlite_parity/cases/SQLITE_PARITY_168_DOT_CHECK_CATALOG.rs) | P4 | catalog | CLI_CATALOG | 1408919 | 2035917 | <span style="color:#2563eb">32.14%</span> |
| 1107 | [00137 DOT_VERSION](crates/bench/sqlite_parity/cases/SQLITE_PARITY_137_DOT_VERSION.rs) | P2 | memory | CLI_DOT_COMMAND_DIAGNOSTIC | 1431542 | 2022591 | <span style="color:#2563eb">32.58%</span> |
| 1108 | [00138 DOT_VFSNAME_LIST_INFO](crates/bench/sqlite_parity/cases/SQLITE_PARITY_138_DOT_VFSNAME_LIST_INFO.rs) | P3 | memory | CLI_DOT_COMMAND_DIAGNOSTIC | 1624607 | 2019565 | <span style="color:#2563eb">32.68%</span> |
| 1109 | [00163 DOT_FILECTRL_CATALOG](crates/bench/sqlite_parity/cases/SQLITE_PARITY_163_DOT_FILECTRL_CATALOG.rs) | P4 | catalog | CLI_CATALOG | 1530820 | 2011250 | <span style="color:#2563eb">32.96%</span> |
| 1110 | [00127 DOT_LIMIT](crates/bench/sqlite_parity/cases/SQLITE_PARITY_127_DOT_LIMIT.rs) | P0 | memory | CLI_DOT_COMMAND | 1371228 | 2001762 | <span style="color:#2563eb">33.27%</span> |
| 1111 | [00159 DOT_SYSTEM_SIDE_EFFECT](crates/bench/sqlite_parity/cases/SQLITE_PARITY_159_DOT_SYSTEM_SIDE_EFFECT.rs) | P4 | side_effect | CLI_SIDE_EFFECT | 1910479 | 2000430 | <span style="color:#2563eb">33.32%</span> |
| 1112 | [00167 DOT_UNMODULE_CATALOG](crates/bench/sqlite_parity/cases/SQLITE_PARITY_167_DOT_UNMODULE_CATALOG.rs) | P4 | catalog | CLI_CATALOG | 1470165 | 1990811 | <span style="color:#2563eb">33.64%</span> |
| 1113 | [00195 OPT_SAFE_MODE_BLOCKS_SHELL](crates/bench/sqlite_parity/cases/SQLITE_PARITY_195_OPT_SAFE_MODE_BLOCKS_SHELL.rs) | P2 | memory | CLI_OPTION_NEGATIVE | 1262251 | 1951537 | <span style="color:#2563eb">34.95%</span> |
| 1114 | [00162 DOT_LOAD_EXTENSION_NEGATIVE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_162_DOT_LOAD_EXTENSION_NEGATIVE.rs) | P4 | side_effect | CLI_SIDE_EFFECT | 1408167 | 1877306 | <span style="color:#2563eb">37.42%</span> |
| 1115 | [00143 DOT_QUIT](crates/bench/sqlite_parity/cases/SQLITE_PARITY_143_DOT_QUIT.rs) | P0 | memory | CLI_DOT_COMMAND | 1439768 | 1869020 | <span style="color:#2563eb">37.70%</span> |
| 1116 | [00158 DOT_SHELL_SIDE_EFFECT](crates/bench/sqlite_parity/cases/SQLITE_PARITY_158_DOT_SHELL_SIDE_EFFECT.rs) | P4 | side_effect | CLI_SIDE_EFFECT | 1903576 | 1771476 | <span style="color:#2563eb">40.95%</span> |
| 1117 | [00142 DOT_EXIT_CODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_142_DOT_EXIT_CODE.rs) | P0 | memory | CLI_DOT_COMMAND | 1394552 | 1763079 | <span style="color:#2563eb">41.23%</span> |
| 1118 | [00165 DOT_INTCK_CATALOG](crates/bench/sqlite_parity/cases/SQLITE_PARITY_165_DOT_INTCK_CATALOG.rs) | P4 | catalog | CLI_CATALOG | 3763970 | 2193385 | <span style="color:#2563eb">41.73%</span> |
| 1119 | [00226 OPT_NOFOLLOW_CATALOG](crates/bench/sqlite_parity/cases/SQLITE_PARITY_226_OPT_NOFOLLOW_CATALOG.rs) | P4 | catalog | CLI_OPTION_CATALOG | 1268392 | 1652260 | <span style="color:#2563eb">44.92%</span> |
| 1120 | [00170 OPT_VERSION](crates/bench/sqlite_parity/cases/SQLITE_PARITY_170_OPT_VERSION.rs) | P1 | memory | CLI_OPTION | 1309361 | 1573942 | <span style="color:#2563eb">47.54%</span> |
| 1121 | [00171 OPT_HELP](crates/bench/sqlite_parity/cases/SQLITE_PARITY_171_OPT_HELP.rs) | P1 | memory | CLI_OPTION | 1359786 | 1564875 | <span style="color:#2563eb">47.84%</span> |
| 1122 | [00194 OPT_IFEXISTS_NEGATIVE_TEMPFILE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_194_OPT_IFEXISTS_NEGATIVE_TEMPFILE.rs) | P3 | tempfile | CLI_OPTION_TEMPFILE_DIAGNOSTIC | 1209421 | 1516263 | <span style="color:#2563eb">49.46%</span> |
| 1123 | [00204 OPT_ZIP_TEMPFILE_CATALOG](crates/bench/sqlite_parity/cases/SQLITE_PARITY_204_OPT_ZIP_TEMPFILE_CATALOG.rs) | P4 | catalog | CLI_OPTION_CATALOG | 1420150 | 1408829 | <span style="color:#2563eb">53.04%</span> |
| 1124 | [00203 OPT_ARCHIVE_A_TEMPFILE_OPTIONAL](crates/bench/sqlite_parity/cases/SQLITE_PARITY_203_OPT_ARCHIVE_A_TEMPFILE_OPTIONAL.rs) | P4 | tempfile | CLI_OPTION_TEMPFILE_OPTIONAL | 1568150 | 1195104 | <span style="color:#2563eb">60.16%</span> |
| 1125 | [00160 DOT_EXCEL_EXTERNAL_APP](crates/bench/sqlite_parity/cases/SQLITE_PARITY_160_DOT_EXCEL_EXTERNAL_APP.rs) | P4 | external_app | CLI_EXTERNAL_APP | 27010024 | 2456433 | <span style="color:#2563eb">90.91%</span> |
| 1126 | [00161 DOT_WWW_EXTERNAL_APP](crates/bench/sqlite_parity/cases/SQLITE_PARITY_161_DOT_WWW_EXTERNAL_APP.rs) | P4 | external_app | CLI_EXTERNAL_APP | 26994043 | 2391841 | <span style="color:#2563eb">91.14%</span> |
| 1127 | [00209 OPT_INTERACTIVE_CATALOG](crates/bench/sqlite_parity/cases/SQLITE_PARITY_209_OPT_INTERACTIVE_CATALOG.rs) | P4 | catalog | CLI_OPTION_CATALOG | 51542303 | 2012462 | <span style="color:#2563eb">96.10%</span> |

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
