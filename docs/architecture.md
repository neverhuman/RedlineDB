# Nested family boundary

Jain owns the portal and its own family manifest. Redline owns a separate
four-repository accepted family. `redline-split-ops` is the only delegation boundary:
it resolves the child checkouts from its manifest, verifies the child commits
against `redline.lock.toml`, and runs each child’s own required check.

The physical family root (`jain-redline/`) is deliberately not a Git repository
and is not a Cargo workspace. `redline-split-ops` and every child are standalone
physical repositories beneath it. Manifest paths are relative to the control
plane; no child path is derived from a fixed home directory. The serialization
root is the held physical parent directory of the canonical control plane.
`REDLINE_SPLIT_ROOT` is accepted by `ci-required` only when it resolves to that
same root. Serialized operations flock and retain the root directory descriptor;
any `.redline-family.lock` entry is non-authoritative. `redline-central`
remains separately reviewed and absent from the active authority until its
protected commit and initial immutable tag exist. The closed-schema parser
reserves that exact row, and its presence makes real-service `family-release`
mandatory before the family receipt can pass.

A Core successor crosses that boundary in two protected states. Preparation
writes only the authoritative ineligible lock; reconciliation copies those
exact reviewed bytes to the compatibility mirror and records a checksummed
receipt. Neither state permits cutover. Family CI, the immutable tag, and two
independent consumer proofs remain separate downstream authorities.

Docker parity is a Testing-owned compatibility harness orchestrated by this
control plane; it is not a server deployment surface. In particular,
`redline-central/docker/` packages the separate 6033 Central service and is
outside the SQLite/PostgreSQL compatibility contract.
