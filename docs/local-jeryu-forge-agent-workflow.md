# Local Jeryu Forge Agent Workflow

This workflow applies to Jain split repositories hosted on the local Jeryu
forge at `http://127.0.0.1:8787`.

## Ground Rules

- The Jain workspace is `/home/ubuntu/jain-split`.
- Local operational remotes use `http://127.0.0.1:8787/git/veox/<repo>.git`.
- Do not use `~/jeryu-split` as an operational source for Jain work.
- `~/.jeryu` is local credential/client state only, not a source checkout.
- Release lifecycle fetch/push operations use the typed `splitctl jeryu-local`
  transport with an explicit token-file path.
- GitHub.com mirrors are only for explicit mirror workflows.
- Do not run `gh auth login` for `127.0.0.1:8787`.
- Do not use generic GitHub connector or MCP tools for the local Jeryu host.
- Prefer `jeryu.*` MCP tools when they are exposed.
- If `jeryu.*` tools are not exposed, use `cargo run --locked -- jeryu-local` or the
  `just jeryu-*` recipes from this control-plane repo.

## Read-Only Git And Forge Inspection

Inspect the claimed Jain checkout without mutating refs:

```bash
git remote get-url origin
git status --porcelain=v1 --branch
git rev-parse HEAD HEAD^{tree}
```

Check the whole family and local forge without changing any checkout:

```bash
cd /home/ubuntu/jain-split/jain-split-ops
just jeryu-ready
```

`jeryu-ready` and `jeryu-doctor` are read-only aliases. If they report drift,
claim the affected checkout and use the exact-head lifecycle below; do not
rewrite every checkout in bulk.

List local forge repositories:

```bash
token_file=/home/ubuntu/.jeryu/secrets/veox-owner-token
just jeryu-repos "$token_file"
```

## Pull Requests

List open PRs:

```bash
repo=veox/jain-core
token_file=/home/ubuntu/.jeryu/secrets/veox-owner-token
just jeryu-prs "$repo" "$token_file"
```

Create a draft PR:

```bash
repo=veox/jain-core
head=codex/local-jeryu-policy
sha=<full-head-sha>
token_file=/home/ubuntu/.jeryu/secrets/veox-owner-token
just jeryu-pr-open-apply "$repo" "split-ops: canonicalize local Jeryu" \
  "$head" "$sha" "$token_file"
```

Check CI status for a commit:

```bash
sha=<commit-sha>
cargo run --locked -- jeryu-local checks --repo veox/jain-core --sha "$sha" \
  --token-file /home/ubuntu/.jeryu/secrets/veox-owner-token
```

Merge a PR after the required check is green:

```bash
pr=<number>
cargo run --locked -- jeryu-local pr-merge --repo veox/jain-core --number "$pr" \
  --expected-head <full-head-sha> --apply \
  --token-file /home/ubuntu/.jeryu/secrets/veox-owner-token
```

## Required CI

Run and post the split host required check through the Jain control plane:

```bash
JAIN_SPLIT_ROOT=/home/ubuntu/jain-split \
  bash /home/ubuntu/jain-split/jain-split-ops/ops/ci/split-host-ci.sh \
  veox jain-core <commit-sha> /home/ubuntu/jain-split/jain-core jain-core/required
```
