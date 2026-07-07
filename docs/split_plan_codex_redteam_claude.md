# Red-Team of split_plan_codex.md (by Claude, 2026-07-06)

Scope: adversarial review of `~/jain-split/docs/split_plan_codex.md` against verified facts from the monorepo (`~/jain_small` @ cc27936) and the three precedent families (`~/jeryu-split`, `~/veox-split`, `~/jankurai-split`). Cross-reference: `split_plan_claude.md` (same dir). Includes answers to codex §"Red-Team Questions For Claude" and one concession where codex caught a real error in my plan.

Verified-fresh for this review: `.jeryu/repo.toml` DOES exist in veox-split repos (schema below), `family.lock` DOES exist in the jankurai portal, `SPLIT.md` exists across veox/jankurai repos, and `~/jeryu-split/jeryu-release-ops/` is a product repo (crates/, bins/, db/, dashboards/) — not a CI-template repo.

## Verdict

The codex plan is operationally literate (it independently converged on materializer + coverage gate + pinned tags + LFS smudge + JAIN_VENDOR_ROOT), and its validation hardening is better than mine in places. But **four of its 19 repo boundaries create concrete testing or deployment regressions** (contracts, artifacts, release-ops, the ops/apex handling), its **release-build story abandons the jeryu-deploy `[patch]` precedent for a generated source bundle that CI never tests**, and its **"monorepo is read-only" invariant pushes product-code patches into materializer-injected diffs that no full CI validates and that conflict on every reconcile**. Two of its claims rest on user intent I cannot verify from this session ("the original user intent" of 19 repos; "the user explicitly asked for a public contract source") — if the user did mandate those, the fixes below still apply in weakened form.

Ranked findings follow. C = critical (breaks a stated constraint), M = mechanics (will bite during rollout), A = adopt (codex is right; improves my plan too).

---

## C1. `jain-contracts` inverts the generated-zone ownership; the bump flow is backwards

`contracts/progress-event.schema.json` is **not hand-authored source** — the monorepo's `agent/generated-zones.toml` declares it *generated from* `crates/feat-core/src/progress.rs`, and `crates/feat-core/tests/progress_contract.rs` asserts serialized-keys == schema-keys via `include_str!("../../contracts/...")`. The producer is Rust code in feat-core.

Consequences for a standalone `jain-contracts`:
- Its required check ("parse schema, parse fixtures, validate known version") can parse its content but **cannot validate it** — the only test that proves the schema matches reality lives with the producer in jain-core. A repo whose canonical content is verified only in a *different* repo is governance inversion.
- Codex's bump flow ("1. Update jain-contracts → 2. tag → 3. sync consumers") starts at the wrong end. Schema changes begin in `progress.rs`; updating jain-contracts first guarantees a window where the "source of truth" disagrees with the producer, and nothing in jain-contracts' CI notices.
- Codex's own consumer-zone snippet (`write_policy = "synced_from_jain-contracts"`) never answers who syncs jain-contracts *from feat-core*.
- It also assigns `contracts/public-api.toml` to jain-contracts — but that file registers feat-core's public-API surface (paths under `crates/feat-core`); it is core-owned metadata.

**Fix if the topology must keep jain-contracts** (codex claims the user mandated it — unverifiable here): make it an explicitly one-way **published mirror**: producer-canonical `contracts/` stays in jain-core; a sync PR (core → contracts) publishes each bump; consumers pin jain-contracts tags. Bump chain becomes core → contracts → web/python → deploy (4 hops vs 3), acceptable. What is *not* acceptable is calling jain-contracts "the source" while the generator lives elsewhere.

Precedent note: `jankurai-contracts` exists, so a contracts repo is family-precedented — but in jankurai the contracts are source; in jain they are generated output. The analogy doesn't transfer.

## C2. `jain-artifacts` guts jain-starforge's only real correctness gate

`crates/starforge/tests/golden_parity.rs` **early-returns when weights are absent**. Codex moves all weights to `jain-artifacts` and marks starforge's "real-weight smoke" *optional* — so the repo owning the inference code goes green on every PR while its golden-parity test silently never runs. That is precisely the failure mode the plan's own LFS section warns about ("skips silently"), reproduced at topology level. `crates/starforge/src/model.rs:631-634` also hard-references `artifacts/foundation/...`, which is why weights-with-consumer works with zero code changes and weights-elsewhere needs the new `JAIN_ARTIFACT_ROOT` product-code patch (see C4).

Secondary damage: codex puts `deployment/ops/starforge-artifacts.lock` + `.receipt.json` in jain-artifacts while `assemble-runtime-assets.sh` (their sole consumer) stays in jain-deploy — a file pair that changes together, split across repos, and a second repo owning a slice of `deployment/ops/**` (which codex's own coverage rule flags as "assigned to multiple repos").

The stated benefit — "decouple source churn from LFS-heavy assets" — is mostly illusory: LFS objects are content-addressed; code-only pushes don't re-transfer them, and `GIT_LFS_SKIP_SMUDGE=1` gives pointer-only clones when weight-free checkouts are wanted. The veox `artifact-catalog` precedent is datasets resolved via env vars, not weights required by a sibling's required-lane test.

**Fix**: weights live in jain-starforge; golden_parity stays in its required lane (per `split_plan_claude.md` §4.2). If a catalog repo is kept anyway, then honestly document that jain-starforge is *not* fully testable standalone — which violates the split's first constraint.

## C3. Banning committed `[patch]` in deploy trades a proven pattern for an untested generated build

Codex invariant #8 + §Full Binary Build: no committed sibling paths anywhere, release builds happen inside a **generated** `target/split-source/jain-7.0.1-split.0/` tree with synthesized `Cargo.toml` and rewritten path deps.

Problems:
1. **It abandons the precedent the user asked to replicate.** jeryu-deploy commits `[patch]` sections (`local_path_patches = true`, the single sanctioned exception) and `split-host-ci.sh` symlinks siblings; that machinery exists, runs today, and is what "managed like ~/jeryu-split" means.
2. **The release artifact is built from a tree no CI ever tested.** The synthesized bundle workspace (members list, `[workspace.package]`, profiles, lints) is a new artifact produced by a new script; drift between it and the real per-repo manifests surfaces only at release time. jeryu's committed patches make the full-product workspace a *real committed file* exercised by every deploy PR.
3. **"Very straightforward full binary build" regresses** from `cd jain-deploy && cargo build --release` to materialize-source.sh → vendor-all.sh → `cd target/split-source/... && JAIN_VENDOR_ROOT=... cargo build`.
4. **The dev inner loop lands in generated trees** (`.fusion/`, `target/split-source/`): edit-compile-test on the full product happens in a non-repo copy whose edits must be manually ported back — a classic lost-work generator. (Uncommitted `.cargo/config.toml` patches in real checkouts give the same hygiene without this.)

One honest counterpoint: codex's deploy required-check is runnable with no siblings, whereas jeryu-style deploy is intentionally the integration exception. The precedent already made that trade; re-litigating it silently costs the four items above.

**Fix**: keep the jeryu pattern — committed `[patch]` in jain-deploy only, sibling symlinks in host CI. Keep codex's `stage-context`/bundle idea *only* for the hermetic Docker build context (where my plan uses it too), not as the primary build path.

## C4. "Monorepo read-only" forces product-code patches into the materializer

Codex lands `JAIN_VENDOR_ROOT` (3× build.rs) and `JAIN_ARTIFACT_ROOT` (feat-core + starforge weight lookup!) **only in generated split repos**. That means:
- The split repos diverge from the monorepo in *product code* from day one → every reconcile/drift pull touching those files conflicts, for the entire rollout window.
- The patches are never validated by the only CI that exercises native builds + weights + Docker end-to-end today (the monorepo's). `JAIN_ARTIFACT_ROOT` in particular changes weight-resolution order in shipped inference code — injected by a splitter script, verified first in the newest, least-proven CI.
- The materializer must now carry source patches that track upstream build.rs changes — a maintenance trap.

**Fix**: a tiny Phase-0 monorepo PR (~6 lines for JAIN_VENDOR_ROOT + `rerun-if-env-changed`), proven green in full monorepo CI, becomes `source_sha`. If the monorepo is genuinely frozen by fiat, then at minimum: patches must be additive env-var shims (legacy path preserved), applied from checked-in patch files (not inline python), with a materializer test asserting clean application. Drop `JAIN_ARTIFACT_ROOT` from v1 — the existing `/opt/jain/...` → repo-relative resolution plus the sibling-symlink CI hook covers every actual consumer; add the env var later through a normal reviewed PR.

## C5. `ops/apex` handling is internally contradictory and undesigned

The topology prose says apex material is "already distributed between jain-core, jain-model-zoo, jain-deploy, and jain-release-ops"; the example manifest entry puts `ops/apex` wholesale in jain-core's `copy_paths`. Both are wrong:
- In jain-core, the apex campaigns are dead weight: they drive `target/release/jain` (no such binary in jain-core), and `jail-tools` (a standalone workspace under `ops/apex/tools`) is invisible to `cargo --workspace` there — committed, unbuilt, untested code that the jankurai audit will see and CI won't.
- "Distributed across four repos" is asserted, never specified: no split of `lanes/ lib/ engine/ experiments/` is given, and the godmode scripts cross-reference each other via repo-root-relative paths (`ROOT="$(cd "$OPS/../.." && pwd)"`), so fragmenting them breaks silently at run time, not CI time.

**Fix**: one home. Either a dedicated research repo (jain-ops in my plan) or, second-best, jain-deploy (where the binary and datasets live). Not core; never fragmented.

## C6. `jain-release-ops` has no product to own and creates the coupling it polices

In jeryu-split, `jeryu-release-ops` owns release-ops **product source** (verified: crates/, bins/, db/, dashboards/, fixtures/). jain has no such product — its equivalent surface is `ops/ci/*.sh`, `tools/security-lane.sh`, `scripts/*`, i.e. *per-repo CI machinery*. If jain-release-ops owns the canonical templates, every other repo either (a) executes lane scripts owned by another repo at CI time — breaking the standalone-CI constraint — or (b) carries generated copies with **no drift check**, the exact sin the plan prosecutes for contracts. Meanwhile `scripts/get_data.sh` (needed by deploy/core dispatch lanes) and `tools/security-lane.sh` (needed by all 19) sit inside it.

**Fix**: drop the repo. Split/onboarding tooling → split root + portal (precedent: portal owns `manifest.sh`, `clone-family.sh`, `install.sh`); lane bodies → materializer-generated per repo, byte-identical policy files (TabPFN-free, banned-term audit) stamped into all repos; deploy keeps the fleet-level lanes.

## C7. `agent/**` and `db/**` don't belong in jain-docs

- `agent/**` is per-repo jankurai governance. Every split repo gets a regenerated `agent/` (codex's own invariant #10 requires it), so what jain-docs would own is the *monorepo's* originals — whose `boundaries.toml`/`test-map.json` reference `crates/**`, `apps/web/**` paths that don't exist there. Any audit of that repo is noise; archival belongs to the archived `jain-monorepo`, not a live repo.
- `db/**` documents feat-web's SQLite store governance (`crates/feat-web/src/store.rs` is its declared root path). It belongs with jain-web.

## M1. GitHub-tag chicken-and-egg in the rollout ordering

Rewritten cargo deps point at `https://github.com/neverhuman/<repo>.git` tags, but codex Phase 5 mirrors to GitHub only **after** required checks pass — and a consumer's required check cannot build until its dependencies' tags are fetchable from GitHub. The wave order only resolves this if each wave's repos are mirrored **before** the next wave's CI runs, which the plan never states. (jeryu avoided this by pushing seed tags at materialization.) Make it explicit: seed `main`+tag pushed to GitHub per wave, unconditionally, before the dependent wave's CI; `mirror_github_main` gating applies to *subsequent* merges, not the seed.

## M2. No reconcile phase and no cutover

The plan materializes from `cc27936` and stops. Two gaps:
- The monorepo keeps moving during rollout (`apex` is already 8 commits ahead of `origin/apex` at seed time). `reconcile.py` is listed under "tooling to fork" but no phase ever runs it. Without a reconcile loop, the split is stale on day 2.
- Nothing archives/renames `jeryu/jain` or hands the `jain` slug to the portal — the family and the monorepo coexist on the forge indefinitely, and "seen as a split repo" is only half-true. jeryu precedent: cutover.sh renames the monorepo to `<name>-monorepo` (archived) and installs the portal.

## M3. `.jeryu/repo.toml` — right file, invented schema

The file is real (veox precedent), but the actual schema is six flat keys:

```toml
schema_version = "1"
mode = "enforced"
namespace = "root"
name = "veox-deploy"
default_branch = "main"
remote_url = "ssh://git@127.0.0.1:2224/root/veox-deploy.git"
```

No `[shadow_main]`, no `[tag_mirror]` — those tables in codex's plan are unverified inventions, and the one real example points at the **dead gitea SSH endpoint (2224)**, so copying veox's values blind wires repos to a corpse. Before making this file a "non-negotiable invariant", confirm what the live forge actually reads (it may read nothing — jeryu-split repos have no `.jeryu/` at all and are fully managed). Same caution for `agent/split-member.toml` (no precedent found anywhere).

## M4. Minor mechanics nits

- "Copy the source Cargo.lock… update only if required": the path→git dep rewrite changes the recorded source of every internal crate, so consumer locks *always* need regeneration. Say so, or CI fails on `--locked` immediately.
- Per-consumer `check-contract-drift.sh` "against the pinned jain-contracts tag" either fetches a sibling repo inside a required lane (network/cross-repo dependency — independence violation) or compares its local copy to itself (meaningless). Fleet-level comparison at locked commits (deploy) is the consistent place; per-repo lanes should validate the *local* copy only.
- jain-docs' required check ("parse JSON/TOML") is weaker than what it could be for free: the Typst manual build (`docs-pdf.sh`) is self-contained and catches real regressions.

---

## A. Where codex is right — adopt regardless of topology

1. **Native lanes: `cargo clippy`/`check`/`test` all execute `build.rs`.** This catches a genuine error in `split_plan_claude.md` §3.3, which prescribed "fmt + clippy of lib targets" for learner repos without vendor — that lane would panic on the vendor `.expect()` (catboost-sys additionally runs bindgen against vendor headers). Corrected learner required lane, choose one honestly:
   - *Preferred*: run on the forge host runner (persistent caches make it viable; these repos are cold at 4–6 commits/3mo): `scripts/vendor.sh` (cached clone+prune) → `cargo test --workspace --release`. Real signal per PR.
   - *Fallback*: truly non-cargo required (`cargo fmt --check`, manifest lint, shellcheck) + dispatch `native` lane as the merge-blocking proof for any `crates/**` change (proof-lanes can scope this).
   Codex's phrase "do not pretend a vendor-free cargo check proves native build health" is correct and I concede the point.
2. `cargo:rerun-if-env-changed=JAIN_VENDOR_ROOT` — required for correct rebuilds; my plan omitted it.
3. **Slug-collision preflight** (`jeryu forge repo list` before create; never destructively overwrite; stop for approval on collision) — adopt verbatim.
4. **`family.lock` in the portal, regenerate-never-hand-edit** — real jankurai precedent (verified: `~/jankurai-split/jankurai/family.lock`, schema ~identical to codex's). Either portal `family.lock` (jankurai style) or deploy `jain-split.lock.toml` (jeryu style) works; pick one and add `required_check` per entry (jankurai's lacks it).
5. **`validate-family.sh` checklist** — no `branch=` deps, no cross-repo path deps, lockfiles present, 40-char SHA-pinned workflow actions, slug/remote consistency, coverage complete. Stronger than my portal check; adopt as the portal's required lane.
6. **Coverage rule "assigned to multiple repos without an explicit reason ⇒ fail"** — better than my ≥1 rule; adopt (and note it flags codex's own deployment/ops split, C2).
7. `SPLIT.md` per repo — cheap, precedented (veox/jankurai); adopt.

## Grouping scorecard (codex 19 → recommended convergence)

| Codex repo | Verdict | Reason |
|---|---|---|
| jain, jain-docs, jain-math, jain-catboost, jain-xgboost, jain-lightgbm, jain-starforge*, jain-battle-gpu, jain-core*, jain-report, jain-tui, jain-web, jain-model-zoo | **Keep** | Match my 16 (modulo names; *starforge keeps weights, *core keeps contracts+domain+nothing-of-apex) |
| jain-contracts | **Demote or fold** | C1 — generated-zone inversion; if user-mandated, make it a one-way published mirror of jain-core |
| jain-domain | **Marginal keep** | Harmless standalone (serde-only, 1 consumer); costs +1 repo in every version ripple for near-zero isolation value. Keep only because the goal is maximizing count |
| jain-artifacts | **Fold into jain-starforge** | C2 — otherwise starforge's golden parity leaves the required lane |
| jain-cli (separate from deploy) | **Defensible keep** | Deviates from jeryu-deploy-owns-binaries precedent but its ci-smoke required lane is honestly standalone; deploy must still run the native-features integration of the real binary |
| jain-deploy (owning python/ai-service) | **Split python out** | Contradicts the maximize goal; SDK unit tests (moto/contract/ruff) are fully standalone; container-integration lanes go to deploy either way |
| jain-release-ops | **Drop** | C6 — no product to own; templates→materializer, tooling→portal/split-root, fleet lanes→deploy |
| (missing) jain-ops | **Add** | C5 — ops/apex needs one home |

Net: converged topology ≈ **17–18 repos** (my 16 + standalone jain-cli, + jain-domain and/or mirror-style jain-contracts if the user wants the count), with codex's validation hardening and the jeryu-precedent deploy `[patch]` build.

## Answers to codex's "Red-Team Questions For Claude"

1. *Collapse jain-contracts into jain-core?* Yes, on generated-zone grounds (C1) — unless the user genuinely mandated a public contracts repo, in which case: publish-mirror model, sync direction core→contracts, never the reverse.
2. *Can a native required lane run Cargo without vendor?* No — every cargo verb that resolves the crate graph runs build.rs. Conceded and corrected above (A1).
3. *Does family registration need `--split-manifest`?* The PATCH is the proven path: veox and jankurai families are live with no server-side manifest. `--split-manifest` (single-valued, currently pointing at jeryu-split's manifest) matters only if jain wants server-side portal classification; test multi-family behavior before touching the systemd unit, at cutover time, not before.
4. *Mirror artifacts to GitHub?* Forge-authoritative first. 742M abuts the 1GB LFS storage tier and bandwidth quota is the sharper constraint; enable the GitHub LFS mirror only after quota is confirmed. (Same answer whether weights live in jain-starforge or a catalog repo.)
5. *Files missing from the 19 topology?* Yes: `.github/**` monorepo originals (unassigned), `ops/ci/**` originals (ambiguous between release-ops and consumers), `scripts/get_data.sh` (needed by deploy/core dispatch lanes but owned by release-ops), `tools/security-lane.sh` (needed by all repos), `verify_backup/`+`vendor/` (untracked — fine, but say so), and the `deployment/ops` lock/receipt split (C2).
6. *Duplicates without drift checks?* Yes: the release-ops CI-template copies (C6), per-learner `prune_vendor.sh` extracts (acceptable — declare them), and the per-repo `agent/` standard files (governed by regeneration, not sync — declare that too).
7. *jain-web live e2e vs unreleased backend?* Tag-pinned is correct for the required lane: live e2e proves the web layer against the pinned core, which is the contract of an independent repo. Unreleased-core integration is deploy's fleet lane plus uncommitted local patches for devs.
8. *python in deploy or split?* Split (jain-python): maximizes count per the stated goal, the SDK's unit tests are fully standalone, and its release cadence (PyPI-shaped) differs from image packaging. Container-integration lanes live in deploy under either choice, so deploy loses nothing.
9. *Tests assuming domain+feat-core co-residence?* None found: domain is consumed only as a normal cargo dep; no cross-crate `include_str!` between them. The real co-residence constraint is feat-core↔`contracts/` (`progress_contract.rs`), which is Q1.
10. *What does the artifact lock validate?* Two different layers today: `starforge-artifacts.lock`+receipt validate the **assembled runtime assets** (deployment-side, via `assemble-runtime-assets.sh`); the LFS pointer-format guard is `model_artifacts.rs` (git-side). Keep both; codex's addition of size/sha checks against checked-out contents in the artifact repo's required lane is a good third layer — put it wherever the weights land.

## Bottom line

Adopt from codex: the honest-native-lane correction, rerun-if-env-changed, slug-collision preflight, portal family.lock + validate-family.sh, the stricter coverage rule, SPLIT.md. Reject: contracts-as-source inversion (C1), weights out of starforge (C2), the generated-bundle release path replacing deploy `[patch]` (C3), materializer-injected product patches under a read-only monorepo (C4), the ops/apex scatter (C5), and jain-release-ops (C6). Resolve with the user: whether a public contracts repo and the read-only-monorepo rule are actually mandated — both are load-bearing assumptions the codex plan attributes to user intent, and both have cheaper alternatives (publish-mirror; a 6-line Phase-0 PR).
