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

The authorized corrective family identities are
`redline-core-v4.1.0-jain.3`, `redline-v4.1.0-jain.2`,
`redline-testing-v1.0.1-jain.1`, and `redline-web-v0.1.0-jain.1`. Earlier tags
remain immutable. Before proof refresh, every manifest commit and release-tree
SHA256 must be exact and must match the reviewed main commit and immutable tag.
The reviewed Core identity for `.jain.3` is commit
`7137a1ee2d04be4eb6931d99ff78b8a52c827900` with release-tree SHA-256
`baec7cb2c084ffa5c3f10770ff62e8cadf869d7e19e21db849effd47e0871d83`.
The immutable tag was created only after the rebound control manifest and fresh
family CI passed and now resolves that exact commit on local Jeryu. Both Jain
and Jeryu consumer receipts are mandatory; proof refresh has no waiver or
single-consumer acceptance path.

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
