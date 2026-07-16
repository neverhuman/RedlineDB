# Control-plane release policy

`jain-split-ops` is the internal release control plane for the Jain split
family. Its current version comes from [`VERSION`](../VERSION), changes are
recorded in [`CHANGELOG.md`](../CHANGELOG.md), and repository/tag authority
comes only from [`repos.manifest.toml`](../repos.manifest.toml). Candidate
metadata remains `formal_ga = false`; this process does not authorize a
production promotion.

## Protected release process

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

## Integrity and proof evidence

The release record binds the full commit and tree, deterministic archive
SHA-256, manifest and policy digests, exact CI head, required-check readback, and
independent review. Any released artifact additionally requires checksums,
SBOM/provenance evidence, and a rollback target. Bare mirrors are disposable CI
caches; they are never release sources.

The protected v4 host boundary accepts only a clean exact-head product checkout
and the governed Jankurai binary named by its protected installation receipt.
It seals the auditor report and validated
`jain.jankurai-exact-sha-evidence/v1` receipt under root-owned storage.
`jankurai/proof` must be published and read back before
`<repo>/required` or its commit status. Repository, SHA, policy, auditor,
score, ratchet, conformance, clean-tree, run, attempt, seal, or readback
mismatches fail closed.

## Installation and monitoring

Publisher installation is a separate post-merge authority action. It must use
the clean protected-merged source and the procedure in
[`HOST_CI_BOUNDARY.md`](../ops/ci/HOST_CI_BOUNDARY.md), then verify the
installed configuration/result protocol is v4 and run the boundary preflight.
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
