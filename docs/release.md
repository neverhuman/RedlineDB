# Release and rollback

This repository supports the Jain 8.0.0 candidate and Redline 4.1.0 Jain
cutover. Its version sources are `Cargo.toml`, `Cargo.lock`, and
`agent/standard-version.toml`; release history is in `CHANGELOG.md`.

The release sequence is `just required`, `just security`, `just score`, a fresh
`just family-ci`, immutable tag readback from local Jeryu, checksummed Jain and
Jeryu consumer evidence, `just proof-refresh ...`, then
`just cutover-verify`. Proof refresh writes the authoritative lock, mirror,
both SHA256 sidecars, and operation receipt as one rollback-safe transaction.
SBOM integrity evidence is produced by `just security`.

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
