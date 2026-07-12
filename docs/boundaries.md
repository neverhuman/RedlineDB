# Boundaries

This repo may edit:

- `repos.manifest.toml`
- `ops/split/*`
- `ops/ci/*`
- `ops/hooks/*`
- `ops/onboard.sh`, `ops/lib.sh`, and local Jeryu orchestration scripts
- control-plane docs and agent policy files

This repo should not own product source. Product changes belong in the matching
split member repository, then flow through local Jeryu PRs and immutable tags.

Generated member standards are changed through the Rust `splitctl` contract
commands first (`cargo run --locked -- refresh-ci-contract`), then regenerated
or manually ported with the same content. The old Python materializer is
deleted and must not be recreated.
