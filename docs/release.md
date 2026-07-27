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

For a host-CI boundary change, source review must also prove
`ops/ci/host-ci-integrity-test.sh` and the privilege-separated
`ops/ci/split-host-ci-integrity-test.sh`. The latter exercises the real
parent/root-broker/worker boundary, caller-input rejection, immutable mounts,
namespace isolation, and one-shot publication. Installing reviewed broker
bytes is not part of a source PR; it occurs only after the protected merge by
the procedure in `ops/ci/HOST_CI_BOUNDARY.md`.

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
installed sandbox configuration protocol is v7 and run the boundary preflight.
A source or PR lane must not install or execute the unmerged publisher, migrate
the forge credential, or publish product checks.

After installation, monitor proof and required-check readbacks for every
request. A failed proof POST prevents required publication. A later
required/status failure consumes the request and leaves the PR blocked; the
request is never replayed.

## Rollback

Keep the previous protected control-plane tag and installed configuration as
the rollback target. On a boundary regression, stop new publication, verify the
last known-good immutable tag and receipts, restore through the reviewed
authority procedure, and rerun preflight before accepting requests. Source
corrections use a new protected PR and the next `-split.N+1` tag; rollback
never rewrites member repositories, moves an immutable tag, or relaxes branch
protection.
