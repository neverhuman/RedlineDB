# Jain Split Execution Plan

## Summary

Read-only preflight confirms the source repo is clean at `cc27936eb45006bda0cae85b0f578f4d5985991d`, Jeryu API is healthy, Docker/LFS/Rust/Node are present, and the six tracked Starforge LFS weights are smudged locally. The execution should follow `docs/split_plan_master.md`, with these required corrections before implementation:

- Skip `.jeryu/repo.toml` unless a live forge reader is added; current Jeryu uses `--split-manifest`, not repo-local TOML.
- Treat `jeryu/jain` as a real collision. Seed a portal preview slug until final cutover, then rename/archive the monorepo to `jeryu/jain-monorepo` and move the portal to `jeryu/jain`.
- Fix the Docker/stage-context Cargo claim: a verbatim root `Cargo.toml` is not enough after manifests are rewritten to git deps. Stage bundles must add stage-only top-level `[patch."<git-url>"]` entries mapping internal packages back to local paths.
- Reconcile stale model-zoo counts from source, not prose: current tree has 493 first-level `reference/ported` dirs and 655 tracked `Cargo.toml` files.
- Refresh credentials and runner deps before any forge/GitHub proof: local `gh` auth is invalid, GitHub auth is absent, and tools like `typst`, `taplo`, `patchelf`, `hadolint`, `cargo-public-api`, and `cargo-semver-checks` need installation or vendored bootstrap.

## Execution Stages

1. **Stage 0: Preflight And Evidence**
   - First mutating action after Plan Mode: write this plan to `/home/ubuntu/jain-split/docs/split_execution_plan_codex.md`.
   - Write `/home/ubuntu/jain-split/ops/split/preflight-report.md` with seed SHA, clean status, 4,388 tracked-file count, Jeryu health, slug collision list, LFS fsck, tool versions, disk, credentials, and missing runner tools.
   - Resolve blockers before Stage 1: re-auth local forge/GitHub CLI, decide GitHub LFS quota policy for `jain-starforge`, and record portal collision handling.
   - Do not modify `/home/ubuntu/jain_small`.

2. **Stage 1: Tooling Fork And Spikes**
   - Fork/adapt split tooling from `~/jeryu-split/ops/split`, `~/jeryu-split/ops/ci/split-host-ci.sh`, `~/veox-split/jeryu-ctl`, and `~/jankurai-split/jankurai/scripts`.
   - Add manifest-driven generation for all 19 repos, source coverage, reconcile, local bare mirrors, offline `insteadOf`, fleet CI, family doctor, and Jankurai gates.
   - Run the deploy anchor spike exactly once. If `cargo build -p feat-cli --bins` cannot build dependency bins from the deploy anchor, implement the fallback build through `--manifest-path ../jain-cli/Cargo.toml` with deploy-owned `--config` patch mappings.
   - Decide stage-context implementation now: use generated stage-only root `[patch]` entries matching the exact internal git URL form `http://127.0.0.1:8787/git/jeryu/<repo>.git`.

3. **Stage 2: Templates, Patches, And Interfaces**
   - Generate per-repo standard files: `AGENTS.md`, `SPLIT.md`, `README.md`, `Justfile`, `VERSION`, `Cargo.lock` where applicable, `scripts/ci-local.sh`, `ops/ci/required.sh`, `ops/ci/score.sh`, `agent/test-map.json`, `agent/owner-map.json`, `agent/audit-policy.toml`, and pinned `.github/workflows/ci.yml`.
   - Do not generate `.jeryu/repo.toml` by default.
   - Add exactly three split patch files: CatBoost, XGBoost, LightGBM `JAIN_VENDOR_ROOT` lookup before legacy `../../vendor/...`, with `cargo:rerun-if-env-changed=JAIN_VENDOR_ROOT`.
   - Standardize Cargo cross-repo deps as git tag deps only. Keep same-repo path deps such as `catboost -> catboost-sys`. `jain-deploy` is the only committed local-sibling `[patch]` exception.

4. **Stage 3: Manifest And Coverage**
   - Build `repos.manifest.toml` from the 19-repo topology in the master plan, but compute all file counts directly from `git ls-tree` at the seed commit.
   - Assign every tracked source file to exactly one repo or an explicit retired/generated bucket. Ignore local ignored files such as `apps/web/test-results`, `.pytest_cache`, and untracked artifact extras.
   - Verify the artifact set for `jain-starforge` is exactly the six tracked LFS safetensors plus required Starforge fixtures and lock/receipt data.
   - Run source coverage until missing and duplicate ownership are both zero.

5. **Stage 4: Materialize Repos**
   - Materialize 19 sibling repos under `/home/ubuntu/jain-split`, preserving monorepo-relative layout.
   - Rewrite manifests to pinned git tags shaped `<repo>-v7.0.1-split.0`; regenerate each Cargo repo's `Cargo.lock`.
   - Make `jain-core` canonical for `contracts/`; make `jain-contracts`, `jain-web`, `jain-python`, and deploy copies one-way mirrors with drift gates.
   - Smudge Starforge LFS files during materialization and assert each tracked `.safetensors` payload is larger than 1 MB.

6. **Stage 5: Offline Independence Gate**
   - Create local bare mirrors for all split repos and run in a scratch HOME with no network, using git `insteadOf` rewrites for the standardized GitHub URLs.
   - Run `bash scripts/ci-local.sh required` in all 19 repos; 16 repos are gating, and the 3 non-gating repos must still pass their smoke/audit lanes.
   - Run per-repo Jankurai score and diff lanes. Fail on hard findings, caps, missing score artifacts, or generated-zone drift.
   - Run dependency hygiene: no `branch =`, no illegal cross-repo `path = "../..."`, every Cargo repo has a lockfile, and deploy/stage patches match exact git source URLs.

7. **Stage 6: Local Forge Proof**
   - Seed non-colliding repos to `jeryu/<repo>`.
   - Seed the portal to a temporary preview slug, e.g. `jeryu/jain-portal-preview`, for CI/hook proof while existing `jeryu/jain` remains the monorepo.
   - Register family metadata through the proven API/manifest path, not `.jeryu/repo.toml`.
   - Run required checks and Jeryu status polling until local forge proof is green.

8. **Stage 7-10: Trial PRs, Release, Cutover, Closeout**
   - Open trial PRs covering contract sync, dependency bump, docs-only audit, native patch, web fixture drift, and deploy integration.
   - In portal, run fleet lanes: validate-family, contracts-sync, version consistency, coverage, and Jankurai fleet board.
   - In deploy, run integration lanes: vendor-all, stage-context, full binary, Docker image, SageMaker contract, SDK-vs-container, and native learners with Starforge weights.
   - Cutover only after green evidence: rename/archive existing local `jeryu/jain` to `jeryu/jain-monorepo`, move portal to `jeryu/jain`, add `--split-manifest /home/ubuntu/jain-split/repos.manifest.toml` to Jeryu serve if classification is desired, then mirror to GitHub according to credential/LFS decisions.

## Multi-Agent Work Allocation

- **Coordinator:** owns manifest decisions, merge order, final gates, and no-mutation protection for `/home/ubuntu/jain_small`.
- **Tooling worker:** materializer, source coverage, reconcile, local mirrors, `insteadOf`, fleet scripts.
- **Cargo/native worker:** dependency rewrites, lockfiles, native vendor patches, deploy anchor/fallback, Docker stage patching.
- **CI/Jankurai worker:** per-repo `ci-local`, `required`, `score`, security, generated-zone, and audit policy templates.
- **Product-lane worker:** web, Python, contracts, model-zoo, Starforge weights, support fixtures, and smoke data.
- **Forge/release worker:** local Jeryu onboarding, portal collision workflow, hooks, polling, GitHub mirror/LFS checks.

Workers must use disjoint write scopes, never edit `/home/ubuntu/jain_small`, and hand back changed paths plus proof logs.

## Test Plan

- Stage 0: preflight report proves seed SHA, clean source, LFS fsck, toolchain, credentials, and collisions.
- Stage 3: source coverage passes for all 4,388 tracked files.
- Stage 4: all 19 repos materialize, lockfiles regenerate, and Starforge LFS payload assertions pass.
- Stage 5: 19/19 offline lanes pass with no sibling repos and no network.
- Stage 5 Jankurai: every repo has clean score artifacts with no hard findings/caps.
- Stage 8 deploy proof: full binary builds, `jain --version`, `jain demo`, Docker/SageMaker contract, native learners, and Starforge weight parity pass.
- Stage 9: local Jeryu family is green after cutover, and GitHub mirrors are pushed only after credentials and LFS policy are confirmed.

## Assumptions

- Plan Mode prevented mutations in this pass; only read-only preflight and scout work was performed.
- `docs/split_plan_master.md` remains authoritative except for the corrections listed above.
- GitHub Starforge LFS upload is disabled until quota is explicitly confirmed.
- `jain-model-zoo` remains one repo; required lanes sample non-native/helper tests only unless a separate vendor-root strategy is added for native port crates.
- The monorepo remains read-only throughout split creation and is only archived/renamed at cutover.
