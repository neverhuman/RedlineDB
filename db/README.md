# db/

The database boundary for redline-web. There are **no checked-in migrations** —
the server opens whatever SQLite database is supplied at runtime. All database
access is owned by the `Connector` adapter (`apps/api/src/connector/`); the demo
schema is seeded from `apps/api/src/connector/seed_demo.sql`. See
[`AGENTS.md`](AGENTS.md) and `agent/boundaries.toml`.

The supplied database remains the authority for foreign-key and check-constraint
policy; redline-web does not apply migrations or backfills. Operators must take a
database backup before an external schema change, bound write-lock duration, and
use that backup as the rollback target if the external change fails.
