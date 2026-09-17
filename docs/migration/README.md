# Single-checkout migration

`neverhuman/RedlineDB` contains the active engine workspace at its root and five
component subtrees. Develop all components here. Nested Cargo workspaces retain
their lockfiles and are explicitly excluded from the engine workspace. Nested
GitHub workflows document the former component implementations; only root
workflows execute. The historical hub and consumer proof records are retained
as evidence, not as installed deployment authority.

`subrepos.toml` records full original commit/tree hashes and import commits.
`inventory.json` records repository copies, bundle digests, original refs,
unreferenced commits and their collision-safe `recovery/20260917/...` tags.
Recovery tags preserve original history after the migration PR is squash merged.
The eight commits ending at `5db4876df7ef9ba69eb2a0b1a8de16d99d06f2e5`
remain history only; the WAL/backup branch was not integrated.

The testing tree is imported at the requested `d25d6a9cfef3` commit. Later
canonical CI-only commits are preserved in recovery refs. Engine crate trees
were already identical to `d0de59930141` on GitHub before this migration.
The old `~/redlineDB` machine remnants are retained outside the published checkout.
Caches, operational databases, credentials and untracked local logs are not imported.
An unrelated incomplete Jankurai bundle discovered within a Redline recovery
folder is retained outside the checkout and is explicitly excluded from Redline refs.

Verify the checkout and, in a full clone with recovery tags, the history:

```bash
./subrepos/redline-split-ops/redlinectl validate
./subrepos/redline-split-ops/redlinectl validate --history
bash scripts/ci-family.sh all
bash ops/ci/audit-family.sh
bash ops/ci/security-family.sh
```

CI requires engine shards, all four declared conformance suites (including
`rql_phase1`), client, web, release-tool, integration, audit, security and all four
native packaging jobs. `RedlineDB/required` rejects failed, cancelled or skipped
required jobs. Compatibility baselines and minimum audit scores are retained.
Each component is audited with its own inherited policy; the engine audit excludes
those separately audited subtrees to avoid applying engine-specific rules to them.

The conformance runner is compiled from the same checkout as the engine and
its binary digest is checked by the existing evidence processor. Missing suites
or regressions fail the gate; the old optional-failure post-filter is not used.
Jankurai is fetched from GitHub and verified against pinned archive and binary
SHA-256 digests at `v1.6.11-deadlang-precision-split.3` (source
`b88562fdb124aa86dedd70ab972e7d0d87e58be1`).

Publishing `v4.1.0-rc.1` or `v4.1.0` runs the complete acceptance workflow before
uploading immutable archives. Stable tags must belong to the primary `main`
history. Existing releases and tags are never replaced. Cutover of old repositories
follows verified publication; installed Jain/Jeryu consumer locks and databases
are outside this migration and remain historical records here.

The GitHub audit baseline retains the historical engine and hub scores (85), caps, and findings. Only its policy fingerprint is qualified for the pinned Jankurai 1.6.11 release and the separately audited component layout. The original baseline and its SHA-256 remain recorded; future policy changes still fail the ratchet.

Security uses cargo-audit 0.22.1: 0.21.2 cannot parse the current RustSec CVSS 4 advisory records. Cargo-deny remains pinned at 0.19.8. The high-severity npm gate passes; the existing Vitest 3 development toolchain still reports two moderate advisories requiring a separate major-version upgrade.
