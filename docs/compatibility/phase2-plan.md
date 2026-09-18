# Phase 2: safety, ABI foundations and trustworthy regression gates

Date: 2026-09-18. Integration owner: root. Controlling ledger: `GROK_GAPS.md`.
This is the next implementation phase, not a claim that the Rust engine or the
SQLite application profile is finished. Execute this phase before expanding
optional modules, PostgreSQL support or performance work.

## Repository decision, verified locally

Use **one Git monorepo with imported component subtrees and separate Cargo
workspaces**. These describe different layers, not competing choices:

| Layer | Authority and working rule |
|---|---|
| Git | `/home/ubuntu/redlineDB`, one branch/commit history and root CI |
| Engine | Root `Cargo.toml`, `crates/`, root `Cargo.lock` |
| Conformance | `subrepos/redline-testing`, its own Cargo workspace/lockfile |
| Console | `subrepos/redline-web`, its Rust workspace and web package lock |
| Client | `subrepos/redline-central`, its own Cargo workspace/lockfile |
| Release tools | `subrepos/redline-split-ops`, retained name, monorepo-aware tooling |
| Historical hub | `subrepos/redline`, historical material, not a second engine authority |

Evidence: `subrepos.toml`, `PUBLIC_MONOREPO.toml`, migration README, Git tree
entries of mode 040000, zero gitlinks, no `.gitmodules`, and the same Git root
when invoked from the testing directory. `redlinectl validate` passes for all
six declared components. Raw evidence is under `target/compatibility-phase2/`.
This validates local topology, not remote-repository archival or complete
recovery history. No new migration, subtree push/pull, submodules, duplicate
checkouts or Git worktrees are needed for this phase.

Correction to cycle-one reporting: `ops/ci/lib.sh::ci_install_redline_testing`
already builds the included runner at this checkout. The release-download helper
is a separate historical path. Both default development and official runner
routes must bind the same parent commit and correct binary hashes. Root workflows
are active; nested workflows and old multi-repository promotion instructions are
historical unless explicitly invoked for reproduction.

Rust remains the production engine implementation language. SQLite C sources are
the oracle, and C consumer fixtures test the exported ABI. They are not a route
to replacing the engine with SQLite. Existing build/CI scripting may remain in
its current language. No GPU or C++ implementation work enters this phase.

## Starting evidence and phase objective

Latest strict corpus: 2,445 cases, 2,147 passes, 298 failures, zero skips. The
failure classes are 138 invalid oracle fixtures, 101 missing comparisons and
59 target mismatches. These are case outcomes, not independent defect counts.
The 148 historical disabled comparisons overlap these classes; the agent must
reconcile that mapping before proposing any denominator change.

Kickoff findings already reproduced:

- Nine independent ABI controls pass against SQLite; Redline passes two and
  fails seven. Three failures terminate isolated children (two SIGSEGV, one
  SIGABRT). Preparation bounds/signature defects are immediate priorities.
- A post-flush commit panic leaves a held row lock and reserved CSN. Continued
  use can acknowledge another commit that is invisible to fresh readers until
  reopen. The new safety test fails, exit 101; this is not a returned-fsync-error
  test or a completed fix.
- All 148 blindspots map to 101 missing comparisons, 41 target mismatches and
  six invalid-oracle outcomes. None passed. Most invalid-oracle cases need
  fixture/baseline adjudication rather than an assumed engine rewrite.

Known integrity blockers include unresolved column dependencies in ALTER,
commit uncertainty after WAL failures, and incomplete ABI preparation/value
semantics. The full SQL diagnostic run also found a persistent empty planner
trace and a load-sensitive queue LockTimeout. Four existing ignored SQL tests,
one ignored PostgreSQL-oracle test, the incomplete requirement manifest and the
preserved audit-file size failure remain visible.

**Phase objective:** establish an integrated, reproducible development baseline
with sound comparison coverage, fix the identified commit/schema/ABI foundation
defects, and leave every remaining incompatibility explicitly mapped to a
requirement, reproducer, owner and acceptance test. Do not equate this phase with
complete SQLite compatibility.

## Team already started: one coordinator and three workers

| Role | Kickoff task and concrete artifact | Initial exclusive scope |
|---|---|---|
| Root integration owner | This plan, repository/CI routing audit, shared-interface decisions and independent review | Phase-two plan, ledger decisions, testing documentation |
| Evidence worker (`evidence`) | Reconcile all 298 failures, all 12 historical exit mismatches and all 148 blindspots; rank grounded repair batches | `profiles/phase2-failure-triage.json` in included runner; `phase2-evidence.md` |
| Storage worker (`oracle`, reassigned) | Reproduce uncertain commit/resource-cleanup behavior and specify safe Rust failure/fencing contract | `strict_commit_faults.rs`; `phase2-durability.md` |
| ABI worker (`sql_regressions`, reassigned) | Compile against the qualified upstream header; execute isolated preparation/type/guard-page probes against both actual libraries | Independent C fixture, probe script, `phase2-abi.md` |

Kickoff probes and designs are bounded deliverables, not completed engine fixes.
They must report observed failures rather than invert expectations to make a
known defect pass. Root reviews these artifacts, then assigns production files
for the following implementation waves. A worker moves to the next bounded task
when its result is integrated; no overlapping ownership of hot files.

## Ordered implementation waves

### Wave 0 — freeze identities and reconcile the first cycle

Root records the dirty source snapshot, prior owners and exact artifact hashes.
Preserve the parser refactor, durability patch, audit documents and unclaimed
PRAGMA edits. Review and commit explicit owned slices; never stage the whole
working tree or absorb unrelated edits. Each tested integration commit gets an
evidence receipt; dirty-tree diagnostics remain distinguishable from that proof.

Audit the actual official/development runner routes, reference receipt, packaged
runner metadata, nested workspace selection and aggregate failure propagation.
One concrete integration defect already found: `default_sqlite_score_ref` in
`scripts/just/run.sh` still extracts a shell `version=` assignment removed by the
new Python-backed reference builder. Replace source-text scraping with the
qualified reference's identity and test the report route before declaring it
working. The kickoff now fixes that version-selection function using the
selected reference CLI; the real CLI returns `version-3.53.1`, and a failed
reference exits nonzero. Evidence: `reference-score-route.json` in the phase-two
artifact directory. Full report execution remains an integration gate.
Retain all audits while separately reconciling the file-size policy;
do not delete audit content or weaken production source limits to obtain green CI.

Exit: repository authority documented, work claimed, no unexplained dirty edits
included in commits, source-built runner paths verified, and report/reference
selection no longer depends on obsolete script text.

### Wave 1A — evidence repair and explicit defect baseline

The evidence worker owns the included runner and fixture repair batches after
claiming exact files. For each oracle-invalid fixture, distinguish invalid SQL,
reference configuration, changed SQLite 3.53.1 behavior, stale expected output,
and an actual oracle/harness failure. Store original fixture hash, exact oracle
output, supporting contract and reviewer rationale for each correction.
Also reconcile applicability with the already agreed profile: for example the
session-shell case must be assessed against the existing session/changeset
exclusion. Preserve its fixture and results in the corpus. Any non-applicable
requirement stays separately reported, never contributes an implemented pass,
and needs an explicit root-reviewed denominator change; no new exclusion may
be invented to hide an implementation defect.

Replace every disabled comparison with an appropriate assertion: exact shell
output, typed rows, ordered/multiset comparison, filesystem effect, or database
state after reopen/error. Never turn comparison off, substitute a broad float
tolerance, copy target behavior into expected output, or remove a failing case.
Prioritize numeric formatting and common shell-output families only after
confirming the pinned baseline; preserve the semantic purpose of each fixture.

Introduce a reviewed development defect baseline keyed by stable case ID,
fixture hash, failure stage and precise failure signature. It may explain known
target defects; it cannot authorize missing comparisons, invalid oracle results,
missing/duplicate cases, changed identities, unexpected skips or infrastructure
failures. Report known defects as failures even if a separate no-new-regressions
check passes. The raw qualification gate remains red until required failures are
zero. Resolved defects must be removed from the baseline, never silently restored.

Exit: all 298 cases reconciled; all 138 oracle-invalid outcomes adjudicated with
reviewed repairs or documented pre-agreed non-applicability; all applicable
historical blindspots have real comparison/state checks and all 148 have a
reviewed disposition;
zero invalid-oracle or missing-comparison outcomes in the required corpus; all
remaining target failures have requirements and reproducible signatures. Publish
both case and requirement reports, with inventory completeness still explicit.

### Wave 1B — commit failures, fencing and recovery

The storage worker first adds distinct returned write/fsync error hooks; the
existing panic and fsync-skipped-success hooks cannot stand in for these faults.
Test errors before write, after bytes reach the OS, before/after sync, and before
publication. Track the observer, writer, catalog/index state, reserved CSN, row
locks, API outcome and recovery outcome at each boundary.

Root and storage worker agree the internal Rust contract before production edits:
which failures prove non-commit, which are uncertain, when the engine/connection
must be fenced, and how reopen establishes an authoritative outcome. Merely
returning `MaybeCommitted`, pretending an uncertain write aborted, or publishing
unflushed data is insufficient. Audit every caller in SQL, Rust API, FFI and CLI;
none may automatically retry an uncertain transaction as if it never committed.

Implement panic/resource cleanup without allowing a caught panic to resume an
unsafe engine. Keep Strict flush-before-visibility and lock release. Verify that
catalog/index updates obey the same outcome contract. Add child-process death,
returned I/O error, ENOSPC and reopen regressions around the selected boundaries.

Exit: acknowledged commits survive the declared process-failure model; error
outcomes cannot falsely promise rollback; unsafe continued use is rejected;
reserved CSNs and locks cannot silently strand future work; fault tests and
existing transaction/recovery tests pass with independent review. Record power
failure/hardware assumptions separately rather than claiming they were tested.

### Wave 1C — ABI preparation and value foundations under v5 identity

The ABI worker's upstream-header fixture is the authority for argument order,
unsigned flags, bounded reads, null/empty statements, error output initialization
and tails. Use protected pages and independent child processes so invalid reads
are observable and cannot kill the entire harness. Verify actual loaded library
paths/hashes; system SQLite controls cannot count as Redline passes.

Before correcting the exported `sqlite3_prepare_v3` signature, root establishes
the v5 prerelease ABI/product/package identity and updates direct call sites and
generated-header ownership. Planned first identity: `v5.0.0-alpha.1`, versioned
v5 installation prefix, Linux SONAME `libredlinedb.so.5`, and macOS install name
with major 5. Keep static libraries and headers in that same versioned prefix.
Do not install a global `libsqlite3` replacement or overwrite a v4 installation;
compatibility aliases require an explicit consumer opt-in. These are release
implementation requirements, not already emitted artifacts. Keep old releases
intact. Implement fixes in Rust:
bounded input access, statement preparation outputs, SQLite type tags and the
conversion/cache lifetimes exercised by the fixtures. Add reset/finalize/error
paths, embedded NULs, empty text/blob versus NULL and integer/real boundaries.

Exit: the selected upstream-header, guard-page, tail/type and conversion-lifetime
fixtures pass against Redline and the oracle; no mismatched C declaration masks
the calling convention. Corrected ABI has an explicit v5 identity. This does not
qualify all callbacks, threading, allocation ownership or real consumers; those
remain distinct ABI-03..06 work.

### Wave 2 — dependency-aware schema changes and connection semantics

After a worker frees up, assign it SQL-02 exclusively. Root owns interface review.
The SQL layer already depends on `sqlparser`; the kernel does not. Build resolved
dependency information in the SQL/binding layer and pass explicit schema patches
to the kernel. Do not create a kernel-to-SQL dependency or add another global
identifier-replacement heuristic.

Bind references to stable schema/table/column identities, including aliases,
quoted names, nested queries, trigger OLD/NEW and main/temp/attached qualifiers.
Validate the prospective schema before atomically publishing its changes under
the current transaction/schema epoch. Cover rejection and rollback, prepared
statement invalidation and reopen. At minimum, the outstanding `a.x` versus
`b.x` regression must pass without breaking true dependents or literals.

Introduce the explicit connection profile/settings snapshot contract before
profile-dependent diagnostic/PRAGMA fixes. Native stays the default for Rust/CLI;
SQLite C entrypoints select SQLite. Include profile/schema/settings in prepared
identity and remove the relevant thread-local ALTER semantic switch as its
replacement is integrated. Do not introduce more global semantic settings.

Exit: related/unrelated object tests, aliases, quoted identifiers, ambiguous
rename rejection, triggers/views, rollback and reopen pass; prospective changes
cannot partially corrupt the schema. Settings/profile isolation passes for two
connections and prepared statement reuse. Broader SQL-01/03/04/05 work remains
mapped to subsequent batches rather than inferred complete.

### Wave 3 — integrated acceptance and handoff

Freeze a reviewed integration commit, build engine/runner/reference artifacts,
and execute deterministic SQL shards, selected ABI fixtures, short fault/process
tests, comparator self-tests and the complete strict corpus. Run root family CI
for every changed component boundary; do not assume a root Cargo command covers
the independent testing/client/web/release workspaces. Retain command, exit,
raw artifacts, hashes and reviewer identity for every gate.

Reproduce the planner-trace failure and queue-contention failure as separately
owned defects. Either fix them with tests or retain precise development-baseline
entries; do not increase timeouts or retry failures until green without evidence.
Classify every pre-existing skip and define whether it is a required semantic
case or explicitly non-qualification performance test. Never count a skip as
implemented coverage.

Phase 2 is accepted only when wave exits have evidence, no new unexplained
regression exists, and remaining requirements are explicitly open. Root maintains
a completion matrix with **implemented**, **tested**, **independently reviewed**,
and **integrated** as separate facts. A passing development regression check is
not permission to publish a complete SQLite compatibility claim.

## Continuing until the Rust port is actually qualified

The next phases remain ordered and retain all earlier gates:

1. Complete SQL transaction/constraint/collation/trigger semantics and the declared
   UTF-8 C API; qualify unmodified selected rusqlite and CPython consumers.
2. Complete admission, read-only, backup, checkpoint/recovery, large values,
   cross-process rollback/WAL behavior and attached atomic transactions.
3. Implement transactional FTS5/RTree/native dbstat, required shell behavior and
   explicit SQLite import/export with integrity-checked round trips.
4. Finish the behavior-level requirement inventory, qualify four packaged platform
   artifacts and publish a precisely named SQLite 3.53.1 application profile only
   with zero required failures, skips, unknown coverage or integrity blockers.
5. Continue the agreed PostgreSQL, native execution/indexing, performance,
   security and operational roadmap while preserving the SQLite gate.

Completion is evidence-based, not a calendar promise or an agent saying its
portion is done. Agents reread/update GROK under the short advisory lock, submit
owned diffs and exact test evidence, and receive another bounded task after
review. Root alone controls shared contracts, denominators, integration commits
and release inputs. Do not start more than three workers beside the coordinator.
