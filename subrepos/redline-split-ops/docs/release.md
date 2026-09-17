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

The fresh Jain.6/Web Jain.2 family and two-consumer proof transaction is carried
by control-plane identity `redline-split-ops-v8.0.0-split.3`. Its control
release commit and checksum remain paired `PENDING` until the complete derived
evidence and lock bundle passes protected review and is immutably tagged.

The authorized corrective family identities are
`redline-core-v4.1.0-jain.6`, `redline-v4.1.0-jain.2`,
`redline-testing-v1.0.1-jain.1`, and `redline-web-v0.1.0-jain.2`. Earlier tags
remain immutable. Before proof refresh, every manifest commit and release-tree
SHA256 must be exact and must match the reviewed main commit and immutable tag.
The reviewed Core Jain.6 identity is commit
`d0de59930141baffcfa2b514480e75b14627f24d` with release-tree SHA-256
`6dfd1a6d4fa160643b901b633f1a613c78fa51eed83c7e0b5360309ae1fcab2f`.
The reviewed Web Jain.2 identity is commit
`215fd9fc9008e4943973598754d591d39c621598` with release-tree SHA-256
`0aa807a7d8f13bdbfbc33cf7cb1c94deda98ca6c6516eae12ab9313a3b9b5082`.
Its governed prior-release authority is immutable
`redline-web-v0.1.0-jain.1` at
`09fd93be10238cd85abc164b0b01cf0f681ea304`, archive SHA-256
`27fef5fd44ad897dbaa244963312b6861e0c54b951ddb4f645f715fbf417c8a9`.
Family CI fails closed unless the remote tag metadata, deterministic archive,
and strict prior-to-current ancestry all match; only Web receives this base.
Both Jain and Jeryu consumer receipts are mandatory; proof refresh has no waiver
or single-consumer acceptance path. Until fresh receipts produce the lock pair,
the tracked Jain.3 authoritative lock is intentionally historical and
cutover-ineligible, and the compatibility mirror may be absent. The historical
Jain.4 prepared/reconciled receipts remain independently verifiable but are not
current Jain.6 readiness gates.

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
