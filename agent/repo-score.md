# jankurai Repo Score

- Standard: `jankurai`
- Auditor: `0.8.16`
- Schema: `1.7.0`
- Paper edition: `2026.05-ed8`
- Target stack ID: `rust-ts-vite-react-postgres-bounded-python`
- Target stack: `Rust core + TypeScript/React/Vite + PostgreSQL + generated contracts + exception-only Python AI/data service`
- Repo: `.`
- Run ID: `1779127012`
- Started at: `1779127012`
- Elapsed: `6611` ms
- Scope: `full`
- Raw score: `85`
- Final score: `64`
- Decision: `advisory`
- Minimum score: `85`
- Caps applied: `fallback-soup-in-product-code, future-hostile-dead-language-in-product-code, agent-tool-supply-chain-gap, rust-bad-behavior`

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
| `fallback-soup-in-product-code` | 70 | yes |
| `future-hostile-dead-language-in-product-code` | 64 | yes |
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
| `agent-tool-supply-chain-gap` | 78 | yes |
| `release-readiness-gap` | 80 | no |
| `missing-rust-property-or-integration-tests` | 82 | no |
| `no-agent-friendly-exception-pattern` | 76 | no |
| `missing-agent-readable-docs` | 80 | no |
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
| `repo-rot-bad-behavior` | 88 | no |
| `comment-hygiene-dangerous-residue` | 72 | no |
| `ci-local-parity` | 70 | no |

## Copy-Code Redundancy

- Status: `review` hard=`0` warning=`57` files=`284`
- Policy: min-lines=`10` min-tokens=`100` max-findings=`50` include-tests=`false` strict=`false`
- Duplicate volume: lines=`125` tokens=`368` bytes=`3513`

- Notes:
  - hard classes are limited to exact active-source file matches and substantial exact same-name units
  - warning classes include same-body different-name units and token/block duplication
  - tests, fixtures, stories, config, Docker, and migrations are omitted unless --include-tests is set
  - showing the top 50 classes and omitting 7 lower-ranked classes

| Kind | Severity | Language | Lines | Tokens | Instances | Reason |
| --- | --- | --- | ---: | ---: | --- | --- |
| `ExactUnitSameName` | `Warning` | `rust` | 13 | 42 | `crates/sql/src/exec/cross_db.rs:194-207, crates/sql/src/exec/cte.rs:137-150, crates/sql/src/exec/view.rs:179-192` | `same-name semantic unit copied across multiple files` |
| `ExactUnitSameName` | `Warning` | `rust` | 16 | 37 | `crates/sql/src/exec/agg/select.rs:82-98, crates/sql/src/planner/access/projection.rs:150-166` | `same-name semantic unit copied across multiple files` |
| `ExactUnitSameName` | `Warning` | `rust` | 11 | 57 | `crates/sql/src/datetime/format.rs:75-86, crates/sql/src/datetime/modifiers.rs:145-156` | `same-name semantic unit copied across multiple files` |
| `ExactUnitSameName` | `Warning` | `rust` | 5 | 9 | `crates/sql/src/exec/json_tv.rs:218-223, crates/sql/src/exec/json_tv.rs:273-278, crates/sql/src/exec/pragma_tv.rs:192-197` | `same-name semantic unit copied across multiple files` |
| `ExactUnitSameName` | `Warning` | `rust` | 5 | 9 | `crates/sql/src/exec/pragma_tv.rs:70-75, crates/sql/src/exec/pragma_tv.rs:102-107, crates/sql/src/exec/pragma_tv.rs:128-133` | `same-name semantic unit copied across multiple files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 2 | `crates/redlinedb/src/value.rs:43-44, crates/redlinedb/src/value.rs:57-58, crates/redlinedb/src/value.rs:64-65, crates/redlinedb/src/value.rs:71-72, crates/redlinedb/src/value.rs:78-79, crates/sql/src/exec/expr/scalar/row/model.rs:64-65, crates/sql/src/exec/expr/scalar/row/model.rs:77-78, crates/sql/src/exec/expr/scalar/row/model.rs:87-88` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 6 | `crates/redlinedb-sqlx/src/bridge.rs:174-175, crates/redlinedb-sqlx/src/bridge.rs:253-254, crates/redlinedb-sqlx/src/bridge.rs:258-259, crates/redlinedb-sqlx/src/bridge.rs:282-283, crates/redlinedb-sqlx/src/bridge.rs:297-298, crates/redlinedb-sqlx/src/bridge.rs:312-313, crates/redlinedb-sqlx/src/bridge.rs:394-395` | `same body appears under different names across files` |
| `ExactUnitSameName` | `Warning` | `rust` | 6 | 20 | `crates/sql/src/exec/agg/select.rs:9-15, crates/sql/src/planner/access/projection.rs:94-100` | `same-name semantic unit copied across multiple files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 2 | `crates/kernel/src/catalog/record.rs:152-153, crates/kernel/src/catalog/stats/wire.rs:162-163, crates/kernel/src/catalog/stats/wire.rs:173-174, crates/redlinedb-sqlx/src/bridge.rs:542-543, crates/redlinedb-sqlx/src/bridge.rs:639-640, crates/redlinedb/src/value_conv.rs:261-262, crates/sql/src/exec/json_tv.rs:231-232` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 0 | `crates/kernel/src/catalog/ddl.rs:250-250, crates/kernel/src/failpoints/mod.rs:41-42, crates/kernel/src/integrity/equivalence.rs:214-214, crates/kernel/src/integrity/page_csum.rs:107-107, crates/redlinedb-sqlx/src/bridge.rs:204-205, crates/redlinedb-sqlx/src/bridge.rs:328-329` | `same body appears under different names across files` |
| `ExactUnitSameName` | `Warning` | `rust` | 4 | 13 | `crates/kernel/src/index/locks.rs:200-204, crates/sql/src/session.rs:269-273` | `same-name semantic unit copied across multiple files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 2 | 5 | `crates/kernel/src/format/bytes.rs:44-46, crates/kernel/src/format/bytes.rs:49-51, crates/kernel/src/format/bytes.rs:54-56` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 4 | 9 | `crates/sql/src/exec/expr/scalar/row/lookup.rs:56-60, crates/sql/src/exec/expr/scalar/row/lookup.rs:117-121` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 2 | 3 | `crates/kernel/src/format/page.rs:102-104, crates/kernel/src/storage/control.rs:147-149, crates/kernel/src/storage/tx_status_checkpoint.rs:147-149` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 4 | 5 | `crates/redlinedb/src/connection.rs:90-94, crates/redlinedb/src/connection.rs:105-109` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 4 | `crates/redlinedb-sqlx/src/bridge.rs:160-161, crates/redlinedb-sqlx/src/bridge.rs:167-168, crates/redlinedb-sqlx/src/bridge.rs:239-240, crates/redlinedb-sqlx/src/bridge.rs:246-247` | `same body appears under different names across files` |
| `ExactUnitSameName` | `Warning` | `rust` | 3 | 4 | `crates/kernel/src/vector/flat.rs:57-60, crates/kernel/src/vector/hnsw/searcher.rs:47-50` | `same-name semantic unit copied across multiple files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 6 | `crates/redlinedb-sqlx/src/dummy.rs:399-400, crates/redlinedb-sqlx/src/dummy.rs:414-415, crates/redlinedb-sqlx/src/dummy.rs:429-430` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 4 | `crates/redlinedb-sqlx/src/bridge.rs:200-201, crates/redlinedb-sqlx/src/bridge.rs:206-207, crates/redlinedb-sqlx/src/bridge.rs:330-331` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 3 | `crates/redlinedb/src/connection.rs:228-229, crates/redlinedb/src/connection.rs:237-238, crates/redlinedb/src/connection.rs:247-248` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 2 | 5 | `crates/kernel/src/json/path_bytecode.rs:138-140, crates/sql/src/parser.rs:186-188` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 2 | `crates/redlinedb/src/connection.rs:52-53, crates/redlinedb/src/connection.rs:118-119, crates/redlinedb/src/connection.rs:124-125` | `same body appears under different names across files` |
| `ExactUnitSameName` | `Warning` | `rust` | 2 | 3 | `crates/redlinedb-tokio/src/lib.rs:268-270, crates/redlinedb/src/pool.rs:199-201` | `same-name semantic unit copied across multiple files` |
| `ExactUnitSameName` | `Warning` | `rust` | 2 | 3 | `crates/redlinedb-tokio/src/lib.rs:263-265, crates/redlinedb/src/pool.rs:204-206` | `same-name semantic unit copied across multiple files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 2 | 1 | `crates/kernel/src/failpoints/mod.rs:65-67, crates/kernel/src/failpoints/mod.rs:109-111` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 2 | 1 | `crates/bench/src/certify/scheduler/dispatch.rs:196-198, crates/kernel/src/engine/runtime/commit.rs:30-32` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 7 | `crates/sql/src/exec/expr/scalar/row/lookup.rs:161-162, crates/sql/src/exec/expr/scalar/row/lookup.rs:165-166` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 7 | `crates/redlinedb/src/value_conv.rs:245-246, crates/redlinedb/src/value_conv.rs:253-254` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 6 | `crates/redlinedb/src/connection.rs:211-212, crates/redlinedb/src/statement.rs:204-205` | `same body appears under different names across files` |
| `ExactUnitSameName` | `Warning` | `rust` | 1 | 6 | `crates/redlinedb-sqlx/src/dummy.rs:159-160, crates/redlinedb-sqlx/src/dummy.rs:187-188` | `same-name semantic unit copied across multiple files` |
| `ExactUnitSameName` | `Warning` | `rust` | 1 | 6 | `crates/redlinedb-sqlx/src/bridge.rs:174-175, crates/redlinedb-sqlx/src/bridge.rs:253-254` | `same-name semantic unit copied across multiple files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 6 | `crates/redlinedb/src/connection.rs:69-70, crates/redlinedb/src/connection.rs:75-76` | `same body appears under different names across files` |
| `ExactUnitSameName` | `Warning` | `rust` | 1 | 5 | `crates/redlinedb-sqlx/src/dummy.rs:155-156, crates/redlinedb-sqlx/src/dummy.rs:183-184` | `same-name semantic unit copied across multiple files` |
| `ExactUnitSameName` | `Warning` | `rust` | 1 | 5 | `crates/kernel/src/json/wire/iter.rs:20-21, crates/kernel/src/json/wire/iter.rs:91-92` | `same-name semantic unit copied across multiple files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 5 | `crates/redlinedb/src/value.rs:331-332, crates/redlinedb/src/value.rs:343-344` | `same body appears under different names across files` |
| `ExactUnitSameName` | `Warning` | `rust` | 1 | 4 | `crates/redlinedb/src/statement.rs:142-143, crates/redlinedb/src/statement.rs:226-227` | `same-name semantic unit copied across multiple files` |
| `ExactUnitSameName` | `Warning` | `rust` | 1 | 4 | `crates/redlinedb/src/statement.rs:150-151, crates/redlinedb/src/statement.rs:234-235` | `same-name semantic unit copied across multiple files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 4 | `crates/domain/src/error.rs:73-74, crates/domain/src/error.rs:91-92` | `same body appears under different names across files` |
| `ExactUnitSameName` | `Warning` | `rust` | 1 | 4 | `crates/redlinedb-sqlx/src/bridge.rs:206-207, crates/redlinedb-sqlx/src/bridge.rs:330-331` | `same-name semantic unit copied across multiple files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 4 | `crates/sql/src/parser/savepoint.rs:64-65, crates/sql/src/parser/savepoint.rs:83-84` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 4 | `crates/redlinedb/src/value.rs:534-535, crates/redlinedb/src/value_conv.rs:389-390` | `same body appears under different names across files` |
| `ExactUnitSameName` | `Warning` | `rust` | 1 | 3 | `crates/redlinedb/src/statement.rs:119-120, crates/redlinedb/src/statement.rs:200-201` | `same-name semantic unit copied across multiple files` |
| `ExactUnitSameName` | `Warning` | `rust` | 1 | 3 | `crates/redlinedb/src/statement.rs:138-139, crates/redlinedb/src/statement.rs:222-223` | `same-name semantic unit copied across multiple files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 3 | `crates/sql/src/exec/expr/scalar/row/lookup.rs:180-181, crates/sql/src/exec/expr/scalar/row/lookup.rs:217-218` | `same body appears under different names across files` |
| `ExactUnitSameName` | `Warning` | `rust` | 1 | 3 | `crates/redlinedb/src/statement.rs:134-135, crates/redlinedb/src/statement.rs:218-219` | `same-name semantic unit copied across multiple files` |
| `ExactUnitSameName` | `Warning` | `rust` | 1 | 3 | `crates/redlinedb/src/statement.rs:146-147, crates/redlinedb/src/statement.rs:230-231` | `same-name semantic unit copied across multiple files` |
| `ExactUnitSameName` | `Warning` | `rust` | 1 | 3 | `crates/redlinedb/src/statement.rs:130-131, crates/redlinedb/src/statement.rs:214-215` | `same-name semantic unit copied across multiple files` |
| `ExactUnitSameName` | `Warning` | `rust` | 1 | 3 | `crates/redlinedb/src/statement.rs:154-155, crates/redlinedb/src/statement.rs:264-265` | `same-name semantic unit copied across multiple files` |
| `ExactUnitSameName` | `Warning` | `rust` | 1 | 3 | `crates/redlinedb/src/statement.rs:114-115, crates/redlinedb/src/statement.rs:195-196` | `same-name semantic unit copied across multiple files` |
| `ExactUnitSameName` | `Warning` | `rust` | 1 | 2 | `crates/redlinedb/src/params.rs:15-16, crates/redlinedb/src/params.rs:25-26` | `same-name semantic unit copied across multiple files` |

## Dimensions

| Dimension | Weight | Score | Weighted | Evidence |
| --- | ---: | ---: | ---: | --- |
| Ownership and navigation surface | 13 | 100 | 13.00 | root `AGENTS.md` present; `CODEOWNERS` present |
| Contract and boundary integrity | 13 | 98 | 12.74 | contract surface found; generated contract artifacts found |
| Proof lanes and test routing | 12 | 100 | 12.00 | one-command setup/validation lane found; deterministic fast lane found |
| Security and supply-chain posture | 12 | 86 | 10.32 | lockfile present; secret or dependency scan tooling found |
| Code shape and semantic surface | 12 | 8 | 0.96 | largest authored code file: crates/redlinedb-sqlx/src/bridge.rs (692 LOC); code file exceeds 500 LOC |
| Data truth and workflow safety | 8 | 95 | 7.60 | database surface present; structured db boundary manifest present |
| Observability and repair evidence | 8 | 88 | 7.04 | observability libraries or patterns found; ops/observability directory present |
| Context economy and agent instructions | 7 | 100 | 7.00 | root `AGENTS.md` present; root `AGENTS.md` stays short |
| Jankurai tool adoption and CI replacement | 7 | 100 | 7.00 | control-plane files present; applicable=16 |
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
- Configured: `16`
- CI evidence: `16`
- Artifact verified: `16`
- Replaced count: `16`
- Missing CI evidence: `none`

| Tool | Category | Mode | Status | Replaced | Artifacts |
| --- | --- | --- | --- | --- | --- |
| `audit-ci` | `audit` | `auto` | `artifact_verified` | `manual repo scoring, ad hoc score gates` | `agent/repo-score.json, agent/repo-score.md` |
| `proof-routing` | `proof` | `auto` | `artifact_verified` | `ad hoc proof lane selection, manual proof receipts` | `agent/repo-score.json, agent/repo-score.md, target/jankurai/repair-queue.jsonl` |
| `proofbind` | `proof` | `auto` | `artifact_verified` | `manual changed-surface routing, ad hoc proof obligation lists` | `target/jankurai/proofbind/surface-witness.json, target/jankurai/proofbind/obligations.json` |
| `proofmark-rust` | `proof` | `auto` | `artifact_verified` | `line-only coverage review, manual in-diff mutation review` | `target/jankurai/proofmark/proofmark-receipt.json, target/jankurai/proofmark/proof-receipt.json` |
| `copy-code` | `audit` | `auto` | `artifact_verified` | `ad hoc copy-code review, manual duplication triage` | `target/jankurai/copy-code.json, target/jankurai/copy-code.md` |
| `security` | `security` | `auto` | `artifact_verified` | `gitleaks, dependency review, SBOM/provenance` | `target/jankurai/security/evidence.json` |
| `ci-bad-behavior` | `security` | `auto` | `artifact_verified` | `mutable workflow refs, secret echo/debug workflow checks, non-blocking security scans` | `target/jankurai/language-bad-behavior.log` |
| `git-bad-behavior` | `audit` | `auto` | `artifact_verified` | `destructive git automation, force-push release scripts, hidden stash-based state` | `target/jankurai/language-bad-behavior.log` |
| `release-bad-behavior` | `release` | `auto` | `artifact_verified` | `manual release checklist, ad hoc tag and artifact review, manual provenance review` | `target/jankurai/language-bad-behavior.log` |
| `ux-qa` | `ux` | `auto` | `not_applicable` | `playwright, axe-core, visual baselines` | `target/jankurai/ux-qa.json` |
| `db-migration-analyze` | `db` | `auto` | `not_applicable` | `manual migration review` | `target/jankurai/migration-report.json` |
| `contract-drift` | `contract` | `auto` | `artifact_verified` | `handwritten contract drift checks, openapi diff` | `agent/repo-score.json, agent/repo-score.md` |
| `rust-witness` | `rust` | `auto` | `artifact_verified` | `manual witness graphing` | `target/jankurai/rust/witness-graph.json` |
| `vibe-coverage` | `audit` | `auto` | `not_applicable` | `manual vibe-coding coverage spreadsheet` | `target/jankurai/vibe-coverage.json, target/jankurai/vibe-coverage.md` |
| `coverage-evidence` | `proof` | `auto` | `not_applicable` | `manual coverage report review, ad hoc mutation survivor review` | `target/jankurai/coverage/coverage-audit.json, target/jankurai/coverage/coverage-audit.md` |
| `authz-matrix` | `security` | `auto` | `artifact_verified` | `manual authz matrix review` | `agent/repo-score.json, agent/repo-score.md` |
| `input-boundary` | `security` | `auto` | `artifact_verified` | `manual unsafe sink review` | `agent/repo-score.json, agent/repo-score.md` |
| `agent-tool-supply` | `security` | `auto` | `artifact_verified` | `manual MCP/tool trust review` | `agent/repo-score.json, agent/repo-score.md` |
| `release-readiness` | `release` | `auto` | `artifact_verified` | `manual launch checklist` | `agent/repo-score.json, agent/repo-score.md` |
| `cost-budget` | `release` | `auto` | `artifact_verified` | `manual spend review` | `agent/repo-score.json, agent/repo-score.md` |

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
   Reason: `Code shape and semantic surface` scored 8 below the standard floor of 85
   Fix: split large or ambiguous authored code into smaller semantic modules with focused tests
   Rerun: `just fast`
   Fingerprint: `sha256:8152831456e172dc6a902cfce17d7860da7ac1bc88978116a06f175e0c746e35`
   Evidence: largest authored code file: crates/redlinedb-sqlx/src/bridge.rs (692 LOC), code file exceeds 500 LOC, most code files stay under 300 LOC, copy-code advisory classes found: 57 (advisory only, no score impact)
2. `high` `security` `.github/workflows/jankurai.yml:24`
   Rule: `HLT-024-AGENT-TOOL-SUPPLY-GAP`
   Check: `HLT-024-AGENT-TOOL-SUPPLY-GAP:security` `hard` confidence `0.88`
   Route: TLR `Security, secrets, agency`, lane `security`, owner `ops`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `agent tool supply`
   Reason: agent tool supply-chain changes alter execution authority
   Fix: pin and review agent tools, MCP servers, hooks, and rule files; keep untrusted tool output separate from trusted policy
   Rerun: `just security`
   Fingerprint: `sha256:a0b183cccf351e772870fdea98ef049ff39ef4dae945f0e6a4dab65c82f50bd6`
   Evidence: runs-on: ubuntu-latest
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
4. `high` `context` `agent/owner-map.json`
   Rule: `HLT-003-OWNERLESS-PATH`
   Check: `HLT-003-OWNERLESS-PATH:context` `hard` confidence `0.88`
   Route: TLR `Context/setup`, lane `fast`, owner `agent`
   Docs: `agent/JANKURAI_STANDARD.md#ownership-boundaries`
   Reason: path `FUCKIT.md` has no owner-map route
   Fix: add the narrowest stable prefix for this path to `agent/owner-map.json`
   Rerun: `just fast`
   Fingerprint: `sha256:64d9977ab066c1cbfc61fc79f87a1414cd4a66001e4b7a672f79c0dafbe6edae`
   Evidence: FUCKIT.md
5. `high` `proof` `agent/test-map.json`
   Rule: `HLT-004-UNMAPPED-PROOF`
   Check: `HLT-004-UNMAPPED-PROOF:proof` `hard` confidence `0.88`
   Route: TLR `Verification`, lane `fast`, owner `agent`
   Docs: `agent/JANKURAI_STANDARD.md#proof-lanes`
   Reason: path `FUCKIT.md` has no test-map proof route
   Fix: add the narrowest stable prefix and runnable proof command to `agent/test-map.json`
   Rerun: `just fast`
   Fingerprint: `sha256:881d1d052b64d65a4b8f255bee57b13815f43fd7077410cc9cc81335b037e493`
   Evidence: FUCKIT.md
6. `high` `vibe` `crates/cli/src/dot/schema.rs:135`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `cli-shell`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: fallback soup detected in product code
   Fix: collapse fallback chains into explicit typed states with bounded retry policy, telemetry, and documented repair guidance
   Rerun: `just fast`
   Fingerprint: `sha256:0a9741bbc5b40ac9c6b77fdf4e4033668a7722abc9970ffd3f5fe02c9a92bbed`
   Evidence: crates/cli/src/dot/schema.rs:135 let sql: String = row.get(4).unwrap_or_default();
7. `high` `vibe` `crates/redlinedb-sqlx/src/bridge.rs:22`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `public-rust-facade`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `dummy` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:5ca4807672179e81b16eddd467692b6c70124f1ee56f5d728ab54149d34dba77`
   Evidence: crates/redlinedb-sqlx/src/bridge.rs:22, future-hostile/dead-language term `dummy` appears
8. `high` `vibe` `crates/redlinedb-sqlx/src/lib.rs:8`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `public-rust-facade`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `dummy` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:5f553511c216690e7dffbd4570acf538bcfc1ede059107733095f5d2c2e39740`
   Evidence: crates/redlinedb-sqlx/src/lib.rs:8, future-hostile/dead-language term `dummy` appears
9. `high` `security` `crates/sql/src/connection/database.rs:356`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `sql-parser-planner-executor`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.static-mut`
   Reason: global mutation proof is missing
   Fix: replace the mutable static with explicit synchronization or scoped ownership
   Rerun: `just fast`
   Fingerprint: `sha256:3d17f30702010d0e4c23b67f620c46ec5bcd12c5de36dbf289b204c2d84956a8`
   Evidence: detector=static mut, proof-window=NearbySafetyComment, snippet=fn ephemeral_registry() -> &'static Mutex<EphemeralRegistry> {
10. `medium` `proof` `crates/sql/tests/parity_coverage.rs:304`
   Rule: `HLT-027-HUMAN-REVIEW-EVIDENCE-GAP`
   Check: `HLT-027-HUMAN-REVIEW-EVIDENCE-GAP:proof` `soft` confidence `0.88`
   Route: TLR `Repair`, lane `audit`, owner `sql-parser-planner-executor`
   Docs: `docs/testing.md`
   Matched term: `review evidence`
   Reason: proof and review claims need receipts
   Fix: attach raw CI logs, review receipts, and replayable commands instead of accepting claims or summaries
   Rerun: `just score`
   Fingerprint: `sha256:4859f8ec7a224d86b579f18d97eed484cb6cba9d05ed4db7129a666d8a60997f`
   Evidence: // rejected instead of returning a fabricated `0` row.

## Policy

- Policy file: `./agent/audit-policy.toml`
- Minimum score: `85`
- Fail on: `critical, high`

## Agent Fix Queue

1. `high` `HLT-004-UNMAPPED-PROOF` `agent/test-map.json` - add the narrowest stable prefix and runnable proof command to `agent/test-map.json`
   Route: `Verification`/`fast`
2. `medium` `HLT-018-PERF-CONCURRENCY-DRIFT` `Justfile` - add fast deterministic build/test targets, caches, and narrow proof lanes for agent iteration
   Route: `Verification`/`fast`
3. `medium` `HLT-027-HUMAN-REVIEW-EVIDENCE-GAP` `crates/sql/tests/parity_coverage.rs` - attach raw CI logs, review receipts, and replayable commands instead of accepting claims or summaries
   Route: `Repair`/`audit`
4. `high` `HLT-003-OWNERLESS-PATH` `agent/owner-map.json` - add the narrowest stable prefix for this path to `agent/owner-map.json`
   Route: `Context/setup`/`fast`
5. `high` `HLT-024-AGENT-TOOL-SUPPLY-GAP` `.github/workflows/jankurai.yml` - pin and review agent tools, MCP servers, hooks, and rule files; keep untrusted tool output separate from trusted policy
   Route: `Security, secrets, agency`/`security`
6. `high` `HLT-001-DEAD-MARKER` `crates/cli/src/dot/schema.rs` - collapse fallback chains into explicit typed states with bounded retry policy, telemetry, and documented repair guidance
   Route: `Entropy`/`fast`
7. `high` `HLT-001-DEAD-MARKER` `crates/redlinedb-sqlx/src/bridge.rs` - remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Route: `Entropy`/`fast`
8. `high` `HLT-001-DEAD-MARKER` `crates/redlinedb-sqlx/src/lib.rs` - remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Route: `Entropy`/`fast`
9. `high` `HLT-029-RUST-BAD-BEHAVIOR` `crates/sql/src/connection/database.rs` - replace the mutable static with explicit synchronization or scoped ownership
   Route: `Security, secrets, agency`/`fast`
10. `medium` `HLT-001-DEAD-MARKER` `.` - split large or ambiguous authored code into smaller semantic modules with focused tests
   Route: `Entropy`/`fast`
