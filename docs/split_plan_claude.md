# jain-split: Detailed Split Plan (authored by Claude, for agent review)

- **Date**: 2026-07-06
- **Source monorepo**: `/home/ubuntu/jain_small` (forge slug `jeryu/jain`, origin `http://127.0.0.1:8787/git/jeryu/jain.git`, github mirror `git@github.com:neverhuman/jain.git`), workspace version **7.0.1**, HEAD at study time `cc27936` on branch `apex` (8 commits ahead of `origin/apex`).
- **Target**: `/home/ubuntu/jain-split/` — a **16-repo split family** `jain-split`, registered on the local jeryu forge, following the proven `~/jeryu-split/` pattern.
- **Goals** (from the request): maximize repo count **without complicating testing or building**; every repo has its own CI and is fully testable **without the others present**; the full binary build stays very straightforward; the family is managed in the local jeryu instance and recognized as a split family.

## 1. The pattern being replicated (findings from ~/jeryu-split, ~/veox-split, ~/jankurai-split)

Terminology: the **forge** is jeryu (`~/.jeryu/bin/jeryu`, systemd user unit `jeryu-api.service`, REST+smart-HTTP at `http://127.0.0.1:8787`). The **audit standard inside each repo** is jankurai (`agent/*.toml`, `.jankurai/`, `jankurai audit`, min score 85). Don't conflate them.

1. **`repos.manifest.toml`** at the split root is the source of truth. jeryu-split schema (`schema_version = "1"`): top-level `workers`, `source_root`, `source_sha`, `split_root`, `repo_family`, `dependency_tag_suffix` (`vX.Y.Z-split.N`), `required_repos`, `shared_source_paths`; per `[[repo]]`: `path, name, github_slug, jeryu_slug, profile, default_branch(=main), rollout_wave, has_jeryu_std, onboarded, mirror_github_main, current_tag(=<name>-vX.Y.Z-split.N), required_check(=<name>/required), note`, plus materialization arrays `cargo_members`, `copy_paths`, `source_paths`. Validator: `ops/split/manifest.sh`. Optionally adopt jankurai-split's `gates = {...}` / `remotes = {...}` extensions for onboarding tracking.
2. **Materialization** (`~/jeryu-split/ops/split/materialize.py`, 1337 lines): asserts monorepo HEAD == `source_sha`; `git archive`s each repo's `copy_paths` **preserving monorepo-relative layout** (e.g. `crates/feat-core` stays at `crates/feat-core`); rewrites the root `Cargo.toml` members to `cargo_members`; converts cross-repo path deps to **pinned public git-tag deps** (`git = "https://github.com/neverhuman/<repo>.git", tag = "<repo>-vX.Y.Z-split.N"`, preserving `optional`/`default-features`/`features` — verified at lines 156–186); appends deploy `[patch]` sections (lines 186–207); generates the full per-repo standard (README, AGENTS.md, Justfile, VERSION, CHANGELOG, `docs/{architecture,testing,release}.md`, `agent/{owner-map.json,test-map.json,generated-zones.toml,proof-lanes.toml,audit-policy.toml,boundaries.toml,JANKURAI_STANDARD.md}`, `ops/ci/*.sh`, `ops/git-hooks/pre-push`, `scripts/ci-local.sh`, `.github/workflows/ci.yml`); `git init -b main` + seed commit + tag. **History is NOT preserved — all three precedent families fresh-seeded** (jeryu-core has 13 commits, root = "chore: seed jeryu split repo from <sha>").
3. **Cross-repo deps = pinned public git tags** (`agent/boundaries.toml`: `cross_repo_dependency_policy = "pinned-public-git-tags"`). **Single exception**: the deploy repo carries `[patch."https://github.com/neverhuman/X.git"] X = { path = "../X/..." }` sections for local sibling development and is the sole release authority (`local_path_patches = true` only there).
4. **Per-repo CI**: generated `Justfile` → `ops/ci/{fast,check,score,security,artifact_support}.sh`; `agent/proof-lanes.toml` marks blocking lanes; `agent/audit-policy.toml` pins jankurai ≥85/0-hard-findings. Heavy CI runs host-native via `~/jeryu-split/ops/ci/split-host-ci.sh <owner> <repo> <sha> <path> [check]`: worktree checkout, **symlinks all sibling split repos next to the worktree** (so deploy `[patch]` paths resolve), governed workers via `jeryu-ci-governor`, persistent `CARGO_TARGET_DIR` + sccache, runs `ops/ci/pr-ci.sh`, then `POST /repos/<o>/<r>/check-runs {"name":"<repo>/required", ...}`.
5. **Forge wiring**: create bare repo `POST $JERYU_BASE/repos {"name":..., "private":true, "default_branch":"main"}` (see `~/veox-split/jeryu-ctl/onboard.sh`); remotes: `origin` → `http://127.0.0.1:8787/git/jeryu/<name>.git`, `github` → `git@github.com:neverhuman/<name>.git`; **family registration** = `PATCH /api/v1/repos/jeryu%2F<name> {"family":"jain-split"}` per repo + verify via `GET /api/v1/repos?host=jeryu` facets (`~/jeryu-split/ops/split/register-family.sh`). Optional hardening: PR-only `pre-receive` hook on main + branch protection `{"required_status_checks":["<repo>/required"], "required_linear_history":true, "enforce_admins":true}`. `jeryu onboard` CLI is dry-run only — real onboarding is these curl primitives.
6. **Portal + deploy**: portal repo = bare family name (`jain`), profile `public-portal`, holds manifest + `scripts/clone-family.sh` + `install.sh`, **no product source**. Deploy repo gets `jain-split.lock.toml` (`schema_version = "jeryu.split.lock/v1"`: name/github_slug/local_path/tag/commit/required_check per repo), `fleet_ci.py`, `product_pipeline.py`, `verify-lock.py`, `smoke_serve.sh`.
7. **Completeness**: `source_coverage.py` fails unless every git-tracked monorepo file at `source_sha` matches some repo's `source_paths` or `shared_source_paths` — the split provably loses nothing.
8. **Version bumps**: `bump-family-version.py` rewrites workspace versions, all tag pins, manifest, lock, VERSION, CHANGELOGs in one shot; `[skip-version]` commit marker suppresses the forge autoversioner.

## 2. What's being split (jain_small inventory)

**Workspace (14 members, virtual manifest, v7.0.1)** — internal dep graph (→ = depends on):

- Leaves: `catboost-sys` (FFI+bindgen, bin `catboost-cbtool`), `xgboost` (FFI/CMake), `lightgbm` (FFI/CMake), `domain` (error contract, serde-only), `feat-math` (GP DSL engine), `starforge` (candle inference, bin `starforge`, feature `cuda`)
- `catboost` → `catboost-sys`; `battle-gpu` → `feat-math` (cudarc **dynamic-loading** — default `gpu` feature builds/tests on CPU hosts)
- `feat-core` → `domain`, `feat-math` + optional `catboost`/`xgboost`/`lightgbm`/`starforge`/candle. **The hub.** Features: `catboost,xgboost,lightgbm,ci-smoke,starforge-cpu,starforge-cuda,hyperion-cpu,hyperion-cuda`; **default = empty**
- `feat-report` → `feat-core`,`feat-math`; `feat-tui` → `feat-core`
- `feat-cli` → `feat-core,feat-math,feat-report,feat-tui,battle-gpu`; **bins `jain` + `jain-entrypoint`**; defaults `catboost,xgboost,lightgbm`
- `feat-web` → `feat-core,feat-report,battle-gpu`; **bin `jain-web`** (axum + bundled rusqlite); **defaults include `hyperion-cuda`**
- `sagemaker-ci` (`deployment/ops/sagemaker-ci`): zero internal deps, 2 contract-test bins

**Non-workspace areas**: `apps/web` (React/Vite cockpit; `scripts/dev.mjs` spawns `cargo run -p feat-web`; Playwright live e2e drives the compiled Rust binary; `agent/boundaries.toml` defines the `web` boundary as `["crates/feat-web/**","apps/web/**"]`), `python/ai-service` (`jain-sagemaker` SDK, transport-only), `contracts/` (**EVENT_SCHEMA_VERSION = 9** lockstep: producer `crates/feat-core/src/progress.rs`; mirrors `apps/web/src/protocol.ts` + `src/generated/progressEvent.ts`, `python/.../contract.py` + `progress/events.py`; fixture `contracts/progress-events.jsonl`; Rust test `progress_contract.rs` does `include_str!("../../contracts/...")`), `artifacts/` (6 LFS safetensors: starforge ~528M + foundation ~214M; runtime resolution `/opt/jain/...` → repo-relative), `vendor/` (1.1G, **gitignored**, cloned+pruned upstream C/C++; all three learner `build.rs` hard-resolve `../../vendor/<name>` and `.expect()`), `reference/ported/` (486 standalone frozen crates = **3,153 of 4,388 tracked files, 72%**; consumed by `ops/parity/*`; 2 crates wired into optional native tests via `agent/test-map.json`), `ops/apex` (jail-tools standalone crate + godmode research), `deployment/ops` (Dockerfiles + `assemble-runtime-assets.sh` with Python-free guard + locks/receipts), `docs/` (hub + Typst manual + research writeups), `agent/` (jankurai governance), `scripts/`, `tests/cbtest.c`, `db/` (docs only — real DB is feat-web's bundled SQLite under `--data-dir`).

**CI lanes today** (all thin wrappers → `ops/ci/*.sh`): fast, agent-fast, rust-api, sagemaker-rust-contract, coverage-api, coverage-full (dispatch), security, contracts, jankurai-audit, native (dispatch, 120 min, FFI from vendor), release, version-consistency, compression, python-client (builds docker), examples (builds docker), web-cockpit (rust ci-smoke + vitest + Playwright mocked+live), docs-pdf, mirror (fast-forwards github after all green). CUDA is never compiled on hosted runners; GPU lanes are self-hosted dispatch/cron.

## 3. The split: 16 repos

Two deliberate deviations from a "17-repo maximal" first draft, both verified:
- **No standalone `jain-contracts` repo.** `domain` has exactly one consumer (`crates/feat-core/Cargo.toml:27`); `contracts/progress-event.schema.json` is a generated zone **produced from** `crates/feat-core/src/progress.rs` (per `agent/generated-zones.toml`) and `progress_contract.rs` include_str!s it relative to feat-core. A contracts repo would turn every schema bump into two ordered PRs + a tag pin before the producer test compiles. jeryu precedent: contracts are duplicated into consumers, core canonical.
- **ALL weights live in `jain-starforge`** (not split core/starforge): `crates/starforge/src/model.rs:631-634` hard-references `artifacts/foundation/tabicl-regressor-v2-20260212.safetensors`; `golden_parity.rs` needs both dirs; jain-core's required tests don't need weights (they skip/early-return — verified for `hyperion_v7_weights.rs`, `foundation.rs`, `artifact.rs`, `model_artifacts.rs`); a single LFS home keeps the 742M inside GitHub LFS free tier once.

Boundaries upheld against merging: **3 learner repos** (zero shared code, per-learner vendor + prune sections + ~40-min native builds, per-learner optional features in feat-core — one 120-min serial lane becomes three independent dispatch lanes); **report/tui separate** (single-crate consumers with distinct proof lanes; tag-ripple is automated by `bump-family-version.py`); **feat-web + apps/web inseparable** (the live-e2e proof lane spans them); **reference/ops/docs out of product repos** (72% of tracked files leave every product clone).

### 3.1 Repo roster

| # | Repo | Profile | Contents (cargo_members ⊕ key copy_paths) | Tag-pinned deps | Wave |
|---|------|---------|-------------------------------------------|-----------------|------|
| 1 | `jain` (portal) | public-portal | manifest, `clone-family.sh`, `install.sh`, family docs hub (authored, no product source) | — | 0 |
| 2 | `jain-math` | rust-workspace | `crates/feat-math` | — | 0 (canary) |
| 3 | `jain-catboost` | rust-workspace | `crates/catboost-sys`, `crates/catboost`, `tests/cbtest.c`, `scripts/vendor.sh` (catboost section of `prune_vendor.sh`) | — | 1 |
| 4 | `jain-xgboost` | rust-workspace | `crates/xgboost`, `scripts/vendor.sh` | — | 1 |
| 5 | `jain-lightgbm` | rust-workspace | `crates/lightgbm`, `scripts/vendor.sh` | — | 1 |
| 6 | `jain-battle-gpu` | rust-workspace | `crates/battle-gpu` | jain-math | 1 |
| 7 | `jain-starforge` | rust-workspace (+LFS) | `crates/starforge`, **`artifacts/starforge` + `artifacts/foundation` (LFS)**, `ops/ci/compression.sh`, `scripts/{compress,prepare}-starforge-*.sh` | — | 1 |
| 8 | `jain-core` | rust-workspace | `crates/feat-core`, `crates/domain`, **`contracts/` (canonical)**, fast/rust-api/coverage-api/contracts lane scripts | jain-math; optional: jain-catboost, jain-xgboost, jain-lightgbm, jain-starforge | 1 |
| 9 | `jain-report` | rust-workspace | `crates/feat-report`, report-pdf (Typst) lane | jain-core, jain-math | 1 |
| 10 | `jain-tui` | rust-workspace | `crates/feat-tui` | jain-core | 1 |
| 11 | `jain-web` | custom (rust-node-hybrid) | `crates/feat-web`, **`apps/web`**, `contracts/` (duplicated read-only), `db/`, `ops/ci/web-cockpit.sh` | jain-core, jain-report, jain-battle-gpu (`default-features=false`, as today) | 1 |
| 12 | `jain-python` | custom (python) | `python/ai-service`, `contracts/` (duplicated read-only) | — | 1 |
| 13 | `jain-deploy` | deploy | `crates/feat-cli` (**the `jain` binary**), `deployment/ops/sagemaker-ci`, `deployment/ops/**` (Dockerfiles, assemble, locks), canonical `ops/ci/**` + `scripts/**` + `tools/security-lane.sh` + `agent/` originals, **`[patch]` for all 8 Rust sibling repos**, `jain-split.lock.toml` + `fleet_ci.py` + `product_pipeline.py` + `stage-context.sh` (authored) | jain-core, jain-math, jain-report, jain-tui, jain-battle-gpu (+patched transitives) | 1 |
| 14 | `jain-reference` | custom | `reference/ported/**` (486 crates, oracles), `ops/parity/**` | — | 2 (non-gating) |
| 15 | `jain-ops` | custom | `ops/apex/**` (jail-tools crate, godmode campaigns, zyal) | — | 2 (non-gating) |
| 16 | `jain-docs` | custom | `docs/**` (Typst manual, research writeups, product docs hub), `ops/ci/docs-pdf.sh` | — | 2 (non-gating) |

`required_repos` = #1–#13. `source_paths` = each repo's `copy_paths` globbed `/**`. Every repo keeps **monorepo-relative layout** (materialize.py convention) — this is what lets `include_str!("../../contracts/...")`, `--static-dir apps/web/dist`, `$ROOT/reference/ported/...` etc. work unchanged.

### 3.2 Manifest header (to author at `~/jain-split/repos.manifest.toml`)

```toml
schema_version = "1"
workers = 40
source_root = "/home/ubuntu/jain_small"
source_sha  = "<post-split-prep HEAD on main>"   # Phase 0 output
split_root  = "/home/ubuntu/jain-split"
repo_family = "jain-split"
dependency_tag_suffix = "v7.0.1-split.0"
required_repos = ["jain","jain-math","jain-catboost","jain-xgboost","jain-lightgbm",
  "jain-battle-gpu","jain-starforge","jain-core","jain-report","jain-tui",
  "jain-web","jain-python","jain-deploy"]
shared_source_paths = [".cargo/**",".gitignore",".gitattributes",".dockerignore",
  "rust-toolchain.toml","Cargo.toml","Cargo.lock","Justfile","AGENTS.md","README.md",
  "CHANGELOG.md","RELEASE.md","RELEASE_PROCESS.md","ROLLBACK.md","RESULTS.md",
  "renovate.json",".grype.yaml",".claude/**",".jankurai/**",".github/**","agent/**",
  "scripts/ci-*.sh","ops/ci/lib.sh","docs/**"]
```

Example member entry (jain-core):

```toml
[[repo]]
path = "/home/ubuntu/jain-split/jain-core"
name = "jain-core"
github_slug = "neverhuman/jain-core"
jeryu_slug  = "jeryu/jain-core"
profile = "rust-workspace"
default_branch = "main"
rollout_wave = 1
has_jeryu_std = true
onboarded = false
mirror_github_main = true
current_tag = "jain-core-v7.0.1-split.0"
required_check = "jain-core/required"
note = "Pipeline hub: GP synthesis, epochs, ensemble; canonical progress-event contract + domain error contract."
cargo_members = ["crates/feat-core", "crates/domain"]
copy_paths    = ["crates/feat-core", "crates/domain", "contracts"]
source_paths  = ["crates/feat-core/**", "crates/domain/**", "contracts/**"]
```

### 3.3 Per-repo CI (`<repo>/required` + extra lanes)

Every repo gets the generated standard: `Justfile` → `ops/ci/{fast,check,score,security}.sh`, `agent/proof-lanes.toml` (fast/check/score/security blocking), `agent/audit-policy.toml` (jankurai 1.6.10, score ≥85, 0 hard findings), `.github/workflows/ci.yml`, and runs under `split-host-ci.sh` posting `<repo>/required` to the forge.

| Repo | `<repo>/required` runs | Extra (non-blocking / dispatch) lanes |
|---|---|---|
| jain | manifest parse (`manifest.sh`) + `clone-family.sh --dry-run` | — |
| jain-math | `cargo test -p feat-math` (full: bench/champion_parity/golden/tokens) | coverage |
| jain-catboost / -xgboost / -lightgbm | fmt + clippy of lib targets + manifest lint — **no `cargo check`** (build.rs panics without vendor) | `native` (dispatch): `scripts/vendor.sh` clone+prune → full FFI build → smoke tests (+ cbtool/cbtest.c for catboost) |
| jain-battle-gpu | `cargo test -p battle-gpu` (default `gpu` compiles on CPU via cudarc dynamic-loading) + `--no-default-features` check | gpu-bench (self-hosted dispatch) |
| jain-starforge | `cargo test -p starforge --all-targets` incl. `golden_parity` (weights in-repo) + LFS pointer guard (`git lfs ls-files` + each safetensors >1MB) | compression (zst round-trip); cuda (self-hosted dispatch) |
| jain-core | monorepo `fast.sh` minus feat-cli/tui/sagemaker lines: fmt, clippy, `cargo test -p feat-core -- --skip apex` + `--test artifact` + `--test model_artifacts` + `--test progress_contract` + domain tests. No weights, no vendor, learner features off | rust-api (public-api drift); coverage-api; contracts (Rust side) |
| jain-report | `cargo test -p feat-report` | report-pdf (Typst) |
| jain-tui | `cargo test -p feat-tui` | — |
| jain-web | `web-cockpit.sh` as today: `cargo test -p feat-web --no-default-features --features ci-smoke` + pnpm typecheck/vitest (incl. `protocol.contract.test.ts` vs local `contracts/` copy) + `pnpm build` + Playwright mocked + live (ci-smoke binary, no FFI/vendor needed) | — |
| jain-python | ruff + pytest (moto; `test_contract.py` vs local `contracts/` copy) | (docker-integration + notebook lanes move to jain-deploy) |
| jain-deploy | `cargo test -p feat-cli --no-default-features --features ci-smoke` + `cargo check -p feat-cli --no-default-features` + sagemaker-ci tests (vendor-free) | sagemaker-rust-contract; release; version-consistency (fleet pin check vs lock); examples (docker); python-client-vs-container; native-integration (feat-core × learners, vendor + artifacts present); coverage-full (cron); **contracts-sync** (fleet drift gate); sagemaker-aws/-gpu (dispatch/cron) |
| jain-reference | integrity walk (every `reference/ported/*/Cargo.toml` parses) + oracle `.npy` checksums — **not** 486 builds | native-spot (dispatch: `cargo test --manifest-path reference/ported/catboost/Cargo.toml --release` + tabicl — the two test-map-wired crates); parity (manual, dev-only) |
| jain-ops | jail-tools `cargo test` + shellcheck of campaign scripts | apex campaigns stay manual |
| jain-docs | `docs-pdf.sh` (Typst manual builds) | — |

## 4. The four hard migrations

### 4.1 EVENT_SCHEMA_VERSION lockstep (v9; 5 files across 3 repos)
- Canonical home **jain-core**: `crates/feat-core/src/progress.rs` (producer) + `contracts/` at monorepo-relative root → `progress_contract.rs`'s `include_str!("../../contracts/...")` compiles **byte-for-byte unchanged**.
- **jain-web** and **jain-python** carry duplicated `contracts/` copies registered in their `agent/generated-zones.toml` as `read_only = true`, `write_policy = "synced_from_jain_core"`. Their existing tests already resolve repo-root-relative (verified: `test_contract.py` uses `parents[3]`; `protocol.contract.test.ts` uses `resolve(root, 'contracts/...')`) — zero code changes.
- Enforcement: (a) each repo's own language test validates its local copy; (b) new deploy fleet lane **`contracts-sync`** sha256-compares `progress-event.schema.json` + `progress-events.jsonl` across the three checkouts at `jain-split.lock.toml` commits.
- Bump flow: PR1 jain-core (code+schema+fixture atomic, new tag) → PR2 jain-web / PR3 jain-python (copy sync + constant bump + regen `progressEvent.ts` + core tag-pin bump **in the same PR**) → PR4 deploy lock bump. `contracts-sync` goes red if any step is skipped.

### 4.2 Artifacts / LFS
- Single git home: **jain-starforge** (`artifacts/starforge` 4 files + `artifacts/foundation` 2 files, `.gitattributes` LFS rules travel with it).
- **materialize.py must LFS-smudge**: `git archive` emits 134-byte pointer files; add a post-copy step detecting `version https://git-lfs.github.com/spec/v1` headers → `git -C /home/ubuntu/jain_small lfs smudge` each, then seed-commit with `.gitattributes` present so `git add` re-cleans into LFS. Add a reconcile assertion: every `*.safetensors` > 1MB (a forgotten smudge makes `golden_parity` silently skip instead of fail).
- jain-core's weight-hungry tests (`foundation.rs` full runs, `hyperion_v7_weights.rs`) run in deploy's `native-integration` fleet lane; `split-host-ci.sh` gains one hook: if lane env sets `JAIN_NEEDS_ARTIFACTS=1`, `ln -s $SPLIT_ROOT/jain-starforge/artifacts $worktree/artifacts` (repo-relative resolution in `feat-core/src/starforge_integration/config.rs` then works; `/opt/jain/...` stays runtime-primary in images).
- Docker: `stage-context.sh` copies `../jain-starforge/artifacts/{starforge,foundation}` (at locked commit) into the build context so the existing `COPY artifacts/...` lines survive. `starforge-artifacts.lock` + receipt stay in jain-deploy (runtime-assembly concern).

### 4.3 vendor/ + FFI builds
- Verified blocker: all three learner `build.rs` resolve `CARGO_MANIFEST_DIR/../../vendor/<name>` and `.expect()` — fine in-repo and under deploy `[patch]` siblings, **fatal when consumed as a cargo git dep** (no vendor in `~/.cargo/git/checkouts`).
- **Phase 0 split-prep (monorepo PR)**: add `JAIN_VENDOR_ROOT` env override to the three build.rs (~6 lines total: `env::var("JAIN_VENDOR_ROOT").map(|r| Path::new(&r).join("catboost")).unwrap_or_else(|_| manifest.join("../../vendor/catboost"))`), verifiable in the monorepo before splitting.
- Each learner repo gets `scripts/vendor.sh` (its pinned upstream clone + its section of `prune_vendor.sh`); jain-deploy gets `scripts/vendor-all.sh` (clones all three under `jain-deploy/vendor/`, exports `JAIN_VENDOR_ROOT=$PWD/vendor`); Docker sets `ENV JAIN_VENDOR_ROOT=/build/vendor` with vendor staged by `stage-context.sh`.
- No required check anywhere needs vendor (learner required = fmt/clippy-no-build; core/cli/web required run learner-feature-free — matches today's `fast.sh`/`web-cockpit.sh`); only dispatch `native` lanes build FFI.

### 4.4 reference/ported ↔ ops/parity ↔ test-map
- Move **together** into jain-reference: `automl_parity.sh`/`regression_parity.sh` resolve `$ROOT/reference/ported/$MODEL/...` and `parity_suite.py` reads `reference/ported/*/parity/` — same-repo monorepo-relative layout ⇒ zero path edits.
- The `agent/test-map.json` entries running `cargo test --manifest-path reference/ported/{catboost,tabicl}/Cargo.toml` regenerate **only** into jain-reference's test-map (stripped from all other repos).
- Full oracle parity suite stays `just parity`-manual (dev-only, needs `remote_super` datasets), exactly as today.

## 5. Full-binary story (developer UX)

One-time setup:
```bash
git clone http://127.0.0.1:8787/git/jeryu/jain.git ~/jain-split/jain
~/jain-split/jain/scripts/clone-family.sh        # clones all 16 siblings; LFS pull only for jain-starforge
cd ~/jain-split/jain-deploy && ./scripts/vendor-all.sh
```

**The `jain` binary** (defaults: catboost+xgboost+lightgbm):
```bash
cd ~/jain-split/jain-deploy && cargo build --release -p feat-cli
# → target/release/{jain,jain-entrypoint}; [patch] resolves ../jain-*/crates/*; vendor via JAIN_VENDOR_ROOT
```

**jain-web + cockpit:**
```bash
cd ~/jain-split/jain-web
pnpm --dir apps/web install && pnpm --dir apps/web build
JAIN_VENDOR_ROOT=~/jain-split/jain-deploy/vendor cargo build --release -p feat-web \
  --no-default-features --features catboost,xgboost,lightgbm,hyperion-cpu
# GPU host: plain `cargo build --release -p feat-web` (defaults incl. hyperion-cuda)
./target/release/jain-web    # --static-dir defaults resolve apps/web/dist from repo root
```
Local cross-repo iteration against unreleased core: generated `ops/dev/local-patches.example.toml` per consumer repo (cargo `[patch]` via `.cargo/config.toml`, never committed, CI-invisible) — deploy remains the only committed-patch workspace.

**SageMaker image:**
```bash
cd ~/jain-split/jain-deploy && just image
# stage-context.sh: reads jain-split.lock.toml → git-archives each Rust sibling at its locked commit into
# .stage/crates/* (monorepo layout) → synthesizes .stage/Cargo.toml with [patch] mapping git URLs → ./crates/*
# (hermetic, offline) → vendor-all into .stage/vendor → copies ../jain-starforge/artifacts →
# docker build -f deployment/ops/Dockerfile.sagemaker .stage   (existing COPY lines survive)
```

## 6. Rollout phases (jeryu-split sequence + jain deltas)

0. **Split-prep PR on `jeryu/jain`** (monorepo): settle `apex` → `main` (branch is 8 commits ahead); add `JAIN_VENDOR_ROOT` overrides ×3 build.rs; split `prune_vendor.sh` into per-learner `scripts/vendor-*.sh` (wrapper kept); full CI green. This commit = `source_sha`.
1. **Fork tooling** into `~/jain-split/ops/`: copy `~/jeryu-split/ops/split/*` + `ops/ci/split-host-ci.sh`. materialize.py deltas: (i) deploy-name `"jain-deploy"` in `render_patch_sections`; (ii) **LFS smudge step** in the archive copier; (iii) new profile renderers `rust-node-hybrid` (jain-web) and `python` (jain-python); (iv) per-repo Justfile/test-map templates seeded from jain's `ops/ci/*.sh` lane scripts (not jeryu's); (v) `split-host-ci.sh` `JAIN_NEEDS_ARTIFACTS` sibling-symlink hook. Port `source_coverage.py`, `reconcile.py`, `manifest.sh`, `register-family.sh`, `rollout-pr-flow.sh`, `commit-baseline.sh`, `cutover.sh`, `closeout-prs.sh`, `bump-family-version.py` with name substitutions.
2. **Author `repos.manifest.toml`** per §3; iterate `source_coverage.py` until all 4,388 tracked files land in ≥1 repo.
3. **Materialize**: 16 fresh seeds, dep rewrite to pinned tags (rewriter preserves `optional`/`default-features`/`features` — required by feat-core's optional learner deps and feat-web's `default-features=false` battle-gpu dep), tags cut, `--capture-dirty-patch` if the monorepo is dirty.
4. **Reconcile + independence proof**: per-repo standalone `just fast` with **no siblings present** (this is the "fully tested without requiring the others" acceptance gate); then deploy family build via patches; iterate conflicts through `dirty/merge-work/`.
5. **Commit-baseline + register-family**: 3 reviewable commits per repo (`[skip-version]`); `POST /repos` per repo; `PATCH {"family":"jain-split"}`; verify facets; create `neverhuman/*` GitHub mirrors; LFS push (jain-starforge only).
6. **Rollout-pr-flow**: branch protection (`<repo>/required`, linear history); one trial PR per repo through `split-host-ci.sh`; enable `mirror_github_main` after green.
7. **Cutover**: rename forge `jeryu/jain` → `jeryu/jain-monorepo` (archived); portal registered as `jeryu/jain`; portal ships manifest + clone-family + install.
8. **Closeout**: commit `jain-split.lock.toml` with real commits; first full `fleet_ci.py` run; `product_pipeline.py` image-build proof; closeout PRs verifying github mirrors.

## 7. Risks & gotchas

- **LFS smudge is load-bearing** (§4.2) — a forgotten smudge seeds pointer-only "weights" and golden_parity *skips silently*. The >1MB reconcile assertion is mandatory.
- **feat-web's `hyperion-cuda` default**: any hosted lane running plain `cargo build -p feat-web` fails (candle CUDA needs the toolkit). All generated jain-web CI pins `--no-default-features --features ci-smoke` (as `web-cockpit.sh` already does); add a workflow-lint guard. Demoting hyperion-cuda from defaults is a separate post-split decision — do not bundle it.
- **feat-core API churn** (hottest crate, 20 commits/3mo, 5 dependents pin its tag): keep `rust-api` public-API drift as early warning; drive bumps only via `bump-family-version.py` (wave order math → core → report/tui/web → deploy); never hand-edit one pin (mismatched pins → two feat-core revs in deploy's graph → `[patch]` unification failure; the `version-consistency` fleet lane checks pins vs lock).
- **Policy propagation**: TabPFN-free + banned-term ("fallback") audits live in `security.sh`/`audit-policy.toml` — materialize must replicate them into **all 16 repos** byte-identical, or policy silently narrows.
- **Learner required checks must not `cargo check`** (build.rs vendor `.expect()` panics); fmt/clippy-of-lib-targets only, real builds in dispatch native lanes — mirrors today's dispatch-only `native` lane.
- **apex tests** need downloaded datasets (`get_data.sh` lives in deploy) — already `--skip apex` in fast; they run only in deploy/jain-ops dispatch lanes.
- **rtk output truncation** (env gotcha for implementing agents): `rtk`-proxied commands truncate stdout ~40 lines — never pipe manifest/coverage listings through it; write to files.

## 8. Decisions made (with rationale) + open items for reviewer

Decided:
- **16 repos** (max that doesn't complicate testing/building); merge-fallbacks if reviewer disagrees: 3 learner repos → 1 `jain-learners`; report+tui → into core; docs/ops → into portal. Each fallback loses independence granularity but nothing else.
- **Fresh-seed history** (all three precedent families did this; monorepo stays archived as `jeryu/jain-monorepo`).
- **Forge owner `jeryu/*`**, github `neverhuman/*` (matches current `jeryu/jain` onboarding).
- **Portal takes the `jain` name at cutover** (jeryu precedent).

Open for reviewer:
1. Does the forge `jeryu serve --split-manifest` flag (currently pointing at jeryu-split's manifest) support multiple families, or is the `family` PATCH sufficient for jain-split? (veox/jankurai families are PATCH-registered without server manifests — the proven path; confirm before Phase 5.)
2. GitHub LFS quota: 742M fits the free tier once; confirm the `neverhuman` org's LFS budget before mirroring jain-starforge, or keep its github mirror LFS-less (forge-only weights).
3. Should `jain-reference` even mirror to GitHub (3,153 files of frozen reference), or stay forge-only?
4. `sagemaker-ci` placement: kept in jain-deploy (it tests the container contract). Alternative: standalone repo #17 — it has zero internal deps; excluded to avoid a repo whose only purpose couples to deploy's image anyway.

## Appendix: load-bearing files to fork/adapt

- `~/jeryu-split/ops/split/materialize.py` (engine; dep-rewrite ll.156–186, patch rendering ll.186–207)
- `~/jeryu-split/ops/split/{manifest.sh,source_coverage.py,reconcile.py,register-family.sh,rollout-pr-flow.sh,commit-baseline.sh,cutover.sh,closeout-prs.sh,bump-family-version.py}`
- `~/jeryu-split/ops/ci/split-host-ci.sh` (sibling-symlink runner; gains `JAIN_NEEDS_ARTIFACTS` hook)
- `~/veox-split/jeryu-ctl/{onboard.sh,lib.sh,host-ci.sh,jeryu-poll.sh,hooks/pre-receive}` (forge REST primitives)
- `~/jain_small/crates/{catboost-sys,xgboost,lightgbm}/build.rs` (Phase-0 `JAIN_VENDOR_ROOT` change)
- `~/jain_small/deployment/ops/Dockerfile.sagemaker*` + `assemble-runtime-assets.sh` (COPY set `stage-context.sh` must reproduce)
- `~/jain_small/ops/ci/{fast,web-cockpit,python-client,examples,native,contracts,compression}.sh` (lane bodies to redistribute per §3.3)
- `~/jeryu-split/repos.manifest.toml` + `~/jankurai-split/repos.manifest.toml` (schema templates; optionally adopt jankurai's `gates`/`remotes` extensions)
