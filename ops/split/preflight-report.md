# Jain Split Preflight Report

Generated: 2026-07-06

## Source

- Source repo: `/home/ubuntu/jain_small`
- Branch: `apex`
- Seed SHA: `cc27936eb45006bda0cae85b0f578f4d5985991d`
- Worktree status: clean (`git status --porcelain=v1` produced no file rows)
- Tracked file count from `git ls-tree -r --name-only HEAD`: 4,388
- `reference/ported` first-level entries from `git ls-tree`: 493
- `reference/ported/**/Cargo.toml` count from `git ls-tree`: 655
- All tracked `Cargo.toml` count from `git ls-tree`: 671

## Forge And Slugs

- Jeryu health: `http://127.0.0.1:8787/api/v1/version` returned `{"name":"jeryu-api","version":"5.0.0"}`
- Collision check method: `git ls-remote --heads http://127.0.0.1:8787/git/jeryu/<repo>.git`
- Existing collision: `jeryu/jain`
  - heads observed: `apex`, `docs-user-guide`
- Non-colliding split slugs:
  - `jeryu/jain-docs`
  - `jeryu/jain-domain`
  - `jeryu/jain-math`
  - `jeryu/jain-contracts`
  - `jeryu/jain-catboost`
  - `jeryu/jain-xgboost`
  - `jeryu/jain-lightgbm`
  - `jeryu/jain-battle-gpu`
  - `jeryu/jain-starforge`
  - `jeryu/jain-core`
  - `jeryu/jain-report`
  - `jeryu/jain-tui`
  - `jeryu/jain-cli`
  - `jeryu/jain-web`
  - `jeryu/jain-python`
  - `jeryu/jain-model-zoo`
  - `jeryu/jain-ops`
  - `jeryu/jain-deploy`
  - `jeryu/jain-portal-preview`
- Collision handling: seed the portal under `jeryu/jain-portal-preview`; at final cutover, rename/archive existing `jeryu/jain` to `jeryu/jain-monorepo`, then move the portal to `jeryu/jain`.

## LFS And Starforge

- `git lfs fsck`: OK
- Tracked LFS safetensors:
  - `artifacts/foundation/tabicl-classifier-v2-chimera-shuffled.safetensors`
  - `artifacts/foundation/tabicl-regressor-v2-20260212.safetensors`
  - `artifacts/starforge/chimera_classification_attnfusion.safetensors`
  - `artifacts/starforge/chimera_classification_expert_a.safetensors`
  - `artifacts/starforge/chimera_stackfusionreg.safetensors`
  - `artifacts/starforge/starlight_core_regressor.safetensors`
- Local checkout is smudged: all six files are present as Git LFS objects; materialization must copy payloads from the worktree, not from `git archive` pointer content.

## Toolchain

- Docker: `Docker version 29.2.1, build a5c7197`
- Git: `git version 2.43.0`
- Git LFS: `git-lfs/3.5.1`
- Rust: `rustc 1.96.0 (ac68faa20 2026-05-25)`
- Cargo: `cargo 1.96.0 (30a34c682 2026-05-25)`
- Node: `v26.1.0`
- pnpm: `11.9.0`
- Python: `Python 3.12.3`
- gh: `gh version 2.62.0 (2024-11-14)`
- Present runner tools:
  - `shellcheck` at `/home/ubuntu/.local/bin/shellcheck`
  - `jq` 1.7
  - `just` 1.51.0
  - `jankurai` 1.6.10
- Missing runner tools to install or vendor before full proof:
  - `typst`
  - `taplo`
  - `patchelf`
  - `hadolint`
  - `cargo-public-api`
  - `cargo-semver-checks`

## Disk

- `/home/ubuntu`: 7.3T total, 4.5T used, 2.5T available, 65% used
- `/tmp`: same filesystem, 2.5T available

## Credentials

- Local forge `gh` auth is invalid for `127.0.0.1:8787`:
  - active account `jeryu`
  - token in `/home/ubuntu/.config/gh/hosts.yml` is invalid
  - required repair: `jeryu gh-setup --host http://127.0.0.1:8787 --token-file ~/.jeryu/secrets/merge-token`
- GitHub auth is not configured in `gh auth status` output.
- Git remotes in source:
  - `origin` = `http://127.0.0.1:8787/git/jeryu/jain.git`
  - `neverhuman` = `git@github.com:neverhuman/jain.git`
- Git LFS access in the source repo reports `AccessDownload=none` and `AccessUpload=none`.

## Blockers Before Forge/GitHub Proof

- Refresh local forge `gh` auth with `jeryu gh-setup --host http://127.0.0.1:8787 --token-file ~/.jeryu/secrets/merge-token`; do not run `gh auth login` for the local Jeryu host.
- Configure GitHub `gh` auth before mirror creation or trial PR work.
- Confirm GitHub LFS quota policy for `neverhuman/jain-starforge`; GitHub Starforge LFS upload remains disabled until explicitly approved.
- Install or vendor the missing runner tools listed above before running full PDF, TOML, Dockerfile, public API, and semver lanes.

## Local Split Proceed/Stop Decision

Local manifest generation, source coverage, materialization, local bare mirrors, and offline `insteadOf` checks can proceed without credentials. Forge onboarding, GitHub mirrors, PRs, and cutover must remain blocked until the credential and LFS decisions above are resolved.
