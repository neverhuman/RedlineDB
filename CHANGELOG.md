# Changelog

## 8.0.0-rc.0

- Added a Rust receipt-driven Redline family CI and proof-lock control plane.
- Added local-Jeryu-only checkout, tag, manifest, mirror, and freshness checks.
- Added rollback-safe lock/receipt writes and Rust safety tests.
- Kept cutover ineligible until exact reviewed tags and fresh Jain/Jeryu
  consumer evidence exist.
- Advanced the canonical Redline core identity to the immutable `.jain.3`
  durability repair after reviewed reopen regression coverage.
- Bound both consumer receipts to the canonical manifest and policy, each
  consumer's manifest and CI policy, and a fresh checksummed test log.
