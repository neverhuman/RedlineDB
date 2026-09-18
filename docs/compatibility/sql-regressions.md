# SQL-02-R: independent ALTER regressions

Date: 2026-09-18. Base: af20826311a067a51383c382013612545156df61 plus preserved dirty changes.
Status: **partial; one real failure remains**. Implementer: SQL agent; reviewer: root.
No integrated commit yet. These eight cases are not a compatibility denominator.

The executable inputs and expected typed values are in
`crates/sql/tests/compatibility_roadmap.rs`. Every successful case also closes and
reopens the database, then checks the result again.

| Case | Qualified CLI output (quote mode) | Redline after patch |
|---|---|---|
| Rename a while trigger belongs to b | `7` | pass, including reopen |
| Rename a.x while view selects b.x | `11` | **FAIL: UnknownColumn("y")** |
| Rename a while view contains literal 'a' | `'a'` | pass, including reopen |
| Rename a with trigger belonging to a | `7` | pass, including reopen |
| Roll back rename, then fire original trigger | `'a',7` | pass, including reopen |
| Quoted identifiers containing apostrophe/comment markers | `1,2,3` | pass, including reopen |
| Quoted table reference | `1` | pass, including reopen |
| Rename bracket/double-quoted references to name containing quote and ] | `1`, `1` | pass, including reopen |

Before the patch, the first three tests all failed: missing audit row, unknown y,
and changed string value `"renamed_a"`, respectively. The narrow patch restricts
trigger-owner changes to the renamed table and protects string literals, escaped
apostrophes, and SQL comments from identifier substitution. Quoted identifiers
are scanned as whole tokens before literals/comments; unrelated tokens retain
their delimiters. Matched tokens receive the replacement verbatim, avoiding double
escaping or invalid bracket escapes. Table replacement escapes embedded quotes. Kernel unit tests cover all three identifier quoting
styles, punctuation, exact quoted targets and comment/literal-only SQL. It does not solve
resolved dependencies, aliases, column binding,
prospective-schema validation, or atomic schema validation. Those remain SQL-02.
In particular the unrelated-view failure is an ordinary failing test, with no
ignore annotation and no inverted assertion. The five-case command must exit 101.

Commands (from the canonical checkout):

```sh
rtk cargo test -p redlinedb-sql --test compatibility_roadmap -- --nocapture
rtk cargo test -p redlinedb-kernel --lib alter_rewrite_regressions -- --nocapture
rtk proxy bash -c 'for fixture in target/compatibility-sql/rename_*.sql; do target/sqlite-reference/3.53.1/bin/sqlite3 :memory: < "$fixture" || exit; done'
```

Results: SQL command exit 101, seven passed/one failed/no ignores; kernel command
exit 0, three passed. Oracle fixtures each exit 0, no stderr. Exact SQL inputs,
outputs, CLI hash and full oracle identity are retained in `oracle.json` below;
the individual `.sql` files are generated verbatim from the Rust fixture inputs.
The oracle must be rebuilt with `scripts/sqlite/build-reference.sh` if missing;
never substitute system SQLite as release evidence.

Artifact and changed-source SHA-256 identities:

- `target/compatibility-sql/regressions.log`: `dabb4e53a7a251f5588f5dfddf1783522ee6571f4ed63664c3df49900a9a1bd3`
- `target/compatibility-sql/unit.log`: `60857ee7b161410e6f36e38bc0d21961838e0ceff34489e54e09b717ede77101`
- `target/compatibility-sql/oracle.json`: `239342b1bcdfd14b9760e7e420bb2a3880ef53f1129fcdbbc30d8d5f8bc05266`
- `crates/kernel/src/catalog/ops.rs`: `582dd6b7765ea41e24f05bd1411e5b59bb435fedd91cfaf34311a6a963a54fdc`
- `crates/sql/tests/compatibility_roadmap.rs`: `92ac45c38f9f9471413c36508be50a4c4326361d38e5e7402cf68decc580adca`
