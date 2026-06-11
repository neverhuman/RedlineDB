<p align="center">
  <img src="assets/redlinedb-banner.png" alt="RedlineDB" width="100%">
</p>

<h1 align="center">RedlineDB</h1>

<p align="center">
  <em>Rust-native embedded SQL with SQLite-shaped compatibility, concurrent writes, and deterministic recovery.</em>
</p>

<p align="center">
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-Apache--2.0-blue" alt="license"></a>
  <img src="https://img.shields.io/badge/SQLite_parity-~97%25-2ea44f" alt="parity">
  <img src="https://img.shields.io/badge/family-redline--split-e23b3b" alt="family">
  <img src="https://img.shields.io/badge/engine-Rust-orange" alt="rust">
  <a href="https://github.com/neverhuman/redline-core"><img src="https://img.shields.io/badge/source-redline--core-24292f" alt="source"></a>
</p>

---

**This repository is the front door.** It's where you read about the project, get the
binary, and find the rest of the code. The engine, the conformance harness, and the
observability console each live in their own repository — see
[**the redline family**](#the-redline-family) below.

RedlineDB is an embedded SQL engine written in Rust. It keeps the SQLite-facing API
familiar — same SQL, same CLI shape, same `sqlite3_*` C ABI so existing drivers link
without code changes — while replacing the storage core with MVCC, a concurrent B-tree,
a group-commit WAL, and crash recovery built for **multi-writer** workloads. On the
external [`redline-testing`](https://github.com/neverhuman/redline-testing) harness it
holds **~97% SQLite parity** (2374/2445 cases) and adds **RQL**, the Redline Query
Language, on top.

## Why RedlineDB

| | |
|---|---|
| **Concurrent writers** | MVCC + a concurrent B-tree — multiple writers without a global database lock. |
| **Durable & deterministic** | Group-commit WAL with fsync-bounded, deterministic crash recovery. |
| **Drop-in surface** | SQLite-shaped SQL, CLI, and a `sqlite3_*` C ABI — existing drivers link unchanged. |
| **RQL** | The Redline Query Language, measured at parity-or-better latency vs the SQL frontend on the shared corpus. |
| **Proven** | Every release is measured by an independent, signed conformance harness — not self-reported. |

## Get the binary

```bash
# One line — detects your OS/arch and installs the latest release:
curl -fsSL https://raw.githubusercontent.com/neverhuman/RedlineDB/main/install.sh | bash
```

This installs the `redline` command to `~/.local/bin` (override with `REDLINE_PREFIX`).
Pin a specific release with `REDLINE_VERSION=vX.Y.Z`. Prebuilt tarballs are published
per CPU class:

| OS | x86_64 | aarch64 |
|---|:---:|:---:|
| **Linux** | ✅ `linux-x86_64` | ✅ `linux-aarch64` |
| **macOS** | ✅ `macos-x86_64` | ✅ `macos-aarch64` |

Prefer to verify by hand? Grab the tarball + its `.sha256` from
[**Releases**](https://github.com/neverhuman/RedlineDB/releases/latest), check the
digest, and put `redline` on your `PATH`:

```bash
sha256sum -c redline-<tag>-<target>.tar.gz.sha256   # verify
redline mydata.db "CREATE TABLE t(id INTEGER PRIMARY KEY, name TEXT);"
redline mydata.db "INSERT INTO t(name) VALUES ('hello'); SELECT * FROM t;"
```

Prefer a UI? Point [**redline-web**](https://github.com/neverhuman/redline-web) at the
same file and browse the schema, run SQL, and watch live metrics in your browser.

## The redline family

| Repository | What it is | Role |
|---|---|---|
| **[redline-core](https://github.com/neverhuman/redline-core)** | The RedlineDB **engine** — MVCC storage core, group-commit WAL, deterministic recovery, SQL + RQL, CLI, server, FFI. | the engine source |
| **[redline-testing](https://github.com/neverhuman/redline-testing)** | The conformance + benchmark **harness** — SQLite-parity, RQL phase-1, and beyond-SQLite suites that point at any SQLite-compatible binary. | proves the engine |
| **[redline-web](https://github.com/neverhuman/redline-web)** | A **SQL console + live observability** dashboard (Rust/Axum + Vite/TS/React) for a running RedlineDB or any SQLite database. | the GUI |

> Working inside the jeryu forge instead of GitHub? The same three repos plus their
> internal clone URLs are listed in [`family.json`](family.json) and
> [`FAMILY.md`](FAMILY.md). GitHub is the public home; jeryu is the internal mirror
> source.

## Build from source

Project policy is to compile from source for your own CPU; the published artifacts are
per-CPU-class and built by the [release workflow](.github/workflows/release.yml) from a
pinned `redline-core` tag.

```bash
git clone https://github.com/neverhuman/redline-core
cd redline-core
cargo build --release -p redlinedb-cli     # binary at target/release/redlinedb
```

> **Binary name:** the published, end-user command is `redline` (what `install.sh`
> drops on your `PATH`). The raw cargo artifact in `redline-core` is `redlinedb` — the
> same engine, the dev-time name. The dev-fusion below uses `redlinedb`.

## Develop the whole stack

The front door stays thin, so the engine, harness, and console each live in their own
repo and build independently. To iterate across all three at once, **fuse** them into a
gitignored `.fusion/` working tree (a normal user never needs this):

```bash
just fuse                       # clone/update redline-core, redline-web, redline-testing
                                # into .fusion/  (use `just fuse-jeryu` for the internal mirror)
cat .fusion/README.dev.md       # the generated dev loop + the exact revisions you pulled
./.fusion/dev.sh build-all      # build all three
./.fusion/dev.sh run-stack      # engine-backed web console -> http://127.0.0.1:7788
./.fusion/dev.sh test-all       # conformance harness against the freshly built engine
```

`.fusion/` is never committed — this repo stays a thin front door with no unified Cargo
workspace, so a red build in one sibling never reddens another. To pin reproducible
revisions, list `<name> <tag-or-sha>` lines in `.fusion/fusion.lock` and re-run
`just fuse`. Full agent/contributor notes are in [AGENTS.md](AGENTS.md).

## Project

- **License** — [Apache-2.0](LICENSE).
- **Security** — see [SECURITY.md](SECURITY.md).
- **Roadmap / lifecycle** — see [docs/deprecation.md](docs/deprecation.md) for how the
  family evolves (what's stable, what's being consolidated).
- **Issues / contributions** — open them on the repository that owns the code (engine
  issues → `redline-core`, harness → `redline-testing`, console → `redline-web`).
