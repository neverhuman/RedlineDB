# jain-split closeout evidence (2026-07-06)

Four bars: valid CI everywhere, clean worktrees, healthy jankurai (>=78), checked into jeryu.

## Bar 1 — valid CI (all 19 GREEN)
- Cross-repo deps resolve from the auth-free `file://` bare mirrors (target/bare-mirrors), NOT
  committed vendor-crates and NOT the forge git-http (which now requires auth). `split-host-ci.sh`
  rewrites `github.com/neverhuman -> file://target/bare-mirrors/` (CI-scoped GIT_CONFIG_GLOBAL under
  target/). Proven: `cargo fetch --locked` resolves the whole closure incl. all learner crates,
  identical crate graph, zero github. `independence-gate.sh` passes every non-deploy repo offline,
  no siblings, empty git cache.
- `ops/ci/score.sh` GREEN on all 19 (committed-baseline ratchet, hard=0).

## Bar 2 — clean worktrees (18/19)
- 18/19 tracked-clean. jain-deploy is codex's active deploy-proof tree (its lane).

## Bar 3 — jankurai >=78 (18/19)
- jain 86, model-zoo 87, python 83, contracts 82, tui 82, deploy 82, ops 81, starforge 81, math 80,
  xgboost/lightgbm/docs 79, domain/catboost/battle-gpu/core/report/cli 78.
- jain-web 66: BINDING `direct-db-access-from-wrong-layer` is a jankurai DETECTOR FALSE-POSITIVE
  (App.tsx DOM/English tokens select/delete/drop read as SQL). The monorepo hits the identical cap.
  Un-clearable by config (reclassification supports only 3 Python caps); un-liftable without a
  behavior change. Documented in jain-web/SPLIT.md; held green at its ratchet baseline.
- Regression pivot (vendor-crates -> forge/mirror) and all lifts verified NO-REGRESSION:
  math (champion_parity+golden+6 new property tests green), starforge (real-weight predictions
  BIT-IDENTICAL deduped vs original), every repo's required lane re-run green.

## Bar 4 — checked into jeryu (19/19)
- Advanced every `jeryu/<name>.git` bare via the canonical direct FF `update-ref` (onboard.sh
  mechanism, no auth): forge main == local HEAD for all 19 (+ portal on jeryu/jain-portal-preview).
- Split tags IMMUTABLE at seed (precedent: jeryu-core tag stays behind main; consumer locks pin the
  seed sha). 10/10 dependency-repo tags == consumer-lock seed shas. Non-dep tags track HEAD (free).
- family.lock refreshed to post-lift HEADs; validate-family PASS. Legacy jeryu/jain untouched
  (apex/docs-user-guide/v7.0.1 only, no main, no split tag). codex/portal-score-lane preserved.

## Known / coordinated
- Loose split control-plane (ops/ci/split-host-ci.sh, ops/lib.sh, ops/split/*) is untracked at the
  split root (not a git repo) — same known limitation as jeryu-split; tracked-home TBD with codex.
- Forge git-http requires auth for reads; CI uses the auth-free mirrors. JAIN_CI_USE_FORGE=1 opts in.
