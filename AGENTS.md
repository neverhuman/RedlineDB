@/home/ubuntu/.codex/RTK.md

# redline-testing Agent Router

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

Mission: keep the external RedlineDB conformance runner deterministic,
release-packaged, and compatible with RedlineDB's pinned artifact consumer.

Access contract: local agent workspaces use `~/.jeryu/access.toml`, `jeryu access doctor`, and `jeryu access repair --repo . --yes`; do not install/use `glab`, scrape credential stores, or keep HTTP local GitLab origins.

Start here:
- `docs/architecture.md` — layers, data-access boundary, output contract
- `docs/boundaries.md` — adapter seam, generated zones, ownership
- `docs/testing.md` — proof lanes, property tests, release readiness
- `docs/operations.md` — monitoring, backups, rollback, abuse controls
- `ops/AGENTS.md` — CI lane ownership (owns / forbidden / proof lane)
- `.jankurai/owner-map.json`, `.jankurai/test-map.json`,
  `.jankurai/proof-lanes.toml`, `.jankurai/generated-zones.toml`,
  `.jankurai/audit-policy.toml`, `.jankurai/unsafe-ledger.toml`

Setup + validate (one command each):
- `bash scripts/setup.sh` — install toolchain + build
- `bash ops/ci/pr-ci.sh` — the single validate command

Rules:
- Keep edits scoped to this repository.
- Never hand-edit paths listed in `.jankurai/generated-zones.toml`.
- Preserve JSONL field compatibility for RedlineDB report parsing.
- Treat `rtk cargo fmt --check`, `rtk cargo check --locked`,
  `rtk cargo test --locked`, and `rtk just release-local` as the local proof.
- The ship contract: a SQLite-parity case ships iff `sqlite3 ↔ sqlite3`
  self-compare passes (`cargo run -p xtask -- ship-gate`); a beyond-SQLite
  case ships iff `psql ↔ psql` self-compare passes. Failing cases are cut
  from the shard, not demoted. RedlineDB reacts on its own to the published
  corpus.
