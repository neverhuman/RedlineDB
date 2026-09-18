# Phase 2 evidence reconciliation and bounded repair batches

Status: kickoff triage, independent review pending. No fixture, expected result,
engine source, or exclusion was changed. Canonical checkout only; base HEAD
`af20826311a067a51383c382013612545156df61` plus preserved dirty work. Implementer:
evidence agent; integration/review owner: root. Task: `P2-EVID`.

The complete machine-readable ledger is
[`phase2-failure-triage.json`](../../subrepos/redline-testing/profiles/phase2-failure-triage.json).
It lists every failed case, source fixture hash, exact diagnostic, observed exits,
artifact paths and hashes, roadmap mapping, proposed acceptance, disposition,
and one investigation group. It also maps all twelve historical exit mismatches
and all 148 disabled-output cases individually. Ten representative probes were
rerun against both binaries; exact stdin, argv, exit, stdout/stderr and binary
identities are embedded, not reconstructed from summaries.

## Reconciled denominator

| First failed comparator check | Cases | Interpretation |
|---|---:|---|
| Oracle does not satisfy authored fixture | 138 | Fixture/reference adjudication first; target may have additional defects |
| Comparison coverage missing | 101 | Existing assertions pass but cannot establish result/state equivalence |
| Target does not satisfy qualified fixture | 59 | Target mismatch; may concern SQL, errors, CLI, or an unresolved profile boundary |
| **Failures** | **298** | Exactly the failed records, not an engine defect count |

The v4 run contains 2,445 unique cases: 2,147 passed, 298 failed, zero skipped.
The ledger groups them into 44 investigation families, **not 44 proven defects**.
A first-failure comparator hides later problems. Neither three categories nor
44 families establish a remaining defect count or a compatibility percentage.

Of the 138 oracle-invalid cases, 133 have identical reference/target stdout hashes;
131 also have identical stderr hashes and exits. These observations strongly
support stale fixture assumptions in many cases, but do not establish typed SQL
correctness. In particular, fixture repair must expose the next comparison; it
must not bless two engines producing an incorrect shared outcome.

## Oracle-invalid cases: 138

| Investigation family | Count | Grounded observation / next action |
|---|---:|---|
| Numeric rendering | 65 | 49 math, 6 datetime, 7 windows, one aggregate and two REAL affinity snapshots use old decimal rendering; both engines emit matching new text |
| Shell control-byte rendering | 9 | Eight affinity BLOBs and UNHEX expect raw control bytes, whereas the shell emits caret escapes; preserve a separate hex/typeof semantic assertion |
| CLI layout/quoting | 48 | Headers, line syntax, tabs/Tcl/HTML/JSON/insert layouts and table spacing differ from old snapshots |
| Reference build assumptions | 4 | secure_delete twice, FTS module enumeration, complete compile_options list assumed another build |
| Catalog usage or absent command | 4 | 163/164/167/168 expect success for usage/unknown-command outcomes |
| Specific contract cases | 8 | median availability, excluded session API, help exit/channel, ifexists error, nofollow missing path, pagecache diagnostic, escape-newline assumption, sqlite_schema spelling |

The source of generated expectations is the **Rust** xtask generator under
`subrepos/redline-testing/xtask/src/generators.rs`, with capture in
`xtask/src/sqlite_runner.rs`. The advertised `corpus/sqlite_parity/rules/` path is
absent in this checkout; do not invent a second generator authority. Regenerate
selected matrices through the existing generator with an explicit qualified
SQLite binary, retain the before/after fixture hashes and review only intended
changes. Do not hand-edit `gen_*.json` or the historical generated manifest.

An exact compile-options fixture is not a suitable assertion that Redline must
impersonate SQLite compiler/page/allocator details. Bind reference qualification
to its receipt and test Redline registrations against actual execution. A profile
owner must decide treatment of `.session` and native physical diagnostics. An
exclusion remains recorded and does not become an implemented feature.

## Missing comparisons: 101

| Checker family | Cases | Required replacement |
|---|---:|---|
| Expected errors | 49 | Structured primary/extended error plus result boundaries and state after error |
| Intentional `.exit 7` | 1 | Requested exit and proof subsequent input did not run |
| Physical/diagnostic output | 14 | Real plan/checkpoint/allocation facts; no invented SQLite bytecode/page structures |
| Backup/recovery/archive files | 4 | Independent destination validation, reopen, source preservation and failure tests |
| Shell/system/external tools | 4 | Controlled executable/file fixtures verifying invocation and safe-mode behavior |
| Other CLI output/state | 29 | Exact deterministic output or explicit command-state assertions |

All 148 disabled-output fixtures remain failures. Their first-failure distribution
is **101 missing comparison, 41 target mismatch, six oracle-invalid**. Merely
switching `compare_stdout` on for empty output would turn many expected-error
cases into false positives again. Add an error/state checker first.

## Target mismatches: 59

The JSON contains all 25 target investigation families. Important duplicate
reductions are six transaction cases to three observed outcomes, two scalar
subquery cases to one arity diagnostic, two readonly/query_only cases to one
rejection contract, three SOUNDEX examples to one dispatch rejection, and four
module cases to a common CREATE VIRTUAL TABLE blocker spanning three module
families. They remain separate case instances and all remain required failures.

Several mismatches need more than changing strings: added-column CHECK case
10413 reports `no such column: b` instead of applying its CHECK; RAISE outside a
trigger (10570) produces a constraint error rather than rejecting placement;
readonly case 10207 opens a SQLite-format fixture directly despite the explicit
conversion boundary. Resolve semantics/profile applicability before rewriting
any expected diagnostic. `WITHIN GROUP` percentile syntax is rejected by this
SQLite build despite its two-argument percentile function being enabled.

## Ordered implementation batches

1. **P2-EVID-A: repair oracle assumptions with retained evidence.** Start with
   the 65 numeric snapshot cases, then nine control-byte and deterministic CLI
   cases. Qualify the generator against the pinned receipt; keep operation-specific
   typed assertions separate from shell formatting. Hand-authored expectation
   changes need per-case review. Exit gate: no newly hidden target mismatch,
   fixture identity register updated, positive/negative oracle self-tests green.
2. **P2-EVID-B: replace blind comparisons.** Implement structured error and
   post-error state probes for the 49 expected-error cases, then deterministic
   CLI/state checkers. Claim runner/error interfaces exclusively. Keep all 148
   entries visible until each has its replacement acceptance evidence.
3. **P2-API: explicit SQLite profile and structured error renderer.** The five
   small Rust candidates below must carry real values/error context, not mutate
   native semantics globally. Claim parser/session/error/public API files before
   editing and coordinate with the ABI agent. Exit gate: codes, offsets where
   applicable, state transitions, prepared reuse and native-mode regression tests.
4. **P2-SQL: mutation/schema correctness.** Prioritize the independent ALTER
   regression, added-column CHECK binding, RAISE placement and statement failure
   outcomes. Then DML LIMIT through shared mutation/index machinery. Transaction
   and durability gates outrank corpus reductions.
5. **P2-MOD/CLI: larger boundary-dependent work.** Transactional module interface
   precedes FTS5/RTree/dbstat. Real readonly, backup/conversion and native
   diagnostics require their storage/profile contracts; they are not quick
   cosmetic ways to remove failures.

## Ten small, high-confidence starting points

These are bounded proposed changes, not implemented fixes. Full exact fixtures
and fresh outputs are in `small_fix_probes` in the JSON. SQL snippets below omit
only the fixtures
