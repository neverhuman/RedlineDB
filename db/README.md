# Database

This public hub owns no database schema, migrations, constraints, or durable
runtime truth. Those belong to `redline-core`; adding them here violates the
thin-hub boundary and is rejected by `scripts/guard-no-duplicate-engine.sh`.

Consumer rollback restores the last proof-refreshed Redline lock through
`redline-split-ops`; it never applies a migration or backfill from this hub.
