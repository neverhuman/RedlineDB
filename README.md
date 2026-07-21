# redline-testing

Authoritative CI and releases run locally through the Jeryu forge. The release
path requires no GitHub service, download, external checkout, or network access.

Full container parity is local-only. `just docker-parity
--source-cargo-home <physical-in-tree-custody>` delegates container lifecycle to
the Redline control plane. It uses only digest-pinned images already present in
local custody, an internal Compose network, and an explicitly supplied evidence
directory. Until PostgreSQL compatibility work closes the current target gaps,
the infrastructure reports those target failures and emits no passing release
receipt.
<!-- jankurai-score-badge:begin -->
[![Jankurai score: 38/100 advisory](https://img.shields.io/badge/jankurai-38%2F100%20advisory-red)](agent/repo-score.json)
<!-- jankurai-score-badge:end -->

Conformance, benchmark, and SQLite-parity test harness for **any SQLite-compatible database**.
Point `--target-bin` at your binary; the runner does the rest.

Suites:

| Suite | What it tests | Cases | Postgres? |
|---|---|---|---|
| `sqlite_parity` | SQL + CLI conformance vs SQLite reference | 2,445 | No |
| `memory` | Same corpus with Linux `/proc` RSS sampling | 2,445 | No |
| `rql_phase1` | Redline Query Language phase 1 conformance | 1,385 | No |
| `beyond_sqlite` | PostgreSQL-class features, oracle-validated | 265 | Optional |
| `all` | All suites + `official-evidence.json` hash bundle | 4,095+ | Optional |

---

## Build from local custody

The canonical checkout builds only from the in-tree Cargo custody:

```bash
REDLINE_CARGO_HOME=/home/ubuntu/jain-split/target/redline-testing-cargo-home \
  bash scripts/setup.sh
target/debug/redline-testing --version
```

Each local release ships a `.sha256` sidecar and content manifest. Verify both
before use:

```bash
sha256sum -c redline-testing-<version>-linux-x86_64.tar.gz.sha256
jq -r '.artifact_hashes | to_entries[] | "\(.value)  \(.key)"' \
  release-manifest.json | sha256sum -c
```

---

## Quick start

Replace `your-db` with any SQLite-compatible binary — not just RedlineDB:

```bash
redline-testing run \
  --suite sqlite_parity \
  --target-bin /path/to/your-db \
  --sqlite-bin /usr/bin/sqlite3 \
  --output results.jsonl
```

The runner compares `your-db` against the SQLite CLI reference using
`corpus/sqlite_parity/` (1,127 pinned upstream cases + 1,318 extended cases).
It writes JSONL records on stdout/file and exits non-zero if any case fails.

---

## Develop

Setup and validate are one command each:

```bash
bash scripts/setup.sh    # install the pinned toolchain, fetch deps, build
bash ops/ci/pr-ci.sh     # the single validate command (fmt, check, test, package, security)
```

Other lanes (also runnable via `just`):

```bash
bash ops/ci/security.sh  # gitleaks + cargo-audit + cargo-deny + zizmor + SBOM
bash ops/ci/jankurai.sh  # jankurai tool-suite evidence -> target/jankurai/**
```

Agent-readable docs: [docs/architecture.md](docs/architecture.md),
[docs/boundaries.md](docs/boundaries.md), [docs/testing.md](docs/testing.md),
[docs/operations.md](docs/operations.md), and per-cell `ops/AGENTS.md`.
The release-control surface is [docs/release.md](docs/release.md). It names the
canonical version source, immutable-tag process, package-integrity evidence,
provenance, rollback, and the separate Jain local-forge corrective lifecycle.
For exact PR parity, run the governed Jankurai changed-fast command documented
in [docs/testing.md](docs/testing.md#forge-proof-parity); a full-tree ratchet
score never substitutes for that changed-head proof.

---

## Run

```bash
redline-testing run \
  --suite sqlite_parity \     # sqlite_parity | memory | rql_phase1 | beyond_sqlite | all
  --target-bin /path/to/db \
  --sqlite-bin /path/to/sqlite3 \
  --workers auto \            # accepted; execution is serial in this release
  --tmp-root auto \
  --output raw.jsonl \
  --repetitions 1 \
  --warmup 0 \
  --progress auto \
  --memory-samples            # Linux /proc RSS sampling (memory suite)
```

### Environment variables

| Variable | Default | Effect |
|---|---|---|
| `REDLINE_TESTING_PINNED_ONLY` | unset | Set to `1` to run only the 1,127 pinned upstream cases (skip extended shards) |
| `REDLINE_TESTING_POSTGRES_URL` | unset | PostgreSQL DSN for the `beyond_sqlite` oracle (e.g. `postgresql://localhost/postgres`) |

### Output fields

`raw.jsonl` follows the report-parser contract:

```
case_id  name  case_file  priority  profile  category  sample_role  repetition_index
sqlite_version  reference_engine  target_engine
reference_executable_path  target_executable_path
reference_executable_sha256  target_executable_sha256
reference_version  target_version
status  reference_elapsed_ns  target_elapsed_ns
```

When `--memory-samples` is set on Linux, records also include:
`memory_status  reference_peak_rss_kb  target_peak_rss_kb`

---

## Suites

### `sqlite_parity`

Compares your target binary against the SQLite CLI reference for 2,445 cases:

- **1,127 pinned upstream cases** (`corpus/sqlite_parity/generated_manifest.json`, IDs 1–1127, read-only)
- **630 hand-authored cases** (`corpus/sqlite_parity/cases/`, IDs 10001–10630, P0 priority) —
  NULL semantics, ordering, aggregates, autoincrement, strict tables, CLI output modes,
  dot-commands, options, PRAGMA P0, foreign keys, transactions, UPSERT, RETURNING, ATTACH,
  schema introspection, ALTER, recursive CTEs, JOINs, compound queries, subqueries,
  aggregate-advanced, error messages, pattern matching, BLOB
- **688 matrix-generated cases** (`gen_*` shards, IDs 11000–12037) — math, cast, affinity,
  string, datetime, JSON path, window functions, pragma sweep

### `memory`

Same parity corpus with Linux `/proc` RSS sampling enabled. Writes:
`memory.raw.jsonl`, `memory-summary.json`, `memory-ranked.csv`,
`memory-manifest.json`, `memory-provenance.json`.
Falls back gracefully if `/proc` is unavailable (`memory_status: unavailable`).

### `rql_phase1`

Redline Query Language phase 1 corpus, exercised against the SQLite reference
CLI and the target binary. Writes:
`rql_phase1.raw.jsonl`, `rql-phase1-summary.json`, `rql-phase1-ranked.csv`,
`rql-phase1-manifest.json`, `rql-phase1-provenance.json`.

### `beyond_sqlite`

265 oracle-validated cases covering 12 PostgreSQL feature areas
(`metadata/beyond_sqlite/features.json`): advanced types, window functions,
CTEs, stored procedures, full-text search, JSON operators, geospatial,
materialized views, advisory locks, logical replication, LISTEN/NOTIFY, and MONEY arithmetic.

When `REDLINE_TESTING_POSTGRES_URL` is set (or the conventional fallback
`/dev/shm/redline-pg-sock:5433` is reachable), the runner executes a
`psql ↔ psql` oracle for each case. Without Postgres every oracle case emits
`status: skipped` with a diagnostic — the SQLite-parity suite is unaffected.

### `all`

Runs every suite and writes `all.jsonl`, `all-manifest.json`, all per-suite
artifacts, and `official-evidence.json` (schema `redline-testing-official-evidence-v1`).
The evidence bundle records the runner, target, SQLite reference, per-suite
totals, and SHA-256 hashes of all declared output files.

---

## Report generation

```bash
redline-testing report \
  --suite sqlite_parity \
  --input raw.jsonl \
  --official-evidence official-evidence.processed.json \
  --out-dir benchmark-results/sqlite-parity/latest \
  --readme README.md \
  --updated-date 2026-05-25
```

`--official-evidence` verifies that the input hash matches the recorded
suite hash before rendering. Omit it only with `--local-diagnostics` for
uncommitted local diagnostics.

---

## Release tarball contents

```
bin/redline-testing                               runner binary
release-manifest.json                             metadata + artifact_hashes (SHA-256 map)
corpus/sqlite_parity/generated_manifest.json      1,127 pinned cases
corpus/sqlite_parity/cases/*.json                 extended hand-authored + generated shards
corpus/beyond_sqlite/generated_manifest.json      265 oracle cases
metadata/beyond_sqlite/features.json              12-entry feature taxonomy
schemas/raw-record.schema.json
schemas/release-manifest.schema.json
templates/README.sqlite-parity.md
```

Protected local Jeryu merges are packaged with `just release-local`. The family
controller binds the content manifest and creates the next unused immutable
`redline-testing-v<product-version>-jain.<revision>` tag. Existing tags never
move.

---

## Development

```bash
# Run all 31 integration tests
cargo test --locked

# Full CI mirror (fmt + check + test + package)
just verify

# Build + package release tarball locally
just release-local

# Validate every corpus case against sqlite3 (ship-gate)
cargo run -p xtask --release -- ship-gate

# Stage/hash oracle artifacts and prove the locked dependency closure offline
cargo run --locked -p xtask -- custody-stage \
  --source-cargo-home /home/ubuntu/.cargo \
  --cargo-home /home/ubuntu/jain-split/target/redline-testing-cargo-home \
  --sqlite-bin /usr/bin/sqlite3 \
  --postgres-client-bin /usr/lib/postgresql/16/bin/psql \
  --postgres-server-bin /usr/lib/postgresql/16/bin/postgres \
  --out-dir /home/ubuntu/jain-split/target/redline-custody

# Detect drift in matrix-generated shards
cargo run -p xtask --release -- generate --check

# Sync the README score badge from agent/jankurai-badge.json
cargo run --locked --quiet -p xtask -- update-badge
```

### Adding test cases

**Hand-authored SQLite-parity shard:** create `corpus/sqlite_parity/cases/<N>_<name>.json`
with IDs starting at 10001+. Each case must pass `sqlite3 ↔ sqlite3` self-compare.
Run `cargo run -p xtask --release -- ship-gate` to validate before committing.

**Matrix-generated shard:** add a generator in [`xtask/src/generators.rs`](xtask/src/generators.rs)
and run `cargo run -p xtask -- generate` to emit `gen_*.json` shards.

**Beyond-SQLite case:** add to `corpus/beyond_sqlite/generated_manifest.json`
(IDs 20001+). Must pass `psql ↔ psql` self-compare with a local Postgres instance:

```bash
# Bring up a dev-time Postgres on /dev/shm
/usr/lib/postgresql/16/bin/initdb -D /dev/shm/redline-pg-data \
  --auth-local=trust --auth-host=trust -U ubuntu
/usr/lib/postgresql/16/bin/pg_ctl -D /dev/shm/redline-pg-data \
  -l /dev/shm/redline-pg.log \
  -o "-p 5433 -k /dev/shm/redline-pg-sock -h ''" start
cargo run --release -- run --suite beyond_sqlite \
  --target-bin sqlite3 --output /tmp/beyond.jsonl
```

### Corpus structure

```
corpus/
  sqlite_parity/
    generated_manifest.json   pinned upstream (IDs 1–1127, read-only)
    cases/                    extended shards (IDs 10001+)
      NN_name.json            hand-authored
      gen_NN_name.json        matrix-generated
  beyond_sqlite/
    generated_manifest.json   oracle cases (IDs 20001+)
metadata/
  beyond_sqlite/
    features.json             12-entry rank/owner feature taxonomy
    skip-list.toml            cases skipped pending engine implementation
```

**Ship contract:** a case ships iff its reference self-compare passes
(`sqlite3 ↔ sqlite3` for parity, `psql ↔ psql` for beyond). Failing cases
must be fixed or deleted — there is no quarantine path.

---

## License

Apache-2.0
