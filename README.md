<p align="center">
  <img src="assets/redlinedb-banner.png" alt="RedlineDB" width="100%">
</p>

<h1 align="center">RedlineDB</h1>

<p align="center">
  <em>Rust-native embedded SQL with SQLite-shaped compatibility, concurrent writes, and deterministic recovery.</em>
</p>

<p align="center">
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-Apache--2.0-blue" alt="license"></a>
  <img src="https://img.shields.io/badge/family-redline--split-e23b3b" alt="family">
  <img src="https://img.shields.io/badge/engine-Rust-orange" alt="rust">
  <a href="https://github.com/neverhuman/redline-core"><img src="https://img.shields.io/badge/source-redline--core-24292f" alt="source"></a>
</p>

---

**This repository is the front door.** It is where you read about the project, get
the binary, and find the rest of the code. The engine, the conformance harness, and
the observability console each live in their own repository — see
[**the redline family**](#the-redline-family) below.

RedlineDB is an embedded SQL engine written in Rust. It keeps the SQLite-facing API
familiar while replacing the storage core with MVCC, a concurrent B-tree, a
group-commit WAL, and crash recovery designed for multi-writer workloads. On the
external [`redline-testing`](https://github.com/neverhuman/redline-testing) harness it
holds **~97% SQLite parity** (2374/2445 cases) and adds **RQL**, the Redline Query
Language, on top.

## Get the binary

```bash
# One line — detects your OS/arch and installs the latest release:
curl -fsSL https://raw.githubusercontent.com/neverhuman/RedlineDB/main/install.sh | bash
```

Or grab a tarball from [**Releases**](https://github.com/neverhuman/RedlineDB/releases/latest)
and put `redline` on your `PATH`. Then:

```bash
redline mydata.db "CREATE TABLE t(id INTEGER PRIMARY KEY, name TEXT);"
redline mydata.db "INSERT INTO t(name) VALUES ('hello'); SELECT * FROM t;"
```

Prefer a UI? Point [**redline-web**](https://github.com/neverhuman/redline-web) at the
same file and browse the schema, run SQL, and watch live metrics in your browser.

## The redline family

| Repository | What it is | Source |
|---|---|---|
| **[redline-core](https://github.com/neverhuman/redline-core)** | The RedlineDB **engine** — MVCC storage core, group-commit WAL, deterministic recovery, SQL + RQL, CLI, server, FFI. | the engine source |
| **[redline-testing](https://github.com/neverhuman/redline-testing)** | The conformance + benchmark **harness** — SQLite-parity, RQL phase-1, and beyond-SQLite suites that point at any SQLite-compatible binary. | proves the engine |
| **[redline-web](https://github.com/neverhuman/redline-web)** | A **SQL console + live observability** dashboard (Rust/Axum + Vite/TS/React) for a running RedlineDB or any SQLite database. | the GUI |

> Working inside the jeryu forge instead of GitHub? The same three repos plus their
> internal clone URLs are listed in [`family.json`](family.json) and
> [`FAMILY.md`](FAMILY.md). GitHub is the public home; jeryu is the internal mirror
> source.

## What's inside the engine

- **MVCC + concurrent B-tree** — multiple writers without a global lock.
- **Group-commit WAL + crash recovery** — deterministic, durable, fsync-bounded.
- **SQLite-shaped surface** — familiar SQL, CLI, and a C ABI (`sqlite3_*`) so existing
  drivers link without code changes.
- **RQL** — the Redline Query Language, measured at parity-or-better latency vs the
  SQL frontend on the shared corpus.

Full architecture, design notes, and the parity methodology live with the engine in
[**redline-core**](https://github.com/neverhuman/redline-core).

## Build from source

```bash
git clone https://github.com/neverhuman/redline-core
cd redline-core
cargo build --release          # binary at target/release/redline
```

By project policy you compile from source for your CPU; release artifacts published
here are per-CPU-class and built by the [release workflow](.github/workflows/release.yml)
from a pinned `redline-core` tag.

## Project

- **License** — [Apache-2.0](LICENSE).
- **Security** — see [SECURITY.md](SECURITY.md).
- **Issues / contributions** — open them on the repository that owns the code (engine
  issues → `redline-core`, harness → `redline-testing`, console → `redline-web`).
