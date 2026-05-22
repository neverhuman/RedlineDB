# Beyond-SQLite Gap Backlog

This backlog records features that are useful beyond SQLite compatibility but
are not default CI failures until RedlineDB chooses an executable contract for
them. The ranking is seeded from `tips/beyond/*.txt`; each row names the local
owner and proof lane that should receive the first executable test when the gap
moves from backlog to implementation.

| Rank | Gap | Owner | Proof lane | Source tips | Status |
| ---: | --- | --- | --- | --- | --- |
| 1 | Multi-writer / row-locking / queue semantics: `FOR UPDATE`, `SKIP LOCKED`, concurrent row reservations | sql-parser-planner-executor | phase11-sql-contracts | tip1.txt, tip3.txt, tip4.txt, tip5.txt, tip6.txt, tip7.txt, tip8.txt, tip9.txt | Manifest backlog |
| 2 | Migration ergonomics: `ALTER COLUMN`, defaults, constraint add/drop, safer table evolution | sql-parser-planner-executor | sql-test | tip1.txt, tip2.txt, tip3.txt, tip4.txt, tip5.txt, tip6.txt, tip7.txt, tip8.txt, tip9.txt | Passing reference |
| 3 | Stored SQL routines: SQL functions/procedures, variables, reusable DB-side logic | sql-parser-planner-executor | sql-check | tip1.txt, tip2.txt, tip3.txt, tip4.txt | Manifest backlog |
| 4 | Replication / sync / CDC: manifest first; executable tests wait for a RedlineDB API | replication-streams | beyond-sqlite-manifest | tip1.txt, tip2.txt, tip3.txt, tip4.txt | Manifest backlog |
| 5 | `LISTEN` / `NOTIFY`: Postgres reference, RedlineDB event contract later | sql-parser-planner-executor | beyond-postgres-reference | tip2.txt, tip3.txt, tip4.txt | Manifest backlog |
| 6 | Materialized views: create, refresh, indexed refresh targets | sql-parser-planner-executor | sql-check | tip1.txt, tip2.txt, tip3.txt, tip4.txt | Manifest backlog |
| 7 | Richer typing: decimal, UUID, boolean, timestamps, stricter mode | sql-parser-planner-executor | sql-test | tip1.txt, tip2.txt, tip3.txt, tip4.txt | Passing reference |
| 8 | Unicode/collation/`ILIKE`: start with active Postgres-vs-RedlineDB `ILIKE` tests because RedlineDB already has partial support | phase10-collations | beyond-postgres-reference | tip1.txt, tip2.txt, tip3.txt, tip4.txt | Passing reference |
| 9 | JSONB/document indexing: containment, path lookup, indexed generated path cases | phase10-jsonb-binary-format | sql-test | tip1.txt, tip2.txt, tip3.txt | Manifest backlog |
| 10 | Schemas/sequences/identity: namespaces, sequence objects, identity syntax | storage-and-catalog | sql-check | tip1.txt, tip2.txt | Manifest backlog |
| 11 | SQL portability syntax: `MERGE`, `LATERAL`, data-modifying CTEs, `DISTINCT ON`, `DEFAULT` in values | sql-parser-planner-executor | sql-check | tip1.txt, tip2.txt, tip3.txt | Passing reference |
| 12 | Advanced indexes/search/vector: manifest entries first unless existing implementations already pass | phase10-vector-flat-and-simd | sql-test | tip1.txt, tip2.txt, tip3.txt | Manifest backlog |

The Postgres lane is an oracle only. SQLite compatibility remains the default
surface; features here become mandatory only after RedlineDB accepts a
Redline-owned contract and promotes the corresponding proof lane.
