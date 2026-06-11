# db — Schema Reference (No Production Database)

The `redlineDB` hub is a thin front-door: it ships an installer, tracks family
pointers, and re-exports release artifacts from `redline-core`. **It does not own
or connect to any production database.**

## What lives here

| Path | Purpose |
|------|---------|
| `db/migrations/001_hub_exceptions.sql` | SQLite-compatible DDL for the hub exception catalog (documentation and tooling only) |

## How the schema is used

The migration file exists so that:

1. **Agent tooling** (jankurai, etc.) can query the exception catalog via `sqlite3` without parsing Rust source.
2. **Integration tests** (`crates/hub/tests/exceptions_integration.rs`) can spin up an in-memory SQLite database, populate it from this migration, and verify that the Rust `CATALOG` constant stays in sync with the SQL seed rows.
3. **Schema drift detection** in CI (`ops/ci/contract-drift.sh`) verifies that the column set matches the `HubException` struct in `crates/hub/src/exceptions.rs`.

## Running locally

```sh
# Bootstrap the schema into an in-memory DB and dump the catalog:
sqlite3 :memory: < db/migrations/001_hub_exceptions.sql
```

## What is NOT here

- No persistent data files (`.db`, `.sqlite3`, `.journal`)
- No connection pools or ORM configuration
- No production write path

The production (release-serving) surface of this hub is the `install.sh` script
and the GitHub Releases page. See [docs/architecture.md](../docs/architecture.md).
