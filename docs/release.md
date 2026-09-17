# Redline hub release process

This repository is the shell-and-documentation front door for the Redline
family. It does not build or publish the Redline engine. Engine releases belong
to `redline-core`; conformance evidence belongs to `redline-testing`; the
console belongs to `redline-web`; and family orchestration belongs to
`redline-split-ops`.

## Version source

[`VERSION`](../VERSION) is the hub's Redline compatibility track. It is not a
Cargo package version and does not authorize a tag by itself. The changelog
records hub-facing changes. The Jain release manifest and the generated,
proof-refreshed Redline lock are authoritative for the exact core tag and
commit consumed by Jain.

Tags are immutable. In particular, existing `redline-v4.1.0-jain.*` tags must
never be moved or recreated on a different commit. A new hub tag is permitted
only when a reviewed control-plane release record names it and the forge proves
that the tag is absent. The Jain 8.0.0 cutover names a new `redline-core` tag;
it does not authorize moving an existing hub tag.

## Candidate workflow

1. Start from a clean branch whose base is the current Jeryu `main` head. The
   checkout must have exactly one managed `origin` pointing to Jeryu.
2. Run `just check`, then the pinned Jankurai audit and security lane from a
   clean detached snapshot. The thin-hub guard must prove that no Cargo
   workspace or engine source has returned.
3. Run the Redline family CI through `redline-split-ops`. Its proof-refresh
   command must consume fresh core, testing, web, hub, and Jain-consumer
   receipts. Never edit `redline.lock.toml` by hand.
4. Open the Jeryu pull request as a draft, publish exact-head required checks,
   request approval only after every required check is green, and merge through
   the protected branch. Direct pushes to `main` are forbidden.
5. Refresh the clean local `main` and bare mirror, then verify forge-head
   equality, protection readback, and all immutable refs. Create only a tag
   explicitly named by the reviewed control plane, using compare-and-swap
   semantics that refuse an existing tag or a commit different from remote
   `main`.

The release remains a candidate until a separately authorized production
promotion. Hub validation never pushes images, changes public routing, or
updates a live alias.

## Evidence and integrity

The release control plane records machine-readable receipts for required CI,
Jankurai, security scans, mirror refresh, immutable refs, family lock metadata,
SHA-256 checksums, SBOM, provenance, signatures, and rollback validation.
Receipts must identify the exact commit and tool version that produced them.

For this shell/docs-only hub, Cargo audit and Cargo metadata are explicitly
`not_applicable` only when both `Cargo.toml` and `Cargo.lock` are absent. A
partial Cargo graph fails closed. Gitleaks remains a hard gate, and the general
SBOM and workflow-lint lanes still run.

## Rollback

Before merge, close the draft pull request and leave its head branch preserved.
After merge, repair or revert through a new reviewed pull request; never rewrite
`main`. If a consumer cutover fails, retain the immutable tags, restore the last
reviewed lock using the control-plane proof-refresh operation, rerun family CI,
and record the rollback receipt. Production routing is outside this workflow.

Prohibited operations include force-pushes, destructive resets, tag deletion or
movement, manual lock edits, direct GitHub release mutation, raw forge API
calls, and any production write without separate authorization.
