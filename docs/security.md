# Security

## Authorization & data isolation (read-only)

`--read-only` is the data-isolation boundary. It is enforced two ways:

1. SQLite is opened with `OpenFlags::SQLITE_OPEN_READ_ONLY`.
2. `POST /api/query` (and table paging) routes through `is_read_only_sql`, which
   rejects any statement whose first significant keyword is not one of
   `SELECT`, `EXPLAIN`, `PRAGMA`, `WITH`, `VALUES`.

### Negative proof (authz matrix)

`apps/api/tests/property.rs` is the negative-proof matrix:

| Caller intent | Statement class | Expected |
|---|---|---|
| reader (allowed) | `SELECT`/`WITH`/`EXPLAIN`/`PRAGMA`/`VALUES` | accepted |
| writer (denied) | `INSERT`/`UPDATE`/`DELETE`/`DROP`/`CREATE`/… | rejected (`ReadOnly`) |
| writer hiding behind a comment | `/* */ delete …`, `-- \n update …` | rejected |

The property test asserts these for randomized whitespace/casing, so a
`--read-only` connection can never be tricked into a write.

## Input boundary (SQL identifiers)

Identifiers interpolated into SQL are escaped by `quote_ident` (every `"` is
doubled, wrapped in `"`). `property.rs` proves, for **any** input, that the
result is a single well-formed quoted token and that a classic breakout
(`users"; DROP TABLE users;--`) is neutralised. Values are always passed as
bound parameters, never interpolated.

## Supply chain

`ops/ci/security.sh` is blocking and network-free. It verifies exact local tool
bytes, archives the exact pinned RustSec Git commit into an isolated local
snapshot and audits it without fetching, runs
`cargo-deny` with fetching disabled and warnings denied, uses the offline npm
advisory cache, requires zero offline Zizmor findings, and runs pinned Syft with
its update check disabled. Source evidence stays at
`target/jankurai/security/source-evidence.json`; the governed Jankurai release
wrapper writes its separate strict receipt beside it. Both are bound to the
exact commit and tree. Every GitHub Action is pinned to a
40-hex commit SHA; workflows never install release security tools from the
public network.
