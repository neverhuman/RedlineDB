# Control-Plane Durable State

`jain-split-ops` does not own a product database. Durable operational truth is
kept in local Jeryu and in versioned files such as `repos.manifest.toml`,
`VERSION`, `CHANGELOG.md`, and generated score artifacts.

The v4 host-CI boundary also writes derived exact-SHA reports and validated
receipts below the root-owned `proof_evidence_root` named by its installed
configuration. This store is authority evidence, not a product database or a
release source. Its permissions and content digests must survive retention or
rollback unchanged; missing, linked, or tampered evidence fails closed and is
never reconstructed by replaying a consumed request.

If a future control-plane table is added, it must document foreign key
relationships, check constraint rules, row level security needs, rollback
steps, backfill procedure, and lock behavior before migrations land.
