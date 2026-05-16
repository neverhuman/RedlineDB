# jankurai Repo Score

- Standard: `jankurai`
- Auditor: `0.8.16`
- Schema: `1.7.0`
- Paper edition: `2026.05-ed8`
- Target stack ID: `rust-ts-vite-react-postgres-bounded-python`
- Target stack: `Rust core + TypeScript/React/Vite + PostgreSQL + generated contracts + exception-only Python AI/data service`
- Repo: `.`
- Run ID: `1778931808`
- Started at: `1778931808`
- Elapsed: `12505` ms
- Scope: `full`
- Raw score: `83`
- Final score: `83`
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

- Status: `review` hard=`0` warning=`42` files=`194`
- Policy: min-lines=`10` min-tokens=`100` max-findings=`50` include-tests=`false` strict=`false`
- Duplicate volume: lines=`56` tokens=`160` bytes=`1486`

- Notes:
  - hard classes are limited to exact active-source file matches and substantial exact same-name units
  - warning classes include same-body different-name units and token/block duplication
  - tests, fixtures, stories, config, Docker, and migrations are omitted unless --include-tests is set

| Kind | Severity | Language | Lines | Tokens | Instances | Reason |
| --- | --- | --- | ---: | ---: | --- | --- |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 2 | `crates/kernel/src/catalog/record.rs:152-153, crates/kernel/src/catalog/stats.rs:342-343, crates/kernel/src/catalog/stats.rs:353-354, crates/kernel/src/catalog/store.rs:544-545, crates/kernel/src/catalog/store.rs:554-555, crates/kernel/src/catalog/store.rs:564-565, crates/kernel/src/catalog/store.rs:574-575, crates/kernel/src/catalog/store.rs:584-585, crates/kernel/src/catalog/store.rs:607-608, crates/sql/src/exec/index_access.rs:694-695` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 2 | `crates/bench/src/config.rs:413-414, crates/bench/src/config.rs:434-435, crates/kernel/src/vector/diskann/mod.rs:359-360, crates/redlinedb/src/value.rs:40-41, crates/sql/src/exec/expr/scalar/row.rs:346-347, crates/sql/src/exec/expr/scalar/row.rs:359-360` | `same body appears under different names across files` |
| `ExactUnitSameName` | `Warning` | `rust` | 4 | 13 | `crates/kernel/src/index/locks.rs:200-204, crates/sql/src/session.rs:196-200` | `same-name semantic unit copied across multiple files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 2 | 5 | `crates/kernel/src/format/bytes.rs:44-46, crates/kernel/src/format/bytes.rs:49-51, crates/kernel/src/format/bytes.rs:54-56` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 2 | `crates/bench/src/config.rs:650-651, crates/redlinedb/src/statement.rs:180-181, crates/redlinedb/src/value.rs:106-107, crates/redlinedb/src/value.rs:118-119, crates/redlinedb/src/value.rs:130-131` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 2 | 3 | `crates/kernel/src/format/page.rs:458-460, crates/kernel/src/storage/control.rs:147-149, crates/kernel/src/storage/tx_status_checkpoint.rs:147-149` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 0 | `crates/kernel/src/catalog/ddl.rs:142-142, crates/kernel/src/failpoints/mod.rs:41-42, crates/kernel/src/integrity/equivalence.rs:207-207, crates/kernel/src/integrity/page_csum.rs:107-107, crates/sql/src/connection/session.rs:513-513` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 3 | 10 | `crates/sql/src/exec/index_batch.rs:451-454, crates/sql/src/exec/index_batch.rs:459-462` | `same body appears under different names across files` |
| `ExactUnitSameName` | `Warning` | `rust` | 3 | 4 | `crates/kernel/src/vector/flat.rs:57-60, crates/kernel/src/vector/hnsw/searcher.rs:47-50` | `same-name semantic unit copied across multiple files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 6 | `crates/sql/src/json/path.rs:543-544, crates/sql/src/json/path.rs:551-552, crates/sql/src/json/path.rs:559-560` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 4 | `crates/kernel/src/catalog/stats.rs:378-379, crates/kernel/src/catalog/store.rs:470-471, crates/kernel/src/catalog/store.rs:661-662` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 3 | `crates/redlinedb/src/connection.rs:194-195, crates/redlinedb/src/connection.rs:203-204, crates/redlinedb/src/connection.rs:213-214` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 3 | `crates/kernel/src/catalog/store.rs:630-631, crates/kernel/src/catalog/store.rs:638-639, crates/kernel/src/catalog/store.rs:646-647` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 2 | `crates/redlinedb/src/connection.rs:52-53, crates/redlinedb/src/connection.rs:88-89, crates/redlinedb/src/connection.rs:94-95` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 2 | 4 | `crates/sql/src/planner/helpers.rs:490-492, crates/sql/src/planner/helpers.rs:494-496` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 2 | 4 | `crates/sql/src/planner/helpers.rs:467-469, crates/sql/src/planner/helpers.rs:471-473` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 2 | 3 | `crates/kernel/src/catalog/key.rs:61-63, crates/kernel/src/index/mod.rs:686-688` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 2 | 1 | `crates/kernel/src/failpoints/mod.rs:65-67, crates/kernel/src/failpoints/mod.rs:109-111` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 2 | 1 | `crates/bench/src/certify/scheduler.rs:392-394, crates/kernel/src/engine/runtime.rs:42-44` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 7 | `crates/sql/src/exec/expr/scalar/row.rs:162-163, crates/sql/src/exec/expr/scalar/row.rs:166-167` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 6 | `crates/redlinedb/src/connection.rs:177-178, crates/redlinedb/src/statement.rs:204-205` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 6 | `crates/redlinedb/src/connection.rs:69-70, crates/redlinedb/src/connection.rs:75-76` | `same body appears under different names across files` |
| `ExactUnitSameName` | `Warning` | `rust` | 1 | 5 | `crates/bench/src/config.rs:561-562, crates/bench/src/config.rs:635-636` | `same-name semantic unit copied across multiple files` |
| `ExactUnitSameName` | `Warning` | `rust` | 1 | 5 | `crates/kernel/src/json/wire.rs:249-250, crates/kernel/src/json/wire.rs:321-322` | `same-name semantic unit copied across multiple files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 5 | `crates/kernel/src/catalog/stats.rs:289-290, crates/kernel/src/catalog/store.rs:289-290` | `same body appears under different names across files` |
| `ExactUnitSameName` | `Warning` | `rust` | 1 | 4 | `crates/redlinedb/src/statement.rs:142-143, crates/redlinedb/src/statement.rs:226-227` | `same-name semantic unit copied across multiple files` |
| `ExactUnitSameName` | `Warning` | `rust` | 1 | 4 | `crates/redlinedb/src/statement.rs:150-151, crates/redlinedb/src/statement.rs:234-235` | `same-name semantic unit copied across multiple files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 4 | `crates/domain/src/error.rs:73-74, crates/domain/src/error.rs:91-92` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 4 | `crates/sql/src/parser/savepoint.rs:64-65, crates/sql/src/parser/savepoint.rs:83-84` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 4 | `crates/sql/src/json/path.rs:116-117, crates/sql/src/json/path.rs:162-163` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 4 | `crates/kernel/src/vector/diskann/sectors.rs:306-307, crates/kernel/src/vector/diskann/sectors.rs:321-322` | `same body appears under different names across files` |
| `ExactUnitSameName` | `Warning` | `rust` | 1 | 3 | `crates/redlinedb/src/statement.rs:119-120, crates/redlinedb/src/statement.rs:200-201` | `same-name semantic unit copied across multiple files` |
| `ExactUnitSameName` | `Warning` | `rust` | 1 | 3 | `crates/redlinedb/src/statement.rs:138-139, crates/redlinedb/src/statement.rs:222-223` | `same-name semantic unit copied across multiple files` |
| `ExactUnitSameName` | `Warning` | `rust` | 1 | 3 | `crates/redlinedb/src/statement.rs:134-135, crates/redlinedb/src/statement.rs:218-219` | `same-name semantic unit copied across multiple files` |
| `ExactUnitSameName` | `Warning` | `rust` | 1 | 3 | `crates/redlinedb/src/statement.rs:146-147, crates/redlinedb/src/statement.rs:230-231` | `same-name semantic unit copied across multiple files` |
| `ExactUnitSameName` | `Warning` | `rust` | 1 | 3 | `crates/redlinedb/src/statement.rs:130-131, crates/redlinedb/src/statement.rs:214-215` | `same-name semantic unit copied across multiple files` |
| `ExactUnitSameName` | `Warning` | `rust` | 1 | 3 | `crates/redlinedb/src/statement.rs:154-155, crates/redlinedb/src/statement.rs:264-265` | `same-name semantic unit copied across multiple files` |
| `ExactUnitSameName` | `Warning` | `rust` | 1 | 3 | `crates/redlinedb/src/statement.rs:114-115, crates/redlinedb/src/statement.rs:195-196` | `same-name semantic unit copied across multiple files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 3 | `crates/kernel/src/catalog/store.rs:654-655, crates/kernel/src/catalog/store.rs:675-676` | `same body appears under different names across files` |
| `ExactUnitSameName` | `Warning` | `rust` | 1 | 2 | `crates/redlinedb/src/params.rs:15-16, crates/redlinedb/src/params.rs:25-26` | `same-name semantic unit copied across multiple files` |
| `ExactUnitSameName` | `Warning` | `rust` | 1 | 2 | `crates/kernel/src/json/wire.rs:273-274, crates/kernel/src/json/wire.rs:345-346` | `same-name semantic unit copied across multiple files` |
| `ExactUnitSameName` | `Warning` | `rust` | 1 | 2 | `crates/bench/src/config.rs:413-414, crates/bench/src/config.rs:434-435` | `same-name semantic unit copied across multiple files` |

## Dimensions

| Dimension | Weight | Score | Weighted | Evidence |
| --- | ---: | ---: | ---: | --- |
| Ownership and navigation surface | 13 | 100 | 13.00 | root `AGENTS.md` present; `CODEOWNERS` present |
| Contract and boundary integrity | 13 | 98 | 12.74 | contract surface found; generated contract artifacts found |
| Proof lanes and test routing | 12 | 100 | 12.00 | one-command setup/validation lane found; deterministic fast lane found |
| Security and supply-chain posture | 12 | 70 | 8.40 | lockfile present; secret or dependency scan tooling found |
| Code shape and semantic surface | 12 | 55 | 6.60 | largest authored code file: crates/sql/src/parser/helpers.rs (871 LOC); code file exceeds 500 LOC |
| Data truth and workflow safety | 8 | 95 | 7.60 | database surface present; structured db boundary manifest present |
| Observability and repair evidence | 8 | 88 | 7.04 | observability libraries or patterns found; ops/observability directory present |
| Context economy and agent instructions | 7 | 100 | 7.00 | root `AGENTS.md` present; root `AGENTS.md` stays short |
| Jankurai tool adoption and CI replacement | 7 | 26 | 1.82 | control-plane files present; applicable=16 |
| Python containment and polyglot hygiene | 4 | 100 | 4.00 | no Python files in scope |
| Build speed signals | 4 | 80 | 3.20 | build acceleration markers found; targeted test/build commands found |

## Reference Profile Structure

- Applicable cells: `8` canonical=`8` noncanonical=`0` guidance missing=`0`

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
| `python-ai` | `not_applicable` | `python/ai-service/` | `-` | `python/, ai-service/, evals/, embeddings/, model/` | `not_required` | `python/ai-service` | `eval / contract tests` | `no action` |
| `ops` | `canonical` | `ops/` | `.github, .github/workflows, ops` | `.github/, .github/workflows/, ci/, release/, observability/, security/` | `present` | `ops` | `security lane / workflow lint` | `keep `ops/AGENTS.md` aligned with owns / forbidden / proof lane guidance` |

## Rendered UX QA

- Web surface: `false`
- Layered UX lane: `true`
- Missing: `none`

## Tool Adoption

- Control plane present: `true`
- Applicable tools: `16`
- Configured: `13`
- CI evidence: `0`
- Artifact verified: `0`
- Replaced count: `0`
- Missing CI evidence: `audit-ci, proof-routing, proofbind, proofmark-rust, copy-code, security, ci-bad-behavior, git-bad-behavior, release-bad-behavior, contract-drift, rust-witness, authz-matrix, input-boundary, agent-tool-supply, release-readiness, cost-budget`

| Tool | Category | Mode | Status | Replaced | Artifacts |
| --- | --- | --- | --- | --- | --- |
| `audit-ci` | `audit` | `auto` | `configured` | `manual repo scoring, ad hoc score gates` | `agent/repo-score.json, agent/repo-score.md` |
| `proof-routing` | `proof` | `auto` | `configured` | `ad hoc proof lane selection, manual proof receipts` | `agent/repo-score.json, agent/repo-score.md, target/jankurai/repair-queue.jsonl` |
| `proofbind` | `proof` | `auto` | `missing` | `manual changed-surface routing, ad hoc proof obligation lists` | `target/jankurai/proofbind/surface-witness.json, target/jankurai/proofbind/obligations.json` |
| `proofmark-rust` | `proof` | `auto` | `missing` | `line-only coverage review, manual in-diff mutation review` | `target/jankurai/proofmark/proofmark-receipt.json, target/jankurai/proofmark/proof-receipt.json` |
| `copy-code` | `audit` | `auto` | `missing` | `ad hoc copy-code review, manual duplication triage` | `target/jankurai/copy-code.json, target/jankurai/copy-code.md` |
| `security` | `security` | `auto` | `configured` | `gitleaks, dependency review, SBOM/provenance` | `target/jankurai/security/evidence.json` |
| `ci-bad-behavior` | `security` | `auto` | `configured` | `mutable workflow refs, secret echo/debug workflow checks, non-blocking security scans` | `target/jankurai/language-bad-behavior.log` |
| `git-bad-behavior` | `audit` | `auto` | `configured` | `destructive git automation, force-push release scripts, hidden stash-based state` | `target/jankurai/language-bad-behavior.log` |
| `release-bad-behavior` | `release` | `auto` | `configured` | `manual release checklist, ad hoc tag and artifact review, manual provenance review` | `target/jankurai/language-bad-behavior.log` |
| `ux-qa` | `ux` | `auto` | `not_applicable` | `playwright, axe-core, visual baselines` | `target/jankurai/ux-qa.json` |
| `db-migration-analyze` | `db` | `auto` | `not_applicable` | `manual migration review` | `target/jankurai/migration-report.json` |
| `contract-drift` | `contract` | `auto` | `configured` | `handwritten contract drift checks, openapi diff` | `agent/repo-score.json, agent/repo-score.md` |
| `rust-witness` | `rust` | `auto` | `configured` | `manual witness graphing` | `target/jankurai/rust/witness-graph.json` |
| `vibe-coverage` | `audit` | `auto` | `not_applicable` | `manual vibe-coding coverage spreadsheet` | `target/jankurai/vibe-coverage.json, target/jankurai/vibe-coverage.md` |
| `coverage-evidence` | `proof` | `auto` | `not_applicable` | `manual coverage report review, ad hoc mutation survivor review` | `target/jankurai/coverage/coverage-audit.json, target/jankurai/coverage/coverage-audit.md` |
| `authz-matrix` | `security` | `auto` | `configured` | `manual authz matrix review` | `agent/repo-score.json, agent/repo-score.md` |
| `input-boundary` | `security` | `auto` | `configured` | `manual unsafe sink review` | `agent/repo-score.json, agent/repo-score.md` |
| `agent-tool-supply` | `security` | `auto` | `configured` | `manual MCP/tool trust review` | `agent/repo-score.json, agent/repo-score.md` |
| `release-readiness` | `release` | `auto` | `configured` | `manual launch checklist` | `agent/repo-score.json, agent/repo-score.md` |
| `cost-budget` | `release` | `auto` | `configured` | `manual spend review` | `agent/repo-score.json, agent/repo-score.md` |

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
   Reason: `Code shape and semantic surface` scored 55 below the standard floor of 85
   Fix: split large or ambiguous authored code into smaller semantic modules with focused tests
   Rerun: `just fast`
   Fingerprint: `sha256:ebdb8314fcd3c08dc352813cbc0adc0b2c959e3593d43b8f20f21ceda95a22dc`
   Evidence: largest authored code file: crates/sql/src/parser/helpers.rs (871 LOC), code file exceeds 500 LOC, copy-code advisory classes found: 42 (advisory only, no score impact), rust bad-behavior advisory signals: 1749
2. `medium` `security` `.github/workflows/jankurai.yml`
   Rule: `HLT-016-SUPPLY-CHAIN-DRIFT`
   Check: `HLT-016-SUPPLY-CHAIN-DRIFT:security` `soft` confidence `0.76`
   Route: TLR `Security, secrets, agency`, lane `security`, owner `ops`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Reason: `Security and supply-chain posture` scored 70 below the standard floor of 85
   Fix: wire secret, dependency, provenance, and workflow scans into an operational CI lane
   Rerun: `just security`
   Fingerprint: `sha256:9a1bcfb2380532658b7b977985f45f9847a29fcbe2a9166297d7ba7f558bf3b4`
   Evidence: lockfile present, secret or dependency scan tooling found, security lane present, canonical security lane wrapper present
3. `medium` `proof` `Justfile`
   Rule: `HLT-018-PERF-CONCURRENCY-DRIFT`
   Check: `HLT-018-PERF-CONCURRENCY-DRIFT:proof` `soft` confidence `0.76`
   Route: TLR `Verification`, lane `fast`, owner `workspace`
   Docs: `docs/testing.md`
   Reason: `Build speed signals` scored 80 below the standard floor of 85
   Fix: add fast deterministic build/test targets, caches, and narrow proof lanes for agent iteration
   Rerun: `just fast`
   Fingerprint: `sha256:2f2531223d7f7036c20d44b58cd52e64aa53ffd6cb85e01e541c1feff0c09cb2`
   Evidence: build acceleration markers found, targeted test/build commands found, locked dependency graph present, CI cache hint found
4. `medium` `release` `docs/testing.md`
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

## Policy

- Policy file: `./agent/audit-policy.toml`
- Minimum score: `85`
- Fail on: `critical, high`

## Agent Fix Queue

1. `medium` `HLT-018-PERF-CONCURRENCY-DRIFT` `Justfile` - add fast deterministic build/test targets, caches, and narrow proof lanes for agent iteration
   Route: `Verification`/`fast`
2. `medium` `HLT-026-COST-BUDGET-GAP` `docs/testing.md` - add explicit budgets, quotas, stop conditions, and kill-switch evidence for paid or unbounded operations
   Route: `Verification`/`release`
3. `medium` `HLT-001-DEAD-MARKER` `.` - split large or ambiguous authored code into smaller semantic modules with focused tests
   Route: `Entropy`/`fast`
4. `medium` `HLT-016-SUPPLY-CHAIN-DRIFT` `.github/workflows/jankurai.yml` - wire secret, dependency, provenance, and workflow scans into an operational CI lane
   Route: `Security, secrets, agency`/`security`
