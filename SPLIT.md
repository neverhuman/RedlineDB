# Split Control-Plane Contract

`jain-split-ops` coordinates the Jain split family but is not itself a family
member. The member repos are siblings under `/home/ubuntu/jain-split` and are
listed in `repos.manifest.toml`.

## Source Policy

Local Jeryu is the operational source of truth for live split work:
`http://127.0.0.1:8787/git/veox/<repo>.git`.

Generated operational remotes, quickstarts, local patch examples, and agent
instructions must use that local Jeryu source. Historical internal Cargo Git
pins keep their existing owner spelling to avoid creating a second crate
identity. The only allowed `target/bare-mirrors` usage is CI-scoped dependency
caching through a temporary `GIT_CONFIG_GLOBAL`.

## Validation

`just required` runs shell syntax checks, Python compilation, materializer tests,
manifest parsing, and the local-Jeryu policy validator. `just score` runs the
Jankurai audit gate and fails on hard findings, score regressions, or new caps.

Coordination claims and handoffs are a three-ledger transaction. Use
`just coordination-status` before work and `just coordination-append <entry>
<receipt>` for an append; the command validates the frozen release-plan prefix,
holds all three locks in deterministic order, appends identical bytes, fsyncs,
and reads them back.

`just quality-status <token-file>` scans every manifest-managed checkout at its
exact local HEAD. It uses authenticated forge check history and the unique
latest `<repo>/required` plus `jankurai/proof` results; `.ci-status` files are
never current quality evidence.
