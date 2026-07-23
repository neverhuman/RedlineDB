# Release and rollback

## Launch gate

This repository supports the Jain 8.0.1 candidate and Redline 4.1.0 Jain
cutover. Its version sources are `Cargo.toml`, `Cargo.lock`, and
`agent/standard-version.toml`; release history is in `CHANGELOG.md`.

The release sequence is `just required`, `just security`, `just score`, a fresh
`just family-ci`, immutable tag readback from the local forge, checksummed Jain and
Jeryu consumer evidence, `just proof-refresh ...`, then
`just cutover-verify`. Proof refresh writes the authoritative lock, mirror,
both SHA256 sidecars, and operation receipt as one rollback-safe transaction.
SBOM integrity evidence is produced by `just security`.

The present recovery is not the historical one-revision successor workflow:
Core `.jain.5` and Central `.jain.1` already exist, while the tracked lock still
has four `.jain.3`-era product rows. Source authority validation cannot relabel
that lock. Repository adoption, fresh five-product family CI, both consumer
receipts, and normal proof refresh are required before cutover.

The authorized recovery identities are
`redline-core-v4.1.0-jain.5`, `redline-v4.1.0-jain.2`,
`redline-testing-v1.0.1-jain.1`, and `redline-web-v0.1.0-jain.1`. Earlier tags
remain immutable. Before proof refresh, every manifest commit and release-tree
SHA256 must be exact and must match the reviewed main commit and immutable tag.
The list also requires `redline-central-v4.1.0-jain.1`. Core `.jain.5` is
commit `2924a34bdca8263adc9ebff9220f5bb99ba4323f` with release-tree SHA-256
`402043243b4d4b057ad1eeb89e8b2f098ed8c402948b8382bc75d0e4b7128704`.
Both
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
