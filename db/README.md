# Data boundary

`crates/db-shim` is the sole database adapter boundary. Application code must
use its typed operations and `{ns}` expansion rather than opening an additional
connection layer in this repository.

Consumer schemas own foreign keys, check constraints, and row-level policy.
Redline Central owns namespace isolation and transport selection. Rollback must
restore an immutable code tag and a separately verified data backup; it must
not rewrite an existing database in place.
