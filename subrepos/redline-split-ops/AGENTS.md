# redline-split-ops

This repository is the Redline nested-family control plane. It is independent
of every child repository and is not a Cargo workspace or a Redline product
member.

The default family root is resolved from this checkout's parent or from
`REDLINE_SPLIT_ROOT`; no command relies on `/home/ubuntu` or another fixed
checkout path. The canonical nested manifest is `repos.manifest.toml`, and the
family lock is `../redline.lock.toml`.

Use `./redlinectl doctor`, `./redlinectl validate`, and
`./redlinectl family-ci` as the control-plane proof lanes.

Operational boundaries are documented in `docs/architecture.md`; exact test,
security, receipt, and repair commands are in `docs/testing.md`; cutover and
rollback rules are in `docs/release.md`. Generated evidence ownership is in
`docs/generated-zones.md`, and audit acceptance is in
`docs/audit-rubric.md`. New control-plane implementation and tests are Rust.
Python belongs only in genuine cross-language parity harnesses.
