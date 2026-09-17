@/home/ubuntu/.codex/RTK.md

# redline-central agent guide

This repository owns the Rust client and switchable database shim for the shared
RedlineDB service. It is a standalone repository; sibling repositories are not
part of its build or Git history.

Rules:

- Never create a Git worktree. Work only in this canonical primary checkout.
- Land changes through a protected local-Jeryu pull request. Never push `main`.
- Keep the native Redline release identity (`4.1.0-jain.N`); Jain binds the
  accepted immutable Redline identity into its own release authority.
- Select Jankurai only from the root-controlled release PATH, then freeze and
  verify its physical path and closed release identity: version `1.6.11`, tag
  `v1.6.11-deadlang-precision-split.2`, source revision
  `4dfbdfa3585f1928d5f996d7b5e14608dff14a03`, and binary SHA-256
  `96d99e6e7d8dc9cf23df1081edd1f975231456592f81d9405385219a2c7298aa`.
- Run `rtk bash scripts/ci-local.sh required` before requesting review.
- The live TCP smoke binary needs a separately managed RedlineDB server. The
  protected required lane remains host-local and does not start external services.

Ownership:

- `crates/redlinedb-client/`: framed-protocol client.
- `crates/db-shim/`: SQLite/Redline backend abstraction and namespace contract.
- `docker/`: central-service packaging contract.
- `ops/`, `scripts/`, `agent/`: CI and proof metadata.
