# `bump-version --rewrite-split-tags` corrective-revision audit

- Audit time: `2026-07-16T05:56:33Z`
- Decision: **BLOCK** use of `splitctl bump-version --rewrite-split-tags` until the repair below is protected-landed and its focused tests pass.
- Scope: read-only source/object audit. No held source, forge ref, PR, tag, worktree, or release state was mutated.
- Pushed audit object: `claude/v8-release-fixes-20260714` at `6dcfe2a027621d742d96e7f7c9b80b63511e97bf`
- Protected-main audit object: `88a673c6c0e829721eebf8265036c1b18f2ef139`
- Held canonical checkout observed at the end of the audit: branch `claude/v8-release-fixes-20260714`, local `HEAD=85f0c16a0cd1d8c3af068a987285c95b20825364`, 45 porcelain rows. This local checkout moved during the lane and was deliberately not used as source authority.

## Finding

The pushed object and protected `main` contain byte-equivalent `bump_version` implementations after line-number/indent normalization (`sha256=52c097c0e497b5a05bb47941539db0e6daa3fc5f2d0ef84740686f6d0a1d8ba1`). The defect therefore exists on both audited source authorities.

At pushed-object lines 4457–4459, the implementation replaces the prefix
`v<from>-split.` with the complete tag `v<new>-split.0`. That does not preserve a
revision:

| Input | Current authority-manifest result | Required result |
|---|---|---|
| `v8.0.0-split.0` | `v8.0.1-split.00` | `v8.0.1-split.0` |
| `v8.0.0-split.1` | `v8.0.1-split.01` | `v8.0.1-split.1` |
| `v8.0.0-split.2` | `v8.0.1-split.02` | `v8.0.1-split.2` |

The same function then writes the single hard-coded `new_tag` to every repository
`VERSION` and changelog (lines 4465 and 4471–4475), collapsing all repositories to
`split.0`. `rewrite_cargo_tree` happens to preserve the numeric suffix because it
uses prefix-to-prefix replacement (lines 4521–4524). One invocation can therefore
create three conflicting identities: malformed authority tags (`.01`/`.02`),
collapsed repository metadata (`.0`), and correctly preserved Cargo tag pins
(`.1`/`.2`).

This is not theoretical on the pushed manifest. It contains 27 split-family
`product_version = "8.0.1"` entries and these corrective revisions:

- `jain-contracts`: `tag_revision=1`, `jain-contracts-v8.0.1-split.1`
- `jain-battle-gpu`: `tag_revision=1`, `jain-battle-gpu-v8.0.1-split.1`
- `jain-catboost`: `tag_revision=2`, `jain-catboost-v8.0.1-split.2`
- `jain-starforge`: `tag_revision=2`, `jain-starforge-v8.0.1-split.2`

The implementation also writes the authority manifest first and each checkout
afterwards with non-atomic `fs::write`. Any missing/unwritable path or later parse
error leaves a partially bumped family.

## Incomplete governed-surface coverage

The existing implementation changes only authority/derived-manifest tag substrings,
managed-repository `VERSION`, managed-repository changelog headers, and recursively
matched `Cargo.toml` text. It does not cover all release identity surfaces.

### Authority fields left stale or malformed

- Top-level `release_version` is not changed.
- Top-level `release_evidence_root` is not changed.
- `dependency_tag_suffix` is malformed from `...split.0` to `...split.00`.
- All 27 split-family `product_version` fields are left at the old version.
- `current_tag`/`immutable_tag` values are malformed as described above.
- `tag_revision` must be preserved exactly; it must never be reset during a product-version bump.
- `[control_plane]` is excluded by `manifest_repos`, so its checkout receives no `VERSION`, Cargo, standard-version, or changelog update.
- The manually string-rewritten portal/deploy manifest mirrors are generated files. They must instead be rendered from the updated authority with `sync-derived-manifests`.
- `family_source_version = "8.0.0"`, `family_source_tag_series = "v8.0.0-split.N"`, and the nested/external Redline `4.1.0-jain.3` identity are source provenance/dependency identities and must remain unchanged.

### Exact pushed Split Ops runtime/policy surfaces not governed by the command

The following authored, non-evidence surfaces contain the release identity at
`6dcfe2a0` but are not updated by the current command (Cargo manifests are reached
only if the control plane is added to the target set; locks are never regenerated):

- `Cargo.toml:3`, `Cargo.lock:89`
- `VERSION:1`
- `agent/standard-version.toml:1,3`
- `agent/boundaries.toml:15`
- `tools/splitctl/src/main.rs:13` (`RELEASE_VERSION`)
- `tools/splitctl/src/release_candidate.rs:13,548,554` (duplicate constant and hard-coded dependency suffix/message)
- `release-candidate.sh:2,6,12`
- `ops/ci/split-host-ci.sh:19,304`
- `ops/onboard.sh:10,103`
- `ops/split/cutover.sh:9,37,47`
- `ops/split/register-family.sh:9`
- `Justfile:109,125,128,131,135,138,159`
- `README.md:24,25,48` and the new changelog header (authored documentation surfaces)

`ops/ci/.claude-shci-snap.sh` also contains 8.0.1 literals in the held checkout,
but it is an obsolete mutable snapshot rather than an active source surface. Do
not expand bump coverage to it; retire or regenerate it through its owner lane.

### Exact managed-family classes missed

A read-only inventory of the 27 current canonical `HEAD` objects found:

- All 27 have a `VERSION` file. The command reaches 26 because the control plane is excluded.
- 25 have `agent/standard-version.toml`; `jain-agent` and `jain-jnoccio` currently do not. None of the 25 profiles are updated by the command.
- `PRODUCT_VERSION` exists in `jain-smartcluster`, `jain-docs`, and `jain-deploy`; none is updated. The current `jain-docs` object still says `8.0.0`, demonstrating the omission.
- Cargo lockfiles are never regenerated after recursive Cargo manifest edits.
- The recursive Cargo walk is also over-broad: it walks every non-vendor Cargo manifest, including tools, fuzz trees, fixtures, and `jain-model-zoo/reference/ported/**`, rather than the authority-declared governed Cargo members.
- Non-Cargo runtime/policy literals are ignored. Concrete current examples are:
  - `jain-smartcluster/PRODUCT_VERSION`, `agent/standard-version.toml`, `tools/release-control/src/main.rs:12-13`, and the version assertions in `crates/jain-smartcluster/src/client.rs`.
  - `jain/agent/standard-version.toml`, `ops/split/src/main.rs`, generated `family.lock`, and the portal derived `repos.manifest.toml`.
  - `jain-deploy/PRODUCT_VERSION`, `agent/standard-version.toml`, `deployment/landing/jain.sh`, `deployment/product/Cargo.toml`, `deployment/stage/Cargo.workspace.toml`, `scripts/artifact-ci.sh`, `scripts/atomicsoul-dry-run.sh`, `scripts/build-cloud-image.sh`, `scripts/release-atomicsoul.sh`, and their focused tests.
  - Artifact/release identity scripts in `jain-xgboost`, `jain-lightgbm`, `jain-llm`, `jain-model-zoo`, and `jain-ops`.
  - Receipt-root policy in `agent/boundaries.toml` where a repository declares a versioned evidence root.

The Web `apps/web/package.json`/`pnpm-lock.yaml` pair did not contain the 8.0.1
product identity in the audited current object. Do not blindly rewrite unrelated
package versions; only add a JavaScript surface if its owning standard explicitly
declares it as the Jain product version.

Generated `Cargo.lock`, `family.lock`, `jain-deploy/jain-split.lock.toml`, and
derived manifests must not be hand/string edited. Use their existing sanctioned
generators after authored surfaces are updated.

## Patch-ready minimal implementation

Keep the repair surgical and split it into one authored rewrite followed by existing
generators.

1. Add a pure `split_release_entries` helper that returns exactly `[control_plane]`,
   every `[[infrastructure_repo]]`, and every `[[repo]]`. Do not include tooling
   components, nested Redline, or external repositories.
2. Add a pure `expected_split_tag(entry, version)` helper. It must read and validate
   `name`, non-negative `tag_revision`, `product_version`, and the appropriate
   `current_tag`/`immutable_tag`, then return
   `<name>-v<new>-split.<unchanged tag_revision>`.
3. Replace the prefix bug with prefix-to-prefix rewriting only:
   `v<from>-split.` -> `v<new>-split.`. Never append `.0` to a prefix.
4. Update the authority fields semantically and fail closed on count/value mismatch:
   top-level `release_version`, `dependency_tag_suffix`, `release_evidence_root`,
   every split entry's `product_version`, and its exact tag field. Preserve every
   `tag_revision`. Explicitly assert that family-source and Redline identities did
   not change.
5. Include the control-plane checkout in the authored target set. For each target,
   plan exact updates for `VERSION`, the owning keys in
   `agent/standard-version.toml`, optional `PRODUCT_VERSION`, governed Cargo
   manifests, declared runtime/policy surfaces, and one changelog header using that
   repository's exact revision.
6. Replace unrestricted recursive Cargo traversal with the authority-declared root
   workspace and `cargo_members`/explicit release-tool manifests. Do not rewrite
   reference, fixture, vendor, evidence, bundle, cache, or snapshot content.
7. Build every before/after byte buffer and validate the complete plan before the
   first write. Reuse `write_atomic_bytes` for every authored file. If any expected
   surface is absent, duplicated, mismatched, or unwritable, write nothing and return
   a precise error.
8. Prefer deriving Rust constants from one source: make Split Ops
   `RELEASE_VERSION` derive from `env!("CARGO_PKG_VERSION")`, have
   `release_candidate.rs` import that constant, and format dependency suffixes from
   it. Keep evidence roots manifest-driven. This removes duplicated future bump
   surfaces instead of growing an open-ended replacement list.
9. After the authored rewrite, run the existing generators in their reviewed lanes:
   Cargo lock generation/update per affected workspace, `splitctl
   sync-derived-manifests --apply`, and `splitctl regenerate-lock --apply`. Read back
   byte identity and manifest/lock validation; do not make `bump-version` string-edit
   generated locks.

## Required focused regression tests

Use the existing `tests::TestDir`; no new dependency or worktree is needed.

1. `split_tag_prefix_rewrite_preserves_revision`: table cases for revisions 0, 1,
   and 2, asserting exact `.0`, `.1`, `.2` output and absence of `.00`, `.01`, `.02`.
2. `bump_plan_preserves_manifest_revisions`: temporary authority with control rev0,
   infrastructure rev0, repo rev1, and repo rev2. Assert updated
   `release_version`, dependency suffix, evidence root, all product versions/tags,
   and byte-identical `tag_revision` values.
3. `bump_plan_updates_control_plane_and_repo_surfaces`: assert exact `VERSION`,
   standard-version, optional `PRODUCT_VERSION`, governed Cargo manifest, runtime
   constant, and revision-specific changelog results.
4. `bump_plan_preserves_source_and_redline_identity`: assert byte identity for
   `family_source_version`, `family_source_tag_series`, and `redline-core-v4.1.0-jain.3`.
5. `bump_plan_is_fail_closed_before_writes`: make one governed surface inconsistent,
   expect an error, and compare hashes proving every planned file remained unchanged.
6. `bump_plan_rejects_tag_revision_mismatch`: a `.0` tag paired with
   `tag_revision=1` must fail instead of being normalized silently.
7. `bump_plan_excludes_unowned_cargo_trees`: place a version literal under a fixture
   or reference tree and prove it is unchanged.
8. `generated_outputs_validate_after_bump`: render derived manifests and the family
   lock through existing helpers, then require `validate-manifest --check-derived`
   and `validate-family-lock` semantics to pass with exact revisions.

Focused verification command after implementation:

```text
cargo test --locked -p jain-split-ops bump_version
```

Then run the normal exact-head Split Ops required lane. Do not run or rely on the
current mutating command as a test against the canonical family.

## Acceptance

This blocker is resolved only when the protected merged Split Ops object proves all
focused tests above, all four current corrective revisions remain `.1`/`.2`, no
`.00`/`.01`/`.02` tag exists in authored or derived output, generated locks/manifests
validate, and the exact merged head passes `jain-split-ops/required`. Until then,
`bump-version --rewrite-split-tags` is unsafe for a production authority update.
