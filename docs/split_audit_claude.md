# jain-split Implementation Audit (Claude, 2026-07-06)

> **UPDATE (post-remediation):** both bars are now met — jankurai **19/19 green** (committed-baseline ratchet, monorepo posture) and required CI **18/19 green** standalone/offline (only jain-deploy, the integration repo, pending a lockfile refresh). See **`docs/split_ci_evidence_claude.md`** for evidence and the three systematic generator bugs that were fixed. The audit below is the original pre-remediation finding.

Audit of the on-disk 19-repo split at `/home/ubuntu/jain-split/` against `docs/split_plan_master.md`. Method: three parallel read-only audit agents (CI/buildability, jankurai readiness, contracts/artifacts/deps) + direct verification. **Verdict: the split is ~80% mechanically correct, but neither hard bar is met yet** — not every repo's CI is *provably* passing standalone, and **no** repo has a committed passing jankurai audit.

User decisions locked this session: **jankurai bar = match the monorepo's posture** (the monorepo itself scores 64 and passes via ratchet-against-baseline, not absolute ≥85); **freeze the family before certifying**.

---

## 0. Correction to the "freeze" premise (important)

An audit agent attributed the live re-materialization to `universe-board.timer`. **That is wrong.** `~/veox-split/jeryu-ctl/universe-board.sh` is explicitly *pure read-only* dashboard aggregation ("No builds, no audits, no mutations"). The actual cause of the observed commit/tag churn (jain-core seen at `881847a`→`30a0c7b`→…→`9e01393` during the audit) is **concurrent `codex --yolo` agents** (multiple live sessions in `ps`) re-running `splitctl materialize --init-git`, which does a fresh `git init` + `git tag -f` each time.

**Current state: the tree is quiescent** — stable ~12 min, and all 19 repos are a consistent snapshot (every `<repo>-v7.0.1-split.0` tag == HEAD). But the peer agents remain live, so the freeze is a **multi-agent coordination** matter, not a timer to mask. Any mutating remediation (re-materialization, forge push) risks clobbering / being clobbered by those agents and must be coordinated.

Frozen snapshot at audit time: jain `951dde7`, jain-domain `dd0eafd`, jain-math `1221423`, jain-contracts `6adf346`, jain-catboost `18182dd`, jain-xgboost `4f1cf71`, jain-lightgbm `bb48e88`, jain-battle-gpu `164afdf`, jain-starforge `7bc4abd`, jain-core `9e01393`, jain-report `597b462`, jain-tui `7b9afa7`, jain-cli `6a6461c`, jain-web `bb209dd`, jain-python `486b373`, jain-model-zoo `4ea51b0`, jain-ops `72b0d1f`, jain-deploy `2f6a7b0`, jain-docs `3ef691f`.

---

## 1. GOOD — verified correct, do not touch

- **Materializer solid.** `ops/split/splitctl materialize` has no stubs/TODOs; 6/6 self-tests pass. Implements LFS smudge + >1 MB assert, 3-patch apply w/ `git apply --check`, per-repo lockfile regen via bare-mirrors+insteadOf, bare-mirror emission, profile renderers. `reconcile.py` / `splitctl source-coverage` real and complete.
- **Contract lockstep byte-identical at v9** across all 5 copies (jain-core canonical + contracts/web/python/deploy mirrors; only MIRROR.md differs). `progress_contract.rs` `include_str!("../../contracts/…")` resolves in-repo. Portal `contracts-sync.sh` is a real drift gate.
- **Artifacts real.** All 6 starforge safetensors LFS-smudged and >1 MB (starforge 528 MB, foundation 214 MB); `.gitattributes` matches monorepo; `starforge-artifacts.lock`+receipt correctly in jain-deploy.
- **Cross-repo deps correct.** Uniformly git-tag-pinned `jain-X-v7.0.1-split.0` with `optional`/`default-features`/`features` preserved; catboost→catboost-sys stays intra-repo path; **zero** forbidden cross-repo `path=../jain-` outside jain-deploy `[patch]`; **zero** `branch=` deps. Root Cargo.tomls replicate `[workspace.package]` 7.0.1 + profiles; committed tag-pinned locks.
- **Coverage clean:** 4,388 tracked, 0 missing / 0 duplicate. 3 vendor patches apply clean and are wired into each learner `build.rs`. Governance (`agent/*`) complete and **per-repo tailored** (not verbatim); boundaries reference only local members; jankurai 1.6.10 installed, matches the pin. Portal fleet scripts real; `family.lock` schema-valid.

## 2. BLOCKERS

- **B0 — peer-agent re-materialization (coordination, not a timer).** See §0. Freeze = coordinate with / pause the concurrent codex agents; the tree is currently stable and certifiable if it stays put.
- **B1 — the independence gate is never actually run.** Only `fleet-ci.sh` (in-place, siblings present) and `ops/ci/split-host-ci.sh` (symlinks **all** siblings unconditionally, lines 52-61) execute `required.sh`. The offline/no-sibling harness exists only as ingredients (`target/local-gitconfig`, `target/bare-mirrors/*.git`) with **no runner**. So "19/19 green standalone" (plan §8.3) is unproven — it was validated *with* siblings. Fix: build a scratch-HOME/no-sibling runner and actually run it.
- **B2 — jankurai never certified.** Zero committed `.jankurai/` scores anywhere; never fleet-graded. Portal stuck at **74 < 85**. The split `score.sh` uses an absolute-85-or-documented-caps gate — **stricter than the monorepo's ratchet** (source scores 64 and passes). `ops/ci/security.sh` (byte-identical across 19) **silently dropped the mandated TabPFN-free + banned-term ("fallback") scans**. Product-code `fallback`/dead-language hits: jain-model-zoo 62, jain-math 10, jain-core 14, jain-web (capped) — fail under the current gate; pass under the monorepo's reference/native profiles.

## 3. MAJOR

- **Offline-hostile required lanes:** jain-web (`pnpm install --frozen-lockfile`, Playwright), jain-python (venv + `pip install -e`), and all Rust lanes pull crates.io — no vendored crates / warm caches committed. "No network" (§8.3) is currently false.
- **Hollow required lanes** (so "green" is cheap): jain-docs = `check.sh` only, **no Typst build** (just prints "required ok"); jain-contracts / jain-ops = `check.sh` only; jain-model-zoo = dir-count; **learners SKIP native `cargo test` unless `JAIN_VENDOR_ROOT` set** → standalone they never exercise build.rs/FFI (the honest-native concession unmet).
- **Missing runner tools:** `typst`, `taplo`, `patchelf`, `hadolint`, `cargo-public-api`, `cargo-semver-checks` (docs lane, core rust-api lane, manifest lint cannot run). Present: cargo/rustc, pnpm/node, git-lfs, jankurai 1.6.10, shellcheck, ruff, pytest, docker, jq, cmake, cargo-nextest, cargo-llvm-cov.
- **jain-deploy required lane is structurally sibling-bound** (committed `[patch]`→`../jain-*`); `cargo metadata --locked` errors with no siblings. Must be explicitly **exempt** from the no-sibling gate (deploy = integration repo by design; its required lane stays sagemaker-ci + `stage-context --plan`).
- **Docker `stage-context.sh` half-wired:** off-by-one `../repos/<name>` patch paths, never stages the workspace toml/lock. **This proves master §6.4 wrong** — a verbatim workspace is NOT enough once manifests are git-tag deps; stage-only top-level `[patch]` redirecting `neverhuman/jain-*` URLs → `.stage/<member>` is required (codex's execution-plan line 9 was correct). Only reached via `--plan` in CI, so untested.
- **Evidence aspirational:** `ops/split/local-proof-report.md` claims "19/19 required passed" but its commit table is stale, admits the score lane isn't green, and no `.ci-status/` logs exist on disk.
- **Content guards missing:** `jain-core/scripts/publish-contracts.sh` absent (canonical→mirror sync tool); starforge required lane has **no LFS `ls-files`/>1 MB guard** (silent golden-parity skip risk — `golden_parity.rs` real-weight test is `STARFORGE_CHIMERA_SMOKE`-gated and skips by default); jain-contracts drift lane omits the §4.16 version-consistency check; starforge `compression`+`cuda` lanes missing (`crates/starforge/src/model.rs:624` dangles a ref to a nonexistent `ops/ci/compression.sh`).

## 4. MINOR

MIRROR.md lacks provenance/version fields; jain-web contract generated-zone under-scoped + `write_policy` diverges from plan (and leaks cross-repo paths into jain-core's zone command); two divergent `split-host-ci.sh` (stale `ops/split/` copy — delete); catboost `vendor.sh` rsyncs from the monorepo rather than a pinned upstream clone; `LICENSE` absent in all 19; some member-repo `test-map.json` references (family.lock) may dangle; `[inherited_source_caps]` tension is moot under the chosen monorepo-posture.

---

## 5. Remediation plan (execute after coordinating peer agents)

**Core principle:** the per-repo lanes/policies/docs are *generated from templates inside `splitctl materialize`* (security.sh is byte-identical across 19 → single template). **Fix the generator + templates, then re-materialize once.** Do NOT hand-edit the 19 repos.

- **Stage A — Freeze:** coordinate/pause the concurrent codex agents (there is no timer to mask); confirm HEADs/tags stable; treat current tags as immutable. Optionally make `splitctl materialize --init-git` idempotent (no commit/tag rewrite when the repo already matches source; guard `splitctl materialize:2540-2548,2663`) so accidental re-runs don't churn.
- **Stage B — Generator/template fixes:** add `publish-contracts.sh`; add starforge LFS `ls-files`+>1 MB guard to `required.sh`; add starforge `compression`+`cuda` lanes and fix the `model.rs:624` dangling ref; add version-consistency to the contracts drift lane; write real MIRROR.md provenance; fix web generated-zone scope + `write_policy`. De-hollow required lanes to §8.1 (docs→Typst+linkcheck; contracts→version-consistency; ops→jail-tools test+shellcheck; model-zoo→integrity walk + oracle `.npy` checksums; **learners→native `cargo test --release` with a cached vendor in the required lane**). Delete stale `ops/split/split-host-ci.sh`; gate sibling symlinks in `ops/ci/split-host-ci.sh` on `JAIN_NEEDS_SIBLINGS`/`JAIN_NEEDS_ARTIFACTS`. Install missing tools + commit a warm-cache strategy (CARGO_HOME registry cache + pnpm store + Playwright browsers) so lanes are network-independent. Fix `stage-context.sh` (path off-by-one + stage workspace toml/lock with stage-only `[patch]`); mark deploy exempt from the no-sibling gate.
- **Stage C — Jankurai to monorepo posture:** port the monorepo's **ratchet** model into the split `score.sh` (`--mode ratchet --baseline <committed accepted-baseline.json>`), replacing the absolute-85 gate; generate+commit per-repo baselines. Replicate the monorepo's `reference_profile`/`native` classification (source: `~/jain_small/agent/boundaries.toml:58,193`, `owner-map.json:61`) into jain-model-zoo + learners. Restore TabPFN-free + banned-term scans in `security.sh` (byte-identical). Provide the standard docs the monorepo's policy expects; add `LICENSE`. Drive genuine **hard findings to 0** (required even under ratchet); document any inherited caps in `allowed[]` with a SPLIT.md note.
- **Stage D — Re-materialize clean, prove, persist:** one clean materialization at frozen tags → regen `family.lock`; run the new independence runner → **19/19 `required ok` offline, no siblings**, persist `.ci-status/` logs; run `jankurai audit` per repo → commit scores+baselines, **every repo passes monorepo-posture (ratchet, 0 hard)**, portal ≥ its baseline; portal fleet lanes green.

**Reference for posture (read-only):** `~/jain_small/agent/audit-policy.toml` (ratchet `audit_ci`, required_docs), `~/jain_small/.jankurai/repo-score.json` (baseline 64). **Invariant:** `~/jain_small` stays untouched at `cc27936`.
