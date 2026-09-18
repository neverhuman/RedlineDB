# Qualified SQLite reference

Run `scripts/sqlite/build-reference.sh`. Its sole stdout result is the CLI path;
configuration/build diagnostics are retained in `target/sqlite-reference/build-extended.log`.
Python 3, a C compiler, make, and the upstream configure prerequisites are required.
The default extended prefix is `target/sqlite-reference/3.53.1`.
Set `REDLINEDB_SQLITE_REFERENCE_PROFILE=ordinary` for an independent ordinary build
at `target/sqlite-reference/3.53.1-ordinary`. A custom prefix, compiler (`CC`),
download URL and make job count use the existing `REDLINEDB_SQLITE_REFERENCE_*`
environment variables. A custom download must still match the pinned digest.

Both configurations build from the complete upstream source archive, generate the
parser, and install CLI, shared/static library and generated upstream headers.
The extended configuration enables FTS5, RTree/rtree_i32, dbstat, math, percentile,
SOUNDEX and UPDATE/DELETE LIMIT. Ordinary retains upstream core defaults, JSON
math and percentile, plus column metadata and DBPAGE, without the extended opt-ins. The CLI additionally
has upstream shell-only generate_series and uint collation registrations.
Both profiles execute `.dbinfo` and round-trip `.recover` output into a fresh
database, checking recovered values and `integrity_check`. DBPAGE is enabled
consistently for these shell operations and is also probed through both SQL paths.
The make shell option override removes upstream CLI-only compiler flags so that
CLI and embedded-library compile options must match exactly.

Qualification executes SQL against both the CLI and the directly loaded installed
shared library, compares exact outputs and compile options, and compiles/runs an
independent static-library consumer against the generated header. The receipt
`oracle-identity.json` contains the source identity, archive digest, compiler identity,
configuration, script hashes, artifact hashes, executed SQL and expected outputs.
Every cache hit rechecks artifact hashes and executes the probes. Failed builds
leave the existing published reference intact; a short file lock serializes builds.
The receipt is oracle qualification evidence, not evidence of Redline compatibility.

The archive SHA3 was obtained from the HTTPS upstream 3.53.1 full-source download
and pinned locally; its manifest UUID matches the independently published
[source identity](https://sqlite.org/releaselog/3_53_1.html). This is not a claim
that SQLite publishes that full-archive digest on its historical release page.
Parser generation uses upstream `--update-limit`, as required by the
[compile-options contract](https://sqlite.org/compile.html#enable_update_delete_limit).
Newer SQLite releases do not silently change this baseline.

Run `python3 scripts/sqlite/test_reference.py` after building the extended profile.
The negative checks reject failed positive SQL, wrong expected output and mismatched
embedded source identity, modified/missing cached artifacts and tampered downloads.
The Rust bench runner still has its separate rusqlite build; connecting that runner
to this qualified installed library is outstanding and must not be inferred from
this builder’s ctypes execution checks. Qualification currently runs on this Linux host; other
platform packages and full upstream/consumer suites remain separate release work.
