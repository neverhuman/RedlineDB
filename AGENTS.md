# jain-split-ops — jain split family control plane

This repo is the single tracked source of the jain-split control plane: the
family manifest (`repos.manifest.toml`), the materializer + reconcile/rollout
tooling (`ops/split/*`), the host CI runner (`ops/ci/split-host-ci.sh`), forge
onboarding (`ops/onboard.sh`, `ops/lib.sh`, `ops/hooks/*`), and the planning /
evidence docs (`docs/`).

It is a SIBLING of the family members under `/home/ubuntu/jain-split/`, not a
member of `repos.manifest.toml` (so `splitctl materialize --force` never touches it).
Each family member remains an independent git repo with its own CI and forge
remote; this repo only orchestrates them.

This control plane has the same zero-worktree rule as the family root: do not
create Git worktrees anywhere. Exact-SHA CI must use an automatically removed,
standalone checkout that is not registered with Git, and sibling dependencies
must be independent clones rather than symlinks.

## Runtime contract (no symlinks, explicit roots)
- The family root (where the sibling repos + `target/bare-mirrors/` live) is the
  EXPLICIT `JAIN_SPLIT_ROOT` (default `/home/ubuntu/jain-split`), never derived
  from a script's location. `ops/ci/split-host-ci.sh` asserts it.
- This repo's own path is derived by the tooling (`splitctl materialize` `ROOT =
  parents[2]`; `rollout-pr-flow.sh`/`closeout-prs.sh`/`cutover.sh` compute
  `ops_root` from `$BASH_SOURCE/../..`) so it is relocatable.
- Bootstrap uses the native Rust control-plane commands in `splitctl`; the legacy
  materializer is being ported and must not gain new Python dependencies.

## Local Jeryu Forge Workflow

The Jain workspace is `/home/ubuntu/jain-split`. Do not use `~/jeryu-split` as
an operational source for Jain work; it is a precedent/product checkout, not a
member of this family.

Use normal Git commands against the local loopback Jeryu remote. Git credentials
are already supplied by local Git/HTTP credential storage, so fetch/push should
be fast and should not require agent-visible token handling.

Canonical family repo remote:
`http://127.0.0.1:8787/git/jeryu/<repo>.git`; infrastructure remotes are
declared explicitly in `repos.manifest.toml` and may use another forge owner.

For a quick check:

```bash
git remote -v
git ls-remote origin HEAD
```

If a repo remote is wrong or Git is slow/failing, run the Jain control-plane
repair once:

```bash
cd /home/ubuntu/jain-split/jain-split-ops
just jeryu-ready
```

That command checks the local forge, removes extra remotes from every live
checkout, sets `origin` to local Jeryu, registers Jain family metadata, and runs
the family policy validator. Do not inspect `~/.jeryu`, run `gh auth login`, or
copy source from Jeryu internals.

For PRs, checks, and merges, use `jeryu.*` MCP tools when they are exposed. If
they are not exposed, use
`cargo run --locked --manifest-path /home/ubuntu/jain-split/jain-split-ops/Cargo.toml -- jeryu-local` or the
`just jeryu-*` recipes from the control-plane repo.

Use `just jeryu-doctor` for a read-only health check. Run or post split CI
through `/home/ubuntu/jain-split/jain-split-ops/ops/ci/split-host-ci.sh`, not
GitHub Actions, unless an explicit GitHub mirror workflow is requested.

## Pre-E2E family gate

The portal owns bounded parallel family lanes. From `jain/`, run these before
starting any browser or live-service test:

```bash
JAIN_FLEET_JOBS=4 JAIN_CI_JOBS=4 just family-fast
JAIN_FLEET_JOBS=4 JAIN_CI_JOBS=4 just family-required
JAIN_FLEET_JOBS=4 JAIN_CI_JOBS=4 just family-security
JAIN_FLEET_JOBS=4 JAIN_CI_JOBS=4 just family-score
```

Each run writes attributable logs and a JSON summary under `jain/.ci-status/`.
Run `just refresh-mirrors` from this control-plane repo first when sibling
heads or split tags have changed; it refreshes only generated cache mirrors.
The web repository's required lane stops at Rust smoke, frontend unit tests,
typecheck, and build. Playwright belongs only to `jain-web/just e2e` after the
family gate is green. `JAIN_CI_EXTENDED=1` enables the deliberately expensive
core/math/Starforge algorithm tests that are skipped in the fast merge lane.
