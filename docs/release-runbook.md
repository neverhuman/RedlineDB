# Jain 8.0.0 release-candidate runbook

This runbook produces the Jain `8.0.0` release candidate. It does not authorize production
promotion. The final status remains `candidate` with `formal_ga = false` until a separately
authorized production change is applied.

The sole Jain manifest authority is [`../repos.manifest.toml`](../repos.manifest.toml). The portal
and deploy copies are generated views and must carry the canonical manifest SHA-256. SmartCluster,
the four Redline repositories, and `redline-split-ops` are managed repositories inside
`/home/ubuntu/jain-split`; they are not external source drops. Redline's independent canonical
manifest remains `../redline-split-ops/repos.manifest.toml`.

All receipts belong under `docs/release-evidence/8.0.0/`. Product, package, CLI, deploy, and image
metadata use `8.0.0`; immutable repository tags retain the separate `*-v8.0.0-split.0` form.

## Non-negotiable safety rules

- Release reviewed `main` commits only. Never tag a review or feature branch.
- Keep exactly one managed remote named `origin`, pointing to the manifest-declared local Jeryu
  repository. Do not add GitHub remotes to release checkouts.
- Never force-push, reset away user work, rewrite or delete history, move an existing tag, bypass a
  required check, relax protection, or overwrite an existing `main`.
- Empty-main bootstrap is allowed only with the reviewed onboarding commit and compare-and-swap
  against an absent ref. Preserve its receipt.
- Preserve deferred CatBoost/Web/tooling state on remote branches or a verifiable bundle before
  moving its checkout outside the release workspace.
- Never edit Redline locks by hand. `proof-refresh` is their only accepted writer.
- Never use Redline's GitHub-oriented `redlinectl clone` as a release refresh path. Release heads
  come from reviewed local-Jeryu `main`.
- Set `ATOMICSOUL_PUSH=0` for every rollout command. Do not push images, alter Caddy, change public
  routes or aliases, mint live canary credentials, or mutate production infrastructure.
- AWS SageMaker is out of scope and must be recorded as `N/A`, never `passed`.

## 1. Canonical preflight and forge readback

From `/home/ubuntu/jain-split/jain-split-ops`:

```bash
just managed-repos
cargo run --locked --quiet -- validate-manifest \
  --manifest repos.manifest.toml --check-paths --check-derived
cargo run --locked --quiet -- source-coverage --manifest repos.manifest.toml --json
cargo run --locked --quiet -- validate-local-jeryu --manifest repos.manifest.toml
just verify-worktrees
```

The managed inventory must contain all Jain repositories, SmartCluster, `jain-split-ops`, Redline,
Redline core/testing/web, and `redline-split-ops`. Generated views are changed only with
`just sync-derived-apply`; validate them again immediately afterward.

For a repository whose forge has no `main`, first run the dry plan, inspect it, and then apply the
same reviewed SHA:

```bash
just bootstrap-main /absolute/checkout http://127.0.0.1:8787/git/OWNER/REPO.git REVIEWED_SHA
just bootstrap-main-apply /absolute/checkout http://127.0.0.1:8787/git/OWNER/REPO.git REVIEWED_SHA
```

Apply and read back immutable-main protection before accepting the repository into the graph:

```bash
just jeryu-protection OWNER/REPO REPO/required
just jeryu-protection-apply OWNER/REPO REPO/required
just jeryu-protection-readback OWNER/REPO REPO/required
```

Protection requires the declared status check, one independent approval, linear history, admin
enforcement, and disabled force-push/deletion.

## 2. Preservation and reviewed PR lifecycle

Classify changes as release/control-plane source, generated evidence, or deferred feature work.
Create and push a review branch without including `target/`, `.stage/`, `.ci-status/`, browser
reports, tokens, or other runtime output. Run the repository's required, security, score,
contract, artifact, and repository-specific lanes on the exact pushed SHA.

Post the declared required check from a detached governed snapshot:

```bash
JAIN_SPLIT_ROOT=/home/ubuntu/jain-split JAIN_RELEASE_CI=1 \
  ./ops/ci/split-host-ci.sh OWNER REPO SHA /absolute/checkout REPO/required
```

Open a draft PR, then use the controlled lifecycle. Every mutating command has a dry-run recipe
and a separate `*-apply` recipe:

```bash
just jeryu-pr-ready OWNER/REPO NUMBER
just jeryu-pr-ready-apply OWNER/REPO NUMBER
just jeryu-pr-approve OWNER/REPO NUMBER FULL_HEAD_SHA "Reviewed exact release SHA and evidence"
just jeryu-pr-approve-apply OWNER/REPO NUMBER FULL_HEAD_SHA "Reviewed exact release SHA and evidence"
just jeryu-pr-merge OWNER/REPO NUMBER
just jeryu-pr-merge-apply OWNER/REPO NUMBER
```

Approval is bound to the full current head SHA and must read back as an independent review. If the
head changes, rerun all affected gates and approve the new SHA. Do not merge deferred CatBoost/Web
feature PRs. After merge, fetch and refresh a clean `main` without destructive reset; local HEAD,
`origin/main`, and live forge `main` must be identical.

## 3. Redline cutover gate

Merge the reviewed Redline database, core, testing, web, and control-plane PRs. Testing has a
diverged history and must be deliberately reconciled through review, never overwritten. Refresh
all five checkouts to clean forge-equal `main`, then run from `redline-split-ops`:

```bash
just test
./redlinectl family-ci --receipt ../jain-split-ops/docs/release-evidence/8.0.0/redline-family-ci.json
./redlinectl proof-refresh \
  --family-ci ../jain-split-ops/docs/release-evidence/8.0.0/redline-family-ci.json \
  --jain-evidence JAIN_CONSUMER.json \
  --jeryu-evidence JERYU_CONSUMER.json
./redlinectl cutover-verify
```

`proof-refresh` accepts only fresh, checksummed successful family CI plus accepted consumer
evidence and must leave both lock mirrors byte-identical. Create
`redline-core-v4.1.0-jain.1` only when the local and remote tag are absent and the reviewed core
`main` is the exact eligible commit. If an existing local tag points elsewhere, stop for reviewed
resolution; never delete or move it manually.

## 4. Jain dependency waves

Complete each repository's reviewed PR, clean-main verification, required/security/score/contract/
artifact gates, immutable tag, mirror refresh, and downstream lock update before the next wave:

1. Redline family and Redline control plane.
2. Domain, math, contracts, CatBoost mainline, XGBoost, and LightGBM.
3. Jable, Battle GPU, and Starforge.
4. Core.
5. LLM, agent, jnoccio, zyal, jailgun, and research.
6. SmartCluster.
7. Report, TUI, CLI, Web mainline, and Python.
8. Model-zoo and operations.
9. Deploy.
10. Portal.

SmartCluster precedes CLI/Web because their immutable dependency graph includes it. CatBoost and
Web feature expansion remain deferred, but their clean mainline repositories still receive the
v8 metadata/tag through separate release PRs.

Create a repository tag with an inspectable dry run followed by the explicit apply operation:

```bash
just immutable-tag /absolute/checkout REMOTE_URL REPO-v8.0.0-split.0 REVIEWED_MAIN_SHA
just immutable-tag-apply /absolute/checkout REMOTE_URL REPO-v8.0.0-split.0 REVIEWED_MAIN_SHA
```

The operation refuses a local or remote tag that resolves to another commit. Refresh local bare
mirrors and verify their refs after each wave. Existing v7 tags are audit inputs only and remain
untouched, including any pre-existing local/forge mismatch.

## 5. Locks and clean-cache verification

Regenerate locks only after all referenced upstream tags exist on local Jeryu. Verify every lock
URL, tag, commit, package version, and checksum against the canonical manifest. Run from a fresh
`CARGO_HOME` and target directory:

```bash
cargo metadata --locked --all-features
cargo build --locked --all-features --release
cargo test --locked --all-features
```

Family CI uses detached locked snapshots without sibling source checkouts. `jain-deploy` is the
single explicit integration exception because its reviewed workspace patches are part of the
tested deployment graph. A tag that advertises package version `7.x` while a v8 lock requires
`8.0.0` is a hard dependency-wave blocker, not a reason to edit the lock.

Required specialized evidence includes real native training paths, Jable CUDA when the governed
GPU is available, Starforge LFS object/checksum and golden parity, Web Playwright flows,
SmartCluster restart/recovery/performance/soak and delegated-cgroup Linux proof, and the complete
Redline family. Capability or host denial must be labeled `blocked`, never silently converted to
`passed`.

## 6. Artifact and runtime evidence

Build from the staged release snapshot, not an arbitrary checkout. Retain artifact inventory and
SHA-256 receipts, SPDX SBOM, zero-high vulnerability scan, provenance, cosign verification, image
inspection, CI receipt, release inventory, and rollback receipt. Verify CLI version behavior, Web
flows, SmartCluster daemon/client/worker health, Redline persistence and crash recovery,
interrupted-session reconciliation, upload/storage limits, and bounded cleanup.

All product metadata must agree on `8.0.0`: Cargo/package manifests, CLI output, deploy stage
manifests, OCI labels, image tags, receipts, and release inventory. Split repository tag names are
not product version strings.

## 7. AtomicSoul dry run only

Run the audited wrapper from `jain-deploy` after the dependency graph resolves:

```bash
cd /home/ubuntu/jain-split/jain-deploy
JAIN_RELEASE_VERSION=8.0.0 ATOMICSOUL_PUSH=0 ./scripts/atomicsoul-v8-dry-run.sh
```

It performs the local `BuildLocal` gate and plans image publish, canary, public-route, promotion,
rollback to known target `7.0.6`, retention, and registry cleanup. Dry-run operations are recorded
as `planned`, never `passed`. Verify the wrapper contract separately:

```bash
bash scripts/test-atomicsoul-v8-dry-run.sh
```

There is no live/apply mode in this workflow. A command attempting to set `ATOMICSOUL_PUSH=1`,
remove `--dry-run`, access canary secrets, call Docker/SSH during a dry run, or change Caddy is a
hard failure.

## 8. Final acceptance

Regenerate `release-worktree-verification.json` and the release status only after all waves:

```bash
just verify-worktrees
just release-snapshot
just release-status
```

Accept the candidate only when every active managed checkout is clean `main`, tracks forge main,
has the one declared origin, required checks and independent reviews are green, immutable tags and
locks match, Redline reports `cutover_eligible = true`, SmartCluster has no required proof failure,
artifact/security gates have no hard findings, and the dry-run rollout receipts prove no external
state change. Because production promotion is not authorized, the expected final state is still
`candidate`, `formal_ga = false`, SageMaker `N/A`.

## Failure signatures, ownership, and rollback

- `candidate versions ... 7.x`: upstream v8 tag/package metadata mismatch; upstream owner fixes and
  retags only if the tag was never published. Never move a published tag.
- `couldn't find remote ref refs/tags/...`: dependency wave has not produced its immutable tag.
- `worktree is dirty`, wrong branch, or forge-head mismatch: repository owner preserves the state
  and refreshes through reviewed Git operations.
- failed/expired Redline receipt: Redline owner reruns family CI; manual lock edits are prohibited.
- cgroup/PSI permission denied: infrastructure owner reruns SmartCluster Linux proof on the
  delegated target host; record `blocked` until then.
- missing scanner, signing tool, or provenance artifact: release-ops owner provisions the pinned
  tool and reruns; do not waive the lane.
- AtomicSoul dry-run invokes Docker, SSH, Caddy, a registry write, or a secret path: deploy owner
  stops immediately and fixes the dry-run boundary before rerunning.

Rollback for this candidate is evidentiary only: validate the command sequence targeting `7.0.6`
and preserve its receipt. Do not alter production aliases or routes. If any release invariant
fails, close or leave the affected PR draft, preserve its branch, mark the candidate blocked, and
resume from the last verified dependency wave.
