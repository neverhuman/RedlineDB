# WORKPLAN_CLAUDE - Parallel Closure Plan

This document is a coordination plan for closing the remaining SQLite-compatibility, benchmark, failpoint, and physical-index work. Treat `docs/WORKPLAN_slam.md` as the current proof ledger and update that ledger with raw commands, exit statuses, artifact paths, and SHA-256 hashes whenever new proof is collected.

## Current Baseline

`docs/WORKPLAN_slam.md` records the verified workspace baseline. The relevant green proof is:

- `rtk cargo fmt --check`
- `./scripts/check_file_sizes.sh`
- `rtk cargo check --workspace --locked`
- `rtk cargo clippy --workspace --all-targets --locked -- -D warnings`
- `rtk cargo test --workspace --quiet --locked` with `174 passed (28 suites, 3.69s)`
- package tests for `redlinedb-bench`, `redlinedb`, `redlinedb-ffi`, `redlinedb-sql`, and later `redlinedb-kernel`
- benchmark smoke, compat, recovery-matrix, and certify-smoke runs with artifact hashes recorded in `WORKPLAN_slam.md`

The current implemented state is:

- The SQL parser split is complete enough to clear the file-size warning. `crates/sql/src/parser.rs` remains the entry point and uses parser submodules.
- Catalog durability now includes `WalPayload::CatalogSnapshot`; recovery replays catalog snapshots and uses them as the durable source for DDL recovery when `schema.redline` is missing.
- SQL table row loading is relation-qualified end to end; executor table access no longer falls back to a global row-directory scan.
- Planner output has been made conservative again and no longer advertises index access paths that the executor cannot actually use.
- `redlinedb-bench certify` exists and writes `runs.jsonl`, `summary.csv`, `report.md`, and `manifest.json` under a dedicated artifact tree.

The remaining closure gaps are:

- Physical indexes are not yet maintained by SQL DML or used by SQL reads.
- Deterministic failpoints are absent, so crash certification is not yet a closed proof lane.
- Benchmark telemetry is still shallow: child-process resource metrics, deeper SQLite validation, syscall tracing, and artifact completeness still need work.
- `xbabe1` still needs full certification through the `certify` lane with raw artifacts and hashes.
- Hot-path `Lsn(1)` and `Lsn::ZERO` mutation sentinels remain in kernel/index mutation paths and WAL page-image accounting. Distinguish legitimate zero initialization from mutation sentinels before changing them.

## Critical Corrections To Claude's Prior Plan

- Remove the old Phase 8.5 stabilization narrative. The workspace baseline has already been re-verified in `WORKPLAN_slam.md`; do not plan around stale claims that the tree fails to compile.
- Do not claim parser splitting is incomplete. The correct parser shape is `crates/sql/src/parser.rs` as the entry point plus submodules under `crates/sql/src/parser/`. Do not require a `parser/mod.rs` migration.
- Treat current catalog snapshots as the durable design for review. Do not require `CatalogDelta` before closure. Catalog deltas can remain a later optimization once snapshot durability and recovery have been reviewed.
- Make `redlinedb-bench certify` the certification lane. `compare` is now a smoke/backcompat wrapper and must not be documented as the source of truth for certification.
- No benchmark headline claims are allowed until physical indexes, deterministic failpoints, benchmark telemetry gates, and `xbabe1` artifacts are complete. Until then, report wording stays conservative and evidence-bound.
- Do not edit `docs/archive/**`; those paths are generated/archive inputs. Use the slam tips only as background.

## Parallel Execution Map

All workers must read `agent/owner-map.json`, `agent/test-map.json`, `agent/proof-lanes.toml`, `agent/generated-zones.toml`, and `agent/unsafe-ledger.toml` before editing. Every worker must avoid reverting changes made by others.

High-conflict files must be fused by one integrator: `crates/kernel/src/engine/mod.rs`, `crates/kernel/src/engine/page_heap.rs`, `crates/kernel/src/index/mod.rs`, SQL executor files, SQL planner files, and benchmark report/config files.

### Lane A: Physical Index Kernel/Catalog

Owns the kernel index manager, `BtreeIndex` transaction-aware creation, `meta_page_id`, catalog helpers to set `IndexDef.meta_page_id`, engine index open/create lifecycle, and DDL backfill.

Rules:

- Do not touch the SQL planner except for compile fallout.
- Coordinate with Lane H before changing LSN/index mutation APIs.
- Package proof should include kernel tests and any catalog recovery tests touched by the API changes.

### Lane B: SQL DML Index Maintenance

Owns INSERT/UPDATE/DELETE index maintenance and unique conflict detection through physical indexes.

Rules:

- Preserve SQLite NULL uniqueness behavior: skip unique checks when any unique key component is NULL.
- Reload heap rows by relation and recheck visibility before trusting index entries.
- Depend on Lane A physical index APIs.

### Lane C: SQL Index Reads And Planner

Owns `index_access` probes, leading-column equality/range support, executor integration, and EXPLAIN output.

Rules:

- Planner may only advertise paths the executor actually uses.
- Covering indexes and multi-index OR/AND stay disabled until implemented.
- Depend on Lane A for physical index handles and on Lane B where read semantics rely on maintained index contents.

### Lane D: Failpoint Infrastructure

Owns feature-gated registry/macros and package feature wiring.

Required actions:

- Implement `panic`, `io-error`, `abort`, `sleep-ms`, hit counts, and `nth`.
- Keep the infrastructure compile-neutral when the feature is disabled.
- Initially avoid hook insertion in kernel files being changed by Lanes A, B, and C.

### Lane E: Failpoint Hooks And Matrix

Runs after Lanes A, B, and C stabilize.

Owns hooks at WAL write/fsync, commit before publish, heap/page mutation, index mutation, catalog temp/fsync/rename/parent fsync, checkpoint/control writes, and WAL prune.

Required actions:

- Add `redlinedb-bench failpoint-matrix`.
- Add a hidden child mode with an fsynced ack oracle.
- Gate strict Redline recovery on zero lost acknowledged commits.

### Lane F: Benchmark Telemetry And Reporting

Owns `redlinedb-bench certify` expansion.

Required actions:

- Capture child-process metrics, `/proc/self/status`, `/proc/self/io`, `/proc/self/statm`, and macOS `getrusage` fallback.
- Verify SQLite PRAGMAs, integrity checks, collection errors, artifact paths, and summary/report fields.
- Implement strace tracing for Docker/Linux and graceful no-op with an explicit reason elsewhere.
- Keep SQLite VFS metrics optional, isolated, disabled by default, and marked `NEEDS_REVIEW`.

### Lane G: xbabe1/Docker/Proof-Lane Integration

Owns Dockerfile updates for `strace` and benchmark tooling, `xbabe1` scripts, `agent/proof-lanes.toml`, `agent/test-map.json`, and justfile benchmark commands.

Required actions:

- Replace stale `compare` certification commands with `certify`.
- Ensure `crates/bench/compat` versus `crates/bench/compat/slt` mismatch is resolved or explicitly documented.
- Keep remote scripts reproducible and artifact-oriented.

### Lane H: WAL/LSN Sentinel Cleanup

Owns hot-path `Lsn(1)` and `Lsn::ZERO` cleanup in heap/index mutation paths and WAL page-image accounting.

Rules:

- Distinguish legitimate zero initialization from mutation sentinels.
- Run after physical index APIs are stable, or assign to the same kernel owner as Lane A.
- Keep recovery tests close to any changed WAL/page-image behavior.

## Fusion And Acceptance

After parallel work, a single integrator must run a fusion phase. Fusion reconciles APIs, feature flags, test maps, proof lanes, docs, and benchmark scripts before broader tests run.

Fusion order:

1. Merge benchmark/doc/script lanes first if they are compile-neutral.
2. Merge failpoint infrastructure before hooks.
3. Merge kernel/catalog physical-index lane.
4. Merge SQL DML index maintenance.
5. Merge SQL index read/planner lane and enable EXPLAIN index paths only then.
6. Merge failpoint hooks and failpoint matrix.
7. Merge WAL/LSN cleanup.
8. Run formatting, file-size checks, package tests, full workspace tests, and benchmark smoke.
9. Update `docs/WORKPLAN_slam.md` with commands, exit statuses, artifact paths, and SHA-256 hashes.

After each lane:

```bash
rtk cargo fmt --check
./scripts/check_file_sizes.sh
# plus package-scoped tests for touched crates
```

Full local acceptance:

```bash
rtk cargo check --workspace --locked
rtk cargo clippy --workspace --all-targets --locked -- -D warnings
rtk cargo test --workspace --quiet --locked
rtk cargo test -p redlinedb-kernel --quiet --locked
rtk cargo test -p redlinedb-sql --quiet --locked
rtk cargo test -p redlinedb-bench --quiet --locked
rtk cargo run -p redlinedb-bench -- compat --engine both --test-dir crates/bench/compat --seed 7 --out target/bench/compat-full.json
rtk cargo run -p redlinedb-bench -- failpoint-matrix --config crates/bench/bench/failpoint-matrix.toml --out target/bench/failpoint-matrix.json --seed 7
rtk cargo run -p redlinedb-bench -- certify --config crates/bench/bench/smoke.toml --out-dir target/bench/certify-smoke --seed 7 --repetitions 1 --warmup 0
```

Acceptance assumptions:

- SQLite "drop-in" means C API and SQL behavior compatibility for the supported subset, not SQLite file-format compatibility.
- `CatalogSnapshot` remains the current authoritative WAL schema mechanism.
- Parallel agents may make branches or patches, but one fusion owner performs final integration and proof collection.

## xbabe1 Certification

Use `certify` as the remote benchmark source of truth. Build and run through the existing `xbabe1` Docker scripts, then fetch the complete artifact tree back into the workspace.

Required preflight on `xbabe1`:

```bash
rtk cargo test --workspace --quiet --locked
```

Initial certification command:

```bash
rtk cargo run -p redlinedb-bench -- certify --config crates/bench/bench/certification.toml --out-dir target/bench/xbabe1/certification --seed 7 --repetitions 5 --warmup 1
```

Required artifacts:

- `runs.jsonl`
- `summary.csv`
- `report.md`
- `manifest.json`
- `plots/*.png`
- `raw/*stdout`
- `raw/*stderr`
- optional `raw/*strace`

The manifest must include:

- git SHA and dirty state
- Docker image digest
- host identity, CPU, RAM, and filesystem
- rustc version
- SQLite version
- benchmark config hash
- seed, row counts, thread counts, and durability modes
- SQLite PRAGMA values and validation status
- Redline and SQLite checksums
- hashes for every emitted artifact

Required metrics:

- throughput
- p50, p95, p99, p999, and max latency
- total failures
- BUSY, LOCKED, and timeout counts reported separately
- RSS peak
- proc I/O counters
- fsync, fdatasync, write, and pwrite counts
- Redline data bytes and WAL bytes
- SQLite checkpoint stats
- integrity-check status and errors

Required validation:

- Docker/Linux runs should collect `strace` syscall counts when available.
- Non-Linux or missing-`strace` runs must record a no-op reason instead of silently omitting tracing.
- SQLite PRAGMAs must be validated and written into the manifest, not assumed from configuration.
- Checksums must be recorded for logical result sets and emitted artifacts.
- Crash/failpoint certification must use the failpoint matrix once Lane E exists.

Report wording must remain conservative. Use "SQLite contention and tail-latency evidence" unless raw artifacts prove a stronger claim and the proof ledger records the exact commands, exit statuses, artifact paths, and hashes.
