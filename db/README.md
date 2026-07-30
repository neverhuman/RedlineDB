# Native data authority

RedlineDB owns an embedded native store; it does not apply application SQL
migrations. Rust engine code under `crates/kernel` owns physical format and
recovery changes. `db/migrations/` records the fail-closed format-transition
policy, while `db/constraints/` records storage constraints that every backup,
restore, and checkpoint must preserve.

The rollback unit is an exact verified physical backup plus its closed manifest.
Restore never overwrites existing custody, follows links, or accepts an
undeclared file. A failed restore leaves no completion marker.
