# Constraint ownership

Every consumer namespace must retain its own foreign keys and check constraints.
The adapter constructs table names only from validated identifiers; it does not rewrite SQL,
weaken constraints, or silently synthesize constraints. Cross-namespace foreign keys are prohibited.
