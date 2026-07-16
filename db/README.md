# Data boundary

`crates/db-shim` is the sole database adapter boundary. Application code must
use its owned values, structural parameter statements, validated table identifiers,
and atomic transaction contract rather than opening an additional connection layer.

Consumer schemas own foreign keys, check constraints, and row-level policy.
Redline Central owns the governed operation corpus and compile-time adapter selection. Rollback must
restore an immutable code tag and a separately verified data backup; it must
not rewrite an existing database in place.
