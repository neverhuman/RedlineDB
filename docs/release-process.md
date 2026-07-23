# Redline release process

The release version comes from `Cargo.toml`, `Cargo.lock`, and
`agent/standard-version.toml`; `repos.manifest.toml` is the sole authority for
the five product repositories and their immutable tags. Release history is in
`CHANGELOG.md`. This control plane prepares the Redline dependency cutover for
Jain 8.0.1 but does not itself push images, change routes, or promote production.

## Required sequence

1. Land or adopt all six canonical `veox/*` repositories without changing the
   existing immutable product tags. Source authority validation is not lock or
   cutover evidence.
2. Preserve the historical `.jain.3` lock as red evidence until the governed
   proof workflow can replace both lock copies atomically.
3. Run `just family-ci` with all five products clean, on `main`, equal to their
   local-forge heads, and supplied real Central service DSNs. Preserve the
   receipt, checksum sidecar, and named logs.
4. Verify every local and `veox/*` forge tag resolves the manifest commit and
   release-tree checksum. Immutable tags are never recreated or moved.
5. Build and test both Jain and Jeryu against the exact Redline engine. Each
   clean-main producer writes closed-schema evidence, its real test log, and a
   checksum sidecar.
6. Place those fresh real inputs under `release-evidence/8.0.1/` and run
   `just proof-refresh <family-ci> <jain-evidence> <jeryu-evidence> <receipt>`.
   No eligibility flag or single-consumer path exists.
7. Require the authoritative and compatibility lock bytes and checksums to
   match, then run `just cutover-verify`. This reconstructs the lock from the
   still-fresh evidence and live tag readback.
8. Commit the derived lock, sidecar, and evidence bundle through protected
   review. On reviewed `main`, restore the compatibility mirror from the exact
   lock bytes and rerun `just cutover-verify` before any Jain dependency wave.

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
