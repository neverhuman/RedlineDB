# Constraint ownership

Every consumer namespace must retain its own foreign keys and check constraints.
The adapter performs deterministic `{ns}` expansion; it does not weaken or
silently synthesize constraints. Cross-namespace foreign keys are prohibited.
