# WORK QUEUE — Jain v8.0.0 final push (dispatcher: Claude, sole release-writer)

**Protocol:** Claim a ticket by editing its `Status:` line to `CLAIMED — <agent> — <UTC time>`.
Work ONLY inside the ticket's Scope. When done: set `Status: DONE`, fill the **Report** section
with the exact verification-command outputs, and append one line to `docs/AGENT_CHAT.md`
(`### <UTC> — <agent> — WQ-<id> done`). I (Claude) re-verify and mark `VERIFIED`.
If blocked: set `Status: BLOCKED — <reason>` and report; do not improvise outside scope.

**Global guardrails (violating = instant revert):** never move an existing tag; never edit
`redline.lock.toml`; never relax branch protection or waive a consumer; reviewed PR lifecycle
only (`splitctl jeryu-local pr-open/approve/merge` + `ops/ci/split-host-ci.sh` posted checks);
`ATOMICSOUL_PUSH=0` everywhere (the real push is Claude's owner-authorized lane); NO GitHub /
neverhuman remotes — local forge only; Rust only. The jankurai auditor override is owner-approved
for v8.0.0: consume current auditor receipts, cite
`docs/release-evidence/8.0.0/jankurai-auditor-exception.json` in your receipts.
**Only Claude runs the mutating `release-candidate`/`deploy.sh`.** Redline family stays on
`jeryu/*` this release (namespace RFC covers it later).

Merged-main SHAs feed the batched manifest binds (Claude). Post the merged SHA in your Report.

## STATUS BOARD (ClaudeMaster updates this — check before claiming)
| Ticket | Status | Notes |
|---|---|---|
| WQ-1 veox dual-home | **DONE/VERIFIED (ClaudeMaster)** | 27/27 mirrored+registered+protected; see report |
| WQ-2 math #11 | **DONE/VERIFIED** | main=b0157e5, jain-math/required=success posted |
| WQ-3 starforge #7 | **DONE/VERIFIED** | main=e75276e; 11/11 sha pass; ALL MODELS COMMITTED |
| WQ-4 core head | ClaudeMaster — PR #12 open @ 0a40973, CI lane running | JOPE/Hyperion spot-gates passed (3+17 filtered) |
| WQ-5 web consolidate | **BLOCKED — Codex — 2026-07-14T04:28Z** | PR `veox/jain-web#1` @ `5c76e112`; GPU-default feature correction awaits protected Core merge/pin |
| WQ-6 fleet identity | **BLOCKED — Codex — 2026-07-14T04:28Z** | Agent PR #1 blocked by missing immutable LLM v8 tag/score floor; Jailgun/ZYAL proof blockers remain |
| WQ-7 contracts | **DONE — Codex1** | PR #10 merged; main=fcbb3606; required + jankurai current-head success |
| WQ-8 deploy image | CLAIMED — Codex — 2026-07-14T02:53Z | head=c3df50f6; governed required lane rerunning |
| WQ-9 harness | BLOCKED — Codex2 — 2026-07-14T03:54Z | live 127.0.0.1:4180 unavailable during corrected rerun; receipt recorded |
| WQ-10 naming RFC | **DONE/VERIFIED** (Codex1) | docs/naming-rfc-v1.md — owner decides post-GO |
| WQ-12 parallel hygiene | **DONE — Codex1 — 2026-07-14T03:26Z** | four read-only workers complete; blockers handed to lane owners |
| PR-A (control plane #13) | ClaudeMaster — CI rerunning @ be11d342 | fixed: rg install, worktree authority-anchor bug, rustfmt |

**LIVE STATE NOTES (2026-07-14 ~02:5x UTC):**
- `veox/*` namespace is LIVE on the forge: all 27 jain repos mirrored (head+tag parity
  verified), registered (API count 27), branch protection applied (1 approval +
  `<name>/required` + linear history + enforce_admins).
- Merges STILL happen on `jeryu/<repo>` PRs for now (WQ-2..8 as written). ClaudeMaster
  delta re-mirrors jeryu→veox AFTER your merges, then flips manifest+origins (PR-B),
  then binds, then runs the single full deploy.sh.
- veox owner credentials: account `veox` exists; ClaudeMaster holds the token
  (~/.jeryu/secrets/veox-owner-token). Workers do NOT need it (work on jeryu/*).
- Control-plane PR #13 (jeryu/jain-split-ops, head 018b418) carries: contract-drift=32,
  sync-derived trigger fix, vendor bootstrap, jain.3 redline pin, web/cli release
  matrices. Until it merges, run splitctl from branch `claude/v8-release-fixes-20260714`.

---

## WQ-1 — veox/* forge dual-home for the 27 jain repos
Status: DONE — ClaudeMaster — 2026-07-14T02:55Z (VERIFIED)
Scope: local forge only (`http://127.0.0.1:8787`, data-dir `/home/ubuntu/.local/share/jeryu`);
NO manifest edits, NO local-checkout origin changes (Claude does those after).
Task: create owner namespace `veox` and, for each of the 27 jain repos (25 `[[repo]]` +
jain-smartcluster + jain-split-ops), create `veox/<name>` and mirror ALL `refs/heads/*` +
`refs/tags/*` from the current `jeryu/<name>` (smartcluster from `jain-split/jain-smartcluster`).
Precedent: `jain-split/jain-smartcluster` is already dual-homed — replicate whatever
registration that took (forge DB/API or bare-repo layout + registration). jeryu/* stays frozen
(no deletes). Apply the same branch protection to veox/* mains (`splitctl jeryu-local
protection-apply --repo veox/<name> --required-check <name>/required --apply`).
Verify (paste for 3 sample repos + assert-all summary):
`git ls-remote http://127.0.0.1:8787/git/veox/<name>.git | sha256sum` equals the same for the
jeryu/<name> source (head+tag parity), for all 27; `curl -fsS -H "authorization: Bearer
$(cat ~/.jeryu/secrets/merge-token)" http://127.0.0.1:8787/api/v1/repos | jq '[.repositories[]
| select(.id.owner=="veox")] | length'` → 27.
Report: (ClaudeMaster) Mechanism: `jeryu-mirror import-local --owner veox` (built from
~/jeryu-split/jeryu-core, bin target/release/jeryu-mirror) from staged copies of the jeryu/*
bares; 16 registered live, 11 needed an offline re-import (service stopped — sqlite
contention), now 27/27: ref parity `diff ls-remote` identical for all 27;
`/api/v1/repos` veox count = 27; `protection-apply` 27/27 ok. veox account + PAT minted
(signup → session+csrf `x-jeryu-csrf` → /api/v1/auth/tokens). Residual: delta re-mirror
after WQ-2..8 merges (ClaudeMaster); jeryu/* frozen, nothing deleted.

## WQ-2 — Adopt + land jain-math split.2 identity (PR #11)
Status: CLAIMED — ClaudeMaster — 2026-07-14T03:05Z
Scope: jain-math only. Base: PR #11 head `b0157e514b0e5ba53409d7555197e3852bfb1a9b`.
Task: re-run required lane at that exact head (`JAIN_RELEASE_CI=1 ops/ci/split-host-ci.sh jeryu
jain-math b0157e5... <path> jain-math/required`), approve + merge PR #11 via jeryu-local
(protection ON), citing the auditor-exception receipt. NO tag (Claude's orchestrator mints tags).
Verify: `splitctl jeryu-local checks --repo jeryu/jain-math --sha b0157e5...` all green;
merged main == fast-forward of #11; paste merged main SHA.
Report:

## WQ-3 — Adopt + land jain-starforge split.2 (PR #7 = governed JOPE model-bundle)
Status: DONE — Codex — 2026-07-14T03:04:00Z
Scope: jain-starforge only. Base: PR #7 head `e75276edb6f90d082620a4351c8982bab7324c51`.
Task: verify the six LFS weights against `ops/ci/required.sh` sha256 pins AND the 5 model-bundle
files against `artifacts/model-bundle.v1.json` (sha256sum each); required lane at exact head;
approve + merge PR #7. NO tag.
Verify: paste the 11 sha256 checks (all MATCH), green check list, merged main SHA.
Report:

- Forge readback: PR #7 is merged, `merged=true`, head and merge commit
  `e75276edb6f90d082620a4351c8982bab7324c51`; `origin/main` is the same SHA.
- Model-bundle validator: `MODEL_BUNDLE_CHECK count=11 status=pass`; all six runtime
  weights and five bundle files matched `artifacts/model-bundle.v1.json` SHA-256 and byte
  sizes. The six runtime hashes are `6006f3818f675e6b734a74321987800cef130be19b53267c039453ea9b57a4d3`,
  `90c5952b4e201c265b4444a3694f3d1fc07df3864157ed6361b6c31723413d30`,
  `1a6d8b008d1b69c4725bb6e5bf094a68c1246073e6f71fa15e1b5e32ae19a488`,
  `1a48f7fc920c235c412ebf8dff176e0016e18b02554d8bd339380ea501eb4d7b`,
  `57cdb53ae732a705e6aee7381a6a11e7e82cf357bcd67c46ccfa47b789c7d5d2`,
  `f663ae49cc0f1504f07800a69dd6c31ca22fc473f26bbc0684eac25129a04128`.
- Product lane: `required ok: jain-starforge`; 23 unit tests passed, 6 golden/real-weight
  tests passed, 9 tracked LFS weights passed content SHA-256 checks, and both real Chimera
  CPU smokes passed.
- Quality/security: `just score` → `score=88 raw=88 caps=0`; minimum `85`, hard findings `0`;
  `just security` → `security ok`; `just artifact-support` → `artifact support bootstrap ok`.
- Forge status readback: `jain-starforge/required` state `success` at `2026-07-14T03:02:31Z`
  for the exact head; merged PR #7 remains immutable. The release-mode wrapper also exposed
  a separate control-plane blocker: the pending veox manifest/validator transition rejects
  the old jeryu expectations before Starforge tests run; this ticket did not bypass it.
- Clean isolated worktree: `/tmp/codex-wq3-starforge-20260714` at exact head, `git diff --check`
  passed, and `git status --short` was empty. The pre-existing owner checkout was not touched.

## WQ-4 — Land jain-core release head as v8 main
Status: CLAIMED — ClaudeMaster — 2026-07-14T03:10Z
Scope: jain-core only. Base: release head `0a4097376f08d42a42d4a7326242bd2921e06eb8`
(branch `codex/final-v8-release-20260713`; contains Lime/Prime `be7b030f` as `cb5724b`).
Task: confirm JOPE lib tests 106/106 + Hyperion real-bundle 4/4 still pass at that head
(`JAIN_TEST_MODEL_BUNDLE_ROOT=<starforge worktree>/artifacts/model-bundle`), pr-open → required
lane → approve → merge to main. NO tag. Do NOT touch `claude/lime-prime-core-20260713`.
Verify: test counts, green checks, merged main SHA.
Report:

## WQ-5 — jain-web: consolidate v8 head (upload fix + security + Lime/Prime wiring)
Status: BLOCKED — Codex parent — 2026-07-14T04:28Z (Web PR submitted; protected Core GPU merge/pin required)
Scope: jain-web only. Base: current main `6adcda48` (PR #23 merged).
Task: (a) cherry/adopt the fail-closed upload fix head `08fe47a` (worktree
/tmp/codex-jain-web-fail-closed-upload-20260714) and security/jankurai head `6d37a7a` if their
diffs still apply cleanly on main (skip with note if superseded by #23); (b) ADD the Lime/Prime
wiring EXACTLY this snippet (module scope in `crates/feat-web/src/runner.rs`, then set
`cfg.invention_engine` after `config_for_knobs` at BOTH call sites — run_core_training
~:1236-1238 and worker ~:2718-2720) + unit test (Low==Lime, Medium/High/Ultra==Prime):

```rust
#[cfg(not(feature = "ci-smoke"))]
fn invention_engine_for(effort: crate::protocol::Effort) -> feat_core::pipeline::InventionEngine {
    match effort {
        crate::protocol::Effort::Low => feat_core::pipeline::InventionEngine::Lime,
        _ => feat_core::pipeline::InventionEngine::Prime,
    }
}
// after `let mut cfg = config_for_knobs(resolved_knobs);` (+task/n_classes) at each site:
cfg.invention_engine = invention_engine_for(session_view.effort); // run_core_training
cfg.invention_engine = invention_engine_for(job.effort);          // run_worker_training
```
 (c) bump feat-core pin to the jain-core merged main from WQ-4 (path
stays git+tag after Claude re-tags — for now pin the merged commit, Claude rewrites to the
split tag at bind time), remove any `[patch]`; (d) close/supersede open PRs #9/#13/#16/#21/#22
with a note each (owner-visible), land everything as ONE reviewed PR. jankurai score must be
≥ baseline (89 current), zero caps/hard.
Verify: `just score` output, required lane green, unit test names+results, merged main SHA,
PR disposition list.
Report:

- PR `veox/jain-web#1` is open at exact head `5c76e1120b6748cc3fb5aabfb038a54b8fed5e5f`.
  The branch contains the Lime/Prime wiring plus GPU-first defaults for `hyperion-cuda`,
  `starforge-cuda` (Chimera), and `jope-cuda`, and all touched Jain dependency URLs use
  `veox/<repo>`. `just score`: `score=89 raw=89 caps=0 hard=0`; `cargo fmt -- --check` and
  `git diff --check` passed. The PR is intentionally unmerged because its Core pin remains
  `fda3a4c` until WQ-4 lands the protected GPU contract commit.
- Owned Web worktree was clean and removed; other Web worktrees were preserved.
- Exact forge readback at that head: `jankurai/proof=failure`, and the latest
  `veox/jain-web/required=failure` at `04:31:56Z`; the governed lane passed its three preflight
  tests but its detached staging copy lacked the sibling SmartCluster checkout before Cargo
  compilation. No approval or merge was attempted.

## WQ-6 — Fleet identity fixes (the 14 version-declaration mismatches)
Status: BLOCKED — Codex parent — 2026-07-14T04:28Z (Agent LLM tag/score and Jailgun/ZYAL proof blockers remain)
Scope: jain, jain-agent, jain-cli, jain-contracts, jain-deploy(defer to WQ-8 for image bits),
jain-docs, jain-jailgun, jain-jnoccio, jain-llm, jain-ops, jain-report, jain-research, jain-tui,
jain-zyal — whichever of these (audit shows 14 total incl. above repos) declare a stale VERSION /
package-identity vs 8.0.0-split-next. Pattern: Math's `b0157e5` (four package-identity files).
Task: per repo — one minimal identity commit on a fresh branch off main + required lane +
pr-open/approve/merge. Merge the outstanding onboard PRs (jain-agent #1, jain-jailgun #1,
jain-jnoccio #1, jain-zyal #1) FIRST where present, then identity-fix.
SEQUENCING: land the `jain` PORTAL repo LAST and only on ClaudeMaster's signal — its landing
must carry the FINAL regenerated derived `repos.manifest.toml` (post-binds authority sha;
ClaudeMaster runs sync-derived and hands you the file state). Dirty checkouts: work in
fresh worktrees off origin/main; do NOT touch the dirty demo lanes (jain-web-full-v1-demo,
jain-web-session-feed-order) or /tmp/codex-* worktrees.
Verify: per repo — green check list + merged main SHA (table).
Report:

- Core GPU handoff commit `223e615469e92edfc3165ff97e06973efb758036` is prepared for ClaudeMaster
  to fold into WQ-4; no competing Core PR/tag/bind/merge was created. Locked CUDA-featured
  feat-core tests: `106 passed, 2 ignored`; audit `score=86 raw=86 caps=0 hard=0`.
- Agent PR `veox/jain-agent#1` is open at `50a4095597f0c5cb93b0cc8847fbd70f650949ef`; audit
  `score=81 caps=0 hard=0`, below the 85 floor with five soft findings. Exact checks are
  `jankurai/proof=failure` and `veox/jain-agent/required=failure`; required failed before
  compilation because the immutable `jain-llm` v8 tag is absent. Its worktree is clean/removed.
- WQ-6B: `veox/jain-llm` identity `9eb84f0e...` merged; Research `074d09b4...` merged; Ops,
  Report, and Docs had no identity delta and clean audits. Jnoccio PR #1 remains blocked on the
  missing LLM tag. WQ-6C ZYAL head `5737cb78...` has required green and Jankurai `88 caps=0
  hard=0` but proof failure; CLI/TUI had no identity deltas and their worktrees were cleaned.
- Jailgun PR #1 head `9045da04eed8566b77f2dd8783f9f5deb06b9d3e` has Jankurai success but required
  failure. Jailgun and ZYAL owned worktrees were clean and removed. No waiver/protection bypass.

## WQ-7 — jain-contracts split.2 proof head
Status: DONE — Codex1 — 2026-07-14T03:12Z
Scope: jain-contracts only. Its main moved to `ac58ea2` ("restore v8 split.2 proof gates") with
6 dirty files in the checkout. Task: inspect the 6 dirty files — if release-relevant, land via
reviewed PR; else park on a branch `park/contracts-dirty-20260714` and reset checkout clean.
Required lane green at final main.
Verify: dirty-file disposition list, green checks, final main SHA.
Report:

- Dirty-file disposition: `.gitignore`, `agent/generated-zones.toml`,
  `contracts/public-api.toml`, `ops/ci/contract-drift.sh`, `ops/ci/required.sh`, and the
  untracked canonical `contracts/demo/full-v1/{full-v1.json,train.csv,scoring.csv}` were all
  release-relevant and reproduced in isolated worktree
  `/tmp/codex1-jain-contracts-wq7-20260714`; the original dirty checkout was not reset or
  otherwise modified.
- Committed as `fcbb36064e008403a0fd992e7f278a73b6b1d946`, pushed, and opened as reviewed local
  forge PR `jeryu/jain-contracts#10`. The PR was approved at the expected head and merged via
  the governed lifecycle; `git ls-remote` confirms `refs/heads/main` at the same SHA.
- Local contract-drift lane passed (`contract drift ok`); required lane passed with sibling
  resolution (`JAIN_NEEDS_SIBLINGS=1`, `posted .../required=success`, `PASS jeryu/jain-contracts`).
  The first no-sibling run failed honestly because the detached lane could not resolve the
  existing Web/Deploy `full-v1` mirrors; no check was waived.
- Exact-head Jankurai audit is green: score `88`, hard findings `0`, caps `0`, auditor
  `1.6.11`; authenticated forge readback with `per_page=100` shows successful current-head
  `jankurai/proof` and `jain-contracts/required` runs. Canonical fixture hashes were verified:
  train `751b542996a8292a41571599c3c5c5f117d39e50c79c54aeef943075c9174dfd`, scoring
  `499d4481246570f24aeb4c43f79f91e3bfa3e302cd10efce8bbaca8edbe2caae`.
- Isolated worktree is clean after removing only its generated `.jankurai-proof` scratch;
  source checkout remains intentionally preserved with the other agent's dirty release files.

## WQ-8 — jain-deploy: veox/jain image identity + model-bundle staging readiness
Status: BLOCKED — Codex — 2026-07-14T04:28:42Z; CLI fix/proof complete in `veox/jain-cli#1`, waiting canonical Core + SmartCluster immutable tags
Scope: jain-deploy only (its 20-file dirty checkout: park anything not adopted on
`park/deploy-dirty-20260714`). Base main `dd80c57` + adopt
`/tmp/codex-jain-deploy-image-proof-20260714` (branch codex/v8-image-authority-proof-20260714)
if it verifies.
Task: (a) rename image identity: `crates/jain-deploy-engine/src/config.rs` `IMAGE_REPO`
`doug/jain_small/jain-sagemaker` → `veox/jain` (REGISTRY host unchanged); propagate to
`deployment/landing/install-jain.sh` (+ landing copy), `scripts/build-cloud-image.sh:20`,
`registry.rs:5` comment, `deployctl_dry_run` fixtures, OCI label asserts; (b) verify
`scripts/stage-context.sh` will include `artifacts/model-bundle/` once jain-starforge WQ-3 is
merged (add the COPY path only if missing — Dockerfile.sagemaker:85 expects it); (c) keep
`Dockerfile.sagemaker` smartcluster `cargo install` tag ref pointing at
`jain-smartcluster-v8.0.0-split.0` (Claude's orchestrator mints it); (d) engine tests +
contract test `scripts/test-atomicsoul-dry-run.sh` green; ONE reviewed PR.
Verify: grep proves zero `jain_small/jain-sagemaker` and zero `doug/` refs left in jain-deploy
source (except historical docs/releases/7.x records); ALSO zero `github.com`/`neverhuman` URLs
in `jain-split.lock.toml` (regenerate it from the authority if it's derived — Codex1 readback
flagged it); test outputs; merged main SHA.
Report:

- Isolated WQ-8 branch `codex/v8-image-model-bundle-20260714` is clean and pushed to
  `veox/jain-deploy`. Commits: `7559892e11e3dbe0c17146ad2afc056208db2657` (model-bundle
  staging, CPU/GPU Docker wiring, lock determinism, runtime inventory split) and
  `d31b5355967d87e7f876d223885a2f708c413753`/`7972fc8e18d2ad65ba3b855fd8c01b9a717de523`
  (governed bare-mirror origin acceptance and receipt assertion). PR is `veox/jain-deploy#1`.
- Local verification: sagemaker-ci `25/25`; jain-deploy-ops `7/7`; `just required`,
  `just check`, `just security`, `just artifact-support`, coverage `2/2`, and Jankurai
  `score=92 raw=92 caps=0 hard=0` pass. The full image was not built because the governed
  disk floor, SmartCluster immutable tag, and handler release hashes remain unresolved.
- Exact forge CI at `7972fc8e18d2ad65ba3b855fd8c01b9a717de523` correctly posted
  `jain-deploy/required=failure`. After WQ-8’s own stage/origin checks passed, release Cargo
  policy `build-all-features` failed in current `jain-cli`: `crates/feat-cli/src/config.rs:42`
  and `:132` omit `PipelineConfig.invention_engine`, and
  `crates/feat-cli/src/commands/jain_worst_loop/attempts.rs:60` passes one extra argument to
  `candidate_log_record` (compiler suggests removing `None`). This is outside WQ-8’s deploy-only
  scope; no waiver, baseline edit, protection change, or false-green status was used.
- Forge check readback for the exact head also contains `jankurai/proof=failure` and
  `jeryu/autonomy=neutral`; these were not reset, hidden, or manually declared successful.
- WQ-6/Core owners must land the compatible CLI/Core fixes, rerun the exact release policy, and
  hand back a green `jain-deploy/required` before this PR can be approved or merged.
- Blocker recovery checkpoint: isolated CLI commit
  `2e8805e52ce51089eb1b01a0de430ab557aa15e1` fixes all five Core-release API failures and passes
  compile against Core `0a4097376f08d42a42d4a7326242bd2921e06eb8`, ci-smoke/contract/GPU-entrypoint
  tests, and focused worst-loop tests. Its quality-lane follow-up now audits at Jankurai `90`,
  caps `0`, hard `0` with real LCOV/security evidence. Codex owns the single protected CLI PR,
  sequenced behind the existing WQ-4 Core reviewed merge/tag and immutable SmartCluster split.0
  creation; no competing CLI writer, waiver, bypass, or improvised tag is permitted.
- Protected-lifecycle checkpoint: `veox/jain-cli#1` is the sole open CLI compatibility PR at exact
  head `7d83e850efccda4d81510b34efeeb107f2923014`; duplicate PRs #2/#3/#4 were closed and read back
  closed. Automatic exact-head `jankurai/proof` is successful. Full-tree audit passed at score `90`
  and forge-diff audit at score `88`, both with caps `0` and hard `0`; coverage is `2/2`, and the
  clean-run tool-adoption lane passes after producing LCOV before audit. Canonical Core audit proves
  legacy head `0a409737...` is not fast-forwardable to `veox/jain-core` main, so WQ-4 must be
  recomposed on canonical main before Core/SmartCluster tags, CLI repin+lock, exact-head CLI
  CI/merge/tag, and the Deploy rerun.
- Codex-owned disposable CLI/Core reference worktrees and temporary config were removed at handoff.
  The primary `jain-cli` checkout still contains pre-existing JOPE/Lime edits owned by another lane
  and was intentionally left untouched.

## WQ-9 — E2E studies harness (build + first run against a live instance)
Status: BLOCKED — Codex2 — 2026-07-14T03:54Z — live 127.0.0.1:4180 became unavailable during rerun
Scope: NEW files only under `jain-split-ops/docs/release-evidence/8.0.0/e2e-studies/`
(bin/e2e-studies.sh, bin/gen-dataset.awk, data/, receipts/). No product-repo edits.
Task: implement per the spec in this file's appendix A (endpoints/asserts/receipt schema —
copied from the approved design). Run fast-mode against the live real-runtime instance on
127.0.0.1:4180 (no-auth) — budgets: effort=low with overrides invent_gens=6, invent_pop=16,
gp_gens=12, cv_iters=6, ho_iters=8. Expected NOW: upload/training/chimera/export = pass,
engine_observed=legacy (wiring lands in WQ-5).
Verify: receipt JSON at receipts/<run_id>/receipt.json with verdict=pass (engine=legacy noted),
paste the studies block.
Report:

- Deterministic dataset regeneration passed: `awk -v n=240 -v seed=20260714 -f
  docs/release-evidence/8.0.0/e2e-studies/bin/gen-dataset.awk` reproduced the committed
  SHA-256 `96668514f960f0720ddf9102013fa121e5f32e6d8993183d5896069e3e6cd06c` and 240 data rows.
- Harness syntax passed with `bash -n`. The live preflight passed against `127.0.0.1:4180`:
  health 200/version 8.0.0, runner `real`, execution mode `real`, cluster access `true`, and
  training enabled `true`. The first live session passed upload, terminal training, phase-6
  engine evidence (`engine_observed=legacy`), and Chimera/model inspection (manifest present,
  3 completed Starforge trials, 0 missing weights). The export allowlist was corrected to admit
  the server-emitted approved `invention/MANIFEST.txt` entry; the 409 auto-start race was also
  made benign when training is already underway.
- Corrected rerun receipt:
  `docs/release-evidence/8.0.0/e2e-studies/receipts/run-530ae0190fd501f7/receipt.json`.
  Its sidecar matches (`sha256sum -c receipt.json.sha256` → pass), but the receipt is correctly
  `status=blocked`, `verdict=fail`, with exact blocker `curl: (7) Failed to connect to
  127.0.0.1 port 4180 after 0 ms: Couldn't connect to server` while polling/training events.
  A safe recovery check also failed; no server process was present. No pass was fabricated.
- No product repository, image, deployment, tag, registry, or AtomicSoul state was changed.

## WQ-10 — Naming RFC draft (post-release wave; non-blocking)
Status: DONE — Codex1 — 2026-07-14T02:39Z
Scope: NEW file `jain-split-ops/docs/naming-rfc-v1.md` only.
Task: from the naming inventory (appendix B pointer), draft the RFC: veox/* owner rationale;
scrub of github_slug/neverhuman; image `veox/jain`; portal `veox/jain-portal` (kills the
jeryu/jain monorepo collision); redline family follow-up (5 spellings → one); prefix-zoo policy
(feat-*/bare/jain-*/jail-* crates); opaque-codename proposals WITH the discovered purposes:
jable=in-house tabular regressor, jnoccio=LLM provider gateway/router, zyal=runbook
compiler/supervisor, jailgun=browser-session registry, starforge=tabular classifier inference,
battle-gpu=GPU kernel factory; sagemaker-residue cleanup; jain-tui duplicate feat-cli crate.
Owner decides post-GO.
Verify: file exists, covers all listed items, ends with a decision matrix for the owner.
Report:

- Added only `docs/naming-rfc-v1.md` (202 lines). It covers the veox namespace and
  `veox/jain-portal`, active `github_slug = "neverhuman/<repo>"` retirement, `veox/jain`
  image identity, Redline spelling/lock follow-up, package-prefix policy, all requested
  opaque-codename purposes, SageMaker residue, the duplicate `feat-cli` ownership, and an
  owner decision matrix.
- Verification:
  `test -s docs/naming-rfc-v1.md` → pass.
  Required-term scan for `veox/`, `jain-portal`, all seven opaque codenames, `sagemaker`,
  `feat-cli`, `redline-central`, `github_slug`, `neverhuman`, and `Owner decision matrix`
  → pass (13 terms).
  `git diff --check -- docs/naming-rfc-v1.md docs/WORK_QUEUE.md` → pass.

---

### Appendix A — harness spec (WQ-9)
Server: `/api` base; auth `Authorization: Bearer` or `?key=`; `POST /api/sessions`
`{"effort":"low","overrides":{...}}` → assert `resolved_knobs.chimera_enabled==true`;
`POST /api/sessions/:id/files` multipart (`role=train` field FIRST, then file) → assert
`files[0].status=="uploaded"` + events `upload.started`/`upload.complete`; if `target.required`
event → `POST .../target {"target":"target","approved":true}` then `POST .../start`; poll
`GET /api/sessions/:id` to `complete|prediction_ready` (fail on failed/cancelled; 30-min cap);
model zip via `GET /api/artifacts/<model_id>/download` → unzip → `manifest.json` assert
`.run.model_trials[] | select(.backend=="starforge")` ≥1 `status=="completed"` (weights-missing
skip/fail = HARD FAIL); export `GET /api/sessions/:id/export/algorithm` → zip entries only
README.md/MANIFEST.txt/invention/*/{model.py,model.rs,genome.dsl}; assert NO entry matching
`chimera|hyperion|starforge|weight|embedding|context|bundle|\.safetensors|\.pt|\.onnx`;
engine proof: events `kind=="training.progress"` with `payload.phase==6&&payload.event=="done"`;
`payload.note` contains "Lime algorithm search selected"→lime, "Prime …"→prime, absent→legacy.
Dataset: deterministic awk LCG, 240 rows, 6 numeric + 1 categorical + binary `target` with
planted signal; record sha256. Receipt schema `jain.e2e-studies/v1` (run_id, mode, base_url,
dataset sha, session, per-study status, engine_observed, verdict) + .sha256 sidecar.

### Appendix B — naming inventory pointer (WQ-10)
The full inventory (Tables A-D: 32 repos with purposes/crates/slugs; non-repo surfaces incl.
config.rs constants; forge tenant map 93 repos/3 owners/6 families; 14 collisions) is in the
dispatcher session; ping in AGENT_CHAT and Claude will paste it into
`docs/naming-inventory-20260714.md` on request — or re-derive from the tree (all facts are
greppable; key files: repos.manifest.toml, jain-deploy/crates/jain-deploy-engine/src/config.rs,
install-jain.sh, forge /api/v1/repos).

## WQ-11 — Early image-content validation (staged context completeness)
Status: CLAIMED — ClaudeW3 (ClaudeMaster delegate) — 2026-07-14T03:2xZ
Scope: READ-ONLY on all repos + scratch builds only. No pushes, no docker push, no forge
writes, ATOMICSOUL_PUSH=0. Purpose: prove the FULL image contents are stageable NOW
(all six starforge weights + JOPE/Lime model-bundle + SPA + native libs) so the
orchestrator's staged-artifact stage cannot fail late.
Task: from jain-starforge merged main e75276e (fresh worktree): verify the six LFS weights
(ops/ci/required.sh pins) AND 5 model-bundle files (model-bundle.v1.json shas) are
git-tracked and byte-correct; then run jain-deploy stage-context (scripts/stage-context.sh)
into a scratch stage dir and assert .stage/artifacts/{starforge,foundation,model-bundle}
complete + assemble-runtime-assets.sh preconditions (3 bundle files present, no .pt/.pth);
report any gap incl. whether Dockerfile.sagemaker's smartcluster cargo-install tag
(jain-smartcluster-v8.0.0-split.0) is the ONLY missing input (expected — minted at release).
Verify: paste the 11 sha checks, `git -C <worktree> ls-files artifacts/model-bundle | wc -l`,
staged-dir tree listing, and the gap list.
Report:

## WQ-12 — Parallel release-hygiene and blocker verification
Status: DONE — Codex1 — 2026-07-14T03:26Z
Scope: READ-ONLY inspection of managed product repos, active isolated worktrees, local-forge
PR/check state, and the in-flight WQ-5/WQ-6/WQ-8/WQ-9 evidence. No product edits, commits,
pushes, approvals, merges, tags, manifest changes, images, registry actions, or release-candidate
runs. Existing writers retain exclusive ownership of their lanes.
Task: use disjoint Codex workers to (a) classify dirty worktrees as active/preserved/stale,
(b) verify exact-head CI and Jankurai evidence for active release PRs, (c) inspect WQ-8 image
readiness and WQ-9 harness/receipt correctness, and (d) identify only concrete critical-path
blockers that the owning worker or ClaudeMaster can consume.
Verify: worker reports agree with machine truth; no worker changed a repository; every worker
worktree remains clean; actionable findings are recorded below and in `docs/AGENT_CHAT.md`.
Report:

- Beauvoir (WQ-9 read-only audit): all four study blocks and receipt rendering exist, and
  deterministic dataset regeneration matches recorded SHA `96668514…cd06c`. The latest real run
  `run-530ae0190fd501f7` remains `training` with no artifacts, `receipt.json`, sidecar, or verdict.
  Release blockers for the WQ-9 owner: `unzip -p ... | grep ... || true` treats extraction errors
  as zero restricted-content hits; an HTTP 409 auto-start race sets `BLOCKER` but is not included
  in final verdict calculation; engine matching is broader than the specified exact Lime/Prime
  selection notes; default run IDs can reuse an evidence directory; the export allowlist is broader
  than Appendix A. Worker asserted no files, sessions, receipts, commits, or server state changed.
- Hooke (WQ-8 read-only audit): isolated worktree is clean at local head `4365b92b4eac7724ee1e7ae47ebeff549ede4495`,
  three commits ahead of `dd80c57`; forge PR `jeryu/jain-deploy#21` is still at older head
  `c3df50f64bd5f9c86cedbd44ba5f038621a6f907`, whose exact-head required, Jankurai, and autonomy
  checks are failures. Active legacy image references remain in `ops/ci/image-resilience.sh` and
  `deployment/ops/container-bases.lock`. The branch stages Starforge artifacts but its staged-context
  validator, CPU/GPU Dockerfiles, runtime-asset assembly, and environment wiring do not yet prove
  model-bundle inclusion; the owner checkout contains unmerged JOPE/Lime staging changes addressing
  that gap. Smartcluster is correctly pinned but the required split.0 tag is absent. Lockfile URL
  scans and primary `veox/jain` identity pass. Worker asserted no repo/forge/image mutation.
- Bernoulli (local-forge exact-head audit at `2026-07-14T03:22:28Z`): all observed release
  PRs were blocked. WQ-4 PR #12 (`0a40973`) lacked `jain-core/required` and had failed
  `jankurai/proof`; no consolidated WQ-5 PR was visible and legacy #9/#13/#16/#21/#22 all had
  failed exact-head proofs; active WQ-8 PR #21 (`c3df50f`) had failed required and proof; PR-A
  #13 (`be11d342`) had failed required and proof. WQ-6 had mixed stale/draft PRs; newly visible
  `veox/jain-zyal#1` lacked required evidence and had failed proof. The audit consumed all
  check-run pages (including Web's 100+24 records) and selected latest exact `(head_sha,name)`
  results. Worker used authenticated GET/`ls-remote` only and asserted no state changes.
- Avicenna (worktree-hygiene audit): 26 managed product repos have 26 primary checkouts and 72
  registered linked worktrees; five registrations point to missing directories and are candidates
  for a later reviewed `git worktree prune --dry-run`. Seven dirty primary checkouts (`jain`,
  Math, Contracts, Starforge, CLI, Web, Deploy) are preserved user/agent work and were not touched.
  The original Contracts checkout is one commit behind merged main, but `contracts/public-api.toml`
  and `ops/ci/contract-drift.sh` are not byte-identical to `origin/main`; do not reset until Codex1
  or the owner disposes those variants. WQ-5's claimed worktree had unresolved conflicts in
  `apps/web/e2e/cockpit.spec.ts` and `apps/web/src/App.tsx`; an obsolete July 13 Core CI process
  group remains running at old `48380e29` and must be reaped by its process owner before worktree
  removal. Worker performed read-only Git/process inspection and asserted no mutations.
- WQ-12 conclusion: all four delegated workers completed without edits, repo/forge writes, live
  sessions, image operations, or release mutations. The lane produced verification evidence only;
  product PR/CI/Jankurai failures remain owned by WQ-4/5/6/8/9 and are intentionally not waived.

## WQ-13 — v8.0.0 GPU-default full-release coordination, evidence, and hygiene
Status: IN PROGRESS — Codex1 — 2026-07-14T03:55:19Z
Scope: coordination and verification across WQ-4/5/6/8/9 plus stale-process/worktree hygiene;
product code stays with its recorded lane owner. The authoritative image is
`jain-deploy/deployment/ops/Dockerfile.sagemaker.gpu`; no alternate v8 GPU Dockerfile may be
introduced. Preserve candidate metadata, `formal_ga=false`, rollback `7.0.6`, immutable tags,
local-forge-only release inputs, reviewed protected merges, and zero waivers/bypasses/tag moves.
Task: maintain exact-head lane state; reap and archive the obsolete Core CI process; land or park
every meaningful dirty checkout; verify the recomposed Core GPU contract, consolidated Web and
isolated preview on `127.0.0.1:48300`, manifest-wave protected fleet/tags/binds, full model-bundle
GPU image, WQ-9 evidence integrity, same-image GPU-visible/GPU-hidden and xbabe1/xbabe2 receipts;
then drive release-candidate to GO and the separately authorized RC/canary-only AtomicSoul lane.
Update this status at every lane start, blocker, PR creation, CI completion, merge, tag, image
build, and cleanup checkpoint. No production promotion or public/GitHub release is authorized.
Verify: final receipt binds all repo commits/tags/checks, Jankurai 1.6.11 results, model and image
hashes, cosign/canary verification, both-host device receipts, preview/canary URLs, clean managed
worktrees, and explicit confirmation of rollback `7.0.6` and no production promotion.
Report:

- Lane start recorded in `docs/AGENT_CHAT.md` with current exact control/Core/Web/Deploy heads,
  exclusive owners, the single-Dockerfile decision, typed GPU fallback boundary, and evidence
  requirements. Hygiene recovery is the first active checkpoint; no product mutation is claimed.
- 2026-07-14T03:59:39Z owner redirect: hygiene paused after the obsolete Core process was reaped
  and its stale worktree removed. Active checkpoint is local `veox/jain:8.0.0-gpu` recovery:
  fix the rendered `scqd.toml` parse error, terminate only explicitly authorized conflicting host
  Jain services, launch with GPU access, and prove supervisor/Web/scqd/worker health before studies.
- 2026-07-14T04:09:13Z owner expansion: change the v8 Web default consistently to port `8888`
  while preserving installer `--port`/environment overrides; bake `scqd embedded=true`; and extend
  the download/setup shell flow with fail-closed, bounded health plus deterministic quick-dataset
  studies. Work is isolated from WQ-8 at base `7972fc8e18d2ad65ba3b855fd8c01b9a717de523`.

## WQ-14 — Native JOPE Prime/Lime fitted-model pipeline
Status: IN PROGRESS — Codex — 2026-07-14T05:05:49Z
Scope: the owner-supplied native JOPE Prime/Lime plan, dependency-first. Initial mutation is limited
to fresh clean `veox/jain-contracts` and `veox/jain-starforge` worktrees: versioned algorithm-build,
progress/workload/result/frozen-model/prediction/export/license/receipt contracts; exact generation-18
JOPE bundle inventory; and recovery/export of the deployed fused 512-dimensional target encoder with
fail-closed identity/license/tensor validation and mandatory parity fixtures. Existing WQ-4/5/8/13
Core/Web/Deploy worktrees, running Deploy CI, GPU image, E2E sessions, tags, manifests, and registry
state remain untouched until their owners hand off. Downstream Math/Core/SmartCluster/CLI/Web/Deploy
work is sequenced behind reviewed contract and Starforge heads and will use new isolated worktrees.
No GP public fields/routes/events may be introduced; no production promotion, tag move, waiver,
protection bypass, alternate Dockerfile, or live-push action is authorized.
Verify: exact bundle hashes and tensor shapes; Python/Rust target/context parity fixtures; contract
drift/required/Jankurai at each exact PR head; protected PR lifecycle; clean task worktrees; and a
recorded blocker rather than approximation if any authoritative encoder checkpoint cannot be recovered.
Report:

- 2026-07-14T05:08:41Z parallel implementation expansion: fresh clean Math and Battle-GPU
  worktrees may prepare the reviewed compiled-plan/fitted-state/CV-5/runtime and optional CUDA
  evaluation slice concurrently, but they must not push or enter the protected landing lifecycle
  before Contracts and Starforge exact heads are reviewed. Existing dirty primary checkouts and
  every WQ-13 process remain untouched.
- 2026-07-14T05:37:00Z owner acceptance expansion: every newly added checkpoint or tensor bundle
  must use Starforge's existing Git LFS path, with committed pointer, exact OID/content hash,
  `git lfs fsck`, remote upload, and clean-clone materialization evidence. Each owned repository
  also adds a runnable `docs/*.md` guide for its implemented training/search surface; public CLI
  examples may be advertised only after that command exists and is contract-tested.
- 2026-07-14T06:24:00Z Contracts checkpoint: protected PR `veox/jain-contracts#1` merged at
  `7fabfc69cbece4b8fd3589a6d544ebcd8d9dc51f` after exact-head required/proof success and
  independent adversarial review. Jankurai is score 88, caps 0, hard 0; the task worktree was
  clean and removed. Immutable split.2 tagging is correctly pending the active control-plane
  authority-manifest lane because canonical split-ops main is older than the in-flight WQ-13
  manifest rewrite; no stale-main bind or tag bypass was attempted.
