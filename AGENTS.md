# jain-split-ops — jain split family control plane

This repo is the single tracked source of the jain-split control plane: the
family manifest (`repos.manifest.toml`), the materializer + reconcile/rollout
tooling (`ops/split/*`), the host CI runner (`ops/ci/split-host-ci.sh`), forge
onboarding (`ops/onboard.sh`, `ops/lib.sh`, `ops/hooks/*`), and the planning /
evidence docs (`docs/`).

It is a SIBLING of the 19 family members under `/home/ubuntu/jain-split/`, not a
member of `repos.manifest.toml` (so `materialize.py --force` never touches it).
Each family member remains an independent git repo with its own CI and forge
remote; this repo only orchestrates them.

## Runtime contract (no symlinks, explicit roots)
- The family root (where the sibling repos + `target/bare-mirrors/` live) is the
  EXPLICIT `JAIN_SPLIT_ROOT` (default `/home/ubuntu/jain-split`), never derived
  from a script's location. `ops/ci/split-host-ci.sh` asserts it.
- This repo's own path is derived by the tooling (`materialize.py` `ROOT =
  parents[2]`; `rollout-pr-flow.sh`/`closeout-prs.sh`/`cutover.sh` compute
  `ops_root` from `$BASH_SOURCE/../..`) so it is relocatable.
- Bootstrap: clone this repo, then `python3 ops/split/materialize.py --manifest
  repos.manifest.toml` materializes the family into `JAIN_SPLIT_ROOT`.

## Local Jeryu Forge Workflow

The Jain workspace is `/home/ubuntu/jain-split`. Do not use `~/jeryu-split` as
an operational source for Jain work; it is a precedent/product checkout, not a
member of this family.

Use normal Git commands against the local loopback Jeryu remote. Git credentials
are already supplied by local Git/HTTP credential storage, so fetch/push should
be fast and should not require agent-visible token handling.

Canonical repo remote:
`http://127.0.0.1:8787/git/jeryu/<repo>.git`.

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
`/home/ubuntu/jain-split/jain-split-ops/ops/split/jeryu-local.py` or the
`just jeryu-*` recipes from the control-plane repo.

Use `just jeryu-doctor` for a read-only health check. Run or post split CI
through `/home/ubuntu/jain-split/jain-split-ops/ops/ci/split-host-ci.sh`, not
GitHub Actions, unless an explicit GitHub mirror workflow is requested.
