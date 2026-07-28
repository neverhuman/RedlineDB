# Architecture

`jain-split-ops` is a control-plane repository. It owns the manifest, generator,
host CI runner, Jeryu onboarding helpers, and policy validation scripts for the
Jain split family.

The family member repositories live beside this repo. Their operational remote
is local Jeryu at `http://127.0.0.1:8787/git/veox/<repo>.git`. Public release
publishing is separate from development source resolution.

The materializer writes standard files into member repos: `AGENTS.md`, CI lanes,
agent policy files, lockfiles, and generated local patch examples. The validator
keeps those outputs aligned with the local-Jeryu contract.

## Host CI trust flow

The unprivileged parent accepts a clean canonical checkout only long enough to
prepare a bounded request. The root-owned sandbox broker snapshots that request,
authenticates the configured protected control-plane ref and requested product
object from local Jeryu, and materializes standalone physical checkouts. It
then starts a private, network-isolated worker with read-only control,
security-tool, advisory-database, and authority mounts. The worker cannot see
the forge credential or root request. When it stops, a separate root auditor
validates exact-head evidence before the one-shot publisher can post
`jankurai/proof` and the required check.

Caller environment is data, never authority. Root-owned paths, security inputs,
sibling bindings, the release-authority projection, and their checksums are
denied in the caller request and injected only after the root broker has
authenticated and sealed them. The reviewed worker independently validates the
mounted bytes before running any product lane.

### Closed JavaScript dependency custody

JavaScript packages follow the same authority boundary. Jain Web receives its
exact-lock pnpm store. Redline Web receives a separate
`jain.npm-cache/v1` CACache authority bound to its committed
`apps/web/package-lock.json` and the Linux/x64/glibc closure. The closure
identity includes each package path, registry URL, and SHA-512 integrity, so
the same URL cannot be rebound to different bytes or silently satisfy a
different lock.

Root validates the closed cache inventory, every content digest, every
canonical URL index, the exact lock digest, platform, and npm version before
making a writable per-request copy. Only that copy is exposed through
`NPM_CONFIG_CACHE`; `NPM_CONFIG_OFFLINE=true` and network isolation remain
fail-closed. The immutable cache under `/var/lib/jain-host-ci/npm-cache/` is
never mounted writable, and an ambient user npm cache is never a runtime
source.

Redline Web browser E2E uses a separate
`jain.playwright-browser-cache/v1` authority bound to the same exact package
lock, Playwright 1.60.0, Chromium revision 1223, headless-shell revision 1223,
ffmpeg revision 1011, and the Linux/x64/glibc inventory. Root validates all
595 files, 18 directories, modes, links, sizes, and digests. Mutable
`DEPENDENCIES_VALIDATED` markers are excluded so each request performs the
dependency check. Each request receives a writable empty Playwright registry
only for `__dirlock` and `.links`; the three
digest-addressed browser payload directories are bind-mounted into it
read-only. The worker remains network-isolated with browser downloads disabled.

## Release-authority projection

`repos.manifest.toml` is the sole source for Jain release version, candidate
status, formal-GA state, and rollback target. The root broker asks the governed
`splitctl manifest --json` implementation to parse the authenticated manifest,
compares its reported canonical digest with a direct SHA-256, and renders an
eight-field `jain.release-authority-projection/v1` document. Its
`control_commit` must equal the protected control checkout; its
`source_manifest_sha256` must equal the mounted manifest.

The projection is a per-request immutable file at
`/opt/jain-ci/authority/release-authority.json`, owned by root, mode `0444`,
with one link and a bounded size. Root binds its SHA-256 in worker re-execution
state and the environment. The worker verifies path, realpath, metadata,
checksum, exact keys, control commit, manifest digest, release `10.0.0`,
candidate status, `formal_ga=false`, and rollback `8.0.1`. This carries release
identity into artifact checks without granting a developer checkout, caller
variable, or product repository authority over family metadata.
