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

**SQLite parity latency:** median gap **-19.69%**, worst gap **-289.42%**, faster cases **306**.

**Benchmark metadata:** RedlineDB target version **redlinedb v2.0.6 (SQLite 3.45.1 compatibility)**, SQLite reference version **3.53.1 2026-05-05 10:34:17 c88b22011a54b4f6fbd149e9f8e4de77658ce58143a1af0e3785e4e6475127e9 (64-bit)**, redline-testing runner version **redline-testing 0.1.3**.

![SQLite parity latency improvement plot](assets/sqlite-parity-latency-gap.svg)

![SQLite parity performance distribution](assets/sqlite-parity-performance-histogram.svg)

<details id="sqlite-parity-ranked-table">
<summary>Full ranked latency table</summary>

| Rank | Case | Priority | Profile | Category | SQLite median ns | RedlineDB median ns | Improvement |
| ---: | --- | --- | --- | --- | ---: | ---: | ---: |
| 1 | AGG_GROUP_HAVING_059 | P1 | memory | GEN_SQL_AGGREGATE | 3866083 | 15055127 | -289.42% |
| 2 | JSON_EXTRACT_SET_051 | P2 | memory | GEN_SQL_JSON | 3070568 | 11762459 | -283.07% |
| 3 | DML_WHERE_ORDER_LIMIT_106 | P1 | memory | GEN_SQL_DML | 4160450 | 15849029 | -280.95% |
| 4 | INDEX_SCHEMA_PRAGMA_018 | P2 | memory | GEN_SQL_INDEX_PRAGMA | 3366709 | 12364889 | -267.27% |
| 5 | INDEX_SCHEMA_PRAGMA_005 | P2 | memory | GEN_SQL_INDEX_PRAGMA | 4157935 | 14334645 | -244.75% |
| 6 | OPT_NOUNICODE_UTF8_CATALOG | P4 | catalog | CLI_OPTION_CATALOG | 3217786 | 10031936 | -211.77% |
| 7 | CTE_RECURSIVE_MATRIX_079 | P1 | memory | GEN_SQL_CTE | 5319732 | 16466738 | -209.54% |
| 8 | WINDOW_PARTITION_SUM_047 | P2 | memory | GEN_SQL_WINDOW | 4547792 | 13640181 | -199.93% |
| 9 | CONSTRAINT_FK_SAVEPOINT_074 | P2 | memory | GEN_SQL_CONSTRAINT_TX | 4652330 | 13460000 | -189.32% |
| 10 | SCALAR_ARITH_038 | P1 | memory | GEN_SQL_SCALAR | 4108661 | 11881625 | -189.18% |
| 11 | CTE_RECURSIVE_MATRIX_006 | P1 | memory | GEN_SQL_CTE | 4272461 | 11969702 | -180.16% |
| 12 | CONSTRAINT_FK_SAVEPOINT_052 | P2 | memory | GEN_SQL_CONSTRAINT_TX | 5459828 | 14870167 | -172.36% |
| 13 | OPT_PCACHETRACE_CATALOG | P4 | catalog | CLI_OPTION_CATALOG | 3833511 | 10385124 | -170.90% |
| 14 | SUBQUERIES_EXISTS_IN | P0 | memory | SQL_SELECT | 3502164 | 9421100 | -169.01% |
| 15 | COLLATE_NOCASE_RTRIM_BINARY | P0 | memory | SQL_COLLATION | 3421071 | 9168513 | -168.00% |
| 16 | JSON_EXTRACT_SET_004 | P2 | memory | GEN_SQL_JSON | 4584231 | 12180560 | -165.71% |
| 17 | AGG_GROUP_HAVING_002 | P1 | memory | GEN_SQL_AGGREGATE | 5150743 | 13309997 | -158.41% |
| 18 | CONSTRAINT_FK_SAVEPOINT_028 | P2 | memory | GEN_SQL_CONSTRAINT_TX | 5766538 | 14734802 | -155.52% |
| 19 | CONSTRAINT_FK_SAVEPOINT_044 | P2 | memory | GEN_SQL_CONSTRAINT_TX | 5556811 | 14070745 | -153.22% |
| 20 | SCALAR_NULL_COALESCE_036 | P1 | memory | GEN_SQL_SCALAR | 5519881 | 13739519 | -148.91% |
| 21 | OPT_NO_ROWID_IN_VIEW | P4 | memory | CLI_OPTION | 5757591 | 14260645 | -147.68% |
| 22 | INDEX_SCHEMA_PRAGMA_036 | P2 | memory | GEN_SQL_INDEX_PRAGMA | 3733973 | 9233306 | -147.28% |
| 23 | WINDOW_PARTITION_SUM_019 | P2 | memory | GEN_SQL_WINDOW | 5987005 | 14758566 | -146.51% |
| 24 | SCALAR_NULL_COALESCE_035 | P1 | memory | GEN_SQL_SCALAR | 5588631 | 13771811 | -146.43% |
| 25 | CONSTRAINT_FK_SAVEPOINT_030 | P2 | memory | GEN_SQL_CONSTRAINT_TX | 4600762 | 11297771 | -145.56% |

</details>
<!-- sqlite-parity-report:end -->

## Jankurai Breakdown

<!-- sqlite-jankurai-breakdown:begin -->

{
  "generated_by": "redline-testing jankurai-compare",
  "redlinedb_score": 87,
  "redlinedb_status": "unknown",
  "score_delta": 67,
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
