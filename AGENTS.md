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
