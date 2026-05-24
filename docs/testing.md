# Testing — Proof Lanes, Budgets, and Repair Receipts

Every change in this repo is validated through a named *proof lane* —
a deterministic recipe an agent can rerun without re-discovering it.
Lanes are declared in `.jankurai/proof-lanes.toml`; this doc indexes them,
records the budgets/kill-switches that bound the long-running ones,
and points at the structured error surface that produces machine-
readable repair receipts.

## Proof-lane index

| Lane                                 | Proves                                                                                                |
|--------------------------------------|--------------------------------------------------------------------------------------------------------|
| `setup`                              | Prime the workspace build cache before a wider proof run.                                             |
| `check`                              | Root validation gate: fast, score, security, rust-map, rust-witness, and rust-diagnose.              |
| `test`                               | Fast workspace test proof.                                                                            |
| `verify`                             | Alias for the root validation gate.                                                                   |
| `fast`                               | Workspace fmt, file-size policy, type-check, and full unit/integration test sweep. Uses `scripts/sccache_wrapper.sh`, which falls back cleanly when local `sccache` is absent. Quick iteration lane, not the pre-push gate. |
| `pr-ci`                              | Exact local mirror of `.github/workflows/ci.yml`: preflight, test shards, the verified `redline-testing-official` gate, and `official-evidence-guard`. Run with `scripts/ci-local.sh pr-ci`. |
| `fast-check`                         | Workspace compile proof for the default health lane.                                                  |
| `fast-test`                          | Workspace test proof for the default health lane.                                                     |
| `hygiene`                            | Format and file-size only; cheapest pre-commit gate.                                                  |
| `clippy`                             | `cargo clippy --workspace --all-targets -- -D warnings`.                                              |
| `medium`                             | `fast` plus `--help` smoke for `cli` and `server`.                                                    |
| `phase8-smoke`                       | Same as `medium`; pinned for phase-8 regression triage.                                               |
| `kernel-cursor`                      | Cursor-specific kernel regression tests without the full workspace sweep.                             |
| `cache-warm`                         | Prime the workspace build cache before a wider proof run.                                             |
| `redline-testing-official`           | Verified external official conformance and benchmark gate; `neverhuman/redline-testing` is the sole official source. |
| `official-evidence-guard`            | Fails if RedlineDB reintroduces official metric/report generation outside the verified external runner or its processed evidence bundle. |
| `sqlite-parity-report-update`        | Regenerates official SQLite parity report and README data through the verified external `redline-testing` artifact and processed evidence bundle. |
| `ffi-abi`                            | C ABI compatibility tests for the SQLite shim surface.                                                |
| `cli-shell`                          | CLI compatibility tests for the shell/batch front end.                                                |
| `kernel-check`                       | Targeted `redlinedb-kernel` compile proof.                                                            |
| `kernel-test`                        | Targeted `redlinedb-kernel` test proof.                                                                |
| `sql-check`                          | Targeted `redlinedb-sql` compile proof.                                                               |
| `sql-test`                           | Targeted `redlinedb-sql` test proof.                                                                   |
| `beyond-sqlite-manifest`             | Verifies the beyond-SQLite backlog ranking, source tips, owners, and proof-lane routing.               |
| `beyond-postgres-reference`          | Runs the beyond-SQLite manifest and Postgres oracle tests against PostgreSQL 16. Starts a Docker container locally when `REDLINEDB_POSTGRES_URL` is unset. |
| `ffi-check`                          | Targeted `redlinedb-ffi` compile proof.                                                               |
| `ffi-test`                           | Targeted `redlinedb-ffi` test proof.                                                                   |
| `cli-check`                          | Targeted `redlinedb-cli` compile proof.                                                               |
| `cli-test`                           | Targeted `redlinedb-cli` test proof.                                                                   |
| `phase9-smoke`                       | Bench harness unit tests plus a one-rep certify and a cross-engine sweep.                                   |
| `phase9-compat-full`                 | Full `cross-engine --engine both` matrix against `crates/bench/compat/`.                                    |
| `phase9-certification`               | 5-rep + 1-warmup certify against `crates/bench/bench/certification.toml`.                             |
| `phase9-xbabe1-gap`                  | Gap certify on the xbabe1 docker host (sync, run, fetch).                                             |
| `phase9-xbabe1-gap-strace`           | Same as above with `strace -c` aggregation.                                                           |
| `phase9-docker-smoke`                | Bench unit tests run inside the xbabe1 docker host.                                                   |
| `phase9-recovery-matrix`             | Recovery matrix run for WAL/checksum failure modes.                                                   |
| `phase9-failpoint-matrix`            | Failpoint matrix run.                                                                                 |
| `phase9-xbabe1-certification`        | Full 5-rep certify on the xbabe1 host.                                                                |
| `phase9-xbabe1-certify-with-strace`  | Same as above with strace-instrumented children.                                                      |
| `phase10-xbabe1-certification`       | Phase-10 closing certification matrix on the 128-core xbabe1 host.                                    |
| `phase10-hnsw-recall`                | HNSW recall test (`-- --ignored`).                                                                    |
| `phase11-oltp-gap`                   | OLTP gap workload certify.                                                                            |
| `phase11-ephemeral-db`               | Ephemeral DB integration test.                                                                        |
| `phase11-sql-contracts`              | Phase-11 SQL contract tests (temp roots, queue, xdoug-compat).                                        |
| `security`                           | `cargo audit` + `cargo deny check` + `gitleaks detect`.                                               |
| `security-local`                     | Same as `security`; pinned for local-only invocation.                                                 |
| `release-binary-smoke`               | Builds and verifies the pinned RedlineDB `v2.0.6` Linux release package, then runs a CLI smoke query. |
| `release`                            | `cargo build --workspace --release --locked`.                                                         |
| `jankurai-tools`                     | Local mirror for every `.github/workflows/jankurai-tools.yml` matrix job. Run with `scripts/ci-local.sh jankurai-tools`. |
| `pr-gate`                            | Local mirror for PR branch freshness plus `jankurai staged-gate` against `origin/main`. Run with `scripts/ci-local.sh pr-gate`. |

Lane definitions: `.jankurai/proof-lanes.toml`. To rerun a lane:

```
rtk just <lane-name>
```

(or invoke the command list from the TOML directly).

SQLite parity boundary: the official evidence flow lives in
[`docs/sqlite-parity.md`](docs/sqlite-parity.md), and `redline-testing-official`
is the only lane that produces committed parity evidence. RedlineDB does not
expose a local SQLite parity coverage/benchmark/report/sentinel producer; the
in-tree `sqlite_parity` commands and prior parity bundle workflows fail closed.
The proof-lane definitions and audit policy remain pinned in
`.jankurai/proof-lanes.toml` and `agent/audit-policy.toml`.

To test a locally built `redline-testing` tarball without editing CI pins, point
the installer at `file://` URLs:

```
  CI_REDLINE_TESTING_URL=file:///home/ubuntu/redline-testing/dist/redline-testing-0.1.3-linux-x86_64.tar.gz \
  CI_REDLINE_TESTING_SHA256_URL=file:///home/ubuntu/redline-testing/dist/redline-testing-0.1.3-linux-x86_64.tar.gz.sha256 \
  rtk just redline-testing-official
```

Set `REDLINEDB_SQLITE_PARITY_SQLITE_BIN=/path/to/sqlite3` to run
`redline-testing-official` against a pinned SQLite shell with optional shell and
extension features enabled. If unset, the official wrapper builds the SQLite
`3.53.1` autoconf shell through `scripts/sqlite/build-reference.sh` and exports
`target/sqlite-reference/3.53.1/bin/sqlite3` before comparing cases. The
builder verifies the upstream SHA3-256 digest and smokes percentile, math,
FTS5, RTREE, DBSTAT, `generate_series`, and `uint` support.

For narrow repair loops, prefer the package-scoped lanes above over `fast` when the touched surface is already known. They stay deterministic without forcing a workspace-wide run.

The `beyond-postgres-reference` lane is self-contained locally:

```
rtk just beyond-postgres-reference
```

If `REDLINEDB_POSTGRES_URL` is set, the lane uses that database. Otherwise it
starts `${REDLINEDB_POSTGRES_IMAGE:-postgres:16-alpine}` with database
`redlinedb_beyond`, user `redlinedb`, password `postgres`, and local port
`${REDLINEDB_POSTGRES_PORT:-55432}`. The script waits for container health,
exports `REDLINEDB_POSTGRES_URL`, runs `beyond_sqlite_manifest` and
`beyond_postgres_reference`, then removes the container. Set
`REDLINEDB_POSTGRES_KEEP=1` to keep the local container for debugging.

To reproduce the PR-side jankurai failure mode before pushing, commit the
candidate changes and run:

```
rtk scripts/ci-local.sh pr-gate
```

That command fetches `origin/main`, applies the same branch-freshness check as
`.github/workflows/jankurai.yml`, then runs `ops/ci/jankurai-staged-gate.sh`
with `BASE_REF=origin/main`.

To reproduce the complete PR CI surface locally, run:

```
rtk scripts/ci-local.sh pr-ci
```

That command runs the same shared dispatchers used by `.github/workflows/ci.yml`:
`CI_FAST_STAGE=preflight`, each `tests` matrix shard, `CI_PARITY_STAGE=redline-testing-official`,
and `scripts/guard-official-evidence.sh`. It stops at the first failing local
job and preserves the underlying command output.

To run local mirrors for the broader PR workflow set, including dependency
review, branch freshness, staged jankurai gate, and the input-boundary FFI
cross-check, run:

```
rtk scripts/ci-local.sh all
```

## Budgets and kill-switches

Long-running bench lanes carry budgets and a kill-switch so an agent
can interrupt them without leaving the cluster wedged. The contract:

| Lane (class)                        | Max wall-clock | Max disk | Max syscalls (strace) |
|-------------------------------------|----------------|----------|------------------------|
| `phase9-smoke`                      | 5 min          | 1 GiB    | 5e7                    |
| `phase9-certification`              | 30 min         | 8 GiB    | 5e8                    |
| `phase9-xbabe1-certification`       | 60 min         | 32 GiB   | 2e9                    |
| `phase9-xbabe1-certify-with-strace` | 90 min         | 32 GiB   | 4e9 (strace overhead)  |
| `phase10-xbabe1-certification`      | 90 min         | 64 GiB   | 4e9                    |
| `phase11-oltp-gap`                  | 20 min         | 4 GiB    | 1e8                    |

These budgets are authored in `.jankurai/cost-budget.toml` (added in
Section H of the repair plan); this doc is the human-readable index.

### `REDLINEDB_BENCH_KILL=1`

The bench harness honors the `REDLINEDB_BENCH_KILL` environment
variable: when set to `1` before a bench process starts, the harness
exits cleanly at the next workload boundary, flushes its in-flight
metrics, and writes a `kill_receipt.json` next to the run's output
directory. This is the supported way to abort a long bench run
without losing the partial evidence already gathered. The variable is
read once at harness startup; flipping it mid-run does not interrupt
an in-flight workload (use SIGTERM for that).

Section H of the jankurai-repair plan lands the implementation; this
doc fixes the contract so downstream tooling can rely on the variable
name today.

## Structured errors and repair receipts

Failures inside the kernel and downstream crates escalate into a
typed exception surface defined at
`crates/domain/src/error.rs::DomainError`. Every `DomainError`
carries six fields:

- `purpose` — a `module.subsystem.event` triple naming where the
  failure occurred (e.g. `kernel.storage.invalid_checksum`).
- `reason` — a one-sentence human explanation suitable for logs.
- `common_fixes` — a `&'static [&'static str]` of grep-able repair
  hints the next agent can scan without rereading the source.
- `docs_url` — the in-repo doc path that explains the dimension this
  failure belongs to (usually `docs/audit-rubric.md#<dimension>`).
- `repair_hint` — the specific proof lane to rerun.
- `source` — the underlying `Box<dyn Error + Send + Sync>` so the
  full causal chain stays attached.

The canonical escalation example lives at
`crates/kernel/src/error.rs::Error::into_domain` for the
`InvalidChecksum` variant. The unit tests in both crates (`cargo test
-p redlinedb-domain` and `cargo test -p redlinedb-kernel`) assert the
field shape so renames stay safe.

Authoring a new escalation:

1. Add the kernel/SQL/FFI variant to the relevant `Error` enum as
   usual.
2. Extend that crate's `into_domain` (or write one if it does not
   exist) to wrap the variant via
   `DomainError::new(...).with_source(self)`.
3. Add a unit test that asserts each of the six fields and the
   source chain.
4. Link the new failure under the relevant dimension in
   `docs/audit-rubric.md`.

A `proof-receipt.md` template lives at
`.jankurai/proof-receipt-template.md`; use it to record the lane name,
seed, raw-log path, and exit code for any non-trivial repair.

## Cost budgets and kill-switches

Every bench / certification workload is enumerated in
[`.jankurai/cost-budget.toml`](../.jankurai/cost-budget.toml) with three
hard limits — `max_wall_clock_minutes`, `max_disk_gb`,
`max_syscalls` — and a single `owner` field. The TOML is the
machine-readable source of truth; the table earlier in this file is
the human-readable summary.

The kill-switch contract: set `REDLINEDB_BENCH_KILL=1` before
launching (or `export` mid-run) and the bench harness exits at the
next workload boundary, flushes its in-flight metrics, and writes a
`kill_receipt.json` next to the run's output directory. The env var
name is fixed under `[global].kill_switch_env` in
`.jankurai/cost-budget.toml` so downstream tools can read the contract
without hardcoding the string. Each kill switch and spend cap ceiling
is defined per-workload in `.jankurai/cost-budget.toml` so the bench
harness and CI both enforce the same limits.

Adding a new long-running workload:

1. Append a `[[workload]]` block to `.jankurai/cost-budget.toml` with
   the three budgets and an owner.
2. Update the "Budgets and kill-switches" table above with the
   summary row.
3. Make the bench binary honor `REDLINEDB_BENCH_KILL` at the same
   poll boundary other workloads use (today: each repetition tick).

Audit reference: HLT-026 cost-budget-gap.

## Release readiness — launch-gate evidence

Test evidence rolls into the release-readiness gate documented in
[`docs/release.md`](release.md). The launch gates that every
tagged release must satisfy:

- **Security** — `just security` (cargo audit, cargo deny,
  gitleaks) green; the `security` job in
  `.github/workflows/jankurai.yml` blocks the PR otherwise.
- **Backups** — kernel `Engine::backup` integration test green
  (`cargo test -p redlinedb-kernel backup`); restore round-trip
  proven by the failpoint matrix lane.
- **Monitoring** — bench `kill_receipt.json` plus
  `.jankurai/repo-score.json` archived per release; the
  audit upload step in `jankurai.yml` is the canonical artifact.
- **Rollback** — `gh release delete` + `cargo yank` runbook in
  `docs/release.md`; `release-bad-behavior` lane in
  `.jankurai/proof-lanes.toml`.
- **Abuse controls** — FFI input boundary tests
  (`cargo test -p redlinedb-ffi shell`) plus the authz matrix lane
  cover misuse of the C ABI from untrusted callers.

These five gates fulfill the audit's `release readiness` evidence
requirement (HLT-025). The release-process steps themselves live
in `docs/release.md`; this section is the testing-side index.

## Budgets, quotas, stop conditions, and kill-switches for paid operations

Canonical source: [`.jankurai/cost-budget.toml`](../.jankurai/cost-budget.toml).
The TOML is machine-readable truth; this section is the agent-facing
operations index for the gates in that file. Audit reference:
HLT-026 cost-budget-gap.

**Scope:** every paid or unbounded operation in this repo (benchmarks,
chaos workloads, CI jobs that fan out matrices) is bounded by an
explicit budget, a quota, a stop condition, and a kill-switch.

- **Max wall-clock per bench run.** Aggregate CI cap is
  `[bench].max_wall_clock_seconds = 1800` (30 minutes). Per-workload
  caps live in each `[[workload]]` block as `max_wall_clock_minutes`
  and bound a single invocation.
- **Max CI concurrent jobs.** `[bench].max_ci_concurrent_jobs = 4`.
  CI matrices that fan out wider than this must shard explicitly or
  serialize behind a job-level `concurrency:` key.
- **Kill-switch (CTRL-C / timeout).** Set `REDLINEDB_BENCH_KILL=1`
  before launching (or `export` mid-run) and the bench harness exits
  at the next workload boundary, flushes in-flight metrics, and
  writes `kill_receipt.json` next to the run output. The env var
  name is fixed under `[global].kill_switch_env`. For hard kills
  use `timeout <seconds> just <lane>` to bound wall-clock from the
  shell side, or `Ctrl-C` (SIGINT) to interrupt the current
  workload iteration.
- **Dry-run a benchmark without exceeding the budget.** Use the
  lowest-rep certify (e.g. `just phase9-smoke`, or
  `cargo run -p redlinedb-bench -- certify --config <toml>
  --seed 7 --repetitions 1 --warmup 0`) with `REDLINEDB_BENCH_KILL=1`
  pre-exported to force exit at the first iteration boundary; the
  resulting `kill_receipt.json` confirms the wiring without paying
  the full budget. Always inspect the matching `[[workload]]` block
  in `.jankurai/cost-budget.toml` before launching a longer run.
- **Quotas (dependency + license).** `[dependencies]` in the budget
  file pins `max_advisory_count = 0` and a license allowlist; any
  PR that introduces a new vulnerable or non-allowlisted dependency
  is rejected by `cargo audit` + `cargo deny` in the security lane.
- **Paid operations register.** All CI lanes that bill compute time
  (bench matrices, cross-engine certification, xbabe1 runs) declare
  a `max_wall_clock_seconds` in their `[[workload]]` block and a
  kill-switch env var. There are no unbounded paid operations in
  this repo; if one is added it must register a budget + stop
  condition here and in `.jankurai/cost-budget.toml` before merging.
