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
  <img src="https://img.shields.io/badge/version-2.0.6-blue" alt="version">
  <!-- jankurai-score-badge:begin -->
  <a href=".jankurai/repo-score.md"><img src="https://img.shields.io/badge/jankurai-85%2F100%20advisory-green" alt="jankurai score: 85/100 advisory"></a>
  <!-- jankurai-score-badge:end -->
</p>

RedlineDB is an embedded SQL engine written in Rust. It keeps the SQLite-facing
API familiar while replacing the storage core with MVCC, a concurrent B-tree,
group-commit WAL, and crash recovery designed for multi-writer workloads.

## Engine Metrics

<!-- sqlite-parity-metrics:begin -->
![SQLite parity score](assets/sqlite-jankurai-score.svg)

![SQLite parity code shape](assets/sqlite-code-shape.svg)

![SQLite parity median test performance](assets/sqlite-median-test-performance.svg)

![SQLite parity KSLOC](assets/sqlite-parity-ksloc.svg)

![SQLite parity Jankurai comparison](assets/sqlite-jankurai-comparison.svg)

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
| SQLite parity gate | `just redline-testing-official` |

## Install

### Rust library

Pin the release in `Cargo.toml`:

```toml
[dependencies]
  redlinedb = "=2.0.6"
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
curl -LsSf https://raw.githubusercontent.com/neverhuman/RedlineDB/main/scripts/install.sh | VERSION=v2.0.6 bash
```

Lock the exact tarball digest in CI or release automation:

```bash
curl -LsSf https://raw.githubusercontent.com/neverhuman/RedlineDB/main/scripts/install.sh | \
  VERSION=v2.0.6 REDLINEDB_SHA256=<sha256> bash
```

### Build from source

```bash
cargo install redlinedb-cli --version 2.0.6 --locked
```

Or install from the tagged repository release:

```bash
cargo install --git https://github.com/neverhuman/RedlineDB.git --tag v2.0.6 --package redlinedb-cli --locked
```

### Direct download

Release tarballs are published on the [releases page](https://github.com/neverhuman/RedlineDB/releases):

| Platform | File |
|---|---|
| Linux x86_64 | `redlinedb-v2.0.6-linux-x86_64.tar.gz` |
| macOS Apple Silicon | `redlinedb-v2.0.6-macos-arm64.tar.gz` |
| macOS Intel | `redlinedb-v2.0.6-macos-x86_64.tar.gz` |

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
rtk just redline-testing-official
rtk just sqlite-parity-report-check
```

## SQLite Parity Status

The official parity lane is sourced only from the verified external
`neverhuman/redline-testing` release artifact, which is the sole official
evidence source. Missing, skipped, failed, or unmeasured cases are hard
report-check failures rather than excluded from the denominator. The live report
below is generated only from `benchmark-results/sqlite-parity/latest/` after
`official-evidence.processed.json` validates `raw.jsonl` against the
hash-bound upstream official evidence chain, including the verified release
tarball SHA-256, the `redline-testing` binary SHA-256, the release manifest, and
the GitHub artifact attestation.

<!-- sqlite-parity-report:begin -->
**SQLite parity coverage:** **1127 / 1127** cases passed in CI. Failed: **0**. Skipped: **0**. Updated 2026-05-24.

**SQLite parity latency:** median gap **-27.25%**, worst gap **-298.83%**, faster cases **218**.

**Benchmark metadata:** RedlineDB target version **redlinedb v2.0.6 (SQLite 3.45.1 compatibility)**, SQLite reference version **3.53.1 2026-05-05 10:34:17 c88b22011a54b4f6fbd149e9f8e4de77658ce58143a1af0e3785e4e6475127e9 (64-bit)**, redline-testing runner version **redline-testing 0.1.3**.

![SQLite parity latency improvement plot](assets/sqlite-parity-latency-gap.svg)

![SQLite parity performance distribution](assets/sqlite-parity-performance-histogram.svg)

<details id="sqlite-parity-ranked-table">
<summary>Full ranked latency table</summary>

| Rank | Case | Priority | Profile | Category | SQLite median ns | RedlineDB median ns | Improvement |
| ---: | --- | --- | --- | --- | ---: | ---: | ---: |
| 1 | AGG_GROUP_HAVING_075 | P1 | memory | GEN_SQL_AGGREGATE | 2893743 | 11964819 | -298.83% |
| 2 | SCALAR_ARITH_034 | P1 | memory | GEN_SQL_SCALAR | 4292058 | 14880964 | -246.71% |
| 3 | AGG_GROUP_HAVING_100 | P1 | memory | GEN_SQL_AGGREGATE | 4829333 | 16500857 | -241.68% |
| 4 | JOIN_SUBQUERY_EXISTS_092 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3097839 | 10559762 | -240.88% |
| 5 | JOIN_SUBQUERY_EXISTS_036 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3951393 | 13414040 | -239.48% |
| 6 | CTE_RECURSIVE_MATRIX_042 | P1 | memory | GEN_SQL_CTE | 5402006 | 16495247 | -205.35% |
| 7 | AGG_GROUP_HAVING_004 | P1 | memory | GEN_SQL_AGGREGATE | 6085249 | 17993200 | -195.69% |
| 8 | JSON_EXTRACT_SET_049 | P2 | memory | GEN_SQL_JSON | 4465475 | 13002512 | -191.18% |
| 9 | WINDOW_PARTITION_SUM_055 | P2 | memory | GEN_SQL_WINDOW | 5264737 | 15296410 | -190.54% |
| 10 | SCALAR_ARITH_023 | P1 | memory | GEN_SQL_SCALAR | 4940874 | 14290948 | -189.24% |
| 11 | WINDOW_PARTITION_SUM_062 | P2 | memory | GEN_SQL_WINDOW | 3977993 | 11386105 | -186.23% |
| 12 | CONSTRAINT_FK_SAVEPOINT_018 | P2 | memory | GEN_SQL_CONSTRAINT_TX | 5475145 | 15548246 | -183.98% |
| 13 | INDEX_SCHEMA_PRAGMA_041 | P2 | memory | GEN_SQL_INDEX_PRAGMA | 3690400 | 10404799 | -181.94% |
| 14 | DML_WHERE_ORDER_LIMIT_046 | P1 | memory | GEN_SQL_DML | 4740767 | 13331694 | -181.21% |
| 15 | AGG_GROUP_HAVING_098 | P1 | memory | GEN_SQL_AGGREGATE | 5168836 | 14415594 | -178.89% |
| 16 | SCALAR_NULL_COALESCE_009 | P1 | memory | GEN_SQL_SCALAR | 4521842 | 12384363 | -173.88% |
| 17 | CONSTRAINT_FK_SAVEPOINT_031 | P2 | memory | GEN_SQL_CONSTRAINT_TX | 4667608 | 12559394 | -169.08% |
| 18 | OPT_BAIL | P1 | memory | CLI_OPTION_NEGATIVE | 4929503 | 13031136 | -164.35% |
| 19 | JOIN_SUBQUERY_EXISTS_018 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 3790569 | 9961701 | -162.80% |
| 20 | DML_WHERE_ORDER_LIMIT_076 | P1 | memory | GEN_SQL_DML | 7376070 | 19272280 | -161.28% |
| 21 | CTE_RECURSIVE_MATRIX_063 | P1 | memory | GEN_SQL_CTE | 6164408 | 15804491 | -156.38% |
| 22 | VIEW_TRIGGER_GENERATED_001 | P2 | memory | GEN_SQL_VIEW_TRIGGER | 5521382 | 14124714 | -155.82% |
| 23 | SCALAR_CAST_TYPEOF_001 | P1 | memory | GEN_SQL_SCALAR | 5411114 | 13840657 | -155.78% |
| 24 | ANALYZE_SQLITE_STAT1 | P0 | memory | SQL_ANALYZE | 4867986 | 12420300 | -155.14% |
| 25 | WINDOW_PARTITION_SUM_036 | P2 | memory | GEN_SQL_WINDOW | 6237857 | 15901424 | -154.92% |

</details>
<!-- sqlite-parity-report:end -->

## Jankurai Breakdown

<!-- sqlite-jankurai-breakdown:begin -->

{
  "generated_by": "redline-testing jankurai-compare",
  "redlinedb_score": 64,
  "redlinedb_status": "unknown",
  "score_delta": 44,
  "sqlite_ref": "version-3.53.1",
  "sqlite_score": 20,
  "sqlite_status": "unknown",
  "updated_date": "2026-05-24"
}
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
- `crates/bench` keeps engine-local tests and non-official harness code only.
- The official conformance corpus, memory suite, beyond-SQLite coverage, benchmark gate, and report authority live in `neverhuman/redline-testing`.

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
| `crates/bench/` | Engine-local tests and non-official harness code; not a parity evidence producer |
| `benchmark-results/sqlite-parity/latest/` | Current external-suite report artifacts and processed official evidence |
| `docs/` | Architecture, testing, and audit guidance |
| `paper/` | Evaluation writeup and reproducibility assets |

## Development Notes

- `just fast` is the default local proof lane for ordinary edits.
- `just redline-testing-official` runs the verified external official suite wrapper.
- `just official-evidence-guard` fails if official metrics can be regenerated without the verified external runner.
- `just sqlite-parity-report-update` refreshes the generated parity report from the latest processed official evidence bundle.
- `just sqlite-parity-report-check` verifies the README report block matches the committed processed official evidence bundle.
- `just sqlite-parity-report-publish-pr` is the CI entrypoint that regenerates the report and opens or updates the draft report PR after main CI succeeds.
- `scripts/ci-local.sh all` mirrors the broader local CI surface when you need it.

The repository does not expose local SQLite parity coverage, benchmark, report,
or sentinel producers. The sole source for SQLite, memory, beyond-SQLite,
latency, chart, README, Jankurai, and score evidence is the verified
`neverhuman/redline-testing` release artifact and its processed evidence
bundle.

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
