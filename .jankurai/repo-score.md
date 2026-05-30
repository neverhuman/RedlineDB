# jankurai Repo Score

- Standard: `jankurai`
- Auditor: `1.5.1`
- Schema: `1.9.0`
- Paper edition: `2026.05-ed8`
- Target stack ID: `rust-ts-vite-react-postgres-bounded-python`
- Target stack: `Rust core + TypeScript/React/Vite + PostgreSQL + generated contracts + exception-only Python AI/data service`
- Repo: `.`
- Run ID: `1779985630`
- Started at: `1779985630`
- Elapsed: `14800` ms
- Scope: `full`
- Raw score: `81`
- Final score: `81`
- Decision: `advisory`
- Minimum score: `85`
- Caps applied: `missing-web-e2e-lane, missing-rendered-ux-qa-lane`

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
| `missing-web-e2e-lane` | 82 | yes |
| `missing-rendered-ux-qa-lane` | 84 | yes |
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

- Status: `review` hard=`0` warning=`124` files=`362`
- Policy: min-lines=`10` min-tokens=`100` max-findings=`50` include-tests=`false` strict=`false`
- Duplicate volume: lines=`238` tokens=`744` bytes=`6850`

- Notes:
  - hard classes are limited to exact active-source file matches and substantial exact same-name units
  - warning classes include same-body different-name units and token/block duplication
  - tests, fixtures, stories, config, Docker, and migrations are omitted unless --include-tests is set
  - showing the top 50 classes and omitting 74 lower-ranked classes

| Kind | Severity | Language | Lines | Tokens | Instances | Reason |
| --- | --- | --- | ---: | ---: | --- | --- |
| `ExactUnitSameName` | `Warning` | `rust` | 5 | 9 | `crates/sql/src/exec/json_tv.rs:228-233, crates/sql/src/exec/json_tv.rs:283-288, crates/sql/src/exec/json_tv.rs:399-404, crates/sql/src/exec/json_tv.rs:433-438, crates/sql/src/exec/json_tv.rs:467-472, crates/sql/src/exec/json_tv.rs:504-509, crates/sql/src/exec/json_tv.rs:541-546, crates/sql/src/exec/json_tv.rs:578-583, crates/sql/src/exec/json_tv.rs:616-621, crates/sql/src/exec/pragma_tv.rs:220-225, crates/sql/src/exec/pragma_tv.rs:243-248, crates/sql/src/exec/pragma_tv.rs:282-287, crates/sql/src/exec/pragma_tv.rs:370-375, crates/sql/src/exec/table_valued.rs:77-82` | `same-name semantic unit copied across multiple files` |
| `ExactUnitSameName` | `Warning` | `rust` | 13 | 42 | `crates/sql/src/exec/cross_db.rs:203-216, crates/sql/src/exec/cte.rs:181-194, crates/sql/src/exec/view.rs:180-193` | `same-name semantic unit copied across multiple files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 7 | 16 | `crates/cli/src/render.rs:472-479, crates/cli/src/render.rs:591-598, crates/cli/src/render.rs:652-659, crates/cli/src/render.rs:726-733` | `same body appears under different names across files` |
| `ExactUnitSameName` | `Warning` | `rust` | 5 | 9 | `crates/sql/src/exec/pragma_tv.rs:76-81, crates/sql/src/exec/pragma_tv.rs:108-113, crates/sql/src/exec/pragma_tv.rs:134-139, crates/sql/src/exec/pragma_tv.rs:429-434, crates/sql/src/exec/pragma_tv.rs:463-468` | `same-name semantic unit copied across multiple files` |
| `ExactUnitSameName` | `Warning` | `rust` | 16 | 37 | `crates/sql/src/exec/agg/select.rs:85-101, crates/sql/src/planner/access/projection.rs:153-169` | `same-name semantic unit copied across multiple files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 2 | 3 | `crates/cli/src/dot/control.rs:112-114, crates/cli/src/dot/control.rs:116-118, crates/cli/src/dot/control.rs:120-122, crates/cli/src/dot/control.rs:124-126, crates/cli/src/dot/control.rs:137-139, crates/cli/src/dot/control.rs:291-293, crates/cli/src/dot/control.rs:374-376` | `same body appears under different names across files` |
| `ExactUnitSameName` | `Warning` | `rust` | 11 | 57 | `crates/sql/src/datetime/format.rs:75-86, crates/sql/src/datetime/modifiers.rs:193-204` | `same-name semantic unit copied across multiple files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 4 | `crates/sql/src/parser.rs:1788-1789, crates/sql/src/parser.rs:1893-1894, crates/sql/src/parser.rs:1992-1993, crates/sql/src/parser.rs:2468-2469, crates/sql/src/parser.rs:2620-2621, crates/sql/src/parser.rs:2663-2664, crates/sql/src/parser.rs:2808-2809, crates/sql/src/parser.rs:3118-3119, crates/sql/src/parser.rs:3166-3167, crates/sql/src/parser.rs:3396-3397, crates/sql/src/parser/split.rs:33-34, crates/sql/src/parser/split.rs:171-172` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 2 | `crates/kernel/src/catalog/record.rs:152-153, crates/kernel/src/catalog/stats/wire.rs:162-163, crates/kernel/src/catalog/stats/wire.rs:173-174, crates/redlinedb-sqlx/src/bridge/runtime.rs:390-391, crates/redlinedb-sqlx/src/bridge/runtime.rs:487-488, crates/redlinedb/src/value_conv.rs:261-262, crates/sql/src/exec/expr/coerce/binary.rs:396-397, crates/sql/src/exec/expr/coerce/binary.rs:403-404, crates/sql/src/exec/expr/coerce/binary.rs:512-513, crates/sql/src/exec/expr/json_dispatch.rs:765-766, crates/sql/src/exec/json_tv.rs:241-242` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 2 | `crates/redlinedb/src/value.rs:43-44, crates/redlinedb/src/value.rs:57-58, crates/redlinedb/src/value.rs:64-65, crates/redlinedb/src/value.rs:71-72, crates/redlinedb/src/value.rs:78-79, crates/sql/src/exec/expr/scalar/row/model.rs:67-68, crates/sql/src/exec/expr/scalar/row/model.rs:81-82, crates/sql/src/exec/expr/scalar/row/model.rs:92-93` | `same body appears under different names across files` |
| `ExactUnitSameName` | `Warning` | `rust` | 7 | 12 | `crates/kernel/src/engine/page_heap/policy.rs:78-85, crates/kernel/src/engine/page_heap/policy.rs:108-115` | `same-name semantic unit copied across multiple files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 6 | `crates/redlinedb-sqlx/src/bridge/options.rs:223-224, crates/redlinedb-sqlx/src/bridge/runtime.rs:52-53, crates/redlinedb-sqlx/src/bridge/runtime.rs:57-58, crates/redlinedb-sqlx/src/bridge/runtime.rs:81-82, crates/redlinedb-sqlx/src/bridge/runtime.rs:96-97, crates/redlinedb-sqlx/src/bridge/runtime.rs:111-112, crates/redlinedb-sqlx/src/bridge/runtime.rs:201-202` | `same body appears under different names across files` |
| `ExactUnitSameName` | `Warning` | `rust` | 6 | 21 | `crates/cli/src/shellzero.rs:217-223, crates/redlinedb-lite/src/shellzero.rs:216-222` | `same-name semantic unit copied across multiple files` |
| `ExactUnitSameName` | `Warning` | `rust` | 6 | 20 | `crates/sql/src/exec/agg/select.rs:9-15, crates/sql/src/planner/access/projection.rs:94-100` | `same-name semantic unit copied across multiple files` |
| `ExactUnitSameName` | `Warning` | `rust` | 6 | 12 | `crates/sql/src/exec/policy.rs:32-38, crates/sql/src/exec/policy.rs:57-63` | `same-name semantic unit copied across multiple files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 0 | `crates/kernel/src/catalog/ddl.rs:313-313, crates/kernel/src/failpoints/mod.rs:41-42, crates/kernel/src/integrity/equivalence.rs:214-214, crates/kernel/src/integrity/page_csum.rs:107-107, crates/redlinedb-sqlx/src/bridge/options.rs:253-254, crates/redlinedb-sqlx/src/bridge/runtime.rs:127-128, crates/sql/src/exec/merge.rs:278-278` | `same body appears under different names across files` |
| `ExactUnitSameName` | `Warning` | `rust` | 5 | 13 | `crates/sql/src/exec/agg/select.rs:103-108, crates/sql/src/planner/access/projection.rs:171-176` | `same-name semantic unit copied across multiple files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 5 | 12 | `crates/cli/src/render.rs:979-984, crates/cli/src/render.rs:1003-1008` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 5 | 10 | `crates/cli/src/render.rs:459-464, crates/cli/src/render.rs:549-554` | `same body appears under different names across files` |
| `ExactUnitSameName` | `Warning` | `rust` | 4 | 13 | `crates/kernel/src/index/locks.rs:200-204, crates/sql/src/session.rs:459-463` | `same-name semantic unit copied across multiple files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 4 | 12 | `crates/sql/src/exec/expr/json_dispatch.rs:62-66, crates/sql/src/exec/expr/json_dispatch.rs:822-826` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 2 | 5 | `crates/kernel/src/format/bytes.rs:44-46, crates/kernel/src/format/bytes.rs:49-51, crates/kernel/src/format/bytes.rs:54-56` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 4 | 9 | `crates/sql/src/exec/expr/scalar/row/lookup.rs:68-72, crates/sql/src/exec/expr/scalar/row/lookup.rs:137-141` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 2 | 3 | `crates/kernel/src/format/page.rs:102-104, crates/kernel/src/storage/control.rs:147-149, crates/kernel/src/storage/tx_status_checkpoint.rs:147-149` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 4 | 5 | `crates/redlinedb/src/connection.rs:154-158, crates/redlinedb/src/connection.rs:169-173` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 2 | 1 | `crates/kernel/src/failpoints/mod.rs:65-67, crates/kernel/src/failpoints/mod.rs:109-111, crates/kernel/src/storage/numa.rs:47-49` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 3 | 16 | `crates/sql/src/exec/expr/scalar/value.rs:556-559, crates/sql/src/exec/expr/scalar/value.rs:710-713` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 4 | `crates/sql/src/parser.rs:1239-1240, crates/sql/src/parser.rs:1262-1263, crates/sql/src/parser.rs:3022-3023, crates/sql/src/parser.rs:3086-3087` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 3 | `crates/sql/src/rql.rs:1634-1635, crates/sql/src/rql.rs:1719-1720, crates/sql/src/rql.rs:1856-1857, crates/sql/src/rql.rs:1880-1881` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 3 | 7 | `crates/sql/src/exec/expr/scalar/value.rs:321-324, crates/sql/src/exec/expr/scalar/value.rs:369-372` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 3 | 6 | `crates/sql/src/parser.rs:1304-1307, crates/sql/src/parser.rs:1332-1335` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 2 | `crates/sql/src/exec/expr/program.rs:970-971, crates/sql/src/exec/expr/program.rs:1027-1028, crates/sql/src/exec/expr/program.rs:1042-1043, crates/sql/src/exec/expr/program.rs:1096-1097` | `same body appears under different names across files` |
| `ExactUnitSameName` | `Warning` | `rust` | 3 | 4 | `crates/kernel/src/vector/flat.rs:57-60, crates/kernel/src/vector/hnsw/searcher.rs:47-50` | `same-name semantic unit copied across multiple files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 1 | `crates/bench/src/sqlite_parity/engine.rs:172-173, crates/redlinedb-sqlx/src/driver.rs:216-217, crates/sql/src/exec/expr/program.rs:372-373, crates/sql/src/exec/morsel/hash_agg.rs:301-302` | `same body appears under different names across files` |
| `ExactUnitSameName` | `Warning` | `rust` | 3 | 2 | `crates/kernel/src/engine/tx/status.rs:306-309, crates/sql/src/exec/hot_row.rs:576-579` | `same-name semantic unit copied across multiple files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 7 | `crates/sql/src/exec/expr/json_dispatch.rs:960-961, crates/sql/src/exec/expr/json_dispatch.rs:972-973, crates/sql/src/exec/expr/json_dispatch.rs:985-986` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 6 | `crates/redlinedb-sqlx/src/driver.rs:399-400, crates/redlinedb-sqlx/src/driver.rs:414-415, crates/redlinedb-sqlx/src/driver.rs:429-430` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 5 | `crates/cli/src/render.rs:410-411, crates/cli/src/render.rs:430-431, crates/cli/src/render.rs:444-445` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 5 | `crates/cli/src/render.rs:641-642, crates/cli/src/render.rs:766-767, crates/cli/src/render.rs:777-778` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 4 | `crates/redlinedb-sqlx/src/bridge/options.rs:249-250, crates/redlinedb-sqlx/src/bridge/options.rs:255-256, crates/redlinedb-sqlx/src/bridge/runtime.rs:129-130` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 4 | `crates/sql/src/exec/morsel/arena.rs:75-76, crates/sql/src/exec/morsel/arena.rs:91-92, crates/sql/src/exec/morsel/column.rs:74-75` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 4 | `crates/sql/src/exec/expr/json_dispatch.rs:623-624, crates/sql/src/exec/expr/json_dispatch.rs:644-645, crates/sql/src/exec/expr/json_dispatch.rs:653-654` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 3 | `crates/redlinedb/src/connection.rs:303-304, crates/redlinedb/src/connection.rs:312-313, crates/redlinedb/src/connection.rs:322-323` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 2 | 5 | `crates/kernel/src/json/path_bytecode.rs:138-140, crates/sql/src/json/jsonb.rs:1062-1064` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 2 | `crates/redlinedb/src/connection.rs:52-53, crates/redlinedb/src/connection.rs:182-183, crates/redlinedb/src/connection.rs:188-189` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 2 | `crates/cli/src/dot/mod.rs:367-368, crates/cli/src/shellzero.rs:153-154, crates/redlinedb-lite/src/shellzero.rs:154-155` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 2 | `crates/sql/src/exec/expr/json_dispatch.rs:914-915, crates/sql/src/exec/expr/json_dispatch.rs:935-936, crates/sql/src/exec/table_valued.rs:120-121` | `same body appears under different names across files` |
| `ExactUnitSameName` | `Warning` | `rust` | 2 | 3 | `crates/redlinedb-tokio/src/lib.rs:268-270, crates/redlinedb/src/pool.rs:199-201` | `same-name semantic unit copied across multiple files` |
| `ExactUnitSameName` | `Warning` | `rust` | 2 | 3 | `crates/redlinedb-tokio/src/lib.rs:263-265, crates/redlinedb/src/pool.rs:204-206` | `same-name semantic unit copied across multiple files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 1 | `crates/bench/src/sqlite_parity/report_gen/io.rs:209-210, crates/cli/src/dot/control.rs:558-559, crates/cli/src/render.rs:1120-1121` | `same body appears under different names across files` |

## Dimensions

| Dimension | Weight | Score | Weighted | Evidence |
| --- | ---: | ---: | ---: | --- |
| Ownership and navigation surface | 13 | 100 | 13.00 | root `AGENTS.md` present; `CODEOWNERS` present |
| Contract and boundary integrity | 13 | 88 | 11.44 | contract surface found; generated contract artifacts found |
| Proof lanes and test routing | 12 | 68 | 8.16 | one-command setup/validation lane found; deterministic fast lane found |
| Security and supply-chain posture | 12 | 86 | 10.32 | lockfile present; secret or dependency scan tooling found |
| Code shape and semantic surface | 12 | 45 | 5.40 | largest authored code file: crates/sql/src/parser.rs (5237 LOC); code file exceeds 500 LOC |
| Data truth and workflow safety | 8 | 85 | 6.80 | database surface present; migration directory present |
| Observability and repair evidence | 8 | 88 | 7.04 | observability libraries or patterns found; ops/observability directory present |
| Context economy and agent instructions | 7 | 91 | 6.37 | root `AGENTS.md` present; root `AGENTS.md` stays short |
| Jankurai tool adoption and CI replacement | 7 | 76 | 5.32 | control-plane files present; applicable=17 |
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

- Web surface: `true`
- Layered UX lane: `false`
- Missing: `Storybook state coverage, Playwright screenshot capture, visual review or geometry runtime, accessibility automation, generated API mocks, design token discipline`

## Tool Adoption

- Control plane present: `true`
- Applicable tools: `17`
- Configured: `0`
- CI evidence: `16`
- Artifact verified: `16`
- Replaced count: `16`
- Missing CI evidence: `ux-qa`

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
| `ux-qa` | `ux` | `auto` | `missing` | `playwright, axe-core, visual baselines` | `target/jankurai/ux-qa.json` |
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

## Security evidence (ingested)

- Source: `target/jankurai/security/evidence.json`
- Envelope exit code: `0` · elapsed: `53839` ms · strict: `false`
- Commands — ran: `1`, skipped: `0`, failed: `0`
- Generated at: `1779631279`
- Git HEAD (envelope): `37342ced9d7a1b9dd18891e12f2a28b6963cae53`

## Boundary Reclassifications

No audited runtime boundary reclassifications declared.

## Findings

1. `medium` `shape` `.`
   Rule: `HLT-001-DEAD-MARKER`
   Check: `HLT-001-DEAD-MARKER:shape` `soft` confidence `0.76`
   Route: TLR `Entropy`, lane `fast`, owner `tools`
   Docs: `docs/audit-rubric.md#future-hostile-language-rule`
   Reason: `Code shape and semantic surface` scored 45 below the standard floor of 85
   Fix: split large or ambiguous authored code into smaller semantic modules with focused tests
   Rerun: `just fast`
   Fingerprint: `sha256:07dfa22c0d8e4e44ddaca2d6607727e9413c20918a90bc35739369dd74b82ce0`
   Evidence: largest authored code file: crates/sql/src/parser.rs (5237 LOC), code file exceeds 500 LOC, code file exceeds 1000 LOC, most code files stay under 300 LOC
2. `medium` `proof` `Justfile`
   Rule: `HLT-018-PERF-CONCURRENCY-DRIFT`
   Check: `HLT-018-PERF-CONCURRENCY-DRIFT:proof` `soft` confidence `0.76`
   Route: TLR `Verification`, lane `fast`, owner `workspace`
   Docs: `docs/testing.md`
   Reason: `Build speed signals` scored 80 below the standard floor of 85
   Fix: add fast deterministic build/test targets, caches, and narrow proof lanes for agent iteration
   Rerun: `just fast`
   Fingerprint: `sha256:2f2531223d7f7036c20d44b58cd52e64aa53ffd6cb85e01e541c1feff0c09cb2`
   Evidence: build acceleration markers found, targeted test/build commands found, locked dependency graph present, CI cache hint found
3. `medium` `proof` `agent/test-map.json`
   Rule: `HLT-004-UNMAPPED-PROOF`
   Check: `HLT-004-UNMAPPED-PROOF:proof` `soft` confidence `0.76`
   Route: TLR `Verification`, lane `fast`, owner `agent`
   Docs: `agent/JANKURAI_STANDARD.md#proof-lanes`
   Reason: `Proof lanes and test routing` scored 68 below the standard floor of 85
   Fix: route each owned path to a deterministic proof command and make the lane executable in CI
   Rerun: `just fast`
   Fingerprint: `sha256:8d38851eaf6241cef21b313f9b9cf67d3a77b44853346b2f7ff3d17dc65eb69f`
   Evidence: one-command setup/validation lane found, deterministic fast lane found, test runner present in automation surface, GitHub workflow files present
4. `high` `test` `apps/web`
   Rule: `HLT-013-RENDERED-UX-GAP`
   Check: `HLT-013-RENDERED-UX-GAP:test` `hard` confidence `0.88`
   Route: TLR `Verification and rendered UX`, lane `web`, owner `tools`
   Docs: `docs/testing.md`
   Reason: web surface lacks a Playwright/Cypress e2e lane
   Fix: add Playwright e2e tests for critical user flows and wire them into the fast or CI proof map
   Rerun: `just ux-qa`
   Fingerprint: `sha256:baba171f944c5384a440bd31dbe0783c2a295323e516450d3b28505654cb406d`
   Evidence: web surface detected
5. `high` `ux-qa` `apps/web`
   Rule: `HLT-013-RENDERED-UX-GAP`
   Check: `HLT-013-RENDERED-UX-GAP:ux-qa` `hard` confidence `0.88`
   Route: TLR `Verification and rendered UX`, lane `web`, owner `tools`
   Docs: `docs/testing.md`
   Reason: web surface lacks layered rendered UX QA evidence
   Fix: add Storybook state coverage, Playwright screenshots, visual review or `@jankurai/ux-qa`, accessibility scans, CLS checks, generated mocks, and design tokens
   Rerun: `just ux-qa`
   Fingerprint: `sha256:571d35c2e730a393b782bac14825b197c0543920bb21967079d264ac602ea5b1`
   Evidence: rendered UX QA lane missing

## Policy

- Policy file: `./agent/audit-policy.toml`
- Minimum score: `85`
- Fail on: `critical, high`

## Agent Fix Queue

1. `medium` `HLT-018-PERF-CONCURRENCY-DRIFT` `Justfile` - add fast deterministic build/test targets, caches, and narrow proof lanes for agent iteration
   Route: `Verification`/`fast`
2. `medium` `HLT-004-UNMAPPED-PROOF` `agent/test-map.json` - route each owned path to a deterministic proof command and make the lane executable in CI
   Route: `Verification`/`fast`
3. `high` `HLT-013-RENDERED-UX-GAP` `apps/web` - add Playwright e2e tests for critical user flows and wire them into the fast or CI proof map
   Route: `Verification and rendered UX`/`web`
4. `high` `HLT-013-RENDERED-UX-GAP` `apps/web` - add Storybook state coverage, Playwright screenshots, visual review or `@jankurai/ux-qa`, accessibility scans, CLS checks, generated mocks, and design tokens
   Route: `Verification and rendered UX`/`web`
5. `medium` `HLT-001-DEAD-MARKER` `.` - split large or ambiguous authored code into smaller semantic modules with focused tests
   Route: `Entropy`/`fast`
