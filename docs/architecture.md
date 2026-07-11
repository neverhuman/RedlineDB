# Nested family boundary

Jain owns the portal and its own family manifest. Redline owns a separate
four-repository family. `redline-split-ops` is the only delegation boundary:
it resolves the child checkouts from its manifest, verifies the child commits
against `redline.lock.toml`, and runs each child’s own required check.

The container is deliberately not a Git repository and is not a Cargo
workspace. A copied Jain checkout can relocate the entire workspace by setting
`REDLINE_SPLIT_ROOT`; no child path is derived from a fixed home directory.
