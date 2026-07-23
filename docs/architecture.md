# Nested family boundary

Jain owns the portal and its own family manifest. Redline owns a separate
six-repository family: one control plane and five products. `redline-split-ops` is the only delegation boundary:
it resolves the child checkouts from its manifest, verifies the child commits
against `redline.lock.toml`, and runs each child’s own required check.

The physical family root (`jain-redline/`) is deliberately not a Git repository
and is not a Cargo workspace. `redline-split-ops` and every child are standalone
physical repositories beneath it. Manifest paths are relative to the control
plane; no child path is derived from a fixed home directory. The serialization
root is the held physical parent directory of the canonical control plane.
`REDLINE_SPLIT_ROOT` is accepted by `ci-required` only when it resolves to that
same root. Serialized operations flock and retain the root directory descriptor;
any `.redline-family.lock` entry is non-authoritative. `redline-central` is a
mandatory product row bound to its existing protected commit and initial
immutable tag, and real-service `family-release` is mandatory before the family
receipt can pass.

The tracked `.jain.3` lock predates Central, canonical `veox/*` ownership, and
the `.jain.5` recovery point. It remains historical and cannot validate the new
authority. Only fresh family CI, immutable tag readback, two independent
consumer proofs, and no-waiver proof refresh can create the next authoritative
lock pair.
