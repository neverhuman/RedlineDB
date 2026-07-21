# Release process

`redline-testing` ships as a single tarball that RedlineDB CI consumes as a
pinned artifact. The authoritative validation and release path is entirely
local through Jeryu and physical custody under `/home/ubuntu/jain-split`.

1. **Version source.** `Cargo.toml` `[package].version` is the canonical
   version. `CHANGELOG.md` is the human-facing log of what changed.
2. **Local rehearsal.** Run `just release-local`. This drives
   `scripts/release-package.sh` which:
   - runs `cargo build --release --locked`
   - copies the binary + every corpus / metadata / schema / template file
     into `dist/redline-testing-<version>-linux-x86_64/`
   - SHA-256-hashes every file (except `release-manifest.json` itself and
     `bin/redline-testing`, which has its own top-level `binary_sha256`)
     via `find ... | xargs sha256sum | jq` — glob-driven so new corpus shards
     auto-appear without editing the recipe
   - writes `dist/redline-testing-<version>-linux-x86_64/release-manifest.json`
   - emits the tarball + `.sha256` sidecar in `dist/`
3. **Integrity check.** `cargo test --locked --test release_manifest_integrity`
   recomputes every bundled file's SHA-256 against the manifest. If a file is
   in the tarball but missing from `artifact_hashes` (or vice versa), this
   test fails loudly. It runs as part of `just pr-ci`.
4. **Custody.** `xtask custody-stage` validates every registry package checksum
   in `Cargo.lock`, stages physical SQLite/PostgreSQL oracle artifacts in-tree,
   performs `cargo build --workspace --locked --offline`, and records compiler,
   dependency-closure, oracle version, and SHA-256 identities.
5. **Tag the release.** After protected merge, the Redline family controller
   creates the next unused manifest-bound immutable
   `redline-testing-v<version>-jain.<revision>` tag with compare-and-swap
   semantics. Existing tags never move.

## Verifying a release locally

From the locally preserved release assets (`*.tar.gz`, `*.tar.gz.sha256`,
`release-manifest.json`):

```bash
sha256sum -c redline-testing-<version>-linux-x86_64.tar.gz.sha256
tar -xzf redline-testing-<version>-linux-x86_64.tar.gz
cd redline-testing-<version>-linux-x86_64

# Verify every artifact_hashes entry matches the bundled file.
jq -r '.artifact_hashes | to_entries[] | "\(.value)  \(.key)"' \
  release-manifest.json | sha256sum -c
```

## Launch-gate readiness

Before a release tag is published, the launch gate proves every control is in
place (see also [`docs/operations.md`](operations.md) and
[`docs/testing.md`](testing.md)):

- **security**: `bash ops/ci/security.sh` runs gitleaks, `cargo audit`,
  `cargo deny`, zizmor, and an SBOM; the CI `security` job is blocking.
- **provenance / integrity**: every artifact is SHA-256 hashed in
  `release-manifest.json`; custody and family-CI **provenance** receipts bind
  compiler, dependency, oracle, commit, tag, and tarball identities.
- **backup**: the content-addressed tarball + `.sha256` sidecar + manifest are
  the immutable **backups** of every shipped artifact; old tags never move.
- **monitoring**: the `release_manifest_integrity` test plus RedlineDB CI
  provide drift **monitoring** against the published corpus.
- **rollback**: ship a higher version that restores prior behavior (below).
- **abuse / rate limit**: the runner only drives allowlisted local subprocess
  shells with bounded timeouts and accepts no untrusted network input, so there
  is no abuse / rate-limit surface to throttle.

## Rollback

A release tag is immutable. To "roll back" we ship a follow-up release with a
higher version that restores the prior behavior; the old tag stays in place
so downstream consumers that pinned it keep working. RedlineDB's pin lives
in `redlineDB/scripts/ci_install_redline_testing.sh` (or equivalent); ask the
RedlineDB team to bump the pin to the desired version.

## Local-forge corrective lifecycle

The release path is 100% local. From
an ordinary branch based directly on protected local-Jeryu `main`, run the
same `bash ops/ci/pr-ci.sh`, governed Jankurai proof, security, package, and
manifest-integrity lanes. The forge must independently publish a successful
exact-head `jankurai/proof` check and the repository required check before one
independent approval and protected fast-forward merge. A null check output is
not success and must never be replaced with a synthetic check.

After forge `main` reads back the reviewed commit, the family controller cuts
the next unused immutable `redline-testing-v<product-version>-jain.<revision>`
tag and verifies its object. Existing tags never move. The Jain authority then
binds that commit and checksum through its own protected PR. Rollback restores
the previous known-good consumer pin by a reviewed source change and, when a
new artifact is required, uses the next unused corrective tag; it never moves
the old tag or edits the Redline lock by hand.

## Contract

Downstream RedlineDB CI consumes:

- `bin/redline-testing` — the runner.
- `corpus/sqlite_parity/generated_manifest.json` — pinned upstream cases.
- `corpus/sqlite_parity/cases/*.json` — extended hand + generated shards.
- `corpus/beyond_sqlite/generated_manifest.json` — beyond-SQLite oracle cases.
- `contracts/*.toml` — versioned compatibility requirements and exclusions.
- `metadata/beyond_sqlite/features.json` — the 12-entry rank/owner taxonomy.
- `schemas/*.json` — raw-record + release-manifest schemas.
- `templates/*.md` — report-generation README templates.

Adding files to the tarball: drop them into the appropriate source directory;
`scripts/release-package.sh` picks them up automatically and they appear in
`artifact_hashes`. The manifest and family-CI receipt bind the tarball as a
whole.
