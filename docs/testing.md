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
The protected control-plane lane never creates a family-container symlink and
does not need the child checkouts: `control-validate` reads only the canonical
manifest, schemas, and authoritative lock from its exact standalone runner.
The required entrypoint removes a parent runner's temporary global Git-config
override before release readiness. This prevents mirror-cache rewrites from
disguising canonical local-Jeryu remote identity while retaining the operator's
supported local-forge credential helper.

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

Candidate readiness permits the compatibility mirror to be absent only when
the authoritative lock and checksum are valid and the proof explicitly says
`cutover_eligible=false`. `review-lock-verify` and the release-readiness receipt
report that state as `authoritative-only-historical`. A partial mirror pair,
mismatched bytes or checksums, malformed proof value, or eligible authoritative
lock without its mirror fails closed. Only normal two-consumer `proof-refresh`
writes the authoritative lock, mirror, both sidecars, and operation receipt.

The Jain.4 successor receipts remain verifiable historical artifacts, but they
are not requirements for the current Jain.6 candidate:

```bash
./redlinectl successor-receipt-verify \
  release-evidence/8.0.0/redline-proof-successor-jain4-reconciled.json
./redlinectl review-lock-verify
```

The first command verifies the closed historical receipt and Jain.4 identities.
The second verifies current Jain.6 readiness and may legitimately report the
mirror-absent historical state. Neither command claims cutover eligibility.

`just family-ci` is intentionally stronger: every child must be clean `main`,
equal its local-Jeryu forge head, and either have no proposed tag yet or have an
immutable tag already bound to that exact commit. It executes the complete
child lanes in automatically removed standalone `git clone --no-local`
checkouts detached at the exact reviewed SHA and writes checksummed JSON plus
raw logs. Each clone must have a physical `.git` directory equal to its common
Git directory, full history, no alternates, no linked-checkout registry, a clean
detached HEAD, and the canonical Jeryu origin. Sandbox cleanup requires its
private marker, runs after successful or failed child commands, and refuses
symlinked root components. Family CI never registers a Git worktree.
Child commands use each repository's pinned toolchain, never the control
plane's `RUSTUP_TOOLCHAIN` override. The runner also removes inherited release,
base-ref, Cargo-target, Rust flag/wrapper/target, and `CARGO_PROFILE_*`
overrides before every child. It preserves the transaction's physical `TMPDIR`
and normal tool homes. Web alone receives `JANKURAI_BASE_REF`, derived from the
manifest's authenticated prior immutable tag, commit, and archive checksum
after remote-tag and strict-ancestry verification. Other children receive no
release or base override. Before Core CI, the runner builds the exact reviewed
Redline Testing release package with its clone-local target, verifies its
commit, manifest, binary, and hashes, and records that binding under Core's
dependency artifacts.

Common repair signatures:

- `existing immutable tag ... points to ...`: do not move it; correct the
  release plan or choose a separately reviewed new identity.
- `checkout is dirty` or `branch is ...`: preserve the work, then refresh a
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

Each expected exception records its `purpose`, `reason`, `common fixes`, and a
local `repair_hint`, plus the owning lane, preserved artifact or receipt, and
rerun command. Unexpected exceptions remain fatal; they are never converted
into a warning, silent fallback, or eligibility waiver.
