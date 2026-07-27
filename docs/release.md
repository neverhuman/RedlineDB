# Control-plane release policy

`jain-split-ops` is the internal release control plane for the Jain split
family. Its current version comes from [`VERSION`](../VERSION), changes are
recorded in [`CHANGELOG.md`](../CHANGELOG.md), and repository/tag authority
comes only from [`repos.manifest.toml`](../repos.manifest.toml). Candidate
metadata remains `formal_ga = false`; this process does not authorize a
production promotion.

## Reviewed lifecycle

1. Start from canonical protected `main`, preserve linear history, and commit
   only the reviewed control-plane delta.
2. Run `just fast`, `just required`, `just security`, `just score`,
   `just contract-drift`, `just artifact-support`, and
   `just tool-adoption` on the exact head.
3. Open one local-Jeryu PR. The exact head must receive successful
   `jankurai/proof` and `jain-split-ops/required` results plus one independent
   approval; a stale result from an earlier head is not reusable.
4. Merge only through the protected fast-forward lifecycle. Read back canonical
   `main`, the merge SHA, and the clean checkout before any installation or
   tag action.
5. Create the next unused immutable `jain-split-ops-v*-split.N` tag at that
   merged SHA. Never move, delete, or reuse a tag.

Provider repositories land and receive immutable tags before downstream Cargo
Git pins and locks are refreshed. The complete family order and receipt
locations are defined in [`release-runbook.md`](release-runbook.md).

## Authority binding

Tagging a member repository does **not** bind it. `splitctl release-candidate`
journals lifecycle transitions only; it never writes this manifest. A member's
release identity enters authority through a reviewed control-plane change to
[`repos.manifest.toml`](../repos.manifest.toml), which is the change class this
document governs.

A row may only be bound when the member repository is genuinely at its release
commit. Verify all three before writing a row:

1. the member's `HEAD` is byte-identical to its **authenticated forge `main`** —
   a local `main` or `origin/main` ref may be stale and is not authority;
2. the intended next-unused immutable release tag resolves to that same commit,
   confirmed by authenticated forge readback rather than local tag state;
3. the checkout is clean, so the tree checksum describes a fixed object.

Write the complete identity, not a subset: `product_version`, `tag_revision`,
`immutable_tag` and `current_tag` (identical), `release_commit`, `release_tree`,
`release_checksum_sha256`, `identity_status = "bound"`, and `onboarded = true`.
`release_tree` is `HEAD^{tree}`, and `release_checksum_sha256` is the SHA-256 of
`git archive --format=tar <commit>` — the same command `splitctl` runs
internally — so the exactness check compares like for like rather than against a
transcribed value.

Binding is the last step for a member and is deliberately separate from tagging,
so an immutable tag can exist while authority still reads `pending`.

**Ordering constraint.** Every write to this manifest changes its SHA-256, and a
nested family's CI receipt binds that exact digest. Manifest writes must
therefore all land *before* a nested family's evidence window is opened;
regenerating that evidence first and binding afterwards invalidates it and forces
the window to be reopened.

For a host-CI boundary change, source review must also prove
`ops/ci/host-ci-integrity-test.sh` and the privilege-separated
`ops/ci/split-host-ci-integrity-test.sh`. The latter exercises the real
parent/root-broker/worker boundary, caller-input rejection, immutable mounts,
namespace isolation, and one-shot publication. Outside the cycle-breaking
self-check exception documented below, installing reviewed broker bytes is not
part of a source PR; ordinary installation occurs only after the protected
merge by the procedure in `ops/ci/HOST_CI_BOUNDARY.md`.

## Evidence

The release record binds the full commit and tree, deterministic archive
SHA-256, manifest and policy digests, exact CI head, required-check readback, and
independent review. Any released artifact additionally requires checksums,
SBOM/provenance evidence, and a rollback target. Bare mirrors are disposable CI
caches; they are never release sources.

Appliance promotion evidence is fail-closed. `release-status` requires a
physical, single-link, non-writable `jain.local-appliance-canary-matrix/v1`
aggregate and a root-owned sealed `jain.local-appliance-canary-verifier/v1`
receipt for the exact release. The receipt binds the aggregate, attestation,
signature, public key, reviewed verifier, and immutable Deploy tag/commit; the
tag is read back through the authenticated local forge transport. Both files
are revalidated after bounded reads. The aggregate must bind distinct qualified
CPU and GPU receipts, the same signed release job, manifest, artifact set, and
OCI index/platform identities. Fixture, unqualified, unknown-field,
secret-like, mutable, linked, mismatched, credential-bearing, loopback, or
unsealed evidence is rejected. Passing this evidence gate does not authorize
publication, routing, promotion, or activation; candidate metadata remains
`formal_ga = false` until a separate owner action.

The protected host boundary accepts only a clean exact-head product checkout
and the governed Jankurai binary named by its protected installation receipt.
It seals the auditor report and validated
`jain.jankurai-exact-sha-evidence/v1` receipt under root-owned storage.
`jankurai/proof` must be published and read back before
`<repo>/required`; the exact required check must be read back before the commit
status, and that status must also be read back. Repository, SHA, policy, auditor,
score, ratchet, conformance, clean-tree, run, attempt, seal, or readback
mismatches fail closed.

The root sandbox derives `jain.release-authority-projection/v1` from the exact
authenticated control-plane commit and its committed `repos.manifest.toml`.
The closed projection binds the control commit, manifest SHA-256, Jain family,
release `10.0.0`, `status=candidate`, `formal_ga=false`, and rollback `8.0.1`.
It is exposed only as the physical, root-owned, mode-`0444`, single-link
`/opt/jain-ci/authority/release-authority.json`; the worker receives that path
and its SHA-256 from root-owned state. Caller-supplied path or checksum values,
wrong or extra fields, a different manifest digest, mutable custody, and any
GA/status/rollback change fail before product artifact evidence can pass.
Product receipts bind the accepted projection digest; they do not turn
candidate metadata into production authorization.

Committed lifecycle evidence belongs under
`docs/release-evidence/10.0.0/`. Ephemeral exact-head audit, sandbox, repair,
and artifact outputs stay under `target/`; forge readback receipts identify the
repository, full commit, check, attempt, and immutable tag rather than relying
on a branch name or prose status.

## Installation and monitoring

Publisher installation is a separate post-merge authority action. It must use
the clean protected-merged source and the procedure in
[`HOST_CI_BOUNDARY.md`](../ops/ci/HOST_CI_BOUNDARY.md), then verify the
installed sandbox configuration protocol is v8 and run the boundary preflight.
The sole pre-merge exception is the documented control-plane self-check:
after independent exact-head review, a distinct authority owner may install
only the reviewed sandbox and bind its published ref, exact commit, and an
expiry no more than two hours ahead in both root configs. The publisher must
remain byte-identical to protected `main`; no unmerged publisher bytes may be
installed or executed. Failure restores ordinary-main authority immediately;
a protected fast-forward is followed by an ordinary-main reinstall, preflight,
and live readback with all bootstrap fields removed. The source author and
unprivileged PR worker must not migrate the forge credential or publish product
checks.

After installation, monitor proof and required-check readbacks for every
request. A failed proof POST prevents required publication. A later
required/status failure consumes the request and leaves the PR blocked; the
request is never replayed.

### Redline Web npm cache authority

`ops/ci/npm-cache.lock.json` binds the exact Redline Web package-lock digest,
Linux/x64/glibc package-path/URL/integrity closure, canonical CACache
inventory, and npm version. Build it only with
`ops/ci/npm-cache-authority.sh` from an ordinary source cache into a new staging
directory. The builder reads content-addressed blobs, verifies every SHA-512,
and writes deterministic index records; the source cache itself is not copied
or trusted as an index.

Before merge, retain the generated directory named by `inventory_sha256` and
prove `npm-runtime-test.sh`, both host-boundary tests, full Jankurai, and true
changed-fast Jankurai. After protected merge, copy that exact directory to
`/var/lib/jain-host-ci/npm-cache/<inventory_sha256>`, set root ownership,
directories to `0555`, and files to `0444`, then validate it against the
merged authority and exact Redline lock. Reinstall the ordinary merged-main
broker and run the boundary preflight before accepting a request. A missing,
mutable, mismatched, or incomplete cache blocks only the affected request; it
never permits network fallback. Rollback restores the previous merged broker
and leaves the new digest-addressed cache inert until separately removed.

## Rollback

Keep the previous protected control-plane tag and installed configuration as
the rollback target. On a boundary regression, stop new publication, verify the
last known-good immutable tag and receipts, restore through the reviewed
authority procedure, and rerun preflight before accepting requests. Source
corrections use a new protected PR and the next `-split.N+1` tag; rollback
never rewrites member repositories, moves an immutable tag, or relaxes branch
protection.
