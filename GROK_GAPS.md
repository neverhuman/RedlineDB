# RedlineDB compatibility implementation ledger

## Governing decisions — 2026-09-18

Canonical checkout: `/home/ubuntu/redlineDB`; integration owner: root agent.
Baseline: `af20826311a067a51383c382013612545156df61`, dirty on entry.
Preserve the existing kernel durability changes, parser rewrite, size-check script,
and `CLAUDE_GAPS.md`. No worktrees. Historical audit follows unchanged below.

Target: 100% of a published, versioned SQLite **application profile**, initially
SQLite 3.53.1, then the agreed PostgreSQL, native execution, performance and
operations roadmap. Corpus percentages are never complete compatibility evidence.
Correctness precedes speed/size. Rust CPU first; GPU needs measured justification.
Native APIs/CLI retain native defaults; SQLite C APIs select SQLite semantics.
Native runtime format stays; explicit import/export preserves original SQLite files.
Corrected ABI requires v5 prereleases, without replacing existing release assets.

Included profile scope: SQL, selected UTF-8 C APIs, transactions, CLI, FTS5,
RTree/rtree_i32, JSON, selected build functions, processes, read-only, backup,
large values, and explicit conversion. Excluded: direct SQLite runtime-file
interchange, arbitrary VFS, C virtual-table/loadable extensions, session/changeset,
shared cache. Native page/WAL/dbstat layout is an explicit physical difference.
PostgreSQL follows SQLite qualification: protocol, common drivers, portable SQL,
and required catalogs; no PostgreSQL storage, replication or PL/pgSQL claim.

## Correction register

- INSTEAD OF INSERT exists; UPDATE/DELETE remain incomplete.
- SELECT NULLS FIRST/LAST already works.
- Four corpus skips cover three module families.
- Case 437 does not establish DML LIMIT as the cause of latency.
- AccessPath is enabled; a morsel scan does not prove vectorized execution.
- Manual checkpoint exists; scheduling and correctness need separate review.
- Accepting USING hnsw does not establish creation of an HNSW index.
- Reported 2,441 passes/four skips are unqualified corpus observations; twelve
  expected-exit mismatches and 148 disabled-output cases need adjudication.

## Coordination and evidence contract

Claim paths before edits; parser/catalog/transaction/public ABI ownership is exclusive.
Reread this ledger before claims and submissions. Serialize changes using
`flock`/`fcntl` on `.git/grok-ledger.lock`; reread under lock; never build under lock.
Only root changes shared decisions, denominators, release inputs or branch state.
Do not stage unrelated files. No task is verified without independent review and
passing integrated evidence. Excluded/deferred/refuted entries never add to totals.
Each task record must retain scope, implementer/reviewer, file claims, base commit,
dependencies, exact reproducer/expected/current result, positive/negative/state/
failure acceptance, commands/exits/artifacts/hashes, implementation and tested
integrated commit, and status. Until committed, record dirty-tree identities.
Oracle/infrastructure failures, missing/duplicate cases, unexpected skips and new
unexplained failures fail gates. Known defects remain failures. Final qualification
allows no required failures/skips or unknown coverage.

## Phase 2 kickoff — safety and evidence closure (2026-09-18)

User explicitly requested a detailed next phase and multiple working agents.
Canonical Git authority remains this monorepo, with imported component subtrees
and independent Cargo workspaces. No worktrees, sibling development checkouts,
submodule updates, external publication or split-repository synchronization.
Rust remains the engine implementation language; upstream C is the reference and
independent ABI test consumer, not the Redline implementation.

Correction: prior cycle notes calling the default official runner lane externally
pinned were inaccurate. `ops/ci/lib.sh::ci_install_redline_testing` builds the
included runner at this checkout; `ci_install_redline_testing_release` is a
separate retained helper. Prior historical records remain for traceability.

| ID | Bounded kickoff deliverable | Implementer / reviewer | Exclusive claims | Status |
|---|---|---|---|---|
| P2-EVID | Complete 298-case failure reconciliation and repair batches, without weakening fixtures | evidence agent / root | subrepos/redline-testing/profiles/phase2-failure-triage.json, docs/compatibility/phase2-evidence.md | review |
| P2-SAFE | Returned WAL I/O-error/uncertain outcome reproducer and reviewed Rust fix contract | storage agent / root | crates/kernel/tests/strict_commit_faults.rs, docs/compatibility/phase2-durability.md | review |
| P2-ABI | Independent upstream-header ABI preparation/type qualification and Rust change map | ABI agent / root | crates/ffi/tests/phase2_abi_probe.c, scripts/compatibility/phase2-abi-probe.sh, docs/compatibility/phase2-abi.md | review; 7 ABI failures reproduced |
| P2-INTEGRATE | Detailed dependency/exit-gate plan, repository/CI authority audit, schema-change routing | root / agents | docs/compatibility/phase2-plan.md, docs/compatibility/implementation-cycle1.md, docs/testing.md, scripts/just/run.sh | implementing |

All use baseline HEAD af20826311a067a51383c382013612545156df61 plus preserved dirty
changes. No production interface edits without an explicit exclusive claim and
review of shared callers. Every result records commands, exits and hashes;
reproductions are not fixes, and passing old corpus cases is not qualification.

### P2-ABI evidence — 2026-09-18

Command: `rtk proxy bash scripts/compatibility/phase2-abi-probe.sh`, exit 1.
Nine pinned upstream-header/library controls pass. Fresh debug Redline library:
2 pass, 7 fail (v3 flags0 MISUSE, nonzero flag SIGABRT, bounded/zero guard
SIGSEGV, embedded-NUL rejection, uncleared error output, NULL/FLOAT wrong tags).
All probes run in isolated bounded children; dladdr validates every used symbol;
receipt binds library/source/compiler/oracle hashes and dirty checkout identity.
Exact inputs: `crates/ffi/tests/phase2_abi_probe.c`; raw results/commands/hashes:
`target/compatibility-phase2/abi/receipt.json`; analysis and Rust implementation/
v5 packaging prerequisites: `docs/compatibility/phase2-abi.md`.
Implementer ABI agent; independent reviewer root pending. Dependencies EVID-01,
v5 ABI identity before production correction. Implementation/integrated commit
pending. No production interface changed; this is a reproducer, not ABI closure.

## First-cycle claims

All claims use the baseline above. Implementation and integrated commit: pending.
Evidence and exact reproducers are to be appended by the owning agent.

| ID | Scope | Implementer / reviewer | Claimed files | Dependencies | Status |
|---|---|---|---|---|---|
| EVID-01 | Qualified 3.53.1 full-source oracle, parser options, capabilities, identity | oracle agent / root | scripts/sqlite/, crates/bench/tests/sqlite_parity_reference_assets.rs | none | review |
| EVID-02 | Fail-closed comparator and integrity regressions | evidence agent / root | subrepos/redline-testing/ | EVID-01 for qualification | review |
| SQL-02-R | Independent ALTER/transaction/trigger/collation reproducers | SQL agent / root | crates/sql/tests/compatibility_roadmap.rs, docs/compatibility/sql-regressions.md, crates/kernel/src/catalog/ops.rs (exclusive root-approved) | EVID-01 | review; one known failure |
| SAFE-01 | Review preserved durability patch and failure tests | root / SQL agent | crates/kernel/src/engine/runtime/commit.rs, crates/kernel/tests/engine_tests.rs, crates/kernel/tests/failpoint_smoke.rs, crates/kernel/tests/strict_commit_recovery.rs, docs/compatibility/durability-review.md | none | review |
| EVID-03 | Versioned profile and requirement manifest / evidence denominator | root + evidence agent / root | docs/compatibility/, runner manifest interface by coordination | EVID-01, EVID-02 | review |
| EVID-04 | Activate deterministic complete SQL integration shards and oracle integrity | root / evidence agent | ops/ci/fast.sh, .github/workflows/ci.yml, docs/testing.md | EVID-01, EVID-02 | review |
| ABI-01/02 | Preparation/type/lifetime fixes and independent upstream-header fixtures | unassigned / root | unclaimed | EVID-01; v5 ABI release identity | open |

Acceptance: EVID-01 executes every declared optional feature and detects stale cache
identity; EVID-02 rejects both engines failing positive fixtures and retains exits;
SQL-02-R reproduces unrelated-table trigger/view/literal corruption against qualified
oracle; SAFE-01 checks flush-before-publication, failed flush and reopen behavior.

### SQL-02-R evidence — 2026-09-18

Scope: ALTER unrelated trigger owner, unrelated view column, string literal;
positive own-trigger rename, rollback and reopen; escaped literals/comments.
Root approved exclusive catalog/ops.rs claim. Narrow fix preserves owner/literal;
resolved column binding remains a required failure. Three initial reproducers
failed before patch; final SQL suite is 7 pass/1 fail/0 ignored, exit 101.
Kernel literal/comment and quoted-token tests: 3 pass, exit 0. Eight pinned CLI probes: exit 0.
Quoted punctuation, exact quoted table references and rename destinations containing
quote/bracket characters pass SQL/reopen checks.
Exact commands, expected/current behavior, artifact/source SHA-256 and limits:
`docs/compatibility/sql-regressions.md`. Raw evidence: `target/compatibility-sql/`.
Implementer SQL agent; independent reviewer root pending. Base unchanged;
implementation/integrated commit pending, dirty source hashes recorded in doc.
SAFE-01 independent read-only review: flush-before-finish correct; panic bypasses
cleanup and WAL append already wakes writer before flush entry; returned fsync
errors may still replay a commit. These remain qualification blockers, sent to root.

## Dependency roadmap and completion summaries

SQLite: **not qualified**. First cycle implemented in dirty tree and reviewed; qualification remains incomplete. No full roadmap task verified yet.
A: EVID-01/02/03, API-01 connection profiles and typed contracts.
B: SAFE-01..05; SQL-01 transaction outcomes, SQL-02 schema dependencies,
SQL-03 collations, SQL-04 views/triggers, SQL-05 full language inventory.
C: ABI-01..06 signatures, values, ownership, callbacks, headers and real consumers.
D: STORE-01 checkpoint/recovery then STORE-02 overflow; PROC-01 handoff then
PROC-02 rollback locking then PROC-03 WAL snapshots; ATTACH-01 lifecycle then
ATTACH-02 durable coordinated commit.
E: MOD-01 transactional interface then MOD-02 FTS5/MOD-03 RTree/MOD-04 native
dbstat; CLI-01 scripting; DATA-01 explicit validated import/export.

PostgreSQL: **open**, follows permanent SQLite gate: PG-01 namespaces/sessions,
PG-02 types/catalogs/errors, PG-03 protocol/consumers, PG-04 portable SQL.
Native features: **open**: EXEC-01 plans/telemetry, EXEC-02 bounded streaming,
EXEC-03 VM/vector paths, RQL-01 translator; IDX-01 real access methods.
Performance: **open**: PERF-01 valid measurements before WAL-01 throughput work;
seven alternating measured repetitions on quiet hardware for optimization promotion.
Operations: **open**: SEC-01 boundaries, OPS-01 storage/restore, OPS-02 optional
authenticated encryption, HEALTH-01 maintainability and truthful claims.

Required completion: complete executable requirement inventory, zero required
failures/skips/unknown coverage, no integrity/memory/durability blockers, independent
process contracts, real consumers, conversion round trips, four-platform packaged
qualification, matching ledger/docs/release identity, explicit exclusions.

---

# Historical audit (preserved; corrections above take precedence)

# GROK_GAPS.md — RedlineDB Full-System Gap Audit and Engineering Spec

**Status:** Draft for later-agent review. Not a MASTER_PLAN phase file.
**Authoring checkout:** `/home/ubuntu/redlineDB` (claimed canonical GitHub checkout).
**Branch at audit:** `fix/report-skipped-warmups` @ `03550b4d2` with unrelated local warmup-report edits.
**Product version in tree:** `4.1.0` (workspace crates).
**Date:** 2026-09-17.
**Method:** Read `docs/*.md`, paper, workplans, then inventory live source under `crates/` and official evidence under `target/redline-testing/`. Prefer source over docs when they disagree.

This document is the controlling handoff for closing RedlineDB toward a **drop-in replacement for SQLite and PostgreSQL**, written in Rust (C++17/20 or CUDA only where the bonus is large and measurable), **smaller** than those engines, **faster** on the workloads that matter, and **smarter internally** on complex queries.

It is not a rewrite of `docs/sqlite-parity.md`, `docs/beyond-postgres-skips.md`, or `docs/architecture/ENGINEERING_SPEC.md`. Those files are inputs. Several of them are stale; this spec records the drift and then plans from **code + official evidence**.

---

## 0. How to read this document

| Section | Use |
|---|---|
| §1 Mission | What the repo actually claims vs what this audit is asked to achieve |
| §2 Current inventory | What exists today, with paths |
| §3 Definitions of “100%” | Without this, “SQLite 100% parity” is undefined |
| §4 SQLite gap catalog | Corpus, SQL language, C ABI, file format, CLI, ecosystem |
| §5 PostgreSQL gap catalog | Wire, SQL, catalogs, ops, security |
| §6 Performance | Why the engine is still slower than SQLite on the official bench, and where it already wins |
| §7 Smarter internals | Optimizer, vectorized exec, WAL, MVCC, vector/JSONB |
| §8 Size, safety, health | LOC vs SQLite/Postgres, 2000-LOC cap, unsafe, docs drift |
| §9 CUDA / C++ policy | When (if ever) to leave pure Rust |
| §10 Recommended target architecture | The product to build, not a clone of either incumbent |
| §11 Phased engineering plan | Ordered workstreams with proof |
| §12 Key decisions | Explicit choices a later agent must not silently reverse |
| §13 PR plan | Independently reviewable slices |
| §14 Success criteria | Verifiable gates |
| §15 Open questions | Need owner decision before coding |

**Evidence rule:** claims that name a count, SHA, fail/skip ID, or file path were observed in this checkout. Do not treat README badges or `ENGINEERING_SPEC.md` as current truth without re-checking.

---

## 1. Mission reconstruction

### 1.1 What the repo says it is

`README.md` “Redline Mission”:

> RedlineDB keeps **SQLite-shaped compatibility** where that contract is valuable: small embedded deployments, familiar SQL, a direct Rust API, and a SQLite-shaped C surface. The engine is **not a SQLite wrapper**. It rebuilds the storage core in Rust so MVCC, concurrent writes, WAL behavior, and recovery can be owned directly.

`paper/PLAN.md` working title:

> A Rust-native, concurrent-write embedded SQL engine that stays SQLite-compatible **without inheriting its concurrency cliff**.

`docs/beyond-sqlite-gaps.md`:

> The Postgres lane is an **oracle only**. SQLite compatibility remains the default surface.

`docs/beyond-postgres-skips.md`:

> 114 beyond-SQLite cases are **deliberately never closed** (LISTEN/NOTIFY, PL/pgSQL, logical replication, matviews, PG lock manager, …). Cap 150 skip entries.

So the **documented** mission is:

1. SQLite **API/SQL shape**, not SQLite **file format**.
2. Native MVCC + group-commit WAL, multi-writer on disjoint rows.
3. Smaller, agent-repairable codebase.
4. Postgres as a **reference oracle** for a portable “beyond SQLite” subset, not as a server to impersonate.

### 1.2 What this audit is asked to achieve

The controlling request for this spec:

- Drop-in replacement for **SQLite and PostgreSQL**.
- 100% Rust, with C++17/20 and CUDA only if the bonus is huge.
- Codebase **smaller** than SQLite and Postgres.
- **Faster**, and **much smarter internally**, built for more complex workloads.
- Close **SQLite 100% parity to 100%**.
- Produce a detailed plan of major gaps.

That request **conflicts** with several documented scope cuts (native file format, no VFS, no loadable extensions, no pgwire, 114 PG-only skips). §12 resolves the conflict instead of pretending both can be true at once.

### 1.3 One-sentence product thesis (recommended)

**RedlineDB should remain a native-format MVCC engine that speaks SQLite’s SQL + C ABI well enough that new and many existing SQLite applications can switch without source changes, and should grow a PostgreSQL wire + SQL dialect mode so networked apps can point `libpq` / `sqlx-postgres` at it — without cloning either incumbent’s storage format, extension zoo, or procedural language.**

File-format clones (open a `.sqlite` with `sqlite3` and a PG data directory with `postgres`) are **importers/exporters**, not the runtime format. Cloning those formats would destroy the size and concurrency advantages.

---

## 2. Current-state inventory (code, not brochure)

### 2.1 Crate DAG (live)

```
redlinedb-domain
      │
redlinedb-kernel          pages, WAL, MVCC, catalog, btree, vector, JSONB
      │
redlinedb-sql             parser, planner, executor, RQL, JSON1, shims
      │
  redlinedb               public Rust facade
  ┌────┴─────────┐
 ffi            cli     server     redlinedb-lite
                      (RLDB TCP)
redlinedb-sqlx / redlinedb-tokio   (Rust adapters, not C ABI)
redlinedb-bench                    (measurement only)
```

Workspace members: `crates/{domain,kernel,sql,bench,redlinedb,redlinedb-tokio,redlinedb-sqlx,ffi,cli,server,redlinedb-lite}`. Nested workspaces under `subrepos/` are excluded.

### 2.2 Lines of code (this checkout, `find crates -name '*.rs' | xargs wc -l`)

| Tree | Files | LOC (incl. tests) |
|---|---:|---:|
| `crates/domain` | 2 | 135 |
| `crates/kernel` | 194 | 41,427 |
| `crates/sql` | 227 | 87,447 |
| `crates/redlinedb` | 28 | 7,320 |
| `crates/ffi` | 36 | 7,526 |
| `crates/cli` | 19 | 7,330 |
| `crates/server` | 1 | 624 |
| `crates/bench` | 94 | 21,421 |
| `crates/redlinedb-lite` | 6 | 1,129 |
| `crates/redlinedb-sqlx` | 12 | 2,481 |
| `crates/redlinedb-tokio` | 11 | 989 |
| **crates total** | | **177,829** |

Paper scanner-counted **core src only** (`paper/data/loc_comparison.csv`): **55.2 KSLOC** vs SQLite reference **155.8 KSLOC**. That 55.2 number **excludes tests, comments, blanks**. The SQL crate alone is now 87k including tests; `parser.rs` is 5,337 lines.

Postgres is ~1.1–1.4 MLOC depending on count. Redline is smaller than Postgres by an order of magnitude. Versus SQLite, **core is smaller; with tests it is in the same band**. Growth is concentrated in `crates/sql` (parser shims + executor).

### 2.3 Files that already violate or sit on the 2,000-LOC policy

`scripts/check_file_sizes.sh` hard-cap 2,000, warn 1,500. **Exempted:** `crates/sql/src/parser.rs`, `docs/architecture/ENGINEERING_SPEC.md`.

| LOC | File | Role |
|---:|---|---|
| 5337 | `crates/sql/src/parser.rs` | Pre-parse rewrites + PG shims + DML ORDER BY LIMIT rewrite. **Exempted.** |
| 1951 | `crates/sql/src/parser/pragma.rs` | PRAGMA table |
| 1935 | `crates/sql/src/exec/select_top.rs` | SELECT runtime |
| 1777 | `crates/sql/tests/rql_native_select.rs` | tests |
| 1770 | `crates/sql/src/parser/select.rs` | SELECT/WITH |
| 1743 | `crates/sql/src/exec/mod.rs` | dispatch |
| 1696 | `crates/sql/src/rql.rs` | RQL IR |
| 1550 | `crates/kernel/src/catalog/ops.rs` | DDL apply (warn band) |
| 1509 | `crates/sql/src/json/jsonb.rs` | JSONB SQL |

Any further SQL work that lands in `parser.rs` or `select_top.rs` must split first.

### 2.4 Storage kernel (what is real)

Native directory, **not** `SQLite format 3`:

| File | Magic | Purpose |
|---|---|---|
| `data.redline` | `RDPG` `0x5244_5047` | 16 KiB slotted pages (heap + btree) |
| `CONTROL_A` / `CONTROL_B` | `RDCT` | dual-generation checkpoint |
| `schema.redline` | `RCAT` | atomic catalog snapshot |
| `wal/{n}.wal` + `.seal` | `RDWL` | segmented WAL |
| tx-status sidecar | `RDTX` | CSN/tx frontier |

**Catalog is native `schema.redline`.** `ENGINEERING_SPEC.md` still claims an embedded SQLite `.redline_catalog`. That is **false in source** (`crates/kernel/src/catalog/store.rs`).

MVCC: `TupleVersion` 72 B header, CSN frontier, SI visibility, row locks (`engine/lock.rs`), unique-key locks (`index/locks.rs`). `Isolation::Serializable` is **rejected** at `Engine::begin` (`UnsupportedIsolation`). SQL always begins `Isolation::Snapshot`; `SET TRANSACTION ISOLATION LEVEL` is recall-only.

WAL: single-lane `WalCoordinator`, group-commit (default 200 µs / 4 MiB). `wal_pipeline` feature is **scaffolding, not wired**. Multi-lane coordinator exists but engine stays `lanes: 1`. Semantic combiner default off.

Recovery: **redo-only**. Uncommitted versions become ghosts until vacuum. PITR enums exist; no replica.

Hard storage limits vs both incumbents:

- **No overflow / TOAST.** Rows larger than one page are `TooBig` (`crates/redlinedb/tests/oversized_row.rs`).
- **No encryption, no compression.**
- **No autovacuum daemon.** Explicit `VACUUM` / `checkpoint()`.
- B-tree splits still take a global `structure_lock`. Non-split writers can use page latches / right-links.

### 2.5 SQL engine (what is real)

Parser: `sqlparser` 0.61 `SQLiteDialect`, with a PostgreSQL-dialect retry for a few shapes and a large **text-rewrite** layer in `parser.rs` (schema prefix strip, `pg_class`/`pg_namespace` VALUES, `::regclass`, INTERVAL, `SELECT INTO` → CTAS, ROLLUP/CUBE → UNION ALL, two LATERAL shapes).

`PreparedKind` covers SQLite CRUD/DDL plus Track J/K extras: schema/sequence/isolation/show/alter-index/MERGE/ATTACH/cross-db.

Working SQLite-shaped surface (engine-local tests + official corpus): SELECT (joins including LEFT/RIGHT/FULL bind, compounds, windows, recursive CTEs), DML + RETURNING + representative UPSERT, CTAS, views, BEFORE/AFTER triggers, generated columns, partial + expression indexes, FK actions, STRICT, WITHOUT ROWID, AUTOINCREMENT + `sqlite_sequence`, `sqlite_master`/`sqlite_stat1`, JSON1, datetime, collations BINARY/NOCASE/RTRIM/UINT, savepoints, ATTACH (partial), `generate_series`.

Optimizer: cost constants, index point/range/covering, multi-index AND/OR, join DP up to `max_exact_join_tables` (default 8) else greedy, ANALYZE with MCV + 100-bucket histograms (`StatsConfig`).

**Default-off / scaffolding (the “smarter” stack is mostly dark):**

| Mechanism | Default | Path |
|---|---|---|
| Scalar bytecode VM | OFF (`PRAGMA redline_scalar_vm`) | `exec/expr/program.rs` |
| AccessPath IR | OFF (`PRAGMA redline_planner_use_access_path`) | `planner/access.rs` |
| Morsel/vector executor | Observe-only; `route_primitive_scan` **always declines** unless `REDLINE_MORSEL_ROUTE` | `exec/morsel/` |
| `exec/vec/*` operators | Types exist; live path is still tuple `SqlRow` | `exec/vec/mod.rs` `dead_code` |
| Parallel covering scan | Needs rayon pool; covering index path often stays serial | `exec/select_parallel.rs` |
| DML `ORDER BY … LIMIT` | Rejected unless `PRAGMA redline_dml_order_limit_rewrite` | `parser.rs` |
| RQL | Additive, default-off typed IR | `rql.rs` |

### 2.6 FFI / C ABI (what is real)

- Canonical header: `contracts/c-abi/redlinedb.h`.
- `contracts/c-abi/sqlite3.h` is a **10-line re-include**, not SQLite’s header.
- ~114 exported `sqlite3_*` `no_mangle` functions. Bundled `libsqlite3-sys` 0.35 / SQLite 3.50.2 header has ~352 `SQLITE_API` lines. Allowlist: 195 `[[exclude]]` in `crates/ffi/tests/symbol_allowlist.toml`.
- Core open/prepare_v2/step/bind64/column64/exec/UDF/collation/blob/hooks exist.
- `sqlite3_libversion()` returns **crate version `4.1.0`**, not SQLite `3.53.1`. Bindings that branch on version will lie.
- `sqlite3_prepare_v3` **argument order does not match SQLite** (`flags` last vs `prepFlags` before `ppStmt`). Linking against official `sqlite3.h` is an ABI break.
- `sqlite3_open_v2` with `SQLITE_OPEN_READONLY` returns `SQLITE_READONLY` instead of opening read-only. VFS name ignored. **`:memory:` is not special-cased** in `open_handle` (`crates/ffi/src/util.rs`) — it is treated as a filesystem path.
- `symbol_diff.rs` coverage test is `#[ignore]`.

### 2.7 Server / clients (what is real)

`crates/server/src/main.rs` (~624 LOC): custom framed TCP. Magic `RLDB`, length-prefixed JSON, SQLite-shaped prepare/bind/step. **No TLS, no auth, no pgwire, no SQLSTATE, one active statement per connection, `interrupt_all` is process-wide.**

Clients:

- `redlinedb-sqlx`: `redline://` / `redlinedb://` on sqlx Any. Not a sqlite or postgres driver swap.
- `redlinedb-tokio`: `spawn_blocking` over sync API.
- `subrepos/redline-central`: RLDB TCP client; SQLite/Postgres are **oracles**.
- `subrepos/redline-web`: HTTP console; talks rusqlite or a SQLite-shaped CLI, not pgwire.

### 2.8 Official conformance evidence (this machine)

Producer: `subrepos/redline-testing` runner **1.0.1**, binary SHA-256 `2b43fb52da03eae05eec0c4991d1a5e5a5e8662322353bb28b1cbad00724c7c8`.
Target: `redlinedb v4.1.0`, SHA-256 `ce6c9168d0441bb0d1edd6d370ffe91e062dc07a0d3b035472113b0cdb638b5c`.
SQLite oracle: **3.53.1**, SHA-256 `fd3bdd25217a849f8f4fa295fb78199cfd69b0c4d47ba8d8c32a1aa328bd147e`.
Raw sqlite_parity SHA-256: `3d249f8c116f15b3cd75acf530b9e16c66082c39b95e272c201dc8ec008f093b`.
Provenance records engine git `f7781b806365790c2d9c1a066fcd118784b640cb`, `git_dirty: true`.

| Suite | Total | Passed | Failed | Skipped |
|---|---:|---:|---:|---:|
| `sqlite_parity` | 2445 | **2441** | **0** | **4** |
| `sqlite_parity_memory` | 2445 | 2441 | 0 | 4 |
| `rql_phase1` | 1385 | 1129 | 0 | 256 |
| `beyond_sqlite` | 277 | 4 | 0 | 273 |

The four sqlite_parity skips are capability gates, not fails:

| ID | Name | Reason |
|---|---|---|
| 00093 | `CREATE_VIRTUAL_TABLE_FTS5_OPTIONAL` | no fts5 module |
| 00094 | `FTS5_HIGHLIGHT_OPTIONAL` | no fts5 |
| 00095 | `CREATE_VIRTUAL_TABLE_RTREE_OPTIONAL` | no rtree |
| 00096 | `DBSTAT_OPTIONAL` | no dbstat |

**README still says 2374/2445 (97.10%) and “67 remaining failures” (typeof, IEEE-754, Unicode, BLOB hex, AUTOINCREMENT).** That is a v4.0.0 historical claim. Those categories **pass** in the current 2445-case JSONL. `scripts/parity-tolerate-known-optional.sh` has `known_failing_v101='^$'` (68 → 0 unique failures).

Committed `benchmark-results/sqlite-parity/latest/summary.json` is also stale (1127 cases, v4.0.1, 2026-05-26).

### 2.9 Performance snapshot (official ranked CSV, 2441 passed cases)

Source: `target/redline-testing/ranked.csv`. Ratio = redline_median_ns / sqlite_median_ns. This is **CLI process-per-case** latency, not in-process OLTP.

| Metric | Value |
|---|---|
| Median | **1.677× slower** than SQLite |
| Mean | 2.577× |
| p90 | 4.199× |
| p95 | 6.268× |
| Max | **142.8×** (`00437 DML_WHERE_ORDER_LIMIT_050`, 2.3 ms → 331 ms) |
| Faster than SQLite | **172 / 2441 (7.0%)** |
| ≥2× slower | 864 |
| ≥10× slower | 54 |
| ≥50× slower | 3 |

Worst classes: `GEN_SQL_DML` (ORDER BY/LIMIT DML; rewrite is default-off), `GEN_SQL_SCALAR` (typeof/cast), DDL/DML setup cases that pay engine-open tax.

Paper/cert (in-process concurrent OLTP, historical): writers-disjoint **~8–16×** vs SQLite at 64 threads; point-read ~parity; hot-row and secondary-index range scans trail badly (paper abstract: hot-row 0.21×, secondary range 0.012×). **The engine already wins the mission workload (concurrent writers) and loses the official parity bench (short CLI SQL).**

### 2.10 Feature flags that change the binary

Kernel: `failpoints`, `numa`, `vector_v1_unmerged` (default on), `wal_cross_lane_coalescer`, `wal_pipeline`.
SQL: `failpoints` only.
CLI allocators: `alloc-mimalloc` (default), `alloc-jemalloc`, `alloc-snmalloc` (**C++17** `cc` backend).
Facade: `chrono`, `decimal`, `json`, `tokio`, `uuid` — type helpers, not engine types.

No CUDA, no `.cu`/`.cpp` engine sources.

---

## 3. Definitions of “100%” — required before planning

“Close SQLite 100% parity to 100%” is four different projects. Mixing them produces infinite work and a larger-than-SQLite codebase.

### 3.1 SQLite layers

| Layer | Name | Meaning | Today | Recommended target |
|---|---|---|---|---|
| **S0** | Official corpus | `sqlite_parity` required cases pass | 2441/2445 pass, 4 optional skips | Keep 0 fails; do not grow silent skips |
| **S1** | Optional modules in corpus | FTS5, RTREE, DBSTAT | 4 skips | Implement as **native Rust modules**, then unskip |
| **S2** | SQLite SQL language | Full lang.html minus deprecated | Partial (views/triggers/ATTACH/UPSERT/PRAGMA) | Close ledger `partial` rows; keep explicit `rejects-by-design` tiny |
| **S3** | C ABI drop-in | `#include <sqlite3.h>` (SQLite’s) + link `libredlinedb` | ~36% of 3.50.2 API; wrong `prepare_v3`; version lie | Fresh-app ABI + rusqlite/Python smoke. Not session/vtab C APIs |
| **S4** | File interchange | Open SQLite format 3 files; SQLite opens Redline files | `not-started` | **Importer/exporter**, not runtime format |
| **S5** | Ecosystem clone | VFS, load_extension, shared-cache, sqlite3 shell 1:1 | Out of scope in allowlist | Do not clone. Native extension traits instead |

**This spec’s definition of “SQLite 100% parity” is S0+S1+S2 plus a declared S3 subset that compiles against SQLite’s `sqlite3.h` for the UTF-8 int64 core and runs rusqlite + CPython `sqlite3` smoke tests.** S4 is a product feature (migrate). S5 is rejected.

### 3.2 PostgreSQL layers

| Layer | Name | Meaning | Today | Recommended target |
|---|---|---|---|---|
| **P0** | Beyond-SQLite portable SQL | MERGE, LATERAL subset, DISTINCT ON, JSONB ops, sequences, ILIKE | Mostly skipped; 4/277 pass; oracle often down | Close the 119 “closable in pure Rust” cases |
| **P1** | pgwire | `psql`, `libpq`, `sqlx-postgres`, JDBC | **None** | New `redlinedb-pgwire` crate |
| **P2** | PG catalogs / roles / GUCs | `pg_class` real, `search_path`, GRANT | Session shims | Minimal `pg_catalog` views + login roles |
| **P3** | PG-only runtime | LISTEN/NOTIFY, replication, PL/pgSQL, matviews, SSI lock matrix | Skip-listed deferred | **Do not clone.** Offer Redline-native equivalents where the workload needs them |
| **P4** | PG data-directory clone | `PGDATA` on-disk compatibility | None | Reject. Logical dump/restore only |

**This spec’s definition of “Postgres drop-in” is P0+P1 plus a thin P2** so a typical CRUD/JSON app can set `DATABASE_URL=postgres://…` and run. P3/P4 would make the tree larger than Postgres’s interesting subset and kill the size goal.

### 3.3 “Faster” and “smarter”

| Claim | Measurement that counts |
|---|---|
| Faster than SQLite **embedded** | In-process OLTP (certify matrix) + official corpus **in-process** (not CLI fork) median ≤ 1.0× |
| Faster than SQLite **CLI parity bench** | `ranked.csv` median ≤ 1.0×, p95 ≤ 1.5×, max ≤ 4×, faster-count ≥ 50% |
| Faster than Postgres | pgbench-style mixed + analytical (TPC-H SF1) on same hardware; not a CLI fork bench |
| Smarter internally | Default-on vectorized/morsel path for scans/agg; histogram-aware costing; join reorder already exists (≤8); WAL group-commit on; overflow pages; `FOR UPDATE` / `SKIP LOCKED`; vector/JSONB as catalog access methods |

The current official bench **cannot** prove “faster than SQLite” until CLI/startup tax and DML ORDER BY LIMIT are fixed. It also cannot prove “smarter” — those cases are short SQL.

---

## 4. SQLite gap catalog

### 4.1 S0 — official corpus (almost done)

**Remaining official fails: none.** Work here is hygiene:

1. Refresh README badge and “67 failures” paragraph from `target/redline-testing/summary.json`.
2. Refresh `benchmark-results/sqlite-parity/latest/` via `just sqlite-parity-report-update` so committed evidence matches 2445/v4.1.0.
3. Keep `known_failing_v101` empty. New skips require a named optional category, not a silent gate.

### 4.2 S1 — the last four corpus skips (true language gaps)

These are the only named sqlite_parity holes.

#### 4.2.1 FTS5 (00093, 00094)

SQLite FTS5 is a virtual table module: tokenizer, inverted index, `MATCH`, `bm25`, `highlight`/`snippet`/`offsets`.

**Do not implement SQLite’s C vtab API to get this.** That API is allowlisted out and would pull in `sqlite3_create_module*` plus a pager-shaped module lifecycle.

**Plan:** native `redlinedb-fts` (or `crates/kernel/src/fts/`) inverted index on the existing heap/btree, SQL surface:

```sql
CREATE VIRTUAL TABLE docs USING fts5(title, body, tokenize='unicode61');
SELECT highlight(docs, 1, '<b>', '</b>') FROM docs WHERE docs MATCH 'redline';
```

Internally this can be a **first-class relation kind**, not a vtab. Parser currently hard-rejects `CREATE VIRTUAL TABLE` in `parse_prepared_template_impl` before bind. Change that to a module registry:

```
ModuleRegistry: fts5 | rtree | dbstat | generate_series (already TVF)
```

`highlight` already has a stub that reads `CURRENT_FTS_MATCH`. Wire it to real offsets.

Tokenizer: start with unicode61 + ascii; porter later. Store postings in a btree keyed `(term, docid)`.

Proof: unskip 00093/00094; add rusqlite-oracle tests for MATCH ranking shape (not necessarily identical bm25 constants — document if ranking is “compatible class” vs byte-identical).

**Size risk:** SQLite FTS5 is tens of KLOC. Cap Redline FTS at ~3–5 KLOC with a hard file split. If ranking identity is required, budget more.

#### 4.2.2 R*Tree (00095)

Geospatial R-tree for bounding boxes. SQLite module `rtree` / `rtree_i32`.

**Plan:** kernel `index/rtree.rs` (or reuse vector-style sidecar). SQL:

```sql
CREATE VIRTUAL TABLE boxes USING rtree(id, minX, maxX, minY, maxY);
SELECT id FROM boxes WHERE minX >= ? AND maxX <= ?;
```

Do not take `sqlite3_rtree_geometry_callback` unless a named customer needs it (allowlisted today).

Proof: unskip 00095; property tests for insert/delete/query consistency vs brute force.

#### 4.2.3 dbstat (00096)

SQLite `dbstat` virtual table exposes pager internals (pageno, pagetype, ncell, payload, unused, …). Redline pages are a different format.

**Plan:** a Redline-native table-valued function `dbstat` that reports **Redline** page stats (`PageKind`, slot count, dead_bytes, page_lsn) with SQLite-shaped column names where they map, and extra columns for CSN/horizon. Corpus case likely checks column names + at least one row after CREATE TABLE.

If the case asserts SQLite page-type strings (`leaf`, `internal`, `overflow`), add a compatibility mapping and document overflow=0 until §4.4.

Proof: unskip 00096.

### 4.3 S2 — SQLite SQL language beyond the corpus

Official cases can pass while the language is still incomplete. Ledger `docs/sqlite-parity.md` still marks several rows `partial`. Engine-local `sqlite_full_parity.rs` encodes explicit remaining holes.

#### 4.3.1 Must-close for “SQLite SQL 100%” (application-visible)

| Gap | Evidence | Close plan |
|---|---|---|
| `INSTEAD OF` triggers on views | `catalog/triggers.rs`, `exec/trigger.rs` deferred | Fire-hook: if target is a view, run INSTEAD OF body instead of “cannot modify view” |
| Trigger recursion cap | `TRIGGER_DEPTH_CAP = 8` (docs say 32 / SQLite 1000) | Raise to 1000 in release with stacker threads or trampoline; keep 8 in debug if stack-limited |
| View expansion freshness | Bind-time materialize; cached stmts stale | Expand at execute, keyed on schema_epoch **and** table xmin/csn |
| ATTACH cross-db transactions | Partial; remaining non-SELECT DML + 2PC | Single CSN across attached engines, or document “autocommit per alias” and fail explicit multi-db tx until 2PC |
| UPSERT full conflict matrix | Representative path; `phase10_sqlc_conflict_matrix.rs` | Enumerate SQLite conflict-target / partial unique / rowid / `excluded.` / WHERE; one table of cases |
| Implicit column collation + LIKE NOCASE index | Ledger remaining | Store collation on `ColumnDef`; planner uses it for index matching |
| `UPDATE/DELETE … ORDER BY … LIMIT` | Default reject; SQLite `ENABLE_UPDATE_DELETE_LIMIT` on in reference build | **Default-on** the existing rewrite (`parser.rs` DML rewrite). This is also the **#1 latency outlier (142×)** |
| `NULLS FIRST/LAST` | `parser/ddl.rs` UnsupportedSql | Index key already has `nulls_first`; wire ORDER BY |
| `FETCH … WITH TIES` / `PERCENT` | `parser/select.rs` | WITH TIES = extra scan of peer keys; PERCENT can wait |
| `sqlite_stat2/3/4` | only `sqlite_stat1` | Optional; ANALYZE already has MCV+histograms internally — expose as tables if sqlite3 shell `.stats` needs them |
| `PRAGMA index_xinfo` | explicit gap fixture | Add `cid, name, desc, coll, key` rows matching SQLite |
| `legacy_alter_table` rewrite of view/trigger SQL on RENAME | stub in `catalog/ops.rs` | Implement or keep default-off with tests |

#### 4.3.2 Keep as tiny `rejects-by-design` (do not fake SQLite)

| Surface | Why reject | Trap |
|---|---|---|
| SQLite WAL frame PRAGMAs that imply pager WAL | Native WAL | **Code currently accepts `PRAGMA wal_checkpoint` and returns `(0,0,0)`.** Ledger says reject. Pick one: either real Redline checkpoint rows `(busy, log, checkpointed)` from `WalCoordinator`, or reject. Fabricated zeros are the worst option. |
| `journal_mode=truncate\|persist` | No rollback journal | Keep reject |
| Unknown PRAGMA names | SQLite no-ops them | Keep `UnsupportedSql` — it is a Redline reliability feature. Document for porting |
| `PRAGMA compile_options` advertising `ENABLE_FTS5`, `ENABLE_RTREE`, `ENABLE_SESSION`, `ENABLE_UPDATE_DELETE_LIMIT`, `MAX_TRIGGER_DEPTH=1000` | **Lies** (`parser/pragma_compile.rs`) | Emit **actual** flags. ORMs probe this. This is a correctness bug, not a feature |
| `pragma_module_list` listing fts5/rtree/dbstat | Same lie | List only registered modules |

#### 4.3.3 SQLite functions still missing or stubbed

Must-have for language completeness:

- `soundex` (explicit “no such function”)
- Real `sqlite_compileoption_used` / `sqlite_compileoption_get` matching the honest compile_options list
- FTS helpers once FTS exists: `bm25`, `highlight`, `snippet`, `offsets`
- `likelihood`/`likely`/`unlikely` exist as no-ops (OK)

Do **not** implement the Sessions changeset C API unless S3 expands (allowlisted; large).

#### 4.3.4 Virtual table C API vs native modules

SQLite’s extension model is C vtab. Redline’s model should be a **Rust `Module` trait** registered at `Database` open:

```rust
trait VirtualModule {
    fn connect(&self, args: &[String]) -> Result<Box<dyn VirtualTable>>;
}
```

Expose FTS5/RTree/dbstat through SQL `USING` names. Do not export `sqlite3_create_module*` until a C extension customer exists. That single decision keeps S5 out of the tree.

### 4.4 S3 — C ABI toward real drop-in

A program that `#include`s **SQLite’s** `sqlite3.h` and links `libredlinedb` **cannot work today**. Blockers:

1. **`sqlite3_prepare_v3` ABI mismatch** — reorder to SQLite’s `(db, sql, nByte, prepFlags, ppStmt, pzTail)`.
2. **Missing symbols rusqlite/CPython use constantly:** `sqlite3_bind_int`, `sqlite3_column_int`, `sqlite3_bind_parameter_count`, `sqlite3_bind_parameter_name`, `sqlite3_initialize`/`shutdown` (can be no-op), `sqlite3_config` (accept `SERIALIZED`/`SINGLETHREAD`/`MULTITHREAD` as no-ops or real), `sqlite3_malloc`/`realloc` (can wrap the Rust allocator), `sqlite3_extended_errcode`, `sqlite3_errmsg16` optional.
3. **`sqlite3_open(":memory:")`** must call `Database::create_in_memory`, not `Path::exists(":memory:")`.
4. **`SQLITE_OPEN_READONLY`** must open read-only, not return `SQLITE_READONLY`.
5. **`sqlite3_libversion` / `libversion_number`:** dual identity. Recommend:
   - `sqlite3_libversion()` → `"3.53.1-redline-4.1.0"` or keep SQLite 3.53.1 string **and** add `rldb_libversion()`.
   - `sqlite3_libversion_number()` → `3053001` so bindings do not take “ancient SQLite” paths.
   - Document clearly. Lying as `4.1.0` is worse than lying as 3.53.1.
6. **Header:** ship a **generated** `sqlite3.h` that is SQLite’s header with Redline extras, or implement the missing macros (`SQLITE_STATIC`, `SQLITE_TRANSIENT`, `SQLITE_UTF8`, open flags, authorizer codes). The current 10-line shim is not a drop-in include.
7. Enable `symbol_diff.rs` (un-ignore) once the allowlist is reduced to true out-of-scope (vtab C, session, vfs, mutex internals).
8. Wire busy handler into `RowLockManager` timeout.
9. Fire trace/profile/commit hooks from `sqlite3_step`, not only `sqlite3_exec`.
10. Call UDF/collation destructors on close/replace.
11. `sqlite3_backup_*`: either implement logical page-copy of Redline files with honest `pagecount`, or document “directory copy” and return real page counts from `BufferPool`.

**Explicitly remain allowlisted (size + mission):** VFS registry, mutex C API, load_extension, create_module*, serialize/deserialize of format 3, WAL frame hooks, session/changeset, UTF-16 family (optional later).

**Acceptance for S3:** a small C file compiled against **upstream** `sqlite3.h` (3.53.1) links `libredlinedb`, opens `:memory:`, creates a table, binds int/text, steps rows. A rusqlite crate built `features = ["bundled"]` **replaced** with `libredlinedb` via `libsqlite3-sys` override runs its basic tests. CPython `sqlite3` module against the .so opens memory DB and executes `SELECT 1`.

### 4.5 S4 — file interchange (do not make this the runtime)

`crates/sql/tests/sqlite_full_parity.rs` already asserts Redline roots are directories and not `SQLite format 3`.

**Plan (importer/exporter, separate crate `redlinedb-sqlite-format`):**

1. **Read path:** SQLite pager/btree/record decoder (pure Rust; `sqlite_format` crate or a tight reimplementation of the well-documented format). Copy tables into Redline heap+indexes. Preserve rowid, types, indexes, views as SQL, triggers as SQL.
2. **Write path:** emit format 3 for `VACUUM INTO 'file.sqlite'` / `.dump` already exists as SQL text; add binary export.
3. **Never** use format 3 as the running pager. That would re-import SQLite’s single-writer WAL cliff.

This is the honest “drop-in” for **data**, not for **process**. Apps that `open("app.db")` need either (a) migrate-on-open (detect SQLite header, import into sidecar Redline dir, optionally keep a `.sqlite` export), or (b) a documented conversion CLI `redlinedb import-sqlite`.

**Do not underestimate this.** SQLite’s pager+btree+wal is a large fraction of its 155 KSLOC. Budget a dedicated crate with a 4–8 KLOC cap and corpus of known SQLite files (empty, wal-mode, without-rowid, fts, strict).

### 4.6 S5 — CLI / shell

`crates/cli` already has a large `.` command table. Many are **no-ops**: `.auth`, `.expert`, `.stats`, `.scanstats`, `.vfsname`, `.progress`, `.connection`, `.session`, `.load` absent.

For SQLite-shell drop-in:

- Implement `.load` as **Rust module path** or reject with a clear message (not silent no-op).
- Make `.stats` / `.dbinfo` report Redline counters (`sqlite3_stats_json` already exists).
- Keep `.shell` behind `--safe` / `.nonce` (already).
- `redlinedb-lite` pre-open path is the right way to beat CLI startup tax (§6). Point the official harness at it after a zero-diff gate (workplan A3, still open as a harness change).

---

## 5. PostgreSQL gap catalog

### 5.1 Strategic fact

There is **no Postgres in the product path**. Postgres is:

- a **dev-dependency** oracle (`postgres = "0.19"` in `crates/sql`),
- a skip-list policy of 114 deferred cases,
- a beyond_sqlite corpus whose last official run **skipped 273/277 because `REDLINE_TESTING_POSTGRES_URL` was unset**.

`docs/beyond-postgres-skips.md` numbers (last triage): 265 beyond cases; 20 target-vs-reference passes; 233 failures; 114 skipped as PG-only; 119 labeled closable. `target/redline-testing/closable-beyond-pg.txt` is **missing** in this checkout — regenerate it.

### 5.2 P0 — closable portable SQL (do this regardless of pgwire)

Natural tracks from the skip policy (do **not** implement the skipped 114):

| Track | ~N | Work | Owner crate |
|---|---:|---|---|
| Portability syntax | 32 | MERGE (exists, bypasses FK/triggers — fix), remaining LATERAL, data-modifying CTEs, DISTINCT ON, named windows, FETCH, GROUPING SETS native (not UNION ALL rewrite), ON CONFLICT WHERE, GENERATED AS IDENTITY | sql |
| Rich types rendering | 22 | boolean `t`/`f` **mode**, numeric/decimal, UUID text, interval/timestamptz, arrays as JSON-shaped TEXT | sql + normalizer |
| JSONB operators | 20 | `@>` `<@` `?` `?\|` `?&` `->` `#>` `jsonb_set` jsonpath `@@` | sql/json + kernel/json |
| Collations / ILIKE | 17 | ILIKE, multibyte LOWER/UPPER, `~` `~*` `SIMILAR TO`, `POSITION`, `octet_length` | sql |
| Schemas / sequences | 14 | Durable `CREATE SCHEMA` (not session strip), `CREATE SEQUENCE` as catalog object, identity | kernel catalog + sql |
| Migration ergonomics | 11 | ALTER COLUMN TYPE USING, ADD/DROP CONSTRAINT, RENAME CONSTRAINT/INDEX, identity add/drop | sql + catalog/ops |
| Isolation binding | 1 | `SET TRANSACTION ISOLATION` must call `Engine::begin` with RC/SI; serializable stays error until SSI | kernel + sql |

**Stop advertising PG catalogs we do not have.** `rewrite_pg_catalog_query` VALUES stubs will break real ORMs (Diesel, sqlx with `sqlx::query_as!` on `pg_type`). Either implement honest `pg_catalog` views over `SchemaSnapshot`, or fail unknown `pg_*` tables.

### 5.3 P1 — pgwire (the actual “drop-in”)

New crate `crates/pgwire` (or `crates/server` split):

- Startup + SSLRequest (TLS via `rustls`).
- Password / SCRAM-SHA-256 (start with trust + cleartext in dev, SCRAM before any public listen).
- Simple Query + Extended Query (parse/bind/describe/execute/sync).
- `RowDescription`, `DataRow`, `CommandComplete`, `ReadyForQuery`, `ErrorResponse` with **SQLSTATE**.
- Parameter status (`server_version` → `16.0 (Redline 4.x)`, `integer_datetimes`, `client_encoding`).
- Cancellation via backend key (today `interrupt_all` is global — must become per-connection).

Proof: `psql` can `\conninfo`, `CREATE TABLE`, `INSERT`, `SELECT`. `sqlx::PgPool` against `postgres://` runs a migration + query. JDBC `org.postgresql.Driver` smoke.

**Do not** try to speak both RLDB JSON and pgwire on one port. Keep RLDB for central, add `redlinedb-server --protocol pg --listen …`.

### 5.4 P2 — minimum catalogs and session model

Required for ORMs:

| Object | Minimum |
|---|---|
| `pg_type` | int2/int4/int8/float4/float8/text/bytea/bool/json/jsonb/uuid/numeric/timestamp/timestamptz/oid |
| `pg_class` / `pg_attribute` / `pg_index` | map `SchemaSnapshot` |
| `pg_namespace` | `pg_catalog`, `public`, plus `CREATE SCHEMA` |
| `pg_proc` | only functions we actually implement |
| `search_path` | real GUC, default `public` |
| `current_schema()` | not the constant `"public"` |
| Roles | `redline` superuser + login roles; no GRANT maze in v1 (CONNECT + table owner) |
| OIDs | stable map from `ObjectId` |

### 5.5 P3 — explicitly out of scope (keep skip list)

Do **not** implement to “be Postgres.” Each item is a new product:

- LISTEN/NOTIFY (if needed later: Redline-native `NOTIFY` on the pgwire connection using the WAL logical stream — still a project)
- `CREATE FUNCTION` / PL/pgSQL / `CALL`
- Logical/physical replication, slots, `pg_wal_lsn`
- Materialized views
- `FOR KEY SHARE` lock matrix, advisory locks, `LOCK TABLE`
- ENUM/DOMAIN/range/MONEY/geometry as first-class types
- Inheritance, UNLOGGED, FDW, partitioning, RLS
- GIN/GiST/BRIN/Hash AMs, `tsvector`
- `citext`, ICU named collations, nondeterministic collations

If a workload needs search, use Redline FTS + vector indexes, not `tsvector`. If it needs pub/sub, use an external broker or a later native NOTIFY.

### 5.6 Server security (blocking for any networked drop-in)

Today: plaintext TCP, no auth, unbounded JSON frames, process-wide interrupt, web console `POST /api/query` runs arbitrary SQL.

Before P1 is more than a toy:

1. TLS (`rustls`).
2. Auth (SCRAM).
3. Frame size cap.
4. Per-connection interrupt.
5. Encryption-at-rest optional (`PRAGMA redline_key` / SQLCipher-shaped) — AES-256-GCM page encryption in kernel I/O. This is also an SQLite ecosystem expectation.

No encryption/TLS code exists in `crates/` today.

---

## 6. Performance gaps (faster than SQLite / Postgres)

### 6.1 Two benches, two stories

| Bench | What it measures | Today | Why |
|---|---|---|---|
| Official `sqlite_parity` CLI | Process start + parse + one-shot SQL | Median **1.68× slower**, max **143×** | mimalloc CLI still ~8 MB vs sqlite3 ~320 KB; Strict fsync; tuple executor; DML LIMIT rewrite off |
| Certify OLTP (paper) | In-process multi-thread | **8–16× faster** on mixed/disjoint writers; ~1× point-read; **0.21× / 0.012×** on hot-row / secondary range | MVCC wins disjoint writes; B-tree structure_lock + no prefetch lose range; hot-row updates serialize |

**Do not optimize only the CLI bench.** Ship both gates.

### 6.2 CLI / harness tax (fastest official-median wins)

Already designed in `speed_up_workplan_FINAL.md` and `super_tasks.md`; several remain open:

1. **Point official harness at `redlinedb-lite`** after stdout zero-diff (A3).
2. **`PRAGMA synchronous` → `Engine::set_commit_durability`** (A1 — commit.rs already reads live durability; verify harness sets NORMAL).
3. **`REDLINEDB_DEFAULT_DURABILITY=normal` in parity CI** (A2). SQLite parity is not a crash-cert.
4. ShellZero pre-open for fromless `SELECT` / dot-commands (`crates/cli/src/shellzero.rs` exists; widen).
5. Lean ephemeral defaults (16 MiB cache / 8 MiB work_mem is huge vs SQLite for `SELECT 1`).
6. Default-on DML ORDER BY LIMIT rewrite — removes the 142× / 57× / 51× outliers.

### 6.3 Executor / planner (complex workloads)

| Gap | Symptom | Fix |
|---|---|---|
| Tuple-at-a-time default | High CPU on scans/agg | Flip morsel route on for PrimitiveScan/PrimitiveAgg after differential vs tuple path |
| Scalar VM off | AST walk per row | Default-on for supported opcode set; keep AST fallback |
| AccessPath IR off | Planner/executor split | Default-on once `access_path_is_consumable_by_executor` holds on corpus |
| MERGE nested-loop materialize | O(\|S\|×\|T\|) | Hash probe on ON keys; share insert/update/delete plumbing (FK/triggers) |
| Recursive CTE clone/dedup | CTE_RECURSIVE_MATRIX slow | Arena + queue indexes (`super_tasks.md` §6) |
| Window cubes | window cases ~3× | Prefix aggregates / inverse accumulators (`super_tasks.md` §5) |
| Expression-index DML | historical 30× class | Already worked in W6; re-measure |
| Secondary-index range | paper 0.012× | Leaf prefetch, reverse cursors, covering default, parallel range |
| Hot-row | paper 0.21× | WAL combiner on for additive updates; group-commit pipeline wired |
| B-tree `structure_lock` | insert serialization | Finish B-link split without global lock |
| No overflow | large rows fail or force application chunking | TOAST-like overflow pages |
| Strict publish-then-fsync fast path | `commit.rs` panics if flush fails after publish | **Durable then visible** always |

### 6.4 Build profile

Already strong: fat LTO, `opt-level=3`, PGO/BOLT scripts, mimalloc default, `x86-64-v3` portable / `native` for benches.

Remaining: make `release-pgo` the official parity binary; allocator A/B (`w2-matrix.sh`); consider `panic=abort` interaction with FFI (`catch_unwind` in FFI still needed — confirm abort profile does not apply to cdylib or keep unwind for ffi).

### 6.5 Versus Postgres

Once P1 exists, measure:

- pgbench `tpcb-like` 1 connection (should lose to PG slightly — process overhead) and 32 connections (should win if MVCC + group commit work).
- Analytical: TPC-H SF1. Will **lose** until morsel + hash join + parallel scan are default-on. This is the “smarter internals” proof.

---

## 7. Smarter internals — architecture gaps

The kernel is already more concurrent than SQLite. The SQL layer is still a **SQLite-era volcano** with a cost model bolted on. The “smart” pieces were built as feature-gated scaffolds so default binaries stayed byte-identical to older releases. That was a good release tactic and a bad end state.

### 7.1 Query execution roadmap (default-on)

```
SQL / RQL / pgwire
        │
    Binder (schema_epoch)
        │
    Planner ── histograms + DP joins (exists) ── AccessPath IR (on)
        │
    Runtime
      ├─ Morsel scan/filter/project          (PrimitiveScan)
      ├─ Morsel hash-agg / SIMD SUM          (exists AVX2 14.4× in gated tests)
      ├─ Vectorized sort / top-k / spill     (exists, not on default SELECT)
      ├─ Hash / index NLJ                    (exists)
      └─ Tuple fallback                      (windows, correlated, exotic)
```

**Rule:** a SELECT that `classify_select_plan_eligibility` marks PrimitiveScan/PrimitiveAgg must not touch `SqlRow` in the inner loop.

### 7.2 RQL

RQL already saves ~6–11% vs Redline SQL on 1,129 shared cases (README v4.0.1 local). 256 skips are rewriter holes, not engine holes.

Plan: widen `rql/native.rs` lowering (WITHOUT ROWID, generated cols, RETURNING, joins, aggregates). Keep SQL as compatibility frontend. pgwire can bind to the same planner IR; do not parse SQL twice.

### 7.3 Statistics and adaptive execution

ANALYZE already writes MCV + histograms. Confirm every `UNKNOWN_EQ_SELECTIVITY = 0.10` site is histogram-backed when stats exist. Add:

- `EXPLAIN ANALYZE` actual vs estimated (fields exist on `PhysicalPlan`).
- Feedback: if actual/est > 10×, mark plan for re-opt next prepare (simple, no ML).

Learned cardinality (Bao/Neo) is **out of scope** until EXPLAIN ANALYZE is honest. Do not add a Python model to the engine (`docs/language-bad-behavior.md`).

### 7.4 Vector and JSON as first-class AMs

Today: SIMD distance + HNSW persist/rebuild + DiskANN **in-RAM** + JSONB codec. SQL has `vector_*` functions. **No `CREATE INDEX … USING hnsw`.** IVFFlat **absent**. DiskANN mmap search is a follow-up comment in `vector/diskann/mod.rs`.

Plan:

```sql
CREATE TABLE items (id INTEGER PRIMARY KEY, embedding VECTOR(768) NOT NULL);
CREATE INDEX items_hnsw ON items USING hnsw (embedding) WITH (m=16, ef=64, metric=cosine);
SELECT id FROM items ORDER BY embedding <=> $1 LIMIT 10;
```

JSONB:

```sql
CREATE INDEX t_j ON t USING gin (j);  -- Redline inverted keys, not PG GIN
SELECT * FROM t WHERE j @> '{"a":1}';
```

This is the “smarter than SQLite” product surface. It is also how to beat Postgres on RAG/document workloads without cloning GiST.

### 7.5 Concurrency smarter than both

SQLite: one writer. Postgres: MVCC + heavyweight locks + SSI option.

Redline today: SI + row locks + unique locks. Missing:

- `SELECT … FOR UPDATE [SKIP LOCKED | NOWAIT]` — **#1 beyond-SQLite backlog**. Maps onto existing `RowLockManager`. This is the queue-claim story (`FEATURE_GAPS.md` / veox).
- Predicate locks / SSI — only if serializable is a product requirement. Prefer `FOR UPDATE` first.
- Deadlock detector (wait-for graph) instead of dual timeout.
- Durable-then-visible commit (fix Strict fast path).

### 7.6 WAL smarter

Wire `wal_pipeline` (writev + page-order) as the production writer. Enable semantic combiner for `SET x = x + ?` hot counters. Consider page-delta records to cut 16 KiB PageImage amplification (engineering spec critique is still valid).

### 7.7 Overflow / TOAST

Without this, neither SQLite blob workloads nor Postgres row workloads are drop-in. Design:

- Heap tuple with `TUPLE_FLAG_EXTERNAL`.
- Overflow chain pages (`PageKind` already has unused kinds; add `Overflow` rather than overloading `Undo`).
- Inline threshold ~2 KiB; rest on overflow.
- JSONB and VECTOR naturally live external.

---

## 8. Size, safety, health, docs drift

### 8.1 Size strategy (stay smaller)

| Engine | Approx | Redline stance |
|---|---|---|
| SQLite amalgamation | 155.8 KSLOC scanned / ~250 KSLOC folklore | Core 55 KSLOC today; **SQL crate is the growth tumor** |
| PostgreSQL | ~1.3 MLOC | Never chase catalogs/PL/FDW |
| Redline crates/*.rs | 178 KLOC with tests | Split parser.rs; delete dead scaffolds after default-on; do not add vtab C + PL/pgSQL |

**Policy:** every new PG-shaped feature must land in ≤ N files totaling ≤ 1,500 LOC or it is a skip. FTS/RTree get their own crates so `redlinedb-sql` does not grow another 20k.

### 8.2 Safety

- Kernel/SQL: mostly safe Rust; SIMD `unsafe` in vector/json/keycmp; DiskANN `align_to`.
- FFI: 547 `unsafe` hits (crate is the ABI boundary). Ledger: `.jankurai/unsafe-ledger.toml`.
- `panic = "abort"` in release vs FFI `catch_unwind` — verify cdylib unwind.
- Strict commit `panic!` on flush-after-publish is a **safety/durability bug**, not a style issue.

### 8.3 Docs drift (do not trust these without re-read of source)

| Document | Stale claim | Live truth |
|---|---|---|
| README badge | 2374/2445, 67 failures | 2441/2445, 0 fails, 4 optional skips |
| `FEATURE_GAPS.md` | partial indexes parser-only | DML + read implication implemented |
| `docs/sqlite-parity.md` | `wal_checkpoint` rejects-by-design | Parser accepts, returns zeros |
| same | `auto_vacuum` reject | Recall-only accept |
| same | views/triggers/ATTACH partial follow-ups | Still true |
| `ENGINEERING_SPEC.md` | 34,999 LOC, 928 tests, catalog in SQLite, Strict==Normal, no join reorder, CHECK/FK not enforced, no partial/expr indexes, no views/triggers | All superseded. Spec is a **historical architecture essay** |
| `ENGINEERING_SPEC.md` PreparedKind | missing MERGE/ATTACH/schema/sequence | `statement.rs` has them |
| Trigger docs | depth 32 / 1000 | cap **8** |
| beyond-sqlite-gaps status | MERGE/LATERAL backlog | Partial implementation exists |
| Paper Table 1 / README tests badge | 691 tests | Workspace tests ~1990 in v4.0.8 notes; re-count |

**Action:** mark `ENGINEERING_SPEC.md` “historical as of phase 10; do not plan from it.” Keep it as architecture explanation of pages/WAL/MVCC which is still mostly accurate at the byte-layout level.

### 8.4 Proof lanes

Default: `just fast`. Official parity: `just redline-testing-official`. Do not resurrect engine-local parity producers (`docs/testing.md`).

Jankurai score: README 85/100; `.jankurai/repo-score.md` shows **86/100 advisory**. Fine; not a gap.

---

## 9. C++17/20 and CUDA policy

**Default: 100% Rust in `crates/{domain,kernel,sql,redlinedb,ffi,server}`.**

Allowed exceptions, each with a measured bonus gate:

| Exception | Already? | When to use |
|---|---|---|
| `snmalloc` C++17 allocator | Optional CLI feature | Keep. Allocator A/B only. Not engine logic |
| C headers for ABI | Yes | Required for SQLite drop-in |
| CUDA distance / HNSW / DiskANN | **No** | Only if CPU SIMD saturates **and** a 10M×768 vector search bench shows ≥5× QPS at same recall after IO is not the bottleneck |
| C++20 for FTS tokenizer / ICU | No | Prefer `unicode-segmentation` / `icu_casemap` crates first |

**Do not** introduce CUDA for SQL execution. Morsel+Rayon+AVX2 is the right CPU design; GPU execution engines (HeavyDB, RAPIDS) are a different product.

HPC tips under `tips/hpc/` are playbooks, not code.

---

## 10. Recommended target architecture

```
                    ┌──────────── sqlite3.h / rusqlite / Python ────────────┐
                    │                     crates/ffi                        │
Apps ── SQL text ───┤                                                       │
     ── RQL JSON ───┤  redlinedb  facade                                    │
     ── pgwire  ────┤  crates/pgwire (new) ── auth/TLS                      │
     ── RLDB TCP ───┤  crates/server (keep)                                 │
                    │         │                                             │
                    │   redlinedb-sql   binder/planner/exec (morsel default)│
                    │         │                                             │
                    │   redlinedb-kernel                                    │
                    │    pages+overflow │ MVCC+row locks │ group WAL        │
                    │    btree+hnsw+fts+rtree │ JSONB AM │ catalog native   │
                    │         │                                             │
                    │   data.redline / wal / schema.redline                 │
                    └────────── import/export sqlite format 3 ──────────────┘
```

**One engine, three dialects of access, one storage format.**

---

## 11. Phased engineering plan

Phases are sequential where noted. Workstreams inside a phase can run in parallel on disjoint crates.

### Phase A — Truth and hygiene (1 week, no behavior change except docs)

**Goal:** stop planning from stale numbers.

1. Regenerate official report into `benchmark-results/sqlite-parity/latest/` and README metrics block.
2. Rewrite `docs/sqlite-parity.md` `wal_checkpoint` / `auto_vacuum` / trigger-depth rows to match code **or** change code to match the ledger. Pick honesty.
3. Make `PRAGMA compile_options` / `pragma_module_list` list **real** capabilities.
4. Banner on `ENGINEERING_SPEC.md`: historical.
5. Restore `closable-beyond-pg.txt` by running beyond_sqlite with Postgres up.
6. Split `parser.rs` (5337) into `parser/rewrite/{pg,dml_limit,strict}.rs` so the 2000-LOC exemption can die.
7. Un-ignore `symbol_diff.rs` in CI as **advisory** first (will fail) to get a live missing-symbol list committed as evidence.

**Proof:** `just fast`; README numbers == `summary.json`; `compile_options` does not mention FTS5 until FTS5 exists.

### Phase B — SQLite S0/S1/S2 language close (4–8 weeks)

Order by user-visible + bench:

1. **Default-on DML ORDER BY LIMIT rewrite** + tests that 00437-class cases stay semantically SQLite. Expected: max ratio collapse.
2. INSTEAD OF triggers; trigger depth; view execute-time expand.
3. Implicit collation + LIKE NOCASE indexes; `PRAGMA index_xinfo`; NULLS FIRST/LAST.
4. UPSERT matrix completion.
5. ATTACH transaction story (document or 2PC-lite).
6. Native **dbstat** TVF → unskip 00096.
7. Native **FTS5** subset → unskip 00093/00094.
8. Native **RTree** subset → unskip 00095.
9. Honest `wal_checkpoint` rows from kernel checkpoint.
10. `soundex` + remaining scalar holes.

**Proof:** sqlite_parity **2445/2445 pass, 0 skip** (or skips=0 after unskip). Engine-local `sqlite_full_parity.rs` explicit-gap tests go green or are deleted with reason. `just redline-testing-official`.

### Phase C — SQLite S3 ABI drop-in (3–6 weeks, overlaps B)

1. Fix `prepare_v3` signature in header **and** Rust.
2. `:memory:`, READONLY open, `bind_int`/`column_int`/`bind_parameter_count`/`bind_parameter_name`.
3. Version identity policy (§4.4).
4. Generate or vendor a real `sqlite3.h`.
5. `sqlite3_initialize` no-op; malloc family wrapping Rust alloc; `extended_errcode`.
6. Hooks on `step`; busy handler → lock manager; UDF destructors.
7. rusqlite + CPython smoke in CI.

**Proof:** C program vs **upstream** sqlite3.h; rusqlite crate tests subset; symbol_diff allowlist shrinks (bind_int etc. removed from exclude).

### Phase D — Performance default-on (4–8 weeks, parallel with B/C)

1. Harness → `redlinedb-lite` + durability NORMAL for parity.
2. Lean ephemeral open for `:memory:`.
3. Morsel PrimitiveScan/Agg **default-on** behind a kill-switch PRAGMA, differential vs tuple on corpus.
4. Scalar VM default-on for supported shapes.
5. AccessPath IR default-on.
6. Wire `wal_pipeline` as production writer; combiner for additive updates.
7. Fix Strict commit order (fsync then publish).
8. B-tree split without global structure_lock.
9. Index range prefetch + reverse cursor.
10. CTE/window/MERGE algorithmic fixes (§6.3).

**Proof:** official median ≤ **1.20×** (phase D gate), then ≤ **1.00×** (stretch). Certify matrix: no regression on writers-disjoint; hot-row ≥ 0.5× SQLite; secondary range ≥ 0.2× as intermediate, ≥ 1.0× later. 0 parity fails.

### Phase E — Overflow + catalog AMs (4–6 weeks)

1. Overflow pages; raise TooBig to SQLite-like blob sizes (at least 1 GiB documented limit, practical 10–100 MB tests).
2. `VECTOR(n)` DDL constraint.
3. `CREATE INDEX … USING hnsw` persisted, reopen without full rebuild if pages already durable.
4. DiskANN mmap search (finish the comment in `diskann/mod.rs`).
5. JSONB GIN-like inverted index; `@>` using kernel bytecode not `serde_json` on the hot path.
6. Durable `CREATE SCHEMA` / `CREATE SEQUENCE` as catalog objects (kills Track J shims).

**Proof:** oversized_row test inverted; vector recall tests on reopen; JSONB containment cases from beyond_sqlite unskipped.

### Phase F — Postgres P0+P1+P2 (8–14 weeks)

1. Close closable beyond-PG cases (P0) with Postgres oracle **up** in CI (`beyond-postgres-reference` lane).
2. `crates/pgwire` + TLS + SCRAM.
3. Honest `pg_catalog` over `SchemaSnapshot`.
4. Real `search_path`.
5. SQLSTATE mapping from `DomainError`.
6. `FOR UPDATE` / `SKIP LOCKED` / `NOWAIT` on row locks (also SQLite-adjacent queue workloads).
7. sqlx-postgres + psql CI smoke.

**Proof:** 119 closable cases pass; skip list stays ≤ 150 and only P3 items; `psql` round-trip; sqlx migrate.

### Phase G — File interchange S4 (6–10 weeks, can start after C)

`redlinedb-sqlite-format` read/write. CLI `redlinedb import-sqlite` / `export-sqlite`. Optional migrate-on-open gated by `OpenOptions`.

**Proof:** import SQLite test corpus; round-trip row counts and sqlite_master SQL; never claim sqlite3 can open `data.redline`.

### Phase H — Encryption / ops (parallel after E)

Page encryption AES-256-GCM; key via URI / PRAGMA / env. WAL encrypted. Backup encrypted. Optional zstd page compression (measure first; may hurt small rows).

---

## 12. Key decisions

| # | Decision | Rationale | Do not |
|---|---|---|---|
| D1 | Runtime format stays Redline (`RDPG`/`RDWL`). SQLite format 3 is import/export only | Preserves MVCC + size. Format clone is a second pager | Make `data.redline` a SQLite file |
| D2 | “SQLite 100%” := S0+S1+S2+declared S3. Not VFS/session/vtab C | Those APIs are SQLite’s extension universe, not SQL | Allowlist-skip FTS/RTree; implement as native modules instead |
| D3 | “Postgres drop-in” := pgwire + portable SQL + thin catalogs. Not PL/pgSQL, not replication, not PGDATA | Size goal; skip policy already correct for P3 | Reverse the 114 skips without a new product charter |
| D4 | One storage engine; dialects are frontends | Avoids dual runtimes | Fork a “Postgres edition” with different heap |
| D5 | Default-on morsel/VM/WAL pipeline after differential | Scaffolds that stay off are dead weight | Add more gated scaffolds |
| D6 | Durable then visible on Strict commit | Current publish-then-fsync+panic is a durability hole | Optimize by acknowledging before fsync |
| D7 | Native Module trait, not sqlite3 vtab C | Keeps FFI small | `sqlite3_load_extension` of existing .so modules (they speak pager) |
| D8 | Honest capability introspection | Lying compile_options/module_list breaks ORMs | Advertise FTS before it exists |
| D9 | CUDA only after CPU SIMD + IO are not the bottleneck and ≥5× is measured | Complexity/size | GPU SQL |
| D10 | Overflow pages before chasing more SQL syntax | Without TOAST, drop-in is a toy for real rows | Raise TooBig without a format plan |
| D11 | `FOR UPDATE` / `SKIP LOCKED` before SSI | Maps to existing locks; unblocks queues | Serializable enum theater |
| D12 | Split `parser.rs`; kill the file-size exemption | 5337 LOC is the highest merge-conflict and drift risk in the repo | Add more rewrites to parser.rs |
| D13 | Version string dual-identity for S3 | Bindings branch on SQLite version | Report `4.1.0` from `sqlite3_libversion_number` |
| D14 | Networked server is pgwire; RLDB JSON stays for central | Actual drop-in | Put auth on RLDB and call it Postgres |
| D15 | Official parity harness uses lite binary + NORMAL durability | Measures engine not CLI/fsync policy | Claim “faster than SQLite” from Strict CLI forks |

---

## 13. PR plan

Each PR independently reviewable. Proof column is the lane a later agent must run.

| PR | Title | Paths | Depends | Proof |
|---|---|---|---|---|
| A1 | Refresh official evidence + README | `README.md`, `benchmark-results/sqlite-parity/latest/**` | — | `just sqlite-parity-report-update` |
| A2 | Honest PRAGMA compile_options / module_list | `crates/sql/src/parser/pragma_compile.rs`, `exec/pragma_tv.rs`, tests | — | `sql-test` |
| A3 | Align wal_checkpoint ledger vs code | `docs/sqlite-parity.md` **or** `parser/pragma.rs` + kernel checkpoint rows | — | `parity_pragma_tv` |
| A4 | Split `parser.rs` rewrites | `crates/sql/src/parser/**` | — | `just fast` |
| A5 | ENGINEERING_SPEC historical banner | `docs/architecture/ENGINEERING_SPEC.md` | — | docs only |
| B1 | Default-on DML ORDER BY LIMIT rewrite | `parser.rs`, `exec/**`, tests | A4 | sqlite_parity max ratio; case 00437 |
| B2 | INSTEAD OF + trigger depth | `exec/trigger.rs`, `catalog/triggers.rs` | — | `parity_trigger` |
| B3 | View execute-time expand | `exec/view.rs`, prepare cache | — | `parity_view` |
| B4 | Collation implicit + index_xinfo + NULLS FIRST | collation, planner, pragma_tv | — | `phase10_sqld_collation*` |
| B5 | UPSERT matrix | `exec/insert.rs`, `phase10_sqlc_conflict_matrix.rs` | — | that test |
| B6 | dbstat TVF | kernel stats + sql TVF | — | unskip 00096 |
| B7 | FTS5 native module | new `crates/fts` or `kernel/fts` | A2, B6 pattern | unskip 00093/00094 |
| B8 | RTree native module | `kernel/index/rtree.rs` | B7 pattern | unskip 00095 |
| C1 | prepare_v3 ABI + real sqlite3.h | `contracts/c-abi/**`, `ffi/src/sqlite3_api/core.rs` | — | C compile vs upstream header |
| C2 | :memory: + READONLY + bind_int/column_int/param count | `ffi/src/**` | C1 | `ffi-test` |
| C3 | Version identity + malloc/initialize | ffi | C1 | rusqlite smoke |
| C4 | Hooks on step + busy→locks + UDF drop | ffi + kernel lock | C2 | `ffi/tests/hooks.rs` |
| D1 | Harness lite + durability env | redline-testing scripts, `options.rs` | — | official median |
| D2 | Morsel PrimitiveScan default-on | `exec/morsel/**`, `select_top.rs` | — | differential + sql-test |
| D3 | Scalar VM default-on | `exec/expr/program.rs` | — | corpus diff |
| D4 | AccessPath IR default-on | planner/exec | — | EXPLAIN tests |
| D5 | WAL pipeline production | `wal/pipeline.rs`, engine | D0 durability fix | failpoint + recovery matrix |
| D6 | Strict commit fsync-then-publish | `engine/runtime/commit.rs` | — | failpoints |
| D7 | B-link split w/o structure_lock | `index/mutate/**` | — | `index_blink_latches` |
| D8 | Range prefetch + reverse cursor | `index/cursor/**` | D7 | certify secondary-index |
| E1 | Overflow pages | `format/page.rs`, heap write | — | oversized_row inverted |
| E2 | VECTOR(n) + HNSW DDL | catalog + sql DDL | E1 | hnsw reopen tests |
| E3 | JSONB inverted index | kernel/json + sql | E1 | beyond JSONB cases |
| E4 | Durable schema/sequence | catalog/ops, session | — | Track J tests rewritten |
| F1 | beyond_sqlite oracle CI | ops/ci, skip list | A1 | `beyond-postgres-reference` |
| F2 | pgwire + TLS + SCRAM | new crate | F1 optional | psql smoke |
| F3 | pg_catalog views + search_path | sql | E4 | ORM smoke |
| F4 | FOR UPDATE / SKIP LOCKED | parser + lock.rs | — | beyond-001 cases |
| G1 | sqlite format reader | new crate | C2 | import corpus |
| G2 | sqlite format writer + VACUUM INTO | G1 | G1 | round-trip |
| H1 | Page encryption | kernel/io | E1 | failpoints + key rotation test |

---

## 14. Success criteria (verifiable)

### 14.1 SQLite

| Gate | Metric |
|---|---|
| S0 | `sqlite_parity` failed_cases = 0 |
| S1 | skipped_cases = 0 on sqlite_parity (FTS/RTree/dbstat unskipped) |
| S2 | `docs/sqlite-parity.md` has no `partial` on SQL surface except documented `rejects-by-design` ≤ 15 rows |
| S3 | Upstream `sqlite3.h` C smoke + rusqlite basic + CPython `sqlite3` memory DB |
| S4 (later) | `redlinedb import-sqlite` on SQLite 3.53.1 empty, wal, without-rowid, strict files |
| Speed CLI | median ≤ 1.00×, p95 ≤ 1.50×, max ≤ 4×, faster-count ≥ 50% |
| Speed OLTP | writers-disjoint ≥ 8× SQLite @ 64T; hot-row ≥ 1× as stretch; 0 lost-acked-commits on failpoint matrix |
| Size | core src (paper scanner) **< 90 KSLOC** after FTS/RTree/overflow; still < SQLite 155.8 |

### 14.2 PostgreSQL

| Gate | Metric |
|---|---|
| P0 | closable-beyond-pg.txt all pass with oracle up |
| P1 | `psql` and `sqlx::PgPool` smoke in CI |
| P2 | `search_path` + `pg_class` readable by `psql \dt` |
| P3 | skip-list still documents deferred; no silent “implemented” lies |
| Security | TLS + SCRAM required for non-loopback listen |

### 14.3 Internals

| Gate | Metric |
|---|---|
| Morsel | PrimitiveScan/Agg on corpus use morsel path (`MORSEL_ROUTE_USED` > 0 in official run with telemetry) |
| VM | `VM_DISPATCH_ENABLED` default true; compile-fail counter bounded |
| WAL | `wal_pipeline` compiled in default release **or** deleted if not |
| Commit | no publish-before-fsync path remains in Strict |
| Overflow | 1 MB blob round-trip |
| Vector | `CREATE INDEX USING hnsw` survives restart, recall@10 ≥ 0.95 on existing tests |

---

## 15. Open questions (owner must answer)

These are product forks. A later agent should not guess.

1. **S3 version identity:** report SQLite 3.53.1, hybrid `3.53.1-redline-4.1.0`, or keep 4.1.0 and abandon rusqlite drop-in?
2. **Migrate-on-open:** if `open("file.db")` sees `SQLite format 3`, auto-import to `file.db.redline/` or require explicit CLI?
3. **FTS ranking:** byte-identical bm25 vs “compatible class”? Byte-identical is expensive.
4. **Boolean rendering:** SQLite `0/1` vs Postgres `t/f` — session `standard_conforming_strings`-style GUC?
5. **Serializable:** never, or SSI later? Recommendation: never until FOR UPDATE exists and someone names a workload.
6. **LISTEN/NOTIFY:** keep deferred, or native WAL-notify on pgwire?
7. **Encryption default:** off (SQLite-like) vs required for server mode?
8. **Official bench durability:** is NORMAL acceptable for published “vs SQLite” numbers if labeled?
9. **parser.rs exemption:** split in Phase A even if it churns blame?
10. **CUDA:** is large-scale vector search a 12-month product goal? If no, do not staff GPU.

---

## 16. What a later reviewer should double-check

This audit did **not** re-run `just redline-testing-official` or the 1728-child certify matrix. It trusted on-disk `target/redline-testing/*` and source reads.

Re-verify before treating numbers as release evidence:

- `target/redline-testing/summary.json` still 2441/2445/0/4.
- `ranked.csv` still median ~1.68× (CLI, this host).
- `wal_checkpoint` behavior in `parser/pragma.rs`.
- `TRIGGER_DEPTH_CAP` in `exec/trigger.rs`.
- `sqlite3_prepare_v3` argument order in both `.h` and `core.rs`.
- `open_handle` still has no `:memory:` branch.
- `Engine::commit` Strict fast path still publishes before `flush_until`.
- `route_primitive_scan` still declines by default.

Kernel byte layouts in `ENGINEERING_SPEC.md` §§3–6 looked consistent with `format/page.rs`, `format/tuple.rs`, `wal/record.rs` on a spot check; catalog persistence and SQL capability tables in that spec did **not**.

---

## 17. Bottom line

RedlineDB is already a **serious native MVCC engine with SQLite-shaped SQL**. On the official 2445-case corpus it is **functionally at 100% of required cases** (0 fails) and **four optional virtual-table modules short of 100% of the suite**. That is not the same as a SQLite drop-in: the C ABI is a subset with ABI bugs, the file format is intentionally foreign, `:memory:`/READONLY are wrong in FFI, and capability PRAGMAs lie.

It is **not** a PostgreSQL drop-in in any networked sense. The server speaks a private JSON protocol. The beyond-SQLite corpus is almost entirely skipped. Track J/K shims will not satisfy `psql` or ORMs.

It is **not yet faster than SQLite** on the public parity bench (median 1.68×, tail 143×), and **already faster** on the concurrent-writer story the paper exists to tell. The “smarter internals” (morsel, VM, WAL pipeline, vector/JSONB AMs) are largely **built and dark**.

The path to the requested mission is not “clone SQLite then clone Postgres.” It is:

1. Unskip FTS/RTree/dbstat as native modules.
2. Finish SQLite SQL partials and an honest C ABI that real bindings can use.
3. Turn on the execution/WAL stack and overflow pages so the engine is faster and can hold real rows.
4. Add pgwire + catalogs for networked Postgres-shaped apps.
5. Import/export SQLite files without becoming SQLite.

That sequence keeps the codebase smaller than Postgres, competitive with SQLite in size, and pointed at the workloads (multi-writer OLTP, JSON, vectors, FTS) where a 2026 engine should beat both incumbents.
)

## EVID-01 implementation evidence — 2026-09-18

Implementer oracle agent; reviewer root; base `af20826311a067a51383c382013612545156df61`;
implementation and integrated commit pending (dirty canonical tree). Status: review.
Owned edits: `scripts/sqlite/build-reference.sh`, `build_reference.py`, `header-probe.c`,
`test_reference.py`, `README.md`; reserved bench asset test was not changed.

Reproducer: previous amalgamation builder passed `-DSQLITE_ENABLE_UPDATE_DELETE_LIMIT`
after parser generation, omitted SOUNDEX, and installed only a CLI. Qualified extended
SQL now runs `UPDATE t SET a=a+10 ORDER BY a LIMIT 1; DELETE FROM t ORDER BY a DESC LIMIT 1;`
on rows 1,2,3 and returns 2,3; soundex('Robert') returns R163. Exact SQL/outputs are
retained in each receipt. Full-source archive SHA3 is pinned to
`27cfc9264b2188fd17f811a8c03424eb65391c2ef9874cbfc860ea25f4322363`;
manifest UUID matches upstream 3.53.1 release source identity. Archive digest was
obtained from upstream HTTPS download, not independently published on the release page.

Implementation: separate ordinary/extended configurations, upstream parser generation
with `--update-limit`, static/shared library plus CLI/generated headers; CLI/library
compile options must match. Independent C consumer verifies header/version/preparation/
column-metadata APIs. Execution probes cover JSON/JSONB, math/percentile in both,
FTS5/RTree/rtree_i32/dbstat/SOUNDEX/DML LIMIT in extended, and shell-only series/uint.
Source/compiler/configuration/script/artifact identities bind the cache; every hit
checks all required artifacts and reruns probes. Failed qualification preserves the
previous installed reference. Build lock serializes publication.

Commands (all exit 0): `rtk proxy bash scripts/sqlite/build-reference.sh` (fresh build
and cached repeat); `rtk proxy env REDLINEDB_SQLITE_REFERENCE_PROFILE=ordinary bash
scripts/sqlite/build-reference.sh`; `rtk proxy python3 scripts/sqlite/test_reference.py`
(7 tests); `rtk proxy python3 -m py_compile scripts/sqlite/build_reference.py
scripts/sqlite/test_reference.py`. Tests reject rejected-positive SQL, wrong results,
wrong embedded identity, stale/incomplete/tampered cache artifacts and tampered source
downloads. Ordinary FTS5 and DML LIMIT probes both returned expected exit 1.

Limitations: Linux host only; packaging qualification remains open. `crates/bench`
still uses rusqlite 0.37 bundled SQLite and is **not** qualified by this builder.
Connecting that embedded bench oracle and the required CI qualification lane remains
integration work; this evidence does not establish complete EVID-01/SQLite-profile
qualification. No independent verification or integrated commit is claimed.

Artifacts:
- `target/sqlite-reference/3.53.1/oracle-identity.json` SHA256 `6ab98285bfde39e857b2c776ee13dc2cf1961e12cf41c3d86d909f90c99230ea`.
- `target/sqlite-reference/3.53.1-ordinary/oracle-identity.json` SHA256 `f8d966ca8783a9f8261d0c4cd8c124cb90058bebc96fdc2862121f3131cd11a8`.
- `target/sqlite-reference/qualification-tests.log` SHA256 `87610810e9c1ffd7f9e0d665e8dee7f5959defda6216972a230c17879e240f49`.
- `target/sqlite-reference/build-extended.log` SHA256 `5101c7b702ee3cf553cf55fd841d95b7201412eaeb583dbc5d75a6b493e5fdfa`.
- `target/sqlite-reference/build-ordinary.log` SHA256 `be1849ea9de1e4912575ba3a4512abcf7eabb71e0965405f0e7a56f7cb2951e9`.


## EVID-02 first-cycle implementation and evidence — 2026-09-18

Implementer: evidence agent; reviewer: root (independent core review received;
final review pending). Claimed paths: `subrepos/redline-testing/`; base commit
`af20826311a067a51383c382013612545156df61`; implementation/integrated commits
pending (dirty tree). Status: **review; partial EVID-02, not verified**.
Dependencies: qualified EVID-01 reference, EVID-03 inventory contract.

Reproducer: historical case 163 expected exit 0 but both engines exited 1 and
was reported passed. New comparator validates every fixture expectation on the
oracle first, then target. Signals, missing negative error assertions and all
148 disabled stdout comparisons fail. No fixtures were removed or rewritten.
`profiles/legacy-adjudication.json` preserves all twelve historical mismatches
(163,164,167,168,171,219,220,226,10546,11437,11438,11439), all 148 blind spots,
fixture hashes and qualified-reference reproduction status for every entry.

Acceptance exercised: matching positive failures and wrong outputs; valid output;
negative correct/wrong error assertions; signal termination; all assertion
channels; empty/duplicate/unmeasured selections; unknown capabilities; skips;
50ms process-group deadline; 1024-byte output limit with retained partial output.
Capability/version probes now share bounded execution, propagate infrastructure
errors and reject signals. REGEXP is registered explicitly after the strict
validator exposed the prior silently ignored uppercase capability token.
Missing target modules run as failures; missing reference capabilities fail the
gate. Child guards terminate process groups and reap on all exits.

Commands from runner root: `rtk cargo fmt --check`, `rtk cargo check --locked`,
`rtk cargo test --locked` all exit 0 (61 passed, 1 existing ignored PostgreSQL
oracle test); `rtk just release-local` exit 0. Raw logs in
`target/compatibility/evidence-cycle1/{fmt,check,tests,package}.log` at root.
Release packaging includes the profile and adjudication JSON/README.

Current CLI built with `rtk cargo build -p redlinedb-cli --locked` (exit 0).
Full strict corpus: `cargo run --locked -- run --suite sqlite_parity --target-bin
/home/ubuntu/redlineDB/target/debug/redlinedb --sqlite-bin
/home/ubuntu/redlineDB/target/sqlite-reference/3.53.1/bin/sqlite3 --workers 4
--repetitions 1 --warmup 0 --progress always --tmp-root
/home/ubuntu/redlineDB/target/compatibility/evidence-cycle1/tmp --output
/home/ubuntu/redlineDB/target/compatibility/evidence-cycle1/strict.raw.jsonl`.
Exit **1**: 2445 cases, 2147 passed, 296 failed, 2 required skips (.dbinfo/.recover).
Failures: 139 oracle fixture invalid, 98 missing coverage, 59 target fixture
mismatch. Counts overlap blind-spot inventory because oracle qualification runs
first. Raw SHA256:
`a71dc30d0bcde40da13cc75f8e60292ea5e9fff514ee7b70a88e14fefae93f9c`.
Full command, parent commit, dirty state, platform, binary/source/corpus hashes:
`target/compatibility/evidence-cycle1/provenance.json`. Failure artifact paths
are retained per JSONL row. These are failed development results, not qualified
compatibility evidence. Initial command-option and unknown-capability failures
are also retained in that evidence directory.

Remaining EVID-02: typed cells, structured error codes/offsets, transaction state,
result boundaries, per-operation float/order rules, replacing each blind spot
with semantic checks, exact CLI policy, per-case structured timeout records and
complete defect-baseline enforcement. All 12 fixtures need individual semantic
adjudication; no expectation was guessed. Process bounds are polled (brief
overshoot possible). CLI comparison alone cannot prove SQL/API compatibility.

EVID-03 runner interface implemented by coordination with root: manifest under
`profiles/sqlite-3.53.1-app-v1.json`, 33 open required roadmap buckets, 6 exclusions,
0 verified, inventory explicitly incomplete. Startup validates IDs/dispositions,
case references and evidence presence; profile qualification always rejects an
incomplete inventory. `REDLINE_TESTING_QUALIFY_PROFILE=1` selects that gate.
Development runs separately report requirement counts. Tests reject fabricated
verification/unknown references/duplicate IDs and qualification of the current
manifest. Behavior-level inventory and reviewed evidence remain open.

### EVID-01 review correction — shell DBPAGE dependency

Independent strict-corpus review found `.dbinfo`/`.recover` absent after clearing
upstream CLI-only options. Both are compiled under `SQLITE_ENABLE_DBPAGE_VTAB`;
BYTECODE is not required for these commands. Add DBPAGE to both profiles in both
CLI/library builds, plus SQL execution, `.dbinfo` field checks, and `.recover`
round-trip into a fresh database with recovered-value/integrity assertions.
Requalification is in progress; prior EVID-01 hashes above are superseded by this
review correction and must not be used for final corpus identities.

EVID-01 shell correction requalification completed: fresh ordinary and extended builds,
cache reuse, seven qualification tests, Python compilation and whitespace checks all
exit 0. `.dbinfo` reports the expected 4096-byte pages/one table; `.recover` output
restores row `7|recovery probe` into a fresh database and upstream integrity_check
returns `ok`. DBPAGE SQL execution and exact CLI/library option equivalence pass.
Final extended CLI SHA256: `a83535427fb304aa238473d19073fe57eecb7512f47c76ec8af97cfae2c924df`.
Evidence agent notified to rerun strict corpus against this corrected oracle.
These artifact hashes supersede the earlier EVID-01 artifact list:
- `target/sqlite-reference/3.53.1/oracle-identity.json` SHA256 `5856df67ce1f1d55ba35b50178679e4fd4684bb8b0b90bc399fbded2f3e1d86c`.
- `target/sqlite-reference/3.53.1-ordinary/oracle-identity.json` SHA256 `d77d82dbd231e70036ed850ffd647a0ded86e4004de5b9ff3f769fa9a9e30045`.
- `target/sqlite-reference/qualification-tests.log` SHA256 `7090d367f6056879e9aa9ec34e3cf5e30428999a17c3641f249fa4b76cee20ef`.
- `target/sqlite-reference/build-extended.log` SHA256 `721d1fd153e348217049012df961c40cc398aeb1bec9914b8e5c83a909ba2ec7`.
- `target/sqlite-reference/build-ordinary.log` SHA256 `12eb89981cb02f2f29cb10f8461d31c30400d424deb0f823c3aa1d4150a356cf`.

## SAFE-01 integration review — 2026-09-18

Scope: preserve flush-before-publication patch; independently review failure
outcomes; add live barrier/lock and abrupt process-exit regressions. Implementer:
root (original patch pre-existed); independent reviewer: SQL regression agent.
Acceptance observed: before barrier release, writer remains InProgress, old value
is visible, competing write times out; after release, existing snapshot stays
stable and fresh snapshot sees commit, lock is reusable. Strict acknowledged row
survives child process exit without destructor/checkpoint.
Command: `rtk cargo test -p redlinedb-kernel --locked --features failpoints
--test failpoint_smoke --test strict_commit_recovery --test engine_tests`, exit 0:
24 + 7 + 1 tests pass. Raw: `target/compatibility-cycle1/kernel-tests.log`;
hashes captured in cycle receipt. Details: `docs/compatibility/durability-review.md`.
Implementation/integrated commits: pending (dirty canonical checkout).
Status: review/partial, not verified. Returned I/O uncertainty, panic lock/CSN
cleanup, ENOSPC and all publication-boundary crashes remain open.

## EVID-04 CI inventory activation — 2026-09-18

Root owns CI dispatch/workflow/testing documentation; evidence agent independently
reviewed matrix/dispatcher/aggregate correspondence. Replaces three named SQL
integration binaries with four required nextest hash partitions of all SQL targets.
The local sql-contracts aggregate executes all four even on failure. Oracle
qualification is a separate required stage for both reference configurations.
`bash -n ops/ci/fast.sh`, `cargo fmt --check`, and `git diff --check` pass.
Inventory partition JSONs: `target/compatibility-cycle1/sql-shard{1,2,3,4}.json`;
344/366/324/359 runnable tests, 1,393 union, no overlaps/missing original tests;
one extra quoted-rename regression was added after original inventory. Four
existing ignored cases remain outside qualification and visibly missing coverage.

Full diagnostic command `rtk cargo nextest run -p redlinedb-sql --tests --locked
--no-fail-fast` exits 100: 1,389 pass, 3 fail, 4 skipped (1,392 run before final
quoted-name regression addition). Failures: unrelated-view ALTER regression,
`w5_planner_trace_path_emits_jsonl_on_ir_decision` (empty trace),
`queue_claims_are_unique_under_contention` (LockTimeout). Raw complete diagnostics
are `target/compatibility-cycle1/sql-all.log`. No baseline exemption introduced.
Default `rtk just fast` exits 1 at preserved 6,911-line CLAUDE_GAPS.md size check,
after workspace test compilation and formatting; raw `fast.log` in same directory.
No audit was removed or limit weakened to pass the gate.

Additional unclaimed PRAGMA source/test edits appeared during execution in
`crates/sql/src/exec/pragma_tv.rs`, `crates/sql/src/parser/pragma_compile.rs`, and
`crates/sql/tests/parity_pragma_tv.rs`. None belong to this team; preserved without
modification. Binary-hashed corpus results and focused test reruns must be
distinguished from a final clean integrated release. No release qualification.


EVID-02/03 final first-cycle follow-up: independent root review requested an exact
profile ID and qualified-reference identity gate; both added. Every official
SQLite run now requires a receipt matching pinned version/source, extended
configuration, successful CLI/library probe declaration, CLI binary hash and
executable version/source text. Missing/wrong receipts have regressions. Runner
suite: **62 passed, 1 existing ignored**, exit 0; fmt/check exit 0. Identity gate
is not a signature check and cannot replace oracle qualification review.

After the oracle agent enabled actual .dbinfo/.recover support, final strict
`strict-v3.raw.jsonl` run (same command above with tmp-v3/strict-v3 paths) exits 1:
**2445 total, 2147 passed, 298 failed, zero skips**. Failures: 138 invalid oracle
fixtures, 101 missing comparisons, 59 target fixture mismatches. The earlier
run's two reference skips are resolved, not hidden. Raw SHA256
`d959563d38136d7ddd4837f0bd5fa18dba098692547f9b034b1cdfe99f1a9aa7`;
`target/compatibility/evidence-cycle1/provenance-v3.json` SHA256
`6d319df5044abb59758e429e6780eb4fc4c5ade524ca4174526f57426bb5bb64`.
All twelve historical mismatches and 148 blind spots carry final v3 reproduction
results in the retained adjudication inventory. Original/v2 evidence preserved.

Limitation: unclaimed concurrent PRAGMA edits appeared after CLI build; raw rows
identify executed binaries, but the later dirty-source snapshot is not an exact
CLI-build-source guarantee. Evidence remains development-only. Root official
parity currently consumes an external pinned runner; included-runner comparator
changes need explicit lane integration/artifact adoption before claiming remote
CI coverage. No release published and no old artifact overwritten remotely.


EVID integration review follow-up: root added required `sqlite-evidence` fast/CI
stage building the included runner and CLI, qualifying reference identity and
running runner tests plus strict corpus. Evidence agent independently reviewed
matrix dispatch, root-relative paths, fail propagation and local aggregate
inclusion; no blocking code issue found. Artifact upload retention was requested.
The previous note about the pinned official lane remains true, but the separate
new required lane now protects this comparator once integrated. No CI execution
claim is made by source review alone.

### EVID-04 integration follow-up

Added required `sqlite-evidence` CI stage using the current included runner and
current CLI, retaining the separately pinned official release lane. Independent
review: evidence agent. Actual stage invocation
`CI_FAST_STAGE=sqlite-evidence bash ops/ci/fast.sh` exits 1 with 2,147 passes,
298 failures, zero skips; raw `target/compatibility-ci/strict.raw.jsonl`,
log `target/compatibility-cycle1/evidence-ci.log`. No expected defects hidden.
Workflow always uploads raw results, failure/skip diagnostics, reference receipts
and build logs from compatibility stages, including failed runs.
`CI_FAST_STAGE=sqlite-oracle bash ops/ci/fast.sh` exits 0; seven tests and both
build configurations pass. Actionlint exits 0; shellcheck flags pre-existing
SC1091 source-following and SC2155 export-assignment warnings (exit 1).
The planner trace failure reproduces in isolation (exit 101); the queue contention
test passes alone (exit 0), leaving a load-sensitive failure requiring diagnosis.
These failures are not marked fixed or exempted. Source-size and other unfinished
roadmap tasks still prevent a green aggregate.


EVID-03 automatic provenance follow-up: runner-owned `provenance.rs` now writes a
pending sidecar before engine qualification/probing, updates cached engine
identities/receipt hash, and preserves failed/completed result plus raw hash.
Snapshot binds parent commit and all-untracked dirty status, OS/architecture,
runner binary, profile manifest, complete/selected serialized corpus and run
configuration. Git unavailable is explicit for extracted packages. Direct callers
initialize raw output after pending snapshot; preflight failures cannot claim
stale bytes from an older run. Failure before initial catalog serialization cannot
produce a snapshot; interrupted execution may leave pending, never success.

Final provenance verification corpus v4 again exits 1 with 2147 passed, 298
failed, zero skips. Raw file `target/compatibility/evidence-cycle1/strict-v4.raw.jsonl`
SHA256 `1e57e41f8d7e86d275fae698e41841ea0c7537448928e5aef6b42ab4d7e3a699`;
automatic sidecar `strict-v4.raw.provenance.json` SHA256
`7e790a2ceed4c92756b296be56337c9f4945116d8d0d5550ed6b86e6c4a20daa`.
Root independently checked failed stage, raw hash, corpus/profile/oracle identities
and failure result. Added pending/failure/hash preservation and stale-output
regression; runner suite now 63 passed, 1 pre-existing ignored, exit 0; fmt/check
exit 0. Required CI upload includes sidecar automatically. Final package proof
recorded in `target/compatibility/evidence-cycle1/package-provenance.log` and
`.exit`. EVID-02/03 remain partial pending semantic gaps and complete inventory.

## Cycle integration receipt

Root independently verified automatic failed-run provenance and raw hash on v4,
reviewed oracle configuration/execution checks, manifest validation, comparator
assertions and narrow ALTER changes; scanner review caught and repaired two
quoted-identifier regressions before final focused checks. Root final receipt:
`target/compatibility-cycle1/receipt.json`. This captures artifact hashes and a
dirty source snapshot, not proof of clean immutable build inputs. All original
changes and concurrent unclaimed PRAGMA edits were preserved.

Implementation summary and outstanding acceptance work:
`docs/compatibility/implementation-cycle1.md`. No commit/release was created.
The complete roadmap is NOT finished; SQLite qualification is NOT achieved.

## P2-SAFE kickoff evidence — 2026-09-18

Storage agent implemented only `crates/kernel/tests/strict_commit_faults.rs` and
`docs/compatibility/phase2-durability.md`; reviewer root. Base remains
`af20826311a067a51383c382013612545156df61` plus preserved dirty changes;
implementation/integrated commit pending. Review status covers this bounded
kickoff artifact; SAFE-01 defect remains open and safety qualification fails.

Exact command: `rtk cargo test -p redlinedb-kernel --locked --features failpoints
--test strict_commit_faults -- --nocapture`, **exit 101, 0 passed, 1 failed,
0 ignored**. Existing `engine::commit::before_publish` injects panic after Strict
flush, NOT returned write/fsync Err. Caught panic leaves local Aborted state,
old row visible, retained row lock (contender LockTimeout), pending CSN2.
Later unrelated commit returns Committed(Csn3), but fresh reader sees no row
because published frontier remains1. Child exits without destructors; reopen
recovers both interrupted new row and later row. Fixture actually fails the
required recovery-fence assertion; no inverted bug-reproduced success.

Design contract maps WAL, commit guard/reservation, engine-shared terminal fence,
SQL explicit/autocommit sequence handling, index rebuild and FFI callers.
Next slice requires exclusive `wal/manager/storage/write.rs` claim for separate
returned-Error hooks around write/sync; production hook edits explicitly deferred
by root during kickoff. Existing flush hook returns fake success and cannot
honestly stand in for returned I/O failure. Minimum design never publishes
unflushed state, never treats MaybeCommitted alone as a fix, and prevents
checkpoint/pruning/reuse until quiescence and exclusive recovery.

Acceptance still required: returned EIO/ENOSPC, partial/full writes, sync outcomes,
writer panic/waiter termination, shared connection fences, schema/index/sequence
recovery and maintenance interactions. Formatting and scoped diff check exit0.

- `target/compatibility-phase2/strict-commit-faults.log` SHA256 `1a849809f7ce9c1f6edb6eb15b0d087ba376c4959a446415257bd18a02c02b10`.
- `target/compatibility-phase2/strict-commit-faults.exit` SHA256 `39b8dc3fc8b44765c8e6f1adee04c5b465e555ab791cc42d0d9e810d5b64297c`.
- `crates/kernel/tests/strict_commit_faults.rs` SHA256 `4e874872b542ed2981b28cfffb797367305d38afddd5680140c453d590f56e1d`.
- `docs/compatibility/phase2-durability.md` SHA256 `d8bedd127f3bb36242e76314c19eba2fcb250a2031c19df2c21c067e4d51cf75`.

### P2-INTEGRATE kickoff result and shared decisions

Detailed execution plan: `docs/compatibility/phase2-plan.md`. Root verified local
Git topology: one root, ordinary component trees, no gitlinks or .gitmodules;
`redlinectl validate` exits 0 for all six components (history/remote retirement
not claimed). Evidence: `target/compatibility-phase2/repository-layout.json`.
Corrected cycle-one/testing documentation about official runner source authority.
Also fixed obsolete shell-version scraping in `scripts/just/run.sh`: actual
qualified CLI returns `version-3.53.1`; failed CLI exits 1; bash syntax passes.
Evidence: `target/compatibility-phase2/reference-score-route.json`. Full report
pipeline remains a next integration gate.

Production ABI target selected for planning: v5.0.0-alpha.1, versioned v5 prefix,
Linux libredlinedb.so.5 / macOS libredlinedb.5.dylib; no global SQLite alias or
v4 replacement. This decision is not an emitted package or production ABI fix.
Rust remains production implementation language; upstream C/independent consumers
serve only as qualification controls.

Root independently checked ABI receipt counts and loaded-library hashes (9/9
SQLite controls, 2/9 Redline passes, 4 assertion failures and 3 isolated signals),
reviewed storage reproduction and explicit failing exit101, and reconciled
triage dimensions (298 failures,12 legacy mismatches,148 blindspots). No fixture
or comparison was weakened and no new exclusion/denominator edit was applied.
Kickoff artifacts are reproduced/under review; production safety/ABI/schema
fixes remain open. No commit or release was created.


### P2-EVID bounded kickoff receipt — 2026-09-18

Status: **review**, not an engine fix. Implementer evidence agent; root independently
reviewed denominator/mapping and phase-plan consistency. Base HEAD unchanged;
implementation/integrated commit pending, dirty tree preserved. Exclusive edits:
`subrepos/redline-testing/profiles/phase2-failure-triage.json` and
`docs/compatibility/phase2-evidence.md`; no fixture/source changes.

All **298 failures** map exactly once to 138 oracle-invalid,101 missing-comparison,
59 target-mismatch outcomes; grouped into 44 investigation families, NOT44 proven
engine defects. All12 historical exit mismatches and148 disabled-output identities
are mapped individually; blindspots split101 missing,41 target,6 oracle-invalid.
133/138 oracle-invalid cases share stdout hashes,131 share stdout/stderr/exits;
this suggests stale assertions, not proof of typed SQL correctness. Numeric
rendering accounts65, control-byte escaping9 and CLI layout/quoting48 oracle-invalid
cases. Remaining build/contract cases retain individual review requirements.

JSON includes exact fixture hash/source, diagnostic, four raw artifact hashes,
roadmap/group/acceptance/disposition for every failure, plus ten fresh paired
reference/target probes with exact stdin/argv/output/exits and binary hashes.
Five bounded Rust candidates: SOUNDEX, bail-off continuation, transaction errors,
savepoint name retention, scalar-subquery arity context; all require positive,
negative and state tests and appropriate SQLite-mode interfaces. Five bounded
fixture/shell candidates and ordered batches are documented. No values were copied
into fixtures; no target mismatch hidden. Existing session exclusion remains an
explicit owner-reviewed applicability decision, not an implemented feature.

Evidence inputs: v4 raw SHA256
`1e57e41f8d7e86d275fae698e41841ea0c7537448928e5aef6b42ab4d7e3a699`;
automatic sidecar `7e790a2ceed4c92756b296be56337c9f4945116d8d0d5550ed6b86e6c4a20daa`.
Triage JSON SHA256 `464f15e1c132c6f50346707407141cd40ea705ad97a8116d03a676174e85bcf1`.
Triage document SHA256 `8ffc515394cbcb051274dfba7130fb9e959c9b054bed2b0236a55705682585f0`.
Validation: `rtk proxy python3` inline reconciliation/hash validator exited0,
checking exact case/group coverage,298/138/101/59,12/148 identities,fixture/source/
artifact/probe hashes and all proposed source paths. Reproducible core command
is embedded in the doc. Ten fresh paired CLI probes (10-second deadline) wrapper
exited0; individual expected exit0/1 retained. `rtk git diff --check --
docs/compatibility/phase2-evidence.md
subrepos/redline-testing/profiles/phase2-failure-triage.json` exited0.
No builds/tests repeated for these documentation/data-only edits.

Reviewed root phase2-plan Wave1A: consistent with triage; explicit complete evidence
repair and no-new-regression baseline do not allow oracle errors/missing coverage.
Rust xtask generator is present; advertised rules directory is absent, documented
rather than inventing an alternate generation authority. Safety/ABI priorities and
profile applicability remain integration-owner decisions.
