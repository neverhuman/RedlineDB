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

`ops/ci/security.sh` is blocking and network-free. It verifies exact tool
bytes and audits the exact pinned RustSec Git commit without fetching. Local
runs archive it into an isolated snapshot; release runs invoke the verified
root-installed wrappers over the root-staged physical databases. The lane runs
`cargo-deny` with fetching disabled and warnings denied, requires zero offline
Zizmor findings, and runs pinned Syft with updates disabled. In release mode,
Cargo evidence uses only the fresh root-staged `$CARGO_HOME/registry`; its exact
receipt, archive/index set, selected-content manifest, and full inventory are
verified before and after `cargo-deny`. Local governed and developer-cache runs
use separate reviewed index manifests and are identified in the evidence.

JavaScript advisory coverage scans only `apps/web/package-lock.json` with dev
dependencies enabled. The sorted multiset of all 376 non-root lock entries must
equal the 376 non-root npm PURLs in that SBOM before update-disabled Grype scans
it against the closed, root-owned v6 database. The database inventory, status,
schema, path, and result are retained and zero High/Critical findings are
required. The whole-repository SBOM is separate and never feeds this npm gate.
`npm ci --no-audit` remains lock/install integrity; `npm audit --offline` is not
advisory evidence because an empty cache can report a false clean result.

Source evidence stays at
`target/jankurai/security/source-evidence.json`; the governed Jankurai release
wrapper writes its separate strict receipt beside it. Both are bound to the
exact commit and tree. Every GitHub Action is pinned to a
40-hex commit SHA; workflows never install release security tools from the
public network.
