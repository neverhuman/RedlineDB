# First implementation cycle — 2026-09-18

This is a development implementation and diagnostic result, **not completion of
the compatibility roadmap**. The profile inventory remains incomplete and all
major tasks retain open acceptance work in `GROK_GAPS.md`.

## Delivered

- A controlling ledger preserving both audits and existing changes, with profile
  boundaries, corrections, ownership, dependencies and evidence requirements.
- A pinned full-source SQLite builder with ordinary/extended configurations,
  parser-generated DML LIMIT, CLI/library/header consistency, executable probes,
  cache identities, independent C consumer and integrity tests.
- A comparator that checks oracle fixture expectations before comparing Redline,
  reports disabled comparisons as missing coverage, rejects invalid case sets,
  bounds child processes and fails on required skips. Official SQLite runs now
  require the qualified reference receipt and matching binary/source identity.
- An explicitly incomplete machine-readable application profile, qualification
  guard, and retained inventories for the twelve historical expected-exit
  mismatches and 148 disabled-output cases. No fixture expectation was guessed.
- Automatic provenance for failed and successful runs, binding profile/corpus,
  runner/engine/oracle identities and raw output hashes; stale output is cleared.
- Narrow ALTER fixes for unrelated trigger ownership and literal/comment/quoted
  token rewriting, with regression and reopen coverage. Full column dependency
  resolution is still missing.
- Independent review of the preserved durability patch, a live blocked-flush
  visibility/lock test and an acknowledged-commit process-exit recovery test.
- Required CI stages for reference integrity, current-runner strict evidence and
  four deterministic partitions covering every SQL test target. Failure artifacts
  are retained. Correction from the phase-two routing audit: the existing
  official lane also builds the included runner from this checkout; the old
  release-download helper is an explicit historical-reproduction path.

## Verification and limits

| Check | Result |
|---|---|
| Reference qualification/integrity | Seven tests pass; both configurations build and execute |
| Runner tests | 63 pass; one pre-existing ignored PostgreSQL-oracle test remains |
| Runner package | Local release package validated |
| Kernel durability suites | 32 pass |
| ALTER regressions | Seven pass; unrelated-view column rename still fails |
| ALTER scanner unit tests | Three pass |
| Pinned SQL regression oracle | Eight cases pass |
| Full SQL diagnostic run | 1,389 pass, three failures, four existing skips |
| Strict 2,445-case corpus | 2,147 pass, 298 fail, zero skips |
| Default `just fast` | Fails preserved audit file's size check |
| Workflow syntax/formatting | Actionlint, bash syntax and Rust formatting pass |

The full SQL run preceded addition of the final quoted-name regression, which
was tested separately. Its other failures are an empty planner trace (reproduces
alone) and a contention LockTimeout (passes alone). The required shard inventories
contain 1,393 runnable tests, with no overlaps or omitted tests from the original
inventory. The four ignored tests remain visibly missing qualification coverage.

The strict corpus's failures include invalid oracle fixtures, missing comparison
coverage, and target mismatches. Counts do not measure SQLite compatibility.
Typed-cell/error/transaction-state comparison, complete requirement enumeration,
individual blind-spot repairs and reviewed defect-baseline enforcement remain
open. The Rust bench's bundled reference is not qualified by the new standalone
reference builder.

Durability acceptance still needs returned write/fsync failure semantics,
uncertain-outcome fencing, panic cleanup and broader crash/fault matrices. See
`durability-review.md`. The C ABI, profiles in connection execution, modules,
storage/process work, conversion, consumers, platform packages and later GROK
roadmap are not completed by this cycle.

Raw logs live under `target/compatibility-cycle1/`; strict evidence is under
`target/compatibility-ci/` and `target/compatibility/evidence-cycle1/`. The cycle
receipt records artifact hashes and dirty source identities. Independent review
does not turn this changing dirty checkout into a qualified release commit.
Additional PRAGMA edits appeared outside this team's claimed files during the
run and were preserved. No release or commit was created.
