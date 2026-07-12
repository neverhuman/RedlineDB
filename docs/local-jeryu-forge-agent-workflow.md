# Local Jeryu Forge Agent Workflow

This workflow applies to Jain split repositories hosted on the local Jeryu
forge at `http://127.0.0.1:8787`.

## Ground Rules

- The Jain workspace is `/home/ubuntu/jain-split`.
- Local remotes use `http://127.0.0.1:8787/git/jeryu/<repo>.git`.
- Do not use `~/jeryu-split` as an operational source for Jain work.
- `~/.jeryu` is local credential/client state only, not a source checkout.
- Normal `git fetch`, `git pull`, and `git push` use local Git/HTTP
  credentials already configured for the loopback host.
- GitHub.com mirrors are only for explicit mirror workflows.
- Do not run `gh auth login` for `127.0.0.1:8787`.
- Do not use generic GitHub connector or MCP tools for the local Jeryu host.
- Prefer `jeryu.*` MCP tools when they are exposed.
- If `jeryu.*` tools are not exposed, use `cargo run --locked -- jeryu-local` or the
  `just jeryu-*` recipes from this control-plane repo.

## Git And Forge

Use Git directly from any Jain repo:

```bash
git remote -v
git ls-remote origin HEAD
git fetch origin
```

If a checkout's remote is wrong, or Git is slow/failing, repair the whole Jain
family with one control-plane command:

```bash
cd /home/ubuntu/jain-split/jain-split-ops
just jeryu-ready
```

That command checks the local forge, canonicalizes each repo `origin`, removes
extra remotes, registers Jain family metadata, and runs the local-source policy
validator. Use the read-only form when you only need a status check:

```bash
just jeryu-doctor
```

List local forge repositories:

```bash
just jeryu-repos
```

## Pull Requests

List open PRs:

```bash
repo=jain-core
just jeryu-prs "$repo"
```

Create a draft PR:

```bash
just jeryu-pr-open jain-core codex/local-jeryu-policy "split-ops: canonicalize local Jeryu"
```

Check CI status for a commit:

```bash
sha=<commit-sha>
cargo run --locked -- jeryu-local checks --repo jain-core --sha "$sha"
```

Merge a PR after the required check is green:

```bash
pr=<number>
cargo run --locked -- jeryu-local pr-merge --repo jain-core --number "$pr"
```

## Required CI

Run and post the split host required check through the Jain control plane:

```bash
JAIN_SPLIT_ROOT=/home/ubuntu/jain-split \
  bash /home/ubuntu/jain-split/jain-split-ops/ops/ci/split-host-ci.sh \
  jeryu jain-core <commit-sha> /home/ubuntu/jain-split/jain-core jain-core/required
```
