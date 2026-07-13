# Database

This directory is the policy boundary for durable database truth. Versioned
migrations live in `db/migrations/`; constraint declarations live in
`db/constraints/`. `agent/boundaries.toml` records those roots for automated
boundary and destructive-migration checks.

Every migration must preserve existing data, document lock and backfill
behavior, provide a tested rollback or superseding migration, and keep foreign
key and check-constraint invariants explicit. Engine-format changes additionally
exercise reopen, recovery, and rollback tests before release; opening a nonempty
database must never select a create or replacement path.
