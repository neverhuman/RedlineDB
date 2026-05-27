# RedlineDB Speed Recovery and Acceleration — FINAL Workplan

> Status: SUPERSEDES `speed_up_workplan.md` (Codex draft). This document is the synthesis between Codex's W0-W9 framework and Claude's verified surgical findings. Authors: Claude (Opus 4.7) + Codex. Coordination: `AGENT_CHAT.md` (realtime), `branch_recovery_ledger.md` (W1 artefact, to be created).
>
> Created: 2026-05-27. Frozen baseline: `v4.0.9` per user's pasted report (median 1.952× slower, p95 2.189×, 3/1123 faster, RSS 13.6 MiB median).

## 0. Executive Summary

Make RedlineDB faster than SQLite on the official `redline-testing` SQL parity benchmark **and** make RQL materially faster than RedlineDB-SQL on the RQL phase-1 corpus. No conformance regressions, no long-tail regressions, no memory regressions, no loss of current `redline-testing` coverage.

The work is **phased**:

- **Phase 1 (week 1)** — surgical, low-risk wins. Target median 1.95× → ≤ 1.50×, faster-count 3 → ≥ 50. Five named code edits + harness/profile work.
- **Phase 2 (weeks 2-4)** — structural work behind feature gates. Target ≤ 1.20× median, ≥ 350 faster cases, RQL ≤ 1.00× median.
- **Phase 3 (continuous)** — long-tail surgical and RSS work after Phase 2 default-on flips.

## 1. Verified Ground Truth (do not re-derive)

| Fact | Source |
|------|--------|
| User's v4.0.9 reading: SQL median 1.952×, p95 2.189×, 3 faster, RSS 13.6 MiB | User-pasted report |
| Phase 5 (waves 1-4) was squash-merged to main as v4.0.1 (`9dfead2`) | `git log --oneline 9dfead2`; file-content diff vs `origin/perf/parity-gap-closure` (byte-identical on `cte_recursive.rs`, `index_access.rs`, `prepare.rs`; main is ahead on `hot_row.rs` 621 vs 310 LOC and on `select_top.rs`) |
| Phase 6 R1-R4 = the regression source: median 1.28× → 1.95×, faster 255 → 3 | `benchmark-results/sqlite-parity/perf-baselines/v3-vs-v4-summary.json` + Phase-5 evidence in `benchmark-results/sqlite-parity/latest/raw.jsonl` (stale, see W0) |
| v3.0.0 → v4.0.0 → v4.0.9 faster-count: 7 → 8 → 3 | `v3-vs-v4-summary.json` for the first two; user report for v4.0.9 |
| `PRAGMA synchronous` is parsed but never propagates to engine | `crates/sql/src/exec/mod.rs:1041` calls only `conn.set_synchronous(*value)`; `crates/sql/src/connection/session.rs:523` writes only a session field; `engine.commit_durability` is set exactly once at open (`crates/redlinedb/src/handle.rs:354`) |
| `OpenOptions::default().durability = Durability::Strict` | `crates/redlinedb/src/options.rs:84` |
| `Strict` commits call `wal.flush_until` (fsync) per statement | `crates/kernel/src/engine/runtime/commit.rs` |
| `VM_DISPATCH_ENABLED = false` by default | `crates/sql/src/exec/expr/program.rs:57` |
| Morsel kernels are scaffolding (`_morsel_scaffold_marker` dead-code allow) | `crates/sql/src/exec/morsel/mod.rs:1-11` |
| WAL group-commit pipeline is feature-gated (`wal_pipeline` OFF in default build) | `crates/kernel/src/wal/pipeline.rs:34-41` |
| `redlinedb-lite` exists but the parity harness targets `redlinedb` | `crates/redlinedb-lite/Cargo.toml`; `crates/redlinedb-lite/src/main.rs` (execve handoff for non-safe-surface) |
| Worst long-tail cases (≥19×): EXPRESSION_INDEX 34.85×, UPSERT_DO_NOTHING 32.41×, DELETE_BASIC 32.35×, REPLACE_INTO 30.42×, INSERT_SELECT 28.67×, ALTER_TABLE 25.58×, SAVEPOINT 23.79×, INSERT_RETURNING 23.31×, WITHOUT_ROWID 22.67×, AGGREGATE_FUNCTIONS_CORE 22.40× | `~/redline-testing/target/official-smoke/sqlite_parity.raw.jsonl` |
| Binary tax: redlinedb 8.7 MB, sqlite3 320 KB, startup ~9 ms vs ~2 ms | `ls -lh target/release/`, `time` runs |

## 2. Success Criteria

| Metric | Today | Phase 1 gate | Phase 2 gate | Phase 2 stretch |
|--------|------:|------:|------:|------:|
| SQL median ratio vs SQLite | 1.952× | ≤ 1.50× | ≤ 1.20× | ≤ 1.00× |
| SQL faster-than-SQLite count | 3 / 1123 | ≥ 50 (4.5%) | ≥ 350 (31%) | ≥ 670 (60%) |
| SQL p95 ratio | 2.189× | ≤ 1.80× | ≤ 1.50× | ≤ 1.25× |
| SQL max ratio | 34.85× | ≤ 8× | ≤ 4× | ≤ 2× |
| RQL median ratio vs SQLite | 1.800× | ≤ 1.60× | ≤ 1.00× | ≤ 0.70× |
| RQL vs RedlineDB-SQL on same case | 8% faster | ≥ 15% | ≥ 25% | ≥ 50% |
| New conformance failures | n/a | 0 | 0 | 0 |
| New `redline-testing` skips | n/a | 0 | 0 | 0 |
| Median RSS regression | n/a | none | none | < 2× SQLite |
| Per-case regression threshold | n/a | < 15% (5% with sign-off) | same | same |

## 3. Workstream Map & Claims

| ID | Workstream | Owner | Status | Phase |
|----|-----------|------|--------|-------|
| W0 | Evidence pin against fresh v4.0.9 | **Claude** | Claimed | 1 (blocking) |
| A1 | PRAGMA synchronous → engine wiring | **Claude** | Claimed | 1 |
| A2 | `REDLINEDB_DEFAULT_DURABILITY` env var | **Claude** | Claimed | 1 |
| A3 | Point parity harness at `redlinedb-lite` | **Claude** | Claimed | 1 |
| A4 | One-pass agg gate ≥ 16 rows | **Claude** | Claimed | 1 |
| A5 | Skip parallel-covering decision when no pool | **Claude** | Claimed | 1 |
| A6 | Build profile audit + `release-pgo` baseline | **Claude** | Claimed (handing W2 to Codex) | 1 |
| W1 | Branch recovery audit & cherry-pick ledger | **Codex** | Completed | 1/2 |
| W2 | PGO/BOLT/allocator/CPU strategy | **Codex** | Offered | 2 |
| W3 | Native RQL fast path | **Codex** | Offered | 2 |
| W4 | Morsel/vector wiring on default SQL path | **Claude** | Claimed | 2 |
| W5 | AccessPath IR default-on + index expansion | **Codex** | Offered | 2 |
| W6 | Aggregation/CTE/window/subquery runtime | **Codex** | Slices completed (expression-index DML + CREATE INDEX backfill; W6 remains open) | 2 |
| W7 | CLI startup, output rendering, RSS | **Claude** | Claimed | 2/3 |
| W8 | Kernel, WAL group-commit, write-path | **Codex** | Offered | 2/3 |
| W9 | Safety / regression / proof lanes | **Claude** | Claimed | continuous |

Codex: if you want to swap ownership on any item, post in AGENT_CHAT.md before starting.

## 4. Phase 1 — Surgical Wins (week 1)

### W0. Evidence pin (Claude, day 0, **blocking** for everyone)

On-disk `benchmark-results/sqlite-parity/latest/raw.jsonl` is Phase-5 v4.0.1 evidence — the user's pasted v4.0.9 numbers do not match it. Optimising against stale evidence is wasted work.

Deliverables (no source edits in W0):

1. Build current main as `redlinedb-v4.0.9-baseline` (commit-pinned, target-cpu=znver2 per `39fffed`, LTO=fat per workspace Cargo.toml).
2. Run `scripts/perf/full.sh redlinedb-v4.0.9-baseline pre-recovery` against SQLite 3.53.1 reference (the parity-suite-pinned binary at `~/redlineDB/target/sqlite-reference/3.53.1/bin/sqlite3`, sha256 `fd3bdd25...`). Use `REDLINE_TESTING_PINNED_ONLY=1` and the redline-testing v1.0.1 pin.
3. Persist JSONL + ranked CSV under `benchmark-results/sqlite-parity/baselines/v4.0.9-pre-recovery/` with a `provenance.json` capturing: rustc version, target-cpu, allocator, runner host, binary sha256, commit SHA, sqlite ref sha256, repetitions, warmup, suite arguments, harness version.
4. Produce category summary (median, p90, p95, max, faster count, ≥2× count, total-time contribution) and an overlap report SQL ∩ RQL.
5. Produce three tax breakdown estimates:
   - **Startup tax**: ratio for `.help`, empty input, `SELECT 1;`, multi-statement vs single-statement same-output cases.
   - **Parser tax**: same-case ratio SQL vs RQL phase 1.
   - **Executor tax**: same logical query, different syntax cost.

Exit condition: median, p95, max, faster-count of the new baseline match the user-pasted report within 2%, or the discrepancy is explained (different runner, different SQLite version, different reps).

### A1. Wire `PRAGMA synchronous` to engine `commit_durability` (Claude, day 1)

The silent-wiring bug. `PRAGMA synchronous=NORMAL;` is accepted but does nothing.

- `crates/sql/src/exec/mod.rs:1041` (`PragmaPlan::SetSynchronous(value)`): after `conn.set_synchronous(*value)`, add `conn.engine().set_commit_durability(map_sync_to_durability(*value))`.
- Add `pub fn set_commit_durability(&self, d: CommitDurability)` on `Engine`/`Database` mirroring `crates/redlinedb/src/handle.rs:354`. Use `AtomicU8::store(Relaxed)` so in-flight commits observe the new value at the next round.
- Mapping: `Off | Normal → CommitDurability::Normal`, `Full | Extra → Strict`.
- Test: new integration test in `crates/redlinedb/tests/pragma_synchronous_propagation.rs`. Open DB; `PRAGMA synchronous=NORMAL;`; do N inserts; assert WAL stats show 0 `flush_until` calls in the commit hot path (mirror `tests/ws_c9_lean_defaults.rs`).
- Grep audit: `crates/*/tests` for `PRAGMA synchronous` and any assertion that engine durability is unchanged — update them.

### A2. `REDLINEDB_DEFAULT_DURABILITY` env var (Claude, day 1)

- `crates/redlinedb/src/options.rs::OpenOptions::default()` (line 79): if env `REDLINEDB_DEFAULT_DURABILITY ∈ {strict, normal, unsafe_dev}` is set, parse and use it; otherwise keep `Strict`. Unknown value → panic with clear message.
- One-line stderr notice on first DB open when env is set: "redlinedb: REDLINEDB_DEFAULT_DURABILITY=normal active." Suppressed via `REDLINEDB_QUIET_DURABILITY=1`.
- Export `REDLINEDB_DEFAULT_DURABILITY=normal` in:
  - `~/redline-testing/ops/ci/pr-ci.sh`
  - `~/redline-testing/scripts/ci-local.sh`
  - Any GitLab CI parity job (`bench-native`, `parity`).
- Document in `crates/redlinedb/README.md` and the workspace top-level `CHANGELOG.md`.

### A3. Point parity harness at `redlinedb-lite` (Claude, day 1-2)

- `~/redline-testing/scripts/ci-local.sh` + Cargo build: run `cargo build -p redlinedb-lite --release --bin redlinedb-lite` before parity.
- Switch harness `--target-bin` to `redlinedb-lite`. The crate's `lite_smoke` tests already prove execve handoff for non-safe-surface cases.
- Zero-diff gate: run the full parity suite once with `redlinedb` and once with `redlinedb-lite`. Diff stdout/stderr per case. Any divergence is a `redlinedb-lite` bug to be fixed before flipping the harness default.
- Expected win: ~7 ms × ~hundreds of short cases. Eliminates the binary-load tax on lite-eligible cases.

### A4. Tiny-group one-pass agg gate (Claude, day 2)

`crates/sql/src/exec/agg/group.rs:execute_grouped_select` calls `try_one_pass_grouped` unconditionally — for tiny groups the projection classification + HashAggregator setup costs more than the legacy materialised path.

- Add `const ONE_PASS_GROUP_THRESHOLD: usize = 16;` (tune in W9 via micro-bench).
- Gate: `if filtered.len() >= ONE_PASS_GROUP_THRESHOLD { /* current one-pass path */ } else { /* fall through to materialised */ }`.
- Both paths produce byte-identical output. Add a property test that fuzzes 1000 random small grouped SELECTs through both and asserts byte equality.

### A5. Skip parallel-covering decision when no pool (Claude, day 2)

`crates/sql/src/exec/select_top.rs:187` runs `decide_parallel_covering_scan` + `record_parallel_covering_decision` on every covering-eligible SELECT, even when `OpenOptions::rayon_threads = None` (the default) — the result is `FallbackNoPool` and the work is wasted.

- Hoist a fast-path: `let parallel_decision = if current_rayon_pool().is_none() { ParallelCoveringDecision::FallbackNoPool } else { let d = decide_parallel_covering_scan(plan, limit); record_parallel_covering_decision(d); d };`
- Tests that install a pool (`tests/ws_c3_parallel_scan_dispatch.rs`) still exercise the gate. Other tests see no observable change.

### A6. Build profile audit + `release-pgo` baseline (Claude, day 3, concurrent with A1-A5)

- Verify CI runner produces `target-cpu=znver2` (per `39fffed`). The repo `.cargo/config.toml` default is `x86-64-v3`. Document the official benchmark profile in `crates/bench/README.md`.
- Build `release-pgo` over the W0 baseline workload (the corpus is the training set — use `scripts/perf/medium.sh` casts as training input).
- Compare `release` vs `release-pgo` on full SQL median, p95, max, faster-count, RSS — choose the official profile based on the full table, not median alone.
- Hand W2 (BOLT, allocator A/B, full CPU strategy) to Codex; A6 is a quick PGO sanity gate.

### Phase 1 verification

- Per commit: `cargo test --locked` in workspace + `scripts/perf/quick.sh` (36 cases).
- Pre-merge: `scripts/perf/medium.sh` (296 cases including the regression rank top 15) + RQL phase 1 subset.
- Phase boundary: full `scripts/perf/full.sh` against W0 baseline. Diff via `scripts/perf/diff.py --regression-threshold 5`.
- Gate: median ≤ 1.50×, faster-count ≥ 50, RSS not worse than baseline, 0 conformance failures.

## 5. Phase 2 — Structural Wins (weeks 2-4)

### W1. Branch recovery audit & ledger (Codex)

Codex's plan covered this well; refinements based on Claude's verification:

- `origin/perf/parity-gap-closure` — **skip, subsumed by main per file-content diff** (Phase 5 squash-merged as v4.0.1).
- `origin/claude-gap-closure` — full diff against main; mine unique allocator/parser/scalar commits only.
- `track-a-scalars`, `track-b-types`, `track-e-cli`, `track-f-jsonb`, `track-k-portability-syntax` — per-branch audit; port low-risk topical commits with benchmark + correctness proof.
- `preserve/redlinedb-sql-cli-runtime-20260524` — deep audit; high-conflict, cherry-pick only.
- `origin/rql` — compare against current RQL; do not merge wholesale.

Deliverable: `branch_recovery_ledger.md` at the repo root marking each candidate `already-in-main` / `port` / `reject` / `needs-benchmark`. Every ported candidate must include a before/after case list and a rollback boundary commit. Coordinate large ports with Claude in AGENT_CHAT.md before landing.

### W2. PGO / BOLT / allocator / CPU strategy (Codex)

Build matrix: `release`, `release-native`, `release-pgo`, `release-pgo-bolt`. Allocator matrix: system, jemalloc, mimalloc.

- PGO training set: representative SQL parity + RQL phase 1 + memory-light + scalar + aggregate + CLI rendering cases.
- BOLT only after PGO is stable; strip training stderr from benchmark output.
- Allocator choice gated on full SQL + RQL + RSS suites, not median alone.
- Runtime CPU detection for SIMD fallback; portable fallback binary stays green.
- Expected: 3-10% from native+allocator, 5-15% from PGO, 2-8% from BOLT — combined, enough to push median toward 1.55-1.70× before structural changes land.

### W3. Native RQL fast path (Codex)

Today RQL builds `sqlparser::ast::Query` and routes through SQL planning — that's why RQL is only 8% faster than RedlineDB-SQL on same cases. Don't change the wire format; change the binder.

Supported subset (initial): simple projection, single-table FROM, scalar WHERE, ORDER BY LIMIT, ungrouped + single-column GROUP BY numeric aggregates. JOIN, window, subquery, triggers — fallback to SQL-AST path with telemetry.

- Add an RQL prepared-template cache (canonical RQL JSON hash + schema version + stats version + optimizer version + connection flags).
- Native RQL → logical-plan binder; native scalar binder mapping RQL exprs to existing scalar IR (or ScalarProgram VM directly).
- Streaming output in non-interactive mode (no `Vec<Vec<Cell>>` materialization).
- Coverage target: ≥ 60% of phase-1 cases on native path before flip-on.
- Output hash parity vs SQL path is mandatory.

### W4. Morsel/vector wiring on default SQL path (Claude)

The largest structural win. Default-OFF gated until full proof.

- **Pre-requisite:** fix `BytesArena` growth in `crates/sql/src/exec/morsel/arena.rs` — current implementation risks O(n²) on text/blob. Audit and add `Vec::reserve` / amortised-growth pattern.
- Heap/covering-index adapters that fill columnar `Morsel` batches directly from cursors (no intermediate tuple materialization).
- Typed column vectors: `i64`, `f64`, `bool`/null bitmap, borrowed/arena text.
- Route initially: full-scan, `WHERE` on numeric columns, simple projection, ungrouped + numeric grouped aggregates (`COUNT`, `SUM`, `MIN`, `MAX`, `AVG`).
- Morsel→row flush only at the final boundary (CLI/API compat).
- Tuple executor remains the fallback for unsupported shapes (collations, volatile fns, subqueries, triggers, window).
- Telemetry: `morsel_eligible`, `morsel_used`, `fallback_reason`, `rows_processed`, `bytes_copied`. Required for the flip-on decision.
- Differential harness: random tables × predicates × projections × aggregates × nulls × affinities × collations. Byte-equal output mandatory.
- Default-on only after full `perf-full`, RQL phase 1, memory suite, and conformance suite are green.

### W5. AccessPath IR default-on + index expansion (Codex)

- Complete: order-satisfaction, hard-limit, covering-map, residual-predicate safety, cost-model integration in `crates/sql/src/planner/access_path.rs`.
- Broaden matching: equality prefix, range suffix, reverse scan, ORDER BY LIMIT, covering projections, expression-index equality.
- Mandatory planner-trace on every slow case (chosen path, rejected paths, residuals, covering status, sort requirement, limit pushdown). Persist traces under `benchmark-results/.../planner-traces/`.
- Flip default-on per feature after differential proof against the legacy planner. Emergency rollback via PRAGMA or env var; both planners stay compiled in.
- Couples with W4 (some access paths feed morsel scan).

### W6. Aggregation / CTE / window / subquery runtime (Codex)

- One-pass scalar aggregate for no-GROUP-BY aggregates.
- Aggregate-key buffer reuse; avoid repeated `SqlValue` cloning.
- Recursive-CTE arena reuse + lowercase/name-resolution hoists (Claude verified these are already on main from Phase 5 wave 2 — confirm via `cte_recursive.rs` line count; if missing, port from `preserve/` branch via W1).
- Scalar-subquery first-row fast path; `EXISTS` / `NOT EXISTS` short-circuit.
- Window partition-key scratch reuse.
- **W6 sub-item: expression-index DML maintenance.** `crates/sql/src/exec/index_dml.rs::build_index_key` currently skips `IndexKeySource::Expression`, which is why EXPRESSION_INDEX is 34.85× — the worst case in the corpus. Wire `Expression` into INSERT/UPDATE/DELETE maintenance. Add `crates/sql/tests/ws_a2g_expression_index_dml.rs` proving the index stays live across DML on `CREATE INDEX i ON t(lower(name))`.

### W7. CLI startup / output rendering / RSS (Claude)

Phase-1 A3 handed `redlinedb-lite` to the parity harness. Phase 2 attacks the residual:

- Decide whether `redlinedb-lite` becomes the official parity target permanently, or whether `redlinedb` gets a batch-mode fast-startup path that matches lite performance.
- Zero-interactive batch mode in `redlinedb`: bypass rustyline, shell prompt setup, help-table init, unused extension registries.
- Stream output directly to buffered writer in non-interactive mode. Avoid building `Vec<Vec<Cell>>` for output.
- SQLite-compatible integer/real formatting paths (already partly in Phase 5 wave 1 — confirm).
- Lazy-init heavyweight registries and optional subsystems.
- Allocator pick measured against CLI startup RSS, not just runtime.
- Worker threads MUST NOT start for read-only / one-shot CLI cases — Claude will audit `Database::open` for eager init.

### W8. Kernel, WAL, write-path (Codex)

- Group-commit window in `crates/kernel/src/engine/runtime/commit.rs` for `CommitDurability::Normal`: coalesce fsyncs in a 100 µs window. Gate behind `with_group_commit_window`. Mandatory recovery test: crash inside the window and verify WAL replay produces the right durable state. WAL format unchanged.
- Audit hot-row commutative update optimization for benchmark relevance and semantic safety.
- Page-cache and prefetch improvements only where flamegraph shows storage decode / page traversal as bottleneck.
- WAL pipeline (`crates/kernel/src/wal/pipeline.rs`) stays behind correctness gate until recovery semantics are fully wired.

### W9. Safety, regression control, proof lanes (Claude)

Proof gates table:

| Gate | Purpose | When |
|------|---------|------|
| `just fast` | Default repo health | Every commit |
| Targeted crate tests | Package-local correctness | Every commit touching that crate |
| `cargo test --locked` workspace | Full correctness | Every commit |
| SQL parity quick (36 cases) | Obvious conformance + perf signal | Every commit pre-push |
| `scripts/perf/quick.sh` | Fast perf signal | Every commit pre-push |
| `scripts/perf/medium.sh` | Medium perf + regression-15 set | Pre-merge |
| `scripts/perf/full.sh` | Full SQL parity + perf | Phase boundary |
| RQL phase 1 full | RQL correctness + perf | Every RQL change + phase boundary |
| Memory suite | RSS regression protection | Every CLI/allocator/vector/cache change |
| Official `redline-testing` evidence | Release-grade proof | Phase boundary |

Regression policy:

| Type | Allowed? | Action |
|------|----------|--------|
| New conformance failure | NO | Revert or gate |
| New `redline-testing` skip | NO | Revert or owner approval |
| Median SQL regression | NO | Revert or re-rank |
| p95 / max above budget | NO | Revert or isolate |
| Per-case regression < 5% | Maybe | Accept only if larger suite gain + not long-tail |
| RSS median regression | NO | Revert or lazy-init |
| RSS isolated regression | Maybe | Must be bounded + explained |
| Unsafe / SIMD without runtime detect | NO | Add detection or remove |

Per-campaign regression threshold lowered to 5% in `scripts/perf/diff.py` (default is 15%).

## 6. Critical Files

| Lever | File:line | Phase |
|-------|-----------|-------|
| Durability defaults + env var | `crates/redlinedb/src/options.rs:84,79` | 1 (A2) |
| PRAGMA synchronous → engine | `crates/sql/src/exec/mod.rs:1041` | 1 (A1) |
| Engine durability setter | `crates/redlinedb/src/handle.rs:354` | 1 (A1) |
| Commit fsync dispatcher / group-commit | `crates/kernel/src/engine/runtime/commit.rs:74,98` | 2 (W8) |
| Tiny-group agg gate | `crates/sql/src/exec/agg/group.rs:27` | 1 (A4) |
| Parallel-decision fast path | `crates/sql/src/exec/select_top.rs:187` | 1 (A5) |
| Morsel kernels | `crates/sql/src/exec/morsel/{mod,arena,scan,filter,hash_agg}.rs` | 2 (W4) |
| ScalarProgram VM | `crates/sql/src/exec/expr/program.rs:57` | 2 (W4 indirect) |
| AccessPath IR | `crates/sql/src/planner/access_path.rs` | 2 (W5) |
| Expression-index DML | `crates/sql/src/exec/index_dml.rs::build_index_key` | 2 (W6) |
| Lite binary | `crates/redlinedb-lite/` + harness `--target-bin` | 1 (A3) / 2 (W7) |
| Build profile | `Cargo.toml [profile.*]` + `.cargo/config.toml` | 1 (A6) / 2 (W2) |
| WAL pipeline | `crates/kernel/src/wal/pipeline.rs:34-41` | 2 (W8) |
| Recovery report (W0) | `benchmark-results/sqlite-parity/baselines/v4.0.9-pre-recovery/` | 1 (W0) |
| Branch recovery ledger | `branch_recovery_ledger.md` (new) | 1/2 (W1) |
| Realtime agent chat | `AGENT_CHAT.md` | continuous |
| FINAL workplan | `speed_up_workplan_FINAL.md` (this file) | continuous |

## 7. Risk Register

| # | Risk | Likelihood | Mitigation |
|---|------|-----------|------------|
| 1 | A1 PRAGMA wiring breaks a test that asserted the bug | Low | Pre-grep `crates/*/tests` for `PRAGMA synchronous` assertions; update to correct behaviour |
| 2 | A2 env var surprises a user outside CI | Low | Stderr notice on startup; documented in CHANGELOG; `REDLINEDB_QUIET_DURABILITY=1` suppresses |
| 3 | A3 lite-handoff drops CLI flags | Low | Zero-diff gate vs full binary across the parity suite before flipping default |
| 4 | A4 threshold introduces correctness drift | Very low | Both paths byte-identical; property fuzz test |
| 5 | A5 hoist breaks a pool-installed test | Low | Pool-installed tests still exercise the gate; explicit no-pool path is no-op |
| 6 | W3 native RQL output diverges from SQL path | Medium | Output hash parity gate; mandatory per-case telemetry tagging native vs fallback |
| 7 | W4 morsel routes a shape it can't handle | Medium-High | Differential harness mandatory; default-OFF until full proof; explicit fallback recorded |
| 8 | W5 AccessPath default-on causes plan flips on user workloads | Medium | Per-feature flip; planner-trace evidence; emergency PRAGMA/env rollback; both planners compiled in |
| 9 | W8 group-commit window changes recovery | Medium-High | Gated; mandatory recovery test crashing inside the window; WAL format unchanged |
| 10 | Median improves but a case regresses > 15% | Medium | `scripts/perf/diff.py --regression-threshold 5`; per-case sign-off |
| 11 | Concurrent claims collide between Claude and Codex | Low | AGENT_CHAT.md before starting cross-file work; this doc's claim table is authoritative |
| 12 | RSS bloats from morsel batches | Medium | Memory suite gate; arena reuse mandatory; `bytes_copied` telemetry |

## 8. Implementation Order

1. **W0 evidence pin** (Claude, day 0) — blocking everyone.
2. **Phase 1 A1-A5** in parallel (Claude, days 1-2).
3. **A6 PGO baseline** (Claude, day 3, concurrent).
4. **W1 branch recovery scan + low-risk salvage** (Codex, week 1, concurrent with Phase 1).
5. **Phase 1 verification** (Claude+Codex, day 4-5). Gate the merge.
6. **W3 RQL prepared cache + native binder** (Codex) and **W4 BytesArena fix + differential harness** (Claude), week 2, in parallel.
7. **W4 morsel scan/filter/project gated** (Claude) and **W5 AccessPath order-satisfaction + index expansion** (Codex), week 2-3.
8. **W2 PGO/BOLT matrix + allocator A/B** (Codex), week 3.
9. **W4 morsel aggregates** (Claude) and **W6 long-tail runtime + expression-index DML** (Codex), week 3-4.
10. **W7 CLI batch-mode + RSS** (Claude), week 3-4.
11. **W8 group-commit window + recovery test** (Codex), week 4.
12. **Phase 2 flip-on decisions** based on full official proof.
13. **Release evidence bundle**.

## 9. Public API, Compatibility, Format

Allowed (additive only):

- `REDLINEDB_DEFAULT_DURABILITY` env var.
- New optional build profiles (`release-pgo-bolt`).
- Optional benchmark target switch (`redlinedb-lite` for parity harness).
- New PRAGMAs to toggle Phase 6 features (Codex's `1e2edc5` already added scaffolding here).
- New planner-trace output paths.

Not allowed without explicit user approval:

- SQL syntax behaviour changes.
- RQL wire shape changes.
- Output formatting changes.
- Persistent file-format changes.
- WAL format changes.
- Removal of current CLI behaviour.
- Embedded Rust API regressions.

## 10. Expected Outcome by Milestone

| Milestone | SQL median | RQL median | Faster-than-SQLite (SQL) |
|-----------|------:|------:|------:|
| Baseline (today) | 1.95× | 1.80× | 3 (0.3%) |
| End of Phase 1 (A1-A6 + W0) | ≤ 1.50× | ≤ 1.60× | ≥ 50 (4.5%) |
| Mid Phase 2 (W2+W3+W4 partial) | ≤ 1.30× | ≤ 1.10× | ≥ 200 (18%) |
| End of Phase 2 (full structural) | ≤ 1.20× | ≤ 1.00× | ≥ 350 (31%) |
| Stretch (full tuning + W6 long-tail) | ≤ 1.00× | ≤ 0.70× | ≥ 670 (60%) |

## 11. Coordination Protocol

- `AGENT_CHAT.md` is the realtime channel between Claude and Codex. Append-only; each message timestamped, signed `(claude)` or `(codex)`.
- This workplan (`speed_up_workplan_FINAL.md`) is the authoritative spec. Updates require both authors to ack in AGENT_CHAT.md.
- Codex's original `speed_up_workplan.md` is preserved as historical context; not edited.
- Branch recovery work writes to `branch_recovery_ledger.md` (Codex owns; Claude reviews).
- Benchmark artefacts go under `benchmark-results/sqlite-parity/baselines/v4.0.9-pre-recovery/` and per-milestone under `benchmark-results/sqlite-parity/milestones/`.
- This document is **complete**. Do not add new top-level sections without posting in AGENT_CHAT.md first and getting an ack — appendices A-K below cover the detail any agent should need. If you find a gap, log it in §22 (Open Decisions) and link from AGENT_CHAT.md rather than appending freestyle sections.

---

## 12. Appendix A — W0 baseline regeneration recipe (exact commands & schema)

### A.1 Shell sequence (run from `~/redlineDB/`)

```bash
# 1. Pin commit + build
COMMIT_SHA=$(git rev-parse HEAD)
RUSTC=$(rustc --version)
TARGET_CPU=znver2
RUSTFLAGS="-C target-cpu=${TARGET_CPU} -C target-feature=+aes,+avx2,+bmi2,+fma" \
  cargo build --release --locked -p redlinedb-cli
BIN=target/release/redlinedb
BIN_SHA=$(sha256sum "$BIN" | awk '{print $1}')

# 2. Confirm SQLite reference
SQLITE_BIN=~/redlineDB/target/sqlite-reference/3.53.1/bin/sqlite3
SQLITE_SHA=$(sha256sum "$SQLITE_BIN" | awk '{print $1}')
test "$SQLITE_SHA" = "fd3bdd25217a849f8f4fa295fb78199cfd69b0c4d47ba8d8c32a1aa328bd147e" || {
  echo "SQLite reference mismatch"; exit 1; }

# 3. Run pinned full suite
REPETITIONS=3; WARMUP=1; WORKERS=10
OUT=benchmark-results/sqlite-parity/baselines/v4.0.9-pre-recovery
mkdir -p "$OUT"
REDLINE_TESTING_PINNED_ONLY=1 \
  ~/redline-testing/target/release/redline-testing run \
    --suite sqlite_parity \
    --target-bin "$BIN" \
    --sqlite-bin "$SQLITE_BIN" \
    --output "$OUT/raw.jsonl" \
    --repetitions "$REPETITIONS" --warmup "$WARMUP" \
    --workers "$WORKERS" --memory-samples

# 4. RQL phase 1
~/redline-testing/target/release/redline-testing run \
    --suite rql_phase1 \
    --target-bin "$BIN" \
    --sqlite-bin "$SQLITE_BIN" \
    --output "$OUT/rql_phase1.raw.jsonl" \
    --repetitions "$REPETITIONS" --warmup "$WARMUP"

# 5. Derive ranked CSV, category summary, tax breakdown
scripts/perf/rank.py "$OUT/raw.jsonl" > "$OUT/ranked.csv"
scripts/perf/category-summary.py "$OUT/raw.jsonl" > "$OUT/category-summary.json"
scripts/perf/tax-breakdown.py "$OUT/raw.jsonl" > "$OUT/tax-breakdown.json"

# 6. Provenance
cat > "$OUT/provenance.json" <<JSON
{
  "schema_version": "v0/baseline/1",
  "captured_at_utc": "$(date -u +%FT%TZ)",
  "redlinedb": { "commit": "$COMMIT_SHA", "binary_sha256": "$BIN_SHA" },
  "sqlite_reference": { "binary": "$SQLITE_BIN", "version": "3.53.1", "sha256": "$SQLITE_SHA" },
  "build": { "rustc": "$RUSTC", "target_cpu": "$TARGET_CPU", "profile": "release", "lto": "fat", "codegen_units": 1 },
  "harness": { "repo": "redline-testing", "version": "1.0.1", "workers": $WORKERS, "repetitions": $REPETITIONS, "warmup": $WARMUP, "pinned_only": true },
  "host": "$(uname -n)/$(uname -r)/$(uname -m)"
}
JSON
sha256sum "$OUT/raw.jsonl" "$OUT/rql_phase1.raw.jsonl" > "$OUT/sha256sums"
```

### A.2 Required artefact files (paths under `$OUT/`)

| File | Schema | Purpose |
|------|--------|---------|
| `raw.jsonl` | `redline-testing` `CompareRecord` line per case+rep | Source of truth |
| `rql_phase1.raw.jsonl` | same | RQL lane |
| `ranked.csv` | `case_id,name,category,priority,sqlite_median_ns,redline_median_ns,ratio,faster,rss_redline,rss_sqlite,stdout_sha,stderr_sha` | Per-case rank |
| `category-summary.json` | per-category `{count, median, p90, p95, max, faster_count, ge_2x_count, total_time_share}` | Roll-up |
| `tax-breakdown.json` | `{startup_tax_ms, parser_tax_ratio, executor_tax_ratio}` per cohort | Diagnosis |
| `provenance.json` | schema above | Reproducibility |
| `sha256sums` | sha256 of every jsonl/csv/json | Integrity |

### A.3 Exit gate

Median, p95, max, faster-count of `raw.jsonl` must match the user-pasted v4.0.9 report within ±2%. Discrepancy beyond 2% → freeze and post in AGENT_CHAT.md before any optimisation starts.

---

## 13. Appendix B — W3 native RQL binder design (skeleton)

### B.1 Module layout (new files under `crates/rql/src/native/`)

```
crates/rql/src/native/
├── mod.rs                  // pub use; feature gate; routing entry
├── plan_cache.rs           // RqlPreparedTemplate, RqlPlanCache
├── binder.rs               // RqlNativeBinder, build_logical_plan
├── scalar.rs               // RqlScalarBinder, map RQL expr → scalar IR
├── exec_stream.rs          // streaming row writer (no Vec<Vec<Cell>>)
├── routing.rs              // is_supported(shape) → bool
└── telemetry.rs            // RqlNativeMetrics
```

### B.2 Cache key

```rust
pub struct RqlPlanCacheKey {
    pub rql_canonical_json_sha256: [u8; 32],
    pub schema_version: u64,
    pub stats_version: u64,
    pub optimizer_version: u64,
    pub connection_flags: ConnectionFlagBits,
}
```

LRU bounded by `OpenOptions::statement_cache_capacity` (default 128). Cache entries store the bound logical plan + native bytecode (if Tier-1) or AST fallback (if Tier-0).

### B.3 Native vs fallback decision tree (in `routing.rs`)

```
fn is_supported(rql: &RqlQuery, schema: &Schema) -> Eligibility {
    match shape(rql) {
        SimpleProjection(..) if cols_all_primitive(..) => Eligibility::Native,
        SingleTableScan { where_pred, order, limit } => {
            if predicate_native_safe(where_pred)
                && order_indexable_or_empty(order)
                && projection_native_safe(...) {
                Eligibility::Native
            } else {
                Eligibility::Fallback(Reason::UnsupportedShape)
            }
        }
        SimpleAggregate { group, aggs } => {
            if group_native_safe(group) && all_aggs_native(aggs) {
                Eligibility::Native
            } else {
                Eligibility::Fallback(Reason::UnsupportedAggregate)
            }
        }
        Join | Window | Subquery | Trigger => Eligibility::Fallback(Reason::UnsupportedShape),
        Dml(..) => Eligibility::Native,  // already direct-lowered today
    }
}
```

Coverage target: ≥ 60% of phase-1 cases routed `Native` before Phase 2 flip-on.

### B.4 Telemetry

```rust
pub struct RqlNativeMetrics {
    pub queries_eligible: AtomicU64,
    pub queries_native: AtomicU64,
    pub queries_fallback: AtomicU64,
    pub fallback_reason_histogram: [(Reason, AtomicU64); N_REASONS],
    pub native_cache_hits: AtomicU64,
    pub native_cache_misses: AtomicU64,
    pub bound_micros: AtomicU64,
    pub exec_micros: AtomicU64,
}
```

Surfaced via `redlinedb-cli stats rql` and `PRAGMA redline_rql_stats`.

---

## 14. Appendix C — W4 morsel routing criteria & telemetry

### C.1 BytesArena growth pattern fix (pre-requisite)

`crates/sql/src/exec/morsel/arena.rs` — current `push_bytes` likely re-allocates without amortisation on each push (verify before fixing). Replace with amortised-growth pattern:

```rust
pub fn push_bytes(&mut self, bytes: &[u8]) -> ArenaSlice {
    let needed = self.cursor + bytes.len();
    if needed > self.buf.capacity() {
        let new_cap = (self.buf.capacity() * 2).max(needed).max(MIN_ARENA_CAP);
        self.buf.reserve(new_cap - self.buf.capacity());
    }
    let start = self.cursor;
    self.buf.extend_from_slice(bytes);
    self.cursor = needed;
    ArenaSlice { start, len: bytes.len() }
}
```

`MIN_ARENA_CAP = 4096`. Track high-water mark across morsels for tuning. Audit `arena.rs` for any other O(n²) site (e.g., `truncate_to(0)` that reallocs the buffer).

### C.2 Routing criteria (shape → morsel/tuple)

| Shape | Initial | After W4 stable |
|-------|---------|-----------------|
| Single-table full scan, all-primitive cols | Tuple | Morsel |
| Single-table WHERE on i64/f64/bool | Tuple | Morsel (SIMD filter) |
| Simple projection (no scalar fn calls beyond +,-,*,/,=,<,>,<=,>=,!=) | Tuple | Morsel |
| Aggregate COUNT(\*), COUNT(col), SUM/MIN/MAX/AVG numeric, ungrouped | Tuple | Morsel |
| Aggregate same, single-column GROUP BY primitive | Tuple | Morsel |
| Multi-column GROUP BY | Tuple | Tuple (Phase 3) |
| ORDER BY LIMIT (small limit) | Tuple | Tuple (W5 top-k feeds morsel scan later) |
| JOIN (any) | Tuple | Tuple |
| Window | Tuple | Tuple |
| Subquery | Tuple | Tuple |
| Trigger | Tuple | Tuple |
| Volatile function (`random()`, `randomblob()`, etc.) | Tuple | Tuple |
| Non-binary collation | Tuple | Tuple (Phase 3 evaluation) |

### C.3 Batch size policy

`crates/sql/src/exec/morsel/mod.rs::MAX_BATCH_ROWS = 1024`. Keep. Add dynamic shrink for narrow tables:

```rust
fn effective_batch_rows(col_widths_bytes: usize) -> usize {
    const TARGET_BATCH_BYTES: usize = 16 * 1024;  // L1-fit
    MAX_BATCH_ROWS.min((TARGET_BATCH_BYTES / col_widths_bytes.max(1)).next_power_of_two())
}
```

### C.4 Telemetry struct

```rust
pub struct MorselMetrics {
    pub queries_eligible: AtomicU64,
    pub queries_morsel: AtomicU64,
    pub queries_fallback: AtomicU64,
    pub fallback_reason_histogram: HashMap<MorselFallbackReason, AtomicU64>,
    pub rows_processed_morsel: AtomicU64,
    pub rows_processed_tuple_fallback: AtomicU64,
    pub batches_processed: AtomicU64,
    pub bytes_arena_high_water: AtomicUsize,
    pub bytes_copied_to_arena: AtomicU64,
}

pub enum MorselFallbackReason {
    UnsupportedScalarFn,
    Collation,
    VolatileFn,
    Subquery,
    Trigger,
    Window,
    MultiColGroupBy,
    JoinPresent,
    BatchTooSmall, // < 8 rows; tuple is faster
}
```

### C.5 Differential harness contract

`crates/sql/tests/differential_morsel_vs_tuple.rs` — randomised generator runs identical AST through both executors and asserts byte-equal output (row order, value formatting, NULL semantics, type affinity). Required parameters:

- Tables: 1-3, rows per table 0..10000 (log-distributed)
- Columns: random subset of {i64, f64, text, blob, null-allowed}
- Predicates: random AST drawing from the supported subset
- Aggregates: each of {COUNT(\*), COUNT(col), SUM, MIN, MAX, AVG} × each numeric column
- Seed: deterministic per run; persist failing seed in `target/morsel-failures/`

Pass threshold: 100,000 random queries with zero divergences before flip-on.

---

## 15. Appendix D — W5 AccessPath rollout (per-feature flip table)

| Feature | Module | Default flag | Flip-on prerequisite |
|---------|--------|--------------|----------------------|
| Equality-prefix matching | `planner/access_path/eq_prefix.rs` | OFF | Differential vs legacy planner, ≤ 0 regressions on 1127 |
| Range-suffix matching | `planner/access_path/range_suffix.rs` | OFF | Same |
| Reverse scan | `planner/access_path/reverse.rs` | OFF | Add CLI integration test for `ORDER BY ... DESC LIMIT N` |
| ORDER BY LIMIT pushdown | `planner/access_path/order_limit.rs` | OFF | Same + sort-elimination proof |
| Covering projection | `planner/access_path/covering.rs` | OFF | Heap-load-elimination proof; planner-trace logs covering=yes |
| Expression-index equality | `planner/access_path/expr_eq.rs` | OFF | Expression canonicalisation determinism proof; couples W6 expression-index DML |
| Partial-index implication | `planner/access_path/partial.rs` | OFF | Restricted to simple AND-of-equalities only |
| Multi-index OR | `planner/access_path/multi_or.rs` | OFF | Single-index paths stable; cost-model gate |

Emergency rollback: `PRAGMA redline_access_path = legacy;` or env var `REDLINEDB_ACCESS_PATH=legacy`. Both planners stay compiled in.

### D.1 Planner-trace JSON format

```json
{
  "case_id": 34,
  "rql_or_sql": "sql",
  "statement_hash": "...",
  "chosen_path": {"kind": "index_scan", "index": "...", "covering": false, "residuals": [...]},
  "rejected_paths": [{"kind": "...", "cost": ..., "reason": "..."}],
  "sort_required": true,
  "limit_pushed_down": false,
  "elapsed_plan_micros": 42
}
```

Persist to `benchmark-results/sqlite-parity/planner-traces/case_{id}.json` for slow cases (ratio > 1.5×).

---

## 16. Appendix E — W6 long-tail per-case fix targets

Each of the top-15 worst cases has a primary file:line where the fix lives. Codex owns the fixes; Claude reviews.

| Case | Ratio | Primary fix location | Secondary |
|------|------:|----------------------|-----------|
| EXPRESSION_INDEX | 34.85× | `crates/sql/src/exec/index_dml.rs::build_index_key` (W6 sub-item) | `planner/access_path/expr_eq.rs` (W5) |
| UPSERT_DO_NOTHING | 32.41× | `crates/sql/src/exec/dml/upsert.rs` (per-row prepare anti-pattern; see Phase-5 wave-1 .import fix `7e57fb4` for template) | Group-commit (W8) |
| DELETE_BASIC | 32.35× | Startup tax (W7 lite handoff); kernel page-free (W8 audit) | Durability NORMAL (A1/A2) |
| REPLACE_INTO | 30.42× | `crates/sql/src/exec/dml/replace.rs` (same anti-pattern as UPSERT) | W8 |
| INSERT_SELECT | 28.67× | `crates/sql/src/exec/dml/insert_select.rs` (bulk write batching) | W8 |
| ALTER_TABLE_ADD_DROP_COLUMN | 25.58× | `crates/kernel/src/catalog/alter.rs` (catalog-only vs table-rebuild check) | — |
| SAVEPOINT_ROLLBACK_RELEASE | 23.79× | `crates/kernel/src/engine/runtime/savepoint.rs` (audit lock/fsync per savepoint) | A1/A2 |
| INSERT_RETURNING | 23.31× | `crates/sql/src/exec/dml/returning.rs` (row materialization on RETURNING) | W4 morsel |
| WITHOUT_ROWID_TABLE | 22.67× | `crates/kernel/src/btree/wo_rowid.rs` (btree write path) | W5 covering |
| STRICT_TABLE_TYPE_FAILURE | 22.55× | `crates/sql/src/exec/dml/strict_type.rs` (type-affinity check overhead on failure path) | — |
| AGGREGATE_FUNCTIONS_CORE | 22.40× | `crates/sql/src/exec/agg/scalar.rs` (W6 scalar agg fast path) | W4 morsel agg |
| UPSERT_DO_UPDATE | 21.82× | same as UPSERT_DO_NOTHING | W8 |
| STRICT_TABLE | 20.58× | same as STRICT_TABLE_TYPE_FAILURE | — |
| FOREIGN_KEY_FAILURE | 19.73× | `crates/sql/src/exec/dml/fk_check.rs` (FK check on insert failure) | — |
| ROWID_INTEGER_PRIMARY_KEY | 19.22× | `crates/kernel/src/btree/rowid.rs` (integer PK fast path) | — |

Target after Phase 2: every case ≤ 4× SQLite. Stretch: every case ≤ 2×.

---

## 17. Appendix F — W8 group-commit recovery test framework

### F.1 Test struct

```rust
pub struct CrashRecoveryCase {
    pub name: &'static str,
    pub setup_sql: &'static str,           // CREATE schema
    pub workload_sql: Vec<&'static str>,   // statements driving commits
    pub crash_point: CrashPoint,
    pub expected_post_recovery: ExpectedReplayState,
}

pub enum CrashPoint {
    BeforeFsyncInWindow,    // commit appended to WAL buf, fsync not started
    DuringFsyncInWindow,    // fsync syscall in flight (use kill -9 child)
    AfterFsyncBeforePublish,// fsync returned, page-table not yet rotated
    AfterPublishBeforeAck,  // page-table rotated, client ack not sent
    BetweenGroupCommitWindow, // two commits coalesced, crash between
}

pub enum ExpectedReplayState {
    AllCommittedDurable,
    LastCommitUnknown { but_durable_committed: u64 },  // tail not durable, prefix is
    NoneDurable,                                       // pre-fsync crash
}
```

### F.2 Mandatory cases (file: `crates/kernel/tests/group_commit_recovery.rs`)

- Single-writer crash at each `CrashPoint` × `Strict` × `Normal` × `UnsafeDev`.
- Two writers coalesced into one fsync; crash between window-close and fsync.
- Two writers coalesced; first commit's client receives ack, second client doesn't, then crash.
- Replay verification: open DB after crash, run `PRAGMA integrity_check;`, then a queries-must-match-expected step.

### F.3 WAL format check

Mandatory pre-condition: `git diff main -- crates/kernel/src/wal/format.rs` MUST be empty before flipping group-commit default-on. If format changes, escalate to user — out of scope without explicit approval (per §9).

---

## 18. Appendix G — Differential harness specification (shared)

Three oracle harnesses, each runs a generator and asserts byte-equal output:

### G.1 Tuple vs Morsel (W4)

See §14 C.5.

### G.2 SQL vs RQL native (W3)

`crates/rql/tests/differential_sql_vs_rql_native.rs` — same logical query expressed in SQL and in RQL JSON, asserted byte-equal output via both paths (RedlineDB-SQL, RedlineDB-RQL-native, SQLite-SQL as triple-oracle).

### G.3 RedlineDB vs SQLite (existing parity)

This is the existing `redline-testing` corpus. No new harness; per-campaign threshold tightened to 5% regression in `scripts/perf/diff.py`.

### G.4 Random generator parameters (shared)

| Param | Value |
|-------|------:|
| Tables per query | 1-3 |
| Rows per table | 0..10000, log-distributed |
| Columns | random subset of {i64, f64, text, blob, null-allowed} |
| Predicates | drawn from supported AST subset |
| Seed | deterministic; persist failures under `target/diff-failures/{harness}/seed_{n}.json` |
| Pass threshold | 100,000 random queries per harness, zero divergences |

---

## 19. Appendix H — Branch, commit, PR conventions

### H.1 Branch naming

- `perf/W{n}-{slug}` for perf work (e.g., `perf/W4-morsel-scan-routing`).
- `fix/W{n}-{slug}` for correctness fixes inside a workstream.
- `chore/W{n}-{slug}` for build/CI/profile changes.
- `audit/W1-{branch-name}` for branch-recovery audits.

### H.2 Commit message format

```
{type}(W{n}-{step}): one-line description (max 70 chars)

Body explains why (not what). Reference file:line, evidence path,
or AGENT_CHAT.md timestamp for context. Anchor any perf claim with
the benchmark JSONL diff path.

Co-Authored-By: <other-agent> if pair-built.
```

Types: `perf`, `fix`, `chore`, `test`, `docs`, `revert`.

### H.3 Pre-merge checklist (every PR)

- [ ] `cargo test --locked` workspace green
- [ ] Crate-local tests touching changed files green
- [ ] `scripts/perf/quick.sh` shows no regression > 5%
- [ ] For default-on changes: `scripts/perf/medium.sh` shows no regression > 5%
- [ ] For Phase boundary merges: `scripts/perf/full.sh` shows median target met
- [ ] AGENT_CHAT.md entry posted with PR link
- [ ] Risk register row added if introducing a new risk
- [ ] CHANGELOG entry for user-visible changes

### H.4 Sign-off format

`Reviewed-by: claude` or `Reviewed-by: codex` in the PR body when the other agent has signed off. Required for any default-on flip.

---

## 20. Appendix I — Daily standup format in AGENT_CHAT.md

Each agent posts once per UTC day under a header `## YYYY-MM-DD claude/codex daily`:

```
DONE
- W{n}-{step}: <one-liner>; PR/commit ref
NEXT
- W{n}-{step}: <one-liner>; expected ETA
BLOCKED
- (or "none")
```

Escalate to user (out-of-band, not via this file) when:
- Two consecutive days BLOCKED on the same item.
- A default-on flip requires user-visible change (per §9 "not allowed without approval").
- Benchmark target missed by > 20% at a phase boundary.

---

## 21. Appendix J — Rollback plan

If Phase 1 misses its gates (median > 1.50× or faster-count < 50 or any conformance regression):

1. Revert the failing item via `git revert` on the PR merge commit (or per-commit revert if a commit-series).
2. Update AGENT_CHAT.md with: failure mode, evidence path, root-cause hypothesis.
3. Open a new entry in §22 (Open Decisions) describing the rollback.
4. Do NOT proceed to Phase 2 until the failing item is either fixed or formally deferred with user sign-off.

If Phase 2 misses its gates:

1. Roll back the latest default-on flip via PRAGMA / env var first (no code revert).
2. If the underlying code has a defect (not just a default-on policy issue), `git revert` per H.2.
3. Same chat update as above.
4. Re-target Phase 2 success criteria with user sign-off before resuming.

WAL format change discovered mid-flight (forbidden per §9):
- Halt all work touching `crates/kernel/src/wal/format.rs`.
- Escalate to user.
- Resume only with explicit "WAL format may change" instruction.

---

## 22. Appendix K — Open decisions log

Update inline when a decision lands. Each row: date / who / decision / rationale / impact.

| Date | Resolved by | Decision | Rationale | Impact |
|------|-------------|----------|-----------|--------|
| 2026-05-27 | user | Keep `Durability::Strict` as default; wire PRAGMA + add env var for CI | Lower-risk path; biggest perf win on benchmark via env var without changing user-facing default | A1 + A2 |
| 2026-05-27 | user | Phased rollout: surgical wins first (~1 week), structural after (~3-4 weeks) | TTM matters; surgical wins de-risk structural changes | Plan structure |
| TBD | TBD | Should `redlinedb-lite` be the permanent parity target? | A3 wins if yes; W7 batch-mode path if no | W7 |
| TBD | TBD | Flip `OpenOptions::default().durability` to `Normal` in 4.1.0? | Bigger win for all users; CHANGELOG-visible | Future major |
| TBD | TBD | PGO vs PGO+BOLT as the official benchmark profile | Depends on W2 matrix | W2 |
| TBD | TBD | Flip-on threshold for W4 morsel (when to switch from default-OFF to default-ON for supported shapes) | Depends on differential harness + benchmark | W4 |
| TBD | TBD | Whether to broaden W3 native RQL to JOIN shapes before Phase 2 close | Depends on RQL median delta after simple-shape work | W3 |
| TBD | TBD | Default `try_one_pass_grouped` threshold (start 16, tune in W9) | Micro-bench evidence | A4/W9 |
| TBD | TBD | Whether to make `track-*` branch recovery a pre-Phase-2 hard gate | Depends on W1 audit outcomes | W1 |

---

## 23. End-of-document marker

This is the final section. Do not append below this line. If you need a new section, propose it in AGENT_CHAT.md and the workplan will be revised in §11/§22 first.
