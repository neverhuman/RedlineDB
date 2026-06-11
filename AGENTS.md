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
- `scripts/fuse.sh` — dev-only "git repo fusion": clones the three sibling repos into a
  gitignored `.fusion/` working tree for end-to-end local iteration (`just fuse`). End
  users never run it; it never touches the tracked, thin front door.
- `assets/` — branding and diagrams used by the README.

## Docs for agents

- [docs/architecture.md](docs/architecture.md) — repo structure, the family, release flow, and CI layout.
- [docs/boundaries.md](docs/boundaries.md) — what crosses the hub boundary; data flow; the public API surface.
- [docs/release.md](docs/release.md) — how to cut a release, cost budget, rollback.
- [docs/deprecation.md](docs/deprecation.md) — family lifecycle: the gated plan for retiring the old `~/redlineDB` checkout and consolidating `redline-testing`.
- [docs/exceptions/README.md](docs/exceptions/README.md) — typed error catalog; maps each ERR_* code to a repair recipe.
- [docs/boundaries.md](docs/boundaries.md) — the hub boundary surface: what crosses the installer API, what doesn't.

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
