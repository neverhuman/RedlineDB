# Jain Split Plan (Codex)

Date: 2026-07-06

Source repository: `/home/ubuntu/jain_small`

Seed commit: `cc27936eb45006bda0cae85b0f578f4d5985991d`

Source branch: `apex`

Target split root: `/home/ubuntu/jain-split`

Local authoritative owner: `jeryu`

GitHub mirror owner: `neverhuman`

Default branch for every split repo: `main`

Release tag shape: `<repo>-v7.0.1-split.0`

## Executive Summary

The split should use the 19-repo topology from the original user intent, not the 16-repo alternative. The 19-repo topology gives cleaner ownership for public contracts, domain types, artifacts, CLI, deployment, and release operations while still keeping every repo independently CI-testable.

The implementation should, however, borrow several operational ideas from Claude's plan and the existing `~/jeryu-split` precedent:

- Use a manifest-driven materializer instead of a one-off extraction script.
- Preserve monorepo-relative paths inside split repos where existing tests rely on them.
- Add a source coverage gate so no tracked source file is silently dropped.
- Convert all cross-repo Rust dependencies to pinned public Git tags.
- Keep local sibling development in generated workspaces under `.fusion/` or `target/split-source/`, not committed path dependencies.
- Create and register Jeryu forge repos through the live forge API instead of relying on `jeryu onboard`, which is dry-run only in this environment.
- Add explicit LFS smudge and artifact-size/hash validation.
- Add `JAIN_VENDOR_ROOT` and `JAIN_ARTIFACT_ROOT` support inside the split repos.

The source monorepo must not be mutated. Any split-normalization code changes, such as `JAIN_VENDOR_ROOT`, land only in the generated split repos.

## Non-Negotiable Invariants

1. The extraction seed is exactly `cc27936eb45006bda0cae85b0f578f4d5985991d`.
2. `/home/ubuntu/jain_small` is read-only for this split task.
3. Jeryu slugs are exactly `jeryu/<repo>`.
4. GitHub mirror slugs are exactly `neverhuman/<repo>`.
5. Every split repo default branch is `main`.
6. Every split repo gets tag `<repo>-v7.0.1-split.0`.
7. No committed Cargo dependency may use `branch =`.
8. No committed cross-repo Cargo dependency may use a sibling checkout path.
9. Every Cargo split repo has a root `Cargo.lock`.
10. Each repo must contain `AGENTS.md`, `SPLIT.md`, `.jeryu/repo.toml`, `agent/split-member.toml`, `agent/owner-map.json`, `agent/test-map.json`, `agent/generated-zones.toml`, `agent/standard-version.toml`, `scripts/ci-local.sh`, `ops/ci/required.sh`, and a thin pinned `.github/workflows/ci.yml`.
11. Generated evidence stays out of source edits: `target/`, `.jankurai/`, `.fusion/`, `target/split-source/`, `vendor/`, and transient model outputs.

## Topology

The v1 split should contain 19 repos:

| Repo | Role | Source ownership | Required check |
| --- | --- | --- | --- |
| `jain` | Portal | `repos.manifest.toml`, `family.lock`, validation/fusion scripts, split docs | `bash scripts/validate-family.sh` |
| `jain-docs` | Docs and agent metadata | `docs/**`, `agent/**`, `db/**`, global runbooks | JSON/TOML parse plus optional Jankurai diff audit |
| `jain-contracts` | Public contracts | `contracts/**` | schema and fixture parse/validation |
| `jain-domain` | Domain/error contracts | `crates/domain` | `cargo test --workspace --locked` |
| `jain-math` | Feature math | `crates/feat-math` | fmt, clippy, tests |
| `jain-catboost` | CatBoost native binding | `crates/catboost-sys`, `crates/catboost`, `tests/cbtest.c` | vendor-aware native lane |
| `jain-xgboost` | XGBoost native binding | `crates/xgboost` | vendor-aware native lane |
| `jain-lightgbm` | LightGBM native binding | `crates/lightgbm` | vendor-aware native lane |
| `jain-starforge` | Starforge runtime code | `crates/starforge` | `cargo test --workspace --all-targets` |
| `jain-artifacts` | Model artifacts | `artifacts/starforge`, `artifacts/foundation`, artifact locks/receipts, compression/prep scripts | LFS present plus lock SHA/size checks |
| `jain-battle-gpu` | GPU search facade | `crates/battle-gpu` | CPU-safe clippy/tests and default GPU feature smoke |
| `jain-core` | Algorithmic truth | `crates/feat-core`, core Apex tests/scripts, controlled contract copies | no-native fmt/clippy/tests, `artifact`, `progress_contract` |
| `jain-report` | Reports | `crates/feat-report` | `cargo test --workspace --locked` |
| `jain-tui` | TUI | `crates/feat-tui` | `cargo test --workspace --locked` |
| `jain-cli` | `jain` binary | `crates/feat-cli` | `cargo check --no-default-features`, ci-smoke tests |
| `jain-web` | Web backend and UI | `crates/feat-web`, `apps/web`, controlled contract copies | feat-web ci-smoke plus pnpm typecheck/test/build/e2e |
| `jain-model-zoo` | Reference models | `reference/ported/**`, `ops/parity/**` | structural smoke plus sampled port tests |
| `jain-deploy` | Deployment and source materialization | `deployment/ops/**`, `python/ai-service/**`, SageMaker examples, final assembly scripts | `sagemaker-ci`, Python tests, materialize-source smoke |
| `jain-release-ops` | Release and CI operations | reusable CI/security/release templates, root `scripts/**`, `tools/security-lane.sh`, mirror/onboard helpers | shell lint, security lane, template render checks |

Rollout waves:

- Wave 0: `jain`, `jain-docs`, `jain-contracts`, `jain-domain`, `jain-math`.
- Wave 1: `jain-catboost`, `jain-xgboost`, `jain-lightgbm`, `jain-starforge`, `jain-artifacts`.
- Wave 2: `jain-core`, `jain-report`, `jain-tui`, `jain-battle-gpu`.
- Wave 3: `jain-cli`, `jain-web`.
- Wave 4: `jain-model-zoo`, `jain-deploy`, `jain-release-ops`.

## Why Keep 19 Repos Instead Of Claude's 16

Claude's plan is stronger operationally but less faithful to the requested ownership model.

Keep `jain-contracts` standalone. The user explicitly asked for a public contract source. Contract copies in `jain-core`, `jain-web`, and `jain-deploy` are test fixtures, not the source of truth.

Keep `jain-domain` standalone. It is a small crate, but it is a stable domain/error surface and can be independently tested without native learners or the core pipeline.

Keep `jain-artifacts` standalone. Moving weights into `jain-starforge` couples source code churn to LFS-heavy release assets. A separate artifact repo is cleaner for storage, release locks, and deploy materialization.

Keep `jain-cli` standalone. The CLI is the runtime product surface. Deployment should consume the CLI; deployment should not own it.

Keep `jain-release-ops` standalone. Release/security/workflow automation changes at a different cadence from deployment packaging and should not be hidden inside `jain-deploy`.

Do not create `jain-python` as a separate v1 repo unless the user changes the topology. The requested plan places `python/ai-service` in `jain-deploy`, which is reasonable because the SDK tests and SageMaker contract are deployment-facing.

Do not create `jain-ops` as a separate v1 repo. Apex/control-plane operational material is already distributed between `jain-core`, `jain-model-zoo`, `jain-deploy`, and `jain-release-ops`.

## Precedent Tooling To Reuse

Use `~/jeryu-split/ops/split` and `~/jeryu-split/ops/ci/split-host-ci.sh` as the base, not a custom one-off extraction script.

Files to fork or adapt into `/home/ubuntu/jain-split/ops/split`:

- `materialize.py`
- `manifest.sh`
- `source_coverage.py`
- `reconcile.py`
- `register-family.sh`
- `rollout-pr-flow.sh`
- `commit-baseline.sh`
- `cutover.sh`
- `closeout-prs.sh`
- `bump-family-version.py`

Files to adapt from `~/veox-split/jeryu-ctl`:

- `onboard.sh`
- `onboard-repo.sh`
- `host-ci.sh`
- `jeryu-poll.sh`
- `hooks/pre-receive`

The adapted tooling should be manifest-driven. Repo definitions should live in `repos.manifest.toml`; scripts should not hard-code repo lists except where bootstrapping the manifest itself.

## Manifest Shape

The root manifest should live at:

- `/home/ubuntu/jain-split/repos.manifest.toml`
- `/home/ubuntu/jain-split/jain/repos.manifest.toml`

Use a Jeryu-compatible shape matching the precedent families, plus optional fields for gates and remotes.

Required top-level fields:

```toml
schema_version = "1.2.0"
workers = 40
source_root = "/home/ubuntu/jain_small"
source_branch = "apex"
source_commit = "cc27936eb45006bda0cae85b0f578f4d5985991d"
split_root = "/home/ubuntu/jain-split"
family = "jain"
repo_family = "jain-split"
umbrella_repo = "jain"
default_branch = "main"
release = "7.0.1-split.0"
local_owner = "jeryu"
public_owner = "neverhuman"
rollout_wave_order = [0, 1, 2, 3, 4]
required_repos = [ ... all 19 repo names ... ]
```

Required per-repo fields:

```toml
[[repo]]
path = "/home/ubuntu/jain-split/jain-core"
name = "jain-core"
slug = "jain-core"
github_slug = "neverhuman/jain-core"
jeryu_slug = "jeryu/jain-core"
role = "core"
profile = "rust-workspace"
branch = "main"
default_branch = "main"
rollout_wave = 2
has_jeryu_std = true
onboarded = true
mirror_github_main = true
current_tag = "jain-core-v7.0.1-split.0"
required_check = "jain-core/required"
copy_paths = ["crates/feat-core", "contracts", "ops/apex"]
cargo_members = ["crates/feat-core"]
source_paths = ["crates/feat-core/**", "contracts/**", "ops/apex/**"]
cross_repo_deps = ["jain-domain", "jain-math", "jain-catboost", "jain-xgboost", "jain-lightgbm", "jain-starforge"]
```

Use `source_paths` for coverage accounting and `copy_paths` for materialization. They are related but not interchangeable.

## Materialization Rules

The materializer must:

1. Assert the source repository contains `cc27936eb45006bda0cae85b0f578f4d5985991d`.
2. Extract from `git archive` at that commit.
3. Preserve monorepo-relative layout inside each split repo.
4. Copy only declared `copy_paths`.
5. Generate a root `Cargo.toml` for each Rust repo with only that repo's `cargo_members`.
6. Copy the source `Cargo.lock` into every Rust split repo, then update it only if required by dependency rewriting.
7. Rewrite cross-repo path dependencies to pinned Git tags.
8. Preserve dependency attributes such as `optional`, `default-features`, and `features`.
9. Add split metadata and CI scaffolding.
10. Initialize each repo with `git init -b main`.
11. Commit with a deterministic message.
12. Create annotated tag `<repo>-v7.0.1-split.0`.
13. Add remotes:
    - `origin = http://127.0.0.1:8787/git/jeryu/<repo>.git`
    - `github = git@github.com:neverhuman/<repo>.git`

The materializer should not run `cargo update` casually. If lockfile normalization is needed, run it per repo and record the reason.

## Cargo Dependency Policy

Inside a repo, path dependencies may remain path dependencies when both crates are in that same repo. Example: `jain-catboost` may keep `catboost = { path = "../catboost-sys" }` because both crates are in `jain-catboost`.

Across repos, path dependencies must become pinned Git-tag dependencies:

```toml
feat-math = { git = "https://github.com/neverhuman/jain-math.git", tag = "jain-math-v7.0.1-split.0" }
```

For optional dependencies, preserve the original attributes:

```toml
catboost = { git = "https://github.com/neverhuman/jain-catboost.git", tag = "jain-catboost-v7.0.1-split.0", optional = true }
```

For dependencies with disabled defaults, preserve them:

```toml
battle-gpu = { git = "https://github.com/neverhuman/jain-battle-gpu.git", tag = "jain-battle-gpu-v7.0.1-split.0", default-features = false }
```

Forbidden:

```toml
feat-core = { path = "../jain-core/crates/feat-core" }
feat-core = { git = "...", branch = "main" }
```

Local sibling development should be generated into `.fusion/` or `target/split-source/`. The committed manifests must remain tag-pinned.

## Native Learner Handling

Native crates currently expect vendored upstream source under `../../vendor/<learner>`. That breaks when they are consumed as Cargo Git dependencies because Cargo checks them out under `~/.cargo/git/checkouts`.

In the generated split repos only, patch:

- `crates/catboost-sys/build.rs`
- `crates/xgboost/build.rs`
- `crates/lightgbm/build.rs`

Add lookup order:

1. `JAIN_VENDOR_ROOT/<learner>` when `JAIN_VENDOR_ROOT` is set.
2. Legacy `../../vendor/<learner>` for monorepo-shaped source bundles.

Also add `cargo:rerun-if-env-changed=JAIN_VENDOR_ROOT`.

Each native repo should include a vendor helper:

- `jain-catboost/scripts/vendor.sh`
- `jain-xgboost/scripts/vendor.sh`
- `jain-lightgbm/scripts/vendor.sh`

`jain-deploy` should include `scripts/vendor-all.sh`, which fetches all three upstream sources into `jain-deploy/vendor` and exports `JAIN_VENDOR_ROOT=$PWD/vendor` for release builds.

Required CI for native repos should be honest. If it runs Cargo commands that execute `build.rs`, it must fetch vendor first. A vendor-free "cargo clippy" lane is unsafe because build scripts still run for crates with `build.rs`.

Practical required native lane:

```bash
bash scripts/vendor.sh
bash scripts/prune_vendor.sh
cargo test --workspace --release
```

If that is too heavy for every PR, split it explicitly:

- `required`: manifest/syntax/source checks that do not execute Cargo build scripts.
- `native`: dispatch or scheduled release proof that fetches vendor and runs release tests.

Do not pretend a vendor-free Cargo check proves native build health.

## Artifact Handling

`jain-artifacts` owns:

- `artifacts/starforge/**`
- `artifacts/foundation/**`
- `deployment/ops/starforge-artifacts.lock`
- `deployment/ops/starforge-artifacts.receipt.json`
- `scripts/compress-starforge-weights.sh`
- `scripts/prepare-starforge-artifacts.sh`
- `.gitattributes`

Materialization must preserve Git LFS behavior:

1. Detect LFS pointer files after extraction.
2. Smudge from the source repository when real artifact contents are required.
3. Seed `jain-artifacts` with `.gitattributes` before adding artifacts.
4. Verify every required `.safetensors` or `.safetensors.zst` file is either a valid LFS pointer in Git or a real file above the expected minimum size.
5. Verify artifact lock SHA and size metadata against the checked-out contents.

Runtime lookup in `jain-core` and `jain-starforge` should gain `JAIN_ARTIFACT_ROOT`.

Lookup order for weights:

1. Explicit CLI/config path.
2. `JAIN_ARTIFACT_ROOT/starforge/...` or `JAIN_ARTIFACT_ROOT/foundation/...`.
3. Runtime image path under `/opt/jain/starforge`.
4. Repo-relative legacy paths such as `artifacts/starforge/...`.

Docker remains free to install weights under `/opt/jain/starforge`.

## Contract Handling

`jain-contracts` is the public contract source.

`jain-core`, `jain-web`, and `jain-deploy` may carry controlled fixture copies for self-tests. These copies must be marked in `agent/generated-zones.toml`:

```toml
[[zone]]
path = "contracts/"
source = "jain-contracts"
command = "bash ops/ci/check-contract-drift.sh"
read_only = true
write_policy = "synced_from_jain-contracts"
```

Add a drift check that compares:

- `contracts/progress-event.schema.json`
- `contracts/progress-events.jsonl`
- `contracts/public-api.toml`

against the pinned `jain-contracts` tag in the consuming repo.

Bump flow:

1. Update `jain-contracts`.
2. Tag `jain-contracts-v7.0.1-split.N`.
3. Sync fixture copies in consumers.
4. Bump consumer pins.
5. Bump `family.lock`.

For v1, because all repos start from `7.0.1-split.0`, the initial copies can be byte-identical to source commit `cc27936`.

## Full Binary Build Story

`jain-deploy` owns full source materialization, not the CLI source itself.

`jain-cli` owns:

- `crates/feat-cli`
- `jain`
- `jain-entrypoint`

`jain-deploy` owns scripts that assemble a monorepo-shaped build context:

```text
target/split-source/jain-7.0.1-split.0/
  Cargo.toml
  Cargo.lock
  rust-toolchain.toml
  crates/domain
  crates/feat-math
  crates/catboost-sys
  crates/catboost
  crates/xgboost
  crates/lightgbm
  crates/starforge
  crates/battle-gpu
  crates/feat-core
  crates/feat-report
  crates/feat-tui
  crates/feat-cli
  crates/feat-web
  apps/web
  contracts
  artifacts/starforge
  artifacts/foundation
  deployment/ops
  python/ai-service
  vendor
```

The materialized source bundle may use local path dependencies because it is generated and ignored. The committed split repos must not.

Release build sequence:

```bash
cd /home/ubuntu/jain-split/jain-deploy
bash scripts/materialize-source.sh
bash scripts/vendor-all.sh target/split-source/jain-7.0.1-split.0/vendor
cd target/split-source/jain-7.0.1-split.0
JAIN_VENDOR_ROOT=$PWD/vendor cargo build --release -p feat-cli
./target/release/jain --version
./target/release/jain demo --out-dir target/demo
cargo test -p sagemaker-ci
```

Docker build uses the same generated bundle, not a hand-maintained committed monorepo copy.

## Jeryu Forge Onboarding

The local Jeryu API is available at:

```text
http://127.0.0.1:8787
```

`jeryu onboard` is dry-run only, so real onboarding should use forge operations:

1. Create missing repo records under owner `jeryu`.
2. Push each local `main` branch and split tag to `origin`.
3. Register family metadata as `jain-split`.
4. Configure required check name `<repo>/required`.
5. Configure GitHub mirror remote as `neverhuman/<repo>`.
6. Mirror only after required checks pass.

Before creating repos, list existing records:

```bash
jeryu forge repo list --owner jeryu --json --api-url http://127.0.0.1:8787
```

If a slug already exists, do not destructively overwrite it. Record the collision and either reuse it only when it is the intended target or stop for explicit user approval.

Expected local remotes in each split checkout:

```bash
origin  http://127.0.0.1:8787/git/jeryu/<repo>.git
github  git@github.com:neverhuman/<repo>.git
```

Expected `.jeryu/repo.toml`:

```toml
repo = "jeryu/<repo>"
default_branch = "main"

[shadow_main]
enabled = true
remote_url = "git@github.com:neverhuman/<repo>.git"
refs = ["refs/heads/main"]

[tag_mirror]
enabled = true
remote_url = "git@github.com:neverhuman/<repo>.git"
tag_pattern = "<repo>-v*"
```

## Portal Validation

`jain/scripts/validate-family.sh` should check:

- Manifest parses as TOML.
- `required_repos` equals the 19 expected repos.
- Every repo directory exists.
- Every repo has required metadata files.
- JSON metadata parses.
- `.jeryu/repo.toml` slug matches `jeryu/<repo>`.
- GitHub mirror URL matches `neverhuman/<repo>`.
- No committed `branch =` dependencies in Cargo or package manifests.
- No committed cross-repo Cargo path dependencies.
- Every root Cargo repo has `Cargo.lock`.
- Node repos have `pnpm-lock.yaml` or `package-lock.json`.
- Workflow actions are pinned to full 40-character SHAs.
- `family.lock` tags and commit SHAs match the local checkouts.
- Source coverage is complete.

Minimum command:

```bash
bash /home/ubuntu/jain-split/jain/scripts/validate-family.sh
```

## Family Lock

`jain/family.lock` should be generated after initial commits and tags.

Suggested shape:

```toml
schema_version = "1.0.0"
family = "jain"
release = "7.0.1-split.0"
source = "repos.manifest.toml"
generated_at = "2026-07-06T00:00:00Z"
source_repo = "jeryu/jain_small"
source_branch = "apex"
source_commit = "cc27936eb45006bda0cae85b0f578f4d5985991d"

[[repo]]
repo = "jain-core"
tag = "jain-core-v7.0.1-split.0"
commit = "<sha>"
github = "https://github.com/neverhuman/jain-core.git"
jeryu = "http://127.0.0.1:8787/git/jeryu/jain-core.git"
required_check = "jain-core/required"
```

Do not manually edit commits in the lock. Regenerate from the actual split checkout heads.

## Per-Repo Required CI

Use `scripts/ci-local.sh required` as the common entry point. It delegates to `ops/ci/required.sh`.

Recommended v1 lanes:

| Repo | Required lane |
| --- | --- |
| `jain` | `bash scripts/validate-family.sh` |
| `jain-docs` | parse JSON/TOML; optional Jankurai diff audit |
| `jain-contracts` | parse schema, parse fixtures, validate known version |
| `jain-domain` | `cargo test --workspace --locked` |
| `jain-math` | `cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets -- -D warnings`; `cargo test --workspace --locked --all-targets` |
| `jain-catboost` | vendor fetch/prune plus `cargo test --workspace --release`, or non-Cargo structural required plus dispatch native proof |
| `jain-xgboost` | vendor fetch/prune plus `cargo test --workspace --release`, or non-Cargo structural required plus dispatch native proof |
| `jain-lightgbm` | vendor fetch/prune plus `cargo test --workspace --release`, or non-Cargo structural required plus dispatch native proof |
| `jain-starforge` | `cargo test --workspace --locked --all-targets`; real-weight smoke optional |
| `jain-artifacts` | LFS/artifact lock/receipt validation |
| `jain-battle-gpu` | `cargo clippy --workspace --all-targets -- -D warnings`; `cargo test --workspace --locked --all-targets`; `cargo check --workspace --no-default-features` |
| `jain-core` | no-native fmt/clippy/tests, `artifact`, `model_artifacts`, `progress_contract`; no native or CUDA defaults |
| `jain-report` | `cargo test --workspace --locked` |
| `jain-tui` | `cargo test --workspace --locked` |
| `jain-cli` | `cargo check -p feat-cli --no-default-features`; `cargo test -p feat-cli --no-default-features --features ci-smoke --all-targets` |
| `jain-web` | `cargo test -p feat-web --no-default-features --features ci-smoke`; `pnpm --dir apps/web typecheck`; `pnpm --dir apps/web test`; `pnpm --dir apps/web build`; Playwright where available |
| `jain-model-zoo` | parse all `reference/ported/*/Cargo.toml`; sampled port tests; full parity manual |
| `jain-deploy` | `cargo test -p sagemaker-ci`; Python tests; materialize-source smoke |
| `jain-release-ops` | shell syntax/lint, workflow pin checks, security lane/template render checks |

## Source Coverage

Add a source coverage report before calling the split complete.

Inputs:

- `git ls-tree -r --name-only cc27936eb45006bda0cae85b0f578f4d5985991d`
- `repos.manifest.toml` `source_paths`
- `shared_source_paths`
- explicit ignored generated paths

Fail if any tracked source file is:

- not assigned to a repo,
- assigned to multiple repos without an explicit reason,
- assigned only through a broad catch-all that hides ownership.

Acceptable duplicate copies:

- Contract fixture copies in `jain-core`, `jain-web`, and `jain-deploy`.
- Shared root metadata such as `rust-toolchain.toml`, `.gitignore`, `.gitattributes`, and CI helper snippets where needed.
- `scripts/prune_vendor.sh` or per-native extracts where native repos need vendor pruning.

The coverage report should be committed to the portal or written under `target/` and referenced from validation output.

## Implementation Phases

### Phase 0: Preflight

1. Confirm source repo status and HEAD:

```bash
git -C /home/ubuntu/jain_small rev-parse HEAD
git -C /home/ubuntu/jain_small rev-parse --abbrev-ref HEAD
git -C /home/ubuntu/jain_small status --short --branch
```

2. Confirm target contains no valuable repo state before generation.
3. Confirm Jeryu API health.
4. List existing Jeryu repos and note slug collisions.
5. Copy/adapt split tooling into `/home/ubuntu/jain-split/ops/split`.

### Phase 1: Author Manifest

1. Write root `repos.manifest.toml`.
2. Include all 19 repos.
3. Include `copy_paths`, `source_paths`, `cargo_members`, `cross_repo_deps`, wave, role, profile, tag, and check name.
4. Run manifest parser.
5. Run source coverage in report-only mode.

### Phase 2: Materialize Repos

1. Extract declared paths from `git archive` at the seed commit.
2. Preserve monorepo-relative layout.
3. Generate root `Cargo.toml` per Rust repo.
4. Copy root `Cargo.lock` per Rust repo.
5. Rewrite cross-repo dependencies to pinned Git tags.
6. Add split metadata and local CI scaffolding.
7. Add native/environment patches in split repos only.
8. Add controlled contract fixture metadata where applicable.
9. Add artifact LFS handling in `jain-artifacts`.

### Phase 3: Git Initialization

1. `git init -b main` in every repo.
2. Commit generated contents.
3. Create annotated split tags.
4. Add `origin` and `github` remotes.
5. Generate `jain/family.lock` from actual repo heads.
6. Commit/tag the portal after lock generation.
7. Re-run lock generation if the portal commit changed.

### Phase 4: Local Validation

Run:

```bash
bash /home/ubuntu/jain-split/jain/scripts/validate-family.sh
```

Then run lightweight required lanes:

```bash
for repo in /home/ubuntu/jain-split/jain-* /home/ubuntu/jain-split/jain; do
  (cd "$repo" && bash scripts/ci-local.sh required)
done
```

For expensive native/web/deploy lanes, record what was skipped and why if they are not run immediately.

### Phase 5: Jeryu Registration

1. Create missing local Jeryu repos.
2. Push `main` and tags.
3. Register family metadata.
4. Verify the family appears through the Jeryu repo listing/facets.
5. Configure required check names.
6. Run or register local CI check results.
7. Mirror green `main` and tags to GitHub.

### Phase 6: Full Product Proof

In `jain-deploy`:

```bash
bash scripts/materialize-source.sh
bash scripts/vendor-all.sh
cd target/split-source/jain-7.0.1-split.0
JAIN_VENDOR_ROOT=$PWD/vendor cargo build --release -p feat-cli
./target/release/jain --version
./target/release/jain demo --out-dir target/demo
cargo test -p sagemaker-ci
```

Optional heavier gates:

- Docker/SageMaker local.
- SageMaker AWS.
- GPU.
- Full web Playwright e2e.
- Full model-zoo parity.

## Red-Team Questions For Claude

1. Is there any reason to collapse `jain-contracts` into `jain-core` despite the explicit user request for a contract repo?
2. Can any native "required" lane safely run Cargo without vendor, or should all native Cargo checks fetch vendor first?
3. Does Jeryu family registration require `jeryu serve --split-manifest`, or is per-repo family PATCH sufficient in this live forge?
4. Should `jain-artifacts` mirror to GitHub with LFS, or remain forge-authoritative only until quota is confirmed?
5. Are any tracked source files missing from the 19-repo topology?
6. Are any files duplicated without a drift check?
7. Does `jain-web` need a generated local source bundle for live e2e against unreleased backend changes, or is tag-pinned CI enough?
8. Should `python/ai-service` stay in `jain-deploy`, as requested, or become a post-v1 `jain-python` split?
9. Are there any existing tests that assume `crates/domain` and `crates/feat-core` are in the same repo?
10. Does the artifact lock validate LFS pointer files, real smudged files, or both?

## Acceptance Criteria

The split is complete when:

1. `/home/ubuntu/jain-split` contains exactly the 19 planned repos plus split tooling/docs.
2. Every repo is an independent Git repo on `main`.
3. Every repo has the required split metadata and CI entrypoint.
4. Every repo has tag `<repo>-v7.0.1-split.0`.
5. `jain/repos.manifest.toml` and root `repos.manifest.toml` agree.
6. `jain/family.lock` records actual local commit SHAs.
7. `bash /home/ubuntu/jain-split/jain/scripts/validate-family.sh` passes.
8. No committed Cargo manifest contains forbidden branch dependencies.
9. No committed Cargo manifest contains cross-repo sibling path dependencies.
10. Source coverage has no unexplained gaps.
11. Controlled contract copies match `jain-contracts`.
12. Artifact lock and LFS checks pass.
13. Jeryu local repos exist at `http://127.0.0.1:8787/git/jeryu/<repo>.git`.
14. Required checks are defined as `<repo>/required`.
15. GitHub remotes point to `neverhuman/<repo>`.
16. `jain-deploy` can materialize a monorepo-shaped source bundle from pinned split tags.

## Current Recommendation

Implement the 19-repo split with the Jeryu precedent tooling, not an ad hoc script. Use Claude's plan as the operational checklist, but keep the original topology and the no-committed-sibling-path rule. The two highest-risk areas are native vendor resolution and artifact LFS handling; handle those before declaring any generated repo green.
