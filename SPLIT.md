# Split Control-Plane Contract

`jain-split-ops` coordinates the Jain split family but is not itself a family
member. The member repos are siblings under `/home/ubuntu/jain-split` and are
listed in `repos.manifest.toml`.

## Source Policy

Local Jeryu is the operational source of truth for live split work:
`http://127.0.0.1:8787/git/jeryu/<repo>.git`.

Generated remotes, internal Cargo Git dependencies, quickstarts, local patch
examples, and agent instructions must use that local Jeryu source. The only
allowed `target/bare-mirrors` usage is CI-scoped dependency caching through a
temporary `GIT_CONFIG_GLOBAL`.

## Validation

`just required` runs shell syntax checks, Python compilation, materializer tests,
manifest parsing, and the local-Jeryu policy validator. `just score` runs the
Jankurai audit gate and fails on hard findings, score regressions, or new caps.

