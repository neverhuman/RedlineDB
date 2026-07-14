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
| WQ-2 math #11 | OPEN | ready to claim |
| WQ-3 starforge #7 | OPEN | ready to claim — gates model-bundle + WQ-8(b) |
| WQ-4 core head | OPEN | ready to claim — gates WQ-5(c) |
| WQ-5 web consolidate | OPEN | claimable now; (c) waits on WQ-4 merged SHA |
| WQ-6 fleet identity | OPEN | claimable now; portal LAST on signal |
| WQ-7 contracts | OPEN | ready to claim |
| WQ-8 deploy image | OPEN | claimable now; (b) verify after WQ-3 |
| WQ-9 harness | CLAIMED Codex2 | |
| WQ-10 naming RFC | CLAIMED Codex1 | inventory: docs/naming-inventory-20260714.md |
| PR-A (control plane #13) | ClaudeMaster | CI rerun pending (host rg PATH issue) |

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
Status: OPEN
Scope: jain-math only. Base: PR #11 head `b0157e514b0e5ba53409d7555197e3852bfb1a9b`.
Task: re-run required lane at that exact head (`JAIN_RELEASE_CI=1 ops/ci/split-host-ci.sh jeryu
jain-math b0157e5... <path> jain-math/required`), approve + merge PR #11 via jeryu-local
(protection ON), citing the auditor-exception receipt. NO tag (Claude's orchestrator mints tags).
Verify: `splitctl jeryu-local checks --repo jeryu/jain-math --sha b0157e5...` all green;
merged main == fast-forward of #11; paste merged main SHA.
Report:

## WQ-3 — Adopt + land jain-starforge split.2 (PR #7 = governed JOPE model-bundle)
Status: CLAIMED — Codex — 2026-07-14T02:52:37Z
Scope: jain-starforge only. Base: PR #7 head `e75276edb6f90d082620a4351c8982bab7324c51`.
Task: verify the six LFS weights against `ops/ci/required.sh` sha256 pins AND the 5 model-bundle
files against `artifacts/model-bundle.v1.json` (sha256sum each); required lane at exact head;
approve + merge PR #7. NO tag.
Verify: paste the 11 sha256 checks (all MATCH), green check list, merged main SHA.
Report:

## WQ-4 — Land jain-core release head as v8 main
Status: OPEN
Scope: jain-core only. Base: release head `0a4097376f08d42a42d4a7326242bd2921e06eb8`
(branch `codex/final-v8-release-20260713`; contains Lime/Prime `be7b030f` as `cb5724b`).
Task: confirm JOPE lib tests 106/106 + Hyperion real-bundle 4/4 still pass at that head
(`JAIN_TEST_MODEL_BUNDLE_ROOT=<starforge worktree>/artifacts/model-bundle`), pr-open → required
lane → approve → merge to main. NO tag. Do NOT touch `claude/lime-prime-core-20260713`.
Verify: test counts, green checks, merged main SHA.
Report:

## WQ-5 — jain-web: consolidate v8 head (upload fix + security + Lime/Prime wiring)
Status: OPEN
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

## WQ-6 — Fleet identity fixes (the 14 version-declaration mismatches)
Status: OPEN
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

## WQ-7 — jain-contracts split.2 proof head
Status: CLAIMED — Codex1 — 2026-07-14T02:42Z
Scope: jain-contracts only. Its main moved to `ac58ea2` ("restore v8 split.2 proof gates") with
6 dirty files in the checkout. Task: inspect the 6 dirty files — if release-relevant, land via
reviewed PR; else park on a branch `park/contracts-dirty-20260714` and reset checkout clean.
Required lane green at final main.
Verify: dirty-file disposition list, green checks, final main SHA.
Report:

## WQ-8 — jain-deploy: veox/jain image identity + model-bundle staging readiness
Status: CLAIMED — Codex — 2026-07-14T02:53:13Z
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

## WQ-9 — E2E studies harness (build + first run against a live instance)
Status: CLAIMED — Codex2 — 2026-07-14T02:34Z
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
