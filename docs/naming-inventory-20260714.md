# Naming inventory — 2026-07-14 (input for WQ-10 naming RFC)

Read-only inventory of every naming surface in the jain-split family. Owner context:
company/parent = **veox**, product family = **jain**; the local jeryu forge is the ONLY
remote; all `neverhuman` GitHub pointers are being scrubbed; forge owner namespace for the
27 jain repos migrates to **veox/** (dual-home, jeryu/* frozen alias) during v8.0.0;
repo basename renames are POST-release (this RFC's subject).

## A. Repo purposes + name surfaces (dir | purpose | crates [prefix] | bins | old slugs)

- **jain** — public portal / installer / clone entrypoint; no product source. Crate:
  `jain-portal-ops`. Slugs: jeryu/jain + neverhuman/jain — **collides with the `jain_small`
  monorepo (same slug both places)**; historical workaround routes proof via
  `jeryu/jain-portal-preview` (hardcoded splitctl main.rs:6131). → veox/jain-portal candidate.
- **jain-docs** — product documentation. No crates.
- **jain-domain** — stable typed error/repair-hint contract crate. Crate: bare `domain`.
- **jain-math** — feature-DSL "new-math" engine: genome DSL interpreter + emitters +
  evolutionary invention. Crate: `feat-math`.
- **jain-contracts** — published mirror of jain-core contracts (profile=mirror).
- **jain-catboost / jain-xgboost / jain-lightgbm** — native learner FFI bindings (built from
  source, no Python). Crates: bare `catboost`(+`-sys`), `xgboost`, `lightgbm`.
- **jain-jable** — pure-Rust deterministic scalar tabular regressor ("Phase 5"). Crate: bare
  `jable`. Opaque codename; = the in-house tabular regressor.
- **jain-battle-gpu** — optional GPU kernel factory over feat-math (batched GPR marginal
  likelihood + ridge; cudarc, CPU fallback). Crate: `battle-gpu`.
- **jain-starforge** — pure-Rust "Starforge" tabular classifier inference + safetensors
  weights (candle, CPU/CUDA); ships the six LFS weights + (post WQ-3) the JOPE/Lime
  model-bundle. Crate/bin: `starforge`.
- **jain-core** — GP feature-synthesis pipeline hub + canonical progress-event contract.
  Crates: `feat-core`, `jain-redline-store`.
- **jain-llm** — native LLM provider + token-routing interfaces. Crate: `jain-llm`.
- **jain-agent** — governed **Jekko** runtime contracts (bounded runbook gen, capability
  checkpoints, tool receipts). Crate: `jain-agent`. NOTE: 1:1 twin of `jekko-agent` in the
  jekko-split family on the same forge.
- **jain-jnoccio** — LLM provider adapters + health-aware routing (probes, Router slots,
  cooldowns, usage accounting). Opaque codename; = provider gateway/router. Twin: jekko-jnoccio.
- **jain-zyal** — "ZYAL 2.6" runbook compiler + durable workflow supervisor (.zyal JSON,
  capability negotiation, budgets, evidence receipts). Twin: jekko-zyal.
- **jain-jailgun** — managed browser-session registry + lifecycle (opaque profile handles,
  secret-safe credential routing; `~/.jailgun/` state). Twin: jekko-jailgun.
- **jain-research** — AutoResearch + theory-chase orchestration contracts.
- **jain-report** — executive report bundle generation. Crate: `feat-report`.
- **jain-tui** — ratatui dashboard layer. Crates: `feat-tui` + a **second `feat-cli`**
  producing bin **`jain`** (duplicates jain-cli's crate+bin; not in manifest cargo_members).
- **jain-cli** — product CLI (+ retired SageMaker entrypoint). Crate: `feat-cli`, bins
  `jain`, `jain-entrypoint`.
- **jain-web** — local web server + React cockpit. Crates: `feat-web`,
  `jain-web-sqlite-migration`, `jain-redline-consumer-contract`, `jain-web-ci`; bin `jain-web`.
- **jain-model-zoo** — frozen reference ports + oracle fixtures + parity harness (~150
  algorithm-named crates; no weights stored).
- **jain-ops** — apex research campaigns, zyal templates, bench manifests. Crate `jail-tools`
  with `jail-*` bins (a fourth prefix family).
- **jain-deploy** — release authority + integration home (registry push/relay/promote,
  canary auth, Caddy, doctor). Crates: `jain-deploy-engine`, `sagemaker-ci`,
  `jain-supervisor`, `jain-product`, `jain-deploy-ops`, `sqlite-to-redline`; bin `deployctl`.
  Holds `.stage/` duplicate staged workspace (same crate names, second on-disk copies).
- **jain-smartcluster** — single-node workload fabric: SCQ protocol → `scqd` durable
  scheduler; bins `scqd`, `scq`, `jain-worker`. Was dual-homed jain-split/+jeryu/ (0 tags).
- **jain-split-ops** — family control plane (authority manifest, splitctl, runbooks).
- **redline family** (redline-split/): `redline` (front door; remote **jeryu/redlineDB**,
  github **neverhuman/RedlineDB**), `redline-core` (embedded SQL engine; crates all
  `redlinedb-*`, bin `redlinedb`), `redline-testing`, `redline-web`, control
  `redline-split-ops`. PLUS an unmanaged on-disk `redline-split/redline-central`
  (crate `redlinedb-client`, NO remote, absent from all manifests).

## B. Non-repo naming surfaces

- Deploy engine constants (jain-deploy/crates/jain-deploy-engine/src/config.rs): module doc
  says "jain (formerly veox) release engine"; REGISTRY `image.neverhuman.org`; IMAGE_REPO
  `doug/jain_small/jain-sagemaker` → **being renamed to `veox/jain` (WQ-8)**; REMOTE_HOST
  `atomicsoul`=192.168.68.78; CANARY `www.neverhuman.org/release/canary/*`; slots
  jain-a/jain-b; token env JAIN_INSTALLER_TOKEN_SECRET ("renamed from veox's NHT_*").
- Second image name `jain-relay:{version}` (registry relay).
- SageMaker residue despite `sagemaker="N/A"`: image name (fixed by WQ-8), crate
  `sagemaker-ci`, bins `sagemaker-*`, `Dockerfile.sagemaker[.gpu]`, /ping+/invocations
  serving contract. RFC decision needed post-release.
- Landing/branding: product marketed as "jain Cloud"; installer at
  www.neverhuman.org/install-jain.sh + /api/install/jain.sh?key=; contact jepson@veox.ai;
  cosign pubkey at www.neverhuman.org/cosign.pub. Docker auth also holds legacy
  `image.forge-ei.com`.
- veox lineage still live: `veox-split` forge family (9 repos: veox-deploy, veox-enclave,
  veox-nht, veox-neverhuman-data, …), VE0X_NATIVE_FEATURE_REQUEST.md in redline docs.
- Tag schemes: jain `<repo>-v8.0.0-split.N`; redline `<repo>-vX.Y.Z-jain.N` (an independent
  family tagged with a consumer's name — RFC item); jeryu `v5.0.0-split.0`.
- `jain-split` is overloaded 5 ways: root dir, forge owner (smartcluster), family value,
  redline consumer identity, required-check prefix. veox/* migration removes use #2.

## C. Forge tenant map (127.0.0.1:8787, 93 repos, 3 owners pre-migration)

- **jeryu** (77): families jain-split (28 incl. jain-portal-preview, jain-python excluded),
  jekko-split (11), jeryu-split (11), jmcp-split (4), redline-split (5), veox-split (9),
  loose (alpha, echoforge, jansu, jbreak, jpoly, openQG, …).
- **root** (15): jankurai-split audit/governance family (kernel, dedup, fleet, analyzers…).
- **jain-split** (1): jain-smartcluster only. → superseded by veox/* for jain repos.

## D. Collisions to resolve in the RFC (beyond what v8.0.0 fixes)

1. jeryu/jain monorepo ↔ portal slug collision (v8 fix: veox/jain-portal).
2. redline: one product, five spellings (redline / redlineDB / RedlineDB / redlinedb-* /
   redline-split; tag suffix -jain.N).
3. Prefix zoo: repo `jain-*` vs crates `feat-*` / bare / `jain-*` / `jail-*` vs bins
   (`jain`, `jain-web`, `scqd`, `scq`, `deployctl`, `redlinedb`). Pick one policy.
4. jain-tui's duplicate `feat-cli` crate + bin `jain`.
5. jain agent-stack twins of jekko-split (agent/jnoccio/zyal/jailgun/llm) — disambiguate
   jain-vs-jekko ownership or unify.
6. SageMaker naming residue (crate/bins/Dockerfiles/serving contract).
7. Unmanaged `redline-split/redline-central` (no remote, no manifest) — register or remove.
8. Stale root mirror `/home/ubuntu/jain-split/repos.manifest.toml` (non-authoritative copy).
