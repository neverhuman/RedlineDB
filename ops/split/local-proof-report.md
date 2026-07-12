# Jain Split Local Proof Report

Generated: 2026-07-06

## Completed Local Stages

- Stage 0 evidence written to `ops/split/preflight-report.md`.
- Execution plan written to `docs/split_execution_plan_codex.md` as the first mutating action.
- Split tooling forked/adapted under `ops/split/` and `ops/ci/`.
- `repos.manifest.toml` authored for 19 repos.
- Exactly three split-only product patch files added:
  - `ops/split/patches/catboost-sys-vendor-root.patch`
  - `ops/split/patches/xgboost-vendor-root.patch`
  - `ops/split/patches/lightgbm-vendor-root.patch`
- 19 split repos materialized under `/home/ubuntu/jain-split`.
- 19 local bare mirrors generated under `target/bare-mirrors`.
- No `.jeryu/repo.toml` files were generated.
- `/home/ubuntu/jain_small` remained clean at `cc27936eb45006bda0cae85b0f578f4d5985991d`.

## Materialized Commits

| Repo | Commit |
| --- | --- |
| `jain` | `e049597457457ffd9bcc063b014408bb5390a435` |
| `jain-docs` | `36de49159471dba06972a09837c4304b9acfbd3c` |
| `jain-domain` | `30a5df05c00a948b5d020112921ab3ab615ba109` |
| `jain-math` | `46fb8c0fc46af9d4a09aca7cdeaaaf68bbae1dc4` |
| `jain-contracts` | `13fa8ecd71345c683b557ebc758bd1770548222e` |
| `jain-catboost` | `36591f2de8ec44ed49cb693d240bea43c44c2981` |
| `jain-xgboost` | `6bfdb604a602775c88e495df31d538fb32c6c881` |
| `jain-lightgbm` | `b5c3d5c9132b1e16dbe0e673ab48a74fb34d95a0` |
| `jain-battle-gpu` | `7402fb2f366c41d53413daafec4999616d5dd682` |
| `jain-starforge` | `37f96c3b4afac8f6c7c292f3fb9c7ee0c4a7ea7f` |
| `jain-core` | `5bb9ec60d0a8a0d891629942c5ef1bc45516c95c` |
| `jain-report` | `491ddcb6a234ac5aee7974e704a5ca4e9d37022e` |
| `jain-tui` | `e3120065e654e7402650d93d2ec9e0c1440c1d76` |
| `jain-cli` | `60f6ed827e94f5b30255809b17fb815d1bbe852b` |
| `jain-web` | `47be6ffbc6fde63b41da1c6b4c17c53a3814ff54` |
| `jain-python` | `d72466de8345e3e74e6cf76fc403a478627db504` |
| `jain-model-zoo` | `d05a91933848e7f9243a320ac3dbecb1c244fe02` |
| `jain-ops` | `1517e79f82b89aa0daed9c38f5db64f39927bb25` |
| `jain-deploy` | `6ca498fedc76425db5594ce2cbb9eabf29009731` |

## Verification Passed

- `cargo test --locked --manifest-path Cargo.toml`
- `bash -n ops/split/manifest.sh ops/ci/split-host-ci.sh`
- `git apply --check ops/split/patches/*.patch` against `/home/ubuntu/jain_small`
- `bash ops/split/manifest.sh --manifest repos.manifest.toml --check-paths`
- `cargo run --locked -- source-coverage --manifest repos.manifest.toml`
  - tracked files: 4,388
  - owned: 4,306
  - retired/generated: 82
  - missing: 0
  - duplicates: 0
- `bash scripts/validate-family.sh` in the portal.
- `bash scripts/ci-local.sh required` in all 19 repos with local mirror config.
- All 19 repo worktrees clean after verification.
- Starforge LFS guard:
  - six tracked `.safetensors` files present
  - all six payloads are larger than 1 MB
  - `git lfs ls-files` lists all six in `jain-starforge`

## Verification Not Yet Green

- Jankurai score lane is not green. Portal score improved from 71 to 74 after routing fixes, but remains below the generated threshold of 85 due remaining Jankurai adoption/security/release-readiness findings.
- Full native learner, web e2e, Docker image, SageMaker, and deploy integration lanes were not run.
- Forge/GitHub proof was not run because credentials and LFS quota policy remain unresolved.

## External Blockers

- Local forge `gh` auth for `127.0.0.1:8787` is invalid.
- GitHub `gh` auth is absent.
- GitHub LFS upload policy for `neverhuman/jain-starforge` is unconfirmed.
- Missing runner tools before full proof:
  - `typst`
  - `taplo`
  - `patchelf`
  - `hadolint`
  - `cargo-public-api`
  - `cargo-semver-checks`

## Cutover State

- Existing forge slug `jeryu/jain` is still the monorepo and was not modified.
- The portal must be seeded to a preview slug such as `jeryu/jain-portal-preview` until final cutover.
- Final cutover remains blocked until required lanes, Jankurai score gates, forge auth, GitHub auth, and LFS policy are resolved.
