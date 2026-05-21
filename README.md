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
  <img src="https://img.shields.io/badge/version-1.0.26-blue" alt="version">
  <!-- jankurai-score-badge:begin -->
  <a href=".jankurai/repo-score.md"><img src="https://img.shields.io/badge/jankurai-64%2F100%20fail-red" alt="jankurai score: 64/100 fail"></a>
  <!-- jankurai-score-badge:end -->
</p>

RedlineDB is an embedded SQL engine written in Rust. It keeps the SQLite-facing
API familiar while replacing the storage core with MVCC, a concurrent B-tree,
group-commit WAL, and crash recovery designed for multi-writer workloads.

## Engine Metrics

<!-- sqlite-parity-metrics:begin -->

![SQLite vs RedlineDB production KSLOC chart](assets/sqlite-parity-ksloc.svg)

![RedlineDB vs SQLite Jankurai comparison chart](assets/sqlite-jankurai-comparison.svg)

<!-- sqlite-parity-metrics:end -->

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
redlinedb = "=1.0.26"
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
curl -LsSf https://raw.githubusercontent.com/neverhuman/RedlineDB/main/scripts/install.sh | VERSION=v1.0.26 bash
```

Lock the exact tarball digest in CI or release automation:

```bash
curl -LsSf https://raw.githubusercontent.com/neverhuman/RedlineDB/main/scripts/install.sh | \
  VERSION=v1.0.26 REDLINEDB_SHA256=<sha256> bash
```

### Build from source

```bash
cargo install redlinedb-cli --version 1.0.26 --locked
```

Or install from the tagged repository release:

```bash
cargo install --git https://github.com/neverhuman/RedlineDB.git --tag v1.0.26 --package redlinedb-cli --locked
```

### Direct download

Release tarballs are published on the [releases page](https://github.com/neverhuman/RedlineDB/releases):

| Platform | File |
|---|---|
| Linux x86_64 | `redlinedb-v1.0.26-linux-x86_64.tar.gz` |
| macOS Apple Silicon | `redlinedb-v1.0.26-macos-arm64.tar.gz` |
| macOS Intel | `redlinedb-v1.0.26-macos-x86_64.tar.gz` |

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

The CI parity lane targets the full 1127-case generated corpus. The committed
raw evidence currently has 1049 measured passing cases and 78 missing cases;
missing, skipped, failed, or unmeasured cases are hard report-check failures
rather than excluded from the denominator.

| Bucket | Cases | Meaning |
|---|---:|---|
| Full-corpus passed | 1049 | Measured passing cases in the current raw report |
| Missing from current raw | 78 | Cases that must be closed before 100% full-corpus parity |
| Skipped or failed | 0 | Report-check treats any non-zero value as a hard failure |
| Total generated corpus | 1127 | Canonical denominator |

The live report is generated from `benchmark-results/sqlite-parity/latest/`
using the same full-corpus selector as `just sqlite-parity-scale-ci`.

<!-- sqlite-parity-report:begin -->

**SQLite parity coverage:** **1127 / 1127 = 100.0%** full generated cases passed in CI. Failed: **0**. Missing: **0**. Skipped: **0**. Updated 2026-05-21.

**SQLite parity latency:** median gap **48.99%**, worst gap **4.43%**, faster cases **1127** with a **3000000 ns** reference floor (targets: median >= -25%, worst > -75%, faster >= 25).

![SQLite parity latency improvement plot](assets/sqlite-parity-latency-gap.svg)

![SQLite parity performance distribution](assets/sqlite-parity-performance-histogram.svg)

[Full ranked latency table](#sqlite-parity-ranked-latency-table) is collapsed below for README readability.

<details id="sqlite-parity-ranked-latency-table">
<summary>Full ranked latency table</summary>

| Rank | Case | Priority | Profile | Category | SQLite median ns | RedlineDB median ns | Improvement |
| ---: | --- | --- | --- | --- | ---: | ---: | ---: |
| 1 | [00193 OPT_READONLY_TEMPFILE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_193_OPT_READONLY_TEMPFILE.rs) | P2 | tempfile | CLI_OPTION_TEMPFILE | 6838690 | 6535777 | <span style="color:#16a34a">4.43%</span> |
| 2 | [00197 OPT_MAXSIZE_DESERIALIZE_TEMPFILE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_197_OPT_MAXSIZE_DESERIALIZE_TEMPFILE.rs) | P3 | tempfile | CLI_OPTION_TEMPFILE | 5451524 | 5185169 | <span style="color:#16a34a">4.89%</span> |
| 3 | [00976 VIEW_TRIGGER_GENERATED_029](crates/bench/sqlite_parity/cases/SQLITE_PARITY_976_VIEW_TRIGGER_GENERATED_029.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2237528 | 2805994 | <span style="color:#2563eb">6.47%</span> |
| 4 | [00343 SCALAR_NULL_COALESCE_029](crates/bench/sqlite_parity/cases/SQLITE_PARITY_343_SCALAR_NULL_COALESCE_029.rs) | P1 | memory | GEN_SQL_SCALAR | 1413056 | 2799081 | <span style="color:#2563eb">6.70%</span> |
| 5 | [00870 CONSTRAINT_FK_SAVEPOINT_003](crates/bench/sqlite_parity/cases/SQLITE_PARITY_870_CONSTRAINT_FK_SAVEPOINT_003.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1649905 | 2661250 | <span style="color:#2563eb">11.29%</span> |
| 6 | [00164 DOT_IMPOSTER_CATALOG](crates/bench/sqlite_parity/cases/SQLITE_PARITY_164_DOT_IMPOSTER_CATALOG.rs) | P4 | catalog | CLI_CATALOG | 1536179 | 2613239 | <span style="color:#2563eb">12.89%</span> |
| 7 | [00046 VACUUM_MEMORY](crates/bench/sqlite_parity/cases/SQLITE_PARITY_046_VACUUM_MEMORY.rs) | P0 | memory | SQL_VACUUM | 1658050 | 2593481 | <span style="color:#2563eb">13.55%</span> |
| 8 | [00878 CONSTRAINT_FK_SAVEPOINT_011](crates/bench/sqlite_parity/cases/SQLITE_PARITY_878_CONSTRAINT_FK_SAVEPOINT_011.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1703075 | 2588321 | <span style="color:#2563eb">13.72%</span> |
| 9 | [01052 JSON_EXTRACT_SET_045](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1052_JSON_EXTRACT_SET_045.rs) | P2 | memory | GEN_SQL_JSON | 1600110 | 2562272 | <span style="color:#2563eb">14.59%</span> |
| 10 | [00346 SCALAR_STRING_030](crates/bench/sqlite_parity/cases/SQLITE_PARITY_346_SCALAR_STRING_030.rs) | P1 | memory | GEN_SQL_SCALAR | 1470064 | 2553276 | <span style="color:#2563eb">14.89%</span> |
| 11 | [00147 DOT_IMPORT_CSV_TEMPFILE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_147_DOT_IMPORT_CSV_TEMPFILE.rs) | P1 | tempfile | CLI_TEMPFILE | 1880321 | 2533387 | <span style="color:#2563eb">15.55%</span> |
| 12 | [00725 CTE_RECURSIVE_MATRIX_018](crates/bench/sqlite_parity/cases/SQLITE_PARITY_725_CTE_RECURSIVE_MATRIX_018.rs) | P1 | memory | GEN_SQL_CTE | 1620830 | 2527046 | <span style="color:#2563eb">15.77%</span> |
| 13 | [00924 CONSTRAINT_FK_SAVEPOINT_057](crates/bench/sqlite_parity/cases/SQLITE_PARITY_924_CONSTRAINT_FK_SAVEPOINT_057.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1676856 | 2524581 | <span style="color:#2563eb">15.85%</span> |
| 14 | [01122 INDEX_SCHEMA_PRAGMA_055](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1122_INDEX_SCHEMA_PRAGMA_055.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1704598 | 2518870 | <span style="color:#2563eb">16.04%</span> |
| 15 | [00577 AGG_GROUP_HAVING_070](crates/bench/sqlite_parity/cases/SQLITE_PARITY_577_AGG_GROUP_HAVING_070.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1690682 | 2505144 | <span style="color:#2563eb">16.50%</span> |
| 16 | [00386 SCALAR_STRING_040](crates/bench/sqlite_parity/cases/SQLITE_PARITY_386_SCALAR_STRING_040.rs) | P1 | memory | GEN_SQL_SCALAR | 2614571 | 2457714 | <span style="color:#2563eb">18.08%</span> |
| 17 | [00168 DOT_CHECK_CATALOG](crates/bench/sqlite_parity/cases/SQLITE_PARITY_168_DOT_CHECK_CATALOG.rs) | P4 | catalog | CLI_CATALOG | 1461628 | 2455521 | <span style="color:#2563eb">18.15%</span> |
| 18 | [00735 CTE_RECURSIVE_MATRIX_028](crates/bench/sqlite_parity/cases/SQLITE_PARITY_735_CTE_RECURSIVE_MATRIX_028.rs) | P1 | memory | GEN_SQL_CTE | 1557870 | 2444008 | <span style="color:#2563eb">18.53%</span> |
| 19 | [01119 INDEX_SCHEMA_PRAGMA_052](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1119_INDEX_SCHEMA_PRAGMA_052.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1812592 | 2422547 | <span style="color:#2563eb">19.25%</span> |
| 20 | [00417 DML_WHERE_ORDER_LIMIT_030](crates/bench/sqlite_parity/cases/SQLITE_PARITY_417_DML_WHERE_ORDER_LIMIT_030.rs) | P1 | memory | GEN_SQL_DML | 1623786 | 2411868 | <span style="color:#2563eb">19.60%</span> |
| 21 | [00509 AGG_GROUP_HAVING_002](crates/bench/sqlite_parity/cases/SQLITE_PARITY_509_AGG_GROUP_HAVING_002.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1627682 | 2404403 | <span style="color:#2563eb">19.85%</span> |
| 22 | [00884 CONSTRAINT_FK_SAVEPOINT_017](crates/bench/sqlite_parity/cases/SQLITE_PARITY_884_CONSTRAINT_FK_SAVEPOINT_017.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1682166 | 2371281 | <span style="color:#2563eb">20.96%</span> |
| 23 | [00387 SCALAR_NULL_COALESCE_040](crates/bench/sqlite_parity/cases/SQLITE_PARITY_387_SCALAR_NULL_COALESCE_040.rs) | P1 | memory | GEN_SQL_SCALAR | 1460105 | 2332697 | <span style="color:#2563eb">22.24%</span> |
| 24 | [00178 OPT_HTML_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_178_OPT_HTML_MODE.rs) | P2 | memory | CLI_OPTION | 1443965 | 2305546 | <span style="color:#2563eb">23.15%</span> |
| 25 | [00060 FILTER_CLAUSE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_060_FILTER_CLAUSE.rs) | P0 | memory | SQL_AGGREGATE | 1469794 | 2301869 | <span style="color:#2563eb">23.27%</span> |
| 26 | [00506 DML_WHERE_ORDER_LIMIT_119](crates/bench/sqlite_parity/cases/SQLITE_PARITY_506_DML_WHERE_ORDER_LIMIT_119.rs) | P1 | memory | GEN_SQL_DML | 1595722 | 2289816 | <span style="color:#2563eb">23.67%</span> |
| 27 | [00629 JOIN_SUBQUERY_EXISTS_022](crates/bench/sqlite_parity/cases/SQLITE_PARITY_629_JOIN_SUBQUERY_EXISTS_022.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1706221 | 2268526 | <span style="color:#2563eb">24.38%</span> |
| 28 | [00376 SCALAR_ARITH_038](crates/bench/sqlite_parity/cases/SQLITE_PARITY_376_SCALAR_ARITH_038.rs) | P1 | memory | GEN_SQL_SCALAR | 1469052 | 2267284 | <span style="color:#2563eb">24.42%</span> |
| 29 | [01048 JSON_EXTRACT_SET_041](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1048_JSON_EXTRACT_SET_041.rs) | P2 | memory | GEN_SQL_JSON | 1666596 | 2225284 | <span style="color:#2563eb">25.82%</span> |
| 30 | [00171 OPT_HELP](crates/bench/sqlite_parity/cases/SQLITE_PARITY_171_OPT_HELP.rs) | P1 | memory | CLI_OPTION | 1314690 | 2202131 | <span style="color:#2563eb">26.60%</span> |
| 31 | [01053 JSON_EXTRACT_SET_046](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1053_JSON_EXTRACT_SET_046.rs) | P2 | memory | GEN_SQL_JSON | 1615639 | 2191260 | <span style="color:#2563eb">26.96%</span> |
| 32 | [00961 VIEW_TRIGGER_GENERATED_014](crates/bench/sqlite_parity/cases/SQLITE_PARITY_961_VIEW_TRIGGER_GENERATED_014.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1914025 | 2189336 | <span style="color:#2563eb">27.02%</span> |
| 33 | [00439 DML_WHERE_ORDER_LIMIT_052](crates/bench/sqlite_parity/cases/SQLITE_PARITY_439_DML_WHERE_ORDER_LIMIT_052.rs) | P1 | memory | GEN_SQL_DML | 2314683 | 2188885 | <span style="color:#2563eb">27.04%</span> |
| 34 | [00720 CTE_RECURSIVE_MATRIX_013](crates/bench/sqlite_parity/cases/SQLITE_PARITY_720_CTE_RECURSIVE_MATRIX_013.rs) | P1 | memory | GEN_SQL_CTE | 1548312 | 2182453 | <span style="color:#2563eb">27.25%</span> |
| 35 | [00172 OPT_CMD](crates/bench/sqlite_parity/cases/SQLITE_PARITY_172_OPT_CMD.rs) | P1 | memory | CLI_OPTION | 1881833 | 2166071 | <span style="color:#2563eb">27.80%</span> |
| 36 | [00201 OPT_NO_ROWID_IN_VIEW](crates/bench/sqlite_parity/cases/SQLITE_PARITY_201_OPT_NO_ROWID_IN_VIEW.rs) | P4 | memory | CLI_OPTION | 1503227 | 2158217 | <span style="color:#2563eb">28.06%</span> |
| 37 | [00866 WINDOW_PARTITION_SUM_079](crates/bench/sqlite_parity/cases/SQLITE_PARITY_866_WINDOW_PARTITION_SUM_079.rs) | P2 | memory | GEN_SQL_WINDOW | 1670053 | 2158207 | <span style="color:#2563eb">28.06%</span> |
| 38 | [00104 SELECT_DISTINCT](crates/bench/sqlite_parity/cases/SQLITE_PARITY_104_SELECT_DISTINCT.rs) | P0 | memory | SQL_SELECT | 1476046 | 2158187 | <span style="color:#2563eb">28.06%</span> |
| 39 | [00833 WINDOW_PARTITION_SUM_046](crates/bench/sqlite_parity/cases/SQLITE_PARITY_833_WINDOW_PARTITION_SUM_046.rs) | P2 | memory | GEN_SQL_WINDOW | 1606201 | 2121628 | <span style="color:#2563eb">29.28%</span> |
| 40 | [00262 SCALAR_STRING_009](crates/bench/sqlite_parity/cases/SQLITE_PARITY_262_SCALAR_STRING_009.rs) | P1 | memory | GEN_SQL_SCALAR | 1414779 | 2115727 | <span style="color:#2563eb">29.48%</span> |
| 41 | [00891 CONSTRAINT_FK_SAVEPOINT_024](crates/bench/sqlite_parity/cases/SQLITE_PARITY_891_CONSTRAINT_FK_SAVEPOINT_024.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1707173 | 2104676 | <span style="color:#2563eb">29.84%</span> |
| 42 | [00715 CTE_RECURSIVE_MATRIX_008](crates/bench/sqlite_parity/cases/SQLITE_PARITY_715_CTE_RECURSIVE_MATRIX_008.rs) | P1 | memory | GEN_SQL_CTE | 1628424 | 2099245 | <span style="color:#2563eb">30.03%</span> |
| 43 | [00764 CTE_RECURSIVE_MATRIX_057](crates/bench/sqlite_parity/cases/SQLITE_PARITY_764_CTE_RECURSIVE_MATRIX_057.rs) | P1 | memory | GEN_SQL_CTE | 1562519 | 2097763 | <span style="color:#2563eb">30.07%</span> |
| 44 | [00061 WINDOW_ROW_NUMBER_RANK](crates/bench/sqlite_parity/cases/SQLITE_PARITY_061_WINDOW_ROW_NUMBER_RANK.rs) | P0 | memory | SQL_WINDOW | 2903158 | 2095168 | <span style="color:#2563eb">30.16%</span> |
| 45 | [00888 CONSTRAINT_FK_SAVEPOINT_021](crates/bench/sqlite_parity/cases/SQLITE_PARITY_888_CONSTRAINT_FK_SAVEPOINT_021.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1644184 | 2092954 | <span style="color:#2563eb">30.23%</span> |
| 46 | [00005 UNIQUE_CONSTRAINT_FAILURE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_005_UNIQUE_CONSTRAINT_FAILURE.rs) | P0 | memory | SQL_CONSTRAINTS_NEGATIVE | 1879399 | 2088285 | <span style="color:#2563eb">30.39%</span> |
| 47 | [00012 INSERT_DEFAULT_VALUES](crates/bench/sqlite_parity/cases/SQLITE_PARITY_012_INSERT_DEFAULT_VALUES.rs) | P0 | memory | SQL_INSERT | 1565886 | 2072805 | <span style="color:#2563eb">30.91%</span> |
| 48 | [01117 INDEX_SCHEMA_PRAGMA_050](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1117_INDEX_SCHEMA_PRAGMA_050.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1823814 | 2054591 | <span style="color:#2563eb">31.51%</span> |
| 49 | [00185 OPT_SEPARATOR](crates/bench/sqlite_parity/cases/SQLITE_PARITY_185_OPT_SEPARATOR.rs) | P1 | memory | CLI_OPTION | 1519728 | 2053518 | <span style="color:#2563eb">31.55%</span> |
| 50 | [00011 CREATE_TABLE_AS_SELECT](crates/bench/sqlite_parity/cases/SQLITE_PARITY_011_CREATE_TABLE_AS_SELECT.rs) | P0 | memory | SQL_DDL | 1593087 | 2027990 | <span style="color:#2563eb">32.40%</span> |
| 51 | [00003 CREATE_TABLE_INSERT_SELECT](crates/bench/sqlite_parity/cases/SQLITE_PARITY_003_CREATE_TABLE_INSERT_SELECT.rs) | P0 | memory | SQL_DDL_DML | 2141936 | 2027279 | <span style="color:#2563eb">32.42%</span> |
| 52 | [00150 DOT_BACKUP_RESTORE_TEMPFILE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_150_DOT_BACKUP_RESTORE_TEMPFILE.rs) | P1 | tempfile | CLI_TEMPFILE | 2013994 | 2021648 | <span style="color:#2563eb">32.61%</span> |
| 53 | [00783 CTE_RECURSIVE_MATRIX_076](crates/bench/sqlite_parity/cases/SQLITE_PARITY_783_CTE_RECURSIVE_MATRIX_076.rs) | P1 | memory | GEN_SQL_CTE | 2219894 | 2020727 | <span style="color:#2563eb">32.64%</span> |
| 54 | [00789 WINDOW_PARTITION_SUM_002](crates/bench/sqlite_parity/cases/SQLITE_PARITY_789_WINDOW_PARTITION_SUM_002.rs) | P2 | memory | GEN_SQL_WINDOW | 1644915 | 2013443 | <span style="color:#2563eb">32.89%</span> |
| 55 | [00894 CONSTRAINT_FK_SAVEPOINT_027](crates/bench/sqlite_parity/cases/SQLITE_PARITY_894_CONSTRAINT_FK_SAVEPOINT_027.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1659002 | 1999386 | <span style="color:#2563eb">33.35%</span> |
| 56 | [01120 INDEX_SCHEMA_PRAGMA_053](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1120_INDEX_SCHEMA_PRAGMA_053.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1689049 | 1993615 | <span style="color:#2563eb">33.55%</span> |
| 57 | [00013 INSERT_SELECT](crates/bench/sqlite_parity/cases/SQLITE_PARITY_013_INSERT_SELECT.rs) | P0 | memory | SQL_INSERT | 1577617 | 1985610 | <span style="color:#2563eb">33.81%</span> |
| 58 | [00165 DOT_INTCK_CATALOG](crates/bench/sqlite_parity/cases/SQLITE_PARITY_165_DOT_INTCK_CATALOG.rs) | P4 | catalog | CLI_CATALOG | 4119321 | 2711615 | <span style="color:#2563eb">34.17%</span> |
| 59 | [00805 WINDOW_PARTITION_SUM_018](crates/bench/sqlite_parity/cases/SQLITE_PARITY_805_WINDOW_PARTITION_SUM_018.rs) | P2 | memory | GEN_SQL_WINDOW | 1665985 | 1968909 | <span style="color:#2563eb">34.37%</span> |
| 60 | [00074 NOT_INDEXED](crates/bench/sqlite_parity/cases/SQLITE_PARITY_074_NOT_INDEXED.rs) | P0 | memory | SQL_INDEX | 1586544 | 1959871 | <span style="color:#2563eb">34.67%</span> |
| 61 | [00649 JOIN_SUBQUERY_EXISTS_042](crates/bench/sqlite_parity/cases/SQLITE_PARITY_649_JOIN_SUBQUERY_EXISTS_042.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2076142 | 1959470 | <span style="color:#2563eb">34.68%</span> |
| 62 | [00622 JOIN_SUBQUERY_EXISTS_015](crates/bench/sqlite_parity/cases/SQLITE_PARITY_622_JOIN_SUBQUERY_EXISTS_015.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1685542 | 1958629 | <span style="color:#2563eb">34.71%</span> |
| 63 | [00996 VIEW_TRIGGER_GENERATED_049](crates/bench/sqlite_parity/cases/SQLITE_PARITY_996_VIEW_TRIGGER_GENERATED_049.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1795110 | 1956435 | <span style="color:#2563eb">34.79%</span> |
| 64 | [00633 JOIN_SUBQUERY_EXISTS_026](crates/bench/sqlite_parity/cases/SQLITE_PARITY_633_JOIN_SUBQUERY_EXISTS_026.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1730707 | 1949081 | <span style="color:#2563eb">35.03%</span> |
| 65 | [00167 DOT_UNMODULE_CATALOG](crates/bench/sqlite_parity/cases/SQLITE_PARITY_167_DOT_UNMODULE_CATALOG.rs) | P4 | catalog | CLI_CATALOG | 1431511 | 1938611 | <span style="color:#2563eb">35.38%</span> |
| 66 | [00004 TABLE_CONSTRAINTS_SUCCESS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_004_TABLE_CONSTRAINTS_SUCCESS.rs) | P0 | memory | SQL_CONSTRAINTS | 1543533 | 1938481 | <span style="color:#2563eb">35.38%</span> |
| 67 | [00718 CTE_RECURSIVE_MATRIX_011](crates/bench/sqlite_parity/cases/SQLITE_PARITY_718_CTE_RECURSIVE_MATRIX_011.rs) | P1 | memory | GEN_SQL_CTE | 1636298 | 1927300 | <span style="color:#2563eb">35.76%</span> |
| 68 | [00284 SCALAR_ARITH_015](crates/bench/sqlite_parity/cases/SQLITE_PARITY_284_SCALAR_ARITH_015.rs) | P1 | memory | GEN_SQL_SCALAR | 1518936 | 1925567 | <span style="color:#2563eb">35.81%</span> |
| 69 | [00850 WINDOW_PARTITION_SUM_063](crates/bench/sqlite_parity/cases/SQLITE_PARITY_850_WINDOW_PARTITION_SUM_063.rs) | P2 | memory | GEN_SQL_WINDOW | 1653601 | 1923793 | <span style="color:#2563eb">35.87%</span> |
| 70 | [00643 JOIN_SUBQUERY_EXISTS_036](crates/bench/sqlite_parity/cases/SQLITE_PARITY_643_JOIN_SUBQUERY_EXISTS_036.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2068006 | 1910248 | <span style="color:#2563eb">36.33%</span> |
| 71 | [00578 AGG_GROUP_HAVING_071](crates/bench/sqlite_parity/cases/SQLITE_PARITY_578_AGG_GROUP_HAVING_071.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1820598 | 1909265 | <span style="color:#2563eb">36.36%</span> |
| 72 | [00151 DOT_SAVE_RESTORE_TEMPFILE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_151_DOT_SAVE_RESTORE_TEMPFILE.rs) | P1 | tempfile | CLI_TEMPFILE | 2200347 | 1907683 | <span style="color:#2563eb">36.41%</span> |
| 73 | [00905 CONSTRAINT_FK_SAVEPOINT_038](crates/bench/sqlite_parity/cases/SQLITE_PARITY_905_CONSTRAINT_FK_SAVEPOINT_038.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1707513 | 1907282 | <span style="color:#2563eb">36.42%</span> |
| 74 | [00157 DOT_ARCHIVE_TEMPFILE_OPTIONAL](crates/bench/sqlite_parity/cases/SQLITE_PARITY_157_DOT_ARCHIVE_TEMPFILE_OPTIONAL.rs) | P3 | tempfile | CLI_TEMPFILE_OPTIONAL | 1889078 | 1904296 | <span style="color:#2563eb">36.52%</span> |
| 75 | [00196 OPT_MMAP](crates/bench/sqlite_parity/cases/SQLITE_PARITY_196_OPT_MMAP.rs) | P3 | memory | CLI_OPTION | 2050483 | 1894708 | <span style="color:#2563eb">36.84%</span> |
| 76 | [00859 WINDOW_PARTITION_SUM_072](crates/bench/sqlite_parity/cases/SQLITE_PARITY_859_WINDOW_PARTITION_SUM_072.rs) | P2 | memory | GEN_SQL_WINDOW | 1604359 | 1894418 | <span style="color:#2563eb">36.85%</span> |
| 77 | [00146 DOT_READ_TEMPFILE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_146_DOT_READ_TEMPFILE.rs) | P1 | tempfile | CLI_TEMPFILE | 1843561 | 1893887 | <span style="color:#2563eb">36.87%</span> |
| 78 | [00776 CTE_RECURSIVE_MATRIX_069](crates/bench/sqlite_parity/cases/SQLITE_PARITY_776_CTE_RECURSIVE_MATRIX_069.rs) | P1 | memory | GEN_SQL_CTE | 1647039 | 1893786 | <span style="color:#2563eb">36.87%</span> |
| 79 | [00702 JOIN_SUBQUERY_EXISTS_095](crates/bench/sqlite_parity/cases/SQLITE_PARITY_702_JOIN_SUBQUERY_EXISTS_095.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1714046 | 1891282 | <span style="color:#2563eb">36.96%</span> |
| 80 | [00933 CONSTRAINT_FK_SAVEPOINT_066](crates/bench/sqlite_parity/cases/SQLITE_PARITY_933_CONSTRAINT_FK_SAVEPOINT_066.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1638112 | 1891051 | <span style="color:#2563eb">36.96%</span> |
| 81 | [00425 DML_WHERE_ORDER_LIMIT_038](crates/bench/sqlite_parity/cases/SQLITE_PARITY_425_DML_WHERE_ORDER_LIMIT_038.rs) | P1 | memory | GEN_SQL_DML | 1850775 | 1888837 | <span style="color:#2563eb">37.04%</span> |
| 82 | [01037 JSON_EXTRACT_SET_030](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1037_JSON_EXTRACT_SET_030.rs) | P2 | memory | GEN_SQL_JSON | 1737942 | 1885881 | <span style="color:#2563eb">37.14%</span> |
| 83 | [00360 SCALAR_ARITH_034](crates/bench/sqlite_parity/cases/SQLITE_PARITY_360_SCALAR_ARITH_034.rs) | P1 | memory | GEN_SQL_SCALAR | 1833553 | 1883597 | <span style="color:#2563eb">37.21%</span> |
| 84 | [00813 WINDOW_PARTITION_SUM_026](crates/bench/sqlite_parity/cases/SQLITE_PARITY_813_WINDOW_PARTITION_SUM_026.rs) | P2 | memory | GEN_SQL_WINDOW | 1638412 | 1882606 | <span style="color:#2563eb">37.25%</span> |
| 85 | [00815 WINDOW_PARTITION_SUM_028](crates/bench/sqlite_parity/cases/SQLITE_PARITY_815_WINDOW_PARTITION_SUM_028.rs) | P2 | memory | GEN_SQL_WINDOW | 1621711 | 1879800 | <span style="color:#2563eb">37.34%</span> |
| 86 | [00493 DML_WHERE_ORDER_LIMIT_106](crates/bench/sqlite_parity/cases/SQLITE_PARITY_493_DML_WHERE_ORDER_LIMIT_106.rs) | P1 | memory | GEN_SQL_DML | 1957477 | 1876624 | <span style="color:#2563eb">37.45%</span> |
| 87 | [00958 VIEW_TRIGGER_GENERATED_011](crates/bench/sqlite_parity/cases/SQLITE_PARITY_958_VIEW_TRIGGER_GENERATED_011.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2159950 | 1875451 | <span style="color:#2563eb">37.48%</span> |
| 88 | [00443 DML_WHERE_ORDER_LIMIT_056](crates/bench/sqlite_parity/cases/SQLITE_PARITY_443_DML_WHERE_ORDER_LIMIT_056.rs) | P1 | memory | GEN_SQL_DML | 1620038 | 1874670 | <span style="color:#2563eb">37.51%</span> |
| 89 | [00099 CLI_UINT_COLLATION_OPTIONAL](crates/bench/sqlite_parity/cases/SQLITE_PARITY_099_CLI_UINT_COLLATION_OPTIONAL.rs) | P3 | memory | CLI_EXTENSION_OPTIONAL | 1509569 | 1874610 | <span style="color:#2563eb">37.51%</span> |
| 90 | [00156 DOT_RECOVER_TEMPFILE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_156_DOT_RECOVER_TEMPFILE.rs) | P3 | tempfile | CLI_TEMPFILE | 2986636 | 1868578 | <span style="color:#2563eb">37.71%</span> |
| 91 | [00758 CTE_RECURSIVE_MATRIX_051](crates/bench/sqlite_parity/cases/SQLITE_PARITY_758_CTE_RECURSIVE_MATRIX_051.rs) | P1 | memory | GEN_SQL_CTE | 2382301 | 1868288 | <span style="color:#2563eb">37.72%</span> |
| 92 | [00782 CTE_RECURSIVE_MATRIX_075](crates/bench/sqlite_parity/cases/SQLITE_PARITY_782_CTE_RECURSIVE_MATRIX_075.rs) | P1 | memory | GEN_SQL_CTE | 1659182 | 1867587 | <span style="color:#2563eb">37.75%</span> |
| 93 | [01062 JSON_EXTRACT_SET_055](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1062_JSON_EXTRACT_SET_055.rs) | P2 | memory | GEN_SQL_JSON | 1601753 | 1864621 | <span style="color:#2563eb">37.85%</span> |
| 94 | [00091 MATH_FUNCTIONS_OPTIONAL](crates/bench/sqlite_parity/cases/SQLITE_PARITY_091_MATH_FUNCTIONS_OPTIONAL.rs) | P2 | memory | SQL_FUNCTIONS_OPTIONAL | 1449746 | 1864260 | <span style="color:#2563eb">37.86%</span> |
| 95 | [00987 VIEW_TRIGGER_GENERATED_040](crates/bench/sqlite_parity/cases/SQLITE_PARITY_987_VIEW_TRIGGER_GENERATED_040.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1760264 | 1863138 | <span style="color:#2563eb">37.90%</span> |
| 96 | [00705 JOIN_SUBQUERY_EXISTS_098](crates/bench/sqlite_parity/cases/SQLITE_PARITY_705_JOIN_SUBQUERY_EXISTS_098.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2241164 | 1863099 | <span style="color:#2563eb">37.90%</span> |
| 97 | [00084 CORE_FORMAT_QUOTE_HEX](crates/bench/sqlite_parity/cases/SQLITE_PARITY_084_CORE_FORMAT_QUOTE_HEX.rs) | P0 | memory | SQL_FUNCTIONS | 1488789 | 1861325 | <span style="color:#2563eb">37.96%</span> |
| 98 | [00932 CONSTRAINT_FK_SAVEPOINT_065](crates/bench/sqlite_parity/cases/SQLITE_PARITY_932_CONSTRAINT_FK_SAVEPOINT_065.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1668470 | 1857928 | <span style="color:#2563eb">38.07%</span> |
| 99 | [00311 SCALAR_NULL_COALESCE_021](crates/bench/sqlite_parity/cases/SQLITE_PARITY_311_SCALAR_NULL_COALESCE_021.rs) | P1 | memory | GEN_SQL_SCALAR | 1459204 | 1854592 | <span style="color:#2563eb">38.18%</span> |
| 100 | [00760 CTE_RECURSIVE_MATRIX_053](crates/bench/sqlite_parity/cases/SQLITE_PARITY_760_CTE_RECURSIVE_MATRIX_053.rs) | P1 | memory | GEN_SQL_CTE | 1544445 | 1848310 | <span style="color:#2563eb">38.39%</span> |
| 101 | [00551 AGG_GROUP_HAVING_044](crates/bench/sqlite_parity/cases/SQLITE_PARITY_551_AGG_GROUP_HAVING_044.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1692325 | 1848060 | <span style="color:#2563eb">38.40%</span> |
| 102 | [00505 DML_WHERE_ORDER_LIMIT_118](crates/bench/sqlite_parity/cases/SQLITE_PARITY_505_DML_WHERE_ORDER_LIMIT_118.rs) | P1 | memory | GEN_SQL_DML | 1664302 | 1845585 | <span style="color:#2563eb">38.48%</span> |
| 103 | [00811 WINDOW_PARTITION_SUM_024](crates/bench/sqlite_parity/cases/SQLITE_PARITY_811_WINDOW_PARTITION_SUM_024.rs) | P2 | memory | GEN_SQL_WINDOW | 1603777 | 1843201 | <span style="color:#2563eb">38.56%</span> |
| 104 | [00077 COMMENTS_AND_CLI_TERMINATORS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_077_COMMENTS_AND_CLI_TERMINATORS.rs) | P0 | memory | CLI_SQL_INPUT | 1469222 | 1834224 | <span style="color:#2563eb">38.86%</span> |
| 105 | [00975 VIEW_TRIGGER_GENERATED_028](crates/bench/sqlite_parity/cases/SQLITE_PARITY_975_VIEW_TRIGGER_GENERATED_028.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1754252 | 1827591 | <span style="color:#2563eb">39.08%</span> |
| 106 | [00148 DOT_OUTPUT_TEMPFILE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_148_DOT_OUTPUT_TEMPFILE.rs) | P1 | tempfile | CLI_TEMPFILE | 1799087 | 1827590 | <span style="color:#2563eb">39.08%</span> |
| 107 | [00769 CTE_RECURSIVE_MATRIX_062](crates/bench/sqlite_parity/cases/SQLITE_PARITY_769_CTE_RECURSIVE_MATRIX_062.rs) | P1 | memory | GEN_SQL_CTE | 1524066 | 1824305 | <span style="color:#2563eb">39.19%</span> |
| 108 | [00632 JOIN_SUBQUERY_EXISTS_025](crates/bench/sqlite_parity/cases/SQLITE_PARITY_632_JOIN_SUBQUERY_EXISTS_025.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1696593 | 1823954 | <span style="color:#2563eb">39.20%</span> |
| 109 | [00512 AGG_GROUP_HAVING_005](crates/bench/sqlite_parity/cases/SQLITE_PARITY_512_AGG_GROUP_HAVING_005.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1642000 | 1821128 | <span style="color:#2563eb">39.30%</span> |
| 110 | [00992 VIEW_TRIGGER_GENERATED_045](crates/bench/sqlite_parity/cases/SQLITE_PARITY_992_VIEW_TRIGGER_GENERATED_045.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1924694 | 1819326 | <span style="color:#2563eb">39.36%</span> |
| 111 | [00537 AGG_GROUP_HAVING_030](crates/bench/sqlite_parity/cases/SQLITE_PARITY_537_AGG_GROUP_HAVING_030.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1699889 | 1817021 | <span style="color:#2563eb">39.43%</span> |
| 112 | [01009 JSON_EXTRACT_SET_002](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1009_JSON_EXTRACT_SET_002.rs) | P2 | memory | GEN_SQL_JSON | 1532342 | 1808645 | <span style="color:#2563eb">39.71%</span> |
| 113 | [00714 CTE_RECURSIVE_MATRIX_007](crates/bench/sqlite_parity/cases/SQLITE_PARITY_714_CTE_RECURSIVE_MATRIX_007.rs) | P1 | memory | GEN_SQL_CTE | 1619667 | 1806280 | <span style="color:#2563eb">39.79%</span> |
| 114 | [00230 SCALAR_STRING_001](crates/bench/sqlite_parity/cases/SQLITE_PARITY_230_SCALAR_STRING_001.rs) | P1 | memory | GEN_SQL_SCALAR | 1507234 | 1805839 | <span style="color:#2563eb">39.81%</span> |
| 115 | [00818 WINDOW_PARTITION_SUM_031](crates/bench/sqlite_parity/cases/SQLITE_PARITY_818_WINDOW_PARTITION_SUM_031.rs) | P2 | memory | GEN_SQL_WINDOW | 1688217 | 1802904 | <span style="color:#2563eb">39.90%</span> |
| 116 | [00481 DML_WHERE_ORDER_LIMIT_094](crates/bench/sqlite_parity/cases/SQLITE_PARITY_481_DML_WHERE_ORDER_LIMIT_094.rs) | P1 | memory | GEN_SQL_DML | 1598868 | 1793767 | <span style="color:#2563eb">40.21%</span> |
| 117 | [00730 CTE_RECURSIVE_MATRIX_023](crates/bench/sqlite_parity/cases/SQLITE_PARITY_730_CTE_RECURSIVE_MATRIX_023.rs) | P1 | memory | GEN_SQL_CTE | 1568029 | 1792995 | <span style="color:#2563eb">40.23%</span> |
| 118 | [00126 DOT_STATS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_126_DOT_STATS.rs) | P3 | memory | CLI_DOT_COMMAND_DIAGNOSTIC | 1487717 | 1789559 | <span style="color:#2563eb">40.35%</span> |
| 119 | [00088 JSON_SCALAR_FUNCTIONS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_088_JSON_SCALAR_FUNCTIONS.rs) | P0 | memory | SQL_JSON | 1530168 | 1788667 | <span style="color:#2563eb">40.38%</span> |
| 120 | [00591 AGG_GROUP_HAVING_084](crates/bench/sqlite_parity/cases/SQLITE_PARITY_591_AGG_GROUP_HAVING_084.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1744794 | 1787986 | <span style="color:#2563eb">40.40%</span> |
| 121 | [00212 SQL_VACUUM_INTO_TEMPFILE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_212_SQL_VACUUM_INTO_TEMPFILE.rs) | P2 | tempfile | SQL_TEMPFILE | 2080340 | 1787725 | <span style="color:#2563eb">40.41%</span> |
| 122 | [00580 AGG_GROUP_HAVING_073](crates/bench/sqlite_parity/cases/SQLITE_PARITY_580_AGG_GROUP_HAVING_073.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1895299 | 1785061 | <span style="color:#2563eb">40.50%</span> |
| 123 | [00447 DML_WHERE_ORDER_LIMIT_060](crates/bench/sqlite_parity/cases/SQLITE_PARITY_447_DML_WHERE_ORDER_LIMIT_060.rs) | P1 | memory | GEN_SQL_DML | 1569041 | 1782646 | <span style="color:#2563eb">40.58%</span> |
| 124 | [00967 VIEW_TRIGGER_GENERATED_020](crates/bench/sqlite_parity/cases/SQLITE_PARITY_967_VIEW_TRIGGER_GENERATED_020.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1747830 | 1778318 | <span style="color:#2563eb">40.72%</span> |
| 125 | [00710 CTE_RECURSIVE_MATRIX_003](crates/bench/sqlite_parity/cases/SQLITE_PARITY_710_CTE_RECURSIVE_MATRIX_003.rs) | P1 | memory | GEN_SQL_CTE | 1578469 | 1772998 | <span style="color:#2563eb">40.90%</span> |
| 126 | [00363 SCALAR_NULL_COALESCE_034](crates/bench/sqlite_parity/cases/SQLITE_PARITY_363_SCALAR_NULL_COALESCE_034.rs) | P1 | memory | GEN_SQL_SCALAR | 1486505 | 1770222 | <span style="color:#2563eb">40.99%</span> |
| 127 | [00807 WINDOW_PARTITION_SUM_020](crates/bench/sqlite_parity/cases/SQLITE_PARITY_807_WINDOW_PARTITION_SUM_020.rs) | P2 | memory | GEN_SQL_WINDOW | 1634465 | 1769821 | <span style="color:#2563eb">41.01%</span> |
| 128 | [00711 CTE_RECURSIVE_MATRIX_004](crates/bench/sqlite_parity/cases/SQLITE_PARITY_711_CTE_RECURSIVE_MATRIX_004.rs) | P1 | memory | GEN_SQL_CTE | 1725597 | 1768699 | <span style="color:#2563eb">41.04%</span> |
| 129 | [00848 WINDOW_PARTITION_SUM_061](crates/bench/sqlite_parity/cases/SQLITE_PARITY_848_WINDOW_PARTITION_SUM_061.rs) | P2 | memory | GEN_SQL_WINDOW | 1635517 | 1767718 | <span style="color:#2563eb">41.08%</span> |
| 130 | [00618 JOIN_SUBQUERY_EXISTS_011](crates/bench/sqlite_parity/cases/SQLITE_PARITY_618_JOIN_SUBQUERY_EXISTS_011.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1653471 | 1766996 | <span style="color:#2563eb">41.10%</span> |
| 131 | [00053 SELECT_WHERE_ORDER_LIMIT_OFFSET](crates/bench/sqlite_parity/cases/SQLITE_PARITY_053_SELECT_WHERE_ORDER_LIMIT_OFFSET.rs) | P0 | memory | SQL_SELECT | 1530639 | 1765794 | <span style="color:#2563eb">41.14%</span> |
| 132 | [00216 ROLLBACK_TRANSACTION_SYNTAX](crates/bench/sqlite_parity/cases/SQLITE_PARITY_216_ROLLBACK_TRANSACTION_SYNTAX.rs) | P0 | memory | SQL_TRANSACTION | 1535298 | 1761576 | <span style="color:#2563eb">41.28%</span> |
| 133 | [01085 INDEX_SCHEMA_PRAGMA_018](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1085_INDEX_SCHEMA_PRAGMA_018.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1662658 | 1754202 | <span style="color:#2563eb">41.53%</span> |
| 134 | [00547 AGG_GROUP_HAVING_040](crates/bench/sqlite_parity/cases/SQLITE_PARITY_547_AGG_GROUP_HAVING_040.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1869060 | 1753762 | <span style="color:#2563eb">41.54%</span> |
| 135 | [00420 DML_WHERE_ORDER_LIMIT_033](crates/bench/sqlite_parity/cases/SQLITE_PARITY_420_DML_WHERE_ORDER_LIMIT_033.rs) | P1 | memory | GEN_SQL_DML | 1596213 | 1752749 | <span style="color:#2563eb">41.58%</span> |
| 136 | [00594 AGG_GROUP_HAVING_087](crates/bench/sqlite_parity/cases/SQLITE_PARITY_594_AGG_GROUP_HAVING_087.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1697514 | 1752449 | <span style="color:#2563eb">41.59%</span> |
| 137 | [00658 JOIN_SUBQUERY_EXISTS_051](crates/bench/sqlite_parity/cases/SQLITE_PARITY_658_JOIN_SUBQUERY_EXISTS_051.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1705409 | 1752138 | <span style="color:#2563eb">41.60%</span> |
| 138 | [00960 VIEW_TRIGGER_GENERATED_013](crates/bench/sqlite_parity/cases/SQLITE_PARITY_960_VIEW_TRIGGER_GENERATED_013.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1903023 | 1744704 | <span style="color:#2563eb">41.84%</span> |
| 139 | [00623 JOIN_SUBQUERY_EXISTS_016](crates/bench/sqlite_parity/cases/SQLITE_PARITY_623_JOIN_SUBQUERY_EXISTS_016.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1678969 | 1739655 | <span style="color:#2563eb">42.01%</span> |
| 140 | [00325 SCALAR_CAST_TYPEOF_025](crates/bench/sqlite_parity/cases/SQLITE_PARITY_325_SCALAR_CAST_TYPEOF_025.rs) | P1 | memory | GEN_SQL_SCALAR | 1701993 | 1737981 | <span style="color:#2563eb">42.07%</span> |
| 141 | [00563 AGG_GROUP_HAVING_056](crates/bench/sqlite_parity/cases/SQLITE_PARITY_563_AGG_GROUP_HAVING_056.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1695300 | 1737641 | <span style="color:#2563eb">42.08%</span> |
| 142 | [01065 JSON_EXTRACT_SET_058](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1065_JSON_EXTRACT_SET_058.rs) | P2 | memory | GEN_SQL_JSON | 1584832 | 1737380 | <span style="color:#2563eb">42.09%</span> |
| 143 | [00907 CONSTRAINT_FK_SAVEPOINT_040](crates/bench/sqlite_parity/cases/SQLITE_PARITY_907_CONSTRAINT_FK_SAVEPOINT_040.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1668821 | 1736328 | <span style="color:#2563eb">42.12%</span> |
| 144 | [00605 AGG_GROUP_HAVING_098](crates/bench/sqlite_parity/cases/SQLITE_PARITY_605_AGG_GROUP_HAVING_098.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1616652 | 1733653 | <span style="color:#2563eb">42.21%</span> |
| 145 | [00995 VIEW_TRIGGER_GENERATED_048](crates/bench/sqlite_parity/cases/SQLITE_PARITY_995_VIEW_TRIGGER_GENERATED_048.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1725628 | 1731078 | <span style="color:#2563eb">42.30%</span> |
| 146 | [00653 JOIN_SUBQUERY_EXISTS_046](crates/bench/sqlite_parity/cases/SQLITE_PARITY_653_JOIN_SUBQUERY_EXISTS_046.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1715769 | 1728103 | <span style="color:#2563eb">42.40%</span> |
| 147 | [00320 SCALAR_ARITH_024](crates/bench/sqlite_parity/cases/SQLITE_PARITY_320_SCALAR_ARITH_024.rs) | P1 | memory | GEN_SQL_SCALAR | 2369567 | 1725618 | <span style="color:#2563eb">42.48%</span> |
| 148 | [01124 INDEX_SCHEMA_PRAGMA_057](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1124_INDEX_SCHEMA_PRAGMA_057.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 2060362 | 1722603 | <span style="color:#2563eb">42.58%</span> |
| 149 | [00973 VIEW_TRIGGER_GENERATED_026](crates/bench/sqlite_parity/cases/SQLITE_PARITY_973_VIEW_TRIGGER_GENERATED_026.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1675603 | 1720288 | <span style="color:#2563eb">42.66%</span> |
| 150 | [01051 JSON_EXTRACT_SET_044](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1051_JSON_EXTRACT_SET_044.rs) | P2 | memory | GEN_SQL_JSON | 1626800 | 1719366 | <span style="color:#2563eb">42.69%</span> |
| 151 | [00615 JOIN_SUBQUERY_EXISTS_008](crates/bench/sqlite_parity/cases/SQLITE_PARITY_615_JOIN_SUBQUERY_EXISTS_008.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1704378 | 1716511 | <span style="color:#2563eb">42.78%</span> |
| 152 | [01012 JSON_EXTRACT_SET_005](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1012_JSON_EXTRACT_SET_005.rs) | P2 | memory | GEN_SQL_JSON | 1520579 | 1713785 | <span style="color:#2563eb">42.87%</span> |
| 153 | [00575 AGG_GROUP_HAVING_068](crates/bench/sqlite_parity/cases/SQLITE_PARITY_575_AGG_GROUP_HAVING_068.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1665724 | 1711952 | <span style="color:#2563eb">42.93%</span> |
| 154 | [00790 WINDOW_PARTITION_SUM_003](crates/bench/sqlite_parity/cases/SQLITE_PARITY_790_WINDOW_PARTITION_SUM_003.rs) | P2 | memory | GEN_SQL_WINDOW | 2202481 | 1711872 | <span style="color:#2563eb">42.94%</span> |
| 155 | [00609 JOIN_SUBQUERY_EXISTS_002](crates/bench/sqlite_parity/cases/SQLITE_PARITY_609_JOIN_SUBQUERY_EXISTS_002.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1692055 | 1704658 | <span style="color:#2563eb">43.18%</span> |
| 156 | [00350 SCALAR_STRING_031](crates/bench/sqlite_parity/cases/SQLITE_PARITY_350_SCALAR_STRING_031.rs) | P1 | memory | GEN_SQL_SCALAR | 1491345 | 1701863 | <span style="color:#2563eb">43.27%</span> |
| 157 | [00335 SCALAR_NULL_COALESCE_027](crates/bench/sqlite_parity/cases/SQLITE_PARITY_335_SCALAR_NULL_COALESCE_027.rs) | P1 | memory | GEN_SQL_SCALAR | 1495753 | 1697786 | <span style="color:#2563eb">43.41%</span> |
| 158 | [01087 INDEX_SCHEMA_PRAGMA_020](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1087_INDEX_SCHEMA_PRAGMA_020.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1720839 | 1697124 | <span style="color:#2563eb">43.43%</span> |
| 159 | [00964 VIEW_TRIGGER_GENERATED_017](crates/bench/sqlite_parity/cases/SQLITE_PARITY_964_VIEW_TRIGGER_GENERATED_017.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1662609 | 1695861 | <span style="color:#2563eb">43.47%</span> |
| 160 | [00971 VIEW_TRIGGER_GENERATED_024](crates/bench/sqlite_parity/cases/SQLITE_PARITY_971_VIEW_TRIGGER_GENERATED_024.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1749323 | 1694729 | <span style="color:#2563eb">43.51%</span> |
| 161 | [00513 AGG_GROUP_HAVING_006](crates/bench/sqlite_parity/cases/SQLITE_PARITY_513_AGG_GROUP_HAVING_006.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1711711 | 1686674 | <span style="color:#2563eb">43.78%</span> |
| 162 | [00098 CLI_REGEXP_OPTIONAL](crates/bench/sqlite_parity/cases/SQLITE_PARITY_098_CLI_REGEXP_OPTIONAL.rs) | P2 | memory | CLI_EXTENSION_OPTIONAL | 1472849 | 1686493 | <span style="color:#2563eb">43.78%</span> |
| 163 | [00062 WINDOW_FRAMES_ROWS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_062_WINDOW_FRAMES_ROWS.rs) | P0 | memory | SQL_WINDOW | 1633193 | 1684540 | <span style="color:#2563eb">43.85%</span> |
| 164 | [00226 OPT_NOFOLLOW_CATALOG](crates/bench/sqlite_parity/cases/SQLITE_PARITY_226_OPT_NOFOLLOW_CATALOG.rs) | P4 | catalog | CLI_OPTION_CATALOG | 1284853 | 1682255 | <span style="color:#2563eb">43.92%</span> |
| 165 | [01074 INDEX_SCHEMA_PRAGMA_007](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1074_INDEX_SCHEMA_PRAGMA_007.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 2082243 | 1682166 | <span style="color:#2563eb">43.93%</span> |
| 166 | [00542 AGG_GROUP_HAVING_035](crates/bench/sqlite_parity/cases/SQLITE_PARITY_542_AGG_GROUP_HAVING_035.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1670613 | 1681735 | <span style="color:#2563eb">43.94%</span> |
| 167 | [00162 DOT_LOAD_EXTENSION_NEGATIVE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_162_DOT_LOAD_EXTENSION_NEGATIVE.rs) | P4 | side_effect | CLI_SIDE_EFFECT | 1827481 | 1679570 | <span style="color:#2563eb">44.01%</span> |
| 168 | [01024 JSON_EXTRACT_SET_017](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1024_JSON_EXTRACT_SET_017.rs) | P2 | memory | GEN_SQL_JSON | 2253747 | 1677167 | <span style="color:#2563eb">44.09%</span> |
| 169 | [00204 OPT_ZIP_TEMPFILE_CATALOG](crates/bench/sqlite_parity/cases/SQLITE_PARITY_204_OPT_ZIP_TEMPFILE_CATALOG.rs) | P4 | catalog | CLI_OPTION_CATALOG | 1627152 | 1672437 | <span style="color:#2563eb">44.25%</span> |
| 170 | [00234 SCALAR_STRING_002](crates/bench/sqlite_parity/cases/SQLITE_PARITY_234_SCALAR_STRING_002.rs) | P1 | memory | GEN_SQL_SCALAR | 1437152 | 1668770 | <span style="color:#2563eb">44.37%</span> |
| 171 | [01023 JSON_EXTRACT_SET_016](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1023_JSON_EXTRACT_SET_016.rs) | P2 | memory | GEN_SQL_JSON | 1722211 | 1665014 | <span style="color:#2563eb">44.50%</span> |
| 172 | [00740 CTE_RECURSIVE_MATRIX_033](crates/bench/sqlite_parity/cases/SQLITE_PARITY_740_CTE_RECURSIVE_MATRIX_033.rs) | P1 | memory | GEN_SQL_CTE | 1577968 | 1664272 | <span style="color:#2563eb">44.52%</span> |
| 173 | [00890 CONSTRAINT_FK_SAVEPOINT_023](crates/bench/sqlite_parity/cases/SQLITE_PARITY_890_CONSTRAINT_FK_SAVEPOINT_023.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1644354 | 1662308 | <span style="color:#2563eb">44.59%</span> |
| 174 | [00646 JOIN_SUBQUERY_EXISTS_039](crates/bench/sqlite_parity/cases/SQLITE_PARITY_646_JOIN_SUBQUERY_EXISTS_039.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1701122 | 1656346 | <span style="color:#2563eb">44.79%</span> |
| 175 | [00536 AGG_GROUP_HAVING_029](crates/bench/sqlite_parity/cases/SQLITE_PARITY_536_AGG_GROUP_HAVING_029.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1748131 | 1648612 | <span style="color:#2563eb">45.05%</span> |
| 176 | [00149 DOT_ONCE_TEMPFILE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_149_DOT_ONCE_TEMPFILE.rs) | P1 | tempfile | CLI_TEMPFILE | 1511973 | 1647229 | <span style="color:#2563eb">45.09%</span> |
| 177 | [00650 JOIN_SUBQUERY_EXISTS_043](crates/bench/sqlite_parity/cases/SQLITE_PARITY_650_JOIN_SUBQUERY_EXISTS_043.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1724956 | 1641528 | <span style="color:#2563eb">45.28%</span> |
| 178 | [00092 PERCENTILE_FUNCTIONS_OPTIONAL](crates/bench/sqlite_parity/cases/SQLITE_PARITY_092_PERCENTILE_FUNCTIONS_OPTIONAL.rs) | P3 | memory | SQL_FUNCTIONS_OPTIONAL | 2195368 | 1640888 | <span style="color:#2563eb">45.30%</span> |
| 179 | [00554 AGG_GROUP_HAVING_047](crates/bench/sqlite_parity/cases/SQLITE_PARITY_554_AGG_GROUP_HAVING_047.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1709999 | 1640497 | <span style="color:#2563eb">45.32%</span> |
| 180 | [00704 JOIN_SUBQUERY_EXISTS_097](crates/bench/sqlite_parity/cases/SQLITE_PARITY_704_JOIN_SUBQUERY_EXISTS_097.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1722412 | 1634235 | <span style="color:#2563eb">45.53%</span> |
| 181 | [00986 VIEW_TRIGGER_GENERATED_039](crates/bench/sqlite_parity/cases/SQLITE_PARITY_986_VIEW_TRIGGER_GENERATED_039.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1684270 | 1628013 | <span style="color:#2563eb">45.73%</span> |
| 182 | [00480 DML_WHERE_ORDER_LIMIT_093](crates/bench/sqlite_parity/cases/SQLITE_PARITY_480_DML_WHERE_ORDER_LIMIT_093.rs) | P1 | memory | GEN_SQL_DML | 1611101 | 1627772 | <span style="color:#2563eb">45.74%</span> |
| 183 | [01116 INDEX_SCHEMA_PRAGMA_049](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1116_INDEX_SCHEMA_PRAGMA_049.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1756066 | 1617703 | <span style="color:#2563eb">46.08%</span> |
| 184 | [00583 AGG_GROUP_HAVING_076](crates/bench/sqlite_parity/cases/SQLITE_PARITY_583_AGG_GROUP_HAVING_076.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1734995 | 1615699 | <span style="color:#2563eb">46.14%</span> |
| 185 | [00445 DML_WHERE_ORDER_LIMIT_058](crates/bench/sqlite_parity/cases/SQLITE_PARITY_445_DML_WHERE_ORDER_LIMIT_058.rs) | P1 | memory | GEN_SQL_DML | 1630177 | 1613596 | <span style="color:#2563eb">46.21%</span> |
| 186 | [00634 JOIN_SUBQUERY_EXISTS_027](crates/bench/sqlite_parity/cases/SQLITE_PARITY_634_JOIN_SUBQUERY_EXISTS_027.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1954381 | 1612103 | <span style="color:#2563eb">46.26%</span> |
| 187 | [00864 WINDOW_PARTITION_SUM_077](crates/bench/sqlite_parity/cases/SQLITE_PARITY_864_WINDOW_PARTITION_SUM_077.rs) | P2 | memory | GEN_SQL_WINDOW | 1909216 | 1611402 | <span style="color:#2563eb">46.29%</span> |
| 188 | [00449 DML_WHERE_ORDER_LIMIT_062](crates/bench/sqlite_parity/cases/SQLITE_PARITY_449_DML_WHERE_ORDER_LIMIT_062.rs) | P1 | memory | GEN_SQL_DML | 2409723 | 1610349 | <span style="color:#2563eb">46.32%</span> |
| 189 | [00882 CONSTRAINT_FK_SAVEPOINT_015](crates/bench/sqlite_parity/cases/SQLITE_PARITY_882_CONSTRAINT_FK_SAVEPOINT_015.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1881212 | 1608957 | <span style="color:#2563eb">46.37%</span> |
| 190 | [00645 JOIN_SUBQUERY_EXISTS_038](crates/bench/sqlite_parity/cases/SQLITE_PARITY_645_JOIN_SUBQUERY_EXISTS_038.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1732050 | 1608817 | <span style="color:#2563eb">46.37%</span> |
| 191 | [00767 CTE_RECURSIVE_MATRIX_060](crates/bench/sqlite_parity/cases/SQLITE_PARITY_767_CTE_RECURSIVE_MATRIX_060.rs) | P1 | memory | GEN_SQL_CTE | 1581776 | 1608245 | <span style="color:#2563eb">46.39%</span> |
| 192 | [00779 CTE_RECURSIVE_MATRIX_072](crates/bench/sqlite_parity/cases/SQLITE_PARITY_779_CTE_RECURSIVE_MATRIX_072.rs) | P1 | memory | GEN_SQL_CTE | 1520570 | 1606944 | <span style="color:#2563eb">46.44%</span> |
| 193 | [00517 AGG_GROUP_HAVING_010](crates/bench/sqlite_parity/cases/SQLITE_PARITY_517_AGG_GROUP_HAVING_010.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2093425 | 1606703 | <span style="color:#2563eb">46.44%</span> |
| 194 | [00581 AGG_GROUP_HAVING_074](crates/bench/sqlite_parity/cases/SQLITE_PARITY_581_AGG_GROUP_HAVING_074.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1706431 | 1601052 | <span style="color:#2563eb">46.63%</span> |
| 195 | [00950 VIEW_TRIGGER_GENERATED_003](crates/bench/sqlite_parity/cases/SQLITE_PARITY_950_VIEW_TRIGGER_GENERATED_003.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1726239 | 1601052 | <span style="color:#2563eb">46.63%</span> |
| 196 | [00327 SCALAR_NULL_COALESCE_025](crates/bench/sqlite_parity/cases/SQLITE_PARITY_327_SCALAR_NULL_COALESCE_025.rs) | P1 | memory | GEN_SQL_SCALAR | 1481486 | 1598988 | <span style="color:#2563eb">46.70%</span> |
| 197 | [00977 VIEW_TRIGGER_GENERATED_030](crates/bench/sqlite_parity/cases/SQLITE_PARITY_977_VIEW_TRIGGER_GENERATED_030.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1793606 | 1598356 | <span style="color:#2563eb">46.72%</span> |
| 198 | [00345 SCALAR_CAST_TYPEOF_030](crates/bench/sqlite_parity/cases/SQLITE_PARITY_345_SCALAR_CAST_TYPEOF_030.rs) | P1 | memory | GEN_SQL_SCALAR | 1479582 | 1598287 | <span style="color:#2563eb">46.72%</span> |
| 199 | [00052 PRAGMA_INTEGRITY_QUICK_CHECK](crates/bench/sqlite_parity/cases/SQLITE_PARITY_052_PRAGMA_INTEGRITY_QUICK_CHECK.rs) | P0 | memory | SQL_PRAGMA | 1832861 | 1596503 | <span style="color:#2563eb">46.78%</span> |
| 200 | [00871 CONSTRAINT_FK_SAVEPOINT_004](crates/bench/sqlite_parity/cases/SQLITE_PARITY_871_CONSTRAINT_FK_SAVEPOINT_004.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1689449 | 1594359 | <span style="color:#2563eb">46.85%</span> |
| 201 | [00064 CTE_NON_RECURSIVE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_064_CTE_NON_RECURSIVE.rs) | P0 | memory | SQL_CTE | 1523054 | 1593568 | <span style="color:#2563eb">46.88%</span> |
| 202 | [01110 INDEX_SCHEMA_PRAGMA_043](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1110_INDEX_SCHEMA_PRAGMA_043.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1737120 | 1592335 | <span style="color:#2563eb">46.92%</span> |
| 203 | [00802 WINDOW_PARTITION_SUM_015](crates/bench/sqlite_parity/cases/SQLITE_PARITY_802_WINDOW_PARTITION_SUM_015.rs) | P2 | memory | GEN_SQL_WINDOW | 1616200 | 1590232 | <span style="color:#2563eb">46.99%</span> |
| 204 | [00985 VIEW_TRIGGER_GENERATED_038](crates/bench/sqlite_parity/cases/SQLITE_PARITY_985_VIEW_TRIGGER_GENERATED_038.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1903695 | 1589680 | <span style="color:#2563eb">47.01%</span> |
| 205 | [00600 AGG_GROUP_HAVING_093](crates/bench/sqlite_parity/cases/SQLITE_PARITY_600_AGG_GROUP_HAVING_093.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1675213 | 1589540 | <span style="color:#2563eb">47.02%</span> |
| 206 | [00444 DML_WHERE_ORDER_LIMIT_057](crates/bench/sqlite_parity/cases/SQLITE_PARITY_444_DML_WHERE_ORDER_LIMIT_057.rs) | P1 | memory | GEN_SQL_DML | 2147837 | 1589410 | <span style="color:#2563eb">47.02%</span> |
| 207 | [00947 CONSTRAINT_FK_SAVEPOINT_080](crates/bench/sqlite_parity/cases/SQLITE_PARITY_947_CONSTRAINT_FK_SAVEPOINT_080.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1636258 | 1589069 | <span style="color:#2563eb">47.03%</span> |
| 208 | [00944 CONSTRAINT_FK_SAVEPOINT_077](crates/bench/sqlite_parity/cases/SQLITE_PARITY_944_CONSTRAINT_FK_SAVEPOINT_077.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1802353 | 1588178 | <span style="color:#2563eb">47.06%</span> |
| 209 | [00110 DOT_MODE_LINE_COLUMN_TABLE_BOX_MARKDOWN](crates/bench/sqlite_parity/cases/SQLITE_PARITY_110_DOT_MODE_LINE_COLUMN_TABLE_BOX_MARKDOWN.rs) | P0 | memory | CLI_DOT_COMMAND | 1536380 | 1587366 | <span style="color:#2563eb">47.09%</span> |
| 210 | [00379 SCALAR_NULL_COALESCE_038](crates/bench/sqlite_parity/cases/SQLITE_PARITY_379_SCALAR_NULL_COALESCE_038.rs) | P1 | memory | GEN_SQL_SCALAR | 1488920 | 1585583 | <span style="color:#2563eb">47.15%</span> |
| 211 | [00862 WINDOW_PARTITION_SUM_075](crates/bench/sqlite_parity/cases/SQLITE_PARITY_862_WINDOW_PARTITION_SUM_075.rs) | P2 | memory | GEN_SQL_WINDOW | 1607855 | 1584310 | <span style="color:#2563eb">47.19%</span> |
| 212 | [00601 AGG_GROUP_HAVING_094](crates/bench/sqlite_parity/cases/SQLITE_PARITY_601_AGG_GROUP_HAVING_094.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2534039 | 1584040 | <span style="color:#2563eb">47.20%</span> |
| 213 | [00582 AGG_GROUP_HAVING_075](crates/bench/sqlite_parity/cases/SQLITE_PARITY_582_AGG_GROUP_HAVING_075.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1739203 | 1583579 | <span style="color:#2563eb">47.21%</span> |
| 214 | [00806 WINDOW_PARTITION_SUM_019](crates/bench/sqlite_parity/cases/SQLITE_PARITY_806_WINDOW_PARTITION_SUM_019.rs) | P2 | memory | GEN_SQL_WINDOW | 1626090 | 1583509 | <span style="color:#2563eb">47.22%</span> |
| 215 | [00935 CONSTRAINT_FK_SAVEPOINT_068](crates/bench/sqlite_parity/cases/SQLITE_PARITY_935_CONSTRAINT_FK_SAVEPOINT_068.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1718123 | 1582998 | <span style="color:#2563eb">47.23%</span> |
| 216 | [00008 STRICT_TABLE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_008_STRICT_TABLE.rs) | P0 | memory | SQL_DDL | 1567278 | 1582938 | <span style="color:#2563eb">47.24%</span> |
| 217 | [00261 SCALAR_CAST_TYPEOF_009](crates/bench/sqlite_parity/cases/SQLITE_PARITY_261_SCALAR_CAST_TYPEOF_009.rs) | P1 | memory | GEN_SQL_SCALAR | 1434787 | 1582657 | <span style="color:#2563eb">47.24%</span> |
| 218 | [00488 DML_WHERE_ORDER_LIMIT_101](crates/bench/sqlite_parity/cases/SQLITE_PARITY_488_DML_WHERE_ORDER_LIMIT_101.rs) | P1 | memory | GEN_SQL_DML | 1643352 | 1582066 | <span style="color:#2563eb">47.26%</span> |
| 219 | [00175 OPT_QUOTE_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_175_OPT_QUOTE_MODE.rs) | P1 | memory | CLI_OPTION | 2294134 | 1581725 | <span style="color:#2563eb">47.28%</span> |
| 220 | [00846 WINDOW_PARTITION_SUM_059](crates/bench/sqlite_parity/cases/SQLITE_PARITY_846_WINDOW_PARTITION_SUM_059.rs) | P2 | memory | GEN_SQL_WINDOW | 1613145 | 1581325 | <span style="color:#2563eb">47.29%</span> |
| 221 | [00297 SCALAR_CAST_TYPEOF_018](crates/bench/sqlite_parity/cases/SQLITE_PARITY_297_SCALAR_CAST_TYPEOF_018.rs) | P1 | memory | GEN_SQL_SCALAR | 1467288 | 1580974 | <span style="color:#2563eb">47.30%</span> |
| 222 | [00494 DML_WHERE_ORDER_LIMIT_107](crates/bench/sqlite_parity/cases/SQLITE_PARITY_494_DML_WHERE_ORDER_LIMIT_107.rs) | P1 | memory | GEN_SQL_DML | 1618515 | 1580874 | <span style="color:#2563eb">47.30%</span> |
| 223 | [00861 WINDOW_PARTITION_SUM_074](crates/bench/sqlite_parity/cases/SQLITE_PARITY_861_WINDOW_PARTITION_SUM_074.rs) | P2 | memory | GEN_SQL_WINDOW | 2156705 | 1580173 | <span style="color:#2563eb">47.33%</span> |
| 224 | [00082 CORE_STRING_FUNCTIONS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_082_CORE_STRING_FUNCTIONS.rs) | P0 | memory | SQL_FUNCTIONS | 1460085 | 1579130 | <span style="color:#2563eb">47.36%</span> |
| 225 | [00384 SCALAR_ARITH_040](crates/bench/sqlite_parity/cases/SQLITE_PARITY_384_SCALAR_ARITH_040.rs) | P1 | memory | GEN_SQL_SCALAR | 1465215 | 1578980 | <span style="color:#2563eb">47.37%</span> |
| 226 | [00295 SCALAR_NULL_COALESCE_017](crates/bench/sqlite_parity/cases/SQLITE_PARITY_295_SCALAR_NULL_COALESCE_017.rs) | P1 | memory | GEN_SQL_SCALAR | 1542211 | 1578960 | <span style="color:#2563eb">47.37%</span> |
| 227 | [00941 CONSTRAINT_FK_SAVEPOINT_074](crates/bench/sqlite_parity/cases/SQLITE_PARITY_941_CONSTRAINT_FK_SAVEPOINT_074.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1694709 | 1578910 | <span style="color:#2563eb">47.37%</span> |
| 228 | [00772 CTE_RECURSIVE_MATRIX_065](crates/bench/sqlite_parity/cases/SQLITE_PARITY_772_CTE_RECURSIVE_MATRIX_065.rs) | P1 | memory | GEN_SQL_CTE | 1619026 | 1578179 | <span style="color:#2563eb">47.39%</span> |
| 229 | [00795 WINDOW_PARTITION_SUM_008](crates/bench/sqlite_parity/cases/SQLITE_PARITY_795_WINDOW_PARTITION_SUM_008.rs) | P2 | memory | GEN_SQL_WINDOW | 1888546 | 1577988 | <span style="color:#2563eb">47.40%</span> |
| 230 | [00136 DOT_LOG](crates/bench/sqlite_parity/cases/SQLITE_PARITY_136_DOT_LOG.rs) | P0 | memory | CLI_DOT_COMMAND | 1424037 | 1576416 | <span style="color:#2563eb">47.45%</span> |
| 231 | [00673 JOIN_SUBQUERY_EXISTS_066](crates/bench/sqlite_parity/cases/SQLITE_PARITY_673_JOIN_SUBQUERY_EXISTS_066.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1710228 | 1575694 | <span style="color:#2563eb">47.48%</span> |
| 232 | [00978 VIEW_TRIGGER_GENERATED_031](crates/bench/sqlite_parity/cases/SQLITE_PARITY_978_VIEW_TRIGGER_GENERATED_031.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1733362 | 1575243 | <span style="color:#2563eb">47.49%</span> |
| 233 | [00706 JOIN_SUBQUERY_EXISTS_099](crates/bench/sqlite_parity/cases/SQLITE_PARITY_706_JOIN_SUBQUERY_EXISTS_099.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2049822 | 1575143 | <span style="color:#2563eb">47.50%</span> |
| 234 | [00691 JOIN_SUBQUERY_EXISTS_084](crates/bench/sqlite_parity/cases/SQLITE_PARITY_691_JOIN_SUBQUERY_EXISTS_084.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1647850 | 1574993 | <span style="color:#2563eb">47.50%</span> |
| 235 | [00378 SCALAR_STRING_038](crates/bench/sqlite_parity/cases/SQLITE_PARITY_378_SCALAR_STRING_038.rs) | P1 | memory | GEN_SQL_SCALAR | 1480574 | 1574151 | <span style="color:#2563eb">47.53%</span> |
| 236 | [00066 VALUES_STATEMENT](crates/bench/sqlite_parity/cases/SQLITE_PARITY_066_VALUES_STATEMENT.rs) | P0 | memory | SQL_VALUES | 1454194 | 1574060 | <span style="color:#2563eb">47.53%</span> |
| 237 | [00592 AGG_GROUP_HAVING_085](crates/bench/sqlite_parity/cases/SQLITE_PARITY_592_AGG_GROUP_HAVING_085.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1683969 | 1573830 | <span style="color:#2563eb">47.54%</span> |
| 238 | [00993 VIEW_TRIGGER_GENERATED_046](crates/bench/sqlite_parity/cases/SQLITE_PARITY_993_VIEW_TRIGGER_GENERATED_046.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2202602 | 1573811 | <span style="color:#2563eb">47.54%</span> |
| 239 | [00257 SCALAR_CAST_TYPEOF_008](crates/bench/sqlite_parity/cases/SQLITE_PARITY_257_SCALAR_CAST_TYPEOF_008.rs) | P1 | memory | GEN_SQL_SCALAR | 2493112 | 1573530 | <span style="color:#2563eb">47.55%</span> |
| 240 | [00717 CTE_RECURSIVE_MATRIX_010](crates/bench/sqlite_parity/cases/SQLITE_PARITY_717_CTE_RECURSIVE_MATRIX_010.rs) | P1 | memory | GEN_SQL_CTE | 1669572 | 1573499 | <span style="color:#2563eb">47.55%</span> |
| 241 | [00822 WINDOW_PARTITION_SUM_035](crates/bench/sqlite_parity/cases/SQLITE_PARITY_822_WINDOW_PARTITION_SUM_035.rs) | P2 | memory | GEN_SQL_WINDOW | 2404734 | 1573449 | <span style="color:#2563eb">47.55%</span> |
| 242 | [00159 DOT_SYSTEM_SIDE_EFFECT](crates/bench/sqlite_parity/cases/SQLITE_PARITY_159_DOT_SYSTEM_SIDE_EFFECT.rs) | P4 | side_effect | CLI_SIDE_EFFECT | 2216337 | 1573410 | <span style="color:#2563eb">47.55%</span> |
| 243 | [00015 UPDATE_BASIC](crates/bench/sqlite_parity/cases/SQLITE_PARITY_015_UPDATE_BASIC.rs) | P0 | memory | SQL_UPDATE | 1571556 | 1573069 | <span style="color:#2563eb">47.56%</span> |
| 244 | [00771 CTE_RECURSIVE_MATRIX_064](crates/bench/sqlite_parity/cases/SQLITE_PARITY_771_CTE_RECURSIVE_MATRIX_064.rs) | P1 | memory | GEN_SQL_CTE | 2524491 | 1572829 | <span style="color:#2563eb">47.57%</span> |
| 245 | [00107 DOT_HELP_PATTERN](crates/bench/sqlite_parity/cases/SQLITE_PARITY_107_DOT_HELP_PATTERN.rs) | P0 | memory | CLI_DOT_COMMAND | 1433014 | 1572047 | <span style="color:#2563eb">47.60%</span> |
| 246 | [00598 AGG_GROUP_HAVING_091](crates/bench/sqlite_parity/cases/SQLITE_PARITY_598_AGG_GROUP_HAVING_091.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1681595 | 1571807 | <span style="color:#2563eb">47.61%</span> |
| 247 | [00545 AGG_GROUP_HAVING_038](crates/bench/sqlite_parity/cases/SQLITE_PARITY_545_AGG_GROUP_HAVING_038.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2251324 | 1571787 | <span style="color:#2563eb">47.61%</span> |
| 248 | [00237 SCALAR_CAST_TYPEOF_003](crates/bench/sqlite_parity/cases/SQLITE_PARITY_237_SCALAR_CAST_TYPEOF_003.rs) | P1 | memory | GEN_SQL_SCALAR | 1539526 | 1571736 | <span style="color:#2563eb">47.61%</span> |
| 249 | [00792 WINDOW_PARTITION_SUM_005](crates/bench/sqlite_parity/cases/SQLITE_PARITY_792_WINDOW_PARTITION_SUM_005.rs) | P2 | memory | GEN_SQL_WINDOW | 1601282 | 1571657 | <span style="color:#2563eb">47.61%</span> |
| 250 | [00177 OPT_JSON_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_177_OPT_JSON_MODE.rs) | P1 | memory | CLI_OPTION | 1495342 | 1571566 | <span style="color:#2563eb">47.61%</span> |
| 251 | [00321 SCALAR_CAST_TYPEOF_024](crates/bench/sqlite_parity/cases/SQLITE_PARITY_321_SCALAR_CAST_TYPEOF_024.rs) | P1 | memory | GEN_SQL_SCALAR | 2316467 | 1571336 | <span style="color:#2563eb">47.62%</span> |
| 252 | [00568 AGG_GROUP_HAVING_061](crates/bench/sqlite_parity/cases/SQLITE_PARITY_568_AGG_GROUP_HAVING_061.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1708415 | 1570704 | <span style="color:#2563eb">47.64%</span> |
| 253 | [00014 INSERT_RETURNING](crates/bench/sqlite_parity/cases/SQLITE_PARITY_014_INSERT_RETURNING.rs) | P0 | memory | SQL_INSERT | 1680823 | 1570644 | <span style="color:#2563eb">47.65%</span> |
| 254 | [00708 CTE_RECURSIVE_MATRIX_001](crates/bench/sqlite_parity/cases/SQLITE_PARITY_708_CTE_RECURSIVE_MATRIX_001.rs) | P1 | memory | GEN_SQL_CTE | 1592105 | 1569994 | <span style="color:#2563eb">47.67%</span> |
| 255 | [00179 OPT_MARKDOWN_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_179_OPT_MARKDOWN_MODE.rs) | P2 | memory | CLI_OPTION | 1441550 | 1569632 | <span style="color:#2563eb">47.68%</span> |
| 256 | [00152 DOT_CLONE_TEMPFILE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_152_DOT_CLONE_TEMPFILE.rs) | P2 | tempfile | CLI_TEMPFILE | 2142317 | 1568831 | <span style="color:#2563eb">47.71%</span> |
| 257 | [00065 CTE_RECURSIVE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_065_CTE_RECURSIVE.rs) | P0 | memory | SQL_CTE | 1599569 | 1568771 | <span style="color:#2563eb">47.71%</span> |
| 258 | [00651 JOIN_SUBQUERY_EXISTS_044](crates/bench/sqlite_parity/cases/SQLITE_PARITY_651_JOIN_SUBQUERY_EXISTS_044.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1720418 | 1568530 | <span style="color:#2563eb">47.72%</span> |
| 259 | [00010 GENERATED_COLUMNS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_010_GENERATED_COLUMNS.rs) | P0 | memory | SQL_DDL | 1601162 | 1568480 | <span style="color:#2563eb">47.72%</span> |
| 260 | [00869 CONSTRAINT_FK_SAVEPOINT_002](crates/bench/sqlite_parity/cases/SQLITE_PARITY_869_CONSTRAINT_FK_SAVEPOINT_002.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1688468 | 1568180 | <span style="color:#2563eb">47.73%</span> |
| 261 | [00009 STRICT_TABLE_TYPE_FAILURE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_009_STRICT_TABLE_TYPE_FAILURE.rs) | P0 | memory | SQL_DDL_NEGATIVE | 1527293 | 1568049 | <span style="color:#2563eb">47.73%</span> |
| 262 | [00559 AGG_GROUP_HAVING_052](crates/bench/sqlite_parity/cases/SQLITE_PARITY_559_AGG_GROUP_HAVING_052.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2082393 | 1567128 | <span style="color:#2563eb">47.76%</span> |
| 263 | [00667 JOIN_SUBQUERY_EXISTS_060](crates/bench/sqlite_parity/cases/SQLITE_PARITY_667_JOIN_SUBQUERY_EXISTS_060.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2083655 | 1566707 | <span style="color:#2563eb">47.78%</span> |
| 264 | [00518 AGG_GROUP_HAVING_011](crates/bench/sqlite_parity/cases/SQLITE_PARITY_518_AGG_GROUP_HAVING_011.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1659503 | 1566607 | <span style="color:#2563eb">47.78%</span> |
| 265 | [00473 DML_WHERE_ORDER_LIMIT_086](crates/bench/sqlite_parity/cases/SQLITE_PARITY_473_DML_WHERE_ORDER_LIMIT_086.rs) | P1 | memory | GEN_SQL_DML | 2446553 | 1565866 | <span style="color:#2563eb">47.80%</span> |
| 266 | [00433 DML_WHERE_ORDER_LIMIT_046](crates/bench/sqlite_parity/cases/SQLITE_PARITY_433_DML_WHERE_ORDER_LIMIT_046.rs) | P1 | memory | GEN_SQL_DML | 1634095 | 1565825 | <span style="color:#2563eb">47.81%</span> |
| 267 | [01099 INDEX_SCHEMA_PRAGMA_032](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1099_INDEX_SCHEMA_PRAGMA_032.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 2063147 | 1565805 | <span style="color:#2563eb">47.81%</span> |
| 268 | [00972 VIEW_TRIGGER_GENERATED_025](crates/bench/sqlite_parity/cases/SQLITE_PARITY_972_VIEW_TRIGGER_GENERATED_025.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2126116 | 1565515 | <span style="color:#2563eb">47.82%</span> |
| 269 | [00236 SCALAR_ARITH_003](crates/bench/sqlite_parity/cases/SQLITE_PARITY_236_SCALAR_ARITH_003.rs) | P1 | memory | GEN_SQL_SCALAR | 1481846 | 1565355 | <span style="color:#2563eb">47.82%</span> |
| 270 | [00785 CTE_RECURSIVE_MATRIX_078](crates/bench/sqlite_parity/cases/SQLITE_PARITY_785_CTE_RECURSIVE_MATRIX_078.rs) | P1 | memory | GEN_SQL_CTE | 1601654 | 1565024 | <span style="color:#2563eb">47.83%</span> |
| 271 | [01069 INDEX_SCHEMA_PRAGMA_002](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1069_INDEX_SCHEMA_PRAGMA_002.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 2022951 | 1564113 | <span style="color:#2563eb">47.86%</span> |
| 272 | [00780 CTE_RECURSIVE_MATRIX_073](crates/bench/sqlite_parity/cases/SQLITE_PARITY_780_CTE_RECURSIVE_MATRIX_073.rs) | P1 | memory | GEN_SQL_CTE | 1537101 | 1564001 | <span style="color:#2563eb">47.87%</span> |
| 273 | [00260 SCALAR_ARITH_009](crates/bench/sqlite_parity/cases/SQLITE_PARITY_260_SCALAR_ARITH_009.rs) | P1 | memory | GEN_SQL_SCALAR | 1498347 | 1563541 | <span style="color:#2563eb">47.88%</span> |
| 274 | [00215 TRANSACTION_SYNONYMS_END_COMMIT_TRANSACTION](crates/bench/sqlite_parity/cases/SQLITE_PARITY_215_TRANSACTION_SYNONYMS_END_COMMIT_TRANSACTION.rs) | P0 | memory | SQL_TRANSACTION | 1568851 | 1563380 | <span style="color:#2563eb">47.89%</span> |
| 275 | [00280 SCALAR_ARITH_014](crates/bench/sqlite_parity/cases/SQLITE_PARITY_280_SCALAR_ARITH_014.rs) | P1 | memory | GEN_SQL_SCALAR | 1477368 | 1562960 | <span style="color:#2563eb">47.90%</span> |
| 276 | [00324 SCALAR_ARITH_025](crates/bench/sqlite_parity/cases/SQLITE_PARITY_324_SCALAR_ARITH_025.rs) | P1 | memory | GEN_SQL_SCALAR | 1836357 | 1562939 | <span style="color:#2563eb">47.90%</span> |
| 277 | [00546 AGG_GROUP_HAVING_039](crates/bench/sqlite_parity/cases/SQLITE_PARITY_546_AGG_GROUP_HAVING_039.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1685722 | 1562870 | <span style="color:#2563eb">47.90%</span> |
| 278 | [00451 DML_WHERE_ORDER_LIMIT_064](crates/bench/sqlite_parity/cases/SQLITE_PARITY_451_DML_WHERE_ORDER_LIMIT_064.rs) | P1 | memory | GEN_SQL_DML | 1621160 | 1562820 | <span style="color:#2563eb">47.91%</span> |
| 279 | [00140 DOT_EXPERT_OPTIONAL](crates/bench/sqlite_parity/cases/SQLITE_PARITY_140_DOT_EXPERT_OPTIONAL.rs) | P3 | memory | CLI_DOT_COMMAND_OPTIONAL | 2325905 | 1562759 | <span style="color:#2563eb">47.91%</span> |
| 280 | [00173 OPT_LIST_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_173_OPT_LIST_MODE.rs) | P1 | memory | CLI_OPTION | 1399300 | 1562378 | <span style="color:#2563eb">47.92%</span> |
| 281 | [00452 DML_WHERE_ORDER_LIMIT_065](crates/bench/sqlite_parity/cases/SQLITE_PARITY_452_DML_WHERE_ORDER_LIMIT_065.rs) | P1 | memory | GEN_SQL_DML | 1631630 | 1562248 | <span style="color:#2563eb">47.93%</span> |
| 282 | [00489 DML_WHERE_ORDER_LIMIT_102](crates/bench/sqlite_parity/cases/SQLITE_PARITY_489_DML_WHERE_ORDER_LIMIT_102.rs) | P1 | memory | GEN_SQL_DML | 1627963 | 1561808 | <span style="color:#2563eb">47.94%</span> |
| 283 | [01005 VIEW_TRIGGER_GENERATED_058](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1005_VIEW_TRIGGER_GENERATED_058.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1717873 | 1561508 | <span style="color:#2563eb">47.95%</span> |
| 284 | [00921 CONSTRAINT_FK_SAVEPOINT_054](crates/bench/sqlite_parity/cases/SQLITE_PARITY_921_CONSTRAINT_FK_SAVEPOINT_054.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1666496 | 1561507 | <span style="color:#2563eb">47.95%</span> |
| 285 | [00455 DML_WHERE_ORDER_LIMIT_068](crates/bench/sqlite_parity/cases/SQLITE_PARITY_455_DML_WHERE_ORDER_LIMIT_068.rs) | P1 | memory | GEN_SQL_DML | 1563691 | 1561377 | <span style="color:#2563eb">47.95%</span> |
| 286 | [00484 DML_WHERE_ORDER_LIMIT_097](crates/bench/sqlite_parity/cases/SQLITE_PARITY_484_DML_WHERE_ORDER_LIMIT_097.rs) | P1 | memory | GEN_SQL_DML | 1618205 | 1561347 | <span style="color:#2563eb">47.96%</span> |
| 287 | [00723 CTE_RECURSIVE_MATRIX_016](crates/bench/sqlite_parity/cases/SQLITE_PARITY_723_CTE_RECURSIVE_MATRIX_016.rs) | P1 | memory | GEN_SQL_CTE | 1762177 | 1561237 | <span style="color:#2563eb">47.96%</span> |
| 288 | [00153 DOT_CD_TEMPFILE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_153_DOT_CD_TEMPFILE.rs) | P2 | tempfile | CLI_TEMPFILE | 1263632 | 1561146 | <span style="color:#2563eb">47.96%</span> |
| 289 | [00945 CONSTRAINT_FK_SAVEPOINT_078](crates/bench/sqlite_parity/cases/SQLITE_PARITY_945_CONSTRAINT_FK_SAVEPOINT_078.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1687426 | 1560946 | <span style="color:#2563eb">47.97%</span> |
| 290 | [00893 CONSTRAINT_FK_SAVEPOINT_026](crates/bench/sqlite_parity/cases/SQLITE_PARITY_893_CONSTRAINT_FK_SAVEPOINT_026.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1725568 | 1560625 | <span style="color:#2563eb">47.98%</span> |
| 291 | [00873 CONSTRAINT_FK_SAVEPOINT_006](crates/bench/sqlite_parity/cases/SQLITE_PARITY_873_CONSTRAINT_FK_SAVEPOINT_006.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1660474 | 1560415 | <span style="color:#2563eb">47.99%</span> |
| 292 | [00876 CONSTRAINT_FK_SAVEPOINT_009](crates/bench/sqlite_parity/cases/SQLITE_PARITY_876_CONSTRAINT_FK_SAVEPOINT_009.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1694168 | 1560415 | <span style="color:#2563eb">47.99%</span> |
| 293 | [00067 CASE_COALESCE_NULLIF_IIF](crates/bench/sqlite_parity/cases/SQLITE_PARITY_067_CASE_COALESCE_NULLIF_IIF.rs) | P0 | memory | SQL_EXPRESSIONS | 1446580 | 1560185 | <span style="color:#2563eb">47.99%</span> |
| 294 | [00928 CONSTRAINT_FK_SAVEPOINT_061](crates/bench/sqlite_parity/cases/SQLITE_PARITY_928_CONSTRAINT_FK_SAVEPOINT_061.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1944262 | 1560074 | <span style="color:#2563eb">48.00%</span> |
| 295 | [00109 DOT_MODE_JSON](crates/bench/sqlite_parity/cases/SQLITE_PARITY_109_DOT_MODE_JSON.rs) | P0 | memory | CLI_DOT_COMMAND | 1485223 | 1559925 | <span style="color:#2563eb">48.00%</span> |
| 296 | [00214 DROP_TABLE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_214_DROP_TABLE.rs) | P0 | memory | SQL_DROP | 1604148 | 1559834 | <span style="color:#2563eb">48.01%</span> |
| 297 | [00636 JOIN_SUBQUERY_EXISTS_029](crates/bench/sqlite_parity/cases/SQLITE_PARITY_636_JOIN_SUBQUERY_EXISTS_029.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1752028 | 1559513 | <span style="color:#2563eb">48.02%</span> |
| 298 | [00590 AGG_GROUP_HAVING_083](crates/bench/sqlite_parity/cases/SQLITE_PARITY_590_AGG_GROUP_HAVING_083.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2706866 | 1559353 | <span style="color:#2563eb">48.02%</span> |
| 299 | [00470 DML_WHERE_ORDER_LIMIT_083](crates/bench/sqlite_parity/cases/SQLITE_PARITY_470_DML_WHERE_ORDER_LIMIT_083.rs) | P1 | memory | GEN_SQL_DML | 1619367 | 1559273 | <span style="color:#2563eb">48.02%</span> |
| 300 | [01114 INDEX_SCHEMA_PRAGMA_047](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1114_INDEX_SCHEMA_PRAGMA_047.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1657088 | 1559193 | <span style="color:#2563eb">48.03%</span> |
| 301 | [00111 DOT_HEADERS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_111_DOT_HEADERS.rs) | P0 | memory | CLI_DOT_COMMAND | 1491735 | 1558992 | <span style="color:#2563eb">48.03%</span> |
| 302 | [01035 JSON_EXTRACT_SET_028](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1035_JSON_EXTRACT_SET_028.rs) | P2 | memory | GEN_SQL_JSON | 1570153 | 1558392 | <span style="color:#2563eb">48.05%</span> |
| 303 | [00353 SCALAR_CAST_TYPEOF_032](crates/bench/sqlite_parity/cases/SQLITE_PARITY_353_SCALAR_CAST_TYPEOF_032.rs) | P1 | memory | GEN_SQL_SCALAR | 1448593 | 1558041 | <span style="color:#2563eb">48.07%</span> |
| 304 | [00573 AGG_GROUP_HAVING_066](crates/bench/sqlite_parity/cases/SQLITE_PARITY_573_AGG_GROUP_HAVING_066.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1623695 | 1558031 | <span style="color:#2563eb">48.07%</span> |
| 305 | [00988 VIEW_TRIGGER_GENERATED_041](crates/bench/sqlite_parity/cases/SQLITE_PARITY_988_VIEW_TRIGGER_GENERATED_041.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1756556 | 1557700 | <span style="color:#2563eb">48.08%</span> |
| 306 | [00446 DML_WHERE_ORDER_LIMIT_059](crates/bench/sqlite_parity/cases/SQLITE_PARITY_446_DML_WHERE_ORDER_LIMIT_059.rs) | P1 | memory | GEN_SQL_DML | 1663780 | 1557680 | <span style="color:#2563eb">48.08%</span> |
| 307 | [00083 CORE_NUMERIC_FUNCTIONS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_083_CORE_NUMERIC_FUNCTIONS.rs) | P0 | memory | SQL_FUNCTIONS | 1504209 | 1557470 | <span style="color:#2563eb">48.08%</span> |
| 308 | [00560 AGG_GROUP_HAVING_053](crates/bench/sqlite_parity/cases/SQLITE_PARITY_560_AGG_GROUP_HAVING_053.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1648242 | 1557459 | <span style="color:#2563eb">48.08%</span> |
| 309 | [01018 JSON_EXTRACT_SET_011](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1018_JSON_EXTRACT_SET_011.rs) | P2 | memory | GEN_SQL_JSON | 1531901 | 1557339 | <span style="color:#2563eb">48.09%</span> |
| 310 | [00432 DML_WHERE_ORDER_LIMIT_045](crates/bench/sqlite_parity/cases/SQLITE_PARITY_432_DML_WHERE_ORDER_LIMIT_045.rs) | P1 | memory | GEN_SQL_DML | 1642090 | 1556899 | <span style="color:#2563eb">48.10%</span> |
| 311 | [00006 ROWID_INTEGER_PRIMARY_KEY](crates/bench/sqlite_parity/cases/SQLITE_PARITY_006_ROWID_INTEGER_PRIMARY_KEY.rs) | P0 | memory | SQL_ROWID | 1566216 | 1556838 | <span style="color:#2563eb">48.11%</span> |
| 312 | [00308 SCALAR_ARITH_021](crates/bench/sqlite_parity/cases/SQLITE_PARITY_308_SCALAR_ARITH_021.rs) | P1 | memory | GEN_SQL_SCALAR | 1496104 | 1556758 | <span style="color:#2563eb">48.11%</span> |
| 313 | [00275 SCALAR_NULL_COALESCE_012](crates/bench/sqlite_parity/cases/SQLITE_PARITY_275_SCALAR_NULL_COALESCE_012.rs) | P1 | memory | GEN_SQL_SCALAR | 1480504 | 1556498 | <span style="color:#2563eb">48.12%</span> |
| 314 | [00867 WINDOW_PARTITION_SUM_080](crates/bench/sqlite_parity/cases/SQLITE_PARITY_867_WINDOW_PARTITION_SUM_080.rs) | P2 | memory | GEN_SQL_WINDOW | 1622002 | 1556477 | <span style="color:#2563eb">48.12%</span> |
| 315 | [00181 OPT_TABLE_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_181_OPT_TABLE_MODE.rs) | P2 | memory | CLI_OPTION | 1487006 | 1556407 | <span style="color:#2563eb">48.12%</span> |
| 316 | [00158 DOT_SHELL_SIDE_EFFECT](crates/bench/sqlite_parity/cases/SQLITE_PARITY_158_DOT_SHELL_SIDE_EFFECT.rs) | P4 | side_effect | CLI_SIDE_EFFECT | 2312008 | 1556288 | <span style="color:#2563eb">48.12%</span> |
| 317 | [00855 WINDOW_PARTITION_SUM_068](crates/bench/sqlite_parity/cases/SQLITE_PARITY_855_WINDOW_PARTITION_SUM_068.rs) | P2 | memory | GEN_SQL_WINDOW | 1641308 | 1556107 | <span style="color:#2563eb">48.13%</span> |
| 318 | [00841 WINDOW_PARTITION_SUM_054](crates/bench/sqlite_parity/cases/SQLITE_PARITY_841_WINDOW_PARTITION_SUM_054.rs) | P2 | memory | GEN_SQL_WINDOW | 1635407 | 1555886 | <span style="color:#2563eb">48.14%</span> |
| 319 | [00383 SCALAR_NULL_COALESCE_039](crates/bench/sqlite_parity/cases/SQLITE_PARITY_383_SCALAR_NULL_COALESCE_039.rs) | P1 | memory | GEN_SQL_SCALAR | 1495623 | 1555646 | <span style="color:#2563eb">48.15%</span> |
| 320 | [00979 VIEW_TRIGGER_GENERATED_032](crates/bench/sqlite_parity/cases/SQLITE_PARITY_979_VIEW_TRIGGER_GENERATED_032.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2248618 | 1555636 | <span style="color:#2563eb">48.15%</span> |
| 321 | [00562 AGG_GROUP_HAVING_055](crates/bench/sqlite_parity/cases/SQLITE_PARITY_562_AGG_GROUP_HAVING_055.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1681555 | 1555586 | <span style="color:#2563eb">48.15%</span> |
| 322 | [00429 DML_WHERE_ORDER_LIMIT_042](crates/bench/sqlite_parity/cases/SQLITE_PARITY_429_DML_WHERE_ORDER_LIMIT_042.rs) | P1 | memory | GEN_SQL_DML | 1639766 | 1555535 | <span style="color:#2563eb">48.15%</span> |
| 323 | [00707 JOIN_SUBQUERY_EXISTS_100](crates/bench/sqlite_parity/cases/SQLITE_PARITY_707_JOIN_SUBQUERY_EXISTS_100.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1693367 | 1555426 | <span style="color:#2563eb">48.15%</span> |
| 324 | [00552 AGG_GROUP_HAVING_045](crates/bench/sqlite_parity/cases/SQLITE_PARITY_552_AGG_GROUP_HAVING_045.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1744313 | 1555385 | <span style="color:#2563eb">48.15%</span> |
| 325 | [00194 OPT_IFEXISTS_NEGATIVE_TEMPFILE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_194_OPT_IFEXISTS_NEGATIVE_TEMPFILE.rs) | P3 | tempfile | CLI_OPTION_TEMPFILE_DIAGNOSTIC | 1317325 | 1555285 | <span style="color:#2563eb">48.16%</span> |
| 326 | [00931 CONSTRAINT_FK_SAVEPOINT_064](crates/bench/sqlite_parity/cases/SQLITE_PARITY_931_CONSTRAINT_FK_SAVEPOINT_064.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1846878 | 1555005 | <span style="color:#2563eb">48.17%</span> |
| 327 | [01108 INDEX_SCHEMA_PRAGMA_041](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1108_INDEX_SCHEMA_PRAGMA_041.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1713536 | 1554764 | <span style="color:#2563eb">48.17%</span> |
| 328 | [01007 VIEW_TRIGGER_GENERATED_060](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1007_VIEW_TRIGGER_GENERATED_060.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1682245 | 1554043 | <span style="color:#2563eb">48.20%</span> |
| 329 | [00809 WINDOW_PARTITION_SUM_022](crates/bench/sqlite_parity/cases/SQLITE_PARITY_809_WINDOW_PARTITION_SUM_022.rs) | P2 | memory | GEN_SQL_WINDOW | 1630698 | 1553923 | <span style="color:#2563eb">48.20%</span> |
| 330 | [00241 SCALAR_CAST_TYPEOF_004](crates/bench/sqlite_parity/cases/SQLITE_PARITY_241_SCALAR_CAST_TYPEOF_004.rs) | P1 | memory | GEN_SQL_SCALAR | 1539546 | 1553872 | <span style="color:#2563eb">48.20%</span> |
| 331 | [00797 WINDOW_PARTITION_SUM_010](crates/bench/sqlite_parity/cases/SQLITE_PARITY_797_WINDOW_PARTITION_SUM_010.rs) | P2 | memory | GEN_SQL_WINDOW | 1574953 | 1553673 | <span style="color:#2563eb">48.21%</span> |
| 332 | [00655 JOIN_SUBQUERY_EXISTS_048](crates/bench/sqlite_parity/cases/SQLITE_PARITY_655_JOIN_SUBQUERY_EXISTS_048.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2454248 | 1553542 | <span style="color:#2563eb">48.22%</span> |
| 333 | [01060 JSON_EXTRACT_SET_053](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1060_JSON_EXTRACT_SET_053.rs) | P2 | memory | GEN_SQL_JSON | 1757058 | 1553141 | <span style="color:#2563eb">48.23%</span> |
| 334 | [00716 CTE_RECURSIVE_MATRIX_009](crates/bench/sqlite_parity/cases/SQLITE_PARITY_716_CTE_RECURSIVE_MATRIX_009.rs) | P1 | memory | GEN_SQL_CTE | 1612404 | 1553131 | <span style="color:#2563eb">48.23%</span> |
| 335 | [00338 SCALAR_STRING_028](crates/bench/sqlite_parity/cases/SQLITE_PARITY_338_SCALAR_STRING_028.rs) | P1 | memory | GEN_SQL_SCALAR | 1480654 | 1553021 | <span style="color:#2563eb">48.23%</span> |
| 336 | [01113 INDEX_SCHEMA_PRAGMA_046](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1113_INDEX_SCHEMA_PRAGMA_046.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1669251 | 1552771 | <span style="color:#2563eb">48.24%</span> |
| 337 | [00466 DML_WHERE_ORDER_LIMIT_079](crates/bench/sqlite_parity/cases/SQLITE_PARITY_466_DML_WHERE_ORDER_LIMIT_079.rs) | P1 | memory | GEN_SQL_DML | 1626651 | 1552450 | <span style="color:#2563eb">48.25%</span> |
| 338 | [00761 CTE_RECURSIVE_MATRIX_054](crates/bench/sqlite_parity/cases/SQLITE_PARITY_761_CTE_RECURSIVE_MATRIX_054.rs) | P1 | memory | GEN_SQL_CTE | 1554464 | 1552340 | <span style="color:#2563eb">48.26%</span> |
| 339 | [01092 INDEX_SCHEMA_PRAGMA_025](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1092_INDEX_SCHEMA_PRAGMA_025.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 2087904 | 1552090 | <span style="color:#2563eb">48.26%</span> |
| 340 | [00738 CTE_RECURSIVE_MATRIX_031](crates/bench/sqlite_parity/cases/SQLITE_PARITY_738_CTE_RECURSIVE_MATRIX_031.rs) | P1 | memory | GEN_SQL_CTE | 1595972 | 1551418 | <span style="color:#2563eb">48.29%</span> |
| 341 | [00657 JOIN_SUBQUERY_EXISTS_050](crates/bench/sqlite_parity/cases/SQLITE_PARITY_657_JOIN_SUBQUERY_EXISTS_050.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1718664 | 1551238 | <span style="color:#2563eb">48.29%</span> |
| 342 | [00477 DML_WHERE_ORDER_LIMIT_090](crates/bench/sqlite_parity/cases/SQLITE_PARITY_477_DML_WHERE_ORDER_LIMIT_090.rs) | P1 | memory | GEN_SQL_DML | 1621802 | 1551167 | <span style="color:#2563eb">48.29%</span> |
| 343 | [00727 CTE_RECURSIVE_MATRIX_020](crates/bench/sqlite_parity/cases/SQLITE_PARITY_727_CTE_RECURSIVE_MATRIX_020.rs) | P1 | memory | GEN_SQL_CTE | 1587356 | 1551157 | <span style="color:#2563eb">48.29%</span> |
| 344 | [00804 WINDOW_PARTITION_SUM_017](crates/bench/sqlite_parity/cases/SQLITE_PARITY_804_WINDOW_PARTITION_SUM_017.rs) | P2 | memory | GEN_SQL_WINDOW | 2029443 | 1551037 | <span style="color:#2563eb">48.30%</span> |
| 345 | [00487 DML_WHERE_ORDER_LIMIT_100](crates/bench/sqlite_parity/cases/SQLITE_PARITY_487_DML_WHERE_ORDER_LIMIT_100.rs) | P1 | memory | GEN_SQL_DML | 1641128 | 1550897 | <span style="color:#2563eb">48.30%</span> |
| 346 | [00652 JOIN_SUBQUERY_EXISTS_045](crates/bench/sqlite_parity/cases/SQLITE_PARITY_652_JOIN_SUBQUERY_EXISTS_045.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1938551 | 1550787 | <span style="color:#2563eb">48.31%</span> |
| 347 | [00058 GROUP_BY_HAVING](crates/bench/sqlite_parity/cases/SQLITE_PARITY_058_GROUP_BY_HAVING.rs) | P0 | memory | SQL_AGGREGATE | 1558371 | 1550687 | <span style="color:#2563eb">48.31%</span> |
| 348 | [00621 JOIN_SUBQUERY_EXISTS_014](crates/bench/sqlite_parity/cases/SQLITE_PARITY_621_JOIN_SUBQUERY_EXISTS_014.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1689089 | 1550456 | <span style="color:#2563eb">48.32%</span> |
| 349 | [00821 WINDOW_PARTITION_SUM_034](crates/bench/sqlite_parity/cases/SQLITE_PARITY_821_WINDOW_PARTITION_SUM_034.rs) | P2 | memory | GEN_SQL_WINDOW | 1633654 | 1550416 | <span style="color:#2563eb">48.32%</span> |
| 350 | [00431 DML_WHERE_ORDER_LIMIT_044](crates/bench/sqlite_parity/cases/SQLITE_PARITY_431_DML_WHERE_ORDER_LIMIT_044.rs) | P1 | memory | GEN_SQL_DML | 1617504 | 1550386 | <span style="color:#2563eb">48.32%</span> |
| 351 | [00962 VIEW_TRIGGER_GENERATED_015](crates/bench/sqlite_parity/cases/SQLITE_PARITY_962_VIEW_TRIGGER_GENERATED_015.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1789800 | 1550196 | <span style="color:#2563eb">48.33%</span> |
| 352 | [01086 INDEX_SCHEMA_PRAGMA_019](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1086_INDEX_SCHEMA_PRAGMA_019.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1706962 | 1550116 | <span style="color:#2563eb">48.33%</span> |
| 353 | [00872 CONSTRAINT_FK_SAVEPOINT_005](crates/bench/sqlite_parity/cases/SQLITE_PARITY_872_CONSTRAINT_FK_SAVEPOINT_005.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1685421 | 1549975 | <span style="color:#2563eb">48.33%</span> |
| 354 | [00576 AGG_GROUP_HAVING_069](crates/bench/sqlite_parity/cases/SQLITE_PARITY_576_AGG_GROUP_HAVING_069.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2026728 | 1549865 | <span style="color:#2563eb">48.34%</span> |
| 355 | [00863 WINDOW_PARTITION_SUM_076](crates/bench/sqlite_parity/cases/SQLITE_PARITY_863_WINDOW_PARTITION_SUM_076.rs) | P2 | memory | GEN_SQL_WINDOW | 1662949 | 1549815 | <span style="color:#2563eb">48.34%</span> |
| 356 | [00210 OPT_NOUNICODE_UTF8_CATALOG](crates/bench/sqlite_parity/cases/SQLITE_PARITY_210_OPT_NOUNICODE_UTF8_CATALOG.rs) | P4 | catalog | CLI_OPTION_CATALOG | 2121448 | 1549785 | <span style="color:#2563eb">48.34%</span> |
| 357 | [00619 JOIN_SUBQUERY_EXISTS_012](crates/bench/sqlite_parity/cases/SQLITE_PARITY_619_JOIN_SUBQUERY_EXISTS_012.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1712984 | 1549705 | <span style="color:#2563eb">48.34%</span> |
| 358 | [00812 WINDOW_PARTITION_SUM_025](crates/bench/sqlite_parity/cases/SQLITE_PARITY_812_WINDOW_PARTITION_SUM_025.rs) | P2 | memory | GEN_SQL_WINDOW | 1900839 | 1549655 | <span style="color:#2563eb">48.34%</span> |
| 359 | [01040 JSON_EXTRACT_SET_033](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1040_JSON_EXTRACT_SET_033.rs) | P2 | memory | GEN_SQL_JSON | 1603236 | 1549474 | <span style="color:#2563eb">48.35%</span> |
| 360 | [00906 CONSTRAINT_FK_SAVEPOINT_039](crates/bench/sqlite_parity/cases/SQLITE_PARITY_906_CONSTRAINT_FK_SAVEPOINT_039.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1666325 | 1549424 | <span style="color:#2563eb">48.35%</span> |
| 361 | [00637 JOIN_SUBQUERY_EXISTS_030](crates/bench/sqlite_parity/cases/SQLITE_PARITY_637_JOIN_SUBQUERY_EXISTS_030.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1687716 | 1549304 | <span style="color:#2563eb">48.36%</span> |
| 362 | [00721 CTE_RECURSIVE_MATRIX_014](crates/bench/sqlite_parity/cases/SQLITE_PARITY_721_CTE_RECURSIVE_MATRIX_014.rs) | P1 | memory | GEN_SQL_CTE | 1541369 | 1549244 | <span style="color:#2563eb">48.36%</span> |
| 363 | [00810 WINDOW_PARTITION_SUM_023](crates/bench/sqlite_parity/cases/SQLITE_PARITY_810_WINDOW_PARTITION_SUM_023.rs) | P2 | memory | GEN_SQL_WINDOW | 1627542 | 1548633 | <span style="color:#2563eb">48.38%</span> |
| 364 | [00454 DML_WHERE_ORDER_LIMIT_067](crates/bench/sqlite_parity/cases/SQLITE_PARITY_454_DML_WHERE_ORDER_LIMIT_067.rs) | P1 | memory | GEN_SQL_DML | 1624496 | 1548061 | <span style="color:#2563eb">48.40%</span> |
| 365 | [00329 SCALAR_CAST_TYPEOF_026](crates/bench/sqlite_parity/cases/SQLITE_PARITY_329_SCALAR_CAST_TYPEOF_026.rs) | P1 | memory | GEN_SQL_SCALAR | 1716000 | 1548052 | <span style="color:#2563eb">48.40%</span> |
| 366 | [00731 CTE_RECURSIVE_MATRIX_024](crates/bench/sqlite_parity/cases/SQLITE_PARITY_731_CTE_RECURSIVE_MATRIX_024.rs) | P1 | memory | GEN_SQL_CTE | 1571797 | 1547862 | <span style="color:#2563eb">48.40%</span> |
| 367 | [01008 JSON_EXTRACT_SET_001](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1008_JSON_EXTRACT_SET_001.rs) | P2 | memory | GEN_SQL_JSON | 2063959 | 1547671 | <span style="color:#2563eb">48.41%</span> |
| 368 | [00879 CONSTRAINT_FK_SAVEPOINT_012](crates/bench/sqlite_parity/cases/SQLITE_PARITY_879_CONSTRAINT_FK_SAVEPOINT_012.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1675193 | 1547631 | <span style="color:#2563eb">48.41%</span> |
| 369 | [00668 JOIN_SUBQUERY_EXISTS_061](crates/bench/sqlite_parity/cases/SQLITE_PARITY_668_JOIN_SUBQUERY_EXISTS_061.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1752879 | 1547590 | <span style="color:#2563eb">48.41%</span> |
| 370 | [01088 INDEX_SCHEMA_PRAGMA_021](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1088_INDEX_SCHEMA_PRAGMA_021.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1699548 | 1547541 | <span style="color:#2563eb">48.42%</span> |
| 371 | [01098 INDEX_SCHEMA_PRAGMA_031](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1098_INDEX_SCHEMA_PRAGMA_031.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 2215275 | 1547440 | <span style="color:#2563eb">48.42%</span> |
| 372 | [00362 SCALAR_STRING_034](crates/bench/sqlite_parity/cases/SQLITE_PARITY_362_SCALAR_STRING_034.rs) | P1 | memory | GEN_SQL_SCALAR | 1440508 | 1547271 | <span style="color:#2563eb">48.42%</span> |
| 373 | [00017 DELETE_BASIC](crates/bench/sqlite_parity/cases/SQLITE_PARITY_017_DELETE_BASIC.rs) | P0 | memory | SQL_DELETE | 1599038 | 1547170 | <span style="color:#2563eb">48.43%</span> |
| 374 | [01082 INDEX_SCHEMA_PRAGMA_015](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1082_INDEX_SCHEMA_PRAGMA_015.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1664863 | 1547120 | <span style="color:#2563eb">48.43%</span> |
| 375 | [00756 CTE_RECURSIVE_MATRIX_049](crates/bench/sqlite_parity/cases/SQLITE_PARITY_756_CTE_RECURSIVE_MATRIX_049.rs) | P1 | memory | GEN_SQL_CTE | 1783918 | 1546869 | <span style="color:#2563eb">48.44%</span> |
| 376 | [00982 VIEW_TRIGGER_GENERATED_035](crates/bench/sqlite_parity/cases/SQLITE_PARITY_982_VIEW_TRIGGER_GENERATED_035.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2014415 | 1546589 | <span style="color:#2563eb">48.45%</span> |
| 377 | [00263 SCALAR_NULL_COALESCE_009](crates/bench/sqlite_parity/cases/SQLITE_PARITY_263_SCALAR_NULL_COALESCE_009.rs) | P1 | memory | GEN_SQL_SCALAR | 1477668 | 1546519 | <span style="color:#2563eb">48.45%</span> |
| 378 | [00190 OPT_BAIL](crates/bench/sqlite_parity/cases/SQLITE_PARITY_190_OPT_BAIL.rs) | P1 | memory | CLI_OPTION_NEGATIVE | 1457550 | 1546318 | <span style="color:#2563eb">48.46%</span> |
| 379 | [00556 AGG_GROUP_HAVING_049](crates/bench/sqlite_parity/cases/SQLITE_PARITY_556_AGG_GROUP_HAVING_049.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1631399 | 1546228 | <span style="color:#2563eb">48.46%</span> |
| 380 | [00729 CTE_RECURSIVE_MATRIX_022](crates/bench/sqlite_parity/cases/SQLITE_PARITY_729_CTE_RECURSIVE_MATRIX_022.rs) | P1 | memory | GEN_SQL_CTE | 1532583 | 1546098 | <span style="color:#2563eb">48.46%</span> |
| 381 | [00407 DML_WHERE_ORDER_LIMIT_020](crates/bench/sqlite_parity/cases/SQLITE_PARITY_407_DML_WHERE_ORDER_LIMIT_020.rs) | P1 | memory | GEN_SQL_DML | 1683388 | 1546057 | <span style="color:#2563eb">48.46%</span> |
| 382 | [01107 INDEX_SCHEMA_PRAGMA_040](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1107_INDEX_SCHEMA_PRAGMA_040.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1695190 | 1545967 | <span style="color:#2563eb">48.47%</span> |
| 383 | [00557 AGG_GROUP_HAVING_050](crates/bench/sqlite_parity/cases/SQLITE_PARITY_557_AGG_GROUP_HAVING_050.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1710500 | 1545957 | <span style="color:#2563eb">48.47%</span> |
| 384 | [00569 AGG_GROUP_HAVING_062](crates/bench/sqlite_parity/cases/SQLITE_PARITY_569_AGG_GROUP_HAVING_062.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1716681 | 1545858 | <span style="color:#2563eb">48.47%</span> |
| 385 | [00491 DML_WHERE_ORDER_LIMIT_104](crates/bench/sqlite_parity/cases/SQLITE_PARITY_491_DML_WHERE_ORDER_LIMIT_104.rs) | P1 | memory | GEN_SQL_DML | 1564062 | 1545817 | <span style="color:#2563eb">48.47%</span> |
| 386 | [00674 JOIN_SUBQUERY_EXISTS_067](crates/bench/sqlite_parity/cases/SQLITE_PARITY_674_JOIN_SUBQUERY_EXISTS_067.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1702093 | 1545767 | <span style="color:#2563eb">48.47%</span> |
| 387 | [01102 INDEX_SCHEMA_PRAGMA_035](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1102_INDEX_SCHEMA_PRAGMA_035.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1677176 | 1545607 | <span style="color:#2563eb">48.48%</span> |
| 388 | [00301 SCALAR_CAST_TYPEOF_019](crates/bench/sqlite_parity/cases/SQLITE_PARITY_301_SCALAR_CAST_TYPEOF_019.rs) | P1 | memory | GEN_SQL_SCALAR | 1459954 | 1545517 | <span style="color:#2563eb">48.48%</span> |
| 389 | [00923 CONSTRAINT_FK_SAVEPOINT_056](crates/bench/sqlite_parity/cases/SQLITE_PARITY_923_CONSTRAINT_FK_SAVEPOINT_056.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1700160 | 1545287 | <span style="color:#2563eb">48.49%</span> |
| 390 | [00078 ON_CONFLICT_ALGORITHMS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_078_ON_CONFLICT_ALGORITHMS.rs) | P0 | memory | SQL_CONFLICT | 1602174 | 1545267 | <span style="color:#2563eb">48.49%</span> |
| 391 | [00496 DML_WHERE_ORDER_LIMIT_109](crates/bench/sqlite_parity/cases/SQLITE_PARITY_496_DML_WHERE_ORDER_LIMIT_109.rs) | P1 | memory | GEN_SQL_DML | 1645496 | 1545206 | <span style="color:#2563eb">48.49%</span> |
| 392 | [00495 DML_WHERE_ORDER_LIMIT_108](crates/bench/sqlite_parity/cases/SQLITE_PARITY_495_DML_WHERE_ORDER_LIMIT_108.rs) | P1 | memory | GEN_SQL_DML | 1613456 | 1545186 | <span style="color:#2563eb">48.49%</span> |
| 393 | [00784 CTE_RECURSIVE_MATRIX_077](crates/bench/sqlite_parity/cases/SQLITE_PARITY_784_CTE_RECURSIVE_MATRIX_077.rs) | P1 | memory | GEN_SQL_CTE | 1564423 | 1545146 | <span style="color:#2563eb">48.50%</span> |
| 394 | [00919 CONSTRAINT_FK_SAVEPOINT_052](crates/bench/sqlite_parity/cases/SQLITE_PARITY_919_CONSTRAINT_FK_SAVEPOINT_052.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1681074 | 1545146 | <span style="color:#2563eb">48.50%</span> |
| 395 | [00400 DML_WHERE_ORDER_LIMIT_013](crates/bench/sqlite_parity/cases/SQLITE_PARITY_400_DML_WHERE_ORDER_LIMIT_013.rs) | P1 | memory | GEN_SQL_DML | 1642450 | 1545126 | <span style="color:#2563eb">48.50%</span> |
| 396 | [00497 DML_WHERE_ORDER_LIMIT_110](crates/bench/sqlite_parity/cases/SQLITE_PARITY_497_DML_WHERE_ORDER_LIMIT_110.rs) | P1 | memory | GEN_SQL_DML | 1613475 | 1545086 | <span style="color:#2563eb">48.50%</span> |
| 397 | [00211 SQL_ATTACH_TEMPFILE_DATABASE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_211_SQL_ATTACH_TEMPFILE_DATABASE.rs) | P1 | tempfile | SQL_TEMPFILE | 1822221 | 1544915 | <span style="color:#2563eb">48.50%</span> |
| 398 | [01106 INDEX_SCHEMA_PRAGMA_039](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1106_INDEX_SCHEMA_PRAGMA_039.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1715168 | 1544906 | <span style="color:#2563eb">48.50%</span> |
| 399 | [01080 INDEX_SCHEMA_PRAGMA_013](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1080_INDEX_SCHEMA_PRAGMA_013.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1753500 | 1544905 | <span style="color:#2563eb">48.50%</span> |
| 400 | [00773 CTE_RECURSIVE_MATRIX_066](crates/bench/sqlite_parity/cases/SQLITE_PARITY_773_CTE_RECURSIVE_MATRIX_066.rs) | P1 | memory | GEN_SQL_CTE | 2756099 | 1544835 | <span style="color:#2563eb">48.51%</span> |
| 401 | [00305 SCALAR_CAST_TYPEOF_020](crates/bench/sqlite_parity/cases/SQLITE_PARITY_305_SCALAR_CAST_TYPEOF_020.rs) | P1 | memory | GEN_SQL_SCALAR | 1484882 | 1544806 | <span style="color:#2563eb">48.51%</span> |
| 402 | [00586 AGG_GROUP_HAVING_079](crates/bench/sqlite_parity/cases/SQLITE_PARITY_586_AGG_GROUP_HAVING_079.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1736268 | 1544726 | <span style="color:#2563eb">48.51%</span> |
| 403 | [00743 CTE_RECURSIVE_MATRIX_036](crates/bench/sqlite_parity/cases/SQLITE_PARITY_743_CTE_RECURSIVE_MATRIX_036.rs) | P1 | memory | GEN_SQL_CTE | 1558581 | 1544645 | <span style="color:#2563eb">48.51%</span> |
| 404 | [00842 WINDOW_PARTITION_SUM_055](crates/bench/sqlite_parity/cases/SQLITE_PARITY_842_WINDOW_PARTITION_SUM_055.rs) | P2 | memory | GEN_SQL_WINDOW | 1895449 | 1544645 | <span style="color:#2563eb">48.51%</span> |
| 405 | [00430 DML_WHERE_ORDER_LIMIT_043](crates/bench/sqlite_parity/cases/SQLITE_PARITY_430_DML_WHERE_ORDER_LIMIT_043.rs) | P1 | memory | GEN_SQL_DML | 1739314 | 1544535 | <span style="color:#2563eb">48.52%</span> |
| 406 | [00639 JOIN_SUBQUERY_EXISTS_032](crates/bench/sqlite_parity/cases/SQLITE_PARITY_639_JOIN_SUBQUERY_EXISTS_032.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1667919 | 1544455 | <span style="color:#2563eb">48.52%</span> |
| 407 | [00613 JOIN_SUBQUERY_EXISTS_006](crates/bench/sqlite_parity/cases/SQLITE_PARITY_613_JOIN_SUBQUERY_EXISTS_006.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1823273 | 1544255 | <span style="color:#2563eb">48.52%</span> |
| 408 | [01041 JSON_EXTRACT_SET_034](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1041_JSON_EXTRACT_SET_034.rs) | P2 | memory | GEN_SQL_JSON | 1582226 | 1543984 | <span style="color:#2563eb">48.53%</span> |
| 409 | [00016 UPDATE_RETURNING](crates/bench/sqlite_parity/cases/SQLITE_PARITY_016_UPDATE_RETURNING.rs) | P0 | memory | SQL_UPDATE | 1629335 | 1543904 | <span style="color:#2563eb">48.54%</span> |
| 410 | [00541 AGG_GROUP_HAVING_034](crates/bench/sqlite_parity/cases/SQLITE_PARITY_541_AGG_GROUP_HAVING_034.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2057186 | 1543864 | <span style="color:#2563eb">48.54%</span> |
| 411 | [01079 INDEX_SCHEMA_PRAGMA_012](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1079_INDEX_SCHEMA_PRAGMA_012.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1707083 | 1543823 | <span style="color:#2563eb">48.54%</span> |
| 412 | [00915 CONSTRAINT_FK_SAVEPOINT_048](crates/bench/sqlite_parity/cases/SQLITE_PARITY_915_CONSTRAINT_FK_SAVEPOINT_048.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1706031 | 1543714 | <span style="color:#2563eb">48.54%</span> |
| 413 | [00606 AGG_GROUP_HAVING_099](crates/bench/sqlite_parity/cases/SQLITE_PARITY_606_AGG_GROUP_HAVING_099.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1677617 | 1543603 | <span style="color:#2563eb">48.55%</span> |
| 414 | [00030 ALTER_TABLE_ADD_DROP_COLUMN](crates/bench/sqlite_parity/cases/SQLITE_PARITY_030_ALTER_TABLE_ADD_DROP_COLUMN.rs) | P0 | memory | SQL_ALTER | 1914516 | 1543543 | <span style="color:#2563eb">48.55%</span> |
| 415 | [01123 INDEX_SCHEMA_PRAGMA_056](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1123_INDEX_SCHEMA_PRAGMA_056.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1652810 | 1543272 | <span style="color:#2563eb">48.56%</span> |
| 416 | [00800 WINDOW_PARTITION_SUM_013](crates/bench/sqlite_parity/cases/SQLITE_PARITY_800_WINDOW_PARTITION_SUM_013.rs) | P2 | memory | GEN_SQL_WINDOW | 1587887 | 1543153 | <span style="color:#2563eb">48.56%</span> |
| 417 | [00377 SCALAR_CAST_TYPEOF_038](crates/bench/sqlite_parity/cases/SQLITE_PARITY_377_SCALAR_CAST_TYPEOF_038.rs) | P1 | memory | GEN_SQL_SCALAR | 1424127 | 1543022 | <span style="color:#2563eb">48.57%</span> |
| 418 | [00856 WINDOW_PARTITION_SUM_069](crates/bench/sqlite_parity/cases/SQLITE_PARITY_856_WINDOW_PARTITION_SUM_069.rs) | P2 | memory | GEN_SQL_WINDOW | 1655675 | 1542962 | <span style="color:#2563eb">48.57%</span> |
| 419 | [00521 AGG_GROUP_HAVING_014](crates/bench/sqlite_parity/cases/SQLITE_PARITY_521_AGG_GROUP_HAVING_014.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1892083 | 1542852 | <span style="color:#2563eb">48.57%</span> |
| 420 | [00709 CTE_RECURSIVE_MATRIX_002](crates/bench/sqlite_parity/cases/SQLITE_PARITY_709_CTE_RECURSIVE_MATRIX_002.rs) | P1 | memory | GEN_SQL_CTE | 1575103 | 1542742 | <span style="color:#2563eb">48.58%</span> |
| 421 | [00749 CTE_RECURSIVE_MATRIX_042](crates/bench/sqlite_parity/cases/SQLITE_PARITY_749_CTE_RECURSIVE_MATRIX_042.rs) | P1 | memory | GEN_SQL_CTE | 1563501 | 1542531 | <span style="color:#2563eb">48.58%</span> |
| 422 | [00274 SCALAR_STRING_012](crates/bench/sqlite_parity/cases/SQLITE_PARITY_274_SCALAR_STRING_012.rs) | P1 | memory | GEN_SQL_SCALAR | 1490242 | 1542471 | <span style="color:#2563eb">48.58%</span> |
| 423 | [00228 SCALAR_ARITH_001](crates/bench/sqlite_parity/cases/SQLITE_PARITY_228_SCALAR_ARITH_001.rs) | P1 | memory | GEN_SQL_SCALAR | 1593278 | 1542361 | <span style="color:#2563eb">48.59%</span> |
| 424 | [01115 INDEX_SCHEMA_PRAGMA_048](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1115_INDEX_SCHEMA_PRAGMA_048.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 2218041 | 1542261 | <span style="color:#2563eb">48.59%</span> |
| 425 | [00038 CREATE_TRIGGER_AFTER](crates/bench/sqlite_parity/cases/SQLITE_PARITY_038_CREATE_TRIGGER_AFTER.rs) | P0 | memory | SQL_TRIGGER | 1574191 | 1542160 | <span style="color:#2563eb">48.59%</span> |
| 426 | [00999 VIEW_TRIGGER_GENERATED_052](crates/bench/sqlite_parity/cases/SQLITE_PARITY_999_VIEW_TRIGGER_GENERATED_052.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1717101 | 1542151 | <span style="color:#2563eb">48.59%</span> |
| 427 | [00424 DML_WHERE_ORDER_LIMIT_037](crates/bench/sqlite_parity/cases/SQLITE_PARITY_424_DML_WHERE_ORDER_LIMIT_037.rs) | P1 | memory | GEN_SQL_DML | 1615770 | 1542110 | <span style="color:#2563eb">48.60%</span> |
| 428 | [00286 SCALAR_STRING_015](crates/bench/sqlite_parity/cases/SQLITE_PARITY_286_SCALAR_STRING_015.rs) | P1 | memory | GEN_SQL_SCALAR | 1462690 | 1542101 | <span style="color:#2563eb">48.60%</span> |
| 429 | [00423 DML_WHERE_ORDER_LIMIT_036](crates/bench/sqlite_parity/cases/SQLITE_PARITY_423_DML_WHERE_ORDER_LIMIT_036.rs) | P1 | memory | GEN_SQL_DML | 1624817 | 1541991 | <span style="color:#2563eb">48.60%</span> |
| 430 | [00079 CHECK_CONSTRAINT_FAILURE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_079_CHECK_CONSTRAINT_FAILURE.rs) | P0 | memory | SQL_CONSTRAINTS_NEGATIVE | 1523896 | 1541860 | <span style="color:#2563eb">48.60%</span> |
| 431 | [01017 JSON_EXTRACT_SET_010](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1017_JSON_EXTRACT_SET_010.rs) | P2 | memory | GEN_SQL_JSON | 1573540 | 1541770 | <span style="color:#2563eb">48.61%</span> |
| 432 | [01059 JSON_EXTRACT_SET_052](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1059_JSON_EXTRACT_SET_052.rs) | P2 | memory | GEN_SQL_JSON | 1564303 | 1541770 | <span style="color:#2563eb">48.61%</span> |
| 433 | [00322 SCALAR_STRING_024](crates/bench/sqlite_parity/cases/SQLITE_PARITY_322_SCALAR_STRING_024.rs) | P1 | memory | GEN_SQL_SCALAR | 1463151 | 1541760 | <span style="color:#2563eb">48.61%</span> |
| 434 | [00722 CTE_RECURSIVE_MATRIX_015](crates/bench/sqlite_parity/cases/SQLITE_PARITY_722_CTE_RECURSIVE_MATRIX_015.rs) | P1 | memory | GEN_SQL_CTE | 1948179 | 1541690 | <span style="color:#2563eb">48.61%</span> |
| 435 | [00757 CTE_RECURSIVE_MATRIX_050](crates/bench/sqlite_parity/cases/SQLITE_PARITY_757_CTE_RECURSIVE_MATRIX_050.rs) | P1 | memory | GEN_SQL_CTE | 1556538 | 1541519 | <span style="color:#2563eb">48.62%</span> |
| 436 | [00759 CTE_RECURSIVE_MATRIX_052](crates/bench/sqlite_parity/cases/SQLITE_PARITY_759_CTE_RECURSIVE_MATRIX_052.rs) | P1 | memory | GEN_SQL_CTE | 1928933 | 1541509 | <span style="color:#2563eb">48.62%</span> |
| 437 | [01019 JSON_EXTRACT_SET_012](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1019_JSON_EXTRACT_SET_012.rs) | P2 | memory | GEN_SQL_JSON | 1604619 | 1541439 | <span style="color:#2563eb">48.62%</span> |
| 438 | [00186 OPT_NEWLINE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_186_OPT_NEWLINE.rs) | P2 | memory | CLI_OPTION | 2509933 | 1541169 | <span style="color:#2563eb">48.63%</span> |
| 439 | [00029 ALTER_TABLE_RENAME_COLUMN](crates/bench/sqlite_parity/cases/SQLITE_PARITY_029_ALTER_TABLE_RENAME_COLUMN.rs) | P0 | memory | SQL_ALTER | 1945464 | 1541099 | <span style="color:#2563eb">48.63%</span> |
| 440 | [00788 WINDOW_PARTITION_SUM_001](crates/bench/sqlite_parity/cases/SQLITE_PARITY_788_WINDOW_PARTITION_SUM_001.rs) | P2 | memory | GEN_SQL_WINDOW | 1656036 | 1540928 | <span style="color:#2563eb">48.64%</span> |
| 441 | [00943 CONSTRAINT_FK_SAVEPOINT_076](crates/bench/sqlite_parity/cases/SQLITE_PARITY_943_CONSTRAINT_FK_SAVEPOINT_076.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1650215 | 1540497 | <span style="color:#2563eb">48.65%</span> |
| 442 | [00238 SCALAR_STRING_003](crates/bench/sqlite_parity/cases/SQLITE_PARITY_238_SCALAR_STRING_003.rs) | P1 | memory | GEN_SQL_SCALAR | 1490843 | 1540337 | <span style="color:#2563eb">48.66%</span> |
| 443 | [00647 JOIN_SUBQUERY_EXISTS_040](crates/bench/sqlite_parity/cases/SQLITE_PARITY_647_JOIN_SUBQUERY_EXISTS_040.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1706552 | 1540327 | <span style="color:#2563eb">48.66%</span> |
| 444 | [00361 SCALAR_CAST_TYPEOF_034](crates/bench/sqlite_parity/cases/SQLITE_PARITY_361_SCALAR_CAST_TYPEOF_034.rs) | P1 | memory | GEN_SQL_SCALAR | 1450156 | 1540306 | <span style="color:#2563eb">48.66%</span> |
| 445 | [00515 AGG_GROUP_HAVING_008](crates/bench/sqlite_parity/cases/SQLITE_PARITY_515_AGG_GROUP_HAVING_008.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1709377 | 1540267 | <span style="color:#2563eb">48.66%</span> |
| 446 | [00041 DROP_TRIGGER](crates/bench/sqlite_parity/cases/SQLITE_PARITY_041_DROP_TRIGGER.rs) | P0 | memory | SQL_DROP | 1577076 | 1540097 | <span style="color:#2563eb">48.66%</span> |
| 447 | [00441 DML_WHERE_ORDER_LIMIT_054](crates/bench/sqlite_parity/cases/SQLITE_PARITY_441_DML_WHERE_ORDER_LIMIT_054.rs) | P1 | memory | GEN_SQL_DML | 1690682 | 1539926 | <span style="color:#2563eb">48.67%</span> |
| 448 | [00539 AGG_GROUP_HAVING_032](crates/bench/sqlite_parity/cases/SQLITE_PARITY_539_AGG_GROUP_HAVING_032.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1714186 | 1539907 | <span style="color:#2563eb">48.67%</span> |
| 449 | [00442 DML_WHERE_ORDER_LIMIT_055](crates/bench/sqlite_parity/cases/SQLITE_PARITY_442_DML_WHERE_ORDER_LIMIT_055.rs) | P1 | memory | GEN_SQL_DML | 1618044 | 1539766 | <span style="color:#2563eb">48.67%</span> |
| 450 | [00775 CTE_RECURSIVE_MATRIX_068](crates/bench/sqlite_parity/cases/SQLITE_PARITY_775_CTE_RECURSIVE_MATRIX_068.rs) | P1 | memory | GEN_SQL_CTE | 2644407 | 1539696 | <span style="color:#2563eb">48.68%</span> |
| 451 | [00095 CREATE_VIRTUAL_TABLE_RTREE_OPTIONAL](crates/bench/sqlite_parity/cases/SQLITE_PARITY_095_CREATE_VIRTUAL_TABLE_RTREE_OPTIONAL.rs) | P2 | memory | SQL_VIRTUAL_TABLE_OPTIONAL | 1958108 | 1539685 | <span style="color:#2563eb">48.68%</span> |
| 452 | [00245 SCALAR_CAST_TYPEOF_005](crates/bench/sqlite_parity/cases/SQLITE_PARITY_245_SCALAR_CAST_TYPEOF_005.rs) | P1 | memory | GEN_SQL_SCALAR | 1480404 | 1539535 | <span style="color:#2563eb">48.68%</span> |
| 453 | [00394 DML_WHERE_ORDER_LIMIT_007](crates/bench/sqlite_parity/cases/SQLITE_PARITY_394_DML_WHERE_ORDER_LIMIT_007.rs) | P1 | memory | GEN_SQL_DML | 1635587 | 1539526 | <span style="color:#2563eb">48.68%</span> |
| 454 | [00458 DML_WHERE_ORDER_LIMIT_071](crates/bench/sqlite_parity/cases/SQLITE_PARITY_458_DML_WHERE_ORDER_LIMIT_071.rs) | P1 | memory | GEN_SQL_DML | 1641449 | 1539315 | <span style="color:#2563eb">48.69%</span> |
| 455 | [00130 DOT_OPEN_MEMORY](crates/bench/sqlite_parity/cases/SQLITE_PARITY_130_DOT_OPEN_MEMORY.rs) | P0 | memory | CLI_DOT_COMMAND | 1550396 | 1539245 | <span style="color:#2563eb">48.69%</span> |
| 456 | [00880 CONSTRAINT_FK_SAVEPOINT_013](crates/bench/sqlite_parity/cases/SQLITE_PARITY_880_CONSTRAINT_FK_SAVEPOINT_013.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1740776 | 1539215 | <span style="color:#2563eb">48.69%</span> |
| 457 | [00033 PARTIAL_INDEX](crates/bench/sqlite_parity/cases/SQLITE_PARITY_033_PARTIAL_INDEX.rs) | P0 | memory | SQL_INDEX | 1754383 | 1539205 | <span style="color:#2563eb">48.69%</span> |
| 458 | [00914 CONSTRAINT_FK_SAVEPOINT_047](crates/bench/sqlite_parity/cases/SQLITE_PARITY_914_CONSTRAINT_FK_SAVEPOINT_047.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1697284 | 1539205 | <span style="color:#2563eb">48.69%</span> |
| 459 | [00256 SCALAR_ARITH_008](crates/bench/sqlite_parity/cases/SQLITE_PARITY_256_SCALAR_ARITH_008.rs) | P1 | memory | GEN_SQL_SCALAR | 1481886 | 1539125 | <span style="color:#2563eb">48.70%</span> |
| 460 | [00316 SCALAR_ARITH_023](crates/bench/sqlite_parity/cases/SQLITE_PARITY_316_SCALAR_ARITH_023.rs) | P1 | memory | GEN_SQL_SCALAR | 1435308 | 1539124 | <span style="color:#2563eb">48.70%</span> |
| 461 | [00032 UNIQUE_INDEX_FAILURE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_032_UNIQUE_INDEX_FAILURE.rs) | P0 | memory | SQL_INDEX_NEGATIVE | 1553652 | 1539094 | <span style="color:#2563eb">48.70%</span> |
| 462 | [00585 AGG_GROUP_HAVING_078](crates/bench/sqlite_parity/cases/SQLITE_PARITY_585_AGG_GROUP_HAVING_078.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1715719 | 1538744 | <span style="color:#2563eb">48.71%</span> |
| 463 | [00475 DML_WHERE_ORDER_LIMIT_088](crates/bench/sqlite_parity/cases/SQLITE_PARITY_475_DML_WHERE_ORDER_LIMIT_088.rs) | P1 | memory | GEN_SQL_DML | 1616461 | 1538644 | <span style="color:#2563eb">48.71%</span> |
| 464 | [00695 JOIN_SUBQUERY_EXISTS_088](crates/bench/sqlite_parity/cases/SQLITE_PARITY_695_JOIN_SUBQUERY_EXISTS_088.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2698761 | 1538614 | <span style="color:#2563eb">48.71%</span> |
| 465 | [00828 WINDOW_PARTITION_SUM_041](crates/bench/sqlite_parity/cases/SQLITE_PARITY_828_WINDOW_PARTITION_SUM_041.rs) | P2 | memory | GEN_SQL_WINDOW | 1640306 | 1538513 | <span style="color:#2563eb">48.72%</span> |
| 466 | [00925 CONSTRAINT_FK_SAVEPOINT_058](crates/bench/sqlite_parity/cases/SQLITE_PARITY_925_CONSTRAINT_FK_SAVEPOINT_058.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1685842 | 1538423 | <span style="color:#2563eb">48.72%</span> |
| 467 | [00390 DML_WHERE_ORDER_LIMIT_003](crates/bench/sqlite_parity/cases/SQLITE_PARITY_390_DML_WHERE_ORDER_LIMIT_003.rs) | P1 | memory | GEN_SQL_DML | 1668811 | 1538343 | <span style="color:#2563eb">48.72%</span> |
| 468 | [00917 CONSTRAINT_FK_SAVEPOINT_050](crates/bench/sqlite_parity/cases/SQLITE_PARITY_917_CONSTRAINT_FK_SAVEPOINT_050.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1985460 | 1538333 | <span style="color:#2563eb">48.72%</span> |
| 469 | [00597 AGG_GROUP_HAVING_090](crates/bench/sqlite_parity/cases/SQLITE_PARITY_597_AGG_GROUP_HAVING_090.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1703607 | 1538133 | <span style="color:#2563eb">48.73%</span> |
| 470 | [01078 INDEX_SCHEMA_PRAGMA_011](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1078_INDEX_SCHEMA_PRAGMA_011.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1717583 | 1537983 | <span style="color:#2563eb">48.73%</span> |
| 471 | [00427 DML_WHERE_ORDER_LIMIT_040](crates/bench/sqlite_parity/cases/SQLITE_PARITY_427_DML_WHERE_ORDER_LIMIT_040.rs) | P1 | memory | GEN_SQL_DML | 1645406 | 1537932 | <span style="color:#2563eb">48.74%</span> |
| 472 | [00901 CONSTRAINT_FK_SAVEPOINT_034](crates/bench/sqlite_parity/cases/SQLITE_PARITY_901_CONSTRAINT_FK_SAVEPOINT_034.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1671325 | 1537893 | <span style="color:#2563eb">48.74%</span> |
| 473 | [00755 CTE_RECURSIVE_MATRIX_048](crates/bench/sqlite_parity/cases/SQLITE_PARITY_755_CTE_RECURSIVE_MATRIX_048.rs) | P1 | memory | GEN_SQL_CTE | 1571045 | 1537892 | <span style="color:#2563eb">48.74%</span> |
| 474 | [00365 SCALAR_CAST_TYPEOF_035](crates/bench/sqlite_parity/cases/SQLITE_PARITY_365_SCALAR_CAST_TYPEOF_035.rs) | P1 | memory | GEN_SQL_SCALAR | 1666426 | 1537882 | <span style="color:#2563eb">48.74%</span> |
| 475 | [00040 INSTEAD_OF_TRIGGER_ON_VIEW](crates/bench/sqlite_parity/cases/SQLITE_PARITY_040_INSTEAD_OF_TRIGGER_ON_VIEW.rs) | P0 | memory | SQL_TRIGGER | 1551728 | 1537872 | <span style="color:#2563eb">48.74%</span> |
| 476 | [00680 JOIN_SUBQUERY_EXISTS_073](crates/bench/sqlite_parity/cases/SQLITE_PARITY_680_JOIN_SUBQUERY_EXISTS_073.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1635748 | 1537852 | <span style="color:#2563eb">48.74%</span> |
| 477 | [00415 DML_WHERE_ORDER_LIMIT_028](crates/bench/sqlite_parity/cases/SQLITE_PARITY_415_DML_WHERE_ORDER_LIMIT_028.rs) | P1 | memory | GEN_SQL_DML | 1605761 | 1537752 | <span style="color:#2563eb">48.74%</span> |
| 478 | [00676 JOIN_SUBQUERY_EXISTS_069](crates/bench/sqlite_parity/cases/SQLITE_PARITY_676_JOIN_SUBQUERY_EXISTS_069.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1762187 | 1537722 | <span style="color:#2563eb">48.74%</span> |
| 479 | [00418 DML_WHERE_ORDER_LIMIT_031](crates/bench/sqlite_parity/cases/SQLITE_PARITY_418_DML_WHERE_ORDER_LIMIT_031.rs) | P1 | memory | GEN_SQL_DML | 1639374 | 1537492 | <span style="color:#2563eb">48.75%</span> |
| 480 | [00464 DML_WHERE_ORDER_LIMIT_077](crates/bench/sqlite_parity/cases/SQLITE_PARITY_464_DML_WHERE_ORDER_LIMIT_077.rs) | P1 | memory | GEN_SQL_DML | 1667188 | 1537401 | <span style="color:#2563eb">48.75%</span> |
| 481 | [01013 JSON_EXTRACT_SET_006](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1013_JSON_EXTRACT_SET_006.rs) | P2 | memory | GEN_SQL_JSON | 1559042 | 1537241 | <span style="color:#2563eb">48.76%</span> |
| 482 | [01097 INDEX_SCHEMA_PRAGMA_030](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1097_INDEX_SCHEMA_PRAGMA_030.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1722552 | 1537191 | <span style="color:#2563eb">48.76%</span> |
| 483 | [00970 VIEW_TRIGGER_GENERATED_023](crates/bench/sqlite_parity/cases/SQLITE_PARITY_970_VIEW_TRIGGER_GENERATED_023.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1710169 | 1537070 | <span style="color:#2563eb">48.76%</span> |
| 484 | [00380 SCALAR_ARITH_039](crates/bench/sqlite_parity/cases/SQLITE_PARITY_380_SCALAR_ARITH_039.rs) | P1 | memory | GEN_SQL_SCALAR | 1464674 | 1537040 | <span style="color:#2563eb">48.77%</span> |
| 485 | [00318 SCALAR_STRING_023](crates/bench/sqlite_parity/cases/SQLITE_PARITY_318_SCALAR_STRING_023.rs) | P1 | memory | GEN_SQL_SCALAR | 1676806 | 1536910 | <span style="color:#2563eb">48.77%</span> |
| 486 | [00719 CTE_RECURSIVE_MATRIX_012](crates/bench/sqlite_parity/cases/SQLITE_PARITY_719_CTE_RECURSIVE_MATRIX_012.rs) | P1 | memory | GEN_SQL_CTE | 1565144 | 1536900 | <span style="color:#2563eb">48.77%</span> |
| 487 | [00532 AGG_GROUP_HAVING_025](crates/bench/sqlite_parity/cases/SQLITE_PARITY_532_AGG_GROUP_HAVING_025.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1682927 | 1536800 | <span style="color:#2563eb">48.77%</span> |
| 488 | [00942 CONSTRAINT_FK_SAVEPOINT_075](crates/bench/sqlite_parity/cases/SQLITE_PARITY_942_CONSTRAINT_FK_SAVEPOINT_075.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2032138 | 1536650 | <span style="color:#2563eb">48.78%</span> |
| 489 | [00701 JOIN_SUBQUERY_EXISTS_094](crates/bench/sqlite_parity/cases/SQLITE_PARITY_701_JOIN_SUBQUERY_EXISTS_094.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1721881 | 1536620 | <span style="color:#2563eb">48.78%</span> |
| 490 | [00903 CONSTRAINT_FK_SAVEPOINT_036](crates/bench/sqlite_parity/cases/SQLITE_PARITY_903_CONSTRAINT_FK_SAVEPOINT_036.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1640496 | 1536579 | <span style="color:#2563eb">48.78%</span> |
| 491 | [00408 DML_WHERE_ORDER_LIMIT_021](crates/bench/sqlite_parity/cases/SQLITE_PARITY_408_DML_WHERE_ORDER_LIMIT_021.rs) | P1 | memory | GEN_SQL_DML | 1619387 | 1536470 | <span style="color:#2563eb">48.78%</span> |
| 492 | [00678 JOIN_SUBQUERY_EXISTS_071](crates/bench/sqlite_parity/cases/SQLITE_PARITY_678_JOIN_SUBQUERY_EXISTS_071.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1706682 | 1536400 | <span style="color:#2563eb">48.79%</span> |
| 493 | [00892 CONSTRAINT_FK_SAVEPOINT_025](crates/bench/sqlite_parity/cases/SQLITE_PARITY_892_CONSTRAINT_FK_SAVEPOINT_025.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1660364 | 1536370 | <span style="color:#2563eb">48.79%</span> |
| 494 | [00474 DML_WHERE_ORDER_LIMIT_087](crates/bench/sqlite_parity/cases/SQLITE_PARITY_474_DML_WHERE_ORDER_LIMIT_087.rs) | P1 | memory | GEN_SQL_DML | 1620850 | 1536309 | <span style="color:#2563eb">48.79%</span> |
| 495 | [00370 SCALAR_STRING_036](crates/bench/sqlite_parity/cases/SQLITE_PARITY_370_SCALAR_STRING_036.rs) | P1 | memory | GEN_SQL_SCALAR | 1498106 | 1536139 | <span style="color:#2563eb">48.80%</span> |
| 496 | [00500 DML_WHERE_ORDER_LIMIT_113](crates/bench/sqlite_parity/cases/SQLITE_PARITY_500_DML_WHERE_ORDER_LIMIT_113.rs) | P1 | memory | GEN_SQL_DML | 1612604 | 1536099 | <span style="color:#2563eb">48.80%</span> |
| 497 | [00927 CONSTRAINT_FK_SAVEPOINT_060](crates/bench/sqlite_parity/cases/SQLITE_PARITY_927_CONSTRAINT_FK_SAVEPOINT_060.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1707233 | 1535879 | <span style="color:#2563eb">48.80%</span> |
| 498 | [00250 SCALAR_STRING_006](crates/bench/sqlite_parity/cases/SQLITE_PARITY_250_SCALAR_STRING_006.rs) | P1 | memory | GEN_SQL_SCALAR | 1447291 | 1535859 | <span style="color:#2563eb">48.80%</span> |
| 499 | [01084 INDEX_SCHEMA_PRAGMA_017](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1084_INDEX_SCHEMA_PRAGMA_017.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1706852 | 1535789 | <span style="color:#2563eb">48.81%</span> |
| 500 | [00382 SCALAR_STRING_039](crates/bench/sqlite_parity/cases/SQLITE_PARITY_382_SCALAR_STRING_039.rs) | P1 | memory | GEN_SQL_SCALAR | 1481776 | 1535718 | <span style="color:#2563eb">48.81%</span> |
| 501 | [00405 DML_WHERE_ORDER_LIMIT_018](crates/bench/sqlite_parity/cases/SQLITE_PARITY_405_DML_WHERE_ORDER_LIMIT_018.rs) | P1 | memory | GEN_SQL_DML | 1628654 | 1535498 | <span style="color:#2563eb">48.82%</span> |
| 502 | [00035 DROP_INDEX](crates/bench/sqlite_parity/cases/SQLITE_PARITY_035_DROP_INDEX.rs) | P0 | memory | SQL_DROP | 1563812 | 1535488 | <span style="color:#2563eb">48.82%</span> |
| 503 | [00616 JOIN_SUBQUERY_EXISTS_009](crates/bench/sqlite_parity/cases/SQLITE_PARITY_616_JOIN_SUBQUERY_EXISTS_009.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1698326 | 1535458 | <span style="color:#2563eb">48.82%</span> |
| 504 | [00073 INDEXED_BY](crates/bench/sqlite_parity/cases/SQLITE_PARITY_073_INDEXED_BY.rs) | P0 | memory | SQL_INDEX | 1564934 | 1535417 | <span style="color:#2563eb">48.82%</span> |
| 505 | [00063 WINDOW_EXCLUDE_CURRENT_ROW](crates/bench/sqlite_parity/cases/SQLITE_PARITY_063_WINDOW_EXCLUDE_CURRENT_ROW.rs) | P0 | memory | SQL_WINDOW | 1569582 | 1535228 | <span style="color:#2563eb">48.83%</span> |
| 506 | [00531 AGG_GROUP_HAVING_024](crates/bench/sqlite_parity/cases/SQLITE_PARITY_531_AGG_GROUP_HAVING_024.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1698116 | 1535227 | <span style="color:#2563eb">48.83%</span> |
| 507 | [00341 SCALAR_CAST_TYPEOF_029](crates/bench/sqlite_parity/cases/SQLITE_PARITY_341_SCALAR_CAST_TYPEOF_029.rs) | P1 | memory | GEN_SQL_SCALAR | 1740195 | 1535207 | <span style="color:#2563eb">48.83%</span> |
| 508 | [00450 DML_WHERE_ORDER_LIMIT_063](crates/bench/sqlite_parity/cases/SQLITE_PARITY_450_DML_WHERE_ORDER_LIMIT_063.rs) | P1 | memory | GEN_SQL_DML | 1670914 | 1535097 | <span style="color:#2563eb">48.83%</span> |
| 509 | [01101 INDEX_SCHEMA_PRAGMA_034](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1101_INDEX_SCHEMA_PRAGMA_034.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1760223 | 1534957 | <span style="color:#2563eb">48.83%</span> |
| 510 | [01075 INDEX_SCHEMA_PRAGMA_008](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1075_INDEX_SCHEMA_PRAGMA_008.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1662048 | 1534956 | <span style="color:#2563eb">48.83%</span> |
| 511 | [00885 CONSTRAINT_FK_SAVEPOINT_018](crates/bench/sqlite_parity/cases/SQLITE_PARITY_885_CONSTRAINT_FK_SAVEPOINT_018.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1696492 | 1534787 | <span style="color:#2563eb">48.84%</span> |
| 512 | [00404 DML_WHERE_ORDER_LIMIT_017](crates/bench/sqlite_parity/cases/SQLITE_PARITY_404_DML_WHERE_ORDER_LIMIT_017.rs) | P1 | memory | GEN_SQL_DML | 1629766 | 1534726 | <span style="color:#2563eb">48.84%</span> |
| 513 | [00529 AGG_GROUP_HAVING_022](crates/bench/sqlite_parity/cases/SQLITE_PARITY_529_AGG_GROUP_HAVING_022.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1715609 | 1534586 | <span style="color:#2563eb">48.85%</span> |
| 514 | [00498 DML_WHERE_ORDER_LIMIT_111](crates/bench/sqlite_parity/cases/SQLITE_PARITY_498_DML_WHERE_ORDER_LIMIT_111.rs) | P1 | memory | GEN_SQL_DML | 1631891 | 1534556 | <span style="color:#2563eb">48.85%</span> |
| 515 | [00567 AGG_GROUP_HAVING_060](crates/bench/sqlite_parity/cases/SQLITE_PARITY_567_AGG_GROUP_HAVING_060.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1661527 | 1534506 | <span style="color:#2563eb">48.85%</span> |
| 516 | [00355 SCALAR_NULL_COALESCE_032](crates/bench/sqlite_parity/cases/SQLITE_PARITY_355_SCALAR_NULL_COALESCE_032.rs) | P1 | memory | GEN_SQL_SCALAR | 1442362 | 1534496 | <span style="color:#2563eb">48.85%</span> |
| 517 | [00412 DML_WHERE_ORDER_LIMIT_025](crates/bench/sqlite_parity/cases/SQLITE_PARITY_412_DML_WHERE_ORDER_LIMIT_025.rs) | P1 | memory | GEN_SQL_DML | 1541459 | 1534486 | <span style="color:#2563eb">48.85%</span> |
| 518 | [00440 DML_WHERE_ORDER_LIMIT_053](crates/bench/sqlite_parity/cases/SQLITE_PARITY_440_DML_WHERE_ORDER_LIMIT_053.rs) | P1 | memory | GEN_SQL_DML | 1634856 | 1534486 | <span style="color:#2563eb">48.85%</span> |
| 519 | [00801 WINDOW_PARTITION_SUM_014](crates/bench/sqlite_parity/cases/SQLITE_PARITY_801_WINDOW_PARTITION_SUM_014.rs) | P2 | memory | GEN_SQL_WINDOW | 1658400 | 1534476 | <span style="color:#2563eb">48.85%</span> |
| 520 | [00989 VIEW_TRIGGER_GENERATED_042](crates/bench/sqlite_parity/cases/SQLITE_PARITY_989_VIEW_TRIGGER_GENERATED_042.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1761276 | 1534286 | <span style="color:#2563eb">48.86%</span> |
| 521 | [00672 JOIN_SUBQUERY_EXISTS_065](crates/bench/sqlite_parity/cases/SQLITE_PARITY_672_JOIN_SUBQUERY_EXISTS_065.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1689941 | 1534085 | <span style="color:#2563eb">48.86%</span> |
| 522 | [00240 SCALAR_ARITH_004](crates/bench/sqlite_parity/cases/SQLITE_PARITY_240_SCALAR_ARITH_004.rs) | P1 | memory | GEN_SQL_SCALAR | 1462239 | 1534075 | <span style="color:#2563eb">48.86%</span> |
| 523 | [00371 SCALAR_NULL_COALESCE_036](crates/bench/sqlite_parity/cases/SQLITE_PARITY_371_SCALAR_NULL_COALESCE_036.rs) | P1 | memory | GEN_SQL_SCALAR | 1456218 | 1534025 | <span style="color:#2563eb">48.87%</span> |
| 524 | [00492 DML_WHERE_ORDER_LIMIT_105](crates/bench/sqlite_parity/cases/SQLITE_PARITY_492_DML_WHERE_ORDER_LIMIT_105.rs) | P1 | memory | GEN_SQL_DML | 1566527 | 1533925 | <span style="color:#2563eb">48.87%</span> |
| 525 | [00225 OPT_NONCE_SAFE_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_225_OPT_NONCE_SAFE_MODE.rs) | P2 | memory | CLI_OPTION | 1470034 | 1533915 | <span style="color:#2563eb">48.87%</span> |
| 526 | [00589 AGG_GROUP_HAVING_082](crates/bench/sqlite_parity/cases/SQLITE_PARITY_589_AGG_GROUP_HAVING_082.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1715058 | 1533865 | <span style="color:#2563eb">48.87%</span> |
| 527 | [00997 VIEW_TRIGGER_GENERATED_050](crates/bench/sqlite_parity/cases/SQLITE_PARITY_997_VIEW_TRIGGER_GENERATED_050.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1725106 | 1533844 | <span style="color:#2563eb">48.87%</span> |
| 528 | [00482 DML_WHERE_ORDER_LIMIT_095](crates/bench/sqlite_parity/cases/SQLITE_PARITY_482_DML_WHERE_ORDER_LIMIT_095.rs) | P1 | memory | GEN_SQL_DML | 1604589 | 1533824 | <span style="color:#2563eb">48.87%</span> |
| 529 | [00406 DML_WHERE_ORDER_LIMIT_019](crates/bench/sqlite_parity/cases/SQLITE_PARITY_406_DML_WHERE_ORDER_LIMIT_019.rs) | P1 | memory | GEN_SQL_DML | 1618666 | 1533815 | <span style="color:#2563eb">48.87%</span> |
| 530 | [00279 SCALAR_NULL_COALESCE_013](crates/bench/sqlite_parity/cases/SQLITE_PARITY_279_SCALAR_NULL_COALESCE_013.rs) | P1 | memory | GEN_SQL_SCALAR | 1490694 | 1533754 | <span style="color:#2563eb">48.87%</span> |
| 531 | [00298 SCALAR_STRING_018](crates/bench/sqlite_parity/cases/SQLITE_PARITY_298_SCALAR_STRING_018.rs) | P1 | memory | GEN_SQL_SCALAR | 1471216 | 1533725 | <span style="color:#2563eb">48.88%</span> |
| 532 | [00832 WINDOW_PARTITION_SUM_045](crates/bench/sqlite_parity/cases/SQLITE_PARITY_832_WINDOW_PARTITION_SUM_045.rs) | P2 | memory | GEN_SQL_WINDOW | 1634906 | 1533554 | <span style="color:#2563eb">48.88%</span> |
| 533 | [00469 DML_WHERE_ORDER_LIMIT_082](crates/bench/sqlite_parity/cases/SQLITE_PARITY_469_DML_WHERE_ORDER_LIMIT_082.rs) | P1 | memory | GEN_SQL_DML | 1610861 | 1533484 | <span style="color:#2563eb">48.88%</span> |
| 534 | [00991 VIEW_TRIGGER_GENERATED_044](crates/bench/sqlite_parity/cases/SQLITE_PARITY_991_VIEW_TRIGGER_GENERATED_044.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1733733 | 1533474 | <span style="color:#2563eb">48.88%</span> |
| 535 | [00994 VIEW_TRIGGER_GENERATED_047](crates/bench/sqlite_parity/cases/SQLITE_PARITY_994_VIEW_TRIGGER_GENERATED_047.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1767337 | 1533314 | <span style="color:#2563eb">48.89%</span> |
| 536 | [00584 AGG_GROUP_HAVING_077](crates/bench/sqlite_parity/cases/SQLITE_PARITY_584_AGG_GROUP_HAVING_077.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1649584 | 1533284 | <span style="color:#2563eb">48.89%</span> |
| 537 | [00936 CONSTRAINT_FK_SAVEPOINT_069](crates/bench/sqlite_parity/cases/SQLITE_PARITY_936_CONSTRAINT_FK_SAVEPOINT_069.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1673038 | 1533213 | <span style="color:#2563eb">48.89%</span> |
| 538 | [00203 OPT_ARCHIVE_A_TEMPFILE_OPTIONAL](crates/bench/sqlite_parity/cases/SQLITE_PARITY_203_OPT_ARCHIVE_A_TEMPFILE_OPTIONAL.rs) | P4 | tempfile | CLI_OPTION_TEMPFILE_OPTIONAL | 1771475 | 1533013 | <span style="color:#2563eb">48.90%</span> |
| 539 | [00086 DATE_TIME_FUNCTIONS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_086_DATE_TIME_FUNCTIONS.rs) | P0 | memory | SQL_FUNCTIONS | 1650476 | 1532843 | <span style="color:#2563eb">48.91%</span> |
| 540 | [01034 JSON_EXTRACT_SET_027](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1034_JSON_EXTRACT_SET_027.rs) | P2 | memory | GEN_SQL_JSON | 1553833 | 1532793 | <span style="color:#2563eb">48.91%</span> |
| 541 | [00255 SCALAR_NULL_COALESCE_007](crates/bench/sqlite_parity/cases/SQLITE_PARITY_255_SCALAR_NULL_COALESCE_007.rs) | P1 | memory | GEN_SQL_SCALAR | 1508647 | 1532723 | <span style="color:#2563eb">48.91%</span> |
| 542 | [00627 JOIN_SUBQUERY_EXISTS_020](crates/bench/sqlite_parity/cases/SQLITE_PARITY_627_JOIN_SUBQUERY_EXISTS_020.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1721380 | 1532442 | <span style="color:#2563eb">48.92%</span> |
| 543 | [00831 WINDOW_PARTITION_SUM_044](crates/bench/sqlite_parity/cases/SQLITE_PARITY_831_WINDOW_PARTITION_SUM_044.rs) | P2 | memory | GEN_SQL_WINDOW | 1648762 | 1532362 | <span style="color:#2563eb">48.92%</span> |
| 544 | [00548 AGG_GROUP_HAVING_041](crates/bench/sqlite_parity/cases/SQLITE_PARITY_548_AGG_GROUP_HAVING_041.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1693276 | 1532342 | <span style="color:#2563eb">48.92%</span> |
| 545 | [00611 JOIN_SUBQUERY_EXISTS_004](crates/bench/sqlite_parity/cases/SQLITE_PARITY_611_JOIN_SUBQUERY_EXISTS_004.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2141846 | 1532331 | <span style="color:#2563eb">48.92%</span> |
| 546 | [00877 CONSTRAINT_FK_SAVEPOINT_010](crates/bench/sqlite_parity/cases/SQLITE_PARITY_877_CONSTRAINT_FK_SAVEPOINT_010.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1724486 | 1532322 | <span style="color:#2563eb">48.92%</span> |
| 547 | [01047 JSON_EXTRACT_SET_040](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1047_JSON_EXTRACT_SET_040.rs) | P2 | memory | GEN_SQL_JSON | 1563441 | 1532312 | <span style="color:#2563eb">48.92%</span> |
| 548 | [01057 JSON_EXTRACT_SET_050](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1057_JSON_EXTRACT_SET_050.rs) | P2 | memory | GEN_SQL_JSON | 1562429 | 1532062 | <span style="color:#2563eb">48.93%</span> |
| 549 | [00312 SCALAR_ARITH_022](crates/bench/sqlite_parity/cases/SQLITE_PARITY_312_SCALAR_ARITH_022.rs) | P1 | memory | GEN_SQL_SCALAR | 1715098 | 1531911 | <span style="color:#2563eb">48.94%</span> |
| 550 | [00115 DOT_SCHEMA_TABLES_INDEXES](crates/bench/sqlite_parity/cases/SQLITE_PARITY_115_DOT_SCHEMA_TABLES_INDEXES.rs) | P0 | memory | CLI_DOT_COMMAND | 1994397 | 1531841 | <span style="color:#2563eb">48.94%</span> |
| 551 | [00141 DOT_SHA3SUM](crates/bench/sqlite_parity/cases/SQLITE_PARITY_141_DOT_SHA3SUM.rs) | P0 | memory | CLI_DOT_COMMAND | 1715950 | 1531681 | <span style="color:#2563eb">48.94%</span> |
| 552 | [01038 JSON_EXTRACT_SET_031](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1038_JSON_EXTRACT_SET_031.rs) | P2 | memory | GEN_SQL_JSON | 1580343 | 1531580 | <span style="color:#2563eb">48.95%</span> |
| 553 | [00662 JOIN_SUBQUERY_EXISTS_055](crates/bench/sqlite_parity/cases/SQLITE_PARITY_662_JOIN_SUBQUERY_EXISTS_055.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1728804 | 1531460 | <span style="color:#2563eb">48.95%</span> |
| 554 | [00823 WINDOW_PARTITION_SUM_036](crates/bench/sqlite_parity/cases/SQLITE_PARITY_823_WINDOW_PARTITION_SUM_036.rs) | P2 | memory | GEN_SQL_WINDOW | 1625418 | 1531431 | <span style="color:#2563eb">48.95%</span> |
| 555 | [01093 INDEX_SCHEMA_PRAGMA_026](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1093_INDEX_SCHEMA_PRAGMA_026.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1698186 | 1531390 | <span style="color:#2563eb">48.95%</span> |
| 556 | [01094 INDEX_SCHEMA_PRAGMA_027](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1094_INDEX_SCHEMA_PRAGMA_027.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1718815 | 1531350 | <span style="color:#2563eb">48.95%</span> |
| 557 | [00688 JOIN_SUBQUERY_EXISTS_081](crates/bench/sqlite_parity/cases/SQLITE_PARITY_688_JOIN_SUBQUERY_EXISTS_081.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1720769 | 1531149 | <span style="color:#2563eb">48.96%</span> |
| 558 | [00724 CTE_RECURSIVE_MATRIX_017](crates/bench/sqlite_parity/cases/SQLITE_PARITY_724_CTE_RECURSIVE_MATRIX_017.rs) | P1 | memory | GEN_SQL_CTE | 1588748 | 1531109 | <span style="color:#2563eb">48.96%</span> |
| 559 | [00463 DML_WHERE_ORDER_LIMIT_076](crates/bench/sqlite_parity/cases/SQLITE_PARITY_463_DML_WHERE_ORDER_LIMIT_076.rs) | P1 | memory | GEN_SQL_DML | 1556788 | 1530939 | <span style="color:#2563eb">48.97%</span> |
| 560 | [00090 JSON_MUTATION_FUNCTIONS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_090_JSON_MUTATION_FUNCTIONS.rs) | P0 | memory | SQL_JSON | 1473250 | 1530809 | <span style="color:#2563eb">48.97%</span> |
| 561 | [00835 WINDOW_PARTITION_SUM_048](crates/bench/sqlite_parity/cases/SQLITE_PARITY_835_WINDOW_PARTITION_SUM_048.rs) | P2 | memory | GEN_SQL_WINDOW | 1634215 | 1530678 | <span style="color:#2563eb">48.98%</span> |
| 562 | [00266 SCALAR_STRING_010](crates/bench/sqlite_parity/cases/SQLITE_PARITY_266_SCALAR_STRING_010.rs) | P1 | memory | GEN_SQL_SCALAR | 1429056 | 1530498 | <span style="color:#2563eb">48.98%</span> |
| 563 | [00918 CONSTRAINT_FK_SAVEPOINT_051](crates/bench/sqlite_parity/cases/SQLITE_PARITY_918_CONSTRAINT_FK_SAVEPOINT_051.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1671536 | 1530458 | <span style="color:#2563eb">48.98%</span> |
| 564 | [00289 SCALAR_CAST_TYPEOF_016](crates/bench/sqlite_parity/cases/SQLITE_PARITY_289_SCALAR_CAST_TYPEOF_016.rs) | P1 | memory | GEN_SQL_SCALAR | 1488138 | 1530428 | <span style="color:#2563eb">48.99%</span> |
| 565 | [00699 JOIN_SUBQUERY_EXISTS_092](crates/bench/sqlite_parity/cases/SQLITE_PARITY_699_JOIN_SUBQUERY_EXISTS_092.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1740095 | 1530398 | <span style="color:#2563eb">48.99%</span> |
| 566 | [00644 JOIN_SUBQUERY_EXISTS_037](crates/bench/sqlite_parity/cases/SQLITE_PARITY_644_JOIN_SUBQUERY_EXISTS_037.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1762477 | 1530328 | <span style="color:#2563eb">48.99%</span> |
| 567 | [00883 CONSTRAINT_FK_SAVEPOINT_016](crates/bench/sqlite_parity/cases/SQLITE_PARITY_883_CONSTRAINT_FK_SAVEPOINT_016.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1731739 | 1530208 | <span style="color:#2563eb">48.99%</span> |
| 568 | [00428 DML_WHERE_ORDER_LIMIT_041](crates/bench/sqlite_parity/cases/SQLITE_PARITY_428_DML_WHERE_ORDER_LIMIT_041.rs) | P1 | memory | GEN_SQL_DML | 1622844 | 1530178 | <span style="color:#2563eb">48.99%</span> |
| 569 | [00310 SCALAR_STRING_021](crates/bench/sqlite_parity/cases/SQLITE_PARITY_310_SCALAR_STRING_021.rs) | P1 | memory | GEN_SQL_SCALAR | 1484942 | 1530117 | <span style="color:#2563eb">49.00%</span> |
| 570 | [01032 JSON_EXTRACT_SET_025](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1032_JSON_EXTRACT_SET_025.rs) | P2 | memory | GEN_SQL_JSON | 1576726 | 1530108 | <span style="color:#2563eb">49.00%</span> |
| 571 | [00677 JOIN_SUBQUERY_EXISTS_070](crates/bench/sqlite_parity/cases/SQLITE_PARITY_677_JOIN_SUBQUERY_EXISTS_070.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1735486 | 1530087 | <span style="color:#2563eb">49.00%</span> |
| 572 | [00519 AGG_GROUP_HAVING_012](crates/bench/sqlite_parity/cases/SQLITE_PARITY_519_AGG_GROUP_HAVING_012.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1666436 | 1530007 | <span style="color:#2563eb">49.00%</span> |
| 573 | [00057 COMPOUND_SELECT_UNION_INTERSECT_EXCEPT](crates/bench/sqlite_parity/cases/SQLITE_PARITY_057_COMPOUND_SELECT_UNION_INTERSECT_EXCEPT.rs) | P0 | memory | SQL_SELECT | 1446108 | 1529907 | <span style="color:#2563eb">49.00%</span> |
| 574 | [00564 AGG_GROUP_HAVING_057](crates/bench/sqlite_parity/cases/SQLITE_PARITY_564_AGG_GROUP_HAVING_057.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1660324 | 1529847 | <span style="color:#2563eb">49.01%</span> |
| 575 | [00826 WINDOW_PARTITION_SUM_039](crates/bench/sqlite_parity/cases/SQLITE_PARITY_826_WINDOW_PARTITION_SUM_039.rs) | P2 | memory | GEN_SQL_WINDOW | 1551388 | 1529827 | <span style="color:#2563eb">49.01%</span> |
| 576 | [00271 SCALAR_NULL_COALESCE_011](crates/bench/sqlite_parity/cases/SQLITE_PARITY_271_SCALAR_NULL_COALESCE_011.rs) | P1 | memory | GEN_SQL_SCALAR | 1414869 | 1529797 | <span style="color:#2563eb">49.01%</span> |
| 577 | [00227 OPT_UNSAFE_TESTING_CATALOG](crates/bench/sqlite_parity/cases/SQLITE_PARITY_227_OPT_UNSAFE_TESTING_CATALOG.rs) | P4 | catalog | CLI_OPTION_CATALOG | 1707213 | 1529767 | <span style="color:#2563eb">49.01%</span> |
| 578 | [00868 CONSTRAINT_FK_SAVEPOINT_001](crates/bench/sqlite_parity/cases/SQLITE_PARITY_868_CONSTRAINT_FK_SAVEPOINT_001.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1674841 | 1529767 | <span style="color:#2563eb">49.01%</span> |
| 579 | [00661 JOIN_SUBQUERY_EXISTS_054](crates/bench/sqlite_parity/cases/SQLITE_PARITY_661_JOIN_SUBQUERY_EXISTS_054.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1664623 | 1529757 | <span style="color:#2563eb">49.01%</span> |
| 580 | [00834 WINDOW_PARTITION_SUM_047](crates/bench/sqlite_parity/cases/SQLITE_PARITY_834_WINDOW_PARTITION_SUM_047.rs) | P2 | memory | GEN_SQL_WINDOW | 2073997 | 1529747 | <span style="color:#2563eb">49.01%</span> |
| 581 | [00963 VIEW_TRIGGER_GENERATED_016](crates/bench/sqlite_parity/cases/SQLITE_PARITY_963_VIEW_TRIGGER_GENERATED_016.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1905929 | 1529737 | <span style="color:#2563eb">49.01%</span> |
| 582 | [00526 AGG_GROUP_HAVING_019](crates/bench/sqlite_parity/cases/SQLITE_PARITY_526_AGG_GROUP_HAVING_019.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1662919 | 1529657 | <span style="color:#2563eb">49.01%</span> |
| 583 | [00540 AGG_GROUP_HAVING_033](crates/bench/sqlite_parity/cases/SQLITE_PARITY_540_AGG_GROUP_HAVING_033.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2097041 | 1529657 | <span style="color:#2563eb">49.01%</span> |
| 584 | [00490 DML_WHERE_ORDER_LIMIT_103](crates/bench/sqlite_parity/cases/SQLITE_PARITY_490_DML_WHERE_ORDER_LIMIT_103.rs) | P1 | memory | GEN_SQL_DML | 1615009 | 1529647 | <span style="color:#2563eb">49.01%</span> |
| 585 | [00980 VIEW_TRIGGER_GENERATED_033](crates/bench/sqlite_parity/cases/SQLITE_PARITY_980_VIEW_TRIGGER_GENERATED_033.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1730777 | 1529616 | <span style="color:#2563eb">49.01%</span> |
| 586 | [01061 JSON_EXTRACT_SET_054](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1061_JSON_EXTRACT_SET_054.rs) | P2 | memory | GEN_SQL_JSON | 1584892 | 1529576 | <span style="color:#2563eb">49.01%</span> |
| 587 | [00434 DML_WHERE_ORDER_LIMIT_047](crates/bench/sqlite_parity/cases/SQLITE_PARITY_434_DML_WHERE_ORDER_LIMIT_047.rs) | P1 | memory | GEN_SQL_DML | 1613476 | 1529407 | <span style="color:#2563eb">49.02%</span> |
| 588 | [00751 CTE_RECURSIVE_MATRIX_044](crates/bench/sqlite_parity/cases/SQLITE_PARITY_751_CTE_RECURSIVE_MATRIX_044.rs) | P1 | memory | GEN_SQL_CTE | 1568220 | 1529407 | <span style="color:#2563eb">49.02%</span> |
| 589 | [00023 TRANSACTION_ROLLBACK](crates/bench/sqlite_parity/cases/SQLITE_PARITY_023_TRANSACTION_ROLLBACK.rs) | P0 | memory | SQL_TRANSACTION | 1533394 | 1529366 | <span style="color:#2563eb">49.02%</span> |
| 590 | [00796 WINDOW_PARTITION_SUM_009](crates/bench/sqlite_parity/cases/SQLITE_PARITY_796_WINDOW_PARTITION_SUM_009.rs) | P2 | memory | GEN_SQL_WINDOW | 1628113 | 1529296 | <span style="color:#2563eb">49.02%</span> |
| 591 | [00502 DML_WHERE_ORDER_LIMIT_115](crates/bench/sqlite_parity/cases/SQLITE_PARITY_502_DML_WHERE_ORDER_LIMIT_115.rs) | P1 | memory | GEN_SQL_DML | 1618184 | 1529046 | <span style="color:#2563eb">49.03%</span> |
| 592 | [00219 UPDATE_LIMIT_OPTIONAL](crates/bench/sqlite_parity/cases/SQLITE_PARITY_219_UPDATE_LIMIT_OPTIONAL.rs) | P3 | memory | SQL_UPDATE_OPTIONAL | 1538223 | 1529036 | <span style="color:#2563eb">49.03%</span> |
| 593 | [00538 AGG_GROUP_HAVING_031](crates/bench/sqlite_parity/cases/SQLITE_PARITY_538_AGG_GROUP_HAVING_031.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1867858 | 1529016 | <span style="color:#2563eb">49.03%</span> |
| 594 | [00681 JOIN_SUBQUERY_EXISTS_074](crates/bench/sqlite_parity/cases/SQLITE_PARITY_681_JOIN_SUBQUERY_EXISTS_074.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1709086 | 1528955 | <span style="color:#2563eb">49.03%</span> |
| 595 | [00965 VIEW_TRIGGER_GENERATED_018](crates/bench/sqlite_parity/cases/SQLITE_PARITY_965_VIEW_TRIGGER_GENERATED_018.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2097893 | 1528935 | <span style="color:#2563eb">49.04%</span> |
| 596 | [00472 DML_WHERE_ORDER_LIMIT_085](crates/bench/sqlite_parity/cases/SQLITE_PARITY_472_DML_WHERE_ORDER_LIMIT_085.rs) | P1 | memory | GEN_SQL_DML | 1617854 | 1528865 | <span style="color:#2563eb">49.04%</span> |
| 597 | [00689 JOIN_SUBQUERY_EXISTS_082](crates/bench/sqlite_parity/cases/SQLITE_PARITY_689_JOIN_SUBQUERY_EXISTS_082.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1678529 | 1528765 | <span style="color:#2563eb">49.04%</span> |
| 598 | [00948 VIEW_TRIGGER_GENERATED_001](crates/bench/sqlite_parity/cases/SQLITE_PARITY_948_VIEW_TRIGGER_GENERATED_001.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1754924 | 1528734 | <span style="color:#2563eb">49.04%</span> |
| 599 | [00388 DML_WHERE_ORDER_LIMIT_001](crates/bench/sqlite_parity/cases/SQLITE_PARITY_388_DML_WHERE_ORDER_LIMIT_001.rs) | P1 | memory | GEN_SQL_DML | 1647149 | 1528465 | <span style="color:#2563eb">49.05%</span> |
| 600 | [00875 CONSTRAINT_FK_SAVEPOINT_008](crates/bench/sqlite_parity/cases/SQLITE_PARITY_875_CONSTRAINT_FK_SAVEPOINT_008.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1721470 | 1528384 | <span style="color:#2563eb">49.05%</span> |
| 601 | [00825 WINDOW_PARTITION_SUM_038](crates/bench/sqlite_parity/cases/SQLITE_PARITY_825_WINDOW_PARTITION_SUM_038.rs) | P2 | memory | GEN_SQL_WINDOW | 1591334 | 1528284 | <span style="color:#2563eb">49.06%</span> |
| 602 | [00750 CTE_RECURSIVE_MATRIX_043](crates/bench/sqlite_parity/cases/SQLITE_PARITY_750_CTE_RECURSIVE_MATRIX_043.rs) | P1 | memory | GEN_SQL_CTE | 1570244 | 1528264 | <span style="color:#2563eb">49.06%</span> |
| 603 | [00742 CTE_RECURSIVE_MATRIX_035](crates/bench/sqlite_parity/cases/SQLITE_PARITY_742_CTE_RECURSIVE_MATRIX_035.rs) | P1 | memory | GEN_SQL_CTE | 1580604 | 1528224 | <span style="color:#2563eb">49.06%</span> |
| 604 | [00059 AGGREGATE_FUNCTIONS_CORE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_059_AGGREGATE_FUNCTIONS_CORE.rs) | P0 | memory | SQL_FUNCTIONS | 1504068 | 1528134 | <span style="color:#2563eb">49.06%</span> |
| 605 | [00031 CREATE_INDEX](crates/bench/sqlite_parity/cases/SQLITE_PARITY_031_CREATE_INDEX.rs) | P0 | memory | SQL_INDEX | 1580313 | 1528094 | <span style="color:#2563eb">49.06%</span> |
| 606 | [00690 JOIN_SUBQUERY_EXISTS_083](crates/bench/sqlite_parity/cases/SQLITE_PARITY_690_JOIN_SUBQUERY_EXISTS_083.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2281711 | 1528074 | <span style="color:#2563eb">49.06%</span> |
| 607 | [00857 WINDOW_PARTITION_SUM_070](crates/bench/sqlite_parity/cases/SQLITE_PARITY_857_WINDOW_PARTITION_SUM_070.rs) | P2 | memory | GEN_SQL_WINDOW | 1687025 | 1528013 | <span style="color:#2563eb">49.07%</span> |
| 608 | [00436 DML_WHERE_ORDER_LIMIT_049](crates/bench/sqlite_parity/cases/SQLITE_PARITY_436_DML_WHERE_ORDER_LIMIT_049.rs) | P1 | memory | GEN_SQL_DML | 1619637 | 1527974 | <span style="color:#2563eb">49.07%</span> |
| 609 | [00499 DML_WHERE_ORDER_LIMIT_112](crates/bench/sqlite_parity/cases/SQLITE_PARITY_499_DML_WHERE_ORDER_LIMIT_112.rs) | P1 | memory | GEN_SQL_DML | 1636910 | 1527944 | <span style="color:#2563eb">49.07%</span> |
| 610 | [00123 DOT_ECHO](crates/bench/sqlite_parity/cases/SQLITE_PARITY_123_DOT_ECHO.rs) | P0 | memory | CLI_DOT_COMMAND | 1455196 | 1527934 | <span style="color:#2563eb">49.07%</span> |
| 611 | [00745 CTE_RECURSIVE_MATRIX_038](crates/bench/sqlite_parity/cases/SQLITE_PARITY_745_CTE_RECURSIVE_MATRIX_038.rs) | P1 | memory | GEN_SQL_CTE | 1568971 | 1527623 | <span style="color:#2563eb">49.08%</span> |
| 612 | [00401 DML_WHERE_ORDER_LIMIT_014](crates/bench/sqlite_parity/cases/SQLITE_PARITY_401_DML_WHERE_ORDER_LIMIT_014.rs) | P1 | memory | GEN_SQL_DML | 1605831 | 1527513 | <span style="color:#2563eb">49.08%</span> |
| 613 | [00659 JOIN_SUBQUERY_EXISTS_052](crates/bench/sqlite_parity/cases/SQLITE_PARITY_659_JOIN_SUBQUERY_EXISTS_052.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1720529 | 1527503 | <span style="color:#2563eb">49.08%</span> |
| 614 | [00093 CREATE_VIRTUAL_TABLE_FTS5_OPTIONAL](crates/bench/sqlite_parity/cases/SQLITE_PARITY_093_CREATE_VIRTUAL_TABLE_FTS5_OPTIONAL.rs) | P1 | memory | SQL_VIRTUAL_TABLE_OPTIONAL | 1838412 | 1527452 | <span style="color:#2563eb">49.08%</span> |
| 615 | [00566 AGG_GROUP_HAVING_059](crates/bench/sqlite_parity/cases/SQLITE_PARITY_566_AGG_GROUP_HAVING_059.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1706371 | 1527222 | <span style="color:#2563eb">49.09%</span> |
| 616 | [00664 JOIN_SUBQUERY_EXISTS_057](crates/bench/sqlite_parity/cases/SQLITE_PARITY_664_JOIN_SUBQUERY_EXISTS_057.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1726059 | 1527222 | <span style="color:#2563eb">49.09%</span> |
| 617 | [00102 WITH_MATERIALIZED_HINTS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_102_WITH_MATERIALIZED_HINTS.rs) | P0 | memory | SQL_CTE | 1557089 | 1527152 | <span style="color:#2563eb">49.09%</span> |
| 618 | [01105 INDEX_SCHEMA_PRAGMA_038](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1105_INDEX_SCHEMA_PRAGMA_038.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1708485 | 1526992 | <span style="color:#2563eb">49.10%</span> |
| 619 | [00887 CONSTRAINT_FK_SAVEPOINT_020](crates/bench/sqlite_parity/cases/SQLITE_PARITY_887_CONSTRAINT_FK_SAVEPOINT_020.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1675874 | 1526972 | <span style="color:#2563eb">49.10%</span> |
| 620 | [00369 SCALAR_CAST_TYPEOF_036](crates/bench/sqlite_parity/cases/SQLITE_PARITY_369_SCALAR_CAST_TYPEOF_036.rs) | P1 | memory | GEN_SQL_SCALAR | 1461328 | 1526942 | <span style="color:#2563eb">49.10%</span> |
| 621 | [00957 VIEW_TRIGGER_GENERATED_010](crates/bench/sqlite_parity/cases/SQLITE_PARITY_957_VIEW_TRIGGER_GENERATED_010.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2179929 | 1526911 | <span style="color:#2563eb">49.10%</span> |
| 622 | [00416 DML_WHERE_ORDER_LIMIT_029](crates/bench/sqlite_parity/cases/SQLITE_PARITY_416_DML_WHERE_ORDER_LIMIT_029.rs) | P1 | memory | GEN_SQL_DML | 1605861 | 1526902 | <span style="color:#2563eb">49.10%</span> |
| 623 | [00628 JOIN_SUBQUERY_EXISTS_021](crates/bench/sqlite_parity/cases/SQLITE_PARITY_628_JOIN_SUBQUERY_EXISTS_021.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1849572 | 1526891 | <span style="color:#2563eb">49.10%</span> |
| 624 | [00565 AGG_GROUP_HAVING_058](crates/bench/sqlite_parity/cases/SQLITE_PARITY_565_AGG_GROUP_HAVING_058.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1690061 | 1526771 | <span style="color:#2563eb">49.11%</span> |
| 625 | [00922 CONSTRAINT_FK_SAVEPOINT_055](crates/bench/sqlite_parity/cases/SQLITE_PARITY_922_CONSTRAINT_FK_SAVEPOINT_055.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1647199 | 1526652 | <span style="color:#2563eb">49.11%</span> |
| 626 | [00395 DML_WHERE_ORDER_LIMIT_008](crates/bench/sqlite_parity/cases/SQLITE_PARITY_395_DML_WHERE_ORDER_LIMIT_008.rs) | P1 | memory | GEN_SQL_DML | 1589870 | 1526571 | <span style="color:#2563eb">49.11%</span> |
| 627 | [00799 WINDOW_PARTITION_SUM_012](crates/bench/sqlite_parity/cases/SQLITE_PARITY_799_WINDOW_PARTITION_SUM_012.rs) | P2 | memory | GEN_SQL_WINDOW | 1594289 | 1526511 | <span style="color:#2563eb">49.12%</span> |
| 628 | [00267 SCALAR_NULL_COALESCE_010](crates/bench/sqlite_parity/cases/SQLITE_PARITY_267_SCALAR_NULL_COALESCE_010.rs) | P1 | memory | GEN_SQL_SCALAR | 1443864 | 1526471 | <span style="color:#2563eb">49.12%</span> |
| 629 | [00553 AGG_GROUP_HAVING_046](crates/bench/sqlite_parity/cases/SQLITE_PARITY_553_AGG_GROUP_HAVING_046.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1726971 | 1526431 | <span style="color:#2563eb">49.12%</span> |
| 630 | [00414 DML_WHERE_ORDER_LIMIT_027](crates/bench/sqlite_parity/cases/SQLITE_PARITY_414_DML_WHERE_ORDER_LIMIT_027.rs) | P1 | memory | GEN_SQL_DML | 1608656 | 1526371 | <span style="color:#2563eb">49.12%</span> |
| 631 | [00746 CTE_RECURSIVE_MATRIX_039](crates/bench/sqlite_parity/cases/SQLITE_PARITY_746_CTE_RECURSIVE_MATRIX_039.rs) | P1 | memory | GEN_SQL_CTE | 1594890 | 1526290 | <span style="color:#2563eb">49.12%</span> |
| 632 | [00786 CTE_RECURSIVE_MATRIX_079](crates/bench/sqlite_parity/cases/SQLITE_PARITY_786_CTE_RECURSIVE_MATRIX_079.rs) | P1 | memory | GEN_SQL_CTE | 1589701 | 1526240 | <span style="color:#2563eb">49.13%</span> |
| 633 | [00522 AGG_GROUP_HAVING_015](crates/bench/sqlite_parity/cases/SQLITE_PARITY_522_AGG_GROUP_HAVING_015.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1718524 | 1526230 | <span style="color:#2563eb">49.13%</span> |
| 634 | [01016 JSON_EXTRACT_SET_009](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1016_JSON_EXTRACT_SET_009.rs) | P2 | memory | GEN_SQL_JSON | 2083485 | 1526211 | <span style="color:#2563eb">49.13%</span> |
| 635 | [00187 OPT_NULLVALUE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_187_OPT_NULLVALUE.rs) | P1 | memory | CLI_OPTION | 1561517 | 1526180 | <span style="color:#2563eb">49.13%</span> |
| 636 | [00323 SCALAR_NULL_COALESCE_024](crates/bench/sqlite_parity/cases/SQLITE_PARITY_323_SCALAR_NULL_COALESCE_024.rs) | P1 | memory | GEN_SQL_SCALAR | 1462189 | 1526150 | <span style="color:#2563eb">49.13%</span> |
| 637 | [00314 SCALAR_STRING_022](crates/bench/sqlite_parity/cases/SQLITE_PARITY_314_SCALAR_STRING_022.rs) | P1 | memory | GEN_SQL_SCALAR | 1463591 | 1526101 | <span style="color:#2563eb">49.13%</span> |
| 638 | [00419 DML_WHERE_ORDER_LIMIT_032](crates/bench/sqlite_parity/cases/SQLITE_PARITY_419_DML_WHERE_ORDER_LIMIT_032.rs) | P1 | memory | GEN_SQL_DML | 1609058 | 1526100 | <span style="color:#2563eb">49.13%</span> |
| 639 | [00294 SCALAR_STRING_017](crates/bench/sqlite_parity/cases/SQLITE_PARITY_294_SCALAR_STRING_017.rs) | P1 | memory | GEN_SQL_SCALAR | 1476165 | 1526040 | <span style="color:#2563eb">49.13%</span> |
| 640 | [00403 DML_WHERE_ORDER_LIMIT_016](crates/bench/sqlite_parity/cases/SQLITE_PARITY_403_DML_WHERE_ORDER_LIMIT_016.rs) | P1 | memory | GEN_SQL_DML | 1611232 | 1526000 | <span style="color:#2563eb">49.13%</span> |
| 641 | [01025 JSON_EXTRACT_SET_018](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1025_JSON_EXTRACT_SET_018.rs) | P2 | memory | GEN_SQL_JSON | 1562499 | 1525959 | <span style="color:#2563eb">49.13%</span> |
| 642 | [00037 DROP_VIEW](crates/bench/sqlite_parity/cases/SQLITE_PARITY_037_DROP_VIEW.rs) | P0 | memory | SQL_DROP | 1522002 | 1525820 | <span style="color:#2563eb">49.14%</span> |
| 643 | [00080 NOT_NULL_FAILURE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_080_NOT_NULL_FAILURE.rs) | P0 | memory | SQL_CONSTRAINTS_NEGATIVE | 1521792 | 1525730 | <span style="color:#2563eb">49.14%</span> |
| 644 | [00192 OPT_INIT_TEMPFILE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_192_OPT_INIT_TEMPFILE.rs) | P2 | tempfile | CLI_OPTION_TEMPFILE | 1348444 | 1525720 | <span style="color:#2563eb">49.14%</span> |
| 645 | [00438 DML_WHERE_ORDER_LIMIT_051](crates/bench/sqlite_parity/cases/SQLITE_PARITY_438_DML_WHERE_ORDER_LIMIT_051.rs) | P1 | memory | GEN_SQL_DML | 1867186 | 1525719 | <span style="color:#2563eb">49.14%</span> |
| 646 | [00026 FOREIGN_KEYS_CASCADE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_026_FOREIGN_KEYS_CASCADE.rs) | P0 | memory | SQL_FOREIGN_KEYS | 1675082 | 1525610 | <span style="color:#2563eb">49.15%</span> |
| 647 | [00163 DOT_FILECTRL_CATALOG](crates/bench/sqlite_parity/cases/SQLITE_PARITY_163_DOT_FILECTRL_CATALOG.rs) | P4 | catalog | CLI_CATALOG | 1464594 | 1525609 | <span style="color:#2563eb">49.15%</span> |
| 648 | [00306 SCALAR_STRING_020](crates/bench/sqlite_parity/cases/SQLITE_PARITY_306_SCALAR_STRING_020.rs) | P1 | memory | GEN_SQL_SCALAR | 1481024 | 1525539 | <span style="color:#2563eb">49.15%</span> |
| 649 | [00599 AGG_GROUP_HAVING_092](crates/bench/sqlite_parity/cases/SQLITE_PARITY_599_AGG_GROUP_HAVING_092.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1703736 | 1525539 | <span style="color:#2563eb">49.15%</span> |
| 650 | [00169 DOT_NONCE_SAFE_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_169_DOT_NONCE_SAFE_MODE.rs) | P2 | memory | CLI_DOT_COMMAND | 1948430 | 1525469 | <span style="color:#2563eb">49.15%</span> |
| 651 | [00456 DML_WHERE_ORDER_LIMIT_069](crates/bench/sqlite_parity/cases/SQLITE_PARITY_456_DML_WHERE_ORDER_LIMIT_069.rs) | P1 | memory | GEN_SQL_DML | 1650245 | 1525469 | <span style="color:#2563eb">49.15%</span> |
| 652 | [00393 DML_WHERE_ORDER_LIMIT_006](crates/bench/sqlite_parity/cases/SQLITE_PARITY_393_DML_WHERE_ORDER_LIMIT_006.rs) | P1 | memory | GEN_SQL_DML | 1641188 | 1525228 | <span style="color:#2563eb">49.16%</span> |
| 653 | [00734 CTE_RECURSIVE_MATRIX_027](crates/bench/sqlite_parity/cases/SQLITE_PARITY_734_CTE_RECURSIVE_MATRIX_027.rs) | P1 | memory | GEN_SQL_CTE | 1554494 | 1525198 | <span style="color:#2563eb">49.16%</span> |
| 654 | [00299 SCALAR_NULL_COALESCE_018](crates/bench/sqlite_parity/cases/SQLITE_PARITY_299_SCALAR_NULL_COALESCE_018.rs) | P1 | memory | GEN_SQL_SCALAR | 1453672 | 1525188 | <span style="color:#2563eb">49.16%</span> |
| 655 | [00679 JOIN_SUBQUERY_EXISTS_072](crates/bench/sqlite_parity/cases/SQLITE_PARITY_679_JOIN_SUBQUERY_EXISTS_072.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1682136 | 1525188 | <span style="color:#2563eb">49.16%</span> |
| 656 | [00283 SCALAR_NULL_COALESCE_014](crates/bench/sqlite_parity/cases/SQLITE_PARITY_283_SCALAR_NULL_COALESCE_014.rs) | P1 | memory | GEN_SQL_SCALAR | 1488078 | 1525118 | <span style="color:#2563eb">49.16%</span> |
| 657 | [00273 SCALAR_CAST_TYPEOF_012](crates/bench/sqlite_parity/cases/SQLITE_PARITY_273_SCALAR_CAST_TYPEOF_012.rs) | P1 | memory | GEN_SQL_SCALAR | 1445387 | 1525028 | <span style="color:#2563eb">49.17%</span> |
| 658 | [00381 SCALAR_CAST_TYPEOF_039](crates/bench/sqlite_parity/cases/SQLITE_PARITY_381_SCALAR_CAST_TYPEOF_039.rs) | P1 | memory | GEN_SQL_SCALAR | 1479161 | 1524948 | <span style="color:#2563eb">49.17%</span> |
| 659 | [00663 JOIN_SUBQUERY_EXISTS_056](crates/bench/sqlite_parity/cases/SQLITE_PARITY_663_JOIN_SUBQUERY_EXISTS_056.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1701012 | 1524868 | <span style="color:#2563eb">49.17%</span> |
| 660 | [00437 DML_WHERE_ORDER_LIMIT_050](crates/bench/sqlite_parity/cases/SQLITE_PARITY_437_DML_WHERE_ORDER_LIMIT_050.rs) | P1 | memory | GEN_SQL_DML | 1625909 | 1524567 | <span style="color:#2563eb">49.18%</span> |
| 661 | [00290 SCALAR_STRING_016](crates/bench/sqlite_parity/cases/SQLITE_PARITY_290_SCALAR_STRING_016.rs) | P1 | memory | GEN_SQL_SCALAR | 1630547 | 1524497 | <span style="color:#2563eb">49.18%</span> |
| 662 | [00514 AGG_GROUP_HAVING_007](crates/bench/sqlite_parity/cases/SQLITE_PARITY_514_AGG_GROUP_HAVING_007.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1737931 | 1524467 | <span style="color:#2563eb">49.18%</span> |
| 663 | [00555 AGG_GROUP_HAVING_048](crates/bench/sqlite_parity/cases/SQLITE_PARITY_555_AGG_GROUP_HAVING_048.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1679341 | 1524437 | <span style="color:#2563eb">49.19%</span> |
| 664 | [00409 DML_WHERE_ORDER_LIMIT_022](crates/bench/sqlite_parity/cases/SQLITE_PARITY_409_DML_WHERE_ORDER_LIMIT_022.rs) | P1 | memory | GEN_SQL_DML | 1592966 | 1524307 | <span style="color:#2563eb">49.19%</span> |
| 665 | [00097 CLI_GENERATE_SERIES_OPTIONAL](crates/bench/sqlite_parity/cases/SQLITE_PARITY_097_CLI_GENERATE_SERIES_OPTIONAL.rs) | P2 | memory | CLI_EXTENSION_OPTIONAL | 1799628 | 1524177 | <span style="color:#2563eb">49.19%</span> |
| 666 | [00399 DML_WHERE_ORDER_LIMIT_012](crates/bench/sqlite_parity/cases/SQLITE_PARITY_399_DML_WHERE_ORDER_LIMIT_012.rs) | P1 | memory | GEN_SQL_DML | 1619016 | 1524086 | <span style="color:#2563eb">49.20%</span> |
| 667 | [00744 CTE_RECURSIVE_MATRIX_037](crates/bench/sqlite_parity/cases/SQLITE_PARITY_744_CTE_RECURSIVE_MATRIX_037.rs) | P1 | memory | GEN_SQL_CTE | 1921900 | 1524046 | <span style="color:#2563eb">49.20%</span> |
| 668 | [00183 OPT_TABS_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_183_OPT_TABS_MODE.rs) | P2 | memory | CLI_OPTION | 1479832 | 1523956 | <span style="color:#2563eb">49.20%</span> |
| 669 | [00333 SCALAR_CAST_TYPEOF_027](crates/bench/sqlite_parity/cases/SQLITE_PARITY_333_SCALAR_CAST_TYPEOF_027.rs) | P1 | memory | GEN_SQL_SCALAR | 2520302 | 1523946 | <span style="color:#2563eb">49.20%</span> |
| 670 | [00375 SCALAR_NULL_COALESCE_037](crates/bench/sqlite_parity/cases/SQLITE_PARITY_375_SCALAR_NULL_COALESCE_037.rs) | P1 | memory | GEN_SQL_SCALAR | 1447541 | 1523946 | <span style="color:#2563eb">49.20%</span> |
| 671 | [00654 JOIN_SUBQUERY_EXISTS_047](crates/bench/sqlite_parity/cases/SQLITE_PARITY_654_JOIN_SUBQUERY_EXISTS_047.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2014224 | 1523946 | <span style="color:#2563eb">49.20%</span> |
| 672 | [00631 JOIN_SUBQUERY_EXISTS_024](crates/bench/sqlite_parity/cases/SQLITE_PARITY_631_JOIN_SUBQUERY_EXISTS_024.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1779019 | 1523825 | <span style="color:#2563eb">49.21%</span> |
| 673 | [01054 JSON_EXTRACT_SET_047](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1054_JSON_EXTRACT_SET_047.rs) | P2 | memory | GEN_SQL_JSON | 1574010 | 1523796 | <span style="color:#2563eb">49.21%</span> |
| 674 | [00666 JOIN_SUBQUERY_EXISTS_059](crates/bench/sqlite_parity/cases/SQLITE_PARITY_666_JOIN_SUBQUERY_EXISTS_059.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1673840 | 1523365 | <span style="color:#2563eb">49.22%</span> |
| 675 | [00459 DML_WHERE_ORDER_LIMIT_072](crates/bench/sqlite_parity/cases/SQLITE_PARITY_459_DML_WHERE_ORDER_LIMIT_072.rs) | P1 | memory | GEN_SQL_DML | 1602014 | 1523325 | <span style="color:#2563eb">49.22%</span> |
| 676 | [00366 SCALAR_STRING_035](crates/bench/sqlite_parity/cases/SQLITE_PARITY_366_SCALAR_STRING_035.rs) | P1 | memory | GEN_SQL_SCALAR | 1585613 | 1523295 | <span style="color:#2563eb">49.22%</span> |
| 677 | [00900 CONSTRAINT_FK_SAVEPOINT_033](crates/bench/sqlite_parity/cases/SQLITE_PARITY_900_CONSTRAINT_FK_SAVEPOINT_033.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1674591 | 1523274 | <span style="color:#2563eb">49.22%</span> |
| 678 | [00959 VIEW_TRIGGER_GENERATED_012](crates/bench/sqlite_parity/cases/SQLITE_PARITY_959_VIEW_TRIGGER_GENERATED_012.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1677396 | 1523174 | <span style="color:#2563eb">49.23%</span> |
| 679 | [00817 WINDOW_PARTITION_SUM_030](crates/bench/sqlite_parity/cases/SQLITE_PARITY_817_WINDOW_PARTITION_SUM_030.rs) | P2 | memory | GEN_SQL_WINDOW | 1640437 | 1523094 | <span style="color:#2563eb">49.23%</span> |
| 680 | [00315 SCALAR_NULL_COALESCE_022](crates/bench/sqlite_parity/cases/SQLITE_PARITY_315_SCALAR_NULL_COALESCE_022.rs) | P1 | memory | GEN_SQL_SCALAR | 1522022 | 1522904 | <span style="color:#2563eb">49.24%</span> |
| 681 | [00328 SCALAR_ARITH_026](crates/bench/sqlite_parity/cases/SQLITE_PARITY_328_SCALAR_ARITH_026.rs) | P1 | memory | GEN_SQL_SCALAR | 1503057 | 1522874 | <span style="color:#2563eb">49.24%</span> |
| 682 | [00019 REPLACE_INTO](crates/bench/sqlite_parity/cases/SQLITE_PARITY_019_REPLACE_INTO.rs) | P0 | memory | SQL_REPLACE | 1607384 | 1522523 | <span style="color:#2563eb">49.25%</span> |
| 683 | [00001 SELECT_CORE_EXPRESSIONS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_001_SELECT_CORE_EXPRESSIONS.rs) | P0 | memory | SQL_SELECT | 1990359 | 1522473 | <span style="color:#2563eb">49.25%</span> |
| 684 | [00344 SCALAR_ARITH_030](crates/bench/sqlite_parity/cases/SQLITE_PARITY_344_SCALAR_ARITH_030.rs) | P1 | memory | GEN_SQL_SCALAR | 1443794 | 1522443 | <span style="color:#2563eb">49.25%</span> |
| 685 | [00934 CONSTRAINT_FK_SAVEPOINT_067](crates/bench/sqlite_parity/cases/SQLITE_PARITY_934_CONSTRAINT_FK_SAVEPOINT_067.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1677156 | 1522363 | <span style="color:#2563eb">49.25%</span> |
| 686 | [00357 SCALAR_CAST_TYPEOF_033](crates/bench/sqlite_parity/cases/SQLITE_PARITY_357_SCALAR_CAST_TYPEOF_033.rs) | P1 | memory | GEN_SQL_SCALAR | 1482988 | 1522243 | <span style="color:#2563eb">49.26%</span> |
| 687 | [01002 VIEW_TRIGGER_GENERATED_055](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1002_VIEW_TRIGGER_GENERATED_055.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1715408 | 1522232 | <span style="color:#2563eb">49.26%</span> |
| 688 | [00229 SCALAR_CAST_TYPEOF_001](crates/bench/sqlite_parity/cases/SQLITE_PARITY_229_SCALAR_CAST_TYPEOF_001.rs) | P1 | memory | GEN_SQL_SCALAR | 1478890 | 1522212 | <span style="color:#2563eb">49.26%</span> |
| 689 | [00946 CONSTRAINT_FK_SAVEPOINT_079](crates/bench/sqlite_parity/cases/SQLITE_PARITY_946_CONSTRAINT_FK_SAVEPOINT_079.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1694720 | 1522123 | <span style="color:#2563eb">49.26%</span> |
| 690 | [00886 CONSTRAINT_FK_SAVEPOINT_019](crates/bench/sqlite_parity/cases/SQLITE_PARITY_886_CONSTRAINT_FK_SAVEPOINT_019.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1676605 | 1521982 | <span style="color:#2563eb">49.27%</span> |
| 691 | [00507 DML_WHERE_ORDER_LIMIT_120](crates/bench/sqlite_parity/cases/SQLITE_PARITY_507_DML_WHERE_ORDER_LIMIT_120.rs) | P1 | memory | GEN_SQL_DML | 1601112 | 1521932 | <span style="color:#2563eb">49.27%</span> |
| 692 | [01072 INDEX_SCHEMA_PRAGMA_005](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1072_INDEX_SCHEMA_PRAGMA_005.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1731459 | 1521782 | <span style="color:#2563eb">49.27%</span> |
| 693 | [00754 CTE_RECURSIVE_MATRIX_047](crates/bench/sqlite_parity/cases/SQLITE_PARITY_754_CTE_RECURSIVE_MATRIX_047.rs) | P1 | memory | GEN_SQL_CTE | 1588027 | 1521731 | <span style="color:#2563eb">49.28%</span> |
| 694 | [00929 CONSTRAINT_FK_SAVEPOINT_062](crates/bench/sqlite_parity/cases/SQLITE_PARITY_929_CONSTRAINT_FK_SAVEPOINT_062.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1662729 | 1521702 | <span style="color:#2563eb">49.28%</span> |
| 695 | [00520 AGG_GROUP_HAVING_013](crates/bench/sqlite_parity/cases/SQLITE_PARITY_520_AGG_GROUP_HAVING_013.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1732480 | 1521581 | <span style="color:#2563eb">49.28%</span> |
| 696 | [00635 JOIN_SUBQUERY_EXISTS_028](crates/bench/sqlite_parity/cases/SQLITE_PARITY_635_JOIN_SUBQUERY_EXISTS_028.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1785030 | 1521521 | <span style="color:#2563eb">49.28%</span> |
| 697 | [00398 DML_WHERE_ORDER_LIMIT_011](crates/bench/sqlite_parity/cases/SQLITE_PARITY_398_DML_WHERE_ORDER_LIMIT_011.rs) | P1 | memory | GEN_SQL_DML | 1593698 | 1521481 | <span style="color:#2563eb">49.28%</span> |
| 698 | [00940 CONSTRAINT_FK_SAVEPOINT_073](crates/bench/sqlite_parity/cases/SQLITE_PARITY_940_CONSTRAINT_FK_SAVEPOINT_073.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1684540 | 1521471 | <span style="color:#2563eb">49.28%</span> |
| 699 | [00550 AGG_GROUP_HAVING_043](crates/bench/sqlite_parity/cases/SQLITE_PARITY_550_AGG_GROUP_HAVING_043.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1698136 | 1521421 | <span style="color:#2563eb">49.29%</span> |
| 700 | [00692 JOIN_SUBQUERY_EXISTS_085](crates/bench/sqlite_parity/cases/SQLITE_PARITY_692_JOIN_SUBQUERY_EXISTS_085.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1691413 | 1521421 | <span style="color:#2563eb">49.29%</span> |
| 701 | [00854 WINDOW_PARTITION_SUM_067](crates/bench/sqlite_parity/cases/SQLITE_PARITY_854_WINDOW_PARTITION_SUM_067.rs) | P2 | memory | GEN_SQL_WINDOW | 1631901 | 1521181 | <span style="color:#2563eb">49.29%</span> |
| 702 | [00549 AGG_GROUP_HAVING_042](crates/bench/sqlite_parity/cases/SQLITE_PARITY_549_AGG_GROUP_HAVING_042.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1977515 | 1521161 | <span style="color:#2563eb">49.29%</span> |
| 703 | [00865 WINDOW_PARTITION_SUM_078](crates/bench/sqlite_parity/cases/SQLITE_PARITY_865_WINDOW_PARTITION_SUM_078.rs) | P2 | memory | GEN_SQL_WINDOW | 1702985 | 1521140 | <span style="color:#2563eb">49.30%</span> |
| 704 | [00410 DML_WHERE_ORDER_LIMIT_023](crates/bench/sqlite_parity/cases/SQLITE_PARITY_410_DML_WHERE_ORDER_LIMIT_023.rs) | P1 | memory | GEN_SQL_DML | 1602434 | 1521130 | <span style="color:#2563eb">49.30%</span> |
| 705 | [00389 DML_WHERE_ORDER_LIMIT_002](crates/bench/sqlite_parity/cases/SQLITE_PARITY_389_DML_WHERE_ORDER_LIMIT_002.rs) | P1 | memory | GEN_SQL_DML | 1594920 | 1521090 | <span style="color:#2563eb">49.30%</span> |
| 706 | [00465 DML_WHERE_ORDER_LIMIT_078](crates/bench/sqlite_parity/cases/SQLITE_PARITY_465_DML_WHERE_ORDER_LIMIT_078.rs) | P1 | memory | GEN_SQL_DML | 1774120 | 1521081 | <span style="color:#2563eb">49.30%</span> |
| 707 | [00684 JOIN_SUBQUERY_EXISTS_077](crates/bench/sqlite_parity/cases/SQLITE_PARITY_684_JOIN_SUBQUERY_EXISTS_077.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1682025 | 1520760 | <span style="color:#2563eb">49.31%</span> |
| 708 | [00523 AGG_GROUP_HAVING_016](crates/bench/sqlite_parity/cases/SQLITE_PARITY_523_AGG_GROUP_HAVING_016.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1893866 | 1520650 | <span style="color:#2563eb">49.31%</span> |
| 709 | [01050 JSON_EXTRACT_SET_043](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1050_JSON_EXTRACT_SET_043.rs) | P2 | memory | GEN_SQL_JSON | 1572407 | 1520640 | <span style="color:#2563eb">49.31%</span> |
| 710 | [00614 JOIN_SUBQUERY_EXISTS_007](crates/bench/sqlite_parity/cases/SQLITE_PARITY_614_JOIN_SUBQUERY_EXISTS_007.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1707774 | 1520599 | <span style="color:#2563eb">49.31%</span> |
| 711 | [01073 INDEX_SCHEMA_PRAGMA_006](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1073_INDEX_SCHEMA_PRAGMA_006.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1675984 | 1520550 | <span style="color:#2563eb">49.31%</span> |
| 712 | [00485 DML_WHERE_ORDER_LIMIT_098](crates/bench/sqlite_parity/cases/SQLITE_PARITY_485_DML_WHERE_ORDER_LIMIT_098.rs) | P1 | memory | GEN_SQL_DML | 1625388 | 1520450 | <span style="color:#2563eb">49.32%</span> |
| 713 | [01044 JSON_EXTRACT_SET_037](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1044_JSON_EXTRACT_SET_037.rs) | P2 | memory | GEN_SQL_JSON | 1577848 | 1520319 | <span style="color:#2563eb">49.32%</span> |
| 714 | [00259 SCALAR_NULL_COALESCE_008](crates/bench/sqlite_parity/cases/SQLITE_PARITY_259_SCALAR_NULL_COALESCE_008.rs) | P1 | memory | GEN_SQL_SCALAR | 1476927 | 1520269 | <span style="color:#2563eb">49.32%</span> |
| 715 | [00530 AGG_GROUP_HAVING_023](crates/bench/sqlite_parity/cases/SQLITE_PARITY_530_AGG_GROUP_HAVING_023.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1668831 | 1520069 | <span style="color:#2563eb">49.33%</span> |
| 716 | [00588 AGG_GROUP_HAVING_081](crates/bench/sqlite_parity/cases/SQLITE_PARITY_588_AGG_GROUP_HAVING_081.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2023903 | 1520038 | <span style="color:#2563eb">49.33%</span> |
| 717 | [00791 WINDOW_PARTITION_SUM_004](crates/bench/sqlite_parity/cases/SQLITE_PARITY_791_WINDOW_PARTITION_SUM_004.rs) | P2 | memory | GEN_SQL_WINDOW | 1609117 | 1519648 | <span style="color:#2563eb">49.35%</span> |
| 718 | [00587 AGG_GROUP_HAVING_080](crates/bench/sqlite_parity/cases/SQLITE_PARITY_587_AGG_GROUP_HAVING_080.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1729786 | 1519437 | <span style="color:#2563eb">49.35%</span> |
| 719 | [00503 DML_WHERE_ORDER_LIMIT_116](crates/bench/sqlite_parity/cases/SQLITE_PARITY_503_DML_WHERE_ORDER_LIMIT_116.rs) | P1 | memory | GEN_SQL_DML | 1628444 | 1519297 | <span style="color:#2563eb">49.36%</span> |
| 720 | [00968 VIEW_TRIGGER_GENERATED_021](crates/bench/sqlite_parity/cases/SQLITE_PARITY_968_VIEW_TRIGGER_GENERATED_021.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2335453 | 1519267 | <span style="color:#2563eb">49.36%</span> |
| 721 | [01063 JSON_EXTRACT_SET_056](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1063_JSON_EXTRACT_SET_056.rs) | P2 | memory | GEN_SQL_JSON | 1584521 | 1519197 | <span style="color:#2563eb">49.36%</span> |
| 722 | [00904 CONSTRAINT_FK_SAVEPOINT_037](crates/bench/sqlite_parity/cases/SQLITE_PARITY_904_CONSTRAINT_FK_SAVEPOINT_037.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1667308 | 1519147 | <span style="color:#2563eb">49.36%</span> |
| 723 | [00247 SCALAR_NULL_COALESCE_005](crates/bench/sqlite_parity/cases/SQLITE_PARITY_247_SCALAR_NULL_COALESCE_005.rs) | P1 | memory | GEN_SQL_SCALAR | 1459303 | 1519127 | <span style="color:#2563eb">49.36%</span> |
| 724 | [00253 SCALAR_CAST_TYPEOF_007](crates/bench/sqlite_parity/cases/SQLITE_PARITY_253_SCALAR_CAST_TYPEOF_007.rs) | P1 | memory | GEN_SQL_SCALAR | 1497696 | 1519096 | <span style="color:#2563eb">49.36%</span> |
| 725 | [00342 SCALAR_STRING_029](crates/bench/sqlite_parity/cases/SQLITE_PARITY_342_SCALAR_STRING_029.rs) | P1 | memory | GEN_SQL_SCALAR | 1465245 | 1518947 | <span style="color:#2563eb">49.37%</span> |
| 726 | [00479 DML_WHERE_ORDER_LIMIT_092](crates/bench/sqlite_parity/cases/SQLITE_PARITY_479_DML_WHERE_ORDER_LIMIT_092.rs) | P1 | memory | GEN_SQL_DML | 1625829 | 1518936 | <span style="color:#2563eb">49.37%</span> |
| 727 | [00085 CORE_RANDOM_SHAPE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_085_CORE_RANDOM_SHAPE.rs) | P0 | memory | SQL_FUNCTIONS | 1477849 | 1518926 | <span style="color:#2563eb">49.37%</span> |
| 728 | [00304 SCALAR_ARITH_020](crates/bench/sqlite_parity/cases/SQLITE_PARITY_304_SCALAR_ARITH_020.rs) | P1 | memory | GEN_SQL_SCALAR | 1549504 | 1518916 | <span style="color:#2563eb">49.37%</span> |
| 729 | [00675 JOIN_SUBQUERY_EXISTS_068](crates/bench/sqlite_parity/cases/SQLITE_PARITY_675_JOIN_SUBQUERY_EXISTS_068.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1718094 | 1518907 | <span style="color:#2563eb">49.37%</span> |
| 730 | [00319 SCALAR_NULL_COALESCE_023](crates/bench/sqlite_parity/cases/SQLITE_PARITY_319_SCALAR_NULL_COALESCE_023.rs) | P1 | memory | GEN_SQL_SCALAR | 1483129 | 1518876 | <span style="color:#2563eb">49.37%</span> |
| 731 | [00252 SCALAR_ARITH_007](crates/bench/sqlite_parity/cases/SQLITE_PARITY_252_SCALAR_ARITH_007.rs) | P1 | memory | GEN_SQL_SCALAR | 1472008 | 1518856 | <span style="color:#2563eb">49.37%</span> |
| 732 | [00829 WINDOW_PARTITION_SUM_042](crates/bench/sqlite_parity/cases/SQLITE_PARITY_829_WINDOW_PARTITION_SUM_042.rs) | P2 | memory | GEN_SQL_WINDOW | 1622122 | 1518846 | <span style="color:#2563eb">49.37%</span> |
| 733 | [00937 CONSTRAINT_FK_SAVEPOINT_070](crates/bench/sqlite_parity/cases/SQLITE_PARITY_937_CONSTRAINT_FK_SAVEPOINT_070.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1684971 | 1518796 | <span style="color:#2563eb">49.37%</span> |
| 734 | [00336 SCALAR_ARITH_028](crates/bench/sqlite_parity/cases/SQLITE_PARITY_336_SCALAR_ARITH_028.rs) | P1 | memory | GEN_SQL_SCALAR | 1845485 | 1518756 | <span style="color:#2563eb">49.37%</span> |
| 735 | [00808 WINDOW_PARTITION_SUM_021](crates/bench/sqlite_parity/cases/SQLITE_PARITY_808_WINDOW_PARTITION_SUM_021.rs) | P2 | memory | GEN_SQL_WINDOW | 1704618 | 1518746 | <span style="color:#2563eb">49.38%</span> |
| 736 | [00726 CTE_RECURSIVE_MATRIX_019](crates/bench/sqlite_parity/cases/SQLITE_PARITY_726_CTE_RECURSIVE_MATRIX_019.rs) | P1 | memory | GEN_SQL_CTE | 1577908 | 1518636 | <span style="color:#2563eb">49.38%</span> |
| 737 | [00504 DML_WHERE_ORDER_LIMIT_117](crates/bench/sqlite_parity/cases/SQLITE_PARITY_504_DML_WHERE_ORDER_LIMIT_117.rs) | P1 | memory | GEN_SQL_DML | 1621210 | 1518556 | <span style="color:#2563eb">49.38%</span> |
| 738 | [01029 JSON_EXTRACT_SET_022](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1029_JSON_EXTRACT_SET_022.rs) | P2 | memory | GEN_SQL_JSON | 1630628 | 1518516 | <span style="color:#2563eb">49.38%</span> |
| 739 | [01046 JSON_EXTRACT_SET_039](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1046_JSON_EXTRACT_SET_039.rs) | P2 | memory | GEN_SQL_JSON | 1539085 | 1518506 | <span style="color:#2563eb">49.38%</span> |
| 740 | [00741 CTE_RECURSIVE_MATRIX_034](crates/bench/sqlite_parity/cases/SQLITE_PARITY_741_CTE_RECURSIVE_MATRIX_034.rs) | P1 | memory | GEN_SQL_CTE | 1591093 | 1518285 | <span style="color:#2563eb">49.39%</span> |
| 741 | [01000 VIEW_TRIGGER_GENERATED_053](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1000_VIEW_TRIGGER_GENERATED_053.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1723344 | 1518235 | <span style="color:#2563eb">49.39%</span> |
| 742 | [00983 VIEW_TRIGGER_GENERATED_036](crates/bench/sqlite_parity/cases/SQLITE_PARITY_983_VIEW_TRIGGER_GENERATED_036.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1713875 | 1518025 | <span style="color:#2563eb">49.40%</span> |
| 743 | [00024 SAVEPOINT_ROLLBACK_RELEASE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_024_SAVEPOINT_ROLLBACK_RELEASE.rs) | P0 | memory | SQL_SAVEPOINT | 1542421 | 1517954 | <span style="color:#2563eb">49.40%</span> |
| 744 | [00683 JOIN_SUBQUERY_EXISTS_076](crates/bench/sqlite_parity/cases/SQLITE_PARITY_683_JOIN_SUBQUERY_EXISTS_076.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1711170 | 1517925 | <span style="color:#2563eb">49.40%</span> |
| 745 | [00640 JOIN_SUBQUERY_EXISTS_033](crates/bench/sqlite_parity/cases/SQLITE_PARITY_640_JOIN_SUBQUERY_EXISTS_033.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1698296 | 1517914 | <span style="color:#2563eb">49.40%</span> |
| 746 | [00002 LITERALS_AND_TYPEOF](crates/bench/sqlite_parity/cases/SQLITE_PARITY_002_LITERALS_AND_TYPEOF.rs) | P0 | memory | SQL_EXPRESSIONS | 2001060 | 1517855 | <span style="color:#2563eb">49.40%</span> |
| 747 | [00025 BEGIN_MODES](crates/bench/sqlite_parity/cases/SQLITE_PARITY_025_BEGIN_MODES.rs) | P0 | memory | SQL_TRANSACTION | 1476146 | 1517734 | <span style="color:#2563eb">49.41%</span> |
| 748 | [00669 JOIN_SUBQUERY_EXISTS_062](crates/bench/sqlite_parity/cases/SQLITE_PARITY_669_JOIN_SUBQUERY_EXISTS_062.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1711130 | 1517734 | <span style="color:#2563eb">49.41%</span> |
| 749 | [00020 UPSERT_DO_UPDATE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_020_UPSERT_DO_UPDATE.rs) | P0 | memory | SQL_UPSERT | 1626881 | 1517574 | <span style="color:#2563eb">49.41%</span> |
| 750 | [00483 DML_WHERE_ORDER_LIMIT_096](crates/bench/sqlite_parity/cases/SQLITE_PARITY_483_DML_WHERE_ORDER_LIMIT_096.rs) | P1 | memory | GEN_SQL_DML | 1636660 | 1517564 | <span style="color:#2563eb">49.41%</span> |
| 751 | [00686 JOIN_SUBQUERY_EXISTS_079](crates/bench/sqlite_parity/cases/SQLITE_PARITY_686_JOIN_SUBQUERY_EXISTS_079.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1704338 | 1517423 | <span style="color:#2563eb">49.42%</span> |
| 752 | [01010 JSON_EXTRACT_SET_003](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1010_JSON_EXTRACT_SET_003.rs) | P2 | memory | GEN_SQL_JSON | 1575814 | 1517394 | <span style="color:#2563eb">49.42%</span> |
| 753 | [00501 DML_WHERE_ORDER_LIMIT_114](crates/bench/sqlite_parity/cases/SQLITE_PARITY_501_DML_WHERE_ORDER_LIMIT_114.rs) | P1 | memory | GEN_SQL_DML | 1617613 | 1517374 | <span style="color:#2563eb">49.42%</span> |
| 754 | [00281 SCALAR_CAST_TYPEOF_014](crates/bench/sqlite_parity/cases/SQLITE_PARITY_281_SCALAR_CAST_TYPEOF_014.rs) | P1 | memory | GEN_SQL_SCALAR | 1485624 | 1517363 | <span style="color:#2563eb">49.42%</span> |
| 755 | [01021 JSON_EXTRACT_SET_014](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1021_JSON_EXTRACT_SET_014.rs) | P2 | memory | GEN_SQL_JSON | 1552209 | 1517333 | <span style="color:#2563eb">49.42%</span> |
| 756 | [00533 AGG_GROUP_HAVING_026](crates/bench/sqlite_parity/cases/SQLITE_PARITY_533_AGG_GROUP_HAVING_026.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1662258 | 1517323 | <span style="color:#2563eb">49.42%</span> |
| 757 | [00847 WINDOW_PARTITION_SUM_060](crates/bench/sqlite_parity/cases/SQLITE_PARITY_847_WINDOW_PARTITION_SUM_060.rs) | P2 | memory | GEN_SQL_WINDOW | 1983436 | 1517284 | <span style="color:#2563eb">49.42%</span> |
| 758 | [00787 CTE_RECURSIVE_MATRIX_080](crates/bench/sqlite_parity/cases/SQLITE_PARITY_787_CTE_RECURSIVE_MATRIX_080.rs) | P1 | memory | GEN_SQL_CTE | 1579491 | 1517213 | <span style="color:#2563eb">49.43%</span> |
| 759 | [00897 CONSTRAINT_FK_SAVEPOINT_030](crates/bench/sqlite_parity/cases/SQLITE_PARITY_897_CONSTRAINT_FK_SAVEPOINT_030.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1691493 | 1517174 | <span style="color:#2563eb">49.43%</span> |
| 760 | [00774 CTE_RECURSIVE_MATRIX_067](crates/bench/sqlite_parity/cases/SQLITE_PARITY_774_CTE_RECURSIVE_MATRIX_067.rs) | P1 | memory | GEN_SQL_CTE | 1559654 | 1517173 | <span style="color:#2563eb">49.43%</span> |
| 761 | [00770 CTE_RECURSIVE_MATRIX_063](crates/bench/sqlite_parity/cases/SQLITE_PARITY_770_CTE_RECURSIVE_MATRIX_063.rs) | P1 | memory | GEN_SQL_CTE | 1527964 | 1517163 | <span style="color:#2563eb">49.43%</span> |
| 762 | [00293 SCALAR_CAST_TYPEOF_017](crates/bench/sqlite_parity/cases/SQLITE_PARITY_293_SCALAR_CAST_TYPEOF_017.rs) | P1 | memory | GEN_SQL_SCALAR | 1459965 | 1516852 | <span style="color:#2563eb">49.44%</span> |
| 763 | [00264 SCALAR_ARITH_010](crates/bench/sqlite_parity/cases/SQLITE_PARITY_264_SCALAR_ARITH_010.rs) | P1 | memory | GEN_SQL_SCALAR | 1459043 | 1516842 | <span style="color:#2563eb">49.44%</span> |
| 764 | [00268 SCALAR_ARITH_011](crates/bench/sqlite_parity/cases/SQLITE_PARITY_268_SCALAR_ARITH_011.rs) | P1 | memory | GEN_SQL_SCALAR | 1457109 | 1516652 | <span style="color:#2563eb">49.44%</span> |
| 765 | [00508 AGG_GROUP_HAVING_001](crates/bench/sqlite_parity/cases/SQLITE_PARITY_508_AGG_GROUP_HAVING_001.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1686144 | 1516633 | <span style="color:#2563eb">49.45%</span> |
| 766 | [00478 DML_WHERE_ORDER_LIMIT_091](crates/bench/sqlite_parity/cases/SQLITE_PARITY_478_DML_WHERE_ORDER_LIMIT_091.rs) | P1 | memory | GEN_SQL_DML | 1623505 | 1516622 | <span style="color:#2563eb">49.45%</span> |
| 767 | [00685 JOIN_SUBQUERY_EXISTS_078](crates/bench/sqlite_parity/cases/SQLITE_PARITY_685_JOIN_SUBQUERY_EXISTS_078.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1717322 | 1516612 | <span style="color:#2563eb">49.45%</span> |
| 768 | [00543 AGG_GROUP_HAVING_036](crates/bench/sqlite_parity/cases/SQLITE_PARITY_543_AGG_GROUP_HAVING_036.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1716952 | 1516552 | <span style="color:#2563eb">49.45%</span> |
| 769 | [00748 CTE_RECURSIVE_MATRIX_041](crates/bench/sqlite_parity/cases/SQLITE_PARITY_748_CTE_RECURSIVE_MATRIX_041.rs) | P1 | memory | GEN_SQL_CTE | 1582086 | 1516412 | <span style="color:#2563eb">49.45%</span> |
| 770 | [00803 WINDOW_PARTITION_SUM_016](crates/bench/sqlite_parity/cases/SQLITE_PARITY_803_WINDOW_PARTITION_SUM_016.rs) | P2 | memory | GEN_SQL_WINDOW | 1660895 | 1516382 | <span style="color:#2563eb">49.45%</span> |
| 771 | [00703 JOIN_SUBQUERY_EXISTS_096](crates/bench/sqlite_parity/cases/SQLITE_PARITY_703_JOIN_SUBQUERY_EXISTS_096.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1703056 | 1516291 | <span style="color:#2563eb">49.46%</span> |
| 772 | [01095 INDEX_SCHEMA_PRAGMA_028](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1095_INDEX_SCHEMA_PRAGMA_028.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1772146 | 1516231 | <span style="color:#2563eb">49.46%</span> |
| 773 | [01064 JSON_EXTRACT_SET_057](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1064_JSON_EXTRACT_SET_057.rs) | P2 | memory | GEN_SQL_JSON | 1607965 | 1516111 | <span style="color:#2563eb">49.46%</span> |
| 774 | [00558 AGG_GROUP_HAVING_051](crates/bench/sqlite_parity/cases/SQLITE_PARITY_558_AGG_GROUP_HAVING_051.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1679741 | 1516021 | <span style="color:#2563eb">49.47%</span> |
| 775 | [00288 SCALAR_ARITH_016](crates/bench/sqlite_parity/cases/SQLITE_PARITY_288_SCALAR_ARITH_016.rs) | P1 | memory | GEN_SQL_SCALAR | 1466016 | 1515921 | <span style="color:#2563eb">49.47%</span> |
| 776 | [00660 JOIN_SUBQUERY_EXISTS_053](crates/bench/sqlite_parity/cases/SQLITE_PARITY_660_JOIN_SUBQUERY_EXISTS_053.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1830867 | 1515750 | <span style="color:#2563eb">49.48%</span> |
| 777 | [01043 JSON_EXTRACT_SET_036](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1043_JSON_EXTRACT_SET_036.rs) | P2 | memory | GEN_SQL_JSON | 1584220 | 1515701 | <span style="color:#2563eb">49.48%</span> |
| 778 | [00574 AGG_GROUP_HAVING_067](crates/bench/sqlite_parity/cases/SQLITE_PARITY_574_AGG_GROUP_HAVING_067.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1680773 | 1515600 | <span style="color:#2563eb">49.48%</span> |
| 779 | [00292 SCALAR_ARITH_017](crates/bench/sqlite_parity/cases/SQLITE_PARITY_292_SCALAR_ARITH_017.rs) | P1 | memory | GEN_SQL_SCALAR | 1798856 | 1515380 | <span style="color:#2563eb">49.49%</span> |
| 780 | [00101 SCHEMA_SQLITE_MASTER_ALIAS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_101_SCHEMA_SQLITE_MASTER_ALIAS.rs) | P0 | memory | SQL_SCHEMA | 1504720 | 1515300 | <span style="color:#2563eb">49.49%</span> |
| 781 | [00874 CONSTRAINT_FK_SAVEPOINT_007](crates/bench/sqlite_parity/cases/SQLITE_PARITY_874_CONSTRAINT_FK_SAVEPOINT_007.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1705039 | 1515299 | <span style="color:#2563eb">49.49%</span> |
| 782 | [00510 AGG_GROUP_HAVING_003](crates/bench/sqlite_parity/cases/SQLITE_PARITY_510_AGG_GROUP_HAVING_003.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1634054 | 1515189 | <span style="color:#2563eb">49.49%</span> |
| 783 | [00103 WINDOW_NAMED_WINDOW](crates/bench/sqlite_parity/cases/SQLITE_PARITY_103_WINDOW_NAMED_WINDOW.rs) | P0 | memory | SQL_WINDOW | 1567168 | 1515089 | <span style="color:#2563eb">49.50%</span> |
| 784 | [00100 SCHEMA_SQLITE_SCHEMA](crates/bench/sqlite_parity/cases/SQLITE_PARITY_100_SCHEMA_SQLITE_SCHEMA.rs) | P0 | memory | SQL_SCHEMA | 1729926 | 1515049 | <span style="color:#2563eb">49.50%</span> |
| 785 | [00525 AGG_GROUP_HAVING_018](crates/bench/sqlite_parity/cases/SQLITE_PARITY_525_AGG_GROUP_HAVING_018.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2091521 | 1514989 | <span style="color:#2563eb">49.50%</span> |
| 786 | [00670 JOIN_SUBQUERY_EXISTS_063](crates/bench/sqlite_parity/cases/SQLITE_PARITY_670_JOIN_SUBQUERY_EXISTS_063.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1732972 | 1514908 | <span style="color:#2563eb">49.50%</span> |
| 787 | [01070 INDEX_SCHEMA_PRAGMA_003](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1070_INDEX_SCHEMA_PRAGMA_003.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1686594 | 1514398 | <span style="color:#2563eb">49.52%</span> |
| 788 | [01015 JSON_EXTRACT_SET_008](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1015_JSON_EXTRACT_SET_008.rs) | P2 | memory | GEN_SQL_JSON | 1819245 | 1514347 | <span style="color:#2563eb">49.52%</span> |
| 789 | [00908 CONSTRAINT_FK_SAVEPOINT_041](crates/bench/sqlite_parity/cases/SQLITE_PARITY_908_CONSTRAINT_FK_SAVEPOINT_041.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1684190 | 1514277 | <span style="color:#2563eb">49.52%</span> |
| 790 | [00218 PRAGMA_FORMS_SCHEMA_EQUALS_PARENS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_218_PRAGMA_FORMS_SCHEMA_EQUALS_PARENS.rs) | P0 | memory | SQL_PRAGMA | 1475264 | 1514047 | <span style="color:#2563eb">49.53%</span> |
| 791 | [00056 SUBQUERIES_EXISTS_IN](crates/bench/sqlite_parity/cases/SQLITE_PARITY_056_SUBQUERIES_EXISTS_IN.rs) | P0 | memory | SQL_SELECT | 1607815 | 1513947 | <span style="color:#2563eb">49.54%</span> |
| 792 | [01036 JSON_EXTRACT_SET_029](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1036_JSON_EXTRACT_SET_029.rs) | P2 | memory | GEN_SQL_JSON | 1558631 | 1513877 | <span style="color:#2563eb">49.54%</span> |
| 793 | [00129 DOT_CONNECTION](crates/bench/sqlite_parity/cases/SQLITE_PARITY_129_DOT_CONNECTION.rs) | P0 | memory | CLI_DOT_COMMAND | 1578059 | 1513676 | <span style="color:#2563eb">49.54%</span> |
| 794 | [00747 CTE_RECURSIVE_MATRIX_040](crates/bench/sqlite_parity/cases/SQLITE_PARITY_747_CTE_RECURSIVE_MATRIX_040.rs) | P1 | memory | GEN_SQL_CTE | 1589450 | 1513657 | <span style="color:#2563eb">49.54%</span> |
| 795 | [00296 SCALAR_ARITH_018](crates/bench/sqlite_parity/cases/SQLITE_PARITY_296_SCALAR_ARITH_018.rs) | P1 | memory | GEN_SQL_SCALAR | 1421953 | 1513646 | <span style="color:#2563eb">49.55%</span> |
| 796 | [00838 WINDOW_PARTITION_SUM_051](crates/bench/sqlite_parity/cases/SQLITE_PARITY_838_WINDOW_PARTITION_SUM_051.rs) | P2 | memory | GEN_SQL_WINDOW | 1639715 | 1513536 | <span style="color:#2563eb">49.55%</span> |
| 797 | [00938 CONSTRAINT_FK_SAVEPOINT_071](crates/bench/sqlite_parity/cases/SQLITE_PARITY_938_CONSTRAINT_FK_SAVEPOINT_071.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1865252 | 1513506 | <span style="color:#2563eb">49.55%</span> |
| 798 | [00397 DML_WHERE_ORDER_LIMIT_010](crates/bench/sqlite_parity/cases/SQLITE_PARITY_397_DML_WHERE_ORDER_LIMIT_010.rs) | P1 | memory | GEN_SQL_DML | 1627762 | 1513336 | <span style="color:#2563eb">49.56%</span> |
| 799 | [01100 INDEX_SCHEMA_PRAGMA_033](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1100_INDEX_SCHEMA_PRAGMA_033.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 2329962 | 1513295 | <span style="color:#2563eb">49.56%</span> |
| 800 | [00852 WINDOW_PARTITION_SUM_065](crates/bench/sqlite_parity/cases/SQLITE_PARITY_852_WINDOW_PARTITION_SUM_065.rs) | P2 | memory | GEN_SQL_WINDOW | 1627492 | 1513276 | <span style="color:#2563eb">49.56%</span> |
| 801 | [00287 SCALAR_NULL_COALESCE_015](crates/bench/sqlite_parity/cases/SQLITE_PARITY_287_SCALAR_NULL_COALESCE_015.rs) | P1 | memory | GEN_SQL_SCALAR | 1487306 | 1513225 | <span style="color:#2563eb">49.56%</span> |
| 802 | [00364 SCALAR_ARITH_035](crates/bench/sqlite_parity/cases/SQLITE_PARITY_364_SCALAR_ARITH_035.rs) | P1 | memory | GEN_SQL_SCALAR | 1507776 | 1513195 | <span style="color:#2563eb">49.56%</span> |
| 803 | [00798 WINDOW_PARTITION_SUM_011](crates/bench/sqlite_parity/cases/SQLITE_PARITY_798_WINDOW_PARTITION_SUM_011.rs) | P2 | memory | GEN_SQL_WINDOW | 1812662 | 1513105 | <span style="color:#2563eb">49.56%</span> |
| 804 | [00820 WINDOW_PARTITION_SUM_033](crates/bench/sqlite_parity/cases/SQLITE_PARITY_820_WINDOW_PARTITION_SUM_033.rs) | P2 | memory | GEN_SQL_WINDOW | 1619497 | 1513035 | <span style="color:#2563eb">49.57%</span> |
| 805 | [00435 DML_WHERE_ORDER_LIMIT_048](crates/bench/sqlite_parity/cases/SQLITE_PARITY_435_DML_WHERE_ORDER_LIMIT_048.rs) | P1 | memory | GEN_SQL_DML | 1606823 | 1512945 | <span style="color:#2563eb">49.57%</span> |
| 806 | [00777 CTE_RECURSIVE_MATRIX_070](crates/bench/sqlite_parity/cases/SQLITE_PARITY_777_CTE_RECURSIVE_MATRIX_070.rs) | P1 | memory | GEN_SQL_CTE | 1494350 | 1512825 | <span style="color:#2563eb">49.57%</span> |
| 807 | [01081 INDEX_SCHEMA_PRAGMA_014](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1081_INDEX_SCHEMA_PRAGMA_014.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1674912 | 1512625 | <span style="color:#2563eb">49.58%</span> |
| 808 | [00696 JOIN_SUBQUERY_EXISTS_089](crates/bench/sqlite_parity/cases/SQLITE_PARITY_696_JOIN_SUBQUERY_EXISTS_089.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1725959 | 1512494 | <span style="color:#2563eb">49.58%</span> |
| 809 | [00232 SCALAR_ARITH_002](crates/bench/sqlite_parity/cases/SQLITE_PARITY_232_SCALAR_ARITH_002.rs) | P1 | memory | GEN_SQL_SCALAR | 1548082 | 1512374 | <span style="color:#2563eb">49.59%</span> |
| 810 | [00220 DELETE_LIMIT_OPTIONAL](crates/bench/sqlite_parity/cases/SQLITE_PARITY_220_DELETE_LIMIT_OPTIONAL.rs) | P3 | memory | SQL_DELETE_OPTIONAL | 1528545 | 1512354 | <span style="color:#2563eb">49.59%</span> |
| 811 | [00265 SCALAR_CAST_TYPEOF_010](crates/bench/sqlite_parity/cases/SQLITE_PARITY_265_SCALAR_CAST_TYPEOF_010.rs) | P1 | memory | GEN_SQL_SCALAR | 1456277 | 1512274 | <span style="color:#2563eb">49.59%</span> |
| 812 | [00641 JOIN_SUBQUERY_EXISTS_034](crates/bench/sqlite_parity/cases/SQLITE_PARITY_641_JOIN_SUBQUERY_EXISTS_034.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1739384 | 1512224 | <span style="color:#2563eb">49.59%</span> |
| 813 | [00372 SCALAR_ARITH_037](crates/bench/sqlite_parity/cases/SQLITE_PARITY_372_SCALAR_ARITH_037.rs) | P1 | memory | GEN_SQL_SCALAR | 1613165 | 1512173 | <span style="color:#2563eb">49.59%</span> |
| 814 | [00511 AGG_GROUP_HAVING_004](crates/bench/sqlite_parity/cases/SQLITE_PARITY_511_AGG_GROUP_HAVING_004.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1663710 | 1512153 | <span style="color:#2563eb">49.59%</span> |
| 815 | [00022 TRANSACTION_COMMIT](crates/bench/sqlite_parity/cases/SQLITE_PARITY_022_TRANSACTION_COMMIT.rs) | P0 | memory | SQL_TRANSACTION | 1519247 | 1511974 | <span style="color:#2563eb">49.60%</span> |
| 816 | [00048 PRAGMA_USER_VERSION](crates/bench/sqlite_parity/cases/SQLITE_PARITY_048_PRAGMA_USER_VERSION.rs) | P0 | memory | SQL_PRAGMA | 1827741 | 1511924 | <span style="color:#2563eb">49.60%</span> |
| 817 | [00896 CONSTRAINT_FK_SAVEPOINT_029](crates/bench/sqlite_parity/cases/SQLITE_PARITY_896_CONSTRAINT_FK_SAVEPOINT_029.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1670774 | 1511793 | <span style="color:#2563eb">49.61%</span> |
| 818 | [00096 DBSTAT_OPTIONAL](crates/bench/sqlite_parity/cases/SQLITE_PARITY_096_DBSTAT_OPTIONAL.rs) | P3 | memory | SQL_VIRTUAL_TABLE_OPTIONAL | 1646248 | 1511782 | <span style="color:#2563eb">49.61%</span> |
| 819 | [00830 WINDOW_PARTITION_SUM_043](crates/bench/sqlite_parity/cases/SQLITE_PARITY_830_WINDOW_PARTITION_SUM_043.rs) | P2 | memory | GEN_SQL_WINDOW | 1609077 | 1511713 | <span style="color:#2563eb">49.61%</span> |
| 820 | [00242 SCALAR_STRING_004](crates/bench/sqlite_parity/cases/SQLITE_PARITY_242_SCALAR_STRING_004.rs) | P1 | memory | GEN_SQL_SCALAR | 1472779 | 1511382 | <span style="color:#2563eb">49.62%</span> |
| 821 | [00251 SCALAR_NULL_COALESCE_006](crates/bench/sqlite_parity/cases/SQLITE_PARITY_251_SCALAR_NULL_COALESCE_006.rs) | P1 | memory | GEN_SQL_SCALAR | 1479903 | 1511382 | <span style="color:#2563eb">49.62%</span> |
| 822 | [01006 VIEW_TRIGGER_GENERATED_059](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1006_VIEW_TRIGGER_GENERATED_059.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1867807 | 1511362 | <span style="color:#2563eb">49.62%</span> |
| 823 | [00374 SCALAR_STRING_037](crates/bench/sqlite_parity/cases/SQLITE_PARITY_374_SCALAR_STRING_037.rs) | P1 | memory | GEN_SQL_SCALAR | 1470103 | 1511352 | <span style="color:#2563eb">49.62%</span> |
| 824 | [00824 WINDOW_PARTITION_SUM_037](crates/bench/sqlite_parity/cases/SQLITE_PARITY_824_WINDOW_PARTITION_SUM_037.rs) | P2 | memory | GEN_SQL_WINDOW | 1633453 | 1511192 | <span style="color:#2563eb">49.63%</span> |
| 825 | [01058 JSON_EXTRACT_SET_051](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1058_JSON_EXTRACT_SET_051.rs) | P2 | memory | GEN_SQL_JSON | 1595953 | 1511132 | <span style="color:#2563eb">49.63%</span> |
| 826 | [00246 SCALAR_STRING_005](crates/bench/sqlite_parity/cases/SQLITE_PARITY_246_SCALAR_STRING_005.rs) | P1 | memory | GEN_SQL_SCALAR | 1468190 | 1510931 | <span style="color:#2563eb">49.64%</span> |
| 827 | [00339 SCALAR_NULL_COALESCE_028](crates/bench/sqlite_parity/cases/SQLITE_PARITY_339_SCALAR_NULL_COALESCE_028.rs) | P1 | memory | GEN_SQL_SCALAR | 1615349 | 1510902 | <span style="color:#2563eb">49.64%</span> |
| 828 | [01096 INDEX_SCHEMA_PRAGMA_029](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1096_INDEX_SCHEMA_PRAGMA_029.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1685291 | 1510901 | <span style="color:#2563eb">49.64%</span> |
| 829 | [00396 DML_WHERE_ORDER_LIMIT_009](crates/bench/sqlite_parity/cases/SQLITE_PARITY_396_DML_WHERE_ORDER_LIMIT_009.rs) | P1 | memory | GEN_SQL_DML | 1623795 | 1510881 | <span style="color:#2563eb">49.64%</span> |
| 830 | [00712 CTE_RECURSIVE_MATRIX_005](crates/bench/sqlite_parity/cases/SQLITE_PARITY_712_CTE_RECURSIVE_MATRIX_005.rs) | P1 | memory | GEN_SQL_CTE | 1573250 | 1510811 | <span style="color:#2563eb">49.64%</span> |
| 831 | [00042 TEMP_TABLE_TEMP_SCHEMA](crates/bench/sqlite_parity/cases/SQLITE_PARITY_042_TEMP_TABLE_TEMP_SCHEMA.rs) | P0 | memory | SQL_TEMP | 1547591 | 1510761 | <span style="color:#2563eb">49.64%</span> |
| 832 | [00974 VIEW_TRIGGER_GENERATED_027](crates/bench/sqlite_parity/cases/SQLITE_PARITY_974_VIEW_TRIGGER_GENERATED_027.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1887254 | 1510701 | <span style="color:#2563eb">49.64%</span> |
| 833 | [00476 DML_WHERE_ORDER_LIMIT_089](crates/bench/sqlite_parity/cases/SQLITE_PARITY_476_DML_WHERE_ORDER_LIMIT_089.rs) | P1 | memory | GEN_SQL_DML | 1617884 | 1510671 | <span style="color:#2563eb">49.64%</span> |
| 834 | [00612 JOIN_SUBQUERY_EXISTS_005](crates/bench/sqlite_parity/cases/SQLITE_PARITY_612_JOIN_SUBQUERY_EXISTS_005.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1733893 | 1510631 | <span style="color:#2563eb">49.65%</span> |
| 835 | [01004 VIEW_TRIGGER_GENERATED_057](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1004_VIEW_TRIGGER_GENERATED_057.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1815818 | 1510610 | <span style="color:#2563eb">49.65%</span> |
| 836 | [00902 CONSTRAINT_FK_SAVEPOINT_035](crates/bench/sqlite_parity/cases/SQLITE_PARITY_902_CONSTRAINT_FK_SAVEPOINT_035.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1652349 | 1510531 | <span style="color:#2563eb">49.65%</span> |
| 837 | [00113 DOT_NULLVALUE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_113_DOT_NULLVALUE.rs) | P0 | memory | CLI_DOT_COMMAND | 1469773 | 1510361 | <span style="color:#2563eb">49.65%</span> |
| 838 | [00231 SCALAR_NULL_COALESCE_001](crates/bench/sqlite_parity/cases/SQLITE_PARITY_231_SCALAR_NULL_COALESCE_001.rs) | P1 | memory | GEN_SQL_SCALAR | 1836197 | 1510330 | <span style="color:#2563eb">49.66%</span> |
| 839 | [00309 SCALAR_CAST_TYPEOF_021](crates/bench/sqlite_parity/cases/SQLITE_PARITY_309_SCALAR_CAST_TYPEOF_021.rs) | P1 | memory | GEN_SQL_SCALAR | 1467298 | 1510220 | <span style="color:#2563eb">49.66%</span> |
| 840 | [00698 JOIN_SUBQUERY_EXISTS_091](crates/bench/sqlite_parity/cases/SQLITE_PARITY_698_JOIN_SUBQUERY_EXISTS_091.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1728293 | 1510210 | <span style="color:#2563eb">49.66%</span> |
| 841 | [00249 SCALAR_CAST_TYPEOF_006](crates/bench/sqlite_parity/cases/SQLITE_PARITY_249_SCALAR_CAST_TYPEOF_006.rs) | P1 | memory | GEN_SQL_SCALAR | 1494731 | 1510070 | <span style="color:#2563eb">49.66%</span> |
| 842 | [00571 AGG_GROUP_HAVING_064](crates/bench/sqlite_parity/cases/SQLITE_PARITY_571_AGG_GROUP_HAVING_064.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1700581 | 1509980 | <span style="color:#2563eb">49.67%</span> |
| 843 | [00608 JOIN_SUBQUERY_EXISTS_001](crates/bench/sqlite_parity/cases/SQLITE_PARITY_608_JOIN_SUBQUERY_EXISTS_001.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2072645 | 1509949 | <span style="color:#2563eb">49.67%</span> |
| 844 | [00697 JOIN_SUBQUERY_EXISTS_090](crates/bench/sqlite_parity/cases/SQLITE_PARITY_697_JOIN_SUBQUERY_EXISTS_090.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1713084 | 1509919 | <span style="color:#2563eb">49.67%</span> |
| 845 | [01104 INDEX_SCHEMA_PRAGMA_037](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1104_INDEX_SCHEMA_PRAGMA_037.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1718364 | 1509449 | <span style="color:#2563eb">49.69%</span> |
| 846 | [01027 JSON_EXTRACT_SET_020](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1027_JSON_EXTRACT_SET_020.rs) | P2 | memory | GEN_SQL_JSON | 1604699 | 1509298 | <span style="color:#2563eb">49.69%</span> |
| 847 | [00105 CASE_SENSITIVE_LIKE_PRAGMA](crates/bench/sqlite_parity/cases/SQLITE_PARITY_105_CASE_SENSITIVE_LIKE_PRAGMA.rs) | P2 | memory | SQL_PRAGMA | 1469813 | 1509248 | <span style="color:#2563eb">49.69%</span> |
| 848 | [00956 VIEW_TRIGGER_GENERATED_009](crates/bench/sqlite_parity/cases/SQLITE_PARITY_956_VIEW_TRIGGER_GENERATED_009.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1745064 | 1509098 | <span style="color:#2563eb">49.70%</span> |
| 849 | [00087 DATE_TIMEDIFF_FUNCTION](crates/bench/sqlite_parity/cases/SQLITE_PARITY_087_DATE_TIMEDIFF_FUNCTION.rs) | P0 | memory | SQL_FUNCTIONS | 1500111 | 1509058 | <span style="color:#2563eb">49.70%</span> |
| 850 | [00625 JOIN_SUBQUERY_EXISTS_018](crates/bench/sqlite_parity/cases/SQLITE_PARITY_625_JOIN_SUBQUERY_EXISTS_018.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1721209 | 1509047 | <span style="color:#2563eb">49.70%</span> |
| 851 | [00368 SCALAR_ARITH_036](crates/bench/sqlite_parity/cases/SQLITE_PARITY_368_SCALAR_ARITH_036.rs) | P1 | memory | GEN_SQL_SCALAR | 1412064 | 1508978 | <span style="color:#2563eb">49.70%</span> |
| 852 | [01089 INDEX_SCHEMA_PRAGMA_022](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1089_INDEX_SCHEMA_PRAGMA_022.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 2970846 | 1508958 | <span style="color:#2563eb">49.70%</span> |
| 853 | [00313 SCALAR_CAST_TYPEOF_022](crates/bench/sqlite_parity/cases/SQLITE_PARITY_313_SCALAR_CAST_TYPEOF_022.rs) | P1 | memory | GEN_SQL_SCALAR | 1428896 | 1508827 | <span style="color:#2563eb">49.71%</span> |
| 854 | [00528 AGG_GROUP_HAVING_021](crates/bench/sqlite_parity/cases/SQLITE_PARITY_528_AGG_GROUP_HAVING_021.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1659462 | 1508687 | <span style="color:#2563eb">49.71%</span> |
| 855 | [01109 INDEX_SCHEMA_PRAGMA_042](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1109_INDEX_SCHEMA_PRAGMA_042.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1686614 | 1508587 | <span style="color:#2563eb">49.71%</span> |
| 856 | [00385 SCALAR_CAST_TYPEOF_040](crates/bench/sqlite_parity/cases/SQLITE_PARITY_385_SCALAR_CAST_TYPEOF_040.rs) | P1 | memory | GEN_SQL_SCALAR | 1506723 | 1508546 | <span style="color:#2563eb">49.72%</span> |
| 857 | [01030 JSON_EXTRACT_SET_023](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1030_JSON_EXTRACT_SET_023.rs) | P2 | memory | GEN_SQL_JSON | 1572899 | 1508536 | <span style="color:#2563eb">49.72%</span> |
| 858 | [00955 VIEW_TRIGGER_GENERATED_008](crates/bench/sqlite_parity/cases/SQLITE_PARITY_955_VIEW_TRIGGER_GENERATED_008.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2260110 | 1508487 | <span style="color:#2563eb">49.72%</span> |
| 859 | [00269 SCALAR_CAST_TYPEOF_011](crates/bench/sqlite_parity/cases/SQLITE_PARITY_269_SCALAR_CAST_TYPEOF_011.rs) | P1 | memory | GEN_SQL_SCALAR | 1458372 | 1508217 | <span style="color:#2563eb">49.73%</span> |
| 860 | [00763 CTE_RECURSIVE_MATRIX_056](crates/bench/sqlite_parity/cases/SQLITE_PARITY_763_CTE_RECURSIVE_MATRIX_056.rs) | P1 | memory | GEN_SQL_CTE | 1924374 | 1508206 | <span style="color:#2563eb">49.73%</span> |
| 861 | [00766 CTE_RECURSIVE_MATRIX_059](crates/bench/sqlite_parity/cases/SQLITE_PARITY_766_CTE_RECURSIVE_MATRIX_059.rs) | P1 | memory | GEN_SQL_CTE | 1586615 | 1508127 | <span style="color:#2563eb">49.73%</span> |
| 862 | [00930 CONSTRAINT_FK_SAVEPOINT_063](crates/bench/sqlite_parity/cases/SQLITE_PARITY_930_CONSTRAINT_FK_SAVEPOINT_063.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1697535 | 1508046 | <span style="color:#2563eb">49.73%</span> |
| 863 | [01001 VIEW_TRIGGER_GENERATED_054](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1001_VIEW_TRIGGER_GENERATED_054.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1702775 | 1507986 | <span style="color:#2563eb">49.73%</span> |
| 864 | [00421 DML_WHERE_ORDER_LIMIT_034](crates/bench/sqlite_parity/cases/SQLITE_PARITY_421_DML_WHERE_ORDER_LIMIT_034.rs) | P1 | memory | GEN_SQL_DML | 1586915 | 1507956 | <span style="color:#2563eb">49.73%</span> |
| 865 | [00768 CTE_RECURSIVE_MATRIX_061](crates/bench/sqlite_parity/cases/SQLITE_PARITY_768_CTE_RECURSIVE_MATRIX_061.rs) | P1 | memory | GEN_SQL_CTE | 1636579 | 1507926 | <span style="color:#2563eb">49.74%</span> |
| 866 | [00258 SCALAR_STRING_008](crates/bench/sqlite_parity/cases/SQLITE_PARITY_258_SCALAR_STRING_008.rs) | P1 | memory | GEN_SQL_SCALAR | 1496524 | 1507855 | <span style="color:#2563eb">49.74%</span> |
| 867 | [00282 SCALAR_STRING_014](crates/bench/sqlite_parity/cases/SQLITE_PARITY_282_SCALAR_STRING_014.rs) | P1 | memory | GEN_SQL_SCALAR | 1490092 | 1507815 | <span style="color:#2563eb">49.74%</span> |
| 868 | [00572 AGG_GROUP_HAVING_065](crates/bench/sqlite_parity/cases/SQLITE_PARITY_572_AGG_GROUP_HAVING_065.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1686775 | 1507626 | <span style="color:#2563eb">49.75%</span> |
| 869 | [01031 JSON_EXTRACT_SET_024](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1031_JSON_EXTRACT_SET_024.rs) | P2 | memory | GEN_SQL_JSON | 1541038 | 1507615 | <span style="color:#2563eb">49.75%</span> |
| 870 | [00351 SCALAR_NULL_COALESCE_031](crates/bench/sqlite_parity/cases/SQLITE_PARITY_351_SCALAR_NULL_COALESCE_031.rs) | P1 | memory | GEN_SQL_SCALAR | 1772898 | 1507585 | <span style="color:#2563eb">49.75%</span> |
| 871 | [00367 SCALAR_NULL_COALESCE_035](crates/bench/sqlite_parity/cases/SQLITE_PARITY_367_SCALAR_NULL_COALESCE_035.rs) | P1 | memory | GEN_SQL_SCALAR | 1514728 | 1507515 | <span style="color:#2563eb">49.75%</span> |
| 872 | [00617 JOIN_SUBQUERY_EXISTS_010](crates/bench/sqlite_parity/cases/SQLITE_PARITY_617_JOIN_SUBQUERY_EXISTS_010.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1695070 | 1507475 | <span style="color:#2563eb">49.75%</span> |
| 873 | [00596 AGG_GROUP_HAVING_089](crates/bench/sqlite_parity/cases/SQLITE_PARITY_596_AGG_GROUP_HAVING_089.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1689780 | 1507424 | <span style="color:#2563eb">49.75%</span> |
| 874 | [00426 DML_WHERE_ORDER_LIMIT_039](crates/bench/sqlite_parity/cases/SQLITE_PARITY_426_DML_WHERE_ORDER_LIMIT_039.rs) | P1 | memory | GEN_SQL_DML | 1558992 | 1507355 | <span style="color:#2563eb">49.75%</span> |
| 875 | [00853 WINDOW_PARTITION_SUM_066](crates/bench/sqlite_parity/cases/SQLITE_PARITY_853_WINDOW_PARTITION_SUM_066.rs) | P2 | memory | GEN_SQL_WINDOW | 1628664 | 1507334 | <span style="color:#2563eb">49.76%</span> |
| 876 | [00524 AGG_GROUP_HAVING_017](crates/bench/sqlite_parity/cases/SQLITE_PARITY_524_AGG_GROUP_HAVING_017.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1737511 | 1507295 | <span style="color:#2563eb">49.76%</span> |
| 877 | [01049 JSON_EXTRACT_SET_042](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1049_JSON_EXTRACT_SET_042.rs) | P2 | memory | GEN_SQL_JSON | 1599229 | 1507144 | <span style="color:#2563eb">49.76%</span> |
| 878 | [00700 JOIN_SUBQUERY_EXISTS_093](crates/bench/sqlite_parity/cases/SQLITE_PARITY_700_JOIN_SUBQUERY_EXISTS_093.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1705930 | 1506983 | <span style="color:#2563eb">49.77%</span> |
| 879 | [00402 DML_WHERE_ORDER_LIMIT_015](crates/bench/sqlite_parity/cases/SQLITE_PARITY_402_DML_WHERE_ORDER_LIMIT_015.rs) | P1 | memory | GEN_SQL_DML | 1761576 | 1506884 | <span style="color:#2563eb">49.77%</span> |
| 880 | [00039 CREATE_TRIGGER_BEFORE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_039_CREATE_TRIGGER_BEFORE.rs) | P0 | memory | SQL_TRIGGER | 1620429 | 1506834 | <span style="color:#2563eb">49.77%</span> |
| 881 | [00359 SCALAR_NULL_COALESCE_033](crates/bench/sqlite_parity/cases/SQLITE_PARITY_359_SCALAR_NULL_COALESCE_033.rs) | P1 | memory | GEN_SQL_SCALAR | 1490523 | 1506423 | <span style="color:#2563eb">49.79%</span> |
| 882 | [00027 FOREIGN_KEY_FAILURE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_027_FOREIGN_KEY_FAILURE.rs) | P0 | memory | SQL_FOREIGN_KEYS_NEGATIVE | 1551378 | 1506363 | <span style="color:#2563eb">49.79%</span> |
| 883 | [01127 INDEX_SCHEMA_PRAGMA_060](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1127_INDEX_SCHEMA_PRAGMA_060.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1681735 | 1506252 | <span style="color:#2563eb">49.79%</span> |
| 884 | [00072 ORDER_BY_NULLS_FIRST_LAST](crates/bench/sqlite_parity/cases/SQLITE_PARITY_072_ORDER_BY_NULLS_FIRST_LAST.rs) | P0 | memory | SQL_SELECT | 1463452 | 1506142 | <span style="color:#2563eb">49.80%</span> |
| 885 | [00840 WINDOW_PARTITION_SUM_053](crates/bench/sqlite_parity/cases/SQLITE_PARITY_840_WINDOW_PARTITION_SUM_053.rs) | P2 | memory | GEN_SQL_WINDOW | 1592927 | 1506092 | <span style="color:#2563eb">49.80%</span> |
| 886 | [01126 INDEX_SCHEMA_PRAGMA_059](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1126_INDEX_SCHEMA_PRAGMA_059.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1658370 | 1506092 | <span style="color:#2563eb">49.80%</span> |
| 887 | [00457 DML_WHERE_ORDER_LIMIT_070](crates/bench/sqlite_parity/cases/SQLITE_PARITY_457_DML_WHERE_ORDER_LIMIT_070.rs) | P1 | memory | GEN_SQL_DML | 1631009 | 1505972 | <span style="color:#2563eb">49.80%</span> |
| 888 | [00951 VIEW_TRIGGER_GENERATED_004](crates/bench/sqlite_parity/cases/SQLITE_PARITY_951_VIEW_TRIGGER_GENERATED_004.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1730508 | 1505832 | <span style="color:#2563eb">49.81%</span> |
| 889 | [00045 REINDEX_COMMAND](crates/bench/sqlite_parity/cases/SQLITE_PARITY_045_REINDEX_COMMAND.rs) | P0 | memory | SQL_REINDEX | 1808535 | 1505722 | <span style="color:#2563eb">49.81%</span> |
| 890 | [00794 WINDOW_PARTITION_SUM_007](crates/bench/sqlite_parity/cases/SQLITE_PARITY_794_WINDOW_PARTITION_SUM_007.rs) | P2 | memory | GEN_SQL_WINDOW | 2029333 | 1505661 | <span style="color:#2563eb">49.81%</span> |
| 891 | [00352 SCALAR_ARITH_032](crates/bench/sqlite_parity/cases/SQLITE_PARITY_352_SCALAR_ARITH_032.rs) | P1 | memory | GEN_SQL_SCALAR | 1498187 | 1505330 | <span style="color:#2563eb">49.82%</span> |
| 892 | [00762 CTE_RECURSIVE_MATRIX_055](crates/bench/sqlite_parity/cases/SQLITE_PARITY_762_CTE_RECURSIVE_MATRIX_055.rs) | P1 | memory | GEN_SQL_CTE | 1558441 | 1505311 | <span style="color:#2563eb">49.82%</span> |
| 893 | [00839 WINDOW_PARTITION_SUM_052](crates/bench/sqlite_parity/cases/SQLITE_PARITY_839_WINDOW_PARTITION_SUM_052.rs) | P2 | memory | GEN_SQL_WINDOW | 1596594 | 1505251 | <span style="color:#2563eb">49.82%</span> |
| 894 | [00144 DOT_PROMPT](crates/bench/sqlite_parity/cases/SQLITE_PARITY_144_DOT_PROMPT.rs) | P0 | memory | CLI_DOT_COMMAND | 1574813 | 1505220 | <span style="color:#2563eb">49.83%</span> |
| 895 | [00909 CONSTRAINT_FK_SAVEPOINT_042](crates/bench/sqlite_parity/cases/SQLITE_PARITY_909_CONSTRAINT_FK_SAVEPOINT_042.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1673108 | 1505111 | <span style="color:#2563eb">49.83%</span> |
| 896 | [00348 SCALAR_ARITH_031](crates/bench/sqlite_parity/cases/SQLITE_PARITY_348_SCALAR_ARITH_031.rs) | P1 | memory | GEN_SQL_SCALAR | 1463030 | 1505030 | <span style="color:#2563eb">49.83%</span> |
| 897 | [00094 FTS5_HIGHLIGHT_OPTIONAL](crates/bench/sqlite_parity/cases/SQLITE_PARITY_094_FTS5_HIGHLIGHT_OPTIONAL.rs) | P2 | memory | SQL_VIRTUAL_TABLE_OPTIONAL | 1756617 | 1504990 | <span style="color:#2563eb">49.83%</span> |
| 898 | [00069 COLLATE_NOCASE_RTRIM_BINARY](crates/bench/sqlite_parity/cases/SQLITE_PARITY_069_COLLATE_NOCASE_RTRIM_BINARY.rs) | P0 | memory | SQL_COLLATION | 1758901 | 1504960 | <span style="color:#2563eb">49.83%</span> |
| 899 | [00793 WINDOW_PARTITION_SUM_006](crates/bench/sqlite_parity/cases/SQLITE_PARITY_793_WINDOW_PARTITION_SUM_006.rs) | P2 | memory | GEN_SQL_WINDOW | 1624506 | 1504820 | <span style="color:#2563eb">49.84%</span> |
| 900 | [00034 EXPRESSION_INDEX](crates/bench/sqlite_parity/cases/SQLITE_PARITY_034_EXPRESSION_INDEX.rs) | P0 | memory | SQL_INDEX | 1565975 | 1504780 | <span style="color:#2563eb">49.84%</span> |
| 901 | [00739 CTE_RECURSIVE_MATRIX_032](crates/bench/sqlite_parity/cases/SQLITE_PARITY_739_CTE_RECURSIVE_MATRIX_032.rs) | P1 | memory | GEN_SQL_CTE | 1537802 | 1504779 | <span style="color:#2563eb">49.84%</span> |
| 902 | [00860 WINDOW_PARTITION_SUM_073](crates/bench/sqlite_parity/cases/SQLITE_PARITY_860_WINDOW_PARTITION_SUM_073.rs) | P2 | memory | GEN_SQL_WINDOW | 2340963 | 1504510 | <span style="color:#2563eb">49.85%</span> |
| 903 | [00272 SCALAR_ARITH_012](crates/bench/sqlite_parity/cases/SQLITE_PARITY_272_SCALAR_ARITH_012.rs) | P1 | memory | GEN_SQL_SCALAR | 1462209 | 1504469 | <span style="color:#2563eb">49.85%</span> |
| 904 | [00358 SCALAR_STRING_033](crates/bench/sqlite_parity/cases/SQLITE_PARITY_358_SCALAR_STRING_033.rs) | P1 | memory | GEN_SQL_SCALAR | 1513746 | 1504369 | <span style="color:#2563eb">49.85%</span> |
| 905 | [00337 SCALAR_CAST_TYPEOF_028](crates/bench/sqlite_parity/cases/SQLITE_PARITY_337_SCALAR_CAST_TYPEOF_028.rs) | P1 | memory | GEN_SQL_SCALAR | 1410161 | 1504368 | <span style="color:#2563eb">49.85%</span> |
| 906 | [00467 DML_WHERE_ORDER_LIMIT_080](crates/bench/sqlite_parity/cases/SQLITE_PARITY_467_DML_WHERE_ORDER_LIMIT_080.rs) | P1 | memory | GEN_SQL_DML | 1585322 | 1504048 | <span style="color:#2563eb">49.87%</span> |
| 907 | [01125 INDEX_SCHEMA_PRAGMA_058](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1125_INDEX_SCHEMA_PRAGMA_058.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1648031 | 1503798 | <span style="color:#2563eb">49.87%</span> |
| 908 | [00682 JOIN_SUBQUERY_EXISTS_075](crates/bench/sqlite_parity/cases/SQLITE_PARITY_682_JOIN_SUBQUERY_EXISTS_075.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1681194 | 1503737 | <span style="color:#2563eb">49.88%</span> |
| 909 | [00837 WINDOW_PARTITION_SUM_050](crates/bench/sqlite_parity/cases/SQLITE_PARITY_837_WINDOW_PARTITION_SUM_050.rs) | P2 | memory | GEN_SQL_WINDOW | 1618074 | 1503707 | <span style="color:#2563eb">49.88%</span> |
| 910 | [00899 CONSTRAINT_FK_SAVEPOINT_032](crates/bench/sqlite_parity/cases/SQLITE_PARITY_899_CONSTRAINT_FK_SAVEPOINT_032.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1714517 | 1503648 | <span style="color:#2563eb">49.88%</span> |
| 911 | [00278 SCALAR_STRING_013](crates/bench/sqlite_parity/cases/SQLITE_PARITY_278_SCALAR_STRING_013.rs) | P1 | memory | GEN_SQL_SCALAR | 1483950 | 1503417 | <span style="color:#2563eb">49.89%</span> |
| 912 | [00127 DOT_LIMIT](crates/bench/sqlite_parity/cases/SQLITE_PARITY_127_DOT_LIMIT.rs) | P0 | memory | CLI_DOT_COMMAND | 1416733 | 1503347 | <span style="color:#2563eb">49.89%</span> |
| 913 | [00356 SCALAR_ARITH_033](crates/bench/sqlite_parity/cases/SQLITE_PARITY_356_SCALAR_ARITH_033.rs) | P1 | memory | GEN_SQL_SCALAR | 1681034 | 1503257 | <span style="color:#2563eb">49.89%</span> |
| 914 | [00648 JOIN_SUBQUERY_EXISTS_041](crates/bench/sqlite_parity/cases/SQLITE_PARITY_648_JOIN_SUBQUERY_EXISTS_041.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1804116 | 1503086 | <span style="color:#2563eb">49.90%</span> |
| 915 | [00527 AGG_GROUP_HAVING_020](crates/bench/sqlite_parity/cases/SQLITE_PARITY_527_AGG_GROUP_HAVING_020.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1658731 | 1503066 | <span style="color:#2563eb">49.90%</span> |
| 916 | [00347 SCALAR_NULL_COALESCE_030](crates/bench/sqlite_parity/cases/SQLITE_PARITY_347_SCALAR_NULL_COALESCE_030.rs) | P1 | memory | GEN_SQL_SCALAR | 1828844 | 1502856 | <span style="color:#2563eb">49.90%</span> |
| 917 | [01066 JSON_EXTRACT_SET_059](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1066_JSON_EXTRACT_SET_059.rs) | P2 | memory | GEN_SQL_JSON | 1607354 | 1502736 | <span style="color:#2563eb">49.91%</span> |
| 918 | [00990 VIEW_TRIGGER_GENERATED_043](crates/bench/sqlite_parity/cases/SQLITE_PARITY_990_VIEW_TRIGGER_GENERATED_043.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1722973 | 1502675 | <span style="color:#2563eb">49.91%</span> |
| 919 | [00638 JOIN_SUBQUERY_EXISTS_031](crates/bench/sqlite_parity/cases/SQLITE_PARITY_638_JOIN_SUBQUERY_EXISTS_031.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1696412 | 1502305 | <span style="color:#2563eb">49.92%</span> |
| 920 | [00244 SCALAR_ARITH_005](crates/bench/sqlite_parity/cases/SQLITE_PARITY_244_SCALAR_ARITH_005.rs) | P1 | memory | GEN_SQL_SCALAR | 1436340 | 1502184 | <span style="color:#2563eb">49.93%</span> |
| 921 | [00049 PRAGMA_TEMP_STORE_MEMORY](crates/bench/sqlite_parity/cases/SQLITE_PARITY_049_PRAGMA_TEMP_STORE_MEMORY.rs) | P0 | memory | SQL_PRAGMA | 1394230 | 1502034 | <span style="color:#2563eb">49.93%</span> |
| 922 | [00954 VIEW_TRIGGER_GENERATED_007](crates/bench/sqlite_parity/cases/SQLITE_PARITY_954_VIEW_TRIGGER_GENERATED_007.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1698747 | 1501904 | <span style="color:#2563eb">49.94%</span> |
| 923 | [00713 CTE_RECURSIVE_MATRIX_006](crates/bench/sqlite_parity/cases/SQLITE_PARITY_713_CTE_RECURSIVE_MATRIX_006.rs) | P1 | memory | GEN_SQL_CTE | 1848431 | 1501794 | <span style="color:#2563eb">49.94%</span> |
| 924 | [00050 PRAGMA_TABLE_INFO_FUNCTION](crates/bench/sqlite_parity/cases/SQLITE_PARITY_050_PRAGMA_TABLE_INFO_FUNCTION.rs) | P0 | memory | SQL_PRAGMA | 1602355 | 1501724 | <span style="color:#2563eb">49.94%</span> |
| 925 | [01014 JSON_EXTRACT_SET_007](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1014_JSON_EXTRACT_SET_007.rs) | P2 | memory | GEN_SQL_JSON | 1558321 | 1501604 | <span style="color:#2563eb">49.95%</span> |
| 926 | [00276 SCALAR_ARITH_013](crates/bench/sqlite_parity/cases/SQLITE_PARITY_276_SCALAR_ARITH_013.rs) | P1 | memory | GEN_SQL_SCALAR | 1478600 | 1501394 | <span style="color:#2563eb">49.95%</span> |
| 927 | [00694 JOIN_SUBQUERY_EXISTS_087](crates/bench/sqlite_parity/cases/SQLITE_PARITY_694_JOIN_SUBQUERY_EXISTS_087.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1721120 | 1501383 | <span style="color:#2563eb">49.95%</span> |
| 928 | [00911 CONSTRAINT_FK_SAVEPOINT_044](crates/bench/sqlite_parity/cases/SQLITE_PARITY_911_CONSTRAINT_FK_SAVEPOINT_044.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1683018 | 1501253 | <span style="color:#2563eb">49.96%</span> |
| 929 | [00413 DML_WHERE_ORDER_LIMIT_026](crates/bench/sqlite_parity/cases/SQLITE_PARITY_413_DML_WHERE_ORDER_LIMIT_026.rs) | P1 | memory | GEN_SQL_DML | 1616821 | 1501243 | <span style="color:#2563eb">49.96%</span> |
| 930 | [00733 CTE_RECURSIVE_MATRIX_026](crates/bench/sqlite_parity/cases/SQLITE_PARITY_733_CTE_RECURSIVE_MATRIX_026.rs) | P1 | memory | GEN_SQL_CTE | 1579531 | 1501222 | <span style="color:#2563eb">49.96%</span> |
| 931 | [00089 JSON_TABLE_VALUED_FUNCTIONS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_089_JSON_TABLE_VALUED_FUNCTIONS.rs) | P0 | memory | SQL_JSON | 1465986 | 1501162 | <span style="color:#2563eb">49.96%</span> |
| 932 | [00984 VIEW_TRIGGER_GENERATED_037](crates/bench/sqlite_parity/cases/SQLITE_PARITY_984_VIEW_TRIGGER_GENERATED_037.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1753260 | 1501103 | <span style="color:#2563eb">49.96%</span> |
| 933 | [00044 ANALYZE_SQLITE_STAT1](crates/bench/sqlite_parity/cases/SQLITE_PARITY_044_ANALYZE_SQLITE_STAT1.rs) | P0 | memory | SQL_ANALYZE | 1589129 | 1500973 | <span style="color:#2563eb">49.97%</span> |
| 934 | [00112 DOT_SEPARATOR](crates/bench/sqlite_parity/cases/SQLITE_PARITY_112_DOT_SEPARATOR.rs) | P0 | memory | CLI_DOT_COMMAND | 1438384 | 1500883 | <span style="color:#2563eb">49.97%</span> |
| 935 | [00303 SCALAR_NULL_COALESCE_019](crates/bench/sqlite_parity/cases/SQLITE_PARITY_303_SCALAR_NULL_COALESCE_019.rs) | P1 | memory | GEN_SQL_SCALAR | 1490182 | 1500391 | <span style="color:#2563eb">49.99%</span> |
| 936 | [00133 DOT_AUTH](crates/bench/sqlite_parity/cases/SQLITE_PARITY_133_DOT_AUTH.rs) | P3 | memory | CLI_DOT_COMMAND_DIAGNOSTIC | 1476647 | 1500361 | <span style="color:#2563eb">49.99%</span> |
| 937 | [00068 CAST_AND_TYPE_AFFINITY](crates/bench/sqlite_parity/cases/SQLITE_PARITY_068_CAST_AND_TYPE_AFFINITY.rs) | P0 | memory | SQL_EXPRESSIONS | 1485152 | 1500351 | <span style="color:#2563eb">49.99%</span> |
| 938 | [01091 INDEX_SCHEMA_PRAGMA_024](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1091_INDEX_SCHEMA_PRAGMA_024.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1699739 | 1500251 | <span style="color:#2563eb">49.99%</span> |
| 939 | [00208 OPT_VFSTRACE_CATALOG](crates/bench/sqlite_parity/cases/SQLITE_PARITY_208_OPT_VFSTRACE_CATALOG.rs) | P4 | catalog | CLI_OPTION_CATALOG | 1471317 | 1500041 | <span style="color:#2563eb">50.00%</span> |
| 940 | [00021 UPSERT_DO_NOTHING](crates/bench/sqlite_parity/cases/SQLITE_PARITY_021_UPSERT_DO_NOTHING.rs) | P0 | memory | SQL_UPSERT | 1544945 | 1499951 | <span style="color:#2563eb">50.00%</span> |
| 941 | [00881 CONSTRAINT_FK_SAVEPOINT_014](crates/bench/sqlite_parity/cases/SQLITE_PARITY_881_CONSTRAINT_FK_SAVEPOINT_014.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1685351 | 1499931 | <span style="color:#2563eb">50.00%</span> |
| 942 | [00516 AGG_GROUP_HAVING_009](crates/bench/sqlite_parity/cases/SQLITE_PARITY_516_AGG_GROUP_HAVING_009.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1909366 | 1499911 | <span style="color:#2563eb">50.00%</span> |
| 943 | [00912 CONSTRAINT_FK_SAVEPOINT_045](crates/bench/sqlite_parity/cases/SQLITE_PARITY_912_CONSTRAINT_FK_SAVEPOINT_045.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1672818 | 1499900 | <span style="color:#2563eb">50.00%</span> |
| 944 | [00354 SCALAR_STRING_032](crates/bench/sqlite_parity/cases/SQLITE_PARITY_354_SCALAR_STRING_032.rs) | P1 | memory | GEN_SQL_SCALAR | 1450717 | 1499891 | <span style="color:#2563eb">50.00%</span> |
| 945 | [00819 WINDOW_PARTITION_SUM_032](crates/bench/sqlite_parity/cases/SQLITE_PARITY_819_WINDOW_PARTITION_SUM_032.rs) | P2 | memory | GEN_SQL_WINDOW | 1612414 | 1499880 | <span style="color:#2563eb">50.00%</span> |
| 946 | [00752 CTE_RECURSIVE_MATRIX_045](crates/bench/sqlite_parity/cases/SQLITE_PARITY_752_CTE_RECURSIVE_MATRIX_045.rs) | P1 | memory | GEN_SQL_CTE | 1945394 | 1499871 | <span style="color:#2563eb">50.00%</span> |
| 947 | [00765 CTE_RECURSIVE_MATRIX_058](crates/bench/sqlite_parity/cases/SQLITE_PARITY_765_CTE_RECURSIVE_MATRIX_058.rs) | P1 | memory | GEN_SQL_CTE | 1551668 | 1499079 | <span style="color:#2563eb">50.03%</span> |
| 948 | [00781 CTE_RECURSIVE_MATRIX_074](crates/bench/sqlite_parity/cases/SQLITE_PARITY_781_CTE_RECURSIVE_MATRIX_074.rs) | P1 | memory | GEN_SQL_CTE | 1499320 | 1499009 | <span style="color:#2563eb">50.03%</span> |
| 949 | [00851 WINDOW_PARTITION_SUM_064](crates/bench/sqlite_parity/cases/SQLITE_PARITY_851_WINDOW_PARTITION_SUM_064.rs) | P2 | memory | GEN_SQL_WINDOW | 1828232 | 1498849 | <span style="color:#2563eb">50.04%</span> |
| 950 | [00624 JOIN_SUBQUERY_EXISTS_017](crates/bench/sqlite_parity/cases/SQLITE_PARITY_624_JOIN_SUBQUERY_EXISTS_017.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1641278 | 1498808 | <span style="color:#2563eb">50.04%</span> |
| 951 | [00889 CONSTRAINT_FK_SAVEPOINT_022](crates/bench/sqlite_parity/cases/SQLITE_PARITY_889_CONSTRAINT_FK_SAVEPOINT_022.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1652770 | 1498749 | <span style="color:#2563eb">50.04%</span> |
| 952 | [00535 AGG_GROUP_HAVING_028](crates/bench/sqlite_parity/cases/SQLITE_PARITY_535_AGG_GROUP_HAVING_028.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1973347 | 1498678 | <span style="color:#2563eb">50.04%</span> |
| 953 | [01020 JSON_EXTRACT_SET_013](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1020_JSON_EXTRACT_SET_013.rs) | P2 | memory | GEN_SQL_JSON | 1609718 | 1498517 | <span style="color:#2563eb">50.05%</span> |
| 954 | [00969 VIEW_TRIGGER_GENERATED_022](crates/bench/sqlite_parity/cases/SQLITE_PARITY_969_VIEW_TRIGGER_GENERATED_022.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1739765 | 1498468 | <span style="color:#2563eb">50.05%</span> |
| 955 | [00028 ALTER_TABLE_RENAME](crates/bench/sqlite_parity/cases/SQLITE_PARITY_028_ALTER_TABLE_RENAME.rs) | P0 | memory | SQL_ALTER | 1868840 | 1498457 | <span style="color:#2563eb">50.05%</span> |
| 956 | [00018 DELETE_RETURNING](crates/bench/sqlite_parity/cases/SQLITE_PARITY_018_DELETE_RETURNING.rs) | P0 | memory | SQL_DELETE | 1594069 | 1498298 | <span style="color:#2563eb">50.06%</span> |
| 957 | [00349 SCALAR_CAST_TYPEOF_031](crates/bench/sqlite_parity/cases/SQLITE_PARITY_349_SCALAR_CAST_TYPEOF_031.rs) | P1 | memory | GEN_SQL_SCALAR | 1463452 | 1498117 | <span style="color:#2563eb">50.06%</span> |
| 958 | [00603 AGG_GROUP_HAVING_096](crates/bench/sqlite_parity/cases/SQLITE_PARITY_603_AGG_GROUP_HAVING_096.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1875321 | 1497937 | <span style="color:#2563eb">50.07%</span> |
| 959 | [00302 SCALAR_STRING_019](crates/bench/sqlite_parity/cases/SQLITE_PARITY_302_SCALAR_STRING_019.rs) | P1 | memory | GEN_SQL_SCALAR | 1460836 | 1497907 | <span style="color:#2563eb">50.07%</span> |
| 960 | [00898 CONSTRAINT_FK_SAVEPOINT_031](crates/bench/sqlite_parity/cases/SQLITE_PARITY_898_CONSTRAINT_FK_SAVEPOINT_031.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1693317 | 1497797 | <span style="color:#2563eb">50.07%</span> |
| 961 | [00055 JOINS_RIGHT_FULL_OUTER](crates/bench/sqlite_parity/cases/SQLITE_PARITY_055_JOINS_RIGHT_FULL_OUTER.rs) | P0 | memory | SQL_JOIN | 2321116 | 1497766 | <span style="color:#2563eb">50.07%</span> |
| 962 | [00544 AGG_GROUP_HAVING_037](crates/bench/sqlite_parity/cases/SQLITE_PARITY_544_AGG_GROUP_HAVING_037.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1747329 | 1497496 | <span style="color:#2563eb">50.08%</span> |
| 963 | [01028 JSON_EXTRACT_SET_021](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1028_JSON_EXTRACT_SET_021.rs) | P2 | memory | GEN_SQL_JSON | 1560405 | 1497466 | <span style="color:#2563eb">50.08%</span> |
| 964 | [00248 SCALAR_ARITH_006](crates/bench/sqlite_parity/cases/SQLITE_PARITY_248_SCALAR_ARITH_006.rs) | P1 | memory | GEN_SQL_SCALAR | 1449885 | 1497386 | <span style="color:#2563eb">50.09%</span> |
| 965 | [00570 AGG_GROUP_HAVING_063](crates/bench/sqlite_parity/cases/SQLITE_PARITY_570_AGG_GROUP_HAVING_063.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1674451 | 1497335 | <span style="color:#2563eb">50.09%</span> |
| 966 | [01033 JSON_EXTRACT_SET_026](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1033_JSON_EXTRACT_SET_026.rs) | P2 | memory | GEN_SQL_JSON | 1561387 | 1497285 | <span style="color:#2563eb">50.09%</span> |
| 967 | [01026 JSON_EXTRACT_SET_019](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1026_JSON_EXTRACT_SET_019.rs) | P2 | memory | GEN_SQL_JSON | 1781153 | 1497246 | <span style="color:#2563eb">50.09%</span> |
| 968 | [00188 OPT_HEADER](crates/bench/sqlite_parity/cases/SQLITE_PARITY_188_OPT_HEADER.rs) | P1 | memory | CLI_OPTION | 1455105 | 1497235 | <span style="color:#2563eb">50.09%</span> |
| 969 | [00221 OPT_DOUBLE_DASH_END_OPTIONS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_221_OPT_DOUBLE_DASH_END_OPTIONS.rs) | P2 | memory | CLI_OPTION | 1496033 | 1497116 | <span style="color:#2563eb">50.10%</span> |
| 970 | [00043 ATTACH_DETACH_MEMORY](crates/bench/sqlite_parity/cases/SQLITE_PARITY_043_ATTACH_DETACH_MEMORY.rs) | P0 | memory | SQL_ATTACH | 1549304 | 1496614 | <span style="color:#2563eb">50.11%</span> |
| 971 | [00453 DML_WHERE_ORDER_LIMIT_066](crates/bench/sqlite_parity/cases/SQLITE_PARITY_453_DML_WHERE_ORDER_LIMIT_066.rs) | P1 | memory | GEN_SQL_DML | 1625608 | 1496574 | <span style="color:#2563eb">50.11%</span> |
| 972 | [00845 WINDOW_PARTITION_SUM_058](crates/bench/sqlite_parity/cases/SQLITE_PARITY_845_WINDOW_PARTITION_SUM_058.rs) | P2 | memory | GEN_SQL_WINDOW | 1805840 | 1496474 | <span style="color:#2563eb">50.12%</span> |
| 973 | [00468 DML_WHERE_ORDER_LIMIT_081](crates/bench/sqlite_parity/cases/SQLITE_PARITY_468_DML_WHERE_ORDER_LIMIT_081.rs) | P1 | memory | GEN_SQL_DML | 1611852 | 1496403 | <span style="color:#2563eb">50.12%</span> |
| 974 | [00124 DOT_BAIL_OFF](crates/bench/sqlite_parity/cases/SQLITE_PARITY_124_DOT_BAIL_OFF.rs) | P0 | memory | CLI_DOT_COMMAND_NEGATIVE | 1585472 | 1496374 | <span style="color:#2563eb">50.12%</span> |
| 975 | [00814 WINDOW_PARTITION_SUM_027](crates/bench/sqlite_parity/cases/SQLITE_PARITY_814_WINDOW_PARTITION_SUM_027.rs) | P2 | memory | GEN_SQL_WINDOW | 1685983 | 1496213 | <span style="color:#2563eb">50.13%</span> |
| 976 | [00235 SCALAR_NULL_COALESCE_002](crates/bench/sqlite_parity/cases/SQLITE_PARITY_235_SCALAR_NULL_COALESCE_002.rs) | P1 | memory | GEN_SQL_SCALAR | 1508477 | 1495953 | <span style="color:#2563eb">50.13%</span> |
| 977 | [00816 WINDOW_PARTITION_SUM_029](crates/bench/sqlite_parity/cases/SQLITE_PARITY_816_WINDOW_PARTITION_SUM_029.rs) | P2 | memory | GEN_SQL_WINDOW | 1616912 | 1495622 | <span style="color:#2563eb">50.15%</span> |
| 978 | [00949 VIEW_TRIGGER_GENERATED_002](crates/bench/sqlite_parity/cases/SQLITE_PARITY_949_VIEW_TRIGGER_GENERATED_002.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2965787 | 1495612 | <span style="color:#2563eb">50.15%</span> |
| 979 | [00277 SCALAR_CAST_TYPEOF_013](crates/bench/sqlite_parity/cases/SQLITE_PARITY_277_SCALAR_CAST_TYPEOF_013.rs) | P1 | memory | GEN_SQL_SCALAR | 1538554 | 1495421 | <span style="color:#2563eb">50.15%</span> |
| 980 | [00307 SCALAR_NULL_COALESCE_020](crates/bench/sqlite_parity/cases/SQLITE_PARITY_307_SCALAR_NULL_COALESCE_020.rs) | P1 | memory | GEN_SQL_SCALAR | 1450627 | 1495332 | <span style="color:#2563eb">50.16%</span> |
| 981 | [00330 SCALAR_STRING_026](crates/bench/sqlite_parity/cases/SQLITE_PARITY_330_SCALAR_STRING_026.rs) | P1 | memory | GEN_SQL_SCALAR | 1757327 | 1495262 | <span style="color:#2563eb">50.16%</span> |
| 982 | [00602 AGG_GROUP_HAVING_095](crates/bench/sqlite_parity/cases/SQLITE_PARITY_602_AGG_GROUP_HAVING_095.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1843441 | 1495211 | <span style="color:#2563eb">50.16%</span> |
| 983 | [00952 VIEW_TRIGGER_GENERATED_005](crates/bench/sqlite_parity/cases/SQLITE_PARITY_952_VIEW_TRIGGER_GENERATED_005.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1746868 | 1495201 | <span style="color:#2563eb">50.16%</span> |
| 984 | [00981 VIEW_TRIGGER_GENERATED_034](crates/bench/sqlite_parity/cases/SQLITE_PARITY_981_VIEW_TRIGGER_GENERATED_034.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1772537 | 1495112 | <span style="color:#2563eb">50.16%</span> |
| 985 | [00270 SCALAR_STRING_011](crates/bench/sqlite_parity/cases/SQLITE_PARITY_270_SCALAR_STRING_011.rs) | P1 | memory | GEN_SQL_SCALAR | 1465546 | 1495001 | <span style="color:#2563eb">50.17%</span> |
| 986 | [00920 CONSTRAINT_FK_SAVEPOINT_053](crates/bench/sqlite_parity/cases/SQLITE_PARITY_920_CONSTRAINT_FK_SAVEPOINT_053.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1671726 | 1494961 | <span style="color:#2563eb">50.17%</span> |
| 987 | [00285 SCALAR_CAST_TYPEOF_015](crates/bench/sqlite_parity/cases/SQLITE_PARITY_285_SCALAR_CAST_TYPEOF_015.rs) | P1 | memory | GEN_SQL_SCALAR | 1469893 | 1494891 | <span style="color:#2563eb">50.17%</span> |
| 988 | [01077 INDEX_SCHEMA_PRAGMA_010](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1077_INDEX_SCHEMA_PRAGMA_010.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1703576 | 1494791 | <span style="color:#2563eb">50.17%</span> |
| 989 | [00460 DML_WHERE_ORDER_LIMIT_073](crates/bench/sqlite_parity/cases/SQLITE_PARITY_460_DML_WHERE_ORDER_LIMIT_073.rs) | P1 | memory | GEN_SQL_DML | 1683979 | 1494640 | <span style="color:#2563eb">50.18%</span> |
| 990 | [00966 VIEW_TRIGGER_GENERATED_019](crates/bench/sqlite_parity/cases/SQLITE_PARITY_966_VIEW_TRIGGER_GENERATED_019.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1669862 | 1494430 | <span style="color:#2563eb">50.19%</span> |
| 991 | [00217 DETACH_DATABASE_SYNTAX](crates/bench/sqlite_parity/cases/SQLITE_PARITY_217_DETACH_DATABASE_SYNTAX.rs) | P0 | memory | SQL_ATTACH | 1552791 | 1494230 | <span style="color:#2563eb">50.19%</span> |
| 992 | [00422 DML_WHERE_ORDER_LIMIT_035](crates/bench/sqlite_parity/cases/SQLITE_PARITY_422_DML_WHERE_ORDER_LIMIT_035.rs) | P1 | memory | GEN_SQL_DML | 1631119 | 1494230 | <span style="color:#2563eb">50.19%</span> |
| 993 | [00243 SCALAR_NULL_COALESCE_004](crates/bench/sqlite_parity/cases/SQLITE_PARITY_243_SCALAR_NULL_COALESCE_004.rs) | P1 | memory | GEN_SQL_SCALAR | 1498147 | 1494200 | <span style="color:#2563eb">50.19%</span> |
| 994 | [00671 JOIN_SUBQUERY_EXISTS_064](crates/bench/sqlite_parity/cases/SQLITE_PARITY_671_JOIN_SUBQUERY_EXISTS_064.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1708445 | 1494149 | <span style="color:#2563eb">50.20%</span> |
| 995 | [00106 DOT_HELP](crates/bench/sqlite_parity/cases/SQLITE_PARITY_106_DOT_HELP.rs) | P0 | memory | CLI_DOT_COMMAND | 1461738 | 1493989 | <span style="color:#2563eb">50.20%</span> |
| 996 | [00998 VIEW_TRIGGER_GENERATED_051](crates/bench/sqlite_parity/cases/SQLITE_PARITY_998_VIEW_TRIGGER_GENERATED_051.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1730497 | 1493879 | <span style="color:#2563eb">50.20%</span> |
| 997 | [00642 JOIN_SUBQUERY_EXISTS_035](crates/bench/sqlite_parity/cases/SQLITE_PARITY_642_JOIN_SUBQUERY_EXISTS_035.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1690231 | 1493869 | <span style="color:#2563eb">50.20%</span> |
| 998 | [00626 JOIN_SUBQUERY_EXISTS_019](crates/bench/sqlite_parity/cases/SQLITE_PARITY_626_JOIN_SUBQUERY_EXISTS_019.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1690522 | 1493749 | <span style="color:#2563eb">50.21%</span> |
| 999 | [00595 AGG_GROUP_HAVING_088](crates/bench/sqlite_parity/cases/SQLITE_PARITY_595_AGG_GROUP_HAVING_088.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1971113 | 1493619 | <span style="color:#2563eb">50.21%</span> |
| 1000 | [00693 JOIN_SUBQUERY_EXISTS_086](crates/bench/sqlite_parity/cases/SQLITE_PARITY_693_JOIN_SUBQUERY_EXISTS_086.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1651077 | 1493539 | <span style="color:#2563eb">50.22%</span> |
| 1001 | [00620 JOIN_SUBQUERY_EXISTS_013](crates/bench/sqlite_parity/cases/SQLITE_PARITY_620_JOIN_SUBQUERY_EXISTS_013.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1691744 | 1493418 | <span style="color:#2563eb">50.22%</span> |
| 1002 | [00753 CTE_RECURSIVE_MATRIX_046](crates/bench/sqlite_parity/cases/SQLITE_PARITY_753_CTE_RECURSIVE_MATRIX_046.rs) | P1 | memory | GEN_SQL_CTE | 1561998 | 1493409 | <span style="color:#2563eb">50.22%</span> |
| 1003 | [00913 CONSTRAINT_FK_SAVEPOINT_046](crates/bench/sqlite_parity/cases/SQLITE_PARITY_913_CONSTRAINT_FK_SAVEPOINT_046.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1674561 | 1493398 | <span style="color:#2563eb">50.22%</span> |
| 1004 | [00448 DML_WHERE_ORDER_LIMIT_061](crates/bench/sqlite_parity/cases/SQLITE_PARITY_448_DML_WHERE_ORDER_LIMIT_061.rs) | P1 | memory | GEN_SQL_DML | 1621230 | 1492908 | <span style="color:#2563eb">50.24%</span> |
| 1005 | [00233 SCALAR_CAST_TYPEOF_002](crates/bench/sqlite_parity/cases/SQLITE_PARITY_233_SCALAR_CAST_TYPEOF_002.rs) | P1 | memory | GEN_SQL_SCALAR | 1993055 | 1492637 | <span style="color:#2563eb">50.25%</span> |
| 1006 | [00916 CONSTRAINT_FK_SAVEPOINT_049](crates/bench/sqlite_parity/cases/SQLITE_PARITY_916_CONSTRAINT_FK_SAVEPOINT_049.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1725828 | 1492636 | <span style="color:#2563eb">50.25%</span> |
| 1007 | [00134 DOT_CRLF](crates/bench/sqlite_parity/cases/SQLITE_PARITY_134_DOT_CRLF.rs) | P3 | memory | CLI_DOT_COMMAND_DIAGNOSTIC | 1453142 | 1492256 | <span style="color:#2563eb">50.26%</span> |
| 1008 | [00291 SCALAR_NULL_COALESCE_016](crates/bench/sqlite_parity/cases/SQLITE_PARITY_291_SCALAR_NULL_COALESCE_016.rs) | P1 | memory | GEN_SQL_SCALAR | 1472910 | 1492106 | <span style="color:#2563eb">50.26%</span> |
| 1009 | [00656 JOIN_SUBQUERY_EXISTS_049](crates/bench/sqlite_parity/cases/SQLITE_PARITY_656_JOIN_SUBQUERY_EXISTS_049.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1681615 | 1492066 | <span style="color:#2563eb">50.26%</span> |
| 1010 | [01112 INDEX_SCHEMA_PRAGMA_045](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1112_INDEX_SCHEMA_PRAGMA_045.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 2049812 | 1492005 | <span style="color:#2563eb">50.27%</span> |
| 1011 | [00593 AGG_GROUP_HAVING_086](crates/bench/sqlite_parity/cases/SQLITE_PARITY_593_AGG_GROUP_HAVING_086.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1704438 | 1491915 | <span style="color:#2563eb">50.27%</span> |
| 1012 | [00128 DOT_DBCONFIG](crates/bench/sqlite_parity/cases/SQLITE_PARITY_128_DOT_DBCONFIG.rs) | P0 | memory | CLI_DOT_COMMAND | 1422845 | 1491885 | <span style="color:#2563eb">50.27%</span> |
| 1013 | [00391 DML_WHERE_ORDER_LIMIT_004](crates/bench/sqlite_parity/cases/SQLITE_PARITY_391_DML_WHERE_ORDER_LIMIT_004.rs) | P1 | memory | GEN_SQL_DML | 1618616 | 1491865 | <span style="color:#2563eb">50.27%</span> |
| 1014 | [00332 SCALAR_ARITH_027](crates/bench/sqlite_parity/cases/SQLITE_PARITY_332_SCALAR_ARITH_027.rs) | P1 | memory | GEN_SQL_SCALAR | 1429397 | 1491855 | <span style="color:#2563eb">50.27%</span> |
| 1015 | [00184 OPT_ASCII_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_184_OPT_ASCII_MODE.rs) | P2 | memory | CLI_OPTION | 1437392 | 1491845 | <span style="color:#2563eb">50.27%</span> |
| 1016 | [00121 DOT_PARAMETER](crates/bench/sqlite_parity/cases/SQLITE_PARITY_121_DOT_PARAMETER.rs) | P0 | memory | CLI_DOT_COMMAND | 1664171 | 1491816 | <span style="color:#2563eb">50.27%</span> |
| 1017 | [00687 JOIN_SUBQUERY_EXISTS_080](crates/bench/sqlite_parity/cases/SQLITE_PARITY_687_JOIN_SUBQUERY_EXISTS_080.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1645677 | 1491775 | <span style="color:#2563eb">50.27%</span> |
| 1018 | [00392 DML_WHERE_ORDER_LIMIT_005](crates/bench/sqlite_parity/cases/SQLITE_PARITY_392_DML_WHERE_ORDER_LIMIT_005.rs) | P1 | memory | GEN_SQL_DML | 1637210 | 1491745 | <span style="color:#2563eb">50.28%</span> |
| 1019 | [00461 DML_WHERE_ORDER_LIMIT_074](crates/bench/sqlite_parity/cases/SQLITE_PARITY_461_DML_WHERE_ORDER_LIMIT_074.rs) | P1 | memory | GEN_SQL_DML | 1606673 | 1491664 | <span style="color:#2563eb">50.28%</span> |
| 1020 | [00081 BLOBS_HEX_ZEROBLOB](crates/bench/sqlite_parity/cases/SQLITE_PARITY_081_BLOBS_HEX_ZEROBLOB.rs) | P0 | memory | SQL_FUNCTIONS | 1460936 | 1491224 | <span style="color:#2563eb">50.29%</span> |
| 1021 | [01071 INDEX_SCHEMA_PRAGMA_004](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1071_INDEX_SCHEMA_PRAGMA_004.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1672608 | 1490984 | <span style="color:#2563eb">50.30%</span> |
| 1022 | [00630 JOIN_SUBQUERY_EXISTS_023](crates/bench/sqlite_parity/cases/SQLITE_PARITY_630_JOIN_SUBQUERY_EXISTS_023.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1696082 | 1490803 | <span style="color:#2563eb">50.31%</span> |
| 1023 | [00778 CTE_RECURSIVE_MATRIX_071](crates/bench/sqlite_parity/cases/SQLITE_PARITY_778_CTE_RECURSIVE_MATRIX_071.rs) | P1 | memory | GEN_SQL_CTE | 1546589 | 1490743 | <span style="color:#2563eb">50.31%</span> |
| 1024 | [00198 OPT_LOOKASIDE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_198_OPT_LOOKASIDE.rs) | P3 | memory | CLI_OPTION | 1467438 | 1490713 | <span style="color:#2563eb">50.31%</span> |
| 1025 | [01055 JSON_EXTRACT_SET_048](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1055_JSON_EXTRACT_SET_048.rs) | P2 | memory | GEN_SQL_JSON | 1543483 | 1490693 | <span style="color:#2563eb">50.31%</span> |
| 1026 | [00213 SQL_WAL_CHECKPOINT_TEMPFILE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_213_SQL_WAL_CHECKPOINT_TEMPFILE.rs) | P2 | tempfile | SQL_TEMPFILE | 1725567 | 1490403 | <span style="color:#2563eb">50.32%</span> |
| 1027 | [01103 INDEX_SCHEMA_PRAGMA_036](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1103_INDEX_SCHEMA_PRAGMA_036.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1697665 | 1490172 | <span style="color:#2563eb">50.33%</span> |
| 1028 | [00732 CTE_RECURSIVE_MATRIX_025](crates/bench/sqlite_parity/cases/SQLITE_PARITY_732_CTE_RECURSIVE_MATRIX_025.rs) | P1 | memory | GEN_SQL_CTE | 1590752 | 1490162 | <span style="color:#2563eb">50.33%</span> |
| 1029 | [00189 OPT_ECHO](crates/bench/sqlite_parity/cases/SQLITE_PARITY_189_OPT_ECHO.rs) | P2 | memory | CLI_OPTION | 1438073 | 1490031 | <span style="color:#2563eb">50.33%</span> |
| 1030 | [00411 DML_WHERE_ORDER_LIMIT_024](crates/bench/sqlite_parity/cases/SQLITE_PARITY_411_DML_WHERE_ORDER_LIMIT_024.rs) | P1 | memory | GEN_SQL_DML | 1599159 | 1489942 | <span style="color:#2563eb">50.34%</span> |
| 1031 | [00051 PRAGMA_INDEX_LIST_FUNCTION](crates/bench/sqlite_parity/cases/SQLITE_PARITY_051_PRAGMA_INDEX_LIST_FUNCTION.rs) | P0 | memory | SQL_PRAGMA | 1931588 | 1489611 | <span style="color:#2563eb">50.35%</span> |
| 1032 | [00071 BETWEEN_IN_ISNULL_IS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_071_BETWEEN_IN_ISNULL_IS.rs) | P0 | memory | SQL_OPERATORS | 1534005 | 1489461 | <span style="color:#2563eb">50.35%</span> |
| 1033 | [00728 CTE_RECURSIVE_MATRIX_021](crates/bench/sqlite_parity/cases/SQLITE_PARITY_728_CTE_RECURSIVE_MATRIX_021.rs) | P1 | memory | GEN_SQL_CTE | 1538925 | 1489401 | <span style="color:#2563eb">50.35%</span> |
| 1034 | [00471 DML_WHERE_ORDER_LIMIT_084](crates/bench/sqlite_parity/cases/SQLITE_PARITY_471_DML_WHERE_ORDER_LIMIT_084.rs) | P1 | memory | GEN_SQL_DML | 1604569 | 1489260 | <span style="color:#2563eb">50.36%</span> |
| 1035 | [00007 WITHOUT_ROWID_TABLE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_007_WITHOUT_ROWID_TABLE.rs) | P0 | memory | SQL_ROWID | 1607645 | 1488959 | <span style="color:#2563eb">50.37%</span> |
| 1036 | [01042 JSON_EXTRACT_SET_035](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1042_JSON_EXTRACT_SET_035.rs) | P2 | memory | GEN_SQL_JSON | 1581986 | 1488068 | <span style="color:#2563eb">50.40%</span> |
| 1037 | [00036 CREATE_VIEW](crates/bench/sqlite_parity/cases/SQLITE_PARITY_036_CREATE_VIEW.rs) | P0 | memory | SQL_VIEW | 2059100 | 1488048 | <span style="color:#2563eb">50.40%</span> |
| 1038 | [00895 CONSTRAINT_FK_SAVEPOINT_028](crates/bench/sqlite_parity/cases/SQLITE_PARITY_895_CONSTRAINT_FK_SAVEPOINT_028.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1787285 | 1487767 | <span style="color:#2563eb">50.41%</span> |
| 1039 | [01011 JSON_EXTRACT_SET_004](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1011_JSON_EXTRACT_SET_004.rs) | P2 | memory | GEN_SQL_JSON | 1852468 | 1487608 | <span style="color:#2563eb">50.41%</span> |
| 1040 | [00114 DOT_PRINT](crates/bench/sqlite_parity/cases/SQLITE_PARITY_114_DOT_PRINT.rs) | P0 | memory | CLI_DOT_COMMAND | 1460776 | 1486495 | <span style="color:#2563eb">50.45%</span> |
| 1041 | [00843 WINDOW_PARTITION_SUM_056](crates/bench/sqlite_parity/cases/SQLITE_PARITY_843_WINDOW_PARTITION_SUM_056.rs) | P2 | memory | GEN_SQL_WINDOW | 1575905 | 1486054 | <span style="color:#2563eb">50.46%</span> |
| 1042 | [00462 DML_WHERE_ORDER_LIMIT_075](crates/bench/sqlite_parity/cases/SQLITE_PARITY_462_DML_WHERE_ORDER_LIMIT_075.rs) | P1 | memory | GEN_SQL_DML | 1581735 | 1485954 | <span style="color:#2563eb">50.47%</span> |
| 1043 | [00122 DOT_CHANGES](crates/bench/sqlite_parity/cases/SQLITE_PARITY_122_DOT_CHANGES.rs) | P0 | memory | CLI_DOT_COMMAND | 1534606 | 1485934 | <span style="color:#2563eb">50.47%</span> |
| 1044 | [01090 INDEX_SCHEMA_PRAGMA_023](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1090_INDEX_SCHEMA_PRAGMA_023.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1681625 | 1485704 | <span style="color:#2563eb">50.48%</span> |
| 1045 | [00202 OPT_APPEND_TEMPFILE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_202_OPT_APPEND_TEMPFILE.rs) | P4 | tempfile | CLI_OPTION_TEMPFILE | 1472438 | 1485042 | <span style="color:#2563eb">50.50%</span> |
| 1046 | [00239 SCALAR_NULL_COALESCE_003](crates/bench/sqlite_parity/cases/SQLITE_PARITY_239_SCALAR_NULL_COALESCE_003.rs) | P1 | memory | GEN_SQL_SCALAR | 1480624 | 1484712 | <span style="color:#2563eb">50.51%</span> |
| 1047 | [01067 JSON_EXTRACT_SET_060](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1067_JSON_EXTRACT_SET_060.rs) | P2 | memory | GEN_SQL_JSON | 1550827 | 1484371 | <span style="color:#2563eb">50.52%</span> |
| 1048 | [01056 JSON_EXTRACT_SET_049](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1056_JSON_EXTRACT_SET_049.rs) | P2 | memory | GEN_SQL_JSON | 1569022 | 1484151 | <span style="color:#2563eb">50.53%</span> |
| 1049 | [00607 AGG_GROUP_HAVING_100](crates/bench/sqlite_parity/cases/SQLITE_PARITY_607_AGG_GROUP_HAVING_100.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1668049 | 1484110 | <span style="color:#2563eb">50.53%</span> |
| 1050 | [00326 SCALAR_STRING_025](crates/bench/sqlite_parity/cases/SQLITE_PARITY_326_SCALAR_STRING_025.rs) | P1 | memory | GEN_SQL_SCALAR | 1429167 | 1483279 | <span style="color:#2563eb">50.56%</span> |
| 1051 | [00195 OPT_SAFE_MODE_BLOCKS_SHELL](crates/bench/sqlite_parity/cases/SQLITE_PARITY_195_OPT_SAFE_MODE_BLOCKS_SHELL.rs) | P2 | memory | CLI_OPTION_NEGATIVE | 1449405 | 1483259 | <span style="color:#2563eb">50.56%</span> |
| 1052 | [00317 SCALAR_CAST_TYPEOF_023](crates/bench/sqlite_parity/cases/SQLITE_PARITY_317_SCALAR_CAST_TYPEOF_023.rs) | P1 | memory | GEN_SQL_SCALAR | 1468281 | 1483259 | <span style="color:#2563eb">50.56%</span> |
| 1053 | [00736 CTE_RECURSIVE_MATRIX_029](crates/bench/sqlite_parity/cases/SQLITE_PARITY_736_CTE_RECURSIVE_MATRIX_029.rs) | P1 | memory | GEN_SQL_CTE | 2602628 | 1483209 | <span style="color:#2563eb">50.56%</span> |
| 1054 | [00340 SCALAR_ARITH_029](crates/bench/sqlite_parity/cases/SQLITE_PARITY_340_SCALAR_ARITH_029.rs) | P1 | memory | GEN_SQL_SCALAR | 1739344 | 1483169 | <span style="color:#2563eb">50.56%</span> |
| 1055 | [00610 JOIN_SUBQUERY_EXISTS_003](crates/bench/sqlite_parity/cases/SQLITE_PARITY_610_JOIN_SUBQUERY_EXISTS_003.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1686705 | 1482688 | <span style="color:#2563eb">50.58%</span> |
| 1056 | [00910 CONSTRAINT_FK_SAVEPOINT_043](crates/bench/sqlite_parity/cases/SQLITE_PARITY_910_CONSTRAINT_FK_SAVEPOINT_043.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1629917 | 1482327 | <span style="color:#2563eb">50.59%</span> |
| 1057 | [00139 DOT_LINT_FKEY_INDEXES](crates/bench/sqlite_parity/cases/SQLITE_PARITY_139_DOT_LINT_FKEY_INDEXES.rs) | P3 | memory | CLI_DOT_COMMAND_DIAGNOSTIC | 1662258 | 1482297 | <span style="color:#2563eb">50.59%</span> |
| 1058 | [00125 DOT_TIMER](crates/bench/sqlite_parity/cases/SQLITE_PARITY_125_DOT_TIMER.rs) | P3 | memory | CLI_DOT_COMMAND_DIAGNOSTIC | 1465255 | 1482117 | <span style="color:#2563eb">50.60%</span> |
| 1059 | [00373 SCALAR_CAST_TYPEOF_037](crates/bench/sqlite_parity/cases/SQLITE_PARITY_373_SCALAR_CAST_TYPEOF_037.rs) | P1 | memory | GEN_SQL_SCALAR | 1449505 | 1482077 | <span style="color:#2563eb">50.60%</span> |
| 1060 | [00534 AGG_GROUP_HAVING_027](crates/bench/sqlite_parity/cases/SQLITE_PARITY_534_AGG_GROUP_HAVING_027.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1980651 | 1481525 | <span style="color:#2563eb">50.62%</span> |
| 1061 | [00331 SCALAR_NULL_COALESCE_026](crates/bench/sqlite_parity/cases/SQLITE_PARITY_331_SCALAR_NULL_COALESCE_026.rs) | P1 | memory | GEN_SQL_SCALAR | 1428516 | 1481395 | <span style="color:#2563eb">50.62%</span> |
| 1062 | [00561 AGG_GROUP_HAVING_054](crates/bench/sqlite_parity/cases/SQLITE_PARITY_561_AGG_GROUP_HAVING_054.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1692415 | 1480574 | <span style="color:#2563eb">50.65%</span> |
| 1063 | [01039 JSON_EXTRACT_SET_032](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1039_JSON_EXTRACT_SET_032.rs) | P2 | memory | GEN_SQL_JSON | 1559754 | 1480153 | <span style="color:#2563eb">50.66%</span> |
| 1064 | [00486 DML_WHERE_ORDER_LIMIT_099](crates/bench/sqlite_parity/cases/SQLITE_PARITY_486_DML_WHERE_ORDER_LIMIT_099.rs) | P1 | memory | GEN_SQL_DML | 1609578 | 1479853 | <span style="color:#2563eb">50.67%</span> |
| 1065 | [01022 JSON_EXTRACT_SET_015](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1022_JSON_EXTRACT_SET_015.rs) | P2 | memory | GEN_SQL_JSON | 1562940 | 1479372 | <span style="color:#2563eb">50.69%</span> |
| 1066 | [00054 JOINS_INNER_LEFT_CROSS_NATURAL](crates/bench/sqlite_parity/cases/SQLITE_PARITY_054_JOINS_INNER_LEFT_CROSS_NATURAL.rs) | P0 | memory | SQL_JOIN | 1713014 | 1479371 | <span style="color:#2563eb">50.69%</span> |
| 1067 | [00205 OPT_VFS_CATALOG](crates/bench/sqlite_parity/cases/SQLITE_PARITY_205_OPT_VFS_CATALOG.rs) | P4 | catalog | CLI_OPTION_CATALOG | 1529046 | 1478761 | <span style="color:#2563eb">50.71%</span> |
| 1068 | [00849 WINDOW_PARTITION_SUM_062](crates/bench/sqlite_parity/cases/SQLITE_PARITY_849_WINDOW_PARTITION_SUM_062.rs) | P2 | memory | GEN_SQL_WINDOW | 1771044 | 1478640 | <span style="color:#2563eb">50.71%</span> |
| 1069 | [00047 PRAGMA_FOREIGN_KEYS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_047_PRAGMA_FOREIGN_KEYS.rs) | P0 | memory | SQL_PRAGMA | 1404400 | 1477638 | <span style="color:#2563eb">50.75%</span> |
| 1070 | [00120 DOT_EXPLAIN](crates/bench/sqlite_parity/cases/SQLITE_PARITY_120_DOT_EXPLAIN.rs) | P0 | memory | CLI_DOT_COMMAND | 1412315 | 1477288 | <span style="color:#2563eb">50.76%</span> |
| 1071 | [00926 CONSTRAINT_FK_SAVEPOINT_059](crates/bench/sqlite_parity/cases/SQLITE_PARITY_926_CONSTRAINT_FK_SAVEPOINT_059.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1663410 | 1477127 | <span style="color:#2563eb">50.76%</span> |
| 1072 | [00953 VIEW_TRIGGER_GENERATED_006](crates/bench/sqlite_parity/cases/SQLITE_PARITY_953_VIEW_TRIGGER_GENERATED_006.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1696072 | 1476616 | <span style="color:#2563eb">50.78%</span> |
| 1073 | [00137 DOT_VERSION](crates/bench/sqlite_parity/cases/SQLITE_PARITY_137_DOT_VERSION.rs) | P2 | memory | CLI_DOT_COMMAND_DIAGNOSTIC | 1445848 | 1476526 | <span style="color:#2563eb">50.78%</span> |
| 1074 | [00836 WINDOW_PARTITION_SUM_049](crates/bench/sqlite_parity/cases/SQLITE_PARITY_836_WINDOW_PARTITION_SUM_049.rs) | P2 | memory | GEN_SQL_WINDOW | 1644955 | 1476396 | <span style="color:#2563eb">50.79%</span> |
| 1075 | [00939 CONSTRAINT_FK_SAVEPOINT_072](crates/bench/sqlite_parity/cases/SQLITE_PARITY_939_CONSTRAINT_FK_SAVEPOINT_072.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1655765 | 1475675 | <span style="color:#2563eb">50.81%</span> |
| 1076 | [01118 INDEX_SCHEMA_PRAGMA_051](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1118_INDEX_SCHEMA_PRAGMA_051.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1687255 | 1475594 | <span style="color:#2563eb">50.81%</span> |
| 1077 | [01003 VIEW_TRIGGER_GENERATED_056](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1003_VIEW_TRIGGER_GENERATED_056.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1737341 | 1475495 | <span style="color:#2563eb">50.82%</span> |
| 1078 | [00154 DOT_DBINFO_TEMPFILE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_154_DOT_DBINFO_TEMPFILE.rs) | P3 | tempfile | CLI_TEMPFILE_DIAGNOSTIC | 1724495 | 1475334 | <span style="color:#2563eb">50.82%</span> |
| 1079 | [00334 SCALAR_STRING_027](crates/bench/sqlite_parity/cases/SQLITE_PARITY_334_SCALAR_STRING_027.rs) | P1 | memory | GEN_SQL_SCALAR | 1449044 | 1475163 | <span style="color:#2563eb">50.83%</span> |
| 1080 | [00132 DOT_TRACE_STDOUT](crates/bench/sqlite_parity/cases/SQLITE_PARITY_132_DOT_TRACE_STDOUT.rs) | P2 | memory | CLI_DOT_COMMAND_DIAGNOSTIC | 1446369 | 1474362 | <span style="color:#2563eb">50.85%</span> |
| 1081 | [00844 WINDOW_PARTITION_SUM_057](crates/bench/sqlite_parity/cases/SQLITE_PARITY_844_WINDOW_PARTITION_SUM_057.rs) | P2 | memory | GEN_SQL_WINDOW | 1598567 | 1474192 | <span style="color:#2563eb">50.86%</span> |
| 1082 | [00166 DOT_SESSION_CATALOG](crates/bench/sqlite_parity/cases/SQLITE_PARITY_166_DOT_SESSION_CATALOG.rs) | P4 | catalog | CLI_CATALOG | 1452541 | 1473811 | <span style="color:#2563eb">50.87%</span> |
| 1083 | [01083 INDEX_SCHEMA_PRAGMA_016](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1083_INDEX_SCHEMA_PRAGMA_016.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1707293 | 1471837 | <span style="color:#2563eb">50.94%</span> |
| 1084 | [00254 SCALAR_STRING_007](crates/bench/sqlite_parity/cases/SQLITE_PARITY_254_SCALAR_STRING_007.rs) | P1 | memory | GEN_SQL_SCALAR | 1467950 | 1471616 | <span style="color:#2563eb">50.95%</span> |
| 1085 | [00300 SCALAR_ARITH_019](crates/bench/sqlite_parity/cases/SQLITE_PARITY_300_SCALAR_ARITH_019.rs) | P1 | memory | GEN_SQL_SCALAR | 1500722 | 1471607 | <span style="color:#2563eb">50.95%</span> |
| 1086 | [00191 OPT_BATCH](crates/bench/sqlite_parity/cases/SQLITE_PARITY_191_OPT_BATCH.rs) | P2 | memory | CLI_OPTION | 1492587 | 1471547 | <span style="color:#2563eb">50.95%</span> |
| 1087 | [00131 DOT_TIMEOUT](crates/bench/sqlite_parity/cases/SQLITE_PARITY_131_DOT_TIMEOUT.rs) | P0 | memory | CLI_DOT_COMMAND | 1681444 | 1470584 | <span style="color:#2563eb">50.98%</span> |
| 1088 | [00199 OPT_PAGECACHE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_199_OPT_PAGECACHE.rs) | P3 | memory | CLI_OPTION | 1451889 | 1469623 | <span style="color:#2563eb">51.01%</span> |
| 1089 | [01068 INDEX_SCHEMA_PRAGMA_001](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1068_INDEX_SCHEMA_PRAGMA_001.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1691343 | 1468531 | <span style="color:#2563eb">51.05%</span> |
| 1090 | [00135 DOT_PROGRESS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_135_DOT_PROGRESS.rs) | P0 | memory | CLI_DOT_COMMAND | 1485273 | 1466397 | <span style="color:#2563eb">51.12%</span> |
| 1091 | [00223 OPT_NOHEADER](crates/bench/sqlite_parity/cases/SQLITE_PARITY_223_OPT_NOHEADER.rs) | P2 | memory | CLI_OPTION | 1455357 | 1466397 | <span style="color:#2563eb">51.12%</span> |
| 1092 | [00200 OPT_HEAP](crates/bench/sqlite_parity/cases/SQLITE_PARITY_200_OPT_HEAP.rs) | P4 | memory | CLI_OPTION | 1427414 | 1466067 | <span style="color:#2563eb">51.13%</span> |
| 1093 | [01045 JSON_EXTRACT_SET_038](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1045_JSON_EXTRACT_SET_038.rs) | P2 | memory | GEN_SQL_JSON | 1797794 | 1465716 | <span style="color:#2563eb">51.14%</span> |
| 1094 | [00604 AGG_GROUP_HAVING_097](crates/bench/sqlite_parity/cases/SQLITE_PARITY_604_AGG_GROUP_HAVING_097.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1649293 | 1465455 | <span style="color:#2563eb">51.15%</span> |
| 1095 | [00858 WINDOW_PARTITION_SUM_071](crates/bench/sqlite_parity/cases/SQLITE_PARITY_858_WINDOW_PARTITION_SUM_071.rs) | P2 | memory | GEN_SQL_WINDOW | 1589360 | 1465004 | <span style="color:#2563eb">51.17%</span> |
| 1096 | [01121 INDEX_SCHEMA_PRAGMA_054](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1121_INDEX_SCHEMA_PRAGMA_054.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1677748 | 1464043 | <span style="color:#2563eb">51.20%</span> |
| 1097 | [00827 WINDOW_PARTITION_SUM_040](crates/bench/sqlite_parity/cases/SQLITE_PARITY_827_WINDOW_PARTITION_SUM_040.rs) | P2 | memory | GEN_SQL_WINDOW | 1647240 | 1462439 | <span style="color:#2563eb">51.25%</span> |
| 1098 | [01111 INDEX_SCHEMA_PRAGMA_044](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1111_INDEX_SCHEMA_PRAGMA_044.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 2092232 | 1462289 | <span style="color:#2563eb">51.26%</span> |
| 1099 | [00737 CTE_RECURSIVE_MATRIX_030](crates/bench/sqlite_parity/cases/SQLITE_PARITY_737_CTE_RECURSIVE_MATRIX_030.rs) | P1 | memory | GEN_SQL_CTE | 1488098 | 1461678 | <span style="color:#2563eb">51.28%</span> |
| 1100 | [00206 OPT_MEMTRACE_CATALOG](crates/bench/sqlite_parity/cases/SQLITE_PARITY_206_OPT_MEMTRACE_CATALOG.rs) | P4 | catalog | CLI_OPTION_CATALOG | 1986872 | 1461558 | <span style="color:#2563eb">51.28%</span> |
| 1101 | [00174 OPT_CSV_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_174_OPT_CSV_MODE.rs) | P1 | memory | CLI_OPTION | 1801011 | 1460787 | <span style="color:#2563eb">51.31%</span> |
| 1102 | [00579 AGG_GROUP_HAVING_072](crates/bench/sqlite_parity/cases/SQLITE_PARITY_579_AGG_GROUP_HAVING_072.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2144962 | 1460536 | <span style="color:#2563eb">51.32%</span> |
| 1103 | [01076 INDEX_SCHEMA_PRAGMA_009](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1076_INDEX_SCHEMA_PRAGMA_009.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1845985 | 1459765 | <span style="color:#2563eb">51.34%</span> |
| 1104 | [00665 JOIN_SUBQUERY_EXISTS_058](crates/bench/sqlite_parity/cases/SQLITE_PARITY_665_JOIN_SUBQUERY_EXISTS_058.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1677857 | 1459384 | <span style="color:#2563eb">51.35%</span> |
| 1105 | [00118 DOT_FULLSCHEMA](crates/bench/sqlite_parity/cases/SQLITE_PARITY_118_DOT_FULLSCHEMA.rs) | P0 | memory | CLI_DOT_COMMAND | 1582446 | 1459364 | <span style="color:#2563eb">51.35%</span> |
| 1106 | [00207 OPT_PCACHETRACE_CATALOG](crates/bench/sqlite_parity/cases/SQLITE_PARITY_207_OPT_PCACHETRACE_CATALOG.rs) | P4 | catalog | CLI_OPTION_CATALOG | 1467459 | 1457711 | <span style="color:#2563eb">51.41%</span> |
| 1107 | [00142 DOT_EXIT_CODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_142_DOT_EXIT_CODE.rs) | P0 | memory | CLI_DOT_COMMAND | 1395142 | 1456778 | <span style="color:#2563eb">51.44%</span> |
| 1108 | [00076 EXPLAIN_BYTECODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_076_EXPLAIN_BYTECODE.rs) | P0 | memory | SQL_EXPLAIN | 1483971 | 1456429 | <span style="color:#2563eb">51.45%</span> |
| 1109 | [00108 DOT_MODE_CSV_AND_QUOTE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_108_DOT_MODE_CSV_AND_QUOTE.rs) | P0 | memory | CLI_DOT_COMMAND | 1501834 | 1455797 | <span style="color:#2563eb">51.47%</span> |
| 1110 | [00116 DOT_DATABASES](crates/bench/sqlite_parity/cases/SQLITE_PARITY_116_DOT_DATABASES.rs) | P0 | memory | CLI_DOT_COMMAND | 1454395 | 1454815 | <span style="color:#2563eb">51.51%</span> |
| 1111 | [00075 EXPLAIN_QUERY_PLAN](crates/bench/sqlite_parity/cases/SQLITE_PARITY_075_EXPLAIN_QUERY_PLAN.rs) | P0 | memory | SQL_EXPLAIN | 2209955 | 1453362 | <span style="color:#2563eb">51.55%</span> |
| 1112 | [00070 LIKE_GLOB_MATCH_ESCAPE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_070_LIKE_GLOB_MATCH_ESCAPE.rs) | P0 | memory | SQL_OPERATORS | 1460075 | 1451950 | <span style="color:#2563eb">51.60%</span> |
| 1113 | [00155 DOT_DBTOTXT_TEMPFILE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_155_DOT_DBTOTXT_TEMPFILE.rs) | P3 | tempfile | CLI_TEMPFILE | 1788167 | 1451739 | <span style="color:#2563eb">51.61%</span> |
| 1114 | [00145 DOT_SCANSTATS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_145_DOT_SCANSTATS.rs) | P3 | memory | CLI_DOT_COMMAND_DIAGNOSTIC | 1562689 | 1449095 | <span style="color:#2563eb">51.70%</span> |
| 1115 | [00117 DOT_DUMP](crates/bench/sqlite_parity/cases/SQLITE_PARITY_117_DOT_DUMP.rs) | P0 | memory | CLI_DOT_COMMAND | 1640397 | 1446750 | <span style="color:#2563eb">51.78%</span> |
| 1116 | [00119 DOT_EQP](crates/bench/sqlite_parity/cases/SQLITE_PARITY_119_DOT_EQP.rs) | P0 | memory | CLI_DOT_COMMAND | 1531360 | 1437171 | <span style="color:#2563eb">52.09%</span> |
| 1117 | [00138 DOT_VFSNAME_LIST_INFO](crates/bench/sqlite_parity/cases/SQLITE_PARITY_138_DOT_VFSNAME_LIST_INFO.rs) | P3 | memory | CLI_DOT_COMMAND_DIAGNOSTIC | 1452210 | 1437141 | <span style="color:#2563eb">52.10%</span> |
| 1118 | [00180 OPT_BOX_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_180_OPT_BOX_MODE.rs) | P2 | memory | CLI_OPTION | 1697985 | 1435859 | <span style="color:#2563eb">52.14%</span> |
| 1119 | [00224 OPT_STATS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_224_OPT_STATS.rs) | P3 | memory | CLI_OPTION_DIAGNOSTIC | 1468441 | 1431281 | <span style="color:#2563eb">52.29%</span> |
| 1120 | [00176 OPT_LINE_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_176_OPT_LINE_MODE.rs) | P2 | memory | CLI_OPTION | 1475855 | 1418496 | <span style="color:#2563eb">52.72%</span> |
| 1121 | [00182 OPT_COLUMN_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_182_OPT_COLUMN_MODE.rs) | P2 | memory | CLI_OPTION | 1473250 | 1417394 | <span style="color:#2563eb">52.75%</span> |
| 1122 | [00222 OPT_ESCAPE_SYMBOL](crates/bench/sqlite_parity/cases/SQLITE_PARITY_222_OPT_ESCAPE_SYMBOL.rs) | P3 | memory | CLI_OPTION | 1428175 | 1403557 | <span style="color:#2563eb">53.21%</span> |
| 1123 | [00143 DOT_QUIT](crates/bench/sqlite_parity/cases/SQLITE_PARITY_143_DOT_QUIT.rs) | P0 | memory | CLI_DOT_COMMAND | 1381786 | 1397647 | <span style="color:#2563eb">53.41%</span> |
| 1124 | [00170 OPT_VERSION](crates/bench/sqlite_parity/cases/SQLITE_PARITY_170_OPT_VERSION.rs) | P1 | memory | CLI_OPTION | 1343725 | 1389271 | <span style="color:#2563eb">53.69%</span> |
| 1125 | [00161 DOT_WWW_EXTERNAL_APP](crates/bench/sqlite_parity/cases/SQLITE_PARITY_161_DOT_WWW_EXTERNAL_APP.rs) | P4 | external_app | CLI_EXTERNAL_APP | 31013452 | 1595040 | <span style="color:#2563eb">94.86%</span> |
| 1126 | [00160 DOT_EXCEL_EXTERNAL_APP](crates/bench/sqlite_parity/cases/SQLITE_PARITY_160_DOT_EXCEL_EXTERNAL_APP.rs) | P4 | external_app | CLI_EXTERNAL_APP | 32315778 | 1518075 | <span style="color:#2563eb">95.30%</span> |
| 1127 | [00209 OPT_INTERACTIVE_CATALOG](crates/bench/sqlite_parity/cases/SQLITE_PARITY_209_OPT_INTERACTIVE_CATALOG.rs) | P4 | catalog | CLI_OPTION_CATALOG | 51803632 | 2015877 | <span style="color:#2563eb">96.11%</span> |

</details>

<!-- sqlite-parity-report:end -->

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
