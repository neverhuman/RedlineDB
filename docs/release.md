# Release process

`neverhuman/RedlineDB` is the release authority for the engine and every included
component. The executable workflows live in the root `.github/workflows/`.
Historical split-repository release instructions and Jain/Jeryu receipts remain
under `subrepos/` for reference. They do not grant deployment authority.

## Required acceptance

Run `just required` locally. It covers engine tests, all declared conformance
suites, central client, web, integration, packaging, security, full-graph dependency review,
and the Jankurai ratchet; none is advisory or soft-gated. Local packaging checks
the current platform. GitHub additionally builds and tests all four native targets.
The branch-protection check `RedlineDB/required` rejects failed, cancelled or
skipped required jobs. Merge with a squash commit to preserve linear history.

CI uses the included `subrepos/redline-testing` runner and an engine from the
same parent commit. Missing evidence, including `rql_phase1`, and compatibility
regressions fail acceptance. Security reviews every active Cargo lockfile,
checks the inherited cargo-deny policies, scans source for secrets and checks npm
for high-severity advisories. The auditor is downloaded from GitHub and verified
against pinned archive and executable SHA-256 digests.

## Candidate and stable publication

The engine crates are already versioned at 4.1.0. This consolidation publishes
GitHub packages; it does not publish a crates.io chain or modify consumer databases.
Release notes are in `docs/migration/RELEASE_NOTES.md`; the existing engine changelog
retains the development history.

1. Merge the migration PR after every required check passes. Verify required CI
   again on the merged `main` commit, and verify the exact README source commands
   from an anonymous clone and the GitHub source archive.
2. Create the immutable annotated tag `v4.1.0-rc.1` at that verified commit and
   push it. Use a signed tag when a maintainer signing key is configured.
3. `.github/workflows/release-build.yml` runs the complete acceptance workflow,
   generates GitHub build-provenance attestations, then creates and publishes
   the prerelease. It never overwrites an existing release or asset.
4. Verify the published candidate archives and installer, including SQL,
   persistence/reopen, server transactions, FFI, embedded web queries, unsupported
   platforms, checksum rejection and installation paths containing spaces.
5. Create `v4.1.0` at the accepted candidate commit. The same workflow repeats
   acceptance and publishes the stable release. Stable commits must belong to
   `origin/main`. Verify the README's exact installer commands against that release.
6. Mark former component repositories superseded and retire their Redline
   publication paths only after stable acceptance. Preserve their existing tags,
   releases, historical proof records and installed consumer authority.

Do not create a release manually while its workflow is running. If a published
candidate needs a fix, use a new immutable candidate tag and requalify it. Never
move an existing tag or replace a published archive. The current publisher accepts
`v4.1.0` and `v4.1.0-rc.N`; extend that policy deliberately for later versions.

## Packages and provenance

`bash scripts/package-release.sh` with `TAG=v4.1.0-rc.1` produces three archives
for the current native platform. The root `packages.yml` matrix covers Linux
x86_64/ARM64 (glibc 2.35+) and macOS Intel/Apple Silicon (macOS 15+).

- `redlinedb-TAG-PLATFORM.tar.gz`: CLI, server, native libraries and C headers.
- `redline-web-TAG-PLATFORM.tar.gz`: web server with embedded frontend assets.
- `redline-testing-TAG-PLATFORM.tar.gz`: conformance runner, corpus and client smoke tool.

Each archive has a `.sha256` sidecar and contains dependency notices, licenses,
a CycloneDX SBOM and `share/redlinedb/build-provenance.json` recording its parent
commit, tag, platform and compiler. The web archive includes a frontend SBOM;
the testing SBOM includes the bundled client. GitHub release attestations bind
the uploaded archive bytes to the workflow identity and source commit.

The binary installer defaults to `~/.local`, honors `VERSION` and `PREFIX`,
and fails if a checksum is absent or incorrect. `REDLINEDB_SHA256` adds an
independent digest pin. No development toolchain is needed at runtime, and the
installer never replaces `sqlite3`.

## Evidence and rollback

Acceptance artifacts are attached to the CI/release run: conformance evidence,
security receipts, component audit reports and all native package archives.
`docs/migration/inventory.json` and `redlinectl validate --history` verify the
preserved source identities and recovery refs. Earlier audit baselines remain
recorded; policy qualification does not lower their score floors.

If a release is defective, retain its immutable forensic evidence, mark it as
superseded in release notes, and publish a corrected version through the same
acceptance process. Consumers can select a previous verified release with
`VERSION`; back up their data and review format compatibility before changing
installed versions. Release automation does not roll back or replace databases.
