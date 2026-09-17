# Architecture

`redline-testing` is a single-crate Rust CLI plus an `xtask` dev tool. It
compares a target database CLI against a reference CLI by running shared SQL /
CLI cases through both and diffing normalized output.

## Layers

```
src/cli.rs            clap entry layer — parses args, routes to suites, renders
src/report.rs         report rendering + official-evidence bundle (over JSONL)
src/evidence.rs       evidence bundle hashing + serialization
src/sqlite_parity/    SQLite-parity suite
  ├── engine.rs       subprocess SQL driver (the data-access / adapter seam)
  ├── runner.rs       case scheduling + compare loop
  ├── rql_phase1.rs   Redline Query Language phase-1 SQL→JSON lowering
  ├── catalog.rs      case catalog + capability gating
  ├── case.rs         case model
  ├── memory.rs       /proc RSS sampling
  └── normalize.rs    output normalization
src/beyond_sqlite/    PostgreSQL-class oracle suite
  ├── engine.rs       subprocess psql driver (adapter seam)
  ├── oracle.rs       psql ↔ psql self-compare oracle
  └── taxonomy.rs     feature rank/owner taxonomy
xtask/                dev-only corpus, badge, and receipt tooling (not shipped)
```

## Data-access boundary

There is **no in-process database driver**. Every "DB access" is a subprocess
invocation of an external `sqlite3` / `psql` shell with a bounded timeout. The
`*/engine.rs` + `runner.rs` + `oracle.rs` modules are the data-access / adapter
seam; the reference CLI name is centralized as
`sqlite_parity::REFERENCE_CLI_BIN`. See [`docs/boundaries.md`](boundaries.md).

## Output contract

The runner emits JSONL raw records (`schemas/raw-record.schema.json`) and a
release manifest (`schemas/release-manifest.schema.json`). RedlineDB CI consumes
the pinned tarball; JSONL field compatibility is a hard contract. Generated
zones are declared in [`.jankurai/generated-zones.toml`](../.jankurai/generated-zones.toml)
and must not be hand-edited.

## Error surface

`src/exceptions.rs` defines the typed `HarnessError` surface. Each variant
exposes `purpose`, `reason`, `common_fixes`, `docs_url`, and `repair_hint` so a
failure routes the next agent straight to a local rerun.
