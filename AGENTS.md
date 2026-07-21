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
- The ship contract is versioned by `contracts/compatibility-v1.toml`. A new
  case first proves `sqlite3 ↔ sqlite3` or `psql ↔ psql` self-comparison.
  Once required by a published contract it may not be cut, skipped, or
  weakened without a compatibility-contract major change. Release mode fails
  closed on unavailable oracles, skips, count drift, missing/duplicate cases,
  malformed output, or any non-pass result.
- Release builds and evidence use only the local Jeryu forge and physical,
  recursively symlink-free custody under `/home/ubuntu/jain-split`; never add
  a GitHub release, download, external checkout, or symlink dependency.
