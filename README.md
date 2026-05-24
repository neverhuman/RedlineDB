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
![Jankurai score](assets/sqlite-jankurai-score.svg)

![Code shape](assets/sqlite-code-shape.svg)

![Median test performance](assets/sqlite-median-test-performance.svg)

![KSLOC](assets/sqlite-parity-ksloc.svg)

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
**SQLite parity coverage:** **1127 / 1127** cases passed in CI. Failed: **0**. Missing: **0**. Skipped: **0**. Updated 2026-05-24.

**SQLite parity latency:** median gap **-1.28%**, worst gap **-46.75%**, faster cases **543**.

![SQLite parity latency improvement plot](assets/sqlite-parity-latency-gap.svg)

![SQLite parity performance distribution](assets/sqlite-parity-performance-histogram.svg)

<details id="sqlite-parity-ranked-latency-table">
<summary>Full ranked latency table</summary>

| Rank | Case | Priority | Profile | Category | SQLite median ns | RedlineDB median ns | Improvement |
| ---: | --- | --- | --- | --- | ---: | ---: | ---: |
| 1 | DOT_BACKUP_RESTORE_TEMPFILE | P1 | tempfile | CLI_TEMPFILE | 2244087 | 4402543 | -46.75% |
| 2 | OPT_MAXSIZE_DESERIALIZE_TEMPFILE | P3 | tempfile | CLI_OPTION_TEMPFILE | 5683523 | 8329290 | -46.55% |
| 3 | DOT_SAVE_RESTORE_TEMPFILE | P1 | tempfile | CLI_TEMPFILE | 2205190 | 4385311 | -46.18% |
| 4 | OPT_READONLY_TEMPFILE | P2 | tempfile | CLI_OPTION_TEMPFILE | 7170487 | 10117661 | -41.10% |
| 5 | DOT_CLONE_TEMPFILE | P2 | tempfile | CLI_TEMPFILE | 2484040 | 3965791 | -32.19% |
| 6 | DOT_IMPORT_CSV_TEMPFILE | P1 | tempfile | CLI_TEMPFILE | 1875674 | 3888733 | -29.62% |
| 7 | JOIN_SUBQUERY_EXISTS_068 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1787768 | 3837098 | -27.90% |
| 8 | JOIN_SUBQUERY_EXISTS_073 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1765014 | 3836297 | -27.88% |
| 9 | JOIN_SUBQUERY_EXISTS_072 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1755187 | 3836247 | -27.87% |
| 10 | JOIN_SUBQUERY_EXISTS_082 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1757681 | 3804854 | -26.83% |
| 11 | JOIN_SUBQUERY_EXISTS_067 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1787287 | 3795840 | -26.53% |
| 12 | JOIN_SUBQUERY_EXISTS_009 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1767459 | 3795490 | -26.52% |
| 13 | JOIN_SUBQUERY_EXISTS_070 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1715922 | 3775029 | -25.83% |
| 14 | JOIN_SUBQUERY_EXISTS_085 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1800221 | 3759362 | -25.31% |
| 15 | JOIN_SUBQUERY_EXISTS_010 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1745648 | 3759282 | -25.31% |
| 16 | JOIN_SUBQUERY_EXISTS_071 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1796945 | 3755685 | -25.19% |
| 17 | JOIN_SUBQUERY_EXISTS_069 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1736240 | 3746942 | -24.90% |
| 18 | JOIN_SUBQUERY_EXISTS_084 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1682349 | 3719827 | -23.99% |
| 19 | SQL_ATTACH_TEMPFILE_DATABASE | P1 | tempfile | SQL_TEMPFILE | 1786337 | 3712593 | -23.75% |
| 20 | JOIN_SUBQUERY_EXISTS_074 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1743594 | 3708646 | -23.62% |
| 21 | JOIN_SUBQUERY_EXISTS_076 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1783159 | 3692926 | -23.10% |
| 22 | DML_WHERE_ORDER_LIMIT_110 | P1 | memory | GEN_SQL_DML | 1579605 | 3679859 | -22.66% |
| 23 | JOIN_SUBQUERY_EXISTS_011 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1814068 | 3677548 | -22.58% |
| 24 | DML_WHERE_ORDER_LIMIT_120 | P1 | memory | GEN_SQL_DML | 1618108 | 3671558 | -22.39% |
| 25 | JOIN_SUBQUERY_EXISTS_080 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 1776637 | 3644235 | -21.47% |

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
