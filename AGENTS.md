# RedlineDB Agent Router

## Workspace Boundary

- Work only in the user-named active repo/worktree.
- Never switch to sibling clones, archives, backups, resolved symlink targets, or duplicate roots.
- Never create repo copies or side folders outside the active repo; preserve work with git branches.
- Before edits, report `pwd`, `git rev-parse --show-toplevel`, and `git status --short --branch`.
- Use Jeryu APIs/CLI for local GitLab/MR work; no `glab`, credential scraping, or raw local GitLab API calls.

## Git Worktree Policy — MANDATORY

**Agents MUST NOT create worktrees in `$HOME` or any persistent directory.**

Rules:
1. **Only create worktrees under `/tmp/`** — e.g. `git worktree add /tmp/rdb-fix-xyz`.
2. **Delete the worktree before the session ends** — `git worktree remove --force /tmp/rdb-fix-xyz`.
3. **Never leave a worktree at `/home/ubuntu/redlineDB-*` or any sibling of the main repo.**
   This causes directory sprawl that accumulates across agent sessions and is very hard to clean up.
4. If you find orphaned worktrees at `git worktree list`, **remove them immediately** with
   `git worktree remove --force <path>` before starting your own work.
5. Branches can and should outlive the worktree — create branches freely, but the working-directory
   checkout must live in `/tmp/` and be cleaned up when done.

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
