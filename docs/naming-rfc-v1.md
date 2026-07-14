# Jain naming RFC v1

Status: proposed for the post-v8.0.0 naming wave

Owner: veox  
Product family: Jain  
Authority: `jain-split-ops/repos.manifest.toml`  
Scope: names, slugs, package prefixes, and deployment surfaces only

This RFC records the naming cleanup to review after the v8.0.0 candidate has
passed its release gates. It does not change v8.0.0 repository basenames, move
tags, or authorize a forge migration by itself. The v8 release keeps the
existing `jain-*` basenames and treats the local forge as the only release
remote.

## Goals and invariants

1. `veox` is the parent/company namespace and `jain` is the product family.
2. Public names must identify their product role without relying on opaque
   internal codenames.
3. A name change must preserve an explicit compatibility alias, a redirect or
   deprecation period, and an auditable mapping from old slug to new slug.
4. Release authority, dependency URLs, installer metadata, documentation, and
   generated locks must use local-forge identities. Historical release records
   may retain their original provenance, but active metadata must not point at
   an external mirror.
5. Existing immutable tags remain immutable. A rename creates a new repository
   identity or alias; it never retags an old commit.
6. The candidate remains fail-closed: `status = candidate`, `formal_ga = false`,
   and `sagemaker = N/A` until the owner explicitly changes release policy.

## Namespace and slug policy

### Parent and product

The canonical parent is `veox`; the canonical product is `jain`. The 27 Jain
repositories (the 25 product repositories plus SmartCluster and the Jain
control plane) should be dual-homed under `veox/<name>` after the v8 release,
with the existing local-forge aliases retained and frozen during the migration.
The aliases must have identical heads and tags at cutover. No alias is deleted
until every active dependency, lockfile, installer, and receipt has moved.

The portal is the exception to the simple repository-name mapping. The current
portal slug collides with the product/source umbrella name, so the proposed
canonical portal slug is `veox/jain-portal`. `veox/jain` remains reserved for
the product family entry point or a compatibility redirect, not a second
source repository.

The Redline family remains a separately governed family for this release. Its
post-release work should converge the product, database, engine, testing, web,
and control-plane spellings under one documented Redline namespace. That work
must continue to use Redline's own lock/proof workflow.

### External-mirror scrub

The authority manifest should have one active forge identity per repository;
legacy mirror fields and links should be removed or marked historical in a
reviewed control-plane PR. The same policy applies to derived manifests,
family locks, Cargo git URLs, installer defaults, workflow metadata, and
release documentation. A repository passes the naming gate only when a fresh
active-metadata scan finds no external-host dependency, mirror slug, or stale
image path. Historical 7.x receipts are retained as evidence and excluded from
the active scan by an explicit path rule.

In particular, `github_slug = "neverhuman/<repo>"` is a retired compatibility
field, not an active release identity. It must be removed from the authority
manifest and derived active manifests (or explicitly classified as historical
data outside the active scan) as part of the namespace migration.

## Repository basename proposals

The existing `jain-*` names remain valid for v8.0.0. The following are
post-release proposals; the old name is kept as a compatibility alias for at
least one release cycle.

| Current repository | Purpose | Proposed public name | Compatibility note |
| --- | --- | --- | --- |
| `jain-jable` | In-house pure-Rust tabular regressor | `jain-tabular` | Keep `jable` as a package/API alias during migration. |
| `jain-jnoccio` | LLM provider gateway and health-aware router | `jain-provider-router` | Preserve provider adapter identifiers in the wire contract. |
| `jain-zyal` | Runbook compiler and workflow supervisor | `jain-workflows` | Keep `.zyal` document format and add a documented format owner. |
| `jain-jailgun` | Browser-session registry and lifecycle | `jain-browser-sessions` | Preserve `~/.jailgun` as a state migration alias. |
| `jain-starforge` | Tabular classifier inference and model assets | `jain-classifier` | Keep model artifact and backend names stable. |
| `jain-battle-gpu` | GPU kernel factory with CPU fallback | `jain-kernels` | Keep the CPU fallback and feature names unchanged. |
| `jain-agent` | Governed Jekko runtime contracts | `jain-agent-runtime` | Resolve the Jain/Jekko twin explicitly before renaming. |
| `jain-llm` | Native LLM provider and token-routing interfaces | `jain-llm` | Name is already descriptive; only namespace cleanup is proposed. |
| `jain-model-zoo` | Frozen reference ports and parity fixtures | `jain-model-zoo` | Name is descriptive; retain it. |
| `jain-smartcluster` | Single-node workload fabric and scheduler | `jain-smartcluster` | Name is descriptive; move only its owner namespace. |

The remaining names are already role-oriented and should not be renamed solely
for symmetry: `jain`, `jain-docs`, `jain-domain`, `jain-math`,
`jain-contracts`, the native learner bindings, `jain-core`, `jain-research`,
`jain-report`, `jain-tui`, `jain-cli`, `jain-web`, `jain-ops`, `jain-deploy`,
and `jain-split-ops`.

## Package, crate, and binary policy

The family currently mixes `feat-*`, bare names, `jain-*`, and `jail-*`
packages, while binaries use several independent conventions. The proposed
policy is:

- Public packages use `jain-<role>`; a `-sys` suffix is reserved for a native
  FFI binding and its safe wrapper keeps the role name.
- Internal implementation packages use `jain-internal-<role>` and are never
  presented as separate products.
- Product binaries use `jain`, `jain-web`, `jain-worker`, or
  `jain-<role>`; protocol/database tools use their established protocol name
  only when that name is part of a public contract.
- `feat-*` is an implementation namespace, not a public product namespace.
  New packages should not add another `feat-*` crate; existing names migrate
  through Cargo package aliases and a deprecation release.
- `jail-*` is reserved for browser/session tooling. It must not be used by
  unrelated operations tools.
- `redlinedb-*` is reserved for Redline engine/database crates.

### Duplicate `feat-cli` ownership

`jain-cli` is the sole owner of the public `feat-cli` package and the `jain`
and `jain-entrypoint` binaries. `jain-tui` should own the dashboard package
only (`feat-tui` during compatibility, then `jain-tui`) and must not publish a
second `feat-cli` package or a competing `jain` binary. The duplicate is
removed after consumers have migrated to the canonical CLI package.

## Agent-stack twins

`jain-agent`, `jain-jnoccio`, `jain-zyal`, `jain-jailgun`, and `jain-llm` have
counterparts in the Jekko family. The release authority should keep the Jain
and Jekko families separate and make ownership explicit in each package
description, namespace, and dependency URL. Shared protocol crates should be
split into a neutral contract package rather than relying on twin repository
names. A future consolidation is an owner decision, not an automatic rename.

## Deployment and serving names

The image's canonical repository path is `veox/jain`; the registry host and
AtomicSoul transport remain infrastructure configuration. The image path must
be consistent across deploy-engine constants, Dockerfile labels, staged
context, installer defaults, canary metadata, dry-run fixtures, and release
receipts. The legacy `jain-sagemaker` path is a compatibility alias only while
download clients migrate.

`sagemaker` is currently a serving-contract and crate-name residue, not a
release claim: the authority remains `sagemaker = N/A`. Post-release options
are to rename the serving crates and Dockerfiles to `jain-serving` or
`jain-runtime`, retain `/ping` and `/invocations` compatibility, and update
the contract documentation in one reviewed migration. No AWS/SageMaker
metadata should be reintroduced merely to preserve an old filename.

The landing installer and canary endpoint should use product-neutral names
(`install-jain.sh`, `jain` image, and a Jain canary route) while preserving
versioned download URLs and signed-digest verification. Signing identity and
registry host are operational settings, not repository names.

## Redline follow-up

The Redline family currently spans `redline`, `redlineDB`, `RedlineDB`,
`redlinedb-*`, and `redline-split`. The proposed canonical product spelling is
**Redline**; repository names use lowercase kebab case (`redline`,
`redline-core`, `redline-testing`, `redline-web`, `redline-split-ops`), crate
names use `redlinedb-*`, and prose uses “Redline”. Consumer suffixes in tags
remain a release-policy concern until the independent family agrees on one
scheme. The unmanaged `redline-split/redline-central` checkout must be either
registered in the Redline control plane with a reviewed remote and proof path,
or removed from the release workspace; it must not silently become a managed
consumer.

## Migration sequence and acceptance tests

1. Owner approves this RFC and the exact old-to-new mapping.
2. Create aliases and dual-home repository heads/tags; compare all refs before
   switching any active dependency.
3. Land dependency URL, package, installer, image-label, and documentation
   updates through reviewed PRs with required CI.
4. Regenerate family locks and derived manifests from the authority; do not
   hand-edit generated identity fields.
5. Publish one compatibility release with both names, then remove aliases only
   after consumer telemetry and a second reviewed decision.

The naming migration is accepted only when:

- every active repo, package, image, installer, and lock has one canonical
  identity and a recorded old-name mapping;
- no active metadata points to an external mirror or the retired image path;
- all alias refs are parity-checked and immutable tags are unchanged;
- `jain-cli` is the sole owner of the public `jain` binary;
- Redline's lock/proof workflow accounts for every managed Redline component;
- the owner has approved the decision matrix below.

## Owner decision matrix

| Decision | Recommendation | Compatibility/rollback | Owner decision |
| --- | --- | --- | --- |
| Jain forge namespace | Dual-home under `veox/*`; freeze existing aliases | Keep aliases until all locks and consumers migrate | Pending owner confirmation after v8 GO |
| Portal slug | `veox/jain-portal` | Keep `veox/jain` as family entrypoint/redirect | Pending |
| Opaque codenames | Adopt role names proposed above selectively | One-release alias and state migration | Pending |
| Package prefixes | `jain-*` public, `jain-internal-*` private, `feat-*` compatibility only | Cargo aliases and deprecation release | Pending |
| `feat-cli` duplicate | Canonicalize on `jain-cli`; remove `jain-tui` duplicate | Keep a compatibility package during migration | Pending |
| Agent/Jekko twins | Keep families separate; share neutral contracts | Explicit namespace and ownership metadata | Pending |
| Image path | `veox/jain` | Signed compatibility alias for old path | Pending |
| Serving residue | Rename to `jain-serving`/`jain-runtime` post-release | Preserve serving endpoints and contract version | Pending |
| Redline spellings | Redline product, lowercase repository names, `redlinedb-*` crates | Independent Redline proof/lock migration | Pending Redline owner review |
| `redline-central` | Register through Redline control plane or remove | Never include unmanaged code silently | Pending |
| Root derived manifest | Generate from authority; do not treat as authority | Digest check in CI | Pending control-plane implementation |
