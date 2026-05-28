# RedlineDB Agent Router

## Workspace Boundary

- Work only in the user-named active repo/worktree.
- Never switch to sibling clones, archives, backups, resolved symlink targets, `/tmp` worktrees, or duplicate roots.
- Never create repo copies or side folders outside the active repo; preserve work with git branches.
- Before edits, report `pwd`, `git rev-parse --show-toplevel`, and `git status --short --branch`.
- Use Jeryu APIs/CLI for local GitLab/MR work; no `glab`, credential scraping, or raw local GitLab API calls.

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
