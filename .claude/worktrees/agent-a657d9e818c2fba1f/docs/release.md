# Release process

Authoritative release control surface for the redlinedb workspace.
Cross-referenced from `CHANGELOG.md`, `.github/workflows/jankurai.yml`,
and the security lane in `justfile`. Audit reference: HLT-025
release-readiness, HLT-016 supply-chain drift.

## Version source

The workspace is published as five crates pinned at the same version.
Each crate carries its own `version = "X.Y.Z"` in
`crates/<crate>/Cargo.toml` (the workspace itself does not yet pin a
`[workspace.package].version`). To bump:

```
cargo install cargo-edit
cargo set-version --workspace 0.2.0
```

`cargo set-version` rewrites every member's `version` and any
`path = "..."` workspace dependency that references the bumped crate.
Commit the manifest churn in a single commit titled
`chore(release): vX.Y.Z`.

## Changelog discipline

`CHANGELOG.md` follows the
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) format.
Every release MUST land a new section in the form
`## [X.Y.Z] - YYYY-MM-DD` *before* the tag is pushed. The
`Unreleased` section at the top stays empty between releases; PRs add
entries under `Unreleased` and the release commit promotes them.

## Release process

Ordered steps. Each step is gated by the previous one passing.

1. **Pre-flight**: `just check` (fast + score + security + rust-map +
   rust-witness + rust-diagnose). All must exit zero.
2. **Bump**: `cargo set-version --workspace X.Y.Z` + edit
   `CHANGELOG.md` (`Unreleased` → `## [X.Y.Z] - YYYY-MM-DD`).
3. **Commit + signed tag**:
   ```
   git commit -am "chore(release): vX.Y.Z"
   git tag -s vX.Y.Z -m "redlinedb vX.Y.Z"
   git push origin main --follow-tags
   ```
4. **Dry-run publish** (dependency order, every crate):
   ```
   cargo publish --dry-run -p redlinedb-domain
   cargo publish --dry-run -p redlinedb-kernel
   cargo publish --dry-run -p redlinedb-sql
   cargo publish --dry-run -p redlinedb-ffi
   cargo publish --dry-run -p redlinedb
   ```
5. **Publish to crates.io** (same order, no `--dry-run`). Wait for
   each upload to index on crates.io before the next, otherwise the
   dependent's `cargo publish` will 404 on the just-pushed version.
6. **Cut the GitHub release**:
   ```
   gh release create vX.Y.Z --title "redlinedb vX.Y.Z" \
     --notes-file CHANGELOG-vX.Y.Z.md \
     target/release/redlinedb-cli \
     target/release/redlinedb-server \
     target/release/SHA256SUMS \
     target/release/sbom.cdx.json
   ```

## CI evidence

The audit/security gate lives in
[`.github/workflows/jankurai.yml`](../.github/workflows/jankurai.yml).
The `security` job runs `cargo audit`, `cargo deny check`, and
`gitleaks detect`; it is a required check on every PR and push to
`main`. The `dependency-review` step (PR-only) compares the base and
head manifests for vulnerable adds. The audit job uploads
`agent/repo-score.json` + the SARIF security feed. Sample runs are
linked from the Actions tab of the repository — pick any green run on
a release tag for permalink evidence.

## Integrity / provenance

Release artifacts MUST ship:

- **SHA-256 manifest** — `cd target/release && sha256sum redlinedb-cli
  redlinedb-server > SHA256SUMS`. Attach `SHA256SUMS` to the GitHub
  release.
- **SBOM** — `cargo install cargo-cyclonedx` once, then
  `cargo cyclonedx --format json --output-pattern bom --all`.
  Attach the generated `bom.cdx.json` (renamed `sbom.cdx.json`) to
  the release.
- **Signed tag** — `git tag -s vX.Y.Z` (gpg or sigstore-style). The
  repo `SECURITY.md` lists the maintainer key fingerprint; verifiers
  run `git tag -v vX.Y.Z`.
- **Dependency review** — the GitHub
  `actions/dependency-review-action` step in
  `.github/workflows/jankurai.yml` fails the PR if a high-severity
  advisory is newly introduced.

## Rollback runbook

A bad release recovers in three moves:

1. **Yank crates** (each, in reverse dependency order):
   ```
   cargo yank --vers X.Y.Z -p redlinedb
   cargo yank --vers X.Y.Z -p redlinedb-ffi
   cargo yank --vers X.Y.Z -p redlinedb-sql
   cargo yank --vers X.Y.Z -p redlinedb-kernel
   cargo yank --vers X.Y.Z -p redlinedb-domain
   ```
   `cargo yank --undo` reverses the operation if the issue turns out
   to be benign.
2. **Delete the GitHub release** (keep the tag for forensics):
   ```
   gh release delete vX.Y.Z --cleanup-tag=false
   ```
3. **Ship a superseding patch**: bump to `X.Y.Z+1` via the full
   release process above. The changelog entry MUST cite the yanked
   `X.Y.Z` and the CVE / issue that motivated the supersede.

For pre-tag rollbacks (the release fails between `cargo publish` of
crate N and crate N+1), file a `redlinedb-<failed-crate>` GitHub issue
and re-attempt the same `X.Y.Z` after fixing the underlying problem;
do not bump until the user-visible crates (`redlinedb`,
`redlinedb-ffi`) are all on the same version.
