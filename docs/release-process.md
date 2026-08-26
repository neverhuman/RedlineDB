# Redline release process

The release version comes from `Cargo.toml`, `Cargo.lock`, and
`agent/standard-version.toml`; `repos.manifest.toml` is the sole authority for
the four child repositories and their immutable tags. Release history is in
`CHANGELOG.md`. This control plane prepares the Redline dependency cutover for
Jain 8.0.0 but does not itself push images, change routes, or promote production.

## Required sequence

1. Bind exact Hub Jain.4, Core Jain.6, Testing Jain.2, and Web Jain.2 tags,
   commits, and tree checksums in the manifest. Hub Jain.3 is immutable but
   unusable because it embeds the Jain.2 version; never move or reuse it.
   This source checkpoint intentionally makes `control-validate` reject the
   still-eligible historical lock because its Hub and Testing identities no
   longer match the manifest. It remains fail-closed until proof refresh writes
   the exact reviewed replacement.
   `review-lock-verify` may report `authoritative-only-historical` only when the
   compatibility mirror and sidecar are both absent and the authoritative proof
   is valid and explicitly ineligible. The Jain.4 successor receipts remain
   verifiable history, not current readiness requirements.
2. Run `bash ops/ci/quality-gates.sh` on the exact reviewed control commit.
3. Run `just family-ci` with all child repositories clean, on `main`, and equal
   to local Jeryu. Web's full changed surface must use the authenticated
   `redline-web-v0.1.0-jain.1` commit as its governed base for Jain.2; the
   runner verifies the prior tag, archive checksum, and strict ancestry before
   injecting that base into Web alone. Preserve the receipt, checksum sidecar,
   and named logs.
4. Create the immutable successor tag, then verify every local, Jeryu, and
   mirror tag resolves the manifest commit and
   release-tree checksum. Immutable tags are never recreated or moved.
5. Build and test both Jain and Jeryu against the exact Redline engine. Each
   clean-main producer writes closed-schema evidence, its real test log, and a
   checksum sidecar.
6. Place those real inputs under `release-evidence/8.0.0/` and run
   `just proof-refresh <family-ci> <jain-evidence> <jeryu-evidence> <receipt>`.
   No eligibility flag or single-consumer path exists.
7. Require the authoritative and compatibility lock bytes and checksums to
   match, then run `just cutover-verify`. This reconstructs the lock from the
   still-fresh evidence and live tag readback.
8. Commit the derived authoritative lock, sidecar, receipt, and evidence bundle
   through protected review. The normal two-consumer `proof-refresh` operation
   is the sole writer of its six outputs: authoritative lock and sidecar,
   compatibility lock and sidecar, proof receipt and sidecar. Each write is
   atomic and reported process errors roll back to prior bytes, but the six-file
   set is not crash-atomic. Never restore any member by hand. Rerun
   `just cutover-verify` before any Jain dependency wave.

## Evidence and integration

The release bundle contains the family receipt and logs, Jain and Jeryu
consumer evidence and test logs, proof-refresh receipt, lock sidecars, manifest
hash, policy hash, source commits, engine identity, and freshness timestamps.
The downstream integration gate is Jain Core's exact Redline lock verification,
followed by the ordered Jain dependency waves and final image/runtime checks.

## Failure and rollback

Any stale evidence, tag mismatch, failed check, dirty checkout, unknown field,
or lock-byte difference is a hard stop. Follow `ROLLBACK.md`: retain immutable
tags and reviewed Git history, refuse cutover, and keep the Jain runtime
rollback target at `7.0.6`. Production promotion remains a separately reviewed
and explicitly authorized Jain operation.
