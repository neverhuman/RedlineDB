# Changelog

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
- Advanced the canonical Core identity to immutable `.jain.5` while retaining
  Jain.4 successor receipts as historical verification-only artifacts.
- Advanced the canonical Core identity to immutable `.jain.6` after its sealed
  Rust-only NUMA replacement passed default and all-feature release gates.
- Advanced the canonical Web identity to immutable `.jain.2` after its sealed
  required, browser E2E, artifact, and governed audit lanes passed.
- Made candidate readiness accept only a valid explicitly ineligible
  authoritative lock when the compatibility mirror is absent.
- Replaced family-CI worktree isolation with marker-bound, automatically
  removed full `git clone --no-local` sandboxes with no object alternates.
- Added manifest-digest-bound successor preparation, protected split-state
  review, and post-merge mirror reconciliation without weakening strict
  operational lock or cutover verification.
- Bound both consumer receipts to the canonical manifest and policy, each
  consumer's manifest and CI policy, and a fresh checksummed test log.
