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
  <img src="https://img.shields.io/badge/version-1.0.24-blue" alt="version">
  <!-- jankurai-score-badge:begin -->
  <a href=".jankurai/repo-score.md"><img src="https://img.shields.io/badge/jankurai-64%2F100%20advisory-orange" alt="jankurai score: 64/100 advisory"></a>
  <!-- jankurai-score-badge:end -->
</p>

RedlineDB is an embedded SQL engine written in Rust. It keeps the SQLite-facing
API familiar while replacing the storage core with MVCC, a concurrent B-tree,
group-commit WAL, and crash recovery designed for multi-writer workloads.

## Engine Metrics

<!-- sqlite-parity-metrics:begin -->

![SQLite vs RedlineDB production KSLOC chart](assets/sqlite-parity-ksloc.svg)

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
redlinedb = "=1.0.24"
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
curl -LsSf https://raw.githubusercontent.com/neverhuman/RedlineDB/main/scripts/install.sh | VERSION=v1.0.24 bash
```

Lock the exact tarball digest in CI or release automation:

```bash
curl -LsSf https://raw.githubusercontent.com/neverhuman/RedlineDB/main/scripts/install.sh | \
  VERSION=v1.0.24 REDLINEDB_SHA256=<sha256> bash
```

### Build from source

```bash
cargo install redlinedb-cli --version 1.0.24 --locked
```

Or install from the tagged repository release:

```bash
cargo install --git https://github.com/neverhuman/RedlineDB.git --tag v1.0.24 --package redlinedb-cli --locked
```

### Direct download

Release tarballs are published on the [releases page](https://github.com/neverhuman/RedlineDB/releases):

| Platform | File |
|---|---|
| Linux x86_64 | `redlinedb-v1.0.24-linux-x86_64.tar.gz` |
| macOS Apple Silicon | `redlinedb-v1.0.24-macos-arm64.tar.gz` |
| macOS Intel | `redlinedb-v1.0.24-macos-x86_64.tar.gz` |

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

**SQLite parity latency:** median gap **47.90%**, worst gap **-31.48%**, faster cases **1117** with a **3000000 ns** reference floor (targets: median >= -25%, worst > -75%, faster >= 25).

![SQLite parity latency improvement plot](assets/sqlite-parity-latency-gap.svg)

[Full ranked latency table](#sqlite-parity-ranked-latency-table) is collapsed below for README readability.

<details id="sqlite-parity-ranked-latency-table">
<summary>Full ranked latency table</summary>

| Rank | Case | Priority | Profile | Category | SQLite median ns | RedlineDB median ns | Improvement |
| ---: | --- | --- | --- | --- | ---: | ---: | ---: |
| 1 | [00168 DOT_CHECK_CATALOG](crates/bench/sqlite_parity/cases/SQLITE_PARITY_168_DOT_CHECK_CATALOG.rs) | P4 | catalog | CLI_CATALOG | 2346253 | 3944479 | <span style="color:#dc2626">-31.48%</span> |
| 2 | [00032 UNIQUE_INDEX_FAILURE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_032_UNIQUE_INDEX_FAILURE.rs) | P0 | memory | SQL_INDEX_NEGATIVE | 1647339 | 3727298 | <span style="color:#dc2626">-24.24%</span> |
| 3 | [00005 UNIQUE_CONSTRAINT_FAILURE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_005_UNIQUE_CONSTRAINT_FAILURE.rs) | P0 | memory | SQL_CONSTRAINTS_NEGATIVE | 1643652 | 3722388 | <span style="color:#dc2626">-24.08%</span> |
| 4 | [00210 OPT_NOUNICODE_UTF8_CATALOG](crates/bench/sqlite_parity/cases/SQLITE_PARITY_210_OPT_NOUNICODE_UTF8_CATALOG.rs) | P4 | catalog | CLI_OPTION_CATALOG | 1568300 | 3620596 | <span style="color:#dc2626">-20.69%</span> |
| 5 | [00027 FOREIGN_KEY_FAILURE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_027_FOREIGN_KEY_FAILURE.rs) | P0 | memory | SQL_FOREIGN_KEYS_NEGATIVE | 1762297 | 3552337 | <span style="color:#dc2626">-18.41%</span> |
| 6 | [00079 CHECK_CONSTRAINT_FAILURE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_079_CHECK_CONSTRAINT_FAILURE.rs) | P0 | memory | SQL_CONSTRAINTS_NEGATIVE | 1535618 | 3452076 | <span style="color:#dc2626">-15.07%</span> |
| 7 | [00009 STRICT_TABLE_TYPE_FAILURE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_009_STRICT_TABLE_TYPE_FAILURE.rs) | P0 | memory | SQL_DDL_NEGATIVE | 1914335 | 3403275 | <span style="color:#dc2626">-13.44%</span> |
| 8 | [00080 NOT_NULL_FAILURE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_080_NOT_NULL_FAILURE.rs) | P0 | memory | SQL_CONSTRAINTS_NEGATIVE | 1880100 | 3243913 | <span style="color:#dc2626">-8.13%</span> |
| 9 | [00206 OPT_MEMTRACE_CATALOG](crates/bench/sqlite_parity/cases/SQLITE_PARITY_206_OPT_MEMTRACE_CATALOG.rs) | P4 | catalog | CLI_OPTION_CATALOG | 2385728 | 3054274 | <span style="color:#f97316">-1.81%</span> |
| 10 | [00205 OPT_VFS_CATALOG](crates/bench/sqlite_parity/cases/SQLITE_PARITY_205_OPT_VFS_CATALOG.rs) | P4 | catalog | CLI_OPTION_CATALOG | 1519567 | 3040959 | <span style="color:#f97316">-1.37%</span> |
| 11 | [00167 DOT_UNMODULE_CATALOG](crates/bench/sqlite_parity/cases/SQLITE_PARITY_167_DOT_UNMODULE_CATALOG.rs) | P4 | catalog | CLI_CATALOG | 1480483 | 2975054 | <span style="color:#16a34a">0.83%</span> |
| 12 | [00746 CTE_RECURSIVE_MATRIX_039](crates/bench/sqlite_parity/cases/SQLITE_PARITY_746_CTE_RECURSIVE_MATRIX_039.rs) | P1 | memory | GEN_SQL_CTE | 1668520 | 2921262 | <span style="color:#16a34a">2.62%</span> |
| 13 | [00207 OPT_PCACHETRACE_CATALOG](crates/bench/sqlite_parity/cases/SQLITE_PARITY_207_OPT_PCACHETRACE_CATALOG.rs) | P4 | catalog | CLI_OPTION_CATALOG | 1556107 | 2842713 | <span style="color:#2563eb">5.24%</span> |
| 14 | [00227 OPT_UNSAFE_TESTING_CATALOG](crates/bench/sqlite_parity/cases/SQLITE_PARITY_227_OPT_UNSAFE_TESTING_CATALOG.rs) | P4 | catalog | CLI_OPTION_CATALOG | 1500151 | 2818587 | <span style="color:#2563eb">6.05%</span> |
| 15 | [00166 DOT_SESSION_CATALOG](crates/bench/sqlite_parity/cases/SQLITE_PARITY_166_DOT_SESSION_CATALOG.rs) | P4 | catalog | CLI_CATALOG | 1584190 | 2800553 | <span style="color:#2563eb">6.65%</span> |
| 16 | [00197 OPT_MAXSIZE_DESERIALIZE_TEMPFILE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_197_OPT_MAXSIZE_DESERIALIZE_TEMPFILE.rs) | P3 | tempfile | CLI_OPTION_TEMPFILE | 6399307 | 5953324 | <span style="color:#2563eb">6.97%</span> |
| 17 | [00163 DOT_FILECTRL_CATALOG](crates/bench/sqlite_parity/cases/SQLITE_PARITY_163_DOT_FILECTRL_CATALOG.rs) | P4 | catalog | CLI_CATALOG | 1472298 | 2771739 | <span style="color:#2563eb">7.61%</span> |
| 18 | [00193 OPT_READONLY_TEMPFILE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_193_OPT_READONLY_TEMPFILE.rs) | P2 | tempfile | CLI_OPTION_TEMPFILE | 7461499 | 6856224 | <span style="color:#2563eb">8.11%</span> |
| 19 | [00208 OPT_VFSTRACE_CATALOG](crates/bench/sqlite_parity/cases/SQLITE_PARITY_208_OPT_VFSTRACE_CATALOG.rs) | P4 | catalog | CLI_OPTION_CATALOG | 1466477 | 2746751 | <span style="color:#2563eb">8.44%</span> |
| 20 | [00164 DOT_IMPOSTER_CATALOG](crates/bench/sqlite_parity/cases/SQLITE_PARITY_164_DOT_IMPOSTER_CATALOG.rs) | P4 | catalog | CLI_CATALOG | 1480704 | 2688862 | <span style="color:#2563eb">10.37%</span> |
| 21 | [00762 CTE_RECURSIVE_MATRIX_055](crates/bench/sqlite_parity/cases/SQLITE_PARITY_762_CTE_RECURSIVE_MATRIX_055.rs) | P1 | memory | GEN_SQL_CTE | 1865893 | 2674354 | <span style="color:#2563eb">10.85%</span> |
| 22 | [00302 SCALAR_STRING_019](crates/bench/sqlite_parity/cases/SQLITE_PARITY_302_SCALAR_STRING_019.rs) | P1 | memory | GEN_SQL_SCALAR | 1523956 | 2669425 | <span style="color:#2563eb">11.02%</span> |
| 23 | [01105 INDEX_SCHEMA_PRAGMA_038](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1105_INDEX_SCHEMA_PRAGMA_038.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1798806 | 2616314 | <span style="color:#2563eb">12.79%</span> |
| 24 | [00389 DML_WHERE_ORDER_LIMIT_002](crates/bench/sqlite_parity/cases/SQLITE_PARITY_389_DML_WHERE_ORDER_LIMIT_002.rs) | P1 | memory | GEN_SQL_DML | 1673008 | 2612447 | <span style="color:#2563eb">12.92%</span> |
| 25 | [00195 OPT_SAFE_MODE_BLOCKS_SHELL](crates/bench/sqlite_parity/cases/SQLITE_PARITY_195_OPT_SAFE_MODE_BLOCKS_SHELL.rs) | P2 | memory | CLI_OPTION_NEGATIVE | 1426582 | 2601567 | <span style="color:#2563eb">13.28%</span> |
| 26 | [00172 OPT_CMD](crates/bench/sqlite_parity/cases/SQLITE_PARITY_172_OPT_CMD.rs) | P1 | memory | CLI_OPTION | 1567178 | 2595525 | <span style="color:#2563eb">13.48%</span> |
| 27 | [00292 SCALAR_ARITH_017](crates/bench/sqlite_parity/cases/SQLITE_PARITY_292_SCALAR_ARITH_017.rs) | P1 | memory | GEN_SQL_SCALAR | 1635216 | 2574775 | <span style="color:#2563eb">14.17%</span> |
| 28 | [00752 CTE_RECURSIVE_MATRIX_045](crates/bench/sqlite_parity/cases/SQLITE_PARITY_752_CTE_RECURSIVE_MATRIX_045.rs) | P1 | memory | GEN_SQL_CTE | 1927269 | 2571931 | <span style="color:#2563eb">14.27%</span> |
| 29 | [01097 INDEX_SCHEMA_PRAGMA_030](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1097_INDEX_SCHEMA_PRAGMA_030.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1752609 | 2531503 | <span style="color:#2563eb">15.62%</span> |
| 30 | [00725 CTE_RECURSIVE_MATRIX_018](crates/bench/sqlite_parity/cases/SQLITE_PARITY_725_CTE_RECURSIVE_MATRIX_018.rs) | P1 | memory | GEN_SQL_CTE | 1569482 | 2501637 | <span style="color:#2563eb">16.61%</span> |
| 31 | [00074 NOT_INDEXED](crates/bench/sqlite_parity/cases/SQLITE_PARITY_074_NOT_INDEXED.rs) | P0 | memory | SQL_INDEX | 2086641 | 2500325 | <span style="color:#2563eb">16.66%</span> |
| 32 | [00052 PRAGMA_INTEGRITY_QUICK_CHECK](crates/bench/sqlite_parity/cases/SQLITE_PARITY_052_PRAGMA_INTEGRITY_QUICK_CHECK.rs) | P0 | memory | SQL_PRAGMA | 1734214 | 2476509 | <span style="color:#2563eb">17.45%</span> |
| 33 | [01098 INDEX_SCHEMA_PRAGMA_031](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1098_INDEX_SCHEMA_PRAGMA_031.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1790020 | 2473824 | <span style="color:#2563eb">17.54%</span> |
| 34 | [00625 JOIN_SUBQUERY_EXISTS_018](crates/bench/sqlite_parity/cases/SQLITE_PARITY_625_JOIN_SUBQUERY_EXISTS_018.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1675423 | 2470358 | <span style="color:#2563eb">17.65%</span> |
| 35 | [00993 VIEW_TRIGGER_GENERATED_046](crates/bench/sqlite_parity/cases/SQLITE_PARITY_993_VIEW_TRIGGER_GENERATED_046.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1726199 | 2447465 | <span style="color:#2563eb">18.42%</span> |
| 36 | [00852 WINDOW_PARTITION_SUM_065](crates/bench/sqlite_parity/cases/SQLITE_PARITY_852_WINDOW_PARTITION_SUM_065.rs) | P2 | memory | GEN_SQL_WINDOW | 1667037 | 2440442 | <span style="color:#2563eb">18.65%</span> |
| 37 | [00247 SCALAR_NULL_COALESCE_005](crates/bench/sqlite_parity/cases/SQLITE_PARITY_247_SCALAR_NULL_COALESCE_005.rs) | P1 | memory | GEN_SQL_SCALAR | 1559032 | 2424762 | <span style="color:#2563eb">19.17%</span> |
| 38 | [00101 SCHEMA_SQLITE_MASTER_ALIAS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_101_SCHEMA_SQLITE_MASTER_ALIAS.rs) | P0 | memory | SQL_SCHEMA | 2011048 | 2416386 | <span style="color:#2563eb">19.45%</span> |
| 39 | [00728 CTE_RECURSIVE_MATRIX_021](crates/bench/sqlite_parity/cases/SQLITE_PARITY_728_CTE_RECURSIVE_MATRIX_021.rs) | P1 | memory | GEN_SQL_CTE | 1575864 | 2405285 | <span style="color:#2563eb">19.82%</span> |
| 40 | [00311 SCALAR_NULL_COALESCE_021](crates/bench/sqlite_parity/cases/SQLITE_PARITY_311_SCALAR_NULL_COALESCE_021.rs) | P1 | memory | GEN_SQL_SCALAR | 1548262 | 2403842 | <span style="color:#2563eb">19.87%</span> |
| 41 | [01058 JSON_EXTRACT_SET_051](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1058_JSON_EXTRACT_SET_051.rs) | P2 | memory | GEN_SQL_JSON | 1616211 | 2382882 | <span style="color:#2563eb">20.57%</span> |
| 42 | [00484 DML_WHERE_ORDER_LIMIT_097](crates/bench/sqlite_parity/cases/SQLITE_PARITY_484_DML_WHERE_ORDER_LIMIT_097.rs) | P1 | memory | GEN_SQL_DML | 1628524 | 2379737 | <span style="color:#2563eb">20.68%</span> |
| 43 | [00122 DOT_CHANGES](crates/bench/sqlite_parity/cases/SQLITE_PARITY_122_DOT_CHANGES.rs) | P0 | memory | CLI_DOT_COMMAND | 1551327 | 2378564 | <span style="color:#2563eb">20.71%</span> |
| 44 | [00308 SCALAR_ARITH_021](crates/bench/sqlite_parity/cases/SQLITE_PARITY_308_SCALAR_ARITH_021.rs) | P1 | memory | GEN_SQL_SCALAR | 1522133 | 2362143 | <span style="color:#2563eb">21.26%</span> |
| 45 | [00288 SCALAR_ARITH_016](crates/bench/sqlite_parity/cases/SQLITE_PARITY_288_SCALAR_ARITH_016.rs) | P1 | memory | GEN_SQL_SCALAR | 1445668 | 2324522 | <span style="color:#2563eb">22.52%</span> |
| 46 | [00750 CTE_RECURSIVE_MATRIX_043](crates/bench/sqlite_parity/cases/SQLITE_PARITY_750_CTE_RECURSIVE_MATRIX_043.rs) | P1 | memory | GEN_SQL_CTE | 1563030 | 2318149 | <span style="color:#2563eb">22.73%</span> |
| 47 | [00104 SELECT_DISTINCT](crates/bench/sqlite_parity/cases/SQLITE_PARITY_104_SELECT_DISTINCT.rs) | P0 | memory | SQL_SELECT | 1542852 | 2316356 | <span style="color:#2563eb">22.79%</span> |
| 48 | [00317 SCALAR_CAST_TYPEOF_023](crates/bench/sqlite_parity/cases/SQLITE_PARITY_317_SCALAR_CAST_TYPEOF_023.rs) | P1 | memory | GEN_SQL_SCALAR | 1505611 | 2306779 | <span style="color:#2563eb">23.11%</span> |
| 49 | [00297 SCALAR_CAST_TYPEOF_018](crates/bench/sqlite_parity/cases/SQLITE_PARITY_297_SCALAR_CAST_TYPEOF_018.rs) | P1 | memory | GEN_SQL_SCALAR | 1494701 | 2291380 | <span style="color:#2563eb">23.62%</span> |
| 50 | [00560 AGG_GROUP_HAVING_053](crates/bench/sqlite_parity/cases/SQLITE_PARITY_560_AGG_GROUP_HAVING_053.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1667789 | 2290608 | <span style="color:#2563eb">23.65%</span> |
| 51 | [00444 DML_WHERE_ORDER_LIMIT_057](crates/bench/sqlite_parity/cases/SQLITE_PARITY_444_DML_WHERE_ORDER_LIMIT_057.rs) | P1 | memory | GEN_SQL_DML | 1845063 | 2272814 | <span style="color:#2563eb">24.24%</span> |
| 52 | [00629 JOIN_SUBQUERY_EXISTS_022](crates/bench/sqlite_parity/cases/SQLITE_PARITY_629_JOIN_SUBQUERY_EXISTS_022.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1747880 | 2269878 | <span style="color:#2563eb">24.34%</span> |
| 53 | [00695 JOIN_SUBQUERY_EXISTS_088](crates/bench/sqlite_parity/cases/SQLITE_PARITY_695_JOIN_SUBQUERY_EXISTS_088.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1773739 | 2232728 | <span style="color:#2563eb">25.58%</span> |
| 54 | [00148 DOT_OUTPUT_TEMPFILE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_148_DOT_OUTPUT_TEMPFILE.rs) | P1 | tempfile | CLI_TEMPFILE | 1851165 | 2203793 | <span style="color:#2563eb">26.54%</span> |
| 55 | [00345 SCALAR_CAST_TYPEOF_030](crates/bench/sqlite_parity/cases/SQLITE_PARITY_345_SCALAR_CAST_TYPEOF_030.rs) | P1 | memory | GEN_SQL_SCALAR | 1575674 | 2198604 | <span style="color:#2563eb">26.71%</span> |
| 56 | [00147 DOT_IMPORT_CSV_TEMPFILE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_147_DOT_IMPORT_CSV_TEMPFILE.rs) | P1 | tempfile | CLI_TEMPFILE | 2023322 | 2191129 | <span style="color:#2563eb">26.96%</span> |
| 57 | [00169 DOT_NONCE_SAFE_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_169_DOT_NONCE_SAFE_MODE.rs) | P2 | memory | CLI_DOT_COMMAND | 1920837 | 2189997 | <span style="color:#2563eb">27.00%</span> |
| 58 | [00696 JOIN_SUBQUERY_EXISTS_089](crates/bench/sqlite_parity/cases/SQLITE_PARITY_696_JOIN_SUBQUERY_EXISTS_089.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1725057 | 2188344 | <span style="color:#2563eb">27.06%</span> |
| 59 | [00931 CONSTRAINT_FK_SAVEPOINT_064](crates/bench/sqlite_parity/cases/SQLITE_PARITY_931_CONSTRAINT_FK_SAVEPOINT_064.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1716821 | 2188084 | <span style="color:#2563eb">27.06%</span> |
| 60 | [00404 DML_WHERE_ORDER_LIMIT_017](crates/bench/sqlite_parity/cases/SQLITE_PARITY_404_DML_WHERE_ORDER_LIMIT_017.rs) | P1 | memory | GEN_SQL_DML | 1692175 | 2167154 | <span style="color:#2563eb">27.76%</span> |
| 61 | [00165 DOT_INTCK_CATALOG](crates/bench/sqlite_parity/cases/SQLITE_PARITY_165_DOT_INTCK_CATALOG.rs) | P4 | catalog | CLI_CATALOG | 4185225 | 3019808 | <span style="color:#2563eb">27.85%</span> |
| 62 | [01095 INDEX_SCHEMA_PRAGMA_028](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1095_INDEX_SCHEMA_PRAGMA_028.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1741387 | 2125846 | <span style="color:#2563eb">29.14%</span> |
| 63 | [00047 PRAGMA_FOREIGN_KEYS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_047_PRAGMA_FOREIGN_KEYS.rs) | P0 | memory | SQL_PRAGMA | 1503437 | 2122108 | <span style="color:#2563eb">29.26%</span> |
| 64 | [00082 CORE_STRING_FUNCTIONS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_082_CORE_STRING_FUNCTIONS.rs) | P0 | memory | SQL_FUNCTIONS | 1552489 | 2115566 | <span style="color:#2563eb">29.48%</span> |
| 65 | [00031 CREATE_INDEX](crates/bench/sqlite_parity/cases/SQLITE_PARITY_031_CREATE_INDEX.rs) | P0 | memory | SQL_INDEX | 1648902 | 2109144 | <span style="color:#2563eb">29.70%</span> |
| 66 | [00932 CONSTRAINT_FK_SAVEPOINT_065](crates/bench/sqlite_parity/cases/SQLITE_PARITY_932_CONSTRAINT_FK_SAVEPOINT_065.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1719837 | 2108332 | <span style="color:#2563eb">29.72%</span> |
| 67 | [00435 DML_WHERE_ORDER_LIMIT_048](crates/bench/sqlite_parity/cases/SQLITE_PARITY_435_DML_WHERE_ORDER_LIMIT_048.rs) | P1 | memory | GEN_SQL_DML | 1638363 | 2104715 | <span style="color:#2563eb">29.84%</span> |
| 68 | [00577 AGG_GROUP_HAVING_070](crates/bench/sqlite_parity/cases/SQLITE_PARITY_577_AGG_GROUP_HAVING_070.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1778298 | 2103343 | <span style="color:#2563eb">29.89%</span> |
| 69 | [00034 EXPRESSION_INDEX](crates/bench/sqlite_parity/cases/SQLITE_PARITY_034_EXPRESSION_INDEX.rs) | P0 | memory | SQL_INDEX | 1616461 | 2089647 | <span style="color:#2563eb">30.35%</span> |
| 70 | [01106 INDEX_SCHEMA_PRAGMA_039](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1106_INDEX_SCHEMA_PRAGMA_039.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 2122449 | 2088835 | <span style="color:#2563eb">30.37%</span> |
| 71 | [00184 OPT_ASCII_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_184_OPT_ASCII_MODE.rs) | P2 | memory | CLI_OPTION | 1552459 | 2058939 | <span style="color:#2563eb">31.37%</span> |
| 72 | [00418 DML_WHERE_ORDER_LIMIT_031](crates/bench/sqlite_parity/cases/SQLITE_PARITY_418_DML_WHERE_ORDER_LIMIT_031.rs) | P1 | memory | GEN_SQL_DML | 1681996 | 2054270 | <span style="color:#2563eb">31.52%</span> |
| 73 | [00562 AGG_GROUP_HAVING_055](crates/bench/sqlite_parity/cases/SQLITE_PARITY_562_AGG_GROUP_HAVING_055.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1747769 | 2054160 | <span style="color:#2563eb">31.53%</span> |
| 74 | [00178 OPT_HTML_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_178_OPT_HTML_MODE.rs) | P2 | memory | CLI_OPTION | 1465135 | 2043630 | <span style="color:#2563eb">31.88%</span> |
| 75 | [00151 DOT_SAVE_RESTORE_TEMPFILE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_151_DOT_SAVE_RESTORE_TEMPFILE.rs) | P1 | tempfile | CLI_TEMPFILE | 2202130 | 2034683 | <span style="color:#2563eb">32.18%</span> |
| 76 | [00708 CTE_RECURSIVE_MATRIX_001](crates/bench/sqlite_parity/cases/SQLITE_PARITY_708_CTE_RECURSIVE_MATRIX_001.rs) | P1 | memory | GEN_SQL_CTE | 1858680 | 2029262 | <span style="color:#2563eb">32.36%</span> |
| 77 | [00495 DML_WHERE_ORDER_LIMIT_108](crates/bench/sqlite_parity/cases/SQLITE_PARITY_495_DML_WHERE_ORDER_LIMIT_108.rs) | P1 | memory | GEN_SQL_DML | 1659222 | 2025455 | <span style="color:#2563eb">32.48%</span> |
| 78 | [00883 CONSTRAINT_FK_SAVEPOINT_016](crates/bench/sqlite_parity/cases/SQLITE_PARITY_883_CONSTRAINT_FK_SAVEPOINT_016.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1729635 | 2012841 | <span style="color:#2563eb">32.91%</span> |
| 79 | [00211 SQL_ATTACH_TEMPFILE_DATABASE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_211_SQL_ATTACH_TEMPFILE_DATABASE.rs) | P1 | tempfile | SQL_TEMPFILE | 1828242 | 2010678 | <span style="color:#2563eb">32.98%</span> |
| 80 | [00969 VIEW_TRIGGER_GENERATED_022](crates/bench/sqlite_parity/cases/SQLITE_PARITY_969_VIEW_TRIGGER_GENERATED_022.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1801151 | 2007622 | <span style="color:#2563eb">33.08%</span> |
| 81 | [00674 JOIN_SUBQUERY_EXISTS_067](crates/bench/sqlite_parity/cases/SQLITE_PARITY_674_JOIN_SUBQUERY_EXISTS_067.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1739434 | 2002682 | <span style="color:#2563eb">33.24%</span> |
| 82 | [00342 SCALAR_STRING_029](crates/bench/sqlite_parity/cases/SQLITE_PARITY_342_SCALAR_STRING_029.rs) | P1 | memory | GEN_SQL_SCALAR | 1531921 | 2000859 | <span style="color:#2563eb">33.30%</span> |
| 83 | [00734 CTE_RECURSIVE_MATRIX_027](crates/bench/sqlite_parity/cases/SQLITE_PARITY_734_CTE_RECURSIVE_MATRIX_027.rs) | P1 | memory | GEN_SQL_CTE | 1614457 | 1986402 | <span style="color:#2563eb">33.79%</span> |
| 84 | [00157 DOT_ARCHIVE_TEMPFILE_OPTIONAL](crates/bench/sqlite_parity/cases/SQLITE_PARITY_157_DOT_ARCHIVE_TEMPFILE_OPTIONAL.rs) | P3 | tempfile | CLI_TEMPFILE_OPTIONAL | 2642875 | 1976763 | <span style="color:#2563eb">34.11%</span> |
| 85 | [00843 WINDOW_PARTITION_SUM_056](crates/bench/sqlite_parity/cases/SQLITE_PARITY_843_WINDOW_PARTITION_SUM_056.rs) | P2 | memory | GEN_SQL_WINDOW | 1652459 | 1970161 | <span style="color:#2563eb">34.33%</span> |
| 86 | [00001 SELECT_CORE_EXPRESSIONS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_001_SELECT_CORE_EXPRESSIONS.rs) | P0 | memory | SQL_SELECT | 1724316 | 1957798 | <span style="color:#2563eb">34.74%</span> |
| 87 | [00007 WITHOUT_ROWID_TABLE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_007_WITHOUT_ROWID_TABLE.rs) | P0 | memory | SQL_ROWID | 1548292 | 1956685 | <span style="color:#2563eb">34.78%</span> |
| 88 | [00914 CONSTRAINT_FK_SAVEPOINT_047](crates/bench/sqlite_parity/cases/SQLITE_PARITY_914_CONSTRAINT_FK_SAVEPOINT_047.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1746166 | 1946796 | <span style="color:#2563eb">35.11%</span> |
| 89 | [00010 GENERATED_COLUMNS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_010_GENERATED_COLUMNS.rs) | P0 | memory | SQL_DDL | 1544305 | 1939512 | <span style="color:#2563eb">35.35%</span> |
| 90 | [00738 CTE_RECURSIVE_MATRIX_031](crates/bench/sqlite_parity/cases/SQLITE_PARITY_738_CTE_RECURSIVE_MATRIX_031.rs) | P1 | memory | GEN_SQL_CTE | 1625739 | 1939142 | <span style="color:#2563eb">35.36%</span> |
| 91 | [00004 TABLE_CONSTRAINTS_SUCCESS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_004_TABLE_CONSTRAINTS_SUCCESS.rs) | P0 | memory | SQL_CONSTRAINTS | 1814676 | 1935686 | <span style="color:#2563eb">35.48%</span> |
| 92 | [01003 VIEW_TRIGGER_GENERATED_056](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1003_VIEW_TRIGGER_GENERATED_056.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1772317 | 1935265 | <span style="color:#2563eb">35.49%</span> |
| 93 | [00915 CONSTRAINT_FK_SAVEPOINT_048](crates/bench/sqlite_parity/cases/SQLITE_PARITY_915_CONSTRAINT_FK_SAVEPOINT_048.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1743912 | 1932459 | <span style="color:#2563eb">35.58%</span> |
| 94 | [00949 VIEW_TRIGGER_GENERATED_002](crates/bench/sqlite_parity/cases/SQLITE_PARITY_949_VIEW_TRIGGER_GENERATED_002.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2095138 | 1930055 | <span style="color:#2563eb">35.66%</span> |
| 95 | [00688 JOIN_SUBQUERY_EXISTS_081](crates/bench/sqlite_parity/cases/SQLITE_PARITY_688_JOIN_SUBQUERY_EXISTS_081.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1739104 | 1922391 | <span style="color:#2563eb">35.92%</span> |
| 96 | [00879 CONSTRAINT_FK_SAVEPOINT_012](crates/bench/sqlite_parity/cases/SQLITE_PARITY_879_CONSTRAINT_FK_SAVEPOINT_012.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1735867 | 1920307 | <span style="color:#2563eb">35.99%</span> |
| 97 | [00980 VIEW_TRIGGER_GENERATED_033](crates/bench/sqlite_parity/cases/SQLITE_PARITY_980_VIEW_TRIGGER_GENERATED_033.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1785682 | 1915828 | <span style="color:#2563eb">36.14%</span> |
| 98 | [01088 INDEX_SCHEMA_PRAGMA_021](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1088_INDEX_SCHEMA_PRAGMA_021.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1782676 | 1913904 | <span style="color:#2563eb">36.20%</span> |
| 99 | [00273 SCALAR_CAST_TYPEOF_012](crates/bench/sqlite_parity/cases/SQLITE_PARITY_273_SCALAR_CAST_TYPEOF_012.rs) | P1 | memory | GEN_SQL_SCALAR | 1501824 | 1911740 | <span style="color:#2563eb">36.28%</span> |
| 100 | [00603 AGG_GROUP_HAVING_096](crates/bench/sqlite_parity/cases/SQLITE_PARITY_603_AGG_GROUP_HAVING_096.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1733613 | 1911590 | <span style="color:#2563eb">36.28%</span> |
| 101 | [00660 JOIN_SUBQUERY_EXISTS_053](crates/bench/sqlite_parity/cases/SQLITE_PARITY_660_JOIN_SUBQUERY_EXISTS_053.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1716390 | 1910588 | <span style="color:#2563eb">36.31%</span> |
| 102 | [00794 WINDOW_PARTITION_SUM_007](crates/bench/sqlite_parity/cases/SQLITE_PARITY_794_WINDOW_PARTITION_SUM_007.rs) | P2 | memory | GEN_SQL_WINDOW | 1664191 | 1910578 | <span style="color:#2563eb">36.31%</span> |
| 103 | [00287 SCALAR_NULL_COALESCE_015](crates/bench/sqlite_parity/cases/SQLITE_PARITY_287_SCALAR_NULL_COALESCE_015.rs) | P1 | memory | GEN_SQL_SCALAR | 2170020 | 1908164 | <span style="color:#2563eb">36.39%</span> |
| 104 | [00844 WINDOW_PARTITION_SUM_057](crates/bench/sqlite_parity/cases/SQLITE_PARITY_844_WINDOW_PARTITION_SUM_057.rs) | P2 | memory | GEN_SQL_WINDOW | 1680272 | 1908084 | <span style="color:#2563eb">36.40%</span> |
| 105 | [00333 SCALAR_CAST_TYPEOF_027](crates/bench/sqlite_parity/cases/SQLITE_PARITY_333_SCALAR_CAST_TYPEOF_027.rs) | P1 | memory | GEN_SQL_SCALAR | 1502846 | 1907763 | <span style="color:#2563eb">36.41%</span> |
| 106 | [01113 INDEX_SCHEMA_PRAGMA_046](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1113_INDEX_SCHEMA_PRAGMA_046.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1883306 | 1906290 | <span style="color:#2563eb">36.46%</span> |
| 107 | [00035 DROP_INDEX](crates/bench/sqlite_parity/cases/SQLITE_PARITY_035_DROP_INDEX.rs) | P0 | memory | SQL_DROP | 1610038 | 1899337 | <span style="color:#2563eb">36.69%</span> |
| 108 | [00263 SCALAR_NULL_COALESCE_009](crates/bench/sqlite_parity/cases/SQLITE_PARITY_263_SCALAR_NULL_COALESCE_009.rs) | P1 | memory | GEN_SQL_SCALAR | 1538193 | 1898606 | <span style="color:#2563eb">36.71%</span> |
| 109 | [00621 JOIN_SUBQUERY_EXISTS_014](crates/bench/sqlite_parity/cases/SQLITE_PARITY_621_JOIN_SUBQUERY_EXISTS_014.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1758480 | 1898586 | <span style="color:#2563eb">36.71%</span> |
| 110 | [01001 VIEW_TRIGGER_GENERATED_054](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1001_VIEW_TRIGGER_GENERATED_054.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1706722 | 1896001 | <span style="color:#2563eb">36.80%</span> |
| 111 | [00192 OPT_INIT_TEMPFILE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_192_OPT_INIT_TEMPFILE.rs) | P2 | tempfile | CLI_OPTION_TEMPFILE | 1829103 | 1895480 | <span style="color:#2563eb">36.82%</span> |
| 112 | [00403 DML_WHERE_ORDER_LIMIT_016](crates/bench/sqlite_parity/cases/SQLITE_PARITY_403_DML_WHERE_ORDER_LIMIT_016.rs) | P1 | memory | GEN_SQL_DML | 1801371 | 1894688 | <span style="color:#2563eb">36.84%</span> |
| 113 | [00559 AGG_GROUP_HAVING_052](crates/bench/sqlite_parity/cases/SQLITE_PARITY_559_AGG_GROUP_HAVING_052.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1758940 | 1894537 | <span style="color:#2563eb">36.85%</span> |
| 114 | [00008 STRICT_TABLE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_008_STRICT_TABLE.rs) | P0 | memory | SQL_DDL | 1849923 | 1887584 | <span style="color:#2563eb">37.08%</span> |
| 115 | [01030 JSON_EXTRACT_SET_023](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1030_JSON_EXTRACT_SET_023.rs) | P2 | memory | GEN_SQL_JSON | 1669311 | 1887104 | <span style="color:#2563eb">37.10%</span> |
| 116 | [00459 DML_WHERE_ORDER_LIMIT_072](crates/bench/sqlite_parity/cases/SQLITE_PARITY_459_DML_WHERE_ORDER_LIMIT_072.rs) | P1 | memory | GEN_SQL_DML | 1624967 | 1885671 | <span style="color:#2563eb">37.14%</span> |
| 117 | [00065 CTE_RECURSIVE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_065_CTE_RECURSIVE.rs) | P0 | memory | SQL_CTE | 1549074 | 1877244 | <span style="color:#2563eb">37.43%</span> |
| 118 | [00531 AGG_GROUP_HAVING_024](crates/bench/sqlite_parity/cases/SQLITE_PARITY_531_AGG_GROUP_HAVING_024.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1741117 | 1876683 | <span style="color:#2563eb">37.44%</span> |
| 119 | [00973 VIEW_TRIGGER_GENERATED_026](crates/bench/sqlite_parity/cases/SQLITE_PARITY_973_VIEW_TRIGGER_GENERATED_026.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1756796 | 1875461 | <span style="color:#2563eb">37.48%</span> |
| 120 | [00583 AGG_GROUP_HAVING_076](crates/bench/sqlite_parity/cases/SQLITE_PARITY_583_AGG_GROUP_HAVING_076.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2136947 | 1868648 | <span style="color:#2563eb">37.71%</span> |
| 121 | [00419 DML_WHERE_ORDER_LIMIT_032](crates/bench/sqlite_parity/cases/SQLITE_PARITY_419_DML_WHERE_ORDER_LIMIT_032.rs) | P1 | memory | GEN_SQL_DML | 1721480 | 1862487 | <span style="color:#2563eb">37.92%</span> |
| 122 | [01108 INDEX_SCHEMA_PRAGMA_041](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1108_INDEX_SCHEMA_PRAGMA_041.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 2139782 | 1862467 | <span style="color:#2563eb">37.92%</span> |
| 123 | [00150 DOT_BACKUP_RESTORE_TEMPFILE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_150_DOT_BACKUP_RESTORE_TEMPFILE.rs) | P1 | tempfile | CLI_TEMPFILE | 2141826 | 1861525 | <span style="color:#2563eb">37.95%</span> |
| 124 | [00030 ALTER_TABLE_ADD_DROP_COLUMN](crates/bench/sqlite_parity/cases/SQLITE_PARITY_030_ALTER_TABLE_ADD_DROP_COLUMN.rs) | P0 | memory | SQL_ALTER | 1986762 | 1860893 | <span style="color:#2563eb">37.97%</span> |
| 125 | [00950 VIEW_TRIGGER_GENERATED_003](crates/bench/sqlite_parity/cases/SQLITE_PARITY_950_VIEW_TRIGGER_GENERATED_003.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1736408 | 1859331 | <span style="color:#2563eb">38.02%</span> |
| 126 | [00340 SCALAR_ARITH_029](crates/bench/sqlite_parity/cases/SQLITE_PARITY_340_SCALAR_ARITH_029.rs) | P1 | memory | GEN_SQL_SCALAR | 1482367 | 1855984 | <span style="color:#2563eb">38.13%</span> |
| 127 | [00040 INSTEAD_OF_TRIGGER_ON_VIEW](crates/bench/sqlite_parity/cases/SQLITE_PARITY_040_INSTEAD_OF_TRIGGER_ON_VIEW.rs) | P0 | memory | SQL_TRIGGER | 1629125 | 1853730 | <span style="color:#2563eb">38.21%</span> |
| 128 | [01024 JSON_EXTRACT_SET_017](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1024_JSON_EXTRACT_SET_017.rs) | P2 | memory | GEN_SQL_JSON | 2688130 | 1849152 | <span style="color:#2563eb">38.36%</span> |
| 129 | [00613 JOIN_SUBQUERY_EXISTS_006](crates/bench/sqlite_parity/cases/SQLITE_PARITY_613_JOIN_SUBQUERY_EXISTS_006.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1756926 | 1847489 | <span style="color:#2563eb">38.42%</span> |
| 130 | [00371 SCALAR_NULL_COALESCE_036](crates/bench/sqlite_parity/cases/SQLITE_PARITY_371_SCALAR_NULL_COALESCE_036.rs) | P1 | memory | GEN_SQL_SCALAR | 1471797 | 1841667 | <span style="color:#2563eb">38.61%</span> |
| 131 | [00351 SCALAR_NULL_COALESCE_031](crates/bench/sqlite_parity/cases/SQLITE_PARITY_351_SCALAR_NULL_COALESCE_031.rs) | P1 | memory | GEN_SQL_SCALAR | 1453512 | 1841517 | <span style="color:#2563eb">38.62%</span> |
| 132 | [01044 JSON_EXTRACT_SET_037](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1044_JSON_EXTRACT_SET_037.rs) | P2 | memory | GEN_SQL_JSON | 1580102 | 1840044 | <span style="color:#2563eb">38.67%</span> |
| 133 | [00212 SQL_VACUUM_INTO_TEMPFILE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_212_SQL_VACUUM_INTO_TEMPFILE.rs) | P2 | tempfile | SQL_TEMPFILE | 2129062 | 1833482 | <span style="color:#2563eb">38.88%</span> |
| 134 | [00606 AGG_GROUP_HAVING_099](crates/bench/sqlite_parity/cases/SQLITE_PARITY_606_AGG_GROUP_HAVING_099.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2085229 | 1830466 | <span style="color:#2563eb">38.98%</span> |
| 135 | [00987 VIEW_TRIGGER_GENERATED_040](crates/bench/sqlite_parity/cases/SQLITE_PARITY_987_VIEW_TRIGGER_GENERATED_040.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1774891 | 1828462 | <span style="color:#2563eb">39.05%</span> |
| 136 | [00957 VIEW_TRIGGER_GENERATED_010](crates/bench/sqlite_parity/cases/SQLITE_PARITY_957_VIEW_TRIGGER_GENERATED_010.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1753480 | 1828111 | <span style="color:#2563eb">39.06%</span> |
| 137 | [00385 SCALAR_CAST_TYPEOF_040](crates/bench/sqlite_parity/cases/SQLITE_PARITY_385_SCALAR_CAST_TYPEOF_040.rs) | P1 | memory | GEN_SQL_SCALAR | 1553482 | 1824866 | <span style="color:#2563eb">39.17%</span> |
| 138 | [00853 WINDOW_PARTITION_SUM_066](crates/bench/sqlite_parity/cases/SQLITE_PARITY_853_WINDOW_PARTITION_SUM_066.rs) | P2 | memory | GEN_SQL_WINDOW | 1623044 | 1824024 | <span style="color:#2563eb">39.20%</span> |
| 139 | [00646 JOIN_SUBQUERY_EXISTS_039](crates/bench/sqlite_parity/cases/SQLITE_PARITY_646_JOIN_SUBQUERY_EXISTS_039.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1735226 | 1823042 | <span style="color:#2563eb">39.23%</span> |
| 140 | [00992 VIEW_TRIGGER_GENERATED_045](crates/bench/sqlite_parity/cases/SQLITE_PARITY_992_VIEW_TRIGGER_GENERATED_045.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1764851 | 1816901 | <span style="color:#2563eb">39.44%</span> |
| 141 | [00033 PARTIAL_INDEX](crates/bench/sqlite_parity/cases/SQLITE_PARITY_033_PARTIAL_INDEX.rs) | P0 | memory | SQL_INDEX | 2714300 | 1814506 | <span style="color:#2563eb">39.52%</span> |
| 142 | [00701 JOIN_SUBQUERY_EXISTS_094](crates/bench/sqlite_parity/cases/SQLITE_PARITY_701_JOIN_SUBQUERY_EXISTS_094.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1706912 | 1808475 | <span style="color:#2563eb">39.72%</span> |
| 143 | [00379 SCALAR_NULL_COALESCE_038](crates/bench/sqlite_parity/cases/SQLITE_PARITY_379_SCALAR_NULL_COALESCE_038.rs) | P1 | memory | GEN_SQL_SCALAR | 1506012 | 1807913 | <span style="color:#2563eb">39.74%</span> |
| 144 | [00564 AGG_GROUP_HAVING_057](crates/bench/sqlite_parity/cases/SQLITE_PARITY_564_AGG_GROUP_HAVING_057.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1976613 | 1803696 | <span style="color:#2563eb">39.88%</span> |
| 145 | [00966 VIEW_TRIGGER_GENERATED_019](crates/bench/sqlite_parity/cases/SQLITE_PARITY_966_VIEW_TRIGGER_GENERATED_019.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1775382 | 1801121 | <span style="color:#2563eb">39.96%</span> |
| 146 | [00390 DML_WHERE_ORDER_LIMIT_003](crates/bench/sqlite_parity/cases/SQLITE_PARITY_390_DML_WHERE_ORDER_LIMIT_003.rs) | P1 | memory | GEN_SQL_DML | 2750769 | 1800700 | <span style="color:#2563eb">39.98%</span> |
| 147 | [00149 DOT_ONCE_TEMPFILE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_149_DOT_ONCE_TEMPFILE.rs) | P1 | tempfile | CLI_TEMPFILE | 1630297 | 1794739 | <span style="color:#2563eb">40.18%</span> |
| 148 | [00144 DOT_PROMPT](crates/bench/sqlite_parity/cases/SQLITE_PARITY_144_DOT_PROMPT.rs) | P0 | memory | CLI_DOT_COMMAND | 1751136 | 1790020 | <span style="color:#2563eb">40.33%</span> |
| 149 | [00507 DML_WHERE_ORDER_LIMIT_120](crates/bench/sqlite_parity/cases/SQLITE_PARITY_507_DML_WHERE_ORDER_LIMIT_120.rs) | P1 | memory | GEN_SQL_DML | 1677406 | 1789539 | <span style="color:#2563eb">40.35%</span> |
| 150 | [00922 CONSTRAINT_FK_SAVEPOINT_055](crates/bench/sqlite_parity/cases/SQLITE_PARITY_922_CONSTRAINT_FK_SAVEPOINT_055.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1771896 | 1784799 | <span style="color:#2563eb">40.51%</span> |
| 151 | [00204 OPT_ZIP_TEMPFILE_CATALOG](crates/bench/sqlite_parity/cases/SQLITE_PARITY_204_OPT_ZIP_TEMPFILE_CATALOG.rs) | P4 | catalog | CLI_OPTION_CATALOG | 2541032 | 1782355 | <span style="color:#2563eb">40.59%</span> |
| 152 | [00782 CTE_RECURSIVE_MATRIX_075](crates/bench/sqlite_parity/cases/SQLITE_PARITY_782_CTE_RECURSIVE_MATRIX_075.rs) | P1 | memory | GEN_SQL_CTE | 2601156 | 1780862 | <span style="color:#2563eb">40.64%</span> |
| 153 | [00572 AGG_GROUP_HAVING_065](crates/bench/sqlite_parity/cases/SQLITE_PARITY_572_AGG_GROUP_HAVING_065.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1760003 | 1779419 | <span style="color:#2563eb">40.69%</span> |
| 154 | [01107 INDEX_SCHEMA_PRAGMA_040](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1107_INDEX_SCHEMA_PRAGMA_040.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1737961 | 1778107 | <span style="color:#2563eb">40.73%</span> |
| 155 | [00109 DOT_MODE_JSON](crates/bench/sqlite_parity/cases/SQLITE_PARITY_109_DOT_MODE_JSON.rs) | P0 | memory | CLI_DOT_COMMAND | 1799507 | 1774260 | <span style="color:#2563eb">40.86%</span> |
| 156 | [00513 AGG_GROUP_HAVING_006](crates/bench/sqlite_parity/cases/SQLITE_PARITY_513_AGG_GROUP_HAVING_006.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2109204 | 1774139 | <span style="color:#2563eb">40.86%</span> |
| 157 | [00905 CONSTRAINT_FK_SAVEPOINT_038](crates/bench/sqlite_parity/cases/SQLITE_PARITY_905_CONSTRAINT_FK_SAVEPOINT_038.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1738923 | 1772236 | <span style="color:#2563eb">40.93%</span> |
| 158 | [00407 DML_WHERE_ORDER_LIMIT_020](crates/bench/sqlite_parity/cases/SQLITE_PARITY_407_DML_WHERE_ORDER_LIMIT_020.rs) | P1 | memory | GEN_SQL_DML | 1664972 | 1771595 | <span style="color:#2563eb">40.95%</span> |
| 159 | [00902 CONSTRAINT_FK_SAVEPOINT_035](crates/bench/sqlite_parity/cases/SQLITE_PARITY_902_CONSTRAINT_FK_SAVEPOINT_035.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1730527 | 1766826 | <span style="color:#2563eb">41.11%</span> |
| 160 | [00373 SCALAR_CAST_TYPEOF_037](crates/bench/sqlite_parity/cases/SQLITE_PARITY_373_SCALAR_CAST_TYPEOF_037.rs) | P1 | memory | GEN_SQL_SCALAR | 1533454 | 1766575 | <span style="color:#2563eb">41.11%</span> |
| 161 | [00939 CONSTRAINT_FK_SAVEPOINT_072](crates/bench/sqlite_parity/cases/SQLITE_PARITY_939_CONSTRAINT_FK_SAVEPOINT_072.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1731539 | 1762828 | <span style="color:#2563eb">41.24%</span> |
| 162 | [00400 DML_WHERE_ORDER_LIMIT_013](crates/bench/sqlite_parity/cases/SQLITE_PARITY_400_DML_WHERE_ORDER_LIMIT_013.rs) | P1 | memory | GEN_SQL_DML | 1672296 | 1753911 | <span style="color:#2563eb">41.54%</span> |
| 163 | [00451 DML_WHERE_ORDER_LIMIT_064](crates/bench/sqlite_parity/cases/SQLITE_PARITY_451_DML_WHERE_ORDER_LIMIT_064.rs) | P1 | memory | GEN_SQL_DML | 1643312 | 1753290 | <span style="color:#2563eb">41.56%</span> |
| 164 | [00898 CONSTRAINT_FK_SAVEPOINT_031](crates/bench/sqlite_parity/cases/SQLITE_PARITY_898_CONSTRAINT_FK_SAVEPOINT_031.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1821179 | 1746457 | <span style="color:#2563eb">41.78%</span> |
| 165 | [00940 CONSTRAINT_FK_SAVEPOINT_073](crates/bench/sqlite_parity/cases/SQLITE_PARITY_940_CONSTRAINT_FK_SAVEPOINT_073.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1722903 | 1744263 | <span style="color:#2563eb">41.86%</span> |
| 166 | [00266 SCALAR_STRING_010](crates/bench/sqlite_parity/cases/SQLITE_PARITY_266_SCALAR_STRING_010.rs) | P1 | memory | GEN_SQL_SCALAR | 1504549 | 1743201 | <span style="color:#2563eb">41.89%</span> |
| 167 | [00909 CONSTRAINT_FK_SAVEPOINT_042](crates/bench/sqlite_parity/cases/SQLITE_PARITY_909_CONSTRAINT_FK_SAVEPOINT_042.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1796943 | 1742059 | <span style="color:#2563eb">41.93%</span> |
| 168 | [00411 DML_WHERE_ORDER_LIMIT_024](crates/bench/sqlite_parity/cases/SQLITE_PARITY_411_DML_WHERE_ORDER_LIMIT_024.rs) | P1 | memory | GEN_SQL_DML | 1778658 | 1740957 | <span style="color:#2563eb">41.97%</span> |
| 169 | [00226 OPT_NOFOLLOW_CATALOG](crates/bench/sqlite_parity/cases/SQLITE_PARITY_226_OPT_NOFOLLOW_CATALOG.rs) | P4 | catalog | CLI_OPTION_CATALOG | 1330860 | 1740095 | <span style="color:#2563eb">42.00%</span> |
| 170 | [00096 DBSTAT_OPTIONAL](crates/bench/sqlite_parity/cases/SQLITE_PARITY_096_DBSTAT_OPTIONAL.rs) | P3 | memory | SQL_VIRTUAL_TABLE_OPTIONAL | 1658460 | 1734133 | <span style="color:#2563eb">42.20%</span> |
| 171 | [00995 VIEW_TRIGGER_GENERATED_048](crates/bench/sqlite_parity/cases/SQLITE_PARITY_995_VIEW_TRIGGER_GENERATED_048.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1895299 | 1733001 | <span style="color:#2563eb">42.23%</span> |
| 172 | [00051 PRAGMA_INDEX_LIST_FUNCTION](crates/bench/sqlite_parity/cases/SQLITE_PARITY_051_PRAGMA_INDEX_LIST_FUNCTION.rs) | P0 | memory | SQL_PRAGMA | 2504062 | 1731429 | <span style="color:#2563eb">42.29%</span> |
| 173 | [00873 CONSTRAINT_FK_SAVEPOINT_006](crates/bench/sqlite_parity/cases/SQLITE_PARITY_873_CONSTRAINT_FK_SAVEPOINT_006.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1757949 | 1730166 | <span style="color:#2563eb">42.33%</span> |
| 174 | [00936 CONSTRAINT_FK_SAVEPOINT_069](crates/bench/sqlite_parity/cases/SQLITE_PARITY_936_CONSTRAINT_FK_SAVEPOINT_069.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1726680 | 1725277 | <span style="color:#2563eb">42.49%</span> |
| 175 | [00036 CREATE_VIEW](crates/bench/sqlite_parity/cases/SQLITE_PARITY_036_CREATE_VIEW.rs) | P0 | memory | SQL_VIEW | 2305276 | 1719957 | <span style="color:#2563eb">42.67%</span> |
| 176 | [00656 JOIN_SUBQUERY_EXISTS_049](crates/bench/sqlite_parity/cases/SQLITE_PARITY_656_JOIN_SUBQUERY_EXISTS_049.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1723424 | 1713736 | <span style="color:#2563eb">42.88%</span> |
| 177 | [00479 DML_WHERE_ORDER_LIMIT_092](crates/bench/sqlite_parity/cases/SQLITE_PARITY_479_DML_WHERE_ORDER_LIMIT_092.rs) | P1 | memory | GEN_SQL_DML | 1634275 | 1712383 | <span style="color:#2563eb">42.92%</span> |
| 178 | [00700 JOIN_SUBQUERY_EXISTS_093](crates/bench/sqlite_parity/cases/SQLITE_PARITY_700_JOIN_SUBQUERY_EXISTS_093.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1775923 | 1711450 | <span style="color:#2563eb">42.95%</span> |
| 179 | [01019 JSON_EXTRACT_SET_012](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1019_JSON_EXTRACT_SET_012.rs) | P2 | memory | GEN_SQL_JSON | 1602415 | 1711170 | <span style="color:#2563eb">42.96%</span> |
| 180 | [00217 DETACH_DATABASE_SYNTAX](crates/bench/sqlite_parity/cases/SQLITE_PARITY_217_DETACH_DATABASE_SYNTAX.rs) | P0 | memory | SQL_ATTACH | 2233359 | 1709167 | <span style="color:#2563eb">43.03%</span> |
| 181 | [00358 SCALAR_STRING_033](crates/bench/sqlite_parity/cases/SQLITE_PARITY_358_SCALAR_STRING_033.rs) | P1 | memory | GEN_SQL_SCALAR | 1555365 | 1709116 | <span style="color:#2563eb">43.03%</span> |
| 182 | [00895 CONSTRAINT_FK_SAVEPOINT_028](crates/bench/sqlite_parity/cases/SQLITE_PARITY_895_CONSTRAINT_FK_SAVEPOINT_028.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1706893 | 1705880 | <span style="color:#2563eb">43.14%</span> |
| 183 | [00346 SCALAR_STRING_030](crates/bench/sqlite_parity/cases/SQLITE_PARITY_346_SCALAR_STRING_030.rs) | P1 | memory | GEN_SQL_SCALAR | 1526421 | 1705099 | <span style="color:#2563eb">43.16%</span> |
| 184 | [00524 AGG_GROUP_HAVING_017](crates/bench/sqlite_parity/cases/SQLITE_PARITY_524_AGG_GROUP_HAVING_017.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1696442 | 1701372 | <span style="color:#2563eb">43.29%</span> |
| 185 | [00548 AGG_GROUP_HAVING_041](crates/bench/sqlite_parity/cases/SQLITE_PARITY_548_AGG_GROUP_HAVING_041.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1710139 | 1701011 | <span style="color:#2563eb">43.30%</span> |
| 186 | [00152 DOT_CLONE_TEMPFILE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_152_DOT_CLONE_TEMPFILE.rs) | P2 | tempfile | CLI_TEMPFILE | 2356212 | 1700140 | <span style="color:#2563eb">43.33%</span> |
| 187 | [00111 DOT_HEADERS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_111_DOT_HEADERS.rs) | P0 | memory | CLI_DOT_COMMAND | 1580413 | 1695992 | <span style="color:#2563eb">43.47%</span> |
| 188 | [01084 INDEX_SCHEMA_PRAGMA_017](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1084_INDEX_SCHEMA_PRAGMA_017.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1789679 | 1693838 | <span style="color:#2563eb">43.54%</span> |
| 189 | [00139 DOT_LINT_FKEY_INDEXES](crates/bench/sqlite_parity/cases/SQLITE_PARITY_139_DOT_LINT_FKEY_INDEXES.rs) | P3 | memory | CLI_DOT_COMMAND_DIAGNOSTIC | 1645095 | 1680012 | <span style="color:#2563eb">44.00%</span> |
| 190 | [00878 CONSTRAINT_FK_SAVEPOINT_011](crates/bench/sqlite_parity/cases/SQLITE_PARITY_878_CONSTRAINT_FK_SAVEPOINT_011.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1739965 | 1678048 | <span style="color:#2563eb">44.07%</span> |
| 191 | [00072 ORDER_BY_NULLS_FIRST_LAST](crates/bench/sqlite_parity/cases/SQLITE_PARITY_072_ORDER_BY_NULLS_FIRST_LAST.rs) | P0 | memory | SQL_SELECT | 1528384 | 1674621 | <span style="color:#2563eb">44.18%</span> |
| 192 | [00391 DML_WHERE_ORDER_LIMIT_004](crates/bench/sqlite_parity/cases/SQLITE_PARITY_391_DML_WHERE_ORDER_LIMIT_004.rs) | P1 | memory | GEN_SQL_DML | 1751236 | 1673178 | <span style="color:#2563eb">44.23%</span> |
| 193 | [00289 SCALAR_CAST_TYPEOF_016](crates/bench/sqlite_parity/cases/SQLITE_PARITY_289_SCALAR_CAST_TYPEOF_016.rs) | P1 | memory | GEN_SQL_SCALAR | 2257085 | 1671726 | <span style="color:#2563eb">44.28%</span> |
| 194 | [00306 SCALAR_STRING_020](crates/bench/sqlite_parity/cases/SQLITE_PARITY_306_SCALAR_STRING_020.rs) | P1 | memory | GEN_SQL_SCALAR | 1622382 | 1669732 | <span style="color:#2563eb">44.34%</span> |
| 195 | [00055 JOINS_RIGHT_FULL_OUTER](crates/bench/sqlite_parity/cases/SQLITE_PARITY_055_JOINS_RIGHT_FULL_OUTER.rs) | P0 | memory | SQL_JOIN | 1913814 | 1669571 | <span style="color:#2563eb">44.35%</span> |
| 196 | [00044 ANALYZE_SQLITE_STAT1](crates/bench/sqlite_parity/cases/SQLITE_PARITY_044_ANALYZE_SQLITE_STAT1.rs) | P0 | memory | SQL_ANALYZE | 1670543 | 1668650 | <span style="color:#2563eb">44.38%</span> |
| 197 | [01102 INDEX_SCHEMA_PRAGMA_035](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1102_INDEX_SCHEMA_PRAGMA_035.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1768559 | 1668209 | <span style="color:#2563eb">44.39%</span> |
| 198 | [00153 DOT_CD_TEMPFILE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_153_DOT_CD_TEMPFILE.rs) | P2 | tempfile | CLI_TEMPFILE | 1277218 | 1665915 | <span style="color:#2563eb">44.47%</span> |
| 199 | [00928 CONSTRAINT_FK_SAVEPOINT_061](crates/bench/sqlite_parity/cases/SQLITE_PARITY_928_CONSTRAINT_FK_SAVEPOINT_061.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1722742 | 1664812 | <span style="color:#2563eb">44.51%</span> |
| 200 | [00951 VIEW_TRIGGER_GENERATED_004](crates/bench/sqlite_parity/cases/SQLITE_PARITY_951_VIEW_TRIGGER_GENERATED_004.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1736248 | 1664542 | <span style="color:#2563eb">44.52%</span> |
| 201 | [00413 DML_WHERE_ORDER_LIMIT_026](crates/bench/sqlite_parity/cases/SQLITE_PARITY_413_DML_WHERE_ORDER_LIMIT_026.rs) | P1 | memory | GEN_SQL_DML | 1705861 | 1664271 | <span style="color:#2563eb">44.52%</span> |
| 202 | [01112 INDEX_SCHEMA_PRAGMA_045](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1112_INDEX_SCHEMA_PRAGMA_045.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1740256 | 1661576 | <span style="color:#2563eb">44.61%</span> |
| 203 | [01114 INDEX_SCHEMA_PRAGMA_047](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1114_INDEX_SCHEMA_PRAGMA_047.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 2105607 | 1656577 | <span style="color:#2563eb">44.78%</span> |
| 204 | [00215 TRANSACTION_SYNONYMS_END_COMMIT_TRANSACTION](crates/bench/sqlite_parity/cases/SQLITE_PARITY_215_TRANSACTION_SYNONYMS_END_COMMIT_TRANSACTION.rs) | P0 | memory | SQL_TRANSACTION | 1870522 | 1656317 | <span style="color:#2563eb">44.79%</span> |
| 205 | [01077 INDEX_SCHEMA_PRAGMA_010](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1077_INDEX_SCHEMA_PRAGMA_010.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1764922 | 1655354 | <span style="color:#2563eb">44.82%</span> |
| 206 | [00838 WINDOW_PARTITION_SUM_051](crates/bench/sqlite_parity/cases/SQLITE_PARITY_838_WINDOW_PARTITION_SUM_051.rs) | P2 | memory | GEN_SQL_WINDOW | 1692485 | 1653331 | <span style="color:#2563eb">44.89%</span> |
| 207 | [01062 JSON_EXTRACT_SET_055](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1062_JSON_EXTRACT_SET_055.rs) | P2 | memory | GEN_SQL_JSON | 1643702 | 1652419 | <span style="color:#2563eb">44.92%</span> |
| 208 | [00186 OPT_NEWLINE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_186_OPT_NEWLINE.rs) | P2 | memory | CLI_OPTION | 1774550 | 1652128 | <span style="color:#2563eb">44.93%</span> |
| 209 | [01087 INDEX_SCHEMA_PRAGMA_020](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1087_INDEX_SCHEMA_PRAGMA_020.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1790079 | 1649965 | <span style="color:#2563eb">45.00%</span> |
| 210 | [00394 DML_WHERE_ORDER_LIMIT_007](crates/bench/sqlite_parity/cases/SQLITE_PARITY_394_DML_WHERE_ORDER_LIMIT_007.rs) | P1 | memory | GEN_SQL_DML | 1680752 | 1649153 | <span style="color:#2563eb">45.03%</span> |
| 211 | [00320 SCALAR_ARITH_024](crates/bench/sqlite_parity/cases/SQLITE_PARITY_320_SCALAR_ARITH_024.rs) | P1 | memory | GEN_SQL_SCALAR | 1852698 | 1648091 | <span style="color:#2563eb">45.06%</span> |
| 212 | [00136 DOT_LOG](crates/bench/sqlite_parity/cases/SQLITE_PARITY_136_DOT_LOG.rs) | P0 | memory | CLI_DOT_COMMAND | 1523184 | 1645937 | <span style="color:#2563eb">45.14%</span> |
| 213 | [00331 SCALAR_NULL_COALESCE_026](crates/bench/sqlite_parity/cases/SQLITE_PARITY_331_SCALAR_NULL_COALESCE_026.rs) | P1 | memory | GEN_SQL_SCALAR | 1495472 | 1644755 | <span style="color:#2563eb">45.17%</span> |
| 214 | [00335 SCALAR_NULL_COALESCE_027](crates/bench/sqlite_parity/cases/SQLITE_PARITY_335_SCALAR_NULL_COALESCE_027.rs) | P1 | memory | GEN_SQL_SCALAR | 1513005 | 1644524 | <span style="color:#2563eb">45.18%</span> |
| 215 | [00545 AGG_GROUP_HAVING_038](crates/bench/sqlite_parity/cases/SQLITE_PARITY_545_AGG_GROUP_HAVING_038.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1718945 | 1643853 | <span style="color:#2563eb">45.20%</span> |
| 216 | [01116 INDEX_SCHEMA_PRAGMA_049](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1116_INDEX_SCHEMA_PRAGMA_049.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 2008143 | 1641518 | <span style="color:#2563eb">45.28%</span> |
| 217 | [00223 OPT_NOHEADER](crates/bench/sqlite_parity/cases/SQLITE_PARITY_223_OPT_NOHEADER.rs) | P2 | memory | CLI_OPTION | 1506112 | 1640377 | <span style="color:#2563eb">45.32%</span> |
| 218 | [01096 INDEX_SCHEMA_PRAGMA_029](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1096_INDEX_SCHEMA_PRAGMA_029.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1743011 | 1639906 | <span style="color:#2563eb">45.34%</span> |
| 219 | [01110 INDEX_SCHEMA_PRAGMA_043](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1110_INDEX_SCHEMA_PRAGMA_043.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1896120 | 1638814 | <span style="color:#2563eb">45.37%</span> |
| 220 | [00112 DOT_SEPARATOR](crates/bench/sqlite_parity/cases/SQLITE_PARITY_112_DOT_SEPARATOR.rs) | P0 | memory | CLI_DOT_COMMAND | 1506533 | 1635297 | <span style="color:#2563eb">45.49%</span> |
| 221 | [00913 CONSTRAINT_FK_SAVEPOINT_046](crates/bench/sqlite_parity/cases/SQLITE_PARITY_913_CONSTRAINT_FK_SAVEPOINT_046.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1726860 | 1635186 | <span style="color:#2563eb">45.49%</span> |
| 222 | [00140 DOT_EXPERT_OPTIONAL](crates/bench/sqlite_parity/cases/SQLITE_PARITY_140_DOT_EXPERT_OPTIONAL.rs) | P3 | memory | CLI_DOT_COMMAND_OPTIONAL | 2264608 | 1634685 | <span style="color:#2563eb">45.51%</span> |
| 223 | [00897 CONSTRAINT_FK_SAVEPOINT_030](crates/bench/sqlite_parity/cases/SQLITE_PARITY_897_CONSTRAINT_FK_SAVEPOINT_030.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1746929 | 1634505 | <span style="color:#2563eb">45.52%</span> |
| 224 | [00747 CTE_RECURSIVE_MATRIX_040](crates/bench/sqlite_parity/cases/SQLITE_PARITY_747_CTE_RECURSIVE_MATRIX_040.rs) | P1 | memory | GEN_SQL_CTE | 1964200 | 1634415 | <span style="color:#2563eb">45.52%</span> |
| 225 | [00807 WINDOW_PARTITION_SUM_020](crates/bench/sqlite_parity/cases/SQLITE_PARITY_807_WINDOW_PARTITION_SUM_020.rs) | P2 | memory | GEN_SQL_WINDOW | 1618635 | 1633072 | <span style="color:#2563eb">45.56%</span> |
| 226 | [00739 CTE_RECURSIVE_MATRIX_032](crates/bench/sqlite_parity/cases/SQLITE_PARITY_739_CTE_RECURSIVE_MATRIX_032.rs) | P1 | memory | GEN_SQL_CTE | 1803235 | 1630918 | <span style="color:#2563eb">45.64%</span> |
| 227 | [00874 CONSTRAINT_FK_SAVEPOINT_007](crates/bench/sqlite_parity/cases/SQLITE_PARITY_874_CONSTRAINT_FK_SAVEPOINT_007.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1816519 | 1630037 | <span style="color:#2563eb">45.67%</span> |
| 228 | [00927 CONSTRAINT_FK_SAVEPOINT_060](crates/bench/sqlite_parity/cases/SQLITE_PARITY_927_CONSTRAINT_FK_SAVEPOINT_060.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2074138 | 1628164 | <span style="color:#2563eb">45.73%</span> |
| 229 | [01090 INDEX_SCHEMA_PRAGMA_023](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1090_INDEX_SCHEMA_PRAGMA_023.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1738081 | 1626450 | <span style="color:#2563eb">45.78%</span> |
| 230 | [00449 DML_WHERE_ORDER_LIMIT_062](crates/bench/sqlite_parity/cases/SQLITE_PARITY_449_DML_WHERE_ORDER_LIMIT_062.rs) | P1 | memory | GEN_SQL_DML | 1678218 | 1624576 | <span style="color:#2563eb">45.85%</span> |
| 231 | [00892 CONSTRAINT_FK_SAVEPOINT_025](crates/bench/sqlite_parity/cases/SQLITE_PARITY_892_CONSTRAINT_FK_SAVEPOINT_025.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1754392 | 1624276 | <span style="color:#2563eb">45.86%</span> |
| 232 | [00866 WINDOW_PARTITION_SUM_079](crates/bench/sqlite_parity/cases/SQLITE_PARITY_866_WINDOW_PARTITION_SUM_079.rs) | P2 | memory | GEN_SQL_WINDOW | 1696332 | 1623875 | <span style="color:#2563eb">45.87%</span> |
| 233 | [00423 DML_WHERE_ORDER_LIMIT_036](crates/bench/sqlite_parity/cases/SQLITE_PARITY_423_DML_WHERE_ORDER_LIMIT_036.rs) | P1 | memory | GEN_SQL_DML | 1674100 | 1622453 | <span style="color:#2563eb">45.92%</span> |
| 234 | [00307 SCALAR_NULL_COALESCE_020](crates/bench/sqlite_parity/cases/SQLITE_PARITY_307_SCALAR_NULL_COALESCE_020.rs) | P1 | memory | GEN_SQL_SCALAR | 1514829 | 1622312 | <span style="color:#2563eb">45.92%</span> |
| 235 | [00354 SCALAR_STRING_032](crates/bench/sqlite_parity/cases/SQLITE_PARITY_354_SCALAR_STRING_032.rs) | P1 | memory | GEN_SQL_SCALAR | 1510460 | 1621972 | <span style="color:#2563eb">45.93%</span> |
| 236 | [00642 JOIN_SUBQUERY_EXISTS_035](crates/bench/sqlite_parity/cases/SQLITE_PARITY_642_JOIN_SUBQUERY_EXISTS_035.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1763700 | 1621199 | <span style="color:#2563eb">45.96%</span> |
| 237 | [01109 INDEX_SCHEMA_PRAGMA_042](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1109_INDEX_SCHEMA_PRAGMA_042.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1760655 | 1621140 | <span style="color:#2563eb">45.96%</span> |
| 238 | [00384 SCALAR_ARITH_040](crates/bench/sqlite_parity/cases/SQLITE_PARITY_384_SCALAR_ARITH_040.rs) | P1 | memory | GEN_SQL_SCALAR | 1778659 | 1620989 | <span style="color:#2563eb">45.97%</span> |
| 239 | [00604 AGG_GROUP_HAVING_097](crates/bench/sqlite_parity/cases/SQLITE_PARITY_604_AGG_GROUP_HAVING_097.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1969660 | 1620689 | <span style="color:#2563eb">45.98%</span> |
| 240 | [00704 JOIN_SUBQUERY_EXISTS_097](crates/bench/sqlite_parity/cases/SQLITE_PARITY_704_JOIN_SUBQUERY_EXISTS_097.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1746287 | 1619727 | <span style="color:#2563eb">46.01%</span> |
| 241 | [00316 SCALAR_ARITH_023](crates/bench/sqlite_parity/cases/SQLITE_PARITY_316_SCALAR_ARITH_023.rs) | P1 | memory | GEN_SQL_SCALAR | 1528865 | 1618074 | <span style="color:#2563eb">46.06%</span> |
| 242 | [00261 SCALAR_CAST_TYPEOF_009](crates/bench/sqlite_parity/cases/SQLITE_PARITY_261_SCALAR_CAST_TYPEOF_009.rs) | P1 | memory | GEN_SQL_SCALAR | 1593718 | 1617593 | <span style="color:#2563eb">46.08%</span> |
| 243 | [00815 WINDOW_PARTITION_SUM_028](crates/bench/sqlite_parity/cases/SQLITE_PARITY_815_WINDOW_PARTITION_SUM_028.rs) | P2 | memory | GEN_SQL_WINDOW | 1682185 | 1617473 | <span style="color:#2563eb">46.08%</span> |
| 244 | [01027 JSON_EXTRACT_SET_020](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1027_JSON_EXTRACT_SET_020.rs) | P2 | memory | GEN_SQL_JSON | 1622402 | 1616722 | <span style="color:#2563eb">46.11%</span> |
| 245 | [01000 VIEW_TRIGGER_GENERATED_053](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1000_VIEW_TRIGGER_GENERATED_053.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1942177 | 1614958 | <span style="color:#2563eb">46.17%</span> |
| 246 | [00478 DML_WHERE_ORDER_LIMIT_091](crates/bench/sqlite_parity/cases/SQLITE_PARITY_478_DML_WHERE_ORDER_LIMIT_091.rs) | P1 | memory | GEN_SQL_DML | 1694639 | 1614227 | <span style="color:#2563eb">46.19%</span> |
| 247 | [00336 SCALAR_ARITH_028](crates/bench/sqlite_parity/cases/SQLITE_PARITY_336_SCALAR_ARITH_028.rs) | P1 | memory | GEN_SQL_SCALAR | 1704488 | 1614187 | <span style="color:#2563eb">46.19%</span> |
| 248 | [00821 WINDOW_PARTITION_SUM_034](crates/bench/sqlite_parity/cases/SQLITE_PARITY_821_WINDOW_PARTITION_SUM_034.rs) | P2 | memory | GEN_SQL_WINDOW | 1657810 | 1613957 | <span style="color:#2563eb">46.20%</span> |
| 249 | [00761 CTE_RECURSIVE_MATRIX_054](crates/bench/sqlite_parity/cases/SQLITE_PARITY_761_CTE_RECURSIVE_MATRIX_054.rs) | P1 | memory | GEN_SQL_CTE | 1895129 | 1613335 | <span style="color:#2563eb">46.22%</span> |
| 250 | [01121 INDEX_SCHEMA_PRAGMA_054](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1121_INDEX_SCHEMA_PRAGMA_054.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1754973 | 1613084 | <span style="color:#2563eb">46.23%</span> |
| 251 | [01118 INDEX_SCHEMA_PRAGMA_051](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1118_INDEX_SCHEMA_PRAGMA_051.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 2075020 | 1613025 | <span style="color:#2563eb">46.23%</span> |
| 252 | [01026 JSON_EXTRACT_SET_019](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1026_JSON_EXTRACT_SET_019.rs) | P2 | memory | GEN_SQL_JSON | 1680853 | 1612363 | <span style="color:#2563eb">46.25%</span> |
| 253 | [00686 JOIN_SUBQUERY_EXISTS_079](crates/bench/sqlite_parity/cases/SQLITE_PARITY_686_JOIN_SUBQUERY_EXISTS_079.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2118301 | 1612253 | <span style="color:#2563eb">46.26%</span> |
| 254 | [01080 INDEX_SCHEMA_PRAGMA_013](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1080_INDEX_SCHEMA_PRAGMA_013.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1810578 | 1611150 | <span style="color:#2563eb">46.29%</span> |
| 255 | [00350 SCALAR_STRING_031](crates/bench/sqlite_parity/cases/SQLITE_PARITY_350_SCALAR_STRING_031.rs) | P1 | memory | GEN_SQL_SCALAR | 1598827 | 1610310 | <span style="color:#2563eb">46.32%</span> |
| 256 | [00751 CTE_RECURSIVE_MATRIX_044](crates/bench/sqlite_parity/cases/SQLITE_PARITY_751_CTE_RECURSIVE_MATRIX_044.rs) | P1 | memory | GEN_SQL_CTE | 2455990 | 1610089 | <span style="color:#2563eb">46.33%</span> |
| 257 | [01091 INDEX_SCHEMA_PRAGMA_024](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1091_INDEX_SCHEMA_PRAGMA_024.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1749122 | 1609858 | <span style="color:#2563eb">46.34%</span> |
| 258 | [01056 JSON_EXTRACT_SET_049](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1056_JSON_EXTRACT_SET_049.rs) | P2 | memory | GEN_SQL_JSON | 1626911 | 1609688 | <span style="color:#2563eb">46.34%</span> |
| 259 | [00960 VIEW_TRIGGER_GENERATED_013](crates/bench/sqlite_parity/cases/SQLITE_PARITY_960_VIEW_TRIGGER_GENERATED_013.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1815337 | 1609628 | <span style="color:#2563eb">46.35%</span> |
| 260 | [00718 CTE_RECURSIVE_MATRIX_011](crates/bench/sqlite_parity/cases/SQLITE_PARITY_718_CTE_RECURSIVE_MATRIX_011.rs) | P1 | memory | GEN_SQL_CTE | 1594790 | 1609608 | <span style="color:#2563eb">46.35%</span> |
| 261 | [00997 VIEW_TRIGGER_GENERATED_050](crates/bench/sqlite_parity/cases/SQLITE_PARITY_997_VIEW_TRIGGER_GENERATED_050.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1771825 | 1609608 | <span style="color:#2563eb">46.35%</span> |
| 262 | [01083 INDEX_SCHEMA_PRAGMA_016](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1083_INDEX_SCHEMA_PRAGMA_016.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1756517 | 1609478 | <span style="color:#2563eb">46.35%</span> |
| 263 | [00626 JOIN_SUBQUERY_EXISTS_019](crates/bench/sqlite_parity/cases/SQLITE_PARITY_626_JOIN_SUBQUERY_EXISTS_019.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1756306 | 1609137 | <span style="color:#2563eb">46.36%</span> |
| 264 | [00986 VIEW_TRIGGER_GENERATED_039](crates/bench/sqlite_parity/cases/SQLITE_PARITY_986_VIEW_TRIGGER_GENERATED_039.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2177804 | 1609127 | <span style="color:#2563eb">46.36%</span> |
| 265 | [00772 CTE_RECURSIVE_MATRIX_065](crates/bench/sqlite_parity/cases/SQLITE_PARITY_772_CTE_RECURSIVE_MATRIX_065.rs) | P1 | memory | GEN_SQL_CTE | 1592686 | 1608396 | <span style="color:#2563eb">46.39%</span> |
| 266 | [00442 DML_WHERE_ORDER_LIMIT_055](crates/bench/sqlite_parity/cases/SQLITE_PARITY_442_DML_WHERE_ORDER_LIMIT_055.rs) | P1 | memory | GEN_SQL_DML | 2292421 | 1608095 | <span style="color:#2563eb">46.40%</span> |
| 267 | [00425 DML_WHERE_ORDER_LIMIT_038](crates/bench/sqlite_parity/cases/SQLITE_PARITY_425_DML_WHERE_ORDER_LIMIT_038.rs) | P1 | memory | GEN_SQL_DML | 1640086 | 1607965 | <span style="color:#2563eb">46.40%</span> |
| 268 | [00378 SCALAR_STRING_038](crates/bench/sqlite_parity/cases/SQLITE_PARITY_378_SCALAR_STRING_038.rs) | P1 | memory | GEN_SQL_SCALAR | 1584520 | 1607874 | <span style="color:#2563eb">46.40%</span> |
| 269 | [00659 JOIN_SUBQUERY_EXISTS_052](crates/bench/sqlite_parity/cases/SQLITE_PARITY_659_JOIN_SUBQUERY_EXISTS_052.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1727851 | 1607574 | <span style="color:#2563eb">46.41%</span> |
| 270 | [00900 CONSTRAINT_FK_SAVEPOINT_033](crates/bench/sqlite_parity/cases/SQLITE_PARITY_900_CONSTRAINT_FK_SAVEPOINT_033.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1824665 | 1607303 | <span style="color:#2563eb">46.42%</span> |
| 271 | [00917 CONSTRAINT_FK_SAVEPOINT_050](crates/bench/sqlite_parity/cases/SQLITE_PARITY_917_CONSTRAINT_FK_SAVEPOINT_050.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1774440 | 1607294 | <span style="color:#2563eb">46.42%</span> |
| 272 | [00872 CONSTRAINT_FK_SAVEPOINT_005](crates/bench/sqlite_parity/cases/SQLITE_PARITY_872_CONSTRAINT_FK_SAVEPOINT_005.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1749623 | 1606863 | <span style="color:#2563eb">46.44%</span> |
| 273 | [00720 CTE_RECURSIVE_MATRIX_013](crates/bench/sqlite_parity/cases/SQLITE_PARITY_720_CTE_RECURSIVE_MATRIX_013.rs) | P1 | memory | GEN_SQL_CTE | 1607664 | 1606272 | <span style="color:#2563eb">46.46%</span> |
| 274 | [00105 CASE_SENSITIVE_LIKE_PRAGMA](crates/bench/sqlite_parity/cases/SQLITE_PARITY_105_CASE_SENSITIVE_LIKE_PRAGMA.rs) | P2 | memory | SQL_PRAGMA | 1647290 | 1606232 | <span style="color:#2563eb">46.46%</span> |
| 275 | [01068 INDEX_SCHEMA_PRAGMA_001](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1068_INDEX_SCHEMA_PRAGMA_001.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1732781 | 1605881 | <span style="color:#2563eb">46.47%</span> |
| 276 | [01075 INDEX_SCHEMA_PRAGMA_008](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1075_INDEX_SCHEMA_PRAGMA_008.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1736068 | 1605781 | <span style="color:#2563eb">46.47%</span> |
| 277 | [00976 VIEW_TRIGGER_GENERATED_029](crates/bench/sqlite_parity/cases/SQLITE_PARITY_976_VIEW_TRIGGER_GENERATED_029.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1769210 | 1605279 | <span style="color:#2563eb">46.49%</span> |
| 278 | [00684 JOIN_SUBQUERY_EXISTS_077](crates/bench/sqlite_parity/cases/SQLITE_PARITY_684_JOIN_SUBQUERY_EXISTS_077.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1711802 | 1604609 | <span style="color:#2563eb">46.51%</span> |
| 279 | [00880 CONSTRAINT_FK_SAVEPOINT_013](crates/bench/sqlite_parity/cases/SQLITE_PARITY_880_CONSTRAINT_FK_SAVEPOINT_013.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1801832 | 1603316 | <span style="color:#2563eb">46.56%</span> |
| 280 | [00073 INDEXED_BY](crates/bench/sqlite_parity/cases/SQLITE_PARITY_073_INDEXED_BY.rs) | P0 | memory | SQL_INDEX | 1955202 | 1602795 | <span style="color:#2563eb">46.57%</span> |
| 281 | [00450 DML_WHERE_ORDER_LIMIT_063](crates/bench/sqlite_parity/cases/SQLITE_PARITY_450_DML_WHERE_ORDER_LIMIT_063.rs) | P1 | memory | GEN_SQL_DML | 1979378 | 1602324 | <span style="color:#2563eb">46.59%</span> |
| 282 | [00582 AGG_GROUP_HAVING_075](crates/bench/sqlite_parity/cases/SQLITE_PARITY_582_AGG_GROUP_HAVING_075.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1739033 | 1602304 | <span style="color:#2563eb">46.59%</span> |
| 283 | [00460 DML_WHERE_ORDER_LIMIT_073](crates/bench/sqlite_parity/cases/SQLITE_PARITY_460_DML_WHERE_ORDER_LIMIT_073.rs) | P1 | memory | GEN_SQL_DML | 1748601 | 1602224 | <span style="color:#2563eb">46.59%</span> |
| 284 | [00202 OPT_APPEND_TEMPFILE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_202_OPT_APPEND_TEMPFILE.rs) | P4 | tempfile | CLI_OPTION_TEMPFILE | 1543773 | 1602204 | <span style="color:#2563eb">46.59%</span> |
| 285 | [00610 JOIN_SUBQUERY_EXISTS_003](crates/bench/sqlite_parity/cases/SQLITE_PARITY_610_JOIN_SUBQUERY_EXISTS_003.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2167324 | 1601663 | <span style="color:#2563eb">46.61%</span> |
| 286 | [01039 JSON_EXTRACT_SET_032](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1039_JSON_EXTRACT_SET_032.rs) | P2 | memory | GEN_SQL_JSON | 1627682 | 1601573 | <span style="color:#2563eb">46.61%</span> |
| 287 | [00248 SCALAR_ARITH_006](crates/bench/sqlite_parity/cases/SQLITE_PARITY_248_SCALAR_ARITH_006.rs) | P1 | memory | GEN_SQL_SCALAR | 1518586 | 1601412 | <span style="color:#2563eb">46.62%</span> |
| 288 | [00328 SCALAR_ARITH_026](crates/bench/sqlite_parity/cases/SQLITE_PARITY_328_SCALAR_ARITH_026.rs) | P1 | memory | GEN_SQL_SCALAR | 1445868 | 1601081 | <span style="color:#2563eb">46.63%</span> |
| 289 | [00875 CONSTRAINT_FK_SAVEPOINT_008](crates/bench/sqlite_parity/cases/SQLITE_PARITY_875_CONSTRAINT_FK_SAVEPOINT_008.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1757157 | 1600531 | <span style="color:#2563eb">46.65%</span> |
| 290 | [00026 FOREIGN_KEYS_CASCADE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_026_FOREIGN_KEYS_CASCADE.rs) | P0 | memory | SQL_FOREIGN_KEYS | 2048539 | 1600160 | <span style="color:#2563eb">46.66%</span> |
| 291 | [01060 JSON_EXTRACT_SET_053](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1060_JSON_EXTRACT_SET_053.rs) | P2 | memory | GEN_SQL_JSON | 1652399 | 1600120 | <span style="color:#2563eb">46.66%</span> |
| 292 | [00327 SCALAR_NULL_COALESCE_025](crates/bench/sqlite_parity/cases/SQLITE_PARITY_327_SCALAR_NULL_COALESCE_025.rs) | P1 | memory | GEN_SQL_SCALAR | 1763088 | 1600111 | <span style="color:#2563eb">46.66%</span> |
| 293 | [01100 INDEX_SCHEMA_PRAGMA_033](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1100_INDEX_SCHEMA_PRAGMA_033.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1725918 | 1599970 | <span style="color:#2563eb">46.67%</span> |
| 294 | [01014 JSON_EXTRACT_SET_007](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1014_JSON_EXTRACT_SET_007.rs) | P2 | memory | GEN_SQL_JSON | 1636038 | 1599328 | <span style="color:#2563eb">46.69%</span> |
| 295 | [00780 CTE_RECURSIVE_MATRIX_073](crates/bench/sqlite_parity/cases/SQLITE_PARITY_780_CTE_RECURSIVE_MATRIX_073.rs) | P1 | memory | GEN_SQL_CTE | 1625949 | 1599168 | <span style="color:#2563eb">46.69%</span> |
| 296 | [00360 SCALAR_ARITH_034](crates/bench/sqlite_parity/cases/SQLITE_PARITY_360_SCALAR_ARITH_034.rs) | P1 | memory | GEN_SQL_SCALAR | 1482247 | 1599078 | <span style="color:#2563eb">46.70%</span> |
| 297 | [01079 INDEX_SCHEMA_PRAGMA_012](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1079_INDEX_SCHEMA_PRAGMA_012.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1753661 | 1598887 | <span style="color:#2563eb">46.70%</span> |
| 298 | [00416 DML_WHERE_ORDER_LIMIT_029](crates/bench/sqlite_parity/cases/SQLITE_PARITY_416_DML_WHERE_ORDER_LIMIT_029.rs) | P1 | memory | GEN_SQL_DML | 1651277 | 1598788 | <span style="color:#2563eb">46.71%</span> |
| 299 | [00475 DML_WHERE_ORDER_LIMIT_088](crates/bench/sqlite_parity/cases/SQLITE_PARITY_475_DML_WHERE_ORDER_LIMIT_088.rs) | P1 | memory | GEN_SQL_DML | 1628694 | 1598186 | <span style="color:#2563eb">46.73%</span> |
| 300 | [00228 SCALAR_ARITH_001](crates/bench/sqlite_parity/cases/SQLITE_PARITY_228_SCALAR_ARITH_001.rs) | P1 | memory | GEN_SQL_SCALAR | 1472508 | 1598156 | <span style="color:#2563eb">46.73%</span> |
| 301 | [00375 SCALAR_NULL_COALESCE_037](crates/bench/sqlite_parity/cases/SQLITE_PARITY_375_SCALAR_NULL_COALESCE_037.rs) | P1 | memory | GEN_SQL_SCALAR | 1613846 | 1598006 | <span style="color:#2563eb">46.73%</span> |
| 302 | [01061 JSON_EXTRACT_SET_054](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1061_JSON_EXTRACT_SET_054.rs) | P2 | memory | GEN_SQL_JSON | 1992574 | 1597094 | <span style="color:#2563eb">46.76%</span> |
| 303 | [00290 SCALAR_STRING_016](crates/bench/sqlite_parity/cases/SQLITE_PARITY_290_SCALAR_STRING_016.rs) | P1 | memory | GEN_SQL_SCALAR | 1548762 | 1597005 | <span style="color:#2563eb">46.77%</span> |
| 304 | [01008 JSON_EXTRACT_SET_001](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1008_JSON_EXTRACT_SET_001.rs) | P2 | memory | GEN_SQL_JSON | 1622883 | 1596894 | <span style="color:#2563eb">46.77%</span> |
| 305 | [00983 VIEW_TRIGGER_GENERATED_036](crates/bench/sqlite_parity/cases/SQLITE_PARITY_983_VIEW_TRIGGER_GENERATED_036.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1789068 | 1596844 | <span style="color:#2563eb">46.77%</span> |
| 306 | [00868 CONSTRAINT_FK_SAVEPOINT_001](crates/bench/sqlite_parity/cases/SQLITE_PARITY_868_CONSTRAINT_FK_SAVEPOINT_001.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1869771 | 1596834 | <span style="color:#2563eb">46.77%</span> |
| 307 | [00421 DML_WHERE_ORDER_LIMIT_034](crates/bench/sqlite_parity/cases/SQLITE_PARITY_421_DML_WHERE_ORDER_LIMIT_034.rs) | P1 | memory | GEN_SQL_DML | 1803375 | 1596713 | <span style="color:#2563eb">46.78%</span> |
| 308 | [01006 VIEW_TRIGGER_GENERATED_059](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1006_VIEW_TRIGGER_GENERATED_059.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1770863 | 1596534 | <span style="color:#2563eb">46.78%</span> |
| 309 | [00620 JOIN_SUBQUERY_EXISTS_013](crates/bench/sqlite_parity/cases/SQLITE_PARITY_620_JOIN_SUBQUERY_EXISTS_013.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1772858 | 1596483 | <span style="color:#2563eb">46.78%</span> |
| 310 | [00291 SCALAR_NULL_COALESCE_016](crates/bench/sqlite_parity/cases/SQLITE_PARITY_291_SCALAR_NULL_COALESCE_016.rs) | P1 | memory | GEN_SQL_SCALAR | 1564422 | 1596433 | <span style="color:#2563eb">46.79%</span> |
| 311 | [00800 WINDOW_PARTITION_SUM_013](crates/bench/sqlite_parity/cases/SQLITE_PARITY_800_WINDOW_PARTITION_SUM_013.rs) | P2 | memory | GEN_SQL_WINDOW | 2536774 | 1596363 | <span style="color:#2563eb">46.79%</span> |
| 312 | [00899 CONSTRAINT_FK_SAVEPOINT_032](crates/bench/sqlite_parity/cases/SQLITE_PARITY_899_CONSTRAINT_FK_SAVEPOINT_032.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1766375 | 1596313 | <span style="color:#2563eb">46.79%</span> |
| 313 | [00011 CREATE_TABLE_AS_SELECT](crates/bench/sqlite_parity/cases/SQLITE_PARITY_011_CREATE_TABLE_AS_SELECT.rs) | P0 | memory | SQL_DDL | 1633574 | 1596092 | <span style="color:#2563eb">46.80%</span> |
| 314 | [00422 DML_WHERE_ORDER_LIMIT_035](crates/bench/sqlite_parity/cases/SQLITE_PARITY_422_DML_WHERE_ORDER_LIMIT_035.rs) | P1 | memory | GEN_SQL_DML | 1663981 | 1595772 | <span style="color:#2563eb">46.81%</span> |
| 315 | [00757 CTE_RECURSIVE_MATRIX_050](crates/bench/sqlite_parity/cases/SQLITE_PARITY_757_CTE_RECURSIVE_MATRIX_050.rs) | P1 | memory | GEN_SQL_CTE | 1591594 | 1595692 | <span style="color:#2563eb">46.81%</span> |
| 316 | [01064 JSON_EXTRACT_SET_057](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1064_JSON_EXTRACT_SET_057.rs) | P2 | memory | GEN_SQL_JSON | 1614377 | 1595521 | <span style="color:#2563eb">46.82%</span> |
| 317 | [00534 AGG_GROUP_HAVING_027](crates/bench/sqlite_parity/cases/SQLITE_PARITY_534_AGG_GROUP_HAVING_027.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1755213 | 1595512 | <span style="color:#2563eb">46.82%</span> |
| 318 | [00294 SCALAR_STRING_017](crates/bench/sqlite_parity/cases/SQLITE_PARITY_294_SCALAR_STRING_017.rs) | P1 | memory | GEN_SQL_SCALAR | 1535998 | 1595321 | <span style="color:#2563eb">46.82%</span> |
| 319 | [00563 AGG_GROUP_HAVING_056](crates/bench/sqlite_parity/cases/SQLITE_PARITY_563_AGG_GROUP_HAVING_056.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1705149 | 1595041 | <span style="color:#2563eb">46.83%</span> |
| 320 | [00649 JOIN_SUBQUERY_EXISTS_042](crates/bench/sqlite_parity/cases/SQLITE_PARITY_649_JOIN_SUBQUERY_EXISTS_042.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1763169 | 1594770 | <span style="color:#2563eb">46.84%</span> |
| 321 | [00552 AGG_GROUP_HAVING_045](crates/bench/sqlite_parity/cases/SQLITE_PARITY_552_AGG_GROUP_HAVING_045.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1686975 | 1594339 | <span style="color:#2563eb">46.86%</span> |
| 322 | [00962 VIEW_TRIGGER_GENERATED_015](crates/bench/sqlite_parity/cases/SQLITE_PARITY_962_VIEW_TRIGGER_GENERATED_015.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1786753 | 1594119 | <span style="color:#2563eb">46.86%</span> |
| 323 | [01017 JSON_EXTRACT_SET_010](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1017_JSON_EXTRACT_SET_010.rs) | P2 | memory | GEN_SQL_JSON | 1648141 | 1593909 | <span style="color:#2563eb">46.87%</span> |
| 324 | [00546 AGG_GROUP_HAVING_039](crates/bench/sqlite_parity/cases/SQLITE_PARITY_546_AGG_GROUP_HAVING_039.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2021969 | 1593808 | <span style="color:#2563eb">46.87%</span> |
| 325 | [00959 VIEW_TRIGGER_GENERATED_012](crates/bench/sqlite_parity/cases/SQLITE_PARITY_959_VIEW_TRIGGER_GENERATED_012.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1884659 | 1593798 | <span style="color:#2563eb">46.87%</span> |
| 326 | [00296 SCALAR_ARITH_018](crates/bench/sqlite_parity/cases/SQLITE_PARITY_296_SCALAR_ARITH_018.rs) | P1 | memory | GEN_SQL_SCALAR | 1551828 | 1593778 | <span style="color:#2563eb">46.87%</span> |
| 327 | [00710 CTE_RECURSIVE_MATRIX_003](crates/bench/sqlite_parity/cases/SQLITE_PARITY_710_CTE_RECURSIVE_MATRIX_003.rs) | P1 | memory | GEN_SQL_CTE | 1644795 | 1593587 | <span style="color:#2563eb">46.88%</span> |
| 328 | [00614 JOIN_SUBQUERY_EXISTS_007](crates/bench/sqlite_parity/cases/SQLITE_PARITY_614_JOIN_SUBQUERY_EXISTS_007.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1948470 | 1593517 | <span style="color:#2563eb">46.88%</span> |
| 329 | [00216 ROLLBACK_TRANSACTION_SYNTAX](crates/bench/sqlite_parity/cases/SQLITE_PARITY_216_ROLLBACK_TRANSACTION_SYNTAX.rs) | P0 | memory | SQL_TRANSACTION | 1660305 | 1593478 | <span style="color:#2563eb">46.88%</span> |
| 330 | [00644 JOIN_SUBQUERY_EXISTS_037](crates/bench/sqlite_parity/cases/SQLITE_PARITY_644_JOIN_SUBQUERY_EXISTS_037.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1769099 | 1593187 | <span style="color:#2563eb">46.89%</span> |
| 331 | [00561 AGG_GROUP_HAVING_054](crates/bench/sqlite_parity/cases/SQLITE_PARITY_561_AGG_GROUP_HAVING_054.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1682295 | 1592906 | <span style="color:#2563eb">46.90%</span> |
| 332 | [01023 JSON_EXTRACT_SET_016](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1023_JSON_EXTRACT_SET_016.rs) | P2 | memory | GEN_SQL_JSON | 1557279 | 1592796 | <span style="color:#2563eb">46.91%</span> |
| 333 | [00091 MATH_FUNCTIONS_OPTIONAL](crates/bench/sqlite_parity/cases/SQLITE_PARITY_091_MATH_FUNCTIONS_OPTIONAL.rs) | P2 | memory | SQL_FUNCTIONS_OPTIONAL | 1552921 | 1592535 | <span style="color:#2563eb">46.92%</span> |
| 334 | [01065 JSON_EXTRACT_SET_058](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1065_JSON_EXTRACT_SET_058.rs) | P2 | memory | GEN_SQL_JSON | 2618338 | 1591965 | <span style="color:#2563eb">46.93%</span> |
| 335 | [00447 DML_WHERE_ORDER_LIMIT_060](crates/bench/sqlite_parity/cases/SQLITE_PARITY_447_DML_WHERE_ORDER_LIMIT_060.rs) | P1 | memory | GEN_SQL_DML | 1655264 | 1591814 | <span style="color:#2563eb">46.94%</span> |
| 336 | [00332 SCALAR_ARITH_027](crates/bench/sqlite_parity/cases/SQLITE_PARITY_332_SCALAR_ARITH_027.rs) | P1 | memory | GEN_SQL_SCALAR | 1535718 | 1591464 | <span style="color:#2563eb">46.95%</span> |
| 337 | [00664 JOIN_SUBQUERY_EXISTS_057](crates/bench/sqlite_parity/cases/SQLITE_PARITY_664_JOIN_SUBQUERY_EXISTS_057.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1821089 | 1591394 | <span style="color:#2563eb">46.95%</span> |
| 338 | [00025 BEGIN_MODES](crates/bench/sqlite_parity/cases/SQLITE_PARITY_025_BEGIN_MODES.rs) | P0 | memory | SQL_TRANSACTION | 1646639 | 1591203 | <span style="color:#2563eb">46.96%</span> |
| 339 | [00260 SCALAR_ARITH_009](crates/bench/sqlite_parity/cases/SQLITE_PARITY_260_SCALAR_ARITH_009.rs) | P1 | memory | GEN_SQL_SCALAR | 1514388 | 1591143 | <span style="color:#2563eb">46.96%</span> |
| 340 | [00702 JOIN_SUBQUERY_EXISTS_095](crates/bench/sqlite_parity/cases/SQLITE_PARITY_702_JOIN_SUBQUERY_EXISTS_095.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1766635 | 1590923 | <span style="color:#2563eb">46.97%</span> |
| 341 | [00188 OPT_HEADER](crates/bench/sqlite_parity/cases/SQLITE_PARITY_188_OPT_HEADER.rs) | P1 | memory | CLI_OPTION | 1494150 | 1590862 | <span style="color:#2563eb">46.97%</span> |
| 342 | [00240 SCALAR_ARITH_004](crates/bench/sqlite_parity/cases/SQLITE_PARITY_240_SCALAR_ARITH_004.rs) | P1 | memory | GEN_SQL_SCALAR | 1630448 | 1590271 | <span style="color:#2563eb">46.99%</span> |
| 343 | [00591 AGG_GROUP_HAVING_084](crates/bench/sqlite_parity/cases/SQLITE_PARITY_591_AGG_GROUP_HAVING_084.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1744744 | 1590031 | <span style="color:#2563eb">47.00%</span> |
| 344 | [00938 CONSTRAINT_FK_SAVEPOINT_071](crates/bench/sqlite_parity/cases/SQLITE_PARITY_938_CONSTRAINT_FK_SAVEPOINT_071.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1772556 | 1589981 | <span style="color:#2563eb">47.00%</span> |
| 345 | [00754 CTE_RECURSIVE_MATRIX_047](crates/bench/sqlite_parity/cases/SQLITE_PARITY_754_CTE_RECURSIVE_MATRIX_047.rs) | P1 | memory | GEN_SQL_CTE | 1884799 | 1589911 | <span style="color:#2563eb">47.00%</span> |
| 346 | [00318 SCALAR_STRING_023](crates/bench/sqlite_parity/cases/SQLITE_PARITY_318_SCALAR_STRING_023.rs) | P1 | memory | GEN_SQL_SCALAR | 1563221 | 1589850 | <span style="color:#2563eb">47.01%</span> |
| 347 | [00941 CONSTRAINT_FK_SAVEPOINT_074](crates/bench/sqlite_parity/cases/SQLITE_PARITY_941_CONSTRAINT_FK_SAVEPOINT_074.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1755203 | 1589560 | <span style="color:#2563eb">47.01%</span> |
| 348 | [00254 SCALAR_STRING_007](crates/bench/sqlite_parity/cases/SQLITE_PARITY_254_SCALAR_STRING_007.rs) | P1 | memory | GEN_SQL_SCALAR | 1484351 | 1589550 | <span style="color:#2563eb">47.02%</span> |
| 349 | [00021 UPSERT_DO_NOTHING](crates/bench/sqlite_parity/cases/SQLITE_PARITY_021_UPSERT_DO_NOTHING.rs) | P0 | memory | SQL_UPSERT | 1620779 | 1589349 | <span style="color:#2563eb">47.02%</span> |
| 350 | [00609 JOIN_SUBQUERY_EXISTS_002](crates/bench/sqlite_parity/cases/SQLITE_PARITY_609_JOIN_SUBQUERY_EXISTS_002.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1738271 | 1589340 | <span style="color:#2563eb">47.02%</span> |
| 351 | [00767 CTE_RECURSIVE_MATRIX_060](crates/bench/sqlite_parity/cases/SQLITE_PARITY_767_CTE_RECURSIVE_MATRIX_060.rs) | P1 | memory | GEN_SQL_CTE | 1630136 | 1589310 | <span style="color:#2563eb">47.02%</span> |
| 352 | [00666 JOIN_SUBQUERY_EXISTS_059](crates/bench/sqlite_parity/cases/SQLITE_PARITY_666_JOIN_SUBQUERY_EXISTS_059.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2272994 | 1589189 | <span style="color:#2563eb">47.03%</span> |
| 353 | [00344 SCALAR_ARITH_030](crates/bench/sqlite_parity/cases/SQLITE_PARITY_344_SCALAR_ARITH_030.rs) | P1 | memory | GEN_SQL_SCALAR | 1522633 | 1589149 | <span style="color:#2563eb">47.03%</span> |
| 354 | [00784 CTE_RECURSIVE_MATRIX_077](crates/bench/sqlite_parity/cases/SQLITE_PARITY_784_CTE_RECURSIVE_MATRIX_077.rs) | P1 | memory | GEN_SQL_CTE | 2458515 | 1589049 | <span style="color:#2563eb">47.03%</span> |
| 355 | [01067 JSON_EXTRACT_SET_060](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1067_JSON_EXTRACT_SET_060.rs) | P2 | memory | GEN_SQL_JSON | 1623284 | 1588899 | <span style="color:#2563eb">47.04%</span> |
| 356 | [00828 WINDOW_PARTITION_SUM_041](crates/bench/sqlite_parity/cases/SQLITE_PARITY_828_WINDOW_PARTITION_SUM_041.rs) | P2 | memory | GEN_SQL_WINDOW | 1663610 | 1588658 | <span style="color:#2563eb">47.04%</span> |
| 357 | [00445 DML_WHERE_ORDER_LIMIT_058](crates/bench/sqlite_parity/cases/SQLITE_PARITY_445_DML_WHERE_ORDER_LIMIT_058.rs) | P1 | memory | GEN_SQL_DML | 1772045 | 1588017 | <span style="color:#2563eb">47.07%</span> |
| 358 | [01004 VIEW_TRIGGER_GENERATED_057](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1004_VIEW_TRIGGER_GENERATED_057.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1802063 | 1587917 | <span style="color:#2563eb">47.07%</span> |
| 359 | [00918 CONSTRAINT_FK_SAVEPOINT_051](crates/bench/sqlite_parity/cases/SQLITE_PARITY_918_CONSTRAINT_FK_SAVEPOINT_051.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1897543 | 1587696 | <span style="color:#2563eb">47.08%</span> |
| 360 | [00837 WINDOW_PARTITION_SUM_050](crates/bench/sqlite_parity/cases/SQLITE_PARITY_837_WINDOW_PARTITION_SUM_050.rs) | P2 | memory | GEN_SQL_WINDOW | 1951175 | 1587556 | <span style="color:#2563eb">47.08%</span> |
| 361 | [00989 VIEW_TRIGGER_GENERATED_042](crates/bench/sqlite_parity/cases/SQLITE_PARITY_989_VIEW_TRIGGER_GENERATED_042.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2146204 | 1587396 | <span style="color:#2563eb">47.09%</span> |
| 362 | [00066 VALUES_STATEMENT](crates/bench/sqlite_parity/cases/SQLITE_PARITY_066_VALUES_STATEMENT.rs) | P0 | memory | SQL_VALUES | 1479652 | 1587386 | <span style="color:#2563eb">47.09%</span> |
| 363 | [00429 DML_WHERE_ORDER_LIMIT_042](crates/bench/sqlite_parity/cases/SQLITE_PARITY_429_DML_WHERE_ORDER_LIMIT_042.rs) | P1 | memory | GEN_SQL_DML | 1879459 | 1587165 | <span style="color:#2563eb">47.09%</span> |
| 364 | [00137 DOT_VERSION](crates/bench/sqlite_parity/cases/SQLITE_PARITY_137_DOT_VERSION.rs) | P2 | memory | CLI_DOT_COMMAND_DIAGNOSTIC | 1691383 | 1586975 | <span style="color:#2563eb">47.10%</span> |
| 365 | [00946 CONSTRAINT_FK_SAVEPOINT_079](crates/bench/sqlite_parity/cases/SQLITE_PARITY_946_CONSTRAINT_FK_SAVEPOINT_079.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1767978 | 1585943 | <span style="color:#2563eb">47.14%</span> |
| 366 | [00180 OPT_BOX_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_180_OPT_BOX_MODE.rs) | P2 | memory | CLI_OPTION | 1510571 | 1585933 | <span style="color:#2563eb">47.14%</span> |
| 367 | [00309 SCALAR_CAST_TYPEOF_021](crates/bench/sqlite_parity/cases/SQLITE_PARITY_309_SCALAR_CAST_TYPEOF_021.rs) | P1 | memory | GEN_SQL_SCALAR | 1457981 | 1585833 | <span style="color:#2563eb">47.14%</span> |
| 368 | [00859 WINDOW_PARTITION_SUM_072](crates/bench/sqlite_parity/cases/SQLITE_PARITY_859_WINDOW_PARTITION_SUM_072.rs) | P2 | memory | GEN_SQL_WINDOW | 1708796 | 1585672 | <span style="color:#2563eb">47.14%</span> |
| 369 | [00698 JOIN_SUBQUERY_EXISTS_091](crates/bench/sqlite_parity/cases/SQLITE_PARITY_698_JOIN_SUBQUERY_EXISTS_091.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1765573 | 1585643 | <span style="color:#2563eb">47.15%</span> |
| 370 | [00677 JOIN_SUBQUERY_EXISTS_070](crates/bench/sqlite_parity/cases/SQLITE_PARITY_677_JOIN_SUBQUERY_EXISTS_070.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1723954 | 1585182 | <span style="color:#2563eb">47.16%</span> |
| 371 | [00636 JOIN_SUBQUERY_EXISTS_029](crates/bench/sqlite_parity/cases/SQLITE_PARITY_636_JOIN_SUBQUERY_EXISTS_029.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1808324 | 1584581 | <span style="color:#2563eb">47.18%</span> |
| 372 | [00016 UPDATE_RETURNING](crates/bench/sqlite_parity/cases/SQLITE_PARITY_016_UPDATE_RETURNING.rs) | P0 | memory | SQL_UPDATE | 1642110 | 1584300 | <span style="color:#2563eb">47.19%</span> |
| 373 | [00426 DML_WHERE_ORDER_LIMIT_039](crates/bench/sqlite_parity/cases/SQLITE_PARITY_426_DML_WHERE_ORDER_LIMIT_039.rs) | P1 | memory | GEN_SQL_DML | 1702173 | 1584019 | <span style="color:#2563eb">47.20%</span> |
| 374 | [01066 JSON_EXTRACT_SET_059](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1066_JSON_EXTRACT_SET_059.rs) | P2 | memory | GEN_SQL_JSON | 1767657 | 1583919 | <span style="color:#2563eb">47.20%</span> |
| 375 | [00904 CONSTRAINT_FK_SAVEPOINT_037](crates/bench/sqlite_parity/cases/SQLITE_PARITY_904_CONSTRAINT_FK_SAVEPOINT_037.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1731389 | 1583849 | <span style="color:#2563eb">47.21%</span> |
| 376 | [00884 CONSTRAINT_FK_SAVEPOINT_017](crates/bench/sqlite_parity/cases/SQLITE_PARITY_884_CONSTRAINT_FK_SAVEPOINT_017.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1731329 | 1583800 | <span style="color:#2563eb">47.21%</span> |
| 377 | [00799 WINDOW_PARTITION_SUM_012](crates/bench/sqlite_parity/cases/SQLITE_PARITY_799_WINDOW_PARTITION_SUM_012.rs) | P2 | memory | GEN_SQL_WINDOW | 1808264 | 1583769 | <span style="color:#2563eb">47.21%</span> |
| 378 | [00791 WINDOW_PARTITION_SUM_004](crates/bench/sqlite_parity/cases/SQLITE_PARITY_791_WINDOW_PARTITION_SUM_004.rs) | P2 | memory | GEN_SQL_WINDOW | 1656487 | 1583739 | <span style="color:#2563eb">47.21%</span> |
| 379 | [00988 VIEW_TRIGGER_GENERATED_041](crates/bench/sqlite_parity/cases/SQLITE_PARITY_988_VIEW_TRIGGER_GENERATED_041.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1740766 | 1583689 | <span style="color:#2563eb">47.21%</span> |
| 380 | [00550 AGG_GROUP_HAVING_043](crates/bench/sqlite_parity/cases/SQLITE_PARITY_550_AGG_GROUP_HAVING_043.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1738833 | 1583639 | <span style="color:#2563eb">47.21%</span> |
| 381 | [00605 AGG_GROUP_HAVING_098](crates/bench/sqlite_parity/cases/SQLITE_PARITY_605_AGG_GROUP_HAVING_098.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1741207 | 1583429 | <span style="color:#2563eb">47.22%</span> |
| 382 | [01045 JSON_EXTRACT_SET_038](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1045_JSON_EXTRACT_SET_038.rs) | P2 | memory | GEN_SQL_JSON | 1607334 | 1583428 | <span style="color:#2563eb">47.22%</span> |
| 383 | [00575 AGG_GROUP_HAVING_068](crates/bench/sqlite_parity/cases/SQLITE_PARITY_575_AGG_GROUP_HAVING_068.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1727561 | 1583268 | <span style="color:#2563eb">47.22%</span> |
| 384 | [00998 VIEW_TRIGGER_GENERATED_051](crates/bench/sqlite_parity/cases/SQLITE_PARITY_998_VIEW_TRIGGER_GENERATED_051.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1825567 | 1582998 | <span style="color:#2563eb">47.23%</span> |
| 385 | [00624 JOIN_SUBQUERY_EXISTS_017](crates/bench/sqlite_parity/cases/SQLITE_PARITY_624_JOIN_SUBQUERY_EXISTS_017.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2085670 | 1582957 | <span style="color:#2563eb">47.23%</span> |
| 386 | [00171 OPT_HELP](crates/bench/sqlite_parity/cases/SQLITE_PARITY_171_OPT_HELP.rs) | P1 | memory | CLI_OPTION | 1447761 | 1582817 | <span style="color:#2563eb">47.24%</span> |
| 387 | [00439 DML_WHERE_ORDER_LIMIT_052](crates/bench/sqlite_parity/cases/SQLITE_PARITY_439_DML_WHERE_ORDER_LIMIT_052.rs) | P1 | memory | GEN_SQL_DML | 1749753 | 1582747 | <span style="color:#2563eb">47.24%</span> |
| 388 | [00056 SUBQUERIES_EXISTS_IN](crates/bench/sqlite_parity/cases/SQLITE_PARITY_056_SUBQUERIES_EXISTS_IN.rs) | P0 | memory | SQL_SELECT | 2101509 | 1582306 | <span style="color:#2563eb">47.26%</span> |
| 389 | [00822 WINDOW_PARTITION_SUM_035](crates/bench/sqlite_parity/cases/SQLITE_PARITY_822_WINDOW_PARTITION_SUM_035.rs) | P2 | memory | GEN_SQL_WINDOW | 1682767 | 1582256 | <span style="color:#2563eb">47.26%</span> |
| 390 | [00089 JSON_TABLE_VALUED_FUNCTIONS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_089_JSON_TABLE_VALUED_FUNCTIONS.rs) | P0 | memory | SQL_JSON | 1714046 | 1582036 | <span style="color:#2563eb">47.27%</span> |
| 391 | [00849 WINDOW_PARTITION_SUM_062](crates/bench/sqlite_parity/cases/SQLITE_PARITY_849_WINDOW_PARTITION_SUM_062.rs) | P2 | memory | GEN_SQL_WINDOW | 1680312 | 1581285 | <span style="color:#2563eb">47.29%</span> |
| 392 | [00622 JOIN_SUBQUERY_EXISTS_015](crates/bench/sqlite_parity/cases/SQLITE_PARITY_622_JOIN_SUBQUERY_EXISTS_015.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1738322 | 1581214 | <span style="color:#2563eb">47.29%</span> |
| 393 | [00999 VIEW_TRIGGER_GENERATED_052](crates/bench/sqlite_parity/cases/SQLITE_PARITY_999_VIEW_TRIGGER_GENERATED_052.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1788247 | 1580934 | <span style="color:#2563eb">47.30%</span> |
| 394 | [00323 SCALAR_NULL_COALESCE_024](crates/bench/sqlite_parity/cases/SQLITE_PARITY_323_SCALAR_NULL_COALESCE_024.rs) | P1 | memory | GEN_SQL_SCALAR | 2383885 | 1580904 | <span style="color:#2563eb">47.30%</span> |
| 395 | [00516 AGG_GROUP_HAVING_009](crates/bench/sqlite_parity/cases/SQLITE_PARITY_516_AGG_GROUP_HAVING_009.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2628106 | 1580894 | <span style="color:#2563eb">47.30%</span> |
| 396 | [00081 BLOBS_HEX_ZEROBLOB](crates/bench/sqlite_parity/cases/SQLITE_PARITY_081_BLOBS_HEX_ZEROBLOB.rs) | P0 | memory | SQL_FUNCTIONS | 1506001 | 1580844 | <span style="color:#2563eb">47.31%</span> |
| 397 | [00833 WINDOW_PARTITION_SUM_046](crates/bench/sqlite_parity/cases/SQLITE_PARITY_833_WINDOW_PARTITION_SUM_046.rs) | P2 | memory | GEN_SQL_WINDOW | 1644354 | 1580253 | <span style="color:#2563eb">47.32%</span> |
| 398 | [00063 WINDOW_EXCLUDE_CURRENT_ROW](crates/bench/sqlite_parity/cases/SQLITE_PARITY_063_WINDOW_EXCLUDE_CURRENT_ROW.rs) | P0 | memory | SQL_WINDOW | 1650346 | 1580182 | <span style="color:#2563eb">47.33%</span> |
| 399 | [00314 SCALAR_STRING_022](crates/bench/sqlite_parity/cases/SQLITE_PARITY_314_SCALAR_STRING_022.rs) | P1 | memory | GEN_SQL_SCALAR | 1500511 | 1579962 | <span style="color:#2563eb">47.33%</span> |
| 400 | [00468 DML_WHERE_ORDER_LIMIT_081](crates/bench/sqlite_parity/cases/SQLITE_PARITY_468_DML_WHERE_ORDER_LIMIT_081.rs) | P1 | memory | GEN_SQL_DML | 1677727 | 1579932 | <span style="color:#2563eb">47.34%</span> |
| 401 | [00448 DML_WHERE_ORDER_LIMIT_061](crates/bench/sqlite_parity/cases/SQLITE_PARITY_448_DML_WHERE_ORDER_LIMIT_061.rs) | P1 | memory | GEN_SQL_DML | 1652890 | 1579822 | <span style="color:#2563eb">47.34%</span> |
| 402 | [00006 ROWID_INTEGER_PRIMARY_KEY](crates/bench/sqlite_parity/cases/SQLITE_PARITY_006_ROWID_INTEGER_PRIMARY_KEY.rs) | P0 | memory | SQL_ROWID | 1878868 | 1579801 | <span style="color:#2563eb">47.34%</span> |
| 403 | [00655 JOIN_SUBQUERY_EXISTS_048](crates/bench/sqlite_parity/cases/SQLITE_PARITY_655_JOIN_SUBQUERY_EXISTS_048.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1730126 | 1579731 | <span style="color:#2563eb">47.34%</span> |
| 404 | [00835 WINDOW_PARTITION_SUM_048](crates/bench/sqlite_parity/cases/SQLITE_PARITY_835_WINDOW_PARTITION_SUM_048.rs) | P2 | memory | GEN_SQL_WINDOW | 2061925 | 1579080 | <span style="color:#2563eb">47.36%</span> |
| 405 | [01122 INDEX_SCHEMA_PRAGMA_055](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1122_INDEX_SCHEMA_PRAGMA_055.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1770422 | 1579010 | <span style="color:#2563eb">47.37%</span> |
| 406 | [00615 JOIN_SUBQUERY_EXISTS_008](crates/bench/sqlite_parity/cases/SQLITE_PARITY_615_JOIN_SUBQUERY_EXISTS_008.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1711672 | 1578740 | <span style="color:#2563eb">47.38%</span> |
| 407 | [00415 DML_WHERE_ORDER_LIMIT_028](crates/bench/sqlite_parity/cases/SQLITE_PARITY_415_DML_WHERE_ORDER_LIMIT_028.rs) | P1 | memory | GEN_SQL_DML | 2646121 | 1578719 | <span style="color:#2563eb">47.38%</span> |
| 408 | [00618 JOIN_SUBQUERY_EXISTS_011](crates/bench/sqlite_parity/cases/SQLITE_PARITY_618_JOIN_SUBQUERY_EXISTS_011.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1741839 | 1578719 | <span style="color:#2563eb">47.38%</span> |
| 409 | [00017 DELETE_BASIC](crates/bench/sqlite_parity/cases/SQLITE_PARITY_017_DELETE_BASIC.rs) | P0 | memory | SQL_DELETE | 1879980 | 1578550 | <span style="color:#2563eb">47.38%</span> |
| 410 | [01042 JSON_EXTRACT_SET_035](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1042_JSON_EXTRACT_SET_035.rs) | P2 | memory | GEN_SQL_JSON | 1615609 | 1578499 | <span style="color:#2563eb">47.38%</span> |
| 411 | [00003 CREATE_TABLE_INSERT_SELECT](crates/bench/sqlite_parity/cases/SQLITE_PARITY_003_CREATE_TABLE_INSERT_SELECT.rs) | P0 | memory | SQL_DDL_DML | 1818904 | 1578389 | <span style="color:#2563eb">47.39%</span> |
| 412 | [01063 JSON_EXTRACT_SET_056](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1063_JSON_EXTRACT_SET_056.rs) | P2 | memory | GEN_SQL_JSON | 1611140 | 1578228 | <span style="color:#2563eb">47.39%</span> |
| 413 | [00058 GROUP_BY_HAVING](crates/bench/sqlite_parity/cases/SQLITE_PARITY_058_GROUP_BY_HAVING.rs) | P0 | memory | SQL_AGGREGATE | 2031527 | 1578148 | <span style="color:#2563eb">47.40%</span> |
| 414 | [00863 WINDOW_PARTITION_SUM_076](crates/bench/sqlite_parity/cases/SQLITE_PARITY_863_WINDOW_PARTITION_SUM_076.rs) | P2 | memory | GEN_SQL_WINDOW | 2000769 | 1577968 | <span style="color:#2563eb">47.40%</span> |
| 415 | [00325 SCALAR_CAST_TYPEOF_025](crates/bench/sqlite_parity/cases/SQLITE_PARITY_325_SCALAR_CAST_TYPEOF_025.rs) | P1 | memory | GEN_SQL_SCALAR | 1677156 | 1577868 | <span style="color:#2563eb">47.40%</span> |
| 416 | [00440 DML_WHERE_ORDER_LIMIT_053](crates/bench/sqlite_parity/cases/SQLITE_PARITY_440_DML_WHERE_ORDER_LIMIT_053.rs) | P1 | memory | GEN_SQL_DML | 1647159 | 1577777 | <span style="color:#2563eb">47.41%</span> |
| 417 | [01047 JSON_EXTRACT_SET_040](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1047_JSON_EXTRACT_SET_040.rs) | P2 | memory | GEN_SQL_JSON | 1905959 | 1577668 | <span style="color:#2563eb">47.41%</span> |
| 418 | [00276 SCALAR_ARITH_013](crates/bench/sqlite_parity/cases/SQLITE_PARITY_276_SCALAR_ARITH_013.rs) | P1 | memory | GEN_SQL_SCALAR | 1505882 | 1577568 | <span style="color:#2563eb">47.41%</span> |
| 419 | [00554 AGG_GROUP_HAVING_047](crates/bench/sqlite_parity/cases/SQLITE_PARITY_554_AGG_GROUP_HAVING_047.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1673940 | 1576826 | <span style="color:#2563eb">47.44%</span> |
| 420 | [00489 DML_WHERE_ORDER_LIMIT_102](crates/bench/sqlite_parity/cases/SQLITE_PARITY_489_DML_WHERE_ORDER_LIMIT_102.rs) | P1 | memory | GEN_SQL_DML | 1704387 | 1576676 | <span style="color:#2563eb">47.44%</span> |
| 421 | [00925 CONSTRAINT_FK_SAVEPOINT_058](crates/bench/sqlite_parity/cases/SQLITE_PARITY_925_CONSTRAINT_FK_SAVEPOINT_058.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1729545 | 1576515 | <span style="color:#2563eb">47.45%</span> |
| 422 | [01074 INDEX_SCHEMA_PRAGMA_007](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1074_INDEX_SCHEMA_PRAGMA_007.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1768279 | 1576376 | <span style="color:#2563eb">47.45%</span> |
| 423 | [00353 SCALAR_CAST_TYPEOF_032](crates/bench/sqlite_parity/cases/SQLITE_PARITY_353_SCALAR_CAST_TYPEOF_032.rs) | P1 | memory | GEN_SQL_SCALAR | 1611792 | 1576355 | <span style="color:#2563eb">47.45%</span> |
| 424 | [00596 AGG_GROUP_HAVING_089](crates/bench/sqlite_parity/cases/SQLITE_PARITY_596_AGG_GROUP_HAVING_089.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1749022 | 1576225 | <span style="color:#2563eb">47.46%</span> |
| 425 | [00329 SCALAR_CAST_TYPEOF_026](crates/bench/sqlite_parity/cases/SQLITE_PARITY_329_SCALAR_CAST_TYPEOF_026.rs) | P1 | memory | GEN_SQL_SCALAR | 1515199 | 1576074 | <span style="color:#2563eb">47.46%</span> |
| 426 | [00395 DML_WHERE_ORDER_LIMIT_008](crates/bench/sqlite_parity/cases/SQLITE_PARITY_395_DML_WHERE_ORDER_LIMIT_008.rs) | P1 | memory | GEN_SQL_DML | 1689579 | 1576044 | <span style="color:#2563eb">47.47%</span> |
| 427 | [00996 VIEW_TRIGGER_GENERATED_049](crates/bench/sqlite_parity/cases/SQLITE_PARITY_996_VIEW_TRIGGER_GENERATED_049.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1795681 | 1575794 | <span style="color:#2563eb">47.47%</span> |
| 428 | [01029 JSON_EXTRACT_SET_022](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1029_JSON_EXTRACT_SET_022.rs) | P2 | memory | GEN_SQL_JSON | 1602514 | 1575784 | <span style="color:#2563eb">47.47%</span> |
| 429 | [00673 JOIN_SUBQUERY_EXISTS_066](crates/bench/sqlite_parity/cases/SQLITE_PARITY_673_JOIN_SUBQUERY_EXISTS_066.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1976262 | 1575664 | <span style="color:#2563eb">47.48%</span> |
| 430 | [00602 AGG_GROUP_HAVING_095](crates/bench/sqlite_parity/cases/SQLITE_PARITY_602_AGG_GROUP_HAVING_095.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1717001 | 1575634 | <span style="color:#2563eb">47.48%</span> |
| 431 | [00692 JOIN_SUBQUERY_EXISTS_085](crates/bench/sqlite_parity/cases/SQLITE_PARITY_692_JOIN_SUBQUERY_EXISTS_085.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1943781 | 1575454 | <span style="color:#2563eb">47.48%</span> |
| 432 | [00281 SCALAR_CAST_TYPEOF_014](crates/bench/sqlite_parity/cases/SQLITE_PARITY_281_SCALAR_CAST_TYPEOF_014.rs) | P1 | memory | GEN_SQL_SCALAR | 1533875 | 1575423 | <span style="color:#2563eb">47.49%</span> |
| 433 | [00244 SCALAR_ARITH_005](crates/bench/sqlite_parity/cases/SQLITE_PARITY_244_SCALAR_ARITH_005.rs) | P1 | memory | GEN_SQL_SCALAR | 1464102 | 1575333 | <span style="color:#2563eb">47.49%</span> |
| 434 | [00952 VIEW_TRIGGER_GENERATED_005](crates/bench/sqlite_parity/cases/SQLITE_PARITY_952_VIEW_TRIGGER_GENERATED_005.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1792153 | 1575313 | <span style="color:#2563eb">47.49%</span> |
| 435 | [01015 JSON_EXTRACT_SET_008](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1015_JSON_EXTRACT_SET_008.rs) | P2 | memory | GEN_SQL_JSON | 1637341 | 1575313 | <span style="color:#2563eb">47.49%</span> |
| 436 | [00437 DML_WHERE_ORDER_LIMIT_050](crates/bench/sqlite_parity/cases/SQLITE_PARITY_437_DML_WHERE_ORDER_LIMIT_050.rs) | P1 | memory | GEN_SQL_DML | 1898295 | 1575303 | <span style="color:#2563eb">47.49%</span> |
| 437 | [00765 CTE_RECURSIVE_MATRIX_058](crates/bench/sqlite_parity/cases/SQLITE_PARITY_765_CTE_RECURSIVE_MATRIX_058.rs) | P1 | memory | GEN_SQL_CTE | 1603687 | 1575293 | <span style="color:#2563eb">47.49%</span> |
| 438 | [00687 JOIN_SUBQUERY_EXISTS_080](crates/bench/sqlite_parity/cases/SQLITE_PARITY_687_JOIN_SUBQUERY_EXISTS_080.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1762888 | 1575263 | <span style="color:#2563eb">47.49%</span> |
| 439 | [00824 WINDOW_PARTITION_SUM_037](crates/bench/sqlite_parity/cases/SQLITE_PARITY_824_WINDOW_PARTITION_SUM_037.rs) | P2 | memory | GEN_SQL_WINDOW | 1685723 | 1575103 | <span style="color:#2563eb">47.50%</span> |
| 440 | [00246 SCALAR_STRING_005](crates/bench/sqlite_parity/cases/SQLITE_PARITY_246_SCALAR_STRING_005.rs) | P1 | memory | GEN_SQL_SCALAR | 1773698 | 1575032 | <span style="color:#2563eb">47.50%</span> |
| 441 | [01124 INDEX_SCHEMA_PRAGMA_057](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1124_INDEX_SCHEMA_PRAGMA_057.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1690811 | 1574962 | <span style="color:#2563eb">47.50%</span> |
| 442 | [00410 DML_WHERE_ORDER_LIMIT_023](crates/bench/sqlite_parity/cases/SQLITE_PARITY_410_DML_WHERE_ORDER_LIMIT_023.rs) | P1 | memory | GEN_SQL_DML | 1689889 | 1574902 | <span style="color:#2563eb">47.50%</span> |
| 443 | [00233 SCALAR_CAST_TYPEOF_002](crates/bench/sqlite_parity/cases/SQLITE_PARITY_233_SCALAR_CAST_TYPEOF_002.rs) | P1 | memory | GEN_SQL_SCALAR | 1498567 | 1574873 | <span style="color:#2563eb">47.50%</span> |
| 444 | [00497 DML_WHERE_ORDER_LIMIT_110](crates/bench/sqlite_parity/cases/SQLITE_PARITY_497_DML_WHERE_ORDER_LIMIT_110.rs) | P1 | memory | GEN_SQL_DML | 1971623 | 1574832 | <span style="color:#2563eb">47.51%</span> |
| 445 | [00446 DML_WHERE_ORDER_LIMIT_059](crates/bench/sqlite_parity/cases/SQLITE_PARITY_446_DML_WHERE_ORDER_LIMIT_059.rs) | P1 | memory | GEN_SQL_DML | 1822762 | 1574552 | <span style="color:#2563eb">47.51%</span> |
| 446 | [00705 JOIN_SUBQUERY_EXISTS_098](crates/bench/sqlite_parity/cases/SQLITE_PARITY_705_JOIN_SUBQUERY_EXISTS_098.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1704518 | 1574542 | <span style="color:#2563eb">47.52%</span> |
| 447 | [00315 SCALAR_NULL_COALESCE_022](crates/bench/sqlite_parity/cases/SQLITE_PARITY_315_SCALAR_NULL_COALESCE_022.rs) | P1 | memory | GEN_SQL_SCALAR | 1506172 | 1574531 | <span style="color:#2563eb">47.52%</span> |
| 448 | [01028 JSON_EXTRACT_SET_021](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1028_JSON_EXTRACT_SET_021.rs) | P2 | memory | GEN_SQL_JSON | 1662608 | 1574432 | <span style="color:#2563eb">47.52%</span> |
| 449 | [00587 AGG_GROUP_HAVING_080](crates/bench/sqlite_parity/cases/SQLITE_PARITY_587_AGG_GROUP_HAVING_080.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2099776 | 1574161 | <span style="color:#2563eb">47.53%</span> |
| 450 | [00218 PRAGMA_FORMS_SCHEMA_EQUALS_PARENS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_218_PRAGMA_FORMS_SCHEMA_EQUALS_PARENS.rs) | P0 | memory | SQL_PRAGMA | 2084908 | 1574080 | <span style="color:#2563eb">47.53%</span> |
| 451 | [00270 SCALAR_STRING_011](crates/bench/sqlite_parity/cases/SQLITE_PARITY_270_SCALAR_STRING_011.rs) | P1 | memory | GEN_SQL_SCALAR | 1559153 | 1573981 | <span style="color:#2563eb">47.53%</span> |
| 452 | [01127 INDEX_SCHEMA_PRAGMA_060](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1127_INDEX_SCHEMA_PRAGMA_060.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1740566 | 1573921 | <span style="color:#2563eb">47.54%</span> |
| 453 | [00573 AGG_GROUP_HAVING_066](crates/bench/sqlite_parity/cases/SQLITE_PARITY_573_AGG_GROUP_HAVING_066.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1743622 | 1573901 | <span style="color:#2563eb">47.54%</span> |
| 454 | [01119 INDEX_SCHEMA_PRAGMA_052](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1119_INDEX_SCHEMA_PRAGMA_052.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 2159108 | 1573870 | <span style="color:#2563eb">47.54%</span> |
| 455 | [00515 AGG_GROUP_HAVING_008](crates/bench/sqlite_parity/cases/SQLITE_PARITY_515_AGG_GROUP_HAVING_008.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1947768 | 1573720 | <span style="color:#2563eb">47.54%</span> |
| 456 | [00896 CONSTRAINT_FK_SAVEPOINT_029](crates/bench/sqlite_parity/cases/SQLITE_PARITY_896_CONSTRAINT_FK_SAVEPOINT_029.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1715137 | 1573710 | <span style="color:#2563eb">47.54%</span> |
| 457 | [01073 INDEX_SCHEMA_PRAGMA_006](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1073_INDEX_SCHEMA_PRAGMA_006.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1726910 | 1573710 | <span style="color:#2563eb">47.54%</span> |
| 458 | [00850 WINDOW_PARTITION_SUM_063](crates/bench/sqlite_parity/cases/SQLITE_PARITY_850_WINDOW_PARTITION_SUM_063.rs) | P2 | memory | GEN_SQL_WINDOW | 1666095 | 1573420 | <span style="color:#2563eb">47.55%</span> |
| 459 | [00648 JOIN_SUBQUERY_EXISTS_041](crates/bench/sqlite_parity/cases/SQLITE_PARITY_648_JOIN_SUBQUERY_EXISTS_041.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1719015 | 1573419 | <span style="color:#2563eb">47.55%</span> |
| 460 | [00414 DML_WHERE_ORDER_LIMIT_027](crates/bench/sqlite_parity/cases/SQLITE_PARITY_414_DML_WHERE_ORDER_LIMIT_027.rs) | P1 | memory | GEN_SQL_DML | 1796292 | 1573350 | <span style="color:#2563eb">47.55%</span> |
| 461 | [00748 CTE_RECURSIVE_MATRIX_041](crates/bench/sqlite_parity/cases/SQLITE_PARITY_748_CTE_RECURSIVE_MATRIX_041.rs) | P1 | memory | GEN_SQL_CTE | 1558611 | 1573049 | <span style="color:#2563eb">47.57%</span> |
| 462 | [01052 JSON_EXTRACT_SET_045](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1052_JSON_EXTRACT_SET_045.rs) | P2 | memory | GEN_SQL_JSON | 1633814 | 1573039 | <span style="color:#2563eb">47.57%</span> |
| 463 | [00632 JOIN_SUBQUERY_EXISTS_025](crates/bench/sqlite_parity/cases/SQLITE_PARITY_632_JOIN_SUBQUERY_EXISTS_025.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1746858 | 1572838 | <span style="color:#2563eb">47.57%</span> |
| 464 | [00589 AGG_GROUP_HAVING_082](crates/bench/sqlite_parity/cases/SQLITE_PARITY_589_AGG_GROUP_HAVING_082.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1707433 | 1572759 | <span style="color:#2563eb">47.57%</span> |
| 465 | [00533 AGG_GROUP_HAVING_026](crates/bench/sqlite_parity/cases/SQLITE_PARITY_533_AGG_GROUP_HAVING_026.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1727130 | 1572708 | <span style="color:#2563eb">47.58%</span> |
| 466 | [00942 CONSTRAINT_FK_SAVEPOINT_075](crates/bench/sqlite_parity/cases/SQLITE_PARITY_942_CONSTRAINT_FK_SAVEPOINT_075.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1740877 | 1572608 | <span style="color:#2563eb">47.58%</span> |
| 467 | [00845 WINDOW_PARTITION_SUM_058](crates/bench/sqlite_parity/cases/SQLITE_PARITY_845_WINDOW_PARTITION_SUM_058.rs) | P2 | memory | GEN_SQL_WINDOW | 1944692 | 1572447 | <span style="color:#2563eb">47.59%</span> |
| 468 | [00944 CONSTRAINT_FK_SAVEPOINT_077](crates/bench/sqlite_parity/cases/SQLITE_PARITY_944_CONSTRAINT_FK_SAVEPOINT_077.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1717493 | 1572287 | <span style="color:#2563eb">47.59%</span> |
| 469 | [00676 JOIN_SUBQUERY_EXISTS_069](crates/bench/sqlite_parity/cases/SQLITE_PARITY_676_JOIN_SUBQUERY_EXISTS_069.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1741517 | 1572237 | <span style="color:#2563eb">47.59%</span> |
| 470 | [00396 DML_WHERE_ORDER_LIMIT_009](crates/bench/sqlite_parity/cases/SQLITE_PARITY_396_DML_WHERE_ORDER_LIMIT_009.rs) | P1 | memory | GEN_SQL_DML | 1712823 | 1572208 | <span style="color:#2563eb">47.59%</span> |
| 471 | [01070 INDEX_SCHEMA_PRAGMA_003](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1070_INDEX_SCHEMA_PRAGMA_003.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1756276 | 1572127 | <span style="color:#2563eb">47.60%</span> |
| 472 | [00611 JOIN_SUBQUERY_EXISTS_004](crates/bench/sqlite_parity/cases/SQLITE_PARITY_611_JOIN_SUBQUERY_EXISTS_004.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1707804 | 1572097 | <span style="color:#2563eb">47.60%</span> |
| 473 | [00493 DML_WHERE_ORDER_LIMIT_106](crates/bench/sqlite_parity/cases/SQLITE_PARITY_493_DML_WHERE_ORDER_LIMIT_106.rs) | P1 | memory | GEN_SQL_DML | 1659612 | 1572087 | <span style="color:#2563eb">47.60%</span> |
| 474 | [01009 JSON_EXTRACT_SET_002](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1009_JSON_EXTRACT_SET_002.rs) | P2 | memory | GEN_SQL_JSON | 1652820 | 1571826 | <span style="color:#2563eb">47.61%</span> |
| 475 | [00864 WINDOW_PARTITION_SUM_077](crates/bench/sqlite_parity/cases/SQLITE_PARITY_864_WINDOW_PARTITION_SUM_077.rs) | P2 | memory | GEN_SQL_WINDOW | 1674241 | 1571716 | <span style="color:#2563eb">47.61%</span> |
| 476 | [00672 JOIN_SUBQUERY_EXISTS_065](crates/bench/sqlite_parity/cases/SQLITE_PARITY_672_JOIN_SUBQUERY_EXISTS_065.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1737219 | 1571276 | <span style="color:#2563eb">47.62%</span> |
| 477 | [01034 JSON_EXTRACT_SET_027](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1034_JSON_EXTRACT_SET_027.rs) | P2 | memory | GEN_SQL_JSON | 1640476 | 1571195 | <span style="color:#2563eb">47.63%</span> |
| 478 | [00319 SCALAR_NULL_COALESCE_023](crates/bench/sqlite_parity/cases/SQLITE_PARITY_319_SCALAR_NULL_COALESCE_023.rs) | P1 | memory | GEN_SQL_SCALAR | 1742900 | 1570965 | <span style="color:#2563eb">47.63%</span> |
| 479 | [00978 VIEW_TRIGGER_GENERATED_031](crates/bench/sqlite_parity/cases/SQLITE_PARITY_978_VIEW_TRIGGER_GENERATED_031.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1796252 | 1570854 | <span style="color:#2563eb">47.64%</span> |
| 480 | [01048 JSON_EXTRACT_SET_041](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1048_JSON_EXTRACT_SET_041.rs) | P2 | memory | GEN_SQL_JSON | 1670063 | 1570584 | <span style="color:#2563eb">47.65%</span> |
| 481 | [00894 CONSTRAINT_FK_SAVEPOINT_027](crates/bench/sqlite_parity/cases/SQLITE_PARITY_894_CONSTRAINT_FK_SAVEPOINT_027.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2088064 | 1570444 | <span style="color:#2563eb">47.65%</span> |
| 482 | [00732 CTE_RECURSIVE_MATRIX_025](crates/bench/sqlite_parity/cases/SQLITE_PARITY_732_CTE_RECURSIVE_MATRIX_025.rs) | P1 | memory | GEN_SQL_CTE | 2141305 | 1570233 | <span style="color:#2563eb">47.66%</span> |
| 483 | [00037 DROP_VIEW](crates/bench/sqlite_parity/cases/SQLITE_PARITY_037_DROP_VIEW.rs) | P0 | memory | SQL_DROP | 2137799 | 1570153 | <span style="color:#2563eb">47.66%</span> |
| 484 | [00840 WINDOW_PARTITION_SUM_053](crates/bench/sqlite_parity/cases/SQLITE_PARITY_840_WINDOW_PARTITION_SUM_053.rs) | P2 | memory | GEN_SQL_WINDOW | 1685371 | 1569673 | <span style="color:#2563eb">47.68%</span> |
| 485 | [00339 SCALAR_NULL_COALESCE_028](crates/bench/sqlite_parity/cases/SQLITE_PARITY_339_SCALAR_NULL_COALESCE_028.rs) | P1 | memory | GEN_SQL_SCALAR | 1536580 | 1569582 | <span style="color:#2563eb">47.68%</span> |
| 486 | [00706 JOIN_SUBQUERY_EXISTS_099](crates/bench/sqlite_parity/cases/SQLITE_PARITY_706_JOIN_SUBQUERY_EXISTS_099.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1766024 | 1569552 | <span style="color:#2563eb">47.68%</span> |
| 487 | [00623 JOIN_SUBQUERY_EXISTS_016](crates/bench/sqlite_parity/cases/SQLITE_PARITY_623_JOIN_SUBQUERY_EXISTS_016.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2040554 | 1569532 | <span style="color:#2563eb">47.68%</span> |
| 488 | [00042 TEMP_TABLE_TEMP_SCHEMA](crates/bench/sqlite_parity/cases/SQLITE_PARITY_042_TEMP_TABLE_TEMP_SCHEMA.rs) | P0 | memory | SQL_TEMP | 1613475 | 1569512 | <span style="color:#2563eb">47.68%</span> |
| 489 | [00146 DOT_READ_TEMPFILE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_146_DOT_READ_TEMPFILE.rs) | P1 | tempfile | CLI_TEMPFILE | 1709427 | 1569432 | <span style="color:#2563eb">47.69%</span> |
| 490 | [00643 JOIN_SUBQUERY_EXISTS_036](crates/bench/sqlite_parity/cases/SQLITE_PARITY_643_JOIN_SUBQUERY_EXISTS_036.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1737771 | 1569172 | <span style="color:#2563eb">47.69%</span> |
| 491 | [00628 JOIN_SUBQUERY_EXISTS_021](crates/bench/sqlite_parity/cases/SQLITE_PARITY_628_JOIN_SUBQUERY_EXISTS_021.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1699297 | 1569122 | <span style="color:#2563eb">47.70%</span> |
| 492 | [00759 CTE_RECURSIVE_MATRIX_052](crates/bench/sqlite_parity/cases/SQLITE_PARITY_759_CTE_RECURSIVE_MATRIX_052.rs) | P1 | memory | GEN_SQL_CTE | 1583308 | 1569021 | <span style="color:#2563eb">47.70%</span> |
| 493 | [00981 VIEW_TRIGGER_GENERATED_034](crates/bench/sqlite_parity/cases/SQLITE_PARITY_981_VIEW_TRIGGER_GENERATED_034.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1831067 | 1568961 | <span style="color:#2563eb">47.70%</span> |
| 494 | [00661 JOIN_SUBQUERY_EXISTS_054](crates/bench/sqlite_parity/cases/SQLITE_PARITY_661_JOIN_SUBQUERY_EXISTS_054.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1816179 | 1568650 | <span style="color:#2563eb">47.71%</span> |
| 495 | [00893 CONSTRAINT_FK_SAVEPOINT_026](crates/bench/sqlite_parity/cases/SQLITE_PARITY_893_CONSTRAINT_FK_SAVEPOINT_026.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1740816 | 1568570 | <span style="color:#2563eb">47.71%</span> |
| 496 | [00365 SCALAR_CAST_TYPEOF_035](crates/bench/sqlite_parity/cases/SQLITE_PARITY_365_SCALAR_CAST_TYPEOF_035.rs) | P1 | memory | GEN_SQL_SCALAR | 1793586 | 1568410 | <span style="color:#2563eb">47.72%</span> |
| 497 | [00547 AGG_GROUP_HAVING_040](crates/bench/sqlite_parity/cases/SQLITE_PARITY_547_AGG_GROUP_HAVING_040.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1735106 | 1568279 | <span style="color:#2563eb">47.72%</span> |
| 498 | [00512 AGG_GROUP_HAVING_005](crates/bench/sqlite_parity/cases/SQLITE_PARITY_512_AGG_GROUP_HAVING_005.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1884478 | 1568270 | <span style="color:#2563eb">47.72%</span> |
| 499 | [00528 AGG_GROUP_HAVING_021](crates/bench/sqlite_parity/cases/SQLITE_PARITY_528_AGG_GROUP_HAVING_021.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1751677 | 1568190 | <span style="color:#2563eb">47.73%</span> |
| 500 | [00972 VIEW_TRIGGER_GENERATED_025](crates/bench/sqlite_parity/cases/SQLITE_PARITY_972_VIEW_TRIGGER_GENERATED_025.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1801732 | 1568079 | <span style="color:#2563eb">47.73%</span> |
| 501 | [00491 DML_WHERE_ORDER_LIMIT_104](crates/bench/sqlite_parity/cases/SQLITE_PARITY_491_DML_WHERE_ORDER_LIMIT_104.rs) | P1 | memory | GEN_SQL_DML | 1723524 | 1567839 | <span style="color:#2563eb">47.74%</span> |
| 502 | [00466 DML_WHERE_ORDER_LIMIT_079](crates/bench/sqlite_parity/cases/SQLITE_PARITY_466_DML_WHERE_ORDER_LIMIT_079.rs) | P1 | memory | GEN_SQL_DML | 2018613 | 1567789 | <span style="color:#2563eb">47.74%</span> |
| 503 | [00249 SCALAR_CAST_TYPEOF_006](crates/bench/sqlite_parity/cases/SQLITE_PARITY_249_SCALAR_CAST_TYPEOF_006.rs) | P1 | memory | GEN_SQL_SCALAR | 1496895 | 1567639 | <span style="color:#2563eb">47.75%</span> |
| 504 | [00300 SCALAR_ARITH_019](crates/bench/sqlite_parity/cases/SQLITE_PARITY_300_SCALAR_ARITH_019.rs) | P1 | memory | GEN_SQL_SCALAR | 1900719 | 1567549 | <span style="color:#2563eb">47.75%</span> |
| 505 | [00749 CTE_RECURSIVE_MATRIX_042](crates/bench/sqlite_parity/cases/SQLITE_PARITY_749_CTE_RECURSIVE_MATRIX_042.rs) | P1 | memory | GEN_SQL_CTE | 1663670 | 1567489 | <span style="color:#2563eb">47.75%</span> |
| 506 | [01057 JSON_EXTRACT_SET_050](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1057_JSON_EXTRACT_SET_050.rs) | P2 | memory | GEN_SQL_JSON | 1637892 | 1567288 | <span style="color:#2563eb">47.76%</span> |
| 507 | [00322 SCALAR_STRING_024](crates/bench/sqlite_parity/cases/SQLITE_PARITY_322_SCALAR_STRING_024.rs) | P1 | memory | GEN_SQL_SCALAR | 1483378 | 1567238 | <span style="color:#2563eb">47.76%</span> |
| 508 | [00303 SCALAR_NULL_COALESCE_019](crates/bench/sqlite_parity/cases/SQLITE_PARITY_303_SCALAR_NULL_COALESCE_019.rs) | P1 | memory | GEN_SQL_SCALAR | 1540387 | 1567198 | <span style="color:#2563eb">47.76%</span> |
| 509 | [00377 SCALAR_CAST_TYPEOF_038](crates/bench/sqlite_parity/cases/SQLITE_PARITY_377_SCALAR_CAST_TYPEOF_038.rs) | P1 | memory | GEN_SQL_SCALAR | 1584380 | 1567198 | <span style="color:#2563eb">47.76%</span> |
| 510 | [00724 CTE_RECURSIVE_MATRIX_017](crates/bench/sqlite_parity/cases/SQLITE_PARITY_724_CTE_RECURSIVE_MATRIX_017.rs) | P1 | memory | GEN_SQL_CTE | 2486488 | 1567168 | <span style="color:#2563eb">47.76%</span> |
| 511 | [00068 CAST_AND_TYPE_AFFINITY](crates/bench/sqlite_parity/cases/SQLITE_PARITY_068_CAST_AND_TYPE_AFFINITY.rs) | P0 | memory | SQL_EXPRESSIONS | 1583599 | 1567158 | <span style="color:#2563eb">47.76%</span> |
| 512 | [00235 SCALAR_NULL_COALESCE_002](crates/bench/sqlite_parity/cases/SQLITE_PARITY_235_SCALAR_NULL_COALESCE_002.rs) | P1 | memory | GEN_SQL_SCALAR | 1496414 | 1567097 | <span style="color:#2563eb">47.76%</span> |
| 513 | [00525 AGG_GROUP_HAVING_018](crates/bench/sqlite_parity/cases/SQLITE_PARITY_525_AGG_GROUP_HAVING_018.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1797193 | 1567078 | <span style="color:#2563eb">47.76%</span> |
| 514 | [00584 AGG_GROUP_HAVING_077](crates/bench/sqlite_parity/cases/SQLITE_PARITY_584_AGG_GROUP_HAVING_077.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1836829 | 1567057 | <span style="color:#2563eb">47.76%</span> |
| 515 | [00985 VIEW_TRIGGER_GENERATED_038](crates/bench/sqlite_parity/cases/SQLITE_PARITY_985_VIEW_TRIGGER_GENERATED_038.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2075300 | 1566817 | <span style="color:#2563eb">47.77%</span> |
| 516 | [00310 SCALAR_STRING_021](crates/bench/sqlite_parity/cases/SQLITE_PARITY_310_SCALAR_STRING_021.rs) | P1 | memory | GEN_SQL_SCALAR | 1435859 | 1566767 | <span style="color:#2563eb">47.77%</span> |
| 517 | [01072 INDEX_SCHEMA_PRAGMA_005](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1072_INDEX_SCHEMA_PRAGMA_005.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1823513 | 1566767 | <span style="color:#2563eb">47.77%</span> |
| 518 | [00453 DML_WHERE_ORDER_LIMIT_066](crates/bench/sqlite_parity/cases/SQLITE_PARITY_453_DML_WHERE_ORDER_LIMIT_066.rs) | P1 | memory | GEN_SQL_DML | 1696632 | 1566676 | <span style="color:#2563eb">47.78%</span> |
| 519 | [00106 DOT_HELP](crates/bench/sqlite_parity/cases/SQLITE_PARITY_106_DOT_HELP.rs) | P0 | memory | CLI_DOT_COMMAND | 1862777 | 1566526 | <span style="color:#2563eb">47.78%</span> |
| 520 | [00631 JOIN_SUBQUERY_EXISTS_024](crates/bench/sqlite_parity/cases/SQLITE_PARITY_631_JOIN_SUBQUERY_EXISTS_024.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1762136 | 1566296 | <span style="color:#2563eb">47.79%</span> |
| 521 | [00496 DML_WHERE_ORDER_LIMIT_109](crates/bench/sqlite_parity/cases/SQLITE_PARITY_496_DML_WHERE_ORDER_LIMIT_109.rs) | P1 | memory | GEN_SQL_DML | 1847458 | 1566276 | <span style="color:#2563eb">47.79%</span> |
| 522 | [01011 JSON_EXTRACT_SET_004](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1011_JSON_EXTRACT_SET_004.rs) | P2 | memory | GEN_SQL_JSON | 1616061 | 1566186 | <span style="color:#2563eb">47.79%</span> |
| 523 | [01036 JSON_EXTRACT_SET_029](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1036_JSON_EXTRACT_SET_029.rs) | P2 | memory | GEN_SQL_JSON | 1676374 | 1566006 | <span style="color:#2563eb">47.80%</span> |
| 524 | [00428 DML_WHERE_ORDER_LIMIT_041](crates/bench/sqlite_parity/cases/SQLITE_PARITY_428_DML_WHERE_ORDER_LIMIT_041.rs) | P1 | memory | GEN_SQL_DML | 1688918 | 1565996 | <span style="color:#2563eb">47.80%</span> |
| 525 | [00427 DML_WHERE_ORDER_LIMIT_040](crates/bench/sqlite_parity/cases/SQLITE_PARITY_427_DML_WHERE_ORDER_LIMIT_040.rs) | P1 | memory | GEN_SQL_DML | 1617082 | 1565855 | <span style="color:#2563eb">47.80%</span> |
| 526 | [00514 AGG_GROUP_HAVING_007](crates/bench/sqlite_parity/cases/SQLITE_PARITY_514_AGG_GROUP_HAVING_007.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1676795 | 1565795 | <span style="color:#2563eb">47.81%</span> |
| 527 | [00565 AGG_GROUP_HAVING_058](crates/bench/sqlite_parity/cases/SQLITE_PARITY_565_AGG_GROUP_HAVING_058.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1802984 | 1565785 | <span style="color:#2563eb">47.81%</span> |
| 528 | [00975 VIEW_TRIGGER_GENERATED_028](crates/bench/sqlite_parity/cases/SQLITE_PARITY_975_VIEW_TRIGGER_GENERATED_028.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1767557 | 1565775 | <span style="color:#2563eb">47.81%</span> |
| 529 | [01054 JSON_EXTRACT_SET_047](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1054_JSON_EXTRACT_SET_047.rs) | P2 | memory | GEN_SQL_JSON | 1632812 | 1565705 | <span style="color:#2563eb">47.81%</span> |
| 530 | [01111 INDEX_SCHEMA_PRAGMA_044](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1111_INDEX_SCHEMA_PRAGMA_044.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1791883 | 1565705 | <span style="color:#2563eb">47.81%</span> |
| 531 | [00304 SCALAR_ARITH_020](crates/bench/sqlite_parity/cases/SQLITE_PARITY_304_SCALAR_ARITH_020.rs) | P1 | memory | GEN_SQL_SCALAR | 1516141 | 1565594 | <span style="color:#2563eb">47.81%</span> |
| 532 | [00798 WINDOW_PARTITION_SUM_011](crates/bench/sqlite_parity/cases/SQLITE_PARITY_798_WINDOW_PARTITION_SUM_011.rs) | P2 | memory | GEN_SQL_WINDOW | 2117410 | 1565465 | <span style="color:#2563eb">47.82%</span> |
| 533 | [00717 CTE_RECURSIVE_MATRIX_010](crates/bench/sqlite_parity/cases/SQLITE_PARITY_717_CTE_RECURSIVE_MATRIX_010.rs) | P1 | memory | GEN_SQL_CTE | 1793006 | 1565445 | <span style="color:#2563eb">47.82%</span> |
| 534 | [00499 DML_WHERE_ORDER_LIMIT_112](crates/bench/sqlite_parity/cases/SQLITE_PARITY_499_DML_WHERE_ORDER_LIMIT_112.rs) | P1 | memory | GEN_SQL_DML | 1684710 | 1565414 | <span style="color:#2563eb">47.82%</span> |
| 535 | [00848 WINDOW_PARTITION_SUM_061](crates/bench/sqlite_parity/cases/SQLITE_PARITY_848_WINDOW_PARTITION_SUM_061.rs) | P2 | memory | GEN_SQL_WINDOW | 2072224 | 1565414 | <span style="color:#2563eb">47.82%</span> |
| 536 | [00654 JOIN_SUBQUERY_EXISTS_047](crates/bench/sqlite_parity/cases/SQLITE_PARITY_654_JOIN_SUBQUERY_EXISTS_047.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1709066 | 1565394 | <span style="color:#2563eb">47.82%</span> |
| 537 | [00574 AGG_GROUP_HAVING_067](crates/bench/sqlite_parity/cases/SQLITE_PARITY_574_AGG_GROUP_HAVING_067.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1734234 | 1565355 | <span style="color:#2563eb">47.82%</span> |
| 538 | [00383 SCALAR_NULL_COALESCE_039](crates/bench/sqlite_parity/cases/SQLITE_PARITY_383_SCALAR_NULL_COALESCE_039.rs) | P1 | memory | GEN_SQL_SCALAR | 1573350 | 1565174 | <span style="color:#2563eb">47.83%</span> |
| 539 | [00670 JOIN_SUBQUERY_EXISTS_063](crates/bench/sqlite_parity/cases/SQLITE_PARITY_670_JOIN_SUBQUERY_EXISTS_063.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1773809 | 1565104 | <span style="color:#2563eb">47.83%</span> |
| 540 | [01007 VIEW_TRIGGER_GENERATED_060](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1007_VIEW_TRIGGER_GENERATED_060.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1738322 | 1564713 | <span style="color:#2563eb">47.84%</span> |
| 541 | [00012 INSERT_DEFAULT_VALUES](crates/bench/sqlite_parity/cases/SQLITE_PARITY_012_INSERT_DEFAULT_VALUES.rs) | P0 | memory | SQL_INSERT | 1561847 | 1564703 | <span style="color:#2563eb">47.84%</span> |
| 542 | [00521 AGG_GROUP_HAVING_014](crates/bench/sqlite_parity/cases/SQLITE_PARITY_521_AGG_GROUP_HAVING_014.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1685802 | 1564472 | <span style="color:#2563eb">47.85%</span> |
| 543 | [00330 SCALAR_STRING_026](crates/bench/sqlite_parity/cases/SQLITE_PARITY_330_SCALAR_STRING_026.rs) | P1 | memory | GEN_SQL_SCALAR | 1523685 | 1564462 | <span style="color:#2563eb">47.85%</span> |
| 544 | [00744 CTE_RECURSIVE_MATRIX_037](crates/bench/sqlite_parity/cases/SQLITE_PARITY_744_CTE_RECURSIVE_MATRIX_037.rs) | P1 | memory | GEN_SQL_CTE | 1539365 | 1564442 | <span style="color:#2563eb">47.85%</span> |
| 545 | [00043 ATTACH_DETACH_MEMORY](crates/bench/sqlite_parity/cases/SQLITE_PARITY_043_ATTACH_DETACH_MEMORY.rs) | P0 | memory | SQL_ATTACH | 1543293 | 1564402 | <span style="color:#2563eb">47.85%</span> |
| 546 | [00532 AGG_GROUP_HAVING_025](crates/bench/sqlite_parity/cases/SQLITE_PARITY_532_AGG_GROUP_HAVING_025.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1715459 | 1564402 | <span style="color:#2563eb">47.85%</span> |
| 547 | [00795 WINDOW_PARTITION_SUM_008](crates/bench/sqlite_parity/cases/SQLITE_PARITY_795_WINDOW_PARTITION_SUM_008.rs) | P2 | memory | GEN_SQL_WINDOW | 1682296 | 1564293 | <span style="color:#2563eb">47.86%</span> |
| 548 | [00990 VIEW_TRIGGER_GENERATED_043](crates/bench/sqlite_parity/cases/SQLITE_PARITY_990_VIEW_TRIGGER_GENERATED_043.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1768358 | 1564152 | <span style="color:#2563eb">47.86%</span> |
| 549 | [00527 AGG_GROUP_HAVING_020](crates/bench/sqlite_parity/cases/SQLITE_PARITY_527_AGG_GROUP_HAVING_020.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1773990 | 1564112 | <span style="color:#2563eb">47.86%</span> |
| 550 | [00802 WINDOW_PARTITION_SUM_015](crates/bench/sqlite_parity/cases/SQLITE_PARITY_802_WINDOW_PARTITION_SUM_015.rs) | P2 | memory | GEN_SQL_WINDOW | 2032358 | 1563941 | <span style="color:#2563eb">47.87%</span> |
| 551 | [00272 SCALAR_ARITH_012](crates/bench/sqlite_parity/cases/SQLITE_PARITY_272_SCALAR_ARITH_012.rs) | P1 | memory | GEN_SQL_SCALAR | 1633634 | 1563861 | <span style="color:#2563eb">47.87%</span> |
| 552 | [00846 WINDOW_PARTITION_SUM_059](crates/bench/sqlite_parity/cases/SQLITE_PARITY_846_WINDOW_PARTITION_SUM_059.rs) | P2 | memory | GEN_SQL_WINDOW | 1628534 | 1563761 | <span style="color:#2563eb">47.87%</span> |
| 553 | [00812 WINDOW_PARTITION_SUM_025](crates/bench/sqlite_parity/cases/SQLITE_PARITY_812_WINDOW_PARTITION_SUM_025.rs) | P2 | memory | GEN_SQL_WINDOW | 1646808 | 1563741 | <span style="color:#2563eb">47.88%</span> |
| 554 | [00616 JOIN_SUBQUERY_EXISTS_009](crates/bench/sqlite_parity/cases/SQLITE_PARITY_616_JOIN_SUBQUERY_EXISTS_009.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1802343 | 1563711 | <span style="color:#2563eb">47.88%</span> |
| 555 | [00412 DML_WHERE_ORDER_LIMIT_025](crates/bench/sqlite_parity/cases/SQLITE_PARITY_412_DML_WHERE_ORDER_LIMIT_025.rs) | P1 | memory | GEN_SQL_DML | 2211969 | 1563571 | <span style="color:#2563eb">47.88%</span> |
| 556 | [00819 WINDOW_PARTITION_SUM_032](crates/bench/sqlite_parity/cases/SQLITE_PARITY_819_WINDOW_PARTITION_SUM_032.rs) | P2 | memory | GEN_SQL_WINDOW | 1679140 | 1563431 | <span style="color:#2563eb">47.89%</span> |
| 557 | [00100 SCHEMA_SQLITE_SCHEMA](crates/bench/sqlite_parity/cases/SQLITE_PARITY_100_SCHEMA_SQLITE_SCHEMA.rs) | P0 | memory | SQL_SCHEMA | 1578078 | 1563350 | <span style="color:#2563eb">47.89%</span> |
| 558 | [00783 CTE_RECURSIVE_MATRIX_076](crates/bench/sqlite_parity/cases/SQLITE_PARITY_783_CTE_RECURSIVE_MATRIX_076.rs) | P1 | memory | GEN_SQL_CTE | 1951936 | 1563350 | <span style="color:#2563eb">47.89%</span> |
| 559 | [00742 CTE_RECURSIVE_MATRIX_035](crates/bench/sqlite_parity/cases/SQLITE_PARITY_742_CTE_RECURSIVE_MATRIX_035.rs) | P1 | memory | GEN_SQL_CTE | 1621150 | 1563321 | <span style="color:#2563eb">47.89%</span> |
| 560 | [01051 JSON_EXTRACT_SET_044](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1051_JSON_EXTRACT_SET_044.rs) | P2 | memory | GEN_SQL_JSON | 1653761 | 1563280 | <span style="color:#2563eb">47.89%</span> |
| 561 | [01126 INDEX_SCHEMA_PRAGMA_059](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1126_INDEX_SCHEMA_PRAGMA_059.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1805198 | 1563270 | <span style="color:#2563eb">47.89%</span> |
| 562 | [00023 TRANSACTION_ROLLBACK](crates/bench/sqlite_parity/cases/SQLITE_PARITY_023_TRANSACTION_ROLLBACK.rs) | P0 | memory | SQL_TRANSACTION | 1585863 | 1563190 | <span style="color:#2563eb">47.89%</span> |
| 563 | [00477 DML_WHERE_ORDER_LIMIT_090](crates/bench/sqlite_parity/cases/SQLITE_PARITY_477_DML_WHERE_ORDER_LIMIT_090.rs) | P1 | memory | GEN_SQL_DML | 1644825 | 1563160 | <span style="color:#2563eb">47.89%</span> |
| 564 | [01050 JSON_EXTRACT_SET_043](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1050_JSON_EXTRACT_SET_043.rs) | P2 | memory | GEN_SQL_JSON | 1672247 | 1562930 | <span style="color:#2563eb">47.90%</span> |
| 565 | [00963 VIEW_TRIGGER_GENERATED_016](crates/bench/sqlite_parity/cases/SQLITE_PARITY_963_VIEW_TRIGGER_GENERATED_016.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1747489 | 1562840 | <span style="color:#2563eb">47.91%</span> |
| 566 | [00792 WINDOW_PARTITION_SUM_005](crates/bench/sqlite_parity/cases/SQLITE_PARITY_792_WINDOW_PARTITION_SUM_005.rs) | P2 | memory | GEN_SQL_WINDOW | 1660224 | 1562769 | <span style="color:#2563eb">47.91%</span> |
| 567 | [00230 SCALAR_STRING_001](crates/bench/sqlite_parity/cases/SQLITE_PARITY_230_SCALAR_STRING_001.rs) | P1 | memory | GEN_SQL_SCALAR | 1712623 | 1562759 | <span style="color:#2563eb">47.91%</span> |
| 568 | [00823 WINDOW_PARTITION_SUM_036](crates/bench/sqlite_parity/cases/SQLITE_PARITY_823_WINDOW_PARTITION_SUM_036.rs) | P2 | memory | GEN_SQL_WINDOW | 1672127 | 1562669 | <span style="color:#2563eb">47.91%</span> |
| 569 | [00961 VIEW_TRIGGER_GENERATED_014](crates/bench/sqlite_parity/cases/SQLITE_PARITY_961_VIEW_TRIGGER_GENERATED_014.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2012250 | 1562609 | <span style="color:#2563eb">47.91%</span> |
| 570 | [00518 AGG_GROUP_HAVING_011](crates/bench/sqlite_parity/cases/SQLITE_PARITY_518_AGG_GROUP_HAVING_011.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1705931 | 1562569 | <span style="color:#2563eb">47.91%</span> |
| 571 | [00691 JOIN_SUBQUERY_EXISTS_084](crates/bench/sqlite_parity/cases/SQLITE_PARITY_691_JOIN_SUBQUERY_EXISTS_084.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1760163 | 1562479 | <span style="color:#2563eb">47.92%</span> |
| 572 | [00398 DML_WHERE_ORDER_LIMIT_011](crates/bench/sqlite_parity/cases/SQLITE_PARITY_398_DML_WHERE_ORDER_LIMIT_011.rs) | P1 | memory | GEN_SQL_DML | 1703145 | 1562189 | <span style="color:#2563eb">47.93%</span> |
| 573 | [00465 DML_WHERE_ORDER_LIMIT_078](crates/bench/sqlite_parity/cases/SQLITE_PARITY_465_DML_WHERE_ORDER_LIMIT_078.rs) | P1 | memory | GEN_SQL_DML | 1662037 | 1562138 | <span style="color:#2563eb">47.93%</span> |
| 574 | [00813 WINDOW_PARTITION_SUM_026](crates/bench/sqlite_parity/cases/SQLITE_PARITY_813_WINDOW_PARTITION_SUM_026.rs) | P2 | memory | GEN_SQL_WINDOW | 1680121 | 1561977 | <span style="color:#2563eb">47.93%</span> |
| 575 | [00862 WINDOW_PARTITION_SUM_075](crates/bench/sqlite_parity/cases/SQLITE_PARITY_862_WINDOW_PARTITION_SUM_075.rs) | P2 | memory | GEN_SQL_WINDOW | 2023301 | 1561767 | <span style="color:#2563eb">47.94%</span> |
| 576 | [00482 DML_WHERE_ORDER_LIMIT_095](crates/bench/sqlite_parity/cases/SQLITE_PARITY_482_DML_WHERE_ORDER_LIMIT_095.rs) | P1 | memory | GEN_SQL_DML | 1649473 | 1561747 | <span style="color:#2563eb">47.94%</span> |
| 577 | [00567 AGG_GROUP_HAVING_060](crates/bench/sqlite_parity/cases/SQLITE_PARITY_567_AGG_GROUP_HAVING_060.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1714426 | 1561747 | <span style="color:#2563eb">47.94%</span> |
| 578 | [00881 CONSTRAINT_FK_SAVEPOINT_014](crates/bench/sqlite_parity/cases/SQLITE_PARITY_881_CONSTRAINT_FK_SAVEPOINT_014.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1743060 | 1561677 | <span style="color:#2563eb">47.94%</span> |
| 579 | [00640 JOIN_SUBQUERY_EXISTS_033](crates/bench/sqlite_parity/cases/SQLITE_PARITY_640_JOIN_SUBQUERY_EXISTS_033.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1739013 | 1561426 | <span style="color:#2563eb">47.95%</span> |
| 580 | [00022 TRANSACTION_COMMIT](crates/bench/sqlite_parity/cases/SQLITE_PARITY_022_TRANSACTION_COMMIT.rs) | P0 | memory | SQL_TRANSACTION | 1597586 | 1561267 | <span style="color:#2563eb">47.96%</span> |
| 581 | [01104 INDEX_SCHEMA_PRAGMA_037](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1104_INDEX_SCHEMA_PRAGMA_037.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1722793 | 1561266 | <span style="color:#2563eb">47.96%</span> |
| 582 | [00839 WINDOW_PARTITION_SUM_052](crates/bench/sqlite_parity/cases/SQLITE_PARITY_839_WINDOW_PARTITION_SUM_052.rs) | P2 | memory | GEN_SQL_WINDOW | 2327527 | 1561126 | <span style="color:#2563eb">47.96%</span> |
| 583 | [00086 DATE_TIME_FUNCTIONS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_086_DATE_TIME_FUNCTIONS.rs) | P0 | memory | SQL_FUNCTIONS | 1524377 | 1560926 | <span style="color:#2563eb">47.97%</span> |
| 584 | [00639 JOIN_SUBQUERY_EXISTS_032](crates/bench/sqlite_parity/cases/SQLITE_PARITY_639_JOIN_SUBQUERY_EXISTS_032.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1732961 | 1560906 | <span style="color:#2563eb">47.97%</span> |
| 585 | [00731 CTE_RECURSIVE_MATRIX_024](crates/bench/sqlite_parity/cases/SQLITE_PARITY_731_CTE_RECURSIVE_MATRIX_024.rs) | P1 | memory | GEN_SQL_CTE | 2852973 | 1560845 | <span style="color:#2563eb">47.97%</span> |
| 586 | [01120 INDEX_SCHEMA_PRAGMA_053](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1120_INDEX_SCHEMA_PRAGMA_053.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 2052688 | 1560666 | <span style="color:#2563eb">47.98%</span> |
| 587 | [00877 CONSTRAINT_FK_SAVEPOINT_010](crates/bench/sqlite_parity/cases/SQLITE_PARITY_877_CONSTRAINT_FK_SAVEPOINT_010.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1761415 | 1560656 | <span style="color:#2563eb">47.98%</span> |
| 588 | [00362 SCALAR_STRING_034](crates/bench/sqlite_parity/cases/SQLITE_PARITY_362_SCALAR_STRING_034.rs) | P1 | memory | GEN_SQL_SCALAR | 1805499 | 1560515 | <span style="color:#2563eb">47.98%</span> |
| 589 | [00882 CONSTRAINT_FK_SAVEPOINT_015](crates/bench/sqlite_parity/cases/SQLITE_PARITY_882_CONSTRAINT_FK_SAVEPOINT_015.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1732241 | 1560465 | <span style="color:#2563eb">47.98%</span> |
| 590 | [01069 INDEX_SCHEMA_PRAGMA_002](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1069_INDEX_SCHEMA_PRAGMA_002.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1738121 | 1560405 | <span style="color:#2563eb">47.99%</span> |
| 591 | [00386 SCALAR_STRING_040](crates/bench/sqlite_parity/cases/SQLITE_PARITY_386_SCALAR_STRING_040.rs) | P1 | memory | GEN_SQL_SCALAR | 1900669 | 1560365 | <span style="color:#2563eb">47.99%</span> |
| 592 | [00038 CREATE_TRIGGER_AFTER](crates/bench/sqlite_parity/cases/SQLITE_PARITY_038_CREATE_TRIGGER_AFTER.rs) | P0 | memory | SQL_TRIGGER | 1681595 | 1560354 | <span style="color:#2563eb">47.99%</span> |
| 593 | [00607 AGG_GROUP_HAVING_100](crates/bench/sqlite_parity/cases/SQLITE_PARITY_607_AGG_GROUP_HAVING_100.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1749924 | 1560344 | <span style="color:#2563eb">47.99%</span> |
| 594 | [00694 JOIN_SUBQUERY_EXISTS_087](crates/bench/sqlite_parity/cases/SQLITE_PARITY_694_JOIN_SUBQUERY_EXISTS_087.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1728924 | 1560285 | <span style="color:#2563eb">47.99%</span> |
| 595 | [00979 VIEW_TRIGGER_GENERATED_032](crates/bench/sqlite_parity/cases/SQLITE_PARITY_979_VIEW_TRIGGER_GENERATED_032.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1779360 | 1560245 | <span style="color:#2563eb">47.99%</span> |
| 596 | [00299 SCALAR_NULL_COALESCE_018](crates/bench/sqlite_parity/cases/SQLITE_PARITY_299_SCALAR_NULL_COALESCE_018.rs) | P1 | memory | GEN_SQL_SCALAR | 1518505 | 1560214 | <span style="color:#2563eb">47.99%</span> |
| 597 | [00689 JOIN_SUBQUERY_EXISTS_082](crates/bench/sqlite_parity/cases/SQLITE_PARITY_689_JOIN_SUBQUERY_EXISTS_082.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1962236 | 1559984 | <span style="color:#2563eb">48.00%</span> |
| 598 | [00912 CONSTRAINT_FK_SAVEPOINT_045](crates/bench/sqlite_parity/cases/SQLITE_PARITY_912_CONSTRAINT_FK_SAVEPOINT_045.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1708325 | 1559974 | <span style="color:#2563eb">48.00%</span> |
| 599 | [00721 CTE_RECURSIVE_MATRIX_014](crates/bench/sqlite_parity/cases/SQLITE_PARITY_721_CTE_RECURSIVE_MATRIX_014.rs) | P1 | memory | GEN_SQL_CTE | 1604198 | 1559854 | <span style="color:#2563eb">48.00%</span> |
| 600 | [00703 JOIN_SUBQUERY_EXISTS_096](crates/bench/sqlite_parity/cases/SQLITE_PARITY_703_JOIN_SUBQUERY_EXISTS_096.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1778848 | 1559804 | <span style="color:#2563eb">48.01%</span> |
| 601 | [00967 VIEW_TRIGGER_GENERATED_020](crates/bench/sqlite_parity/cases/SQLITE_PARITY_967_VIEW_TRIGGER_GENERATED_020.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1776805 | 1559763 | <span style="color:#2563eb">48.01%</span> |
| 602 | [00787 CTE_RECURSIVE_MATRIX_080](crates/bench/sqlite_parity/cases/SQLITE_PARITY_787_CTE_RECURSIVE_MATRIX_080.rs) | P1 | memory | GEN_SQL_CTE | 1608707 | 1559703 | <span style="color:#2563eb">48.01%</span> |
| 603 | [01078 INDEX_SCHEMA_PRAGMA_011](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1078_INDEX_SCHEMA_PRAGMA_011.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1710299 | 1559654 | <span style="color:#2563eb">48.01%</span> |
| 604 | [00675 JOIN_SUBQUERY_EXISTS_068](crates/bench/sqlite_parity/cases/SQLITE_PARITY_675_JOIN_SUBQUERY_EXISTS_068.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1730657 | 1559633 | <span style="color:#2563eb">48.01%</span> |
| 605 | [00258 SCALAR_STRING_008](crates/bench/sqlite_parity/cases/SQLITE_PARITY_258_SCALAR_STRING_008.rs) | P1 | memory | GEN_SQL_SCALAR | 1463181 | 1559223 | <span style="color:#2563eb">48.03%</span> |
| 606 | [00481 DML_WHERE_ORDER_LIMIT_094](crates/bench/sqlite_parity/cases/SQLITE_PARITY_481_DML_WHERE_ORDER_LIMIT_094.rs) | P1 | memory | GEN_SQL_DML | 1658751 | 1559012 | <span style="color:#2563eb">48.03%</span> |
| 607 | [00374 SCALAR_STRING_037](crates/bench/sqlite_parity/cases/SQLITE_PARITY_374_SCALAR_STRING_037.rs) | P1 | memory | GEN_SQL_SCALAR | 1678358 | 1558942 | <span style="color:#2563eb">48.04%</span> |
| 608 | [00911 CONSTRAINT_FK_SAVEPOINT_044](crates/bench/sqlite_parity/cases/SQLITE_PARITY_911_CONSTRAINT_FK_SAVEPOINT_044.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1706501 | 1558781 | <span style="color:#2563eb">48.04%</span> |
| 609 | [01099 INDEX_SCHEMA_PRAGMA_032](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1099_INDEX_SCHEMA_PRAGMA_032.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1927460 | 1558751 | <span style="color:#2563eb">48.04%</span> |
| 610 | [00770 CTE_RECURSIVE_MATRIX_063](crates/bench/sqlite_parity/cases/SQLITE_PARITY_770_CTE_RECURSIVE_MATRIX_063.rs) | P1 | memory | GEN_SQL_CTE | 1986582 | 1558712 | <span style="color:#2563eb">48.04%</span> |
| 611 | [00715 CTE_RECURSIVE_MATRIX_008](crates/bench/sqlite_parity/cases/SQLITE_PARITY_715_CTE_RECURSIVE_MATRIX_008.rs) | P1 | memory | GEN_SQL_CTE | 1627703 | 1558682 | <span style="color:#2563eb">48.04%</span> |
| 612 | [00242 SCALAR_STRING_004](crates/bench/sqlite_parity/cases/SQLITE_PARITY_242_SCALAR_STRING_004.rs) | P1 | memory | GEN_SQL_SCALAR | 1631861 | 1558471 | <span style="color:#2563eb">48.05%</span> |
| 613 | [01013 JSON_EXTRACT_SET_006](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1013_JSON_EXTRACT_SET_006.rs) | P2 | memory | GEN_SQL_JSON | 1593518 | 1558341 | <span style="color:#2563eb">48.06%</span> |
| 614 | [00120 DOT_EXPLAIN](crates/bench/sqlite_parity/cases/SQLITE_PARITY_120_DOT_EXPLAIN.rs) | P0 | memory | CLI_DOT_COMMAND | 1480043 | 1558291 | <span style="color:#2563eb">48.06%</span> |
| 615 | [00851 WINDOW_PARTITION_SUM_064](crates/bench/sqlite_parity/cases/SQLITE_PARITY_851_WINDOW_PARTITION_SUM_064.rs) | P2 | memory | GEN_SQL_WINDOW | 1671766 | 1558161 | <span style="color:#2563eb">48.06%</span> |
| 616 | [00678 JOIN_SUBQUERY_EXISTS_071](crates/bench/sqlite_parity/cases/SQLITE_PARITY_678_JOIN_SUBQUERY_EXISTS_071.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1730697 | 1558141 | <span style="color:#2563eb">48.06%</span> |
| 617 | [00785 CTE_RECURSIVE_MATRIX_078](crates/bench/sqlite_parity/cases/SQLITE_PARITY_785_CTE_RECURSIVE_MATRIX_078.rs) | P1 | memory | GEN_SQL_CTE | 1607785 | 1558101 | <span style="color:#2563eb">48.06%</span> |
| 618 | [00020 UPSERT_DO_UPDATE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_020_UPSERT_DO_UPDATE.rs) | P0 | memory | SQL_UPSERT | 1671465 | 1557991 | <span style="color:#2563eb">48.07%</span> |
| 619 | [00454 DML_WHERE_ORDER_LIMIT_067](crates/bench/sqlite_parity/cases/SQLITE_PARITY_454_DML_WHERE_ORDER_LIMIT_067.rs) | P1 | memory | GEN_SQL_DML | 1662789 | 1557840 | <span style="color:#2563eb">48.07%</span> |
| 620 | [00829 WINDOW_PARTITION_SUM_042](crates/bench/sqlite_parity/cases/SQLITE_PARITY_829_WINDOW_PARTITION_SUM_042.rs) | P2 | memory | GEN_SQL_WINDOW | 1680492 | 1557680 | <span style="color:#2563eb">48.08%</span> |
| 621 | [00662 JOIN_SUBQUERY_EXISTS_055](crates/bench/sqlite_parity/cases/SQLITE_PARITY_662_JOIN_SUBQUERY_EXISTS_055.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1801552 | 1557650 | <span style="color:#2563eb">48.08%</span> |
| 622 | [00501 DML_WHERE_ORDER_LIMIT_114](crates/bench/sqlite_parity/cases/SQLITE_PARITY_501_DML_WHERE_ORDER_LIMIT_114.rs) | P1 | memory | GEN_SQL_DML | 1654924 | 1557609 | <span style="color:#2563eb">48.08%</span> |
| 623 | [00834 WINDOW_PARTITION_SUM_047](crates/bench/sqlite_parity/cases/SQLITE_PARITY_834_WINDOW_PARTITION_SUM_047.rs) | P2 | memory | GEN_SQL_WINDOW | 1641708 | 1557600 | <span style="color:#2563eb">48.08%</span> |
| 624 | [00392 DML_WHERE_ORDER_LIMIT_005](crates/bench/sqlite_parity/cases/SQLITE_PARITY_392_DML_WHERE_ORDER_LIMIT_005.rs) | P1 | memory | GEN_SQL_DML | 1714055 | 1557560 | <span style="color:#2563eb">48.08%</span> |
| 625 | [00690 JOIN_SUBQUERY_EXISTS_083](crates/bench/sqlite_parity/cases/SQLITE_PARITY_690_JOIN_SUBQUERY_EXISTS_083.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1743722 | 1557549 | <span style="color:#2563eb">48.08%</span> |
| 626 | [00267 SCALAR_NULL_COALESCE_010](crates/bench/sqlite_parity/cases/SQLITE_PARITY_267_SCALAR_NULL_COALESCE_010.rs) | P1 | memory | GEN_SQL_SCALAR | 1531991 | 1557539 | <span style="color:#2563eb">48.08%</span> |
| 627 | [00847 WINDOW_PARTITION_SUM_060](crates/bench/sqlite_parity/cases/SQLITE_PARITY_847_WINDOW_PARTITION_SUM_060.rs) | P2 | memory | GEN_SQL_WINDOW | 1636308 | 1557379 | <span style="color:#2563eb">48.09%</span> |
| 628 | [00977 VIEW_TRIGGER_GENERATED_030](crates/bench/sqlite_parity/cases/SQLITE_PARITY_977_VIEW_TRIGGER_GENERATED_030.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1849713 | 1557319 | <span style="color:#2563eb">48.09%</span> |
| 629 | [00461 DML_WHERE_ORDER_LIMIT_074](crates/bench/sqlite_parity/cases/SQLITE_PARITY_461_DML_WHERE_ORDER_LIMIT_074.rs) | P1 | memory | GEN_SQL_DML | 1904677 | 1557289 | <span style="color:#2563eb">48.09%</span> |
| 630 | [00953 VIEW_TRIGGER_GENERATED_006](crates/bench/sqlite_parity/cases/SQLITE_PARITY_953_VIEW_TRIGGER_GENERATED_006.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1876213 | 1557159 | <span style="color:#2563eb">48.09%</span> |
| 631 | [00522 AGG_GROUP_HAVING_015](crates/bench/sqlite_parity/cases/SQLITE_PARITY_522_AGG_GROUP_HAVING_015.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1750134 | 1557119 | <span style="color:#2563eb">48.10%</span> |
| 632 | [01040 JSON_EXTRACT_SET_033](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1040_JSON_EXTRACT_SET_033.rs) | P2 | memory | GEN_SQL_JSON | 1639084 | 1557068 | <span style="color:#2563eb">48.10%</span> |
| 633 | [00083 CORE_NUMERIC_FUNCTIONS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_083_CORE_NUMERIC_FUNCTIONS.rs) | P0 | memory | SQL_FUNCTIONS | 1499229 | 1556878 | <span style="color:#2563eb">48.10%</span> |
| 634 | [00543 AGG_GROUP_HAVING_036](crates/bench/sqlite_parity/cases/SQLITE_PARITY_543_AGG_GROUP_HAVING_036.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1717612 | 1556858 | <span style="color:#2563eb">48.10%</span> |
| 635 | [00382 SCALAR_STRING_039](crates/bench/sqlite_parity/cases/SQLITE_PARITY_382_SCALAR_STRING_039.rs) | P1 | memory | GEN_SQL_SCALAR | 1563070 | 1556828 | <span style="color:#2563eb">48.11%</span> |
| 636 | [00571 AGG_GROUP_HAVING_064](crates/bench/sqlite_parity/cases/SQLITE_PARITY_571_AGG_GROUP_HAVING_064.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1695822 | 1556738 | <span style="color:#2563eb">48.11%</span> |
| 637 | [00693 JOIN_SUBQUERY_EXISTS_086](crates/bench/sqlite_parity/cases/SQLITE_PARITY_693_JOIN_SUBQUERY_EXISTS_086.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2113753 | 1556738 | <span style="color:#2563eb">48.11%</span> |
| 638 | [00933 CONSTRAINT_FK_SAVEPOINT_066](crates/bench/sqlite_parity/cases/SQLITE_PARITY_933_CONSTRAINT_FK_SAVEPOINT_066.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1691333 | 1556688 | <span style="color:#2563eb">48.11%</span> |
| 639 | [01125 INDEX_SCHEMA_PRAGMA_058](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1125_INDEX_SCHEMA_PRAGMA_058.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1721380 | 1556537 | <span style="color:#2563eb">48.12%</span> |
| 640 | [00595 AGG_GROUP_HAVING_088](crates/bench/sqlite_parity/cases/SQLITE_PARITY_595_AGG_GROUP_HAVING_088.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1727341 | 1556477 | <span style="color:#2563eb">48.12%</span> |
| 641 | [00788 WINDOW_PARTITION_SUM_001](crates/bench/sqlite_parity/cases/SQLITE_PARITY_788_WINDOW_PARTITION_SUM_001.rs) | P2 | memory | GEN_SQL_WINDOW | 2181521 | 1556447 | <span style="color:#2563eb">48.12%</span> |
| 642 | [00645 JOIN_SUBQUERY_EXISTS_038](crates/bench/sqlite_parity/cases/SQLITE_PARITY_645_JOIN_SUBQUERY_EXISTS_038.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1737961 | 1556388 | <span style="color:#2563eb">48.12%</span> |
| 643 | [00062 WINDOW_FRAMES_ROWS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_062_WINDOW_FRAMES_ROWS.rs) | P0 | memory | SQL_WINDOW | 1560445 | 1556247 | <span style="color:#2563eb">48.13%</span> |
| 644 | [00368 SCALAR_ARITH_036](crates/bench/sqlite_parity/cases/SQLITE_PARITY_368_SCALAR_ARITH_036.rs) | P1 | memory | GEN_SQL_SCALAR | 1520960 | 1556217 | <span style="color:#2563eb">48.13%</span> |
| 645 | [00814 WINDOW_PARTITION_SUM_027](crates/bench/sqlite_parity/cases/SQLITE_PARITY_814_WINDOW_PARTITION_SUM_027.rs) | P2 | memory | GEN_SQL_WINDOW | 1654383 | 1556177 | <span style="color:#2563eb">48.13%</span> |
| 646 | [00774 CTE_RECURSIVE_MATRIX_067](crates/bench/sqlite_parity/cases/SQLITE_PARITY_774_CTE_RECURSIVE_MATRIX_067.rs) | P1 | memory | GEN_SQL_CTE | 1617022 | 1556066 | <span style="color:#2563eb">48.13%</span> |
| 647 | [00793 WINDOW_PARTITION_SUM_006](crates/bench/sqlite_parity/cases/SQLITE_PARITY_793_WINDOW_PARTITION_SUM_006.rs) | P2 | memory | GEN_SQL_WINDOW | 1650586 | 1556066 | <span style="color:#2563eb">48.13%</span> |
| 648 | [01053 JSON_EXTRACT_SET_046](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1053_JSON_EXTRACT_SET_046.rs) | P2 | memory | GEN_SQL_JSON | 1656276 | 1556016 | <span style="color:#2563eb">48.13%</span> |
| 649 | [01055 JSON_EXTRACT_SET_048](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1055_JSON_EXTRACT_SET_048.rs) | P2 | memory | GEN_SQL_JSON | 1656156 | 1555996 | <span style="color:#2563eb">48.13%</span> |
| 650 | [00284 SCALAR_ARITH_015](crates/bench/sqlite_parity/cases/SQLITE_PARITY_284_SCALAR_ARITH_015.rs) | P1 | memory | GEN_SQL_SCALAR | 1505371 | 1555986 | <span style="color:#2563eb">48.13%</span> |
| 651 | [01103 INDEX_SCHEMA_PRAGMA_036](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1103_INDEX_SCHEMA_PRAGMA_036.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1739494 | 1555956 | <span style="color:#2563eb">48.13%</span> |
| 652 | [00680 JOIN_SUBQUERY_EXISTS_073](crates/bench/sqlite_parity/cases/SQLITE_PARITY_680_JOIN_SUBQUERY_EXISTS_073.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1773618 | 1555936 | <span style="color:#2563eb">48.14%</span> |
| 653 | [00015 UPDATE_BASIC](crates/bench/sqlite_parity/cases/SQLITE_PARITY_015_UPDATE_BASIC.rs) | P0 | memory | SQL_UPDATE | 1584551 | 1555916 | <span style="color:#2563eb">48.14%</span> |
| 654 | [00124 DOT_BAIL_OFF](crates/bench/sqlite_parity/cases/SQLITE_PARITY_124_DOT_BAIL_OFF.rs) | P0 | memory | CLI_DOT_COMMAND_NEGATIVE | 1512354 | 1555876 | <span style="color:#2563eb">48.14%</span> |
| 655 | [00229 SCALAR_CAST_TYPEOF_001](crates/bench/sqlite_parity/cases/SQLITE_PARITY_229_SCALAR_CAST_TYPEOF_001.rs) | P1 | memory | GEN_SQL_SCALAR | 1842790 | 1555827 | <span style="color:#2563eb">48.14%</span> |
| 656 | [01016 JSON_EXTRACT_SET_009](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1016_JSON_EXTRACT_SET_009.rs) | P2 | memory | GEN_SQL_JSON | 1633794 | 1555807 | <span style="color:#2563eb">48.14%</span> |
| 657 | [00348 SCALAR_ARITH_031](crates/bench/sqlite_parity/cases/SQLITE_PARITY_348_SCALAR_ARITH_031.rs) | P1 | memory | GEN_SQL_SCALAR | 1641649 | 1555736 | <span style="color:#2563eb">48.14%</span> |
| 658 | [00641 JOIN_SUBQUERY_EXISTS_034](crates/bench/sqlite_parity/cases/SQLITE_PARITY_641_JOIN_SUBQUERY_EXISTS_034.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1764942 | 1555736 | <span style="color:#2563eb">48.14%</span> |
| 659 | [00431 DML_WHERE_ORDER_LIMIT_044](crates/bench/sqlite_parity/cases/SQLITE_PARITY_431_DML_WHERE_ORDER_LIMIT_044.rs) | P1 | memory | GEN_SQL_DML | 1667067 | 1555666 | <span style="color:#2563eb">48.14%</span> |
| 660 | [00773 CTE_RECURSIVE_MATRIX_066](crates/bench/sqlite_parity/cases/SQLITE_PARITY_773_CTE_RECURSIVE_MATRIX_066.rs) | P1 | memory | GEN_SQL_CTE | 1606763 | 1555416 | <span style="color:#2563eb">48.15%</span> |
| 661 | [00397 DML_WHERE_ORDER_LIMIT_010](crates/bench/sqlite_parity/cases/SQLITE_PARITY_397_DML_WHERE_ORDER_LIMIT_010.rs) | P1 | memory | GEN_SQL_DML | 1608415 | 1555205 | <span style="color:#2563eb">48.16%</span> |
| 662 | [01022 JSON_EXTRACT_SET_015](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1022_JSON_EXTRACT_SET_015.rs) | P2 | memory | GEN_SQL_JSON | 1582297 | 1555135 | <span style="color:#2563eb">48.16%</span> |
| 663 | [00714 CTE_RECURSIVE_MATRIX_007](crates/bench/sqlite_parity/cases/SQLITE_PARITY_714_CTE_RECURSIVE_MATRIX_007.rs) | P1 | memory | GEN_SQL_CTE | 1617774 | 1554894 | <span style="color:#2563eb">48.17%</span> |
| 664 | [00417 DML_WHERE_ORDER_LIMIT_030](crates/bench/sqlite_parity/cases/SQLITE_PARITY_417_DML_WHERE_ORDER_LIMIT_030.rs) | P1 | memory | GEN_SQL_DML | 2077705 | 1554634 | <span style="color:#2563eb">48.18%</span> |
| 665 | [01041 JSON_EXTRACT_SET_034](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1041_JSON_EXTRACT_SET_034.rs) | P2 | memory | GEN_SQL_JSON | 1636369 | 1554434 | <span style="color:#2563eb">48.19%</span> |
| 666 | [00707 JOIN_SUBQUERY_EXISTS_100](crates/bench/sqlite_parity/cases/SQLITE_PARITY_707_JOIN_SUBQUERY_EXISTS_100.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1764701 | 1554343 | <span style="color:#2563eb">48.19%</span> |
| 667 | [00364 SCALAR_ARITH_035](crates/bench/sqlite_parity/cases/SQLITE_PARITY_364_SCALAR_ARITH_035.rs) | P1 | memory | GEN_SQL_SCALAR | 1485062 | 1554273 | <span style="color:#2563eb">48.19%</span> |
| 668 | [00542 AGG_GROUP_HAVING_035](crates/bench/sqlite_parity/cases/SQLITE_PARITY_542_AGG_GROUP_HAVING_035.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1847458 | 1554163 | <span style="color:#2563eb">48.19%</span> |
| 669 | [00069 COLLATE_NOCASE_RTRIM_BINARY](crates/bench/sqlite_parity/cases/SQLITE_PARITY_069_COLLATE_NOCASE_RTRIM_BINARY.rs) | P0 | memory | SQL_COLLATION | 1491345 | 1554073 | <span style="color:#2563eb">48.20%</span> |
| 670 | [00891 CONSTRAINT_FK_SAVEPOINT_024](crates/bench/sqlite_parity/cases/SQLITE_PARITY_891_CONSTRAINT_FK_SAVEPOINT_024.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1682546 | 1553983 | <span style="color:#2563eb">48.20%</span> |
| 671 | [00854 WINDOW_PARTITION_SUM_067](crates/bench/sqlite_parity/cases/SQLITE_PARITY_854_WINDOW_PARTITION_SUM_067.rs) | P2 | memory | GEN_SQL_WINDOW | 1669782 | 1553953 | <span style="color:#2563eb">48.20%</span> |
| 672 | [00067 CASE_COALESCE_NULLIF_IIF](crates/bench/sqlite_parity/cases/SQLITE_PARITY_067_CASE_COALESCE_NULLIF_IIF.rs) | P0 | memory | SQL_EXPRESSIONS | 1546659 | 1553913 | <span style="color:#2563eb">48.20%</span> |
| 673 | [00363 SCALAR_NULL_COALESCE_034](crates/bench/sqlite_parity/cases/SQLITE_PARITY_363_SCALAR_NULL_COALESCE_034.rs) | P1 | memory | GEN_SQL_SCALAR | 1517694 | 1553662 | <span style="color:#2563eb">48.21%</span> |
| 674 | [00535 AGG_GROUP_HAVING_028](crates/bench/sqlite_parity/cases/SQLITE_PARITY_535_AGG_GROUP_HAVING_028.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1972205 | 1553662 | <span style="color:#2563eb">48.21%</span> |
| 675 | [01033 JSON_EXTRACT_SET_026](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1033_JSON_EXTRACT_SET_026.rs) | P2 | memory | GEN_SQL_JSON | 1637160 | 1553421 | <span style="color:#2563eb">48.22%</span> |
| 676 | [00234 SCALAR_STRING_002](crates/bench/sqlite_parity/cases/SQLITE_PARITY_234_SCALAR_STRING_002.rs) | P1 | memory | GEN_SQL_SCALAR | 1506743 | 1553412 | <span style="color:#2563eb">48.22%</span> |
| 677 | [00771 CTE_RECURSIVE_MATRIX_064](crates/bench/sqlite_parity/cases/SQLITE_PARITY_771_CTE_RECURSIVE_MATRIX_064.rs) | P1 | memory | GEN_SQL_CTE | 1548993 | 1553402 | <span style="color:#2563eb">48.22%</span> |
| 678 | [00885 CONSTRAINT_FK_SAVEPOINT_018](crates/bench/sqlite_parity/cases/SQLITE_PARITY_885_CONSTRAINT_FK_SAVEPOINT_018.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1757278 | 1553301 | <span style="color:#2563eb">48.22%</span> |
| 679 | [00221 OPT_DOUBLE_DASH_END_OPTIONS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_221_OPT_DOUBLE_DASH_END_OPTIONS.rs) | P2 | memory | CLI_OPTION | 2381510 | 1553222 | <span style="color:#2563eb">48.23%</span> |
| 680 | [00176 OPT_LINE_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_176_OPT_LINE_MODE.rs) | P2 | memory | CLI_OPTION | 1527603 | 1553191 | <span style="color:#2563eb">48.23%</span> |
| 681 | [00286 SCALAR_STRING_015](crates/bench/sqlite_parity/cases/SQLITE_PARITY_286_SCALAR_STRING_015.rs) | P1 | memory | GEN_SQL_SCALAR | 1659272 | 1553131 | <span style="color:#2563eb">48.23%</span> |
| 682 | [00699 JOIN_SUBQUERY_EXISTS_092](crates/bench/sqlite_parity/cases/SQLITE_PARITY_699_JOIN_SUBQUERY_EXISTS_092.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1754112 | 1553101 | <span style="color:#2563eb">48.23%</span> |
| 683 | [01025 JSON_EXTRACT_SET_018](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1025_JSON_EXTRACT_SET_018.rs) | P2 | memory | GEN_SQL_JSON | 1667708 | 1552890 | <span style="color:#2563eb">48.24%</span> |
| 684 | [00805 WINDOW_PARTITION_SUM_018](crates/bench/sqlite_parity/cases/SQLITE_PARITY_805_WINDOW_PARTITION_SUM_018.rs) | P2 | memory | GEN_SQL_WINDOW | 1792014 | 1552630 | <span style="color:#2563eb">48.25%</span> |
| 685 | [00804 WINDOW_PARTITION_SUM_017](crates/bench/sqlite_parity/cases/SQLITE_PARITY_804_WINDOW_PARTITION_SUM_017.rs) | P2 | memory | GEN_SQL_WINDOW | 2058327 | 1552580 | <span style="color:#2563eb">48.25%</span> |
| 686 | [00903 CONSTRAINT_FK_SAVEPOINT_036](crates/bench/sqlite_parity/cases/SQLITE_PARITY_903_CONSTRAINT_FK_SAVEPOINT_036.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1735146 | 1552340 | <span style="color:#2563eb">48.26%</span> |
| 687 | [00262 SCALAR_STRING_009](crates/bench/sqlite_parity/cases/SQLITE_PARITY_262_SCALAR_STRING_009.rs) | P1 | memory | GEN_SQL_SCALAR | 1855804 | 1552310 | <span style="color:#2563eb">48.26%</span> |
| 688 | [00355 SCALAR_NULL_COALESCE_032](crates/bench/sqlite_parity/cases/SQLITE_PARITY_355_SCALAR_NULL_COALESCE_032.rs) | P1 | memory | GEN_SQL_SCALAR | 1492105 | 1552220 | <span style="color:#2563eb">48.26%</span> |
| 689 | [00982 VIEW_TRIGGER_GENERATED_035](crates/bench/sqlite_parity/cases/SQLITE_PARITY_982_VIEW_TRIGGER_GENERATED_035.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1777556 | 1552199 | <span style="color:#2563eb">48.26%</span> |
| 690 | [00265 SCALAR_CAST_TYPEOF_010](crates/bench/sqlite_parity/cases/SQLITE_PARITY_265_SCALAR_CAST_TYPEOF_010.rs) | P1 | memory | GEN_SQL_SCALAR | 1829084 | 1552169 | <span style="color:#2563eb">48.26%</span> |
| 691 | [01012 JSON_EXTRACT_SET_005](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1012_JSON_EXTRACT_SET_005.rs) | P2 | memory | GEN_SQL_JSON | 1646989 | 1551948 | <span style="color:#2563eb">48.27%</span> |
| 692 | [00301 SCALAR_CAST_TYPEOF_019](crates/bench/sqlite_parity/cases/SQLITE_PARITY_301_SCALAR_CAST_TYPEOF_019.rs) | P1 | memory | GEN_SQL_SCALAR | 1551989 | 1551919 | <span style="color:#2563eb">48.27%</span> |
| 693 | [00430 DML_WHERE_ORDER_LIMIT_043](crates/bench/sqlite_parity/cases/SQLITE_PARITY_430_DML_WHERE_ORDER_LIMIT_043.rs) | P1 | memory | GEN_SQL_DML | 1650385 | 1551729 | <span style="color:#2563eb">48.28%</span> |
| 694 | [00685 JOIN_SUBQUERY_EXISTS_078](crates/bench/sqlite_parity/cases/SQLITE_PARITY_685_JOIN_SUBQUERY_EXISTS_078.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1735296 | 1551609 | <span style="color:#2563eb">48.28%</span> |
| 695 | [00506 DML_WHERE_ORDER_LIMIT_119](crates/bench/sqlite_parity/cases/SQLITE_PARITY_506_DML_WHERE_ORDER_LIMIT_119.rs) | P1 | memory | GEN_SQL_DML | 1986632 | 1551588 | <span style="color:#2563eb">48.28%</span> |
| 696 | [01082 INDEX_SCHEMA_PRAGMA_015](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1082_INDEX_SCHEMA_PRAGMA_015.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1756917 | 1551558 | <span style="color:#2563eb">48.28%</span> |
| 697 | [00790 WINDOW_PARTITION_SUM_003](crates/bench/sqlite_parity/cases/SQLITE_PARITY_790_WINDOW_PARTITION_SUM_003.rs) | P2 | memory | GEN_SQL_WINDOW | 1677426 | 1551177 | <span style="color:#2563eb">48.29%</span> |
| 698 | [00608 JOIN_SUBQUERY_EXISTS_001](crates/bench/sqlite_parity/cases/SQLITE_PARITY_608_JOIN_SUBQUERY_EXISTS_001.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1766505 | 1551167 | <span style="color:#2563eb">48.29%</span> |
| 699 | [00650 JOIN_SUBQUERY_EXISTS_043](crates/bench/sqlite_parity/cases/SQLITE_PARITY_650_JOIN_SUBQUERY_EXISTS_043.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1773468 | 1551067 | <span style="color:#2563eb">48.30%</span> |
| 700 | [00492 DML_WHERE_ORDER_LIMIT_105](crates/bench/sqlite_parity/cases/SQLITE_PARITY_492_DML_WHERE_ORDER_LIMIT_105.rs) | P1 | memory | GEN_SQL_DML | 1937089 | 1550997 | <span style="color:#2563eb">48.30%</span> |
| 701 | [00594 AGG_GROUP_HAVING_087](crates/bench/sqlite_parity/cases/SQLITE_PARITY_594_AGG_GROUP_HAVING_087.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1746267 | 1550836 | <span style="color:#2563eb">48.31%</span> |
| 702 | [00578 AGG_GROUP_HAVING_071](crates/bench/sqlite_parity/cases/SQLITE_PARITY_578_AGG_GROUP_HAVING_071.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1750474 | 1550767 | <span style="color:#2563eb">48.31%</span> |
| 703 | [00801 WINDOW_PARTITION_SUM_014](crates/bench/sqlite_parity/cases/SQLITE_PARITY_801_WINDOW_PARTITION_SUM_014.rs) | P2 | memory | GEN_SQL_WINDOW | 1661125 | 1550536 | <span style="color:#2563eb">48.32%</span> |
| 704 | [00420 DML_WHERE_ORDER_LIMIT_033](crates/bench/sqlite_parity/cases/SQLITE_PARITY_420_DML_WHERE_ORDER_LIMIT_033.rs) | P1 | memory | GEN_SQL_DML | 1958168 | 1550496 | <span style="color:#2563eb">48.32%</span> |
| 705 | [00653 JOIN_SUBQUERY_EXISTS_046](crates/bench/sqlite_parity/cases/SQLITE_PARITY_653_JOIN_SUBQUERY_EXISTS_046.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2099896 | 1550487 | <span style="color:#2563eb">48.32%</span> |
| 706 | [00934 CONSTRAINT_FK_SAVEPOINT_067](crates/bench/sqlite_parity/cases/SQLITE_PARITY_934_CONSTRAINT_FK_SAVEPOINT_067.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1670053 | 1550486 | <span style="color:#2563eb">48.32%</span> |
| 707 | [00671 JOIN_SUBQUERY_EXISTS_064](crates/bench/sqlite_parity/cases/SQLITE_PARITY_671_JOIN_SUBQUERY_EXISTS_064.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1783016 | 1550466 | <span style="color:#2563eb">48.32%</span> |
| 708 | [00409 DML_WHERE_ORDER_LIMIT_022](crates/bench/sqlite_parity/cases/SQLITE_PARITY_409_DML_WHERE_ORDER_LIMIT_022.rs) | P1 | memory | GEN_SQL_DML | 1640637 | 1550346 | <span style="color:#2563eb">48.32%</span> |
| 709 | [01035 JSON_EXTRACT_SET_028](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1035_JSON_EXTRACT_SET_028.rs) | P2 | memory | GEN_SQL_JSON | 1652128 | 1550336 | <span style="color:#2563eb">48.32%</span> |
| 710 | [00580 AGG_GROUP_HAVING_073](crates/bench/sqlite_parity/cases/SQLITE_PARITY_580_AGG_GROUP_HAVING_073.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1707613 | 1550195 | <span style="color:#2563eb">48.33%</span> |
| 711 | [00061 WINDOW_ROW_NUMBER_RANK](crates/bench/sqlite_parity/cases/SQLITE_PARITY_061_WINDOW_ROW_NUMBER_RANK.rs) | P0 | memory | SQL_WINDOW | 1778698 | 1550175 | <span style="color:#2563eb">48.33%</span> |
| 712 | [00028 ALTER_TABLE_RENAME](crates/bench/sqlite_parity/cases/SQLITE_PARITY_028_ALTER_TABLE_RENAME.rs) | P0 | memory | SQL_ALTER | 1868749 | 1550115 | <span style="color:#2563eb">48.33%</span> |
| 713 | [00719 CTE_RECURSIVE_MATRIX_012](crates/bench/sqlite_parity/cases/SQLITE_PARITY_719_CTE_RECURSIVE_MATRIX_012.rs) | P1 | memory | GEN_SQL_CTE | 1677697 | 1550095 | <span style="color:#2563eb">48.33%</span> |
| 714 | [00099 CLI_UINT_COLLATION_OPTIONAL](crates/bench/sqlite_parity/cases/SQLITE_PARITY_099_CLI_UINT_COLLATION_OPTIONAL.rs) | P3 | memory | CLI_EXTENSION_OPTIONAL | 1805098 | 1550065 | <span style="color:#2563eb">48.33%</span> |
| 715 | [00293 SCALAR_CAST_TYPEOF_017](crates/bench/sqlite_parity/cases/SQLITE_PARITY_293_SCALAR_CAST_TYPEOF_017.rs) | P1 | memory | GEN_SQL_SCALAR | 1489511 | 1549915 | <span style="color:#2563eb">48.34%</span> |
| 716 | [00551 AGG_GROUP_HAVING_044](crates/bench/sqlite_parity/cases/SQLITE_PARITY_551_AGG_GROUP_HAVING_044.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1707143 | 1549885 | <span style="color:#2563eb">48.34%</span> |
| 717 | [01089 INDEX_SCHEMA_PRAGMA_022](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1089_INDEX_SCHEMA_PRAGMA_022.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 2014405 | 1549814 | <span style="color:#2563eb">48.34%</span> |
| 718 | [01101 INDEX_SCHEMA_PRAGMA_034](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1101_INDEX_SCHEMA_PRAGMA_034.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1742320 | 1549735 | <span style="color:#2563eb">48.34%</span> |
| 719 | [00014 INSERT_RETURNING](crates/bench/sqlite_parity/cases/SQLITE_PARITY_014_INSERT_RETURNING.rs) | P0 | memory | SQL_INSERT | 1678328 | 1549584 | <span style="color:#2563eb">48.35%</span> |
| 720 | [00257 SCALAR_CAST_TYPEOF_008](crates/bench/sqlite_parity/cases/SQLITE_PARITY_257_SCALAR_CAST_TYPEOF_008.rs) | P1 | memory | GEN_SQL_SCALAR | 1509899 | 1549504 | <span style="color:#2563eb">48.35%</span> |
| 721 | [00505 DML_WHERE_ORDER_LIMIT_118](crates/bench/sqlite_parity/cases/SQLITE_PARITY_505_DML_WHERE_ORDER_LIMIT_118.rs) | P1 | memory | GEN_SQL_DML | 1619777 | 1549454 | <span style="color:#2563eb">48.35%</span> |
| 722 | [00013 INSERT_SELECT](crates/bench/sqlite_parity/cases/SQLITE_PARITY_013_INSERT_SELECT.rs) | P0 | memory | SQL_INSERT | 1565445 | 1549364 | <span style="color:#2563eb">48.35%</span> |
| 723 | [00135 DOT_PROGRESS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_135_DOT_PROGRESS.rs) | P0 | memory | CLI_DOT_COMMAND | 1501173 | 1549314 | <span style="color:#2563eb">48.36%</span> |
| 724 | [00667 JOIN_SUBQUERY_EXISTS_060](crates/bench/sqlite_parity/cases/SQLITE_PARITY_667_JOIN_SUBQUERY_EXISTS_060.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1734605 | 1549304 | <span style="color:#2563eb">48.36%</span> |
| 725 | [00324 SCALAR_ARITH_025](crates/bench/sqlite_parity/cases/SQLITE_PARITY_324_SCALAR_ARITH_025.rs) | P1 | memory | GEN_SQL_SCALAR | 1706551 | 1549133 | <span style="color:#2563eb">48.36%</span> |
| 726 | [00256 SCALAR_ARITH_008](crates/bench/sqlite_parity/cases/SQLITE_PARITY_256_SCALAR_ARITH_008.rs) | P1 | memory | GEN_SQL_SCALAR | 1750845 | 1548923 | <span style="color:#2563eb">48.37%</span> |
| 727 | [00735 CTE_RECURSIVE_MATRIX_028](crates/bench/sqlite_parity/cases/SQLITE_PARITY_735_CTE_RECURSIVE_MATRIX_028.rs) | P1 | memory | GEN_SQL_CTE | 1600972 | 1548903 | <span style="color:#2563eb">48.37%</span> |
| 728 | [00183 OPT_TABS_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_183_OPT_TABS_MODE.rs) | P2 | memory | CLI_OPTION | 1541048 | 1548873 | <span style="color:#2563eb">48.37%</span> |
| 729 | [00886 CONSTRAINT_FK_SAVEPOINT_019](crates/bench/sqlite_parity/cases/SQLITE_PARITY_886_CONSTRAINT_FK_SAVEPOINT_019.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1718544 | 1548853 | <span style="color:#2563eb">48.37%</span> |
| 730 | [00143 DOT_QUIT](crates/bench/sqlite_parity/cases/SQLITE_PARITY_143_DOT_QUIT.rs) | P0 | memory | CLI_DOT_COMMAND | 1404550 | 1548613 | <span style="color:#2563eb">48.38%</span> |
| 731 | [00485 DML_WHERE_ORDER_LIMIT_098](crates/bench/sqlite_parity/cases/SQLITE_PARITY_485_DML_WHERE_ORDER_LIMIT_098.rs) | P1 | memory | GEN_SQL_DML | 1678559 | 1548602 | <span style="color:#2563eb">48.38%</span> |
| 732 | [00370 SCALAR_STRING_036](crates/bench/sqlite_parity/cases/SQLITE_PARITY_370_SCALAR_STRING_036.rs) | P1 | memory | GEN_SQL_SCALAR | 1833402 | 1548512 | <span style="color:#2563eb">48.38%</span> |
| 733 | [00964 VIEW_TRIGGER_GENERATED_017](crates/bench/sqlite_parity/cases/SQLITE_PARITY_964_VIEW_TRIGGER_GENERATED_017.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1750845 | 1548512 | <span style="color:#2563eb">48.38%</span> |
| 734 | [00830 WINDOW_PARTITION_SUM_043](crates/bench/sqlite_parity/cases/SQLITE_PARITY_830_WINDOW_PARTITION_SUM_043.rs) | P2 | memory | GEN_SQL_WINDOW | 1620228 | 1548393 | <span style="color:#2563eb">48.39%</span> |
| 735 | [00108 DOT_MODE_CSV_AND_QUOTE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_108_DOT_MODE_CSV_AND_QUOTE.rs) | P0 | memory | CLI_DOT_COMMAND | 1577557 | 1548392 | <span style="color:#2563eb">48.39%</span> |
| 736 | [00107 DOT_HELP_PATTERN](crates/bench/sqlite_parity/cases/SQLITE_PARITY_107_DOT_HELP_PATTERN.rs) | P0 | memory | CLI_DOT_COMMAND | 1469222 | 1548382 | <span style="color:#2563eb">48.39%</span> |
| 737 | [00658 JOIN_SUBQUERY_EXISTS_051](crates/bench/sqlite_parity/cases/SQLITE_PARITY_658_JOIN_SUBQUERY_EXISTS_051.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1780992 | 1548272 | <span style="color:#2563eb">48.39%</span> |
| 738 | [00502 DML_WHERE_ORDER_LIMIT_115](crates/bench/sqlite_parity/cases/SQLITE_PARITY_502_DML_WHERE_ORDER_LIMIT_115.rs) | P1 | memory | GEN_SQL_DML | 1760183 | 1548052 | <span style="color:#2563eb">48.40%</span> |
| 739 | [00321 SCALAR_CAST_TYPEOF_024](crates/bench/sqlite_parity/cases/SQLITE_PARITY_321_SCALAR_CAST_TYPEOF_024.rs) | P1 | memory | GEN_SQL_SCALAR | 1558501 | 1548031 | <span style="color:#2563eb">48.40%</span> |
| 740 | [00295 SCALAR_NULL_COALESCE_017](crates/bench/sqlite_parity/cases/SQLITE_PARITY_295_SCALAR_NULL_COALESCE_017.rs) | P1 | memory | GEN_SQL_SCALAR | 1677586 | 1548021 | <span style="color:#2563eb">48.40%</span> |
| 741 | [01032 JSON_EXTRACT_SET_025](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1032_JSON_EXTRACT_SET_025.rs) | P2 | memory | GEN_SQL_JSON | 1612313 | 1547952 | <span style="color:#2563eb">48.40%</span> |
| 742 | [00817 WINDOW_PARTITION_SUM_030](crates/bench/sqlite_parity/cases/SQLITE_PARITY_817_WINDOW_PARTITION_SUM_030.rs) | P2 | memory | GEN_SQL_WINDOW | 1708445 | 1547721 | <span style="color:#2563eb">48.41%</span> |
| 743 | [00334 SCALAR_STRING_027](crates/bench/sqlite_parity/cases/SQLITE_PARITY_334_SCALAR_STRING_027.rs) | P1 | memory | GEN_SQL_SCALAR | 1549745 | 1547671 | <span style="color:#2563eb">48.41%</span> |
| 744 | [00130 DOT_OPEN_MEMORY](crates/bench/sqlite_parity/cases/SQLITE_PARITY_130_DOT_OPEN_MEMORY.rs) | P0 | memory | CLI_DOT_COMMAND | 1579231 | 1547651 | <span style="color:#2563eb">48.41%</span> |
| 745 | [00679 JOIN_SUBQUERY_EXISTS_072](crates/bench/sqlite_parity/cases/SQLITE_PARITY_679_JOIN_SUBQUERY_EXISTS_072.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1779760 | 1547581 | <span style="color:#2563eb">48.41%</span> |
| 746 | [00053 SELECT_WHERE_ORDER_LIMIT_OFFSET](crates/bench/sqlite_parity/cases/SQLITE_PARITY_053_SELECT_WHERE_ORDER_LIMIT_OFFSET.rs) | P0 | memory | SQL_SELECT | 1649794 | 1547500 | <span style="color:#2563eb">48.42%</span> |
| 747 | [00741 CTE_RECURSIVE_MATRIX_034](crates/bench/sqlite_parity/cases/SQLITE_PARITY_741_CTE_RECURSIVE_MATRIX_034.rs) | P1 | memory | GEN_SQL_CTE | 1608275 | 1547410 | <span style="color:#2563eb">48.42%</span> |
| 748 | [00236 SCALAR_ARITH_003](crates/bench/sqlite_parity/cases/SQLITE_PARITY_236_SCALAR_ARITH_003.rs) | P1 | memory | GEN_SQL_SCALAR | 1456358 | 1547350 | <span style="color:#2563eb">48.42%</span> |
| 749 | [00077 COMMENTS_AND_CLI_TERMINATORS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_077_COMMENTS_AND_CLI_TERMINATORS.rs) | P0 | memory | CLI_SQL_INPUT | 2206638 | 1547331 | <span style="color:#2563eb">48.42%</span> |
| 750 | [00476 DML_WHERE_ORDER_LIMIT_089](crates/bench/sqlite_parity/cases/SQLITE_PARITY_476_DML_WHERE_ORDER_LIMIT_089.rs) | P1 | memory | GEN_SQL_DML | 1691022 | 1547290 | <span style="color:#2563eb">48.42%</span> |
| 751 | [00504 DML_WHERE_ORDER_LIMIT_117](crates/bench/sqlite_parity/cases/SQLITE_PARITY_504_DML_WHERE_ORDER_LIMIT_117.rs) | P1 | memory | GEN_SQL_DML | 1649394 | 1547150 | <span style="color:#2563eb">48.43%</span> |
| 752 | [00458 DML_WHERE_ORDER_LIMIT_071](crates/bench/sqlite_parity/cases/SQLITE_PARITY_458_DML_WHERE_ORDER_LIMIT_071.rs) | P1 | memory | GEN_SQL_DML | 1622573 | 1546979 | <span style="color:#2563eb">48.43%</span> |
| 753 | [01043 JSON_EXTRACT_SET_036](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1043_JSON_EXTRACT_SET_036.rs) | P2 | memory | GEN_SQL_JSON | 1639204 | 1546949 | <span style="color:#2563eb">48.44%</span> |
| 754 | [00085 CORE_RANDOM_SHAPE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_085_CORE_RANDOM_SHAPE.rs) | P0 | memory | SQL_FUNCTIONS | 1536570 | 1546899 | <span style="color:#2563eb">48.44%</span> |
| 755 | [00588 AGG_GROUP_HAVING_081](crates/bench/sqlite_parity/cases/SQLITE_PARITY_588_AGG_GROUP_HAVING_081.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1730497 | 1546840 | <span style="color:#2563eb">48.44%</span> |
| 756 | [00627 JOIN_SUBQUERY_EXISTS_020](crates/bench/sqlite_parity/cases/SQLITE_PARITY_627_JOIN_SUBQUERY_EXISTS_020.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1748752 | 1546769 | <span style="color:#2563eb">48.44%</span> |
| 757 | [01117 INDEX_SCHEMA_PRAGMA_050](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1117_INDEX_SCHEMA_PRAGMA_050.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1774279 | 1546759 | <span style="color:#2563eb">48.44%</span> |
| 758 | [00488 DML_WHERE_ORDER_LIMIT_101](crates/bench/sqlite_parity/cases/SQLITE_PARITY_488_DML_WHERE_ORDER_LIMIT_101.rs) | P1 | memory | GEN_SQL_DML | 1684680 | 1546539 | <span style="color:#2563eb">48.45%</span> |
| 759 | [01115 INDEX_SCHEMA_PRAGMA_048](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1115_INDEX_SCHEMA_PRAGMA_048.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1706181 | 1546298 | <span style="color:#2563eb">48.46%</span> |
| 760 | [00500 DML_WHERE_ORDER_LIMIT_113](crates/bench/sqlite_parity/cases/SQLITE_PARITY_500_DML_WHERE_ORDER_LIMIT_113.rs) | P1 | memory | GEN_SQL_DML | 1671635 | 1546268 | <span style="color:#2563eb">48.46%</span> |
| 761 | [00763 CTE_RECURSIVE_MATRIX_056](crates/bench/sqlite_parity/cases/SQLITE_PARITY_763_CTE_RECURSIVE_MATRIX_056.rs) | P1 | memory | GEN_SQL_CTE | 1956895 | 1546218 | <span style="color:#2563eb">48.46%</span> |
| 762 | [00926 CONSTRAINT_FK_SAVEPOINT_059](crates/bench/sqlite_parity/cases/SQLITE_PARITY_926_CONSTRAINT_FK_SAVEPOINT_059.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1698397 | 1546158 | <span style="color:#2563eb">48.46%</span> |
| 763 | [00128 DOT_DBCONFIG](crates/bench/sqlite_parity/cases/SQLITE_PARITY_128_DOT_DBCONFIG.rs) | P0 | memory | CLI_DOT_COMMAND | 1478139 | 1546148 | <span style="color:#2563eb">48.46%</span> |
| 764 | [00405 DML_WHERE_ORDER_LIMIT_018](crates/bench/sqlite_parity/cases/SQLITE_PARITY_405_DML_WHERE_ORDER_LIMIT_018.rs) | P1 | memory | GEN_SQL_DML | 1604238 | 1545998 | <span style="color:#2563eb">48.47%</span> |
| 765 | [00555 AGG_GROUP_HAVING_048](crates/bench/sqlite_parity/cases/SQLITE_PARITY_555_AGG_GROUP_HAVING_048.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1782956 | 1545897 | <span style="color:#2563eb">48.47%</span> |
| 766 | [00557 AGG_GROUP_HAVING_050](crates/bench/sqlite_parity/cases/SQLITE_PARITY_557_AGG_GROUP_HAVING_050.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1744804 | 1545837 | <span style="color:#2563eb">48.47%</span> |
| 767 | [00776 CTE_RECURSIVE_MATRIX_069](crates/bench/sqlite_parity/cases/SQLITE_PARITY_776_CTE_RECURSIVE_MATRIX_069.rs) | P1 | memory | GEN_SQL_CTE | 1569752 | 1545777 | <span style="color:#2563eb">48.47%</span> |
| 768 | [00519 AGG_GROUP_HAVING_012](crates/bench/sqlite_parity/cases/SQLITE_PARITY_519_AGG_GROUP_HAVING_012.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1732160 | 1545737 | <span style="color:#2563eb">48.48%</span> |
| 769 | [00860 WINDOW_PARTITION_SUM_073](crates/bench/sqlite_parity/cases/SQLITE_PARITY_860_WINDOW_PARTITION_SUM_073.rs) | P2 | memory | GEN_SQL_WINDOW | 1692906 | 1545718 | <span style="color:#2563eb">48.48%</span> |
| 770 | [00191 OPT_BATCH](crates/bench/sqlite_parity/cases/SQLITE_PARITY_191_OPT_BATCH.rs) | P2 | memory | CLI_OPTION | 1986702 | 1545717 | <span style="color:#2563eb">48.48%</span> |
| 771 | [00633 JOIN_SUBQUERY_EXISTS_026](crates/bench/sqlite_parity/cases/SQLITE_PARITY_633_JOIN_SUBQUERY_EXISTS_026.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1716190 | 1545647 | <span style="color:#2563eb">48.48%</span> |
| 772 | [00740 CTE_RECURSIVE_MATRIX_033](crates/bench/sqlite_parity/cases/SQLITE_PARITY_740_CTE_RECURSIVE_MATRIX_033.rs) | P1 | memory | GEN_SQL_CTE | 1603456 | 1545597 | <span style="color:#2563eb">48.48%</span> |
| 773 | [01081 INDEX_SCHEMA_PRAGMA_014](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1081_INDEX_SCHEMA_PRAGMA_014.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1750775 | 1545597 | <span style="color:#2563eb">48.48%</span> |
| 774 | [00737 CTE_RECURSIVE_MATRIX_030](crates/bench/sqlite_parity/cases/SQLITE_PARITY_737_CTE_RECURSIVE_MATRIX_030.rs) | P1 | memory | GEN_SQL_CTE | 1900850 | 1545517 | <span style="color:#2563eb">48.48%</span> |
| 775 | [00722 CTE_RECURSIVE_MATRIX_015](crates/bench/sqlite_parity/cases/SQLITE_PARITY_722_CTE_RECURSIVE_MATRIX_015.rs) | P1 | memory | GEN_SQL_CTE | 1620408 | 1545446 | <span style="color:#2563eb">48.49%</span> |
| 776 | [00919 CONSTRAINT_FK_SAVEPOINT_052](crates/bench/sqlite_parity/cases/SQLITE_PARITY_919_CONSTRAINT_FK_SAVEPOINT_052.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1703866 | 1545377 | <span style="color:#2563eb">48.49%</span> |
| 777 | [00238 SCALAR_STRING_003](crates/bench/sqlite_parity/cases/SQLITE_PARITY_238_SCALAR_STRING_003.rs) | P1 | memory | GEN_SQL_SCALAR | 1484411 | 1545376 | <span style="color:#2563eb">48.49%</span> |
| 778 | [00955 VIEW_TRIGGER_GENERATED_008](crates/bench/sqlite_parity/cases/SQLITE_PARITY_955_VIEW_TRIGGER_GENERATED_008.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1805710 | 1545347 | <span style="color:#2563eb">48.49%</span> |
| 779 | [00388 DML_WHERE_ORDER_LIMIT_001](crates/bench/sqlite_parity/cases/SQLITE_PARITY_388_DML_WHERE_ORDER_LIMIT_001.rs) | P1 | memory | GEN_SQL_DML | 1697334 | 1545226 | <span style="color:#2563eb">48.49%</span> |
| 780 | [00200 OPT_HEAP](crates/bench/sqlite_parity/cases/SQLITE_PARITY_200_OPT_HEAP.rs) | P4 | memory | CLI_OPTION | 1504519 | 1545126 | <span style="color:#2563eb">48.50%</span> |
| 781 | [00274 SCALAR_STRING_012](crates/bench/sqlite_parity/cases/SQLITE_PARITY_274_SCALAR_STRING_012.rs) | P1 | memory | GEN_SQL_SCALAR | 1523465 | 1545126 | <span style="color:#2563eb">48.50%</span> |
| 782 | [00968 VIEW_TRIGGER_GENERATED_021](crates/bench/sqlite_parity/cases/SQLITE_PARITY_968_VIEW_TRIGGER_GENERATED_021.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1843471 | 1545075 | <span style="color:#2563eb">48.50%</span> |
| 783 | [00305 SCALAR_CAST_TYPEOF_020](crates/bench/sqlite_parity/cases/SQLITE_PARITY_305_SCALAR_CAST_TYPEOF_020.rs) | P1 | memory | GEN_SQL_SCALAR | 1514117 | 1545065 | <span style="color:#2563eb">48.50%</span> |
| 784 | [00129 DOT_CONNECTION](crates/bench/sqlite_parity/cases/SQLITE_PARITY_129_DOT_CONNECTION.rs) | P0 | memory | CLI_DOT_COMMAND | 1631139 | 1544855 | <span style="color:#2563eb">48.50%</span> |
| 785 | [00806 WINDOW_PARTITION_SUM_019](crates/bench/sqlite_parity/cases/SQLITE_PARITY_806_WINDOW_PARTITION_SUM_019.rs) | P2 | memory | GEN_SQL_WINDOW | 1627001 | 1544785 | <span style="color:#2563eb">48.51%</span> |
| 786 | [00214 DROP_TABLE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_214_DROP_TABLE.rs) | P0 | memory | SQL_DROP | 1688848 | 1544485 | <span style="color:#2563eb">48.52%</span> |
| 787 | [00094 FTS5_HIGHLIGHT_OPTIONAL](crates/bench/sqlite_parity/cases/SQLITE_PARITY_094_FTS5_HIGHLIGHT_OPTIONAL.rs) | P2 | memory | SQL_VIRTUAL_TABLE_OPTIONAL | 1829134 | 1544264 | <span style="color:#2563eb">48.52%</span> |
| 788 | [00511 AGG_GROUP_HAVING_004](crates/bench/sqlite_parity/cases/SQLITE_PARITY_511_AGG_GROUP_HAVING_004.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1949692 | 1544244 | <span style="color:#2563eb">48.53%</span> |
| 789 | [00098 CLI_REGEXP_OPTIONAL](crates/bench/sqlite_parity/cases/SQLITE_PARITY_098_CLI_REGEXP_OPTIONAL.rs) | P2 | memory | CLI_EXTENSION_OPTIONAL | 2115045 | 1544224 | <span style="color:#2563eb">48.53%</span> |
| 790 | [00057 COMPOUND_SELECT_UNION_INTERSECT_EXCEPT](crates/bench/sqlite_parity/cases/SQLITE_PARITY_057_COMPOUND_SELECT_UNION_INTERSECT_EXCEPT.rs) | P0 | memory | SQL_SELECT | 1535187 | 1544174 | <span style="color:#2563eb">48.53%</span> |
| 791 | [00769 CTE_RECURSIVE_MATRIX_062](crates/bench/sqlite_parity/cases/SQLITE_PARITY_769_CTE_RECURSIVE_MATRIX_062.rs) | P1 | memory | GEN_SQL_CTE | 1536981 | 1544154 | <span style="color:#2563eb">48.53%</span> |
| 792 | [00743 CTE_RECURSIVE_MATRIX_036](crates/bench/sqlite_parity/cases/SQLITE_PARITY_743_CTE_RECURSIVE_MATRIX_036.rs) | P1 | memory | GEN_SQL_CTE | 1613005 | 1544003 | <span style="color:#2563eb">48.53%</span> |
| 793 | [00832 WINDOW_PARTITION_SUM_045](crates/bench/sqlite_parity/cases/SQLITE_PARITY_832_WINDOW_PARTITION_SUM_045.rs) | P2 | memory | GEN_SQL_WINDOW | 1654062 | 1543854 | <span style="color:#2563eb">48.54%</span> |
| 794 | [00855 WINDOW_PARTITION_SUM_068](crates/bench/sqlite_parity/cases/SQLITE_PARITY_855_WINDOW_PARTITION_SUM_068.rs) | P2 | memory | GEN_SQL_WINDOW | 1996831 | 1543823 | <span style="color:#2563eb">48.54%</span> |
| 795 | [01010 JSON_EXTRACT_SET_003](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1010_JSON_EXTRACT_SET_003.rs) | P2 | memory | GEN_SQL_JSON | 1647019 | 1543774 | <span style="color:#2563eb">48.54%</span> |
| 796 | [00544 AGG_GROUP_HAVING_037](crates/bench/sqlite_parity/cases/SQLITE_PARITY_544_AGG_GROUP_HAVING_037.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1707293 | 1543674 | <span style="color:#2563eb">48.54%</span> |
| 797 | [01031 JSON_EXTRACT_SET_024](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1031_JSON_EXTRACT_SET_024.rs) | P2 | memory | GEN_SQL_JSON | 1712072 | 1543573 | <span style="color:#2563eb">48.55%</span> |
| 798 | [00599 AGG_GROUP_HAVING_092](crates/bench/sqlite_parity/cases/SQLITE_PARITY_599_AGG_GROUP_HAVING_092.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1689539 | 1543543 | <span style="color:#2563eb">48.55%</span> |
| 799 | [00269 SCALAR_CAST_TYPEOF_011](crates/bench/sqlite_parity/cases/SQLITE_PARITY_269_SCALAR_CAST_TYPEOF_011.rs) | P1 | memory | GEN_SQL_SCALAR | 1535368 | 1543373 | <span style="color:#2563eb">48.55%</span> |
| 800 | [00239 SCALAR_NULL_COALESCE_003](crates/bench/sqlite_parity/cases/SQLITE_PARITY_239_SCALAR_NULL_COALESCE_003.rs) | P1 | memory | GEN_SQL_SCALAR | 1528976 | 1543362 | <span style="color:#2563eb">48.55%</span> |
| 801 | [01085 INDEX_SCHEMA_PRAGMA_018](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1085_INDEX_SCHEMA_PRAGMA_018.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1686875 | 1543312 | <span style="color:#2563eb">48.56%</span> |
| 802 | [00049 PRAGMA_TEMP_STORE_MEMORY](crates/bench/sqlite_parity/cases/SQLITE_PARITY_049_PRAGMA_TEMP_STORE_MEMORY.rs) | P0 | memory | SQL_PRAGMA | 1479983 | 1543252 | <span style="color:#2563eb">48.56%</span> |
| 803 | [00657 JOIN_SUBQUERY_EXISTS_050](crates/bench/sqlite_parity/cases/SQLITE_PARITY_657_JOIN_SUBQUERY_EXISTS_050.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1751487 | 1543153 | <span style="color:#2563eb">48.56%</span> |
| 804 | [00337 SCALAR_CAST_TYPEOF_028](crates/bench/sqlite_parity/cases/SQLITE_PARITY_337_SCALAR_CAST_TYPEOF_028.rs) | P1 | memory | GEN_SQL_SCALAR | 1777376 | 1543072 | <span style="color:#2563eb">48.56%</span> |
| 805 | [00651 JOIN_SUBQUERY_EXISTS_044](crates/bench/sqlite_parity/cases/SQLITE_PARITY_651_JOIN_SUBQUERY_EXISTS_044.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1694889 | 1542992 | <span style="color:#2563eb">48.57%</span> |
| 806 | [00789 WINDOW_PARTITION_SUM_002](crates/bench/sqlite_parity/cases/SQLITE_PARITY_789_WINDOW_PARTITION_SUM_002.rs) | P2 | memory | GEN_SQL_WINDOW | 1674771 | 1542952 | <span style="color:#2563eb">48.57%</span> |
| 807 | [01059 JSON_EXTRACT_SET_052](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1059_JSON_EXTRACT_SET_052.rs) | P2 | memory | GEN_SQL_JSON | 2849586 | 1542901 | <span style="color:#2563eb">48.57%</span> |
| 808 | [00683 JOIN_SUBQUERY_EXISTS_076](crates/bench/sqlite_parity/cases/SQLITE_PARITY_683_JOIN_SUBQUERY_EXISTS_076.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1759742 | 1542831 | <span style="color:#2563eb">48.57%</span> |
| 809 | [00553 AGG_GROUP_HAVING_046](crates/bench/sqlite_parity/cases/SQLITE_PARITY_553_AGG_GROUP_HAVING_046.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1719516 | 1542782 | <span style="color:#2563eb">48.57%</span> |
| 810 | [00187 OPT_NULLVALUE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_187_OPT_NULLVALUE.rs) | P1 | memory | CLI_OPTION | 1865893 | 1542771 | <span style="color:#2563eb">48.57%</span> |
| 811 | [00463 DML_WHERE_ORDER_LIMIT_076](crates/bench/sqlite_parity/cases/SQLITE_PARITY_463_DML_WHERE_ORDER_LIMIT_076.rs) | P1 | memory | GEN_SQL_DML | 1664121 | 1542541 | <span style="color:#2563eb">48.58%</span> |
| 812 | [00162 DOT_LOAD_EXTENSION_NEGATIVE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_162_DOT_LOAD_EXTENSION_NEGATIVE.rs) | P4 | side_effect | CLI_SIDE_EFFECT | 1859000 | 1542521 | <span style="color:#2563eb">48.58%</span> |
| 813 | [00956 VIEW_TRIGGER_GENERATED_009](crates/bench/sqlite_parity/cases/SQLITE_PARITY_956_VIEW_TRIGGER_GENERATED_009.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2339530 | 1542360 | <span style="color:#2563eb">48.59%</span> |
| 814 | [00298 SCALAR_STRING_018](crates/bench/sqlite_parity/cases/SQLITE_PARITY_298_SCALAR_STRING_018.rs) | P1 | memory | GEN_SQL_SCALAR | 1500281 | 1542220 | <span style="color:#2563eb">48.59%</span> |
| 815 | [00716 CTE_RECURSIVE_MATRIX_009](crates/bench/sqlite_parity/cases/SQLITE_PARITY_716_CTE_RECURSIVE_MATRIX_009.rs) | P1 | memory | GEN_SQL_CTE | 1637371 | 1541899 | <span style="color:#2563eb">48.60%</span> |
| 816 | [00177 OPT_JSON_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_177_OPT_JSON_MODE.rs) | P1 | memory | CLI_OPTION | 2454307 | 1541880 | <span style="color:#2563eb">48.60%</span> |
| 817 | [00018 DELETE_RETURNING](crates/bench/sqlite_parity/cases/SQLITE_PARITY_018_DELETE_RETURNING.rs) | P0 | memory | SQL_DELETE | 1668409 | 1541819 | <span style="color:#2563eb">48.61%</span> |
| 818 | [00540 AGG_GROUP_HAVING_033](crates/bench/sqlite_parity/cases/SQLITE_PARITY_540_AGG_GROUP_HAVING_033.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1940925 | 1541740 | <span style="color:#2563eb">48.61%</span> |
| 819 | [01038 JSON_EXTRACT_SET_031](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1038_JSON_EXTRACT_SET_031.rs) | P2 | memory | GEN_SQL_JSON | 1653902 | 1541739 | <span style="color:#2563eb">48.61%</span> |
| 820 | [00102 WITH_MATERIALIZED_HINTS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_102_WITH_MATERIALIZED_HINTS.rs) | P0 | memory | SQL_CTE | 1882034 | 1541679 | <span style="color:#2563eb">48.61%</span> |
| 821 | [01094 INDEX_SCHEMA_PRAGMA_027](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1094_INDEX_SCHEMA_PRAGMA_027.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1744604 | 1541599 | <span style="color:#2563eb">48.61%</span> |
| 822 | [00078 ON_CONFLICT_ALGORITHMS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_078_ON_CONFLICT_ALGORITHMS.rs) | P0 | memory | SQL_CONFLICT | 1621430 | 1541529 | <span style="color:#2563eb">48.62%</span> |
| 823 | [00539 AGG_GROUP_HAVING_032](crates/bench/sqlite_parity/cases/SQLITE_PARITY_539_AGG_GROUP_HAVING_032.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2026378 | 1541479 | <span style="color:#2563eb">48.62%</span> |
| 824 | [00935 CONSTRAINT_FK_SAVEPOINT_068](crates/bench/sqlite_parity/cases/SQLITE_PARITY_935_CONSTRAINT_FK_SAVEPOINT_068.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2374206 | 1541328 | <span style="color:#2563eb">48.62%</span> |
| 825 | [00199 OPT_PAGECACHE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_199_OPT_PAGECACHE.rs) | P3 | memory | CLI_OPTION | 1664181 | 1541269 | <span style="color:#2563eb">48.62%</span> |
| 826 | [00509 AGG_GROUP_HAVING_002](crates/bench/sqlite_parity/cases/SQLITE_PARITY_509_AGG_GROUP_HAVING_002.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1739814 | 1541198 | <span style="color:#2563eb">48.63%</span> |
| 827 | [00090 JSON_MUTATION_FUNCTIONS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_090_JSON_MUTATION_FUNCTIONS.rs) | P0 | memory | SQL_JSON | 1825026 | 1541158 | <span style="color:#2563eb">48.63%</span> |
| 828 | [00283 SCALAR_NULL_COALESCE_014](crates/bench/sqlite_parity/cases/SQLITE_PARITY_283_SCALAR_NULL_COALESCE_014.rs) | P1 | memory | GEN_SQL_SCALAR | 1492556 | 1541139 | <span style="color:#2563eb">48.63%</span> |
| 829 | [00245 SCALAR_CAST_TYPEOF_005](crates/bench/sqlite_parity/cases/SQLITE_PARITY_245_SCALAR_CAST_TYPEOF_005.rs) | P1 | memory | GEN_SQL_SCALAR | 1561016 | 1541048 | <span style="color:#2563eb">48.63%</span> |
| 830 | [00529 AGG_GROUP_HAVING_022](crates/bench/sqlite_parity/cases/SQLITE_PARITY_529_AGG_GROUP_HAVING_022.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1681073 | 1541048 | <span style="color:#2563eb">48.63%</span> |
| 831 | [00697 JOIN_SUBQUERY_EXISTS_090](crates/bench/sqlite_parity/cases/SQLITE_PARITY_697_JOIN_SUBQUERY_EXISTS_090.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1758620 | 1540988 | <span style="color:#2563eb">48.63%</span> |
| 832 | [01046 JSON_EXTRACT_SET_039](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1046_JSON_EXTRACT_SET_039.rs) | P2 | memory | GEN_SQL_JSON | 1617332 | 1540968 | <span style="color:#2563eb">48.63%</span> |
| 833 | [00268 SCALAR_ARITH_011](crates/bench/sqlite_parity/cases/SQLITE_PARITY_268_SCALAR_ARITH_011.rs) | P1 | memory | GEN_SQL_SCALAR | 1480693 | 1540727 | <span style="color:#2563eb">48.64%</span> |
| 834 | [00991 VIEW_TRIGGER_GENERATED_044](crates/bench/sqlite_parity/cases/SQLITE_PARITY_991_VIEW_TRIGGER_GENERATED_044.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1784639 | 1540647 | <span style="color:#2563eb">48.65%</span> |
| 835 | [00865 WINDOW_PARTITION_SUM_078](crates/bench/sqlite_parity/cases/SQLITE_PARITY_865_WINDOW_PARTITION_SUM_078.rs) | P2 | memory | GEN_SQL_WINDOW | 1675432 | 1540627 | <span style="color:#2563eb">48.65%</span> |
| 836 | [00103 WINDOW_NAMED_WINDOW](crates/bench/sqlite_parity/cases/SQLITE_PARITY_103_WINDOW_NAMED_WINDOW.rs) | P0 | memory | SQL_WINDOW | 1579991 | 1540567 | <span style="color:#2563eb">48.65%</span> |
| 837 | [00796 WINDOW_PARTITION_SUM_009](crates/bench/sqlite_parity/cases/SQLITE_PARITY_796_WINDOW_PARTITION_SUM_009.rs) | P2 | memory | GEN_SQL_WINDOW | 1664862 | 1540538 | <span style="color:#2563eb">48.65%</span> |
| 838 | [00241 SCALAR_CAST_TYPEOF_004](crates/bench/sqlite_parity/cases/SQLITE_PARITY_241_SCALAR_CAST_TYPEOF_004.rs) | P1 | memory | GEN_SQL_SCALAR | 1549615 | 1540387 | <span style="color:#2563eb">48.65%</span> |
| 839 | [01002 VIEW_TRIGGER_GENERATED_055](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1002_VIEW_TRIGGER_GENERATED_055.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1772296 | 1540266 | <span style="color:#2563eb">48.66%</span> |
| 840 | [00954 VIEW_TRIGGER_GENERATED_007](crates/bench/sqlite_parity/cases/SQLITE_PARITY_954_VIEW_TRIGGER_GENERATED_007.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1884458 | 1540217 | <span style="color:#2563eb">48.66%</span> |
| 841 | [00779 CTE_RECURSIVE_MATRIX_072](crates/bench/sqlite_parity/cases/SQLITE_PARITY_779_CTE_RECURSIVE_MATRIX_072.rs) | P1 | memory | GEN_SQL_CTE | 1613425 | 1540126 | <span style="color:#2563eb">48.66%</span> |
| 842 | [00803 WINDOW_PARTITION_SUM_016](crates/bench/sqlite_parity/cases/SQLITE_PARITY_803_WINDOW_PARTITION_SUM_016.rs) | P2 | memory | GEN_SQL_WINDOW | 1674361 | 1540007 | <span style="color:#2563eb">48.67%</span> |
| 843 | [00965 VIEW_TRIGGER_GENERATED_018](crates/bench/sqlite_parity/cases/SQLITE_PARITY_965_VIEW_TRIGGER_GENERATED_018.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1783878 | 1539966 | <span style="color:#2563eb">48.67%</span> |
| 844 | [01018 JSON_EXTRACT_SET_011](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1018_JSON_EXTRACT_SET_011.rs) | P2 | memory | GEN_SQL_JSON | 1742840 | 1539916 | <span style="color:#2563eb">48.67%</span> |
| 845 | [00114 DOT_PRINT](crates/bench/sqlite_parity/cases/SQLITE_PARITY_114_DOT_PRINT.rs) | P0 | memory | CLI_DOT_COMMAND | 1472489 | 1539856 | <span style="color:#2563eb">48.67%</span> |
| 846 | [00537 AGG_GROUP_HAVING_030](crates/bench/sqlite_parity/cases/SQLITE_PARITY_537_AGG_GROUP_HAVING_030.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1769120 | 1539856 | <span style="color:#2563eb">48.67%</span> |
| 847 | [00110 DOT_MODE_LINE_COLUMN_TABLE_BOX_MARKDOWN](crates/bench/sqlite_parity/cases/SQLITE_PARITY_110_DOT_MODE_LINE_COLUMN_TABLE_BOX_MARKDOWN.rs) | P0 | memory | CLI_DOT_COMMAND | 1505721 | 1539796 | <span style="color:#2563eb">48.67%</span> |
| 848 | [00723 CTE_RECURSIVE_MATRIX_016](crates/bench/sqlite_parity/cases/SQLITE_PARITY_723_CTE_RECURSIVE_MATRIX_016.rs) | P1 | memory | GEN_SQL_CTE | 1626370 | 1539756 | <span style="color:#2563eb">48.67%</span> |
| 849 | [00870 CONSTRAINT_FK_SAVEPOINT_003](crates/bench/sqlite_parity/cases/SQLITE_PARITY_870_CONSTRAINT_FK_SAVEPOINT_003.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1743341 | 1539566 | <span style="color:#2563eb">48.68%</span> |
| 850 | [00638 JOIN_SUBQUERY_EXISTS_031](crates/bench/sqlite_parity/cases/SQLITE_PARITY_638_JOIN_SUBQUERY_EXISTS_031.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1755334 | 1539465 | <span style="color:#2563eb">48.68%</span> |
| 851 | [00441 DML_WHERE_ORDER_LIMIT_054](crates/bench/sqlite_parity/cases/SQLITE_PARITY_441_DML_WHERE_ORDER_LIMIT_054.rs) | P1 | memory | GEN_SQL_DML | 1673259 | 1539405 | <span style="color:#2563eb">48.69%</span> |
| 852 | [00712 CTE_RECURSIVE_MATRIX_005](crates/bench/sqlite_parity/cases/SQLITE_PARITY_712_CTE_RECURSIVE_MATRIX_005.rs) | P1 | memory | GEN_SQL_CTE | 1592585 | 1539155 | <span style="color:#2563eb">48.69%</span> |
| 853 | [00753 CTE_RECURSIVE_MATRIX_046](crates/bench/sqlite_parity/cases/SQLITE_PARITY_753_CTE_RECURSIVE_MATRIX_046.rs) | P1 | memory | GEN_SQL_CTE | 1586655 | 1539015 | <span style="color:#2563eb">48.70%</span> |
| 854 | [00433 DML_WHERE_ORDER_LIMIT_046](crates/bench/sqlite_parity/cases/SQLITE_PARITY_433_DML_WHERE_ORDER_LIMIT_046.rs) | P1 | memory | GEN_SQL_DML | 2164148 | 1538995 | <span style="color:#2563eb">48.70%</span> |
| 855 | [00619 JOIN_SUBQUERY_EXISTS_012](crates/bench/sqlite_parity/cases/SQLITE_PARITY_619_JOIN_SUBQUERY_EXISTS_012.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1741778 | 1538934 | <span style="color:#2563eb">48.70%</span> |
| 856 | [00483 DML_WHERE_ORDER_LIMIT_096](crates/bench/sqlite_parity/cases/SQLITE_PARITY_483_DML_WHERE_ORDER_LIMIT_096.rs) | P1 | memory | GEN_SQL_DML | 1633834 | 1538924 | <span style="color:#2563eb">48.70%</span> |
| 857 | [00387 SCALAR_NULL_COALESCE_040](crates/bench/sqlite_parity/cases/SQLITE_PARITY_387_SCALAR_NULL_COALESCE_040.rs) | P1 | memory | GEN_SQL_SCALAR | 1514057 | 1538834 | <span style="color:#2563eb">48.71%</span> |
| 858 | [00352 SCALAR_ARITH_032](crates/bench/sqlite_parity/cases/SQLITE_PARITY_352_SCALAR_ARITH_032.rs) | P1 | memory | GEN_SQL_SCALAR | 1799498 | 1538824 | <span style="color:#2563eb">48.71%</span> |
| 859 | [00002 LITERALS_AND_TYPEOF](crates/bench/sqlite_parity/cases/SQLITE_PARITY_002_LITERALS_AND_TYPEOF.rs) | P0 | memory | SQL_EXPRESSIONS | 1690641 | 1538533 | <span style="color:#2563eb">48.72%</span> |
| 860 | [00943 CONSTRAINT_FK_SAVEPOINT_076](crates/bench/sqlite_parity/cases/SQLITE_PARITY_943_CONSTRAINT_FK_SAVEPOINT_076.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2195878 | 1538523 | <span style="color:#2563eb">48.72%</span> |
| 861 | [00637 JOIN_SUBQUERY_EXISTS_030](crates/bench/sqlite_parity/cases/SQLITE_PARITY_637_JOIN_SUBQUERY_EXISTS_030.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2077394 | 1538373 | <span style="color:#2563eb">48.72%</span> |
| 862 | [00797 WINDOW_PARTITION_SUM_010](crates/bench/sqlite_parity/cases/SQLITE_PARITY_797_WINDOW_PARTITION_SUM_010.rs) | P2 | memory | GEN_SQL_WINDOW | 1671825 | 1538354 | <span style="color:#2563eb">48.72%</span> |
| 863 | [00438 DML_WHERE_ORDER_LIMIT_051](crates/bench/sqlite_parity/cases/SQLITE_PARITY_438_DML_WHERE_ORDER_LIMIT_051.rs) | P1 | memory | GEN_SQL_DML | 1625348 | 1538233 | <span style="color:#2563eb">48.73%</span> |
| 864 | [00367 SCALAR_NULL_COALESCE_035](crates/bench/sqlite_parity/cases/SQLITE_PARITY_367_SCALAR_NULL_COALESCE_035.rs) | P1 | memory | GEN_SQL_SCALAR | 1678047 | 1538163 | <span style="color:#2563eb">48.73%</span> |
| 865 | [00930 CONSTRAINT_FK_SAVEPOINT_063](crates/bench/sqlite_parity/cases/SQLITE_PARITY_930_CONSTRAINT_FK_SAVEPOINT_063.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1729124 | 1538073 | <span style="color:#2563eb">48.73%</span> |
| 866 | [00958 VIEW_TRIGGER_GENERATED_011](crates/bench/sqlite_parity/cases/SQLITE_PARITY_958_VIEW_TRIGGER_GENERATED_011.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1734455 | 1537972 | <span style="color:#2563eb">48.73%</span> |
| 867 | [00585 AGG_GROUP_HAVING_078](crates/bench/sqlite_parity/cases/SQLITE_PARITY_585_AGG_GROUP_HAVING_078.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2001340 | 1537932 | <span style="color:#2563eb">48.74%</span> |
| 868 | [00279 SCALAR_NULL_COALESCE_013](crates/bench/sqlite_parity/cases/SQLITE_PARITY_279_SCALAR_NULL_COALESCE_013.rs) | P1 | memory | GEN_SQL_SCALAR | 1523575 | 1537922 | <span style="color:#2563eb">48.74%</span> |
| 869 | [00469 DML_WHERE_ORDER_LIMIT_082](crates/bench/sqlite_parity/cases/SQLITE_PARITY_469_DML_WHERE_ORDER_LIMIT_082.rs) | P1 | memory | GEN_SQL_DML | 1683839 | 1537782 | <span style="color:#2563eb">48.74%</span> |
| 870 | [00569 AGG_GROUP_HAVING_062](crates/bench/sqlite_parity/cases/SQLITE_PARITY_569_AGG_GROUP_HAVING_062.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1732551 | 1537721 | <span style="color:#2563eb">48.74%</span> |
| 871 | [00185 OPT_SEPARATOR](crates/bench/sqlite_parity/cases/SQLITE_PARITY_185_OPT_SEPARATOR.rs) | P1 | memory | CLI_OPTION | 1578909 | 1537672 | <span style="color:#2563eb">48.74%</span> |
| 872 | [00758 CTE_RECURSIVE_MATRIX_051](crates/bench/sqlite_parity/cases/SQLITE_PARITY_758_CTE_RECURSIVE_MATRIX_051.rs) | P1 | memory | GEN_SQL_CTE | 1607303 | 1537511 | <span style="color:#2563eb">48.75%</span> |
| 873 | [01071 INDEX_SCHEMA_PRAGMA_004](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1071_INDEX_SCHEMA_PRAGMA_004.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1738482 | 1537441 | <span style="color:#2563eb">48.75%</span> |
| 874 | [00592 AGG_GROUP_HAVING_085](crates/bench/sqlite_parity/cases/SQLITE_PARITY_592_AGG_GROUP_HAVING_085.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1737360 | 1537422 | <span style="color:#2563eb">48.75%</span> |
| 875 | [00730 CTE_RECURSIVE_MATRIX_023](crates/bench/sqlite_parity/cases/SQLITE_PARITY_730_CTE_RECURSIVE_MATRIX_023.rs) | P1 | memory | GEN_SQL_CTE | 1570093 | 1537371 | <span style="color:#2563eb">48.75%</span> |
| 876 | [00869 CONSTRAINT_FK_SAVEPOINT_002](crates/bench/sqlite_parity/cases/SQLITE_PARITY_869_CONSTRAINT_FK_SAVEPOINT_002.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1758790 | 1537291 | <span style="color:#2563eb">48.76%</span> |
| 877 | [00059 AGGREGATE_FUNCTIONS_CORE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_059_AGGREGATE_FUNCTIONS_CORE.rs) | P0 | memory | SQL_FUNCTIONS | 1506032 | 1537201 | <span style="color:#2563eb">48.76%</span> |
| 878 | [00775 CTE_RECURSIVE_MATRIX_068](crates/bench/sqlite_parity/cases/SQLITE_PARITY_775_CTE_RECURSIVE_MATRIX_068.rs) | P1 | memory | GEN_SQL_CTE | 1607243 | 1537191 | <span style="color:#2563eb">48.76%</span> |
| 879 | [00906 CONSTRAINT_FK_SAVEPOINT_039](crates/bench/sqlite_parity/cases/SQLITE_PARITY_906_CONSTRAINT_FK_SAVEPOINT_039.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1672657 | 1536991 | <span style="color:#2563eb">48.77%</span> |
| 880 | [00264 SCALAR_ARITH_010](crates/bench/sqlite_parity/cases/SQLITE_PARITY_264_SCALAR_ARITH_010.rs) | P1 | memory | GEN_SQL_SCALAR | 1868879 | 1536950 | <span style="color:#2563eb">48.77%</span> |
| 881 | [00158 DOT_SHELL_SIDE_EFFECT](crates/bench/sqlite_parity/cases/SQLITE_PARITY_158_DOT_SHELL_SIDE_EFFECT.rs) | P4 | side_effect | CLI_SIDE_EFFECT | 2714820 | 1536880 | <span style="color:#2563eb">48.77%</span> |
| 882 | [00480 DML_WHERE_ORDER_LIMIT_093](crates/bench/sqlite_parity/cases/SQLITE_PARITY_480_DML_WHERE_ORDER_LIMIT_093.rs) | P1 | memory | GEN_SQL_DML | 1650796 | 1536860 | <span style="color:#2563eb">48.77%</span> |
| 883 | [00681 JOIN_SUBQUERY_EXISTS_074](crates/bench/sqlite_parity/cases/SQLITE_PARITY_681_JOIN_SUBQUERY_EXISTS_074.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1741278 | 1536840 | <span style="color:#2563eb">48.77%</span> |
| 884 | [00858 WINDOW_PARTITION_SUM_071](crates/bench/sqlite_parity/cases/SQLITE_PARITY_858_WINDOW_PARTITION_SUM_071.rs) | P2 | memory | GEN_SQL_WINDOW | 1664563 | 1536680 | <span style="color:#2563eb">48.78%</span> |
| 885 | [00947 CONSTRAINT_FK_SAVEPOINT_080](crates/bench/sqlite_parity/cases/SQLITE_PARITY_947_CONSTRAINT_FK_SAVEPOINT_080.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1748170 | 1536670 | <span style="color:#2563eb">48.78%</span> |
| 886 | [00280 SCALAR_ARITH_014](crates/bench/sqlite_parity/cases/SQLITE_PARITY_280_SCALAR_ARITH_014.rs) | P1 | memory | GEN_SQL_SCALAR | 1515630 | 1536660 | <span style="color:#2563eb">48.78%</span> |
| 887 | [00070 LIKE_GLOB_MATCH_ESCAPE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_070_LIKE_GLOB_MATCH_ESCAPE.rs) | P0 | memory | SQL_OPERATORS | 1629555 | 1536600 | <span style="color:#2563eb">48.78%</span> |
| 888 | [00046 VACUUM_MEMORY](crates/bench/sqlite_parity/cases/SQLITE_PARITY_046_VACUUM_MEMORY.rs) | P0 | memory | SQL_VACUUM | 1720127 | 1536580 | <span style="color:#2563eb">48.78%</span> |
| 889 | [01086 INDEX_SCHEMA_PRAGMA_019](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1086_INDEX_SCHEMA_PRAGMA_019.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 2054019 | 1536580 | <span style="color:#2563eb">48.78%</span> |
| 890 | [00424 DML_WHERE_ORDER_LIMIT_037](crates/bench/sqlite_parity/cases/SQLITE_PARITY_424_DML_WHERE_ORDER_LIMIT_037.rs) | P1 | memory | GEN_SQL_DML | 1643312 | 1536389 | <span style="color:#2563eb">48.79%</span> |
| 891 | [01005 VIEW_TRIGGER_GENERATED_058](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1005_VIEW_TRIGGER_GENERATED_058.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2287902 | 1536369 | <span style="color:#2563eb">48.79%</span> |
| 892 | [00598 AGG_GROUP_HAVING_091](crates/bench/sqlite_parity/cases/SQLITE_PARITY_598_AGG_GROUP_HAVING_091.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1776784 | 1535979 | <span style="color:#2563eb">48.80%</span> |
| 893 | [00498 DML_WHERE_ORDER_LIMIT_111](crates/bench/sqlite_parity/cases/SQLITE_PARITY_498_DML_WHERE_ORDER_LIMIT_111.rs) | P1 | memory | GEN_SQL_DML | 2008643 | 1535898 | <span style="color:#2563eb">48.80%</span> |
| 894 | [00901 CONSTRAINT_FK_SAVEPOINT_034](crates/bench/sqlite_parity/cases/SQLITE_PARITY_901_CONSTRAINT_FK_SAVEPOINT_034.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1716220 | 1535588 | <span style="color:#2563eb">48.81%</span> |
| 895 | [00617 JOIN_SUBQUERY_EXISTS_010](crates/bench/sqlite_parity/cases/SQLITE_PARITY_617_JOIN_SUBQUERY_EXISTS_010.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1792264 | 1535478 | <span style="color:#2563eb">48.82%</span> |
| 896 | [00937 CONSTRAINT_FK_SAVEPOINT_070](crates/bench/sqlite_parity/cases/SQLITE_PARITY_937_CONSTRAINT_FK_SAVEPOINT_070.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1745485 | 1535468 | <span style="color:#2563eb">48.82%</span> |
| 897 | [00635 JOIN_SUBQUERY_EXISTS_028](crates/bench/sqlite_parity/cases/SQLITE_PARITY_635_JOIN_SUBQUERY_EXISTS_028.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1738101 | 1535447 | <span style="color:#2563eb">48.82%</span> |
| 898 | [00948 VIEW_TRIGGER_GENERATED_001](crates/bench/sqlite_parity/cases/SQLITE_PARITY_948_VIEW_TRIGGER_GENERATED_001.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2206458 | 1534957 | <span style="color:#2563eb">48.83%</span> |
| 899 | [00366 SCALAR_STRING_035](crates/bench/sqlite_parity/cases/SQLITE_PARITY_366_SCALAR_STRING_035.rs) | P1 | memory | GEN_SQL_SCALAR | 1513015 | 1534956 | <span style="color:#2563eb">48.83%</span> |
| 900 | [00399 DML_WHERE_ORDER_LIMIT_012](crates/bench/sqlite_parity/cases/SQLITE_PARITY_399_DML_WHERE_ORDER_LIMIT_012.rs) | P1 | memory | GEN_SQL_DML | 1750525 | 1534926 | <span style="color:#2563eb">48.84%</span> |
| 901 | [00541 AGG_GROUP_HAVING_034](crates/bench/sqlite_parity/cases/SQLITE_PARITY_541_AGG_GROUP_HAVING_034.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1727752 | 1534926 | <span style="color:#2563eb">48.84%</span> |
| 902 | [00285 SCALAR_CAST_TYPEOF_015](crates/bench/sqlite_parity/cases/SQLITE_PARITY_285_SCALAR_CAST_TYPEOF_015.rs) | P1 | memory | GEN_SQL_SCALAR | 1776955 | 1534857 | <span style="color:#2563eb">48.84%</span> |
| 903 | [00974 VIEW_TRIGGER_GENERATED_027](crates/bench/sqlite_parity/cases/SQLITE_PARITY_974_VIEW_TRIGGER_GENERATED_027.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1749794 | 1534747 | <span style="color:#2563eb">48.84%</span> |
| 904 | [00970 VIEW_TRIGGER_GENERATED_023](crates/bench/sqlite_parity/cases/SQLITE_PARITY_970_VIEW_TRIGGER_GENERATED_023.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1750295 | 1534306 | <span style="color:#2563eb">48.86%</span> |
| 905 | [00820 WINDOW_PARTITION_SUM_033](crates/bench/sqlite_parity/cases/SQLITE_PARITY_820_WINDOW_PARTITION_SUM_033.rs) | P2 | memory | GEN_SQL_WINDOW | 1678529 | 1534285 | <span style="color:#2563eb">48.86%</span> |
| 906 | [00113 DOT_NULLVALUE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_113_DOT_NULLVALUE.rs) | P0 | memory | CLI_DOT_COMMAND | 1497285 | 1534195 | <span style="color:#2563eb">48.86%</span> |
| 907 | [00472 DML_WHERE_ORDER_LIMIT_085](crates/bench/sqlite_parity/cases/SQLITE_PARITY_472_DML_WHERE_ORDER_LIMIT_085.rs) | P1 | memory | GEN_SQL_DML | 1635828 | 1534175 | <span style="color:#2563eb">48.86%</span> |
| 908 | [00380 SCALAR_ARITH_039](crates/bench/sqlite_parity/cases/SQLITE_PARITY_380_SCALAR_ARITH_039.rs) | P1 | memory | GEN_SQL_SCALAR | 2444639 | 1534015 | <span style="color:#2563eb">48.87%</span> |
| 909 | [00189 OPT_ECHO](crates/bench/sqlite_parity/cases/SQLITE_PARITY_189_OPT_ECHO.rs) | P2 | memory | CLI_OPTION | 1514928 | 1534005 | <span style="color:#2563eb">48.87%</span> |
| 910 | [00338 SCALAR_STRING_028](crates/bench/sqlite_parity/cases/SQLITE_PARITY_338_SCALAR_STRING_028.rs) | P1 | memory | GEN_SQL_SCALAR | 1522042 | 1533905 | <span style="color:#2563eb">48.87%</span> |
| 911 | [00408 DML_WHERE_ORDER_LIMIT_021](crates/bench/sqlite_parity/cases/SQLITE_PARITY_408_DML_WHERE_ORDER_LIMIT_021.rs) | P1 | memory | GEN_SQL_DML | 1818924 | 1533825 | <span style="color:#2563eb">48.87%</span> |
| 912 | [00984 VIEW_TRIGGER_GENERATED_037](crates/bench/sqlite_parity/cases/SQLITE_PARITY_984_VIEW_TRIGGER_GENERATED_037.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1744663 | 1533774 | <span style="color:#2563eb">48.87%</span> |
| 913 | [00634 JOIN_SUBQUERY_EXISTS_027](crates/bench/sqlite_parity/cases/SQLITE_PARITY_634_JOIN_SUBQUERY_EXISTS_027.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1722151 | 1533664 | <span style="color:#2563eb">48.88%</span> |
| 914 | [00549 AGG_GROUP_HAVING_042](crates/bench/sqlite_parity/cases/SQLITE_PARITY_549_AGG_GROUP_HAVING_042.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1747869 | 1533645 | <span style="color:#2563eb">48.88%</span> |
| 915 | [00579 AGG_GROUP_HAVING_072](crates/bench/sqlite_parity/cases/SQLITE_PARITY_579_AGG_GROUP_HAVING_072.rs) | P1 | memory | GEN_SQL_AGGREGATE | 2051696 | 1533635 | <span style="color:#2563eb">48.88%</span> |
| 916 | [00836 WINDOW_PARTITION_SUM_049](crates/bench/sqlite_parity/cases/SQLITE_PARITY_836_WINDOW_PARTITION_SUM_049.rs) | P2 | memory | GEN_SQL_WINDOW | 1670112 | 1533554 | <span style="color:#2563eb">48.88%</span> |
| 917 | [00536 AGG_GROUP_HAVING_029](crates/bench/sqlite_parity/cases/SQLITE_PARITY_536_AGG_GROUP_HAVING_029.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1931548 | 1533284 | <span style="color:#2563eb">48.89%</span> |
| 918 | [00277 SCALAR_CAST_TYPEOF_013](crates/bench/sqlite_parity/cases/SQLITE_PARITY_277_SCALAR_CAST_TYPEOF_013.rs) | P1 | memory | GEN_SQL_SCALAR | 1478720 | 1533283 | <span style="color:#2563eb">48.89%</span> |
| 919 | [00237 SCALAR_CAST_TYPEOF_003](crates/bench/sqlite_parity/cases/SQLITE_PARITY_237_SCALAR_CAST_TYPEOF_003.rs) | P1 | memory | GEN_SQL_SCALAR | 1831188 | 1533143 | <span style="color:#2563eb">48.90%</span> |
| 920 | [01092 INDEX_SCHEMA_PRAGMA_025](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1092_INDEX_SCHEMA_PRAGMA_025.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1743302 | 1532923 | <span style="color:#2563eb">48.90%</span> |
| 921 | [00920 CONSTRAINT_FK_SAVEPOINT_053](crates/bench/sqlite_parity/cases/SQLITE_PARITY_920_CONSTRAINT_FK_SAVEPOINT_053.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2071121 | 1532913 | <span style="color:#2563eb">48.90%</span> |
| 922 | [00231 SCALAR_NULL_COALESCE_001](crates/bench/sqlite_parity/cases/SQLITE_PARITY_231_SCALAR_NULL_COALESCE_001.rs) | P1 | memory | GEN_SQL_SCALAR | 1481495 | 1532903 | <span style="color:#2563eb">48.90%</span> |
| 923 | [00745 CTE_RECURSIVE_MATRIX_038](crates/bench/sqlite_parity/cases/SQLITE_PARITY_745_CTE_RECURSIVE_MATRIX_038.rs) | P1 | memory | GEN_SQL_CTE | 1543322 | 1532832 | <span style="color:#2563eb">48.91%</span> |
| 924 | [00401 DML_WHERE_ORDER_LIMIT_014](crates/bench/sqlite_parity/cases/SQLITE_PARITY_401_DML_WHERE_ORDER_LIMIT_014.rs) | P1 | memory | GEN_SQL_DML | 1868729 | 1532783 | <span style="color:#2563eb">48.91%</span> |
| 925 | [00088 JSON_SCALAR_FUNCTIONS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_088_JSON_SCALAR_FUNCTIONS.rs) | P0 | memory | SQL_JSON | 1557760 | 1532692 | <span style="color:#2563eb">48.91%</span> |
| 926 | [00601 AGG_GROUP_HAVING_094](crates/bench/sqlite_parity/cases/SQLITE_PARITY_601_AGG_GROUP_HAVING_094.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1722963 | 1532692 | <span style="color:#2563eb">48.91%</span> |
| 927 | [00443 DML_WHERE_ORDER_LIMIT_056](crates/bench/sqlite_parity/cases/SQLITE_PARITY_443_DML_WHERE_ORDER_LIMIT_056.rs) | P1 | memory | GEN_SQL_DML | 1713284 | 1532592 | <span style="color:#2563eb">48.91%</span> |
| 928 | [00155 DOT_DBTOTXT_TEMPFILE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_155_DOT_DBTOTXT_TEMPFILE.rs) | P3 | tempfile | CLI_TEMPFILE | 1847228 | 1532542 | <span style="color:#2563eb">48.92%</span> |
| 929 | [00871 CONSTRAINT_FK_SAVEPOINT_004](crates/bench/sqlite_parity/cases/SQLITE_PARITY_871_CONSTRAINT_FK_SAVEPOINT_004.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1749042 | 1532462 | <span style="color:#2563eb">48.92%</span> |
| 930 | [00709 CTE_RECURSIVE_MATRIX_002](crates/bench/sqlite_parity/cases/SQLITE_PARITY_709_CTE_RECURSIVE_MATRIX_002.rs) | P1 | memory | GEN_SQL_CTE | 1760283 | 1532452 | <span style="color:#2563eb">48.92%</span> |
| 931 | [00503 DML_WHERE_ORDER_LIMIT_116](crates/bench/sqlite_parity/cases/SQLITE_PARITY_503_DML_WHERE_ORDER_LIMIT_116.rs) | P1 | memory | GEN_SQL_DML | 1619937 | 1532432 | <span style="color:#2563eb">48.92%</span> |
| 932 | [00808 WINDOW_PARTITION_SUM_021](crates/bench/sqlite_parity/cases/SQLITE_PARITY_808_WINDOW_PARTITION_SUM_021.rs) | P2 | memory | GEN_SQL_WINDOW | 1974879 | 1532391 | <span style="color:#2563eb">48.92%</span> |
| 933 | [00474 DML_WHERE_ORDER_LIMIT_087](crates/bench/sqlite_parity/cases/SQLITE_PARITY_474_DML_WHERE_ORDER_LIMIT_087.rs) | P1 | memory | GEN_SQL_DML | 1591965 | 1532382 | <span style="color:#2563eb">48.92%</span> |
| 934 | [00921 CONSTRAINT_FK_SAVEPOINT_054](crates/bench/sqlite_parity/cases/SQLITE_PARITY_921_CONSTRAINT_FK_SAVEPOINT_054.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1696052 | 1532351 | <span style="color:#2563eb">48.92%</span> |
| 935 | [00593 AGG_GROUP_HAVING_086](crates/bench/sqlite_parity/cases/SQLITE_PARITY_593_AGG_GROUP_HAVING_086.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1704217 | 1532162 | <span style="color:#2563eb">48.93%</span> |
| 936 | [00663 JOIN_SUBQUERY_EXISTS_056](crates/bench/sqlite_parity/cases/SQLITE_PARITY_663_JOIN_SUBQUERY_EXISTS_056.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1741969 | 1532151 | <span style="color:#2563eb">48.93%</span> |
| 937 | [00173 OPT_LIST_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_173_OPT_LIST_MODE.rs) | P1 | memory | CLI_OPTION | 1663170 | 1532061 | <span style="color:#2563eb">48.93%</span> |
| 938 | [01123 INDEX_SCHEMA_PRAGMA_056](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1123_INDEX_SCHEMA_PRAGMA_056.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1755645 | 1531981 | <span style="color:#2563eb">48.93%</span> |
| 939 | [00517 AGG_GROUP_HAVING_010](crates/bench/sqlite_parity/cases/SQLITE_PARITY_517_AGG_GROUP_HAVING_010.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1723253 | 1531971 | <span style="color:#2563eb">48.93%</span> |
| 940 | [00282 SCALAR_STRING_014](crates/bench/sqlite_parity/cases/SQLITE_PARITY_282_SCALAR_STRING_014.rs) | P1 | memory | GEN_SQL_SCALAR | 1450506 | 1531870 | <span style="color:#2563eb">48.94%</span> |
| 941 | [00271 SCALAR_NULL_COALESCE_011](crates/bench/sqlite_parity/cases/SQLITE_PARITY_271_SCALAR_NULL_COALESCE_011.rs) | P1 | memory | GEN_SQL_SCALAR | 1519147 | 1531851 | <span style="color:#2563eb">48.94%</span> |
| 942 | [00041 DROP_TRIGGER](crates/bench/sqlite_parity/cases/SQLITE_PARITY_041_DROP_TRIGGER.rs) | P0 | memory | SQL_DROP | 1576075 | 1531641 | <span style="color:#2563eb">48.95%</span> |
| 943 | [00097 CLI_GENERATE_SERIES_OPTIONAL](crates/bench/sqlite_parity/cases/SQLITE_PARITY_097_CLI_GENERATE_SERIES_OPTIONAL.rs) | P2 | memory | CLI_EXTENSION_OPTIONAL | 1582647 | 1531620 | <span style="color:#2563eb">48.95%</span> |
| 944 | [00024 SAVEPOINT_ROLLBACK_RELEASE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_024_SAVEPOINT_ROLLBACK_RELEASE.rs) | P0 | memory | SQL_SAVEPOINT | 1648562 | 1531510 | <span style="color:#2563eb">48.95%</span> |
| 945 | [00343 SCALAR_NULL_COALESCE_029](crates/bench/sqlite_parity/cases/SQLITE_PARITY_343_SCALAR_NULL_COALESCE_029.rs) | P1 | memory | GEN_SQL_SCALAR | 1538694 | 1531490 | <span style="color:#2563eb">48.95%</span> |
| 946 | [00810 WINDOW_PARTITION_SUM_023](crates/bench/sqlite_parity/cases/SQLITE_PARITY_810_WINDOW_PARTITION_SUM_023.rs) | P2 | memory | GEN_SQL_WINDOW | 1646989 | 1531480 | <span style="color:#2563eb">48.95%</span> |
| 947 | [00462 DML_WHERE_ORDER_LIMIT_075](crates/bench/sqlite_parity/cases/SQLITE_PARITY_462_DML_WHERE_ORDER_LIMIT_075.rs) | P1 | memory | GEN_SQL_DML | 2194395 | 1531410 | <span style="color:#2563eb">48.95%</span> |
| 948 | [00781 CTE_RECURSIVE_MATRIX_074](crates/bench/sqlite_parity/cases/SQLITE_PARITY_781_CTE_RECURSIVE_MATRIX_074.rs) | P1 | memory | GEN_SQL_CTE | 1588569 | 1531410 | <span style="color:#2563eb">48.95%</span> |
| 949 | [00857 WINDOW_PARTITION_SUM_070](crates/bench/sqlite_parity/cases/SQLITE_PARITY_857_WINDOW_PARTITION_SUM_070.rs) | P2 | memory | GEN_SQL_WINDOW | 1671636 | 1531380 | <span style="color:#2563eb">48.95%</span> |
| 950 | [00019 REPLACE_INTO](crates/bench/sqlite_parity/cases/SQLITE_PARITY_019_REPLACE_INTO.rs) | P0 | memory | SQL_REPLACE | 1879589 | 1531099 | <span style="color:#2563eb">48.96%</span> |
| 951 | [00093 CREATE_VIRTUAL_TABLE_FTS5_OPTIONAL](crates/bench/sqlite_parity/cases/SQLITE_PARITY_093_CREATE_VIRTUAL_TABLE_FTS5_OPTIONAL.rs) | P1 | memory | SQL_VIRTUAL_TABLE_OPTIONAL | 1890991 | 1531029 | <span style="color:#2563eb">48.97%</span> |
| 952 | [01037 JSON_EXTRACT_SET_030](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1037_JSON_EXTRACT_SET_030.rs) | P2 | memory | GEN_SQL_JSON | 1648111 | 1530908 | <span style="color:#2563eb">48.97%</span> |
| 953 | [00777 CTE_RECURSIVE_MATRIX_070](crates/bench/sqlite_parity/cases/SQLITE_PARITY_777_CTE_RECURSIVE_MATRIX_070.rs) | P1 | memory | GEN_SQL_CTE | 1594249 | 1530788 | <span style="color:#2563eb">48.97%</span> |
| 954 | [00278 SCALAR_STRING_013](crates/bench/sqlite_parity/cases/SQLITE_PARITY_278_SCALAR_STRING_013.rs) | P1 | memory | GEN_SQL_SCALAR | 1511953 | 1530689 | <span style="color:#2563eb">48.98%</span> |
| 955 | [00253 SCALAR_CAST_TYPEOF_007](crates/bench/sqlite_parity/cases/SQLITE_PARITY_253_SCALAR_CAST_TYPEOF_007.rs) | P1 | memory | GEN_SQL_SCALAR | 1506924 | 1530618 | <span style="color:#2563eb">48.98%</span> |
| 956 | [00760 CTE_RECURSIVE_MATRIX_053](crates/bench/sqlite_parity/cases/SQLITE_PARITY_760_CTE_RECURSIVE_MATRIX_053.rs) | P1 | memory | GEN_SQL_CTE | 1627142 | 1530238 | <span style="color:#2563eb">48.99%</span> |
| 957 | [00251 SCALAR_NULL_COALESCE_006](crates/bench/sqlite_parity/cases/SQLITE_PARITY_251_SCALAR_NULL_COALESCE_006.rs) | P1 | memory | GEN_SQL_SCALAR | 1814757 | 1530148 | <span style="color:#2563eb">49.00%</span> |
| 958 | [00818 WINDOW_PARTITION_SUM_031](crates/bench/sqlite_parity/cases/SQLITE_PARITY_818_WINDOW_PARTITION_SUM_031.rs) | P2 | memory | GEN_SQL_WINDOW | 1729014 | 1530127 | <span style="color:#2563eb">49.00%</span> |
| 959 | [00250 SCALAR_STRING_006](crates/bench/sqlite_parity/cases/SQLITE_PARITY_250_SCALAR_STRING_006.rs) | P1 | memory | GEN_SQL_SCALAR | 1500812 | 1530097 | <span style="color:#2563eb">49.00%</span> |
| 960 | [00581 AGG_GROUP_HAVING_074](crates/bench/sqlite_parity/cases/SQLITE_PARITY_581_AGG_GROUP_HAVING_074.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1705830 | 1529917 | <span style="color:#2563eb">49.00%</span> |
| 961 | [00048 PRAGMA_USER_VERSION](crates/bench/sqlite_parity/cases/SQLITE_PARITY_048_PRAGMA_USER_VERSION.rs) | P0 | memory | SQL_PRAGMA | 1486324 | 1529907 | <span style="color:#2563eb">49.00%</span> |
| 962 | [00971 VIEW_TRIGGER_GENERATED_024](crates/bench/sqlite_parity/cases/SQLITE_PARITY_971_VIEW_TRIGGER_GENERATED_024.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 1754182 | 1529787 | <span style="color:#2563eb">49.01%</span> |
| 963 | [01049 JSON_EXTRACT_SET_042](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1049_JSON_EXTRACT_SET_042.rs) | P2 | memory | GEN_SQL_JSON | 1872015 | 1529677 | <span style="color:#2563eb">49.01%</span> |
| 964 | [00558 AGG_GROUP_HAVING_051](crates/bench/sqlite_parity/cases/SQLITE_PARITY_558_AGG_GROUP_HAVING_051.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1771805 | 1529627 | <span style="color:#2563eb">49.01%</span> |
| 965 | [00916 CONSTRAINT_FK_SAVEPOINT_049](crates/bench/sqlite_parity/cases/SQLITE_PARITY_916_CONSTRAINT_FK_SAVEPOINT_049.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1759983 | 1529537 | <span style="color:#2563eb">49.02%</span> |
| 966 | [00682 JOIN_SUBQUERY_EXISTS_075](crates/bench/sqlite_parity/cases/SQLITE_PARITY_682_JOIN_SUBQUERY_EXISTS_075.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1761465 | 1529527 | <span style="color:#2563eb">49.02%</span> |
| 967 | [00452 DML_WHERE_ORDER_LIMIT_065](crates/bench/sqlite_parity/cases/SQLITE_PARITY_452_DML_WHERE_ORDER_LIMIT_065.rs) | P1 | memory | GEN_SQL_DML | 1669331 | 1529417 | <span style="color:#2563eb">49.02%</span> |
| 968 | [00727 CTE_RECURSIVE_MATRIX_020](crates/bench/sqlite_parity/cases/SQLITE_PARITY_727_CTE_RECURSIVE_MATRIX_020.rs) | P1 | memory | GEN_SQL_CTE | 1647199 | 1529316 | <span style="color:#2563eb">49.02%</span> |
| 969 | [00381 SCALAR_CAST_TYPEOF_039](crates/bench/sqlite_parity/cases/SQLITE_PARITY_381_SCALAR_CAST_TYPEOF_039.rs) | P1 | memory | GEN_SQL_SCALAR | 2072906 | 1529155 | <span style="color:#2563eb">49.03%</span> |
| 970 | [00213 SQL_WAL_CHECKPOINT_TEMPFILE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_213_SQL_WAL_CHECKPOINT_TEMPFILE.rs) | P2 | tempfile | SQL_TEMPFILE | 1898295 | 1529046 | <span style="color:#2563eb">49.03%</span> |
| 971 | [00198 OPT_LOOKASIDE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_198_OPT_LOOKASIDE.rs) | P3 | memory | CLI_OPTION | 1587796 | 1528855 | <span style="color:#2563eb">49.04%</span> |
| 972 | [00471 DML_WHERE_ORDER_LIMIT_084](crates/bench/sqlite_parity/cases/SQLITE_PARITY_471_DML_WHERE_ORDER_LIMIT_084.rs) | P1 | memory | GEN_SQL_DML | 1667518 | 1528724 | <span style="color:#2563eb">49.04%</span> |
| 973 | [00154 DOT_DBINFO_TEMPFILE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_154_DOT_DBINFO_TEMPFILE.rs) | P3 | tempfile | CLI_TEMPFILE_DIAGNOSTIC | 1789739 | 1528684 | <span style="color:#2563eb">49.04%</span> |
| 974 | [00652 JOIN_SUBQUERY_EXISTS_045](crates/bench/sqlite_parity/cases/SQLITE_PARITY_652_JOIN_SUBQUERY_EXISTS_045.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1713444 | 1528555 | <span style="color:#2563eb">49.05%</span> |
| 975 | [00406 DML_WHERE_ORDER_LIMIT_019](crates/bench/sqlite_parity/cases/SQLITE_PARITY_406_DML_WHERE_ORDER_LIMIT_019.rs) | P1 | memory | GEN_SQL_DML | 1684059 | 1527954 | <span style="color:#2563eb">49.07%</span> |
| 976 | [00490 DML_WHERE_ORDER_LIMIT_103](crates/bench/sqlite_parity/cases/SQLITE_PARITY_490_DML_WHERE_ORDER_LIMIT_103.rs) | P1 | memory | GEN_SQL_DML | 1656347 | 1527733 | <span style="color:#2563eb">49.08%</span> |
| 977 | [00054 JOINS_INNER_LEFT_CROSS_NATURAL](crates/bench/sqlite_parity/cases/SQLITE_PARITY_054_JOINS_INNER_LEFT_CROSS_NATURAL.rs) | P0 | memory | SQL_JOIN | 1779991 | 1527713 | <span style="color:#2563eb">49.08%</span> |
| 978 | [01020 JSON_EXTRACT_SET_013](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1020_JSON_EXTRACT_SET_013.rs) | P2 | memory | GEN_SQL_JSON | 1583218 | 1527543 | <span style="color:#2563eb">49.08%</span> |
| 979 | [00908 CONSTRAINT_FK_SAVEPOINT_041](crates/bench/sqlite_parity/cases/SQLITE_PARITY_908_CONSTRAINT_FK_SAVEPOINT_041.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 2092061 | 1527393 | <span style="color:#2563eb">49.09%</span> |
| 980 | [00530 AGG_GROUP_HAVING_023](crates/bench/sqlite_parity/cases/SQLITE_PARITY_530_AGG_GROUP_HAVING_023.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1753140 | 1527352 | <span style="color:#2563eb">49.09%</span> |
| 981 | [00486 DML_WHERE_ORDER_LIMIT_099](crates/bench/sqlite_parity/cases/SQLITE_PARITY_486_DML_WHERE_ORDER_LIMIT_099.rs) | P1 | memory | GEN_SQL_DML | 1626039 | 1527092 | <span style="color:#2563eb">49.10%</span> |
| 982 | [00175 OPT_QUOTE_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_175_OPT_QUOTE_MODE.rs) | P1 | memory | CLI_OPTION | 2013042 | 1527082 | <span style="color:#2563eb">49.10%</span> |
| 983 | [00436 DML_WHERE_ORDER_LIMIT_049](crates/bench/sqlite_parity/cases/SQLITE_PARITY_436_DML_WHERE_ORDER_LIMIT_049.rs) | P1 | memory | GEN_SQL_DML | 1717042 | 1527052 | <span style="color:#2563eb">49.10%</span> |
| 984 | [00275 SCALAR_NULL_COALESCE_012](crates/bench/sqlite_parity/cases/SQLITE_PARITY_275_SCALAR_NULL_COALESCE_012.rs) | P1 | memory | GEN_SQL_SCALAR | 1528584 | 1526922 | <span style="color:#2563eb">49.10%</span> |
| 985 | [00369 SCALAR_CAST_TYPEOF_036](crates/bench/sqlite_parity/cases/SQLITE_PARITY_369_SCALAR_CAST_TYPEOF_036.rs) | P1 | memory | GEN_SQL_SCALAR | 1501704 | 1526891 | <span style="color:#2563eb">49.10%</span> |
| 986 | [00876 CONSTRAINT_FK_SAVEPOINT_009](crates/bench/sqlite_parity/cases/SQLITE_PARITY_876_CONSTRAINT_FK_SAVEPOINT_009.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1942227 | 1526781 | <span style="color:#2563eb">49.11%</span> |
| 987 | [00312 SCALAR_ARITH_022](crates/bench/sqlite_parity/cases/SQLITE_PARITY_312_SCALAR_ARITH_022.rs) | P1 | memory | GEN_SQL_SCALAR | 1526130 | 1526641 | <span style="color:#2563eb">49.11%</span> |
| 988 | [00888 CONSTRAINT_FK_SAVEPOINT_021](crates/bench/sqlite_parity/cases/SQLITE_PARITY_888_CONSTRAINT_FK_SAVEPOINT_021.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1711831 | 1526521 | <span style="color:#2563eb">49.12%</span> |
| 989 | [00121 DOT_PARAMETER](crates/bench/sqlite_parity/cases/SQLITE_PARITY_121_DOT_PARAMETER.rs) | P0 | memory | CLI_DOT_COMMAND | 1751306 | 1526501 | <span style="color:#2563eb">49.12%</span> |
| 990 | [00372 SCALAR_ARITH_037](crates/bench/sqlite_parity/cases/SQLITE_PARITY_372_SCALAR_ARITH_037.rs) | P1 | memory | GEN_SQL_SCALAR | 1524888 | 1526090 | <span style="color:#2563eb">49.13%</span> |
| 991 | [00060 FILTER_CLAUSE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_060_FILTER_CLAUSE.rs) | P0 | memory | SQL_AGGREGATE | 1559102 | 1526080 | <span style="color:#2563eb">49.13%</span> |
| 992 | [00597 AGG_GROUP_HAVING_090](crates/bench/sqlite_parity/cases/SQLITE_PARITY_597_AGG_GROUP_HAVING_090.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1754722 | 1525939 | <span style="color:#2563eb">49.14%</span> |
| 993 | [00945 CONSTRAINT_FK_SAVEPOINT_078](crates/bench/sqlite_parity/cases/SQLITE_PARITY_945_CONSTRAINT_FK_SAVEPOINT_078.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1689089 | 1525850 | <span style="color:#2563eb">49.14%</span> |
| 994 | [00630 JOIN_SUBQUERY_EXISTS_023](crates/bench/sqlite_parity/cases/SQLITE_PARITY_630_JOIN_SUBQUERY_EXISTS_023.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2039792 | 1525739 | <span style="color:#2563eb">49.14%</span> |
| 995 | [00665 JOIN_SUBQUERY_EXISTS_058](crates/bench/sqlite_parity/cases/SQLITE_PARITY_665_JOIN_SUBQUERY_EXISTS_058.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1754923 | 1525639 | <span style="color:#2563eb">49.15%</span> |
| 996 | [00361 SCALAR_CAST_TYPEOF_034](crates/bench/sqlite_parity/cases/SQLITE_PARITY_361_SCALAR_CAST_TYPEOF_034.rs) | P1 | memory | GEN_SQL_SCALAR | 1552981 | 1525459 | <span style="color:#2563eb">49.15%</span> |
| 997 | [00134 DOT_CRLF](crates/bench/sqlite_parity/cases/SQLITE_PARITY_134_DOT_CRLF.rs) | P3 | memory | CLI_DOT_COMMAND_DIAGNOSTIC | 1463751 | 1525308 | <span style="color:#2563eb">49.16%</span> |
| 998 | [00357 SCALAR_CAST_TYPEOF_033](crates/bench/sqlite_parity/cases/SQLITE_PARITY_357_SCALAR_CAST_TYPEOF_033.rs) | P1 | memory | GEN_SQL_SCALAR | 1473390 | 1524917 | <span style="color:#2563eb">49.17%</span> |
| 999 | [00713 CTE_RECURSIVE_MATRIX_006](crates/bench/sqlite_parity/cases/SQLITE_PARITY_713_CTE_RECURSIVE_MATRIX_006.rs) | P1 | memory | GEN_SQL_CTE | 1595841 | 1524837 | <span style="color:#2563eb">49.17%</span> |
| 1000 | [00733 CTE_RECURSIVE_MATRIX_026](crates/bench/sqlite_parity/cases/SQLITE_PARITY_733_CTE_RECURSIVE_MATRIX_026.rs) | P1 | memory | GEN_SQL_CTE | 1559233 | 1524797 | <span style="color:#2563eb">49.17%</span> |
| 1001 | [00647 JOIN_SUBQUERY_EXISTS_040](crates/bench/sqlite_parity/cases/SQLITE_PARITY_647_JOIN_SUBQUERY_EXISTS_040.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1734074 | 1524647 | <span style="color:#2563eb">49.18%</span> |
| 1002 | [00811 WINDOW_PARTITION_SUM_024](crates/bench/sqlite_parity/cases/SQLITE_PARITY_811_WINDOW_PARTITION_SUM_024.rs) | P2 | memory | GEN_SQL_WINDOW | 1641699 | 1524397 | <span style="color:#2563eb">49.19%</span> |
| 1003 | [00755 CTE_RECURSIVE_MATRIX_048](crates/bench/sqlite_parity/cases/SQLITE_PARITY_755_CTE_RECURSIVE_MATRIX_048.rs) | P1 | memory | GEN_SQL_CTE | 1587987 | 1524166 | <span style="color:#2563eb">49.19%</span> |
| 1004 | [00520 AGG_GROUP_HAVING_013](crates/bench/sqlite_parity/cases/SQLITE_PARITY_520_AGG_GROUP_HAVING_013.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1695440 | 1523976 | <span style="color:#2563eb">49.20%</span> |
| 1005 | [00538 AGG_GROUP_HAVING_031](crates/bench/sqlite_parity/cases/SQLITE_PARITY_538_AGG_GROUP_HAVING_031.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1772367 | 1523725 | <span style="color:#2563eb">49.21%</span> |
| 1006 | [00827 WINDOW_PARTITION_SUM_040](crates/bench/sqlite_parity/cases/SQLITE_PARITY_827_WINDOW_PARTITION_SUM_040.rs) | P2 | memory | GEN_SQL_WINDOW | 1957447 | 1523666 | <span style="color:#2563eb">49.21%</span> |
| 1007 | [00050 PRAGMA_TABLE_INFO_FUNCTION](crates/bench/sqlite_parity/cases/SQLITE_PARITY_050_PRAGMA_TABLE_INFO_FUNCTION.rs) | P0 | memory | SQL_PRAGMA | 2665618 | 1523264 | <span style="color:#2563eb">49.22%</span> |
| 1008 | [00668 JOIN_SUBQUERY_EXISTS_061](crates/bench/sqlite_parity/cases/SQLITE_PARITY_668_JOIN_SUBQUERY_EXISTS_061.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1786723 | 1522984 | <span style="color:#2563eb">49.23%</span> |
| 1009 | [00131 DOT_TIMEOUT](crates/bench/sqlite_parity/cases/SQLITE_PARITY_131_DOT_TIMEOUT.rs) | P0 | memory | CLI_DOT_COMMAND | 1488860 | 1522924 | <span style="color:#2563eb">49.24%</span> |
| 1010 | [00196 OPT_MMAP](crates/bench/sqlite_parity/cases/SQLITE_PARITY_196_OPT_MMAP.rs) | P3 | memory | CLI_OPTION | 1561247 | 1522864 | <span style="color:#2563eb">49.24%</span> |
| 1011 | [00764 CTE_RECURSIVE_MATRIX_057](crates/bench/sqlite_parity/cases/SQLITE_PARITY_764_CTE_RECURSIVE_MATRIX_057.rs) | P1 | memory | GEN_SQL_CTE | 1597956 | 1522543 | <span style="color:#2563eb">49.25%</span> |
| 1012 | [00432 DML_WHERE_ORDER_LIMIT_045](crates/bench/sqlite_parity/cases/SQLITE_PARITY_432_DML_WHERE_ORDER_LIMIT_045.rs) | P1 | memory | GEN_SQL_DML | 1637941 | 1522442 | <span style="color:#2563eb">49.25%</span> |
| 1013 | [00778 CTE_RECURSIVE_MATRIX_071](crates/bench/sqlite_parity/cases/SQLITE_PARITY_778_CTE_RECURSIVE_MATRIX_071.rs) | P1 | memory | GEN_SQL_CTE | 1572348 | 1522433 | <span style="color:#2563eb">49.25%</span> |
| 1014 | [00586 AGG_GROUP_HAVING_079](crates/bench/sqlite_parity/cases/SQLITE_PARITY_586_AGG_GROUP_HAVING_079.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1901771 | 1522382 | <span style="color:#2563eb">49.25%</span> |
| 1015 | [00455 DML_WHERE_ORDER_LIMIT_068](crates/bench/sqlite_parity/cases/SQLITE_PARITY_455_DML_WHERE_ORDER_LIMIT_068.rs) | P1 | memory | GEN_SQL_DML | 1644123 | 1522153 | <span style="color:#2563eb">49.26%</span> |
| 1016 | [00467 DML_WHERE_ORDER_LIMIT_080](crates/bench/sqlite_parity/cases/SQLITE_PARITY_467_DML_WHERE_ORDER_LIMIT_080.rs) | P1 | memory | GEN_SQL_DML | 1618665 | 1522072 | <span style="color:#2563eb">49.26%</span> |
| 1017 | [00523 AGG_GROUP_HAVING_016](crates/bench/sqlite_parity/cases/SQLITE_PARITY_523_AGG_GROUP_HAVING_016.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1740807 | 1521912 | <span style="color:#2563eb">49.27%</span> |
| 1018 | [00045 REINDEX_COMMAND](crates/bench/sqlite_parity/cases/SQLITE_PARITY_045_REINDEX_COMMAND.rs) | P0 | memory | SQL_REINDEX | 1577888 | 1521802 | <span style="color:#2563eb">49.27%</span> |
| 1019 | [00376 SCALAR_ARITH_038](crates/bench/sqlite_parity/cases/SQLITE_PARITY_376_SCALAR_ARITH_038.rs) | P1 | memory | GEN_SQL_SCALAR | 1522123 | 1521582 | <span style="color:#2563eb">49.28%</span> |
| 1020 | [00768 CTE_RECURSIVE_MATRIX_061](crates/bench/sqlite_parity/cases/SQLITE_PARITY_768_CTE_RECURSIVE_MATRIX_061.rs) | P1 | memory | GEN_SQL_CTE | 1598517 | 1521531 | <span style="color:#2563eb">49.28%</span> |
| 1021 | [00071 BETWEEN_IN_ISNULL_IS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_071_BETWEEN_IN_ISNULL_IS.rs) | P0 | memory | SQL_OPERATORS | 1575965 | 1521391 | <span style="color:#2563eb">49.29%</span> |
| 1022 | [00393 DML_WHERE_ORDER_LIMIT_006](crates/bench/sqlite_parity/cases/SQLITE_PARITY_393_DML_WHERE_ORDER_LIMIT_006.rs) | P1 | memory | GEN_SQL_DML | 1992102 | 1521351 | <span style="color:#2563eb">49.29%</span> |
| 1023 | [00867 WINDOW_PARTITION_SUM_080](crates/bench/sqlite_parity/cases/SQLITE_PARITY_867_WINDOW_PARTITION_SUM_080.rs) | P2 | memory | GEN_SQL_WINDOW | 1686674 | 1521261 | <span style="color:#2563eb">49.29%</span> |
| 1024 | [00456 DML_WHERE_ORDER_LIMIT_069](crates/bench/sqlite_parity/cases/SQLITE_PARITY_456_DML_WHERE_ORDER_LIMIT_069.rs) | P1 | memory | GEN_SQL_DML | 1649333 | 1521100 | <span style="color:#2563eb">49.30%</span> |
| 1025 | [00816 WINDOW_PARTITION_SUM_029](crates/bench/sqlite_parity/cases/SQLITE_PARITY_816_WINDOW_PARTITION_SUM_029.rs) | P2 | memory | GEN_SQL_WINDOW | 1649925 | 1521100 | <span style="color:#2563eb">49.30%</span> |
| 1026 | [01076 INDEX_SCHEMA_PRAGMA_009](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1076_INDEX_SCHEMA_PRAGMA_009.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 2051435 | 1521080 | <span style="color:#2563eb">49.30%</span> |
| 1027 | [00600 AGG_GROUP_HAVING_093](crates/bench/sqlite_parity/cases/SQLITE_PARITY_600_AGG_GROUP_HAVING_093.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1711090 | 1520990 | <span style="color:#2563eb">49.30%</span> |
| 1028 | [01093 INDEX_SCHEMA_PRAGMA_026](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1093_INDEX_SCHEMA_PRAGMA_026.rs) | P2 | memory | GEN_SQL_INDEX_PRAGMA | 1757317 | 1520760 | <span style="color:#2563eb">49.31%</span> |
| 1029 | [00756 CTE_RECURSIVE_MATRIX_049](crates/bench/sqlite_parity/cases/SQLITE_PARITY_756_CTE_RECURSIVE_MATRIX_049.rs) | P1 | memory | GEN_SQL_CTE | 1532983 | 1520739 | <span style="color:#2563eb">49.31%</span> |
| 1030 | [00612 JOIN_SUBQUERY_EXISTS_005](crates/bench/sqlite_parity/cases/SQLITE_PARITY_612_JOIN_SUBQUERY_EXISTS_005.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1697524 | 1520369 | <span style="color:#2563eb">49.32%</span> |
| 1031 | [00487 DML_WHERE_ORDER_LIMIT_100](crates/bench/sqlite_parity/cases/SQLITE_PARITY_487_DML_WHERE_ORDER_LIMIT_100.rs) | P1 | memory | GEN_SQL_DML | 1630828 | 1520249 | <span style="color:#2563eb">49.33%</span> |
| 1032 | [00861 WINDOW_PARTITION_SUM_074](crates/bench/sqlite_parity/cases/SQLITE_PARITY_861_WINDOW_PARTITION_SUM_074.rs) | P2 | memory | GEN_SQL_WINDOW | 1678769 | 1519738 | <span style="color:#2563eb">49.34%</span> |
| 1033 | [00590 AGG_GROUP_HAVING_083](crates/bench/sqlite_parity/cases/SQLITE_PARITY_590_AGG_GROUP_HAVING_083.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1778658 | 1519608 | <span style="color:#2563eb">49.35%</span> |
| 1034 | [00464 DML_WHERE_ORDER_LIMIT_077](crates/bench/sqlite_parity/cases/SQLITE_PARITY_464_DML_WHERE_ORDER_LIMIT_077.rs) | P1 | memory | GEN_SQL_DML | 2016478 | 1519126 | <span style="color:#2563eb">49.36%</span> |
| 1035 | [00434 DML_WHERE_ORDER_LIMIT_047](crates/bench/sqlite_parity/cases/SQLITE_PARITY_434_DML_WHERE_ORDER_LIMIT_047.rs) | P1 | memory | GEN_SQL_DML | 1667969 | 1518987 | <span style="color:#2563eb">49.37%</span> |
| 1036 | [00726 CTE_RECURSIVE_MATRIX_019](crates/bench/sqlite_parity/cases/SQLITE_PARITY_726_CTE_RECURSIVE_MATRIX_019.rs) | P1 | memory | GEN_SQL_CTE | 1578289 | 1518876 | <span style="color:#2563eb">49.37%</span> |
| 1037 | [00201 OPT_NO_ROWID_IN_VIEW](crates/bench/sqlite_parity/cases/SQLITE_PARITY_201_OPT_NO_ROWID_IN_VIEW.rs) | P4 | memory | CLI_OPTION | 1511342 | 1518856 | <span style="color:#2563eb">49.37%</span> |
| 1038 | [00359 SCALAR_NULL_COALESCE_033](crates/bench/sqlite_parity/cases/SQLITE_PARITY_359_SCALAR_NULL_COALESCE_033.rs) | P1 | memory | GEN_SQL_SCALAR | 1738392 | 1518796 | <span style="color:#2563eb">49.37%</span> |
| 1039 | [00138 DOT_VFSNAME_LIST_INFO](crates/bench/sqlite_parity/cases/SQLITE_PARITY_138_DOT_VFSNAME_LIST_INFO.rs) | P3 | memory | CLI_DOT_COMMAND_DIAGNOSTIC | 1869119 | 1518285 | <span style="color:#2563eb">49.39%</span> |
| 1040 | [00341 SCALAR_CAST_TYPEOF_029](crates/bench/sqlite_parity/cases/SQLITE_PARITY_341_SCALAR_CAST_TYPEOF_029.rs) | P1 | memory | GEN_SQL_SCALAR | 1490582 | 1518245 | <span style="color:#2563eb">49.39%</span> |
| 1041 | [00402 DML_WHERE_ORDER_LIMIT_015](crates/bench/sqlite_parity/cases/SQLITE_PARITY_402_DML_WHERE_ORDER_LIMIT_015.rs) | P1 | memory | GEN_SQL_DML | 1673549 | 1517814 | <span style="color:#2563eb">49.41%</span> |
| 1042 | [00457 DML_WHERE_ORDER_LIMIT_070](crates/bench/sqlite_parity/cases/SQLITE_PARITY_457_DML_WHERE_ORDER_LIMIT_070.rs) | P1 | memory | GEN_SQL_DML | 1847258 | 1517544 | <span style="color:#2563eb">49.42%</span> |
| 1043 | [00556 AGG_GROUP_HAVING_049](crates/bench/sqlite_parity/cases/SQLITE_PARITY_556_AGG_GROUP_HAVING_049.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1762347 | 1517493 | <span style="color:#2563eb">49.42%</span> |
| 1044 | [00809 WINDOW_PARTITION_SUM_022](crates/bench/sqlite_parity/cases/SQLITE_PARITY_809_WINDOW_PARTITION_SUM_022.rs) | P2 | memory | GEN_SQL_WINDOW | 1613877 | 1517414 | <span style="color:#2563eb">49.42%</span> |
| 1045 | [00576 AGG_GROUP_HAVING_069](crates/bench/sqlite_parity/cases/SQLITE_PARITY_576_AGG_GROUP_HAVING_069.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1754893 | 1517063 | <span style="color:#2563eb">49.43%</span> |
| 1046 | [00568 AGG_GROUP_HAVING_061](crates/bench/sqlite_parity/cases/SQLITE_PARITY_568_AGG_GROUP_HAVING_061.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1698927 | 1516972 | <span style="color:#2563eb">49.43%</span> |
| 1047 | [00039 CREATE_TRIGGER_BEFORE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_039_CREATE_TRIGGER_BEFORE.rs) | P0 | memory | SQL_TRIGGER | 1645185 | 1516963 | <span style="color:#2563eb">49.43%</span> |
| 1048 | [00255 SCALAR_NULL_COALESCE_007](crates/bench/sqlite_parity/cases/SQLITE_PARITY_255_SCALAR_NULL_COALESCE_007.rs) | P1 | memory | GEN_SQL_SCALAR | 1540076 | 1516823 | <span style="color:#2563eb">49.44%</span> |
| 1049 | [00889 CONSTRAINT_FK_SAVEPOINT_022](crates/bench/sqlite_parity/cases/SQLITE_PARITY_889_CONSTRAINT_FK_SAVEPOINT_022.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1704157 | 1516743 | <span style="color:#2563eb">49.44%</span> |
| 1050 | [00831 WINDOW_PARTITION_SUM_044](crates/bench/sqlite_parity/cases/SQLITE_PARITY_831_WINDOW_PARTITION_SUM_044.rs) | P2 | memory | GEN_SQL_WINDOW | 1610499 | 1516732 | <span style="color:#2563eb">49.44%</span> |
| 1051 | [00907 CONSTRAINT_FK_SAVEPOINT_040](crates/bench/sqlite_parity/cases/SQLITE_PARITY_907_CONSTRAINT_FK_SAVEPOINT_040.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1688197 | 1515740 | <span style="color:#2563eb">49.48%</span> |
| 1052 | [00127 DOT_LIMIT](crates/bench/sqlite_parity/cases/SQLITE_PARITY_127_DOT_LIMIT.rs) | P0 | memory | CLI_DOT_COMMAND | 1525689 | 1515359 | <span style="color:#2563eb">49.49%</span> |
| 1053 | [00890 CONSTRAINT_FK_SAVEPOINT_023](crates/bench/sqlite_parity/cases/SQLITE_PARITY_890_CONSTRAINT_FK_SAVEPOINT_023.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1676004 | 1515219 | <span style="color:#2563eb">49.49%</span> |
| 1054 | [00470 DML_WHERE_ORDER_LIMIT_083](crates/bench/sqlite_parity/cases/SQLITE_PARITY_470_DML_WHERE_ORDER_LIMIT_083.rs) | P1 | memory | GEN_SQL_DML | 1972916 | 1514969 | <span style="color:#2563eb">49.50%</span> |
| 1055 | [00669 JOIN_SUBQUERY_EXISTS_062](crates/bench/sqlite_parity/cases/SQLITE_PARITY_669_JOIN_SUBQUERY_EXISTS_062.rs) | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 2054120 | 1514478 | <span style="color:#2563eb">49.52%</span> |
| 1056 | [00252 SCALAR_ARITH_007](crates/bench/sqlite_parity/cases/SQLITE_PARITY_252_SCALAR_ARITH_007.rs) | P1 | memory | GEN_SQL_SCALAR | 1514478 | 1514438 | <span style="color:#2563eb">49.52%</span> |
| 1057 | [00887 CONSTRAINT_FK_SAVEPOINT_020](crates/bench/sqlite_parity/cases/SQLITE_PARITY_887_CONSTRAINT_FK_SAVEPOINT_020.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1681284 | 1514368 | <span style="color:#2563eb">49.52%</span> |
| 1058 | [00924 CONSTRAINT_FK_SAVEPOINT_057](crates/bench/sqlite_parity/cases/SQLITE_PARITY_924_CONSTRAINT_FK_SAVEPOINT_057.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1753681 | 1513786 | <span style="color:#2563eb">49.54%</span> |
| 1059 | [00119 DOT_EQP](crates/bench/sqlite_parity/cases/SQLITE_PARITY_119_DOT_EQP.rs) | P0 | memory | CLI_DOT_COMMAND | 1571646 | 1513777 | <span style="color:#2563eb">49.54%</span> |
| 1060 | [00510 AGG_GROUP_HAVING_003](crates/bench/sqlite_parity/cases/SQLITE_PARITY_510_AGG_GROUP_HAVING_003.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1701042 | 1513677 | <span style="color:#2563eb">49.54%</span> |
| 1061 | [00766 CTE_RECURSIVE_MATRIX_059](crates/bench/sqlite_parity/cases/SQLITE_PARITY_766_CTE_RECURSIVE_MATRIX_059.rs) | P1 | memory | GEN_SQL_CTE | 1599339 | 1513245 | <span style="color:#2563eb">49.56%</span> |
| 1062 | [00494 DML_WHERE_ORDER_LIMIT_107](crates/bench/sqlite_parity/cases/SQLITE_PARITY_494_DML_WHERE_ORDER_LIMIT_107.rs) | P1 | memory | GEN_SQL_DML | 1718624 | 1513165 | <span style="color:#2563eb">49.56%</span> |
| 1063 | [00087 DATE_TIMEDIFF_FUNCTION](crates/bench/sqlite_parity/cases/SQLITE_PARITY_087_DATE_TIMEDIFF_FUNCTION.rs) | P0 | memory | SQL_FUNCTIONS | 1544425 | 1512945 | <span style="color:#2563eb">49.57%</span> |
| 1064 | [00092 PERCENTILE_FUNCTIONS_OPTIONAL](crates/bench/sqlite_parity/cases/SQLITE_PARITY_092_PERCENTILE_FUNCTIONS_OPTIONAL.rs) | P3 | memory | SQL_FUNCTIONS_OPTIONAL | 1434517 | 1512825 | <span style="color:#2563eb">49.57%</span> |
| 1065 | [00313 SCALAR_CAST_TYPEOF_022](crates/bench/sqlite_parity/cases/SQLITE_PARITY_313_SCALAR_CAST_TYPEOF_022.rs) | P1 | memory | GEN_SQL_SCALAR | 1522754 | 1512384 | <span style="color:#2563eb">49.59%</span> |
| 1066 | [00232 SCALAR_ARITH_002](crates/bench/sqlite_parity/cases/SQLITE_PARITY_232_SCALAR_ARITH_002.rs) | P1 | memory | GEN_SQL_SCALAR | 1532011 | 1512314 | <span style="color:#2563eb">49.59%</span> |
| 1067 | [00194 OPT_IFEXISTS_NEGATIVE_TEMPFILE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_194_OPT_IFEXISTS_NEGATIVE_TEMPFILE.rs) | P3 | tempfile | CLI_OPTION_TEMPFILE_DIAGNOSTIC | 1314830 | 1512304 | <span style="color:#2563eb">49.59%</span> |
| 1068 | [00029 ALTER_TABLE_RENAME_COLUMN](crates/bench/sqlite_parity/cases/SQLITE_PARITY_029_ALTER_TABLE_RENAME_COLUMN.rs) | P0 | memory | SQL_ALTER | 2280869 | 1512143 | <span style="color:#2563eb">49.60%</span> |
| 1069 | [00145 DOT_SCANSTATS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_145_DOT_SCANSTATS.rs) | P3 | memory | CLI_DOT_COMMAND_DIAGNOSTIC | 1810518 | 1511182 | <span style="color:#2563eb">49.63%</span> |
| 1070 | [00570 AGG_GROUP_HAVING_063](crates/bench/sqlite_parity/cases/SQLITE_PARITY_570_AGG_GROUP_HAVING_063.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1969520 | 1510961 | <span style="color:#2563eb">49.63%</span> |
| 1071 | [00526 AGG_GROUP_HAVING_019](crates/bench/sqlite_parity/cases/SQLITE_PARITY_526_AGG_GROUP_HAVING_019.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1716120 | 1510701 | <span style="color:#2563eb">49.64%</span> |
| 1072 | [00132 DOT_TRACE_STDOUT](crates/bench/sqlite_parity/cases/SQLITE_PARITY_132_DOT_TRACE_STDOUT.rs) | P2 | memory | CLI_DOT_COMMAND_DIAGNOSTIC | 1553832 | 1510160 | <span style="color:#2563eb">49.66%</span> |
| 1073 | [00356 SCALAR_ARITH_033](crates/bench/sqlite_parity/cases/SQLITE_PARITY_356_SCALAR_ARITH_033.rs) | P1 | memory | GEN_SQL_SCALAR | 1614307 | 1510050 | <span style="color:#2563eb">49.66%</span> |
| 1074 | [00825 WINDOW_PARTITION_SUM_038](crates/bench/sqlite_parity/cases/SQLITE_PARITY_825_WINDOW_PARTITION_SUM_038.rs) | P2 | memory | GEN_SQL_WINDOW | 1697104 | 1509809 | <span style="color:#2563eb">49.67%</span> |
| 1075 | [00219 UPDATE_LIMIT_OPTIONAL](crates/bench/sqlite_parity/cases/SQLITE_PARITY_219_UPDATE_LIMIT_OPTIONAL.rs) | P3 | memory | SQL_UPDATE_OPTIONAL | 2358016 | 1509248 | <span style="color:#2563eb">49.69%</span> |
| 1076 | [00179 OPT_MARKDOWN_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_179_OPT_MARKDOWN_MODE.rs) | P2 | memory | CLI_OPTION | 1505601 | 1509088 | <span style="color:#2563eb">49.70%</span> |
| 1077 | [00826 WINDOW_PARTITION_SUM_039](crates/bench/sqlite_parity/cases/SQLITE_PARITY_826_WINDOW_PARTITION_SUM_039.rs) | P2 | memory | GEN_SQL_WINDOW | 1664712 | 1508576 | <span style="color:#2563eb">49.71%</span> |
| 1078 | [00095 CREATE_VIRTUAL_TABLE_RTREE_OPTIONAL](crates/bench/sqlite_parity/cases/SQLITE_PARITY_095_CREATE_VIRTUAL_TABLE_RTREE_OPTIONAL.rs) | P2 | memory | SQL_VIRTUAL_TABLE_OPTIONAL | 1742620 | 1507915 | <span style="color:#2563eb">49.74%</span> |
| 1079 | [00910 CONSTRAINT_FK_SAVEPOINT_043](crates/bench/sqlite_parity/cases/SQLITE_PARITY_910_CONSTRAINT_FK_SAVEPOINT_043.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1734405 | 1507224 | <span style="color:#2563eb">49.76%</span> |
| 1080 | [00842 WINDOW_PARTITION_SUM_055](crates/bench/sqlite_parity/cases/SQLITE_PARITY_842_WINDOW_PARTITION_SUM_055.rs) | P2 | memory | GEN_SQL_WINDOW | 1986151 | 1507063 | <span style="color:#2563eb">49.76%</span> |
| 1081 | [00508 AGG_GROUP_HAVING_001](crates/bench/sqlite_parity/cases/SQLITE_PARITY_508_AGG_GROUP_HAVING_001.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1722642 | 1507044 | <span style="color:#2563eb">49.77%</span> |
| 1082 | [00174 OPT_CSV_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_174_OPT_CSV_MODE.rs) | P1 | memory | CLI_OPTION | 1512965 | 1506543 | <span style="color:#2563eb">49.78%</span> |
| 1083 | [00259 SCALAR_NULL_COALESCE_008](crates/bench/sqlite_parity/cases/SQLITE_PARITY_259_SCALAR_NULL_COALESCE_008.rs) | P1 | memory | GEN_SQL_SCALAR | 1458692 | 1506533 | <span style="color:#2563eb">49.78%</span> |
| 1084 | [00347 SCALAR_NULL_COALESCE_030](crates/bench/sqlite_parity/cases/SQLITE_PARITY_347_SCALAR_NULL_COALESCE_030.rs) | P1 | memory | GEN_SQL_SCALAR | 1584139 | 1506022 | <span style="color:#2563eb">49.80%</span> |
| 1085 | [00349 SCALAR_CAST_TYPEOF_031](crates/bench/sqlite_parity/cases/SQLITE_PARITY_349_SCALAR_CAST_TYPEOF_031.rs) | P1 | memory | GEN_SQL_SCALAR | 2021528 | 1505140 | <span style="color:#2563eb">49.83%</span> |
| 1086 | [00566 AGG_GROUP_HAVING_059](crates/bench/sqlite_parity/cases/SQLITE_PARITY_566_AGG_GROUP_HAVING_059.rs) | P1 | memory | GEN_SQL_AGGREGATE | 1702223 | 1504899 | <span style="color:#2563eb">49.84%</span> |
| 1087 | [00923 CONSTRAINT_FK_SAVEPOINT_056](crates/bench/sqlite_parity/cases/SQLITE_PARITY_923_CONSTRAINT_FK_SAVEPOINT_056.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1687045 | 1504749 | <span style="color:#2563eb">49.84%</span> |
| 1088 | [00994 VIEW_TRIGGER_GENERATED_047](crates/bench/sqlite_parity/cases/SQLITE_PARITY_994_VIEW_TRIGGER_GENERATED_047.rs) | P2 | memory | GEN_SQL_VIEW_TRIGGER | 2547885 | 1504228 | <span style="color:#2563eb">49.86%</span> |
| 1089 | [00729 CTE_RECURSIVE_MATRIX_022](crates/bench/sqlite_parity/cases/SQLITE_PARITY_729_CTE_RECURSIVE_MATRIX_022.rs) | P1 | memory | GEN_SQL_CTE | 1580583 | 1503286 | <span style="color:#2563eb">49.89%</span> |
| 1090 | [00133 DOT_AUTH](crates/bench/sqlite_parity/cases/SQLITE_PARITY_133_DOT_AUTH.rs) | P3 | memory | CLI_DOT_COMMAND_DIAGNOSTIC | 1548512 | 1503196 | <span style="color:#2563eb">49.89%</span> |
| 1091 | [00736 CTE_RECURSIVE_MATRIX_029](crates/bench/sqlite_parity/cases/SQLITE_PARITY_736_CTE_RECURSIVE_MATRIX_029.rs) | P1 | memory | GEN_SQL_CTE | 1603957 | 1503186 | <span style="color:#2563eb">49.89%</span> |
| 1092 | [00064 CTE_NON_RECURSIVE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_064_CTE_NON_RECURSIVE.rs) | P0 | memory | SQL_CTE | 1522964 | 1502805 | <span style="color:#2563eb">49.91%</span> |
| 1093 | [00929 CONSTRAINT_FK_SAVEPOINT_062](crates/bench/sqlite_parity/cases/SQLITE_PARITY_929_CONSTRAINT_FK_SAVEPOINT_062.rs) | P2 | memory | GEN_SQL_CONSTRAINT_TX | 1706792 | 1500902 | <span style="color:#2563eb">49.97%</span> |
| 1094 | [00243 SCALAR_NULL_COALESCE_004](crates/bench/sqlite_parity/cases/SQLITE_PARITY_243_SCALAR_NULL_COALESCE_004.rs) | P1 | memory | GEN_SQL_SCALAR | 1498458 | 1500722 | <span style="color:#2563eb">49.98%</span> |
| 1095 | [00084 CORE_FORMAT_QUOTE_HEX](crates/bench/sqlite_parity/cases/SQLITE_PARITY_084_CORE_FORMAT_QUOTE_HEX.rs) | P0 | memory | SQL_FUNCTIONS | 1508296 | 1500662 | <span style="color:#2563eb">49.98%</span> |
| 1096 | [01021 JSON_EXTRACT_SET_014](crates/bench/sqlite_parity/cases/SQLITE_PARITY_1021_JSON_EXTRACT_SET_014.rs) | P2 | memory | GEN_SQL_JSON | 1572858 | 1499509 | <span style="color:#2563eb">50.02%</span> |
| 1097 | [00841 WINDOW_PARTITION_SUM_054](crates/bench/sqlite_parity/cases/SQLITE_PARITY_841_WINDOW_PARTITION_SUM_054.rs) | P2 | memory | GEN_SQL_WINDOW | 2052206 | 1498447 | <span style="color:#2563eb">50.05%</span> |
| 1098 | [00220 DELETE_LIMIT_OPTIONAL](crates/bench/sqlite_parity/cases/SQLITE_PARITY_220_DELETE_LIMIT_OPTIONAL.rs) | P3 | memory | SQL_DELETE_OPTIONAL | 1574562 | 1498297 | <span style="color:#2563eb">50.06%</span> |
| 1099 | [00326 SCALAR_STRING_025](crates/bench/sqlite_parity/cases/SQLITE_PARITY_326_SCALAR_STRING_025.rs) | P1 | memory | GEN_SQL_SCALAR | 1498537 | 1498067 | <span style="color:#2563eb">50.06%</span> |
| 1100 | [00126 DOT_STATS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_126_DOT_STATS.rs) | P3 | memory | CLI_DOT_COMMAND_DIAGNOSTIC | 1759723 | 1496714 | <span style="color:#2563eb">50.11%</span> |
| 1101 | [00786 CTE_RECURSIVE_MATRIX_079](crates/bench/sqlite_parity/cases/SQLITE_PARITY_786_CTE_RECURSIVE_MATRIX_079.rs) | P1 | memory | GEN_SQL_CTE | 1624907 | 1496524 | <span style="color:#2563eb">50.12%</span> |
| 1102 | [00190 OPT_BAIL](crates/bench/sqlite_parity/cases/SQLITE_PARITY_190_OPT_BAIL.rs) | P1 | memory | CLI_OPTION_NEGATIVE | 1567208 | 1496303 | <span style="color:#2563eb">50.12%</span> |
| 1103 | [00473 DML_WHERE_ORDER_LIMIT_086](crates/bench/sqlite_parity/cases/SQLITE_PARITY_473_DML_WHERE_ORDER_LIMIT_086.rs) | P1 | memory | GEN_SQL_DML | 1639444 | 1494420 | <span style="color:#2563eb">50.19%</span> |
| 1104 | [00711 CTE_RECURSIVE_MATRIX_004](crates/bench/sqlite_parity/cases/SQLITE_PARITY_711_CTE_RECURSIVE_MATRIX_004.rs) | P1 | memory | GEN_SQL_CTE | 1587626 | 1493889 | <span style="color:#2563eb">50.20%</span> |
| 1105 | [00856 WINDOW_PARTITION_SUM_069](crates/bench/sqlite_parity/cases/SQLITE_PARITY_856_WINDOW_PARTITION_SUM_069.rs) | P2 | memory | GEN_SQL_WINDOW | 1643693 | 1490613 | <span style="color:#2563eb">50.31%</span> |
| 1106 | [00142 DOT_EXIT_CODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_142_DOT_EXIT_CODE.rs) | P0 | memory | CLI_DOT_COMMAND | 1767497 | 1489321 | <span style="color:#2563eb">50.36%</span> |
| 1107 | [00076 EXPLAIN_BYTECODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_076_EXPLAIN_BYTECODE.rs) | P0 | memory | SQL_EXPLAIN | 1478680 | 1486805 | <span style="color:#2563eb">50.44%</span> |
| 1108 | [00225 OPT_NONCE_SAFE_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_225_OPT_NONCE_SAFE_MODE.rs) | P2 | memory | CLI_OPTION | 1915577 | 1482567 | <span style="color:#2563eb">50.58%</span> |
| 1109 | [00116 DOT_DATABASES](crates/bench/sqlite_parity/cases/SQLITE_PARITY_116_DOT_DATABASES.rs) | P0 | memory | CLI_DOT_COMMAND | 2535081 | 1482377 | <span style="color:#2563eb">50.59%</span> |
| 1110 | [00141 DOT_SHA3SUM](crates/bench/sqlite_parity/cases/SQLITE_PARITY_141_DOT_SHA3SUM.rs) | P0 | memory | CLI_DOT_COMMAND | 1703986 | 1480634 | <span style="color:#2563eb">50.65%</span> |
| 1111 | [00118 DOT_FULLSCHEMA](crates/bench/sqlite_parity/cases/SQLITE_PARITY_118_DOT_FULLSCHEMA.rs) | P0 | memory | CLI_DOT_COMMAND | 1637591 | 1480453 | <span style="color:#2563eb">50.65%</span> |
| 1112 | [00222 OPT_ESCAPE_SYMBOL](crates/bench/sqlite_parity/cases/SQLITE_PARITY_222_OPT_ESCAPE_SYMBOL.rs) | P3 | memory | CLI_OPTION | 1569331 | 1479372 | <span style="color:#2563eb">50.69%</span> |
| 1113 | [00123 DOT_ECHO](crates/bench/sqlite_parity/cases/SQLITE_PARITY_123_DOT_ECHO.rs) | P0 | memory | CLI_DOT_COMMAND | 1818484 | 1474362 | <span style="color:#2563eb">50.85%</span> |
| 1114 | [00125 DOT_TIMER](crates/bench/sqlite_parity/cases/SQLITE_PARITY_125_DOT_TIMER.rs) | P3 | memory | CLI_DOT_COMMAND_DIAGNOSTIC | 1599859 | 1473260 | <span style="color:#2563eb">50.89%</span> |
| 1115 | [00170 OPT_VERSION](crates/bench/sqlite_parity/cases/SQLITE_PARITY_170_OPT_VERSION.rs) | P1 | memory | CLI_OPTION | 2389244 | 1472118 | <span style="color:#2563eb">50.93%</span> |
| 1116 | [00159 DOT_SYSTEM_SIDE_EFFECT](crates/bench/sqlite_parity/cases/SQLITE_PARITY_159_DOT_SYSTEM_SIDE_EFFECT.rs) | P4 | side_effect | CLI_SIDE_EFFECT | 2479836 | 1464474 | <span style="color:#2563eb">51.18%</span> |
| 1117 | [00224 OPT_STATS](crates/bench/sqlite_parity/cases/SQLITE_PARITY_224_OPT_STATS.rs) | P3 | memory | CLI_OPTION_DIAGNOSTIC | 1523295 | 1461037 | <span style="color:#2563eb">51.30%</span> |
| 1118 | [00075 EXPLAIN_QUERY_PLAN](crates/bench/sqlite_parity/cases/SQLITE_PARITY_075_EXPLAIN_QUERY_PLAN.rs) | P0 | memory | SQL_EXPLAIN | 1599188 | 1460145 | <span style="color:#2563eb">51.33%</span> |
| 1119 | [00182 OPT_COLUMN_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_182_OPT_COLUMN_MODE.rs) | P2 | memory | CLI_OPTION | 1496885 | 1459554 | <span style="color:#2563eb">51.35%</span> |
| 1120 | [00156 DOT_RECOVER_TEMPFILE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_156_DOT_RECOVER_TEMPFILE.rs) | P3 | tempfile | CLI_TEMPFILE | 3062008 | 1486235 | <span style="color:#2563eb">51.46%</span> |
| 1121 | [00117 DOT_DUMP](crates/bench/sqlite_parity/cases/SQLITE_PARITY_117_DOT_DUMP.rs) | P0 | memory | CLI_DOT_COMMAND | 2547474 | 1453022 | <span style="color:#2563eb">51.57%</span> |
| 1122 | [00115 DOT_SCHEMA_TABLES_INDEXES](crates/bench/sqlite_parity/cases/SQLITE_PARITY_115_DOT_SCHEMA_TABLES_INDEXES.rs) | P0 | memory | CLI_DOT_COMMAND | 3139315 | 1517353 | <span style="color:#2563eb">51.67%</span> |
| 1123 | [00203 OPT_ARCHIVE_A_TEMPFILE_OPTIONAL](crates/bench/sqlite_parity/cases/SQLITE_PARITY_203_OPT_ARCHIVE_A_TEMPFILE_OPTIONAL.rs) | P4 | tempfile | CLI_OPTION_TEMPFILE_OPTIONAL | 1775893 | 1449434 | <span style="color:#2563eb">51.69%</span> |
| 1124 | [00181 OPT_TABLE_MODE](crates/bench/sqlite_parity/cases/SQLITE_PARITY_181_OPT_TABLE_MODE.rs) | P2 | memory | CLI_OPTION | 1511732 | 1434536 | <span style="color:#2563eb">52.18%</span> |
| 1125 | [00160 DOT_EXCEL_EXTERNAL_APP](crates/bench/sqlite_parity/cases/SQLITE_PARITY_160_DOT_EXCEL_EXTERNAL_APP.rs) | P4 | external_app | CLI_EXTERNAL_APP | 33527178 | 1745786 | <span style="color:#2563eb">94.79%</span> |
| 1126 | [00161 DOT_WWW_EXTERNAL_APP](crates/bench/sqlite_parity/cases/SQLITE_PARITY_161_DOT_WWW_EXTERNAL_APP.rs) | P4 | external_app | CLI_EXTERNAL_APP | 35129783 | 1732971 | <span style="color:#2563eb">95.07%</span> |
| 1127 | [00209 OPT_INTERACTIVE_CATALOG](crates/bench/sqlite_parity/cases/SQLITE_PARITY_209_OPT_INTERACTIVE_CATALOG.rs) | P4 | catalog | CLI_OPTION_CATALOG | 52261624 | 1838852 | <span style="color:#2563eb">96.48%</span> |

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
