# jankurai Repo Score

- Standard: `jankurai`
- Auditor: `1.5.1`
- Schema: `1.9.0`
- Paper edition: `2026.05-ed8`
- Target stack ID: `rust-ts-vite-react-postgres-bounded-python`
- Target stack: `Rust core + TypeScript/React/Vite + PostgreSQL + generated contracts + exception-only Python AI/data service`
- Repo: `.`
- Run ID: `1779372388`
- Started at: `1779372388`
- Elapsed: `14085` ms
- Scope: `full`
- Raw score: `72`
- Final score: `64`
- Decision: `advisory`
- Minimum score: `85`
- Caps applied: `python-direct-product-truth-or-db-ownership, non-optimal-product-language-found, fallback-soup-in-product-code, future-hostile-dead-language-in-product-code, severe-duplication-in-product-code, input-boundary-gap, agent-tool-supply-chain-gap, rust-bad-behavior, repo-rot-bad-behavior`

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
| `vibe-placeholders-in-product-code` | 68 | no |
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
| `authz-or-data-isolation-gap` | 78 | no |
| `input-boundary-gap` | 78 | yes |
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
| `repo-rot-bad-behavior` | 88 | yes |
| `comment-hygiene-dangerous-residue` | 72 | no |
| `ci-local-parity` | 70 | no |

## Copy-Code Redundancy

- Status: `review` hard=`1` warning=`100` files=`1510`
- Policy: min-lines=`10` min-tokens=`100` max-findings=`50` include-tests=`false` strict=`false`
- Duplicate volume: lines=`282` tokens=`857` bytes=`8396`

- Notes:
  - hard classes are limited to exact active-source file matches and substantial exact same-name units
  - warning classes include same-body different-name units and token/block duplication
  - tests, fixtures, stories, config, Docker, and migrations are omitted unless --include-tests is set
  - showing the top 50 classes and omitting 51 lower-ranked classes

| Kind | Severity | Language | Lines | Tokens | Instances | Reason |
| --- | --- | --- | ---: | ---: | --- | --- |
| `ExactUnitSameName` | `Hard` | `rust` | 47 | 172 | `crates/bench/src/feature_workloads.rs:639-686, crates/bench/src/workload.rs:459-506` | `same-name semantic unit copied across multiple files` |
| `ExactUnitSameName` | `Warning` | `rust` | 13 | 42 | `crates/sql/src/exec/cross_db.rs:194-207, crates/sql/src/exec/cte.rs:137-150, crates/sql/src/exec/view.rs:179-192` | `same-name semantic unit copied across multiple files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 6 | 14 | `crates/cli/src/render.rs:187-193, crates/cli/src/render.rs:276-282, crates/cli/src/render.rs:301-307, crates/cli/src/render.rs:334-340, crates/cli/src/render.rs:363-369` | `same body appears under different names across files` |
| `ExactUnitSameName` | `Warning` | `rust` | 22 | 66 | `crates/bench/src/feature_workloads.rs:688-710, crates/bench/src/workload.rs:508-530` | `same-name semantic unit copied across multiple files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 2 | 3 | `crates/cli/src/dot/control.rs:109-111, crates/cli/src/dot/control.rs:113-115, crates/cli/src/dot/control.rs:117-119, crates/cli/src/dot/control.rs:121-123, crates/cli/src/dot/control.rs:125-127, crates/cli/src/dot/control.rs:129-131, crates/cli/src/dot/control.rs:133-135, crates/cli/src/dot/control.rs:137-139, crates/cli/src/dot/control.rs:141-143, crates/cli/src/dot/control.rs:145-147, crates/cli/src/dot/control.rs:149-151` | `same body appears under different names across files` |
| `ExactUnitSameName` | `Warning` | `rust` | 16 | 37 | `crates/sql/src/exec/agg/select.rs:83-99, crates/sql/src/planner/access/projection.rs:153-169` | `same-name semantic unit copied across multiple files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 15 | 36 | `crates/sql/src/exec/json_tv.rs:37-52, crates/sql/src/json/scalar.rs:320-335` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 2 | `crates/kernel/src/catalog/record.rs:152-153, crates/kernel/src/catalog/stats/wire.rs:162-163, crates/kernel/src/catalog/stats/wire.rs:173-174, crates/kernel/src/catalog/store.rs:888-889, crates/kernel/src/catalog/store.rs:898-899, crates/kernel/src/catalog/store.rs:908-909, crates/kernel/src/catalog/store.rs:918-919, crates/kernel/src/catalog/store.rs:928-929, crates/kernel/src/catalog/store.rs:951-952, crates/redlinedb-sqlx/src/bridge/runtime.rs:390-391, crates/redlinedb-sqlx/src/bridge/runtime.rs:487-488, crates/redlinedb/src/value_conv.rs:261-262, crates/sql/src/exec/index_access.rs:732-733, crates/sql/src/exec/json_tv.rs:231-232` | `same body appears under different names across files` |
| `ExactUnitSameName` | `Warning` | `rust` | 11 | 57 | `crates/sql/src/datetime/format.rs:75-86, crates/sql/src/datetime/modifiers.rs:145-156` | `same-name semantic unit copied across multiple files` |
| `ExactUnitSameName` | `Warning` | `rust` | 5 | 9 | `crates/sql/src/exec/json_tv.rs:218-223, crates/sql/src/exec/json_tv.rs:273-278, crates/sql/src/exec/pragma_tv.rs:192-197` | `same-name semantic unit copied across multiple files` |
| `ExactUnitSameName` | `Warning` | `rust` | 5 | 9 | `crates/sql/src/exec/pragma_tv.rs:70-75, crates/sql/src/exec/pragma_tv.rs:102-107, crates/sql/src/exec/pragma_tv.rs:128-133` | `same-name semantic unit copied across multiple files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 2 | `crates/bench/src/config.rs:413-414, crates/bench/src/config.rs:434-435, crates/redlinedb/src/value.rs:43-44, crates/redlinedb/src/value.rs:57-58, crates/redlinedb/src/value.rs:64-65, crates/redlinedb/src/value.rs:71-72, crates/redlinedb/src/value.rs:78-79, crates/sql/src/exec/expr/scalar/row/model.rs:64-65, crates/sql/src/exec/expr/scalar/row/model.rs:77-78, crates/sql/src/exec/expr/scalar/row/model.rs:87-88` | `same body appears under different names across files` |
| `ExactUnitSameName` | `Warning` | `rust` | 7 | 15 | `crates/sql/src/parser/select.rs:1056-1063, crates/sql/src/parser/select.rs:1127-1134` | `same-name semantic unit copied across multiple files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 6 | `crates/redlinedb-sqlx/src/bridge/options.rs:223-224, crates/redlinedb-sqlx/src/bridge/runtime.rs:52-53, crates/redlinedb-sqlx/src/bridge/runtime.rs:57-58, crates/redlinedb-sqlx/src/bridge/runtime.rs:81-82, crates/redlinedb-sqlx/src/bridge/runtime.rs:96-97, crates/redlinedb-sqlx/src/bridge/runtime.rs:111-112, crates/redlinedb-sqlx/src/bridge/runtime.rs:201-202` | `same body appears under different names across files` |
| `ExactUnitSameName` | `Warning` | `rust` | 6 | 20 | `crates/sql/src/exec/agg/select.rs:9-15, crates/sql/src/planner/access/projection.rs:94-100` | `same-name semantic unit copied across multiple files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 0 | `crates/kernel/src/catalog/ddl.rs:248-248, crates/kernel/src/failpoints/mod.rs:41-42, crates/kernel/src/integrity/equivalence.rs:214-214, crates/kernel/src/integrity/page_csum.rs:107-107, crates/redlinedb-sqlx/src/bridge/options.rs:253-254, crates/redlinedb-sqlx/src/bridge/runtime.rs:127-128, crates/sql/src/connection/session.rs:700-700` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 5 | 15 | `crates/sql/src/exec/mod.rs:830-835, crates/sql/src/exec/mod.rs:845-850` | `same body appears under different names across files` |
| `ExactUnitSameName` | `Warning` | `rust` | 5 | 13 | `crates/sql/src/exec/agg/select.rs:101-106, crates/sql/src/planner/access/projection.rs:171-176` | `same-name semantic unit copied across multiple files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 4 | `crates/sql/src/parser.rs:92-93, crates/sql/src/parser.rs:221-222, crates/sql/src/parser.rs:399-400, crates/sql/src/parser.rs:461-462, crates/sql/src/parser.rs:469-470` | `same body appears under different names across files` |
| `ExactUnitSameName` | `Warning` | `rust` | 4 | 13 | `crates/kernel/src/index/locks.rs:200-204, crates/sql/src/session.rs:273-277` | `same-name semantic unit copied across multiple files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 2 | 5 | `crates/kernel/src/format/bytes.rs:44-46, crates/kernel/src/format/bytes.rs:49-51, crates/kernel/src/format/bytes.rs:54-56` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 4 | 9 | `crates/sql/src/exec/expr/scalar/row/lookup.rs:56-60, crates/sql/src/exec/expr/scalar/row/lookup.rs:117-121` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 4 | 8 | `crates/cli/src/render.rs:559-563, crates/cli/src/render.rs:623-627` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 2 | 3 | `crates/kernel/src/format/page.rs:102-104, crates/kernel/src/storage/control.rs:147-149, crates/kernel/src/storage/tx_status_checkpoint.rs:147-149` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 4 | 5 | `crates/redlinedb/src/connection.rs:112-116, crates/redlinedb/src/connection.rs:127-131` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 1 | `crates/bench/src/sqlite_parity/engine.rs:302-303, crates/bench/src/sqlite_parity/report_gen.rs:1246-1247, crates/bench/src/sqlite_parity/report_gen.rs:1301-1302, crates/cli/src/dot/control.rs:299-300, crates/cli/src/render.rs:637-638` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 4 | `crates/kernel/src/vector/diskann/sectors.rs:419-420, crates/redlinedb/src/value_conv.rs:336-337, crates/redlinedb/src/value_conv.rs:357-358, crates/redlinedb/src/value_conv.rs:378-379` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 3 | 10 | `crates/sql/src/exec/index_batch.rs:450-453, crates/sql/src/exec/index_batch.rs:458-461` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 3 | 7 | `crates/sql/src/parser.rs:697-700, crates/sql/src/parser.rs:713-716` | `same body appears under different names across files` |
| `ExactUnitSameName` | `Warning` | `rust` | 3 | 5 | `crates/ffi/src/sqlite3_api/hooks.rs:72-75, crates/ffi/src/sqlite3_api/hooks_fire.rs:37-40` | `same-name semantic unit copied across multiple files` |
| `ExactUnitSameName` | `Warning` | `rust` | 3 | 4 | `crates/kernel/src/vector/flat.rs:57-60, crates/kernel/src/vector/hnsw/searcher.rs:47-50` | `same-name semantic unit copied across multiple files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 1 | `crates/bench/src/sqlite_parity/report_gen.rs:1117-1118, crates/bench/src/sqlite_parity/report_gen.rs:1124-1125, crates/bench/src/sqlite_parity/report_gen.rs:1131-1132, crates/bench/src/sqlite_parity/report_gen.rs:1226-1227` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 6 | `crates/redlinedb-sqlx/src/driver.rs:399-400, crates/redlinedb-sqlx/src/driver.rs:414-415, crates/redlinedb-sqlx/src/driver.rs:429-430` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 6 | `crates/sql/src/json/path.rs:543-544, crates/sql/src/json/path.rs:551-552, crates/sql/src/json/path.rs:559-560` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 5 | `crates/bench/src/sqlite_parity/engine.rs:313-314, crates/bench/src/sqlite_parity/engine.rs:357-358, crates/bench/src/sqlite_parity/engine.rs:368-369` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 4 | `crates/redlinedb-sqlx/src/bridge/options.rs:249-250, crates/redlinedb-sqlx/src/bridge/options.rs:255-256, crates/redlinedb-sqlx/src/bridge/runtime.rs:129-130` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 4 | `crates/kernel/src/catalog/stats/wire.rs:198-199, crates/kernel/src/catalog/store.rs:813-814, crates/kernel/src/catalog/store.rs:1005-1006` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 3 | `crates/redlinedb/src/connection.rs:250-251, crates/redlinedb/src/connection.rs:259-260, crates/redlinedb/src/connection.rs:269-270` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 3 | `crates/kernel/src/catalog/store.rs:974-975, crates/kernel/src/catalog/store.rs:982-983, crates/kernel/src/catalog/store.rs:990-991` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 3 | `crates/kernel/src/catalog/store.rs:599-600, crates/kernel/src/catalog/store.rs:756-757, crates/kernel/src/catalog/store.rs:772-773` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 2 | 5 | `crates/bench/src/engine/mod.rs:204-206, crates/bench/src/workload.rs:449-451` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 2 | `crates/redlinedb/src/connection.rs:52-53, crates/redlinedb/src/connection.rs:140-141, crates/redlinedb/src/connection.rs:146-147` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 2 | 4 | `crates/sql/src/planner/helpers.rs:484-486, crates/sql/src/planner/helpers.rs:488-490` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 2 | `crates/bench/src/config.rs:650-651, crates/redlinedb/src/statement.rs:180-181, crates/redlinedb/src/value.rs:226-227` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 2 | 4 | `crates/sql/src/planner/helpers.rs:461-463, crates/sql/src/planner/helpers.rs:465-467` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 2 | `crates/kernel/src/index/mod.rs:332-333, crates/kernel/src/index/mod.rs:404-405, crates/kernel/src/index/mod.rs:730-731` | `same body appears under different names across files` |
| `ExactUnitSameName` | `Warning` | `rust` | 2 | 3 | `crates/redlinedb-tokio/src/lib.rs:268-270, crates/redlinedb/src/pool.rs:199-201` | `same-name semantic unit copied across multiple files` |
| `ExactUnitSameName` | `Warning` | `rust` | 2 | 3 | `crates/redlinedb-tokio/src/lib.rs:263-265, crates/redlinedb/src/pool.rs:204-206` | `same-name semantic unit copied across multiple files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 2 | 2 | `crates/bench/src/process_metrics.rs:150-152, crates/bench/src/process_metrics.rs:155-157` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 2 | 1 | `crates/kernel/src/failpoints/mod.rs:65-67, crates/kernel/src/failpoints/mod.rs:109-111` | `same body appears under different names across files` |

## Dimensions

| Dimension | Weight | Score | Weighted | Evidence |
| --- | ---: | ---: | ---: | --- |
| Ownership and navigation surface | 13 | 100 | 13.00 | root `AGENTS.md` present; `CODEOWNERS` present |
| Contract and boundary integrity | 13 | 88 | 11.44 | contract surface found; generated contract artifacts found |
| Proof lanes and test routing | 12 | 98 | 11.76 | one-command setup/validation lane found; deterministic fast lane found |
| Security and supply-chain posture | 12 | 86 | 10.32 | lockfile present; secret or dependency scan tooling found |
| Code shape and semantic surface | 12 | 0 | 0.00 | largest authored code file: crates/bench/src/sqlite_parity/report_gen.rs (1605 LOC); code file exceeds 500 LOC |
| Data truth and workflow safety | 8 | 85 | 6.80 | database surface present; migration directory present |
| Observability and repair evidence | 8 | 88 | 7.04 | observability libraries or patterns found; ops/observability directory present |
| Context economy and agent instructions | 7 | 91 | 6.37 | root `AGENTS.md` present; root `AGENTS.md` stays short |
| Jankurai tool adoption and CI replacement | 7 | 25 | 1.75 | control-plane files present; applicable=16 |
| Python containment and polyglot hygiene | 4 | 0 | 0.00 | Python appears outside the allowed roots: ops/ci/filter-generated-sarif.py; non-optimal product language marker: crates/ffi/include/sqlite3.h |
| Build speed signals | 4 | 80 | 3.20 | build acceleration markers found; targeted test/build commands found |

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
- Applicable tools: `16`
- Configured: `0`
- CI evidence: `4`
- Artifact verified: `0`
- Replaced count: `4`
- Missing CI evidence: `audit-ci, proof-routing, proofbind, proofmark-rust, copy-code, security, ci-bad-behavior, git-bad-behavior, release-bad-behavior, contract-drift, rust-witness, authz-matrix, input-boundary, agent-tool-supply, release-readiness, cost-budget`

| Tool | Category | Mode | Status | Replaced | Artifacts |
| --- | --- | --- | --- | --- | --- |
| `audit-ci` | `audit` | `auto` | `missing` | `manual repo scoring, ad hoc score gates` | `agent/repo-score.json, agent/repo-score.md` |
| `proof-routing` | `proof` | `auto` | `missing` | `ad hoc proof lane selection, manual proof receipts` | `agent/repo-score.json, agent/repo-score.md, target/jankurai/repair-queue.jsonl` |
| `proofbind` | `proof` | `auto` | `missing` | `manual changed-surface routing, ad hoc proof obligation lists` | `target/jankurai/proofbind/surface-witness.json, target/jankurai/proofbind/obligations.json` |
| `proofmark-rust` | `proof` | `auto` | `missing` | `line-only coverage review, manual in-diff mutation review` | `target/jankurai/proofmark/proofmark-receipt.json, target/jankurai/proofmark/proof-receipt.json` |
| `copy-code` | `audit` | `auto` | `missing` | `ad hoc copy-code review, manual duplication triage` | `target/jankurai/copy-code.json, target/jankurai/copy-code.md` |
| `security` | `security` | `auto` | `missing` | `gitleaks, dependency review, SBOM/provenance` | `target/jankurai/security/evidence.json` |
| `ci-bad-behavior` | `security` | `auto` | `ci_evidence` | `mutable workflow refs, secret echo/debug workflow checks, non-blocking security scans` | `target/jankurai/language-bad-behavior.log` |
| `git-bad-behavior` | `audit` | `auto` | `ci_evidence` | `destructive git automation, force-push release scripts, hidden stash-based state` | `target/jankurai/language-bad-behavior.log` |
| `release-bad-behavior` | `release` | `auto` | `ci_evidence` | `manual release checklist, ad hoc tag and artifact review, manual provenance review` | `target/jankurai/language-bad-behavior.log` |
| `ux-qa` | `ux` | `auto` | `not_applicable` | `playwright, axe-core, visual baselines` | `target/jankurai/ux-qa.json` |
| `db-migration-analyze` | `db` | `auto` | `not_applicable` | `manual migration review` | `target/jankurai/migration-report.json` |
| `contract-drift` | `contract` | `auto` | `missing` | `handwritten contract drift checks, openapi diff` | `agent/repo-score.json, agent/repo-score.md` |
| `rust-witness` | `rust` | `auto` | `ci_evidence` | `manual witness graphing` | `target/jankurai/rust/witness-graph.json` |
| `vibe-coverage` | `audit` | `auto` | `not_applicable` | `manual vibe-coding coverage spreadsheet` | `target/jankurai/vibe-coverage.json, target/jankurai/vibe-coverage.md` |
| `coverage-evidence` | `proof` | `auto` | `not_applicable` | `manual coverage report review, ad hoc mutation survivor review` | `target/jankurai/coverage/coverage-audit.json, target/jankurai/coverage/coverage-audit.md` |
| `authz-matrix` | `security` | `auto` | `missing` | `manual authz matrix review` | `agent/repo-score.json, agent/repo-score.md` |
| `input-boundary` | `security` | `auto` | `missing` | `manual unsafe sink review` | `agent/repo-score.json, agent/repo-score.md` |
| `agent-tool-supply` | `security` | `auto` | `missing` | `manual MCP/tool trust review` | `agent/repo-score.json, agent/repo-score.md` |
| `release-readiness` | `release` | `auto` | `missing` | `manual launch checklist` | `agent/repo-score.json, agent/repo-score.md` |
| `cost-budget` | `release` | `auto` | `missing` | `manual spend review` | `agent/repo-score.json, agent/repo-score.md` |

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
   Fingerprint: `sha256:e76a4d1a62080c6eb02e6323d88a9a06d6126881b3510e77f502583206343f8c`
   Evidence: largest authored code file: crates/bench/src/sqlite_parity/report_gen.rs (1605 LOC), code file exceeds 500 LOC, code file exceeds 1000 LOC, most code files stay under 300 LOC
2. `high` `security` `.github/workflows/jankurai.yml:18`
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
4. `high` `security` `crates/bench/sqlite_parity/cases/SQLITE_PARITY_001_SELECT_CORE_EXPRESSIONS.rs:22`
   Rule: `HLT-023-INPUT-BOUNDARY-GAP`
   Check: `HLT-023-INPUT-BOUNDARY-GAP:security` `hard` confidence `0.88`
   Route: TLR `Security, secrets, agency`, lane `security`, owner `tools`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `string sql`
   Reason: input handling risk needs deterministic negative tests
   Fix: replace unsafe sinks with typed schemas, parameterized APIs, allowlists, or sandboxed execution plus negative tests
   Rerun: `just security`
   Fingerprint: `sha256:1faeff16c70106c6d9b497d0156e37f0f2036139549ab61d01b9b37607dd1af9`
   Evidence: SELECT 1+2, 'a'||'b', 7/2, 7/2.0, 7%2, -5, +5;
5. `high` `vibe` `crates/bench/sqlite_parity/cases/SQLITE_PARITY_042_TEMP_TABLE_TEMP_SCHEMA.rs:13`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `tools`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `temp` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:e2f29eefc049edc0933cb989ee29eb0753183b83b6a0aeece6505275afc59882`
   Evidence: crates/bench/sqlite_parity/cases/SQLITE_PARITY_042_TEMP_TABLE_TEMP_SCHEMA.rs:13, future-hostile/dead-language term `temp` appears
6. `high` `vibe` `crates/bench/sqlite_parity/cases/SQLITE_PARITY_042_TEMP_TABLE_TEMP_SCHEMA.rs:21`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `tools`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `temp` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:2ff896240677e030d8467fcbc8ebe50fba8049c57f556c5b19dec87df8b2d918`
   Evidence: crates/bench/sqlite_parity/cases/SQLITE_PARITY_042_TEMP_TABLE_TEMP_SCHEMA.rs:21, future-hostile/dead-language term `temp` appears
7. `high` `vibe` `crates/bench/sqlite_parity/cases/SQLITE_PARITY_096_DBSTAT_OPTIONAL.rs:23`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `tools`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `temp` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:ef9286b96db5d4f08a9dd3401529d31a19388aeeea876b8eb1e508454ce8e21e`
   Evidence: crates/bench/sqlite_parity/cases/SQLITE_PARITY_096_DBSTAT_OPTIONAL.rs:23, future-hostile/dead-language term `temp` appears
8. `high` `vibe` `crates/bench/sqlite_parity/cases/SQLITE_PARITY_146_DOT_READ_TEMPFILE.rs:13`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `tools`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `temp` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:6d1748c630527e17f73359642d9e4682f0e192ed45e394383536949e10af34f3`
   Evidence: crates/bench/sqlite_parity/cases/SQLITE_PARITY_146_DOT_READ_TEMPFILE.rs:13, future-hostile/dead-language term `temp` appears
9. `high` `vibe` `crates/bench/sqlite_parity/cases/SQLITE_PARITY_147_DOT_IMPORT_CSV_TEMPFILE.rs:13`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `tools`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `temp` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:d6631129361c4efef625fd7a056da7b696a1f889c57eb634cd55eb0231a46d02`
   Evidence: crates/bench/sqlite_parity/cases/SQLITE_PARITY_147_DOT_IMPORT_CSV_TEMPFILE.rs:13, future-hostile/dead-language term `temp` appears
10. `high` `vibe` `crates/bench/sqlite_parity/cases/SQLITE_PARITY_148_DOT_OUTPUT_TEMPFILE.rs:13`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `tools`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `temp` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:564356f3556baed31a88358985f2fd67addae1b1b5187103e84e6dd92a475356`
   Evidence: crates/bench/sqlite_parity/cases/SQLITE_PARITY_148_DOT_OUTPUT_TEMPFILE.rs:13, future-hostile/dead-language term `temp` appears
11. `high` `vibe` `crates/bench/sqlite_parity/cases/SQLITE_PARITY_149_DOT_ONCE_TEMPFILE.rs:13`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `tools`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `temp` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:a42cad470efb357ee45418b23b2168b3951f875d159f45568c820e5cb4d65349`
   Evidence: crates/bench/sqlite_parity/cases/SQLITE_PARITY_149_DOT_ONCE_TEMPFILE.rs:13, future-hostile/dead-language term `temp` appears
12. `medium` `context` `crates/bench/sqlite_parity/cases/SQLITE_PARITY_150_DOT_BACKUP_RESTORE_TEMPFILE.rs:1`
   Rule: `HLT-040-REPO-ROT-BAD-BEHAVIOR`
   Check: `HLT-040-REPO-ROT-BAD-BEHAVIOR:context` `soft` confidence `0.88`
   Route: TLR `Context/setup`, lane `audit`, owner `tools`
   Docs: `docs/language-bad-behavior.md#web-security-and-repo-rot-detectors`
   Matched term: `repo-rot.path.fake-versioned-source`
   Reason: ambiguous old-looking active source makes agents and reviewers guess whether code is live
   Fix: delete the stale copy, move history to VCS/archive tooling, or document owner, proof lane, expiry, and migration plan
   Rerun: `just score`
   Fingerprint: `sha256:9b80ccf13013d19c92569bafe76f96718badf72ad32430069c488111442882b6`
   Evidence: detector=repo-rot.path.fake-versioned-source, path=crates/bench/sqlite_parity/cases/SQLITE_PARITY_150_DOT_BACKUP_RESTORE_TEMPFILE.rs, line=1, proof_window=None, snippet=// Auto-generated SQLite parity case.
13. `high` `vibe` `crates/bench/sqlite_parity/cases/SQLITE_PARITY_150_DOT_BACKUP_RESTORE_TEMPFILE.rs:13`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `tools`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `temp` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:54170d35e6bd711e829d8e8247b20926d02d9b6c4fc2cbbdfd9fd2b9399aef1f`
   Evidence: crates/bench/sqlite_parity/cases/SQLITE_PARITY_150_DOT_BACKUP_RESTORE_TEMPFILE.rs:13, future-hostile/dead-language term `temp` appears
14. `high` `vibe` `crates/bench/sqlite_parity/cases/SQLITE_PARITY_152_DOT_CLONE_TEMPFILE.rs:13`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `tools`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `temp` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:5adeb42f83175ed7af2cb96732d86031d3f433e0d39b383a6067e49931b157ab`
   Evidence: crates/bench/sqlite_parity/cases/SQLITE_PARITY_152_DOT_CLONE_TEMPFILE.rs:13, future-hostile/dead-language term `temp` appears
15. `high` `vibe` `crates/bench/sqlite_parity/cases/SQLITE_PARITY_153_DOT_CD_TEMPFILE.rs:13`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `tools`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `temp` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:462b3f99bbbac21761c8c525fc6be30d27e12c2da2147c442ed49595730118bc`
   Evidence: crates/bench/sqlite_parity/cases/SQLITE_PARITY_153_DOT_CD_TEMPFILE.rs:13, future-hostile/dead-language term `temp` appears
16. `high` `vibe` `crates/bench/sqlite_parity/cases/SQLITE_PARITY_154_DOT_DBINFO_TEMPFILE.rs:13`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `tools`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `temp` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:ae62354d52fad1645632f7086c215c430e305ed03a1aa65f9e4a878041cb0167`
   Evidence: crates/bench/sqlite_parity/cases/SQLITE_PARITY_154_DOT_DBINFO_TEMPFILE.rs:13, future-hostile/dead-language term `temp` appears
17. `high` `vibe` `crates/bench/sqlite_parity/cases/SQLITE_PARITY_192_OPT_INIT_TEMPFILE.rs:13`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `tools`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `temp` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:4248a4a2c1888f75645494221c0133afcd18c2b6edcf39b773abe6f11966958d`
   Evidence: crates/bench/sqlite_parity/cases/SQLITE_PARITY_192_OPT_INIT_TEMPFILE.rs:13, future-hostile/dead-language term `temp` appears
18. `high` `vibe` `crates/bench/sqlite_parity/cases/SQLITE_PARITY_193_OPT_READONLY_TEMPFILE.rs:13`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `tools`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `temp` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:088f106ae6e0fbf4d32ad46a9575281dc53c9b783cde761cfcbdcfcc7ec7064a`
   Evidence: crates/bench/sqlite_parity/cases/SQLITE_PARITY_193_OPT_READONLY_TEMPFILE.rs:13, future-hostile/dead-language term `temp` appears
19. `high` `vibe` `crates/bench/sqlite_parity/cases/SQLITE_PARITY_194_OPT_IFEXISTS_NEGATIVE_TEMPFILE.rs:13`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `tools`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `temp` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:1466dded868e20f3bc294b3d937ea541ae7734b61aaffbb86ee010a6700e005e`
   Evidence: crates/bench/sqlite_parity/cases/SQLITE_PARITY_194_OPT_IFEXISTS_NEGATIVE_TEMPFILE.rs:13, future-hostile/dead-language term `temp` appears
20. `high` `vibe` `crates/bench/sqlite_parity/cases/SQLITE_PARITY_197_OPT_MAXSIZE_DESERIALIZE_TEMPFILE.rs:13`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `tools`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `temp` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:8af8ac41899c5a656c3db6e93b60ee78430978ddb75207c32e0f195672665cdb`
   Evidence: crates/bench/sqlite_parity/cases/SQLITE_PARITY_197_OPT_MAXSIZE_DESERIALIZE_TEMPFILE.rs:13, future-hostile/dead-language term `temp` appears
21. `high` `vibe` `crates/bench/sqlite_parity/cases/SQLITE_PARITY_211_SQL_ATTACH_TEMPFILE_DATABASE.rs:13`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `tools`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `temp` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:9a3eaecd7759376d5014a4eca68d36493b78fc961202ed4c8f8f7da609ca097b`
   Evidence: crates/bench/sqlite_parity/cases/SQLITE_PARITY_211_SQL_ATTACH_TEMPFILE_DATABASE.rs:13, future-hostile/dead-language term `temp` appears
22. `high` `vibe` `crates/bench/sqlite_parity/cases/SQLITE_PARITY_212_SQL_VACUUM_INTO_TEMPFILE.rs:13`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `tools`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `temp` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:80aff3112d419b63005d4e73bb8d59bbb0a43d62c8f4ccf6cb0dc53f5589bb00`
   Evidence: crates/bench/sqlite_parity/cases/SQLITE_PARITY_212_SQL_VACUUM_INTO_TEMPFILE.rs:13, future-hostile/dead-language term `temp` appears
23. `high` `vibe` `crates/bench/sqlite_parity/cases/SQLITE_PARITY_213_SQL_WAL_CHECKPOINT_TEMPFILE.rs:13`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `tools`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `temp` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:d3e5799a68efffc7cb053ac5d0f2cab5293693e7aab20d1135ee415d6707eb7f`
   Evidence: crates/bench/sqlite_parity/cases/SQLITE_PARITY_213_SQL_WAL_CHECKPOINT_TEMPFILE.rs:13, future-hostile/dead-language term `temp` appears
24. `high` `vibe` `crates/bench/src/bin/chaos_report/main.rs:163`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `tools`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: fallback soup detected in product code
   Fix: collapse fallback chains into explicit typed states with bounded retry policy, telemetry, and documented repair guidance
   Rerun: `just fast`
   Fingerprint: `sha256:ab77b994862e1cba9b451d12303492c275e1cc8767e4a07ad302baf786dfdd00`
   Evidence: crates/bench/src/bin/chaos_report/main.rs:163 .unwrap_or_default()
25. `high` `copy-code` `crates/bench/src/feature_workloads.rs:639`
   Rule: `HLT-043-COPY-PASTE-BAD-BEHAVIOR`
   Check: `HLT-043-COPY-PASTE-BAD-BEHAVIOR:copy-code` `hard` confidence `0.95`
   Route: TLR `Maintainability entropy`, lane `copy-code`, owner `tools`
   Docs: `docs/BAD_COPY.md`
   Matched term: `checksum_from_rows`
   Reason: same-name semantic unit copied across multiple files
   Fix: keep the named unit in one owner and call it from the other sites
   Rerun: `cargo run -p jankurai -- copy-code . --json target/jankurai/copy-code.json --md target/jankurai/copy-code.md`
   Fingerprint: `sha256:c11dea32b68100d51d71ad5b7cf29f9e4a0cd9cf035c5e7868486eda405b2a04`
   Evidence: kind=ExactUnitSameName, language=rust, duplicate_lines=47, duplicate_tokens=172, duplicate_bytes=1836, instances=crates/bench/src/feature_workloads.rs:639-686, crates/bench/src/workload.rs:459-506
26. `high` `security` `crates/bench/src/process_metrics.rs:119`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `tools`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.zeroed`
   Reason: all-zero validity was not proven
   Fix: construct the type with a valid initializer instead of zeroing it
   Rerun: `just fast`
   Fingerprint: `sha256:aa96f629630b057efa0f1786eb528e79e8c6482d4ada20e64ec5019e02ebfcd0`
   Evidence: detector=zeroed, proof-window=NearbySafetyComment, snippet=let mut usage: libc::rusage = unsafe { std::mem::zeroed() };
27. `high` `security` `crates/bench/src/process_metrics.rs:127`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `tools`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:a48d8a99038147ce527ec88ff61047d549efc9b2f913f5f9c5ca6c9d2df660f3`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=let rc = unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut usage as *mut libc::rusage) };
28. `high` `stack` `crates/ffi/include/sqlite3.h`
   Check: `HLT-000-SCORE-DIMENSION:stack` `hard` confidence `0.88`
   Route: TLR `Context/setup`, lane `audit`, owner `tools`
   Reason: runtime code uses a language outside the chosen optimal stack
   Fix: move product runtime behavior to Rust core, TypeScript web, SQL migrations, or generated contracts; Python needs a dated advanced-ML/data exception
   Rerun: `just score`
   Fingerprint: `sha256:58ded11d14f8db77863cc7266cfcb5332d3a1068faf4bd43a3e1ea1dea7ed27a`
   Evidence: crates/ffi/include/sqlite3.h uses `.h`, Rust core + TypeScript/React/Vite + PostgreSQL + generated contracts + exception-only Python AI/data service
29. `high` `security` `crates/ffi/src/error.rs:42`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `tools`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:c93da4a982dc672a5605ff73cfe6bcc5cbac5445f7e49deffd735f851846a9a2`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=unsafe {
30. `high` `security` `crates/ffi/src/error.rs:43`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `tools`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.raw-parts`
   Reason: ownership provenance is missing
   Fix: use the matching constructor/destructor pair or add a documented ownership proof
   Rerun: `just fast`
   Fingerprint: `sha256:fb68a65ce20650b76a90bc88edfe06a198128b0d750b32b32f7c2a0727384404`
   Evidence: detector=CString::from_raw, proof-window=NearbySafetyComment, snippet=drop(CString::from_raw(ptr as *mut c_char));
31. `high` `security` `crates/ffi/src/lifecycle.rs:81`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `tools`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:0c336bebb472adeb8cd62352f231caf2314357706b37a0c936a91483d23d771a`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=unsafe {
32. `high` `security` `crates/ffi/src/lifecycle.rs:82`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `tools`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.raw-parts`
   Reason: ownership provenance is missing
   Fix: use the matching constructor/destructor pair or add a documented ownership proof
   Rerun: `just fast`
   Fingerprint: `sha256:bdbc8665d06771630a270b7628b5ce1d4ea68b943861e02146cd65daab567c1f`
   Evidence: detector=Box::from_raw, proof-window=NearbySafetyComment, snippet=drop(Box::from_raw(db));
33. `high` `security` `crates/ffi/src/snapshot.rs:94`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `tools`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:b347036fe1a347edd74341c921e2d343b3f975cbae79807321ed1d8b66412c39`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=unsafe {
34. `high` `security` `crates/ffi/src/snapshot.rs:95`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `tools`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.raw-parts`
   Reason: ownership provenance is missing
   Fix: use the matching constructor/destructor pair or add a documented ownership proof
   Rerun: `just fast`
   Fingerprint: `sha256:abc9d33162968df3c2f0a73e25bee873f289b0f867d2811f1feb4de200a60990`
   Evidence: detector=Box::from_raw, proof-window=NearbySafetyComment, snippet=drop(Box::from_raw(backup));
35. `high` `security` `crates/ffi/src/sqlite3_api/blob.rs:214`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `tools`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:ed2809a423d6faeb9c44e73134ec63c87c2af6068dcf28e83badbd75e9387908`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=let _ = unsafe { Box::from_raw(blob) };
36. `high` `security` `crates/ffi/src/sqlite3_api/collation.rs:112`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `tools`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:0df87d707956d3e26b654a87e3656a26d8c504ab7838a4de37b7b43bd82ec3f8`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=unsafe {
37. `high` `security` `crates/ffi/src/sqlite3_api/udf.rs:139`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `tools`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:f75a5b10dea2a415ce76d63815362fed2df4ee8573df5b75d89fbe3559bb0601`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=unsafe {
38. `high` `security` `crates/ffi/src/sqlite3_api/udf.rs:150`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `tools`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:d931c05f64fba820fd603013535bb6a38c2b8958ba2ab3bc8dce978493cf2162`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=let _ = unsafe { Box::from_raw(ptr) };
39. `high` `security` `crates/ffi/src/sqlite3_api/udf.rs:158`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `tools`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:ec8a5493aec13c69af26caf1463956c9a9c1731045a0fdb4fd9ed90a62192384`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=let ctx_ref = unsafe { &*ctx_ptr };
40. `high` `security` `crates/ffi/src/sqlite3_api/udf.rs:166`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `tools`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:8af1ba398db74bafc5ba13305b7bc42e408e2b7266216aa5712ac73670d41dc8`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=let _ = unsafe { Box::from_raw(ctx_ptr) };
41. `high` `security` `crates/ffi/src/sqlite3_api/udf.rs:175`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `tools`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:f75a5b10dea2a415ce76d63815362fed2df4ee8573df5b75d89fbe3559bb0601`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=unsafe {
42. `high` `security` `crates/ffi/src/sqlite3_api/udf.rs:185`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `tools`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:412eb4d329fc0cf05931b6c54313f969bfd285d6eb01bd2f3db862822cc66f28`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=let ctx_box = unsafe { Box::from_raw(ctx_ptr) };
43. `high` `security` `crates/ffi/src/sqlite3_api/udf.rs:234`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `tools`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:f75a5b10dea2a415ce76d63815362fed2df4ee8573df5b75d89fbe3559bb0601`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=unsafe {
44. `high` `security` `crates/ffi/src/sqlite3_api/udf.rs:243`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `tools`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:412eb4d329fc0cf05931b6c54313f969bfd285d6eb01bd2f3db862822cc66f28`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=let ctx_box = unsafe { Box::from_raw(ctx_ptr) };
45. `high` `security` `crates/ffi/src/sqlite3_api/udf.rs:252`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `tools`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:d931c05f64fba820fd603013535bb6a38c2b8958ba2ab3bc8dce978493cf2162`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=let _ = unsafe { Box::from_raw(ptr) };
46. `high` `security` `crates/ffi/src/sqlite3_api/udf.rs:295`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `tools`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:f75a5b10dea2a415ce76d63815362fed2df4ee8573df5b75d89fbe3559bb0601`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=unsafe {
47. `high` `security` `crates/ffi/src/sqlite3_api/udf.rs:325`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `tools`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:0e322fa4c99d6e5cec38e42e03ec78203b889be8211671771651b92dfe9a4872`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=let name = match unsafe { name_to_string(name) } {
48. `high` `security` `crates/ffi/src/sqlite3_api/udf.rs:375`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `tools`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:f75a5b10dea2a415ce76d63815362fed2df4ee8573df5b75d89fbe3559bb0601`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=unsafe {
49. `high` `security` `crates/ffi/src/sqlite3_api/udf.rs:419`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `tools`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:84cd48e1540a652aa0f49bff099546223baf77a177694fa3ccdb967175492e5b`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=let slice = unsafe { std::slice::from_raw_parts(ptr, len) };
50. `high` `security` `crates/ffi/src/sqlite3_api/udf.rs:432`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `tools`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:f75a5b10dea2a415ce76d63815362fed2df4ee8573df5b75d89fbe3559bb0601`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=unsafe {
51. `high` `security` `crates/ffi/src/stmt.rs:178`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `tools`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.raw-parts`
   Reason: ownership provenance is missing
   Fix: use the matching constructor/destructor pair or add a documented ownership proof
   Rerun: `just fast`
   Fingerprint: `sha256:b4f1a6ca90b064a685ea26de401a4ea01a4018f38fbe80c7f14d209db28ce468`
   Evidence: detector=Box::from_raw, proof-window=NearbySafetyComment, snippet=let boxed = unsafe { Box::from_raw(stmt) };
52. `high` `security` `crates/ffi/src/util.rs:157`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `tools`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:6e04cc93e77a387c83a93748c8a3651bdcd9fe78794dd8d741cc40e0a2de034d`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=unsafe { std::slice::from_raw_parts(ptr, len) }
53. `high` `vibe` `crates/sql/src/exec/attach.rs:66`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `tools`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `temp` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:3fe6c92be1d3e15233bbe0147ded0a573e8f92de381cd5bce03f4c69886ff41c`
   Evidence: crates/sql/src/exec/attach.rs:66, future-hostile/dead-language term `temp` appears
54. `high` `vibe` `crates/sql/src/exec/attach.rs:87`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `tools`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `temp` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:3edececb4f93d801a028e3bc1ebfbfe1147cd307c658574bf65400d955345cfc`
   Evidence: crates/sql/src/exec/attach.rs:87, future-hostile/dead-language term `temp` appears
55. `high` `security` `crates/sql/src/exec/mod.rs:704`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `tools`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:90bdf58bb1ca657de5b89f0dd907bd0cad0c8d6bffcb6196f4e43f8cc15577a3`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=let session_ref: &mut SessionState = unsafe { &mut *session_ptr };
56. `high` `security` `crates/sql/src/exec/mod.rs:705`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `tools`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:8b0de0dabe33b5278a3aa01cf20b5a0f422df7e74f32a84d2e1a476bdf36bd4b`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=let tx_ref: &mut Txn = unsafe { &mut *tx_ptr };
57. `high` `security` `crates/sql/src/exec/mod.rs:753`
   Rule: `HLT-029-RUST-BAD-BEHAVIOR`
   Check: `HLT-029-RUST-BAD-BEHAVIOR:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `fast`, owner `tools`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `rust.unsafe.undocumented-block`
   Reason: no nearby SAFETY comment was found
   Fix: add a precise `SAFETY:` comment or remove the unsafe block
   Rerun: `just fast`
   Fingerprint: `sha256:80d7b05f79961607fb46f21588e38a787868590be75485961d0a192bbbfff091`
   Evidence: detector=unsafe {, proof-window=NearbySafetyComment, snippet=let tx_ref = unsafe { &mut *tx_ptr };
58. `high` `vibe` `crates/sql/src/exec/trigger.rs:56`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `tools`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `old` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:cab48fe346b9eeb551a5d33e424db975487696667dc2794bb9b209dc78fa27bd`
   Evidence: crates/sql/src/exec/trigger.rs:56, future-hostile/dead-language term `old` appears
59. `high` `vibe` `crates/sql/src/exec/trigger.rs:80`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `tools`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `old` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:b1986cc5a4176cbd289c01d3aa1770906a7a00bae9cf0fff8ded63493e4f1797`
   Evidence: crates/sql/src/exec/trigger.rs:80, future-hostile/dead-language term `old` appears
60. `high` `vibe` `crates/sql/src/exec/trigger.rs:124`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `tools`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `old` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:71efcef919d07234c00c51b24fa22dacdaf63c3ca70644a5029ae9d608f49e3e`
   Evidence: crates/sql/src/exec/trigger.rs:124, future-hostile/dead-language term `old` appears
61. `high` `vibe` `crates/sql/src/exec/trigger.rs:135`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `tools`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `old` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:b0adaa6421da25e3db44a502722e6e83af57ce23eb92f46bd0fbff4cdc8ca30a`
   Evidence: crates/sql/src/exec/trigger.rs:135, future-hostile/dead-language term `old` appears
62. `high` `vibe` `crates/sql/src/exec/trigger.rs:144`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `tools`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `old` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:05c3cc7708281408962277fb3ea7cd4b8fec741ed0c3ad67a06c298778279cd6`
   Evidence: crates/sql/src/exec/trigger.rs:144, future-hostile/dead-language term `old` appears
63. `high` `vibe` `crates/sql/src/exec/trigger.rs:147`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `tools`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `old` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:481e2eea4fc08a5a116ac04e8e8b43e52bbb58175d8ff451e8420dfbb8b805e2`
   Evidence: crates/sql/src/exec/trigger.rs:147, future-hostile/dead-language term `old` appears
64. `high` `vibe` `crates/sql/src/parser/bind.rs:22`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `tools`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `temporary` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:8e84d46e0643be9cc351afac25862bc37dcc09a63c4916fc2912505971744960`
   Evidence: crates/sql/src/parser/bind.rs:22, future-hostile/dead-language term `temporary` appears
65. `high` `vibe` `crates/sql/src/parser/ddl.rs:729`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:vibe` `hard` confidence `0.88`
   Route: TLR `Entropy`, lane `fast`, owner `tools`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: future-hostile/dead-language term `temporary` appears in product/runtime code
   Fix: remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Rerun: `just fast`
   Fingerprint: `sha256:26cfc733c9b78bcda8c63ca207f726d948a15321f726324dc82d72eba423091a`
   Evidence: crates/sql/src/parser/ddl.rs:729, future-hostile/dead-language term `temporary` appears
66. `high` `python` `python/`
   Rule: `HLT-005-PYTHON-PRODUCT-TRUTH`
   Check: `HLT-005-PYTHON-PRODUCT-TRUTH:python` `hard` confidence `0.88`
   Route: TLR `Business truth`, lane `contract`, owner `tools`
   Docs: `docs/agent-native-standard.md`
   Reason: Python appears without a rare advanced-ML/data exception or owns product truth
   Fix: remove Python or box it under `python/ai-service` only when a dated advanced-ML/data exception exists; otherwise migrate it to Rust
   Rerun: `just fast`
   Fingerprint: `sha256:a9ed62b36b036c9cc50ca8f42844065f4bb44a12c1b6a715d99db7453617b019`
   Evidence: Python must stay away from product truth, repo tooling, proof lanes, backend glue, and production DB ownership
67. `medium` `python` `python/ai-service`
   Rule: `HLT-005-PYTHON-PRODUCT-TRUTH`
   Check: `HLT-005-PYTHON-PRODUCT-TRUTH:python` `soft` confidence `0.76`
   Route: TLR `Business truth`, lane `contract`, owner `tools`
   Docs: `docs/agent-native-standard.md`
   Reason: `Python containment and polyglot hygiene` scored 0 below the standard floor of 85
   Fix: remove Python unless it is a dated advanced-ML/data exception and move product truth into Rust, SQL, and generated contracts
   Rerun: `just fast`
   Fingerprint: `sha256:254f6154c00212a765420e3d814338ca1f80cc43eb7bfbaf48473348b617bf08`
   Evidence: Python appears outside the allowed roots: ops/ci/filter-generated-sarif.py, non-optimal product language marker: crates/ffi/include/sqlite3.h, Python leaks into product surface, runtime code should converge to Rust, TypeScript, SQL, generated contracts, and rare advanced-ML/data Python exceptions

## Policy

- Policy file: `./agent/audit-policy.toml`
- Minimum score: `85`
- Fail on: `critical, high`

## Agent Fix Queue

1. `high` `HLT-005-PYTHON-PRODUCT-TRUTH` `python/` - remove Python or box it under `python/ai-service` only when a dated advanced-ML/data exception exists; otherwise migrate it to Rust
   Route: `Business truth`/`contract`
2. `medium` `HLT-005-PYTHON-PRODUCT-TRUTH` `python/ai-service` - remove Python unless it is a dated advanced-ML/data exception and move product truth into Rust, SQL, and generated contracts
   Route: `Business truth`/`contract`
3. `medium` `HLT-018-PERF-CONCURRENCY-DRIFT` `Justfile` - add fast deterministic build/test targets, caches, and narrow proof lanes for agent iteration
   Route: `Verification`/`fast`
4. `high` `crates/ffi/include/sqlite3.h` - move product runtime behavior to Rust core, TypeScript web, SQL migrations, or generated contracts; Python needs a dated advanced-ML/data exception
   Route: `Context/setup`/`audit`
5. `medium` `HLT-040-REPO-ROT-BAD-BEHAVIOR` `crates/bench/sqlite_parity/cases/SQLITE_PARITY_150_DOT_BACKUP_RESTORE_TEMPFILE.rs` - delete the stale copy, move history to VCS/archive tooling, or document owner, proof lane, expiry, and migration plan
   Route: `Context/setup`/`audit`
6. `high` `HLT-024-AGENT-TOOL-SUPPLY-GAP` `.github/workflows/jankurai.yml` - pin and review agent tools, MCP servers, hooks, and rule files; keep untrusted tool output separate from trusted policy
   Route: `Security, secrets, agency`/`security`
7. `high` `HLT-023-INPUT-BOUNDARY-GAP` `crates/bench/sqlite_parity/cases/SQLITE_PARITY_001_SELECT_CORE_EXPRESSIONS.rs` - replace unsafe sinks with typed schemas, parameterized APIs, allowlists, or sandboxed execution plus negative tests
   Route: `Security, secrets, agency`/`security`
8. `high` `HLT-001-DEAD-MARKER` `crates/bench/sqlite_parity/cases/SQLITE_PARITY_042_TEMP_TABLE_TEMP_SCHEMA.rs` - remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Route: `Entropy`/`fast`
9. `high` `HLT-001-DEAD-MARKER` `crates/bench/sqlite_parity/cases/SQLITE_PARITY_096_DBSTAT_OPTIONAL.rs` - remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Route: `Entropy`/`fast`
10. `high` `HLT-001-DEAD-MARKER` `crates/bench/sqlite_parity/cases/SQLITE_PARITY_146_DOT_READ_TEMPFILE.rs` - remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Route: `Entropy`/`fast`
11. `high` `HLT-001-DEAD-MARKER` `crates/bench/sqlite_parity/cases/SQLITE_PARITY_147_DOT_IMPORT_CSV_TEMPFILE.rs` - remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Route: `Entropy`/`fast`
12. `high` `HLT-001-DEAD-MARKER` `crates/bench/sqlite_parity/cases/SQLITE_PARITY_148_DOT_OUTPUT_TEMPFILE.rs` - remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Route: `Entropy`/`fast`
13. `high` `HLT-001-DEAD-MARKER` `crates/bench/sqlite_parity/cases/SQLITE_PARITY_149_DOT_ONCE_TEMPFILE.rs` - remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Route: `Entropy`/`fast`
14. `high` `HLT-001-DEAD-MARKER` `crates/bench/sqlite_parity/cases/SQLITE_PARITY_150_DOT_BACKUP_RESTORE_TEMPFILE.rs` - remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Route: `Entropy`/`fast`
15. `high` `HLT-001-DEAD-MARKER` `crates/bench/sqlite_parity/cases/SQLITE_PARITY_152_DOT_CLONE_TEMPFILE.rs` - remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Route: `Entropy`/`fast`
16. `high` `HLT-001-DEAD-MARKER` `crates/bench/sqlite_parity/cases/SQLITE_PARITY_153_DOT_CD_TEMPFILE.rs` - remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Route: `Entropy`/`fast`
17. `high` `HLT-001-DEAD-MARKER` `crates/bench/sqlite_parity/cases/SQLITE_PARITY_154_DOT_DBINFO_TEMPFILE.rs` - remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Route: `Entropy`/`fast`
18. `high` `HLT-001-DEAD-MARKER` `crates/bench/sqlite_parity/cases/SQLITE_PARITY_192_OPT_INIT_TEMPFILE.rs` - remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Route: `Entropy`/`fast`
19. `high` `HLT-001-DEAD-MARKER` `crates/bench/sqlite_parity/cases/SQLITE_PARITY_193_OPT_READONLY_TEMPFILE.rs` - remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Route: `Entropy`/`fast`
20. `high` `HLT-001-DEAD-MARKER` `crates/bench/sqlite_parity/cases/SQLITE_PARITY_194_OPT_IFEXISTS_NEGATIVE_TEMPFILE.rs` - remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Route: `Entropy`/`fast`
21. `high` `HLT-001-DEAD-MARKER` `crates/bench/sqlite_parity/cases/SQLITE_PARITY_197_OPT_MAXSIZE_DESERIALIZE_TEMPFILE.rs` - remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Route: `Entropy`/`fast`
22. `high` `HLT-001-DEAD-MARKER` `crates/bench/sqlite_parity/cases/SQLITE_PARITY_211_SQL_ATTACH_TEMPFILE_DATABASE.rs` - remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Route: `Entropy`/`fast`
23. `high` `HLT-001-DEAD-MARKER` `crates/bench/sqlite_parity/cases/SQLITE_PARITY_212_SQL_VACUUM_INTO_TEMPFILE.rs` - remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Route: `Entropy`/`fast`
24. `high` `HLT-001-DEAD-MARKER` `crates/bench/sqlite_parity/cases/SQLITE_PARITY_213_SQL_WAL_CHECKPOINT_TEMPFILE.rs` - remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Route: `Entropy`/`fast`
25. `high` `HLT-001-DEAD-MARKER` `crates/bench/src/bin/chaos_report/main.rs` - collapse fallback chains into explicit typed states with bounded retry policy, telemetry, and documented repair guidance
   Route: `Entropy`/`fast`
26. `high` `HLT-043-COPY-PASTE-BAD-BEHAVIOR` `crates/bench/src/feature_workloads.rs` - keep the named unit in one owner and call it from the other sites
   Route: `Maintainability entropy`/`copy-code`
27. `high` `HLT-029-RUST-BAD-BEHAVIOR` `crates/bench/src/process_metrics.rs` - construct the type with a valid initializer instead of zeroing it
   Route: `Security, secrets, agency`/`fast`
28. `high` `HLT-029-RUST-BAD-BEHAVIOR` `crates/bench/src/process_metrics.rs` - add a precise `SAFETY:` comment or remove the unsafe block
   Route: `Security, secrets, agency`/`fast`
29. `high` `HLT-029-RUST-BAD-BEHAVIOR` `crates/ffi/src/error.rs` - add a precise `SAFETY:` comment or remove the unsafe block
   Route: `Security, secrets, agency`/`fast`
30. `high` `HLT-029-RUST-BAD-BEHAVIOR` `crates/ffi/src/error.rs` - use the matching constructor/destructor pair or add a documented ownership proof
   Route: `Security, secrets, agency`/`fast`
31. `high` `HLT-029-RUST-BAD-BEHAVIOR` `crates/ffi/src/lifecycle.rs` - add a precise `SAFETY:` comment or remove the unsafe block
   Route: `Security, secrets, agency`/`fast`
32. `high` `HLT-029-RUST-BAD-BEHAVIOR` `crates/ffi/src/lifecycle.rs` - use the matching constructor/destructor pair or add a documented ownership proof
   Route: `Security, secrets, agency`/`fast`
33. `high` `HLT-029-RUST-BAD-BEHAVIOR` `crates/ffi/src/snapshot.rs` - add a precise `SAFETY:` comment or remove the unsafe block
   Route: `Security, secrets, agency`/`fast`
34. `high` `HLT-029-RUST-BAD-BEHAVIOR` `crates/ffi/src/snapshot.rs` - use the matching constructor/destructor pair or add a documented ownership proof
   Route: `Security, secrets, agency`/`fast`
35. `high` `HLT-029-RUST-BAD-BEHAVIOR` `crates/ffi/src/sqlite3_api/blob.rs` - add a precise `SAFETY:` comment or remove the unsafe block
   Route: `Security, secrets, agency`/`fast`
36. `high` `HLT-029-RUST-BAD-BEHAVIOR` `crates/ffi/src/sqlite3_api/collation.rs` - add a precise `SAFETY:` comment or remove the unsafe block
   Route: `Security, secrets, agency`/`fast`
37. `high` `HLT-029-RUST-BAD-BEHAVIOR` `crates/ffi/src/sqlite3_api/udf.rs` - add a precise `SAFETY:` comment or remove the unsafe block
   Route: `Security, secrets, agency`/`fast`
38. `high` `HLT-029-RUST-BAD-BEHAVIOR` `crates/ffi/src/stmt.rs` - use the matching constructor/destructor pair or add a documented ownership proof
   Route: `Security, secrets, agency`/`fast`
39. `high` `HLT-029-RUST-BAD-BEHAVIOR` `crates/ffi/src/util.rs` - add a precise `SAFETY:` comment or remove the unsafe block
   Route: `Security, secrets, agency`/`fast`
40. `high` `HLT-001-DEAD-MARKER` `crates/sql/src/exec/attach.rs` - remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Route: `Entropy`/`fast`
41. `high` `HLT-029-RUST-BAD-BEHAVIOR` `crates/sql/src/exec/mod.rs` - add a precise `SAFETY:` comment or remove the unsafe block
   Route: `Security, secrets, agency`/`fast`
42. `high` `HLT-001-DEAD-MARKER` `crates/sql/src/exec/trigger.rs` - remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Route: `Entropy`/`fast`
43. `high` `HLT-001-DEAD-MARKER` `crates/sql/src/parser/bind.rs` - remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Route: `Entropy`/`fast`
44. `high` `HLT-001-DEAD-MARKER` `crates/sql/src/parser/ddl.rs` - remove or rename the marker, implement the intended behavior, model a typed unsupported state, or move docs/generated/vendor/product-copy text into an allowlisted context
   Route: `Entropy`/`fast`
45. `medium` `HLT-001-DEAD-MARKER` `.` - split large or ambiguous authored code into smaller semantic modules with focused tests
   Route: `Entropy`/`fast`
