@../AGENTS.md

# Database contract ownership

This directory owns the machine-readable backend contract, migration guidance, and constraint
policy. The Rust implementation remains under `crates/db-shim`.

- Keep `backend-contract.toml` aligned with Cargo feature names and `db-shim::corpus::VERSION`.
- Never claim arbitrary SQL parity; evidence covers only the governed used-operation corpus.
- Never place credentials, live DSNs, generated evidence, or mutable service identities here.
- Changes require `rtk bash ops/ci/backend-dependency-guard.sh` and
  `rtk bash ops/ci/fast.sh`; release-backed semantic credit additionally requires the armed live
  Redline and Postgres corpus runs.
