# jain-split-ops durable-state instructions

This control-plane repo has no product database. Durable state is local Jeryu
metadata plus versioned control-plane files such as `repos.manifest.toml`,
`VERSION`, `CHANGELOG.md`, `.jankurai/repo-score.json`, and release docs.

Do not add SQL migrations without documenting rollback, backfill, lock behavior,
foreign key rules, check constraints, and row level security expectations.
