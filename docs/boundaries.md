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

The control plane owns closed dependency-cache formats, validation, and
root-side provisioning. A product repo owns its dependency lock. Redline Web's
`apps/web/package-lock.json` is therefore an input to the
`jain.npm-cache/v1` authority, not control-plane product source. The immutable
root cache is never exposed writable; only its validated per-request copy
crosses into the network-isolated worker. Ambient home-directory caches and
network fallback remain outside the boundary.

Playwright browser binaries follow the same custody rule. The control plane
owns their closed authority, validation, immutable root cache, and read-only
request mounts. Redline Web owns the package lock that selects Playwright.
Only the small request-local Playwright registry is writable; browser payloads
never come from an ambient home directory and are never downloaded by host CI.

Generated member standards are changed through the Rust `splitctl` contract
commands first (`cargo run --locked -- refresh-ci-contract`), then regenerated
or manually ported with the same content. The old Python materializer is
deleted and must not be recreated.
