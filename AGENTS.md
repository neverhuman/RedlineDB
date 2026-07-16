# RedlineDB Agent Router

## Workspace Boundary

- Work only in the user-named active repo/worktree.
- Never switch to sibling clones, archives, backups, resolved symlink targets, or duplicate roots.
- Never create repo copies or side folders outside the active repo; preserve work with git branches.
- Before edits, report `pwd`, `git rev-parse --show-toplevel`, and `git status --short --branch`.
- Use Jeryu APIs/CLI for local GitLab/MR work; no `glab`, credential scraping, or raw local GitLab API calls.

## Zero-worktree policy — absolute

- Do not create or move a Git worktree anywhere, including under `/tmp`. Never
  run `git worktree add` or `git worktree move`.
- Work only in the existing primary checkout. If it is dirty, busy, or held by
  another owner, stop and wait for a clean stopped-head handoff.
- Existing worktrees are cleanup inputs only. Never force-remove or prune one.
  Removal requires separate authority, proof that no regression or unmerged
  work would be lost, and a fresh recursive scan proving the path has no
  symlink.
- Exact-SHA CI may use only an automatically removed standalone clone/sandbox
  that is not registered with `git worktree`.

Mission: keep invariants local, edit the smallest lawful surface, and preserve raw evidence.

Access contract: local agent workspaces use `~/.jeryu/access.toml`, `jeryu access doctor`, and `jeryu access repair --repo . --yes`; do not install/use `glab`, scrape credential stores, or keep HTTP local GitLab origins.

Start here:
- `.jankurai/owner-map.json`
- `.jankurai/test-map.json`
- `.jankurai/proof-lanes.toml`
- `.jankurai/generated-zones.toml`
- `.jankurai/unsafe-ledger.toml`
- `docs/audit-rubric.md` · `docs/language-bad-behavior.md` · `docs/testing.md`
- `docs/architecture.md` · `docs/boundaries.md`

Rules:
- Prefer package-scoped edits over workspace-wide edits.
- Never hand-edit paths listed in `.jankurai/generated-zones.toml`.
- Keep active source files under 2,000 LOC; split or archive anything larger.
- Do not compress away exit codes, failing test names, panic text, spans, advisory IDs, seeds, raw-log paths, or raw-log hashes.
- Treat `just fast` as the default proof lane, then widen only when the edit crosses contract, security, or concurrency boundaries.


<!-- jankurai merge marker: review and merge canonical guidance for AGENTS.md -->
