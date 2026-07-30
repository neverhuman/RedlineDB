# Database boundary

Read [`../AGENTS.md`](../AGENTS.md) first.

This family hub owns no database or durable product truth. The `db/` tree is
an explicit not-applicable sentinel for repository audits.

- Owns: documentation of the no-database boundary.
- Forbidden: migrations, schema objects, connection configuration, runtime
  persistence, product data, or copied database authority from another
  Redline component.
- Proof lane: `bash scripts/ci-local.sh score` from the repository root.
