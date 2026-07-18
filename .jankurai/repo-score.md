# jankurai Repo Score

- Standard: `jankurai`
- Auditor: `1.6.11`
- Schema: `jain-split-ops-v8.0.1-split.0`
- Paper edition: `2026.05-ed8`
- Target stack ID: `rust-ts-vite-react-postgres-bounded-python`
- Target stack: `Rust core + TypeScript/React/Vite + PostgreSQL + generated contracts + exception-only Python AI/data service`
- Repo: `.`
- Run ID: `1784289223`
- Started at: `1784289223`
- Elapsed: `3209` ms
- Scope: `full`
- Raw score: `92`
- Final score: `92`
- Decision: `advisory`
- Minimum score: `85`
- Caps applied: `none`

## Hard Rule Caps

| Rule | Max Score | Applied |
| --- | ---: | --- |
| `no-root-agent-instructions` | 75 | no |
| `no-one-command-setup-or-validation` | 70 | no |
| `no-deterministic-fast-lane` | 65 | no |
| `no-security-lane-on-high-risk-repo` | 60 | no |
| `generated-contracts-or-public-api-drift-untested` | 80 | no |
| `python-direct-product-truth-or-db-ownership` | 72 | no |
| `no-secret-or-dependency-scanning-in-ci` | 78 | no |
| `no-jankurai-audit-lane-in-ci` | 82 | no |
| `jankurai-required-tool-ci-evidence-gap` | 88 | no |
| `non-optimal-product-language-found` | 74 | no |
| `too-much-python-in-product-surface` | 72 | no |
| `boundary-reclassification-evidence-gap` | 72 | no |
| `vibe-placeholders-in-product-code` | 68 | no |
| `fallback-soup-in-product-code` | 70 | no |
| `future-hostile-dead-language-in-product-code` | 64 | no |
| `severe-duplication-in-product-code` | 70 | no |
| `generated-zone-mutation-risk` | 76 | no |
| `direct-db-access-from-wrong-layer` | 66 | no |
| `missing-web-e2e-lane` | 82 | no |
| `missing-rendered-ux-qa-lane` | 84 | no |
| `prompt-injection-risk` | 78 | no |
| `overbroad-agent-agency` | 65 | no |
| `secret-like-content-detected` | 60 | no |
| `false-green-test-risk` | 76 | no |
| `destructive-migration-risk` | 70 | no |
| `authz-or-data-isolation-gap` | 78 | no |
| `input-boundary-gap` | 78 | no |
| `agent-tool-supply-chain-gap` | 78 | no |
| `release-readiness-gap` | 80 | no |
| `missing-rust-property-or-integration-tests` | 82 | no |
| `no-agent-friendly-exception-pattern` | 76 | no |
| `missing-agent-readable-docs` | 80 | no |
| `streaming-runtime-drift` | 78 | no |
| `rust-bad-behavior` | 72 | no |
| `sql-bad-behavior` | 72 | no |
| `typescript-bad-behavior` | 72 | no |
| `docker-bad-behavior` | 72 | no |
| `python-bad-behavior` | 72 | no |
| `ci-bad-behavior` | 70 | no |
| `git-bad-behavior` | 70 | no |
| `gittools-bad-behavior` | 70 | no |
| `release-bad-behavior` | 70 | no |
| `web-security-bad-behavior` | 68 | no |
| `repo-rot-bad-behavior` | 88 | no |
| `comment-hygiene-dangerous-residue` | 72 | no |
| `ci-local-parity` | 70 | no |

## Copy-Code Redundancy

- Status: `review` hard=`0` warning=`12` files=`5`
- Policy: min-lines=`10` min-tokens=`100` max-findings=`50` include-tests=`false` strict=`false`
- Duplicate volume: lines=`14` tokens=`42` bytes=`454`

- Notes:
  - hard classes are limited to exact active-source file matches and substantial exact same-name units
  - warning classes include same-body different-name units and token/block duplication
  - tests, fixtures, stories, config, Docker, and migrations are omitted unless --include-tests is set

| Kind | Severity | Language | Lines | Tokens | Instances | Reason |
| --- | --- | --- | ---: | ---: | --- | --- |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 2 | `tools/splitctl/src/jeryu_client.rs:123-124, tools/splitctl/src/jeryu_client.rs:169-170, tools/splitctl/src/jeryu_client.rs:179-180, tools/splitctl/src/jeryu_client.rs:189-190, tools/splitctl/src/jeryu_client.rs:199-200, tools/splitctl/src/jeryu_client.rs:233-234, tools/splitctl/src/jeryu_client.rs:244-245, tools/splitctl/src/jeryu_client.rs:258-259, tools/splitctl/src/jeryu_client.rs:267-268, tools/splitctl/src/jeryu_client.rs:277-278` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 4 | `tools/splitctl/src/jeryu_client.rs:1386-1387, tools/splitctl/src/jeryu_client.rs:1407-1408, tools/splitctl/src/jeryu_client.rs:1421-1422, tools/splitctl/src/jeryu_client.rs:1431-1432, tools/splitctl/src/jeryu_client.rs:1447-1448, tools/splitctl/src/jeryu_client.rs:1486-1487, tools/splitctl/src/jeryu_client.rs:1500-1501` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 3 | 7 | `tools/splitctl/src/main.rs:2639-2642, tools/splitctl/src/main.rs:2655-2658` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 1 | `tools/splitctl/src/jeryu_client.rs:505-506, tools/splitctl/src/jeryu_client.rs:546-547, tools/splitctl/src/jeryu_client.rs:553-554, tools/splitctl/src/jeryu_client.rs:764-765` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 8 | `tools/splitctl/src/jeryu_client.rs:1521-1522, tools/splitctl/src/jeryu_client.rs:1608-1609` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 5 | `tools/splitctl/src/main.rs:2357-2358, tools/splitctl/src/main.rs:2365-2366` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 3 | `tools/splitctl/src/jeryu_client.rs:1652-1653, tools/splitctl/src/jeryu_client.rs:1707-1708` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 3 | `tools/splitctl/src/jeryu_client.rs:1802-1803, tools/splitctl/src/jeryu_client.rs:1817-1818` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 3 | `tools/splitctl/src/jeryu_client.rs:573-574, tools/splitctl/src/jeryu_client.rs:603-604` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 3 | `tools/splitctl/src/jeryu_client.rs:561-562, tools/splitctl/src/jeryu_client.rs:683-684` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 2 | `tools/splitctl/src/jeryu_client.rs:672-673, tools/splitctl/src/main.rs:1677-1678` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 1 | `ops/ci/fake-git-http.rs:231-232, ops/ci/fake-git-http.rs:239-240` | `same body appears under different names across files` |

## Dimensions

| Dimension | Weight | Score | Weighted | Evidence |
| --- | ---: | ---: | ---: | --- |
| Ownership and navigation surface | 13 | 100 | 13.00 | root `AGENTS.md` present; owner map present |
| Contract and boundary integrity | 13 | 75 | 9.75 | generated contract artifacts found; boundary manifest present |
| Proof lanes and test routing | 12 | 96 | 11.52 | one-command setup/validation lane found; deterministic fast lane found |
| Security and supply-chain posture | 12 | 86 | 10.32 | lockfile present; secret or dependency scan tooling found |
| Code shape and semantic surface | 12 | 90 | 10.80 | no authored adopter product code files in scope |
| Data truth and workflow safety | 8 | 100 | 8.00 | database surface present; structured db boundary manifest present |
| Observability and repair evidence | 8 | 100 | 8.00 | ops/observability directory present; repair receipts or raw artifact language found |
| Context economy and agent instructions | 7 | 93 | 6.51 | root `AGENTS.md` present; root `AGENTS.md` stays short |
| Jankurai tool adoption and CI replacement | 7 | 100 | 7.00 | control-plane files present; applicable=17 |
| Python containment and polyglot hygiene | 4 | 100 | 4.00 | no Python files in scope |
| Build speed signals | 4 | 70 | 2.80 | build acceleration markers found; targeted test/build commands found |

## Reference Profile Structure

- Applicable cells: `2` canonical=`2` noncanonical=`0` guidance missing=`0`

| Cell | Status | Canonical | Detected | Aliases | Guidance | Owner | Proof lane | Agent fix |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `web` | `not_applicable` | `apps/web/` | `-` | `frontend/, ui/, packages/web/, packages/ui/` | `not_required` | `apps/web` | `rendered UX / Playwright` | `no action` |
| `api` | `not_applicable` | `apps/api/` | `-` | `api/, server/, backend/` | `not_required` | `apps/api` | `edge handler / contract tests` | `no action` |
| `domain` | `not_applicable` | `crates/domain/` | `-` | `domain/, core/` | `not_required` | `crates/domain` | `unit / property tests` | `no action` |
| `application` | `not_applicable` | `crates/application/` | `-` | `application/, usecases/, use-cases/` | `not_required` | `crates/application` | `use-case / authz tests` | `no action` |
| `adapters` | `not_applicable` | `crates/adapters/` | `-` | `adapters/, infra/, integrations/` | `not_required` | `crates/adapters` | `adapter integration tests` | `no action` |
| `workers` | `not_applicable` | `crates/workers/` | `-` | `workers/, jobs/, scheduler/, queue/` | `not_required` | `crates/workers` | `workflow / replay tests` | `no action` |
| `contracts` | `not_applicable` | `contracts/` | `-` | `openapi/, protobuf/, json-schema/, generated/` | `not_required` | `contracts` | `generation / drift checks` | `no action` |
| `db` | `canonical` | `db/` | `db` | `migrations/, constraints/, sql/` | `present` | `db` | `migration / constraint tests` | `keep `db/AGENTS.md` aligned with owns / forbidden / proof lane guidance` |
| `python-ai` | `not_applicable` | `python/ai-service/` | `-` | `python/, ai-service/, evals/, embeddings/, model/` | `not_required` | `python/ai-service` | `eval / contract tests` | `no action` |
| `ops` | `canonical` | `ops/` | `.github, .github/workflows, ops` | `.github/, .github/workflows/, ci/, release/, observability/, security/` | `present` | `ops` | `security lane / workflow lint` | `keep `ops/AGENTS.md` aligned with owns / forbidden / proof lane guidance` |

## Rendered UX QA

- Web surface: `false`
- Layered UX lane: `true`
- Missing: `none`

## Tool Adoption

- Control plane present: `true`
- Applicable tools: `17`
- Configured: `17`
- CI evidence: `17`
- Artifact verified: `17`
- Replaced count: `17`
- Missing CI evidence: `none`

| Tool | Category | Mode | Status | Replaced | Artifacts |
| --- | --- | --- | --- | --- | --- |
| `audit-ci` | `audit` | `auto` | `artifact_verified` | `manual repo scoring, ad hoc score gates` | `.jankurai/repo-score.json, .jankurai/repo-score.md` |
| `proof-routing` | `proof` | `auto` | `artifact_verified` | `ad hoc proof lane selection, manual proof receipts` | `.jankurai/repo-score.json, .jankurai/repo-score.md, target/jankurai/repair-queue.jsonl` |
| `proofbind` | `proof` | `auto` | `artifact_verified` | `manual changed-surface routing, ad hoc proof obligation lists` | `target/jankurai/proofbind/surface-witness.json, target/jankurai/proofbind/obligations.json` |
| `proofmark-rust` | `proof` | `advisory` | `artifact_verified` | `line-only coverage review, manual in-diff mutation review` | `target/jankurai/proofmark/proofmark-receipt.json, target/jankurai/proofmark/proof-receipt.json` |
| `copy-code` | `audit` | `advisory` | `artifact_verified` | `ad hoc copy-code review, manual duplication triage` | `target/jankurai/copy-code.json, target/jankurai/copy-code.md` |
| `security` | `security` | `auto` | `artifact_verified` | `gitleaks, dependency review, SBOM/provenance` | `target/jankurai/security/evidence.json` |
| `ci-bad-behavior` | `security` | `auto` | `artifact_verified` | `mutable workflow refs, secret echo/debug workflow checks, non-blocking security scans` | `target/jankurai/language-bad-behavior.log` |
| `git-bad-behavior` | `audit` | `auto` | `artifact_verified` | `destructive git automation, force-push release scripts, hidden stash-based state` | `target/jankurai/language-bad-behavior.log` |
| `release-bad-behavior` | `release` | `auto` | `artifact_verified` | `manual release checklist, ad hoc tag and artifact review, manual provenance review` | `target/jankurai/language-bad-behavior.log` |
| `ux-qa` | `ux` | `auto` | `not_applicable` | `playwright, axe-core, visual baselines` | `target/jankurai/ux-qa.json` |
| `db-migration-analyze` | `db` | `auto` | `not_applicable` | `manual migration review` | `target/jankurai/migration-report.json` |
| `contract-drift` | `contract` | `auto` | `artifact_verified` | `handwritten contract drift checks, openapi diff` | `.jankurai/repo-score.json, .jankurai/repo-score.md` |
| `rust-witness` | `rust` | `advisory` | `artifact_verified` | `manual witness graphing` | `target/jankurai/rust/witness-graph.json` |
| `vibe-coverage` | `audit` | `auto` | `not_applicable` | `manual vibe-coding coverage spreadsheet` | `target/jankurai/vibe-coverage.json, target/jankurai/vibe-coverage.md` |
| `coverage-evidence` | `proof` | `auto` | `artifact_verified` | `manual coverage report review, ad hoc mutation survivor review` | `target/jankurai/coverage/coverage-audit.json, target/jankurai/coverage/coverage-audit.md` |
| `authz-matrix` | `security` | `advisory` | `artifact_verified` | `manual authz matrix review` | `.jankurai/repo-score.json, .jankurai/repo-score.md` |
| `input-boundary` | `security` | `advisory` | `artifact_verified` | `manual unsafe sink review` | `.jankurai/repo-score.json, .jankurai/repo-score.md` |
| `agent-tool-supply` | `security` | `auto` | `artifact_verified` | `manual MCP/tool trust review` | `.jankurai/repo-score.json, .jankurai/repo-score.md` |
| `release-readiness` | `release` | `auto` | `artifact_verified` | `manual launch checklist` | `.jankurai/repo-score.json, .jankurai/repo-score.md` |
| `cost-budget` | `release` | `auto` | `artifact_verified` | `manual spend review` | `.jankurai/repo-score.json, .jankurai/repo-score.md` |

## Boundary Reclassifications

No audited runtime boundary reclassifications declared.

## Coverage Evidence

- Artifact: `target/jankurai/coverage/coverage-audit.json`
- Status: `missing`
- Sources: total=`0` present=`0`
- Findings: hard=`0` soft=`1`

## Findings

1. `medium` `proof` `Justfile`
   Rule: `HLT-018-PERF-CONCURRENCY-DRIFT`
   Check: `HLT-018-PERF-CONCURRENCY-DRIFT:proof` `soft` confidence `0.76`
   Route: TLR `Verification`, lane `fast`, owner `workspace`
   Docs: `docs/testing.md`
   Reason: `Build speed signals` scored 70 below the standard floor of 85
   Fix: add fast deterministic build/test targets, caches, and narrow proof lanes for agent iteration
   Rerun: `just fast`
   Fingerprint: `sha256:a256a7390d4b91a5b0a95d6f092e524c8f4080f27fe2b62e28cf0801343d0fef`
   Evidence: build acceleration markers found, targeted test/build commands found, locked dependency graph present, CI cache hint found
2. `medium` `boundary` `agent/boundaries.toml`
   Rule: `HLT-007-HANDWRITTEN-CONTRACT`
   Check: `HLT-007-HANDWRITTEN-CONTRACT:boundary` `soft` confidence `0.76`
   Route: TLR `Contracts/data`, lane `contract`, owner `agent`
   Docs: `docs/audit-rubric.md#known-vibe-coding-insults`
   Reason: `Contract and boundary integrity` scored 75 below the standard floor of 85
   Fix: add generated contracts and boundary checks for public APIs, data access, and cross-runtime seams
   Rerun: `just fast`
   Fingerprint: `sha256:993eec07ffbd3370fe6126f0b1f95bf9e6c57a5c7133dc00526a401f4e344e42`
   Evidence: generated contract artifacts found, boundary manifest present, machine-readable schemas present, schema/tooling contract posture is clean
3. `medium` `test` `agent/coverage-sources.toml`
   Rule: `HLT-008-FALSE-GREEN-RISK`
   Check: `HLT-008-FALSE-GREEN-RISK:coverage-evidence` `soft` confidence `0.76`
   Route: TLR `Verification`, lane `coverage-audit`, owner `agent`
   Docs: `docs/testing.md`
   Matched term: `coverage-evidence`
   Reason: coverage evidence artifact `target/jankurai/coverage/coverage-audit.json`
   Fix: run `cargo run -p jankurai -- coverage audit . --config agent/coverage-sources.toml --json target/jankurai/coverage/coverage-audit.json --md target/jankurai/coverage/coverage-audit.md`
   Rerun: `cargo run -p jankurai -- coverage audit . --config agent/coverage-sources.toml --json target/jankurai/coverage/coverage-audit.json --md target/jankurai/coverage/coverage-audit.md`
   Fingerprint: `sha256:d80fbd966029c39ef6dde3a73b2f46834e117ee6c16c85385ccf415d0d3719b5`
   Evidence: agent/coverage-sources.toml exists

## Policy

- Policy file: `./agent/audit-policy.toml`
- Minimum score: `85`
- Fail on: ``

## Agent Fix Queue

1. `medium` `HLT-007-HANDWRITTEN-CONTRACT` `agent/boundaries.toml` - add generated contracts and boundary checks for public APIs, data access, and cross-runtime seams
   Route: `Contracts/data`/`contract`
2. `medium` `HLT-018-PERF-CONCURRENCY-DRIFT` `Justfile` - add fast deterministic build/test targets, caches, and narrow proof lanes for agent iteration
   Route: `Verification`/`fast`
3. `medium` `HLT-008-FALSE-GREEN-RISK` `agent/coverage-sources.toml` - run `cargo run -p jankurai -- coverage audit . --config agent/coverage-sources.toml --json target/jankurai/coverage/coverage-audit.json --md target/jankurai/coverage/coverage-audit.md`
   Route: `Verification`/`coverage-audit`
