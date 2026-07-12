# Testing and repair

Run `just check` for formatting, clippy, Rust unit tests, manifest validation,
and structural lock validation. Run `just security` for fail-closed secret,
dependency, workflow, and SBOM checks. Run `just score` for the pinned audit
and the Rust score/hard-finding/cap gate.

`just family-ci` is intentionally stronger: every child must be clean `main`,
equal its local-Jeryu forge head, and either have no proposed tag yet or have an
immutable tag already bound to that exact commit. It executes the complete
child lanes in detached worktrees and writes checksummed JSON plus raw logs.

Common repair signatures:

- `existing immutable tag ... points to ...`: do not move it; correct the
  release plan or choose a separately reviewed new identity.
- `worktree is dirty` or `branch is ...`: preserve the work, then refresh a
  clean `main`; never reset or delete it.
- `tampered evidence`: regenerate the producer receipt and checksum together.
- `proof is not derived as eligible`: obtain fresh family and consumer
  evidence; never edit the lock boolean.

The Rust command errors include the failing repository, field, or path and the
next exact lane is recorded in `agent/test-map.json`.
