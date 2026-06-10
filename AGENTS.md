# RedlineDB hub — agent guide

This repository is the **fusion hub** / front-door, **not** the engine. The engine
lives in [redline-core](https://github.com/neverhuman/redline-core); see
[FAMILY.md](FAMILY.md) / [family.json](family.json) for the full map.

## What lives here

- `README.md` — the public landing page (what RedlineDB is, how to get the binary).
- `install.sh` — one-line installer that fetches the latest release binary.
- `family.json` / `FAMILY.md` — pointers to the family (public GitHub + internal jeryu).
- `.github/workflows/` — hub CI + the release pipeline that builds the binary from a
  pinned `redline-core` tag.
- `assets/` — branding and diagrams used by the README.

## Rules

1. **No engine code here.** Anything about storage/SQL/RQL/FFI belongs in
   `redline-core`. Keep this repo thin: docs, pointers, installer, release glue.
2. **Public-first.** This repo is built for public access; GitHub `neverhuman` is the
   primary audience. Don't surface internal-only (`127.0.0.1`) URLs in the README —
   keep those in `family.json` / `FAMILY.md` under the "internal" column.
3. **Keep pointers in sync.** If a family repo is renamed/moved, update `family.json`,
   `FAMILY.md`, and `README.md` together.
4. **MR-only.** Land via a jeryu PR; `main` mirrors to `github.com/neverhuman/RedlineDB`
   on a green merge.
5. **jankurai standard.** Audit with the pinned `~/.cargo/bin/jankurai` (never the
   stale `~/.local/bin` shadow).
