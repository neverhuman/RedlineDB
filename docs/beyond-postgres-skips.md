# Beyond-Postgres skip-list policy

This document explains which beyond-SQLite cases in the corpus we deliberately
do NOT attempt to close against RedlineDB, and why.

The full skip set lives in [`metadata/beyond_sqlite/skip-list.toml`](../metadata/beyond_sqlite/skip-list.toml).
The list of cases we DO intend to close lives at
`/home/ubuntu/redlineDB/target/redline-testing/closable-beyond-pg.txt` after each
target-vs-reference triage pass.

## When to add a skip entry

Add a `[[skip]]` entry ONLY when one or more of the following is true:

1. **No SQLite-shape analog.** The case requires a feature that has no
   reasonable mapping into SQLite-shaped surfaces — e.g. first-class array
   types, `LOCK TABLE` semantics, role-based `GRANT`, `LISTEN/NOTIFY`
   pub/sub, `CREATE SUBSCRIPTION` replication, `CREATE PROCEDURE` with own
   block-level state.
2. **Would require a new top-level surface.** Implementing it would require
   adding a new top-level type, a new privilege/session model, or a new
   system-catalog surface that does not fit a `sqlite_master`-style
   introspection model.
3. **No acceptable lower-fidelity port.** There is no acceptable
   lower-fidelity portable shape that would still pass the case (e.g.
   storing UUIDs as TEXT is fine; storing MONEY as TEXT defeats typed
   arithmetic).

Cases that are stdout-diff-only (e.g. boolean `t/f` vs `1/0`, decimal
precision rendering, JSONB key reordering) are **NOT** skips. Those are
closable via either normalizer extensions on the testing side or rendering
parity in redlinedb.

## Skip-set sizing

Current beyond-SQLite shard size: 265 cases.

Total corpus target at full build-out (sqlite_parity + beyond_sqlite +
generated_matrix): ~2,710 cases.

| Quantity | Count | % of 265 beyond | % of 2,710 corpus |
|---|---:|---:|---:|
| Beyond-SQLite cases | 265 | 100% | 9.8% |
| Self-compare passes (psql) | 253 | 95.5% | 9.3% |
| Self-compare skips | 12 | 4.5% | 0.4% |
| Target-vs-reference passes | 20 | 7.5% | 0.7% |
| Target-vs-reference failures | 233 | 87.9% | 8.6% |
| -- skipped as PG-only (this policy) | **114** | **43.0%** | **4.2%** |
| -- triaged closable in pure Rust | 119 | 44.9% | 4.4% |

**Cap.** A previous draft of this policy proposed a 30-entry cap. After
triaging all 233 beyond-SQLite failures, that cap is too tight: the
fundamental PG-only feature surfaces (matviews, stored procedures, logical
replication, LISTEN/NOTIFY, advisory locks) alone contribute 75 deliberately
deferred cases. Raising the cap to **150 entries** keeps us inside the
1%-5% band against the 2,710 target while leaving ~36 entries of headroom for
future beyond-PG additions (e.g. partitioning, FDWs, row-level security)
that have not yet been added to the corpus.

## Per-category rationale

### BEYOND_LISTEN_NOTIFY -- 14 skipped (of 14 failed)

All 14 LISTEN/NOTIFY cases skip. LISTEN/NOTIFY is a session-bound async pub/sub
IPC primitive. Implementing it would require a connection-scoped event queue, a
multi-session signaling layer, and a NOTIFY catalog surface that does not fit
SQLite-shape introspection. Adjacent cases that combine NOTIFY with PL/pgSQL
functions or triggers are also deferred because they depend on the (also
deferred) stored-procedures surface.

### BEYOND_STORED_PROCEDURES -- 19 skipped (of 19 failed)

All 19 stored-procedure cases skip. `CREATE FUNCTION` / `CREATE PROCEDURE` /
`CALL` / PL/pgSQL require a `pg_proc`-style catalog, a SQL-bodied function
executor, AND a full PL/pgSQL interpreter (DECLARE blocks, control flow,
EXCEPTION blocks, `RETURN NEXT`/`RETURN QUERY`). SQLite has user-defined
functions only via the host API (not via SQL DDL) and has no procedural
language at the SQL level. The compound surface area is too large to absorb
into a SQLite-shaped engine and is therefore deliberately deferred as a single
scope cut.

### BEYOND_REPLICATION_CDC -- 13 skipped (of 13 failed)

All 13 replication/CDC cases skip. This category exercises Postgres
logical-replication infrastructure (`CREATE PUBLICATION`/`SUBSCRIPTION`,
`pg_replication_slots`, `pg_stat_replication`), WAL LSN inspection
(`pg_current_wal_lsn`, `pg_wal_lsn_diff`), and replication GUCs (`wal_level`,
`session_replication_role`). All require a server topology — multi-backend
processes, WAL streaming, replication slot management — that does not fit
RedlineDB's single-process embedded model.

### BEYOND_MATERIALIZED_VIEWS -- 20 skipped (of 20 failed)

All 20 materialized-view cases skip. `CREATE/REFRESH/DROP MATERIALIZED VIEW`
requires a new storage surface that caches query results separately from
regular tables AND from `VIEW` definitions. SQLite has only `VIEW`
(always-recomputed); the materialized variant would require new catalog rows,
new physical storage, a refresh protocol, and snapshot isolation from base
relations. The `REFRESH CONCURRENTLY` variant additionally requires the
unique-index-driven row-diff merge. Deferred as a single integrated surface.

### BEYOND_MVCC_LOCKING -- 9 skipped (of 10 failed)

9 of 10 MVCC/locking cases skip. The row-level lock matrix (FOR KEY SHARE /
SHARE / NO KEY UPDATE / UPDATE), `pg_locks`, advisory locks (`pg_advisory_lock`
/ `pg_try_advisory_lock`), `LOCK TABLE` explicit relation locks, and
backend/xact introspection (`pg_backend_pid`, `txid_current`,
`pg_current_xact_id`) all require Postgres's heavyweight lock manager and
per-backend identity surface, none of which fit SQLite's implicit
RESERVED/PENDING/EXCLUSIVE lock progression. One case in the category
(`SET_TRANSACTION_ISOLATION_LEVELS`) is treated as closable because the SQL
shape parses cleanly and only the executor binding is missing.

### BEYOND_COLLATIONS_ILIKE -- 8 skipped (of 25 failed)

8 of 25 collation cases skip. The deep skips are the `citext` extension (3
cases: first-class case-insensitive text type, requires an extension loader
plus pg_type entry), named ICU collations like `"en-x-icu"` and `"und-x-icu"`
(3 cases: SQLite has only `BINARY`/`NOCASE`/`RTRIM` plus host-registered
collations), and `CREATE COLLATION ... deterministic = false` (2 cases:
nondeterministic equality breaks SQLite's reflexive byte-equality assumption).
The remaining 17 cases (ILIKE rendering, `LOWER`/`UPPER` on multibyte, regex
match operators, `SIMILAR TO`, `POSITION`, `octet_length`) are returned as
closable in pure Rust.

### BEYOND_RICH_TYPES -- 6 skipped (of 28 failed)

6 of 28 rich-type cases skip. The deep skips are ENUM and DOMAIN (require
`pg_type` catalog), range types (`int4range` with `@>` and `&&`), MONEY
(locale-sensitive currency rendering), and the geometric type family (POINT,
LINE, BOX, distance operator `<->`). All require new top-level types that do
not fit the SQLite TEXT/INTEGER/REAL/BLOB/NULL storage classes. The remaining
22 cases (booleans as `t`/`f`, decimal precision via TEXT storage, UUID as
TEXT/BLOB, intervals/timestamptz as text, arrays as JSON-shaped TEXT, JSONB
roundtrip) are returned as closable — most via normalizer extensions or
rendering parity changes in redlinedb.

### BEYOND_MIGRATION_ERGONOMICS -- 6 skipped (of 17 failed)

6 of 17 migration-ergonomics cases skip. The skips target ALTER TABLE knobs
that operate on Postgres-specific physical layout concepts: INHERIT/NO
INHERIT (table inheritance), SET UNLOGGED/LOGGED (per-table WAL
participation), SET STATISTICS (per-column ANALYZE target), SET STORAGE
(per-column TOAST strategy), SET (autovacuum_enabled), OWNER TO + SET
WITHOUT CLUSTER (role/owner model + CLUSTER heap rewrite). The remaining 11
cases (ALTER COLUMN TYPE USING, SET/DROP DEFAULT, DROP NOT NULL, ADD/DROP
CONSTRAINT, ADD/DROP IDENTITY, RENAME CONSTRAINT, RENAME INDEX) are core
schema evolution that is returned as closable.

### BEYOND_SCHEMAS_SEQUENCES -- 4 skipped (of 18 failed)

4 of 18 schema/sequence cases skip. The skips are the per-session search_path
stack (3 cases: SET search_path, current_schema(), SET search_path TO '') and
CREATE SCHEMA AUTHORIZATION (1 case: requires Postgres role/owner model).
SQLite has ATTACH DATABASE for cross-namespace access but no per-session
search_path; implementing would require a new session-settings surface plus a
dynamic resolver. The remaining 14 cases (CREATE/DROP SCHEMA, schema-qualified
table/sequence references, CREATE SEQUENCE, nextval/currval/setval, identity
columns) are returned as closable.

### BEYOND_VECTOR_ADVANCED_INDEXES -- 15 skipped (of 17 failed)

15 of 17 advanced-index cases skip. RedlineDB DOES have its own vector indexes
(HNSW, DiskANN, IVFFlat) — these are tracked under the closable list when
expressed in portable shape. Skipped here are PG-specific access methods
(BRIN — heap-page-shaped, Hash — no SQLite AM analog, GIN/GiST/SP-GiST on
range/point — depend on PG-only types) and the FTS family
(`to_tsvector`/`to_tsquery`/`ts_rank` requires snowball dictionaries and a
ts_config registry), plus the trigram/btree-helper extensions (pg_trgm,
btree_gin, btree_gist). The two cases returned as closable are
`INDEX_UNIQUE_PARTIAL` and `INDEX_EXPR_AND_INCLUDE`, which exercise B-tree
CREATE INDEX features that redlinedb already supports.

### BEYOND_PORTABILITY_SYNTAX -- 0 skipped (of 32 failed)

All 32 portability-syntax cases are returned as closable. This category
covers parser-level work that has a portable SQL standard form: MERGE
(SQLite 3.39+ also has it), LATERAL joins, data-modifying CTEs, DISTINCT ON,
named windows, FETCH FIRST/NEXT ROWS, GROUPING SETS / ROLLUP / CUBE, ARRAY
literals at parser level, ON CONFLICT WHERE, GENERATED AS IDENTITY. None of
these require a new top-level surface; they require parser productions and
plan-node wiring.

### BEYOND_JSONB_INDEXING -- 0 skipped (of 20 failed)

All 20 JSONB cases are returned as closable. JSONB containment (`@>`/`<@`),
key-existence (`?`/`?|`/`?&`), path operators (`->`/`->>`/`#>`/`#>>`),
mutation (`jsonb_set`/`jsonb_insert`/`jsonb_strip_nulls`/`jsonb_pretty`), and
removal (`-`/`#-`) are pure-Rust JSON-traversal work over the existing JSONB
storage. The `@@` jsonpath operator and `jsonb_path_exists`/`jsonb_path_query`
need a jsonpath parser, also pure Rust. Several cases just need parser wiring
for `?` / `?|` / `?&` operators (they currently parse-error).

## Closable work tracks

The 119 closable failures are listed in
`/home/ubuntu/redlineDB/target/redline-testing/closable-beyond-pg.txt` so
future agents can pick up specific tracks. The natural parallel tracks are:

1. **BEYOND_PORTABILITY_SYNTAX (32)** -- parser-level work: MERGE, LATERAL,
   data-modifying CTEs, DISTINCT ON, named windows, FETCH FIRST/NEXT ROWS,
   GROUPING SETS / ROLLUP / CUBE, ARRAY literals, ON CONFLICT WHERE, GENERATED
   AS IDENTITY (sequence-backed).
2. **BEYOND_RICH_TYPES (22)** -- normalizer extensions and rendering parity:
   booleans as `t`/`f`, decimal precision, UUID/interval/timestamptz/jsonb
   roundtrip, array shape.
3. **BEYOND_JSONB_INDEXING (20)** -- JSON traversal operators and functions
   in pure Rust over existing JSONB storage; jsonpath parser for `@@`.
4. **BEYOND_COLLATIONS_ILIKE (17)** -- ILIKE rendering, multibyte
   LOWER/UPPER, regex match operators (`~`/`~*`/`!~`/`!~*`), `SIMILAR TO`,
   `POSITION`, `octet_length`.
5. **BEYOND_SCHEMAS_SEQUENCES (14)** -- CREATE/DROP SCHEMA, schema-qualified
   references, CREATE SEQUENCE, nextval/currval/setval, GENERATED AS IDENTITY
   round-trip.
6. **BEYOND_MIGRATION_ERGONOMICS (11)** -- ALTER COLUMN TYPE USING,
   SET/DROP DEFAULT, DROP NOT NULL, ADD/DROP CONSTRAINT, RENAME CONSTRAINT,
   RENAME INDEX, ADD/DROP IDENTITY.
7. **BEYOND_VECTOR_ADVANCED_INDEXES (2)** -- INDEX_UNIQUE_PARTIAL,
   INDEX_EXPR_AND_INCLUDE.
8. **BEYOND_MVCC_LOCKING (1)** -- SET_TRANSACTION_ISOLATION_LEVELS executor
   binding.
