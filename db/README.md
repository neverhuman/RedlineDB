# Database boundary

Database state is not applicable to this shell-and-documentation family hub.
It has no runtime, service, database connection, migration authority, or
durable product truth. Review evidence under `target/` is rebuildable and
must never be treated as a database.

The empty `db/migrations/` and `db/constraints/` sentinels document that
boundary for repository audits. Product database ownership belongs to the
relevant Redline family component, not this hub.
