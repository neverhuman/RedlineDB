<p align="center">
  <img src="assets/redlinedb-banner.png" alt="RedlineDB" width="100%">
</p>

<h1 align="center">RedlineDB</h1>

<p align="center">
  <em>Rust-native embedded SQL with SQLite-shaped compatibility, concurrent writes, and deterministic recovery.</em>
</p>

<p align="center">
  <a href="#whats-new-in-v400"><img src="https://img.shields.io/badge/sqlite%20parity-2374%2F2445%20(97.10%25)-brightgreen" alt="sqlite parity"></a>
  <a href="#whats-new-in-v400"><img src="https://img.shields.io/badge/corpus%20cases-2445-blue" alt="corpus cases"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-Apache--2.0-blue" alt="license"></a>
  <a href="rust-toolchain.toml"><img src="https://img.shields.io/badge/rust-1.95-orange" alt="rust"></a>
  <img src="https://img.shields.io/badge/version-4.0.0-blue" alt="version">
  <!-- jankurai-score-badge:begin -->
  <a href=".jankurai/repo-score.md"><img src="https://img.shields.io/badge/jankurai-85%2F100%20advisory-green" alt="jankurai score: 85/100 advisory"></a>
  <!-- jankurai-score-badge:end -->
</p>

RedlineDB is an embedded SQL engine written in Rust. It keeps the SQLite-facing
API familiar while replacing the storage core with MVCC, a concurrent B-tree,
group-commit WAL, and crash recovery designed for multi-writer workloads.

## What's new in v4.0.0

**Phase 0-4 SQLite-parity speed-gap closure.** Fourteen named optimizations across the build profile, parser, scalar fast paths, and CTE/aggregate/window hot paths, measured against the external [`redline-testing v1.0.0`](https://github.com/neverhuman/redline-testing) parity harness on the full 2445-case `sqlite_parity` suite. Median per-case latency ratio against SQLite improved from **1.837× → 1.738×** with **zero parity regressions** (identical 2374/2445 pass set in v3.0.0 and v4.0.0; the 67 failures are pre-existing edge cases in `typeof()` reporting, IEEE-754 last-digit precision, fullwidth Unicode case-folding, BLOB hex encoding, and `AUTOINCREMENT` semantics). Jankurai code-health score holds at **85/100 (pass)**.

> **Note on corpus size.** The redline-testing official corpus has grown from 1127 cases (prior CI snapshot) to **2445 cases** in v1.0.0. The v4.0.0 numbers in this section are measured against the larger current corpus. The auto-generated `## Engine Metrics` block below still reflects the previous 1127-case CI snapshot and will be refreshed by the next CI parity report.

### Per-case latency distribution — RedlineDB / SQLite ratio (full 2445-case corpus, passed cases only)

| Bucket | v3.0.0 (main) | v4.0.0 | Delta |
|---|---:|---:|---:|
| `< 1.0×` (RedlineDB faster than SQLite) | 7 | 8 | **+1** |
| `1.0 – 1.2×` | 16 | 28 | **+12** |
| `1.2 – 1.5×` | 173 | 292 | **+119** |
| `1.5 – 2.0×` | 1622 | 1748 | **+126** |
| `2.0 – 3.0×` | 555 | 297 | **−258** |
| `≥ 3.0×` (tail outliers) | 1 | 1 | 0 |
| **Total** | **2374** | **2374** | 0 |

258 cases moved out of the `2.0–3.0×` slow band; 119 moved into the `1.2–1.5×` band. Per-case: **1410 cases (59.4%) are ≥5% faster** in v4.0.0, 386 (16.3%) are ≥5% slower, 578 (24.3%) within ±5% noise. Mean per-case target-latency change: **−6.85%** (median **−7.67%**).

### Named optimizations shipped

| Phase | Commit | Optimization |
|---|---|---|
| 1.1 | `f8ed61f` | fat LTO + `opt-level=3` + `target-cpu=native` release profile |
| 1.2 | `b62d4ad` | parser rewrite-pass allocation elimination |
| 1.3 | `4a89e9a` | borrow + stack-buffer function-name lowercase |
| 1.4 | `b229f90` | cache + lighten `/dev/shm` writability probe |
| 1.5 | `2e13dc5` | fromless `SELECT` fast path |
| 1.6 | `a20de92` | `ahash::RandomState` for `StatementCache` |
| 2.1–2.2 | `5bbe650` | ASCII fast paths for `LENGTH`/`UPPER`/`LOWER` + `memmem` for `INSTR` |
| 2.3+2.5 | `9abab6c` | `value_as_str` + hot scalar fn migration to `Cow` |
| 2.4 | `32e078d` | `itoa` for streaming i64 CLI output |
| 4.1 | `efc9a6e` | fromless-SELECT walker covers `sqlparser` scalar variants |
| 4.2 | `d348e0b` | dedup aggregate cache key + reuse fn-name lower |
| 4.3 | `e569d6c` | capacity hints in per-row hot allocations |
| 4.4 | `32200c2` | hoist CTE lowercase out of recursive iteration loop |
| 4.5 | `2f21ea3` | reuse scratch buffer for window partition keys |

### Benchmark provenance

- **Harness:** [`redline-testing v1.0.0`](https://github.com/neverhuman/redline-testing) — external repository, not in-tree fixtures.
- **SQLite reference:** `sqlite3 3.53.1` (release build, SHA-256 `fd3bdd25217a849f8f4fa295fb78199cfd69b0c4d47ba8d8c32a1aa328bd147e`).
- **Workload:** full `sqlite_parity` suite — 2445 cases × 3 measured reps + 1 warmup, **`--workers 30`** on a 128-core Linux x86_64 host, no CPU pinning.
- **Target binary (v4.0.0):** SHA-256 `7ae60cb513e866b4a94996968b0c6b9f01b0071776bc842f526702be33f05e56` (release profile, fat LTO, `target-cpu=native`).
- **Baseline binary (v3.0.0):** SHA-256 `da770dfd25beeb36aa22f8ce7a09d935b4e9fd7c8b2a77c36e621c46cec69ef2`.
- **Raw JSONL evidence (committed):** [`benchmark-results/sqlite-parity/perf-baselines/v3.0.0-baseline.jsonl`](benchmark-results/sqlite-parity/perf-baselines/v3.0.0-baseline.jsonl), [`v4.0.0-baseline.jsonl`](benchmark-results/sqlite-parity/perf-baselines/v4.0.0-baseline.jsonl), and the structured A/B summary [`v3-vs-v4-summary.json`](benchmark-results/sqlite-parity/perf-baselines/v3-vs-v4-summary.json).
- **Reproduce:**
  ```bash
  cargo build --release -p redlinedb-cli
  PERF_WORKERS=30 \
    REDLINE_TESTING_BIN=/path/to/redline-testing \
    SQLITE_REF_BIN=/path/to/sqlite3-3.53.1 \
    scripts/perf/full.sh target/release/redlinedb v4.0.0-final
  ```

### Jankurai code-health score (v4.0.0)

**85 / 100 — `pass` (advisory)** — unchanged from main; Phase 0-4 perf work introduced no code-health regressions. Full report at [`.jankurai/repo-score.md`](.jankurai/repo-score.md). Top dimensions: Ownership & navigation (100), Proof lanes & test routing (98), Contract & boundary integrity (88), Security & supply-chain posture (86).

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
  redlinedb = "=4.0.0"
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
curl -LsSf https://raw.githubusercontent.com/neverhuman/RedlineDB/main/scripts/install.sh | VERSION=v4.0.0 bash
```

Lock the exact tarball digest in CI or release automation:

```bash
curl -LsSf https://raw.githubusercontent.com/neverhuman/RedlineDB/main/scripts/install.sh | \
  VERSION=v4.0.0 REDLINEDB_SHA256=<sha256> bash
```

### Build from source

```bash
cargo install redlinedb-cli --version 4.0.0 --locked
```

Or install from the tagged repository release:

```bash
cargo install --git https://github.com/neverhuman/RedlineDB.git --tag v4.0.0 --package redlinedb-cli --locked
```

### Direct download

Release tarballs are published on the [releases page](https://github.com/neverhuman/RedlineDB/releases):

| Platform | File |
|---|---|
| Linux x86_64 | `redlinedb-v4.0.0-linux-x86_64.tar.gz` |
| macOS Apple Silicon | `redlinedb-v4.0.0-macos-arm64.tar.gz` |
| macOS Intel | `redlinedb-v4.0.0-macos-x86_64.tar.gz` |

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
**SQLite parity coverage:** **1123 / 1127** cases passed in CI. Failed: **0**. Skipped: **4**. Updated 2026-05-26.

**SQLite parity latency:** median gap **-22.12%**, worst gap **-287.85%**, faster cases **255**.

**Benchmark metadata:** RedlineDB target version **redlinedb v4.0.1 (SQLite 3.45.1 compatibility)**, SQLite reference version **3.53.1 2026-05-05 10:34:17 c88b22011a54b4f6fbd149e9f8e4de77658ce58143a1af0e3785e4e6475127e9 (64-bit)**, redline-testing runner version **redline-testing 1.0.0**.

![SQLite parity latency improvement plot](assets/sqlite-parity-latency-gap.svg)

![SQLite parity performance distribution](assets/sqlite-parity-performance-histogram.svg)

<details id="sqlite-parity-ranked-table">
<summary>Full ranked latency table</summary>

| Rank | Case | Priority | Profile | Category | SQLite median ns | RedlineDB median ns | Improvement |
| ---: | --- | --- | --- | --- | ---: | ---: | ---: |
| 1 | DOT_SAVE_RESTORE_TEMPFILE | P1 | tempfile | CLI_TEMPFILE | 3678752 | 14267881 | -287.85% |
| 2 | INDEX_SCHEMA_PRAGMA_044 | P2 | memory | GEN_SQL_INDEX_PRAGMA | 2951125 | 10090366 | -236.35% |
| 3 | SCALAR_ARITH_018 | P1 | memory | GEN_SQL_SCALAR | 3745318 | 11646570 | -210.96% |
| 4 | SCHEMA_SQLITE_SCHEMA | P0 | memory | SQL_SCHEMA | 4032532 | 12337838 | -205.96% |
| 5 | CTE_RECURSIVE_MATRIX_077 | P1 | memory | GEN_SQL_CTE | 6849262 | 19504391 | -184.77% |
| 6 | INDEX_SCHEMA_PRAGMA_011 | P2 | memory | GEN_SQL_INDEX_PRAGMA | 4339903 | 11996974 | -176.43% |
| 7 | AGG_GROUP_HAVING_011 | P1 | memory | GEN_SQL_AGGREGATE | 6634425 | 18072260 | -172.40% |
| 8 | CTE_RECURSIVE_MATRIX_038 | P1 | memory | GEN_SQL_CTE | 7148588 | 19401436 | -171.40% |
| 9 | JOIN_SUBQUERY_EXISTS_020 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 5282957 | 14250929 | -169.75% |
| 10 | AGG_GROUP_HAVING_010 | P1 | memory | GEN_SQL_AGGREGATE | 6904626 | 18086868 | -161.95% |
| 11 | CONSTRAINT_FK_SAVEPOINT_013 | P2 | memory | GEN_SQL_CONSTRAINT_TX | 5744381 | 14965471 | -160.52% |
| 12 | DOT_EXIT_CODE | P0 | memory | CLI_DOT_COMMAND | 4854657 | 12603581 | -159.62% |
| 13 | INDEX_SCHEMA_PRAGMA_015 | P2 | memory | GEN_SQL_INDEX_PRAGMA | 6127135 | 15664043 | -155.65% |
| 14 | INDEX_SCHEMA_PRAGMA_023 | P2 | memory | GEN_SQL_INDEX_PRAGMA | 4847253 | 12191983 | -151.52% |
| 15 | DOT_CHANGES | P0 | memory | CLI_DOT_COMMAND | 3739767 | 9397665 | -151.29% |
| 16 | JOIN_SUBQUERY_EXISTS_037 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 4455211 | 11192561 | -151.22% |
| 17 | CONSTRAINT_FK_SAVEPOINT_018 | P2 | memory | GEN_SQL_CONSTRAINT_TX | 5843308 | 14482267 | -147.84% |
| 18 | JOIN_SUBQUERY_EXISTS_074 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | 6465997 | 15809278 | -144.50% |
| 19 | DETACH_DATABASE_SYNTAX | P0 | memory | SQL_ATTACH | 5199330 | 12630302 | -142.92% |
| 20 | DOT_MODE_CSV_AND_QUOTE | P0 | memory | CLI_DOT_COMMAND | 4455972 | 10734374 | -140.90% |
| 21 | DOT_EXPERT_OPTIONAL | P3 | memory | CLI_DOT_COMMAND_OPTIONAL | 5225178 | 12499204 | -139.21% |
| 22 | DML_WHERE_ORDER_LIMIT_056 | P1 | memory | GEN_SQL_DML | 6317776 | 14829845 | -134.73% |
| 23 | CTE_RECURSIVE_MATRIX_045 | P1 | memory | GEN_SQL_CTE | 6545347 | 15189095 | -132.06% |
| 24 | DML_WHERE_ORDER_LIMIT_037 | P1 | memory | GEN_SQL_DML | 5569621 | 12902958 | -131.67% |
| 25 | ON_CONFLICT_ALGORITHMS | P0 | memory | SQL_CONFLICT | 5169253 | 11923444 | -130.66% |

</details>
<!-- sqlite-parity-report:end -->

## Jankurai Breakdown

<!-- sqlite-jankurai-breakdown:begin -->

{
  "generated_by": "redline-testing jankurai-compare",
  "redlinedb_score": 85,
  "redlinedb_status": "unknown",
  "score_delta": 65,
  "sqlite_ref": "version-3.53.1",
  "sqlite_score": 20,
  "sqlite_status": "unknown",
  "updated_date": "2026-05-26"
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
