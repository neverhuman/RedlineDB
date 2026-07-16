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
- Select Jankurai only through `/home/ubuntu/.jeryu/bin/jankurai`, version
  `1.6.11`, SHA-256
  `fdb42e5fa7d9851c0729e59bf1e582c895aa9cfc03a7175b420c6025d2fd014e`.
- Run `rtk bash scripts/ci-local.sh required` before requesting review.
- The live TCP smoke binary needs a separately managed RedlineDB server. The
  protected required lane remains host-local and does not start external services.

Ownership:

- `crates/redlinedb-client/`: framed-protocol client.
- `crates/db-shim/`: SQLite/Redline backend abstraction and namespace contract.
- `docker/`: central-service packaging contract.
- `ops/`, `scripts/`, `agent/`: CI and proof metadata.
