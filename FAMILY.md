# The redline family

RedlineDB is shipped as a small family of independently-built, independently-CI'd
repositories. This repo (`RedlineDB`) is the **fusion hub**: the public front door
that ties them together. The machine-readable version of this map is
[`family.json`](family.json).

> **Pointers vary by context.** GitHub (`neverhuman`) is the **public** home and the
> primary audience. The jeryu forge is the **internal** mirror source; its URLs are
> reachable only inside the jeryu network.

| Repo | Role | Public (GitHub) | Internal (jeryu) |
|---|---|---|---|
| **redline-core** | engine | https://github.com/neverhuman/redline-core | http://127.0.0.1:8787/jeryu/redline-core |
| **redline-testing** | conformance + bench harness | https://github.com/neverhuman/redline-testing | http://127.0.0.1:8787/jeryu/redline-testing |
| **redline-web** | SQL console + observability | https://github.com/neverhuman/redline-web | http://127.0.0.1:8787/jeryu/redline-web |
| **RedlineDB** (this) | front-door / distribution | https://github.com/neverhuman/RedlineDB | http://127.0.0.1:8787/jeryu/redlineDB |

## How it flows

- The **engine** is developed in `redline-core`. It is the single source of truth for
  the storage core, SQL/RQL, CLI, server, and FFI.
- `redline-testing` proves the engine (SQLite parity, RQL, beyond-SQLite) and is
  pointed at any SQLite-compatible binary via `--target-bin`.
- `redline-web` is a standalone GUI that talks to a running RedlineDB **or any SQLite
  database** over HTTP — it never links the engine crates.
- This **hub** publishes the binary (built from a pinned `redline-core` tag) and is
  where end-users land first.

Each repo advances `main` only through its own jeryu PR-CI; on a green merge the forge
mirrors `main` to its public GitHub repo under `neverhuman`.
