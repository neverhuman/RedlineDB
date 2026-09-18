# SQLite qualification inputs

`sqlite-3.53.1-app-v1.json` is an explicitly incomplete requirement inventory.
The runner validates its structure and case references at startup and reports
requirement counts separately from corpus counts. Set
`REDLINE_TESTING_QUALIFY_PROFILE=1` to demand profile qualification; the current
inventory necessarily fails that gate. Passing an ordinary development run is
never a claim of complete profile compatibility.

Comparison policy 2 validates the oracle against all fixture expectations before
checking the target. Disabled stdout comparisons remain coverage failures until
replaced with explicit comparison or side-effect checking. Negative cases require
error assertions. Target module absence is a test failure; skipped reference
capabilities fail completion. No case was removed or expectation rewritten.

`legacy-adjudication.json` retains all 12 historical exit mismatches and 148
stdout blind spots, including fixture identities and the old raw evidence hash.
Its disposition is invalid evidence or missing coverage, not a guessed SQL fix.
Qualified reference reproduction results are recorded for each case; individual
fixture/behavior adjudication remains open.

Current comparison still operates on normalized CLI text. Typed cells, error
codes/offsets, transaction state after failure, result boundaries and exact CLI
policy selection remain EVID-02 work. Presence assertions on negative errors do
not establish structured-error equivalence. The manifest is a roadmap inventory,
not a complete behavior-level denominator. Evidence presence validation does not
substitute for independent review of the artifacts.

Case and capability-probe processes have a 60-second deadline and 16 MiB combined
output bound. Unix execution uses a separate process group for termination.
Limit failures retain partial output files and are infrastructure failures;
full structured per-case timeout records remain future work. Bounds are polled,
so files may briefly exceed the bound before termination.

Official SQLite runs require the qualified extended reference layout
`<prefix>/bin/sqlite3` and `<prefix>/oracle-identity.json`. The runner checks the
receipt schema, pinned version/source, extended profile, successful CLI/library
probe declaration, CLI hash, and the executable version identity. An arbitrary
system SQLite or stale/missing receipt fails before execution; there is no
unqualified fallback. Receipt content is build provenance, not a cryptographic
signature or independent behavior qualification.

Each official SQLite run writes an automatic `.provenance.json` sidecar beside
the requested JSONL output (replacing its final extension). A `pending` snapshot
is persisted before engine qualification/probes; cached engine identities and
oracle receipt hashes are added before execution. Normal failed runs retain
`stage: failed`, the error and raw-output hash; successful corpus execution uses
`completed`. An interrupted process may leave `pending`, which is never success.
The sidecar includes parent Git commit/status (including untracked paths), runner
hash, OS/architecture, configuration and pinned policy/profile/full and selected
fixture hashes. Git absence is explicit when running extracted packages. Fixture
hashes use serialized `Case` arrays, with default fields and catalog ordering.
This does not prove the dirty source snapshot generated the executed binaries.
