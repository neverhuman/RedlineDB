# Local Jeryu Forge Agent Workflow

This workflow applies to Jain split repositories hosted on the local Jeryu
forge at `http://127.0.0.1:8787`.

## Ground Rules

- Local remotes use `http://127.0.0.1:8787/git/jeryu/<repo>.git`.
- GitHub.com mirrors are only for explicit mirror workflows.
- Do not run `gh auth login` for `127.0.0.1:8787`.
- Do not use generic GitHub connector or MCP tools for the local Jeryu host.
- Prefer `jeryu.*` MCP tools when they are exposed.
- If `jeryu.*` tools are not exposed, use authenticated local Jeryu REST.

## Health And Auth

Check that the local forge is up:

```bash
curl http://127.0.0.1:8787/health
```

Repair local Jeryu `gh` host auth with Jeryu's setup command:

```bash
jeryu gh-setup --host http://127.0.0.1:8787 --token-file ~/.jeryu/secrets/merge-token
```

Do not print the token. For direct REST calls, keep it in a local shell
variable:

```bash
base=http://127.0.0.1:8787
token="$(<~/.jeryu/secrets/merge-token)"
```

Inspect local forge capabilities:

```bash
curl -fsS \
  -H "Authorization: Bearer ${token}" \
  "${base}/.jeryu/capabilities"
```

The capabilities response should advertise the local auth policy with
`gh_auth_policy.run_instead` pointing at:

```text
jeryu gh-setup --host http://127.0.0.1:8787 --token-file ~/.jeryu/secrets/merge-token
```

## Pull Requests

Create a draft PR:

```bash
owner=jeryu
repo=jain-core
curl -fsS -X POST "${base}/repos/${owner}/${repo}/pulls" \
  -H "Authorization: Bearer ${token}" \
  -H "content-type: application/json" \
  --data '{
    "title": "feat-core: require Chimera Starforge study",
    "head": "codex/chimera-starforge-required",
    "base": "main",
    "draft": true,
    "actor": "codex"
  }'
```

List open PRs:

```bash
curl -fsS \
  -H "Authorization: Bearer ${token}" \
  "${base}/repos/${owner}/${repo}/pulls?state=open"
```

Check CI status for a commit:

```bash
sha=<commit-sha>
curl -fsS \
  -H "Authorization: Bearer ${token}" \
  "${base}/repos/${owner}/${repo}/commits/${sha}/check-runs"
```

Merge a PR after the required check is green:

```bash
pr=<number>
curl -fsS -X PUT "${base}/repos/${owner}/${repo}/pulls/${pr}/merge" \
  -H "Authorization: Bearer ${token}" \
  -H "content-type: application/json" \
  --data '{}'
```

## Required CI

Run and post the split host required check through the Jain control plane:

```bash
JAIN_SPLIT_ROOT=/home/ubuntu/jain-split \
  bash /home/ubuntu/jain-split/jain-split-ops/ops/ci/split-host-ci.sh \
  jeryu jain-core <commit-sha> /home/ubuntu/jain-split/jain-core jain-core/required
```

For the already pushed Chimera Starforge branch, the known commit is:

```text
3b34b3e2171838e0f91b36b29807d11a23b1c3a2
```
