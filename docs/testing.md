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
Before the existing gates run, the Rust `ci-required` wrapper verifies the
tracked predecessor and prepared authoritative digests, then constructs the
physical predecessor compatibility mirror required by the review verifier.
The serialized commands flock the held physical family-root directory in Rust;
a replaceable `.redline-family.lock` file is never a lock authority. The root
path and descriptor identity remain bound through final command validation.
`ci-required` likewise holds and repeatedly validates descriptors for the
control root, manifest, authoritative lock, and mirror parent across the gate
invocation.
The mirror and sidecar are distinct from the authoritative lock and owned by a
private sibling marker. Pre-existing, partial, aliased, symlinked, or
wrong-digest state is rejected. Cleanup removes only the marker-bound files
after revalidating their still-open creation handle, creation-time device/inode
identity, exact mode, single-link ownership, and bytes, on both gate success and
failure. Each removal first quarantines the directory entry relative to the
held parent, verifies that it is the owned inode, unlinks it, and proves the
held inode reached link count zero before cleanup state advances.
The required lane uses `control-review-lock-verify`, which validates the exact
manifest, derived lock transition, and tracked predecessor binding
without consulting sibling checkouts. The operational `review-lock-verify`
retains live clean-main and immutable-tag readback; `family-ci` remains the
authority for running every child lane.
The retained 8.0.0 successor operation receipts describe the pre-relocation
checkout paths and manifest digest, so the 8.0.1 control lane does not credit
them as current release evidence.
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

`./redlinectl proof-refresh --prepare-successor --receipt PATH` is the only
supported way to derive an ineligible authoritative candidate for a
one-revision Core correction. Tests require the successor tag to be absent,
every child main to be clean and forge-equal, and the two starting lock copies
to match the manifest-bound predecessor SHA-256. Preparation never writes the
product mirror. `review-lock-verify` accepts the temporary split state only when
the candidate has the exact bound historical digest and records it as
cutover-ineligible; strict operational verification continues to fail.
`successor-receipt-verify PATH` rejects unknown fields, a bad sidecar, path
substitution, manual eligibility, mismatched transition flags, or any manifest,
engine, predecessor, or prepared-lock identity difference.
Post-merge, `proof-refresh --reconcile-successor --receipt PATH` accepts only
that authoritative candidate plus the byte-exact predecessor mirror, updates
both copies transactionally, and emits a `reconciled` receipt. Unrelated drift
is rejected. Both receipt variants use `redline.proof-successor/v1` and have
checksum sidecars.

For a historical 8.0.0 successor-receipt audit only, verify the retained
artifact with:

```bash
./redlinectl successor-receipt-verify \
  release-evidence/8.0.0/redline-proof-successor-jain4-reconciled.json
./redlinectl review-lock-verify
```

These checks prove synchronization only; the receipt must continue to report
`cutover_eligible=false` until normal two-consumer proof refresh completes.

`just family-ci` is intentionally stronger: every child must be clean `main`,
equal its local-Jeryu forge head, and either have no proposed tag yet or have an
immutable tag already bound to that exact commit. It executes the complete
child lanes in automatically removed standalone `git clone --no-local`
checkouts detached at the exact reviewed SHA and writes checksummed JSON plus
raw logs. Each clone must have a physical `.git` directory equal to its common
Git directory, full history, no alternates, no linked-checkout registry, a clean
detached HEAD, and the canonical Jeryu origin. Sandbox roots live only beneath
`redline-split-ops/target/standalone-sandboxes`. Cleanup requires its private
marker, never follows symlink targets, holds the root and marker identities,
quarantines the root relative to its held parent, then traverses and removes
only through held directory descriptors. A late entry at the original sandbox
name is left untouched, and both held root and marker inodes must be unlinked.
Child commands use each repository's pinned toolchain, never the control
plane's `RUSTUP_TOOLCHAIN` override. Before Core CI, the runner builds the exact
reviewed Redline Testing release package locally, verifies its commit, manifest,
binary, and hashes, and records that binding under Core's dependency artifacts.

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
