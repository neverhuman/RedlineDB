# SQLite application profile v1 — qualification draft

Profile identifier: `sqlite-3.53.1-app-v1`. Status: **not qualified**; the
requirement inventory is incomplete. This document defines the target, not a
claim that the current release implements it. Corpus pass rates and symbols
exported by a library are not compatibility percentages.

The machine-readable inventory belongs to the included runner at
`subrepos/redline-testing/profiles/sqlite-3.53.1-app-v1.json`. Required requirement
coverage, required case results, known failures, missing coverage, exclusions,
and skips must be reported separately. An incomplete inventory prevents final
qualification even when all currently enumerated tests pass.

## Reference identity

The baseline is [SQLite 3.53.1](https://sqlite.org/releaselog/3_53_1.html), source ID
`2026-05-05 10:34:17 c88b22011a54b4f6fbd149e9f8e4de77658ce58143a1af0e3785e4e6475127e9`.
`scripts/sqlite/build-reference.sh` builds the reference from pinned full source.
Its receipt records the source, compiler, flags, builder and output identities.
CLI, generated header and library must come from the same configuration. Newer
SQLite releases are drift inputs until explicitly adopted.

The extended reference must execute FTS5, RTree/rtree_i32, dbstat, math,
percentile, SOUNDEX and ordered UPDATE/DELETE LIMIT probes. A macro in the
amalgamation compile command is insufficient to enable DML LIMIT: the option
must also be active during [parser generation](https://sqlite.org/compile.html#enable_update_delete_limit).
The ordinary reference is a separate configuration, never a substitute for
missing extended-profile behavior.

The inventory must include baseline changes: ALTER constraint operations,
REINDEX EXPRESSIONS, TEMP trigger access, JSON array insertion, shell formatting
and dot-command changes, numeric text conversion, new string APIs, preparation
flags, UTF-8 flags and parser-depth/configuration controls. Applicable APIs need
independent upstream-header fixtures; exclusions cannot be inferred from missing
implementations. See the pinned release specification for exact contracts.

## Application boundary

Included: SQL semantics and applicable PRAGMAs, prepared statements, constraints,
transactions, selected UTF-8 C API families and real consumer workflows; FTS5,
RTree/rtree_i32, JSON and selected reference functions; CLI batch scripting;
independent processes, read-only access, backup, large values, and explicit
SQLite import/export. The function/API/case inventory remains to be completed.

Excluded: direct use of SQLite runtime files, arbitrary VFS replacement,
SQLite C virtual-table extensions, binary loadable extensions, session/changeset
APIs, and shared cache. Native physical page numbers, WAL counts and dbstat
layout are documented differences; diagnostics must describe actual native data.

Native Rust APIs and CLI retain native defaults. The planned connection-level
selector supplies SQLite semantics to `sqlite3_*` and to CLI users selecting
`--compatibility sqlite`; it is not implemented by this document. PostgreSQL
listeners later select their own profile. Prepared identity must include the
profile, schema epoch and execution settings.

Native storage remains the runtime format. Conversion uses explicit import/export,
stable snapshots, staged validation and atomic publication, preserving sources.
No automatic conversion on open is permitted. Corrected public ABI signatures
ship under a new major identity, starting with v5 prereleases.

## Evidence and release gate

First validate reference results against fixture expectations, then compare the
target. Both engines failing a positive fixture is an invalid fixture result.
Missing comparison coverage is a failure. CLI text comparison alone does not
establish typed results, extended errors, offsets, result boundaries, counters,
post-error transaction state, or accessor lifetime correctness.

Evidence must bind parent commit and dirty state, runner/engine/corpus hashes,
oracle receipt, platform and comparison-policy version. Development defect
baselines must name stable IDs and signatures and still report defects as
failures; qualification permits no required failures or skips. No baseline may
allow missing cases, changed fixture identities, oracle failures or unexplained
new failures.

Final qualification requires complete executable requirements, independent
review, passing SQL/ABI/consumer/process/durability/module/interchange tests,
four-platform packaged-artifact qualification, and matching release/ledger/docs
identities. Applicable public SQLite Tcl and SQLLogicTest imports need provenance;
no proprietary TH3 or dbsqlfuzz access is assumed. Subsequent PostgreSQL,
performance and operational work retains the SQLite gate permanently.
