# Changelog

## 8.0.1-rc.0

- Rebound the closed family authority to one control plane plus five products,
  all under canonical local-forge `veox/*` ownership; added the existing
  Central `.jain.1` release and Core `.jain.5` recovery identity without
  changing the historical lock or claiming cutover eligibility.
- Relocated the Redline family to physical standalone repositories under
  `jain-redline/` and replaced family-CI linked checkouts with exact-SHA
  standalone clones.
- Advanced only the child control-plane Jain identity and fresh evidence root
  to candidate 8.0.1; native Redline product identities and historical
  successor proof bindings remain unchanged.

## 8.0.0-rc.0

- Added a Rust receipt-driven Redline family CI and proof-lock control plane.
- Added local-Jeryu-only checkout, tag, manifest, mirror, and freshness checks.
- Added rollback-safe lock/receipt writes and Rust safety tests.
- Kept cutover ineligible until exact reviewed tags and fresh Jain/Jeryu
  consumer evidence exist.
- Advanced the canonical Redline core identity to the immutable `.jain.3`
  durability repair after reviewed reopen regression coverage.
- Advanced the canonical Redline core identity to the `.jain.4` bounded-cell
  repair after exact INSERT/UPDATE boundaries, repeated no-growth rejection,
  WAL stability, checkpoint, and reopen coverage passed protected CI.
- Added manifest-digest-bound successor preparation, protected split-state
  review, and post-merge mirror reconciliation without weakening strict
  operational lock or cutover verification.
- Bound both consumer receipts to the canonical manifest and policy, each
  consumer's manifest and CI policy, and a fresh checksummed test log.
