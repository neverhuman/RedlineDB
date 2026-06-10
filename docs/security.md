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

`ops/ci/security.sh` (blocking in CI) runs gitleaks, cargo-audit, cargo-deny
(`deny.toml`), `npm audit --audit-level=high`, zizmor, and emits an SPDX SBOM.
Every GitHub Action is pinned to a 40-hex commit SHA.
