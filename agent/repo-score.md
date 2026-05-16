# jankurai Repo Score

- Standard: `jankurai`
- Auditor: `0.8.16`
- Schema: `1.7.0`
- Paper edition: `2026.05-ed8`
- Target stack ID: `rust-ts-vite-react-postgres-bounded-python`
- Target stack: `Rust core + TypeScript/React/Vite + PostgreSQL + generated contracts + exception-only Python AI/data service`
- Repo: `.`
- Run ID: `1778892294`
- Started at: `1778892294`
- Elapsed: `5524` ms
- Scope: `full`
- Raw score: `69`
- Final score: `64`
- Decision: `advisory`
- Minimum score: `85`
- Caps applied: `python-direct-product-truth-or-db-ownership, non-optimal-product-language-found, vibe-placeholders-in-product-code, fallback-soup-in-product-code, future-hostile-dead-language-in-product-code, severe-duplication-in-product-code, authz-or-data-isolation-gap, input-boundary-gap, release-readiness-gap, no-agent-friendly-exception-pattern, missing-agent-readable-docs, rust-bad-behavior, repo-rot-bad-behavior`

## Hard Rule Caps

| Rule | Max Score | Applied |
| --- | ---: | --- |
| `no-root-agent-instructions` | 75 | no |
| `no-one-command-setup-or-validation` | 70 | no |
| `no-deterministic-fast-lane` | 65 | no |
| `no-security-lane-on-high-risk-repo` | 60 | no |
| `generated-contracts-or-public-api-drift-untested` | 80 | no |
| `python-direct-product-truth-or-db-ownership` | 72 | yes |
| `no-secret-or-dependency-scanning-in-ci` | 78 | no |
| `no-jankurai-audit-lane-in-ci` | 82 | no |
| `jankurai-required-tool-ci-evidence-gap` | 88 | no |
| `non-optimal-product-language-found` | 74 | yes |
| `too-much-python-in-product-surface` | 72 | no |
| `boundary-reclassification-evidence-gap` | 72 | no |
| `vibe-placeholders-in-product-code` | 68 | yes |
| `fallback-soup-in-product-code` | 70 | yes |
| `future-hostile-dead-language-in-product-code` | 64 | yes |
| `severe-duplication-in-product-code` | 70 | yes |
| `generated-zone-mutation-risk` | 76 | no |
| `direct-db-access-from-wrong-layer` | 66 | no |
| `missing-web-e2e-lane` | 82 | no |
| `missing-rendered-ux-qa-lane` | 84 | no |
| `prompt-injection-risk` | 78 | no |
| `overbroad-agent-agency` | 65 | no |
| `secret-like-content-detected` | 60 | no |
| `false-green-test-risk` | 76 | no |
| `destructive-migration-risk` | 70 | no |
| `authz-or-data-isolation-gap` | 78 | yes |
| `input-boundary-gap` | 78 | yes |
| `agent-tool-supply-chain-gap` | 78 | no |
| `release-readiness-gap` | 80 | yes |
| `missing-rust-property-or-integration-tests` | 82 | no |
| `no-agent-friendly-exception-pattern` | 76 | yes |
| `missing-agent-readable-docs` | 80 | yes |
| `streaming-runtime-drift` | 78 | no |
| `rust-bad-behavior` | 72 | yes |
| `sql-bad-behavior` | 72 | no |
| `typescript-bad-behavior` | 72 | no |
| `docker-bad-behavior` | 72 | no |
| `python-bad-behavior` | 72 | no |
| `ci-bad-behavior` | 70 | no |
| `git-bad-behavior` | 70 | no |
| `gittools-bad-behavior` | 70 | no |
| `release-bad-behavior` | 70 | no |
| `web-security-bad-behavior` | 68 | no |
| `repo-rot-bad-behavior` | 88 | yes |
| `comment-hygiene-dangerous-residue` | 72 | no |

## Dimensions

| Dimension | Weight | Score | Weighted | Evidence |
| --- | ---: | ---: | ---: | --- |
| Ownership and navigation surface | 13 | 100 | 13.00 | root `AGENTS.md` present; `CODEOWNERS` present |
| Contract and boundary integrity | 13 | 98 | 12.74 | contract surface found; generated contract artifacts found |
| Proof lanes and test routing | 12 | 100 | 12.00 | one-command setup/validation lane found; deterministic fast lane found |
| Security and supply-chain posture | 12 | 78 | 9.36 | lockfile present; secret or dependency scan tooling found |
| Code shape and semantic surface | 12 | 0 | 0.00 | largest authored code file: crates/sql/src/connection.rs (972 LOC); code file exceeds 500 LOC |
| Data truth and workflow safety | 8 | 95 | 7.60 | database surface present; structured db boundary manifest present |
| Observability and repair evidence | 8 | 48 | 3.84 | observability libraries or patterns found; ops/observability directory present |
| Context economy and agent instructions | 7 | 81 | 5.67 | root `AGENTS.md` present; root `AGENTS.md` stays short |
| Jankurai tool adoption and CI replacement | 7 | 31 | 2.17 | control-plane files present; applicable=15 |
| Python containment and polyglot hygiene | 4 | 0 | 0.00 | Python appears outside the allowed roots: scripts/bench/dick_head_choas_report.py; non-optimal product language marker: crates/ffi/include/redlinedb.h |
| Build speed signals | 4 | 70 | 2.80 | build acceleration markers found; targeted test/build commands found |

## Reference Profile Structure

- Applicable cells: `9` canonical=`9` noncanonical=`0` guidance missing=`0`

| Cell | Status | Canonical | Detected | Aliases | Guidance | Owner | Proof lane | Agent fix |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `web` | `not_applicable` | `apps/web/` | `-` | `frontend/, ui/, packages/web/, packages/ui/` | `not_required` | `apps/web` | `rendered UX / Playwright` | `no action` |
| `api` | `canonical` | `apps/api/` | `apps/api` | `api/, server/, backend/` | `present` | `apps/api` | `edge handler / contract tests` | `keep `apps/api/AGENTS.md` aligned with owns / forbidden / proof lane guidance` |
| `domain` | `canonical` | `crates/domain/` | `crates/domain` | `domain/, core/` | `present` | `crates/domain` | `unit / property tests` | `keep `crates/domain/AGENTS.md` aligned with owns / forbidden / proof lane guidance` |
| `application` | `canonical` | `crates/application/` | `crates/application` | `application/, usecases/, use-cases/` | `present` | `crates/application` | `use-case / authz tests` | `keep `crates/application/AGENTS.md` aligned with owns / forbidden / proof lane guidance` |
| `adapters` | `canonical` | `crates/adapters/` | `crates/adapters` | `adapters/, infra/, integrations/` | `present` | `crates/adapters` | `adapter integration tests` | `keep `crates/adapters/AGENTS.md` aligned with owns / forbidden / proof lane guidance` |
| `workers` | `canonical` | `crates/workers/` | `crates/workers` | `workers/, jobs/, scheduler/, queue/` | `present` | `crates/workers` | `workflow / replay tests` | `keep `crates/workers/AGENTS.md` aligned with owns / forbidden / proof lane guidance` |
| `contracts` | `canonical` | `contracts/` | `contracts` | `openapi/, protobuf/, json-schema/, generated/` | `present` | `contracts` | `generation / drift checks` | `keep `contracts/AGENTS.md` aligned with owns / forbidden / proof lane guidance` |
| `db` | `canonical` | `db/` | `db` | `migrations/, constraints/, sql/` | `present` | `db` | `migration / constraint tests` | `keep `db/AGENTS.md` aligned with owns / forbidden / proof lane guidance` |
| `python-ai` | `canonical` | `python/ai-service/` | `python, python/ai-service` | `python/, ai-service/, evals/, embeddings/, model/` | `present` | `python/ai-service` | `eval / contract tests` | `keep `python/ai-service/AGENTS.md` aligned with owns / forbidden / proof lane guidance` |
| `ops` | `canonical` | `ops/` | `.github, .github/workflows, ops` | `.github/, .github/workflows/, ci/, release/, observability/, security/` | `present` | `ops` | `security lane / workflow lint` | `keep `ops/AGENTS.md` aligned with owns / forbidden / proof lane guidance` |

## Rendered UX QA

- Web surface: `false`
- Layered UX lane: `true`
- Missing: `none`

## Tool Adoption

- Control plane present: `true`
- Applicable tools: `15`
- Configured: `5`
- CI evidence: `3`
- Artifact verified: `3`
- Replaced count: `3`
- Missing CI evidence: `audit-ci, proof-routing, security, ci-bad-behavior, git-bad-behavior, release-bad-behavior, contract-drift, authz-matrix, input-boundary, agent-tool-supply, release-readiness, cost-budget`

| Tool | Category | Mode | Status | Replaced | Artifacts |
| --- | --- | --- | --- | --- | --- |
| `audit-ci` | `audit` | `auto` | `configured` | `manual repo scoring, ad hoc score gates` | `agent/repo-score.json, agent/repo-score.md` |
| `proof-routing` | `proof` | `auto` | `configured` | `ad hoc proof lane selection, manual proof receipts` | `agent/repo-score.json, agent/repo-score.md, target/jankurai/repair-queue.jsonl` |
| `proofbind` | `proof` | `auto` | `artifact_verified` | `manual changed-surface routing, ad hoc proof obligation lists` | `target/jankurai/proofbind/surface-witness.json, target/jankurai/proofbind/obligations.json` |
| `proofmark-rust` | `proof` | `auto` | `artifact_verified` | `line-only coverage review, manual in-diff mutation review` | `target/jankurai/proofmark/proofmark-receipt.json, target/jankurai/proofmark/proof-receipt.json` |
| `security` | `security` | `auto` | `configured` | `gitleaks, dependency review, SBOM/provenance` | `target/jankurai/security/evidence.json` |
| `ci-bad-behavior` | `security` | `auto` | `missing` | `mutable workflow refs, secret echo/debug workflow checks, non-blocking security scans` | `target/jankurai/language-bad-behavior.log` |
| `git-bad-behavior` | `audit` | `auto` | `missing` | `destructive git automation, force-push release scripts, hidden stash-based state` | `target/jankurai/language-bad-behavior.log` |
| `release-bad-behavior` | `release` | `auto` | `missing` | `manual release checklist, ad hoc tag and artifact review, manual provenance review` | `target/jankurai/language-bad-behavior.log` |
| `ux-qa` | `ux` | `auto` | `not_applicable` | `playwright, axe-core, visual baselines` | `target/jankurai/ux-qa.json` |
| `db-migration-analyze` | `db` | `auto` | `not_applicable` | `manual migration review` | `target/jankurai/migration-report.json` |
| `contract-drift` | `contract` | `auto` | `configured` | `handwritten contract drift checks, openapi diff` | `agent/repo-score.json, agent/repo-score.md` |
| `rust-witness` | `rust` | `auto` | `artifact_verified` | `manual witness graphing` | `target/jankurai/rust/witness-graph.json` |
| `vibe-coverage` | `audit` | `auto` | `not_applicable` | `manual vibe-coding coverage spreadsheet` | `target/jankurai/vibe-coverage.json, target/jankurai/vibe-coverage.md` |
| `coverage-evidence` | `proof` | `auto` | `not_applicable` | `manual coverage report review, ad hoc mutation survivor review` | `target/jankurai/coverage/coverage-audit.json, target/jankurai/coverage/coverage-audit.md` |
| `authz-matrix` | `security` | `auto` | `missing` | `manual authz matrix review` | `agent/repo-score.json, agent/repo-score.md` |
| `input-boundary` | `security` | `auto` | `missing` | `manual unsafe sink review` | `agent/repo-score.json, agent/repo-score.md` |
| `agent-tool-supply` | `security` | `auto` | `missing` | `manual MCP/tool trust review` | `agent/repo-score.json, agent/repo-score.md` |
| `release-readiness` | `release` | `auto` | `missing` | `manual launch checklist` | `agent/repo-score.json, agent/repo-score.md` |
| `cost-budget` | `release` | `auto` | `missing` | `manual spend review` | `agent/repo-score.json, agent/repo-score.md` |

## Boundary manifest (ingested)

- Path: `agent/boundaries.toml`
- Stack: `rust-ts-vite-react-postgres-bounded-python` · version: `0.4.0`
- Queue path counts — adapter: `2`, event_contract: `1`, generated_type: `1`, client_marker: `7`, streaming_exception: `1`
- Content fingerprint: `sha256:7a7bf640d469152bca6f0a9a4feabb90ae6eb170f3fe33a196871bfefb3936e3`

## Boundary Reclassifications

No audited runtime boundary reclassifications declared.

## Findings

1. `medium` `shape` `.`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:shape` `soft` confidence `0.76`
   Route: TLR `Entropy`, lane `fast`, owner `tools`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: `Code shape and semantic surface` scored 0 below the standard floor of 85
   Fix: split large or ambiguous authored code into smaller semantic modules with focused tests
   Rerun: `just fast`
   Fingerprint: `sha256:c5dbc05e9b548067a555090cb830057c11a73f679a3b9e29d2caa0a4d72fae2f`
   Evidence: largest authored code file: crates/sql/src/connection.rs (972 LOC), code file exceeds 500 LOC, duplicate code block marker found, TODO/stub marker found
2. `medium` `security` `.github/workflows/jankurai.yml`
   Rule: `HLT-016-SUPPLY-CHAIN-DRIFT`
   Check: `HLT-016-SUPPLY-CHAIN-DRIFT:security` `soft` confidence `0.76`
   Route: TLR `Security, secrets, agency`, lane `security`, owner `ops`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Reason: `Security and supply-chain posture` scored 78 below the standard floor of 85
   Fix: wire secret, dependency, provenance, and workflow scans into an operational CI lane
   Rerun: `just security`
   Fingerprint: `sha256:d24ab5697e66411af8d5424d1d36ebf888793ebced3685d5fa95bb912e9f12e2`
   Evidence: lockfile present, secret or dependency scan tooling found, provenance/SBOM tooling found, security lane present
3. `medium` `context` `AGENTS.md`
   Rule: `HLT-015-CONTEXT-SETUP-GAP`
   Check: `HLT-015-CONTEXT-SETUP-GAP:context` `soft` confidence `0.76`
   Route: TLR `Context/setup`, lane `fast`, owner `agent`
   Docs: `docs/agent-native-standard.md`
   Reason: `Context economy and agent instructions` scored 81 below the standard floor of 85
   Fix: keep root guidance short and route durable detail through agent-readable manifests and docs
   Rerun: `just fast`
   Fingerprint: `sha256:ae7ec2d93126bb6d5f051ba62ace7e1034fc828ae729862f50a5eb22994c4ede`
   Evidence: root `AGENTS.md` present, root `AGENTS.md` stays short, machine-readable routing artifacts present, thin IDE/agent adapters are present
4. `medium` `proof` `Justfile`
   Rule: `HLT-018-PERF-CONCURRENCY-DRIFT`
   Check: `HLT-018-PERF-CONCURRENCY-DRIFT:proof` `soft` confidence `0.76`
   Route: TLR `Verification`, lane `fast`, owner `workspace`
   Docs: `docs/testing.md`
   Reason: `Build speed signals` scored 70 below the standard floor of 85
   Fix: add fast deterministic build/test targets, caches, and narrow proof lanes for agent iteration
   Rerun: `just fast`
   Fingerprint: `sha256:a256a7390d4b91a5b0a95d6f092e524c8f4080f27fe2b62e28cf0801343d0fef`
   Evidence: build acceleration markers found, targeted test/build commands found, locked dependency graph present, CI cache hint found
5. `high` `context` `agent/owner-map.json`
   Rule: `HLT-003-OWNERLESS-PATH`
   Check: `HLT-003-OWNERLESS-PATH:context` `hard` confidence `0.88`
   Route: TLR `Context/setup`, lane `fast`, owner `agent`
   Docs: `agent/JANKURAI_STANDARD.md#ownership-boundaries`
   Reason: path `.gitignore` has no owner-map route
   Fix: add the narrowest stable prefix for this path to `agent/owner-map.json`
   Rerun: `just fast`
   Fingerprint: `sha256:73b0d05063e2ba8c1c0cdee444a5be8dda239dbe9748b61427699317c5c33f4b`
   Evidence: .gitignore
6. `high` `context` `agent/owner-map.json`
   Rule: `HLT-003-OWNERLESS-PATH`
   Check: `HLT-003-OWNERLESS-PATH:context` `hard` confidence `0.88`
   Route: TLR `Context/setup`, lane `fast`, owner `agent`
   Docs: `agent/JANKURAI_STANDARD.md#ownership-boundaries`
   Reason: path `CHANGELOG.md` has no owner-map route
   Fix: add the narrowest stable prefix for this path to `agent/owner-map.json`
   Rerun: `just fast`
   Fingerprint: `sha256:2911a47f9548e75233bc304c284818b5b7fb6c72ecde5d7bc5c97be0635cd18a`
   Evidence: CHANGELOG.md
7. `high` `context` `agent/owner-map.json`
   Rule: `HLT-003-OWNERLESS-PATH`
   Check: `HLT-003-OWNERLESS-PATH:context` `hard` confidence `0.88`
   Route: TLR `Context/setup`, lane `fast`, owner `agent`
   Docs: `agent/JANKURAI_STANDARD.md#ownership-boundaries`
   Reason: path `CLAUDE.md` has no owner-map route
   Fix: add the narrowest stable prefix for this path to `agent/owner-map.json`
   Rerun: `just fast`
   Fingerprint: `sha256:bd8f4e8361bf13c5d18526bd2f35de9a89447626ad25a0e87883fe1785d17399`
   Evidence: CLAUDE.md
8. `high` `context` `agent/owner-map.json`
   Rule: `HLT-003-OWNERLESS-PATH`
   Check: `HLT-003-OWNERLESS-PATH:context` `hard` confidence `0.88`
   Route: TLR `Context/setup`, lane `fast`, owner `agent`
   Docs: `agent/JANKURAI_STANDARD.md#ownership-boundaries`
   Reason: path `CONTRIBUTING.md` has no owner-map route
   Fix: add the narrowest stable prefix for this path to `agent/owner-map.json`
   Rerun: `just fast`
   Fingerprint: `sha256:53c38db6efc8c868fc6490a7ca739dad4dd43e68ebdf098f88f6e1df99795075`
   Evidence: CONTRIBUTING.md
9. `high` `context` `agent/owner-map.json`
   Rule: `HLT-003-OWNERLESS-PATH`
   Check: `HLT-003-OWNERLESS-PATH:context` `hard` confidence `0.88`
   Route: TLR `Context/setup`, lane `fast`, owner `agent`
   Docs: `agent/JANKURAI_STANDARD.md#ownership-boundaries`
   Reason: path `Cargo.lock` has no owner-map route
   Fix: add the narrowest stable prefix for this path to `agent/owner-map.json`
   Rerun: `just fast`
   Fingerprint: `sha256:b039e38fe066b0ca37060986515b6a5787a63d246c5dada2a12dc16aec26381a`
   Evidence: Cargo.lock
10. `high` `context` `agent/owner-map.json`
   Rule: `HLT-003-OWNERLESS-PATH`
   Check: `HLT-003-OWNERLESS-PATH:context` `hard` confidence `0.88`
   Route: TLR `Context/setup`, lane `fast`, owner `agent`
   Docs: `agent/JANKURAI_STANDARD.md#ownership-boundaries`
   Reason: path `Cargo.toml` has no owner-map route
   Fix: add the narrowest stable prefix for this path to `agent/owner-map.json`
   Rerun: `just fast`
   Fingerprint: `sha256:5cee550532ba9f298e31a18ffa4f8b841e75577b203144fda3ac04b25761dd79`
   Evidence: Cargo.toml
11. `high` `context` `agent/owner-map.json`
   Rule: `HLT-003-OWNERLESS-PATH`
   Check: `HLT-003-OWNERLESS-PATH:context` `hard` confidence `0.88`
   Route: TLR `Context/setup`, lane `fast`, owner `agent`
   Docs: `agent/JANKURAI_STANDARD.md#ownership-boundaries`
   Reason: path `EXTERNAL_REVIEW_MASTER_PROMPT.md` has no owner-map route
   Fix: add the narrowest stable prefix for this path to `agent/owner-map.json`
   Rerun: `just fast`
   Fingerprint: `sha256:1935ca50ef08cf279640d1c3e0153d2d482a0acb4bca8407c4e3af43234a938c`
   Evidence: EXTERNAL_REVIEW_MASTER_PROMPT.md
12. `high` `context` `agent/owner-map.json`
   Rule: `HLT-003-OWNERLESS-PATH`
   Check: `HLT-003-OWNERLESS-PATH:context` `hard` confidence `0.88`
   Route: TLR `Context/setup`, lane `fast`, owner `agent`
   Docs: `agent/JANKURAI_STANDARD.md#ownership-boundaries`
   Reason: path `EXTERNAL_REVIEW_SUPERPROMPT.md` has no owner-map route
   Fix: add the narrowest stable prefix for this path to `agent/owner-map.json`
   Rerun: `just fast`
   Fingerprint: `sha256:3f14f6d5231d151e242cc9c7a559075a1d73db88b624698465a3cde4fab6a7be`
   Evidence: EXTERNAL_REVIEW_SUPERPROMPT.md
13. `high` `context` `agent/owner-map.json`
   Rule: `HLT-003-OWNERLESS-PATH`
   Check: `HLT-003-OWNERLESS-PATH:context` `hard` confidence `0.88`
   Route: TLR `Context/setup`, lane `fast`, owner `agent`
   Docs: `agent/JANKURAI_STANDARD.md#ownership-boundaries`
   Reason: path `FEATURE_GAPS.md` has no owner-map route
   Fix: add the narrowest stable prefix for this path to `agent/owner-map.json`
   Rerun: `just fast`
   Fingerprint: `sha256:2372d1d68ea2f342f127a3533021ca0165f023c2f5e2f5c395fc0c532e58561d`
   Evidence: FEATURE_GAPS.md
14. `high` `context` `agent/owner-map.json`
   Rule: `HLT-003-OWNERLESS-PATH`
   Check: `HLT-003-OWNERLESS-PATH:context` `hard` confidence `0.88`
   Route: TLR `Context/setup`, lane `fast`, owner `agent`
   Docs: `agent/JANKURAI_STANDARD.md#ownership-boundaries`
   Reason: path `GEMINI.md` has no owner-map route
   Fix: add the narrowest stable prefix for this path to `agent/owner-map.json`
   Rerun: `just fast`
   Fingerprint: `sha256:271eabd1e2dad2560b72f19c12fb8b519af9a79fabf2b83e29243d4c83ecefed`
   Evidence: GEMINI.md
15. `high` `proof` `agent/test-map.json`
   Rule: `HLT-004-UNMAPPED-PROOF`
   Check: `HLT-004-UNMAPPED-PROOF:proof` `hard` confidence `0.88`
   Route: TLR `Verification`, lane `fast`, owner `agent`
   Docs: `agent/JANKURAI_STANDARD.md#proof-lanes`
   Reason: path `.github/CODEOWNERS` has no test-map proof route
   Fix: add the narrowest stable prefix and runnable proof command to `agent/test-map.json`
   Rerun: `just fast`
   Fingerprint: `sha256:c6facbfbb742d951249284cc037845c1f2ab2a1d5213a03c80b4c687898d6ab3`
   Evidence: .github/CODEOWNERS
16. `high` `proof` `agent/test-map.json`
   Rule: `HLT-004-UNMAPPED-PROOF`
   Check: `HLT-004-UNMAPPED-PROOF:proof` `hard` confidence `0.88`
   Route: TLR `Verification`, lane `fast`, owner `agent`
   Docs: `agent/JANKURAI_STANDARD.md#proof-lanes`
   Reason: path `.github/PULL_REQUEST_TEMPLATE.md` has no test-map proof route
   Fix: add the narrowest stable prefix and runnable proof command to `agent/test-map.json`
   Rerun: `just fast`
   Fingerprint: `sha256:6be0a2dfcc6bb22c4a24996ced5349c39b8361695babbd981ac6482c1979a121`
   Evidence: .github/PULL_REQUEST_TEMPLATE.md
17. `high` `proof` `agent/test-map.json`
   Rule: `HLT-004-UNMAPPED-PROOF`
   Check: `HLT-004-UNMAPPED-PROOF:proof` `hard` confidence `0.88`
   Route: TLR `Verification`, lane `fast`, owner `agent`
   Docs: `agent/JANKURAI_STANDARD.md#proof-lanes`
   Reason: path `.github/copilot-instructions.md` has no test-map proof route
   Fix: add the narrowest stable prefix and runnable proof command to `agent/test-map.json`
   Rerun: `just fast`
   Fingerprint: `sha256:1d8715b91aa1798a04a0f8eb739d2009aa97b0f5e933a34d7e8dc7a97a44bfde`
   Evidence: .github/copilot-instructions.md
18. `high` `proof` `agent/test-map.json`
   Rule: `HLT-004-UNMAPPED-PROOF`
   Check: `HLT-004-UNMAPPED-PROOF:proof` `hard` confidence `0.88`
   Route: TLR `Verification`, lane `fast`, owner `agent`
   Docs: `agent/JANKURAI_STANDARD.md#proof-lanes`
   Reason: path `.github/instructions/jankurai-rust.instructions.md` has no test-map proof route
   Fix: add the narrowest stable prefix and runnable proof command to `agent/test-map.json`
   Rerun: `just fast`
   Fingerprint: `sha256:769147261ab786beb2930c92df19fca83e809443cbdb1387cc3b6f6bda643a89`
   Evidence: .github/instructions/jankurai-rust.instructions.md
19. `high` `proof` `agent/test-map.json`
   Rule: `HLT-004-UNMAPPED-PROOF`
   Check: `HLT-004-UNMAPPED-PROOF:proof` `hard` confidence `0.88`
   Route: TLR `Verification`, lane `fast`, owner `agent`
   Docs: `agent/JANKURAI_STANDARD.md#proof-lanes`
   Reason: path `.github/instructions/jankurai-web.instructions.md` has no test-map proof route
   Fix: add the narrowest stable prefix and runnable proof command to `agent/test-map.json`
   Rerun: `just fast`
   Fingerprint: `sha256:2e6c740bb8b013b5bcecf365e91d5fc8361c9963155e98596eeb409042f6688c`
   Evidence: .github/instructions/jankurai-web.instructions.md
20. `high` `proof` `agent/test-map.json`
   Rule: `HLT-004-UNMAPPED-PROOF`
   Check: `HLT-004-UNMAPPED-PROOF:proof` `hard` confidence `0.88`
   Route: TLR `Verification`, lane `fast`, owner `agent`
   Docs: `agent/JANKURAI_STANDARD.md#proof-lanes`
   Reason: path `.github/instructions/jankurai.instructions.md` has no test-map proof route
   Fix: add the narrowest stable prefix and runnable proof command to `agent/test-map.json`
   Rerun: `just fast`
   Fingerprint: `sha256:0af736bb824df4107187600e202e8960d55f3b081501863062f57ea9d3a20e69`
   Evidence: .github/instructions/jankurai.instructions.md
21. `high` `proof` `agent/test-map.json`
   Rule: `HLT-004-UNMAPPED-PROOF`
   Check: `HLT-004-UNMAPPED-PROOF:proof` `hard` confidence `0.88`
   Route: TLR `Verification`, lane `fast`, owner `agent`
   Docs: `agent/JANKURAI_STANDARD.md#proof-lanes`
   Reason: path `.github/workflows/ci.yml` has no test-map proof route
   Fix: add the narrowest stable prefix and runnable proof command to `agent/test-map.json`
   Rerun: `just fast`
   Fingerprint: `sha256:e8b9c4d51fd3f1a3ed0e0507f920035cc2c51d1be80cb468f20e9b3d9056c8ed`
   Evidence: .github/workflows/ci.yml
22. `high` `proof` `agent/test-map.json`
   Rule: `HLT-004-UNMAPPED-PROOF`
   Check: `HLT-004-UNMAPPED-PROOF:proof` `hard` confidence `0.88`
   Route: TLR `Verification`, lane `fast`, owner `agent`
   Docs: `agent/JANKURAI_STANDARD.md#proof-lanes`
   Reason: path `.github/workflows/jankurai.yml` has no test-map proof route
   Fix: add the narrowest stable prefix and runnable proof command to `agent/test-map.json`
   Rerun: `just fast`
   Fingerprint: `sha256:fbe0f82818625191cd5ba476f315b601a7e78ed1635009d516a469724f230a3e`
   Evidence: .github/workflows/jankurai.yml
23. `high` `proof` `agent/test-map.json`
   Rule: `HLT-004-UNMAPPED-PROOF`
   Check: `HLT-004-UNMAPPED-PROOF:proof` `hard` confidence `0.88`
   Route: TLR `Verification`, lane `fast`, owner `agent`
   Docs: `agent/JANKURAI_STANDARD.md#proof-lanes`
   Reason: path `.gitignore` has no test-map proof route
   Fix: add the narrowest stable prefix and runnable proof command to `agent/test-map.json`
   Rerun: `just fast`
   Fingerprint: `sha256:8fe7edc74ce7873d59c9b6ccb57a19bca48fe9589c015113fb5028c2cc245992`
   Evidence: .gitignore
24. `high` `proof` `agent/test-map.json`
   Rule: `HLT-004-UNMAPPED-PROOF`
   Check: `HLT-004-UNMAPPED-PROOF:proof` `hard` confidence `0.88`
   Route: TLR `Verification`, lane `fast`, owner `agent`
   Docs: `agent/JANKURAI_STANDARD.md#proof-lanes`
   Reason: path `CHANGELOG.md` has no test-map proof route
   Fix: add the narrowest stable prefix and runnable proof command to `agent/test-map.json`
   Rerun: `just fast`
   Fingerprint: `sha256:eaa40b784b5bb02ef321df1133743a622cda79939f89a05728e47c374dc90555`
   Evidence: CHANGELOG.md
25. `medium` `context` `crates/bench/bench/certification-phase10-v3-compare.toml:1`
   Rule: `HLT-040-REPO-ROT-BAD-BEHAVIOR`
   Check: `HLT-040-REPO-ROT-BAD-BEHAVIOR:context` `soft` confidence `0.88`
   Route: TLR `Context/setup`, lane `audit`, owner `bench-harness`
   Docs: `docs/language-bad-behavior.md#web-security-and-repo-rot-detectors`
   Matched term: `repo-rot.path.fake-versioned-source`
   Reason: ambiguous old-looking active source makes agents and reviewers guess whether code is live
   Fix: delete the stale copy, move history to VCS/archive tooling, or document owner, proof lane, expiry, and migration plan
   Rerun: `just score`
   Fingerprint: `sha256:22d5fc685fc8b529830b0f91bf40cc745ed3fb6b7b2a20ce9fe4d8fd1ec92562`
   Evidence: detector=repo-rot.path.fake-versioned-source, path=crates/bench/bench/certification-phase10-v3-compare.toml, line=1, proof_window=None, snippet=out_dir = "target/bench/xbabe1/phase10-cert-v3-compare"
26. `medium` `context` `crates/bench/bench/certification-phase10-v3-smoke.toml:1`
   Rule: `HLT-040-REPO-ROT-BAD-BEHAVIOR`
   Check: `HLT-040-REPO-ROT-BAD-BEHAVIOR:context` `soft` confidence `0.88`
   Route: TLR `Context/setup`, lane `audit`, owner `bench-harness`
   Docs: `docs/language-bad-behavior.md#web-security-and-repo-rot-detectors`
   Matched term: `repo-rot.path.fake-versioned-source`
   Reason: ambiguous old-looking active source makes agents and reviewers guess whether code is live
   Fix: delete the stale copy, move history to VCS/archive tooling, or document owner, proof lane, expiry, and migration plan
   Rerun: `just score`
   Fingerprint: `sha256:307a51211739992f3e48fa484a9b85323518afc2189be1e0929d66c770c0194c`
   Evidence: detector=repo-rot.path.fake-versioned-source, path=crates/bench/bench/certification-phase10-v3-smoke.toml, line=1, proof_window=None, snippet=out_dir = "target/bench/phase10-cert-v3-smoke"
27. `medium` `context` `crates/bench/bench/certification-phase10-v3-stress.toml:1`
   Rule: `HLT-040-REPO-ROT-BAD-BEHAVIOR`
   Check: `HLT-040-REPO-ROT-BAD-BEHAVIOR:context` `soft` confidence `0.88`
   Route: TLR `Context/setup`, lane `audit`, owner `bench-harness`
   Docs: `docs/language-bad-behavior.md#web-security-and-repo-rot-detectors`
   Matched term: `repo-rot.path.fake-versioned-source`
   Reason: ambiguous old-looking active source makes agents and reviewers guess whether code is live
   Fix: delete the stale copy, move history to VCS/archive tooling, or document owner, proof lane, expiry, and migration plan
   Rerun: `just score`
   Fingerprint: `sha256:b35841ee76d63fbe95d4c1aa00cbf9d9fe090869149939cbcb4cf6f84829c5e1`
   Evidence: detector=repo-rot.path.fake-versioned-source, path=crates/bench/bench/certification-phase10-v3-stress.toml, line=1, proof_window=None, snippet=out_dir = "target/bench/xbabe1/phase10-cert-v3-stress"
28. `medium` `context` `crates/bench/bench/certification-phase10-v3.toml:1`
   Rule: `HLT-040-REPO-ROT-BAD-BEHAVIOR`
   Check: `HLT-040-REPO-ROT-BAD-BEHAVIOR:context` `soft` confidence `0.88`
   Route: TLR `Context/setup`, lane `audit`, owner `bench-harness`
   Docs: `docs/language-bad-behavior.md#web-security-and-repo-rot-detectors`
   Matched term: `repo-rot.path.fake-versioned-source`
   Reason: ambiguous old-looking active source makes agents and reviewers guess whether code is live
   Fix: delete the stale copy, move history to VCS/archive tooling, or document owner, proof lane, expiry, and migration plan
   Rerun: `just score`
   Fingerprint: `sha256:65b64cd6b8fafcb14aa5bcb22394441d4c2ab33bb00623426d4f83a7d2d89aac`
   Evidence: detector=repo-rot.path.fake-versioned-source, path=crates/bench/bench/certification-phase10-v3.toml, line=1, proof_window=None, snippet=out_dir = "target/bench/xbabe1/phase10-cert-v3-cert"
29. `high` `vibe` `crates/bench/src/certify.rs:213`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `bench-harness`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `placeholder` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:399beb49d16b534a88a7335e6984c1e83bdee985875a2fcc36adf0120cea0df5`
   Evidence: crates/bench/src/certify.rs:213, future-hostile/dead-language term `placeholder` appears
30. `high` `vibe` `crates/bench/src/certify.rs:244`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `bench-harness`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: fallback soup detected in product code
   Fix: collapse fallback chains into explicit typed states with bounded retry policy, telemetry, and documented repair guidance
   Rerun: `just fast`
   Fingerprint: `sha256:c7c5bb20208bb49bd7962fd71d37ff2b181e731d8909e525920ffa2659cbc8ba`
   Evidence: crates/bench/src/certify.rs:244 let payload = serde_json::to_vec(&record.checksum).unwrap_or_default();
31. `high` `vibe` `crates/bench/src/chaos.rs:318`
   Check: `HLT-000-SCORE-DIMENSION:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `bench-harness`
   Reason: duplicated product code block detected
   Fix: extract the duplicated behavior behind one named boundary and add focused tests before changing behavior
   Rerun: `just fast`
   Fingerprint: `sha256:14d51d274955871873d5873633d398981321dcd845f47ce0f86842c09eeb6406`
   Evidence: duplicate block also appears at crates/bench/src/chaos.rs:284
32. `high` `vibe` `crates/bench/src/chaos.rs:391`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `bench-harness`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `temp` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:2e2f6e69c152833f03207311f3baae1cd25d201fb2465238e351555021b8c4d1`
   Evidence: crates/bench/src/chaos.rs:391, future-hostile/dead-language term `temp` appears
33. `high` `vibe` `crates/bench/src/chaos.rs:394`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `bench-harness`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `temp` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:210b5776dc56a55979f19cdeab6343ff0540990b260a0623c9495a6aa3d09e4c`
   Evidence: crates/bench/src/chaos.rs:394, future-hostile/dead-language term `temp` appears
34. `high` `vibe` `crates/bench/src/checksum.rs:3`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `phase10-real-row-hashes`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `placeholder` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:f6b52a29a0c2bdb3a7f57d45a69e5732ebd2e50d9292a53a7b80b0727f0ff225`
   Evidence: crates/bench/src/checksum.rs:3, future-hostile/dead-language term `placeholder` appears
35. `high` `vibe` `crates/bench/src/compat.rs:35`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `bench-harness`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `compat` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:56103641411309fbf851ae147514bef0dea619275473035a87fa18a46a67ddac`
   Evidence: crates/bench/src/compat.rs:35, future-hostile/dead-language term `compat` appears
36. `high` `vibe` `crates/bench/src/compat.rs:55`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `bench-harness`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `compat` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:0bbe155063bab0a83055d29c27fe9e5ff2ed5bfdffcf6ecd6063ce4c539f875b`
   Evidence: crates/bench/src/compat.rs:55, future-hostile/dead-language term `compat` appears
37. `high` `vibe` `crates/bench/src/compat.rs:64`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `bench-harness`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `compat` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:870fac236373da890324811c8e34318201369f193ac80530bb4ab6fb8d946a85`
   Evidence: crates/bench/src/compat.rs:64, future-hostile/dead-language term `compat` appears
38. `high` `vibe` `crates/bench/src/compat.rs:144`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `bench-harness`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `compat` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:d811fcb147ef69ae0c5d9b987c9f575599521acc2e138768662041816710c023`
   Evidence: crates/bench/src/compat.rs:144, future-hostile/dead-language term `compat` appears
39. `high` `vibe` `crates/bench/src/compat.rs:174`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `bench-harness`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `temp` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:95941d1d6d95f05f9fa6d61475a67b65f34b260935422cc6b49175c56b3e4205`
   Evidence: crates/bench/src/compat.rs:174, future-hostile/dead-language term `temp` appears
40. `high` `vibe` `crates/bench/src/config.rs:22`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `bench-harness`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `compat` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:e5d1659cd05b2317395638969761287eff33e30c45946945e8bb3f4feccb8977`
   Evidence: crates/bench/src/config.rs:22, future-hostile/dead-language term `compat` appears
41. `high` `security` `crates/bench/src/config.rs:291`
   Rule: `HLT-022-AUTHZ-ISOLATION-GAP`
   Check: `HLT-022-AUTHZ-ISOLATION-GAP:security` `hard` confidence `0.88`
   Route: TLR `Business truth`, lane `db`, owner `bench-harness`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `tenant_id`
   Reason: authz/data isolation requires negative proof evidence
   Fix: add owner/non-owner authorization tests or RLS evidence for the touched data boundary
   Rerun: `just fast`
   Fingerprint: `sha256:a6afd78567ed0a67ae7fc634e599aca5bbb21557df1590c37150845fcec6d7c9`
   Evidence: /// existing `kv_tenant_idx`. Fixture shape mirrors
42. `high` `vibe` `crates/bench/src/config.rs:371`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `bench-harness`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `temp` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:b576b67354810d7d5e5bc6201f18b199eba4f34a5c1034248fe16e9b25a9cfeb`
   Evidence: crates/bench/src/config.rs:371, future-hostile/dead-language term `temp` appears
43. `high` `vibe` `crates/bench/src/config.rs:517`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `bench-harness`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `legacy` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:43fd71e385ecac7965e4bbf68bbf0a3d67f9e2f23aa95fb30c4b1872542971c9`
   Evidence: crates/bench/src/config.rs:517, future-hostile/dead-language term `legacy` appears
44. `high` `vibe` `crates/bench/src/config.rs:649`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `bench-harness`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `temp` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:afe87668200f10b1f41151a8da70a3e0bfa23b7870358c7d22bf6e38354902c3`
   Evidence: crates/bench/src/config.rs:649, future-hostile/dead-language term `temp` appears
45. `high` `vibe` `crates/bench/src/engine/redline.rs:220`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `bench-harness`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `legacy` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:7db0c6c4a23747a0a68ee321758923577a92ea3c5a5ee24fb98a377f4ceda6e2`
   Evidence: crates/bench/src/engine/redline.rs:220, future-hostile/dead-language term `legacy` appears
46. `high` `vibe` `crates/bench/src/feature_workloads.rs:86`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `bench-harness`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `temp` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:2a69182cba9b904b20c4788f9c19d91e95221e0261c233ffd9b070139d4f6830`
   Evidence: crates/bench/src/feature_workloads.rs:86, future-hostile/dead-language term `temp` appears
47. `high` `vibe` `crates/bench/src/feature_workloads.rs:87`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `bench-harness`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `temp` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:74aeb4328334397b97d21fe27bd4a4d4f0a206e3e5174d5d593bc60a1ec7c6ae`
   Evidence: crates/bench/src/feature_workloads.rs:87, future-hostile/dead-language term `temp` appears
48. `high` `vibe` `crates/bench/src/feature_workloads.rs:135`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `bench-harness`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `temp` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:f66fca02b7be01c8fbcf4e07793f50dac6a440861fe400372aed808d4fcc98b0`
   Evidence: crates/bench/src/feature_workloads.rs:135, future-hostile/dead-language term `temp` appears
49. `high` `vibe` `crates/bench/src/feature_workloads.rs:136`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `bench-harness`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `temp` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:bb09689da959015889681a07730bd5f917e3cc1b210b853df00e42a6afea3da3`
   Evidence: crates/bench/src/feature_workloads.rs:136, future-hostile/dead-language term `temp` appears
50. `high` `vibe` `crates/bench/src/lib.rs:4`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `bench-harness`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `compat` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:4de04f2cb92a824247103c1a1f4a4a3bd3dc56d7aa1090d0eac74ff9033c5d1a`
   Evidence: crates/bench/src/lib.rs:4, future-hostile/dead-language term `compat` appears
51. `high` `vibe` `crates/bench/src/lib.rs:38`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `bench-harness`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `compat` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:2930699a5380a4b16d9c5fdae8b1161e4c7831d23b0ea3f05979a272071f2be2`
   Evidence: crates/bench/src/lib.rs:38, future-hostile/dead-language term `compat` appears
52. `high` `vibe` `crates/bench/src/lib.rs:39`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `bench-harness`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `compat` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:04ec1a954e37237d5c35688fd68148ea59ce98b74e88e024405f291fd0b536a2`
   Evidence: crates/bench/src/lib.rs:39, future-hostile/dead-language term `compat` appears
53. `high` `vibe` `crates/bench/src/process_metrics.rs:102`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `bench-harness`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `old` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:abdfe593dedff01450bc713224ecdc7f8e232bb41d2aeb498d955413b6179894`
   Evidence: crates/bench/src/process_metrics.rs:102, future-hostile/dead-language term `old` appears
54. `high` `security` `crates/bench/src/process_metrics.rs:106`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `bench-harness`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.zeroed`
   Reason: all-zero validity was not proven
   Fix: construct the type with a valid initializer instead of zeroing it
   Rerun: `just fast`
   Fingerprint: `sha256:c9785dbe973d7ce493c6368656c6814aa0f57ad458c285b47d549df5ae47aca7`
   Evidence: detector=zeroed, proof-window=NearbySafetyComment, snippet=let mut usage: libc::rusage = std::mem::zeroed();
55. `high` `vibe` `crates/bench/src/report.rs:23`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `bench-harness`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `old` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:aa4bdcd3b296a396e8521d973f2c1432fa7ec0d422dff471ffb46ea1f3079313`
   Evidence: crates/bench/src/report.rs:23, future-hostile/dead-language term `old` appears
56. `high` `vibe` `crates/bench/src/report.rs:24`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `bench-harness`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `placeholder` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:b5105b7b3b06761503d554fddbda4fbed29885e176e3de78740117f56ec9bc6e`
   Evidence: crates/bench/src/report.rs:24, future-hostile/dead-language term `placeholder` appears
57. `high` `vibe` `crates/bench/src/workload.rs:608`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `bench-harness`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `compat` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:d715520fb01d2ed65ff18a50cd10fcb2bbcb04e24bc417a55b4ac01040f0c32a`
   Evidence: crates/bench/src/workload.rs:608, future-hostile/dead-language term `compat` appears
58. `high` `exceptions` `crates/domain`
   Rule: `HLT-017-OPAQUE-OBSERVABILITY`
   Check: `HLT-017-OPAQUE-OBSERVABILITY:exceptions` `hard` confidence `0.88`
   Route: TLR `Repair`, lane `observability`, owner `tools`
   Docs: `agent/JANKURAI_STANDARD.md#repair-receipts`
   Reason: no agent-friendly exception/error pattern was detected
   Fix: define a typed exception surface with purpose, reason, common fixes, docs_url, and repair_hint so the next rerun is local
   Rerun: `just score`
   Fingerprint: `sha256:538667a01e35d8e91eae100627364816dd225911862fa2fa1578642af63d4af8`
   Evidence: route repair work to the next agent, opaque failures slow local debugging and reruns, add a typed repair hint; name the common fixes; point at the local docs URL, docs/testing.md
59. `high` `stack` `crates/ffi/include/redlinedb.h`
   Check: `HLT-000-SCORE-DIMENSION:stack` `hard` confidence `0.88`
   Route: TLR `Context/setup`, lane `audit`, owner `c-abi`
   Reason: runtime code uses a language outside the chosen optimal stack
   Fix: move product runtime behavior to Rust core, TypeScript web, SQL migrations, or generated contracts; Python needs a dated advanced-ML/data exception
   Rerun: `just score`
   Fingerprint: `sha256:7789f9e4b1aac10caf5e262e4eea1642b7862f83b21d46c4820f2c1dc6f8da77`
   Evidence: crates/ffi/include/redlinedb.h uses `.h`, Rust core + TypeScript/React/Vite + PostgreSQL + generated contracts + exception-only Python AI/data service
60. `high` `security` `crates/ffi/include/redlinedb.h:124`
   Rule: `HLT-023-INPUT-BOUNDARY-GAP`
   Check: `HLT-023-INPUT-BOUNDARY-GAP:security` `hard` confidence `0.88`
   Route: TLR `Security, secrets, agency`, lane `security`, owner `c-abi`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `shell execution`
   Reason: input handling risk needs deterministic negative tests
   Fix: replace unsafe sinks with typed schemas, parameterized APIs, allowlists, or sandboxed execution plus negative tests
   Rerun: `just security`
   Fingerprint: `sha256:3af2e0fe10b23d7952f3b390713303f0e4bd1abe9eeac34710e611bf7a24e821`
   Evidence: int rldb_exec(rldb *db, const char *sql, rldb_exec_callback callback, void *ctx, char **errmsg);
61. `medium` `context` `crates/ffi/src/backup.rs:1`
   Rule: `HLT-040-REPO-ROT-BAD-BEHAVIOR`
   Check: `HLT-040-REPO-ROT-BAD-BEHAVIOR:context` `soft` confidence `0.88`
   Route: TLR `Context/setup`, lane `audit`, owner `c-abi`
   Docs: `docs/language-bad-behavior.md#web-security-and-repo-rot-detectors`
   Matched term: `repo-rot.path.fake-versioned-source`
   Reason: ambiguous old-looking active source makes agents and reviewers guess whether code is live
   Fix: delete the stale copy, move history to VCS/archive tooling, or document owner, proof lane, expiry, and migration plan
   Rerun: `just score`
   Fingerprint: `sha256:d701d215f5368104681623d315b2ba5772d530b0f13a2552c0c15e6333e22f9e`
   Evidence: detector=repo-rot.path.fake-versioned-source, path=crates/ffi/src/backup.rs, line=1, proof_window=None, snippet=//! Backup API: copy a redline database directory to a destination path.
62. `high` `security` `crates/ffi/src/backup.rs:22`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `c-abi`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:97081d05aaa268626d0f5bd1a7c849d75defa51db4309707c73cc153ac49c268`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=let src_ref = unsafe { &*src };
63. `high` `security` `crates/ffi/src/backup.rs:23`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `c-abi`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:ba30a4064c79b8d559fadbf35397708dc3dc445dd3d4498f3170a1b6aaec956f`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=let dst = unsafe { CStr::from_ptr(dst_path) }
64. `high` `security` `crates/ffi/src/backup.rs:34`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `c-abi`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:9a7efb404d91d896c65028dc46df7c1ade811d52cf4a311bb91f78501c848da0`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=unsafe {
65. `high` `security` `crates/ffi/src/backup.rs:47`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `c-abi`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:cb6932cda5aeb17ea2cf87f2083457f757df53a8f253a15ec1dc0c75f599a154`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=let backup = unsafe { &mut *backup };
66. `high` `security` `crates/ffi/src/backup.rs:74`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `c-abi`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:9a7efb404d91d896c65028dc46df7c1ade811d52cf4a311bb91f78501c848da0`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=unsafe {
67. `high` `security` `crates/ffi/src/backup.rs:75`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `c-abi`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.raw-parts`
   Reason: ownership provenance is missing
   Fix: use the matching constructor/destructor pair or add a documented ownership proof
   Rerun: `just fast`
   Fingerprint: `sha256:c7b2bb7521c8ae17506a939d89ccab06523d7cf3ec82b53b3eff53f230306c46`
   Evidence: detector=Box::from_raw, proof-window=NearbySafetyComment, snippet=drop(Box::from_raw(backup));
68. `high` `security` `crates/ffi/src/backup.rs:85`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `c-abi`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:e1cd8d920130e5062c21c9d389e9a261e9d2c102410a4ef8505602bbdc8f441a`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=unsafe { (*backup).remaining as c_int }
69. `high` `security` `crates/ffi/src/backup.rs:93`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `c-abi`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:80c7e594fc340fc960f1aebf905f937198549f0dafe13d4d24a4d00ade3915fa`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=unsafe { (*backup).pagecount as c_int }
70. `high` `security` `crates/ffi/src/bind.rs:12`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `c-abi`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:07dc7f73918296deafb8050a27995b60a31426923d4d8bbf6114e758e5daeac6`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=let stmt = unsafe { &mut *stmt };
71. `high` `security` `crates/ffi/src/bind.rs:21`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `c-abi`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:07dc7f73918296deafb8050a27995b60a31426923d4d8bbf6114e758e5daeac6`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=let stmt = unsafe { &mut *stmt };
72. `high` `security` `crates/ffi/src/bind.rs:30`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `c-abi`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:07dc7f73918296deafb8050a27995b60a31426923d4d8bbf6114e758e5daeac6`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=let stmt = unsafe { &mut *stmt };
73. `high` `security` `crates/ffi/src/bind.rs:47`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `c-abi`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:07dc7f73918296deafb8050a27995b60a31426923d4d8bbf6114e758e5daeac6`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=let stmt = unsafe { &mut *stmt };
74. `high` `security` `crates/ffi/src/bind.rs:49`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `c-abi`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:464bd0368f1f9ff55ff471455d33ccf7ee1342788111077fc9776306e4443896`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=unsafe { CStr::from_ptr(value) }.to_bytes().to_vec()
75. `high` `security` `crates/ffi/src/bind.rs:51`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `c-abi`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:32451cbc7179012e584e97e9fc27ba1585eeed25f2b547942a088968dbde91a8`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=unsafe { std::slice::from_raw_parts(value as *const u8, nbytes as usize) }.to_vec()
76. `high` `security` `crates/ffi/src/bind.rs:70`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `c-abi`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:07dc7f73918296deafb8050a27995b60a31426923d4d8bbf6114e758e5daeac6`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=let stmt = unsafe { &mut *stmt };
77. `high` `security` `crates/ffi/src/bind.rs:71`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `c-abi`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:642fc78690621f3c8fb48e88d79829131065aa8889c9cd742cb4aebcad108aff`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=let slice = unsafe { std::slice::from_raw_parts(value as *const u8, nbytes as usize) };
78. `high` `security` `crates/ffi/src/bind.rs:82`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `c-abi`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:6c3f2b93620cb9ae345dc0adc864737c158d52814606065356b9b1c9273370dd`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=unsafe { (*stmt).stmt.parameter_count() as c_int }
79. `high` `security` `crates/ffi/src/bind.rs:91`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `c-abi`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:07dc7f73918296deafb8050a27995b60a31426923d4d8bbf6114e758e5daeac6`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=let stmt = unsafe { &mut *stmt };
80. `high` `security` `crates/ffi/src/bind.rs:92`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `c-abi`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:f1a119c75b4af04eefd5733c2bf9461c1ae1e8f75ad72243b7c0178cfbc1c107`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=let name = unsafe { CStr::from_ptr(name) }
81. `high` `security` `crates/ffi/src/column.rs:14`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `c-abi`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:fdbef8f8a89ce4a326c92b8a8a60ca809b8da5a2771f9cc52c225b23b543703d`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=unsafe { (*stmt).stmt.column_count() as c_int }
82. `high` `security` `crates/ffi/src/column.rs:22`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `c-abi`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:bf9f4798cd6b39233163c5e340b378f9dbcc42de97f355ff9ca6bac41e3ccabd`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=unsafe {
83. `high` `security` `crates/ffi/src/column.rs:34`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `c-abi`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:3317a3750875422bfdcdcf98fe123e460ffee613520d966243d2384f57724aa3`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=let stmt = unsafe { &mut *stmt };
84. `high` `security` `crates/ffi/src/column.rs:54`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `c-abi`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:780a1ff9080869e91c0785d28c32344c8f7c998456b16701e27ed365fb127080`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=unsafe { (*stmt).stmt.column_i64(index as usize).unwrap_or(0) }
85. `high` `security` `crates/ffi/src/column.rs:62`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `c-abi`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:f299fd0ed53f0ad10f6280418e7f85ce8b56ca60fc1ff58ebbc61da17c93a74e`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=unsafe { (*stmt).stmt.column_f64(index as usize).unwrap_or(0.0) }
86. `high` `security` `crates/ffi/src/column.rs:70`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `c-abi`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:bf9f4798cd6b39233163c5e340b378f9dbcc42de97f355ff9ca6bac41e3ccabd`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=unsafe {
87. `high` `security` `crates/ffi/src/column.rs:84`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `c-abi`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:bf9f4798cd6b39233163c5e340b378f9dbcc42de97f355ff9ca6bac41e3ccabd`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=unsafe {
88. `high` `security` `crates/ffi/src/column.rs:95`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `c-abi`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:3317a3750875422bfdcdcf98fe123e460ffee613520d966243d2384f57724aa3`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=let stmt = unsafe { &mut *stmt };
89. `high` `security` `crates/ffi/src/config.rs:18`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `c-abi`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:80e4c1a40327f104962dd55f9762ed6179b2230cf88bcc99f2a191485e503771`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=let db = unsafe { &*db };
90. `high` `security` `crates/ffi/src/config.rs:39`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `c-abi`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:80e4c1a40327f104962dd55f9762ed6179b2230cf88bcc99f2a191485e503771`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=let db = unsafe { &*db };
91. `high` `security` `crates/ffi/src/config.rs:48`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `c-abi`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:80e4c1a40327f104962dd55f9762ed6179b2230cf88bcc99f2a191485e503771`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=let db = unsafe { &*db };
92. `high` `security` `crates/ffi/src/config.rs:60`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `c-abi`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:80e4c1a40327f104962dd55f9762ed6179b2230cf88bcc99f2a191485e503771`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=let db = unsafe { &*db };
93. `high` `security` `crates/ffi/src/config.rs:70`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `c-abi`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:f0982448926681ea56b4ea75da82ad5f0425fd23127ce7d8c09109018a628646`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=unsafe {
94. `high` `security` `crates/ffi/src/error.rs:31`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `c-abi`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:c93da4a982dc672a5605ff73cfe6bcc5cbac5445f7e49deffd735f851846a9a2`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=unsafe {
95. `high` `security` `crates/ffi/src/error.rs:32`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `c-abi`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.raw-parts`
   Reason: ownership provenance is missing
   Fix: use the matching constructor/destructor pair or add a documented ownership proof
   Rerun: `just fast`
   Fingerprint: `sha256:fb68a65ce20650b76a90bc88edfe06a198128b0d750b32b32f7c2a0727384404`
   Evidence: detector=CString::from_raw, proof-window=NearbySafetyComment, snippet=drop(CString::from_raw(ptr as *mut c_char));
96. `high` `security` `crates/ffi/src/exec.rs:26`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `c-abi`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:b5fa98c5b94571ddec1a81717bc9b11fdcdc56c3e56033c22e333106861f5621`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=unsafe {
97. `high` `security` `crates/ffi/src/exec.rs:34`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `c-abi`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:eebebf64904234e42f02f766fc5a38b66976f6e87f018f13f682a86d2157e436`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=let db_ref = unsafe { &*db };
98. `high` `security` `crates/ffi/src/exec.rs:35`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `c-abi`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:5ccc7f149d27197cb5b343ec73a918d3db44e32ddc934f08819d70ad87e681bc`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=let sql_text = unsafe { CStr::from_ptr(sql) }
99. `high` `security` `crates/ffi/src/exec.rs:72`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `c-abi`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:be510871a320af20bd9aaf07d434e90403fc45e128e5b3654866ff67e32fefe4`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=unsafe { set_errmsg(errmsg, &msg) };
100. `high` `security` `crates/ffi/src/exec.rs:90`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `c-abi`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:be510871a320af20bd9aaf07d434e90403fc45e128e5b3654866ff67e32fefe4`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=unsafe { set_errmsg(errmsg, &msg) };
101. `high` `security` `crates/ffi/src/exec.rs:112`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `c-abi`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:be510871a320af20bd9aaf07d434e90403fc45e128e5b3654866ff67e32fefe4`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=unsafe { set_errmsg(errmsg, &msg) };
102. `high` `security` `crates/ffi/src/exec.rs:153`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `c-abi`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:d7fc18ba68d899738cf9a0bf71e005b899e677ffe1b30e23604e6c653b5035b1`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=unsafe { set_errmsg(errmsg, "callback returned non-zero") };
103. `high` `vibe` `crates/ffi/src/lib.rs:9`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `c-abi`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `compat` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:ff9c1a85b2ba1e2713c57c09d0c7e3f02fc1e955a3d9d1177685cda1fca30d00`
   Evidence: crates/ffi/src/lib.rs:9, future-hostile/dead-language term `compat` appears
104. `high` `vibe` `crates/ffi/src/lib.rs:10`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `c-abi`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `shim` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:d0bd49577f0f8aaba907bab9ad49446433c8cbfe972ac7aee04b011ecca7db9b`
   Evidence: crates/ffi/src/lib.rs:10, future-hostile/dead-language term `shim` appears
105. `high` `vibe` `crates/ffi/src/lib.rs:16`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `c-abi`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `legacy` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:fdc57cccef9b91811e958fe417a651926eb47aebdc758d8a461d5f1cd6232032`
   Evidence: crates/ffi/src/lib.rs:16, future-hostile/dead-language term `legacy` appears
106. `high` `security` `crates/ffi/src/lifecycle.rs:16`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `c-abi`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:f560f05eaaeb2bf2deb56e19dcd58870abedcf8049e745448e96b206b98f4c2b`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=let handle = open_handle(unsafe { CStr::from_ptr(path) }, None, true)?;
107. `high` `security` `crates/ffi/src/lifecycle.rs:17`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `c-abi`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:0c336bebb472adeb8cd62352f231caf2314357706b37a0c936a91483d23d771a`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=unsafe {
108. `high` `security` `crates/ffi/src/lifecycle.rs:37`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `c-abi`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:d34135ddb5e02f0ef323df479e0e3f4a8eb84904cb00fdcf455313596d0eca7c`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=Some(unsafe { &*config })
109. `high` `security` `crates/ffi/src/lifecycle.rs:39`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `c-abi`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:f6a1d9951d9949502cee1462af2ac9c8864b2d007a207bea2f6bb36cb95c352d`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=let handle = open_handle(unsafe { CStr::from_ptr(path) }, config, true)?;
110. `high` `security` `crates/ffi/src/lifecycle.rs:40`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `c-abi`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:0c336bebb472adeb8cd62352f231caf2314357706b37a0c936a91483d23d771a`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=unsafe {
111. `high` `security` `crates/ffi/src/lifecycle.rs:50`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `c-abi`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:024945520dede985cb8b0ea84a8e0b96d1b9f21cf93e7634dcbf311c7b29b763`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=let db_ref = unsafe { &*db };
112. `high` `security` `crates/ffi/src/lifecycle.rs:54`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `c-abi`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:0c336bebb472adeb8cd62352f231caf2314357706b37a0c936a91483d23d771a`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=unsafe {
113. `high` `security` `crates/ffi/src/lifecycle.rs:55`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `c-abi`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.raw-parts`
   Reason: ownership provenance is missing
   Fix: use the matching constructor/destructor pair or add a documented ownership proof
   Rerun: `just fast`
   Fingerprint: `sha256:bdbc8665d06771630a270b7628b5ce1d4ea68b943861e02146cd65daab567c1f`
   Evidence: detector=Box::from_raw, proof-window=NearbySafetyComment, snippet=drop(Box::from_raw(db));
114. `high` `security` `crates/ffi/src/sqlite3_compat.rs:88`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `c-abi`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:c769570cfa208ecc8045879de56ad144a3dbdd39eb2f2ce4938e2c6cada396a1`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=let handle = open_handle(unsafe { CStr::from_ptr(path) }, None, create_if_missing)?;
115. `high` `security` `crates/ffi/src/sqlite3_compat.rs:89`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `c-abi`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:9be46d182d3069de2b5a8c506c8e72e08702c4906c38589104b6d7c413bc7a6b`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=unsafe {
116. `high` `security` `crates/ffi/src/sqlite3_compat.rs:114`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `c-abi`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:30e37337d6316857bce2c00f0da9a890b1fa502f6cddb3a938843bcb5fbec976`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=unsafe { (*stmt).stmt.is_readonly() as c_int }
117. `high` `security` `crates/ffi/src/sqlite3_compat.rs:122`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `c-abi`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:f75ead57ab1f7617af074fcda8d5636a724406f01db245441c8163271e184e53`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=unsafe { (*stmt).stmt.is_busy() as c_int }
118. `high` `security` `crates/ffi/src/sqlite3_compat.rs:130`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `c-abi`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:3f44e525f8fb3156d8c08be5e041adc599a8a16553ba6c9de92687901cbf79b1`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=unsafe { (*stmt).sql_text.as_ptr() }
119. `high` `vibe` `crates/ffi/src/sqlite3_compat.rs:154`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `c-abi`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `stub` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:2f42377cf6c45b13e22b7d529883ec0cd03f3f39934a83011fdeedb9499887d7`
   Evidence: crates/ffi/src/sqlite3_compat.rs:154, future-hostile/dead-language term `stub` appears
120. `high` `vibe` `crates/ffi/src/sqlite3_compat.rs:154`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `c-abi`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: product code contains TODO/stub/unimplemented/unreachable placeholder markers
   Fix: replace placeholders with implemented behavior, typed unsupported-state errors, or a tracked exception record with docs
   Rerun: `just fast`
   Fingerprint: `sha256:ea935d50c0f2f0f6717fd2281c8b42b72dabf158f321d59f4ff479c2a8db71d2`
   Evidence: crates/ffi/src/sqlite3_compat.rs:154 // overwriting it with the generic "ok"/"error" stub would lose the
121. `high` `security` `crates/ffi/src/sqlite3_compat.rs:160`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `c-abi`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:9be46d182d3069de2b5a8c506c8e72e08702c4906c38589104b6d7c413bc7a6b`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=unsafe {
122. `high` `security` `crates/ffi/src/sqlite3_compat.rs:171`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `c-abi`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:c21e4382836619c6210d5891fea78711e99a50586bffb2bb7e49e31f6f6a7c32`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=let db = unsafe { (*stmt).db };
123. `high` `security` `crates/ffi/src/sqlite3_compat.rs:177`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `c-abi`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:9be46d182d3069de2b5a8c506c8e72e08702c4906c38589104b6d7c413bc7a6b`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=unsafe {
124. `high` `security` `crates/ffi/src/sqlite3_compat.rs:189`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `c-abi`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:c21e4382836619c6210d5891fea78711e99a50586bffb2bb7e49e31f6f6a7c32`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=let db = unsafe { (*stmt).db };
125. `high` `security` `crates/ffi/src/sqlite3_compat.rs:204`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `c-abi`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:c21e4382836619c6210d5891fea78711e99a50586bffb2bb7e49e31f6f6a7c32`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=let db = unsafe { (*stmt).db };
126. `high` `security` `crates/ffi/src/sqlite3_compat.rs:214`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `c-abi`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:c21e4382836619c6210d5891fea78711e99a50586bffb2bb7e49e31f6f6a7c32`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=let db = unsafe { (*stmt).db };
127. `high` `security` `crates/ffi/src/sqlite3_compat.rs:224`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `c-abi`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:c21e4382836619c6210d5891fea78711e99a50586bffb2bb7e49e31f6f6a7c32`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=let db = unsafe { (*stmt).db };
128. `high` `security` `crates/ffi/src/sqlite3_compat.rs:234`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `c-abi`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:c21e4382836619c6210d5891fea78711e99a50586bffb2bb7e49e31f6f6a7c32`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=let db = unsafe { (*stmt).db };
129. `high` `security` `crates/ffi/src/sqlite3_compat.rs:250`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `c-abi`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:c21e4382836619c6210d5891fea78711e99a50586bffb2bb7e49e31f6f6a7c32`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=let db = unsafe { (*stmt).db };
130. `high` `security` `crates/ffi/src/sqlite3_compat.rs:266`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `c-abi`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:c21e4382836619c6210d5891fea78711e99a50586bffb2bb7e49e31f6f6a7c32`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=let db = unsafe { (*stmt).db };
131. `high` `security` `crates/ffi/src/sqlite3_compat.rs:343`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `c-abi`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:9be46d182d3069de2b5a8c506c8e72e08702c4906c38589104b6d7c413bc7a6b`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=unsafe {
132. `high` `security` `crates/ffi/src/sqlite3_compat.rs:420`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `c-abi`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:4a6294dd0378f9234ea85ceba1af8e0fe00cb2e1d0b45573e12e13f2c9afec7c`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=unsafe { (*stmt).db }
133. `high` `security` `crates/ffi/src/sqlite3_compat.rs:428`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `c-abi`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:9be46d182d3069de2b5a8c506c8e72e08702c4906c38589104b6d7c413bc7a6b`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=unsafe {
134. `high` `security` `crates/ffi/src/stmt.rs:27`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `c-abi`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:eff84d7f9c974413001f8027ffcf76b4e1bbbf6d1e5dc92eaeef88327eac273b`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=let db_ref = unsafe { &*db };
135. `high` `security` `crates/ffi/src/stmt.rs:28`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `c-abi`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:3c49343c9cb9c4c022ab2f422cbba3cc52c5652d874e47d5741d528cb28dabf1`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=let sql_cstr = unsafe { CStr::from_ptr(sql) };
136. `high` `security` `crates/ffi/src/stmt.rs:32`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `c-abi`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:5e99e46699cc859c8bae8b1d9a7164f675f1316d7625731522f7d3506a16a454`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=let bytes = unsafe {
137. `high` `security` `crates/ffi/src/stmt.rs:33`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `c-abi`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.raw-parts`
   Reason: ownership provenance is missing
   Fix: use the matching constructor/destructor pair or add a documented ownership proof
   Rerun: `just fast`
   Fingerprint: `sha256:ad73071eae3b23a5163b99b7f96873c7bc464c4738a0fda62fa55b62bb81fcaa`
   Evidence: detector=from_raw_parts, proof-window=NearbySafetyComment, snippet=std::slice::from_raw_parts(sql_cstr.as_ptr() as *const u8, nbytes as usize)
138. `high` `security` `crates/ffi/src/stmt.rs:54`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `c-abi`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:c83fd2709e2acaff2cd37e5df263ea19dc4ce3d5ac86b987b8c64f025094c06f`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=unsafe {
139. `high` `security` `crates/ffi/src/stmt.rs:60`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `c-abi`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:c83fd2709e2acaff2cd37e5df263ea19dc4ce3d5ac86b987b8c64f025094c06f`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=unsafe {
140. `high` `security` `crates/ffi/src/stmt.rs:85`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `c-abi`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:c83fd2709e2acaff2cd37e5df263ea19dc4ce3d5ac86b987b8c64f025094c06f`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=unsafe {
141. `high` `security` `crates/ffi/src/stmt.rs:95`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `c-abi`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:58e81080a7a41302e367c6b647726f7e62c16bba69d2f9e05835515faf42dc7d`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=let stmt_ref = unsafe { &mut *stmt };
142. `high` `security` `crates/ffi/src/stmt.rs:97`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `c-abi`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:99b39adcb5ba4f6fe2e318d893288ef98a3bf24c62e3bdb9b76f7944e3bb9ed2`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=if unsafe { (*db).interrupted.load(Ordering::Relaxed) } {
143. `high` `security` `crates/ffi/src/stmt.rs:119`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `c-abi`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:7b2df65236d2434b54a9cb7d1437b1360795f48592dbfc0cd96a554028d2bac7`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=let stmt = unsafe { &mut *stmt };
144. `high` `security` `crates/ffi/src/stmt.rs:132`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `c-abi`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:3324a25d518de85e90bbaf3757c8d3becd1bdbe6037bacdf7d38a4f43c914dce`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=let boxed = unsafe { Box::from_raw(stmt) };
145. `high` `security` `crates/ffi/src/stmt.rs:133`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `c-abi`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:c83fd2709e2acaff2cd37e5df263ea19dc4ce3d5ac86b987b8c64f025094c06f`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=unsafe {
146. `high` `security` `crates/ffi/src/stmt.rs:145`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `c-abi`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:7b2df65236d2434b54a9cb7d1437b1360795f48592dbfc0cd96a554028d2bac7`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=let stmt = unsafe { &mut *stmt };
147. `high` `vibe` `crates/ffi/src/types.rs:89`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `c-abi`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `compat` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:c709ec52dc4ffb4a38bae1b1447fd37b08dc4bfefc0c82d6a7fe206953b95d76`
   Evidence: crates/ffi/src/types.rs:89, future-hostile/dead-language term `compat` appears
148. `high` `security` `crates/ffi/src/util.rs:119`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `c-abi`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:27b07baa9ab9c5391c72380e5ce9457cc357a99260a2154b29ca8b0385c23edc`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=Ok(f(unsafe { &*db }))
149. `high` `vibe` `crates/ffi/src/util.rs:258`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `c-abi`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `stub` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:65fe8e26e8b9387410bc6d3e5396660ccfb98cc29051e9ebca31467e490517ba`
   Evidence: crates/ffi/src/util.rs:258, future-hostile/dead-language term `stub` appears
150. `high` `security` `crates/ffi/src/util.rs:285`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `c-abi`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.public-fn-missing-safety-doc`
   Reason: missing `# Safety` docs above the public unsafe item
   Fix: document caller obligations with a `# Safety` section
   Rerun: `just fast`
   Fingerprint: `sha256:8bb819c706a5cc56d2f7a1ff3e77b886bd321dcebaa2bc14a0cc629100890022`
   Evidence: detector=pub unsafe fn, proof-window=NearbySafetyDocs, snippet=pub(crate) unsafe fn set_errmsg(errmsg: *mut *mut c_char, msg: &str) {
151. `high` `security` `crates/ffi/src/util.rs:289`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `c-abi`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:ddf1ee1c568ee92999686341159723c9bd9599833f8e9b7f32e9ca098c6d43e5`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=unsafe {
152. `high` `vibe` `crates/kernel/src/catalog/store.rs:53`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `storage-and-catalog`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `temp` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:842ca0bc7e2dda92a81caff2ef1e8f385ddcf3efbe2afa1867475fa1f66f6970`
   Evidence: crates/kernel/src/catalog/store.rs:53, future-hostile/dead-language term `temp` appears
153. `high` `vibe` `crates/kernel/src/catalog/store.rs:54`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `storage-and-catalog`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `old` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:eca51531363793713ecc5baddffd1b9d85e0e73a627f56f9ef2e76c2e217725b`
   Evidence: crates/kernel/src/catalog/store.rs:54, future-hostile/dead-language term `old` appears
154. `high` `vibe` `crates/kernel/src/catalog/store.rs:59`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `storage-and-catalog`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `temp` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:c1c06fb2962e3f3b2e67ca46c5402cf6b6e3ac4259fcc14ddfbb25576a0ee74d`
   Evidence: crates/kernel/src/catalog/store.rs:59, future-hostile/dead-language term `temp` appears
155. `high` `vibe` `crates/kernel/src/catalog/store.rs:60`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `storage-and-catalog`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `temp` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:cf921c872c38372ede4bda3e1b048d703bc11c34c905cfabb4662a303720223c`
   Evidence: crates/kernel/src/catalog/store.rs:60, future-hostile/dead-language term `temp` appears
156. `high` `vibe` `crates/kernel/src/catalog/store.rs:65`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `storage-and-catalog`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `temp` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:275965b1b592e1c47dcddcf0f878496aa2153f9112588f1e4ae52e8363b9c59c`
   Evidence: crates/kernel/src/catalog/store.rs:65, future-hostile/dead-language term `temp` appears
157. `high` `vibe` `crates/kernel/src/catalog/store.rs:485`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `storage-and-catalog`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `compat` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:70bd7425f8902e16e59ecf9860bfb1f4388b506a33c2352665d7ae948b0d9c80`
   Evidence: crates/kernel/src/catalog/store.rs:485, future-hostile/dead-language term `compat` appears
158. `high` `vibe` `crates/kernel/src/engine/catalog_ops.rs:153`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `storage-and-catalog`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `todo` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:569c91caaa93ac8667b556ee4084e2dc23c7ae95649ae0dce4449ce688217429`
   Evidence: crates/kernel/src/engine/catalog_ops.rs:153, future-hostile/dead-language term `todo` appears
159. `high` `vibe` `crates/kernel/src/engine/catalog_ops.rs:187`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `storage-and-catalog`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `legacy` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:8699b00a2f715984fbb14d0d234ce8c56efb537d775dd88a4c952817607fbdc2`
   Evidence: crates/kernel/src/engine/catalog_ops.rs:187, future-hostile/dead-language term `legacy` appears
160. `high` `vibe` `crates/kernel/src/index/cursor.rs:6`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `storage-and-catalog`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `legacy` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:95f305cbb548b5628c62aa640635a769c097251cc97be40895ff6b719920a3ce`
   Evidence: crates/kernel/src/index/cursor.rs:6, future-hostile/dead-language term `legacy` appears
161. `high` `vibe` `crates/kernel/src/index/cursor.rs:21`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `storage-and-catalog`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `legacy` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:07b70b673edaa350d163eb8c64e28ab558ec21a0f71ef1db1d7137af1aee882f`
   Evidence: crates/kernel/src/index/cursor.rs:21, future-hostile/dead-language term `legacy` appears
162. `high` `vibe` `crates/kernel/src/index/cursor.rs:25`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `storage-and-catalog`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `legacy` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:cd66d28fcfaabd6354179c1cc9aa7c7c248ecdc51da5ac49aa10334c9dce72b3`
   Evidence: crates/kernel/src/index/cursor.rs:25, future-hostile/dead-language term `legacy` appears
163. `high` `vibe` `crates/kernel/src/index/cursor.rs:36`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `storage-and-catalog`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `legacy` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:5524a16e95f3356527f874cfa2d1da00eeb2c9686988c4f69e8ff29884009bef`
   Evidence: crates/kernel/src/index/cursor.rs:36, future-hostile/dead-language term `legacy` appears
164. `high` `vibe` `crates/kernel/src/index/cursor.rs:40`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `storage-and-catalog`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `legacy` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:f36766b758e03e00a4348bf331c74efaae09af15152e47af30b3e5791e99d55e`
   Evidence: crates/kernel/src/index/cursor.rs:40, future-hostile/dead-language term `legacy` appears
165. `high` `vibe` `crates/kernel/src/index/cursor.rs:45`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `storage-and-catalog`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `legacy` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:c3b59e4c3bca4f66bb0d18c2de10b8b15fbaa8585ee1cdc8ebc8c5cdce80e346`
   Evidence: crates/kernel/src/index/cursor.rs:45, future-hostile/dead-language term `legacy` appears
166. `high` `vibe` `crates/kernel/src/index/cursor.rs:163`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `storage-and-catalog`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `legacy` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:b2604311d273579bae4c1e79c68f16918eac182830cd15fe0466900ee8e385e7`
   Evidence: crates/kernel/src/index/cursor.rs:163, future-hostile/dead-language term `legacy` appears
167. `high` `vibe` `crates/kernel/src/index/cursor.rs:293`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `storage-and-catalog`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `legacy` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:2e904a4cb10b2d0ad0e677a19d09bce5a1c8a11ed5281c54dbe95f120d7e7061`
   Evidence: crates/kernel/src/index/cursor.rs:293, future-hostile/dead-language term `legacy` appears
168. `high` `vibe` `crates/kernel/src/index/cursor.rs:448`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `storage-and-catalog`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `legacy` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:f1a8b9cccb98feefe4cf7037b4d564644d6589cfdc18cf70f423c45abbe8dc87`
   Evidence: crates/kernel/src/index/cursor.rs:448, future-hostile/dead-language term `legacy` appears
169. `high` `vibe` `crates/kernel/src/index/cursor.rs:468`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `storage-and-catalog`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `legacy` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:a9af9e36035a9cb7736df37b41ffc008fe481a9ac62bd784302fd05dffa50d62`
   Evidence: crates/kernel/src/index/cursor.rs:468, future-hostile/dead-language term `legacy` appears
170. `high` `vibe` `crates/kernel/src/index/cursor.rs:534`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `storage-and-catalog`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `legacy` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:97a65b07af1b3126b7f8de1a872eb611987234af566cb4ac12241efae27ccc5a`
   Evidence: crates/kernel/src/index/cursor.rs:534, future-hostile/dead-language term `legacy` appears
171. `high` `vibe` `crates/kernel/src/index/cursor.rs:535`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `storage-and-catalog`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `legacy` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:1279b5c89020ae706f51a8291bc9dff34d79c2de3231849af0fa3d87173302e2`
   Evidence: crates/kernel/src/index/cursor.rs:535, future-hostile/dead-language term `legacy` appears
172. `high` `vibe` `crates/kernel/src/index/cursor/raw.rs:18`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `storage-and-catalog`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `legacy` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:ee7382d3d4b181cb766efa45f23d2dff50d0b9a5e04c2775ae55f0548bb318a7`
   Evidence: crates/kernel/src/index/cursor/raw.rs:18, future-hostile/dead-language term `legacy` appears
173. `high` `vibe` `crates/kernel/src/index/scan.rs:10`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `storage-and-catalog`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `legacy` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:d0f72fb862a1826b00b0ccff2c50fe02c6772165e1e0764fbeb91ffde81daf51`
   Evidence: crates/kernel/src/index/scan.rs:10, future-hostile/dead-language term `legacy` appears
174. `high` `vibe` `crates/kernel/src/index/scan.rs:43`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `storage-and-catalog`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `legacy` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:684dbe46dc84876b2ede3a5ecaf48826a4f66179c2819c226ecc5953b3b47441`
   Evidence: crates/kernel/src/index/scan.rs:43, future-hostile/dead-language term `legacy` appears
175. `high` `vibe` `crates/kernel/src/index/scan.rs:68`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `storage-and-catalog`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `legacy` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:17fd0b54e3155a058c3be83ab59094e09940acd3ab1ef1cfe14f300ba2ae71e6`
   Evidence: crates/kernel/src/index/scan.rs:68, future-hostile/dead-language term `legacy` appears
176. `high` `vibe` `crates/kernel/src/integrity/equivalence.rs:77`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `phase10-integrity-checker`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `fallback` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:2d6b23153c7ed1bdf6f078d200b1d432a6d290d154d5df134446cacd6936ffc4`
   Evidence: crates/kernel/src/integrity/equivalence.rs:77, future-hostile/dead-language term `fallback` appears
177. `high` `vibe` `crates/kernel/src/integrity/equivalence.rs:104`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `phase10-integrity-checker`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `fallback` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:be5765e175f380cc334537171a9dd359bb2cdedd0394763c9c76619a59e47da4`
   Evidence: crates/kernel/src/integrity/equivalence.rs:104, future-hostile/dead-language term `fallback` appears
178. `high` `vibe` `crates/kernel/src/integrity/mod.rs:3`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `phase10-integrity-checker`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `stub` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:e8b2fbecaaec7557600619e92469bd12d4f6740584651d752b1f4be2bd0269e2`
   Evidence: crates/kernel/src/integrity/mod.rs:3, future-hostile/dead-language term `stub` appears
179. `high` `vibe` `crates/kernel/src/integrity/page_csum.rs:17`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `phase10-integrity-checker`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `stale` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:f909a63d259a65cc8fe9beed8727e14686fbe07a78a51496ba95bfbb909330a0`
   Evidence: crates/kernel/src/integrity/page_csum.rs:17, future-hostile/dead-language term `stale` appears
180. `high` `vibe` `crates/kernel/src/integrity/page_csum.rs:80`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `phase10-integrity-checker`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `placeholder` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:e86aa65cda718e342bce02e8a3300a6b686f898b8b6e3a1cdb8bd237c5995a70`
   Evidence: crates/kernel/src/integrity/page_csum.rs:80, future-hostile/dead-language term `placeholder` appears
181. `high` `vibe` `crates/kernel/src/json/simd_key.rs:12`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `phase10-jsonb-binary-format`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `fallback` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:40bf3a1eb022afefc7a7ec73734e53daff522aba64909dd49b7b012eb5845e6f`
   Evidence: crates/kernel/src/json/simd_key.rs:12, future-hostile/dead-language term `fallback` appears
182. `high` `vibe` `crates/kernel/src/json/simd_key.rs:61`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `phase10-jsonb-binary-format`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `fallback` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:79ae42ccd2d1af4ae21805d5b5590955cd6e4f56496a3a576b78a6709db7ed67`
   Evidence: crates/kernel/src/json/simd_key.rs:61, future-hostile/dead-language term `fallback` appears
183. `high` `vibe` `crates/kernel/src/vector/diskann/mod.rs:23`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `phase10-diskann-index`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `todo` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:a97abc5cb119920a45cea705955aa6c77ecdf3e0c709b79d3466355a9d3cf364`
   Evidence: crates/kernel/src/vector/diskann/mod.rs:23, future-hostile/dead-language term `todo` appears
184. `high` `vibe` `crates/kernel/src/vector/diskann/mod.rs:28`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `phase10-diskann-index`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `fallback` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:461b504fdc9928fec0fd19f7a46d6551a8c91335d99312780f600d5634dd5d28`
   Evidence: crates/kernel/src/vector/diskann/mod.rs:28, future-hostile/dead-language term `fallback` appears
185. `high` `vibe` `crates/kernel/src/vector/diskann/mod.rs:29`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `phase10-diskann-index`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `fallback` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:de1cf9a20a73039d5932569f69a60a064e23d607b99cbf2de337192d18bdd36a`
   Evidence: crates/kernel/src/vector/diskann/mod.rs:29, future-hostile/dead-language term `fallback` appears
186. `high` `vibe` `crates/kernel/src/vector/diskann/mod.rs:233`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `phase10-diskann-index`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `todo` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:be6cede2f120feac1cef57d25873978941f9bb01b8d4306367b6d374bb970700`
   Evidence: crates/kernel/src/vector/diskann/mod.rs:233, future-hostile/dead-language term `todo` appears
187. `high` `vibe` `crates/kernel/src/vector/diskann/mod.rs:358`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `phase10-diskann-index`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `fallback` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:3b7176e09b125c15ecb452ae65078b9ece7a4ecbbe0c4f663cf7754b0ac6abf1`
   Evidence: crates/kernel/src/vector/diskann/mod.rs:358, future-hostile/dead-language term `fallback` appears
188. `high` `vibe` `crates/kernel/src/vector/diskann/mod.rs:361`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `phase10-diskann-index`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `todo` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:baac9d4443bd09662d11d85bf824fcd059423d6ff9b429e32d0823690be4d3a4`
   Evidence: crates/kernel/src/vector/diskann/mod.rs:361, future-hostile/dead-language term `todo` appears
189. `high` `vibe` `crates/kernel/src/vector/diskann/sectors.rs:39`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `phase10-diskann-index`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `unused` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:31e68ebd208f53d5f235b67aa01003b27ada33bb839388d88774dd2bfe59c1d6`
   Evidence: crates/kernel/src/vector/diskann/sectors.rs:39, future-hostile/dead-language term `unused` appears
190. `high` `security` `crates/kernel/src/vector/diskann/sectors.rs:244`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `phase10-diskann-index`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.static-mut`
   Reason: global mutation proof is missing
   Fix: replace the mutable static with explicit synchronization or scoped ownership
   Rerun: `just fast`
   Fingerprint: `sha256:4ad715b1adb98962b839ecdd8f5951fa4c9504494fe46fdfcc242cad5a81760d`
   Evidence: detector=static mut, proof-window=NearbySafetyComment, snippet=// copy via a static mutable trick is unsafe. Use a helper that builds a
191. `high` `security` `crates/kernel/src/vector/diskann/sectors.rs:248`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `phase10-diskann-index`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:92e065eda38b2e9ea7e3fd662514480ec66a9cbd109de74c0c04c5f7ca752004`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=let (head, mid, tail) = unsafe { vector_bytes.align_to::<f32>() };
192. `high` `vibe` `crates/kernel/src/vector/flat.rs:13`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `phase10-vector-flat-and-simd`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `fallback` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:4e96da1d757c8a89c8f82111a1917bd5b5f58cfa5bbcc739c238fdc58e1f316a`
   Evidence: crates/kernel/src/vector/flat.rs:13, future-hostile/dead-language term `fallback` appears
193. `high` `vibe` `crates/kernel/src/vector/hnsw/builder.rs:260`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `phase10-hnsw-index`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `fallback` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:83ef01851ede4e5c5a204a80bbb8fba292b26a31d2feafc522c89f3082b84d54`
   Evidence: crates/kernel/src/vector/hnsw/builder.rs:260, future-hostile/dead-language term `fallback` appears
194. `high` `vibe` `crates/kernel/src/vector/hnsw/mod.rs:647`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `phase10-hnsw-index`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `old` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:74d6c47a7dc8ad38a437a851a4407a0303a92ef8803b059480f317af8686bd26`
   Evidence: crates/kernel/src/vector/hnsw/mod.rs:647, future-hostile/dead-language term `old` appears
195. `high` `vibe` `crates/kernel/src/vector/hnsw/mod.rs:671`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `phase10-hnsw-index`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `unused` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:c6c9a4487de737c8d8f2edd44d14c0ac95996cd5bbc22907f14405b78b78baa7`
   Evidence: crates/kernel/src/vector/hnsw/mod.rs:671, future-hostile/dead-language term `unused` appears
196. `high` `vibe` `crates/kernel/src/vector/hnsw/searcher.rs:30`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `phase10-hnsw-index`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `fallback` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:7e9cf7a0f732fb68eddbfc468440988e84a9f85a2a5f590edd83898b4691293c`
   Evidence: crates/kernel/src/vector/hnsw/searcher.rs:30, future-hostile/dead-language term `fallback` appears
197. `high` `vibe` `crates/kernel/src/vector/simd.rs:8`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `phase10-vector-flat-and-simd`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `fallback` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:f2d1af2e0be874b1d31ef706c1b336a3c8641e65b43422445c1908dbfef1f871`
   Evidence: crates/kernel/src/vector/simd.rs:8, future-hostile/dead-language term `fallback` appears
198. `high` `security` `crates/kernel/src/vector/simd.rs:25`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `phase10-vector-flat-and-simd`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:481de3ae5700b40721479f386d18eec7ee5084ce29f31829ac8064fc14bcb9f2`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=//! Rust 2024 edition still demands per-call `unsafe { … }` blocks for any
199. `high` `vibe` `crates/kernel/src/wal/manager.rs:40`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `wal-archive-retention`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `stub` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:31da672d371794badbecd162a509644b51125f8e5d4f043d086d8d9dc7d516d7`
   Evidence: crates/kernel/src/wal/manager.rs:40, future-hostile/dead-language term `stub` appears
200. `high` `vibe` `crates/kernel/src/wal/manager.rs:252`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `wal-archive-retention`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `fallback` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:5a0fed65590837e45fb399bb1e4a5b3b0aac03fdd06bc044fbd19bef214d13a6`
   Evidence: crates/kernel/src/wal/manager.rs:252, future-hostile/dead-language term `fallback` appears
201. `high` `vibe` `crates/kernel/src/wal/manager/storage.rs:32`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `wal-archive-retention`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `stale` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:e67ec2a492801a0aa97834aada798ce1bcbcb8d5360e4c52f6f78c5e43564890`
   Evidence: crates/kernel/src/wal/manager/storage.rs:32, future-hostile/dead-language term `stale` appears
202. `medium` `context` `crates/redlinedb/src/backup.rs:1`
   Rule: `HLT-040-REPO-ROT-BAD-BEHAVIOR`
   Check: `HLT-040-REPO-ROT-BAD-BEHAVIOR:context` `soft` confidence `0.88`
   Route: TLR `Context/setup`, lane `audit`, owner `rust-public-api`
   Docs: `docs/language-bad-behavior.md#web-security-and-repo-rot-detectors`
   Matched term: `repo-rot.path.fake-versioned-source`
   Reason: ambiguous old-looking active source makes agents and reviewers guess whether code is live
   Fix: delete the stale copy, move history to VCS/archive tooling, or document owner, proof lane, expiry, and migration plan
   Rerun: `just score`
   Fingerprint: `sha256:a154920484e6aa526643797bd392c9d762e9f516bfb0a0955596cbf11ee0a125`
   Evidence: detector=repo-rot.path.fake-versioned-source, path=crates/redlinedb/src/backup.rs, line=1, proof_window=None, snippet=use std::fs;
203. `high` `vibe` `crates/redlinedb/src/lib.rs:136`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `rust-public-api`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `temp` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:da331db828872a250e1bf54662cbfca67f5736129fcc3f024c7ed0f7dda22114`
   Evidence: crates/redlinedb/src/lib.rs:136, future-hostile/dead-language term `temp` appears
204. `high` `vibe` `crates/redlinedb/src/lib.rs:153`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `rust-public-api`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `temp` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:c66e880ca27dc89cd211fcbd1367f2e8eda5f993587f838adf4605746e4b00d8`
   Evidence: crates/redlinedb/src/lib.rs:153, future-hostile/dead-language term `temp` appears
205. `high` `vibe` `crates/redlinedb/src/machine.rs:615`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `rust-public-api`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `stale` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:5380291b488b97f9623460f97bcea282ce36b7c7d54ad65509a9ac196bbf5251`
   Evidence: crates/redlinedb/src/machine.rs:615, future-hostile/dead-language term `stale` appears
206. `high` `security` `crates/redlinedb/src/registry.rs:86`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `rust-public-api`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.static-mut`
   Reason: global mutation proof is missing
   Fix: replace the mutable static with explicit synchronization or scoped ownership
   Rerun: `just fast`
   Fingerprint: `sha256:6a7c8f4b82a3842611d0e98cdbae334790b29a10627a42ede26a5e2727b8f284`
   Evidence: detector=static mut, proof-window=NearbySafetyComment, snippet=fn registry() -> &'static Mutex<Registry> {
207. `high` `security` `crates/redlinedb/src/registry.rs:258`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `rust-public-api`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:721c6e734c00a5655cb8d71a49a0015f5ca5bfc951fc11917ff37b68bfbb1ed3`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=let rc = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
208. `high` `vibe` `crates/sql/src/connection.rs:886`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `sql-parser-planner-executor`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `temp` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:733787fcbd845e33ab8e8bafb4f15647dd6b33a1bbeaa935c0ddaabf8fdef9cc`
   Evidence: crates/sql/src/connection.rs:886, future-hostile/dead-language term `temp` appears
209. `high` `vibe` `crates/sql/src/exec/expr/mod.rs:107`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `sql-parser-planner-executor`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `placeholder` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:c97d98613b7ee1f42d615711847831ce0acf4fc6462d31e80f02aecb445d14e7`
   Evidence: crates/sql/src/exec/expr/mod.rs:107, future-hostile/dead-language term `placeholder` appears
210. `high` `vibe` `crates/sql/src/exec/expr/scalar.rs:37`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `sql-parser-planner-executor`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `legacy` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:2e98843ff7e322700ee56c1d4296e74eb20d2bc45edbc7c9838224622883abae`
   Evidence: crates/sql/src/exec/expr/scalar.rs:37, future-hostile/dead-language term `legacy` appears
211. `high` `vibe` `crates/sql/src/exec/expr/scalar.rs:38`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `sql-parser-planner-executor`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `unused` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:2c86b677b2614dab15422a79d2321fa184fe2f3686c589e336172afc4b75109b`
   Evidence: crates/sql/src/exec/expr/scalar.rs:38, future-hostile/dead-language term `unused` appears
212. `high` `vibe` `crates/sql/src/exec/index_access.rs:44`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `sql-parser-planner-executor`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `legacy` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:082486d01f8fa2fce25c8748071b40ec9be765b9675099f16941f844bb0cacac`
   Evidence: crates/sql/src/exec/index_access.rs:44, future-hostile/dead-language term `legacy` appears
213. `high` `vibe` `crates/sql/src/exec/index_access.rs:241`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `sql-parser-planner-executor`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `stale` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:57bb3aacb0be0521c45ce116101797f2d69a743f56bb9841d1bdda620e8899a5`
   Evidence: crates/sql/src/exec/index_access.rs:241, future-hostile/dead-language term `stale` appears
214. `high` `vibe` `crates/sql/src/exec/index_access.rs:740`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `sql-parser-planner-executor`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `placeholder` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:6646ec0360ad31799c1a5197498142ce7cc3e3e5a845a440a07cad962c4a29f0`
   Evidence: crates/sql/src/exec/index_access.rs:740, future-hostile/dead-language term `placeholder` appears
215. `high` `vibe` `crates/sql/src/exec/index_dml.rs:14`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `sql-parser-planner-executor`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `legacy` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:287cce994e12cb772e4c8191b949dbfbada4fd74d2f04023f5e184d7f8cf3f0f`
   Evidence: crates/sql/src/exec/index_dml.rs:14, future-hostile/dead-language term `legacy` appears
216. `high` `vibe` `crates/sql/src/exec/index_dml.rs:64`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `sql-parser-planner-executor`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `legacy` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:03e7a0a0dffcb990f33547ba90e91340cc1bb3696f29f7bb7025558ec1193218`
   Evidence: crates/sql/src/exec/index_dml.rs:64, future-hostile/dead-language term `legacy` appears
217. `high` `vibe` `crates/sql/src/exec/index_dml.rs:164`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `sql-parser-planner-executor`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `old` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:361658907a3609be02ed57834f7fa41731ff24c506b734fe8d84123de7de7f95`
   Evidence: crates/sql/src/exec/index_dml.rs:164, future-hostile/dead-language term `old` appears
218. `high` `vibe` `crates/sql/src/exec/vec/hash_agg.rs:416`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `phase10-vectorized-executor`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `legacy` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:ffc3afef3290451f08394fc8df8c7ebb0f4e41058c97f24a5887d9031a991c47`
   Evidence: crates/sql/src/exec/vec/hash_agg.rs:416, future-hostile/dead-language term `legacy` appears
219. `high` `vibe` `crates/sql/src/exec/vec/hash_agg.rs:424`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `phase10-vectorized-executor`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `legacy` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:e3ae7d8d7a2dbbc3354e602ec7cfa5bba3ed04554fa4a0035b9a7570efcd1396`
   Evidence: crates/sql/src/exec/vec/hash_agg.rs:424, future-hostile/dead-language term `legacy` appears
220. `high` `vibe` `crates/sql/src/exec/vec/mod.rs:5`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `phase10-vectorized-executor`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `temp` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:5c3d8cab1c135c0918fffe99f56d12c8043c0aa5a706b9b406df732651026eb2`
   Evidence: crates/sql/src/exec/vec/mod.rs:5, future-hostile/dead-language term `temp` appears
221. `high` `vibe` `crates/sql/src/exec/vec/spill.rs:4`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `phase10-vectorized-executor`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `temp` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:aaec67fcb9e8bb23046f7c2254a1089cc043bc51b77178a90feedf59371d5c71`
   Evidence: crates/sql/src/exec/vec/spill.rs:4, future-hostile/dead-language term `temp` appears
222. `high` `vibe` `crates/sql/src/parser.rs:62`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `sql-parser-planner-executor`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `legacy` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:31a7a1bdcc861b4a3f476d10154dc5e4f2ea92725d0f72eac2701586bb686978`
   Evidence: crates/sql/src/parser.rs:62, future-hostile/dead-language term `legacy` appears
223. `high` `vibe` `crates/sql/src/parser.rs:87`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `sql-parser-planner-executor`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `legacy` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:214b34047e1eb42b1b22e8a076ca339ec942b1a694b06286f8c2277524017557`
   Evidence: crates/sql/src/parser.rs:87, future-hostile/dead-language term `legacy` appears
224. `high` `vibe` `crates/sql/src/parser/ddl.rs:14`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `sql-parser-planner-executor`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `temporary` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:9a90a79746fd11753e6995b5a8c95bfed9af518daa0d42e4cf9f5f883bb49478`
   Evidence: crates/sql/src/parser/ddl.rs:14, future-hostile/dead-language term `temporary` appears
225. `high` `vibe` `crates/sql/src/parser/ddl.rs:111`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `sql-parser-planner-executor`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `placeholder` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:ed00dd906863db8005d8b5cb38996fc9395bbdc1273d25fad61f9e562b3121c7`
   Evidence: crates/sql/src/parser/ddl.rs:111, future-hostile/dead-language term `placeholder` appears
226. `high` `vibe` `crates/sql/src/parser/ddl.rs:284`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `sql-parser-planner-executor`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `stub` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:37ea80ca268d145ea883d8ef10e44f5228c1598d6352e68b7961e34fdf3687f0`
   Evidence: crates/sql/src/parser/ddl.rs:284, future-hostile/dead-language term `stub` appears
227. `high` `vibe` `crates/sql/src/parser/helpers.rs:140`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `sql-parser-planner-executor`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `todo` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:5204094668a81c790e246c7b687e61700e3bff604bb5b71af6edb33de138a516`
   Evidence: crates/sql/src/parser/helpers.rs:140, future-hostile/dead-language term `todo` appears
228. `high` `vibe` `crates/sql/src/parser/helpers.rs:295`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `sql-parser-planner-executor`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `placeholder` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:5945114bf5de4708840da79d56df3ef3bc80f034c78495e8257bba9c0afe67ff`
   Evidence: crates/sql/src/parser/helpers.rs:295, future-hostile/dead-language term `placeholder` appears
229. `high` `vibe` `crates/sql/src/parser/select.rs:489`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `sql-parser-planner-executor`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `placeholder` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:1df337e3b6d4eacb5dc61ef4e206f7733608fc1298b9ecc3a9f5594c1368c179`
   Evidence: crates/sql/src/parser/select.rs:489, future-hostile/dead-language term `placeholder` appears
230. `high` `vibe` `crates/sql/src/parser/select.rs:492`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `sql-parser-planner-executor`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `placeholder` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:d172e1e1bc342b1d4765ee4bfc4a6ebbf8bb99de9c418931fc2114fb0a0ac981`
   Evidence: crates/sql/src/parser/select.rs:492, future-hostile/dead-language term `placeholder` appears
231. `high` `vibe` `crates/sql/src/planner/helpers.rs:209`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `sql-parser-planner-executor`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `placeholder` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:481ad1ff936bad8faa24913d4954f75c89d7eaa1acd77db00c5126457adeee84`
   Evidence: crates/sql/src/planner/helpers.rs:209, future-hostile/dead-language term `placeholder` appears
232. `high` `vibe` `crates/sql/src/regexp.rs:8`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `phase10-regexp`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `compat` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:3af9234eb7fe90c54addf78cb595c7a7ce443b6a2b81dcdc449238c947eceab9`
   Evidence: crates/sql/src/regexp.rs:8, future-hostile/dead-language term `compat` appears
233. `high` `vibe` `crates/sql/src/session.rs:30`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `sql-parser-planner-executor`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `unused` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:087648e3b7eeaea4e919f12041aa094cabd55ef3c6572da48e6ca68a7f8d54e4`
   Evidence: crates/sql/src/session.rs:30, future-hostile/dead-language term `unused` appears
234. `high` `security` `crates/sql/src/statement.rs:298`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `sql-parser-planner-executor`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:8848cc2a14147743290dea4b1fa0014b8c651522730897474c2bd474631f6663`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=let tx = unsafe { ptr.as_mut() };
235. `medium` `docs` `docs/`
   Check: `HLT-000-SCORE-DIMENSION:docs` `soft` confidence `0.76`
   Route: TLR `Context/setup`, lane `audit`, owner `standard`
   Reason: agent-readable documentation is incomplete
   Fix: add concise docs for architecture, boundaries, tests, generated zones, and audit rules; route them from root `AGENTS.md`
   Rerun: `just score`
   Fingerprint: `sha256:517269e227b26a69e8747d0d8cdb8a8d03582550b0de9e4c7c2b9287d8cee256`
   Evidence: docs/architecture.md or docs/boundaries.md, docs/testing.md
236. `high` `release` `docs/release.md`
   Rule: `HLT-025-RELEASE-READINESS-GAP`
   Check: `HLT-025-RELEASE-READINESS-GAP:release` `hard` confidence `0.88`
   Route: TLR `Verification`, lane `release`, owner `standard`
   Docs: `docs/testing.md`
   Matched term: `release structure`
   Reason: launch gates need artifact-backed release evidence
   Fix: add a release control surface with version source, changelog, release process docs, CI or script evidence, integrity/provenance evidence, and rollback guidance
   Rerun: `just check`
   Fingerprint: `sha256:640a623fff7ab3e802c3902adc8139100c4773a9c1de7d85998852f57db765fa`
   Evidence: release structure missing: release process doc
237. `medium` `observability` `docs/testing.md`
   Rule: `HLT-017-OPAQUE-OBSERVABILITY`
   Check: `HLT-017-OPAQUE-OBSERVABILITY:observability` `soft` confidence `0.76`
   Route: TLR `Repair`, lane `observability`, owner `standard`
   Docs: `agent/JANKURAI_STANDARD.md#repair-receipts`
   Reason: `Observability and repair evidence` scored 48 below the standard floor of 85
   Fix: add structured errors, telemetry, and repair receipts that tell the next agent where to rerun proof
   Rerun: `just score`
   Fingerprint: `sha256:f3e82a38d48dbca4e3cec3b0d72c64db7745489a72a65e284a47fe8f7652396e`
   Evidence: observability libraries or patterns found, ops/observability directory present, repair receipts or raw artifact language found, no agent-friendly exception pattern found
238. `medium` `release` `docs/testing.md`
   Rule: `HLT-026-COST-BUDGET-GAP`
   Check: `HLT-026-COST-BUDGET-GAP:release` `soft` confidence `0.88`
   Route: TLR `Verification`, lane `release`, owner `standard`
   Docs: `docs/testing.md`
   Matched term: `budget`
   Reason: unbounded paid work needs budgets and stop conditions
   Fix: add explicit budgets, quotas, stop conditions, and kill-switch evidence for paid or unbounded operations
   Rerun: `just check`
   Fingerprint: `sha256:edd248b7afc24b644107205fa5b84a88103ac4b622009ff9f19b779de8798f59`
   Evidence: cost surface found without budget/stop-condition policy
239. `high` `python` `python/`
   Rule: `HLT-005-PYTHON-PRODUCT-TRUTH`
   Check: `HLT-005-PYTHON-PRODUCT-TRUTH:python` `hard` confidence `0.88`
   Route: TLR `Business truth`, lane `contract`, owner `tools`
   Docs: `docs/agent-native-standard.md`
   Reason: Python appears without a rare advanced-ML/data exception or owns product truth
   Fix: remove Python or box it under `python/ai-service` only when a dated advanced-ML/data exception exists; otherwise migrate it to Rust
   Rerun: `just fast`
   Fingerprint: `sha256:a9ed62b36b036c9cc50ca8f42844065f4bb44a12c1b6a715d99db7453617b019`
   Evidence: Python must stay away from product truth, repo tooling, proof lanes, backend glue, and production DB ownership
240. `medium` `python` `python/ai-service`
   Rule: `HLT-005-PYTHON-PRODUCT-TRUTH`
   Check: `HLT-005-PYTHON-PRODUCT-TRUTH:python` `soft` confidence `0.76`
   Route: TLR `Business truth`, lane `contract`, owner `tools`
   Docs: `docs/agent-native-standard.md`
   Reason: `Python containment and polyglot hygiene` scored 0 below the standard floor of 85
   Fix: remove Python unless it is a dated advanced-ML/data exception and move product truth into Rust, SQL, and generated contracts
   Rerun: `just fast`
   Fingerprint: `sha256:557d4f169ccef311815c39ae27bafa95b0bc6289f2ec16b7ec886d1991c7ab38`
   Evidence: Python appears outside the allowed roots: scripts/bench/dick_head_choas_report.py, non-optimal product language marker: crates/ffi/include/redlinedb.h, Python leaks into product surface, runtime code should converge to Rust, TypeScript, SQL, generated contracts, and rare advanced-ML/data Python exceptions

## Policy

- Policy file: `./agent/audit-policy.toml`
- Minimum score: `85`
- Fail on: `critical, high`

## Agent Fix Queue

1. `high` `HLT-022-AUTHZ-ISOLATION-GAP` `crates/bench/src/config.rs` - add owner/non-owner authorization tests or RLS evidence for the touched data boundary
   Route: `Business truth`/`db`
2. `high` `HLT-005-PYTHON-PRODUCT-TRUTH` `python/` - remove Python or box it under `python/ai-service` only when a dated advanced-ML/data exception exists; otherwise migrate it to Rust
   Route: `Business truth`/`contract`
3. `medium` `HLT-005-PYTHON-PRODUCT-TRUTH` `python/ai-service` - remove Python unless it is a dated advanced-ML/data exception and move product truth into Rust, SQL, and generated contracts
   Route: `Business truth`/`contract`
4. `high` `HLT-004-UNMAPPED-PROOF` `agent/test-map.json` - add the narrowest stable prefix and runnable proof command to `agent/test-map.json`
   Route: `Verification`/`fast`
5. `high` `HLT-025-RELEASE-READINESS-GAP` `docs/release.md` - add a release control surface with version source, changelog, release process docs, CI or script evidence, integrity/provenance evidence, and rollback guidance
   Route: `Verification`/`release`
6. `medium` `HLT-018-PERF-CONCURRENCY-DRIFT` `Justfile` - add fast deterministic build/test targets, caches, and narrow proof lanes for agent iteration
   Route: `Verification`/`fast`
7. `medium` `HLT-026-COST-BUDGET-GAP` `docs/testing.md` - add explicit budgets, quotas, stop conditions, and kill-switch evidence for paid or unbounded operations
   Route: `Verification`/`release`
8. `high` `HLT-017-OPAQUE-OBSERVABILITY` `crates/domain` - define a typed exception surface with purpose, reason, common fixes, docs_url, and repair_hint so the next rerun is local
   Route: `Repair`/`observability`
9. `medium` `HLT-017-OPAQUE-OBSERVABILITY` `docs/testing.md` - add structured errors, telemetry, and repair receipts that tell the next agent where to rerun proof
   Route: `Repair`/`observability`
10. `high` `HLT-003-OWNERLESS-PATH` `agent/owner-map.json` - add the narrowest stable prefix for this path to `agent/owner-map.json`
   Route: `Context/setup`/`fast`
11. `high` `crates/ffi/include/redlinedb.h` - move product runtime behavior to Rust core, TypeScript web, SQL migrations, or generated contracts; Python needs a dated advanced-ML/data exception
   Route: `Context/setup`/`audit`
12. `medium` `HLT-015-CONTEXT-SETUP-GAP` `AGENTS.md` - keep root guidance short and route durable detail through agent-readable manifests and docs
   Route: `Context/setup`/`fast`
13. `medium` `HLT-040-REPO-ROT-BAD-BEHAVIOR` `crates/bench/bench/certification-phase10-v3-compare.toml` - delete the stale copy, move history to VCS/archive tooling, or document owner, proof lane, expiry, and migration plan
   Route: `Context/setup`/`audit`
14. `medium` `HLT-040-REPO-ROT-BAD-BEHAVIOR` `crates/bench/bench/certification-phase10-v3-smoke.toml` - delete the stale copy, move history to VCS/archive tooling, or document owner, proof lane, expiry, and migration plan
   Route: `Context/setup`/`audit`
15. `medium` `HLT-040-REPO-ROT-BAD-BEHAVIOR` `crates/bench/bench/certification-phase10-v3-stress.toml` - delete the stale copy, move history to VCS/archive tooling, or document owner, proof lane, expiry, and migration plan
   Route: `Context/setup`/`audit`
16. `medium` `HLT-040-REPO-ROT-BAD-BEHAVIOR` `crates/bench/bench/certification-phase10-v3.toml` - delete the stale copy, move history to VCS/archive tooling, or document owner, proof lane, expiry, and migration plan
   Route: `Context/setup`/`audit`
17. `medium` `HLT-040-REPO-ROT-BAD-BEHAVIOR` `crates/ffi/src/backup.rs` - delete the stale copy, move history to VCS/archive tooling, or document owner, proof lane, expiry, and migration plan
   Route: `Context/setup`/`audit`
18. `medium` `HLT-040-REPO-ROT-BAD-BEHAVIOR` `crates/redlinedb/src/backup.rs` - delete the stale copy, move history to VCS/archive tooling, or document owner, proof lane, expiry, and migration plan
   Route: `Context/setup`/`audit`
19. `medium` `docs/` - add concise docs for architecture, boundaries, tests, generated zones, and audit rules; route them from root `AGENTS.md`
   Route: `Context/setup`/`audit`
20. `high` `HLT-001-DEAD-MARKER` `crates/bench/src/certify.rs` - remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Route: `Entropy`/`fast`
21. `high` `HLT-001-DEAD-MARKER` `crates/bench/src/certify.rs` - collapse fallback chains into explicit typed states with bounded retry policy, telemetry, and documented repair guidance
   Route: `Entropy`/`fast`
22. `high` `crates/bench/src/chaos.rs` - extract the duplicated behavior behind one named boundary and add focused tests before changing behavior
   Route: `Entropy`/`fast`
23. `high` `HLT-001-DEAD-MARKER` `crates/bench/src/chaos.rs` - remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Route: `Entropy`/`fast`
24. `high` `HLT-001-DEAD-MARKER` `crates/bench/src/checksum.rs` - remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Route: `Entropy`/`fast`
25. `high` `HLT-001-DEAD-MARKER` `crates/bench/src/compat.rs` - remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Route: `Entropy`/`fast`
26. `high` `HLT-001-DEAD-MARKER` `crates/bench/src/config.rs` - remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Route: `Entropy`/`fast`
27. `high` `HLT-001-DEAD-MARKER` `crates/bench/src/engine/redline.rs` - remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Route: `Entropy`/`fast`
28. `high` `HLT-001-DEAD-MARKER` `crates/bench/src/feature_workloads.rs` - remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Route: `Entropy`/`fast`
29. `high` `HLT-001-DEAD-MARKER` `crates/bench/src/lib.rs` - remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Route: `Entropy`/`fast`
30. `high` `HLT-001-DEAD-MARKER` `crates/bench/src/process_metrics.rs` - remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Route: `Entropy`/`fast`
31. `high` `HLT-029-RUST-BAD-BEHAVIOR` `crates/bench/src/process_metrics.rs` - construct the type with a valid initializer instead of zeroing it
   Route: `Security, secrets, agency`/`fast`
32. `high` `HLT-001-DEAD-MARKER` `crates/bench/src/report.rs` - remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Route: `Entropy`/`fast`
33. `high` `HLT-001-DEAD-MARKER` `crates/bench/src/workload.rs` - remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Route: `Entropy`/`fast`
34. `high` `HLT-023-INPUT-BOUNDARY-GAP` `crates/ffi/include/redlinedb.h` - replace unsafe sinks with typed schemas, parameterized APIs, allowlists, or sandboxed execution plus negative tests
   Route: `Security, secrets, agency`/`security`
35. `high` `HLT-029-RUST-BAD-BEHAVIOR` `crates/ffi/src/backup.rs` - add a precise `SAFETY:` comment or remove the unsafe block
   Route: `Security, secrets, agency`/`fast`
36. `high` `HLT-029-RUST-BAD-BEHAVIOR` `crates/ffi/src/backup.rs` - use the matching constructor/destructor pair or add a documented ownership proof
   Route: `Security, secrets, agency`/`fast`
37. `high` `HLT-029-RUST-BAD-BEHAVIOR` `crates/ffi/src/bind.rs` - add a precise `SAFETY:` comment or remove the unsafe block
   Route: `Security, secrets, agency`/`fast`
38. `high` `HLT-029-RUST-BAD-BEHAVIOR` `crates/ffi/src/column.rs` - add a precise `SAFETY:` comment or remove the unsafe block
   Route: `Security, secrets, agency`/`fast`
39. `high` `HLT-029-RUST-BAD-BEHAVIOR` `crates/ffi/src/config.rs` - add a precise `SAFETY:` comment or remove the unsafe block
   Route: `Security, secrets, agency`/`fast`
40. `high` `HLT-029-RUST-BAD-BEHAVIOR` `crates/ffi/src/error.rs` - add a precise `SAFETY:` comment or remove the unsafe block
   Route: `Security, secrets, agency`/`fast`
41. `high` `HLT-029-RUST-BAD-BEHAVIOR` `crates/ffi/src/error.rs` - use the matching constructor/destructor pair or add a documented ownership proof
   Route: `Security, secrets, agency`/`fast`
42. `high` `HLT-029-RUST-BAD-BEHAVIOR` `crates/ffi/src/exec.rs` - add a precise `SAFETY:` comment or remove the unsafe block
   Route: `Security, secrets, agency`/`fast`
43. `high` `HLT-001-DEAD-MARKER` `crates/ffi/src/lib.rs` - remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Route: `Entropy`/`fast`
44. `high` `HLT-029-RUST-BAD-BEHAVIOR` `crates/ffi/src/lifecycle.rs` - add a precise `SAFETY:` comment or remove the unsafe block
   Route: `Security, secrets, agency`/`fast`
45. `high` `HLT-029-RUST-BAD-BEHAVIOR` `crates/ffi/src/lifecycle.rs` - use the matching constructor/destructor pair or add a documented ownership proof
   Route: `Security, secrets, agency`/`fast`
46. `high` `HLT-029-RUST-BAD-BEHAVIOR` `crates/ffi/src/sqlite3_compat.rs` - add a precise `SAFETY:` comment or remove the unsafe block
   Route: `Security, secrets, agency`/`fast`
47. `high` `HLT-001-DEAD-MARKER` `crates/ffi/src/sqlite3_compat.rs` - remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Route: `Entropy`/`fast`
48. `high` `HLT-001-DEAD-MARKER` `crates/ffi/src/sqlite3_compat.rs` - replace placeholders with implemented behavior, typed unsupported-state errors, or a tracked exception record with docs
   Route: `Entropy`/`fast`
49. `high` `HLT-029-RUST-BAD-BEHAVIOR` `crates/ffi/src/stmt.rs` - add a precise `SAFETY:` comment or remove the unsafe block
   Route: `Security, secrets, agency`/`fast`
50. `high` `HLT-029-RUST-BAD-BEHAVIOR` `crates/ffi/src/stmt.rs` - use the matching constructor/destructor pair or add a documented ownership proof
   Route: `Security, secrets, agency`/`fast`
51. `high` `HLT-001-DEAD-MARKER` `crates/ffi/src/types.rs` - remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Route: `Entropy`/`fast`
52. `high` `HLT-029-RUST-BAD-BEHAVIOR` `crates/ffi/src/util.rs` - add a precise `SAFETY:` comment or remove the unsafe block
   Route: `Security, secrets, agency`/`fast`
53. `high` `HLT-001-DEAD-MARKER` `crates/ffi/src/util.rs` - remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Route: `Entropy`/`fast`
54. `high` `HLT-029-RUST-BAD-BEHAVIOR` `crates/ffi/src/util.rs` - document caller obligations with a `# Safety` section
   Route: `Security, secrets, agency`/`fast`
55. `high` `HLT-001-DEAD-MARKER` `crates/kernel/src/catalog/store.rs` - remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Route: `Entropy`/`fast`
56. `high` `HLT-001-DEAD-MARKER` `crates/kernel/src/engine/catalog_ops.rs` - remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Route: `Entropy`/`fast`
57. `high` `HLT-001-DEAD-MARKER` `crates/kernel/src/index/cursor.rs` - remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Route: `Entropy`/`fast`
58. `high` `HLT-001-DEAD-MARKER` `crates/kernel/src/index/cursor/raw.rs` - remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Route: `Entropy`/`fast`
59. `high` `HLT-001-DEAD-MARKER` `crates/kernel/src/index/scan.rs` - remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Route: `Entropy`/`fast`
60. `high` `HLT-001-DEAD-MARKER` `crates/kernel/src/integrity/equivalence.rs` - remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Route: `Entropy`/`fast`
61. `high` `HLT-001-DEAD-MARKER` `crates/kernel/src/integrity/mod.rs` - remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Route: `Entropy`/`fast`
62. `high` `HLT-001-DEAD-MARKER` `crates/kernel/src/integrity/page_csum.rs` - remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Route: `Entropy`/`fast`
63. `high` `HLT-001-DEAD-MARKER` `crates/kernel/src/json/simd_key.rs` - remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Route: `Entropy`/`fast`
64. `high` `HLT-001-DEAD-MARKER` `crates/kernel/src/vector/diskann/mod.rs` - remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Route: `Entropy`/`fast`
65. `high` `HLT-001-DEAD-MARKER` `crates/kernel/src/vector/diskann/sectors.rs` - remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Route: `Entropy`/`fast`
66. `high` `HLT-029-RUST-BAD-BEHAVIOR` `crates/kernel/src/vector/diskann/sectors.rs` - replace the mutable static with explicit synchronization or scoped ownership
   Route: `Security, secrets, agency`/`fast`
67. `high` `HLT-029-RUST-BAD-BEHAVIOR` `crates/kernel/src/vector/diskann/sectors.rs` - add a precise `SAFETY:` comment or remove the unsafe block
   Route: `Security, secrets, agency`/`fast`
68. `high` `HLT-001-DEAD-MARKER` `crates/kernel/src/vector/flat.rs` - remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Route: `Entropy`/`fast`
69. `high` `HLT-001-DEAD-MARKER` `crates/kernel/src/vector/hnsw/builder.rs` - remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Route: `Entropy`/`fast`
70. `high` `HLT-001-DEAD-MARKER` `crates/kernel/src/vector/hnsw/mod.rs` - remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Route: `Entropy`/`fast`
71. `high` `HLT-001-DEAD-MARKER` `crates/kernel/src/vector/hnsw/searcher.rs` - remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Route: `Entropy`/`fast`
72. `high` `HLT-001-DEAD-MARKER` `crates/kernel/src/vector/simd.rs` - remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Route: `Entropy`/`fast`
73. `high` `HLT-029-RUST-BAD-BEHAVIOR` `crates/kernel/src/vector/simd.rs` - add a precise `SAFETY:` comment or remove the unsafe block
   Route: `Security, secrets, agency`/`fast`
74. `high` `HLT-001-DEAD-MARKER` `crates/kernel/src/wal/manager.rs` - remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Route: `Entropy`/`fast`
75. `high` `HLT-001-DEAD-MARKER` `crates/kernel/src/wal/manager/storage.rs` - remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Route: `Entropy`/`fast`
76. `high` `HLT-001-DEAD-MARKER` `crates/redlinedb/src/lib.rs` - remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Route: `Entropy`/`fast`
77. `high` `HLT-001-DEAD-MARKER` `crates/redlinedb/src/machine.rs` - remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Route: `Entropy`/`fast`
78. `high` `HLT-029-RUST-BAD-BEHAVIOR` `crates/redlinedb/src/registry.rs` - replace the mutable static with explicit synchronization or scoped ownership
   Route: `Security, secrets, agency`/`fast`
79. `high` `HLT-029-RUST-BAD-BEHAVIOR` `crates/redlinedb/src/registry.rs` - add a precise `SAFETY:` comment or remove the unsafe block
   Route: `Security, secrets, agency`/`fast`
80. `high` `HLT-001-DEAD-MARKER` `crates/sql/src/connection.rs` - remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Route: `Entropy`/`fast`
81. `high` `HLT-001-DEAD-MARKER` `crates/sql/src/exec/expr/mod.rs` - remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Route: `Entropy`/`fast`
82. `high` `HLT-001-DEAD-MARKER` `crates/sql/src/exec/expr/scalar.rs` - remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Route: `Entropy`/`fast`
83. `high` `HLT-001-DEAD-MARKER` `crates/sql/src/exec/index_access.rs` - remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Route: `Entropy`/`fast`
84. `high` `HLT-001-DEAD-MARKER` `crates/sql/src/exec/index_dml.rs` - remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Route: `Entropy`/`fast`
85. `high` `HLT-001-DEAD-MARKER` `crates/sql/src/exec/vec/hash_agg.rs` - remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Route: `Entropy`/`fast`
86. `high` `HLT-001-DEAD-MARKER` `crates/sql/src/exec/vec/mod.rs` - remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Route: `Entropy`/`fast`
87. `high` `HLT-001-DEAD-MARKER` `crates/sql/src/exec/vec/spill.rs` - remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Route: `Entropy`/`fast`
88. `high` `HLT-001-DEAD-MARKER` `crates/sql/src/parser.rs` - remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Route: `Entropy`/`fast`
89. `high` `HLT-001-DEAD-MARKER` `crates/sql/src/parser/ddl.rs` - remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Route: `Entropy`/`fast`
90. `high` `HLT-001-DEAD-MARKER` `crates/sql/src/parser/helpers.rs` - remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Route: `Entropy`/`fast`
91. `high` `HLT-001-DEAD-MARKER` `crates/sql/src/parser/select.rs` - remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Route: `Entropy`/`fast`
92. `high` `HLT-001-DEAD-MARKER` `crates/sql/src/planner/helpers.rs` - remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Route: `Entropy`/`fast`
93. `high` `HLT-001-DEAD-MARKER` `crates/sql/src/regexp.rs` - remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Route: `Entropy`/`fast`
94. `high` `HLT-001-DEAD-MARKER` `crates/sql/src/session.rs` - remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Route: `Entropy`/`fast`
95. `high` `HLT-029-RUST-BAD-BEHAVIOR` `crates/sql/src/statement.rs` - add a precise `SAFETY:` comment or remove the unsafe block
   Route: `Security, secrets, agency`/`fast`
96. `medium` `HLT-001-DEAD-MARKER` `.` - split large or ambiguous authored code into smaller semantic modules with focused tests
   Route: `Entropy`/`fast`
97. `medium` `HLT-016-SUPPLY-CHAIN-DRIFT` `.github/workflows/jankurai.yml` - wire secret, dependency, provenance, and workflow scans into an operational CI lane
   Route: `Security, secrets, agency`/`security`
