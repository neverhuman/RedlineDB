# RedlineDB Speed Recovery and Acceleration Workplan

## Summary

Primary objective: make RedlineDB faster than SQLite on the official `redline-testing` SQL parity benchmarks and make RQL materially faster than SQL, without conformance regressions, long-tail regressions, memory regressions, or loss of current coverage.

Key finding: the earlier performance work was not simply "lost," but a lot of it is either not active on the benchmark path or still exists only on divergent branches. Phase 6 Morsel/vector, ScalarProgram VM, parallel scan, AccessPath IR, and WAL pipeline work are present in `main`, but much of it is gated, scaffolding-only, or not routed into default SQL/RQL execution. Several branches still carry unique speed/parity work that should be audited and selectively ported.

Important discrepancy: the repo's checked-in benchmark artifact appears older than the pasted `v4.0.9` report. The checked-in `latest` evidence refers to `v4.0.1` style data, while the user-provided numbers are `v4.0.9`, `1123` cases, median `1.952x` slower. First work must pin and import the exact failing evidence so agents optimize against the current truth.

## Current Evidence

| Area | Finding | Engineering implication |
|---|---|---|
| SQL benchmark | User report says SQL median is `1.952x` slower than SQLite, p95 `2.189x`, only `3/1123` faster | Current engine is broadly slower, not just bad at a few cases |
| RQL benchmark | RQL median is `1.800x` slower on `527` cases | RQL is only parser-bypass today, not a real faster execution path |
| Memory | Median RSS `13.6 MiB` vs SQLite `4.3 MiB`; worst startup-heavy case is extreme | Startup and CLI footprint are benchmark-visible |
| Branch history | `perf/parity-gap-closure`, `claude-gap-closure`, `track-*`, and `preserve/redlinedb-sql-cli-runtime-20260524` contain unique work | Need a recovery ledger before new architecture work |
| Phase 6 | Morsel/vector, ScalarProgram VM, AccessPath IR, parallel scan, WAL pipeline exist | Big speedups are latent until routing/gates/defaults are fixed |
| RQL | RQL select currently builds SQL AST and binds through normal SQL path | RQL cannot become dramatically faster without native RQL planning/execution |

## Success Criteria

| Metric | Required gate | Stretch target |
|---|---:|---:|
| SQL median ratio vs SQLite | `<1.20x` | `<1.00x` |
| SQL faster-than-SQLite cases | `>=35%` | `>=60%` |
| SQL p95 ratio | `<1.60x` | `<1.25x` |
| SQL max ratio | `<2.00x` | `<1.50x` |
| RQL median ratio vs SQLite | `<1.00x` | `<0.70x` |
| RQL vs Redline SQL same cases | `>=25% faster` | `>=50% faster` |
| Conformance | `0` new failures | `0` new skips |
| Memory median RSS | no regression | `<2x SQLite` |
| Official coverage | no loss of `redline-testing` pass coverage | expand coverage where safe |

## Parallel Workstreams

| ID | Owner lane | Goal | Expected impact | Dependency |
|---|---|---|---|---|
| W0 | Evidence | Pin exact benchmark truth and profiling targets | Prevent wasted optimization | None |
| W1 | Git recovery | Recover lost/divergent performance work | Fastest low-risk gains | W0 for ranking |
| W2 | Build/HPC | Make native, PGO, BOLT, allocator, and CPU features reproducible | `5-20%` broad gain if clean | W0 |
| W3 | RQL | Make RQL a native prepared-plan path, not SQL AST bypass | Biggest RQL-specific gain | W0 |
| W4 | Executor/vector | Wire Morsel/vector into real SELECT, filter, projection, aggregate paths | Biggest median SQL gain | W0 |
| W5 | Planner/index | Expand AccessPath/index coverage and avoid sort/materialization | Long-tail reduction | W0 |
| W6 | Aggregation/CTE/window | Remove row-clone and materialization hot paths | Long-tail and median gain | W0 |
| W7 | CLI/startup/memory | Cut per-case CLI startup, rendering, allocator, RSS overhead | Helps every official benchmark case | W0 |
| W8 | Kernel/write path | Route WAL pipeline/hot-row wins where benchmark-visible | Write-heavy and future workload gain | W0 |
| W9 | Safety/proof | Differential gates, perf gates, regression budgets | Required for no regressions | All |

## W0: Evidence and Benchmark Ground Truth

Create a benchmark evidence bundle before any optimization work.

Required actions:

- Capture exact `redline-testing` commit, RedlineDB commit, SQLite version, Rust version, target CPU, allocator, profile, binary hash, runner host, worker count, repetition count, warmup count, and suite arguments.
- Import or regenerate the `v4.0.9` SQL report, RQL phase 1 report, and memory report into a non-generated analysis artifact.
- Produce ranked CSVs for SQL, RQL, and memory with columns: case id, category, name, SQLite median, Redline median, ratio, RSS, stdout hash, stderr hash, target binary hash.
- Produce category summaries for median, p90, p95, max, faster count, `>=2x` count, and total time contribution.
- Produce overlap report showing which SQL slow cases also exist in RQL phase 1.
- Produce startup tax estimate by comparing empty input, `.help`, `SELECT 1`, and multi-statement cases.
- Produce parser tax estimate by comparing SQL and RQL same-case pairs.
- Produce executor tax estimate by grouping same logical query shape with different syntax cost.
- Produce memory tax estimate by separating CLI startup RSS from query heap growth.

Acceptance criteria:

- The workplan has a frozen baseline table matching the pasted `v4.0.9` numbers or explicitly explains any mismatch.
- Every later workstream uses this baseline, not stale committed `v4.0.1` evidence.
- No source changes are made in W0 except generated benchmark/report artifacts if execution mode later permits it.

## W1: Git Recovery and Lost Work Audit

Treat divergent branches as a salvage queue, not merge targets.

High-priority branches:

| Branch | Status | Action |
|---|---|---|
| `origin/perf/parity-gap-closure` | Contains Phase 5 speed-gap closure work not fully identical to `main` | Audit commit-by-commit and port isolated wins |
| `origin/claude-gap-closure` | Large divergent SQL/CLI/HPC branch | Mine only isolated allocator/hash/parser/scalar commits |
| `track-a-scalars` | Scalar/math/GLOB/format fixes | Port if absent and benchmark-positive |
| `track-b-types` | native/PGO profiles and type formatting | Port profile pieces first, semantic fixes only behind tests |
| `track-e-cli` | CLI output and dot-command rendering | Port rendering speedups if conformance-safe |
| `track-f-jsonb` | JSONB and aggregate/window changes | Mine JSON scalar/operator speedups separately |
| `track-k-portability-syntax` | AHash/string interning/HPC plus PG syntax | Port HPC commits only after hash determinism audit |
| `preserve/redlinedb-sql-cli-runtime-20260524` | Predicate, aggregate, top-k, CLI runtime work | Deep audit; likely valuable but high-conflict |
| `origin/rql` | Older RQL phase 1 support and docs | Compare with current RQL; do not merge wholesale |

Specific salvage candidates:

| Commit/topic | Expected value | Risk |
|---|---:|---|
| Parser allocation reduction | Broad SQL median | Medium |
| Function-name lowercase caching | Scalar-heavy cases | Low |
| Fromless SELECT fast path | Scalar and PRAGMA cases | Low |
| ASCII `LENGTH`/`UPPER`/`LOWER`, `INSTR` memmem | Scalar cases | Low |
| `itoa` CLI output | Output-heavy benchmark cases | Low |
| CTE lowercase hoist | Recursive CTE long tail | Low |
| Window scratch buffer reuse | Window long tail | Low |
| Expression-index equality matching | Index cases | Medium |
| `NOT INDEXED` and ORDER BY prefix fixes | Planner correctness and perf | Medium |
| PGO/BOLT scripts | Broad perf | Medium |
| Mimalloc/global allocator | Startup and allocation-heavy cases | Medium |
| AHash hot SQL maps | Planner/executor maps | Medium |
| String interning hot identifiers | Parser/planner/binder | Medium |
| Predicate runtime work from preserve branch | WHERE/filter cases | High |
| Top-k runtime work from preserve branch | ORDER BY LIMIT cases | Medium |
| Simple aggregate runtime work from preserve branch | Aggregate long tail | High |

Acceptance criteria:

- Create a branch recovery ledger with every candidate marked `already in main`, `port`, `reject`, or `needs benchmark`.
- No branch is merged wholesale.
- Every ported candidate has a before/after case list and a rollback commit boundary.
- Any semantic change must pass targeted conformance before perf comparison.

## W2: Build, Profile, Allocator, and CPU Strategy

Make the official benchmark binary as optimized as possible without changing semantics.

Required changes:

- Standardize benchmark profiles: `release`, `release-native`, `release-pgo`, and `release-pgo-bolt`.
- Ensure official native CI uses the intended CPU target consistently. Current config should be checked against the pasted `znver2` claim because repo config defaults looked closer to generic `x86-64-v3`.
- Add a reproducible PGO training flow using representative SQL parity, RQL phase 1, memory-light cases, scalar cases, aggregate cases, and CLI rendering cases.
- Add BOLT only after PGO is stable and stderr pollution is eliminated from training.
- Compare `mimalloc`, `jemalloc`, and system allocator under SQL, RQL, and RSS suites.
- Gate CPU-specific SIMD behind runtime detection unless the benchmark binary is intentionally per-host native.
- Avoid illegal instructions in CI by keeping portable fallback binaries.

Expected impact:

- Native CPU plus allocator: `3-10%` broad improvement.
- PGO: `5-15%` broad improvement if trained on the actual corpus.
- BOLT: `2-8%` additional improvement if profile quality is good.
- Combined best case: enough to reduce broad `1.95x` median toward `1.55-1.70x`, but not enough alone to beat SQLite.

Acceptance criteria:

- Four binaries are measured on the same frozen evidence set.
- Chosen official profile is based on full-suite median, p95, max, faster-count, and RSS, not median alone.
- Training warnings do not contaminate benchmark stderr.
- Portable fallback remains green.

## W3: Native RQL Fast Path

RQL needs a separate prepared execution path, not just a JSON-to-SQL-AST bypass.

Required changes:

- Add an RQL prepared-template cache keyed by canonical RQL JSON hash, schema version, stats version, optimizer version, and connection flags.
- Bind RQL `SELECT` directly from RQL structures into internal logical plan structures without constructing `sqlparser::ast::Query`.
- Preserve existing RQL DML direct lowering, but extend it with cache reuse.
- Add a native RQL scalar expression binder that maps RQL expressions to existing scalar IR or ScalarProgram VM directly.
- Add a native RQL output path that streams rows without building unnecessary `Vec<Vec<Cell>>` in non-interactive benchmark mode.
- Keep SQL semantic equivalence by lowering to the same typed execution primitives after planning.
- Add a compatibility fallback that routes unsupported RQL shapes through the current SQL-AST path and records telemetry.

RQL fast-path routing:

| RQL shape | Required route |
|---|---|
| Simple projection | native logical plan |
| FROM single table | native table scan/access path |
| WHERE scalar predicate | native predicate/ScalarProgram VM |
| ORDER BY LIMIT | native AccessPath/top-k route |
| GROUP BY aggregate | native aggregate route |
| JOIN/subquery/window | fallback initially unless already safe |
| DML | existing direct lowering plus cache |

Expected impact:

- RQL prepare/cache improvement: `10-25%` on repeated or multi-statement RQL cases.
- Native RQL select binding: `15-35%` on supported select cases.
- Streaming output: `5-15%` on output-heavy RQL cases.
- Stretch: RQL median below SQLite for the simple/select-heavy phase 1 subset.

Acceptance criteria:

- RQL and SQL same-case output hashes remain identical.
- RQL reports per-case whether it used native path or fallback.
- Native-path coverage starts with at least `60%` of current RQL phase 1 passing cases.
- Fallback cases have no behavior change.
- RQL median improves by at least `20%` before this workstream is considered successful.

## W4: Wire Morsel and Vector Execution Into Default SQL

The Morsel work exists but is not on the main execution path. This is the largest structural win.

Required changes:

- Fix `BytesArena` growth before enabling text/blob morsels because current push behavior risks O(n^2) copying.
- Add heap and covering-index adapters that fill columnar `Morsel` batches directly from storage/index cursors.
- Add typed column vectors for `i64`, `f64`, bool/null bitmap, and borrowed/arena text.
- Route simple `SELECT ... FROM table WHERE ...` through morsel scan/filter/project when predicates and projections are supported.
- Route simple aggregate cases through `MorselHashAggregator`, including ungrouped `COUNT`, `SUM`, `MIN`, `MAX`, `AVG`.
- Add morsel-to-row flush only at the final boundary for CLI/API compatibility.
- Keep old tuple executor as fallback for every unsupported shape.
- Add telemetry counters for morsel eligible, morsel used, fallback reason, rows processed, batches processed, and bytes copied.
- Default to off behind an env/pragma gate for initial integration, then flip on after full proof.

Initial supported query set:

| Query shape | Target |
|---|---|
| Single table full scan | morsel scan |
| Single table WHERE on numeric columns | SIMD morsel filter |
| Simple projection of columns/scalars | morsel projection where scalar VM supports it |
| COUNT(*) | morsel aggregate |
| COUNT(col) | morsel aggregate with validity |
| SUM/MIN/MAX/AVG numeric | morsel aggregate |
| GROUP BY one or more primitive columns | morsel hash aggregate after correctness proof |
| ORDER BY LIMIT | stay tuple/top-k until W5/W6 proof |

Expected impact:

- Scan/filter/projection cases: `20-50%` improvement.
- Numeric aggregate cases: `30-70%` improvement.
- Long-tail aggregate cases: reduce many `2x+` cases below `1.5x`.
- Median SQL impact depends on case mix, but this is the workstream most likely to move median below `1.3x`.

Acceptance criteria:

- Tuple executor remains available and produces identical results.
- Differential test harness compares tuple vs morsel for randomized tables, nulls, affinities, predicates, projections, and aggregates.
- Morsel path has no RSS regression above `5%` on median memory suite.
- Morsel path is disabled automatically for unsupported collations, unsupported scalar functions, volatile functions, subqueries, triggers, or window functions.
- Enable-by-default only after `just redline-testing-official`, SQL parity, RQL phase 1, and memory gates pass.

## W5: Planner, AccessPath, and Index Long-Tail

Several worst cases are planner/index/sort/materialization problems. The AccessPath IR exists but is not fully default.

Required changes:

- Complete AccessPath IR integration for order satisfaction, hard limit, covering map, residual predicate safety, and cost model.
- Enable safe AccessPath IR by default after differential proof against legacy planner.
- Broaden index matching for equality prefixes, range suffixes, reverse scan, ORDER BY LIMIT, and covering projections.
- Keep residual predicates explicit and rechecked instead of rejecting useful indexes.
- Expand partial-index implication only for provably safe simple predicates.
- Expand expression-index equality only when expression identity is canonical and deterministic.
- Add multi-index OR only after single-index paths are stable.
- Add planner trace output for every slow case showing chosen path, rejected paths, residuals, covering status, sort requirement, and limit pushdown.

Expected impact:

- ORDER BY LIMIT cases: `20-60%` improvement when sort avoided.
- Index/filter cases: `20-80%` improvement when heap loads avoided.
- Long-tail max reduction: expected to remove most remaining `>=2x` index/planner cases.

Acceptance criteria:

- No index path may skip residual predicates.
- Every new index optimization has a negative correctness test.
- Planner trace must explain why fallback happened.
- Any default-on AccessPath change requires full SQL parity and targeted index suites.

## W6: Aggregation, CTE, Window, Join, and Subquery Runtime

Attack the current row materialization and cloning hotspots.

Required changes:

- Add one-pass scalar aggregate path for no-GROUP-BY aggregates.
- Remove fallback group materialization into `Vec<Vec<SqlRow>>` for common grouped aggregates.
- Reuse aggregate key buffers and avoid repeated `SqlValue` cloning where safe.
- Route simple grouped aggregate through `HashAggregator` or Morsel aggregate rather than row-group materialization.
- Port or redesign preserve-branch `simple.rs` aggregate work after conflict audit.
- Add recursive CTE arena reuse and lowercase/name-resolution hoists.
- Add scalar subquery first-row fast path and EXISTS/NOT EXISTS short-circuit path.
- Add join/subquery decorrelation only for simple proven shapes.
- Reuse window partition key scratch buffers and avoid repeated key serialization.

Expected impact:

- Aggregate long tail: `25-70%` improvement.
- Recursive CTE long tail: `15-40%` improvement.
- Subquery/EXISTS cases: `20-60%` improvement.
- Window cases: `10-30%` improvement.

Acceptance criteria:

- Aggregate NULL, DISTINCT, COLLATE, affinity, and overflow semantics match SQLite.
- CTE recursion limits and ordering semantics remain unchanged.
- Subquery changes preserve SQLite's first-row scalar behavior.
- Window changes are differential-tested against existing executor and SQLite outputs.

## W7: CLI Startup, Output Rendering, and RSS

The official benchmark is CLI-visible, so startup and rendering overhead matter.

Required changes:

- Decide whether `redlinedb-lite` should become the official parity benchmark target or whether the main `redlinedb` binary should get a batch-mode fast startup path.
- Add a zero-interactive batch mode that bypasses rustyline, shell prompt setup, help table initialization, and unused extension registries.
- Stream output directly to buffered writer in all non-interactive benchmark modes.
- Use specialized integer/real formatting paths where SQLite-compatible.
- Avoid building `Vec<Vec<Cell>>` for output unless API requires it.
- Lazy-initialize heavyweight registries and optional subsystems.
- Measure allocator choice against CLI startup RSS, not just runtime.
- Keep interactive CLI behavior unchanged.

Expected impact:

- Per-case startup/rendering: `5-25%` broad official benchmark gain.
- RSS median: possible reduction from `3.17x` overhead toward `2x`.
- Worst startup-only memory cases should drop substantially.

Acceptance criteria:

- Interactive CLI snapshot tests remain unchanged.
- Batch mode emits byte-identical output to current CLI for parity cases.
- Startup benchmark includes empty input, scalar select, schema workload, and output-heavy workload.
- RSS median and max are tracked separately.

## W8: Kernel, WAL, and Write Path

Kernel work matters most for write-heavy cases and product credibility, but current SQLite parity median looks dominated by SQL/CLI/executor.

Required changes:

- Keep WAL pipeline work behind a correctness gate until recovery semantics are fully wired.
- Identify write-heavy benchmark cases where WAL/fsync actually dominates.
- Route safe group-commit only for durable multi-write sessions, not one-shot cases where it adds thread overhead.
- Audit hot-row commutative update optimization for benchmark relevance and semantic safety.
- Add page-cache and prefetch improvements only where profile shows storage decode or page traversal as bottleneck.
- Avoid background worker startup in read-only/short-lived CLI cases.

Expected impact:

- Write-heavy cases: potentially large.
- Overall median: likely small unless benchmark has many write-heavy cases.
- Startup RSS: risk if workers initialize eagerly, so all kernel workers must be lazy.

Acceptance criteria:

- Crash/recovery tests pass under pipeline on/off.
- WAL format compatibility is explicit.
- No worker thread starts for read-only `SELECT 1` style cases.
- No median RSS regression.

## W9: Safety, Regression Control, and Proof Lanes

Every speed change must be paired with proof.

Required proof gates:

| Gate | Purpose |
|---|---|
| `just fast` | Default repo health |
| Targeted crate tests | Fast package-local correctness |
| SQL parity quick | Catch obvious conformance regressions |
| `just perf-quick target/release/redlinedb candidate` | Fast perf signal |
| `just perf-medium target/release/redlinedb candidate` | Medium perf confidence |
| `just perf-full target/release/redlinedb candidate` | Full SQL parity performance |
| RQL phase 1 full | Required for RQL work |
| Memory suite | RSS regression protection |
| Official redline-testing evidence | Release-grade proof |

Regression policy:

| Regression type | Allowed? | Action |
|---|---|---|
| New conformance failure | No | Revert or gate |
| New skipped official case | No | Revert or justify with owner approval |
| Median SQL regression | No | Revert or re-rank |
| p95/max regression above budget | No | Revert or isolate |
| Single-case regression `<5%` | Maybe | Accept only if larger suite gain and not long-tail |
| RSS median regression | No | Revert or lazy-init |
| RSS isolated regression | Maybe | Must be explained and bounded |
| Unsafe/SIMD change without fallback | No | Runtime detect or remove |

Required artifacts:

- Baseline report.
- Per-workstream before/after report.
- Branch recovery ledger.
- Slow-case ranking.
- Fallback reason histogram.
- Perf flamegraphs for top time contributors.
- RSS table.
- Final release evidence bundle.

## Agent Parallelization Plan

| Agent | Workstream | Files/subsystems | Output |
|---|---|---|---|
| Agent A | W0 evidence | `redline-testing`, benchmark reports, scripts | Frozen baseline and ranked case matrix |
| Agent B | W1 recovery | Git history and branch diffs | Recovery ledger and cherry-pick candidates |
| Agent C | W2 build/HPC | Cargo profiles, build scripts, allocator config | Reproducible native/PGO/BOLT matrix |
| Agent D | W3 RQL | RQL binder, connection cache, CLI RQL path | Native RQL fast path design and implementation |
| Agent E | W4 vector | Morsel scan/filter/project/aggregate | Tuple-vs-morsel executor route |
| Agent F | W5 planner | AccessPath, index access, select top-k | Default-safe planner/index improvements |
| Agent G | W6 runtime | Aggregate, CTE, window, subquery | Row-clone/materialization reductions |
| Agent H | W7 CLI/RSS | CLI batch mode, formatting, startup | Batch fast path and RSS reduction |
| Agent I | W9 proof | Tests, perf gates, evidence packaging | Regression dashboard and release gate |

Coordination rules:

- Agents may work in parallel only after W0 publishes the frozen baseline.
- W3, W4, W5, and W6 must expose feature gates until W9 proves safety.
- W1 can port isolated low-risk commits while W4/W5 do structural work.
- W2 measurements should be repeated after each major default-on change.
- No agent may edit generated zones manually.
- No agent may merge divergent branches wholesale.

## Implementation Order

1. Freeze evidence and import exact `v4.0.9` SQL/RQL/memory reports.
2. Build branch recovery ledger and identify low-risk salvage commits.
3. Land low-risk broad wins: CLI streaming, scalar fast paths, parser allocation reductions, allocator/profile improvements.
4. Add RQL prepared cache and native simple-select binder behind gate.
5. Fix Morsel arena and add tuple-vs-morsel differential harness.
6. Wire Morsel scan/filter/project for simple single-table selects behind gate.
7. Wire Morsel aggregates for simple numeric aggregates behind gate.
8. Complete AccessPath default-safe planner/index improvements.
9. Attack aggregate/CTE/window/subquery long-tail with targeted runtime changes.
10. Run native/PGO/BOLT matrix and pick official profile.
11. Flip safe gates default-on only after full official proof.
12. Produce final benchmark report and release notes.

## Expected Outcome by Milestone

| Milestone | Expected SQL median | Expected RQL median | Faster-than-SQLite cases |
|---|---:|---:|---:|
| Baseline | `~1.95x` | `~1.80x` | `~0.3%` SQL |
| After W1/W2/W7 quick wins | `1.50-1.75x` | `1.35-1.60x` | `5-15%` |
| After W3 native RQL | SQL unchanged or slight gain | `0.90-1.20x` | RQL materially improved |
| After W4/W5 vector/planner | `1.10-1.35x` | `0.75-1.00x` | `25-50%` |
| After W6 long-tail | `<1.20x` p95 target path | `<0.90x` target path | `35-60%` |
| Stretch after full tuning | `<1.00x` median | `<0.70x` median | `>=60%` |

## Public API, Interface, and Compatibility Changes

Expected public changes should be minimal.

Allowed additions:

- Optional env/pragma gates for native RQL, morsel executor, AccessPath planner, ScalarProgram VM, and CLI batch fast path.
- Optional telemetry counters for selected execution path and fallback reason.
- Optional build profiles for native, PGO, and BOLT.
- Optional benchmark target selection if `redlinedb-lite` becomes official.

Not allowed without explicit approval:

- SQL syntax behavior changes.
- RQL wire shape changes.
- Output formatting changes.
- Persistent file format changes.
- WAL compatibility changes.
- Removal of current CLI behavior.
- Regression in embedded Rust API behavior.

## Test Plan

Correctness tests:

- Existing workspace tests.
- Targeted SQL parser/planner/executor tests for each changed path.
- RQL SQL-equivalence tests for every native RQL supported shape.
- Tuple-vs-morsel randomized differential tests.
- SQLite oracle tests for changed SQL semantics.
- CLI byte-output snapshot tests.
- Crash/recovery tests for any WAL path change.

Performance tests:

- Per-case before/after on targeted cases.
- `perf-quick` after every workstream.
- `perf-medium` before default-on gate.
- `perf-full` before merge/release.
- RQL phase 1 full after every RQL change.
- Memory suite after CLI, allocator, vector, or cache changes.
- Native/PGO/BOLT binary matrix before release.

Acceptance tests:

- `0` new official SQL parity failures.
- `0` new RQL conformance failures.
- `0` new memory correctness failures.
- SQL median improves in every default-on milestone.
- RQL median improves by at least `20%` before native RQL work is accepted.
- No p95 or max long-tail regression beyond defined budget.
- Final evidence includes raw logs, hashes, command lines, binary hashes, and report paths.

## Assumptions

- Primary benchmark target is the official `redline-testing` SQL/RQL CLI benchmark unless the project explicitly changes the official target.
- RedlineDB should optimize the real shipped binary, not only synthetic in-process benches.
- RQL may add faster internal planning/execution, but must preserve current public RQL format.
- SQLite compatibility remains mandatory for the SQLite parity suite.
- Feature gates are acceptable during development, but release defaults must be evidence-driven.
- Branch salvage is allowed only through selective ports with tests and benchmark proof.
