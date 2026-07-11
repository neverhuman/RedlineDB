@/home/ubuntu/.codex/RTK.md

# redline-testing Agent Router

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
