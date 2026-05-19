# jankurai Repo Score

- Standard: `jankurai`
- Auditor: `1.5.1`
- Schema: `1.7.0`
- Paper edition: `2026.05-ed8`
- Target stack ID: `rust-ts-vite-react-postgres-bounded-python`
- Target stack: `Rust core + TypeScript/React/Vite + PostgreSQL + generated contracts + exception-only Python AI/data service`
- Repo: `.`
- Run ID: `1779225828`
- Started at: `1779225828`
- Elapsed: `7017` ms
- Scope: `full`
- Raw score: `92`
- Final score: `78`
- Decision: `advisory`
- Minimum score: `85`
- Caps applied: `input-boundary-gap, agent-tool-supply-chain-gap`

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
| `input-boundary-gap` | 78 | yes |
| `agent-tool-supply-chain-gap` | 78 | yes |
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

- Status: `review` hard=`0` warning=`59` files=`284`
- Policy: min-lines=`10` min-tokens=`100` max-findings=`50` include-tests=`false` strict=`false`
- Duplicate volume: lines=`127` tokens=`370` bytes=`3561`

- Notes:
  - hard classes are limited to exact active-source file matches and substantial exact same-name units
  - warning classes include same-body different-name units and token/block duplication
  - tests, fixtures, stories, config, Docker, and migrations are omitted unless --include-tests is set
  - showing the top 50 classes and omitting 9 lower-ranked classes

| Kind | Severity | Language | Lines | Tokens | Instances | Reason |
| --- | --- | --- | ---: | ---: | --- | --- |
| `ExactUnitSameName` | `Warning` | `rust` | 13 | 42 | `crates/sql/src/exec/cross_db.rs:194-207, crates/sql/src/exec/cte.rs:137-150, crates/sql/src/exec/view.rs:179-192` | `same-name semantic unit copied across multiple files` |
| `ExactUnitSameName` | `Warning` | `rust` | 16 | 37 | `crates/sql/src/exec/agg/select.rs:82-98, crates/sql/src/planner/access/projection.rs:150-166` | `same-name semantic unit copied across multiple files` |
| `ExactUnitSameName` | `Warning` | `rust` | 11 | 57 | `crates/sql/src/datetime/format.rs:75-86, crates/sql/src/datetime/modifiers.rs:145-156` | `same-name semantic unit copied across multiple files` |
| `ExactUnitSameName` | `Warning` | `rust` | 5 | 9 | `crates/sql/src/exec/json_tv.rs:218-223, crates/sql/src/exec/json_tv.rs:273-278, crates/sql/src/exec/pragma_tv.rs:192-197` | `same-name semantic unit copied across multiple files` |
| `ExactUnitSameName` | `Warning` | `rust` | 5 | 9 | `crates/sql/src/exec/pragma_tv.rs:70-75, crates/sql/src/exec/pragma_tv.rs:102-107, crates/sql/src/exec/pragma_tv.rs:128-133` | `same-name semantic unit copied across multiple files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 2 | `crates/redlinedb/src/value.rs:43-44, crates/redlinedb/src/value.rs:57-58, crates/redlinedb/src/value.rs:64-65, crates/redlinedb/src/value.rs:71-72, crates/redlinedb/src/value.rs:78-79, crates/sql/src/exec/expr/scalar/row/model.rs:64-65, crates/sql/src/exec/expr/scalar/row/model.rs:77-78, crates/sql/src/exec/expr/scalar/row/model.rs:87-88` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 6 | `crates/redlinedb-sqlx/src/bridge.rs:219-220, crates/redlinedb-sqlx/src/bridge.rs:298-299, crates/redlinedb-sqlx/src/bridge.rs:303-304, crates/redlinedb-sqlx/src/bridge.rs:327-328, crates/redlinedb-sqlx/src/bridge.rs:342-343, crates/redlinedb-sqlx/src/bridge.rs:357-358, crates/redlinedb-sqlx/src/bridge.rs:447-448` | `same body appears under different names across files` |
| `ExactUnitSameName` | `Warning` | `rust` | 6 | 20 | `crates/sql/src/exec/agg/select.rs:9-15, crates/sql/src/planner/access/projection.rs:94-100` | `same-name semantic unit copied across multiple files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 2 | `crates/kernel/src/catalog/record.rs:152-153, crates/kernel/src/catalog/stats/wire.rs:162-163, crates/kernel/src/catalog/stats/wire.rs:173-174, crates/redlinedb-sqlx/src/bridge.rs:636-637, crates/redlinedb-sqlx/src/bridge.rs:733-734, crates/redlinedb/src/value_conv.rs:261-262, crates/sql/src/exec/json_tv.rs:231-232` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 0 | `crates/kernel/src/catalog/ddl.rs:250-250, crates/kernel/src/failpoints/mod.rs:41-42, crates/kernel/src/integrity/equivalence.rs:214-214, crates/kernel/src/integrity/page_csum.rs:107-107, crates/redlinedb-sqlx/src/bridge.rs:249-250, crates/redlinedb-sqlx/src/bridge.rs:373-374` | `same body appears under different names across files` |
| `ExactUnitSameName` | `Warning` | `rust` | 4 | 13 | `crates/kernel/src/index/locks.rs:200-204, crates/sql/src/session.rs:269-273` | `same-name semantic unit copied across multiple files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 2 | 5 | `crates/kernel/src/format/bytes.rs:44-46, crates/kernel/src/format/bytes.rs:49-51, crates/kernel/src/format/bytes.rs:54-56` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 4 | 9 | `crates/sql/src/exec/expr/scalar/row/lookup.rs:56-60, crates/sql/src/exec/expr/scalar/row/lookup.rs:117-121` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 2 | 3 | `crates/kernel/src/format/page.rs:102-104, crates/kernel/src/storage/control.rs:147-149, crates/kernel/src/storage/tx_status_checkpoint.rs:147-149` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 4 | 5 | `crates/redlinedb/src/connection.rs:90-94, crates/redlinedb/src/connection.rs:105-109` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 4 | `crates/redlinedb-sqlx/src/bridge.rs:205-206, crates/redlinedb-sqlx/src/bridge.rs:212-213, crates/redlinedb-sqlx/src/bridge.rs:284-285, crates/redlinedb-sqlx/src/bridge.rs:291-292` | `same body appears under different names across files` |
| `ExactUnitSameName` | `Warning` | `rust` | 3 | 4 | `crates/kernel/src/vector/flat.rs:57-60, crates/kernel/src/vector/hnsw/searcher.rs:47-50` | `same-name semantic unit copied across multiple files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 6 | `crates/redlinedb-sqlx/src/driver.rs:399-400, crates/redlinedb-sqlx/src/driver.rs:414-415, crates/redlinedb-sqlx/src/driver.rs:429-430` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 4 | `crates/redlinedb-sqlx/src/bridge.rs:245-246, crates/redlinedb-sqlx/src/bridge.rs:251-252, crates/redlinedb-sqlx/src/bridge.rs:375-376` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 3 | `crates/redlinedb/src/connection.rs:228-229, crates/redlinedb/src/connection.rs:237-238, crates/redlinedb/src/connection.rs:247-248` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 2 | 5 | `crates/kernel/src/json/path_bytecode.rs:138-140, crates/sql/src/parser.rs:188-190` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 2 | `crates/redlinedb/src/connection.rs:52-53, crates/redlinedb/src/connection.rs:118-119, crates/redlinedb/src/connection.rs:124-125` | `same body appears under different names across files` |
| `ExactUnitSameName` | `Warning` | `rust` | 2 | 3 | `crates/redlinedb-tokio/src/lib.rs:268-270, crates/redlinedb/src/pool.rs:199-201` | `same-name semantic unit copied across multiple files` |
| `ExactUnitSameName` | `Warning` | `rust` | 2 | 3 | `crates/redlinedb-tokio/src/lib.rs:263-265, crates/redlinedb/src/pool.rs:204-206` | `same-name semantic unit copied across multiple files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 2 | 1 | `crates/kernel/src/failpoints/mod.rs:65-67, crates/kernel/src/failpoints/mod.rs:109-111` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 2 | 1 | `crates/bench/src/certify/scheduler/dispatch.rs:196-198, crates/kernel/src/engine/runtime/commit.rs:30-32` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 7 | `crates/sql/src/exec/expr/scalar/row/lookup.rs:161-162, crates/sql/src/exec/expr/scalar/row/lookup.rs:165-166` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 7 | `crates/redlinedb/src/value_conv.rs:245-246, crates/redlinedb/src/value_conv.rs:253-254` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 6 | `crates/redlinedb/src/connection.rs:211-212, crates/redlinedb/src/statement.rs:204-205` | `same body appears under different names across files` |
| `ExactUnitSameName` | `Warning` | `rust` | 1 | 6 | `crates/redlinedb-sqlx/src/driver.rs:159-160, crates/redlinedb-sqlx/src/driver.rs:187-188` | `same-name semantic unit copied across multiple files` |
| `ExactUnitSameName` | `Warning` | `rust` | 1 | 6 | `crates/redlinedb-sqlx/src/bridge.rs:219-220, crates/redlinedb-sqlx/src/bridge.rs:298-299` | `same-name semantic unit copied across multiple files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 6 | `crates/redlinedb/src/connection.rs:69-70, crates/redlinedb/src/connection.rs:75-76` | `same body appears under different names across files` |
| `ExactUnitSameName` | `Warning` | `rust` | 1 | 5 | `crates/redlinedb-sqlx/src/driver.rs:155-156, crates/redlinedb-sqlx/src/driver.rs:183-184` | `same-name semantic unit copied across multiple files` |
| `ExactUnitSameName` | `Warning` | `rust` | 1 | 5 | `crates/kernel/src/json/wire/iter.rs:20-21, crates/kernel/src/json/wire/iter.rs:91-92` | `same-name semantic unit copied across multiple files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 5 | `crates/redlinedb/src/value.rs:331-332, crates/redlinedb/src/value.rs:343-344` | `same body appears under different names across files` |
| `ExactUnitSameName` | `Warning` | `rust` | 1 | 4 | `crates/redlinedb/src/statement.rs:142-143, crates/redlinedb/src/statement.rs:226-227` | `same-name semantic unit copied across multiple files` |
| `ExactUnitSameName` | `Warning` | `rust` | 1 | 4 | `crates/redlinedb/src/statement.rs:150-151, crates/redlinedb/src/statement.rs:234-235` | `same-name semantic unit copied across multiple files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 4 | `crates/domain/src/error.rs:73-74, crates/domain/src/error.rs:91-92` | `same body appears under different names across files` |
| `ExactUnitSameName` | `Warning` | `rust` | 1 | 4 | `crates/redlinedb-sqlx/src/bridge.rs:251-252, crates/redlinedb-sqlx/src/bridge.rs:375-376` | `same-name semantic unit copied across multiple files` |
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
| Code shape and semantic surface | 12 | 65 | 7.80 | largest authored code file: crates/redlinedb-sqlx/src/bridge.rs (1000 LOC); code file exceeds 500 LOC |
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
   Reason: `Code shape and semantic surface` scored 65 below the standard floor of 85
   Fix: split large or ambiguous authored code into smaller semantic modules with focused tests
   Rerun: `just fast`
   Fingerprint: `sha256:d22e50a5ffeed3bd53c1f72fc7651b23e9fb87744d220769ddcafae5f05e0e58`
   Evidence: largest authored code file: crates/redlinedb-sqlx/src/bridge.rs (1000 LOC), code file exceeds 500 LOC, most code files stay under 300 LOC, copy-code advisory classes found: 59 (advisory only, no score impact)
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
4. `high` `security` `crates/sql/src/exec/expr/predicate.rs:21`
   Rule: `HLT-023-INPUT-BOUNDARY-GAP`
   Check: `HLT-023-INPUT-BOUNDARY-GAP:security` `hard` confidence `0.88`
   Route: TLR `Security, secrets, agency`, lane `security`, owner `sql-parser-planner-executor`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `eval(`
   Reason: input handling risk needs deterministic negative tests
   Fix: replace unsafe sinks with typed schemas, parameterized APIs, allowlists, or sandboxed execution plus negative tests
   Rerun: `just security`
   Fingerprint: `sha256:8c2123f8c21a6dd70d0016d934f6c5a717a81e1522c5c4be29d8a1483118f36d`
   Evidence: let operand = eval(operand)?;

## Policy

- Policy file: `./agent/audit-policy.toml`
- Minimum score: `85`
- Fail on: `critical, high`

## Agent Fix Queue

1. `medium` `HLT-018-PERF-CONCURRENCY-DRIFT` `Justfile` - add fast deterministic build/test targets, caches, and narrow proof lanes for agent iteration
   Route: `Verification`/`fast`
2. `high` `HLT-024-AGENT-TOOL-SUPPLY-GAP` `.github/workflows/jankurai.yml` - pin and review agent tools, MCP servers, hooks, and rule files; keep untrusted tool output separate from trusted policy
   Route: `Security, secrets, agency`/`security`
3. `high` `HLT-023-INPUT-BOUNDARY-GAP` `crates/sql/src/exec/expr/predicate.rs` - replace unsafe sinks with typed schemas, parameterized APIs, allowlists, or sandboxed execution plus negative tests
   Route: `Security, secrets, agency`/`security`
4. `medium` `HLT-001-DEAD-MARKER` `.` - split large or ambiguous authored code into smaller semantic modules with focused tests
   Route: `Entropy`/`fast`
