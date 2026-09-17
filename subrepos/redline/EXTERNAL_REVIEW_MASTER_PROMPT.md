# External Review Master Prompt

This is a briefing, not ground truth. Verify every claim against source code, tests, manifests, and raw benchmark artifacts before you accept it.

## Read This First

1. Read `AGENTS.md`.
2. Read `.jankurai/owner-map.json`, `.jankurai/test-map.json`, `.jankurai/proof-lanes.toml`, `.jankurai/generated-zones.toml`, and `.jankurai/unsafe-ledger.toml`.
3. Read `docs/WORKPLAN_slam.md` and treat `docs/PHASE10_HANDOFF.md` as historical handoff state, not current proof state.
4. Run `git status --short` before attributing any file, artifact, or benchmark result to a committed tag. The current worktree is dirty and includes phase11 bench additions.
5. Inspect code and artifacts directly. Do not trust README prose, paper prose, or this prompt without checking the underlying evidence.
6. Do not hand-edit generated or archive zones. The repository marks `docs/archive/**`, `paper/figs/*.eps`, and `target/**` as generated or archive output.
7. Keep active source files under the 2,000 LOC cap. If a reviewable source file is getting close, split first and review second.

## Repository Map

| Area | Responsibility |
|---|---|
| `crates/kernel` | MVCC, WAL, pages, indexes, catalog, recovery |
| `crates/sql` | Parser, planner, executor, SQLite surface |
| `crates/redlinedb` | Public Rust API |
| `crates/ffi` | C API and SQLite shim path |
| `crates/bench` | Certification, compat, recovery, failpoint, workload harnesses |
| `paper/` | Paper source, figures, and data tables |
| `docs/` | Workplans, handoffs, proof ledgers |
| `assets/` | Rendered figures and diagrams |
| `scripts/bench/` | Remote benchmark orchestration and artifact export |

## Inspect These First

- Kernel engine, WAL, index, and recovery paths: `crates/kernel/src/engine/mod.rs`, `crates/kernel/src/wal/*`, `crates/kernel/src/index/*`, `crates/kernel/tests/*`.
- SQL DML, index access, vectorized executor, JSON, collation, datetime, and regexp: `crates/sql/src/exec/*`, `crates/sql/src/json/*`, `crates/sql/src/collation.rs`, `crates/sql/src/datetime.rs`, `crates/sql/src/regexp.rs`, `crates/sql/tests/*`.
- Public facade stats and benchmark telemetry: `crates/redlinedb/src/*`, `crates/bench/src/certify.rs`, `crates/bench/src/config.rs`, `crates/bench/src/workload.rs`, `crates/bench/src/chaos.rs`.
- FFI ABI and compatibility surface: `crates/ffi/src/lib.rs`, `crates/ffi/include/sqlite3.h`.
- Proof-lane metadata: `.jankurai/proof-lanes.toml`, `.jankurai/test-map.json`, `.jankurai/owner-map.json`, `.jankurai/generated-zones.toml`, `.jankurai/unsafe-ledger.toml`.

## Current Evidence To Verify

The current workspace re-verification recorded in `docs/WORKPLAN_slam.md` is:

- `cargo fmt --check`
- `./scripts/check_file_sizes.sh`
- `cargo check --workspace --locked`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `cargo test --workspace --quiet --locked` -> `734 passed, 3 ignored`

The same ledger records the phase10 cert smoke certify artifact hashes:

- `target/bench/phase10-cert-smoke/manifest.json` -> `5d3c9df0c524c29edd8b5d2a7af1957a6d03b7d7c9ff9304a3c37be8dd79ae1c`
- `target/bench/phase10-cert-smoke/runs.jsonl` -> `5379ccf7c4906fc6b2fc2e43c4b03bc7309a3b2bd37ffbe5a5ca315c7e043195`
- `target/bench/phase10-cert-smoke/summary.csv` -> `618165672e8dc2959c9dde6bfe07cffc0880437b8097bdf4d713ba35e5eea0b3`
- `target/bench/phase10-cert-smoke/report.md` -> `6efdd3fb51dfdf77cc700f268703a15613451c39c2f4f1fbe22ffbe2086a296b`
- `target/bench/phase10-cert-smoke/report.json` -> `3fca397adfe54be98368f16d7903ccbf5e16093b7db473a342456c1ec5af7d28`

Do not confuse that current workspace re-verification with the older phase10 proof matrix that also appears in the workplan. The newer `734 passed, 3 ignored` count is the one to treat as the latest workspace proof in that document.

## Published Certification And Benchmark Catalog

Use the configs below as the navigation map for the benchmark tree. Verify each claim from the config, the runner, and the exported artifacts.

| Config or lane | What it covers |
|---|---|
| `crates/bench/bench/certification.toml` | Published phase10 xbabe1 certification across point reads, secondary-index reads and range reads, writers-disjoint, hot-row-update, mixed OLTP, and connection-limit |
| `crates/bench/bench/recovery-matrix.toml` | Crash/recovery matrix |
| `crates/bench/bench/failpoint-matrix.toml` | Failpoint matrix with zero-lost-acked-commit checks |
| `crates/bench/compat` | Compatibility suite used by `compat --engine both` |
| `crates/bench/bench/certification-phase10-cert.toml` | Full phase10 feature certification |
| `crates/bench/bench/certification-phase10-smoke.toml` | Local phase10 cert smoke lane |
| `crates/bench/bench/certification-phase10-stress.toml` | Stress lane for the phase10 cert feature set |
| `crates/bench/bench/certification-phase10-compare.toml` | Compare lane for the phase10 cert feature set |
| `crates/bench/bench/phase11-oltp-gap.toml` | Phase11 OLTP gap lane |
| `crates/bench/bench/connection-limit-256.toml` | Connection-limit sweep at fixed high concurrency |
| `crates/bench/bench/connection-fixed-high.toml` | Fixed-high-connection workload |
| `crates/bench/bench/queue-mixed-highload.toml` | Queue mixed high-load workload |
| `crates/bench/bench/dick-head-choas.toml` | Chaos smoke suite |
| `crates/bench/bench/dick-head-choas-bounded.toml` | Chaos bounded certification profile |
| `crates/bench/bench/dick-head-choas-extreme.toml` | Chaos extreme profile |
| `phase9-xbabe1-certify-with-strace` | Strace-enabled certification lane |
| `phase9-xbabe1-gap-strace` | Strace-enabled gap-cert lane |

The phase10 cert configs are feature lanes, not headline SQLite-comparison claims. The workplan shows the local smoke lane as certified; the other phase10 cert configs should be treated as live bench surfaces unless you inspect their raw outputs.

## Phase 10 Comparison Headlines

Cross-check these against `README.md`, `paper/sections/evaluation.tex`, `paper/data/headline_table.csv`, and `paper/data/perf_aggregates.csv`.

- `writers-disjoint` at 64 threads: RedlineDB 15.89x vs SQLite 79 qps, roughly doubling the phase9 ratio.
- `mixed-95-5` at 64 threads: 14.74x.
- `mixed-80-20` at 64 threads: 15.21x.
- `mixed-50-50` at 64 threads: 15.55x.
- `point-read-pk` at 64 threads: 0.99x parity.
- `hot-row-update` at 64 threads: 0.44x, still behind SQLite.
- `secondary-index-range` at 64 threads: 0.048x, still behind SQLite.
- `secondary-index-read` at 64 threads: 0.13x, a new headline but not a parity claim.

This is the claim set to challenge, not to repeat. Check whether the measured data really matches the narrative, especially where the paper says the contended-write gains come from MVCC index changes and better group-commit visibility.

## What Looks Strong

- MVCC write scaling is real on disjoint and mixed OLTP workloads.
- Group-commit WAL telemetry is now observable in benchmark artifacts.
- Crash and failpoint evidence exists and is more detailed than a simple pass/fail report.
- Physical index maintenance and indexed read paths have advanced materially.
- JSON, JSONB, vector, HNSW, and DiskANN features broaden the surface area beyond a minimal SQLite clone.
- The vectorized executor and spill work are moving the analytical path forward.
- Compatibility coverage is growing in a reproducible way, not by hand-picked anecdotes.
- Benchmark manifests are reproducible and hashable, which makes review possible.

## Gaps And Risks

- Hot-row contention still trails SQLite.
- Secondary-index range scans still trail badly and need cursor prefetch or equivalent warm-leaf reuse.
- Single-thread transaction overhead is still higher than SQLite.
- Tier 2 and Tier 3 SQLite syntax is still not fully executable end to end.
- The `sqlite3` ABI and link-compatibility story remains partial.
- DiskANN mmap-resident search is still pending.
- HNSW recall at lower `M` is still not where the project wants it.
- SQLite VFS and syscall metrics are still incomplete in the exported benchmark set.
- Encryption at rest is absent.
- Snapshot isolation is not serializable.

## Review Questions

- Does crash recovery preserve correctness when commit, WAL flush, catalog snapshot, or index maintenance fail mid-flight?
- Are planner and executor index invariants aligned, especially around live handles, `meta_page_id`, and access-path consumption?
- Is WAL catalog snapshot durability actually proven under replay, not just assumed from happy-path tests?
- Do index split, cursor, duplicate-key, and boundary conditions still hold under adversarial inputs?
- What happens on transaction rollback or commit failure when heap and index state have already diverged?
- Where do SQLite compatibility divergences remain in parser, planner, executor, and FFI behavior?
- Are the benchmark lanes fair, reproducible, and free of hidden warmup, artifact, or host-side bias?
- Are the benchmark artifacts sufficient to reconstruct the claim without hidden state?
- Is the FFI surface memory-safe at the ABI boundary, including lifetime and error-path handling?

## High-Value Improvement Ideas

- Add range-cursor prefetch and warm-leaf reuse for secondary-index scans.
- Explore hot-row group-commit or commutative-delta optimization for pathological contention.
- Expand differential testing against SQLite and SQLLogicTest-style inputs.
- Validate the full ABI shim against real clients such as rusqlite, Python, and Go.
- Add deterministic fuzzing for parser, executor, index, and WAL paths.
- Capture richer syscall and VFS metrics in the benchmark manifest.
- Design and review serializable isolation before calling the concurrency model complete.
- Finish DiskANN mmap support.
- Tune HNSW for lower `M` recall.
- Add security lanes for `audit`, `deny`, and `gitleaks`, plus an encryption-at-rest design review.

## Commands To Run

Start from the default proof lane and widen only when the change crosses contract, security, or concurrency boundaries.

```bash
just fast
just clippy
just phase10-cert-smoke
just phase11-oltp-gap
just phase9-failpoint-matrix

rtk cargo fmt --check
rtk cargo clippy --workspace --all-targets --locked -- -D warnings
rtk cargo test --workspace --quiet --locked
rtk cargo run -p redlinedb-bench --release -- certify --config crates/bench/bench/certification-phase10-smoke.toml --out-dir target/bench/phase10-cert-smoke --seed 7 --repetitions 1 --warmup 0
rtk cargo run -p redlinedb-bench --release -- certify --config crates/bench/bench/phase11-oltp-gap.toml --out-dir target/bench/phase11-oltp-gap --seed 7 --repetitions 3 --warmup 1
rtk cargo run -p redlinedb-bench -- recover-matrix --config crates/bench/bench/recovery-matrix.toml --out target/bench/recovery-matrix.json --seed 7
rtk cargo run -p redlinedb-bench -- failpoint-matrix --config crates/bench/bench/failpoint-matrix.toml --out target/bench/failpoint-matrix.json --seed 7
rtk cargo run -p redlinedb-bench -- compat --engine both --test-dir crates/bench/compat --seed 7
```

The recovery matrix currently runs via the direct `rtk cargo run -p redlinedb-bench -- recover-matrix ...` command above; the tree does not expose a standalone `just phase9-recovery-matrix` recipe today.

For remote xbabe1 review, inspect the lane definitions in `.jankurai/proof-lanes.toml` and the runner scripts in `scripts/bench/`. The published phase10 cert lane is `phase10-xbabe1-certification`; the strace-capable lanes are `phase9-xbabe1-certify-with-strace` and `phase9-xbabe1-gap-strace`.

## How To Challenge The Claims

1. Prefer source and artifact inspection over prose.
2. Cross-check every ratio against the raw CSVs and manifests.
3. Look for counterexamples in tests, failpoints, and recovery outputs.
4. Treat benchmark fairness as a first-class review topic.
5. Call out any mismatch between the code, the exported data, and the narrative.
