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

## Agent-readable map

- Architecture and the parent/root-broker/worker trust flow:
  [`docs/architecture.md`](docs/architecture.md).
- Owned and prohibited repository boundaries:
  [`docs/boundaries.md`](docs/boundaries.md).
- Exact local, adversarial host-CI, and sealed verification lanes:
  [`docs/testing.md`](docs/testing.md).
- Reviewed lifecycle, evidence, monitoring, and rollback:
  [`docs/release.md`](docs/release.md).
- Generated versus hand-authored zones:
  [`agent/generated-zones.toml`](agent/generated-zones.toml).
- Governed Jankurai tool, floor, ratchet, and cap policy:
  [`agent/audit-policy.toml`](agent/audit-policy.toml).

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

The Jain workspace is `/home/ubuntu/jain-split`. The independently released
Jeryu family is physically nested at `/home/ubuntu/jain-split/jeryu-split` and
is governed by `jeryu-release-ops/repos.manifest.toml`; preserve its `jeryu/*`
namespace and v5 lineage. `/home/ubuntu/jeryu-split`,
`/home/ubuntu/jain-split/jeryu`, copied source roots, symlink shims, and a
duplicate `jeryu-redline` container are forbidden. Jeryu consumes Redline
evidence from the canonical `/home/ubuntu/jain-split/jain-redline` family.

Use the typed `splitctl jeryu-local` transport against the local loopback Jeryu
forge. Pass credentials only by an explicit token-file path; never rely on an
ambient credential store for release lifecycle operations.

Before mutating this checkout, claim the branch and exact scope with one guarded
`splitctl coordination-ledger --root /home/ubuntu/jain-split --entry-file <path>
--apply` call. It holds and validates `RELEASE_V10.md`, `UPGRADE_CHAT.md`, and
`DATA_SHARD_CHAT.md` together; three independent append commands are forbidden.

Canonical family repo remote:
`http://127.0.0.1:8787/git/veox/<repo>.git`. Historical `jeryu/*` and
`jain-split/*` spellings remain valid only inside pinned Cargo dependency URLs.

For a quick check:

```bash
git remote -v
git ls-remote origin HEAD
```

If a repo remote is wrong or Git is slow/failing, inspect the Jain control-plane
authority without changing any checkout:

```bash
cd /home/ubuntu/jain-split/jain-split-ops
just jeryu-ready
```

That command is read-only: it checks the forge, remotes, family metadata, and
local-source policy. Repair only the specifically claimed checkout through the
reviewed `splitctl jeryu-local` lifecycle. Do not inspect `~/.jeryu`, run
`gh auth login`, or copy source from Jeryu internals.

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
