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

Local Jain/Jeryu remotes use `http://127.0.0.1:8787/git/jeryu/<repo>.git`.
For that host, do not run `gh auth login` and do not use generic GitHub.com
connector tools. If `gh` host auth is stale, repair it with:

```bash
jeryu gh-setup --host http://127.0.0.1:8787 --token-file ~/.jeryu/secrets/merge-token
```

For PRs, checks, and merges, use `jeryu.*` MCP tools when they are exposed. If
they are not exposed, use the local Jeryu REST API with `Authorization: Bearer
<merge-token>` from `~/.jeryu/secrets/merge-token`.

Use `curl http://127.0.0.1:8787/health` for a health check and authenticated
`GET /.jeryu/capabilities` to inspect local forge policy. Run or post split CI
through `/home/ubuntu/jain-split/jain-split-ops/ops/ci/split-host-ci.sh`, not
GitHub Actions, unless an explicit GitHub mirror workflow is requested.

