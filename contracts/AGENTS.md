# Release contract instructions

This directory contains externally consumable, fail-closed control-plane schemas. Keep every
schema versioned, bounded, and closed to unknown fields. A schema change must remain synchronized
with its Rust producer and validator in `tools/splitctl`, include a negative regression test, and
pass `just contract-drift`, `just required`, and `just score`.

Do not add runtime state, credentials, generated evidence, product endpoints, or GA/traffic
activation controls here. Release-candidate contracts must preserve `formal_ga=false` and must not
represent registry publication, installation, or production routing actions.
