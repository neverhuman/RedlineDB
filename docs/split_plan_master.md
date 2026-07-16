# jain-split MASTER PLAN (converged, v2 — 2026-07-06)

**Status: authoritative.** This document supersedes `split_plan_claude.md`, `split_plan_codex.md`, and `split_plan_codex_redteam_claude.md` (kept for lineage). It is the single spec an implementing agent (or fleet of agents) executes.

- **Source monorepo**: `/home/ubuntu/jain_small` — forge slug `jeryu/jain`, origin `http://127.0.0.1:8787/git/jeryu/jain.git`, github mirror `http://127.0.0.1:8787/git/jeryu/jain.git`. Workspace version **7.0.1**. Seed commit **`cc27936eb45006bda0cae85b0f578f4d5985991d`** (branch `apex`). **The monorepo is READ-ONLY for this entire effort.**
- **Split root**: `/home/ubuntu/jain-split/` — 19 sibling repos + split tooling.
- **Central repo**: `/home/ubuntu/jain-split/jain` — the portal that brings the family together (manifest, family.lock, fleet CI, orchestration Justfile). At cutover it takes the `jeryu/jain` slug.
- **Family**: `repo_family = "jain-split"`, tag shape `<repo>-v7.0.1-split.N`, default branch `main` everywhere, forge owner `jeryu/*`, github mirror owner `neverhuman/*`.

---

## §0 Lineage and what was converged

Five review passes produced this plan: Claude 16-repo plan → Codex 19-repo plan → Claude red-team (6 critical findings) → Codex corrections (all accepted) → this convergence. The corrections locked in here:

1. `jain-contracts` is **not** canonical source — the schema is *generated from* `crates/feat-core/src/progress.rs` and proven by `progress_contract.rs`. It survives only as a **one-way published mirror** fed by jain-core.
2. `jain-artifacts` is **folded into `jain-starforge`** — `golden_parity.rs` skips silently when weights are absent, so weights must live with the inference code to keep the required lane honest. (`crates/starforge/src/model.rs:631-634` also hard-references `artifacts/foundation/…`.)
3. The **jeryu-precedent deploy `[patch]` exception is restored** — jain-deploy is the sole repo with committed local-sibling patches; the generated source bundle is used ONLY as the hermetic Docker build context.
4. The monorepo stays read-only, but split-only product shims come from **checked patch files** (`ops/split/patches/*.patch`), never inline materializer string-editing.
5. `ops/apex/**` gets one home: **`jain-ops`** (not jain-core, not scattered).
6. **`jain-release-ops` is dropped** — jain has no release-ops product surface (the jeryu repo of that name owns crates); split tooling lives in the portal/split root, lane bodies are materializer-generated per repo.
7. `python/ai-service` is **standalone `jain-python`** — its unit tests are fully independent; deploy keeps container integration.
8. `.jeryu/repo.toml` is generated with the **verified 6-key veox schema** only after confirming the live forge reads it (jeryu-split itself has none; the invented `[shadow_main]`/`[tag_mirror]` tables are gone; never copy veox's dead gitea `:2224` URL).

Open items carried to §11: live-forge `.jeryu/repo.toml` reader confirmation; GitHub LFS quota before mirroring jain-starforge; whether jain-model-zoo mirrors to GitHub at all.

## §1 Goals and constraints

1. **Maximize repo count without complicating testing or building.** Result: 19 repos.
2. **Every repo has its own CI and is fully testable with no sibling repos present.** This is a hard acceptance gate (Stage 5 runs every required lane in isolation, offline). Cross-repo tests exist but each has exactly one declared home (portal fleet lanes or deploy integration lanes) — they are additive evidence, never a repo's required check.
3. **AI can develop each repo.** Every repo ships a generated `AGENTS.md` development contract, a `test-map.json` with exact commands, mocks/fixtures committed in-repo, and a documented boundary of "change freely" vs "requires a family version bump".
4. **The full binary build is very straightforward** — one `just build` from the portal (§6).
5. **`/home/ubuntu/jain_small` is never modified.** Rollback at any stage = stop; the monorepo is untouched until the (reversible) forge-rename cutover.
6. Fresh-seed history (all three precedent families did this); monorepo history stays in the archived `jeryu/jain-monorepo`.

## §2 Topology — the 19 repos

### 2.1 Dependency DAG (tag-pinned cargo deps; `[opt]` = optional feature-gated)

```
jain-domain ────────────────┐
jain-math ──────────────────┼──► jain-core ──► jain-report ──┬─► jain-cli ──► jain-deploy
                            │        ▲   └───► jain-tui ─────┤        ▲
jain-math ─► jain-battle-gpu│        │                       │        │ [patch]-integrates
                        │   │  [opt] │                       │        │ every Rust repo
                        ├───┼────────┴── jain-catboost       │
                        │   │            jain-xgboost        │
                        │   │            jain-lightgbm       │
                        │   │            jain-starforge      │
                        └───┴──────────────────► jain-web ◄──┘ (core, report, battle-gpu)

non-cargo:  jain-contracts (mirror of jain-core:contracts/) ; jain-python ; jain-web:apps/web
            jain-model-zoo ; jain-ops ; jain-docs ; jain (portal)
```

### 2.2 Roster

| # | Repo | Profile | Wave | Contents (cargo_members ⊕ key copy_paths) | Tag-pinned deps | Gating |
|---|------|---------|------|--------------------------------------------|-----------------|--------|
| 1 | `jain` | public-portal | 0 | `repos.manifest.toml`, `family.lock`, `scripts/{clone-family,validate-family,fleet-ci,family-doctor,install}.sh`, orchestration Justfile, split docs (authored, no product source) | — | required |
| 2 | `jain-docs` | custom | 0 | `docs/**` (Typst manual, research writeups, product docs hub) | — | non-gating |
| 3 | `jain-domain` | rust-workspace | 0 | `crates/domain` | — | required |
| 4 | `jain-math` | rust-workspace | 0 | `crates/feat-math` (incl. committed `assets/champions/8944cbdf.dsl`) | — | required |
| 5 | `jain-contracts` | custom (mirror) | 0 | `contracts/**` byte-identical mirror + `MIRROR.md` provenance | — (content-synced from jain-core) | required |
| 6 | `jain-catboost` | rust-workspace | 1 | `crates/catboost-sys`, `crates/catboost`, `tests/cbtest.c`, `scripts/vendor.sh` | — | required |
| 7 | `jain-xgboost` | rust-workspace | 1 | `crates/xgboost`, `scripts/vendor.sh` | — | required |
| 8 | `jain-lightgbm` | rust-workspace | 1 | `crates/lightgbm`, `scripts/vendor.sh` | — | required |
| 9 | `jain-battle-gpu` | rust-workspace | 1 | `crates/battle-gpu` | jain-math | required |
| 10 | `jain-starforge` | rust-workspace +LFS | 1 | `crates/starforge`, **`artifacts/starforge` + `artifacts/foundation` (all 6 LFS safetensors)**, `.gitattributes` LFS allowlist, `scripts/{compress,prepare}-starforge-*.sh`, compression lane | — | required |
| 11 | `jain-core` | rust-workspace | 2 | `crates/feat-core`, **`contracts/` (canonical)**, `scripts/publish-contracts.sh` | jain-domain, jain-math; `[opt]` jain-catboost, jain-xgboost, jain-lightgbm, jain-starforge | required |
| 12 | `jain-report` | rust-workspace | 3 | `crates/feat-report` (incl. `templates/report.typ`) | jain-core, jain-math | required |
| 13 | `jain-tui` | rust-workspace | 3 | `crates/feat-tui` | jain-core | required |
| 14 | `jain-cli` | rust-workspace | 3 | `crates/feat-cli` (**bins `jain`, `jain-entrypoint`**) | jain-core, jain-math, jain-report, jain-tui, jain-battle-gpu | required |
| 15 | `jain-web` | custom (rust-node-hybrid) | 3 | `crates/feat-web` (bin `jain-web`), **`apps/web`**, `contracts/` (read-only copy), `db/` (store governance docs) | jain-core, jain-report, jain-battle-gpu (`default-features=false`) | required |
| 16 | `jain-python` | custom (python) | 3 | `python/ai-service`, `contracts/` (read-only copy) | — | required |
| 17 | `jain-model-zoo` | custom | 4 | `reference/ported/**` (486 standalone crates + committed `oracle/dump/*.npy`), `ops/parity/**` | — | non-gating |
| 18 | `jain-ops` | custom | 4 | `ops/apex/**` (jail-tools standalone crate, godmode campaigns, lanes/lib/engine), `agent/zyal/*.zyal`, `scripts/global_bench.*`, `scripts/manifest_reg*.txt` | — | non-gating |
| 19 | `jain-deploy` | deploy | 4 | `deployment/ops/**` (Dockerfiles, `assemble-runtime-assets.sh`, `starforge-artifacts.lock`+receipt, `container-bases.lock`, examples), `deployment/ops/sagemaker-ci` (cargo member), `deployment/product/` (anchor, authored), **committed `[patch]` for all 9 Rust sibling repos**, `scripts/{vendor-all.sh,stage-context.sh,get_data.sh,sync-base-digests.sh}` | jain-cli (anchor dep; + transitively everything via `[patch]`) | required |

`required_repos` = #1,3–16,19 (16 repos). `jain-docs`, `jain-model-zoo`, `jain-ops` are non-gating. Rollout waves order both seeding and CI bring-up: a wave's seeds (main + tag) are pushed to the forge **and GitHub** before the next wave's CI runs (§7.4) — this resolves the consumer-CI-needs-dependency-tags ordering.

**Layout rule (load-bearing):** every repo preserves **monorepo-relative layout** (`crates/feat-core` stays at `crates/feat-core`, `apps/web` at `apps/web`, `contracts/` at repo root). This is what lets `include_str!("../../contracts/…")` in `progress_contract.rs`, `--static-dir apps/web/dist`, `$ROOT/reference/ported/…` in parity scripts, and the Dockerfile `COPY` lines all work with **zero code edits**.

## §3 The central repo: `~/jain-split/jain`

The portal is the product’s front door and the family’s control plane. It contains **no product source** (jeryu invariant) but everything needed to discover, validate, assemble, and drive the family.

### 3.1 Contents

```
jain/
  README.md                  # family map, quickstart (clone → build → run)
  AGENTS.md                  # how AI agents work across the family (see §3.5)
  repos.manifest.toml        # THE manifest (schema in §12.1)
  family.lock                # generated pins: repo → tag/commit/required_check (§12.2)
  Justfile                   # orchestration UX (§3.2)
  scripts/
    clone-family.sh          # clone/update all siblings from forge origin (github as backup); `git lfs pull` only in jain-starforge
    validate-family.sh       # fleet validation checklist (§3.4)
    fleet-ci.sh              # run every repo's `just required` in a bounded pool; writes .ci-status/ dashboard (veox ci-status.sh pattern)
    family-doctor.sh         # forge health, remotes, LFS objects, caches, runner deps
    regen-family-lock.sh     # regenerate family.lock from actual sibling HEADs/tags — never hand-edited
    install.sh               # end-user install (downloads deploy's released artifacts)
  docs/                      # split docs (this file and lineage), family runbooks, bump playbook
  agent/ ops/ci/ .github/    # generated standard (like every repo)
```

### 3.2 Orchestration Justfile (the "brings it all together" UX)

Every target checks the needed sibling exists (else prints `run: just family-clone`), then delegates:

```
just family-clone      # scripts/clone-family.sh
just family-validate   # scripts/validate-family.sh          (fleet lane)
just family-ci         # scripts/fleet-ci.sh                 (fleet lane; red if any repo red)
just family-doctor     # environment/forge health
just build             # → ../jain-deploy: vendor-all (cached) + full `jain` binary build (§6.2)
just image             # → ../jain-deploy: stage-context + docker build (§6.4)
just web               # → ../jain-web:   pnpm build + jain-web release build (§6.3)
just lock-regen        # scripts/regen-family-lock.sh
```

### 3.3 Portal CI: required vs fleet

- **`jain/required`** (standalone — runs with NO siblings): manifest parses + schema-validates (`ops/split/manifest.sh` port); `family.lock` parses and every entry has name/tag/40-hex commit/required_check; shellcheck on `scripts/*.sh`; docs link check; `clone-family.sh --dry-run` (prints plan, no network).
- **Fleet lanes** (host-only, siblings symlinked by `split-host-ci.sh` with `JAIN_NEEDS_SIBLINGS=1`; non-required but release-blocking evidence in Stage 8):
  - `validate-family` — §3.4 checklist.
  - `contracts-sync` — sha256-compare `contracts/progress-event.schema.json` + `progress-events.jsonl` (+ `public-api.toml`) across **jain-core (canonical), jain-contracts (mirror), jain-web, jain-python** at `family.lock` commits.
  - `version-consistency` — every consumer Cargo.toml tag pin == family.lock tag; deploy `[patch]` section covers exactly the 9 Rust repos; all `VERSION` files agree; no two revs of any internal crate in deploy's `cargo metadata`.
  - `coverage-report` — re-run `splitctl source-coverage` against the seed (and post-reconcile) manifest.

### 3.4 `validate-family.sh` checklist (adopted from codex, hardened)

Manifest ↔ disk agreement (all 19 dirs exist, slugs match); every repo has the generated standard files (AGENTS.md, SPLIT.md, agent/owner-map.json, agent/test-map.json, agent/proof-lanes.toml, agent/audit-policy.toml, scripts/ci-local.sh, ops/ci/required.sh, thin workflow); **no `branch =` git deps anywhere**; **no cross-repo `path =` deps outside jain-deploy's `[patch]`**; every Rust repo has a committed root `Cargo.lock`; jain-web has `pnpm-lock.yaml`; workflow actions pinned to 40-char SHAs; remotes: `origin` = `http://127.0.0.1:8787/git/jeryu/<name>.git`, `github` = `http://127.0.0.1:8787/git/jeryu/<name>.git`; family.lock commits == sibling HEAD ancestry; LFS: jain-starforge safetensors are LFS-tracked AND each smudged file > 1 MB; TabPFN-free + banned-term policy files byte-identical across all 19.

### 3.5 Portal AGENTS.md (the AI-development contract, family level)

States: repo map + one-line purposes; "work in ONE repo per PR unless executing the bump playbook (§5.5)"; how to run any repo's CI (`just required` inside it); which changes ripple (feat-core public API → family bump; Event schema → contract playbook §5.1); where cross-repo evidence lives (portal fleet, deploy integration); environment gotchas (rtk 40-line stdout truncation — write long output to files; sandbox reaps backgrounded servers — drive servers via one foreground supervisor).

## §4 Repo-by-repo specification

Common to ALL repos (generated by the materializer, §12.4): the jankurai standard (`agent/*`, audit-policy pinned jankurai 1.6.10 / score ≥ 85 / 0 hard findings), `Justfile` → `ops/ci/{required,score,security}.sh` (+ repo-specific lanes), `scripts/ci-local.sh`, SHA-pinned thin `.github/workflows/ci.yml` (structural checks only — the real gate is the host runner), `SPLIT.md`, `VERSION`, seed commit `chore: seed jain split repo from cc27936…`, tag `<name>-v7.0.1-split.0`. Rust repos: root `Cargo.toml` replicating the monorepo's `[workspace.package]`, `[profile.*]` and lints, plus committed `Cargo.lock` (regenerated — the path→git rewrite always changes lock sources). Security lane (TabPFN-free scan + banned-term "fallback" audit) byte-identical in all 19.

Below, **REQ** = the `<repo>/required` check (runs on the forge host runner via `split-host-ci.sh`, must pass standalone), **LANES** = additional non-required/dispatch lanes, **TEST DATA** = committed mocks/fixtures, **AI NOTES** = development contract highlights.

### 4.1 `jain` (portal) — see §3.

### 4.2 `jain-docs`
- **REQ**: `ops/ci/docs-pdf.sh` — Typst User-Guide/manual builds from `docs/manual/`; markdown link check over `docs/`.
- **LANES**: none. **TEST DATA**: none needed (Typst sources are self-contained).
- **AI NOTES**: free to edit any doc; product-behavior claims must cite the owning repo; per-repo `docs/{architecture,testing,release}.md` stubs live in each repo, NOT here.

### 4.3 `jain-domain`
- **cargo_members**: `crates/domain` (serde-only error/repair-hint contract).
- **REQ**: `cargo fmt --check` + `cargo clippy --workspace --all-targets -- -D warnings` + `cargo test --workspace --locked`.
- **TEST DATA**: inline unit tests only. **AI NOTES**: this is a public error contract with 1 consumer (feat-core); any variant/field change is semver-relevant → family bump. Cheapest repo in the family; ideal canary for tooling changes.

### 4.4 `jain-math`
- **cargo_members**: `crates/feat-math`. Fully hermetic (anyhow/serde/rayon/sha2 only).
- **REQ**: fmt + clippy + `cargo test --workspace --locked --all-targets` — runs the full suite: `bench.rs`, `champion_parity.rs` (against the committed champion genome `crates/feat-math/assets/champions/8944cbdf.dsl`), `golden.rs`, `tokens.rs`.
- **TEST DATA**: the committed `.dsl` champion asset + golden fixtures (already in-crate).
- **AI NOTES**: the DSL grammar and emitters are consumed by feat-core and battle-gpu — grammar changes are breaking → family bump; new pure functions are free.

### 4.5–4.7 `jain-catboost`, `jain-xgboost`, `jain-lightgbm` (one pattern, three repos)
- **cargo_members**: catboost: `crates/catboost-sys` + `crates/catboost` (internal path dep stays — same repo); xgboost/lightgbm: single crate each. catboost also owns `tests/cbtest.c` (C-ABI smoke) and the `catboost-cbtool` bin.
- **Vendor**: `scripts/vendor.sh` = pinned upstream clone (catboost v1.2.10 / xgboost v2.1.4 / lightgbm v4.6.0) + that learner's section of the monorepo `prune_vendor.sh`, into `<repo>/vendor/<learner>` (gitignored). The build.rs **`JAIN_VENDOR_ROOT` shim** (§5.4) resolves `$JAIN_VENDOR_ROOT/<learner>` first, then legacy `../../vendor/<learner>`.
- **REQ** (host runner, honest — conceded: `cargo check/clippy/test` ALL execute build.rs, so there is no vendor-free cargo lane): `bash scripts/vendor.sh` (idempotent; host-cached clone ~free after first run) → `cargo fmt --check` → `cargo test --workspace --release` (release: matches how consumers build FFI). catboost adds: build `catboost-cbtool`, compile+run `tests/cbtest.c`, assert **zero Python interpreter invocations** during the build (existing native-lane guard).
- **Thin GitHub workflow** (structural only): fmt, `taplo`/manifest lint, shellcheck — never invokes a build script.
- **TEST DATA**: `tests/smoke.rs` generates synthetic training data inline; `cbtest.c`. No committed datasets needed.
- **AI NOTES**: these repos are cold (4–6 commits/3mo). The C-ABI adapter surface (`links = …`) is the contract; changing exported symbols → family bump. First vendor.sh run downloads ~1 GB — expected, cached thereafter.

### 4.8 `jain-battle-gpu`
- **cargo_members**: `crates/battle-gpu` (cudarc 0.19.8, **dynamic-loading** — default `gpu` feature compiles and tests on CPU-only hosts; `kernels.cu` via `include_str!` is intra-crate).
- **REQ**: fmt + clippy + `cargo test --workspace --locked --all-targets` (defaults on) + `cargo check --workspace --no-default-features`.
- **LANES**: `gpu-bench` (self-hosted GPU dispatch).
- **AI NOTES**: never assume a GPU in tests; the CPU path must stay green. Kernel-signature changes affect feat-cli/feat-web consumers → family bump.

### 4.9 `jain-starforge` (code + ALL weights)
- **cargo_members**: `crates/starforge` (candle 0.11 inference, bin `starforge`, feature `cuda`). **Owns `artifacts/starforge/` (4 safetensors, ~528 MB) + `artifacts/foundation/` (2 safetensors, ~214 MB) via LFS**, with the monorepo `.gitattributes` LFS allowlist lines carried over exactly.
- **REQ**: fmt + clippy + `cargo test -p starforge --all-targets` — **including `golden_parity.rs` with the in-repo weights** (the runner does `git lfs pull` in the worktree) — + LFS guard: `git lfs ls-files` covers all 6 files AND each smudged file > 1 MB (a pointer-only checkout makes golden parity skip silently — this guard makes that loud).
- **LANES**: `compression` (safetensors ↔ `.zst` round-trip, from `ops/ci/compression.sh`); `cuda` (self-hosted dispatch).
- **TEST DATA**: the weights themselves; golden-parity expected outputs in-crate.
- **AI NOTES**: weights are versioned product artifacts — replacing one requires: LFS add + golden-parity update + deploy's `starforge-artifacts.lock`/receipt bump (cross-repo → bump playbook). `ruzstd` keeps decompression pure-Rust; keep it that way.

### 4.10 `jain-core` (the hub)
- **cargo_members**: `crates/feat-core`. **Canonical `contracts/`** at repo root (schema, `progress-events.jsonl` fixture, `public-api.toml`, AGENTS/README). Deps: jain-domain, jain-math (tags); optional feature-gated: jain-catboost, jain-xgboost, jain-lightgbm, jain-starforge, candle. **Default features = empty** — the required lane builds no FFI, no CUDA, no weights.
- **REQ**: fmt + clippy + `cargo test -p feat-core --no-default-features --locked -- --skip apex` + explicitly `--test artifact --test model_artifacts --test progress_contract --test pipeline_mock --test api_contract --test router`. All pass with no vendor, no weights, no siblings: weight-hungry tests (`foundation.rs`, `hyperion_v7_weights.rs`) skip gracefully by design; `artifact.rs` builds temp artifacts; `model_artifacts.rs` is a git-side LFS-pointer format guard (trivially green here); `progress_contract.rs`'s `include_str!("../../contracts/…")` resolves against the in-repo canonical copy.
- **LANES**: `rust-api` (cargo public-api / semver-checks drift vs `contracts/public-api.toml` — the early-warning gate for the family's hottest API); `coverage-api`. Weight/learner-integrated runs happen in **deploy's** `native-integration` lane (§4.19), never here.
- **Also owns** `scripts/publish-contracts.sh` — copies `contracts/` byte-identically into a `jain-contracts` checkout and opens the mirror PR (§5.1).
- **TEST DATA**: `contracts/progress-events.jsonl`; `tests/apex_datasets.txt` (external-path dataset list — apex tests are `--skip`ped in REQ and run only in deploy/ops dispatch lanes); proptest strategies in `properties.rs`.
- **AI NOTES**: the most active crate (20 commits/3mo). Anything visible in `cargo public-api` or in the `Event` struct is contract: schema/Event changes follow §5.1; other API changes follow §5.5. Internal algorithm work (GP synthesis, selection, ensembling) is free as long as `api_contract.rs` and `rust-api` stay green.

### 4.11 `jain-report`
- **cargo_members**: `crates/feat-report` (deps: jain-core, jain-math; `templates/report.typ` is intra-crate `include_str!`).
- **REQ**: fmt + clippy + `cargo test --workspace --locked`. **LANES**: `report-pdf` (renders the executive bundle with the host's Typst).
- **AI NOTES**: consumes feat-core's artifact API read-only; template changes are free; new data requirements from core → family bump ordering (§5.5).

### 4.12 `jain-tui`
- **cargo_members**: `crates/feat-tui` (ratatui; dep: jain-core).
- **REQ**: fmt + clippy + `cargo test --workspace --locked`. **AI NOTES**: pure presentation layer; snapshot-style unit tests; no terminal needed in CI.

### 4.13 `jain-cli`
- **cargo_members**: `crates/feat-cli` — **the product binaries `jain` and `jain-entrypoint`** (lib + bins; main.rs calls `feat_cli::run()`). Deps (tags): jain-core, jain-math, jain-report, jain-tui, jain-battle-gpu. Default features `catboost,xgboost,lightgbm` (FFI → needs vendor → NOT built in REQ).
- **REQ**: fmt + clippy + `cargo check -p feat-cli --no-default-features` + `cargo test -p feat-cli --no-default-features --features ci-smoke --all-targets` (runs `cli_contract.rs` against the mock backend).
- **LANES**: none native here — the full-featured `jain` binary is built and exercised in **deploy** (§4.19, §6.2). **TEST DATA**: ci-smoke mock backend (feature-gated in feat-core), `cli_contract.rs` fixtures.
- **AI NOTES**: CLI surface (`args.rs`, exit codes, `serve` HTTP contract for SageMaker `/ping` + `/invocations`) is the product contract — changes need `cli_contract.rs` updates and are release-noted. Known wart that travels as-is: hardcoded `/home/ubuntu/*` default paths in `args.rs` (dev-only defaults; post-split cleanup ticket, do NOT fix during the split — no-behavior-change rule).

### 4.14 `jain-web`
- **Contents**: `crates/feat-web` (bin `jain-web`: axum 0.7 + bundled rusqlite + ws) ⊕ `apps/web` (React/Vite/pnpm cockpit; `apps/web/scripts/dev.mjs` spawns `cargo run -p feat-web`) ⊕ `contracts/` read-only copy ⊕ `db/` (SQLite store governance docs — the store itself is `feat-web/src/store.rs`, data under `--data-dir`). Deps (tags): jain-core, jain-report, jain-battle-gpu (`default-features = false`, preserved by the rewriter).
- **REQ** = today's `web-cockpit.sh`, unchanged in spirit: `cargo test -p feat-web --no-default-features --features ci-smoke` (**never default features on a hosted/CPU lane — defaults include `hyperion-cuda`**; a generated workflow-lint asserts no default-features feat-web build appears in CI) → `pnpm --dir apps/web install --frozen-lockfile` → typecheck → vitest (~30 unit files incl. `protocol.contract.test.ts` against the local `contracts/` copy) → `pnpm build` → Playwright **mocked** e2e (`apps/web/e2e/`) → Playwright **live** e2e (`apps/web/e2e-live/`): spawns the compiled **ci-smoke** `jain-web` binary (scripted mock GP — no FFI, no vendor, no GPU) and drives real HTTP+WS.
- **TEST DATA**: ci-smoke scripted-GP mock; Playwright mocked fixtures; `contracts/` copy registered in `agent/generated-zones.toml` as `read_only = true, write_policy = "synced_from_jain-contracts"`.
- **AI NOTES**: the `web` boundary spans Rust and TS on purpose — UI + backend change together in one PR here. Never edit `contracts/` or `src/generated/progressEvent.ts` by hand (synced; §5.1). Dev server: `pnpm --dir apps/web dev` (`JAIN_WEB_DEV_SMOKE=1` for the mock backend). Environment gotcha: this sandbox reaps backgrounded servers — drive `jain-web` via one foreground supervisor process in tests.

### 4.15 `jain-python`
- **Contents**: `python/ai-service` (`jain-sagemaker` SDK v7.0.1, hatchling; transport-only — shells `docker run …` and speaks HTTP; requests/tqdm, optional boto3/mcp/ipywidgets) ⊕ `contracts/` read-only copy.
- **REQ**: `pip install -e .[dev,aws]` → `ruff check` → `pytest` — all six suites run hermetically: `test_aws.py` (**moto** AWS stubs), `test_contract.py` (validates the local `contracts/` copy; `EVENT_SCHEMA_VERSION` assert), `test_csvio.py`, `test_endpoint.py`, `test_no_banned_terms.py`, `test_progress.py`. No Docker, no network.
- **LANES**: none here — `python-client-vs-container` (SDK against the real image) lives in **deploy**.
- **AI NOTES**: Python must never own pipeline truth (boundary: transport only). Event parsing mirrors (`contract.py`, `progress/events.py`) change only via §5.1.

### 4.16 `jain-contracts` (published mirror — NOT a source)
- **Contents**: byte-identical mirror of jain-core's `contracts/` + `MIRROR.md` (provenance: source repo/tag/commit, sync procedure, "PRs that hand-edit contract files will be closed").
- **REQ** (standalone, no network): JSON/TOML parse; internal version consistency — `schema.version == public-api.toml schema_version == every fixture line's .v == version recorded in MIRROR.md`; jq structural sanity on every fixture line against the schema's required keys.
- **Sync**: only via jain-core's `scripts/publish-contracts.sh` PRs. Cross-repo freshness enforced by the portal `contracts-sync` fleet lane (a lagging mirror turns it red).
- **AI NOTES**: read-only by policy. External consumers pin this repo's tags to get the event contract without cloning jain-core.

### 4.17 `jain-model-zoo`
- **Contents**: `reference/ported/**` — 486 standalone frozen crates (72% of the monorepo's tracked files — moving them out is the biggest clone-weight win of the split) with committed `oracle/dump/*.npy` fixtures — ⊕ `ops/parity/**` (harness resolves `$ROOT/reference/ported/…`; same-repo monorepo-relative layout ⇒ zero path edits).
- **REQ** (integrity, NOT 486 builds): generated `ops/ci/integrity.sh` — every `reference/ported/*/Cargo.toml` parses; committed oracle `.npy` checksums match a generated `oracle-checksums.txt`; parity-harness shellcheck.
- **LANES**: `native-spot` (dispatch): `cargo test --manifest-path reference/ported/catboost/Cargo.toml --release` + same for `tabicl` — the two crates today's `agent/test-map.json` wires into native testing (those test-map entries regenerate ONLY into this repo). `parity` (manual, dev-only — needs external `remote_super` datasets, exactly as today).
- **AI NOTES**: frozen reference material; new ports = new folders + oracle fixtures + checksum regen. Never referenced by any product repo's build or tests.

### 4.18 `jain-ops`
- **Contents**: `ops/apex/**` in one piece (jail-tools standalone workspace at `ops/apex/tools` — 7 bins; godmode drive scripts, `lanes/`, `lib/`, `engine/`, `experiments/`, campaign configs), `agent/zyal/*.zyal`, `scripts/global_bench.*`, `scripts/manifest_reg*.txt`.
- **REQ**: `cargo test --manifest-path ops/apex/tools/Cargo.toml` + `cargo fmt/clippy` on jail-tools + shellcheck across the campaign scripts.
- **AI NOTES**: research campaigns drive a built `jain` binary and external datasets — they run manually against a deploy-built binary (document `JAIN_BIN=~/jain-split/jain-deploy/target/release/jain`), never in CI. The scripts' `ROOT="$(cd "$OPS/../.." && pwd)"` resolution still works (monorepo-relative layout).

### 4.19 `jain-deploy` (release authority + integration home)
- **Contents**: `deployment/ops/**` (both Dockerfiles, `assemble-runtime-assets.sh` with its Python-free rootfs guard, `starforge-artifacts.lock` + `.receipt.json`, `container-bases.lock`, `examples/examples.json`), cargo members `deployment/ops/sagemaker-ci` + authored `deployment/product/` (anchor package `jain-product`, `publish = false`, whose sole dependency is `feat-cli = { git = …jain-cli.git, tag = … }` — it puts feat-cli into deploy's graph so `cargo build -p feat-cli` works); **committed `[patch."http://127.0.0.1:8787/git/jeryu/<X>.git"]` sections for all 9 Rust repos** → `../jain-<x>/crates/<crate>` (`agent/boundaries.toml`: `local_path_patches = true` here ONLY); a committed monorepo-shaped workspace template `deployment/stage/Cargo.workspace.toml` (verbatim copy of the monorepo root Cargo.toml) + `Cargo.lock` for the Docker bundle; scripts `vendor-all.sh`, `stage-context.sh`, `get_data.sh`, `sync-base-digests.sh`.
- **REQ** (standalone, no siblings, no vendor): fmt + clippy + `cargo test -p sagemaker-ci`; `stage-context.sh --plan` (parses family.lock + prints the staging plan, offline); lock/receipt schema validation.
- **INTEGRATION LANES** (host, `JAIN_NEEDS_SIBLINGS=1` + `JAIN_NEEDS_ARTIFACTS=1`; dispatch/cron + Stage-8 release evidence):
  - `full-binary` — §6.2 build + `jain --version` + `jain demo` smoke.
  - `native-integration` — feat-core × real learners: `cargo test -p feat-core --features catboost,xgboost,lightgbm,starforge-cpu` with `JAIN_VENDOR_ROOT` set and `artifacts/` symlinked (runs the weight-hungry `foundation.rs`/starforge integration tests that skip in jain-core's REQ).
  - `examples` — every documented `jain` example against the image; docs parity.
  - `python-client-vs-container` — build image, `pip install` the sibling jain-python, run SDK/CLI/notebook against the container.
  - `sagemaker-local` / `sagemaker-gpu` (self-hosted) / `sagemaker-aws` (cron; attestation + cosign as today).
  - `coverage-full` (cron), `version-consistency` contribution (its `version-consistency.sh` now checks Dockerfiles/changelog/docs *within deploy*; the family-wide check is the portal's).
- **AI NOTES**: the ONLY repo allowed committed sibling paths. Its PRs are integration PRs by nature; its required lane stays cheap so day-to-day deploys aren't blocked on 30-minute lanes.

## §5 Cross-cutting systems

### 5.1 Contract governance (EVENT_SCHEMA_VERSION, currently v9)

Canonical chain (one direction, never reversed):

```
crates/feat-core/src/progress.rs  (producer struct + EVENT_SCHEMA_VERSION const)
        │  same-repo, same-PR, guarded by progress_contract.rs
        ▼
jain-core:contracts/{progress-event.schema.json, progress-events.jsonl, public-api.toml}
        │  scripts/publish-contracts.sh → mirror PR
        ▼
jain-contracts (published mirror, byte-identical)
        │  consumer sync (copy + regen)
        ▼
jain-web:contracts/ + src/protocol.ts + src/generated/progressEvent.ts
jain-python:contracts/ + contract.py + progress/events.py
```

**Bump playbook** (goes in the portal docs verbatim):
1. **PR1 → jain-core**: change `Event`/const + regenerate schema + fixture, all atomic; `progress_contract.rs` proves keys==schema; tag `jain-core-v7.0.1-split.(N+1)`.
2. **PR2 → jain-contracts**: run core's `publish-contracts.sh`; mirror REQ validates internal consistency; tag.
3. **PR3 → jain-web** and **PR4 → jain-python** (parallel): sync `contracts/` copy + bump the language constant + regen `progressEvent.ts` + bump the jain-core tag pin — each in ONE PR so no green-but-skewed window exists (their own contract tests enforce copy==constant).
4. **PR5 → jain-deploy/portal**: `regen-family-lock.sh`; portal `contracts-sync` fleet lane must return green.

Skipping any step turns `contracts-sync` red at the next fleet run. Consumer copies are declared in `agent/generated-zones.toml` (`read_only = true`) so the jankurai audit flags hand edits.

### 5.2 Artifacts & LFS (where the weights happen)

```
git home            dev checkout             CI                      Docker build             runtime
jain-starforge  ──► clone-family does    ──► split-host-ci hook  ──► stage-context.sh    ──► /opt/jain/{starforge,
artifacts/{starforge, `git lfs pull` here     JAIN_NEEDS_ARTIFACTS=1   copies ../jain-starforge/  foundation}/... (primary);
foundation}/*.safetensors  only               ln -s $SPLIT_ROOT/jain-  artifacts@family.lock      repo-relative artifacts/
(6 files, ~742 MB, LFS)                       starforge/artifacts →    into .stage/ so existing   fallback for dev
                                              $worktree/artifacts      COPY lines work
```

Guards at every hop: materializer **LFS-smudges** after `git archive` (which emits 134-byte pointers) and asserts each `.safetensors` > 1 MB; jain-starforge REQ re-asserts (pointer-only checkout ⇒ loud failure, not silent golden-parity skip); `starforge-artifacts.lock` + receipt (in deploy) pin sha/size for image assembly; `assemble-runtime-assets.sh`'s Python-free rootfs guard is unchanged. **No `JAIN_ARTIFACT_ROOT` in v1** — the existing `/opt/jain → repo-relative` resolution plus the CI symlink hook covers every consumer; adding the env var later is a normal reviewed PR, not a materializer injection.

### 5.3 Vendor & native FFI

- The three learner `build.rs` resolve `CARGO_MANIFEST_DIR/../../vendor/<name>` and `.expect()` — fatal when consumed from `~/.cargo/git/checkouts`. The **shim** (§5.4 patch files) prepends: `JAIN_VENDOR_ROOT` env lookup (`$JAIN_VENDOR_ROOT/<learner>`) with `cargo:rerun-if-env-changed=JAIN_VENDOR_ROOT`, legacy path preserved.
- Vendor population: per-learner `scripts/vendor.sh` (pinned upstream clone + that learner's prune section); deploy `scripts/vendor-all.sh` → `jain-deploy/vendor/{catboost,xgboost,lightgbm}` + exports `JAIN_VENDOR_ROOT`; Docker: `ENV JAIN_VENDOR_ROOT=/build/vendor`, vendor staged by `stage-context.sh`. All vendor trees gitignored; host runner keeps a shared cache (`~/.cache/jain-split/vendor/`) that vendor.sh hardlinks from.
- **No required lane anywhere runs cargo against an FFI crate without vendor** (clippy/check/test all execute build.rs). Learner REQ = vendor(cached)+test on the host runner; core/cli/web REQ run learner-feature-free; deploy REQ is vendor-free (sagemaker-ci only).

### 5.4 Checked patch files (split-only shims; monorepo stays read-only)

- Location: `~/jain-split/ops/split/patches/` — exactly three files: `catboost-sys-vendor-root.patch`, `xgboost-vendor-root.patch`, `lightgbm-vendor-root.patch` (unified diffs against `cc27936`, ~10 lines each, additive-only: env lookup before legacy path).
- Materializer applies with `git apply --check` first (hard fail on fuzz), records applied patches in each repo's `SPLIT.md`, and a **tooling self-test** (Stage 1) asserts clean application against the seed tree.
- `reconcile.py` re-applies patches after each 3-way merge pull; if upstream touches a patched file, the conflict lands in `dirty/merge-work/` for a human — expected and bounded (3 cold files).
- Policy: NO other product-code patches. Anything more belongs in a normal PR to the split repos after seeding (they are the living source once cutover completes).

### 5.5 Versioning, tags, and the family bump

- `dependency_tag_suffix = "v7.0.1-split.N"`; per-repo `current_tag = "<name>-v7.0.1-split.N"`; **tags are immutable after Stage-7 announce** — never force-move (cargo caches by tag); recut = bump N.
- `bump-family-version.py` (forked from jeryu) is the ONLY way to bump: rewrites workspace versions, every consumer tag pin, deploy `[patch]`+anchor pin, manifest, family.lock, VERSIONs, CHANGELOGs — in dependency wave order **math/domain → learners/starforge/battle-gpu → core → report/tui/web/cli/contracts/python → deploy → portal lock regen**. Hand-editing a single pin is how you get two feat-core revs in deploy's graph and a `[patch]` unification failure — the portal `version-consistency` lane exists to catch exactly that.
- `[skip-version]` commit marker suppresses the forge autoversioner (jeryu convention), used by all generated baseline commits.

### 5.6 Cross-repo development (day-to-day, without breaking independence)

- Every consumer repo gets generated `ops/dev/local-patches.example.toml`: copy into `.cargo/config.toml` (gitignored) to `[patch]` github URLs → `../jain-<x>` siblings while iterating against unreleased deps. **Never committed** — CI never sees it; committed manifests stay tag-pinned. jain-deploy remains the only repo with committed patches.
- Multi-repo features follow the bump playbook (§5.1/§5.5); single-repo work needs nothing special. This is the line that makes "AI develops each repo" safe: an agent inside one repo cannot silently depend on sibling state.

## §6 The global build story

### 6.1 One-time setup

```bash
git clone http://127.0.0.1:8787/git/jeryu/jain.git ~/jain-split/jain
cd ~/jain-split/jain
just family-clone          # clones the 18 siblings from the forge (github as backup); LFS pull only in jain-starforge
just family-doctor         # forge health, remotes, LFS objects, runner deps (pnpm, playwright, typst, docker, git-lfs)
```

### 6.2 The full `jain` binary (defaults: CatBoost + XGBoost + LightGBM)

```bash
cd ~/jain-split/jain && just build
# ≡ cd ../jain-deploy
#   bash scripts/vendor-all.sh                        # cached; exports JAIN_VENDOR_ROOT=$PWD/vendor
#   cargo build --release -p feat-cli                 # anchor puts feat-cli in the graph; committed [patch]
#                                                     # resolves every internal crate to ../jain-*/crates/*
# → jain-deploy/target/release/{jain,jain-entrypoint}
```

Fallback mechanics if `-p feat-cli` bin-building through the anchor misbehaves on the pinned toolchain (verified as a Stage-1 spike, §9): `cargo build --release --manifest-path ../jain-cli/Cargo.toml -p feat-cli` from deploy's directory with deploy's `.cargo/config.toml` carrying the `[patch]` set and `CARGO_TARGET_DIR=$PWD/target`. Either way: **one `just build`, binary lands in `jain-deploy/target/release/jain`.**

### 6.3 The web cockpit

```bash
cd ~/jain-split/jain && just web
# ≡ cd ../jain-web
#   pnpm --dir apps/web install && pnpm --dir apps/web build
#   cargo build --release -p feat-web --no-default-features \
#         --features catboost,xgboost,lightgbm,hyperion-cpu     # GPU host: plain --release (defaults incl. hyperion-cuda)
#   (native learner features need JAIN_VENDOR_ROOT=~/jain-split/jain-deploy/vendor)
./target/release/jain-web --static-dir apps/web/dist            # serves the cockpit; SQLite store under --data-dir
```

Dev loop stays `pnpm --dir apps/web dev` (spawns `cargo run -p feat-web`; `JAIN_WEB_DEV_SMOKE=1` for the mock GP).

### 6.4 The SageMaker image (the shipping artifact)

```bash
cd ~/jain-split/jain && just image
# ≡ cd ../jain-deploy && bash scripts/stage-context.sh && docker build -f deployment/ops/Dockerfile.sagemaker.gpu .stage
```

`stage-context.sh`: reads `family.lock` → `git archive` each Rust sibling **at its locked commit** into `.stage/` in monorepo shape (`crates/*`, `deployment/ops`, `contracts`) → drops in the committed `deployment/stage/Cargo.workspace.toml` + `Cargo.lock` (verbatim monorepo workspace — since the bundle is monorepo-shaped, the original path-dep workspace works unchanged; **no synthesized manifests**) → `vendor-all` into `.stage/vendor` → copies `../jain-starforge/artifacts/{starforge,foundation}` (sha/size-checked against `starforge-artifacts.lock`). The existing Dockerfile `COPY crates/ vendor/ artifacts/ …` lines and the Python-free rootfs guard run **byte-for-byte unchanged**. Signing/attestation (cosign, provenance) as today.

### 6.5 Artifact map (what lives where)

| Artifact class | Git home | Build-time | Runtime |
|---|---|---|---|
| Model weights (6 safetensors, LFS) | jain-starforge `artifacts/` | CI symlink hook; Docker `.stage/artifacts` | `/opt/jain/{starforge,foundation}` primary, repo-relative fallback |
| Vendored C/C++ (catboost/xgboost/lightgbm) | none (gitignored) | per-repo `vendor.sh`, deploy `vendor-all.sh`, host cache | compiled into the binary |
| `jain` / `jain-web` binaries | none | `jain-deploy/target/release/`, `jain-web/target/release/` | installed / image `ENTRYPOINT` |
| SageMaker image | registry (`image.neverhuman.org/...`), cosign-signed | deploy `just image` | SageMaker / local docker |
| Cockpit static bundle | none (built) | `jain-web/apps/web/dist` | served by `jain-web --static-dir` |
| Contract schema+fixture | jain-core (canonical), jain-contracts (mirror), copies in web/python | — | validated in 3 languages |
| Build caches | — | per-repo persistent `CARGO_TARGET_DIR`, sccache, pnpm store, cargo git cache, vendor cache (host runner) | — |

## §7 CI architecture

### 7.1 Two tiers per repo

1. **Thin GitHub workflow** (`.github/workflows/ci.yml`, SHA-pinned actions): structural checks that never execute build scripts — fmt, manifest/TOML lint, shellcheck. Exists so the GitHub mirrors aren't blind.
2. **The real gate: `<repo>/required` on the forge host runner** via `split-host-ci.sh <owner> <repo> <sha> <path> [check]`: verify the protected control-plane identity → governed worker count (`jeryu-ci-governor`) → create an automatically removed `--no-local --no-checkout` physical exact-SHA clone beneath the split root → physically clone only explicitly required siblings and copy artifact inputs without links → run the required and release lanes → root-seal the proof → publish and read back `jankurai/proof`, `<repo>/required`, and the matching commit status in strict order through the typed fixed-origin Rust transport.
3. **Autonomous loop** (post-rollout): fork `~/veox-split/jeryu-ctl/jeryu-poll.sh` — per onboarded repo: open PR → host-ci → gated FF-merge → `mirror_github_main`.

### 7.2 Forge primitives (all proven; `jeryu onboard` is dry-run only)

Repo create `POST $JERYU_BASE/repos {"name":…, "private":true, "default_branch":"main"}` (409/422 = exists — **never overwrite; list first** `jeryu forge repo list --owner jeryu --json`, stop on unexpected collisions); family registration `PATCH /api/v1/repos/jeryu%2F<name> {"family":"jain-split"}` + verify via `GET /api/v1/repos?host=jeryu` facets; branch protection `PUT …/branches/main/protection {"required_status_checks":["<repo>/required"], "required_linear_history":true, "enforce_admins":true}`; optional PR-only `pre-receive` hook on `main` (veox `jeryu-ctl/hooks/pre-receive`).

### 7.3 Runner prerequisites (preflight-checked by `family-doctor.sh`)

git-lfs; docker; pnpm + node; `npx playwright install` browsers; typst; python3 (+pip, ruff, pytest, moto, jsonschema); jankurai 1.6.10; sccache; disk budget (19 persistent target dirs — cleanup policy: `cargo sweep`-style prune of target dirs idle > 30 days; the monorepo's 171 GB target dir is NOT copied anywhere); per-repo GitHub deploy keys or a forge-side mirror credential (the monorepo's `NEVERHUMAN_DEPLOY_KEY` model, multiplied — provision in Stage 0).

### 7.4 Seed/mirror ordering (kills the tag chicken-and-egg)

Consumer required lanes fetch cargo git deps from `http://127.0.0.1:8787/git/jeryu/<repo>.git` tags. Therefore: **each wave's seed `main` + tag is pushed to the forge AND GitHub immediately at registration (Stage 6), before the next wave's CI ever runs.** Seeds are pre-review bootstrap commits — pushing them unreviewed is the precedent (fresh-seeded families). `mirror_github_main` gating applies to post-seed merges only. Before any push exists at all, Stage-5 validation uses local mirrors (§8.3).

## §8 Testing matrix

### 8.1 Standalone (required) — every box runs with zero siblings

| Repo | Required lane (host runner) | Mocks / support data (committed in-repo) |
|---|---|---|
| jain | manifest+lock lint, shellcheck, clone dry-run | — |
| jain-docs | Typst manual build, link check | Typst sources |
| jain-domain | fmt+clippy+test | inline units |
| jain-math | full test suite | `assets/champions/8944cbdf.dsl`, golden fixtures |
| jain-contracts | parse + internal version consistency | schema + fixture (mirrored) |
| jain-catboost/-xgboost/-lightgbm | vendor(cached) → `cargo test --workspace --release` (+cbtool/cbtest.c; zero-Python guard) | synthetic in-test data; cbtest.c |
| jain-battle-gpu | test (gpu default, CPU-safe) + no-default check | inline units |
| jain-starforge | full tests **incl. golden_parity** + LFS pointer/size guard | **weights in-repo (LFS)**, golden outputs |
| jain-core | no-default tests `--skip apex` + artifact/model_artifacts/progress_contract/pipeline_mock/api_contract/router | ci-smoke mock, proptest, `contracts/` canonical, apex_datasets.txt (skip-listed) |
| jain-report | fmt+clippy+test | `templates/report.typ` |
| jain-tui | fmt+clippy+test | inline units |
| jain-cli | no-default check + ci-smoke tests (`cli_contract.rs`) | ci-smoke mock backend |
| jain-web | ci-smoke rust tests + vitest + build + Playwright mocked + **live vs ci-smoke binary** | scripted mock GP, Playwright fixtures, `contracts/` copy |
| jain-python | ruff + pytest (moto, contract, csvio, endpoint, progress, banned-terms) | moto stubs, `contracts/` copy |
| jain-model-zoo | manifest integrity walk + oracle checksums | committed `oracle/dump/*.npy` |
| jain-ops | jail-tools tests + shellcheck | — |
| jain-deploy | sagemaker-ci tests + stage-context `--plan` + lock schema | `examples/examples.json` |

### 8.2 Cross-repo — each test has exactly one home

| Cross-repo concern | Home | Lane |
|---|---|---|
| Contract lockstep across core/mirror/web/python | portal | `contracts-sync` (fleet) |
| Tag-pin/lock/VERSION coherence, single-rev graph | portal | `version-consistency` (fleet) |
| Family completeness vs manifest | portal | `validate-family`, `coverage-report` (fleet) |
| feat-core × real learners × weights | deploy | `native-integration` (dispatch + Stage-8) |
| Full `jain` binary + demo smoke | deploy | `full-binary` |
| SDK ↔ container, examples ↔ image, SageMaker contracts | deploy | `python-client-vs-container`, `examples`, `sagemaker-*` |
| Reference-port spot builds | model-zoo | `native-spot` (dispatch) |
| GPU/CUDA proof | battle-gpu / starforge / deploy | self-hosted dispatch/cron |

### 8.3 The independence acceptance gate (Stage 5)

For each repo, in a scratch `HOME` with **no siblings and no network**: local bare mirrors of all 19 seeds (materializer byproduct) + `git config url."file://$MIRRORS/<repo>.git".insteadOf "http://127.0.0.1:8787/git/jeryu/<repo>.git"` + cargo `net.git-fetch-with-cli = true` → run the full required lane. **19/19 green in isolation is the definition of "each repo fully testable without the others."** Any repo that can't pass here has a boundary bug — fix the boundary, don't add a sibling dependency.

## §9 Rollout stages

Every stage has explicit exit criteria; an agent should not proceed past a stage until they hold.

**Stage 0 — Preflight** (read-only). Verify: monorepo clean at `cc27936`; forge `/api/v1/version` healthy; `jeryu forge repo list --owner jeryu --json` → record any `jain-*` slug collisions (STOP on unexpected ones); LFS objects present locally (`git lfs ls-files` + smudge test on one file); runner deps (§7.3); GitHub mirror credentials plan (19 repos). *Exit*: preflight report at `~/jain-split/ops/split/preflight-report.md`.

**Stage 1 — Fork tooling.** Copy `~/jeryu-split/ops/split/{splitctl materialize, manifest.sh, splitctl source-coverage, reconcile.py, register-family.sh, rollout-pr-flow.sh, commit-baseline.sh, cutover.sh, closeout-prs.sh, bump-family-version.py}` + `ops/ci/split-host-ci.sh` + `~/veox-split/jeryu-ctl/{onboard.sh,lib.sh,jeryu-poll.sh,hooks/pre-receive}` into `~/jain-split/ops/`. Materializer deltas: (i) deploy-name `jain-deploy` in patch-section rendering; (ii) **LFS smudge step** post-archive + >1 MB assertion; (iii) **patch-file application** (`git apply --check` then apply; record in SPLIT.md); (iv) new profile renderers `rust-node-hybrid`, `python`, `mirror`, `custom`; (v) lane templates seeded from jain's actual `ops/ci/*.sh` (fast/web-cockpit/python-client/compression/native/contracts), not jeryu's; (vi) `split-host-ci.sh` hooks `JAIN_NEEDS_ARTIFACTS` / `JAIN_NEEDS_SIBLINGS` + LFS-aware worktrees; (vii) local-bare-mirror emission for Stage 5. **Spike test** (scratch dir): anchor-package `[patch]` mechanics — confirm `cargo build -p feat-cli` builds bins of a patched git dep on the pinned toolchain; record result, select §6.2 primary or fallback. *Exit*: tooling self-tests green (`pytest ops/split/tests`: archive+smudge on a fixture, patch application against `cc27936`, manifest round-trip); spike documented.

**Stage 2 — Author patches + templates.** Write the 3 vendor-shim patches (§5.4); write per-profile templates (AGENTS.md incl. per-repo AI notes from §4, SPLIT.md, Justfile, required.sh bodies, thin workflow, `local-patches.example.toml`, jain-deploy anchor + stage template). *Exit*: `git apply --check` passes for all patches against the seed tree; template render golden-tests green.

**Stage 3 — Manifest + coverage.** Author `~/jain-split/repos.manifest.toml` (19 `[[repo]]` entries per §2.2 + §12.1). Coverage config: multi-assignment reasons (contracts copies in core/mirror/web/python; per-learner prune extracts) + `retired_paths` with reasons (monorepo `agent/**` → regenerated per-repo; `.github/**`, root `Justfile`, `ops/ci/**`, `scripts/ci-*.sh`, `tools/security-lane.sh` → regenerated/redistributed; `tests/__pycache__/*` → stray; `verify_backup/`, `vendor/`, `target/` → untracked anyway). *Exit*: `manifest.sh` green; `splitctl source-coverage` reports **all 4,388 tracked files** assigned or retired-with-reason, zero unexplained multi-assignments.

**Stage 4 — Materialize.** Run the materializer: 19 fresh `git init -b main` seeds at monorepo-relative layout; dep rewrite path→pinned tags (preserving `optional` / `default-features` / `features` — required by feat-core's optional learners and feat-web's `default-features=false` battle-gpu); patches applied; standards generated; per-repo `Cargo.lock` regenerated; seed commits + annotated tags; local bare mirrors emitted; remotes configured (not pushed). *Exit*: 19 clean `git status`; all 6 safetensors real (>1 MB) and LFS-clean in jain-starforge's index; tags exist; mirrors exist.

**Stage 5 — Standalone validation (THE independence gate).** §8.3 procedure for all 19; also run `jankurai audit` locally per repo (score ≥ 85; tune per-repo `audit-policy.toml` only with an explicit note in SPLIT.md — never weaken the family policy files). Fix-and-rematerialize loops are cheap here (nothing is pushed; tags may be recut freely until Stage 7). *Exit*: **19/19 required lanes green offline with no siblings**; 19/19 audits pass.

**Stage 6 — Forge registration + seed push.** Per wave 0→4: `POST /repos` (collision-safe), push `main`+tag to forge origin; push seed+tag to GitHub mirror (LFS push for jain-starforge only — confirm quota first, §11); `PATCH family=jain-split`; branch protection + `<repo>/required`; optional pre-receive PR-only hooks. Then `register-family.sh --verify` (every repo exists, family matches, facets list `jain-split`). *Exit*: 19 forge repos live + family-tagged; GitHub mirrors seeded per wave; portal `family-doctor` green.

**Stage 7 — Wave rollout PRs.** Per wave: one trial PR per repo (whitespace-level) driven through `split-host-ci.sh`; required check posts green; gated merge; enable `mirror_github_main`. After the last wave: **tags frozen** (immutable policy active). *Exit*: 19/19 trial PRs merged with green `<repo>/required`; mirrors following.

**Stage 8 — Fleet proof (release evidence).** Portal: `validate-family`, `contracts-sync`, `version-consistency`, `coverage-report` all green; `regen-family-lock.sh` → commit final `family.lock`. Deploy: `full-binary` (`jain --version`, `jain demo`), `native-integration`, `just image` + `sagemaker-local` contract, `python-client-vs-container`, `examples`. Cockpit: `just web` + live e2e. *Exit*: all listed lanes green; family.lock committed; a written Stage-8 evidence report in portal docs.

**Stage 9 — Cutover.** Fork of `cutover.sh`: forge-rename `jeryu/jain` → `jeryu/jain-monorepo` (archived, read-only — monorepo history preserved there); register the portal as `jeryu/jain`; update local checkout remotes; announce. Reversal documented (rename back; portal re-slugs) — this is the ONLY step that affects existing consumers. *Exit*: `git clone http://127.0.0.1:8787/git/jeryu/jain.git` yields the portal; monorepo reachable as `jain-monorepo`.

**Stage 10 — Closeout + steady state.** Final `reconcile.py` pull of any monorepo drift since `cc27936` (3-way merge; patches re-applied; conflicts via `dirty/merge-work/`); closeout PRs verifying GitHub mirrors == forge mains; publish the bump playbook + family runbooks in portal docs; enable the `jeryu-poll` autonomous loop; update agent memory. *Exit*: zero un-reconciled drift; runbooks merged; loop live.

## §10 Risk register

| Risk | Mitigation (already designed in) |
|---|---|
| LFS smudge forgotten → pointer-only "weights", golden parity skips silently | Materializer smudge + >1 MB assert (Stage 4); starforge REQ re-asserts (§4.9) |
| feat-web `hyperion-cuda` default breaks hosted/CPU lanes | All web CI pins `--no-default-features --features ci-smoke`; generated workflow-lint guard; demoting the default is a post-split decision, NOT bundled |
| feat-core API churn (hottest crate, 5 dependents) | `rust-api` drift lane in core; bumps ONLY via `bump-family-version.py` in wave order; portal `version-consistency` catches stray pins |
| Pin drift → two feat-core revs in deploy graph → `[patch]` unification failure | Same fleet lane; anchor + patches rewritten together by the bump tool |
| Cargo-git fetch needs GitHub in consumer CI | Warmed cargo-git cache on host runner; `git-fetch-with-cli`; outage only blocks cold builds |
| Tag force-move → stale cargo caches | Immutability after Stage 7; recut = bump `split.N` |
| Vendor-free cargo lane panics on FFI build.rs | Conceded + designed out: no such lane exists (§5.3) |
| `-p feat-cli`-through-anchor bin-build mechanics uncertain | Stage-1 spike decides primary vs `--manifest-path`+config-patch fallback (§6.2) |
| jankurai score on tiny repos (domain/contracts/docs) | Stage-5 local audits; per-repo policy tune with SPLIT.md note; family policy files stay byte-identical |
| Disk: 19 persistent target dirs | Governor + idle-prune policy (§7.3); monorepo's 171 GB target never copied |
| GitHub LFS quota (742 MB ≈ free-tier storage; bandwidth is the sharper limit) | Forge-authoritative first; mirror starforge LFS only after quota confirmed (§11) |
| Mirror credentials ×19 | Stage-0 preflight item; forge-side `mirror_github_main` uses one server credential |
| Monorepo drift during rollout (read-only ≠ frozen) | `reconcile.py` cadence + Stage-10 final pull; patch re-application automated; conflicts surfaced, never silent |
| ops/apex scripts assume repo root + external datasets | Whole-dir move preserves `ROOT` resolution; campaigns documented as manual, driven by a deploy-built binary |
| Hardcoded `/home/ubuntu/*` defaults in feat-cli | Travel as-is (no-behavior-change); post-split cleanup ticket recorded in jain-cli AGENTS.md |
| Backgrounded servers reaped in this sandbox (exit 144) | Documented in web/portal AGENTS.md: one foreground supervisor drives servers |
| rtk stdout truncation (~40 lines) misleads implementing agents | Documented in portal AGENTS.md: long output → files, never through rtk pipes |

## §11 Open items (confirm before the marked stage)

1. **Does the live forge read `.jeryu/repo.toml`?** (before Stage 4 template freeze) — jeryu-split repos have none; veox's exist with a 6-key schema pointing at the dead gitea. Generate the 6-key file with the live 8787 URL only if the forge consumes it; otherwise skip the file entirely.
2. **GitHub LFS quota for `neverhuman`** (before Stage 6 pushes jain-starforge to GitHub) — 742 MB storage fits the free tier once; bandwidth is the real constraint. Until confirmed: forge-authoritative weights, GitHub mirror without LFS objects.
3. **Does jain-model-zoo mirror to GitHub at all?** (before Stage 6, wave 4) — 3,153 files of frozen reference; forge-only is acceptable and cheaper.
4. **`jeryu serve --split-manifest` multi-family behavior** (before Stage 9) — veox/jankurai are PATCH-registered without server manifests (the proven path); only touch the systemd unit at cutover if server-side portal classification is wanted for jain.

## §12 Appendices

### 12.1 Manifest header + example entry

```toml
schema_version = "1"
workers = 40
source_root  = "/home/ubuntu/jain_small"
source_branch = "apex"
source_sha   = "cc27936eb45006bda0cae85b0f578f4d5985991d"
split_root   = "/home/ubuntu/jain-split"
repo_family  = "jain-split"
umbrella_repo = "jain"
dependency_tag_suffix = "v7.0.1-split.0"
rollout_wave_order = [0, 1, 2, 3, 4]
required_repos = ["jain","jain-domain","jain-math","jain-contracts","jain-catboost",
  "jain-xgboost","jain-lightgbm","jain-battle-gpu","jain-starforge","jain-core",
  "jain-report","jain-tui","jain-cli","jain-web","jain-python","jain-deploy"]
shared_source_paths = [".cargo/**",".gitignore",".gitattributes",".dockerignore",
  "rust-toolchain.toml","Cargo.toml","Cargo.lock","README.md","AGENTS.md","CHANGELOG.md",
  "RELEASE.md","RELEASE_PROCESS.md","ROLLBACK.md","RESULTS.md","renovate.json",".grype.yaml",
  ".claude/**",".jankurai/**"]
# retired_paths (coverage class w/ reasons): agent/** .github/** Justfile ops/ci/**
#   scripts/ci-*.sh tools/security-lane.sh tests/__pycache__/**  → regenerated per-repo

[[repo]]
path = "/home/ubuntu/jain-split/jain-core"
name = "jain-core"
github_slug = "neverhuman/jain-core"
jeryu_slug  = "jeryu/jain-core"
profile = "rust-workspace"
role = "core"
default_branch = "main"
rollout_wave = 2
has_jeryu_std = true
onboarded = false
mirror_github_main = true
current_tag = "jain-core-v7.0.1-split.0"
required_check = "jain-core/required"
note = "Pipeline hub: GP synthesis, epochs, ensemble; canonical progress-event contract producer."
cargo_members = ["crates/feat-core"]
copy_paths    = ["crates/feat-core", "contracts"]
source_paths  = ["crates/feat-core/**", "contracts/**"]
cross_repo_deps = ["jain-domain","jain-math","jain-catboost","jain-xgboost","jain-lightgbm","jain-starforge"]
```

### 12.2 family.lock (portal; jankurai-precedent schema + required_check)

```toml
schema_version = "1.0.0"
family = "jain"
release = "7.0.1-split.0"
source = "repos.manifest.toml"
generated_at = "<stage-8 timestamp>"
source_repo = "jeryu/jain-monorepo"
source_branch = "apex"
source_commit = "cc27936eb45006bda0cae85b0f578f4d5985991d"

[[repo]]
repo = "jain-core"
tag = "jain-core-v7.0.1-split.0"
commit = "<actual seed/rolled sha — regenerated, never hand-edited>"
github = "http://127.0.0.1:8787/git/jeryu/jain-core.git"
jeryu = "http://127.0.0.1:8787/git/jeryu/jain-core.git"
required_check = "jain-core/required"
```

### 12.3 Patch files (`~/jain-split/ops/split/patches/`)

`catboost-sys-vendor-root.patch`, `xgboost-vendor-root.patch`, `lightgbm-vendor-root.patch` — each: `JAIN_VENDOR_ROOT` env lookup before the legacy `../../vendor/<name>` resolution + `cargo:rerun-if-env-changed=JAIN_VENDOR_ROOT`. Additive-only; ~10 lines; `git apply --check`-gated; recorded in each learner repo's SPLIT.md. **This is the complete patch list — nothing else is patched.**

### 12.4 Generated per-repo standard (materializer output)

`README.md`, `AGENTS.md` (per-§4 AI notes), `SPLIT.md`, `Justfile`, `VERSION`, `CHANGELOG.md`, `docs/{architecture,testing,release}.md`, `agent/{owner-map.json,test-map.json,generated-zones.toml,proof-lanes.toml,audit-policy.toml,boundaries.toml,JANKURAI_STANDARD.md}`, `ops/ci/{required,score,security}.sh` (+ repo-specific lanes per §4), `ops/git-hooks/pre-push`, `ops/dev/local-patches.example.toml`, `scripts/ci-local.sh`, `.github/workflows/ci.yml` (SHA-pinned, structural), `.jeryu/repo.toml` (only if §11.1 confirms). Rust repos: root `Cargo.toml` (members + monorepo `[workspace.package]`/`[profile.*]`/lints) + regenerated `Cargo.lock`. Portal additionally: manifest, family.lock, family scripts. Deploy additionally: `[patch]` sections, anchor package, stage template, vendor-all/stage-context/get_data.

### 12.5 Load-bearing precedent files to fork

`~/jeryu-split/ops/split/splitctl materialize` (dep-rewrite ll.156–186, patch rendering ll.186–207) and siblings listed in Stage 1; `~/jeryu-split/ops/ci/split-host-ci.sh`; `~/veox-split/jeryu-ctl/{onboard.sh,lib.sh,host-ci.sh,jeryu-poll.sh,hooks/pre-receive}`; `~/veox-split/ci-status.sh` (fleet dashboard pattern for `fleet-ci.sh`); `~/jankurai-split/jankurai/family.lock` (lock schema); jain lane bodies: `~/jain_small/ops/ci/{fast,web-cockpit,python-client,examples,native,contracts,compression,sagemaker-*}.sh`, `~/jain_small/tools/security-lane.sh`, `~/jain_small/scripts/prune_vendor.sh`; Docker set: `~/jain_small/deployment/ops/Dockerfile.sagemaker*` + `assemble-runtime-assets.sh`.

---

*End of master plan. Implementing agents: work stage-by-stage (§9), hold every exit criterion, and treat §8.3 — 19/19 required lanes green offline with no siblings — as the definition of done for the split itself.*
