# Control-Plane Durable State

`jain-split-ops` does not own a product database. Durable operational truth is
kept in local Jeryu and in versioned files such as `repos.manifest.toml`,
`VERSION`, `CHANGELOG.md`, and generated score artifacts.

If a future control-plane table is added, it must document foreign key
relationships, check constraint rules, row level security needs, rollback
steps, backfill procedure, and lock behavior before migrations land.

