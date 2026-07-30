# Native storage constraints

Checkpoint, archive, backup, and restore preserve:

- exact database and timeline identity;
- contiguous WAL address coverage;
- checkpoint, replication-slot, and archive retention horizons;
- closed single-link regular-file inventories;
- declared per-file and aggregate byte ceilings;
- exact SHA-256 digests before restore publication.

Constraint failure aborts before pruning or completion-marker publication.
