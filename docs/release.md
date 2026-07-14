# Release and rollback

## Launch gate

This repository supports the Jain 8.0.0 candidate and Redline 4.1.0 Jain
cutover. Its version sources are `Cargo.toml`, `Cargo.lock`, and
`agent/standard-version.toml`; release history is in `CHANGELOG.md`.

The release sequence is `just required`, `just security`, `just score`, a fresh
`just family-ci`, immutable tag readback from local Jeryu, checksummed Jain and
Jeryu consumer evidence, `just proof-refresh ...`, then
`just cutover-verify`. Proof refresh writes the authoritative lock, mirror,
both SHA256 sidecars, and operation receipt as one rollback-safe transaction.
SBOM integrity evidence is produced by `just security`.

For a one-revision Core correction, the reviewed manifest change is paired with
`proof-refresh --prepare-successor` before the new tag exists. That read-safe
mode derives only the candidate authoritative lock from the manifest-bound
predecessor without accepting release evidence, an eligibility flag, or writing
the product mirror. Following the protected control-plane merge,
`proof-refresh --reconcile-successor` may atomically update only the exact
checksummed predecessor mirror. Fresh family CI, the immutable successor tag,
both consumer receipts, and normal proof refresh are still required before
cutover.

The authorized corrective family identities are
`redline-core-v4.1.0-jain.4`, `redline-v4.1.0-jain.2`,
`redline-testing-v1.0.1-jain.1`, and `redline-web-v0.1.0-jain.1`. Earlier tags
remain immutable. Before proof refresh, every manifest commit and release-tree
SHA256 must be exact and must match the reviewed main commit and immutable tag.
The reviewed Core identity for `.jain.4` is commit
`3567bdced0ca1fe3671c9ebda876c914e2fc2c9e` with release-tree SHA-256
`b36a4ac5afd332bab7473f1007061356a991a7590eeda1336809be5dad5ce746`.
The immutable tag remains absent until this rebound control manifest and fresh
family CI pass, and must then resolve that exact commit on local Jeryu. Both
Jain and Jeryu consumer receipts are mandatory; proof refresh has no waiver or
single-consumer acceptance path. Until fresh receipts produce a new lock, the
tracked `.jain.3` lock is intentionally historical and cutover-ineligible.

This control plane is stateless, so backups are not applicable; the durable
truth is reviewed Git history plus checksummed receipts. Monitoring is the
machine-readable family and cutover status. Abuse controls are not applicable
to this local-only operator tool. Network spend and external spend budgets are
zero, with `ATOMICSOUL_PUSH=0` as the deployment kill switch. The release
readiness receipt records each of these controls without claiming production
promotion.

The current state remains a candidate. Production promotion, public routing,
image pushes, and live aliases are outside this control plane. Rollback means
retaining the prior ineligible lock and known Jain runtime target `7.0.6`, then
refusing cutover; it never means moving an immutable tag, force-pushing, or
rewriting child history.
