# Testing and repair

Run `just check` for formatting, clippy, Rust unit tests, manifest validation,
and structural lock validation. Run `just security` for fail-closed secret,
dependency, workflow, and SBOM checks. Run `just score` for the pinned audit
and the Rust score/hard-finding/cap gate.

The local Jeryu host runner enters through `scripts/ci-local.sh required`. That
entrypoint runs `ops/ci/quality-gates.sh`, so the single protected required
status is published only after required, security, pinned score, and release
readiness all pass on the exact detached commit. It accepts the narrower
`security`, `score`, and `release-readiness` lanes for targeted repair reruns.
Because the compatibility lock intentionally lives outside this repository, a
detached runner must set `REDLINE_SPLIT_MIRROR_LOCK` to the reviewed mirror; the
entrypoint fails before any success publication when neither that variable nor
the normal sibling path is available.

The required lane uses `./redlinectl control-validate`, which validates the
canonical manifest and control-plane lock without requiring sibling checkouts.
Mirror identity, live family checkout, and hub-engine guards remain in
`./redlinectl lock-verify`, `./redlinectl validate`, and
`./redlinectl family-ci`.

An explicitly ineligible historical lock may retain its prior immutable tags
while the canonical manifest advances to reviewed corrective identities. It
cannot pass cutover. `proof-refresh` is the only writer that can replace it and
requires exact manifest product, revision, commit, tree checksum, remote, and
protection-policy metadata first.

`just family-ci` is intentionally stronger: every child must be clean `main`,
equal its local-Jeryu forge head, and either have no proposed tag yet or have an
immutable tag already bound to that exact commit. It executes the complete
child lanes in detached worktrees and writes checksummed JSON plus raw logs.
Child commands use each repository's pinned toolchain, never the control
plane's `RUSTUP_TOOLCHAIN` override. Before Core CI, the runner builds the exact
reviewed Redline Testing release package locally, verifies its commit, manifest,
binary, and hashes, and records that binding under Core's dependency artifacts.

Common repair signatures:

- `existing immutable tag ... points to ...`: do not move it; correct the
  release plan or choose a separately reviewed new identity.
- `worktree is dirty` or `branch is ...`: preserve the work, then refresh a
  clean `main`; never reset or delete it.
- `tampered evidence`: regenerate the producer receipt and checksum together.
- `proof is not derived as eligible`: obtain fresh family and consumer
  evidence; never edit the lock boolean.

## Agent-readable repair contract

Every Rust command error is rendered as stable, named fields: `purpose`,
`reason`, `common_fixes`, `repair_hint`, and `docs_url`. The reason includes the
failing repository, field, or path when one is available. The repair hint sends
operators to the exact owning lane recorded in `agent/test-map.json`; common
fixes require producer regeneration, immutable-tag preservation, and clean
forge-equal release verification. These fields are diagnostic only and never
relax a failed gate.
