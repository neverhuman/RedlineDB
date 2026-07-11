# redline-split-ops

This repository is the Redline nested-family control plane. It is independent
of every child repository and is not a Cargo workspace or a Redline product
member.

The default family root is resolved from this checkout's parent or from
`REDLINE_SPLIT_ROOT`; no command relies on `/home/ubuntu` or another fixed
checkout path. The canonical nested manifest is `repos.manifest.toml`, and the
family lock is `../redline-split/redline.lock.toml`.

Use `./redlinectl doctor`, `./redlinectl validate`, and
`./redlinectl family-ci` as the control-plane proof lanes.
