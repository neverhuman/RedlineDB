<h1 align="center">redline-web</h1>

<p align="center">
  <em>A SQL console &amp; live observability dashboard for a running redline-core — or any SQLite database.</em>
</p>

<p align="center">
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-Apache--2.0-blue" alt="license"></a>
  <img src="https://img.shields.io/badge/backend-Rust%20%2F%20Axum-orange" alt="backend">
  <img src="https://img.shields.io/badge/frontend-Vite%20%2F%20TS%20%2F%20React-3178c6" alt="frontend">
  <!-- jankurai-score-badge:begin -->
  <a href="agent/repo-score.md"><img src="https://img.shields.io/badge/jankurai-pending-lightgrey" alt="jankurai score"></a>
  <!-- jankurai-score-badge:end -->
</p>

`redline-web` is the observability surface of the **redline** family. It is 100%
Rust (Axum backend) + Vite/TypeScript/React (frontend), with **no** dependency on
the engine's internal crates: it speaks to any SQLite-compatible database.
Agents and maintainers must read the canonical [AGENTS.md](AGENTS.md) ownership
and proof guide before changing this repository.

## What it does

- **Connect** to a SQLite database file, or drive an external SQLite-compatible
  CLI (`--target-bin`, e.g. a running `redline-core`) — the same pattern
  `redline-testing` uses.
- **Browse schema** — tables, views, indexes, triggers, columns, row counts.
- **Run SQL** — a query console with timing, row counts, truncation guard, and a
  read-only mode that rejects writes.
- **Page tables** — order-by + limit/offset over any table.
- **Observe, live** — queries/sec, latency p50/p95/p99, database size, WAL size,
  page/freelist counts, per-table sizes, and a slow-query log, streamed over SSE.
  A Prometheus exposition is served at `/metrics`.

## Layout

```
apps/api/   Rust + Axum backend (embeds apps/web/dist)
apps/web/   Vite + TS + React frontend
ops/ci/     CI lanes (one per script); ops/ci/pr-ci.sh is the single gate
docs/       architecture, boundaries, testing, security, operations, release
CONTRACT.md single source of truth for every endpoint and DTO
```

## Setup & validate (one command each)

```bash
bash scripts/setup.sh     # install deps, build the frontend, build the binary
bash ops/ci/pr-ci.sh      # the single validate command (local == CI)
```

`bash scripts/ci-doctor.sh` confirms your toolchain matches CI. See
[`docs/testing.md`](docs/testing.md) for the proof lanes.

## Run

```bash
# Build the frontend, then the embedded release binary:
just build
# Serve over a SQLite file (seeded with a demo schema if empty):
./target/release/redline-web --db ./demo.sqlite --bind 127.0.0.1:7788
# open http://127.0.0.1:7788
```

Point it at a running redline-core build instead:

```bash
./target/release/redline-web --target-bin /path/to/redline-core --db ./engine.db
```

See [`CONTRACT.md`](CONTRACT.md) for the full HTTP API and DTOs, and `--help` for
all flags (`--read-only`, `--max-rows`, `--query-timeout-ms`, `--slow-ms`).

## Develop

```bash
just dev-server   # backend on :7788 (terminal A)
just dev-web      # Vite dev server on :5173, proxies /api -> :7788 (terminal B)
```

## CI

`redline-web` is **independently** CI'd: `bash ops/ci/pr-ci.sh` is the
authoritative jeryu gate (fast + frontend + backend + security + web e2e +
cost-budget + release-readiness + jankurai evidence). GitHub Actions
([`.github/workflows/ci.yml`](.github/workflows/ci.yml)) mirrors it lane-for-lane
(ci-local parity), with every action pinned to a 40-hex commit SHA. A red build
in `redline-core` or `redline-testing` never turns this repo red.
